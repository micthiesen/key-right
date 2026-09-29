# Key Right

Rust firmware replaces only the original Elgato Key Light's Realtek controller
with the installed ESP32-C3_MINI_V1. Retain the PCA9635 and stock LED power
circuitry. Reliable local Apple Home control and automatic recovery are the
product goals.

## Architecture

- `docs/spec.md` owns requirements and acceptance criteria.
- `docs/research.md` and `docs/references/` hold observations and source evidence.
- `docs/hardware.md` owns the minimum hardware plan and physical limits.
- `docs/development.md` records build, flashing, console use, and coverage.
- `firmware/core` is dependency-free `no_std` state and PCA9635 logic.
- `firmware/cli` runs the core against an in-memory adapter.
- `firmware/app` is the separate Matter workspace adapted from Stillair; it still
  targets the old XIAO ESP32-C6 and requires a C3 port before flashing this assembly.
- `scripts/check.sh` and `scripts/check-firmware.sh` are host and MCU gates.

The installed board is marked ESP32-C3_MINI_V1, not an official MINI-1U module.
GPIO4 is SDA, GPIO5 SCL, and GPIO6 active-low PCA OE. OE must be open-drain:
set/release HIGH before enabling output mode, then drive LOW to enable the PCA.
Preserve native USB on GPIO18/19; do not copy XIAO antenna GPIO3/14 writes.
Stock U3 is the PCA9635 at address `0x15`, 100 kHz; stock active channels are
LED0/warm and LED4/cool. `docs/hardware.md` owns the user-measured five-wire pad
map and 3.37 V bus/rail readings with approximately 9.9 kΩ pull-ups. These settle
the connection points, not physical output or power stability under radio load.
Plan for 4 MiB flash/no PSRAM, but detect capacity before flashing.

The C3 takes power directly from J6's measured 3.37 V rail at its `3.3` pad.
No buck, C3 `5V` connection, extra pull-ups, translator, lifted PCA pins, or output
interlock is part of the design. The rocker is already bypassed ON. With lamp
wiring attached, use lamp/bench power and USB with VBUS blocked, data and ground
intact. Ordinary powered USB requires disconnecting all five lamp wires first;
unplugging the lamp adapter alone is insufficient. The old C6/buck PDF is historical.
On 2026-09-28 the user reported wiring complete, LEDs disconnected, ready for bench
PSU bring-up. Flashing, powered bench operation, and LED-output tests remain pending.
Do not claim off during cold start, reset, or brownout until the actual light is
observed; there is no independent output cutoff.

Keep the Rust stack, saved-intent/startup policy, and automatic recovery behavior.
The two fixed presets are 3300 K and 5000 K. Signed firmware emulation establishes
warm/cool raw PCA values 6/2 and 3/6 at nominal 3%; these are commands, not an
optical calibration. Keep intended, acknowledged, and measured physical output
distinct. Network recovery must preserve intent and cannot be claimed from
simulated tests.

Use Rust, Cargo, rustfmt, Clippy, and Rust tests. Follow `../triplet` and
`../stillair` for compatible embedded conventions and preserve the pinned Matter
dependency set. Keep hardware, network, time, and persistence I/O out of the
portable core. Use strong types, focused modules, explicit errors, and behavioral
tests for changed control/failure semantics. No TypeScript, Bun, mitools, Biome,
or Zod baseline applies.

## Validation

Run from the root after changes:

```sh
sh scripts/check.sh
sh scripts/check-firmware.sh
```

Use `cargo fmt --all` at the root and `cargo fmt` in `firmware/app`. The MCU gate
currently builds explicit real and simulated C6 images, not C3 compatibility.
Host tests cannot establish wiring, PCA bus levels, startup behavior, radio
performance in the closed housing, or physical output. Keep bench simulation
separate from real I/O and record physical results in `docs/validation-record.md`.
No credentials or hardware are needed for
the software gates.

Personal project: review, verify, commit, and push completed scoped changes to
`main`. Preserve concurrent changes and existing reference material. Update these
living instructions as durable conventions emerge. Keep project knowledge here.
