# Development

## Current implementation

Key Right has a dependency-free `no_std` control core, a host simulator, and a
separate ESP32-C6 Matter workspace. The real image targets the Seeed XIAO ESP32C6
and exposes two mutually exclusive On/Off Light endpoints: **3300 K** and
**5000 K**, both reproducing the stock firmware's nominal 3% setting.

The real adapter drives two ESP32 LEDC channels through a TXU0102 level translator
with an independently disabled output. It does **not** communicate with the
PCA9635. The PCA's two original PWM feeds must be disconnected from the retained
LED driver inputs. [Hardware](hardware.md) owns the exact parts, wiring, power
jumper rules, antenna installation, and electrical acceptance procedure. Firmware
and host tests do not establish that a particular assembly is safe or correctly
wired.

| Path | Responsibility |
| --- | --- |
| `firmware/core` | Intended/applied state, validated profile format, stock PWM values, portable PCA9635 reference driver |
| `firmware/cli` | In-memory simulator and stock profile generation/inspection |
| `firmware/app/src/device.rs` | Real `key-right` image with USB provisioning, direct PWM, Matter, and recovery supervision |
| `firmware/app/src/main.rs` | Separate `key-right-bench` image with simulated light output |
| `firmware/app/host-tests` | Actual application control/Matter logic exercised with mock storage and output |
| `scripts/device.py` | Native USB console client; Python 3, macOS/Linux, no extra packages |

The portable core never owns hardware, network, time, or persistence I/O. It keeps
intent separate from acknowledged output, preserves intent after failure, and
requires a complete output application on retry. Brightness commands in the core
are ignored, including zero; power-off is explicit. The physical Matter endpoints
do not advertise a brightness slider or a continuous colour-temperature control.

## Toolchain and checks

Install Rust through [rustup](https://rustup.rs/). The manifests use Rust 1.88 as
the minimum and the 2021 edition; `rust-toolchain.toml` selects stable Rust,
rustfmt, and Clippy. The app toolchain also selects
`riscv32imac-unknown-none-elf`. The checked builds used Rust 1.97.1 on macOS arm64.
Both workspaces have committed lockfiles.

Run from the repository root:

```sh
sh scripts/check.sh
sh scripts/check-firmware.sh
```

The host gate runs formatting, strict Clippy, core/CLI tests, a simulator smoke
test, and Python console-client tests. The firmware gate first checks and tests the
app's host harness from the root, then runs formatting, strict Clippy, and release
builds for **both** real and bench images from `firmware/app`. CI runs these gates
on Linux. No hardware, credentials, or environment file is needed for these gates.

Use `cargo fmt --all` at the root and `cargo fmt` inside `firmware/app`. The separate
host harness can be formatted with
`cargo fmt --manifest-path firmware/app/host-tests/Cargo.toml` from the root.
Scripts put rustup's shims first in `PATH`: a standalone Homebrew Rust installation
can otherwise shadow them and report a missing target. For direct commands:

```sh
export PATH="$HOME/.cargo/bin:$PATH"
cd firmware/app
cargo build --release --bin key-right --features hardware-light --locked
```

The resulting ELF is
`firmware/app/target/riscv32imac-unknown-none-elf/release/key-right`, relative to the
repository root. Select the real binary explicitly: the bench binary deliberately
fails compilation without its separate `bench-light` feature.

## Flash the real image

Follow [the hardware power rules](hardware.md) first. In particular, **open the
external 5 V supply shunt before connecting USB**. Only close it for standalone
operation after USB is unplugged. Keep the light disconnected during initial
assembly checks as directed by that procedure.

From the repository root, install the tested flasher and list USB candidates:

```sh
cargo install espflash --version 4.5.0 --locked
python3 scripts/device.py --list
```

Replace `PORT` in all examples with the exact returned path, such as
`/dev/cu.usbmodem...` on macOS or `/dev/ttyACM0` on Linux. Close any serial monitor
before flashing or issuing console commands. Build and flash:

```sh
sh scripts/flash.sh PORT
python3 scripts/device.py --port PORT status
```

The helper builds `key-right` with `hardware-light`, verifies the ESP32-C6 target,
and flashes the checked-in 4 MiB partition table. It preserves NVS and does not
open a monitor. For reference, its flash operation is equivalent to the following
command **from `firmware/app`**, after the build above:

```sh
espflash flash --chip esp32c6 --flash-size 4mb \
  --partition-table partitions.csv --port PORT --non-interactive \
  --skip-update-check target/riscv32imac-unknown-none-elf/release/key-right
```

There is one factory application partition and no OTA update path. NVS occupies
`0x9000..0x19000`; the application occupies `0x20000..0x400000`. Firmware discovers
NVS through the partition table. Use this image's table and never copy populated
NVS from Stillair or another device. This is a sequential-map store, not an
interchangeable stock ESP-IDF NVS image.

## Provision and pair

A fresh device has no active output profile. It holds translator OE low, reports
`fault=MissingProfile`, and does not start the Matter transport. These software
states require the specified isolation hardware to keep the physical light off.

The stock candidate is checked in at
[`hardware/profiles/stock-3300-5000.hex`](../hardware/profiles/stock-3300-5000.hex).
To inspect it without touching a device:

```sh
cargo run -p key-right-cli --locked -- profile inspect hardware/profiles/stock-3300-5000.hex
cargo run -p key-right-cli --locked -- profile stock
```

The second command prints the same uncommissioned candidate as 160 hexadecimal
digits. The 80-byte record contains a version, CRC, stock PCA frame evidence,
temperature values, and physical acceptance flags. The real adapter accepts only
the exact decoded stock configuration; arbitrary PCA modes, extra channels,
inversion, grouping, or replacement PWM values are rejected.

After completing the disconnected electrical checks in [Hardware](hardware.md),
stage the candidate over USB:

```sh
python3 scripts/device.py --port PORT profile stage @hardware/profiles/stock-3300-5000.hex
python3 scripts/device.py --port PORT profile show
python3 scripts/device.py --port PORT profile test off
python3 scripts/device.py --port PORT outputs
```

Staging stops normal output and clears any imported acceptance flags. It does not
activate a profile or authorize Matter control. Candidate tests are local only,
do not save an On intent, and never become an acknowledged Matter state.
While staging or testing, On commands are Busy; Off from either existing Matter
endpoint or the USB console remains available to stop output.

Perform each powered test only at the corresponding step of the hardware
procedure. Start the 3300 K test, observe and record it before continuing:

```sh
python3 scripts/device.py --port PORT profile test 1
```

Each candidate test stops automatically after about ten seconds, checked by the
100 ms maintenance loop. Use `off` to stop early. Then perform the 5000 K test:

```sh
python3 scripts/device.py --port PORT profile test 2
```

The acceptance bits mean: `1` wiring, `2` disabled-output isolation, `4` cold
boot/reset behaviour, and `8` both stock 3% presets. Only after the required
physical checks pass, assert all four locally and commit:

```sh
python3 scripts/device.py --port PORT off
python3 scripts/device.py --port PORT profile attest 15
python3 scripts/device.py --port PORT profile commit
python3 scripts/device.py --port PORT status
```

Attestation is the operator's recorded assertion, not an automatic measurement.
Commit saves Off intent before the profile, activates the profile in the Off
state, and permits Matter startup. No source edits or firmware rebuild are needed
for this provisioning step. Repeat local attestation for each installation;
exporting and importing a profile cannot transfer it.

Request the manual Apple Home pairing code:

```sh
python3 scripts/device.py --port PORT commissioning code
```

The reply contains `pairing_code=` followed by an 11-digit manual code and opens
or renews a 15-minute commissioning window. Add the accessory in Apple Home using
that code; BLE commissioning supplies Wi-Fi credentials. The command is available
only after profile commit and before a Matter fabric exists. For an already
commissioned device, use the controller's accessory-sharing flow.

The chip MAC supplies stable serial/unique identity and discriminator. A fresh
NVS receives a hardware-random passcode which is stored and reused. The real image
returns its code only on explicit USB request. It uses public development
attestation and test vendor/product IDs, so it remains an uncertified personal
accessory. No live Wi-Fi credentials or passcode belong in the repository.

## Matter controls and saved state

| Endpoint | Fixed label | On | Off |
| --- | --- | --- | --- |
| 1 | `3300 K` | Select preset 1 and turn on | Turn off only if preset 1 is selected |
| 2 | `5000 K` | Select preset 2 and turn on | Turn off only if preset 2 is selected |

Both are standard On/Off Light devices. Selecting one makes the other read Off;
an Off command to the inactive endpoint does not change intent. The fixed labels
are metadata; Apple Home's actual presentation and names still need testing.
Rename the two controls in Home if necessary.

Reads reflect acknowledged adapter output. Unknown output returns a Matter error
instead of optimistic On/Off state. The adapter checks LEDC duty, timer, clock,
channel configuration, GPIO routing, and the translator-enable output latch.
These checks do not measure downstream voltage, emitted light, brightness, or CCT.
On each application it disables the translator, sets the complete PWM state,
waits 50 microseconds, verifies it, and only then enables On output.

The raw 8-bit duties are warm/cool **6/2** for 3300 K and **3/6** for 5000 K;
Off is 0/0. These reproduce the decoded stock 3% command, not a raw 3% duty cycle.
The nominal carrier is 97,680.10 Hz with the pinned HAL's divider, approximately
0.0244% above the PCA's nominal 97,656.25 Hz. Waveform and physical equivalence
still require the hardware acceptance measurements.

Changed intent is stored before output is applied. A failed save returns an error,
isolates output, marks acknowledgement unknown, and retains pending RAM intent for
retry. Repeated identical intent does not write flash. Local `off` always asserts
physical isolation even when intent was already Off. NVS holds Matter fabrics and
network settings, the passcode, active profile, and a versioned real-image record
containing power, selected preset, and both startup policies. The bench has its
own separate intent record.

A transient read failure keeps output isolated and retries both application
records every five seconds. Startup policies run only after the records load
successfully. A local Off requested during that wait is retained and saved once
the valid record is available, so recovery cannot undo the stop. Successfully read
but malformed records are a distinct `InvalidRecord` fault: staging, candidate
tests, and Off preserve their bytes. An explicit profile commit is the repair
boundary, saving valid Off intent before replacing the profile.

### StartUpOnOff and timed commands

Each endpoint implements and persists Matter `StartUpOnOff`. Its default is null,
which retains the last intended state. Off, On, and Toggle are supported. Policies
are applied once per firmware boot in endpoint order: endpoint 1, then endpoint 2.
Endpoint 2 wins if both request On. Null leaves the preceding result unchanged;
Toggle is evaluated against the result after the earlier policy. The resulting
boot intent is saved before output is applied. A transport restart does not
reapply these policies. A settings write changes future startup behaviour, not
current power, and a failed write is not acknowledged as accepted.

The Lighting On/Off attributes and timed commands are implemented. Timed cutoff
runs independently of the network; selecting the other preset clears the old
endpoint's timer. Timer countdowns are not durable across a firmware reset.
`OffWithEffect` applies final Off without introducing an uncalibrated intermediate
brightness. Identify has no physical blink effect.

## USB diagnostics

Use one console client at a time. Commands are one ASCII line, with a bounded
256-byte input. Replies are `KR OK ...` or `KR ERR ...`; ordinary log lines have
level prefixes. The helper uses a ten-second timeout and never retries or toggles
reset lines. A timeout or disconnect can mean the command executed: inspect fresh
status before deciding to repeat it.

Useful commands from the repository root:

```sh
python3 scripts/device.py --port PORT status
python3 scripts/device.py --port PORT outputs
python3 scripts/device.py --port PORT verify
python3 scripts/device.py --port PORT on 1
python3 scripts/device.py --port PORT on 2
python3 scripts/device.py --port PORT off
python3 scripts/device.py --port PORT profile show
python3 scripts/device.py --port PORT --log local/usb.log monitor
```

Run On commands only on a provisioned and physically accepted assembly. Stop the
monitor before another command. `local/` and log files are ignored by Git;
captures can contain commissioning credentials.

`status` reports firmware version, MAC identity, uptime, reset reason, intended
power/preset, acknowledged state, fault, profile presence, output/storage failure
and recovery counters, Wi-Fi association/RSSI, local IP readiness, scan/connect
attempts and timeout/restart counters, and dropped USB logs. Every status explicitly
reports `physical_output=unmeasured`. `configured=true` means a committed profile
exists, not that physical output was measured.

`outputs` reports actual LEDC command and active duty registers, duty resolution,
divider, source frequency, and translator-enable latch. Expected values:

| State | Warm command/active Q4 | Cool command/active Q4 | `translator_oe` |
| --- | --- | --- | --- |
| Off | 0 | 0 | false |
| 3300 K | 96 | 32 | true |
| 5000 K | 48 | 96 | true |

Q4 is the register representation: raw 8-bit duty multiplied by 16. Normal
configuration is `duty_bits=8 divider_q8=819 clock_hz=80000000`. The `registers`
command explicitly rejects PCA access with `no_PCA_bus_use_outputs`.

| Fault | Meaning and next step |
| --- | --- |
| `None` | No current runtime fault; physical output is still unmeasured |
| `MissingProfile` | Output disabled; complete local provisioning |
| `Testing` | Bounded local candidate test; Matter output remains unacknowledged |
| `Storage` | Record read/save failed; isolated output and automatic retry, inspect logs and storage counters |
| `InvalidRecord` | Stored profile/intent failed validation; output stays isolated pending explicit local repair |
| `Output` | Output application/readback failed; isolated output and automatic retry |
| `Rebooting` | Deliberate software reset is pending; output isolated without changing saved intent |

The bounded logger holds 16 records of up to 1024 characters and drops ordinary
logs under backpressure. USB disconnection does not block the maintenance or Matter
futures. Early partition, credential-store, or Matter-load failures can occur before
the command console starts; they emit recurring diagnostics while intentionally
feeding the watchdog, preserving flash for repair instead of entering a reboot
loop.

## Recovery and fault injection

| Mechanism | Current policy |
| --- | --- |
| Maintenance | Poll every 100 ms; advance local timers and feed the watchdog |
| Output verification | Read back every 5 seconds; failure isolates and invalidates acknowledgement |
| Output retry | Reapply full configuration after 5 seconds, preserving intent |
| Pending storage retry | Maintenance retries; repeated failures back off for 5 seconds |
| Boot record read failure | Retry every 5 seconds while isolated; malformed decoded records require explicit repair |
| Scan/connect operation | 30-second deadline including controller-lock acquisition |
| Missing local IP | While associated, 60 seconds without usable local IPv4 or IPv6 triggers transport restart; checked every 2 seconds |
| Transport exit/timeout | Cancel and recreate Wi-Fi/BLE transport, then retry after 5 seconds, retaining Matter state, NVS, and light intent |
| Thread/executor stall | TIMG0 system watchdog resets after approximately 15 seconds without maintenance feeds |

A missing AP uses the network manager's reconnect loop. No reset is triggered merely
because the internet or Home controller is absent. IPv6 link-local readiness is
valid for local Matter, so lack of a DHCP lease alone is not a fault. Network
recovery does not intentionally toggle the physical output.

**Remaining blind spot:** a logically hung Matter operation with a running executor
and usable local IP can escape these checks. The watchdog detects lost maintenance
progress, not all live protocol failures. End-to-end Apple Home fault injection and
soak testing remain required; this implementation does not prove complete recovery.

For a deliberate software reset, preserving durable intent:

```sh
python3 scripts/device.py --port PORT reboot
```

This isolates output, attempts to flush the acknowledgement within one second,
then resets. Boot applies the saved startup policies; their default restores the
last intended state. If USB disappears before the reply, inspect status after
reconnection to determine the outcome.

For the separate watchdog acceptance test, first select the intended preset and
confirm `verify` succeeds. Then, with the physical observation/measurement setup
ready, deliberately stall the firmware:

```sh
python3 scripts/device.py --port PORT test watchdog
```

This USB-only command requires a committed profile, verified acknowledged output,
and no active staging/test/fault or unsaved intent. It acknowledges and attempts a
bounded flush, then spins without feeding the watchdog. LEDC retains the last duty
until the expected reset about 15 seconds later; this command does not pre-emptively
turn the light off or rewrite intent. The device cannot answer console commands
during the stall. After USB reappears, run `status` and `outputs`, check the reset
reason and restored state, and record actual reset-time isolation/no-flash behaviour.
Host tests validate command admission and unchanged intent, not the physical reset.

### Reflash or deliberately erase configuration

For broken application firmware, use the XIAO's documented USB download mode and
rerun `sh scripts/flash.sh PORT` with a known-good checkout. Ordinary reflash keeps
NVS. Inspect diagnostics before erasing; invalid application records are preserved
until an explicit local repair. Restaging and committing a validated profile can
repair those application records. Persistent partition/store failures may require
a deliberate reset of NVS.

To intentionally remove all saved setup, first remove the accessory from Apple
Home. With the correct port and hardware power arrangement, run from the root:

```sh
espflash erase-parts --chip esp32c6 --port PORT \
  --partition-table firmware/app/partitions.csv \
  --non-interactive --skip-update-check nvs
```

This deletes fabrics, Wi-Fi settings, passcode, active profile, light intent, and
startup policies, including the separate bench record. It leaves the application
image intact. The next boot generates a new passcode and remains disabled with no
profile. Repeat the physical provisioning/attestation and pairing process. Firmware
never automatically erases NVS to recover a fault.

## Bench image and inherited stack

The explicit `bench-light` image is retained for Matter connectivity experiments
with simulated output. It has one On/Off endpoint, no brightness control, and no
physical light driver. It does not exercise the two-endpoint hardware runtime,
TXU isolation, LEDC, or the real-image recovery supervisor.

From `firmware/app`:

```sh
cargo build --release --bin key-right-bench --features bench-light --locked
cargo run --release --bin key-right-bench --features bench-light --locked -- --port PORT
```

The Cargo runner flashes the partition table and opens a monitor. Use a separate
ESP32-C6 bench board with at least 4 MiB flash and native USB. Its passcode is also
random and persisted, but the bench stack prints commissioning information to USB.
Its retained intent and `StartUpOnOff` operate only on simulated output. A returned
transport error retries after five seconds; a hung task is not supervised as in
the real image. Bench success does not count as hardware acceptance.

The port is based on Stillair commit
`af12fec55430b4af7704dd89636bdd102a0c4158`: `firmware/app/src/matter.rs` supplied
transport, entropy, and NVS integration; `src/output.rs` supplied bounded USB
logging; Cargo/target configuration supplied the compatible dependency baseline.
The upstream On/Off example supplied API shape, but its automatic five-second
toggling test device is not used.

Keep these revisions together:

- `rs-matter-embassy`: `f31233a6fd4530ff25ad3bcbd9abf8fe854320aa`.
- All ESP workspace patches: `10e48dd74837bae4be663a7d1825d12875363727`.
- `rs-matter` 0.2.0 and `rs-matter-stack` 0.1.0, with the remaining resolved
  Stillair versions preserved in the app lockfile.

The real image's `network.rs` and `network_driver.rs` adapt the pinned SDK's
`src/wifi/esp.rs` and `src/wireless/wifi/esp_wifi.rs` for bounded operations,
peripheral restart, RSSI, and local-interface metrics. `hardware.rs` uses the pinned
HAL's LEDC implementation and public read-only register access. Dependency updates
are separate changes requiring both software gates and fresh physical commissioning
and recovery checks. Consult the [Espressif Rust documentation](https://docs.espressif.com/projects/rust/book/)
and [rs-matter-embassy](https://github.com/ivmarkov/rs-matter-embassy).

Stillair's `testing/loaded-tuning-2026-08-21.md` records Apple Home commissioning and
operational subscriptions, including two cold boots restoring fabrics/network
settings. Its network-loss and Matter-hang tests, CTL-01 and CTL-04, were not
performed. That evidence supports stack reuse but does not validate Key Right.

## Validation status

On 2026-09-23, the firmware gate passed with **27 application host tests**, strict
Clippy, formatting, and both ESP32-C6 release builds. The portable core, reference
PCA driver, CLI, and console helper have separate host tests in the root gate.
The app tests cover persistence ordering/failures, transient reads, malformed-record
preservation through staging, profile attestation, bounded candidate tests,
acknowledgement failures/retry, mutually exclusive endpoints, startup policies,
timed commands, local-IP timeout policy,
and watchdog-test admission. Mock output and storage cannot validate ESP peripheral
behaviour or electrical safety.

No physical board was flashed or commissioned during development. Still required:
assembly continuity/rail measurements; actual PWM and isolation across either-rail
power sequencing, reset, and brownout; both physical stock presets; USB bootloader
recovery; Apple Home endpoint presentation, pairing and retained fabrics; AP-loss
and protocol-stall fault injection; closed-housing RF performance; and the required
seven-day soak. Record those results in [the validation record](validation-record.md)
against [the specification](spec.md). Keep software verification and physical
acceptance as separate evidence.

The dated [software verification record](software-validation.md) records build size,
ELF hash, offline-emulation coverage and review outcomes.
