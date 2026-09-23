"""Exercise the actual USB client over a local pseudo-terminal, no board needed."""
import importlib.util
import os
from pathlib import Path
import pty
import threading
import unittest

SPEC = importlib.util.spec_from_file_location("device", Path(__file__).parents[1] / "device.py")
device = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(device)


class ConsoleTests(unittest.TestCase):
    def setUp(self):
        self.master, self.slave = pty.openpty()
        self.console = device.Console(os.ttyname(self.slave), 1.0)

    def tearDown(self):
        self.console.close()
        os.close(self.master)
        os.close(self.slave)

    def serve(self, response):
        def run():
            received = bytearray()
            while not received.endswith(b"\n"):
                received.extend(os.read(self.master, 256))
            self.received = bytes(received)
            os.write(self.master, response)
        thread = threading.Thread(target=run)
        thread.start()
        return thread

    def test_logs_do_not_count_as_command_success(self):
        thread = self.serve(b"INFO still starting\r\nKR OK applied=off\r\n")
        self.assertEqual(self.console.command("status"), "KR OK applied=off")
        thread.join()
        self.assertEqual(self.received, b"status\n")

    def test_device_errors_are_errors(self):
        thread = self.serve(b"KR ERR profile not commissioned\n")
        with self.assertRaisesRegex(device.ConsoleError, "not commissioned"):
            self.console.command("on 1")
        thread.join()

    def test_timeout_never_retries_a_mutating_command(self):
        thread = self.serve(b"INFO saving\n")
        with self.assertRaisesRegex(device.ConsoleError, "may have executed"):
            self.console.command("profile commit")
        thread.join()
        self.assertEqual(self.received, b"profile commit\n")

    def test_rejects_multiple_commands_before_writing(self):
        with self.assertRaisesRegex(device.ConsoleError, "one non-empty ASCII"):
            self.console.command("off\non 1")


if __name__ == "__main__":
    unittest.main()
