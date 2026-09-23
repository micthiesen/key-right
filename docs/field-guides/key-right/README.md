# Key Right field guide

[Print-ready PDF](key-right-field-guide.pdf): eight portrait US Letter pages,
ordinary reading order. Print one copy, duplex on the long edge. No dimensional
cutting template is included, so this does not depend on calibrated print scaling.

## Rebuild

From the repository root, using Python 3 and uv:

```sh
uv run --with-requirements docs/field-guides/key-right/requirements.txt python docs/field-guides/key-right/build.py
pdftoppm -scale-to 1400 -png docs/field-guides/key-right/key-right-field-guide.pdf /tmp/keyright-guide-page
```

Alternatively install `requirements.txt` into a virtual environment and run
`build.py`. The builder uses deterministic PDF metadata and rejects text/table
overflow. It checks all eight pages have Letter media boxes. Rendered-page visual
inspection is still required after changes.

## Sources and authority

| Content | Source |
| --- | --- |
| Actual board photograph | User-provided `docs/references/photos/keylight-3623.png`, preserved without invented pin annotations |
| U3 top-view pin numbering | [NXP PCA9635 datasheet](https://www.nxp.com/docs/en/data-sheet/PCA9635.pdf), also in `docs/references/datasheets` |
| TXU top-view pin numbering and rail behavior | [TI TXU0102 datasheet](https://www.ti.com/lit/ds/symlink/txu0102.pdf) |
| XIAO pin names, USB/5 V, antenna selection | [Seeed XIAO ESP32-C6 documentation](https://wiki.seeedstudio.com/xiao_esp32c6_getting_started/) and [schematic](https://files.seeedstudio.com/wiki/SeeedStudio-XIAO-ESP32C6/XIAO-ESP32-C6_v1.0_SCH_PDF_24028.pdf) |
| Exact parts and source links | [Hardware plan](../../hardware.md) |
| Warm/cool counts at stock nominal 3% | [Hash-gated signed-stock-firmware analysis](../../references/firmware-analysis.md), not an optical measurement |
| MCU pin assignments and readbacks | `firmware/app/src/hardware.rs`; pinned HAL register/API definitions |
| Commands and persistence semantics | `firmware/app/src/console.rs`, `runtime.rs`; [development reference](../../development.md) |
| Apple Home manual-code flow | [Apple add-accessory instructions](https://support.apple.com/en-ca/102135) |
| Generic probe technique image | `assets/probing-technique.png`; generated with ImageGen, full prompt/provenance in `imagegen-prompts.json` |

The two chip drawings are manually constructed vector pin maps from the respective
manufacturer top-view diagrams. They are not generated depictions of the board.
The source photo is resampled to 1600 pixels for its four-inch PDF placement and
JPEG-compressed; its contents and aspect ratio are preserved. The photo cannot
establish the PCA pin-1 mark; the guide requires finding it on
the actual package. Do not replace that condition with a guessed photo overlay.

## Review and limits

All eight rendered pages were inspected for clipping, wrapping, readable text,
pin-number alignment and monochrome usability. The narrow BOM quantity cells were
corrected after the first render. The PDF is intended for the original full-size
board 53 rev 1 only, subject to the voltage/isolation checks it spells out.

Firmware checks and offline emulation have run. Physical assembly, pad voltages,
PWM waveforms, optical output, commissioning, fault recovery and the seven-day
soak have not. Record them using [validation-record.md](../../validation-record.md).
A failed branch in the guide calls for investigation; it does not authorize
bypassing isolation or inventing a replacement pinout.

The print receipt records the reviewed PDF hash, delivery revision, requested
settings and observed printer state. Printer-reported completion is not physical
inspection of the paper.
