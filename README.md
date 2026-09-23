# Key Right

Rust firmware replacing the original full-size Elgato Key Light's Realtek
controller with a Seeed XIAO ESP32-C6. It retains the stock LED power stages and
provides local Apple Home control through Stillair's Matter-over-Wi-Fi stack.

Two mutually exclusive light controls select **3300 K** or **5000 K**, both at
the stock **nominal 3%** setting. Offline emulation of the signed vendor firmware
establishes the warm/cool PWM counts; optical output is still unmeasured.

The real firmware builds and passes host behavior tests. **No board has been
modified, flashed or physically validated.** Assembly, electrical checks,
Apple Home pairing, fault injection and the seven-day soak are in the guide.

## Assemble and use

- [Printable field guide](docs/field-guides/key-right/key-right-field-guide.pdf)
- [Exact parts and wiring](docs/hardware.md)
- [Build, flash, USB commands and recovery](docs/development.md)
- [Per-light acceptance record](docs/validation-record.md)

The selected circuit drives the stock driver pads through a TXU0102 level
translator, with the PCA9635's two output legs lifted and insulated. The translator
isolates the outputs during startup. Follow the guide's measurements and power
jumper rules before enabling either preset.

## Develop

```sh
sh scripts/check.sh
sh scripts/check-firmware.sh
cargo run -p key-right-cli -- profile inspect hardware/profiles/stock-3300-5000.hex
python3 scripts/device.py --list
```

`firmware/core` is dependency-free `no_std` state/profile logic; `firmware/cli`
simulates it without hardware. `firmware/app` has separate real and bench binaries.
The gates run formatting, strict Clippy, Rust/Python tests and both MCU builds.

See [requirements](docs/spec.md), [research](docs/research.md),
[stock firmware evidence](docs/references/firmware-analysis.md) and
[original photos/datasheets](docs/references/README.md). Firmware build and bench
tests establish neither electrical safety nor real-network recovery.
