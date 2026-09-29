import importlib.util
from pathlib import Path
import tempfile
import struct
import unittest
from unittest.mock import patch


spec = importlib.util.spec_from_file_location("key_right_flash", Path(__file__).parents[1] / "flash.py")
flash = importlib.util.module_from_spec(spec)
spec.loader.exec_module(flash)


class PreflightTests(unittest.TestCase):
    def setUp(self):
        stack_check = patch.object(flash, "check_stack", return_value=70_000)
        stack_check.start()
        self.addCleanup(stack_check.stop)

    def test_chip_and_capacity_must_be_detected(self):
        self.assertEqual(flash.board_capacity("Chip type: ESP32-C3 (revision v0.4)\nFlash size: 4MB\n"), 4 * 1024 * 1024)
        self.assertEqual(flash.board_capacity("Chip type: esp32c3\nFlash size: 8 MiB\n"), 8 * 1024 * 1024)
        for report in (
            "Chip type: esp32s3\nFlash size: 4MB\n",
            "Chip type: esp32c3\nFlash size: 2MB\n",
            "Chip type: esp32c3\nFlash size: unknown\n",
            "Chip type: esp32c3\nFlash size: 4MB\nWARN Could not detect flash size (FlashID=0x00), defaulting to 4MB\n",
            "Flash size: 4MB\n",
        ):
            with self.subTest(report=report), self.assertRaises(ValueError):
                flash.board_capacity(report)

    def test_partition_overflow_overlap_and_missing_factory_fail(self):
        for contents in (
            "factory,app,factory,0x20000,0x400000,\n",
            "nvs,data,nvs,0x9000,0x30000,\nfactory,app,factory,0x20000,0x10000,\n",
            "nvs,data,nvs,0x9000,0x10000,\n",
        ):
            with tempfile.TemporaryDirectory(prefix="key-right-flash-test-") as directory:
                path = Path(directory) / "partitions.csv"
                path.write_text(contents)
                with self.assertRaises(ValueError):
                    flash.validate_partitions(path, flash.MIN_FLASH)
        self.assertEqual(flash.validate_partitions(flash.APP / "partitions.csv", flash.MIN_FLASH), flash.MIN_FLASH)

    def test_info_does_not_build_or_flash(self):
        with patch.object(flash.shutil, "which", return_value="espflash"), patch.object(flash, "run", return_value="Chip type: esp32c3\nFlash size: 4MB\n") as run:
            flash.main(["--info", "/dev/fake"])
        self.assertEqual(run.call_count, 1)
        self.assertEqual(run.call_args.args[0][:2], ["espflash", "board-info"])
        self.assertNotIn("--chip", run.call_args.args[0])

    def test_failed_detection_stops_before_image_conversion_or_flash(self):
        with patch.object(flash.shutil, "which", return_value="espflash"), patch.object(flash, "run", side_effect=["", "Chip type: esp32c3\nFlash size: 2MB\n"]) as run:
            with self.assertRaises(ValueError):
                flash.main(["/dev/fake"])
        self.assertEqual([call.args[0][1] for call in run.call_args_list], ["build", "board-info"])

    def test_flash_uses_detected_capacity_and_preserves_data_partitions(self):
        def execute(arguments, **_kwargs):
            if arguments[1] == "board-info":
                return "Chip type: esp32c3\nFlash size: 8MB\n"
            if arguments[1] == "save-image":
                Path(arguments[-1]).write_bytes(bytes(256))
            return ""

        with patch.object(flash.shutil, "which", return_value="espflash"), patch.object(flash, "run", side_effect=execute) as run:
            flash.main(["--bench", "/dev/fake"])
        calls = [call.args[0] for call in run.call_args_list]
        self.assertEqual([call[1] for call in calls], ["build", "board-info", "save-image", "flash"])
        self.assertIn("key-right-bench", calls[0])
        self.assertEqual(calls[-1][calls[-1].index("--flash-size") + 1], "8mb")
        for forbidden in ("--chip", "--force", "--no-verify", "--erase-parts", "--erase-data-parts"):
            self.assertNotIn(forbidden, calls[-1])
        self.assertTrue(calls[-1][-1].endswith("/key-right-bench"))

    def test_oversized_generated_image_stops_before_flash(self):
        def execute(arguments, **_kwargs):
            if arguments[1] == "board-info":
                return "Chip type: esp32c3\nFlash size: 4MB\n"
            if arguments[1] == "save-image":
                with Path(arguments[-1]).open("wb") as output:
                    output.truncate(flash.MIN_FLASH + 1)
            return ""

        with patch.object(flash.shutil, "which", return_value="espflash"), patch.object(flash, "run", side_effect=execute) as run:
            with self.assertRaises(ValueError):
                flash.main(["/dev/fake"])
        self.assertNotIn("flash", [call.args[0][1] for call in run.call_args_list])

    def test_radio_diagnostics_retains_hardware_and_all_flash_preflights(self):
        def execute(arguments, **_kwargs):
            if arguments[1] == "board-info":
                return "Chip type: esp32c3\nFlash size: 4MB\n"
            if arguments[1] == "save-image":
                Path(arguments[-1]).write_bytes(bytes(256))
            return ""

        with patch.object(flash.shutil, "which", return_value="espflash"), patch.object(flash, "run", side_effect=execute) as run:
            flash.main(["--radio-diagnostics", "/dev/fake"])
        calls = [call.args[0] for call in run.call_args_list]
        self.assertEqual([call[1] for call in calls], ["build", "board-info", "save-image", "flash"])
        self.assertEqual(calls[0][calls[0].index("--features") + 1], "hardware-light,radio-diagnostics")
        self.assertTrue(calls[-1][-1].endswith("/key-right"))
        self.assertNotIn("--erase-data-parts", calls[-1])

    def test_inherited_cargo_target_cannot_select_a_stale_image(self):
        def execute(arguments, **_kwargs):
            if arguments[1] == "board-info":
                return "Chip type: esp32c3\nFlash size: 4MB\n"
            if arguments[1] == "save-image":
                Path(arguments[-1]).write_bytes(bytes(256))
            return ""

        with patch.dict(flash.os.environ, {"CARGO_BUILD_TARGET": "wrong-target", "CARGO_TARGET_DIR": "/tmp/key-right-other-target"}), patch.object(flash.shutil, "which", return_value="espflash"), patch.object(flash, "run", side_effect=execute) as run:
            flash.main(["/dev/fake"])
        build = run.call_args_list[0].args[0]
        target = build[build.index("--target") + 1]
        directory = Path(build[build.index("--target-dir") + 1])
        expected_image = directory / target / "release" / "key-right"
        self.assertEqual(target, flash.TARGET)
        self.assertEqual(directory, flash.APP / "target")
        self.assertEqual(Path(run.call_args_list[-1].args[0][-1]), expected_image)


class StackTests(unittest.TestCase):
    @staticmethod
    def elf(size):
        data = bytearray(256)
        data[:6] = b"\x7fELF\x01\x01"
        struct.pack_into("<H", data, 18, 243)
        struct.pack_into("<I", data, 32, 64)
        struct.pack_into("<HHH", data, 46, 40, 3, 1)
        names = b"\0.shstrtab\0.stack\0"
        data[192:192 + len(names)] = names
        struct.pack_into("<10I", data, 104, 1, 3, 0, 0, 192, len(names), 0, 0, 1, 0)
        struct.pack_into("<10I", data, 144, 11, 8, 3, 0x3FCCE400 - size, 0, size, 0, 0, 16, 0)
        return data

    def test_original_small_c3_stack_fails_and_reclaimed_heap_stack_passes(self):
        for size, accepted in ((4_424, False), (22_080, False), (flash.MIN_STACK - 1, False), (flash.MIN_STACK, True), (70_720, True)):
            with tempfile.TemporaryDirectory(prefix="key-right-stack-test-") as directory:
                image = Path(directory) / "firmware.elf"
                image.write_bytes(self.elf(size))
                if accepted:
                    self.assertEqual(flash.check_stack(image), size)
                else:
                    with self.assertRaises(ValueError):
                        flash.check_stack(image)

    def test_non_riscv_or_missing_stack_images_fail(self):
        wrong_chip = self.elf(70_720)
        struct.pack_into("<H", wrong_chip, 18, 40)
        missing_stack = self.elf(70_720)
        missing_stack[203] = ord("x")
        for data in (b"", wrong_chip, missing_stack):
            with self.assertRaises(ValueError):
                flash.elf_stack_size(data)


if __name__ == "__main__":
    unittest.main()
