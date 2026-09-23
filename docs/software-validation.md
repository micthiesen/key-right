# Software verification, 2026-09-23

Verified on macOS arm64 with Rust 1.98.0. `RUSTUP_TOOLCHAIN=1.98.0` was set for
the checks; the repository's default toolchain selection was not changed.

| Check | Result |
| --- | --- |
| `RUSTUP_TOOLCHAIN=1.98.0 sh scripts/check.sh` | Pass: formatting, strict Clippy, 16 Rust tests, simulator smoke test, 4 Python tests |
| `RUSTUP_TOOLCHAIN=1.98.0 sh scripts/check-firmware.sh` | Pass: app formatting, strict Clippy, 24 application host tests, real and bench ESP32-C6 release builds |
| `espflash 4.5.0 save-image` | Pass: 1,951,328-byte image fits the 4,063,232-byte application partition (48.02%) |
| Real-image ELF | 6,483,372 bytes; SHA-256 `bab46c85e5ca3cea7aa5fabdc475d225ffb540139c71133f6f1c4bfaf5fa0686` |
| Signed stock-firmware emulation | Pass: initialization, Off, and all 202 integer temperatures 143–344 at nominal brightness 3 |
| Field guide | Two Letter pages, 12 steps; final render reviewed in color and grayscale |
| GitHub CI | [Run 35912679240](https://github.com/micthiesen/key-right/actions/runs/35912679240): host and ESP32-C6 jobs passed for code commit `87e1cdab4c15d7ef7d4b3a2f0d4c011cbe70b851` |
| Guide printing | Executor reports job 233 completed both pages, one-sided Letter; [receipt](field-guides/key-right/print-receipt.json) |

These checks establish software behavior and buildability, not physical operation.
The emulator reproduces PCA register commands; it does not emulate the board's
electrical startup or measure light output.

## Physical status

No board was flashed or electrically probed. Actual PCA bus access/levels, OE
routing, cold power-up and reset behavior, physical light output, closed-housing
radio performance, and Apple Home recovery remain unverified. PCA register
readback is not a physical output measurement.
