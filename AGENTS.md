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
- `firmware/app` is the separate ESP32-C3 Matter workspace adapted from Stillair.
- `scripts/check.sh` and `scripts/check-firmware.sh` are host and MCU gates.

The installed board is marked ESP32-C3_MINI_V1, not an official MINI-1U module.
Current firmware assigns GPIO4 to SDA, GPIO5 to SCL, and GPIO6 to active-low PCA OE.
On 2026-09-28 Michael found the field guide's signal connections incorrect.
The physical SDA/SCL/OE map is withdrawn pending his corrected connections;
do not reuse its U4 pad positions or ESP row-position instructions. Keep power
off for rewiring and confirm end-to-end continuity before resuming powered tests.
OE must be open-drain:
set/release HIGH before enabling output mode, then drive LOW to enable the PCA.
Preserve native USB on GPIO18/19; do not invent an antenna-selection GPIO.
Stock U3 is the PCA9635 at address `0x15`, 100 kHz; stock active channels are
LED0/warm and LED4/cool. `docs/hardware.md` owns the wiring correction and
historical 3.37 V bus/rail readings with approximately 9.9 kΩ pull-ups. Earlier
reported continuity does not validate the withdrawn signal map.
The first board reports 4 MiB flash; no PSRAM is required. Detect each device's
capacity before flashing.

The C3 takes power directly from J6's measured 3.37 V rail at its `3.3` pad.
No buck, C3 `5V` connection, extra pull-ups, translator, lifted PCA pins, or output
interlock is part of the design. The rocker is already bypassed ON. With lamp
wiring attached, use lamp/bench power and USB with VBUS blocked, data and ground
intact. Ordinary powered USB requires disconnecting all five lamp wires first;
unplugging the lamp adapter alone is insufficient.
On 2026-09-28 the wired assembly powered from the 13 V bench supply, the user
measured 3.345 V at the ESP, USB identified the C3, and the real firmware flashed
after an original-flash backup. LEDs remain disconnected. Consult
`docs/validation-record.md` for bench results and remaining acceptance checks.
Do not claim off during cold start, reset, or brownout until the actual light is
observed; there is no independent output cutoff.

Keep the Rust stack, durable intent/startup policy, and automatic recovery.
Each lamp is one Matter Color Temperature Light (device type `0x010C`), with
On/Off, real dimming, and temperature-only Color Control. Home's nonzero 1–100%
brightness maps to stock nominal 1–10%; Home 100% is the 10% ceiling. Support
143–344 mired. First boot is Off with level 57 (about Home 22%, stock 3%) and
303 mired. At level 57, preserve raw warm/cool 6/2 at 303 mired and 3/6 at 200
mired. These are command regressions, not optical calibration. Low output has
eight-bit PCA quantization. Do not advertise RGB or Adaptive Lighting.

Two physical lamps are two separately commissioned nodes grouped in Apple Home;
firmware does not couple them or promise simultaneous output. The former
3300 K/5000 K presets are optional Home scenes, not firmware endpoints. Preserve
power, level, temperature, pairing, and startup intent across normal resets and
flashes. Keep intended, acknowledged, and measured physical output distinct.
Network recovery must preserve intent and cannot be claimed from simulated tests.

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
builds explicit real and simulated C3 images for `riscv32imc-unknown-none-elf`.
Keep the C3 heap split across ordinary and reclaimed bootloader RAM. The MCU
gate and flash helper reject linked main-stack reservations below 16 KiB;
passing that gate does not measure runtime stack or heap use.
Keep the separate Matter transport arena at 32 KiB; 20,000 bytes panicked on
the first real C3 boot. LEDs cannot be reconnected until lamp reassembly;
unloaded measurements at the four two-pin LED connectors do not prove output.
Host tests cannot establish wiring, PCA bus levels, startup behavior, radio
performance in the closed housing, or physical output. Keep bench simulation
separate from real I/O and record physical results in `docs/validation-record.md`.
No credentials or hardware are needed for the software gates. Follow
`docs/bench-bring-up.md` for powered testing, with results recorded separately.

Personal project: review, verify, commit, and push completed scoped changes to
`main`. Preserve concurrent changes and existing reference material. Update these
living instructions as durable conventions emerge. Keep project knowledge here.
