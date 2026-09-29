# C3 firmware and live checks

This procedure is for the installed **ESP32-C3_MINI_V1** and retained stock
PCA9635. The first board's bench probing is complete: corrected wiring,
powered PCA readback, steady OE, unloaded connector Off/On/Off, Home controls,
and one cold power-cycle restoration passed. It retains both Home fabrics.
Do not repeat those measurements without new fault evidence.

Firmware 0.1.1 hardens target reporting, startup restoration and local recovery;
it needs its own live verification. The second board proceeds through chip and
capacity preflight, flashing, commissioning and live operation without another
routine meter-probing sequence. Record each board's actual image and results
in [the validation record](validation-record.md). Passing the first board does
not establish the second board's behavior.

## 1. Power and preparation

With lamp wiring attached, use lamp/bench power and USB with **VBUS/5 V blocked,
data and ground intact**. A data blocker or charging-only cable is unsuitable.
Ordinary powered USB requires disconnecting **all five lamp wires** from the
C3 first; turning off the lamp supply alone does not isolate the rail and signals.
Nominal 13 V connects only to the stock lamp input, never a C3 pad.

Keep power off and USB unplugged for rework or panel reconnection. Michael
cannot reconnect the panels until reassembly. Accept his completed continuity
and short checks. The corrected U4 map is in [hardware.md](hardware.md): top-row
OE is fifth from left, SCL rightmost, SDA second from right. ESP assignments
remain GPIO4/SDA, GPIO5/SCL and GPIO6/OE.

Run the software gates before flashing and record the commit and outcomes:

```sh
sh scripts/check.sh
sh scripts/check-firmware.sh
```

These need no board or credentials. They cannot prove physical light or radio
behavior. No simulated-image reflash is required for live checks.

## 2. Identify, preflight and flash

Connect the VBUS-blocked USB cable under the power arrangement above, then list
ports. Replace `PORT` in subsequent commands with the C3 serial path and close
other serial monitors:

```sh
python3 scripts/device.py --list
sh scripts/flash.sh --info PORT
sh scripts/flash.sh --check
```

The identity query does not write flash, but entering the ROM loader may reset
the MCU. Require **ESP32-C3** and **at least 4 MiB detected flash** on each board.
The layout uses NVS at `0x9000..0x19000` and one factory application at
`0x20000..0x400000`. The offline image check validates the target, partition fit
and minimum 16 KiB linked stack. Do not force an unknown identity or capacity.

Flash the real image after preflight passes:

```sh
sh scripts/flash.sh PORT
```

Normal flashing preserves NVS. Keep both existing first-board Home fabrics and
do not copy populated NVS to the second board. Omit `--bench`, which selects
simulated output. Record image version/commit and the flash outcome.

If no serial port appears, `python3 scripts/usb-watch.py` records USB detection
without resetting or writing the board. If automatic ROM entry fails, hold
BOOT, press and release RESET, then release BOOT and re-list ports. Keep the
same power isolation. See [USB watcher details](development.md#usb-detection-watcher-macos).

## 3. Verify restoration before changing output

```sh
python3 scripts/device.py --port PORT status
python3 scripts/device.py --port PORT verify
python3 scripts/device.py --port PORT registers
```

Expect hardware mode, the flashed firmware version, no output/storage fault,
and agreement between the saved target and the settled acknowledged frame.
Read status before an Off/On command so restoration remains observable. Valid
saved power, level and temperature restore by default. Missing intent starts
Off at level 57 and 303 mired. Explicit startup Off remains supported; old
startup On/Toggle policies normalize to Restore. This does not force every
restart Off.

The PCA address is `0x15`. Settled stock configuration is `MODE1 & 0x1f = 0`,
`MODE2 = 0x14`, GRPPWM `0xff`, GRPFREQ zero and LEDOUT0–3 `0xaa`. Off has all
16 PWM bytes zero. At level 57, warm/cool PWM is `6/2` at 303 mired and `3/6`
at 200 mired. These are register expectations, not light measurements.

Healthy Matter attributes report the final durable target while a fade runs;
USB status retains the intermediate acknowledged frame separately. Known I/O
faults remain errors. WithOnOff at minimum level 1 turns Off. An ordinary level
command preserves an Off target even during an Off fade.

Record errors, resets or unexpected state. A console timeout can follow a
command that executed, so read status before repeating it. A new PCA or power
fault is grounds for targeted diagnosis, not a reason to restart the completed
probing checklist. If measurement is needed, use the confirmed harness colours:
black GND, red power, yellow SDA, green SCL and blue OE. Resistance/continuity
checks require power removed.

## 4. Apple Home

Keep the first board paired. Only an uncommissioned board needs its own pairing
credential. Each uncommissioned boot opens a 15-minute window; an explicit USB
command reopens it and returns the stable credential:

```sh
python3 scripts/device.py --port PORT commissioning code
```

Alternatively, render its QR locally on macOS and scan it from Home:

```sh
python3 scripts/device.py --port PORT commissioning qr | swift scripts/pairing-qr.swift /tmp/key-right-pairing.png
open /tmp/key-right-pairing.png
```

Use a fresh private output path if the PNG already exists. Keep codes and QR
images out of captures, screenshots and Git. Both USB commissioning commands
reject an already commissioned board. Discovery advertises `Key Right XXXX`
using the last two MAC bytes; Home controls its displayed picker label.

Each lamp should have one tile with power, brightness and white temperature.
Use Home for these checks while reading USB status/verification as needed.
Home 100% means stock nominal 10%, not the lamp's original full output.
At 100%, the warm and cool slider ends should select the corresponding bank;
Home may stop slightly inside the advertised 143–344 mired limits. Check a
middle and low brightness, then Off. Do not repeat the first board's unloaded
connector measurements. Group the two separately commissioned lamps in Home
when both are available; grouping does not guarantee simultaneous transitions.

## 5. Recovery checks

On a commissioned board, these local commands exercise different recovery paths
without unpairing or changing network settings:

```sh
python3 scripts/device.py --port PORT test wifi
python3 scripts/device.py --port PORT test network
python3 scripts/device.py --port PORT test watchdog
```

Run one at a time and verify recovery before the next. `test wifi` requests an
actual station disconnect followed by normal reconnection. `test network`
recreates the complete transport. Observe Wi-Fi/local-IPv6 readiness and restored
Home control with the same intended power, brightness, temperature and fabrics.
An absent Internet connection or an idle Home controller is not a failure.

`test watchdog` requires settled, verified output. It stalls the firmware task
until the 15-second watchdog resets the MCU; the PCA retains its previous frame
during the stall. Confirm the watchdog reset reason and saved-state restoration.
This is not an Off command or proof of dark startup. Software tests cover boot
storage retry and transmit-stall policy; these console checks do not inject every
possible hardware/network fault or establish long-term reliability.

## 6. Reassemble and observe the actual light

Issue and verify Off before assembly, remove lamp/bench power and unplug USB,
then reconnect the panels during reassembly. Completed probing supports this
step. Record connected-output results separately from register and unloaded
voltage results.

Observe power, low/middle/high brightness, warm/cool response, transitions and
Off through Home. Check cold power-up and reset for unexpected flashes, then
verify saved-state restoration and operation without USB. OE release and zero
PWM minimize output once the ESP starts; they cannot guarantee darkness before
it boots. Loaded startup has not yet been observed.

With the housing closed, verify Home control and local recovery using the
installed antenna arrangement. For two grouped lamps, confirm the other remains
controllable while one is offline. Record failures and skipped checks in the
[validation record](validation-record.md); do not infer optical brightness,
Kelvin, current, flicker or thermal measurements from the PCA registers.
