# Physical validation record

This record covers Michael's original full-size Elgato Key Light and its wired
**ESP32-C3_MINI_V1** replacement controller. Keep pairing codes and Wi-Fi
credentials out of this file. Distinguish reported measurements, firmware
evidence, controller acknowledgements, and observed light output.

## Reported baseline, 2026-09-28

The measurements below were recorded in the
[handoff at commit `a617f1e`](https://github.com/micthiesen/key-right/blob/a617f1e77529d3fb66339c6e1a4e7b3636a03fcf/docs/handoff).
Its sources were Michael's completed probing worksheet and
`Elgato_Key_Light_C3_Wiring_Field_Guide.pdf`, which are not present in this
repository. The precise measurement dates and instrument details were not
supplied. These are user-reported observations, not agent measurements or a
first-power pass for the completed assembly.

| Item | Reported observation |
| --- | --- |
| Replacement board | Blue `ESP32-C3_MINI_V1`, USB-C, BOOT/RESET, bare ESP32-C3 package |
| DC input | 13 V |
| PCA9635 VDD | 3.37 V |
| Idle SDA / SCL / OE | 3.37 V each |
| SDA / SCL / OE to VDD | Approximately 9.9 kΩ each |
| SDA to SCL | Approximately 20 kΩ |
| VDD to GND, unpowered | Approximately 1 kΩ |
| IC pin to selected connection pad | Approximately 0–0.1 Ω |
| Rocker | Already bypassed into working ON state |

The reported readings support approximately 10 kΩ existing pull-ups and direct
use of the stock regulated rail. They do not establish rail stability under
radio load or the PCA's physical output behavior.

Michael separately reported on **2026-09-28** that wiring was finished and the
assembly was ready for bench-PSU testing and flashing. **The LED panels are not
connected.** The completed five-wire map is documented in
[hardware.md](hardware.md): J6/DEBUG 3.37 V to `3.3`, J8/UART ground to `G`,
U4 SDA to GPIO4, U4 SCL to GPIO5, and U4 OE to GPIO6. The installed design has no
buck converter or added pull-ups. No successful power-up, flash, USB session,
I²C exchange, or optical test of the completed assembly has been reported.

## Bench session pending, LEDs disconnected

The current MCU application and flash helper target the C6. Complete and verify
the C3 port before using them with this board; see
[development.md](development.md). With lamp wires attached, USB must block
VBUS/5 V while retaining data and ground. Ordinary powered USB requires
disconnecting all five lamp wires first, even when lamp power is off.

Record actual values and outcomes in the following table during the bench
session. `Pending` is not a pass. The current limit has not been specified and
must be recorded as the chosen bench setting, not inferred from the adapter's
4 A rating.

| Check or session detail | Result |
| --- | --- |
| Date, operator, stock-board revision | Pending |
| C3 chip identity and detected flash ID/capacity | Pending; plan assumes 4 MiB and no PSRAM |
| C3 firmware commit, build target, image/partition fit | Pending |
| Bench-PSU connection/polarity, voltage setting, current limit | Pending; reported original input is 13 V |
| Power/USB isolation arrangement used | Pending |
| First power-up, current draw, 3.3 V rail under load | Pending |
| Flashing, native USB console, boot/reset reasons | Pending |
| GPIO4/5 assignment and 100 kHz bus operation | Pending |
| GPIO6 open-drain release HIGH before output enable; LOW enables configured outputs | Pending |
| PCA individual address `0x15`, setup writes and critical-register readback | Pending; address/configuration are stock-firmware evidence |
| BLE/Wi-Fi activity, loaded rail, brownout/reset behavior | Pending |

## LED-connected acceptance pending

Reconnect the LED panels with power removed only after the preceding bench work
supports proceeding. Record the firmware commit and actual observations for
each check. Tests with disconnected LEDs cannot fill these rows.

| Check | Observation |
| --- | --- |
| Firmware commit and LED reconnection date | Pending |
| Off after application initialization | Pending |
| 3300 K at nominal 3%, warm/cool raw PCA `6/2` | Pending |
| 5000 K at nominal 3%, warm/cool raw PCA `3/6` | Pending |
| LED0 warm / LED4 cool physical behavior | Pending |
| OE HIGH with `MODE2=0x14` and the actual external driver stage | Pending |
| Cold power-up, MCU reset, and any startup flash | Pending |
| Power-cycle persistence and intended/applied/output state agreement | Pending |
| Installed antenna arrangement and closed-housing BLE/Apple Home pairing | Pending |
| Closed-housing control of both presets and Off | Pending |
| Short Wi-Fi/AP interruption and automatic recovery preserving intent | Pending |
| USB removed, ordinary lamp-powered operation | Pending |

## Interpretation

[Stock-firmware emulation](references/firmware-analysis.md) establishes address,
channel, configuration, and preset command evidence. It does not establish
measured brightness, color temperature, flicker, LED current, temperature,
startup behavior, or radio/network reliability on this assembly. There are no
connected voltage, current, or optical sensors; fixed values and register
readback must not be reported as live physical telemetry.

Keep observations direct, including unexpected light, bus failures, reset loops,
and radio failures. Record skipped checks and unresolved contradictions. Do not
claim safe startup from a successful software build, an OE level, or a PCA
acknowledgement. Do not claim long-term reliability from a single bench session.
