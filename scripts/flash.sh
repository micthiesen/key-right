#!/bin/sh
set -eu

if [ "$#" -eq 1 ] && [ "$1" = "--help" ]; then
    echo 'Usage: sh scripts/flash.sh /dev/cu.usbmodemPORT'
    echo 'Build and flash the XIAO ESP32C6 hardware image; preserves NVS.'
    exit 0
fi
if [ "$#" -ne 1 ]; then
    echo 'Provide exactly one USB serial port. List: python3 scripts/device.py --list' >&2
    exit 2
fi

export PATH="$HOME/.cargo/bin:$PATH"
if ! command -v espflash >/dev/null 2>&1; then
    echo 'Install the tested flasher: cargo install espflash --version 4.5.0 --locked' >&2
    exit 1
fi

# Invoke from the repository root, as in the field guide. No erased data
# partitions, forced chip overrides, or disabled flash verification.
cd firmware/app
cargo build --release --bin key-right --features hardware-light --locked
exec espflash flash --chip esp32c6 --flash-size 4mb \
    --partition-table partitions.csv --port "$1" --non-interactive \
    --skip-update-check target/riscv32imac-unknown-none-elf/release/key-right
