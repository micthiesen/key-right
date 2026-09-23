# Development

## Current implementation

Key Right contains a portable `no_std` control/PCA9635 core, an in-memory host
simulator, and a separate ESP32-C6 Matter workspace. The real image targets the
Seeed XIAO ESP32-C6 and writes the retained PCA9635 over 100 kHz I²C. The project
GPIO choices are D10/GPIO18 for SDA, D9/GPIO20 for SCL, and D3/GPIO21 for optional
active-low OE control. OE is a board pin, not an independent safety interlock.

| Path | Responsibility |
| --- | --- |
| `firmware/core` | Portable intended/applied state and PCA9635 commands/readback |
| `firmware/cli` | In-memory state simulator; no hardware or network I/O |
| `firmware/app/src/device.rs` | Real XIAO image, Matter, PCA I²C, storage, and recovery |
| `firmware/app/src/main.rs` | Separate simulated-output bench image |
| `firmware/app/host-tests` | Application control and Matter logic with mock hardware/storage |
| `scripts/device.py` | Native USB console client for Python 3 on macOS/Linux |

Matter exposes mutually exclusive On/Off controls with **3300 K** and
**5000 K** metadata at the stock nominal 3% setting. Apple Home may use generic
names; identify and rename the controls as shown in the guide. These are fixed
presets, not an
optical calibration. The controller can verify PCA register contents; that is
not a measurement of emitted light. No software check proves safe cold start or
reset behavior.

## Toolchain and checks

Install Rust using [rustup](https://rustup.rs/). The root manifest requires Rust
1.88 or newer; `rust-toolchain.toml` selects stable, rustfmt, and Clippy. The app
also targets `riscv32imac-unknown-none-elf` and has its own lockfile.

Run from the repository root:

```sh
sh scripts/check.sh
sh scripts/check-firmware.sh
```

The host gate checks formatting, strict Clippy, Rust tests, simulator behavior,
and the Python console client. The firmware gate runs the application host tests,
formatting and Clippy, and release builds for the real and bench images. These
gates require no board or credentials. Use `cargo fmt --all` at the root and
`cargo fmt` inside `firmware/app`.

For a direct real-image build:

```sh
cd firmware/app
cargo build --release --bin key-right --features hardware-light --locked
```

The resulting ELF is
`target/riscv32imac-unknown-none-elf/release/key-right`. The explicit
`bench-light` feature builds the simulated image.

## Flash and USB power

Flash the XIAO and request a pairing code **before connecting any buck wiring**.
For later USB service, disconnect the buck's 5 V lead from the XIAO first, then
connect USB. Unplug USB before reconnecting that lead. The XIAO 5V pad shares USB
VBUS; do not rely on turning off only the 13 V adapter because an unpowered buck
may still be connected to the 5 V pin.

Install the tested flasher and list USB ports:

```sh
cargo install espflash --version 4.5.0 --locked
python3 scripts/device.py --list
```

Use the returned port for flashing and commands:

```sh
sh scripts/flash.sh PORT
python3 scripts/device.py --port PORT status
```

The flash helper builds the real image and preserves NVS. It uses the checked-in
4 MiB partition table: one factory application, no OTA slot, and NVS at
`0x9000..0x19000`. Do not copy populated NVS from another device. For a broken
application, use the XIAO's USB bootloader mode and rerun the flash helper.

While still USB-powered and before adding the buck wiring, request and record the
pairing code:

```sh
python3 scripts/device.py --port PORT commissioning code
```

## Connect and use

The short field guide gives the physical wiring order. After attaching the buck,
the uncommissioned device opens a new pairing window at boot. Before running
output commands, follow the guide's board-specific PCA bus and OE wiring checks.
There is no profile provisioning or startup attestation step.

The code requested before buck wiring is stable for this XIAO. After the
post-assembly boot, add it in Apple Home. Each uncommissioned boot opens a new
15-minute commissioning window. If it expires, power-cycle the lamp to reopen it
and use the same code.

BLE provides Wi-Fi credentials. No passcode or Wi-Fi credentials belong in the
repository. The image uses development Matter identifiers and is a personal,
uncertified accessory.

Useful console commands:

```sh
python3 scripts/device.py --port PORT status
python3 scripts/device.py --port PORT registers
python3 scripts/device.py --port PORT on 1
python3 scripts/device.py --port PORT on 2
python3 scripts/device.py --port PORT off
python3 scripts/device.py --port PORT reboot
python3 scripts/device.py --port PORT --log local/usb.log monitor
```

`registers` reads PCA state at `0x15`; it does not prove physical output. `status`
reports intended and acknowledged state separately and marks physical output
unmeasured. Selecting endpoint 1 or 2 turns that preset on and the other off;
turning off the inactive endpoint leaves the active preset on. Close a monitor
before issuing a command. The USB helper does not retry commands automatically;
a timeout can mean a command executed, so read `status` before repeating it.

The native USB connection is local diagnostics only. Matter state and the saved
intent survive normal firmware resets according to the application policy. No
physical board has been flashed or validated as part of software verification.

The port is based on Stillair commit
`af12fec55430b4af7704dd89636bdd102a0c4158`. Keep compatible dependency revisions
together: `rs-matter-embassy` commit
`f31233a6fd4530ff25ad3bcbd9abf8fe854320aa`, ESP workspace patches commit
`10e48dd74837bae4be663a7d1825d12875363727`, `rs-matter` 0.2.0, and
`rs-matter-stack` 0.1.0. The committed app lockfile records the resolved set.

## Bench and limitations

Wi-Fi scans/connections have a 30-second deadline. An associated interface with
no usable local IP for 60 seconds restarts the transport; IPv6 link-local counts
as usable. Failed transports retry after 5 seconds. A 15-second watchdog handles
stalled execution. Network retries preserve light intent. A Matter operation
that hangs while the CPU and local IP stay healthy may evade these checks; this
case still needs observation on the real device.

The `bench-light` image exercises connectivity with simulated output. It does
not exercise PCA wiring or establish that the lamp turns off during reset. Host
tests likewise cannot validate bus voltage/pull-ups, PCA power-on behavior,
closed-housing radio performance, Apple Home behavior on this assembly, or
physical recovery. Record physical observations separately in
[the validation record](validation-record.md).
