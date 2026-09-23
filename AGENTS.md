# Key Right

Rust firmware replaces only the original Elgato Key Light's Realtek controller
with an ESP32-C6. Retain the PCA9635 and stock LED power circuitry. Reliable local
Apple Home control and automatic recovery are the product goals.

## Architecture

- `docs/spec.md` owns requirements and acceptance criteria.
- `docs/research.md` and `docs/references/` hold observations and source evidence.
- `docs/hardware.md` owns the minimum hardware plan and physical limits.
- `docs/development.md` records build, flashing, console use, and coverage.
- `firmware/core` is dependency-free `no_std` state and PCA9635 logic.
- `firmware/cli` runs the core against an in-memory adapter.
- `firmware/app` is the separate XIAO ESP32-C6 Matter workspace adapted from Stillair.
- `scripts/check.sh` and `scripts/check-firmware.sh` are host and MCU gates.

The selected controller is the Seeed XIAO ESP32-C6. The project routes D10/GPIO18
to SDA, D9/GPIO20 to SCL, and may route D3/GPIO21 to the PCA9635 active-low OE
only when the board net is physically verified. Stock U3 is the PCA9635 at
address `0x15`, 100 kHz; stock active channels are LED0/warm and LED4/cool. The
actual board connection points, bus levels/pull-ups, and OE route remain physical
checks. Never infer them from a photo or module documentation alone.

Hardware baseline is one ESP dev board, the selected buck converter, and wiring
and insulated mounting. Keep the PCA and driver path intact. No TXU translator,
lifted PCA pins, external antenna, fuse, source jumper, or output interlock is part
of the design. USB and buck power are mutually exclusive: flash/request the
pairing code before buck wiring; for later USB service, disconnect the buck's
5 V lead from the XIAO before connecting USB, and unplug USB before reconnecting it.
Do not claim off during cold start, reset, or brownout until the actual light is
observed; there is no independent output cutoff.

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
builds explicit real and simulated images. Host tests cannot establish wiring,
PCA bus levels, startup behavior, radio performance in the closed housing, or
physical output. Keep bench simulation separate from real I/O and record physical
results in `docs/validation-record.md`. No credentials or hardware are needed for
the software gates.

Personal project: review, verify, commit, and push completed scoped changes to
`main`. Preserve concurrent changes and existing reference material. Update these
living instructions as durable conventions emerge. Keep project knowledge here.
