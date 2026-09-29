# Software verification

## 2026-09-29 smooth transitions, 0.1.4 (not flashed)

Both software gates passed for the prepared release, with the installed lamps
left on 0.1.3. Basic Information software version is **5 / `0.1.4`**. Dependency
pins, NVS layout, network recovery, board wiring and settled PCA configuration
are unchanged.

| Check | Result |
| --- | --- |
| `sh scripts/check.sh` | Formatting, strict Clippy, 34 core/CLI Rust tests, CLI simulation and 29 Python tests passed |
| `sh scripts/check-firmware.sh` | Formatting, strict host/C3 Clippy, 86 application host tests, simulated/real/radio-diagnostic release builds and linked-stack checks passed |
| `sh scripts/flash.sh --check` | Real image: 1,931,440 / 4,063,232 bytes, 47.53%; linked main-stack reservation 41,200 bytes |
| `sh scripts/flash.sh --bench --check` | Simulated image: 1,914,304 / 4,063,232 bytes, 47.11%; linked main-stack reservation 42,592 bytes |

New coverage exercises smoothstep timing and true-zero endpoints, physical
brightness below the normal On floor, interrupted/reversed fades, independent
temperature/brightness tracks, stable Home targets, no per-frame flash writes,
remembered Off settings, linear Matter Move, Stop rounding, scene commits and
zero-first restoration. Existing fault tests cover immediate shutdown and
subsequent recovery to the saved destination. Core tests verify the exact
physical frame and retain readback for quantized-equivalent PWM.

Independent general and invariant reviews found two reproducible defects in
the initial implementation: endpoint deduplication discarded replacement Move
rates, and SlowFade could round a sub-minimum frame upward. Both were corrected.
The original isolated repros and committed regressions pass; focused re-review
found no further issue. Hardware-evidence review checked the separate
[glow investigation](off-glow-investigation.md). Michael later confirmed panel
emission and ruled out the ESP indicator; the document now reflects that
correction. Its electrical diagnosis and resistor candidate remain conditional.

An initial ad hoc application check from the repository root selected the host
architecture and failed in `portable-atomic`. The required gate runs from the
application directory with the explicit C3 target; all three MCU variants passed.
No SDK or dependency change was needed.

These checks do not establish visual smoothness, darkness, or live Home behavior
on 0.1.4. No board was connected, flashed, reset or probed for this release.
Observe the fades and check retained Home targets after the next requested
update. Eight-bit PCA quantization remains visible in principle at low output.
The ELF files remain under
`firmware/app/target/riscv32imc-unknown-none-elf/release/`.

## 2026-09-28 final firmware hardening, 0.1.3

The current target is `esp32c3` / `riscv32imc-unknown-none-elf`, with one Matter
Color Temperature Light per physical lamp. Home's nonzero brightness range maps
to stock nominal 1–10%; temperature spans 143–344 mired.

The final review covers Home target reporting, minimum-level power semantics,
restore-or-Off startup, and recovery from local network/storage faults. The
following software gates passed after these changes. They do not access the
connected board; final-flash observations belong in the validation record.

| Check | Result |
| --- | --- |
| `sh scripts/check.sh` | Passed: formatting, strict Clippy, 26 Rust tests, CLI simulation, and 29 Python tests |
| `sh scripts/check-firmware.sh` | Passed: 75 application host tests, formatting, strict host/C3 Clippy, real/simulated/radio-diagnostic release builds, and linked-stack checks |
| Stock mixing, level-57 reference frames, low/high limits, and quantization tests | Passed; includes full-frame verification, reset/readback faults, and updates without repeated OE blanking |
| Single-light commands, persistence migration, and recovery tests | Passed; includes independent transitions, Stop, timed Off, global scenes, validated scene staging, single-record recall, and failures |
| Home target reporting | Real cluster getter tests cover target values during fades, immediate Off intent, minimum-level Off, fault reads and recovery. Report-loop tests cover data versions, countdown-only updates and completion |
| Startup policy | Saved On and Off restore; missing/corrupt/unreadable intent cannot energize. Old On/Toggle startup overrides migrate to Restore; new On/Toggle writes reject |
| Boot storage retries | Five tests cover the production retry helper, credential restore, partial two-fabric reload, scenes, capped waits/watchdog feeds and permanent decoded-data errors |
| Local network recovery | IPv6 readiness, internal-error streaks, associated radio health, capped backoff, TX stall/escalation, and packet progress through a full queue are covered. These are fault-policy tests, not reproduced radio-driver failures |
| C3 real and simulated release images and partition fit | Both `sh scripts/flash.sh --check` and `sh scripts/flash.sh --bench --check` passed |
| Flash-helper chip/capacity preflight and rejection behavior | Nine mocked flasher/ELF tests passed; includes unknown-capacity fallback, wrong chip, undersized flash, stale Cargo paths, and insufficient stack |
| Documentation links and `git diff --check` | Passed; local targets in all 13 Markdown documents exist, photo conversion visually checked |

### Image and memory evidence

Both ELF files are under
`firmware/app/target/riscv32imc-unknown-none-elf/release/`. The factory partition
has 4,063,232 bytes available within the planned 4 MiB flash layout.

| Image | Application image bytes | Partition used | Linked main-stack reservation |
| --- | ---: | ---: | ---: |
| `key-right` | 1,930,256 | 47.51% | 41,200 bytes |
| `key-right-bench` | 1,910,864 | 47.03% | 42,568 bytes |

Both configure 102,400 bytes of heap: 36,080 bytes in ordinary DRAM and 66,320
bytes in the SDK's reclaimed bootloader RAM. A portable ELF gate rejects a
linked main-stack reservation below 32 KiB. The initial C3 layout reserved only
4,424 bytes; the heap split above addresses that finding. Linked
reservations are not runtime stack or heap high-water measurements.

The pinned Matter/ESP revisions are unchanged. The new direct
`embassy-net-driver = 0.2.0` dependency names the trait already used by the
SDK; it does not upgrade the network stack. Basic Information software version
is 4 / `0.1.3` so the installed image can be identified.

The first review found target/current fade confusion, minimum-level On/Off and
Off-fade relighting errors, stale connection flags, and an unrecoverable boot
read-error path. Follow-up review caught busy-traffic false TX-stall detection
and recovery-history accounting; regression tests cover those corrections.
Original-code scratch tests failed for target getter, minimum-level Off,
Off-fade relighting, and startup On overriding saved Off.

Live testing then exposed a repeated BLE GAP initialization panic. The vendored
`trouble-host` 0.6.0 changes only the device-name storage lifetime. Three tests
cover repeated peripheral/central construction, distinct borrowed names,
read-only access and byte-length limits; the original crate fails these tests.
The build uses a pinned `rs-matter-stack` capacity patch: 15 subscriptions,
20 IM buffers and two request responders. This covers five fabrics with three
subscriptions each, two active RX/TX request pairs and a publishing buffer.
Compiled assertions check the actual unified features against the advertised
minimum. The earlier 16/32 profile passed the old 16 KiB stack gate but failed
live boot with 22,080 bytes reserved. That exact layout is now rejected by a
regression test and a 32 KiB minimum. The dump did not prove the precise crash
cause; static headroom is a guard, not a runtime stack measurement. The 32 KiB
transport arena and 100 KiB heap are unchanged; runtime high-water usage remains
unmeasured.
Fresh review found no additional issue in the patch, its provenance or capacity
sizing. Both final-image preflights passed. Dependency versions were preserved.

The first physical boot of `fb241df` panicked with `Out of bump memory` in
`rs-matter-stack` and entered a watchdog reset loop. Commit `ec1cff9` increases
the separate static Matter transport arena from 20,000 bytes to 32 KiB, costing
12,768 bytes of linked stack space while leaving the 100 KiB heap unchanged.
After reflashing, the hardware USB console answered beyond 100 seconds uptime
with no storage fault. Review found no additional defect in that change;
commissioning and transport restarts were still unverified at that point.
Later live results are recorded separately; runtime memory high-water usage
remains unmeasured.

Review also corrected cancellation of a pending fade to Off, stale flash-image
selection under inherited Cargo settings, and scene/global-state bookkeeping.
Scene recall validates all fields before committing one durable light target
and returns actuation failures. The pinned SDK persists scene bookkeeping
separately, so restored SceneValid is always invalidated at startup while the
scene table is retained.

Software checks establish implementation behavior and image buildability. They
do not establish successful flashing, GPIO voltage levels, rail stability,
light output, Apple Home behavior, or network recovery on the installed assembly.

## 2026-09-28 USB detection watcher

Both software gates passed again after adding `scripts/usb-watch.py`: 26 core/CLI
Rust tests, 36 application host tests, and 28 Python tests. The 15 new watcher
tests cover failed inventory reads, connection changes, quiet terminal output,
private full-detail captures, and log-process cleanup. No firmware was changed.

Live macOS checks passed for timed capture and Ctrl-C shutdown, including
termination of the background log process. The terminal showed existing
USB-C/accessory connections without JSON or idle-timer noise. Detailed registry
snapshots and system logs were saved under ignored `local/` directories. No ESP
USB device was present, so a physical ESP attachment event remains untested;
these checks do not establish post-rework USB or PCA operation.

## 2026-09-28 Wi-Fi discovery diagnosis

Both software gates passed after extending discovery dwell and adding a targeted
pre-association scan: 26 core/CLI Rust tests, 36 application host tests, and 29
Python tests. The flash-helper regression checks that radio diagnostics retain
the real-output image, chip/capacity and partition preflights, and NVS
preservation. Real, simulated, and radio-diagnostic C3 images passed strict
Clippy, release builds, and linked-stack checks. The normal real image reserves
57,912 bytes; this does not measure runtime stack use.

Review checked first-boot station initialization, scan cancellation and radio
recreation at the existing 30-second deadline, fallback after a failed scan,
and the channel hint's all-channel association semantics. The diagnostic image
ran all three scans on the actual board and saw the intended AP at -58 dBm.
Longer explicit scanning does not change the SDK's internal association scan
timing. A fresh Apple Home attempt is still required to establish joining and
commissioning; see [the physical record](validation-record.md).

## 2026-09-28 bounded Matter scan response

The follow-up scan-response regression uses the pinned Matter TLV encoder and
the complete InvokeResponse envelope, including CommandRef, ten 32-byte SSIDs,
and six-byte BSSIDs. It fits in 620 bytes of the actual 1,178-byte exchange
payload. The same encoder returns `NoSpace` for 35 maximum-length results.
Production discovery now caps results at ten before invoking that encoder and
logs callback failures. SSID filtering remains ahead of the cap.

Both software gates passed: 26 core/CLI Rust tests, 38 application host tests,
and 29 Python tests. Normal real, simulated, and radio-diagnostic C3 builds
passed Clippy and linked-stack checks. Independent review found no further
defect in the production cap or error propagation.

This reproduces an overflow mechanism consistent with the latest Home failure,
which ended after discovering 35 APs and before any Wi-Fi connection attempt.
The device's precise encoding error was lost when the cable was replaced.
Successful association and commissioning still require a fresh physical retry.

## 2026-09-28 named discovery and local QR setup

Both gates passed with 26 core/CLI Rust tests, 41 application host tests, and
29 Python tests. Three added tests check the per-device name against the pinned
31-byte BLE advertisement, canonical SDK QR encoding and undersized buffers,
and the complete identity payload's fit. Real, simulated, and radio-diagnostic
images passed Clippy and linked-stack checks; the normal real image reserves
58,256 bytes. Independent review found no additional defect in the name adapter,
credential handling, or command wiring.

The macOS QR renderer round-tripped the public SDK fixture back to its exact
payload. Private 0600 output, invalid-input rejection, and refusal to replace
an existing file or symlink passed. Creation uses exclusive open to enforce
that policy without a check/create race. No setup secret is sent to an online
renderer. The installed BLE name, mDNS DN, and actual Home picker label still
need physical verification; the QR and name do not establish a pairing fix.

## Stock-firmware evidence

On 2026-09-23, offline emulation of the signed original firmware reproduced
initialization, Off, and all 202 integer temperatures from 143 through 344 mired
at stock nominal brightness 3. It established PCA address `0x15`, `MODE2=0x14`,
LED0/warm and LED4/cool, and warm/cool raw PWM `6/2` at 303 mired and `3/6` at
200 mired. See [the analysis](references/firmware-analysis.md) for the method,
source hashes, and limits. The current initial level `57` must preserve those
two reference frames.

This is evidence of commands, not electrical startup or optical calibration.
Brightness percentages use the stock command scale; low-level PWM quantization
can map multiple slider settings to the same register values.

## Physical status

The first board is running firmware 0.1.3 with both Home fabrics retained. Its
corrected wiring, real PCA readback, steady OE levels, unloaded connector
Off/On/Off, Home controls and a cold power cycle passed earlier bench checks.
The final image additionally passed Wi-Fi reconnection, two full transport
recreations within one boot, watchdog restoration of saved On and software-
reboot restoration of saved Off. Software version 4 distinguishes it from the
rejected intermediate images. Home's card temporarily reverted to an older
30% indication while the device remained Off with 60% remembered. Its controller
re-established a subscription about 4.6 minutes after the last deliberate reset,
and the card caught up without another firmware restart. The final check after
resubscription passed: Michael confirmed the card held 45% and then stayed Off
without a spinner; PCA readback and firmware state agreed. The first board was
released for unpowered reassembly with saved Off, and Michael subsequently
reported successful overall operation of that lamp.

The second board passed chip/capacity and image preflight, flashed the unchanged
real firmware 0.1.3, and passed default-Off startup, six On-frame register checks,
Off and saved-Off reboot. Both Home fabrics completed commissioning and were
persisted; two controller subscriptions were primed. Home power, brightness and
temperature commands reached the real driver. An idle spinner cleared after
restarting Home; Michael confirmed Off held, and zero PWM verified. The second
board was saved Off before assembly; the first board's network/watchdog fault
tests were not repeated on it. Michael subsequently reported that both assembled
lamps work well, with faint orange panel emission while Off and powered,
including at the centre and edges. He rules out the ESP indicator. The
electrical cause remains unresolved; register-zero Off is not proof of darkness.

These live results are separate from the build/test gates above. The
[validation record](validation-record.md) retains their measurements, exact
states, failure findings and private-log locations. Michael considers probing
complete; no further routine connector measurements are owed on either board.

Detailed loaded-output/startup checks, closed-housing radio recovery, two-lamp
Home grouping and sustained outages remain
unverified. Use [bench bring-up](bench-bring-up.md) for service and reassembly.
