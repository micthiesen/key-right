# Findings and references

Recorded 2026-09-22; firmware research added 2026-09-23; installed hardware and
user measurements reconciled 2026-09-28. Keep user observations, software evidence,
and untested physical behavior distinct.

**Current design:** the installed ESP32-C3_MINI_V1 replaces only the Realtek module
and drives the retained PCA9635 over I²C; see [hardware.md](hardware.md). Wiring is
complete and LEDs are disconnected. User measurements establish the connection
points, bus levels, and existing pull-ups. Powered ESP/PCA operation and light
output remain untested. [Signed firmware emulation](references/firmware-analysis.md)
establishes the selected address/configuration, channel order, and nominal 3%
reference commands at 3300 K and 5000 K. The current design exposes one Matter
Color Temperature Light per lamp with stock nominal 1–10% brightness and
143–344 mired temperature. Two physical lamps are grouped in Home; see
[the spec](spec.md) for the control contract.

## Actual light and photos

The user owns the original full-size Elgato Key Light, not the Mini. A live read of the left light's accessory-info endpoint reported:

- Product: Elgato Key Light; board type 53; hardware revision 1.
- Firmware: 1.0.3 build 222.
- MAC: 3C:6A:9D:16:BB:B2; display name Key Light Left.
- Reachable at `http://key-light-left.local.:9123`; RSSI -51 dBm on `SyNet-2G`.
- On at brightness 3; API temperature value 303 at the time of the initial check.

Saved historical IPs 10.10.1.97 (left) and 10.10.1.98 (right) did not respond during this session. The right light did not advertise in the short discovery window. This does not establish its current address or firmware build. Saved right MAC: 3C:6A:9D:16:BB:B1.

The user's supplied photographs independently confirm:

- Dexatek DK-9169 V1.0 module, RTL8711AM processor.
- U3 marked PCA9635PW, separate from the wireless module.
- J6 DEBUG: unpopulated 2x5 through-hole footprint.
- J8 UART and J7 FW DOWNLOAD: unpopulated pad areas.
- Four two-pin LED connections, disconnected by the user.
- Existing power resistors and output circuitry remain in place.

The initial photos did not establish a full J6 pinout. The later user worksheet
identifies J6's top-left pad as the measured 3.37 V supply and the selected J8 pad
as ground, in the orientation defined in [hardware.md](hardware.md). This does
not establish a standard debug-cable pinout or J7's function.

## Retained driver

NXP documents the PCA9635PW as a 16-channel I2C PWM LED driver with individual 8-bit duty control at 97 kHz. It can control external drivers. Do not equate its low-current outputs with the panel's LED power connections.

Chip-level reference only, not a finalized board wiring instruction:

- SDA: pin 27; SCL: pin 26.
- Ground/VSS: pin 14; VDD: pin 28.
- Active-low output enable: pin 23.

The September 28 handoff reports 3.37 V idle levels with approximately 9.9 kΩ
signal-to-VDD resistance, supporting retention of the existing pull-ups. Its
selected SDA/SCL/OE pad map was later corrected by Michael's bench inspection.
[hardware.md](hardware.md) records his corrected U4 positions; post-rework
end-to-end checks remain pending. Firmware
emulation establishes LED0/warm and LED4/cool, address `0x15`, stock I²C setup,
and `MODE2=0x14`; physical readback and LED behavior are still pending.

[NXP datasheet](https://www.nxp.com/docs/en/data-sheet/PCA9635.pdf), also saved under references/datasheets.

## Installed C3 hardware evidence

The [September 28 handoff in Git history](https://github.com/micthiesen/key-right/blob/a617f1e77529d3fb66339c6e1a4e7b3636a03fcf/docs/handoff)
transcribes the user's probing worksheet and final C3 wiring guide. It identifies
the board as `ESP32-C3_MINI_V1`, maps GPIO4/5/6 to SDA/SCL/OE, and records direct
power from J6's 3.37 V rail. The worksheet and named C3 PDF are not stored here;
the historical readings are retained in [validation-record.md](validation-record.md).
Michael later corrected the guide's U4 pad positions and confirmed the ESP end
was correct; [hardware.md](hardware.md) owns the revised map. LEDs remain
disconnected during rewiring and bench work.

The [ESP32-C3 datasheet](https://www.espressif.com/sites/default/files/documentation/esp32-c3_datasheet_en.pdf)
documents GPIO multiplexing, including the alternate pad-JTAG functions on
GPIO4/5/6. The [native USB Serial/JTAG guide](https://docs.espressif.com/projects/esp-idf/en/stable/esp32c3/api-guides/usb-serial-jtag-console.html)
documents the fixed USB peripheral. Neither identifies this third-party board's
antenna routing or flash capacity. Do not infer an antenna GPIO or MINI-1U module
identity. Preserve USB GPIO18/19 and use open-drain GPIO6. The September 28
[component-side board photo](references/photos/esp32-c3-mini-v1.png) shows the
USB-C connector, buttons, onboard antenna component, and antenna socket. It
does not prove RF routing or electrical function. Use the wiring correction
status and USB power rule in [hardware.md](hardware.md).

## Original-controller research

The [EEVblog 1453 teardown discussion](https://www.eevblog.com/forum/blog/eevblog-1453-elgato-key-light-teardown/) identifies the DK-9169 and discusses the PCA9635 and output transistors. [Teardown video](https://www.youtube.com/watch?v=kcWwAweWjQg).

The [Dexatek module datasheet](https://www.dexatek.com/_files/ugd/c97cac_817c1987ed1b49bea26071756f2888f7.pdf) documents JTAG signals on module pins 27-31, log UART on 32/33, and CHIP_EN on 24. These are module numbers, not J6 numbers. [Realtek development documentation](https://fcc.report/FCC-ID/TX2-RTL8711AM/2714033.pdf) describes J-Link/CMSIS-DAP support. Debug accessibility and lock state on the user's light were not tested.

Read-only inspection of the installed vendor image:

```text
/Applications/Elgato Control Center.app/Contents/Resources/Firmware_Key_Light.bin
Size: 698901 bytes
SHA-256: 57f48194dbec6ee989db6536a6cc0008cde84180301f73d1e7cd4935e3eb6c83
Header: board 53, version 1.0.3, build 222
Payload container: RTKWin
Ed25519 signature: cryptographically verified valid
```

The image contains Ameba/RTL8195A driver strings, `/elgato/firmware-update/prepare`, `/elgato/firmware-update/data`, `/elgato/firmware-update/execute`, `/elgato/uart`, and `Firmware signature invalid`. Strings establish the presence of code/data, not proof that an endpoint permits a particular live operation. No firmware writes or bypass attempts were performed. The proprietary binary is not copied into this folder.

The [latest listed original Key Light firmware](https://help.elgato.com/hc/en-us/articles/30938624511373-Elgato-Key-Light-Firmware-Release-History) is build 222. [Control Center 1.7.1 notes, 2024-08-20](https://help.elgato.com/hc/en-us/articles/29959336411277-Elgato-Control-Center-1-7-1-Release-Notes-macOS) describe a fix for crashes causing lights to become undiscoverable. The left light already has that build. No claim of official end-of-support was verified.

## Related projects

- [Key Light Mini firmware research](https://github.com/schlarpc/elgato-key-light-mini-firmware-re): firmware-container analysis, network update tooling, and demonstrated modified stock firmware on Mini board 202/build 240. Its container parser also describes older RTKWin images. Its bypass was NOT verified on original board 53. It is neither a replacement firmware nor a demonstrated Wi-Fi fix for this light.
- [Author's account](https://schlarp.com/posts/everything-i-own-owned/).
- [Realtek Ameba1 SDK](https://github.com/Ameba-AIoT/ameba-rtos-1): historical controller reference, not the selected target.
- [Espressif Rust Wi-Fi documentation](https://docs.espressif.com/projects/rust/esp-wifi/0.15.0/esp32/esp_wifi/index.html): historical Rust-support reference; [development.md](development.md) records the pinned C3 stack.
- [ESP-IDF Wi-Fi documentation](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-guides/wifi-driver/index.html).
- [Embedded Rust/C interoperability](https://doc.rust-lang.org/embedded-book/interoperability/c-with-rust.html).

## Existing Homebridge work

The user maintains [@micthiesen/homebridge-elgato-key-lights](https://github.com/micthiesen/homebridge-elgato-key-lights). Prior saved notes report version 1.1.0 with independent device supervisors, retry/backoff, HTTP deadlines, unavailable-state handling, and stable accessory identity. Publication and simulated recovery tests were recorded previously; installation of that fork on Boris and real HomeKit operation were not verified in this session.

This is historical integration context; the selected replacement uses Matter directly. It cannot repair a hung radio in the old light and is not evidence that the replacement's recovery requirements already pass.

## Selected Matter implementation

Selected 2026-09-23 and retained in the September 28 reconciliation: reuse
`../stillair`'s Rust Matter connectivity.
This replaces the earlier direct-HAP/Homebridge evaluation path. The source snapshot
is Stillair commit `af12fec55430b4af7704dd89636bdd102a0c4158`, particularly
`firmware/app/src/matter.rs`, `firmware/app/src/output.rs`, its Cargo manifest,
lockfile, and RISC-V target configuration.

The reused `rs-matter-embassy` stack supplies concurrent BLE commissioning and
Wi-Fi, hardware-seeded randomness, and an NVS partition discovered from the
flash partition table. Keep compatible dependency revisions together. Stillair's
fan handler and motor GPIO configuration do not apply to Key Right's C3 board.
See [development.md](development.md) for build details and remaining validation.
The handoff's C++/ESP-Matter rewrite, 30% brightness default, generic calibration
system, and changed restart/fault policies were not adopted.
