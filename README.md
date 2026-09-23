# Key Right

Rust firmware replacing only the original Elgato Key Light's Realtek controller
with a Seeed XIAO ESP32-C6. It retains the PCA9635, stock LED driver circuitry,
LEDs, and power supply. Matter over Wi-Fi provides local Apple Home control.

The two controls select **3300 K** or **5000 K** at fixed nominal **3%** stock
brightness. Offline emulation of signed vendor firmware establishes the PCA
commands, not measured optical output.

The host checks and both ESP32-C6 release builds pass. **No board has been
modified, flashed, or physically validated.** Physical output, startup,
closed-housing radio performance, and Apple Home operation remain unverified.

## Build and use

- [Printable field guide](docs/field-guides/key-right/key-right-field-guide.pdf)
- [Minimum parts, wiring, and limits](docs/hardware.md)
- [Development and USB instructions](docs/development.md)
- [Physical validation record](docs/validation-record.md)

The controller uses the retained PCA9635 over I²C. It does not lift PCA output
legs or add level translators or output interlocks. Flash and request the pairing
code before connecting any buck wiring. For later USB service, disconnect the
buck's 5 V lead from the XIAO first. The guide identifies board-specific bus/OE
points and nominal voltage before wiring.

## Develop

```sh
sh scripts/check.sh
sh scripts/check-firmware.sh
cargo run -p key-right-cli -- simulate on preset-1 off
python3 scripts/device.py --list
```

`firmware/core` is dependency-free `no_std` state and PCA9635 logic;
`firmware/cli` simulates it without hardware. `firmware/app` has separate real
and bench images. Host tests establish software behavior only, not correct wiring,
safe startup, physical light output, or real-network recovery.

See [requirements](docs/spec.md), [research](docs/research.md), and
[stock firmware evidence](docs/references/firmware-analysis.md). Original photos
and datasheets remain in [docs/references](docs/references/README.md).
