# Software verification

## 2026-09-28 final firmware hardening, 0.1.1

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
| `sh scripts/check-firmware.sh` | Passed: 71 application host tests, formatting, strict host/C3 Clippy, real/simulated/radio-diagnostic release builds, and linked-stack checks |
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
| `key-right` | 1,931,808 | 47.54% | 57,600 bytes |
| `key-right-bench` | 1,912,448 | 47.07% | 58,968 bytes |

Both configure 102,400 bytes of heap: 36,080 bytes in ordinary DRAM and 66,320
bytes in the SDK's reclaimed bootloader RAM. A portable ELF gate rejects a
linked main-stack reservation below 16 KiB. The initial C3 layout reserved only
4,424 bytes; the heap split above addresses that finding. Linked
reservations are not runtime stack or heap high-water measurements.

The pinned Matter/ESP revisions are unchanged. The new direct
`embassy-net-driver = 0.2.0` dependency names the trait already used by the
SDK; it does not upgrade the network stack. Basic Information software version
is 2 / `0.1.1` so the installed image can be identified.

The first review found target/current fade confusion, minimum-level On/Off and
Off-fade relighting errors, stale connection flags, and an unrecoverable boot
read-error path. Follow-up review caught busy-traffic false TX-stall detection
and recovery-history accounting; regression tests cover those corrections.
Original-code scratch tests failed for target getter, minimum-level Off,
Off-fade relighting, and startup On overriding saved Off.

The first physical boot of `fb241df` panicked with `Out of bump memory` in
`rs-matter-stack` and entered a watchdog reset loop. Commit `ec1cff9` increases
the separate static Matter transport arena from 20,000 bytes to 32 KiB, costing
12,768 bytes of linked stack space while leaving the 100 KiB heap unchanged.
After reflashing, the hardware USB console answered beyond 100 seconds uptime
with no storage fault. Review found no additional defect in that change;
commissioning, transport-restart behavior, and runtime memory high-water usage
remain unverified.

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

On September 28, the completed assembly powered from the 13 V bench supply
with LEDs disconnected; Michael measured 3.345 V at the ESP. USB confirmed a
C3 revision v0.4 with 4 MiB flash. The full original flash was backed up and
the real image flashed successfully. The arena fix subsequently brought up the
hardware USB console. PCA operations initially failed with an I²C acknowledgement
error. Michael corrected the field guide's U4 signal map, rewired the first
board, and adjusted an intermittent USB cable. The first board then passed Off
and ten On-frame register checks, with no output or storage failures, and was
left at acknowledged Off. The [validation record](validation-record.md) tracks
per-board rework and exact register results. These results are separate
from the software checks above; bench testing remains in progress.

The first board subsequently joined Apple Home successfully. Both Home fabrics
completed commissioning and persisted, and Michael confirmed successful addition
using the locally generated QR. Final read-only status and PCA verification
passed with Off intent and no hardware/storage fault. Both Home fabrics are
retained. This establishes bench commissioning, not loaded output or long-term
network reliability.

Use [bench bring-up](bench-bring-up.md) for the initial sequence and record
results in [the validation record](validation-record.md). Loaded rail behavior,
startup, physical output, closed-housing radio performance, one Home tile per
lamp, two-lamp Home grouping, and automatic recovery remain unverified.
