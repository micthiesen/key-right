#!/usr/bin/env python3
"""Build and preflight the ESP32-C3 image before writing any device flash."""

import argparse
import csv
import os
from pathlib import Path
import re
import shutil
import subprocess
import struct
import sys
import tempfile


ROOT = Path(__file__).resolve().parent.parent
APP = ROOT / "firmware" / "app"
MIN_FLASH = 4 * 1024 * 1024
TARGET = "riscv32imc-unknown-none-elf"
# A 22,080-byte reservation passed the old 16 KiB gate but failed on real boot
# while restoring Home fabrics. Keep more headroom; this is still not a runtime
# high-water measurement and cannot replace live boot/recovery checks.
MIN_STACK = 32 * 1024
# esp-lan-ota's rollback-enabled ESP-IDF bootloader; see bootloader/README.md.
BOOTLOADER = APP / "bootloader" / "esp32c3-bootloader.bin"
# Both lamps' Home fabrics live here. Never move or resize it.
NVS = (0x9000, 0x10000)
OTADATA_SIZE = 0x2000
PARTITION_TABLE_OFFSET = 0x8000
APP_ALIGN = 0x10000
# USB flashes write ota_0 and erase only otadata, so the bootloader starts the
# new image and the firmware opens its one-time esp-ota enrollment window.
APP_SLOT = "ota_0"
ERASE_PARTS = "otadata"


def elf_stack_size(data):
    """Read the linked main-stack reservation without a platform-specific tool."""
    try:
        if data[:6] != b"\x7fELF\x01\x01" or struct.unpack_from("<H", data, 18)[0] != 243:
            raise ValueError("Expected a little-endian ELF32 RISC-V image")
        offset = struct.unpack_from("<I", data, 32)[0]
        entry_size, count, names_index = struct.unpack_from("<HHH", data, 46)
        if entry_size != 40 or names_index >= count:
            raise ValueError("Unsupported ELF section table")
        sections = [struct.unpack_from("<10I", data, offset + index * entry_size) for index in range(count)]
        names = sections[names_index]
        names = data[names[4]:names[4] + names[5]]
        for section in sections:
            name, kind, flags, address, _, size, *_ = section
            end = names.find(b"\0", name)
            if end >= 0 and names[name:end] == b".stack":
                if kind != 8 or not flags & 2 or not 0x3FC80000 <= address < address + size <= 0x3FCCE400:
                    raise ValueError("Stack section is outside the C3 application's data RAM")
                return size
    except (struct.error, IndexError) as error:
        raise ValueError("Malformed ELF section table") from error
    raise ValueError("ELF has no linked main-stack reservation")


def check_stack(image):
    size = elf_stack_size(image.read_bytes())
    if size < MIN_STACK:
        raise ValueError(f"Linked main stack is only {size} bytes; require at least {MIN_STACK}")
    print(f"Linked main-stack reservation: {size} bytes (minimum {MIN_STACK}); runtime use unmeasured")
    return size


def board_capacity(report):
    """Fail closed on unknown chip/capacity in espflash 4.5.0's board-info output."""
    report = re.sub(r"\x1b\[[0-9;]*m", "", report)
    # espflash 4.5.0 reports a guessed 4MB after an unknown flash ID. Its warning
    # is part of the preflight result and must never count as detected capacity.
    if re.search(r"could not detect flash size|defaulting to", report, re.IGNORECASE):
        raise ValueError("Flash capacity was guessed by espflash; no flash was written")
    chip = re.search(r"^Chip type:\s*(\S+)", report, re.MULTILINE)
    if not chip or chip[1].lower().replace("-", "") != "esp32c3":
        raise ValueError("Detected chip must be ESP32-C3; no flash was written")
    size = re.search(r"^Flash size:\s*(\d+)\s*(Ki?B|Mi?B)\s*$", report, re.MULTILINE | re.IGNORECASE)
    if not size:
        raise ValueError("Could not verify detected flash capacity; no flash was written")
    capacity = int(size[1]) * (1024 if size[2].upper().startswith("K") else 1024 * 1024)
    if capacity < MIN_FLASH:
        raise ValueError("Detected flash is smaller than the required 4 MiB; no flash was written")
    return capacity


def check_bootloader(path):
    """The C3 bootloader sits at 0x0 and must end before the table at 0x8000."""
    if not path.is_file():
        raise ValueError(
            f"Missing {path}: add esp-lan-ota's rollback-enabled ESP32-C3 bootloader "
            "(see firmware/app/bootloader/README.md); no flash was written"
        )
    data = path.read_bytes()
    if not data or data[0] != 0xE9:
        raise ValueError(f"{path} is not an ESP image (no 0xE9 magic); no flash was written")
    if len(data) >= PARTITION_TABLE_OFFSET:
        raise ValueError(f"{path} is {len(data)} bytes and would overlap the partition table; no flash was written")
    return len(data)


def quantity(value):
    value = value.strip().lower()
    for suffix, factor in (("k", 1024), ("m", 1024 * 1024)):
        if value.endswith(suffix):
            return int(value[:-1], 0) * factor
    return int(value, 0)


def validate_partitions(path, capacity):
    """Check the OTA layout. Returns the ota_0 slot size an image must fit."""
    regions = []
    with path.open() as source:
        rows = csv.reader(line for line in source if line.strip() and not line.lstrip().startswith("#"))
        for row in rows:
            if len(row) < 5:
                raise ValueError("Partition rows require name, type, subtype, explicit offset, and size")
            name, kind, subtype, offset, size = (value.strip() for value in row[:5])
            start, length = quantity(offset), quantity(size)
            end = start + length
            if start < 0x9000 or length <= 0 or end > capacity:
                raise ValueError(f"Partition {name!r} exceeds verified flash bounds")
            regions.append((start, end, name, kind, subtype))
    regions.sort()
    if any(left[1] > right[0] for left, right in zip(regions, regions[1:])):
        raise ValueError("Flash partitions overlap")
    by_subtype = {}
    for start, end, name, kind, subtype in regions:
        by_subtype.setdefault((kind, subtype), []).append((start, end - start, name))
    if by_subtype.get(("data", "nvs")) != [(*NVS, "nvs")]:
        raise ValueError("nvs must stay at 0x9000 with size 0x10000; paired Home fabrics live there")
    if ("app", "factory") in by_subtype:
        raise ValueError("The OTA layout has no factory partition")
    otadata = by_subtype.get(("data", "ota"), [])
    if len(otadata) != 1 or otadata[0][1] != OTADATA_SIZE or otadata[0][2] != ERASE_PARTS:
        raise ValueError("Exactly one 0x2000-byte otadata partition is required")
    slots = [by_subtype.get(("app", label), []) for label in ("ota_0", "ota_1")]
    if any(len(slot) != 1 or slot[0][2] != label for slot, label in zip(slots, ("ota_0", "ota_1"))):
        raise ValueError("Exactly one ota_0 and one ota_1 app partition are required")
    (first, first_size, _), (second, second_size, _) = slots[0][0], slots[1][0]
    if first_size != second_size:
        raise ValueError("ota_0 and ota_1 must be the same size")
    if any(value % APP_ALIGN for value in (first, first_size, second)):
        raise ValueError("App partitions must be 64 KiB aligned")
    return first_size


def run(arguments, *, capture=False):
    environment = os.environ.copy()
    # Do not let caller logging preferences hide a flash-detection fallback.
    if arguments[0] == "espflash":
        environment["RUST_LOG"] = "info"
    result = subprocess.run(arguments, cwd=APP, env=environment, text=True, capture_output=capture, check=False)
    if capture:
        print(result.stdout, end="")
        print(result.stderr, end="", file=sys.stderr)
    if result.returncode:
        raise ValueError(f"{arguments[0]} {arguments[1]} failed (exit {result.returncode})")
    return result.stdout + result.stderr if capture else ""


def detect(port):
    # No chip/size override: both values must come from the connected device.
    report = run([
        "espflash", "board-info", "--port", port,
        "--non-interactive", "--skip-update-check",
    ], capture=True)
    return board_capacity(report)


def main(arguments=None):
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--info", action="store_true", help="detect chip/flash without writing flash (may reset the board)")
    mode.add_argument("--check", action="store_true", help="build and check image/partition fit without a connected board")
    mode.add_argument("--check-stack", type=Path, metavar="ELF", help="check the linked C3 stack reservation without building or device access")
    parser.add_argument("--bench", action="store_true", help="select the simulated-output image instead of the PCA image")
    parser.add_argument("--radio-diagnostics", action="store_true", help="include a bounded Wi-Fi boot scan in the image")
    parser.add_argument("port", nargs="?", help="native USB port, such as /dev/cu.usbmodemPORT")
    args = parser.parse_args(arguments)
    if args.check_stack:
        if args.port or args.bench or args.radio_diagnostics:
            parser.error("--check-stack takes only an ELF path")
        check_stack(args.check_stack)
        return
    if args.check and args.port:
        parser.error("--check does not use a USB port")
    if not args.check and not args.port:
        parser.error("a USB port is required; list with python3 scripts/device.py --list")
    if shutil.which("espflash") is None:
        raise ValueError("Install espflash: cargo install espflash --version 4.5.0 --locked")
    if not args.check and not args.info:
        check_bootloader(BOOTLOADER)
    partition_table = APP / "partitions.csv"
    if args.info:
        capacity = detect(args.port)
        validate_partitions(partition_table, capacity)
        print(f"Preflight passed: ESP32-C3, {capacity} bytes detected; flash unchanged")
        return

    binary, feature = ("key-right-bench", "bench-light") if args.bench else ("key-right", "hardware-light")
    if args.radio_diagnostics:
        feature += ",radio-diagnostics"
    # Pin both values so inherited Cargo environment variables cannot redirect a
    # fresh build while leaving an older ELF at the path subsequently flashed.
    run([
        "cargo", "build", "--release", "--bin", binary, "--features", feature,
        "--target", TARGET, "--target-dir", str(APP / "target"), "--locked",
    ])
    image = APP / "target" / TARGET / "release" / binary
    check_stack(image)
    capacity = MIN_FLASH if args.check else detect(args.port)
    slot = validate_partitions(partition_table, capacity)
    # Convert the app image alone against ota_0. Nothing here is flashed.
    with tempfile.TemporaryDirectory(prefix="key-right-flash-") as temporary:
        converted = Path(temporary) / "app.bin"
        run([
            "espflash", "save-image", "--chip", "esp32c3", "--flash-size", "4mb",
            "--partition-table", str(partition_table), "--target-app-partition", APP_SLOT,
            "--skip-update-check", str(image), str(converted),
        ])
        size = converted.stat().st_size
    if size > slot:
        raise ValueError(f"App image is {size} bytes; the {APP_SLOT} slot holds {slot}")
    print(f"App image: {size} of {slot} slot bytes; {slot - size} bytes headroom per OTA slot")
    if args.check:
        print("Build/image/partition check passed for 4 MiB; hardware identity remains unverified")
        return
    print("Flashing verified ESP32-C3 to ota_0; NVS is preserved; otadata is erased. USB VBUS must be blocked while lamp wires are attached.")
    # Let espflash detect the chip again. Capacity comes from the completed
    # preflight, overriding any unrelated espflash project setting. The only
    # erase is otadata (boot selection), never NVS. No force or no-verify flags.
    run([
        "espflash", "flash", "--bootloader", str(BOOTLOADER),
        "--partition-table", str(partition_table),
        "--target-app-partition", APP_SLOT, "--erase-parts", ERASE_PARTS,
        "--flash-size", f"{capacity // (1024 * 1024)}mb",
        "--port", args.port, "--non-interactive", "--skip-update-check", str(image),
    ])


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError) as error:
        sys.exit(f"error: {error}")
