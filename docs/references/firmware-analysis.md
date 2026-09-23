# Original Key Light firmware evidence

Researched 2026-09-23. Applies to the original Key Light, board type **53**,
firmware **1.0.3, build 222**. This is offline software evidence. No PCA register
readback, continuity, voltage, oscilloscope, optical, or cold-start measurement
was performed.

## Results that can guide the replacement

| Item | Finding | Confidence and limit |
| --- | --- | --- |
| PCA address | Seven-bit `0x15`, 100 kHz | High for this firmware's PCA9635 branch; verify the physical board |
| Bus pins | Realtek `PC_4` SDA, `PC_5` SCL | High software evidence; DK9169 pads **10 SDA**, **9 SCL** are datasheet-derived candidates |
| Active channels | PCA LED0 and LED4, registers `0x02` and `0x06` | High for the light output callback |
| Temperature direction | LED0 is the warm contribution; LED4 is the cool contribution | Traced and emulated through the stock wrapper and transition; confirm LED string connections |
| Output mode | `MODE2=0x14`: inverted, push-pull, update on STOP, OE-high outputs low | High software evidence; external transistor/net polarity still needs probing |
| 3300 K preset | Stock API temperature `303`, brightness `3`: **PWM0=6, PWM4=2** | Executed original wrapper, transition, arithmetic and driver bytes in an emulator |
| 5000 K preset | Stock API temperature `200`, brightness `3`: **PWM0=3, PWM4=6** | Executed original wrapper, transition, arithmetic and driver bytes in an emulator |
| Off | The driver's all-channel-zero routine writes zero to all sixteen PWM registers | Emulated; normal power-off also requests a transition to zero |
| Extra control candidate | Realtek `PC_1`, DK9169 pad **14**, is configured as push-pull output before PCA initialization | High software evidence; its electrical function and output level are unknown |

`303` mired represents approximately 3300.33 K; it is the stock integer encoding
for the requested 3300 K preset. The two requested temperatures were confirmed
by the user. They were not inferred from remembered device presets.

## Firmware provenance and reproduction

The firmware came from Elgato's signed Control Center 1.7.1 macOS distribution:

- [Official release notes](https://help.elgato.com/hc/en-us/articles/29959336411277-Elgato-Control-Center-1-7-1-Release-Notes-macOS)
- [Official application ZIP](https://edge.elgato.com/egc/macos/eccm/1.7.1/ElgatoControlCenter-1.7.1.20508.app.zip)
- ZIP member: `Elgato Control Center.app/Contents/Resources/Firmware_Key_Light.bin`.
- File size: `698901` bytes.
- SHA-256: `57f48194dbec6ee989db6536a6cc0008cde84180301f73d1e7cd4935e3eb6c83`.
- Elgato header: board `53`, version `(1,0,3)`, build `222`, payload offset `128`,
  payload length `698773`, signature slot `2`.

The [independent Elgato container parser](https://github.com/schlarpc/elgato-key-light-mini-firmware-re/blob/main/tools/elgato_fw.py)
reported the image's Ed25519 signature **VALID**. Its OTA1 unpacker did not handle
this older RTKWin payload, so the two loadable segments were extracted directly.
The repository's reproduction script checks the exact hash; it does not repeat
signature verification or download anything. No proprietary image is stored here.

| Segment | File header offset | File data offset | Length | Load address |
| --- | --- | --- | --- | --- |
| 1 | `0x80` | `0x90` | `0x22b40` = 142144 | `0x10006000` |
| 2 | `0x22bd0` | `0x22be0` | `0x87e31` = 556593 | `0x30000000` |

Each segment header is four little-endian words: length, load address,
`0xffffffff`, `0xffffffff`. Four trailing bytes remain after segment 2.
For the addresses below, subtract the segment load address and add its file
data offset to obtain the offset in the original file. Function addresses are
even; stored Thumb function pointers have their low bit set.

```sh
python3 scripts/analyze-stock-firmware.py /path/to/Firmware_Key_Light.bin
uv run --with unicorn==2.1.4 python scripts/analyze-stock-firmware.py \
  /path/to/Firmware_Key_Light.bin --emulate
python3 scripts/analyze-stock-firmware.py /path/to/Firmware_Key_Light.bin \
  --dump-segments /tmp/key-right-stock-segments
rizin -a arm -b 16 -m 0x30000000 -q -e scr.color=0 \
  -c 's 0x30055fe0; pd 36' /tmp/key-right-stock-segments/image-30000000.bin
```

The dump destination must be a new directory. The optional emulator maps the
two images, isolated RAM, and a return sentinel. It executes bounded individual
functions, with I2C writes, settings getters, logging, memory clearing, clock and
timer services intercepted. It executes the original light wrapper, transition
setup and a settled transition tick, arithmetic and channel driver. The clock
advances from 1000 to 2000 ticks with the fade setting supplied as zero. It does
not execute the network stack, real peripherals, boot process, or intermediate
transition frames. It verifies all integer temperatures `143..344` at brightness
`3` against the formula below, plus initialization and all-channel-zero writes.

## Address and initialization trace

| Address | Role and evidence |
| --- | --- |
| `0x30013f1a` | Light task setup; `0x30013f24` loads SDA=`0x24`, SCL=`0x25`; calls detection, creates the selected I2C object, calls selected driver initialization |
| `0x300412de` | `PCALedCtrl_Detect`, identified by diagnostic string; walks two entries in the driver table |
| `0x30058914` | Driver table entry 0: address `0x62`, frequency `100000`, diagnostic name `9633` |
| `0x30058934` | Entry 1: address `0x15`, frequency `100000`, diagnostic name `9635`; init pointer `0x30055fe1`, zero pointer `0x30055f19`, PWM pointer `0x30055f3f` |
| `0x30058954` | Selected driver index, initially `0xffffffff`; emulator sets it to `1` |
| `0x30041d48` | I2C object creation, stores address at object offset `0x29c`, pins at `0x2a0/0x2a4`, frequency at `0x2a8` |
| `0x30041dd8` | I2C write wrapper, calls HAL write `0x3004af1a` |
| `0x3004afca` | HAL transfers address unchanged into target-address configuration; no eight-bit-address shift |
| `0x3004bd6c` | HAL masks the address to ten bits and writes the hardware target-address field |
| `0x300413a6` | Dispatch selected initialization |
| `0x300413bc` | Dispatch selected all-channel-zero routine |
| `0x300413d2` | Dispatch selected channel PWM routine |

The PCA9635 initializer at `0x30055fe0` emits this exact sequence to its selected
address. Each row is one I2C transaction:

```text
03          # register pointer only, not a software reset
delay 10 ms
00 00       # MODE1: oscillator running, subaddresses/all-call disabled
delay 10 ms
01 14       # MODE2: INVRT=1, OUTDRV=1, OUTNE=00, OCH=0
delay 10 ms
14 aa       # LEDOUT0: individual PWM on channels 0..3
15 aa       # LEDOUT1: individual PWM on channels 4..7
16 aa       # LEDOUT2: individual PWM on channels 8..11
17 aa       # LEDOUT3: individual PWM on channels 12..15
```

The first `03` is a single byte sent to `0x15`. It is not the PCA software reset
sequence, which uses a separate address and two-byte magic. Initialization does
not clear PWM registers. Detection itself performs writes: after initialization
it writes `00 01`, waits, and reads MODE1 to test for `1`. Do not port detection
as a harmless address scan.

The light task at `0x30013c84` installs callback `0x300414fc`, waits approximately
100 ms, and in its ordinary branch dispatches all-channel zero at `0x30013cb4`.
The zero routine `0x30055f18` writes `02 00` through `11 00`, then delays 10 ms.
Normal power-off routine `0x30041a8e` instead requests a transition with both
mix values and brightness zero via `0x30040fb6`.

## Exact settled brightness and temperature arithmetic

Temperature routine `0x3004a220` returns two integers `(cool, warm)`:

```text
t < 144:         (100, 0)
144 <= t < 244:  (100, trunc((t - 143) / 1.01))
t == 244:        (100, 100)
244 < t < 344:   (344 - t, 100)
t >= 344:        (0, 100)
```

The denominator `1.01` is an IEEE-754 double at `0x3004a288`.
The values are mixed contributions, not percentages whose sum must be 100.

**Channel order matters:** wrapper `0x30041580` reverses the temperature tuple.
It supplies `(warm, cool, brightness)` as float32 arguments to transition
command `0x30040fb6`. The transition setter at `0x30041026` stores these in order;
the timer callback at `0x30040f8c` retrieves the two current values in that same
order for the light output callback. The reproduction runs this path and checks
the argument order; directly feeding `(cool, warm)` to the output callback would
produce swapped and incorrect register assignments.

Output callback `0x300414fc` accepts three **float32** values: warm contribution,
cool contribution, and brightness. It promotes them to double through
`0x3000dd54`, multiplies each
mix by `0.4095` and brightness, converts to integer, and passes start=0/end=value
to the selected PWM function, first for channel 0 and then channel 4.

The PCA9635 PWM routine at `0x30055f3e` computes signed integer `(end-start)/16`,
truncating toward zero, then multiplies by double `0.9`, truncates again, and
writes the low byte to register `channel+2`. There is **no `255-N` transform**.

For nonnegative settled integer inputs, the combined formula is:

```text
N = trunc(mix * 0.4095 * brightness)
PWM = trunc(trunc(N / 16) * 0.9)
```

| API temperature | Warm/cool output order | N values at brightness 3 | After /16 | Final PWM0/PWM4 |
| --- | --- | --- | --- | --- |
| 303 | 100 / 41 | 122 / 50 | 7 / 3 | **6 / 2** |
| 200 | 56 / 100 | 68 / 122 | 4 / 7 | **3 / 6** |

`0.4095` is at `0x30041bec`; `0.9` is at `0x30055fd0`. The emulator uses the
original segment-1 soft-float conversion/multiply/divide routines, including
`0x1000d550`, `0x1000d5a8`, `0x1000d2c4`, `0x1000d758`, and `0x10024b68`.
Thus these bytes reproduce stock nominal 3%; they do not assert measured light
output, exact optical CCT, transistor rise-time compensation, or equal perceived
brightness between presets. Transient interpolation can produce other bytes.

## Module pins, GPIO, and the unresolved startup circuit

Realtek's [PinNames.h](https://github.com/Ameba-AIoT/ameba-arduino-1/blob/1f81eb79bf6d88eb21dfa70a7917e23dfd992415/Arduino_package/hardware/system/component/common/mbed/targets/hal/rtl8195a/PinNames.h)
defines `PC_4=0x24`, `PC_5=0x25`, `I2C_SDA=PC_4`, and `I2C_SCL=PC_5`.
The [DK9169 module datasheet](https://www.dexatek.com/_files/ugd/6752fc_b86b4686fbbe4ffd94d6026b169b3a97.pdf)
maps those to pads 10 and 9. The repository's newer saved DK9169 datasheet agrees.

Before PCA setup, startup `0x300169fe` calls GPIO initialization `0x30010ee6` with
table `0x300588c8`. Its sole entry contains pin `0x21`, direction `1`, mode `2`,
then a terminator. These are PC_1, output, PullDown. The driver translates this
to HAL `DOUT_PUSH_PULL=3`, consistent with Realtek's
[hal_gpio.h](https://github.com/Ameba-AIoT/ameba-arduino-1/blob/1f81eb79bf6d88eb21dfa70a7917e23dfd992415/Arduino_package/hardware/system/component/soc/realtek/8195a/fwlib/hal_gpio.h).
This is **module pad 14**. The traced initializer does not explicitly write an
output value. ROM GPIO initialization and the retained output latch have not
been emulated. PC_1 may control OE or another board function; the binary does
not prove that connection.

After PCA setup, startup `0x30016b80` initializes PC_3 (`0x23`, module pad 13) as
input, no pull, then reads it in a reset/test flow. Another GPIO path handles
interrupts for buttons. No separate output-enable write was identified in the
light callback, PCA driver, or traced application startup. This is bounded
negative evidence, not proof that no gate or power sequencing exists. The
`0xb3` passed at `0x30013f5a` initializes a timer object; it is not evidence of
a module GPIO or PCA output-enable connection.

### Why OE-high alone does not establish safe startup

The [NXP PCA9635 datasheet](https://www.nxp.com/docs/en/data-sheet/PCA9635.pdf),
sections 7.3.2, 7.4 and 7.7, distinguishes programmed operation from reset:

- Stock `MODE2=0x14` gives a low output when OE is high. With OE low, its inverted
  push-pull setting is the documented configuration for external N-type drivers.
  Zero PWM is consistent with stock off behavior; verify the actual load circuit.
- At power-on reset, `MODE2=0x05` and `LEDOUT=00`. Push-pull outputs are high with
  OE low, and OUTNE=`01` also makes them high with OE high. An external active-high
  gate could therefore turn on before software changes MODE2.
- SLEEP is not an independent blanking control. The datasheet says outputs
  cannot be changed normally while the oscillator is stopped.
- A controller-only reset need not reset PCA9635; retained PWM and MODE2 values
  must be handled. True PCA power-on reset requires VDD to fall below its stated
  reset threshold, 0.2 V.

The stock initializer's order is not a safe-start design proof. It enables all
PWM outputs before the later zero routine, which can expose retained values.
Do not claim a flash-free replacement until the physical enable/power path and
both cold power-up and controller-only reset have been measured.

## Minimum physical probes that settle the remaining branches

With the original controller retained, power removed, and capacitors discharged:

1. Confirm the chip pin-1 orientation. Check candidate module pad 10 to U3 SDA
   pin 27, and module pad 9 to U3 SCL pin 26. Record resistance, including any
   series resistors, rather than requiring a continuity beep.
2. Trace **U3 OE pin 23** to module pad 14 (PC_1), GND, supply, and any intervening
   resistor/transistor. Trace pad 14 independently if it is not OE. Determine
   whether any other signal gates LED power or PCA power.
3. Trace U3 LED0 pin 6 and LED4 pin 10 to the actual gate networks and LED
   connectors. Confirm which strings respond to each. The photographed QW/QWM
   labels and four MOSFET packages alone do not establish these nets.
4. Measure PCA VDD pin 28 and the idle SDA/SCL high voltage with stock power.
   ESP32 pins must see 3.3 V-compatible levels. Identify every bus pull-up and
   prevent an unpowered controller from being back-powered through them.
5. Capture stock boot, off, 303/3 and 200/3 on SDA/SCL plus OE, both gate outputs,
   and any power gate. A passive analyzer can confirm `0x15` and the byte pairs.
   Record cold power-up and controller-only reset separately.

If OE is tied low, adding ESP control requires isolating that connection. If
OE reaches PC_1, confirm actual logic and reset behavior before reusing the pad.
If neither controls a gate that is safe during PCA reset, an independent,
default-off load or gate blanking circuit is needed. Its design depends on the
measured transistor topology; forcing a high PCA output low directly can create
contention and is not a valid generic fix.

## Other research and observation limits

- User photos `photos/keylight-3623.png` and `photos/keylight-3624.png` visibly show
  DK-9169 V1.0, PCA9635PW, four nearby transistor packages, and power resistors.
  Their visible routing does not establish the OE or channel nets above.
- [EEVblog original Key Light teardown discussion](https://www.eevblog.com/forum/blog/eevblog-1453-elgato-key-light-teardown/)
  discusses the four MOSFETs and series gate resistors but supplies no complete
  verified channel/module pad map. Claims about switching softness are discussion,
  not measurements of this unit.
- [FCC internal photos, original model](https://fccid.io/2AAFM-LGHT001/Internal-Photos/Int-Photos-4105915.pdf)
  were located, but download endpoints rejected retrieval. They were not used to
  assert any trace or pad connection.
- A read-only GET to the documented left-light hostname failed DNS resolution
  on 2026-09-23. No new device state was captured and no control mutation was sent.
  The recorded earlier `303`/brightness `3` logical state remains separate
  evidence. The stock LAN API cannot establish PCA waveforms or wiring.

## Selected assembly path

The production design in [hardware.md](../hardware.md) uses direct ESP LEDC PWM
through TXU0102, after isolating PCA pins 6/10. Original-bus capture, address probing
and OE tracing described above are research options, not required assembly steps.
Use the printed guide's actual pin-1, voltage, disabled-pad and PWM checks instead.
