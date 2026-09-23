# Development

## Current setup

The repository is a Rust/Cargo workspace following the nearby `triplet` and
`stillair` hardware projects. The core has no dependencies, allocation, or `std`
requirement. The CLI uses the host standard library and has only the local core
dependency. `Cargo.lock` is committed. `rust-toolchain.toml` selects stable Rust
with rustfmt and Clippy; the manifests follow the siblings' Rust 1.88 minimum and
2021 edition. Setup was verified with Rust 1.97.1 on macOS arm64.

Install Rust through [rustup](https://rustup.rs/) if needed, then run from the root:

```sh
sh scripts/check.sh
cargo run -p key-right-cli -- simulate on preset-2 brightness=100 off on
```

The host gate checks formatting, treats Clippy warnings as failures, runs behavioral
and CLI tests, and executes a simulator smoke test. CI runs the same gate on Linux.
Use `cargo fmt --all` to format. There are no secrets or environment variables to
configure, so no environment template or credential test preload is needed.

## Implemented control contract

Each simulator invocation is one in-memory session. It starts off at preset one,
applies commands in order, and prints intended state, acknowledged applied state,
nominal brightness, and simulated output-write count. Invalid input exits with
code 2 before the session starts. All output is labelled `mode=simulation`.

The core starts off at preset one without stored intent. `Controller::new(Some(...))`
accepts previously validated intent; storage belongs to the application. Boot
begins with unknown applied state and requires an output acknowledgement. Power
changes preserve the selected preset. Brightness writes are ignored, including
writes of zero; turning off is an explicit power command. Nominal brightness is
3 while on and 0 while off. The preset slots have no guessed Kelvin values.

Commands update intent separately from output. Reconciliation skips redundant
writes, propagates output errors, marks applied state unknown after a potentially
partial failure, and preserves intent for retry. The adapter must apply the entire
state on retry. An acknowledgement means the I/O operation succeeded; it does not
measure the LEDs. The bench application adds flash storage for intent; the physical
driver's I/O deadlines, retry timing, and diagnostics remain future work.

## Matter connectivity baseline

The user selected Stillair's Matter setup on 2026-09-23. `firmware/app` is a
separate `no_std` ESP32-C6 workspace with its own target configuration and lockfile.
It uses BLE commissioning and Matter over Wi-Fi for Apple Home. The exact dev
board and light wiring remain unselected; C6 is the software target inherited from
Stillair. The host workspace needs no ESP dependencies or board SDK.

The port comes from Stillair commit `af12fec55430b4af7704dd89636bdd102a0c4158`:
`firmware/app/src/matter.rs` (stack, entropy, persistence), `src/output.rs` (bounded
USB logging), and its Cargo/target configuration. The upstream on/off-light
example supplies the light-cluster API shape. Its automatic five-second toggling
test implementation is not part of Key Right.

Keep these compatible revisions together:

- `rs-matter-embassy`: `f31233a6fd4530ff25ad3bcbd9abf8fe854320aa`.
- All `esp-hal` workspace patches: `10e48dd74837bae4be663a7d1825d12875363727`.
- The application lockfile preserves the rest of Stillair's resolved versions.

Consult the [Espressif Rust documentation](https://docs.espressif.com/projects/rust/book/)
and [rs-matter-embassy](https://github.com/ivmarkov/rs-matter-embassy) when updating
the stack. A dependency update is a separate change that requires firmware checks
and hardware commissioning/recovery verification.

Build from the root:

```sh
sh scripts/check-firmware.sh
```

This first checks `firmware/app/host-tests` from the repository root, exercising
the actual Matter light hooks with an in-memory store: record round trips,
malformed records, repeated requests, failed saves, and startup policy. It then
runs formatting, Clippy, and a release build in the application's directory, so
Cargo sees its `riscv32imac-unknown-none-elf` target and linker settings. CI has an
independent job for this gate. For a direct build:

```sh
cd firmware/app
cargo build --release --features bench-light --locked
```

The scripts put rustup's shims first in `PATH`. A standalone Homebrew Rust install
can otherwise shadow them and report a missing target even when rustup has it.
For direct firmware commands, use `export PATH="$HOME/.cargo/bin:$PATH"` first if
your shell resolves Cargo/rustc to Homebrew. The app toolchain file installs the
RISC-V target through rustup.

The `bench-light` feature is explicit because this image's light output is
simulated. It is a commissioning scaffold, not a driver for the Key Light's LEDs.
It exposes on/off without a brightness slider. Temperature presets remain in the
portable core until the two temperatures and their calibration are supplied.

Stillair records Apple Home commissioning and operational subscriptions in
`testing/loaded-tuning-2026-08-21.md`, including two cold boots that restored
network/fabric settings. Its network-loss and Matter-hang tests (CTL-01 and CTL-04)
remain unperformed. Those records support reuse of the stack but do not establish
Key Right's recovery or hardware acceptance.

## Bench provisioning and storage

The image requires an ESP32-C6 board with at least 4 MiB of flash and accessible
native USB Serial/JTAG. It configures the radio and USB, with no PCA9635 pins or
LED output. The exact board's USB and bootloader wiring still needs verification.

After selecting and connecting that board, install `espflash` with
`cargo install espflash --version 4.5.0 --locked` if it is unavailable. The commands
below were checked against 4.5.0. From `firmware/app`, use
`espflash list-ports` to identify the board, then replace `PORT` below with its port:

```sh
cargo run --release --features bench-light --locked -- --port PORT
```

The Cargo runner flashes with `partitions.csv` and opens the serial monitor. The
partition table reserves `0x9000..0x19000` for Key Right's sequential-map NVS store
and `0x20000..0x400000` for the application. Firmware discovers the NVS partition
from the table rather than hardcoding its address. Flash this image with its own
table. Do not reuse another project's populated NVS; its contents and ownership
may differ. Ordinary application reflashing with the same table preserves NVS.

The chip's base MAC supplies the stable serial/unique ID and commissioning
discriminator. A fresh NVS gets a hardware-random setup passcode which is stored
and reused. The Matter stack prints commissioning information over USB; supply
Wi-Fi credentials through the controller's BLE commissioning flow. Keep captured
commissioning codes private. No Wi-Fi password belongs in source or a build file.
The image uses public development attestation and test vendor/product IDs, like
Stillair; it is an uncertified bench accessory.

NVS also holds Matter fabrics/network settings and a versioned light-intent
record. Repeated identical requests do not write light intent again. Changed
on/off intent is saved, with failed saves retried every five seconds. The Matter
`StartUpOnOff` setting is retained and applied once when the handler is constructed;
its default retains the last intended state. This restoration concerns simulated
output only.

If the transport returns, the application waits five seconds and restarts it
using the same stack, handler, and stored state. Missing partitions, corrupt
records, or failed startup produce a recurring diagnostic and preserve NVS for
repair. This does not detect a hung task; watchdog and escalation behaviour remain
to be implemented and tested.

To recover broken application firmware, reflash a known-good bench image through
the board's USB bootloader. To deliberately reset pairing and setup credentials,
remove the bench accessory from Apple Home, then erase only Key Right's NVS using
the correct port and table:

```sh
espflash erase-parts --port PORT --partition-table partitions.csv nvs
```

This deletes fabrics, Wi-Fi settings, the passcode, and saved light intent; the next
boot creates a new code and requires commissioning again. No device was flashed,
erased, or commissioned during setup.

## Remaining integration

The remaining work is:

1. Select the ESP32-C6 board and prove USB flash, serial logs, commissioning,
   retained pairing, and bootloader recovery on Key Right.
2. Establish the retained driver's wiring, pull-ups, enables, address, polarity,
   and channels from evidence. Implement and verify the PCA9635 adapter.
3. Supply both temperature presets and calibrate their output against the stock
   3% setting. A nominal percentage must never be copied into raw PWM blindly.
4. Replace bench output with the calibrated driver, add the two preset controls,
   and finish recovery supervision, watchdogs, retained light intent, and diagnostics.
5. Execute every hardware, failure-injection, Apple Home, and seven-day soak check
   in [the specification](spec.md#validation-steps-and-acceptance).

No driver communication, network recovery, HomeKit operation, power-on electrical
behaviour, or physical brightness has been verified by this setup. The simulator
does not model those systems.

## Setup verification

On 2026-09-23, the nine portable-core/CLI tests, six Matter light-hook tests, and a
RISC-V compile of `key-right-core` passed with Rust 1.97.1 on macOS arm64. The
complete host and firmware gates passed, including formatting and Clippy with
warnings treated as errors. The
ESP32-C6 bench release linked and `espflash 4.5.0 save-image` packaged it with the
checked-in partition table: 1,932,816 application bytes in a 4,063,232-byte
application partition. Building the application without `bench-light` fails with
an explicit simulated-image diagnostic. No physical board was used.
