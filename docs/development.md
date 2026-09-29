# Development

## Implementation

Key Right contains a portable `no_std` control/PCA9635 core, an in-memory host
simulator, and a separate ESP32-C3 Matter workspace for the installed
**ESP32-C3_MINI_V1**. Wiring is complete, with the LEDs disconnected; powered
bench tests and flashing are pending. See [hardware.md](hardware.md) for the
measured connections.

The board configuration is:

| Area | Configuration |
| --- | --- |
| Cargo and target | `esp32c3` crate features, `riscv32imc-unknown-none-elf` |
| `src/device.rs` | SDA GPIO4, SCL GPIO5, 100 kHz I²C, open-drain OE GPIO6 initially released HIGH |
| USB and antenna | Native USB on GPIO18/19; no firmware antenna-selection GPIO |
| Flash | Detect C3 and capacity; require at least 4 MiB and verify image/partition fit before writing |
| Firmware gates | Real and simulated C3 images; software checks do not establish physical operation |

Keep the pinned Rust/Matter set together where compatible. Record necessary
dependency changes rather than silently switching SDKs. OE controls the PCA's
programmed disabled state; it is not an independent safety interlock.

| Path | Responsibility |
| --- | --- |
| `firmware/core` | Portable intended/applied state and PCA9635 commands/readback |
| `firmware/cli` | In-memory state simulator; no hardware or network I/O |
| `firmware/app/src/device.rs` | Real C3 image entry point |
| `firmware/app/src/device_matter.rs` | Real-image Matter, storage, and recovery integration |
| `firmware/app/src/main.rs` | Same runtime/Matter/console with simulated output |
| `firmware/app/host-tests` | Application control and Matter logic with mock hardware/storage |
| `scripts/device.py` | Native USB console client for Python 3 on macOS/Linux |

Each lamp exposes one Matter Color Temperature Light (device type `0x010C`).
On/Off, Level Control, and temperature-only Color Control provide one ordinary
Home light. Home's nonzero 1–100% brightness maps to stock nominal 1–10%, so
Home 100% is the selected 10% ceiling. Temperature spans 143–344 mired (about
6993–2907 K). Initial level `57` is about Home 22% and stock nominal 3%.

Pair lamps separately, then group them in Apple Home for shared brightness and
temperature control. There is no firmware coupling or timing guarantee between
lamps. The former 3300 K and 5000 K settings can be Home scenes. RGB and Adaptive
Lighting are not advertised. PCA quantization can give adjacent low slider
settings the same output; neither the UI values nor register readback are
optical measurements or proof of dark cold start/reset behavior.

## Toolchain and checks

Install Rust using [rustup](https://rustup.rs/). The root manifest requires Rust
1.88 or newer; `rust-toolchain.toml` selects stable, rustfmt, and Clippy. The app
targets `riscv32imc-unknown-none-elf` and has its own lockfile.

Run from the repository root:

```sh
sh scripts/check.sh
sh scripts/check-firmware.sh
```

The host gate checks formatting, strict Clippy, Rust tests, simulator behavior,
and the Python console/flash helpers. The firmware gate runs application host
tests, formatting and Clippy, release builds for the real and simulated C3
images, and a minimum 16 KiB linked main-stack check.
These gates require no board or credentials. Use `cargo fmt --all` at the root
and `cargo fmt` inside `firmware/app`. Record actual gate outcomes in
[software validation](software-validation.md).

For a direct real-image build:

```sh
cd firmware/app
cargo build --release --bin key-right --features hardware-light --locked
```

The resulting ELF is
`target/riscv32imc-unknown-none-elf/release/key-right`. The explicit
`bench-light` feature builds the simulated image.

The port is based on Stillair commit
`af12fec55430b4af7704dd89636bdd102a0c4158`. Keep compatible dependency revisions
together: `rs-matter-embassy` commit
`f31233a6fd4530ff25ad3bcbd9abf8fe854320aa`, ESP workspace patches commit
`10e48dd74837bae4be663a7d1825d12875363727`, `rs-matter` 0.2.0, and
`rs-matter-stack` 0.1.0. The committed app lockfile records the resolved set.

The C3 port retains those revisions and adds direct `portable-atomic` and
`esp-metadata-generated` dependencies. Both images keep a 100 KiB heap split
across ordinary DRAM and the SDK's reclaimed bootloader RAM, following its
persistent Wi-Fi example. This avoids consuming almost all of the C3's linked
stack region. Linked memory sizes are recorded in
[software validation](software-validation.md); live high-water usage still
requires bench testing.

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

Follow [bench bring-up](bench-bring-up.md) for the ordered first-power,
read-only chip inspection, preflight, flash, and probing sequence. Install the
pinned flasher and list USB ports:

```sh
cargo install espflash --version 4.5.0 --locked
python3 scripts/device.py --list
```

Before the first write, the flash helper must verify chip identity and detected
flash capacity. The layout requires 4 MiB: NVS at `0x9000..0x19000` and one
factory application at `0x20000..0x400000`, with no OTA slot. Treat 4 MiB as a
budget until the C3 reports its capacity; verify image/layout fit. Preserve NVS
on normal flashes, never erase implicitly, and do not copy populated NVS between
devices. Validate BOOT/RESET native USB recovery on this board during bring-up.
The [Espressif USB guide](https://docs.espressif.com/projects/esp-idf/en/stable/esp32c3/api-guides/usb-serial-jtag-console.html)
describes the chip's fixed Serial/JTAG function; this project retains its Rust
USB implementation rather than switching to ESP-IDF or TinyUSB.

The flash helper supports these separate operations:

```sh
sh scripts/flash.sh --info PORT
sh scripts/flash.sh --check
sh scripts/flash.sh PORT
```

`--info` queries chip/capacity without building or writing flash. `--check`
builds and checks image/partition fit without a connected board. The default
port command checks the connected device before flashing the real image.
`--bench` explicitly selects the simulated image; it cannot validate the PCA.

Once the image has passed its build checks and been flashed under the power
rule above, verify USB `status`, issue local `off`, and inspect `registers` before
connecting the LEDs. Request the actual stable per-device pairing code with:

```sh
python3 scripts/device.py --port PORT commissioning code
```

## Console and Matter behavior

These commands do not establish that flashing or Apple Home pairing has passed.
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
python3 scripts/device.py --port PORT level 57
python3 scripts/device.py --port PORT temperature 303
python3 scripts/device.py --port PORT on
python3 scripts/device.py --port PORT off
python3 scripts/device.py --port PORT reboot
python3 scripts/device.py --port PORT --log local/usb.log monitor
```

`registers` reads PCA state at `0x15`; it does not prove physical output. `status`
reports intended and acknowledged state separately and marks physical output
unmeasured. `level` accepts 1–254; `temperature` accepts 143–344 mired. Both
retain the current power state. `on` restores that level and temperature.
Legacy USB shortcuts `on 1` and `on 2` select level `57` at 303 and 200 mired
respectively and turn on; these do not create extra Matter endpoints.
One serialized state owner handles both Matter and console commands. Close a
monitor before issuing a command. The USB helper does not retry commands
automatically; a timeout can mean a command executed, so read `status` before
repeating it.

The native USB connection is local diagnostics only. The protocol returns
`KR OK` or `KR ERR`; it is not the handoff's proposed `keylight`/JSON interface.
Matter state and durable power, level, and temperature intent survive normal
resets. First boot defaults Off with level `57` and 303 mired. The lamp's saved
Matter startup policy can override restored power intent. Version-3 intent
records store power, level, and temperature. Migrating a version-2 preset record
applies its two startup policies once, selects 303 or 200 mired at level `57`,
and uses Restore as the new startup policy without erasing commissioning.
Initial stock-Off setup and register readback do not prove that the physical
lamp remains dark throughout startup.

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

The `bench-light` image uses the same runtime, Matter, console, and network
paths with simulated output. Status identifies simulation and register values
are simulated. It does not exercise PCA wiring or establish that the lamp turns
off during reset.
Reported continuity and voltage measurements are recorded separately from
software checks. Host tests cannot establish rail stability under radio load,
PCA power-on behavior, closed-housing radio performance, Apple Home behavior on
this assembly, or physical recovery. Record those results separately in
[the validation record](validation-record.md).
