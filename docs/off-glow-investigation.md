# Off-state orange glow

The first thing to check at the next planned opening is **light from the ESP
board's red indicator leaking through the diffuser**. This is the leading
hypothesis, not a confirmed diagnosis. A small opaque insulating cover may be
all that is needed. Do not add a pull-down or change PCA polarity speculatively.

Michael reports that both assembled lamps work well, but with power connected
and Home set to Off they have a faint orange glow, concentrated near the centre.
This is a new loaded observation. The earlier unloaded Off measurements and
register checks did not establish optical darkness.

## Evidence and likely causes

| Rank | Possible source | Evidence and limitation |
| --- | --- | --- |
| 1 | C3 indicator light reaching the panel/diffuser | Michael previously observed red and blue lights on the replacement board, including a red light that remained lit. The new glow is orange and concentrated centrally. That combination fits a local indicator source, but neither its optical path nor its present source has been observed. |
| 2 | Small current through the retained LED power stage | Possible even when the commanded output is zero. No current measurement with the LED panels connected exists. The previous 0.002–0.008 V across unloaded connectors weighs against a substantial steady output, but does not rule out tiny current under different loading or intermittent pulses. |
| 3 | A control, reference-ground, or power-stage fault | Possible, but repeated correct PCA readback, measured OE levels, and good loaded control make a gross polarity or wiring error less likely. The actual transistor gate voltages and circuit have not been established. |
| 4 | Residual stored charge or startup behavior | Could explain a brief decay or flash, but does not explain a continuing glow by itself. The reported steady Off glow and an unobserved startup flash are separate issues. |

The [actual C3 photo](references/photos/esp32-c3-mini-v1.png) shows small light
emitter packages near the BOOT/RESET area. It does not identify their colour,
GPIO, series resistor, or power connection. No board-vendor schematic matching
this exact revision was found. Generic SuperMini GPIO8 assignments are not
evidence for this board's red LED. Firmware currently leaves unassigned GPIOs
alone. GPIO8 is also a C3 strapping pin, which is another reason not to add an
unverified external pull resistor there. See Espressif's
[C3 schematic checklist](https://docs.espressif.com/projects/esp-hardware-design-guidelines/en/latest/esp32c3/schematic-checklist.html).

The local [whole-board photo](references/photos/keylight-3622.png) and
[power-stage close-up](references/photos/keylight-3624.png) show the four output
transistors and surrounding resistors. They do not supply a verified
gate/source/drain net map. Their small package markings and visible traces are
insufficient to assign a new solder connection.

## What the firmware already does

The installed 0.1.3 Off path in
[`Pca9635::initialize_off`](../firmware/core/src/pca9635.rs) releases blue/OE
HIGH, programs stock `MODE2=0x14`, writes all sixteen PWM values to zero, wakes
the oscillator and verifies the registers. `LEDOUT0..3` remain `0xaa`.
Both boards passed zero-PWM readback. On the first board Michael measured
blue/OE to black/GND at 3.34 V Off and 0.01 V On. The
[validation record](validation-record.md) owns those receipts.

NXP's relevant truth tables say:

| Configuration | PCA output |
| --- | --- |
| OE HIGH, `OUTNE=00`, as in `MODE2=0x14` | LOW, independently of PWM and LEDOUT |
| OE LOW, `INVRT=1`, `OUTDRV=1`, `LEDOUT=00` | LOW |
| OE LOW, `INVRT=1`, `OUTDRV=1`, individual PWM zero | LOW at PWM=0; zero high-time for an external N-type driver |

Thus changing LEDOUT to its named “off” mode would not make the existing
OE-disabled LOW lower. Changing OUTNE to high impedance would release that
drive. SLEEP stops the oscillator; it is not an independent blanking mechanism.
PCA power-on defaults differ from the programmed state, so this describes
settled Off, not a cold-start guarantee.
[NXP PCA9635 datasheet, §§7.3.1–7.4 and tables 14–16](https://www.nxp.com/docs/en/data-sheet/PCA9635.pdf).

**No new PCA-mode change is justified by the current evidence.** In particular,
an OE-to-ground pull-down would oppose the installed pull-up and enable the
PCA. Repeated Off writes cannot extinguish a separately powered indicator.
Firmware cannot turn off a hardwired power LED; a GPIO-controlled indicator
could be disabled only after identifying its actual circuit and inactive level.

The stock firmware also turns Off by reducing the warm/cool commands to zero.
Its all-channel-zero routine clears all sixteen PWM registers. The existing
[signed-image analysis](references/firmware-analysis.md) found no proven extra
power-cut control to reproduce. The old Realtek PC_1 observation is not an
additional verified board net.

## Preferred fix to prepare: cover the indicator

At the next planned reflash, identify the visibly glowing C3 component, then
fit a **small opaque, electrically insulating cover over that emitter**.
Black electrical insulation tape such as Scotch Super 33+ is a practical
candidate. Its manufacturer documents black PVC construction and a 90 °C
continuous operating rating in the linked specification; that is a material
rating, not a measured lamp temperature. Use enough layers to block the visible
light. Ordinary amber polyimide tape may transmit it.
[3M Super 33+ technical data](https://multimedia.3m.com/mws/media/1983315O/scotch-vinyl-electrical-tape-super-33-datasheet-en-eu.pdf).

Keep the patch local to the emitter. Do not wrap the whole board, cover the
power resistors or regulator, cover the antenna, or use conductive foil tape.
There is no electrical pad assignment or firmware change for this fix.
Desoldering a confirmed indicator is an optional later alternative, but its
series resistor is not identified and should not be removed by guesswork.

No new work is needed while the lamps remain assembled. During the planned
opening:

1. Remove lamp power and USB before opening, adjusting wiring or fitting tape.
   Retain the [existing five-wire and VBUS-blocked USB rules](hardware.md).
2. With the opened assembly secured and powered as for flashing, use sight to
   identify which C3 emitter is lit while the light is Off. Do not move
   components or connections while powered. Record the exact emitter, then
   remove power and USB before masking it. Mask another indicator too if it
   is visibly contributing light.
3. Reconnect the LED panels and put the optical parts in position with power
   removed, as already required by this lamp's assembly. Power it, leave Home
   Off, and compare in a dark room. No meter probing is needed for this check.
4. If the glow disappears, record the optical source and cover as the fix.
   If it persists, record whether light is visibly emitted by the actual
   panel LEDs with the controller's indicators blocked. That distinction
   decides whether the electrical fallback is relevant.

This plan does not require repeating the completed bus/OE/connector probing.
It does require a visual comparison before claiming the glow is fixed.

## Conditional electrical fallback

If the panel LEDs themselves continue glowing with the C3 indicators masked,
the remaining question is whether there is small off-state leakage or an
incorrect drive state. MOSFETs can pass drain-source leakage even at zero
gate-source voltage; a stronger gate pull-down does not eliminate that
mechanism. Nexperia describes it separately from gate leakage in
[AN90009, §§2.1–2.2](https://assets.nexperia.com/documents/application-note/AN90009.pdf).
This is a general mechanism, not identification of the parts in these lamps.

A resistor **across the two contacts of an affected LED output**, in parallel
with that LED load, is a reversible candidate for bypassing small leakage.
The known connection points are the two contacts within **F-1, F-2, W-1 or
W-2**. Resistor polarity does not matter. This is not a connection from an
output to black/GND, and it must not bridge two different connectors.

For a later controlled trial, **10 kΩ, 0.25 W, insulated metal-film** is a
reasonable starting component to have available. At the previously measured
13.3 V it would draw `13.3 / 10000 = 1.33 mA` and dissipate
`13.3² / 10000 = 17.7 mW` per connector. Those are reference values for a
continuous 13.3 V across the resistor, not measured leakage or an established
maximum instantaneous/RMS voltage in the assembled lamp. The resistor would
carry small leakage instead of leaving all of it available to the LED string.
The final value depends on the actual leakage and the voltage at which the
string stops visibly emitting, approximately `R < V_dark / I_leak`.

This is **not an instruction to install four resistors now**, nor a proven fix.
A resistor cannot correct a wrongly enabled power stage. If the masked-panel
check points here, first use the existing console `status`, `registers` and
`verify` to confirm settled Off and zero PWM. Further electrical diagnosis is
then a targeted fault investigation, not another routine bench acceptance
sequence. Make or change any resistor connection only with lamp power and USB
removed, insulate both leads and secure it away from the antenna and hot parts.

For a gate pull-down or active output cutoff, the exact transistor pinout,
gate/source nets and needed control timing are still unknown. No such design
is ready to install. The verified accessible nets remain:

| Net | Verified point |
| --- | --- |
| OE, blue | U4 top row pad 5 from left in the hardware guide's orientation; PCA U3 pin 23; C3 GPIO6 |
| Ground, black | Existing J8 ground connection / C3 `G`; PCA U3 VSS pin 14 |
| Warm control | PCA U3 LED0, pin 6; no convenient transistor-gate pad has been verified |
| Cool control | PCA U3 LED4, pin 10; no convenient transistor-gate pad has been verified |

The chip pin numbers above are references, not new solder instructions. A
completed visual source check is the only missing fact needed to choose the
preferred optical fix; a different electrical remedy would need additional
evidence rather than a guessed pad or component value.
