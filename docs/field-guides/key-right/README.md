# Historical Key Right field guide

**Superseded on 2026-09-28. Do not use this guide to wire, power, or flash the
installed ESP32-C3_MINI_V1.** It describes the old XIAO C6/buck plan and its USB
instructions do not apply to the direct-3.37 V assembly. Use the current
[hardware map](../../hardware.md) and [development instructions](../../development.md).

[Open the archived two-page guide](key-right-field-guide.pdf).

Revision 2 replaced the eight-page direct-PWM guide. It keeps the stock PCA9635
and uses an ESP32-C6, a buck converter and wiring. There are twelve steps, a large
board photo, pin maps to circle, spaces for measurements and a wiring diagram.

## Reproduce the historical guide

From the repository root:

```sh
uv run --with-requirements docs/field-guides/key-right/requirements.txt python docs/field-guides/key-right/build.py
```

The builder uses ReportLab. Original photos and datasheets stay in
`docs/references/`; the PDF clips the photo without changing its geometry. The
pin maps are diagrams, not physical templates.

## Sources

| Detail | Source |
| --- | --- |
| Board photograph | [Original PNG](../../references/photos/keylight-3623.png), from the user's IMG_3623.HEIC |
| Module pad numbers | [Dexatek DK9169 datasheet](../../references/datasheets/Dexatek-DK9169.pdf), pages 3-4; drawing rotated so the antenna is at the left, matching the photo |
| U3 pin numbers | [NXP PCA9635 datasheet](../../references/datasheets/NXP-PCA9635.pdf), TSSOP28 top view |
| SDA/SCL pad candidates and presets | [Original firmware analysis](../../references/firmware-analysis.md); module 10/9 are candidates until checked; module 14 is only an OE candidate |
| XIAO pad names and power net | [Seeed pin guide](https://wiki.seeedstudio.com/xiao_esp32c6_getting_started/) and [schematic](https://files.seeedstudio.com/wiki/SeeedStudio-XIAO-ESP32C6/XIAO-ESP32-C6_v1.0_SCH_PDF_24028.pdf) |
| Buck converter | [Pololu D24V25F5](https://www.pololu.com/product/2850/specs) |

The U3 drawing is a top view. Its orientation must be matched to the real pin-1
mark; the photo is not clear enough to assign that mark. Nothing in the guide
claims that a pictured connection has been measured.

## Copy and review

The guide's copy received an `unslop` pass: short actions, exact pins and
commands, and one compact instruction for unresolved connections or voltage.
Extra-component investigations stay in [hardware.md](../../hardware.md).
Both PDF pages were rendered and checked before printing. The source, PDF, and
receipts remain as historical reference. They are not the later C3 wiring guide
cited in the September 28 handoff; that PDF is not stored in this repository.
Current physical results belong in [the validation record](../../validation-record.md).

## Printing

Revision 2 was printed as two single-sided Letter sheets so the pin notes and
wiring can sit beside one another. [The receipt](print-receipt.json) records
Executor's completed response for job **233**, with two impressions. Printer
completion is not inspection of physical paper.

[Revision 1 receipt](print-receipt-rev1.json) records the previous eight-page
copy, job 232. Its PDF is preserved at the commit URL in that receipt. Discard
that printed guide; its added output circuitry is no longer the project plan.
