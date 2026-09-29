# Key Right

Rust firmware replacing only the original Elgato Key Light's Realtek controller
with an ESP32-C3_MINI_V1. It retains the PCA9635, stock LED driver circuitry,
LEDs, and power supply. Matter over Wi-Fi provides local Apple Home control.

Each lamp exposes one Matter Color Temperature Light with power, brightness,
and white-temperature controls. Home's **1% through 100%** brightness maps to
stock nominal **1% through 10%**; Home 100% is the selected low-light ceiling.
The initial level is about Home 22%, reproducing the former stock nominal 3%
output. Temperature spans 143 through 344 mired (about 6993 through 2907 K).

Pair two lamps separately, then group them in Apple Home to control brightness
and temperature together. The former 3300 K and 5000 K choices can be Home
scenes. Grouping does not guarantee simultaneous output. Stock firmware evidence
defines PCA commands, not measured brightness or Kelvin, and low-output steps
are limited by eight-bit PWM quantization.

**Wiring is complete as reported on 2026-09-28; the LEDs remain disconnected.**
The assembly is ready for bench-PSU bring-up and flashing preparation. Powered
bench operation, flashing, physical output, startup, closed-housing radio
performance, and Apple Home operation remain unverified.

The Rust application targets `esp32c3`. The [spec](docs/spec.md) records the
single-light control contract. Build results and pending checks are recorded in
[software validation](docs/software-validation.md).

## Build and use

- [Installed board, five-wire map, measurements, and limits](docs/hardware.md)
- [Initial bench power, chip inspection, flashing, and probing](docs/bench-bring-up.md)
- [Development and USB instructions](docs/development.md)
- [Physical validation record](docs/validation-record.md)

The controller uses the retained PCA9635 over I²C. It does not lift PCA output
legs or add level translators or output interlocks. GPIO4/5/6 provide SDA/SCL/OE;
the board takes power directly from the lamp's measured 3.37 V rail. With lamp
wiring attached, USB must block VBUS/5 V while retaining data and ground.
Ordinary powered USB requires disconnecting all five lamp wires first.

## Develop

```sh
sh scripts/check.sh
sh scripts/check-firmware.sh
python3 scripts/device.py --list
```

`firmware/core` is dependency-free `no_std` state and PCA9635 logic;
`firmware/cli` simulates it without hardware. `firmware/app` has separate real
and bench C3 images. Host tests establish software behavior
only, not correct wiring, safe startup, physical light output, or real-network
recovery.

See [requirements](docs/spec.md), [research](docs/research.md), and
[stock firmware evidence](docs/references/firmware-analysis.md). Original photos
and datasheets remain in [docs/references](docs/references/README.md).
