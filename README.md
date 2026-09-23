# Key Right

Replace the original full-size Elgato Key Light's Realtek controller with an ESP32 running custom Rust firmware. Keep the existing LED driver and power circuitry. The primary requirement is reliable HomeKit control and automatic recovery without daily manual power cycles.

The Rust workspace contains a portable `no_std` light-control core and a host
simulator. The core tracks intended/applied on/off state and two preset slots,
keeps nominal brightness fixed at 3%, and preserves intent across output errors.
An ESP32-C6 bench application reuses Stillair's Matter-over-Wi-Fi connectivity,
BLE commissioning, and flash persistence. Its light output is simulated; actual
LED control and temperature presets still need wiring and calibration.

## Develop

Install [Rust](https://rustup.rs/), then run from this directory:

```sh
sh scripts/check.sh
cargo run -p key-right-cli -- simulate on preset-2 brightness=100 off on
sh scripts/check-firmware.sh
```

The simulator runs one in-memory session without device or network access.
`brightness=100` demonstrates an ignored brightness write. The 3% target is the
stock light's nominal setting, not a verified PWM duty. `preset-1` and `preset-2`
are slots awaiting actual colour temperatures.

## Project map

- `firmware/core`: portable control logic and behavioral tests.
- `firmware/cli`: host simulator and command-line tests.
- `firmware/app`: separate ESP32-C6 Matter bench firmware workspace.
- `scripts/check.sh`: formatting, Clippy, tests, and smoke test; also run in CI.
- `scripts/check-firmware.sh`: Matter storage tests and ESP32-C6 release build checks.
- [Development and remaining integration](docs/development.md)
- [Specification and validation plan](docs/spec.md)
- [Hardware findings and research](docs/research.md)
- [Reference photos and datasheets](docs/references/README.md)

Hardware plan recorded 2026-09-22; Rust/Matter setup updated 2026-09-23.
