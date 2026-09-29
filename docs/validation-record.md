# Physical validation record

This record covers Michael's original full-size Elgato Key Light and its wired
**ESP32-C3_MINI_V1** replacement controller. Keep pairing codes and Wi-Fi
credentials out of this file. Distinguish reported measurements, firmware
evidence, controller acknowledgements, and observed light output.

**Current wiring correction:** after the bus voltage checks, Michael identified
the error at the Key Light's U4 pads and confirmed the ESP end is correct. On
U4's top row, OE is pad 5 counting from 1 at the left, SCL is rightmost, and SDA
is second from right. ESP GPIO4/SDA, GPIO5/SCL, and GPIO6/OE remain unchanged.
Michael reports that the first board's rework is complete, the specific signal
paths and absence of shorts are confirmed, and it is connected and powered.
The second board's rework completion is not yet reported. After Michael adjusted
the cable, the first board returned on `/dev/cu.usbmodem101` and passed powered
PCA register checks. Subsequent OE and connector probing is recorded below.
Michael later replaced the cable with a stable one and confirmed that its
VBUS/5 V is also blocked.
The earlier results below precede rework unless explicitly marked otherwise.

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
connected.** The initially reported signal map was later found incorrect;
[hardware.md](hardware.md) tracks its replacement. Reported power is J6/DEBUG
3.37 V to `3.3`, with J8/UART ground to `G`. The installed design has no
buck converter or added pull-ups. At that point, powered operation, flashing,
USB, I²C, and optical tests of the completed assembly were still pending.

## Bench session in progress, 2026-09-28, LEDs disconnected

Follow [bench bring-up](bench-bring-up.md) and record the software gate results
before flashing the C3. With lamp wires attached, USB must block
VBUS/5 V while retaining data and ground. Ordinary powered USB requires
disconnecting all five lamp wires first, even when lamp power is off.

Record actual values and outcomes in the following table during the bench
session. `Pending` is not a pass. The current limit has not been specified and
must be recorded as the chosen bench setting, not inferred from the adapter's
4 A rating.

| Check or session detail | Result |
| --- | --- |
| Date, operator, stock-board revision | 2026-09-28; Michael operating the bench, agent inspecting macOS/serial; stock-board revision not supplied |
| U4 signal-wiring correction | Michael reports OE = top-row pad 5 from left (1-based), SCL = rightmost, SDA = second from right; ESP ends correct. First board rewired, signal-pin continuity and shorts checked by Michael. Powered PCA readback now passes on this board. Second board rework completion not yet reported |
| USB after first-board rework | Initially no device in serial nodes, `ioreg`, or `system_profiler SPUSBHostDataType`, despite BOOT/RESET and reported red power LED. USB watcher then captured connection activity; Michael identified a position-sensitive cable and adjusted it. `/dev/cu.usbmodem101` now answers as the original `KR-88:56:a6:39:ec:f4`. Initial status at 346,199 ms uptime reports `ChipPowerOn`, acknowledged Off, and no output/storage fault. No reflash was needed. Michael reports power is good; no new numerical rail reading or PSU CV/CC indication supplied |
| C3 chip identity and detected flash ID/capacity | ESP32-C3 revision v0.4, 40 MHz crystal, 4 MiB flash detected by espflash; raw flash ID was not printed |
| C3 firmware commit, build target, image/partition fit | Current normal real `hardware-light` image is `4fee6bc`, `riscv32imc-unknown-none-elf`; 1,915,920 / 4,063,232 app-partition bytes, linked stack 57,912 bytes. Both software gates and flash preflights passed. Initially flashed `fb241df`, then arena fix `ec1cff9`; temporary radio-diagnostic images were replaced by this normal image |
| Read-only serial inspection and flash-helper preflight | Passed `sh scripts/flash.sh --info /dev/cu.usbmodem1101`; no flash write. macOS identified Espressif USB VID `0x303a`, PID `0x1001`, 12 Mb/s. Holding BOOT, tapping RESET, then releasing BOOT exposed USB and produced the accessory prompt |
| Bench-PSU connection/polarity, voltage setting, current limit | User reports 13 V supply enabled; current limit/current draw and CV/CC indication not supplied |
| Power/USB isolation arrangement used | User opened a USB-C cable and disconnected its larger red conductor. With USB alone, no ESP power indication/enumeration was reported. USB data works with bench power. VBUS isolation has not been independently measured |
| First power-up, current draw, 3.3 V rail under load | User reports ESP LEDs lit, red plus flashing blue, with 13 V bench power; measured 3.345 V between ESP `3.3` and `G` during the powered USB session. Current draw and radio-load measurements pending; this is not a rail-stability pass |
| Original firmware backup | Full 4,194,304-byte read completed and digest verified by espflash before writing. SHA-256 `9a4b8a001b605d24bac52dfb157656d1f16ef8a06726842cd546046466378daa`; private local copy under `~/Library/Application Support/key-right/backups/2026-09-28-c3-first-flash/`, outside Git |
| Flashing, native USB console, boot/reset reasons | Initial flash succeeded; first console request timed out. After manual RESET, captured `Out of bump memory` panic and `TG0WDT_SYS_RST` loop. Enlarged Matter's static arena from 20,000 bytes to 32 KiB, reflashed with NVS preserved; USB `status` then succeeded at 9.6, 20.5, 63.8, and 107.974 seconds uptime, reset `CoreUsbUart`. No storage failures reported |
| GPIO4/5 assignment and 100 kHz bus operation | Configured in real firmware; before rework, PCA bus reported `AcknowledgeCheckFailed(Unknown)`. Michael confirms corrected signal-pin continuity. Post-rework register writes/readback and `verify` pass; actual bus timing remains unmeasured |
| Powered bus DC levels at the ESP | Before rework, Michael reported GPIO4 3.4 V and GPIO5 3.34 V relative to ESP G. Neither appeared held low. The HAL enables internal pull-ups, so these readings did not establish end-to-end continuity or I²C timing. Michael subsequently confirmed corrected signal-pin continuity during rework |
| GPIO6 open-drain release HIGH before output enable; LOW enables configured outputs | Steady levels passed: Michael measured blue/OE to black/GND at 3.34 V after verified Off and 0.01 V after acknowledged On with register verification. Returned to verified Off afterward. Startup/reset transitions remain unmeasured |
| PCA individual address `0x15`, setup writes and critical-register readback | Passed after corrected U4 wiring: Off and ten On frames read back and verified, including full MODE/PWM/group/LEDOUT registers. Before rework these commands failed with an I²C acknowledgement error. See the post-rework results below |
| BLE/Wi-Fi activity, loaded rail, brownout/reset behavior | BLE commissioning reached certificate setup. Wi-Fi scans see the intended AP, but association failed with `NoAccessPointFound`; the discovery candidate awaits a fresh Home retry. Radio-load rail stability and brownout/reset behavior remain pending |

### First-board post-rework register checks

With LEDs disconnected, the agent exercised the real `hardware-light` image via
`/dev/cu.usbmodem101`, verified device identity, and compared all 24 returned PCA
registers against independently calculated stock mixing values. Every frame had
`MODE1=0x80`, `MODE2=0x14`, unused PWM channels zero, `GRPPWM=0xff`, `GRPFREQ=0`,
and all four LEDOUT registers `0xaa`. Every firmware `verify` command passed.

| On level | Temperature (mired) | Readback warm/cool PWM |
| ---: | ---: | ---: |
| 57 | 303 | 6 / 2 |
| 57 | 200 | 3 / 6 |
| 1 | 143 | 0 / 1 |
| 1 | 344 | 1 / 0 |
| 254 | 143 | 0 / 22 |
| 254 | 344 | 22 / 0 |
| 254 | 244 | 22 / 22 |
| 254 | 303 | 22 / 9 |
| 128 | 244 | 12 / 12 |
| 128 | 200 | 6 / 12 |

Off readback had all 16 PWM registers zero. Level 0/255 and temperature 142/345
were rejected with `ConstraintError` without changing the Off register frame.
Final cleanup reconfirmed acknowledged Off at level 57, 303 mired, with uptime
356,756 ms and no reset during the sweep. `output_failures`, `storage_failures`,
and `recoveries` remained zero; `usb_dropped` stayed at its initial value of 25.
Wi-Fi was not connected. Local response capture is
`/tmp/key-right-post-rewire-readback.log`; no pairing code was requested.

This proves powered register communication and commanded frames on the first
board. Michael subsequently measured blue/OE relative to black/GND at 3.34 V
Off and 0.01 V On, confirming expected steady HIGH/LOW levels. Startup
transitions, unloaded LED-connector behavior, light output, and radio operation
remain separate checks.

Michael confirmed the harness colours: black GND, red power, yellow SDA, green
SCL, blue OE. A USB `status` request initially timed out before the On OE
measurement; no On command was sent in that failed attempt. At Michael's next
request, USB responded and On was acknowledged/verified at uptime 589,497 ms.
After his 0.01 V reading, Off and register verification passed at uptime
648,953 ms, with acknowledged Off, level 57, 303 mired, no fault, and zero output
or storage failures.

### Unloaded LED-connector probing

Michael identified all four connectors as `F-1`, `F-2`, `W-1`, and `W-2`.
With the firmware acknowledged Off, he measured **0.002–0.008 V DC across the
two pins of every connector**. Individual connector values within that range
were not supplied. These near-zero readings apply with the LED panels absent.

For the On comparison, the agent commanded On at level 57, 303 mired and
verified warm/cool PWM 6/2, acknowledged On, and no faults at uptime 775,527 ms.
Michael then measured **approximately 13.3 V DC across each of F-1, F-2, W-1,
and W-2**. This establishes an unloaded voltage change between commanded Off
and On on all four connectors; it does not establish LED current, PWM duty,
brightness, or warm/cool mixing under load. Connector letters have not yet been
confirmed as physical warm/cool channel assignments.

The agent returned the board to Off, with successful register verification,
acknowledged Off, no faults, and zero output/storage failures at uptime
858,296 ms. Michael confirmed that all four connector voltages returned near
zero, as before. The unloaded Off/On/Off connector check passes. The lamp
remains at acknowledged Off, level 57, 303 mired; connected-LED behavior and
startup transients remain unverified.

After completing the unloaded checks, `commissioning code` succeeded and opened
the pairing window for the first board. The private code was returned to Michael
without a serial capture file and is omitted here. Home pairing subsequently
failed; diagnosis and the pending retry are recorded below.

### Apple Home pairing diagnosis

Michael reported an initial attempt from his 5 GHz network, then corrected his
iPhone to `SyNet-2G`. The first retained log reached BLE, attestation, AddNOC,
and Wi-Fi scanning, with one connection attempt but no connected/IP-ready state.
Some logs were dropped, so its exact radio error is unknown. A software reboot
preserved NVS and opened a fresh BLE window without erasing data.

The captured retry explicitly supplied `SyNet-2G`. After successful certificate
setup and a three-result scan, the radio returned `NoAccessPointFound`, zero
BSSID, and RSSI sentinel -128. This is not a measured weak signal. The failure
preceded Wi-Fi association and DHCP; accepting the uncertified-accessory warning
did not prevent progress through attestation. Capture: `local/pairing-retry.log`.

Read-only UniFi checks found the intended AP online, SSID visible and enabled,
2.4 GHz channel 1 / 20 MHz, WPA2-Personal/CCMP, and 18 associated clients. PMF,
MAC filtering, client isolation, and fast roaming were off; the multicast
enhancement retained after Stillair's testing was still on. The ESP was absent
from current/historical client lists. No AP or network settings were changed.
Stillair's documented `NoAccessPointFound` retry used the wrong SSID; its later
multicast workaround followed successful association and is not yet implicated
in this C3's failure.

An optional full-application radio diagnostic build compared scans without
joining. The first default 10–20 ms scan returned four APs and omitted the home
SSID. A later default scan returned eight including `SyNet-2G` at -58 dBm;
100–300 ms active and 300 ms passive scans each returned 20, also including it
at -58 dBm on channel 1. All eight and all 20 per-scan records were captured;
startup USB-log drops occurred outside those complete result lists. The nearby
network names remain only in `local/radio-diagnostics.log`.

The candidate changes ordinary discovery to 100–300 ms and adds a targeted scan
before association, choosing a starting-channel hint and all-channel search.
The 30-second total deadline and fallback when no AP is found remain bounded.
This is a discovery reliability change, not a proven pairing fix. A fresh Home
retry must establish association, IP readiness, commissioning completion, and
tile behavior. The disconnected-LED Off/PCA checks still pass after diagnostic
flashes; the light intent remains Off, level 57, 303 mired.

The normal `hardware-light` candidate at `4fee6bc` was then flashed with all
preflights passed and NVS preserved. At 13,339 ms uptime it reported the expected
identity, `CoreUsbUart` reset, acknowledged Off at level 57 / 303 mired, no output
or storage failures, and no recovery events. Explicit `off` and `verify` both
passed. The pairing window reopened, with a fresh capture at
`local/pairing-discovery-retry.log`; no joining attempt had occurred at that point.

Michael's next Home attempt failed. Replacing the cable disconnected the capture
process; reopening the same USB port recovered queued logs in
`local/pairing-after-cable-change.log`. Certificate setup completed and the
longer scan returned 35 APs, but the trailing scan-response completion log was
absent. The commissioning failsafe expired. At 667,014 ms uptime the device
had zero Wi-Fi connection attempts, no resets since flashing, and no light or
storage faults. This attempt stopped before association, unlike the earlier
`NoAccessPointFound` failure.

Source review found that the pinned Matter ScanNetworks handler writes every
reported AP into one bounded response without chunking. A callback encoding
failure aborts the handler before the trailing completion log. Production scan
results are now capped at ten in SDK RSSI order, with explicit callback-error
logging. The target SSID appeared within the first six results in the captured
diagnostic scans. The specific device-side error was not retained during the
cable change; the encoder regression test establishes the overflow mechanism
separately. Fresh Home commissioning remains the acceptance check.

## LED-connected acceptance pending

Michael reports the LED panels cannot be connected until the lamp is put back
together. Keep them disconnected for bench work; the four two-pin LED connectors
are available for probing. Record unloaded connector measurements separately
from physical light results. Reconnect during reassembly with power removed,
only after the preceding bench work supports proceeding. Record the firmware
commit and actual observations for each check. Tests with disconnected LEDs
cannot fill these rows.

| Check | Observation |
| --- | --- |
| Firmware commit and LED reconnection date | Pending |
| Off after application initialization | Pending |
| Home low/high brightness, stock nominal 1%/10% limits | Pending |
| Temperature minimum/maximum, 143/344 mired | Pending |
| Initial level 57 and 303 mired, warm/cool raw PCA `6/2` | Pending |
| Level 57 and 200 mired, warm/cool raw PCA `3/6` | Pending |
| Intermediate brightness/temperature response and low-level quantization | Pending |
| LED0 warm / LED4 cool physical behavior | Pending |
| OE HIGH with `MODE2=0x14` and the actual external driver stage | Pending |
| Cold power-up, MCU reset, and any startup flash | Pending |
| Power-cycle persistence and intended/applied/output state agreement | Pending |
| Installed antenna arrangement and closed-housing BLE/Apple Home pairing | Pending |
| One Home light tile with power, brightness, and temperature per lamp | Pending |
| Two separately paired lamps grouped in Home, shared brightness/temperature control | Pending; no simultaneous-output guarantee |
| One grouped lamp offline while the other remains controllable | Pending |
| Short Wi-Fi/AP interruption and automatic recovery preserving intent | Pending |
| USB removed, ordinary lamp-powered operation | Pending |

## Interpretation

[Stock-firmware emulation](references/firmware-analysis.md) establishes address,
channel, configuration, and reference command evidence. It does not establish
measured brightness, color temperature, flicker, LED current, temperature,
startup behavior, or radio/network reliability on this assembly. There are no
connected voltage, current, or optical sensors; fixed values and register
readback must not be reported as live physical telemetry.

Keep observations direct, including unexpected light, bus failures, reset loops,
and radio failures. Record skipped checks and unresolved contradictions. Do not
claim safe startup from a successful software build, an OE level, or a PCA
acknowledgement. Do not claim long-term reliability from a single bench session.
