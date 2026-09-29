# Development

## Implementation

Key Right contains a portable `no_std` control/PCA9635 core, an in-memory host
simulator, and a separate ESP32-C3 Matter workspace for the installed
**ESP32-C3_MINI_V1**. The first board has passed powered PCA checks, steady OE
levels, unloaded connector Off/On/Off, Home power/brightness/temperature controls,
and one cold power-cycle restoration. It retains both Home fabrics. Michael
considers bench probing complete; LEDs reconnect during unpowered reassembly.
Firmware 0.1.3 passed real Wi-Fi reconnection, two transport recreations in one
boot, watchdog recovery of saved On and reboot recovery of saved Off.
After delayed Home resubscription, the final target/Off card check also passed.
The first board is ready for unpowered reassembly, with saved Off.
The corrected U4 map is in [hardware.md](hardware.md); ESP pins remain unchanged.
See [the validation record](validation-record.md) for per-board rework and results.

Keep the pinned SDK versions. The repository carries `trouble-host` 0.6.0 in
`firmware/vendor/` with a small, tested GAP device-name lifetime patch, selected
by both application and host-test Cargo manifests. Its original global
`StaticCell` panicked on a second network/BLE transport start. The
[patch record](../firmware/vendor/trouble-host-0.6.0/KEY-RIGHT-PATCH.md) preserves
source provenance and the exact correction; do not remove this patch when
updating lockfiles unless the replacement passes the repeated-build tests.

The Matter build enables 15 subscriptions, 20 interaction-model buffers and
two concurrent request responders.
The pinned core supports at least five fabrics and advertises three subscriptions
per fabric. Each active subscription retains a buffer; additional buffers must
remain available for two request RX/TX pairs and publishing. The pinned
`rs-matter-stack` copy adds these capacity choices and asserts that they cover
the advertised minima. Keep these limits together and run the C3 linked-stack
gate after changing either. This corrects the default capacity mismatch; it is
not proof that the observed Home spinner
was caused by exhaustion.

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
| `firmware/app/src/boot.rs` | Boot storage retries and stable commissioning credential |
| `firmware/app/src/network_tx.rs` | Transmit-capacity monitoring on the existing network interface |
| `firmware/app/src/main.rs` | Same runtime/Matter/console with simulated output |
| `firmware/app/host-tests` | Application control and Matter logic with mock hardware/storage |
| `scripts/device.py` | Native USB console client for Python 3 on macOS/Linux |
| `scripts/usb-watch.py` | Read-only macOS USB/USB-C/accessory and serial-event capture |

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
and the Python console, flash, and USB watcher helpers. The firmware gate runs
application host tests, formatting and Clippy, release builds for the real and
simulated C3 images, the optional radio-diagnostic real image, and a minimum
32 KiB linked main-stack check.
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
stack region. Matter's separate static transport arena is 32 KiB; the initial
20,000-byte example allocation panicked during the first real C3 startup.
Linked memory sizes are recorded in
[software validation](software-validation.md); live high-water usage still
requires bench testing.

## Bench preparation and USB power

The installed C3 is powered directly from the lamp's measured 3.37 V rail.
With lamp wiring attached, power the stock board from the bench PSU/lamp input
and connect USB with **VBUS/5 V blocked, data and ground intact**. A data blocker
or charging-only cable is unsuitable. To use ordinary powered USB, disconnect
all five lamp wires from the C3 first. Turning off or unplugging the lamp supply
alone does not isolate the rail or signal paths.

Nominal 13 V goes only to the stock lamp input. Keep the panels disconnected
until unpowered reassembly. The first board's probing is complete; do not repeat
continuity, OE, rail or connector measurements without contradictory evidence.
The second board proceeds through chip/capacity preflight, flash and live tests
without a new routine probing sequence. Record actual results in
[the validation record](validation-record.md).

Follow [bench bring-up](bench-bring-up.md) for chip inspection, preflight,
flash, firmware checks and loaded acceptance. Install the
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

### USB detection watcher (macOS)

If no serial port appears, run this before trying the cable, Mac ports, or
BOOT/RESET:

```sh
python3 scripts/usb-watch.py
```

The terminal shows short connection/disconnection messages, serial paths,
meaningful USB-C/port-state changes, and errors. Existing connections are marked
`BASELINE`. No JSON or macOS debug-log stream is printed, and idle port timers,
built-in serial nodes, and duplicate `/dev/tty.*` nodes stay quiet. It does not
open serial devices, reset the ESP, or write flash. Keep the existing lamp-power
and VBUS-blocked USB arrangement during diagnosis.

Stop with **Ctrl-C**. The printed `local/usb-watch-*` directory contains
`events.log` (terminal output), `snapshots.jsonl` (detailed changed inventories,
with bookkeeping/idle timers omitted), and `macos.log` (raw system-log output).
The inventories include USB devices, interfaces, host ports, USB-C/accessory
state, serial drivers, and both `/dev/cu.*` and `/dev/tty.*` nodes, including
details hidden from the terminal. Captures are private to the current user,
Git-ignored, and may include hardware serial numbers.

The default polling interval is 0.5 seconds; very brief changes may be missed.
System-log visibility depends on macOS, and no events does not prove an
electrically inactive cable. Errors remain visible and do not count as device
removals. If macOS denies access to its log stream, registry monitoring continues;
rerunning with `sudo` can expose more logs. Use `--no-system-log` for registry-only
capture, `--duration 30` for a timed capture, or `--help` for other options.

### Flash helper

For radio diagnosis, `sh scripts/flash.sh --radio-diagnostics PORT` keeps the
complete light/Matter firmware and compares default, longer active, and passive
boot scans before commissioning. Each reports up to 20 AP names, channels, RSSI
and security modes, with a 15-second deadline. Scan logs may identify nearby
networks; keep captures under `local/`.
This does not join a network or change credentials. Reflash without the option
to remove the boot scan. All chip, partition, stack and NVS-preservation checks
still apply. Use `--radio-diagnostics --check` for an offline image check.

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

After flashing under the power rule above, inspect USB `status` and `verify`
before changing output, so saved-state restoration remains observable. Issue
and verify Off before unpowered reassembly. Preserve the first board's Home
fabrics. For an uncommissioned second board, request its own pairing code with:

```sh
python3 scripts/device.py --port PORT commissioning code
```

## Console and Matter behavior

These commands do not establish that flashing or Apple Home pairing has passed.
No profile provisioning or channel-calibration wizard is required. Physical
bench testing uses the real-output image with LEDs disconnected; `bench-light`
is a separate software simulation and cannot test the PCA.

Each uncommissioned boot opens a 15-minute pairing window. The explicit USB
`commissioning code` and `commissioning qr` commands can reopen the window and
return the existing stable credential as a manual code or standard Matter QR
payload. Both commands reject a device that already has a fabric. On macOS,
render the QR locally with the installed Swift toolchain, then scan it from
Apple Home's Add Accessory camera:

```sh
python3 scripts/device.py --port PORT commissioning qr | swift scripts/pairing-qr.swift /tmp/key-right-pairing.png
open /tmp/key-right-pairing.png
```

The renderer accepts the USB response or a bare `MT:` payload on stdin. It writes
a private PNG and refuses to overwrite an existing file. The image contains the
setup secret; keep it out of Git and shared screenshots. No online QR service
or extra Python package is used. QR scanning avoids typing the code, but retains
Matter's proof-of-possession requirement.

BLE and commissionable mDNS advertise `Key Right XXXX`, where `XXXX` is the last
two MAC bytes in uppercase hexadecimal, for example `Key Right ECF4`. The name
fits alongside the Matter service in the 31-byte BLE advertisement. Apple Home
controls the picker label, so the displayed name still needs device validation.
For an accessory previously paired on the iPhone, Apple supports Add Accessory
→ More options → selecting the accessory. The C3 has no NFC setup hardware.
See [Apple's pairing guidance](https://support.apple.com/en-us/102135) and
[Matter's NFC onboarding explanation](https://csa-iot.org/newsroom/a-smarter-start-matter-1-4-1-makes-setup-easier/).

BLE provides Wi-Fi credentials. No passcode or Wi-Fi credentials belong in the
repository. The image uses development Matter identifiers and is a personal,
uncertified accessory.

Wi-Fi discovery uses 100–300 ms active dwell per channel and returns at most
ten APs in the SDK's descending RSSI order. The pinned Matter encoder builds
one ScanNetworks response without chunking, so an unrestricted result list
can overflow its packet budget. Host tests encode worst-case result entries
with the pinned SDK to keep this bound within the response buffer. SSID
filtering happens before the result cap. Before association,
the driver scans for the requested SSID and uses its strongest result as a
starting-channel hint, with all-channel association and no pinned BSSID. Empty
or failed discovery falls back to all-channel association. Discovery and joining
share the existing 30-second deadline; expiry recreates the radio controller.
The first board subsequently completed Home commissioning with both fabrics;
the second board still needs its own commissioning. The channel is only a
scan-order hint
per the [Espressif station configuration](https://docs.espressif.com/projects/esp-idf/en/v5.5/esp32c3/api-guides/wifi.html).

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
python3 scripts/device.py --port PORT test wifi
python3 scripts/device.py --port PORT test network
python3 scripts/device.py --port PORT test watchdog
python3 scripts/device.py --port PORT --log local/usb.log monitor
```

`registers` reads PCA state at `0x15`; it does not prove physical output. `status`
reports intended and acknowledged state separately and marks physical output
unmeasured. `level` accepts 1–254; `temperature` accepts 143–344 mired. Both
retain the current power state. `on` restores that level and temperature.
Healthy Matter attributes report the durable target during a transition;
`status` still exposes the separately acknowledged intermediate frame. Known
output/storage faults remain read errors. A WithOnOff level command at minimum
level 1 turns Off; ordinary level commands preserve an Off target during its fade.
Legacy USB shortcuts `on 1` and `on 2` select level `57` at 303 and 200 mired
respectively and turn on; these do not create extra Matter endpoints.
One serialized state owner handles both Matter and console commands. Close a
monitor before issuing a command. The USB helper does not retry commands
automatically; a timeout can mean a command executed, so read `status` before
repeating it.

The native USB connection is local diagnostics only. The protocol returns
`KR OK` or `KR ERR`; it is not the handoff's proposed `keylight`/JSON interface.
Matter state and durable power, level, and temperature intent survive normal
resets. Restore valid saved power by default; missing intent starts Off with
level `57` and 303 mired. Explicit startup Off and startup level/temperature
settings remain supported. New startup On/Toggle writes are rejected, and
stored On/Toggle policies normalize to Restore. Version-3 records retain power,
level and temperature. Version-2 preset migration retains saved power and the
selected 303/200 mired temperature at level `57`, with the selected preset's
explicit startup Off retained. Migration never erases commissioning.
OE release and stock zero-PWM initialization minimize output once the ESP runs;
they do not prove darkness before boot or under connected-panel startup.

Output faults invalidate acknowledgement, attempt Off, and retry durable intent
after five seconds; recovery does not require a new On command. Invalid stored
intent records report a fault and are retained until explicit local Off repairs them.
No physical flash or operational pass is claimed by the software checks.

## Recovery and validation limits

Radio operations have a 30-second deadline. Three consecutive internal driver
errors recreate the transport; expected association failures such as an absent
AP do not count as internal errors. While associated, 60 seconds of failed RSSI
queries or missing usable local IPv6 also recreate it. IPv6 link-local is usable;
IPv4 alone is insufficient for Matter readiness.

Transport retries wait 5, 10, 20, 40, then at most 60 seconds. The backoff resets
only after 120 seconds of continuous local health, excluding known transmit
backpressure. The guard uses the existing station interface and requires no
Internet ping or traffic from Home. Sixty seconds without linked transmit
capacity/progress recreates transport; another such stall after recreation
resets the MCU, preserving durable intent and fabrics. Successful capacity or
transmit progress clears the stall timer even under busy traffic. A 15-second
watchdog covers stalled execution. Other hangs that leave these local signals
healthy can still evade detection; live recovery evidence belongs in the
validation record.

Boot partition/credential/Matter/scene operations retry adapter-reported
`StdIoError` with 5–60-second capped backoff. One-second async waits feed the
watchdog and permit USB logging. Firmware requests no erase or factory reset
and does not initialize output from invented state. The pinned storage adapter
also maps corruption/buffer errors to `StdIoError` and may repair pages during
reads. Tests establish logical fabric preservation, not identical physical NVS
bytes. Malformed application/Matter records detected by decoding remain faults.

On a commissioned board, `test wifi` requests a real station disconnect and
normal reconnection; `test network` recreates the complete transport. Neither
unpairs the board. Observe recovery through status and Home, including unchanged
intent and fabrics. `test watchdog` requires settled, verified output and stalls
the firmware task until the 15-second watchdog resets it. The PCA retains its
previous frame during that stall; this is a recovery check, not an Off command.
All three diagnostics passed on the first board with firmware 0.1.3. A separate
software reboot restored saved Off and zero PWM. These controlled checks do not
establish recovery from every AP outage or driver failure, or closed-housing
radio performance. Exact results are in [the validation record](validation-record.md).

After deliberate resets, verify local state separately from Home's cached card.
In the final bench session, Wi-Fi/IP returned within seconds but Home's controller
did not prime a new subscription until about 4.6 minutes after the last reboot.
The card temporarily showed an old 30% target while the board held verified Off,
then caught up without another restart or re-pairing. Allow the controller to
reconnect and test controls after subscription recovery. Do not keep restarting
a healthy device because of a stale card, or treat radio readiness alone as proof
that Home has resumed reports.

The `bench-light` image uses the same runtime, Matter, console, and network
paths with simulated output. Status identifies simulation and register values
are simulated. It does not exercise PCA wiring or establish that the lamp turns
off during reset.
Reported continuity and voltage measurements are recorded separately from
software checks. Host tests cannot establish rail stability under radio load,
PCA power-on behavior, closed-housing radio performance, Apple Home behavior on
this assembly, or physical recovery. Record those results separately in
[the validation record](validation-record.md).
