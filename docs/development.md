# Development

## Current implementation and C3 port

Key Right contains a portable `no_std` control/PCA9635 core, an in-memory host
simulator, and a separate Matter workspace. **The installed board is now an
ESP32-C3_MINI_V1, but the application still targets the old XIAO ESP32-C6.**
The existing images and `scripts/flash.sh` must not be used on the installed C3.
Wiring is complete, with the LEDs disconnected; powered bench tests and flashing
are pending. See [hardware.md](hardware.md) for the measured connections.

The C3 port must preserve existing control/Matter behavior while changing:

| Area | Current C6 implementation | Required C3 adaptation |
| --- | --- | --- |
| Cargo and target | `esp32c6` crate features, `riscv32imac-unknown-none-elf` in app config/toolchain | Compatible C3 features and target throughout, with verified builds |
| `src/device.rs` | SDA GPIO18, SCL GPIO20, push-pull OE GPIO21 | SDA GPIO4, SCL GPIO5, open-drain OE GPIO6 initially released HIGH |
| Board setup | XIAO antenna writes on GPIO3/14; XIAO identity in `src/device_matter.rs` | Remove these writes, report actual C3 board, retain USB on GPIO18/19 |
| `scripts/flash.sh` | Explicit `--chip esp32c6 --flash-size 4mb` | Detect C3 and flash capacity; reject mismatch or insufficient space before writing |
| Firmware gates | Real and simulated C6 images | Both C3 builds plus hardware bring-up; C6 success is not C3 validation |

Keep the pinned Rust/Matter set together where compatible. Record necessary
dependency changes rather than silently switching SDKs. OE controls the PCA's
programmed disabled state; it is not an independent safety interlock.

| Path | Responsibility |
| --- | --- |
| `firmware/core` | Portable intended/applied state and PCA9635 commands/readback |
| `firmware/cli` | In-memory state simulator; no hardware or network I/O |
| `firmware/app/src/device.rs` | Current real C6 image entry point |
| `firmware/app/src/device_matter.rs` | Real-image Matter, storage, and recovery integration |
| `firmware/app/src/main.rs` | Separate simulated-output image |
| `firmware/app/host-tests` | Application control and Matter logic with mock hardware/storage |
| `scripts/device.py` | Native USB console client for Python 3 on macOS/Linux |

Matter exposes mutually exclusive On/Off controls with **3300 K** and
**5000 K** metadata at the stock nominal 3% setting. Apple Home may use generic
names; identify and rename the controls after observing their actual output.
These are fixed presets, not optical calibration. PCA register readback is not
a measurement of emitted light or proof of dark cold start/reset behavior.

## Toolchain and checks

Install Rust using [rustup](https://rustup.rs/). The root manifest requires Rust
1.88 or newer; `rust-toolchain.toml` selects stable, rustfmt, and Clippy. The app
currently targets `riscv32imac-unknown-none-elf` and has its own lockfile.

Run from the repository root:

```sh
sh scripts/check.sh
sh scripts/check-firmware.sh
```

The host gate checks formatting, strict Clippy, Rust tests, simulator behavior,
and the Python console client. The firmware gate runs application host tests,
formatting and Clippy, and release builds for the real and simulated C6 images.
These gates require no board or credentials. Use `cargo fmt --all` at the root
and `cargo fmt` inside `firmware/app`. They do not yet test C3 compatibility.

For a direct build of the **current C6 real image**, for software verification only:

```sh
cd firmware/app
cargo build --release --bin key-right --features hardware-light --locked
```

The resulting ELF is
`target/riscv32imac-unknown-none-elf/release/key-right`. The explicit
`bench-light` feature builds the simulated image.

The port is based on Stillair commit
`af12fec55430b4af7704dd89636bdd102a0c4158`. Keep compatible dependency revisions
together: `rs-matter-embassy` commit
`f31233a6fd4530ff25ad3bcbd9abf8fe854320aa`, ESP workspace patches commit
`10e48dd74837bae4be663a7d1825d12875363727`, `rs-matter` 0.2.0, and
`rs-matter-stack` 0.1.0. The committed app lockfile records the resolved set.

## Bench preparation and USB power

The installed C3 is powered directly from the lamp's measured 3.37 V rail.
With lamp wiring attached, power the stock board from the bench PSU/lamp input
and connect USB with **VBUS/5 V blocked, data and ground intact**. A data blocker
or charging-only cable is unsuitable. To use ordinary powered USB, disconnect
all five lamp wires from the C3 first. Turning off or unplugging the lamp supply
alone does not isolate the rail or signal paths.

Keep the LEDs disconnected for initial work. Verify input polarity and record
the bench PSU voltage/current limit; nominal 13 V goes only to the stock lamp
input. Bench power under radio load and USB communication have not yet been
verified. Record them in [the validation record](validation-record.md).

The existing tooling uses espflash 4.5.0. Install it and list USB ports:

```sh
cargo install espflash --version 4.5.0 --locked
python3 scripts/device.py --list
```

Port detection does not make the current flash helper C3-compatible. Before the
first write, the ported helper must verify chip identity and detected flash
capacity. The existing layout requires 4 MiB: NVS at `0x9000..0x19000` and one
factory application at `0x20000..0x400000`, with no OTA slot. Treat 4 MiB as a
budget until the C3 reports its capacity; verify image/layout fit. Preserve NVS
on normal flashes, never erase implicitly, and do not copy populated NVS between
devices. Validate BOOT/RESET native USB recovery on this board during bring-up.
The [Espressif USB guide](https://docs.espressif.com/projects/esp-idf/en/stable/esp32c3/api-guides/usb-serial-jtag-console.html)
describes the chip's fixed Serial/JTAG function; this project retains its Rust
USB implementation rather than switching to ESP-IDF or TinyUSB.

Once the C3 port has passed its build checks and been flashed under the power
rule above, verify USB `status`, issue local `off`, and inspect `registers` before
connecting the LEDs. Request the actual stable per-device pairing code with:

```sh
python3 scripts/device.py --port PORT commissioning code
```

## Console and Matter behavior to retain

These commands describe the implemented C6 application contract to preserve in
the C3 port, not a claim that C3 flashing or Apple Home pairing has been tested.
No profile provisioning or channel-calibration wizard is required. Physical
bench testing uses the real-output image with LEDs disconnected; `bench-light`
is a separate software simulation and cannot test the PCA.

Each uncommissioned boot opens a 15-minute pairing window. The explicit USB
`commissioning code` command can open the window and return the stable code.
Commissioning uses that code in Apple Home; no online QR service is required.
BLE provides Wi-Fi credentials. No passcode or Wi-Fi credentials belong in the
repository. The image uses development Matter identifiers and is a personal,
uncertified accessory.

Useful console commands:

```sh
python3 scripts/device.py --port PORT status
python3 scripts/device.py --port PORT registers
python3 scripts/device.py --port PORT verify
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

The native USB connection is local diagnostics only. The protocol returns
`KR OK` or `KR ERR`; it is not the handoff's proposed `keylight`/JSON interface.
Matter state and durable intent survive normal resets. First boot defaults Off;
saved per-endpoint Matter startup policies can override restored intent, with
endpoint 2 winning conflicting On policies. Initial stock-Off setup and register
readback do not prove that the physical lamp remains dark throughout startup.

Output faults invalidate acknowledgement, attempt Off, and retry durable intent
after five seconds; recovery does not require a new On command. Invalid stored
intent records report a fault and are retained until explicit local Off repairs them.
No physical flash or operational pass is claimed by the software checks.

## Recovery and validation limits

Wi-Fi scans/connections have a 30-second deadline. An associated interface with
no usable local IP for 60 seconds restarts the transport; IPv6 link-local counts
as usable. Failed transports retry after 5 seconds. A 15-second watchdog handles
stalled execution. Network retries preserve light intent. A Matter operation
that hangs while the CPU and local IP stay healthy may evade these checks; this
case still needs observation on the real device.

The `bench-light` image exercises connectivity with simulated output. It does
not exercise PCA wiring or establish that the lamp turns off during reset.
Reported continuity and voltage measurements are recorded separately from
software checks. Host tests cannot establish rail stability under radio load,
PCA power-on behavior, closed-housing radio performance, Apple Home behavior on
this assembly, or physical recovery. Record those results separately in
[the validation record](validation-record.md).
