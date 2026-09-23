#!/usr/bin/env python3
"""Key Right native USB console client (Python 3, macOS/Linux, no packages).

No flashing, reset-line toggling, network access or automatic retries. A timed-out
write may have reached the device: inspect status before deciding to retry.
Capture files can contain commissioning credentials; keep them out of Git.
"""

import argparse
import datetime
import glob
import os
import select
import sys
import termios
import time
import tty
from pathlib import Path


def utc_now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="milliseconds")


def ports():
    return sorted(set(glob.glob("/dev/cu.usbmodem*") + glob.glob("/dev/ttyACM*")))


class ConsoleError(Exception):
    pass


class Console:
    def __init__(self, port, timeout, log=None):
        self.timeout = timeout
        self.log = log
        self.buffer = bytearray()
        self.fd = os.open(port, os.O_RDWR | os.O_NOCTTY | os.O_NONBLOCK)
        try:
            self.original = termios.tcgetattr(self.fd)
            tty.setraw(self.fd, termios.TCSANOW)
            attrs = termios.tcgetattr(self.fd)
            attrs[2] |= termios.CLOCAL | termios.CREAD
            attrs[4] = attrs[5] = termios.B115200
            termios.tcsetattr(self.fd, termios.TCSANOW, attrs)
        except BaseException:
            os.close(self.fd)
            raise

    def close(self):
        try:
            termios.tcsetattr(self.fd, termios.TCSANOW, self.original)
        except (OSError, termios.error):
            pass  # A reboot/disconnection can remove the device first.
        os.close(self.fd)

    def record(self, line):
        if self.log:
            self.log.write(f"{utc_now()} {line}\n")
            self.log.flush()

    def readline(self, deadline):
        while True:
            if b"\n" in self.buffer:
                raw, _, rest = self.buffer.partition(b"\n")
                self.buffer = bytearray(rest)
                line = raw.rstrip(b"\r").decode("utf-8", errors="replace")
                self.record(line)
                return line
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise ConsoleError("USB response timed out; the command may have executed. Inspect status before retrying.")
            readable, _, _ = select.select([self.fd], [], [], remaining)
            if not readable:
                continue
            data = os.read(self.fd, 4096)
            if not data:
                raise ConsoleError("USB disconnected; command outcome is unknown.")
            self.buffer.extend(data)
            if len(self.buffer) > 65536:
                raise ConsoleError("USB line exceeded 64 KiB; refusing an unbounded response.")

    def command(self, command):
        if not command or "\n" in command or "\r" in command or not command.isascii():
            raise ConsoleError("Send one non-empty ASCII command without line breaks.")
        encoded = (command + "\n").encode("ascii")
        if len(encoded) > 256:
            raise ConsoleError("Command exceeds the firmware's 256-byte input limit.")
        # Discard old buffered replies before issuing exactly one request.
        termios.tcflush(self.fd, termios.TCIFLUSH)
        self.buffer.clear()
        deadline = time.monotonic() + self.timeout
        offset = 0
        while offset < len(encoded):
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise ConsoleError("USB write timed out; command outcome is unknown.")
            _, writable, _ = select.select([], [self.fd], [], remaining)
            if writable:
                offset += os.write(self.fd, encoded[offset:])
        while True:
            line = self.readline(deadline)
            if line.startswith("KR OK"):
                return line
            if line.startswith("KR ERR"):
                raise ConsoleError(line)
            # Firmware logs are captured when --log is supplied; they are not
            # mistaken for the acknowledgement of this command.

    def monitor(self):
        while True:
            try:
                line = self.readline(time.monotonic() + 60)
                print(f"{utc_now()} {line}", flush=True)
            except ConsoleError as error:
                if str(error).startswith("USB response timed out"):
                    continue
                raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", help="USB serial path; auto-selects only when exactly one port exists")
    parser.add_argument("--list", action="store_true", help="list native USB serial candidates")
    parser.add_argument("--timeout", type=float, default=10.0)
    parser.add_argument("--log", type=Path, help="append timestamped received lines (may contain secrets)")
    parser.add_argument("command", nargs="*", help="status, monitor, or one firmware command")
    args = parser.parse_args()
    if args.list:
        print("\n".join(ports()))
        return 0
    if not args.command or not 0 < args.timeout <= 120:
        parser.error("provide a command and a timeout between 0 and 120 seconds")
    available = ports()
    port = args.port or (available[0] if len(available) == 1 else None)
    if not port:
        parser.error("specify --port; use --list to see candidates")
    log = None
    console = None
    try:
        command = " ".join(args.command)
        if args.log:
            args.log.parent.mkdir(parents=True, exist_ok=True)
            # New captures are private even when the user's umask is permissive.
            fd = os.open(args.log, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
            log = os.fdopen(fd, "a", encoding="utf-8")
        console = Console(port, args.timeout, log)
        if command == "monitor":
            console.monitor()
        else:
            print(console.command(command))
        return 0
    except KeyboardInterrupt:
        return 130
    except (ConsoleError, OSError, termios.error) as error:
        print(str(error), file=sys.stderr)
        return 1
    finally:
        if console:
            console.close()
        if log:
            log.close()


if __name__ == "__main__":
    raise SystemExit(main())
