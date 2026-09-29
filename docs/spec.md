# ESP32 controller replacement specification

## Outcome and current state

Replace only the original full-size Elgato Key Light's Realtek controller with
the installed **ESP32-C3_MINI_V1** board. Retain the PCA9635, stock LED power and
current-limiting circuitry, LED panels, housing, and 13 V / 4 A adapter. Provide
local Apple Home control through Matter over 2.4 GHz Wi-Fi, BLE commissioning,
and automatic recovery without cloud services or Homebridge. Each physical lamp
is one normal light in Home, with power, brightness, and white-temperature
controls. Two lamps remain separate Matter nodes and can be grouped in Home.

On **2026-09-28**, Michael completed the wiring and powered the assembly from a
13 V bench supply with the LEDs disconnected. He measured 3.345 V at the ESP.
USB confirmed an ESP32-C3 with 4 MiB flash, and the real firmware was flashed
after backing up the original image. The first board subsequently passed PCA
readback, steady OE levels, unloaded connector Off/On/Off, Apple Home power,
brightness and temperature controls, and one cold power-cycle restoration.
Michael considers bench probing complete. Firmware 0.1.3 passed real Wi-Fi
reconnection, repeated full transport recreation, watchdog recovery of saved
On, and reboot recovery of saved Off. Home card reporting after those resets
remains under investigation. Loaded output/startup and the second board remain
unverified. The [physical validation record](validation-record.md) owns
per-image results, including the rejected intermediate builds.

The firmware target is `esp32c3`, using `riscv32imc-unknown-none-elf`. Software
validation and physical acceptance are separate; passing build gates does not
establish correct wiring, USB power isolation, or physical lamp behavior.

## Design and evidence

Keep the existing Rust architecture and compatible Stillair-derived Matter
dependency set. The September 28 handoff supplies the actual board identity,
initial five-wire map, and user measurements. Michael corrected the field guide's
U4 signal-pad positions during bench testing; the ESP end remains correct.
[hardware.md](hardware.md) owns the corrected map; the
[validation record](validation-record.md) tracks each board's rework and tests.
The user selected one light per lamp, Home grouping, and a low-light range
capped at stock nominal 10%.
[Signed stock-firmware analysis](references/firmware-analysis.md) remains the
basis for the PCA configuration and warm/cool mixing. This is a single known
lamp design, not a general channel-discovery or calibration platform.

| Item | Project requirement or evidence | Remaining limit |
| --- | --- | --- |
| ESP board | `ESP32-C3_MINI_V1`, target `esp32c3` | Read chip identity and flash capacity before writing; plan for 4 MiB, no PSRAM |
| Power and signal wiring | Corrected U4 map in hardware.md; first-board continuity/short checks reported complete; ESP assignments unchanged | Do not repeat routine probing; investigate a new fault if one appears |
| PCA address and bus | Seven-bit `0x15`, 100 kHz; first-board writes/readback passed | Bus timing is not measured by register readback |
| Output configuration | Stock `MODE2=0x14`; LED0/warm drives F connectors and LED4/cool drives W connectors in unloaded checks | Connected-panel colour, brightness and startup remain unobserved |
| Brightness range | Stock nominal 1% through 10%, mapped across Home's nonzero brightness range | Stock scale, not measured optical brightness or raw electrical PWM duty |
| Temperature range | Stock 143 through 344 mired, approximately 6993 through 2907 K | Command range, not optical calibration |
| Initial level | Matter level `57`, approximately Home 22% and stock nominal 3% | Preserve raw warm/cool `6/2` at 303 mired and `3/6` at 200 mired |

The former 3300 K and 5000 K choices can be Home scenes; they are no longer
separate firmware endpoints. No pure-bank defaults, alternate polarity wizard,
or mandatory channel scan is required. If actual readback or light behavior
contradicts the stock evidence, record and investigate it before changing the
driver or mixing model.

## Home control contract

Expose one Matter Color Temperature Light endpoint, device type `0x010C`, per
physical lamp. Support On/Off, Level Control, and temperature-only Color Control.
Do not advertise RGB/hue/saturation control or Adaptive Lighting support.

Home's nonzero 1% through 100% brightness range maps to stock nominal 1% through
10%. **Home 100% means stock nominal 10%**, the user-selected ceiling; it does
not unlock the stock lamp's full output. Dimming must change the PCA commands.
Stored brightness spans Matter levels 1 through 254. Commands with On/Off
coupling turn Off at the advertised minimum level 1, including a requested 0
clamped to that minimum; an ordinary level command preserves power intent.
Retain brightness and temperature intent while off. The first-boot state is Off
with level `57` and 303 mired (about 3300 K) ready for the next On command.

Support Matter Groups and Scenes Management on that endpoint, including a
16-entry scene table. Level and temperature transitions run independently;
Stop retains the last acknowledged setting for its axis. Persist the final
destination once, not each interpolated frame. An explicit Off cancels pending
transitions. Scene recall saves the complete power/level/temperature target
together and reports actuation failures. Timed On/Off and OffWithEffect use the
same state owner; effect frames do not replace the saved brightness setting.
Healthy Matter reads/reports expose the durable target immediately, keeping
intermediate acknowledged frames separate in diagnostics. Failed output or
storage does not become a successful target report. A level command without
On/Off coupling cannot relight an Off target during its fade.

Support the full stock temperature command range, 143 through 344 mired. At the
initial level `57`, 303 mired must reproduce warm/cool raw PCA values `6/2`, and
200 mired (5000 K) must reproduce `3/6`. These are regression points from the
former fixed presets, not additional endpoints. Eight-bit PCA quantization is
visible at low output: adjacent slider settings can produce the same register
values, and displayed percentages or Kelvin values are not measurements.

Commission each lamp separately with its own stable identity and pairing data.
Group the two accessories in Apple Home to send power, brightness, and
temperature changes together. Grouping belongs to Home; firmware has no
cross-lamp state or coupling. Delivery and transitions need not be simultaneous
or frame-perfect, and one lamp's outage must not prevent the other operating.

## Installed hardware contract

The board is the small blue USB-C board marked `ESP32-C3_MINI_V1`, with BOOT and
RESET buttons and a bare ESP32-C3 package. It is not an official
`ESP32-C3-MINI-1U` module. Its printed pad numbers are GPIO numbers, not board
`D` aliases. Do not assume a firmware antenna-selection pin on this board.

The physical connection map is owned by [hardware.md](hardware.md), including
Michael's corrected U4 pad positions. He confirmed the ESP end is correct:
SDA remains GPIO4, SCL GPIO5, and OE GPIO6. No firmware pin change is required.
Accept the first board's reported corrected continuity and short checks.
The second board proceeds to flashing and live tests without another routine
probing sequence. Investigate a new wiring fault with power removed if evidence
requires it; do not repeat completed measurements as a prerequisite.

The reported measurements are 13 V input, 3.37 V PCA VDD and idle SDA/SCL/OE,
approximately 9.9 kΩ from each signal to VDD, approximately 20 kΩ SDA-to-SCL,
approximately 1 kΩ unpowered VDD-to-GND, and approximately 0–0.1 Ω continuity
from each selected signal pad to its PCA pin. These are historical reports,
superseded as wiring evidence by the reported rework checks and successful
powered PCA readback. Retain the existing pull-ups and use 100 kHz. Radio-load
rail stability was not measured; that limit is not a request for more routine
probing. Provenance is in [hardware.md](hardware.md) and the
[validation record](validation-record.md).

GPIO6 must be open-drain: set its latch HIGH before enabling output mode;
release HIGH through the existing pull-up to disable configured outputs, and
drive LOW to enable. Explicitly assign GPIO4/5/6 to I²C/GPIO rather than external
pad JTAG. Preserve native USB Serial/JTAG on GPIO18/19 for programming and the
bidirectional console. Leave unverified board LED and antenna pins unused.
See the [C3 datasheet](https://www.espressif.com/sites/default/files/documentation/esp32-c3_datasheet_en.pdf)
for chip pin functions; it does not establish this board's antenna routing.

The original rocker has already been bypassed into its working ON state. No
buck converter, connection to the C3 `5V` pad, extra pull-ups, level shifter,
lifted PCA pins, or independent output cutoff is part of the installed design.

### Bench power and USB service

With lamp wiring attached, power the assembly through the lamp's input and
use USB with **VBUS/5 V blocked while data and ground remain connected**.
A charging-only cable or USB data blocker does not meet this requirement.
Ordinary powered USB requires disconnecting **all five lamp wires** from the
C3 first. Merely unplugging the lamp adapter does not isolate its rail and signal
paths.

For initial bench work, leave the LEDs disconnected, verify input polarity, and
record the bench PSU voltage and chosen current limit before energizing the
stock board. Apply nominal 13 V at the lamp input, never at the C3 `3.3` or
`5V` pad. The first board powered successfully at 13 V and measured 3.345 V at
the ESP; a bench current-limit value was not supplied.

OE HIGH selects the PCA's programmed disabled-output state; it does not itself
prove that the lamp is off. Stock `MODE2=0x14` and PCA power-on defaults differ.
Keep brownout detection enabled and report reset reasons. Do not claim dark
cold start, reset, brownout, or failed-bus behavior until observed with the LEDs
connected. There is no optical, current, or temperature feedback wired to the
ESP, and no independent output cutoff.

## Firmware behavior to preserve

### Controls and persistence

- Expose the single light and ranges above. Power, level, and temperature
  commands share one state owner; repeated commands are idempotent. Report
  the lamp's actual supported ranges so Home can present ordinary controls.
- Use one serialized state owner for Matter and USB commands. Distinguish
  intended state, acknowledged PCA register state, and measured physical output.
  Never report a known failed write as successful actuation.
- Preserve durable power, level, temperature, and Matter membership across
  ordinary restarts and flashes. Startup first releases OE and attempts stock
  zero PWM, then restores validated saved intent. Missing intent defaults Off.
  Retain explicit startup Off and startup level/temperature settings; reject
  new On/Toggle startup policies and migrate old On/Toggle to Restore.
  Version-2 preset migration retains saved power and selected temperature at
  level `57`, preserving an explicit Off policy for the selected preset.
  This minimizes startup output once the ESP runs; it cannot guarantee darkness
  before the ESP boots. Do not replace restoration with unconditional Off.
  Corrupt storage reports a fault rather than a successful restore. Retry boot
  adapter-reported `StdIoError` with 5–60-second capped backoff and watchdog
  feeds. Firmware must not request an erase or factory reset. The pinned
  adapter also maps storage-format/buffer errors to this code and may repair
  storage pages during reads; logical fabric preservation is the contract.
  Malformed application or Matter records detected during decoding remain faults.
- Avoid unnecessary flash writes. Keep pairing codes and Wi-Fi credentials out
  of source control and routine logs. Provide actual pairing data through the
  explicit local USB command, using the existing development Matter identity.

### PCA output and recovery

- Retain the dependency-free `no_std` core and stock PCA9635 driver. Keep hardware,
  network, time, and persistence I/O in the application adapters.
- Use seven-bit `0x15`, stock `MODE2=0x14`, eight-bit individual PWM, and complete
  frames covering all 16 channels. Unused channels remain zero. Do not use group
  dimming or ESP-generated PWM in place of the PCA.
- Preserve the existing disable/configure/zero/wake/verify sequence, 500 μs
  oscillator settling delay, auto-increment framing, STOP-update configuration,
  and MODE1 readback masking. Enable requested output only after successful
  setup/frame verification. Do not use all-call writes or routine reset sweeps.
- Keep I²C serialized with finite timeouts and explicit errors. On output failure,
  invalidate acknowledgement and attempt stock Off/OE release. Preserve intended
  state and the existing bounded recovery attempts that reinitialize, verify,
  and reapply it. Recovery is not evidence of physical Off during the fault.
- Recover Wi-Fi, AP, local-address, and Matter transport interruptions without
  discarding intent or commissioning. Radio operations have 30-second deadlines.
  Three consecutive internal driver errors, 60 seconds of failed RSSI queries
  while associated, or 60 seconds without usable local IPv6 while associated
  recreate the transport. IPv4 alone does not establish Matter readiness.
  Transport retries back off from 5 to 60 seconds and reset after 120 seconds
  of actual local health. Neither Internet reachability nor Home traffic is a
  health requirement.
- Monitor transmit capacity/progress on the existing network interface. A
  60-second linked stall first recreates transport; persistence after recreation
  triggers a full MCU reset with durable intent and fabrics retained. Successful
  transmit progress clears the stall timer, including under busy traffic.
  Keep the 15-second execution watchdog. These detect specified local failures;
  they do not prove recovery from every possible Matter or network hang.

PCA register semantics come from the
[NXP datasheet](https://www.nxp.com/docs/en/data-sheet/PCA9635.pdf);
the selected commands come from the stock firmware evidence above.

### Build and console

Continue with Rust, Cargo, the current portable core, and the pinned compatible
Matter stack with C3 chip-specific adapters and build configuration. Keep the
real application and explicitly simulated bench image distinct. Bench simulation
is not a prerequisite reflash or a substitute for real-I/O testing.

The real C3 image must include normal Matter control and USB status,
register-readback, power, level, temperature, verification, pairing, and recovery.
Keep `scripts/device.py` and its `KR OK`/`KR ERR` protocol. The console must not
block startup waiting for a host. No calibration wizard, new JSON command family,
network console, cloud service, OTA system, or extra installation firmware is
required for this installation.

The bench image uses the same runtime, Matter, console, and network paths with
simulated output. Its status and register responses must identify simulation;
they cannot establish successful physical PCA communication or output.

Before flashing:

1. Verify C3 throughout Cargo features, target configuration, runtime/peripheral
   setup, board identity, image generation, and the flash helper. Use GPIO4/5 and
   open-drain GPIO6; reserve USB GPIO18/19 and leave antenna routing to hardware.
2. Preserve compatible dependency pins where possible and document changes
   required for the C3. Build both actual C3 images and retain the
   existing host/control tests.
3. Detect chip and flash capacity before writing. Reject the wrong chip or an
   image/partition layout that cannot fit. Start from a 4 MiB budget, without
   PSRAM or OTA slots, and validate against the detected device. Normal flashing
   must preserve NVS; full erase must be explicit.
4. Document and exercise native USB flashing, console access, and BOOT/RESET
   recovery under the installed assembly's power-isolation rule.

[Development](development.md) owns build and console commands;
[bench bring-up](bench-bring-up.md) owns the initial powered sequence.

## Acceptance

Run `sh scripts/check.sh` and `sh scripts/check-firmware.sh` after changes. Keep
behavioral coverage of the 1%/10% stock limits, brightness and temperature
mapping, the level-57 regression points, single-light power/toggle/idempotent
transitions, persistence migration, readback mismatch and bus failure, storage
errors, startup policy, and recovery. Exercise standard Matter commands and
reporting for the advertised level and temperature features.
Verify pin modes and target/flash-fit checks for the C3.

The physical acceptance sequence is:

1. **Firmware update:** preserve completed first-board probing. Record chip
   identity/capacity, image/commit, safe USB connection and flash outcome, then
   check status, PCA verification and Home operation. Preserve both existing
   Home fabrics. The second board follows preflight, flash and live tests without
   another routine meter-probing stage. Investigate faults if they appear.
2. **LEDs connected after power is removed:** observe Off, low and high brightness,
   temperature endpoints and intermediate values, the level-57 reference points,
   bank identity, cold power-up, controller reset, and saved-state restoration.
   Record quantization, flashes, incorrect banks, unexpected output, or loss of
   control. Confirm Home 100% respects the stock nominal 10% ceiling.
3. **Completed housing:** retain the first board's pairing and commission the
   second with its actual antenna hardware. Verify one light tile with power,
   brightness, and temperature per lamp. Group the two separately commissioned
   lamps in Home and verify both follow those
   controls. Interrupt one lamp's Wi-Fi briefly, confirm the other still works,
   and verify automatic recovery with intent retained. The development host
   must remain reachable independently of the lamp's Wi-Fi.

Record observations, failures, and pending checks in
[the validation record](validation-record.md). Do not require destructive fault
injection or imply measured Kelvin, lumens, current, flicker, thermal behavior,
or long-term reliability from this acceptance session.
