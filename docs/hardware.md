# Key Right hardware plan

Selected 2026-09-23. This is a conditional assembly plan for the ESP32-C6 controller. It retains the original PCA9635 and the stock LED driver/current-limit/power circuitry, but disconnects the two PCA PWM outputs from the stock driver and drives those inputs directly from XIAO PWM through one fail-safe level translator. It does not certify physical operation; board-specific voltages, driver input mapping, PWM polarity, and RF performance must be measured before the light is enabled.

## Exact recommended parts

| Qty | Exact part | Purpose and verified details |
| --- | --- | --- |
| 1 | Seeed Studio XIAO ESP32-C6, SKU 113991254 | ESP32-C6FH4, 4 MB flash, native USB Serial/JTAG, D3=GPIO21, D9=GPIO20, D10=GPIO18. Official product: https://www.seeedstudio.com/Seeed-Studio-XIAO-ESP32C6-p-5884.html. Board pinout/power/antenna instructions: https://wiki.seeedstudio.com/xiao_esp32c6_getting_started/ |
| 1 | TI TXU0102DCUR | 2-channel, fixed-direction A-to-B translator with 3-state outputs, VSSOP-8 (DCU), dual 1.1–5.5 V rails. Active part. Official product/order page: https://www.ti.com/product/TXU0102/part-details/TXU0102DCUR. Datasheet: https://www.ti.com/lit/ds/symlink/txu0102.pdf |
| 1 | Chip Quik PA0042C | Compact VSSOP-8 (0.5 mm pitch) to DIP-8 adapter, 10.16 × 10.16 × 1.6 mm FR-4, preinstalled DIP pins; suitable for perfboard. Fine-pitch IC soldering is still required. Official listing showed in stock when checked: https://www.chipquik.com/store/product_info.php?products_id=4600046 |
| 1 | Pololu D24V25F5, item 2850 | 5 V, 2.5 A step-down regulator, 6–38 V input, reverse-voltage protection; 0.7 × 0.7 × 0.35 in board. Pololu lists active, purchase/backorder enabled, no exact stock count: https://www.pololu.com/product/2850/specs |
| 1 | Vishay 1N5822-E3/54 | Axial DO-201AD Schottky diode, 40 V, 3 A; follow Seeed’s instruction for external 5 V input to the XIAO. Cathode band faces XIAO 5V. Official datasheet: https://www.vishay.com/docs/88526/1n5820.pdf |
| 1 | Taoglas FXP73.07.0100A | External 2.4 GHz flex PCB antenna, 47 × 7 × 0.1 mm, 100 mm Ø1.13 coax, I-PEX MHF I / U.FL-compatible connector. Official datasheet: https://www.taoglas.com/datasheets/FXP73.07.0100A.pdf |
| 1 | Littelfuse 0251002.MXL, 2 A axial PICO II fuse | Install close to the +13 V takeoff in the added buck branch, with insulated leads. Protects added wiring; not a precision semiconductor current limiter. 125 V DC rating. Datasheet: https://www.littelfuse.com/assetdocs/littelfuse_fuse_251_253_datasheet.pdf?assetguid=f47a0bb7-8ede-4679-9646-7114c3787688 |
| 1 | Harwin M20-9990246 2-pin header + M7567-05 shunt, 3 A | Install in series with buck positive output feeding XIAO 5V. OPEN whenever USB is connected; close only with USB unplugged. https://www.harwin.com/products/M20-9990246 and https://www.harwin.com/products/M7567-05 |
| 5 | Vishay MRS25000C1002FCT00, 10 kΩ, 1%, 0.6 W axial resistors | One pulldown on each XIAO PWM input, one on TXU OE, and one on each stock driver input pad. https://www.vishay.com/en/product/28724/ |
| 2 | Vishay MRS25000C1001FCT00, 1 kΩ, 1%, 0.6 W axial resistors | One series resistor between each TXU B output and its driver pad; temporarily reuse one for the static input-load screen before final wiring. https://www.vishay.com/en/product/28724/ |
| 2 | Vishay K104K15X7RF5TL2, 100 nF, 50 V, X7R radial capacitors | One at TXU VCCA-to-GND, one at VCCB-to-GND. https://www.vishay.com/doc?45171= |
| As needed | 22 AWG stranded insulated wire for power/GND; 30 AWG insulated wire for fine signal pads; 2.54 mm isolated-pad perfboard; heat-shrink; polyimide tape; nylon standoffs and strain relief | Secure the daughterboard and prevent exposed conductors from touching the metal lamp body. |

Availability can change. Reconfirm stock at purchase time. The user's original 13 V, 4 A supply is reused; do not assume its plug polarity from convention or a photograph.

## Direct PWM and level translation

The PCA9635 stays on the board but its LED0 and LED4 pins are lifted from the PCB and isolated from the two stock driver-input pads. The TXU0102 drives those pads from ESP32-C6 LEDC PWM, preserving the stock LED power and current-limiting circuitry. PCA I²C and OE do not participate in this output path. The TXU's OE pin is the only hardware output-enable signal.

### Pin table, TXU0102 VSSOP-8 top view

| TXU pin | Name | Connection |
| ---: | --- | --- |
| 1 | B2Y | Through 1 kΩ to vacated PCB pad from U3 pin 10, LED4 / cool driver input |
| 2 | GND | Verified board ground |
| 3 | VCCA | XIAO 3V3 |
| 4 | A2 | XIAO D9 / GPIO20 PWM for cool channel |
| 5 | A1 | XIAO D10 / GPIO18 PWM for warm channel |
| 6 | OE | XIAO D3 / GPIO21, active high |
| 7 | VCCB | Measured PCA9635 VDD / stock driver logic-high rail, only if the measured normal PCA rail is 2.3–5.5 V |
| 8 | B1Y | Through 1 kΩ to vacated PCB pad from U3 pin 6, LED0 / warm driver input |

TXU0102 is non-inverting. The signed stock firmware uses active-high, 8-bit PWM: warm/cool = 6/2 at 3300 K and 3/6 at 5000 K, both at nominal 3%. The ESP uses these raw counts, not 3% duty. Its 80 MHz LEDC clock and Q8 divider 819 give about 97,680.10 Hz, 0.0244% above the PCA nominal 97,656.25 Hz. Confirm high-level voltage, polarity and duty at the driver pads before reconnecting the LEDs. These reproduce the stock digital command; actual optical brightness/CCT remain unmeasured.

Place these five 10 kΩ pulldowns:

1. GPIO18/D10 to verified GND.
2. GPIO20/D9 to verified GND.
3. TXU OE pin 6 to verified GND.
4. B1Y/warm driver-input pad to GND; the powered disabled-output test must confirm this is physically off.
5. B2Y/cool driver-input pad to GND, subject to the same powered test.

The two driver-input pull-downs hold the lamp off while TXU outputs are Hi-Z and discharge the driver inputs. The TXU0102 also has internal 5 MΩ pulldowns on its data and control inputs. Place the two 100 nF bypass capacitors close to TXU pins 3/2 and 7/2.

Keep the two 1 kΩ series resistors permanently in the B-output paths, with the
10 kΩ pulldowns on the **driver-pad side**. The series resistors limit a capacitive
charge/discharge or short-to-ground load to at most 5.5 mA over the accepted rail
range, below TXU's lowest applicable 8 mA recommended drive at 2.3 V. They also
form a nominal 0.909 divider with the pulldowns. They can slow pulses into an
unknown capacitive load, so verify the shortest ~80 ns pulse at the actual driver
pad. If pulse integrity or optical equivalence fails, stop for a reviewed stronger
buffer design; do not bypass these resistors or increase duty to hide the problem.

For this XIAO, GPIO18, GPIO20, and GPIO21 are not ESP32-C6 strapping pins. Avoid GPIO4, GPIO5, GPIO8, GPIO9, and GPIO15, which are strap pins; GPIO3/GPIO14 are reserved by Seeed's external antenna switch instructions.

### Reset and rail-failure behavior

TI specifies both TXU supplies from 1.08 V to 5.5 V; data/OE input voltages from 0 to 5.5 V; OE LOW means both B outputs are high impedance. If either VCCA or VCCB is below 100 mV or disconnected, outputs are disabled/Hi-Z. TI also specifies partial-power-down Ioff (maximum ±2.5 µA over full temperature for the table conditions) and glitch-free power sequencing in either supply order. This supports 3.3 V XIAO signals while the stock PCA rail is off without relying on an analog switch's unqualified control-input behavior.

At reset, GPIO18/GPIO20 become high impedance and GPIO21 must remain low; the five external 10 kΩ resistors hold the PWM inputs/OE/driver nodes low. Firmware must set OE low first, initialize both PWM channels to the safe-off/nominal startup intent, and raise OE only after state/profile validation. On any detected fault or incomplete state, lower OE. If the stock rail returns while the XIAO remains running and OE is still high, TXU resumes the current ESP PWM state rather than passing the PCA9635 POR output; therefore firmware PWM must remain the acknowledged intent. TXU rail sequencing is glitch-free, but a halted MCU whose GPIO21 is stuck high is not an independently latched fault case.

### Fine-pitch work is conditional

PCA9635 TSSOP-28 pin 6 is LED0/warm, pin 10 is LED4/cool, pin 14 is VSS, pin 23 is active-low OE, and pin 28 is VDD. Datasheet top view: https://www.nxp.com/docs/en/data-sheet/PCA9635.pdf. Existing PCB photos do not resolve the package pin-1 mark well enough for a trustworthy pin overlay. Find the real dot/notch under magnification and orient to the datasheet before lifting pins 6 and 10. Lift only the IC legs; do not cut LED traces or bypass the existing driver/current-limit parts. If pin-1 orientation or the two downstream pads cannot be identified with confidence, stop.

Before connecting TXU B outputs, unpowered continuity checks must establish separation from the lifted PCA legs and lack of shorts to power. Continuity alone cannot prove polarity or current load. With outputs disconnected, use a 10 kΩ pulldown on each driver pad, then power the stock board with LEDs disconnected and measure each pad. Each must stay below 0.1 V. This bounds an opposing pull-up to about 10 µA under that test condition; it does not characterize dynamic input capacitance. The final scope test must establish clean full-swing pulses under load. TXU VCCB must match the original logic rail, within 2.3–5.5 V. If a pad is tied to the LED supply, an unknown active circuit, or a voltage above 5.5 V, stop and remap; do not connect a TXU output.

Next, with LED leads and TXU outputs still disconnected, temporarily feed one
driver pad from measured stock VDD through 1 kΩ, retaining its 10 kΩ pulldown.
Wire/unwire only with power removed. Power the stock board and measure the pad:
require at least **0.88 × VDD** (2.90 V at 3.3 V; 4.40 V at 5.0 V). Repeat on the
other pad. This screens out a substantial DC sink without exposing the translator
to it. Remove the temporary VDD feed before installing either TXU output. A pass
does not prove dynamic loading; the series resistors and scope test remain required.

## Power and USB branch

Wire the user's measured +13 V/GND through the 2 A axial fuse to Pololu D24V25F5 VIN/GND. The regulator's 5 V output passes through the 1N5822 (anode toward buck, banded cathode toward XIAO), then through the removable 2-pin shunt, to XIAO 5V. Share ground. The diode drop is intentional and follows Seeed's external-input guidance. Leave TXU VCCA on XIAO 3V3 and VCCB on the measured stock logic rail.

Use two exclusive XIAO power modes:

- **USB programming:** open the 5 V shunt before connecting USB. USB powers XIAO; the 13 V supply may still power the stock PCA/driver board. After the isolation and staged-test checks pass, light output can be tested in this mode with the stock 13 V supply connected.
- **Standalone light operation:** disconnect USB, then close the shunt. The buck feeds XIAO through the series diode. The stock board continues to receive the 13 V adapter.

Do not claim simultaneous USB and buck power to the XIAO is safe. Seeed's external 5 V diode instruction and schematic do not certify USB-C VBUS isolation for this assembly; the removable shunt is the physical source selector.

Safe measurement sequence:

1. Disconnect USB and the lamp supply before opening the enclosure or wiring. With the 13 V adapter unplugged from the lamp, measure its output voltage and polarity at the loose plug. Label positive/negative. Stop if they do not match the lamp input marking/spec.
2. With the board unpowered and capacitors discharged, identify PCA U3 pins from the physical pin-1 mark and datasheet. Check continuity from U3 pin 14 to ground; do not use an adjacent unknown pad as a ground reference.
3. With insulated probe tips, power the unmodified stock board from the verified 13 V supply and measure U3 pin 28 VDD relative to pin 14 VSS. Then measure/inspect the stock driver-input high level. Continue only if VCCB is 2.3–5.5 V. If the original controller cannot produce a useful signal, the disabled-pad and staged PWM tests below replace the stock waveform capture; do not guess a different voltage or polarity. Do not probe between adjacent IC legs with an exposed long probe tip.
4. Test buck output separately with the XIAO disconnected: verify polarity and 5 V before the diode; the unloaded post-diode reading is not a reliable loaded-voltage test. Under XIAO load, verify the input stays within Seeed’s supported external supply range and the 3V3 rail remains stable. Check that the 5 V shunt is open before plugging USB.
5. Before first modified-board power-up, verify all five pulldowns, both capacitor placements, no solder bridges, and each lifted PCA leg is isolated from its vacated pad. Keep GPIO21 low. With stock LED supply available, check that both downstream pads stay low and the light stays off. Stop immediately on unexpected voltage, any light with OE low, heating, smell, or unstable power.

## Antenna and metal housing

The XIAO has an onboard ceramic antenna and a selectable U.FL port. Use the Taoglas FXP73.07.0100A on the U.FL port, mounted on a nonmetal RF window/cover or routed outside the metal cavity; do not place the flex antenna behind continuous metal. The antenna datasheet gives no minimum metal clearance for this installation, so final closed-housing Wi-Fi and BLE commissioning must be measured. Seeed's XIAO C6 instructions select the external path by setting GPIO3 LOW, then GPIO14 HIGH. Confirm this on the exact board revision. Do not connect/disconnect the antenna while the radio is active.

## Physical acceptance record

Record measured adapter polarity/voltage, U3 VDD, stock driver-input high voltage and safe-off reference, verified PCA pins and pin-1 orientation, PWM frequency/polarity/duties at both driver pads, light-off reset behavior, current/thermal observations, and Wi-Fi/BLE commissioning with the metal housing closed. Host simulation cannot establish any of these results.

## Build and records

Follow the printable [field guide](field-guides/key-right/key-right-field-guide.pdf) in order. Save each light’s measurements in a private copy of [the acceptance record](validation-record.md). The stock firmware evidence and reproducible emulator are in [firmware-analysis.md](references/firmware-analysis.md). No physical validation has been performed by the agent.

The previous PCA I²C/OE proposal is superseded. PCA power-on MODE2=0x05 can drive an output high even with OE high. Direct ESP PWM through TXU avoids that source of startup output; an analog switch around PCA would not protect against the PCA rail cycling independently. This design still requires real startup, rail-loss, pulse-integrity, thermal and recovery tests.
