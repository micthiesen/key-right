# Software verification, 2026-09-23

Verified on macOS arm64 with Rust 1.97.1. These checks establish software behavior
and buildability, not physical light operation.

| Check | Observed result |
| --- | --- |
| `sh scripts/check.sh` | Pass: rustfmt, strict Clippy, 19 Rust tests, simulator smoke, 4 Python pseudo-terminal tests |
| `sh scripts/check-firmware.sh` | Pass: formatting, strict Clippy, 27 application host tests, real and bench ESP32-C6 release builds |
| Bench build without `bench-light` | Rejected with the expected explicit simulated-image diagnostic |
| `espflash 4.5.0 save-image` for real image | Pass: 1,946,800-byte application fits 4,063,232-byte application partition, 47.91% |
| `sh -n` and ShellCheck on `scripts/flash.sh` | Pass; no board accessed |
| Python syntax compilation | Pass for device client, offline analyzer and PDF builder |
| Signed stock-image emulation | Pass for initialization, Off and all 202 integer temperature values 143..344 at nominal brightness 3 |
| Field guide | Eight Letter pages built, text extracted and every page visually inspected after final layout changes |
| Whitespace / local documentation links | Checked; no unresolved paths in the development/guide references |

Real-image ELF SHA-256 from the checked build:
`51370780ec54c67692273aaf25b8b7cfb46c426aa1299577c7c0b5f9262e6073`.
The ELF is a local build product, not a committed binary. Toolchain updates can
change the bytes; the build scripts are the delivery mechanism.

## Review outcomes

Independent code review covered the core, CLI, scripts and application failure
paths. Findings fixed and regression-tested included stale timed-on state after
preset switching, missing startup-policy attributes, unnecessary Off writes,
imported physical attestations, transient storage-read retry, corrupt-record
preservation until explicit commit, and Matter Off during a local candidate test.
The final bounded recheck found those application issues resolved.

Hardware review led to direct PWM through TXU0102 instead of relying on PCA OE,
and to current-limiting output resistors plus a bounded DC input-load screen.
The guide uses manufacturer package numbering and requires finding the real
pin-1 marker; the available photo does not establish it.

## Still requires the board

No flashing, electrical probing, physical output measurement, Apple Home pairing,
Wi-Fi failure injection, watchdog reset observation or seven-day soak was performed.
The [field guide](field-guides/key-right/key-right-field-guide.pdf) and
[acceptance record](validation-record.md) specify those checks and stop conditions.
A healthy CPU/local interface does not prove that every Matter operation remains
responsive; the documented live-protocol blind spot needs actual fault testing.
