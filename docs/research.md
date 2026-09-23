# Findings and references

Recorded 2026-09-22; autonomous firmware/hardware research added 2026-09-23. Separate observed hardware facts from proposals and unverified wiring.

**Current design:** remove only the Realtek module and drive the retained PCA9635 over I²C; see [hardware.md](hardware.md). An earlier TXU0102/direct-PWM proposal that lifted PCA pins 6/10 is abandoned and is not assembly guidance. [Signed firmware emulation](references/firmware-analysis.md) resolves address/configuration, channel order and nominal 3% PWM. The user selected 3300 K and 5000 K; actual bus access/levels and physical output remain unverified.

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
- J8 UART and J7 FW DOWNLOAD: unpopulated 1x3 footprints.
- Four two-pin LED connections, disconnected by the user.
- Existing power resistors and output circuitry remain in place.

The J6 traces run toward the module's documented debug-pin edge. No complete J6 mapping was established. Square pads provide orientation clues, not proof of a standard cable pinout. J7's exact function is unverified. These interfaces are now background reference; the decided approach replaces the module.

## Retained driver

NXP documents the PCA9635PW as a 16-channel I2C PWM LED driver with individual 8-bit duty control at 97 kHz. It can control external drivers. Do not equate its low-current outputs with the panel's LED power connections.

Chip-level reference only, not a finalized board wiring instruction:

- SDA: pin 27; SCL: pin 26.
- Ground/VSS: pin 14; VDD: pin 28.
- Active-low output enable: pin 23.

The board's convenient connection pads, bus voltage, pull-ups, and OE route still require field verification. Firmware emulation establishes LED0/warm and LED4/cool, address `0x15`, stock I²C setup, and `MODE2=0x14`. Pull-ups and OE tracing matter to the selected I²C design. The old module may carry supporting bus components, so do not presume they remain after removal. The user selected an external buck converter for ESP power.

[NXP datasheet](https://www.nxp.com/docs/en/data-sheet/PCA9635.pdf), also saved under references/datasheets.

## XIAO power and GPIO sources

The selected XIAO pin map is documented by [Seeed](https://wiki.seeedstudio.com/xiao_esp32c6_getting_started/): D10/GPIO18, D9/GPIO20, D3/GPIO21, 5V and GND. Seeed's [official schematic](https://files.seeedstudio.com/wiki/SeeedStudio-XIAO-ESP32C6/XIAO-ESP32-C6_v1.0_SCH_PDF_24028.pdf) shows the 5V header on the USB VBUS net. This build avoids connecting the two sources together: flash/request the pairing code before buck wiring, and disconnect the buck's 5 V lead before later USB service. No diode or source-select jumper is in the baseline. This is the selected exclusive-source wiring, not a general recommendation for projects that leave USB and external 5 V connected together.

If I²C does not communicate, first establish whether pull-ups remain after module removal and whether the bus levels suit XIAO GPIO. Additional pull-ups or level translation are conditional troubleshooting only; no value or part is selected without measurements on this board.

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
- [Espressif Rust Wi-Fi documentation](https://docs.espressif.com/projects/rust/esp-wifi/0.15.0/esp32/esp_wifi/index.html): evidence of Rust support; select current compatible versions for the eventual board.
- [ESP-IDF Wi-Fi documentation](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-guides/wifi-driver/index.html).
- [Embedded Rust/C interoperability](https://doc.rust-lang.org/embedded-book/interoperability/c-with-rust.html).

## Existing Homebridge work

The user maintains [@micthiesen/homebridge-elgato-key-lights](https://github.com/micthiesen/homebridge-elgato-key-lights). Prior saved notes report version 1.1.0 with independent device supervisors, retry/backoff, HTTP deadlines, unavailable-state handling, and stable accessory identity. Publication and simulated recovery tests were recorded previously; installation of that fork on Boris and real HomeKit operation were not verified in this session.

This is historical integration context; the selected replacement uses Matter directly. It cannot repair a hung radio in the old light and is not evidence that the replacement's recovery requirements already pass.

## Selected Matter implementation

Updated 2026-09-23: the user selected reuse of `../stillair`'s Matter connectivity.
This replaces the earlier direct-HAP/Homebridge evaluation path. The source snapshot
is Stillair commit `af12fec55430b4af7704dd89636bdd102a0c4158`, particularly
`firmware/app/src/matter.rs`, `firmware/app/src/output.rs`, its Cargo manifest,
lockfile, and RISC-V target configuration.

Stillair uses ESP32-C6, `rs-matter-embassy`, concurrent BLE commissioning and Wi-Fi,
hardware-seeded randomness, and an NVS partition discovered from the flash partition
table. Keep its dependency revisions together when adapting the stack. Its fan
handler and motor GPIO configuration do not apply to Key Right. See
[development.md](development.md) for the port's scope and remaining validation.
