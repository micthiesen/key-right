"""USB observation must distinguish disconnection from failed inventory reads."""
import importlib.util
import json
from pathlib import Path
import plistlib
import stat
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch


SPEC = importlib.util.spec_from_file_location("usb_watch", Path(__file__).parents[1] / "usb-watch.py")
watch = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(watch)


class UsbWatchTests(unittest.TestCase):
    def test_empty_registry_output_is_successful_empty_inventory(self):
        with patch.object(watch.subprocess, "run", return_value=Mock(stdout=b"")):
            self.assertEqual(watch.registry_snapshot("IOUSBHostDevice"), {})

    def test_bad_registry_output_is_not_an_empty_inventory(self):
        for raw in (b"not a plist", plistlib.dumps({"unexpected": "shape"})):
            with self.subTest(raw=raw):
                with patch.object(watch.subprocess, "run", return_value=Mock(stdout=raw)):
                    with self.assertRaises(ValueError):
                        watch.registry_snapshot("IOUSBHostDevice")

    def test_normalization_keeps_connection_state_and_binary_identifiers(self):
        row = {"IORegistryEntryID": 123, "IOObjectRetainCount": 99,
               "ConnectionActive": True, "serial": b"\x01\xff",
               "IORegistryEntryChildren": [{"irrelevant": "child"}]}
        with patch.object(watch.subprocess, "run", return_value=Mock(stdout=plistlib.dumps([row]))):
            self.assertEqual(watch.registry_snapshot("IOAccessoryManager"), {
                "123": {"IORegistryEntryID": 123, "ConnectionActive": True, "serial": "0x01ff"},
            })

    def test_changes_include_added_removed_and_changed_properties(self):
        before = {"gone": {"path": "old"}, "port": {"active": False, "old": 1}}
        after = {"new": {"path": "new"}, "port": {"active": True, "orientation": 2}}
        self.assertEqual(list(watch.changes(before, after)), [
            ("REMOVED", "gone", {"path": "old"}),
            ("ADDED", "new", {"path": "new"}),
            ("CHANGED", "port", {"active": {"before": False, "after": True},
                                  "old": {"before": 1, "after": None},
                                  "orientation": {"before": None, "after": 2}}),
        ])

    def test_failed_snapshot_preserves_devices_until_successful_removal(self):
        capture = Mock()
        observer = watch.Observer(capture)
        initial = {"esp": {"idVendor": 0x303a}}
        observer.poll("IOUSBHostDevice", lambda: initial)
        capture.reset_mock()
        reader = Mock(side_effect=subprocess.TimeoutExpired("ioreg", 3))
        observer.poll("IOUSBHostDevice", reader)
        observer.poll("IOUSBHostDevice", reader)
        self.assertEqual(observer.previous["IOUSBHostDevice"], initial)
        capture.snapshot.assert_not_called()
        self.assertEqual([call.args[0] for call in capture.emit.call_args_list], ["ERROR"])
        observer.poll("IOUSBHostDevice", lambda: {})
        self.assertEqual([call.args[0] for call in capture.emit.call_args_list],
                         ["ERROR", "RECOVERED", "REMOVED"])
        capture.snapshot.assert_called_once_with("IOUSBHostDevice", {})

    def test_first_success_after_failure_is_baseline(self):
        capture = Mock()
        observer = watch.Observer(capture)
        observer.poll("USB", Mock(side_effect=ValueError("invalid plist")))
        self.assertNotIn("USB", observer.previous)
        observer.poll("USB", lambda: {})
        self.assertEqual([call.args[0] for call in capture.emit.call_args_list],
                         ["ERROR", "RECOVERED"])
        capture.snapshot.assert_called_once_with("USB", {})

    def test_unchanged_snapshot_does_not_create_noise(self):
        capture = Mock()
        observer = watch.Observer(capture)
        observer.poll("serial", lambda: {"path": {"path": "/dev/cu.example"}})
        capture.reset_mock()
        observer.poll("serial", lambda: {"path": {"path": "/dev/cu.example"}})
        capture.emit.assert_not_called()
        capture.snapshot.assert_not_called()

    def test_command_error_includes_stderr(self):
        capture = Mock()
        watch.Observer(capture).poll("USB", Mock(side_effect=subprocess.CalledProcessError(
            1, ["ioreg"], stderr=b"permission denied")))
        self.assertIn("permission denied", capture.emit.call_args.args[1])

    def test_full_capture_is_private_and_never_prints_json_or_system_log_spam(self):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            capture = watch.Capture(folder)
            records = {"esp": {"USB Product Name": "ESP USB", "idVendor": 0x303a,
                               "idProduct": 0x1001, "details": "x" * 2000}}
            try:
                with patch("builtins.print") as output:
                    watch.Observer(capture).poll("IOUSBHostDevice", lambda: records)
                    capture.system_line("driver event")
                    output.assert_called_once()
                    terminal = output.call_args.args[0]
                    self.assertIn("Present: ESP USB (303a:1001)", terminal)
                    self.assertNotIn("{", terminal)
                    self.assertNotIn("details", terminal)
            finally:
                capture.close()
            stored = json.loads((folder / "snapshots.jsonl").read_text())
            self.assertEqual(stored["records"], records)
            self.assertNotIn("driver event", (folder / "events.log").read_text())
            self.assertEqual((folder / "macos.log").read_text(), "driver event\n")
            for path in folder.iterdir():
                self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)

    def test_idle_port_timers_do_not_count_as_activity(self):
        before = {"port-statistics": {"kPortStatPowerStateTime": "10ms", "kPortStatConnectCount": 1}}
        after = {"port-statistics": {"kPortStatPowerStateTime": "20ms", "kPortStatConnectCount": 1}}
        self.assertEqual(watch.normalize(before), watch.normalize(after))
        after["port-statistics"]["kPortStatConnectCount"] = 2
        self.assertNotEqual(watch.normalize(before), watch.normalize(after))

    def test_serial_duplicate_and_builtin_nodes_are_quiet(self):
        for path in ("/dev/cu.debug-console", "/dev/cu.Bluetooth-Incoming-Port", "/dev/tty.usbmodem1101"):
            self.assertIsNone(watch.describe("serial nodes", "BASELINE", {"path": path}, {}))
        self.assertEqual(watch.describe("serial nodes", "ADDED", {"path": "/dev/cu.usbmodem1101"}, {}),
                         "Connected: serial /dev/cu.usbmodem1101")

    def test_accessory_changes_are_concise_but_unknown_properties_are_quiet(self):
        record = {"PortDescription": "USB-C port 2"}
        delta = {"ConnectionActive": {"before": False, "after": True}}
        self.assertEqual(watch.describe("IOAccessoryManager", "CHANGED", record, delta),
                         "USB-C port 2: connected=yes")
        self.assertIsNone(watch.describe("IOAccessoryManager", "CHANGED", record,
                                        {"irrelevant": {"before": 0, "after": 1}}))

    def test_port_enumeration_failure_remains_visible(self):
        delta = {"port-statistics": {"before": {"kPortStatEnumerationFailureCount": 0},
                                     "after": {"kPortStatEnumerationFailureCount": 1}}}
        self.assertIn("enumeration failures=1", watch.describe("AppleUSBHostPort", "CHANGED", {}, delta))

    def test_transport_attachment_without_device_enumeration_remains_visible(self):
        path = "IOService:/very/long/registry/path/Port-USB-C@2/USB2"
        for before, after, expected in ((None, path, "attached"), (path, None, "detached")):
            message = watch.describe("AppleUSBHostPort", "CHANGED", {"IORegistryEntryName": "port 2"},
                                     {"UsbTransportState": {"before": before, "after": after}})
            self.assertEqual(message, f"port 2: USB transport {expected}")

    def test_log_shutdown_kills_unresponsive_child_and_joins_reader(self):
        process, thread = Mock(), Mock()
        process.poll.return_value = None
        process.wait.side_effect = [subprocess.TimeoutExpired("log", 2), -9]
        watch.stop_system_log(process, thread)
        process.terminate.assert_called_once_with()
        process.kill.assert_called_once_with()
        thread.join.assert_called_once_with()


if __name__ == "__main__":
    unittest.main()
