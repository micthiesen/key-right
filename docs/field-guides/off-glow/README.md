# Off-glow resistor trial field guide

[Printable one-page guide](key-right-resistor-trial.pdf), designed for Letter
paper and monochrome printing. This is a proposed warm-bank bleeder trial,
not a proven repair. It includes a brief reminder to flash prepared firmware
0.1.4, not a full flashing procedure. No lamp was modified or flashed while
creating this guide.

## Sources and accuracy

Hardware decisions are from commit
`5fdaf9d514de31273bc69084dbeaa24eccb1f7ae`:

- [Off-glow investigation](../../off-glow-investigation.md): two 10 kΩ,
  at least ¼ W resistors; one across F-1/J1 and one across F-2/J3 on one lamp.
- [Hardware](../../hardware.md) and [bench procedure](../../bench-bring-up.md):
  lamp/bench power with VBUS-blocked USB, retain panel wiring and Home storage.
- [Original photograph](../../references/photos/keylight-3622.png): component
  side with white power resistors at top and DC input wires at bottom.
  The old controller is visible; output connector locations are unchanged.
  PDF crops and vector rings retain original image geometry. The full-resolution
  photo is JPEG-encoded at quality 94 for the PDF, without resizing or retouching.
  The small rings
  mark the metal electrical contacts. The underside is not photographed, so
  the guide does not claim to show underside solder pads.
- `assets/axial-resistor.png`: generated component-shape illustration, not a
  resistor colour-code, circuit diagram or source for PCB locations. Prompt
  and provenance are retained in `imagegen-prompts.json`.

## Rebuild

Python 3 with `reportlab==5.0.1` and `pillow==12.3.0`:

```sh
python -m venv /tmp/key-right-guide-venv
/tmp/key-right-guide-venv/bin/pip install reportlab==5.0.1 pillow==12.3.0
/tmp/key-right-guide-venv/bin/python docs/field-guides/off-glow/build.py
pdftoppm -r 150 -gray -png -singlefile \
  docs/field-guides/off-glow/key-right-resistor-trial.pdf /tmp/key-right-guide
```

The builder uses macOS's Arial and Arial Bold from
`/System/Library/Fonts/Supplemental`. Set `GUIDE_FONT_DIR` to a directory with
`Arial.ttf` and `Arial Bold.ttf` on another system. Embedded fonts and PDF
vector text keep labels sharp. Original photograph colour is retained as
source evidence; review the grayscale render for monochrome print contrast.

Inspect the whole page and enlarged contact details after rebuilding. Keep
scratch renders outside the repository. `print-receipt.json` records the
reviewed PDF hash and actual printer result; a rebuild requires a new review
and makes any old receipt applicable only to its recorded hash.
