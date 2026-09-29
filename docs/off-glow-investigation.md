# Off-state LED-panel glow

Michael confirms that **the LED panel itself emits the faint orange light**
while Home is Off and lamp power remains connected. It is difficult to see even
at night, with a central dot and faint illumination at the edges and elsewhere.
He explicitly rules out the ESP indicator. The earlier indicator-spill
hypothesis and masking plan are withdrawn; do not ask him to repeat that check.

This establishes unwanted panel emission, but not its electrical cause.
The working possibilities are small off-state current through the stock power
stage, residual control drive, or intermittent activation. The evidence does
not distinguish a steady current from short pulses. Colour alone does not
identify which of the four output connectors is affected.

Firmware **0.1.4 remains prepared, not flashed**. Its animation change retains
0.1.3's settled-Off configuration and is not a demonstrated glow fix.

## Evidence and limits

Both assembled lamps otherwise operate well. This loaded observation takes
precedence over an assumption of darkness from register readback. Both boards
passed all-zero PCA PWM readback. On the first board Michael measured blue/OE
to black/GND at 3.34 V Off and 0.01 V On.

Earlier first-board readings of 0.002–0.008 V across the four **unloaded** LED
connectors do not explain the present assembled behavior. They cannot rule out
a changed drive state, tiny current or brief pulses with the panels fitted.
No loaded current, transistor control voltage or waveform was measured.
The [validation record](validation-record.md) retains those readings.

The [whole-board photo](references/photos/keylight-3622.png) and
[power-stage close-up](references/photos/keylight-3624.png) show the output
transistors and surrounding resistors, but do not establish their pinouts or
a verified gate/source/drain net map. No new transistor solder point is known.

## Firmware assessment

The installed Off path in
[`Pca9635::initialize_off`](../firmware/core/src/pca9635.rs) releases blue/OE
HIGH, programs stock `MODE2=0x14`, writes all sixteen PWM values to zero, wakes
the oscillator and verifies the registers. `LEDOUT0..3` remain `0xaa`.

NXP specifies that OE HIGH with `OUTNE=00` drives LEDn LOW regardless of PWM
and LEDOUT. With OE LOW, inverted push-pull mode and either LEDOUT off or zero
individual PWM also produce LOW. Changing LEDOUT to its named off mode does
not offer a stronger specified LOW than the existing OE-disabled state.
High-impedance OUTNE would release the drive; SLEEP stops the oscillator and is
not an independent cutoff. These are PCA pin semantics, not measurements of
the lamp's external power stage.
[NXP PCA9635 datasheet, §§7.3.1–7.4 and tables 14–16](https://www.nxp.com/docs/en/data-sheet/PCA9635.pdf).

An **OE-to-ground pull-down is not the proposed remedy**: it would oppose the
installed pull-up and enable the PCA. A pull-down at a transistor's control
terminal would be a different modification requiring its actual circuit map.

The stock firmware turns Off by reducing warm/cool commands to zero, and its
all-channel-zero routine clears all sixteen PWM registers. The existing
[signed-image analysis](references/firmware-analysis.md) found no proven extra
power-cut control to reproduce. The Realtek PC_1 observation is not a verified
additional shutdown net. Since the glow was noticed after the controller
replacement, the old and new shutdown/control states remain relevant; do not
assume intrinsic transistor leakage is the explanation.

**There is no evidence-backed firmware glow fix yet.** At the next planned
service, read `status`, `registers` and `verify` after Off has settled. If they
show nonzero PWM, a fault or repeated resets, investigate that before adding a
component. Correct registers establish the commanded state only.

## Electrical candidate to prepare

A **bleeder resistor across an affected LED load** can provide an alternate
path for a small off-state current and reduce voltage across the LEDs. This is
a candidate to test, not a proven repair. It cannot correct a power stage that
is being substantially driven On.

The known connection points are **the two contacts within the same F-1, F-2,
W-1 or W-2 LED connector**, in parallel with its connected panel load. Polarity
does not matter. Do not connect the resistor from an output to black/GND or
between separate connectors. These connector contacts are established; an
alternate pair of convenient PCB solder pads has not been mapped.

Keep **10 kΩ, 0.25 W metal-film resistors** available for a controlled trial.
At a continuous 13.3 V across one resistor, the calculated current is
`13.3 / 10000 = 1.33 mA` and dissipation is
`13.3² / 10000 = 17.7 mW`. These are reference values, not measured leakage or
an established maximum instantaneous/RMS connector voltage in the assembled
lamp. Whether 10 kΩ is sufficient depends on the actual current and the
voltage below which the panel is visually dark. Approximately,
`R < V_dark / I_leak` for a small steady leakage current.

MOSFET drain-source leakage can exist even at zero gate-source voltage; a
stronger gate pull-down does not eliminate that mechanism. Residual gate drive
would require a different diagnosis. This distinction is described in
[Nexperia AN90009, §§2.1–2.2](https://assets.nexperia.com/documents/application-note/AN90009.pdf).
It does not identify the transistor types or current path in these lamps.

At the next planned opening:

1. Follow the [existing power/USB rules](hardware.md) and read the settled-Off
   console state. No new work is requested while the lamps remain assembled.
2. If the software state is correct, a resistor trial across an affected
   connector is the available intervention without inventing transistor pads.
   Fit it only with lamp power and USB removed; insulate both leads and secure
   it away from hot parts and the antenna. This is not an instruction to fit
   four resistors indiscriminately.
3. Reconnect the panels and optical parts with power removed. After reassembly,
   compare Off in a dark room and check normal low-brightness operation.
   Record the affected connector, component and observed result.
4. If the glow remains, do not assume a lower resistance is the answer. A
   targeted investigation of the actual drive/current path is then needed to
   distinguish leakage from residual or intermittent drive.

Routine bench probing is complete. Further electrical measurement would address
this specific observed fault. Without it, a guaranteed component value or gate
pull-down design cannot be established in advance.

## Verified control references

| Net | Verified point |
| --- | --- |
| OE, blue | U4 top row pad 5 from left in the hardware guide's orientation; PCA U3 pin 23; C3 GPIO6 |
| Ground, black | Existing J8 ground connection / C3 `G`; PCA U3 VSS pin 14 |
| Warm control | PCA U3 LED0, pin 6; no convenient transistor-gate pad verified |
| Cool control | PCA U3 LED4, pin 10; no convenient transistor-gate pad verified |

These chip pin numbers are references, not new solder instructions. A gate
pull-down or active cutoff still needs verified transistor connections and
control timing. No such modification is ready to install.
