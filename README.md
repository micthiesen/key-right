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

On September 29, Michael reports that both closed-up lamps work well and have a
solid connection without external antennas. This is his current experience,
not a long-term reliability result. Bench probing is complete. A resistor trial
visibly reduced the faint orange panel glow while Off, but residual glow remains.
Whether it disappears after longer Off is unknown. Its electrical cause remains
under [investigation](docs/off-glow-investigation.md); zero PWM does not establish
optical darkness.

Board A runs **0.1.4**; board B remains on **0.1.3**. Version 0.1.4 adds
400 ms eased power, brightness and temperature transitions while Home continues
to report selected targets immediately. Output-frame fades reach zero;
intermediate frames never write flash. Visual fades and detailed loaded startup
remain unverified. Low-output steps remain limited by the PCA's 8-bit resolution.
The [validation record](docs/validation-record.md) separates installed-image
results, user observations and remaining checks.

The Rust application targets `esp32c3`. The [spec](docs/spec.md) records the
single-light control contract. Build results and pending checks are recorded in
[software validation](docs/software-validation.md).

## Build and use

- [Installed board, five-wire map, measurements, and limits](docs/hardware.md)
- [Initial bench power, chip inspection, flashing, and probing](docs/bench-bring-up.md)
- [Development and USB instructions](docs/development.md)
- [Physical validation record](docs/validation-record.md)
- [Off-glow diagnosis and next-opening fix plan](docs/off-glow-investigation.md)

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
