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
Confirmed harness colours: black GND, red power, yellow SDA, green SCL, blue OE.
Use wire colour plus signal in probing instructions.
On 2026-09-28 Michael corrected the field guide's U4 pad map: top-row OE is
pad 5 counting from 1 at the left, SCL is rightmost, and SDA is second from
right. The ESP end is correct and remains GPIO4/5/6 for SDA/SCL/OE. Keep power
off for rework.
`docs/hardware.md` owns the physical map; `docs/validation-record.md` owns
per-board rework and testing status. Accept Michael's reported continuity and
short checks; do not request them again without contradictory evidence.
Michael considers the first board's bench probing complete. Proceed with
firmware verification, unpowered reassembly, and loaded operation. The second
board proceeds through chip/capacity preflight, flashing, and live tests without
another routine meter-probing sequence; investigate wiring only if a fault appears.
OE must be open-drain:
set/release HIGH before enabling output mode, then drive LOW to enable the PCA.
Preserve native USB on GPIO18/19; do not invent an antenna-selection GPIO.
Stock U3 is the PCA9635 at address `0x15`, 100 kHz; stock active channels are
LED0/warm and LED4/cool. `docs/hardware.md` records historical 3.37 V bus/rail
readings with approximately 9.9 kΩ pull-ups. The corrected pad map is user
reported; powered PCA readback subsequently passed on the first rewired board.
Michael measured blue/OE to black/GND at 3.34 V Off and 0.01 V On. These steady
levels pass. First-board unloaded connector Off/On/Off, Home power/brightness/
temperature controls, and one cold power-cycle restoration also passed.
Loaded startup transitions, physical LED output, and the second board remain
unverified; do not turn those limits into requests to repeat completed probing.
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
303 mired. Restore valid saved power, level, and temperature by default; an
explicit startup Off remains supported. Reject new startup On/Toggle writes
and normalize old On/Toggle policies to Restore. Keep startup level and
temperature settings. Do not change this to unconditional Off at every boot.
At level 57, preserve raw warm/cool 6/2 at 303 mired and 3/6 at 200
mired. These are command regressions, not optical calibration. Low output has
eight-bit PCA quantization. Do not advertise RGB or Adaptive Lighting.

Two physical lamps are two separately commissioned nodes grouped in Apple Home;
firmware does not couple them or promise simultaneous output. The former
3300 K/5000 K presets are optional Home scenes, not firmware endpoints. Preserve
power, level, temperature, pairing, and startup intent across normal resets and
flashes. Healthy Matter attributes report the durable command target during a
fade; USB diagnostics retain the separate acknowledged intermediate frame.
Known storage/output faults remain errors. WithOnOff at minimum level 1 is Off;
ordinary level commands cannot change an Off target back to On during its fade.
Keep intended, acknowledged, and measured physical output distinct.
Network recovery must preserve intent and cannot be claimed from simulated tests.
The first board (`KR-88:56:a6:39:ec:f4`) successfully joined Apple Home on
2026-09-28 with two persisted Home fabrics. Preserve both; temporary test
controllers were already removed. Routine bench checks must not unpair this
commissioned board or erase its NVS. See `docs/validation-record.md` for
successful commissioning evidence and the remaining physical checks.

Recovery uses local radio/IP/transmit evidence, not Internet reachability or
Home traffic. Keep 30-second radio-operation deadlines, 60-second local-health
deadlines, capped 5–60-second transport retry delays, and the 15-second watchdog.
Boot storage errors reported by the adapter as `StdIoError` retry while feeding
the watchdog; firmware never requests an erase or factory reset. The SDK may
repair storage pages. Malformed decoded records remain explicit faults.
Firmware 0.1.1 recovery and reporting changes require their own live results;
earlier bench passes do not establish the new image's behavior.

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
