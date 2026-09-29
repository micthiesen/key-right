# Key Right

Rust firmware replacing only the original Elgato Key Light's Realtek controller
with an ESP32-C3_MINI_V1. It retains the PCA9635, stock LED driver circuitry,
LEDs, and power supply. Matter over Wi-Fi provides local Apple Home control.

The two controls select **3300 K** or **5000 K** at fixed nominal **3%** stock
brightness. Offline emulation of signed vendor firmware establishes the PCA
commands, not measured optical output.

**Wiring is complete as reported on 2026-09-28; the LEDs remain disconnected.**
The assembly is ready for bench-PSU bring-up and flashing preparation. Powered
bench operation, flashing, physical output, startup, closed-housing radio
performance, and Apple Home operation remain unverified.

The existing firmware and flash helper still target the old XIAO ESP32-C6.
**Do not flash those images onto the C3.** The [spec](docs/spec.md) records the
required C3 port while preserving the existing Rust firmware behavior.

## Build and use

- [Installed board, five-wire map, measurements, and limits](docs/hardware.md)
- [Development and USB instructions](docs/development.md)
- [Physical validation record](docs/validation-record.md)

The controller uses the retained PCA9635 over I²C. It does not lift PCA output
legs or add level translators or output interlocks. GPIO4/5/6 provide SDA/SCL/OE;
the board takes power directly from the lamp's measured 3.37 V rail. With lamp
wiring attached, USB must block VBUS/5 V while retaining data and ground.
Ordinary powered USB requires disconnecting all five lamp wires first.

The [old printable field guide](docs/field-guides/key-right/README.md) is historical
C6/buck reference material and does not describe this assembly.

## Develop

```sh
sh scripts/check.sh
sh scripts/check-firmware.sh
cargo run -p key-right-cli -- simulate on preset-1 off
python3 scripts/device.py --list
```

`firmware/core` is dependency-free `no_std` state and PCA9635 logic;
`firmware/cli` simulates it without hardware. `firmware/app` has separate real
and bench C6 images pending the C3 port. Host tests establish software behavior
only, not correct wiring, safe startup, physical light output, or real-network
recovery.

See [requirements](docs/spec.md), [research](docs/research.md), and
[stock firmware evidence](docs/references/firmware-analysis.md). Original photos
and datasheets remain in [docs/references](docs/references/README.md).
