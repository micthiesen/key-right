# Software verification

## 2026-09-28 documentation reconciliation

Both required gates passed on macOS arm64 with Rust 1.97.1:

- `sh scripts/check.sh`: formatting, strict Clippy, 16 Rust tests, simulator smoke
  test, and 4 Python tests.
- `sh scripts/check-firmware.sh`: formatting, strict Clippy, 24 application host
  tests, and both real and simulated ESP32-C6 release builds.
- Changed Markdown local links and `git diff --check`: passed.

This change updates documentation only. No C3 image was built or flashed, and no
physical bench test was performed. The C3 port remains a separate implementation
requirement. The earlier software/artifact evidence below is retained with its
original date and toolchain.

## 2026-09-23 implementation and artifacts

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

## Physical status and scope

No board was flashed or electrically probed during the September 23 software
verification above. Those results apply to the C6 implementation and archived
C6/buck guide, not to a C3 image or the installed assembly.

As of September 28, Michael has supplied pad continuity, bus/rail voltage, and
pull-up measurements and reports completed C3 wiring with LEDs disconnected.
See [the validation record](validation-record.md) for those user observations and
pending bench tests. The C3 port, flashing, powered ESP/PCA operation, startup,
physical output, closed-housing radio performance, and Apple Home recovery remain
unverified. PCA register readback is not a physical output measurement.
