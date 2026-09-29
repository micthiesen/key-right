#!/usr/bin/env python3
"""Watch macOS USB, USB-C/accessory, driver and serial events without touching devices.

Run while trying cables, ports, BOOT or RESET. Ctrl-C stops capture. No serial
port is opened and no device command, reset or flash write is sent. Captures
include hardware identifiers and stay in a private, Git-ignored local folder.
"""

import argparse
import datetime
import glob
import json
import math
import os
from pathlib import Path
import platform
import plistlib
import signal
import subprocess
import sys
import tempfile
import threading
import time


ROOT = Path(__file__).resolve().parent.parent
CLASSES = (
    "IOUSBHostDevice", "IOUSBDevice", "IOUSBHostInterface",
    "AppleUSBHostPort", "IOAccessoryManager", "IOSerialBSDClient",
)
# These are reference-count/bookkeeping noise, not USB connection state.
IGNORED = {
    "IORegistryEntryChildren", "IOObjectRetainCount", "IOServiceBusyTime",
    "IOGeneralInterest", "IOBusyInterest", "IOReportLegend", "kPortStatPowerStateTime",
}
LOG_PREDICATE = (
    'subsystem CONTAINS[c] "usb" OR subsystem CONTAINS[c] "accessor" '
    'OR subsystem CONTAINS[c] "typec" '
    'OR process IN {"accessoryd", "usbd", "usbmuxd", "ioud"} '
    'OR (process == "kernel" AND (eventMessage CONTAINS[c] "USB" '
    'OR eventMessage CONTAINS[c] "TypeC" OR eventMessage CONTAINS[c] "Type-C" '
    'OR eventMessage CONTAINS[c] "AppleHPM" '
    'OR eventMessage CONTAINS[c] "Accessory"))'
)


def timestamp():
    return datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="milliseconds")


def normalize(value):
    if isinstance(value, dict):
        return {key: normalize(item) for key, item in value.items() if key not in IGNORED}
    if isinstance(value, list):
        return [normalize(item) for item in value]
    if isinstance(value, bytes):
        return "0x" + value.hex()
    if isinstance(value, datetime.datetime):
        return value.isoformat()
    return value


def registry_snapshot(class_name):
    result = subprocess.run(
        ["ioreg", "-a", "-r", "-c", class_name, "-d", "1"],
        capture_output=True, timeout=3, check=True,
    )
    # ioreg exits successfully with empty stdout when no object matches.
    rows = plistlib.loads(result.stdout) if result.stdout.strip() else []
    if not isinstance(rows, list) or any(not isinstance(row, dict) for row in rows):
        raise ValueError("ioreg returned an unexpected plist structure")
    return {str(row["IORegistryEntryID"]): normalize(row) for row in rows}


def port_snapshot():
    return {path: {"path": path} for pattern in ("/dev/cu.*", "/dev/tty.*")
            for path in glob.glob(pattern)}


def changes(previous, current):
    """An empty successful snapshot means removal; a failed read must never call this."""
    for key in sorted(previous.keys() - current.keys()):
        yield "REMOVED", key, previous[key]
    for key in sorted(current.keys() - previous.keys()):
        yield "ADDED", key, current[key]
    for key in sorted(previous.keys() & current.keys()):
        before, after = previous[key], current[key]
        delta = {name: {"before": before.get(name), "after": after.get(name)}
                 for name in sorted(before.keys() | after.keys())
                 if before.get(name) != after.get(name)}
        if delta:
            yield "CHANGED", key, delta


def short(value, limit=100):
    text = " ".join(str(value).split())
    text = "".join(char for char in text if char.isprintable())
    return text if len(text) <= limit else text[:limit - 3] + "..."


def describe(source, event, record, delta):
    """Only human-useful connection information belongs in terminal output."""
    action = {"BASELINE": "Present", "ADDED": "Connected", "REMOVED": "Disconnected",
              "CHANGED": "Changed"}[event]
    if source == "serial nodes":
        path = record["path"]
        if not path.startswith("/dev/cu.") or path in (
                "/dev/cu.debug-console", "/dev/cu.Bluetooth-Incoming-Port"):
            return None
        return f"{action}: serial {short(path)}"
    if source in ("IOUSBHostDevice", "IOUSBDevice"):
        name = record.get("USB Product Name") or record.get("IORegistryEntryName") or "USB device"
        ids = [record.get("idVendor"), record.get("idProduct")]
        identity = f" ({ids[0]:04x}:{ids[1]:04x})" if all(isinstance(v, int) for v in ids) else ""
        return f"{action}: {short(name)}{identity}"
    if source not in ("IOAccessoryManager", "AppleUSBHostPort"):
        return None
    name = short(record.get("PortDescription") or record.get("IORegistryEntryName") or source, 60)
    if event == "BASELINE":
        if source == "IOAccessoryManager" and (record.get("ConnectionActive") or record.get("IOAccessoryUSBActive")):
            return f"Present: USB-C/accessory connection at {name}"
        return None
    if event != "CHANGED":
        return f"{action}: {name}"
    labels = {
        "ConnectionActive": "connected", "IOAccessoryUSBActive": "USB active",
        "IOAccessoryUSBConnectString": "USB mode", "PlugOrientation": "orientation",
        "UserAuthorizationStatusDescription": "permission",
        "AuthorizationRequired": "approval required", "Plug Event Count": "plug count",
        "ConnectionCount": "connection count", "Overcurrent Count": "overcurrent count",
        "port-status": "port status",
    }
    details = []
    if "UsbTransportState" in delta:
        attached = bool(delta["UsbTransportState"]["after"])
        details.append("USB transport attached" if attached else "USB transport detached")
    for key, label in labels.items():
        if key in delta:
            value = delta[key]["after"]
            if isinstance(value, bool):
                value = "yes" if value else "no"
            if key == "port-status" and isinstance(value, int):
                value = hex(value)
            details.append(f"{label}={short(value, 45)}")
    stats = delta.get("port-statistics", {})
    for key, label in (("kPortStatConnectCount", "connections"),
                       ("kPortStatEnumerationFailureCount", "enumeration failures"),
                       ("kPortStatOverCurrentCount", "overcurrent events")):
        before, after = stats.get("before") or {}, stats.get("after") or {}
        if before.get(key) != after.get(key):
            details.append(f"{label}={short(after.get(key), 20)}")
    return short(f"{name}: {', '.join(details)}", 240) if details else None


class Capture:
    def __init__(self, folder):
        self.folder = folder
        self.lock = threading.Lock()
        self.files = {}
        try:
            for name in ("events.log", "snapshots.jsonl", "macos.log"):
                fd = os.open(folder / name, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
                self.files[name] = os.fdopen(fd, "w", buffering=1)
        except OSError:
            self.close()
            raise

    def emit(self, source, message):
        line = f"{timestamp()} [{source}] {message}"
        with self.lock:
            self.files["events.log"].write(line + "\n")
            print(line, flush=True)

    def snapshot(self, source, records):
        with self.lock:
            self.files["snapshots.jsonl"].write(json.dumps(
                {"time": timestamp(), "source": source, "records": records},
                sort_keys=True,
            ) + "\n")

    def system_line(self, line):
        with self.lock:
            self.files["macos.log"].write(line + "\n")

    def close(self):
        for output in self.files.values():
            output.close()


class Observer:
    def __init__(self, capture):
        self.capture = capture
        self.previous = {}
        self.errors = {}

    def poll(self, source, reader):
        try:
            current = reader()
        except (OSError, subprocess.SubprocessError, ValueError, KeyError) as error:
            message = f"{type(error).__name__}: {error}"
            stderr = getattr(error, "stderr", None)
            if stderr:
                if isinstance(stderr, bytes):
                    stderr = stderr.decode("utf-8", errors="replace")
                message += "; " + stderr.strip()
            if self.errors.get(source) != message:
                self.capture.emit("ERROR", f"{source}: {message}; retaining last snapshot")
            self.errors[source] = message
            return
        if self.errors.pop(source, None) is not None:
            self.capture.emit("RECOVERED", source)
        before = self.previous.get(source)
        if before is None or before != current:
            self.capture.snapshot(source, current)
        for event, key, detail in changes(before or {}, current):
            event = "BASELINE" if before is None else event
            record = (before if event == "REMOVED" else current)[key]
            message = describe(source, event, record, detail if event == "CHANGED" else {})
            if message:
                self.capture.emit(event, message)
        self.previous[source] = current


def start_system_log(capture, stop):
    try:
        process = subprocess.Popen(
            ["log", "stream", "--style", "compact", "--level", "debug",
             "--predicate", LOG_PREDICATE],
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
            encoding="utf-8", errors="replace", start_new_session=True,
        )
    except OSError as error:
        capture.emit("ERROR", f"macOS log stream unavailable: {error}; registry watch continues")
        return None, None

    def drain():
        try:
            for line in process.stdout:
                capture.system_line(line.rstrip("\r\n"))
            result = process.wait()
            if not stop.is_set():
                capture.emit("ERROR", f"macOS log stream ended (exit {result}); see macos.log. Registry watch continues. "
                             "If permission was denied, rerun this command with sudo.")
        finally:
            process.stdout.close()

    thread = threading.Thread(target=drain, name="macos-usb-log", daemon=True)
    thread.start()
    return process, thread


def stop_system_log(process, thread):
    if process is not None:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        thread.join()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--interval", type=float, default=0.5, help="registry polling seconds (default: 0.5)")
    parser.add_argument("--duration", type=float, help="stop after this many seconds; default: until Ctrl-C")
    parser.add_argument("--no-system-log", action="store_true", help="watch registry and serial nodes only")
    args = parser.parse_args()
    if sys.platform != "darwin":
        parser.error("this watcher uses macOS ioreg and unified logging")
    if not math.isfinite(args.interval) or not 0.1 <= args.interval <= 10:
        parser.error("--interval must be between 0.1 and 10 seconds")
    if args.duration is not None and (not math.isfinite(args.duration) or args.duration <= 0):
        parser.error("--duration must be a positive number of seconds")
    parent = ROOT / "local"
    parent.mkdir(mode=0o700, exist_ok=True)
    prefix = "usb-watch-" + datetime.datetime.now().strftime("%Y%m%d-%H%M%S") + "-"
    folder = Path(tempfile.mkdtemp(prefix=prefix, dir=parent))
    capture = Capture(folder)
    stop = threading.Event()
    handlers = {sig: signal.getsignal(sig) for sig in (signal.SIGINT, signal.SIGTERM)}
    for sig in handlers:
        signal.signal(sig, lambda *_: stop.set())
    process = thread = None
    started = time.monotonic()
    try:
        capture.emit("WATCH", f"Logs: {folder}")
        capture.emit("WATCH", "Watching USB, USB-C and serial connections. Ctrl-C stops. Details saved to files.")
        capture.emit("WATCH", f"macOS {platform.mac_ver()[0]}; poll interval {args.interval}s. "
                     "Read-only; nothing is sent to the ESP.")
        if not args.no_system_log:
            process, thread = start_system_log(capture, stop)
        observer = Observer(capture)
        first_poll = True
        while not stop.is_set():
            for class_name in CLASSES:
                if stop.is_set():
                    break
                observer.poll(class_name, lambda name=class_name: registry_snapshot(name))
            observer.poll("serial nodes", port_snapshot)
            if first_poll:
                if all(observer.previous.get(name) == {} for name in ("IOUSBHostDevice", "IOUSBDevice")):
                    capture.emit("WATCH", "No USB devices detected yet. Waiting for changes.")
                first_poll = False
            now = time.monotonic()
            if args.duration is not None and now - started >= args.duration:
                break
            delay = args.interval
            if args.duration is not None:
                delay = min(delay, max(0, args.duration - (now - started)))
            stop.wait(delay)
    finally:
        stop.set()
        stop_system_log(process, thread)
        capture.emit("WATCH", f"Stopped. Saved logs: {folder}")
        capture.close()
        for sig, handler in handlers.items():
            signal.signal(sig, handler)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, subprocess.SubprocessError) as error:
        sys.exit(f"USB watcher failed: {error}")
