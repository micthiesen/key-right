# Physical validation record

This record covers Michael's original full-size Elgato Key Light and its wired
**ESP32-C3_MINI_V1** replacement controller. Keep pairing codes and Wi-Fi
credentials out of this file. Distinguish reported measurements, firmware
evidence, controller acknowledgements, and observed light output.

**Current wiring correction:** after the bus voltage checks, Michael identified
the error at the Key Light's U4 pads and confirmed the ESP end is correct. On
U4's top row, OE is pad 5 counting from 1 at the left, SCL is rightmost, and SDA
is second from right. ESP GPIO4/SDA, GPIO5/SCL, and GPIO6/OE remain unchanged.
Michael reports that the first board's rework is complete, the specific signal
paths and absence of shorts are confirmed, and it is connected and powered.
The second board is still being reworked. The Mac currently sees no USB device
or serial port for the first board; post-rework PCA testing awaits reconnection.
The earlier results below precede rework unless explicitly marked otherwise.

## Reported baseline, 2026-09-28

The measurements below were recorded in the
[handoff at commit `a617f1e`](https://github.com/micthiesen/key-right/blob/a617f1e77529d3fb66339c6e1a4e7b3636a03fcf/docs/handoff).
Its sources were Michael's completed probing worksheet and
`Elgato_Key_Light_C3_Wiring_Field_Guide.pdf`, which are not present in this
repository. The precise measurement dates and instrument details were not
supplied. These are user-reported observations, not agent measurements or a
first-power pass for the completed assembly.

| Item | Reported observation |
| --- | --- |
| Replacement board | Blue `ESP32-C3_MINI_V1`, USB-C, BOOT/RESET, bare ESP32-C3 package |
| DC input | 13 V |
| PCA9635 VDD | 3.37 V |
| Idle SDA / SCL / OE | 3.37 V each |
| SDA / SCL / OE to VDD | Approximately 9.9 kΩ each |
| SDA to SCL | Approximately 20 kΩ |
| VDD to GND, unpowered | Approximately 1 kΩ |
| IC pin to selected connection pad | Approximately 0–0.1 Ω |
| Rocker | Already bypassed into working ON state |

The reported readings support approximately 10 kΩ existing pull-ups and direct
use of the stock regulated rail. They do not establish rail stability under
radio load or the PCA's physical output behavior.

Michael separately reported on **2026-09-28** that wiring was finished and the
assembly was ready for bench-PSU testing and flashing. **The LED panels are not
connected.** The initially reported signal map was later found incorrect;
[hardware.md](hardware.md) tracks its replacement. Reported power is J6/DEBUG
3.37 V to `3.3`, with J8/UART ground to `G`. The installed design has no
buck converter or added pull-ups. At that point, powered operation, flashing,
USB, I²C, and optical tests of the completed assembly were still pending.

## Bench session in progress, 2026-09-28, LEDs disconnected

Follow [bench bring-up](bench-bring-up.md) and record the software gate results
before flashing the C3. With lamp wires attached, USB must block
VBUS/5 V while retaining data and ground. Ordinary powered USB requires
disconnecting all five lamp wires first, even when lamp power is off.

Record actual values and outcomes in the following table during the bench
session. `Pending` is not a pass. The current limit has not been specified and
must be recorded as the chosen bench setting, not inferred from the adapter's
4 A rating.

| Check or session detail | Result |
| --- | --- |
| Date, operator, stock-board revision | 2026-09-28; Michael operating the bench, agent inspecting macOS/serial; stock-board revision not supplied |
| U4 signal-wiring correction | Michael reports OE = top-row pad 5 from left (1-based), SCL = rightmost, SDA = second from right; ESP ends correct. First board rewired, signal-pin continuity and shorts checked by Michael, connected and powered. Second board rework pending; no post-rework PCA result yet |
| USB after first-board rework | No `/dev/cu.usbmodem*` device, and no attached ESP in `ioreg -p IOUSB` or `system_profiler SPUSBHostDataType`. RESET alone and then holding BOOT/tapping RESET/releasing BOOT did not expose USB; Michael reports the red power LED lit. Detection remained absent after requesting cable reseat, another Mac port, and reversed ESP plug orientation. No post-rework control command reached the board. Michael reports power is good; no new numerical rail reading or PSU CV/CC indication supplied. Added `scripts/usb-watch.py` for read-only USB-C/accessory, USB registry, serial-node, and macOS log capture while troubleshooting |
| C3 chip identity and detected flash ID/capacity | ESP32-C3 revision v0.4, 40 MHz crystal, 4 MiB flash detected by espflash; raw flash ID was not printed |
| C3 firmware commit, build target, image/partition fit | Current real `hardware-light` image is `ec1cff9`, `riscv32imc-unknown-none-elf`; 1,914,272 / 4,063,232 app-partition bytes, linked stack 57,912 bytes; both software gates and both image-fit checks passed. Initially flashed `fb241df` before the arena fix |
| Read-only serial inspection and flash-helper preflight | Passed `sh scripts/flash.sh --info /dev/cu.usbmodem1101`; no flash write. macOS identified Espressif USB VID `0x303a`, PID `0x1001`, 12 Mb/s. Holding BOOT, tapping RESET, then releasing BOOT exposed USB and produced the accessory prompt |
| Bench-PSU connection/polarity, voltage setting, current limit | User reports 13 V supply enabled; current limit/current draw and CV/CC indication not supplied |
| Power/USB isolation arrangement used | User opened a USB-C cable and disconnected its larger red conductor. With USB alone, no ESP power indication/enumeration was reported. USB data works with bench power. VBUS isolation has not been independently measured |
| First power-up, current draw, 3.3 V rail under load | User reports ESP LEDs lit, red plus flashing blue, with 13 V bench power; measured 3.345 V between ESP `3.3` and `G` during the powered USB session. Current draw and radio-load measurements pending; this is not a rail-stability pass |
| Original firmware backup | Full 4,194,304-byte read completed and digest verified by espflash before writing. SHA-256 `9a4b8a001b605d24bac52dfb157656d1f16ef8a06726842cd546046466378daa`; private local copy under `~/Library/Application Support/key-right/backups/2026-09-28-c3-first-flash/`, outside Git |
| Flashing, native USB console, boot/reset reasons | Initial flash succeeded; first console request timed out. After manual RESET, captured `Out of bump memory` panic and `TG0WDT_SYS_RST` loop. Enlarged Matter's static arena from 20,000 bytes to 32 KiB, reflashed with NVS preserved; USB `status` then succeeded at 9.6, 20.5, 63.8, and 107.974 seconds uptime, reset `CoreUsbUart`. No storage failures reported |
| GPIO4/5 assignment and 100 kHz bus operation | Configured in real firmware; before rework, PCA bus reported `AcknowledgeCheckFailed(Unknown)`. Michael confirms corrected signal-pin continuity; post-rework I²C operation and timing still unmeasured |
| Powered bus DC levels at the ESP | Before rework, Michael reported GPIO4 3.4 V and GPIO5 3.34 V relative to ESP G. Neither appeared held low. The HAL enables internal pull-ups, so these readings did not establish end-to-end continuity or I²C timing. Michael subsequently confirmed corrected signal-pin continuity during rework |
| GPIO6 open-drain release HIGH before output enable; LOW enables configured outputs | Pending |
| PCA individual address `0x15`, setup writes and critical-register readback | Failed: `off` and `registers` returned `KR ERR Failure`; `verify` returned `KR ERR InvalidState`. Logs identify I²C acknowledgement failure. Status retains intended Off, level 57, 303 mired, `acknowledged=None`, `fault=Output`; no successful register readback |
| BLE/Wi-Fi activity, loaded rail, brownout/reset behavior | Pending |

## LED-connected acceptance pending

Michael reports the LED panels cannot be connected until the lamp is put back
together. Keep them disconnected for bench work; the four two-pin LED connectors
are available for probing. Record unloaded connector measurements separately
from physical light results. Reconnect during reassembly with power removed,
only after the preceding bench work supports proceeding. Record the firmware
commit and actual observations for each check. Tests with disconnected LEDs
cannot fill these rows.

| Check | Observation |
| --- | --- |
| Firmware commit and LED reconnection date | Pending |
| Off after application initialization | Pending |
| Home low/high brightness, stock nominal 1%/10% limits | Pending |
| Temperature minimum/maximum, 143/344 mired | Pending |
| Initial level 57 and 303 mired, warm/cool raw PCA `6/2` | Pending |
| Level 57 and 200 mired, warm/cool raw PCA `3/6` | Pending |
| Intermediate brightness/temperature response and low-level quantization | Pending |
| LED0 warm / LED4 cool physical behavior | Pending |
| OE HIGH with `MODE2=0x14` and the actual external driver stage | Pending |
| Cold power-up, MCU reset, and any startup flash | Pending |
| Power-cycle persistence and intended/applied/output state agreement | Pending |
| Installed antenna arrangement and closed-housing BLE/Apple Home pairing | Pending |
| One Home light tile with power, brightness, and temperature per lamp | Pending |
| Two separately paired lamps grouped in Home, shared brightness/temperature control | Pending; no simultaneous-output guarantee |
| One grouped lamp offline while the other remains controllable | Pending |
| Short Wi-Fi/AP interruption and automatic recovery preserving intent | Pending |
| USB removed, ordinary lamp-powered operation | Pending |

## Interpretation

[Stock-firmware emulation](references/firmware-analysis.md) establishes address,
channel, configuration, and reference command evidence. It does not establish
measured brightness, color temperature, flicker, LED current, temperature,
startup behavior, or radio/network reliability on this assembly. There are no
connected voltage, current, or optical sensors; fixed values and register
readback must not be reported as live physical telemetry.

Keep observations direct, including unexpected light, bus failures, reset loops,
and radio failures. Record skipped checks and unresolved contradictions. Do not
claim safe startup from a successful software build, an OE level, or a PCA
acknowledgement. Do not claim long-term reliability from a single bench session.
