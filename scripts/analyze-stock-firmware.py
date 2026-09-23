#!/usr/bin/env python3
"""Inspect the exact original Key Light 1.0.3-222 image offline.

No downloads, device access, or firmware execution outside an optional emulator.
The hash gate deliberately rejects other revisions: addresses are revision-specific.
See docs/references/firmware-analysis.md for provenance and interpretation.
"""

import argparse
import hashlib
import json
from pathlib import Path
import struct


SHA256 = "57f48194dbec6ee989db6536a6cc0008cde84180301f73d1e7cd4935e3eb6c83"
SIZE = 698901
SEGMENTS = ((0x80, 0x22B40, 0x10006000), (0x22BD0, 0x87E31, 0x30000000))


def inspect(path):
    if path.stat().st_size != SIZE:
        raise ValueError(f"Expected exactly {SIZE} bytes")
    data = path.read_bytes()
    if hashlib.sha256(data).hexdigest() != SHA256:
        raise ValueError("Unsupported firmware SHA-256; do not reuse these addresses")
    images = {}
    for offset, size, address in SEGMENTS:
        if struct.unpack_from("<IIII", data, offset) != (
            size, address, 0xFFFFFFFF, 0xFFFFFFFF
        ):
            raise ValueError("Unexpected RTKWin segment header")
        images[address] = data[offset + 16:offset + 16 + size]
    return data, images


def inferred_pair(mired):
    if mired < 144:
        return (100, 0)
    if mired >= 344:
        return (0, 100)
    if mired == 244:
        return (100, 100)
    if mired < 244:
        return (100, int((mired - 143) / 1.01))
    return (344 - mired, 100)


def inferred_pwm(mix, brightness=3):
    # Nonnegative, settled integer input only. Preserve both truncations.
    return int((int(mix * 0.4095 * brightness) // 16) * 0.9)


def emulate(images):
    try:
        import unicorn
        from unicorn import arm_const
    except ImportError as error:
        raise ValueError("Use uv run --with unicorn==2.1.4 for --emulate") from error

    machine = unicorn.Uc(unicorn.UC_ARCH_ARM, unicorn.UC_MODE_THUMB)
    # Only executable images, isolated RAM, and a return sentinel are mapped.
    # No peripheral address space, filesystem, system calls, or network exists.
    machine.mem_map(0x10000000, 0x100000)
    machine.mem_map(0x30000000, 0x100000)
    for base, image in images.items():
        machine.mem_write(base, image)
    machine.mem_map(0x20000000, 0x10000)
    machine.mem_map(0x50000000, 0x1000)
    registers = [getattr(arm_const, f"UC_ARM_REG_R{i}") for i in range(4)]
    writes = []
    current_mired = 303
    current_tick = 1000
    transition_arguments = []

    def intercept(cpu, address, _size, _context):
        # Observe the real wrapper's order without replacing the transition.
        if address == 0x30040FB6:
            transition_arguments[:] = [cpu.reg_read(reg) for reg in registers[:3]]
        result = 0
        if address == 0x30041DD8:
            pointer = cpu.reg_read(registers[1])
            length = cpu.reg_read(registers[2])
            if not 1 <= length <= 2:
                raise ValueError("Unexpected I2C write length")
            writes.append(bytes(cpu.mem_read(pointer, length)).hex(" "))
        elif address == 0x30013C74:
            result = 0x20000800  # Isolated, dummy I2C object.
        elif address == 0x30013C7C:
            result = 1  # Light subsystem initialized.
        elif address in (0x30001AF6, 0x30001A64):
            value = current_mired if address == 0x30001AF6 else 3
            cpu.mem_write(cpu.reg_read(registers[0]), struct.pack("<I", value))
        elif address == 0x300019C8:
            cpu.mem_write(cpu.reg_read(registers[0]), b"\x01")  # On.
        elif address == 0x300022B6:
            cpu.mem_write(cpu.reg_read(registers[0]), b"\x00\x00")  # Fade setting.
        elif address == 0x30000B00:
            result = cpu.reg_read(registers[0])
            length = cpu.reg_read(registers[2])
            if length > 0x1000:
                raise ValueError("Unexpected memset length")
            cpu.mem_write(result, bytes([cpu.reg_read(registers[1]) & 255]) * length)
        elif address == 0x30000D10:
            result = current_tick
        elif address == 0x30000C56:
            result = 0x20000900  # Dummy timer handle.
        elif address not in (
            0x30055F38, 0x30010990, 0x30000CD2, 0x30000C6A, 0x3004A7BC
        ):
            return  # Original code executes for every other function.
        cpu.reg_write(registers[0], result)
        cpu.reg_write(arm_const.UC_ARM_REG_PC, cpu.reg_read(arm_const.UC_ARM_REG_LR))

    machine.hook_add(unicorn.UC_HOOK_CODE, intercept)

    def run(address, *arguments):
        machine.reg_write(arm_const.UC_ARM_REG_SP, 0x2000F000)
        machine.reg_write(arm_const.UC_ARM_REG_LR, 0x50000001)
        for register, argument in zip(registers, arguments):
            machine.reg_write(register, argument)
        machine.emu_start(address | 1, 0x50000000, timeout=1000000, count=100000)
        if machine.reg_read(arm_const.UC_ARM_REG_PC) != 0x50000000:
            raise ValueError("Emulation exceeded its instruction/time bound")

    def float_bits(value):
        return struct.unpack("<I", struct.pack("<f", value))[0]

    # Select the PCA9635 table entry. Device detection itself is not emulated.
    machine.mem_write(0x30058954, struct.pack("<I", 1))
    run(0x30040B3A, 0x300414FD)  # Real transition setup and callback registration.
    result = {"unicorn_version": unicorn.__version__, "presets": {}}
    for mired in range(143, 345):
        run(0x3004A220, 0x20000100, mired)
        pair = struct.unpack("<ii", machine.mem_read(0x20000100, 8))
        if pair != inferred_pair(mired):
            raise ValueError(f"Temperature formula mismatch at {mired}")
        current_mired = mired
        current_tick = 1000
        run(0x30041580)  # Real getter wrapper and transition command.
        ordered = [float_bits(pair[1]), float_bits(pair[0]), float_bits(3)]
        if transition_arguments != ordered:
            raise ValueError(f"Channel order mismatch at {mired}")
        writes.clear()
        current_tick = 2000
        run(0x30040E56, 0x20000900, 0x30087F1C)  # Real settled transition tick.
        expected = [f"02 {inferred_pwm(pair[1]):02x}", f"06 {inferred_pwm(pair[0]):02x}"]
        if writes != expected:
            raise ValueError(f"PWM formula mismatch at {mired}: {writes}")
        if mired in (200, 303):
            result["presets"][str(mired)] = {
                "temperature_pair_cool_warm": pair,
                "output_order": "PWM0 warm, PWM4 cool",
                "writes": list(writes),
            }
    result["verified_mired_range_at_brightness_3"] = [143, 344]
    writes.clear()
    run(0x30055FE0, 0x20000800)
    expected_init = ["03", "00 00", "01 14", "14 aa", "15 aa", "16 aa", "17 aa"]
    if writes != expected_init:
        raise ValueError(f"Unexpected initialization: {writes}")
    result["initialization_writes"] = list(writes)
    writes.clear()
    run(0x30055F18, 0x20000800)
    if writes != [f"{register:02x} 00" for register in range(2, 18)]:
        raise ValueError(f"Unexpected all-channel zero routine: {writes}")
    result["all_channel_zero_writes"] = list(writes)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("firmware", type=Path)
    parser.add_argument("--emulate", action="store_true")
    parser.add_argument("--dump-segments", type=Path, metavar="NEW_DIRECTORY")
    args = parser.parse_args()
    try:
        data, images = inspect(args.firmware)
        report = {
            "sha256": SHA256,
            "board_type": data[45],
            "version_and_build": struct.unpack_from("<HHHH", data, 46),
            "segments": [
                {"file_data_offset": hex(offset + 16), "length": size, "load_address": hex(base)}
                for offset, size, base in SEGMENTS
            ],
            "signature_verification": "not performed; exact image hash checked",
            "hardware_validation": "none",
        }
        if args.emulate:
            report["emulation"] = emulate(images)
        if args.dump_segments:
            args.dump_segments.mkdir(parents=True, exist_ok=False)
            for base, image in images.items():
                (args.dump_segments / f"image-{base:08x}.bin").write_bytes(image)
        print(json.dumps(report, indent=2))
    except (OSError, ValueError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
