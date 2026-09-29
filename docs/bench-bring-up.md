# C3 bench bring-up

This sequence is for the wired **ESP32-C3_MINI_V1** and retained stock PCA9635,
with the **LED panels disconnected** until the lamp is reassembled. The first
powered USB identification, original-flash backup, and real firmware flash
completed on 2026-09-28.
Record each actual result and remaining check in
[the validation record](validation-record.md); this guide is the procedure.

**Wiring correction required:** Michael found the field guide's signal map
incorrect during bench testing. Keep bench power off and USB unplugged while
rewiring. The powered steps below apply only after the corrected SDA/SCL/OE map
and complete-path continuity have been recorded in [hardware.md](hardware.md).
The [component-side C3 photo](references/photos/esp32-c3-mini-v1.png) identifies
the connector and buttons visually; it does not prove antenna routing or
electrical wiring.

## 1. Prepare without power

- Keep the LED panels disconnected and the board on an insulated surface.
- Check power and ground: J6/DEBUG regulated supply to `3.3`, J8/UART ground
  to `G`; do not connect `5V`. Establish the corrected SDA/SCL/OE paths from the
  actual printed ESP GPIO labels to PCA pins 27/26/23 with power removed.
  Current firmware uses GPIO4/5/6 respectively; reconcile that configuration
  with the corrected wiring before power-up. Do not reuse the withdrawn row
  positions in the earlier field guide.
- Connect the bench PSU only to the stock lamp input with the confirmed
  polarity. The nominal input is **13 V**. Never put 13 V on a C3 pad.
  Use the bench PSU as the lamp's input source, with the ordinary adapter
  disconnected. Record the chosen current limit; no current-limit setting has
  yet been supplied, and the adapter's 4 A rating is not a bench setting.
- With lamp wires attached, USB must have **VBUS/5 V blocked, data and ground
  intact**. A charging-only cable or data blocker is unsuitable. Ordinary
  powered USB requires disconnecting all five lamp wires from the C3, even if
  the lamp supply is off. Remove USB before restoring those wires.
- Run the software gates before flashing and record the commit and outcomes:

```sh
sh scripts/check.sh
sh scripts/check-firmware.sh
```

The software gates need no connected board. They do not replace the power and
wiring checks below.

## 2. First power and read-only identification

Energize the stock lamp input from the bench PSU with the LEDs still
disconnected. Record input current, the C3 supply at `3.3`/`G`, and whether the
PSU is current-limiting. The reported rail baseline is 3.37 V. Stop to investigate
an unstable rail, current-limit cycling, unexpected heating, or repeated resets;
do not treat an idle voltage as proof of stability under radio load.

Connect the VBUS-blocked USB data/ground path. List ports before any flash write:

```sh
python3 scripts/device.py --list
```

Replace `PORT` in subsequent commands with the returned C3 serial device, such
as `/dev/cu.usbmodem...`. Close other serial monitors. Inspect the connected chip:

```sh
sh scripts/flash.sh --info PORT
```

This invokes the chip-information query without building or writing flash;
entering the ROM loader can reset the MCU. Record chip identity, revision, flash
ID/capacity, and the serial port. Require **ESP32-C3** and **at least 4 MiB flash**
for this layout. Do not continue on an identity mismatch, unknown capacity, or a
smaller device.

If automatic ROM entry fails, hold BOOT, press and release RESET, then release
BOOT. Re-list ports if USB re-enumerates and retry the read-only identity query.
This sequence exposed USB successfully on the first board. Keep the same
lamp-power and VBUS-blocked USB arrangement throughout.

## 3. Preflight and flash

Build and check the image against the partition layout without writing to the
board:

```sh
sh scripts/flash.sh --check
```

Record the reported target, image size, and partition fit alongside the detected
flash capacity from the preceding device query.
The application layout requires 4 MiB, with NVS at `0x9000..0x19000` and one
factory application at `0x20000..0x400000`. A rejected check is not permission
to force a chip or flash-size override.

After the offline check succeeds, flash the real hardware image. The default
command repeats device identity/capacity checks before its write operation:

```sh
sh scripts/flash.sh PORT
```

Normal flashing preserves NVS and commissioning; do not add a full-chip erase.
The separate `bench-light` image simulates the PCA and cannot validate these
wires. The `--bench` helper option explicitly selects that simulation; omit it
for this physical sequence. Record the flashed commit and outcome, then confirm
the application restarts and exposes its native USB console.

## 4. Inspect the real PCA with LEDs disconnected

Begin with status and explicit Off, then read and verify the controller:

```sh
python3 scripts/device.py --port PORT status
python3 scripts/device.py --port PORT off
python3 scripts/device.py --port PORT registers
python3 scripts/device.py --port PORT verify
```

`off` configures/writes the PCA; it is not a read-only query. Expect `KR OK` for
successful commands. Record `KR ERR`, reset reasons, output/storage faults, or
timeouts. A timeout can follow a command that executed, so inspect `status`
before repeating a control command.

Check the following separately and record the method as well as the result:

| Check | Expected command or electrical state |
| --- | --- |
| PCA communication | Seven-bit address `0x15`, finite transactions at 100 kHz |
| Critical register readback after Off | `MODE1 & 0x1f = 0`, `MODE2 = 0x14`, PWM0–15 zero, GRPPWM `0xff`, GRPFREQ zero, LEDOUT0–3 `0xaa` |
| Supply | Record actual C3/PCA rail voltage and PSU current at idle and during radio activity |
| SDA/SCL | Existing pull-ups remain; compare idle levels with the reported 3.37 V baseline |
| OE | GPIO6 open-drain; released HIGH for Off and pulled LOW only after a verified On frame |
| Firmware health | Record reset reason, uptime, brownout/reset loops, output faults, and storage faults |

With the panels still disconnected, exercise real register changes and return
to Off:

```sh
python3 scripts/device.py --port PORT level 57
python3 scripts/device.py --port PORT temperature 303
python3 scripts/device.py --port PORT on
python3 scripts/device.py --port PORT verify
python3 scripts/device.py --port PORT registers
python3 scripts/device.py --port PORT temperature 200
python3 scripts/device.py --port PORT verify
python3 scripts/device.py --port PORT registers
python3 scripts/device.py --port PORT off
```

At level `57`, the expected warm/cool PWM bytes are `6/2` for 303 mired and
`3/6` for 200 mired. Level and temperature commands retain the current power
state; `on` enables the configured output. Confirm the real image reports
hardware mode. Simulation mode and simulated register values cannot pass the
PCA checks, even though they use the same console and Matter control paths.

Do resistance or continuity checks only with power removed. Because the field
guide's signal map was found wrong, check each complete corrected signal path;
the earlier pad-continuity report cannot validate it. A meter reading alone does
not establish I²C timing or the absence of short rail transients. No test in this
disconnected-LED stage can prove emitted light, darkness, temperature, or safe startup.

## 5. Pairing and the next physical checks

Request the stable per-device code explicitly over USB when ready to pair:

```sh
python3 scripts/device.py --port PORT commissioning code
```

Keep the code private and out of logs, screenshots, and committed records. Each
uncommissioned boot opens a 15-minute pairing window; this explicit command can
reopen it. Pair each lamp separately. Apple Home should show one light with
power, brightness, and temperature controls; group the two accessories in Home
to control them together. Grouping has no frame-perfect timing guarantee.

Michael cannot reconnect the panels until reassembly. Use the four two-pin LED
connectors as accessible probe points during disconnected bench work, recording
the connector, reference point, instrument, requested state, and measured value.
Do not interpret an unloaded connector voltage as LED current or brightness;
PWM timing needs an oscilloscope or logic measurement at an appropriate node.

After the disconnected-LED checks support reassembly, request Off, remove power,
and reconnect the panels during assembly. Use the LED-connected rows in
[the validation record](validation-record.md) to observe Off, brightness and
temperature, the Home 100%/stock 10% ceiling, startup/reset behavior, and recovery.
Keep first connected-output tests distinct from the preceding readback results.
At initial level `57`, 303 mired uses raw warm/cool `6/2` and 200 mired uses
`3/6`; these are register expectations, not measured optical values.
