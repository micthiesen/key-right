# Physical validation record

This record covers Michael's original full-size Elgato Key Light and its wired
**ESP32-C3_MINI_V1** replacement controller. Keep pairing codes and Wi-Fi
credentials out of this file. Distinguish reported measurements, firmware
evidence, controller acknowledgements, and observed light output.

**Current Home status:** board A (first controller, `39:ec:f4`) runs firmware
0.1.4; board B (`39:f4:14`) remains on 0.1.3. Both joined Home on 2026-09-28,
each with two persisted Home fabrics. Both fabrics loaded after board A's update.
Preserve all of them.
The first board passed the physical bench checks and controlled recovery tests
below; Michael now reports successful overall operation of that lamp. The second
board passed PCA register checks, saved-Off reboot and Home control checks.
Michael confirmed Off held without a spinner after refreshing Home. The second
board passed the final saved-Off check before assembly. Both assembled lamps
worked well per Michael before board A was connected for its update. He reports
faint orange glow near the centre while Off and powered. He confirms actual panel emission, also visible
at the edges, and rules out the ESP indicator; optical darkness is unresolved.
Probing is complete. Board A's 0.1.4 flash and saved-Off reboot passed below;
visual fades and the resistor trial remain unverified. Board B awaits its update.
Detailed loaded results and remaining limits are recorded per board below.

**Current wiring correction:** after the bus voltage checks, Michael identified
the error at the Key Light's U4 pads and confirmed the ESP end is correct. On
U4's top row, OE is pad 5 counting from 1 at the left, SCL is rightmost, and SDA
is second from right. ESP GPIO4/SDA, GPIO5/SCL, and GPIO6/OE remain unchanged.
Michael reports that the first board's rework is complete, the specific signal
paths and absence of shorts are confirmed, and it is connected and powered.
The second board subsequently passed real PCA writes/readback, as recorded below.
After Michael adjusted the cable, the first board returned on `/dev/cu.usbmodem101` and passed powered
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

## First-board bench session, 2026-09-28, LEDs disconnected

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
| U4 signal-wiring correction | Michael reports OE = top-row pad 5 from left (1-based), SCL = rightmost, SDA = second from right; ESP ends correct. First board rewired, signal-pin continuity and shorts checked by Michael. Both boards subsequently passed powered PCA writes/readback; second-board results are recorded separately below |
| USB after first-board rework | Initially no device in serial nodes, `ioreg`, or `system_profiler SPUSBHostDataType`, despite BOOT/RESET and reported red power LED. USB watcher then captured connection activity; Michael identified a position-sensitive cable and adjusted it. `/dev/cu.usbmodem101` now answers as the original `KR-88:56:a6:39:ec:f4`. Initial status at 346,199 ms uptime reports `ChipPowerOn`, acknowledged Off, and no output/storage fault. No reflash was needed. Michael reports power is good; no new numerical rail reading or PSU CV/CC indication supplied |
| C3 chip identity and detected flash ID/capacity | ESP32-C3 revision v0.4, 40 MHz crystal, 4 MiB flash detected by espflash; raw flash ID was not printed |
| C3 firmware commit, build target, image/partition fit | Current normal real `hardware-light` image is `b43e6bd`, firmware 0.1.3 / Matter software version 4, `riscv32imc-unknown-none-elf`; 1,930,256 / 4,063,232 app-partition bytes, linked stack 41,200 bytes. Both software gates and flash preflights passed. Earlier images and rejected qualification builds are recorded below |
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
| BLE/Wi-Fi activity, loaded rail, brownout/reset behavior | Wi-Fi joins SyNet-2G and acquires IPv4/IPv6. Home now completes both fabrics; Michael confirms successful addition. Earlier temporary test-controller fabrics were removed before Home pairing. Radio-load rail stability and brownout/reset behavior remain pending |

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

The bounded-scan image `c21825c` was flashed with NVS preserved. At 15,992 ms
uptime, identity and real-output mode matched, intent and acknowledged state
were Off at level 57 / 303 mired, and output/storage/recovery counters were zero.
Explicit `off` and `verify` passed. Pairing reopened with a new capture at
`local/pairing-bounded-scan-retry.log`, which also attempts to reattach if the
same USB port disconnects and marks any capture gap.

The next captured retry passed the bounded scan (ten results and the final
completion log), added `SyNet-2G`, found channel 1 at -58 dBm, and connected at
04:28:00 UTC on September 29 (September 28 local). IPv6 link-local was ready
immediately; DHCP supplied `10.10.1.18` at 04:28:15. At 04:28:47 the board
received Commissioning Complete, persisted fabric 1 and its network settings,
and then primed a subscription. Home proceeded to add fabric 2 under a
30-second failsafe, which expired without a second Commissioning Complete.
Home removed fabric 1 at 04:29:36 and reported failure. The first Wi-Fi and
commissioning stages now pass; complete Apple Home setup still does not.

Read-only U7 checks confirmed multicast enhancement is enabled both in the
controller and the live driver (mode 5), matching Stillair's successful
workaround. The ESP was associated and authorized, and both IPv4 ARP and IPv6
neighbour discovery resolved it. mDNS silence after Home removed the fabric is
not evidence of failed multicast delivery because no live service may remain.
At 488,030 ms uptime the USB status still showed Wi-Fi connected at -57 dBm,
IPv4/local-IP ready, one connection attempt, and zero output/storage/recovery
faults or network timeouts/restarts. `verify` passed and the commissioning
window reopened without a reboot or NVS erase for meaningful discovery probes.

With a live commissioning window, AP-origin targeted mDNS probes passed over
IPv4 unicast, IPv4 multicast, and IPv6 unicast. IPv6 multicast timed out twice
even with QU and hop limit 255. The already working Stillair returned exactly
the same results, so this is not a demonstrated Key Right-specific cause.
AP capture also saw Key Right emit advertisements over both IP families.
Private probe receipts are in `/tmp/key-right-mdns-read-20260929/`.

Two isolated Matter.js 0.17.9 controllers then commissioned the board without
phone intervention. Controller 929 used its known IPv4 address; controller 930
used discovery without a supplied address after controller 929 opened a basic
window. Both completed commissioning, persisted their separate fabrics, and
primed subscriptions at 04:36:38 and 04:37:52 UTC. Secure remote attribute reads
through fabric 2 returned Off, level 57, and 303 mired. This establishes working
two-fabric setup through PASE and operational discovery, but does not reproduce
Home's exact second-fabric flow over an existing CASE session. Both temporary
fabrics were removed successfully at 04:38:52–53, and both controllers exited.
No light-On command, AP setting change, or NVS erase was used. Serial evidence:
`local/mdns-probe-window.log`. At that point the Home timeout remained unresolved.

The named-discovery/QR image `db228e2` then flashed with NVS preserved. At
33,675 ms uptime it had automatically rejoined Wi-Fi at -53 dBm with IPv4 and
local-IP ready, one connection attempt, and zero output/storage/recovery or
network-timeout counters. Off and PCA verification passed. Live AP capture and
an IPv4 multicast query verified `DN=Key Right ECF4` in the new mDNS
advertisement. The actual BLE name and Home picker label remain unobserved.
The explicit USB QR command reopened pairing and generated a private local PNG;
the payload/image is deliberately omitted here. Serial capture is
`local/pairing-named-qr-retry.log`; five-minute AP captures are in
`/tmp/key-right-home-retry-20260929/`.

### Apple Home success

Michael reported that the QR attempt worked. Serial logs show fabric 1's
Commissioning Complete at 04:44:08 UTC and fabric 2's at 04:44:18, followed by
persisted fabric/network settings, Home label updates, and primed subscriptions.
Both AP-interface captures cover the attempt, with 60 IPv6 Matter UDP packets
(24 ESP replies), two operational mDNS identities, and zero capture drops.
The unsupported optional OTA writes did not prevent completion. Home later
removed its initial subscription with `InvalidSubscription`; the second had
primed and was not removed in the retained capture. This does not establish
long-term notification reliability.

The actual private QR image decoded successfully in CoreImage, and independent
Base38/header checks matched the installed discriminator and unchanged setup
credential. The successful attempt also started with Wi-Fi already connected
from preserved settings. These observations do not isolate QR scanning, naming,
or retained networking as the cause of success; no AP setting changed.

The final read-only console check at 491,439 ms uptime reported intended and
acknowledged Off, level 57, 303 mired, Wi-Fi connected at -64 dBm, IPv4/local-IP
ready, one connection attempt, zero network timeouts/restarts, and no
output/storage/recovery fault. PCA `verify` passed. Serial and AP captures were
closed after success. Both Home fabrics were retained, with no unpairing or NVS
erase. Status receipt: `local/home-paired-status.log`. Subsequent Home controls
and cold power-cycle checks are recorded below; the second physical board and
loaded light behavior remain untested.

### Apple Home controls with panels disconnected

The first board remains on normal image `db228e2`, with both Home fabrics intact.
Michael operated Home; the agent used only read-only USB status, register and
verification commands during the control checks. Instrument: DC multimeter.
Each voltage below is across the two pins of the named connector, with no LED
panels attached. Private serial receipts are in
`local/home-controls-bench-20260929.log`.

| Home setting | Settled level / mired | PCA warm/cool | F-1 / F-2 | W-1 / W-2 |
| --- | --- | --- | --- | --- |
| 100%, warmest selected | 254 / 341 | 22 / 0; `verify` passed | Approximately 13 V each | Near 0 V each |
| 100%, coolest selected | 254 / 146 | 0 / 22; `verify` passed | Near 0 V each | Approximately 13 V each |
| Approximately 50%, coolest | 128 / 146 | 0 / 12; `verify` passed | Not measured | Not measured |
| Lowest nonzero selection, coolest | 4 / 146 | 0 / 1; `verify` passed | Not measured | Approximately 0.2 V each |
| Off in Home after cold power cycle | 4 / 146, Off | 0 / 0; all PWM zero, `verify` passed | Near 0 V each | Near 0 V each |

Home reached the configured maximum level. The warm-only command energized the
F connectors while the W connectors remained near zero. This associates those
connector banks with the commanded channel; emitted warm/cool colour is still
unobserved. At 787,129 ms uptime, intended and acknowledged states agreed,
Wi-Fi remained connected, and output/storage/recovery counters were zero.
Home's selected warm endpoint reported 341 mired rather than the advertised
maximum of 344; both produce the observed `22/0` frame at level 254.
The coolest selected setting reported 146 mired and produced `0/22`; Michael
reported that the connector voltages swapped. The two banks therefore respond
independently to the expected warm/cool channel commands. This is unloaded
electrical evidence, not confirmation of panel colour or LED current.
The Home brightness sweep reduced the cool PWM from 22 to 12 to 1, while the
warm channel remained zero. The lowest selected UI value delivered level 4,
not the protocol minimum of 1. At 922,921 ms uptime the intended/applied low
setting agreed, and output/storage/recovery counters remained zero.
Michael then reported approximately 0.2 V at both W connectors at this minimum
setting. This is a further unloaded electrical change, not a duty-cycle or
optical measurement.

Michael then switched the bench supply off for five seconds and back on, with
the VBUS-blocked USB cable attached and panels disconnected. At 8,513 ms after
`ChipPowerOn`, intended and acknowledged On, level 4, and 146 mired had been
restored without USB control commands. At 16,917 ms, Wi-Fi and local IP were
ready, with one connection attempt and no network timeout/restart; IPv4 was
still pending in that snapshot. The PCA read `0/1` and `verify` passed, with
no output/storage/recovery faults. Boot/status evidence is in
`local/home-power-cycle-bench-20260929.log` and the control log above. This
establishes one cold power-cycle restoration with unloaded outputs, not
glitch-free startup or behaviour with the panels connected.

After the restart, Michael turned Off in Home and confirmed all four connector
voltages were near zero. At 59,042 ms uptime, the console confirmed intended
and acknowledged Off, all PWM registers zero, and successful verification.
Wi-Fi, local IP, and IPv4 were ready; output/storage/recovery counters and
network timeouts/restarts remained zero. No re-pairing or USB output command
was required. The serial monitor was closed afterward. The unloaded bench
checks support proceeding to unpowered reassembly and LED reconnection.

The session changed documentation only. Both `sh scripts/check.sh` and
`sh scripts/check-firmware.sh` passed again; no image was flashed during these
Home control checks. Actual loaded output and startup observations remain open
in the acceptance table below.

## Final firmware qualification, 2026-09-28 local / September 29 UTC

Firmware `0.1.1` / Matter software version 2, commit `691b0a7`, flashed with
NVS preserved. The first status restored saved Off, level 4 and 146 mired,
rejoined Wi-Fi, and reported no output or storage fault. A direct operational
mDNS query found both Home identities. Michael's subsequent Home commands
changed the durable targets and verified PCA output, including On at level
150 (59%) and 172 mired. Firmware intent did not revert to level 4 when Home's
card displayed 1% with a persistent spinner. Michael reported that restarting
the Home app cleared the spinner. This does not isolate its cause or establish
long-term reporting reliability. UniFi reported a 100/100 Wi-Fi experience,
-66 dBm and channel 1 during the symptom; firmware RSSI was around -60 dBm.

The `test wifi` diagnostic caused an actual station disconnect. Association
returned within 4.1 seconds, local IPv6 within 6.2 seconds, and IPv4 within
16.5 seconds. Uptime continued; On, level 150 and 172 mired were unchanged,
PCA readback remained `3/14`, and `verify` passed without output/storage faults.

The subsequent `test network` uncovered a real failure in the installed
dependency: rebuilding the BLE GAP service initialized a process-global
`StaticCell` for the device name twice. The second transport start panicked at
05:23:47 UTC. The watchdog reset the MCU (`CoreMwdt0`), after which saved On,
level 77 and 172 mired, both fabrics and Wi-Fi returned. This was a failed
transport-recreation check, despite successful watchdog recovery. The fix is
a narrowly patched copy of the same locked `trouble-host` release; it must
pass repeated live recreation before qualification is complete.

Candidate `0.1.2` / software version 3, commit `00ac734`, passed all software
gates but was rejected after live boot. Its 16-subscription/32-buffer pool left
22,080 bytes of linked main stack. The boot capture reported stored-fabric
loading followed by a load-access exception at `0x403836ca`, with fault address
`0x00000004`. USB logs were dropped, so this does not establish the precise
initialization stage. The truncated dump cannot prove stack overflow; the
reduced headroom is a memory-layout concern. The previous known
working `0.1.0` ELF was temporarily restored without erasing NVS. At 30,029 ms
uptime it again reported On, level 77 and 172 mired, connected Wi-Fi and IPv4/IPv6,
and no output/storage fault. The flash gate now rejects reservations below
32 KiB and has a regression rejecting this candidate's exact 22,080-byte layout.
This stronger static gate still does not replace live validation.

Private serial receipts: `local/final-firmware-live-20260929.log`,
`local/final-spinner-20260929.log`,
`local/final-firmware-recovery-20260929.log`, and the existing Home control log.
No extra connector probing or LED connection was performed.

### Accepted memory profile and repeated recovery, firmware 0.1.3

Commit `b43e6bd`, Matter software version 4, uses 15 subscriptions, 20 IM
buffers and two responders. The real image is 1,930,256 bytes, with 41,200
bytes of linked main stack. The radio heap and transport arena are unchanged.
All software gates, including 75 application host tests, and both flash-image
preflights passed. NVS was preserved while flashing.

At 8,981 ms uptime, the real image restored On, level 77 and 172 mired without
an output/storage fault. Wi-Fi and IPv6 were ready by 12,077 ms, and IPv4 by
21,286 ms. PCA readback was `1/8`; `verify` passed. This accepts the revised
memory layout for the observed boot path, not every possible stack depth.

Two consecutive `test network` operations within that same MCU boot passed.
The first used a five-second retry delay and had Wi-Fi/IPv6/IPv4 ready within
12.5 seconds; the second used the expected ten-second backoff, with IPv6 ready
within 17.5 seconds and IPv4 within 27.8 seconds. Uptime continued and the
restart counter advanced to two. Saved and acknowledged On/77/172 and PCA `1/8`
were unchanged, with successful verification and no output/storage faults.
Neither attempt required a watchdog reset, reflash or re-pairing.

The subsequent actual station disconnect (`test wifi`) re-associated within
4.1 seconds and restored IPv6/IPv4 within 6.2 seconds. Its connection-attempt
counter advanced from three to four; the MCU stayed in the same boot and
retained the same verified output. These are controlled local recovery checks,
not a long AP outage, a reproduced missing-TX-completion fault, or a closed-
housing radio test. No AP setting was changed.

The deliberate watchdog stall reset the MCU in about 15 seconds. Reset reason
was `CoreMwdt0`; saved On/77/172 returned, PCA `1/8` verified, and IPv6/IPv4
were ready by 21,384 ms into the new boot. No output/storage fault occurred.
Michael subsequently changed the controls in Home between checks. The agent
then persisted Off at the new level 41 and 303 mired and verified zero PWM.
A normal software reboot (`CoreSw`) restored that exact Off/41/303 state;
Wi-Fi and IPv6/IPv4 were ready by 11,158 ms. Zero PWM and `verify` passed, and
the state remained Off in the final 33,309 ms snapshot. These checks establish
saved On and Off restoration on this image, without another meter-probing
sequence. Reset receipts are in `local/final-reset-check-20260929.log`.

### Home card reporting after the final reset

After the final reboot, a direct operational mDNS query returned both persisted
Matter identities. Home commands reached the board, and settled PCA readbacks
matched the selected targets. Michael initially confirmed that controls held
and Off remained Off for 20 seconds, then reported that the main card repeatedly
returned to 30%. This reopened the reporting acceptance check.

Michael's last deliberate changes were approximately 60%, then Off. Continuous
USB status from 05:46:21 UTC showed Off, remembered level 153/254 (about 60%),
303 mired, acknowledged Off and no output/storage or network-restart faults.
The last 30% target had been level 77 before the reset tests. The post-reset log
also contains stale-session retries at 05:44:32–35 and no subscription until
05:48:13.896, primed at 05:48:14.403. The pinned SDK logs priming after the
controller accepts the initial report and the subscription response is sent.
This is evidence of delayed controller resubscription, but does not by itself
prove which cached value Home displayed or that every subsequent report was
received. The board did not spontaneously turn On in this continuous capture.
Private receipt:
`local/home-controls-bench-20260929.log`.

Michael subsequently confirmed that the card showed Off. Firmware stayed Off
at level 153/254 and 303 mired throughout the intervening capture, with zero
PWM verified. No restart, reflash, command or Home re-pairing was needed to
resolve the displayed mismatch. This supports delayed Home resubscription and
cached state after the deliberate resets; it does not establish an exact
controller recovery deadline.

The final check ran with that subscription established and no further reset.
At 05:53:13 UTC, Home selected level 115/254 (45%), then temperature 144 mired
at 05:53:15. Settled On readback was warm/cool `0/10`, and `verify` passed.
At 05:53:45 Home selected Off; zero PWM and `verify` passed with the same
remembered level and temperature. Uptime continued, Wi-Fi and IPv4/IPv6 stayed
ready, and no output/storage fault, retry or transport restart occurred.
Michael confirmed that the main card held 45% during the 30-second On check,
then stayed Off without a spinner during the requested one-minute Off check.
This passes the final Home control check and releases the first board for
unpowered reassembly. Firmware remains `b43e6bd` / 0.1.3; subsequent documentation
commits do not require another flash. No further connector probing is owed.

## Second-board flash, 2026-09-28 local / September 29 UTC

Michael connected the second board for flashing and live checks, with no further
probing requested. Native USB on `/dev/cu.usbmodem101` identified ESP32-C3
revision v0.4, a 40 MHz crystal, 4 MiB flash and MAC `88:56:a6:39:f4:14`.
This is distinct from the first board's `88:56:a6:39:ec:f4`.

The unchanged final real image, commit `b43e6bd` / firmware 0.1.3, passed the
image/partition and 32 KiB linked-stack preflight. Application size remains
1,930,256 bytes and linked stack 41,200 bytes. Flashing completed successfully
at 06:00:10 UTC with NVS preserved. An optional full original-flash read did not
complete and was cancelled before flashing; no second-board backup was produced.
The empty backup placeholder was removed. Flash receipt:
`/tmp/key-right-board2-flash.log`.

The application console did not respond after flashing or the tool-driven reset.
A physical RESET press was requested with power and the VBUS-blocked USB cable
left connected. After Michael confirmed the press, the console reported hardware
mode, firmware 0.1.3 and identity `KR-88:56:a6:39:f4:14`. Initial state was Off,
level 57 and 303 mired, with reset reason `ChipPowerOn`, no stored network, and
no output/storage fault. This establishes the default-Off first boot on this
board; the earlier silent console did not establish an application boot failure.

Between 06:10:07 and 06:10:12 UTC, all 24 PCA registers matched the expected
stock configuration and each `verify` passed:

| State | Level | Temperature (mired) | Warm/cool PWM |
| --- | ---: | ---: | ---: |
| Initial Off | 57 | 303 | 0 / 0 |
| On | 57 | 303 | 6 / 2 |
| On | 57 | 200 | 3 / 6 |
| On | 254 | 143 | 0 / 22 |
| On | 254 | 344 | 22 / 0 |
| On | 1 | 143 | 0 / 1 |
| On | 1 | 344 | 1 / 0 |
| Final Off | 57 | 303 | 0 / 0 |

The following software reboot returned `CoreSw` and preserved Off/57/303.
At 10,496 ms uptime, zero PWM and `verify` passed, with output failures, storage
failures and recoveries all zero. Wi-Fi remained unconfigured, as expected.
These are controller acknowledgements and register checks, not new meter or
physical LED measurements. Private receipt: `local/board2-bench-20260929.log`.

The board's own stable QR was read through the explicit USB command and rendered
locally as terminal blocks and a private PNG. Discovery name is `Key Right F414`.
No first-board credentials or NVS were copied. Michael confirmed successful
addition to Home. Operational fabrics 1 and 2 were added at 06:17:23 and
06:18:01 UTC; commissioning-complete receipts at 06:17:58 and 06:18:03 explicitly
confirmed that fabric and network settings were persisted. Two Home controller
subscriptions were primed at 06:17:59 and 06:18:07. Wi-Fi and IPv4/IPv6 were ready
with one connection attempt and no timeout or transport restart. At 508,942 ms
uptime, state remained Off/57/303 with no output/storage fault. Optional attribute
and cluster requests received UnsupportedAttribute/UnsupportedCluster responses;
these did not prevent successful commissioning. Private receipt:
`local/board2-home-controls-20260929.log`.

### Second-board final Home check

Home's controls reached the real driver: level 115/254 (45%) was selected,
followed by 144 mired. Settled warm/cool PWM `0/10` and `verify` passed; Off
verified zero PWM. Michael reported no control-value jumping but an idle spinner.
Further state changes in the capture ended On at level 90/254 (about 35%) and
144 mired. That earlier interval cannot establish an unattended Off pass.

At 06:19:51 UTC, one controller rejected its initial subscription with
`InvalidSubscription`; the SDK removed it while the second subscription remained.
There was no Wi-Fi timeout, reconnect, transport restart, MCU reset or
output/storage fault during these checks. The log does not isolate the spinner's
cause. After fully closing and reopening Home, Michael selected Off at 06:22:13.
Zero PWM and `verify` passed, with level 90 and 144 mired remembered. Michael
confirmed that Off held and the spinner cleared during the requested idle check.
The final read-only snapshot at 770,989 ms uptime still showed Off/90/144,
acknowledged Off, connected Wi-Fi and IPv4/IPv6, and zero output/storage failures
or transport restarts. USB subsequently disconnected and the monitor exited.

This passes the second board's final Home check and releases it for unpowered
reassembly. Both boards run the unchanged real firmware `b43e6bd` / 0.1.3 with
their own persisted Home fabrics. The second board is left saved Off. No further
probing or reflash is required. Loaded operation, grouping and long-term radio
behavior remain separate observations; the first board's deliberate network and
watchdog fault tests were not repeated on the second board.

## Board A update to 0.1.4, 2026-09-29

Michael connected board A and requested the prepared firmware. Native USB
`/dev/cu.usbmodem2101` identified it as the first controller,
`KR-88:56:a6:39:ec:f4`. Before flashing, 0.1.3 reported saved On, level 39 and
343 mired, with matching acknowledgement, no faults and Wi-Fi/IP ready.

The normal `hardware-light` image from checkout
`0848f101eadc60396132ffe5324bdabe509d53dc` (firmware implementation `c626863`)
was built and flashed at 19:09:31 UTC. Both software gates had passed on this
unchanged firmware before the flash. Chip/capacity preflight detected ESP32-C3
revision v0.4, 40 MHz crystal and 4 MiB flash. The verified application occupied
1,931,440 / 4,063,232 bytes; linked main-stack reservation was 41,200 bytes.
ELF SHA-256: `4aa2ae5f8ae1c5f9d96a45b761312342bf0f4ff8490f111b75dd0d2d50b6b629`.
The ordinary flash path preserved NVS, with no erase or commissioning command.

| Check | Observed result |
| --- | --- |
| New real firmware | USB reported hardware mode, firmware 0.1.4 and the same board identity; reset reason `CoreUsbUart` |
| Saved state after flash | At 27,909 ms uptime: On, level 39, 343 mired restored and acknowledged; stock-percent numerator 595/253, both transitions settled |
| Actual PCA readback | `verify` passed; `MODE1=0x80`, `MODE2=0x14`, warm PWM 5, cool PWM 0, all other PWM zero, GRPPWM `0xff`, GRPFREQ 0, LEDOUT0–3 `0xaa` |
| Home storage | Boot logs loaded fabric 1 and fabric 2 from storage after the flash and after the later software reboot; no new pairing performed |
| Network after flash | Wi-Fi connected at -59 dBm, local IP and IPv4 ready; one Wi-Fi attempt, no timeouts or transport restarts |
| Off before reboot | Local `off` and `verify` passed. `reboot` acknowledged `intent_preserved=true off_registers_verified=true` |
| Saved-Off restoration | At 35,208 ms after `CoreSw` reset: intended and acknowledged Off, level 39 and 343 mired retained; physical-output numerator 0; transitions settled; `verify` passed and all sixteen PCA PWM registers read zero |
| Network after reboot | Wi-Fi connected at -56 dBm, local IP and IPv4 ready; one attempt, no timeouts/restarts |
| Faults | Both settled checks: `fault=None`, zero output/storage failures and no output recoveries |

Board A is left saved Off. The flash request did not report resistor installation
or an optical result. No new meter probing, Home interaction, measured physical
output, visual fade, cold-start darkness or closed-housing radio check was
performed. Existing Home fabrics are retained; visual control confirmation is
still separate. Board B remains on 0.1.3.

Private captures: `local/board-a-014-20260929.log` and
`/tmp/key-right-board-a-014-flash.log`. Startup USB-log drops were reported;
the observed restoration/readback results above do not imply a complete log.
Both project software gates passed again after recording the update. A later
optional status request found `/dev/cu.usbmodem2101` absent, and port listing
returned no device. This happened after the successful reboot/status/PCA checks;
the disconnect cause is unknown and the last verified state remains saved Off.

## LED-connected observations and remaining checks

After both boards were released for reassembly, Michael reported that both
lamps work great. Record this as successful overall operation, without
inferring individual startup-flash, optical-calibration, radio-recovery or
grouping checks from that report. The installed image at that observation was
firmware 0.1.3, commit `b43e6bd`, on both lamps. Board A was later updated to
0.1.4 as recorded above; no new optical result is implied.

On 2026-09-29 he reported a faint orange glow, concentrated near the centre,
while Off with power connected. He subsequently clarified that the panel LEDs
themselves are slightly activated: a central dot, edges and other areas glow,
barely visibly even at night. He explicitly rules out the ESP indicator. The
electrical cause is not established. The earlier
zero-PWM readback and unloaded millivolt readings do not supersede this loaded
observation. The [investigation](off-glow-investigation.md) now addresses
leakage, residual drive and a conditional connector-bleeder trial. The earlier
indicator hypothesis and masking plan are withdrawn. No new electrical
measurement exists yet.

The panels cannot be connected until reassembly. Reconnect with power removed,
after the bench results support proceeding. No further routine probing is owed.
Record unloaded connector measurements separately from physical light results,
including the firmware commit and actual observations. Tests with disconnected
LEDs cannot establish the remaining loaded checks below.

| Check | Observation |
| --- | --- |
| Firmware commit and LED reconnection date | Both were observed assembled on `b43e6bd` / 0.1.3 on September 29. Board A later updated to 0.1.4; post-update optical results pending |
| Off after application initialization | Confirmed panel emission at centre, edges and other areas; ESP indicator ruled out; electrical cause unresolved |
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

Firmware 0.1.4 adds 400 ms eased physical transitions, true-zero fade endpoints,
and separate target reporting. It is now installed on board A, with register
and saved-Off reboot checks passed, but has not been visually checked on the
panels. Software results belong in [software validation](software-validation.md).
After reassembly, verify fades, target display and saved-Off reboot visually;
keep the panel-glow investigation separate. Board B awaits its requested update.
Routine meter probing remains complete. Eight-bit PWM can still produce visible
steps near minimum output.

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
