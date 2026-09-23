# Key Right

Rust firmware replacing the original Elgato Key Light's controller with an ESP32,
retaining its PCA9635 and LED power circuitry. Reliable local Apple Home control
and automatic recovery are the product goals.

## Architecture

- `docs/spec.md` owns requirements and hardware acceptance criteria.
- `docs/research.md` and `docs/references/` hold evidence, photos, and datasheets.
- `docs/development.md` records setup, current coverage, and remaining integration.
- `firmware/core` is dependency-free `no_std` intended/applied light state.
- `firmware/cli` runs the real core against an in-memory output adapter.
- `firmware/app` is a separate ESP32-C6 Matter bench workspace adapted from Stillair.
- `scripts/check.sh` and `scripts/check-firmware.sh` are the host and MCU CI gates.

Use Rust and Cargo, rustfmt, Clippy, and Rust tests. Follow maintained hardware
siblings `../triplet` and `../stillair` for evolving conventions. No TypeScript,
Bun, mitools, Biome, or Zod baseline applies to this firmware. Keep strong types,
focused modules, explicit errors, and no debug leftovers. Keep hardware, network,
time, and persistence I/O out of the portable core. Matter over Wi-Fi with BLE
commissioning uses Stillair's `rs-matter-embassy` and `esp-hal` baseline. Preserve
its pinned compatible dependency set; update the ESP patches together.

The build targets ESP32-C6. The exact board, GPIOs, bus voltage/address, output
polarity, channel mapping, calibration, and two preset temperatures remain
unresolved. Do not invent defaults for these.
Preserve the fixed nominal 3% setting; it is not 3% raw PWM. Report acknowledged
output separately from pending intent and from measured physical output. Network
recovery must preserve intent and cannot claim success based on simulated tests.

## Validation and delivery

Run from the root after code changes:

```sh
sh scripts/check.sh
sh scripts/check-firmware.sh
```

Use `cargo fmt --all` at the root and `cargo fmt` from `firmware/app` to format.
Add behavioral tests for changed control semantics and failures. The MCU gate
builds the explicit `bench-light` feature; it does not validate PCA9635 output.
Keep bench simulation clearly labelled and separate from future real I/O.
Record physical validation separately using the spec;
host tests cannot establish wiring, startup flashes, Wi-Fi/HomeKit recovery, or soak
reliability. No credentials or hardware are needed for the current host gate.

Personal project: review, verify, commit, and push completed scoped changes to
`main`. Preserve concurrent changes and existing reference material. Update these
living instructions as durable conventions emerge. Keep project knowledge here.
