# Software verification

## 2026-09-28 C3 single-light implementation

The current target is `esp32c3` / `riscv32imc-unknown-none-elf`, with one Matter
Color Temperature Light per physical lamp. Home's nonzero brightness range maps
to stock nominal 1–10%; temperature spans 143–344 mired.

The following checks passed on September 28 for the C3 single-light conversion
and passed again after the bench-discovered arena fix in `ec1cff9`. These gates
run without accessing connected hardware; physical results are recorded below.

| Check | Result |
| --- | --- |
| `sh scripts/check.sh` | Passed: formatting, strict Clippy, 26 Rust tests, CLI simulation, and 13 Python tests |
| `sh scripts/check-firmware.sh` | Passed: 36 application host tests, formatting, strict host/C3 Clippy, both release builds, and linked-stack checks |
| Stock mixing, level-57 reference frames, low/high limits, and quantization tests | Passed; includes full-frame verification, reset/readback faults, and updates without repeated OE blanking |
| Single-light commands, persistence migration, and recovery tests | Passed; includes independent transitions, Stop, timed Off, global scenes, validated scene staging, single-record recall, and failures |
| C3 real and simulated release images and partition fit | Both `sh scripts/flash.sh --check` and `sh scripts/flash.sh --bench --check` passed |
| Flash-helper chip/capacity preflight and rejection behavior | Nine mocked flasher/ELF tests passed; includes unknown-capacity fallback, wrong chip, undersized flash, stale Cargo paths, and insufficient stack |
| Documentation links and `git diff --check` | Passed; local targets in all 13 Markdown documents exist, photo conversion visually checked |

### Image and memory evidence

Both ELF files are under
`firmware/app/target/riscv32imc-unknown-none-elf/release/`. The factory partition
has 4,063,232 bytes available within the planned 4 MiB flash layout.

| Image | Application image bytes | Partition used | Linked main-stack reservation |
| --- | ---: | ---: | ---: |
| `key-right` | 1,914,272 | 47.11% | 57,912 bytes |
| `key-right-bench` | 1,896,336 | 46.67% | 59,288 bytes |

Both configure 102,400 bytes of heap: 36,080 bytes in ordinary DRAM and 66,320
bytes in the SDK's reclaimed bootloader RAM. A portable ELF gate rejects a
linked main-stack reservation below 16 KiB. The initial C3 layout reserved only
4,424 bytes; the heap split above addresses that finding. Linked
reservations are not runtime stack or heap high-water measurements.

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
hardware USB console. PCA operations fail with an I²C acknowledgement error;
Michael subsequently corrected the field guide's U4 signal map and confirmed
the ESP end is correct. The [validation record](validation-record.md) tracks
per-board rework and subsequent register checks. These results are separate
from the software checks above; bench testing remains in progress.

Use [bench bring-up](bench-bring-up.md) for the initial sequence and record
results in [the validation record](validation-record.md). Loaded rail behavior,
startup, physical output, closed-housing radio performance, one Home tile per
lamp, two-lamp Home grouping, and automatic recovery remain unverified.
