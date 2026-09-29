# Software verification

## 2026-09-28 C3 single-light implementation

The current target is `esp32c3` / `riscv32imc-unknown-none-elf`, with one Matter
Color Temperature Light per physical lamp. Home's nonzero brightness range maps
to stock nominal 1–10%; temperature spans 143–344 mired.

The following checks passed on September 28 for the C3 single-light conversion.
These are local software results, with no connected hardware.

| Check | Result |
| --- | --- |
| `sh scripts/check.sh` | Passed: formatting, strict Clippy, 26 Rust tests, CLI simulation, and 13 Python tests |
| `sh scripts/check-firmware.sh` | Passed: 36 application host tests, formatting, strict host/C3 Clippy, both release builds, and linked-stack checks |
| Stock mixing, level-57 reference frames, low/high limits, and quantization tests | Passed; includes full-frame verification, reset/readback faults, and updates without repeated OE blanking |
| Single-light commands, persistence migration, and recovery tests | Passed; includes independent transitions, Stop, timed Off, global scenes, validated scene staging, single-record recall, and failures |
| C3 real and simulated release images and partition fit | Both `sh scripts/flash.sh --check` and `sh scripts/flash.sh --bench --check` passed |
| Flash-helper chip/capacity preflight and rejection behavior | Nine mocked flasher/ELF tests passed; includes unknown-capacity fallback, wrong chip, undersized flash, stale Cargo paths, and insufficient stack |
| Documentation links and `git diff --check` | Passed; local targets in all 12 Markdown documents exist, photo conversion visually checked |

### Image and memory evidence

Both ELF files are under
`firmware/app/target/riscv32imc-unknown-none-elf/release/`. The factory partition
has 4,063,232 bytes available within the planned 4 MiB flash layout.

| Image | Application image bytes | Partition used | Linked main-stack reservation |
| --- | ---: | ---: | ---: |
| `key-right` | 1,914,416 | 47.12% | 70,680 bytes |
| `key-right-bench` | 1,896,480 | 46.67% | 72,056 bytes |

Both configure 102,400 bytes of heap: 36,080 bytes in ordinary DRAM and 66,320
bytes in the SDK's reclaimed bootloader RAM. A portable ELF gate rejects a
linked main-stack reservation below 16 KiB. The initial C3 layout reserved only
4,424 bytes; the heap split above addresses that finding. Linked
reservations are not runtime stack or heap high-water measurements.

Review also corrected cancellation of a pending fade to Off, stale flash-image
selection under inherited Cargo settings, and scene/global-state bookkeeping.
Scene recall validates all fields before committing one durable light target
and returns actuation failures. The pinned SDK persists scene bookkeeping
separately, so restored SceneValid is always invalidated at startup while the
scene table is retained.

Software checks establish implementation behavior and image buildability. They
do not establish successful flashing, GPIO voltage levels, rail stability,
light output, Apple Home behavior, or network recovery on the installed assembly.

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

As of September 28, Michael has supplied pad continuity, bus/rail voltage, and
pull-up measurements and reports completed C3 wiring with LEDs disconnected.
No first-power, flash, USB session, PCA exchange, or optical pass for the
completed assembly has been reported.

Use [bench bring-up](bench-bring-up.md) for the initial sequence and record
results in [the validation record](validation-record.md). Loaded rail behavior,
startup, physical output, closed-housing radio performance, one Home tile per
lamp, two-lamp Home grouping, and automatic recovery remain unverified.
