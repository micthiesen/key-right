# Hardware plan

The replacement removes only the original Realtek controller module. Keep the
PCA9635, its LED driver circuitry, the LED panels, and the 13 V / 4 A adapter.
The ESP32-C6 talks to the PCA9635 over I²C. No output leg is lifted and no added
output interlock is part of this design. The photographed board does not identify
safe connection pads or prove startup behavior; use the field guide's stop
conditions and do not treat the plan as physically validated.

## Minimum parts

| Qty | Part | Use |
| --- | --- | --- |
| 1 | Seeed Studio XIAO ESP32-C6, SKU 113991254 | 4 MB flash; native USB; official [product page](https://www.seeedstudio.com/Seeed-Studio-XIAO-ESP32C6-p-5884.html) and [pin/power guide](https://wiki.seeedstudio.com/xiao_esp32c6_getting_started/) |
| 1 | Pololu D24V25F5, item 2850 | 5 V buck from the existing 13 V supply; 6–38 V input. [Manufacturer specifications](https://www.pololu.com/product/2850/specs) |
| as needed | Insulated hookup wire, insulated mounting, heat-shrink/strain relief | Secure the XIAO and avoid contact with the metal housing |

No external antenna, fuse, jumper, diode, level shifter, pull-downs, or output
buffer is included in the baseline. The user's original adapter powers both the
stock lamp and the buck. Do not assume the adapter plug polarity; check its label
or the field guide before wiring.

## Power

Connect the existing adapter's verified +13 V and GND to the buck input. Verify
the buck's fixed 5 V output before connecting the XIAO. Connect buck 5 V to
the XIAO 5V pin and buck GND to XIAO GND. The stock board and XIAO share GND.
Do not connect the stock PCA VDD/3.3 V rail to XIAO 3V3.

The Seeed schematic connects the XIAO 5V pin directly to USB VBUS. Flash the
XIAO and request its pairing code over USB before connecting any buck wires.
Unplug USB before attaching the buck. For later USB service, disconnect the
buck's 5 V lead from the XIAO first; unplug USB before reconnecting that lead.
This excludes simultaneous sources without a jumper or series diode. The 13 V
adapter may remain connected during later USB service once the buck's 5 V lead
has been removed from the XIAO.

Official references: [XIAO ESP32-C6 schematic](https://files.seeedstudio.com/wiki/SeeedStudio-XIAO-ESP32C6/XIAO-ESP32-C6_v1.0_SCH_PDF_24028.pdf),
[Seeed power/pin guidance](https://wiki.seeedstudio.com/xiao_esp32c6_getting_started/).
Seeed recommends a diode for external input on the 5V/VBUS net. For this build,
the buck's 5 V lead is physically disconnected before USB is attached; with only
one source connected, the diode is not required to power the XIAO. This is based
on the schematic's shared 5V/VBUS net and the exclusive connection sequence.

## PCA9635 bus and optional OE connection

Stock firmware analysis identifies PCA9635 U3 at address `0x15`, 100 kHz, with
SDA on PCA pin 27, SCL on pin 26, active-low OE on pin 23, LED0/warm on pin 6, and
LED4/cool on pin 10. Software routes the selected XIAO GPIOs as:

| XIAO pad | ESP32-C6 GPIO | Function |
| --- | ---: | --- |
| D10 | 18 | SDA |
| D9 | 20 | SCL |
| D3 | 21 | Optional PCA OE; active low |

These are the project's explicit I²C GPIO choices. D10/D9 are not the XIAO
board's documented default SDA/SCL pads. The hardware bus connection must be
found from the actual board, preferably at the former module's bus pads only
after continuity to U3 pins 27/26 is verified. Photos and the DK9169 firmware
do not prove a convenient board-pad mapping or prove the former module pads are
still usable after removal. Do not guess by silkscreen or orientation.

Before connecting GPIO18/20, verify with the lamp powered and XIAO disconnected
that idle SDA/SCL levels are nominally 3.3 V; do not connect them to a 5 V bus.
If the bus does not communicate after wiring, investigate its existing pull-ups
before adding any parts. No pull-up or level-shifter parts are in the baseline.

PCA OE must have a known electrical state. If the actual board already ties OE
low after module removal, it may remain there and D3 is unused. Otherwise GPIO21/D3
may connect to U3 OE pin 23 only after the net and board access point are verified
and confirmed exclusive; firmware drives HIGH to disable and LOW to enable. If OE
is floating or its route is unknown, stop and trace it. Do not assume a floating
OE will enable the PCA, and do not tie it to a rail based on a photograph.

## Startup and operating limits

The signed stock firmware initializes PCA mode registers and later clears PWM
registers. Firmware tests do not establish what the lamp does during cold start,
reset, brownout, or failed I²C. This design has no independent output cutoff; its
startup and fault behavior remains physically unverified.

The board's onboard antenna is the default; no external antenna is required or
selected. Test Wi-Fi and BLE after the cover is closed. A metal enclosure may
weaken radio performance; if the closed assembly cannot commission reliably,
record that finding before choosing any antenna change.

## Minimum physical checks

Follow the printable field guide. Identify U3 pin 1 and the actual SDA/SCL points;
verify their connection to U3 pins 27/26 and nominal 3.3 V bus levels. Verify OE
is tied low or connect it to D3 only on a proven OE net. Verify
adapter polarity and buck output, then test Off, both presets, a power cycle, and
Apple Home with the housing closed. No hardware has been modified or tested by
the agent.

The earlier TXU0102/direct-PWM design in historical notes is abandoned. Do not
lift PCA pins 6 or 10, install a translator, or follow the old direct-PWM BOM.
