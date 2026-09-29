#!/usr/bin/env python3
"""Compose a Letter field guide from an unchanged board photo and vector labels.

Requirements: reportlab==5.0.1, pillow==12.3.0.
Source decisions: 5fdaf9d514de31273bc69084dbeaa24eccb1f7ae,
docs/off-glow-investigation.md and docs/hardware.md.
Photo coordinates use the 1824 x 1368 preview frame, with origin at top left.
The PDF retains the original photo; clipping and annotations are PDF objects.
"""

from pathlib import Path
from io import BytesIO
import os

from PIL import Image
from reportlab.lib import colors
from reportlab.lib.utils import ImageReader
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.pdfgen import canvas


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
PHOTO = REPO / "docs/references/photos/keylight-3622.png"
OUTPUT = HERE / "key-right-resistor-trial.pdf"
FONT_DIR = Path(os.environ.get("GUIDE_FONT_DIR", "/System/Library/Fonts/Supplemental"))


def build():
    pdfmetrics.registerFont(TTFont("Guide", str(FONT_DIR / "Arial.ttf")))
    pdfmetrics.registerFont(TTFont("GuideBold", str(FONT_DIR / "Arial Bold.ttf")))
    c = canvas.Canvas(str(OUTPUT), pagesize=(612, 792), invariant=1)
    c.setTitle("Key Right: off-glow resistor trial")
    c.setAuthor("Key Right")
    c.setSubject("F-1/J1 and F-2/J3 connector references; 10 kΩ bleeder trial")
    # Encode at full source resolution for a manageable print file. This changes
    # compression only: no resampling, rotation, retouching or generated geometry.
    photo_bytes = BytesIO()
    with Image.open(PHOTO) as source:
        source.convert("RGB").save(photo_bytes, "JPEG", quality=94, optimize=True)
    photo_bytes.seek(0)
    photo = ImageReader(photo_bytes)

    def text(x, y, value, size=11, bold=False, colour=colors.black):
        c.setFillColor(colour)
        c.setFont("GuideBold" if bold else "Guide", size)
        c.drawString(x, y, value)

    def crop(box, x, y, width, height):
        """Place a top-origin source crop without altering its aspect ratio."""
        sx, sy, sw, sh = box
        scale = min(width / sw, height / sh)
        drawn_w, drawn_h = sw * scale, sh * scale
        x += (width - drawn_w) / 2
        y += (height - drawn_h) / 2
        c.saveState()
        clip = c.beginPath()
        clip.rect(x, y, drawn_w, drawn_h)
        c.clipPath(clip, stroke=0)
        c.drawImage(photo, x - sx * scale,
                    y + drawn_h - (1368 - sy) * scale,
                    width=1824 * scale, height=1368 * scale)
        c.restoreState()

        def point(px, py):
            return x + (px - sx) * scale, y + drawn_h - (py - sy) * scale

        return point, scale

    def ring(x, y, radius, width=1.8):
        # White under-stroke makes the black ring readable on the photograph.
        c.setStrokeColor(colors.white)
        c.setLineWidth(width + 3)
        c.circle(x, y, radius, stroke=1, fill=0)
        c.setStrokeColor(colors.black)
        c.setLineWidth(width)
        c.circle(x, y, radius, stroke=1, fill=0)

    def badge(x, y, label):
        c.setFillColor(colors.black)
        c.setStrokeColor(colors.white)
        c.setLineWidth(1.2)
        c.circle(x, y, 10.5, fill=1, stroke=1)
        c.setFont("GuideBold", 12)
        c.setFillColor(colors.white)
        c.drawCentredString(x, y - 4, label)

    # Heading and component identification. The generated resistor is illustrative
    # only; its bands do not specify a value or any PCB connection.
    text(34, 770, "KEY RIGHT  /  FIELD GUIDE", 9.5, True)
    text(34, 744, "Off-glow resistor trial", 23, True)
    text(34, 724, "2 × 10 kΩ axial resistors · ¼ W minimum · one lamp first", 11.5)
    c.drawImage(str(HERE / "assets/axial-resistor.png"), 448, 739,
                width=130, height=44.82, preserveAspectRatio=True, mask="auto")

    c.setFillColor(colors.black)
    c.roundRect(34, 682, 544, 28, 3, stroke=0, fill=1)
    text(45, 691, "UNPLUG LAMP POWER + USB BEFORE SOLDERING", 12, True,
         colors.white)

    text(34, 665, "COMPONENT SIDE", 9.5, True)
    text(169, 665, "White power resistors at top", 9.5)
    locate, scale = crop((528, 300, 872, 938), 34, 303, 328, 353)
    for label, px, py in (("A", 1329, 486), ("B", 1328, 595)):
        cx, cy = locate(px, py)
        ring(cx, cy, 54 * scale)
        badge(cx - 27, cy + 12, label)

    # Separate details retain connector reference lettering and neighboring PCB.
    # Rings identify the electrical metal contacts, not unphotographed solder pads.
    text(382, 665, "A   F-1 / J1", 13, True)
    detail_a, detail_scale_a = crop((1255, 425, 145, 123), 382, 506, 196, 153)
    for px, py in ((1329, 471), (1329, 496)):
        ring(*detail_a(px, py), 10.3 * detail_scale_a, width=1.5)
    text(382, 494, "Upper right connector", 10.5)

    text(382, 469, "B   F-2 / J3", 13, True)
    detail_b, detail_scale_b = crop((1255, 539, 145, 123), 382, 310, 196, 153)
    for px, py in ((1329, 579), (1329, 606)):
        ring(*detail_b(px, py), 10.3 * detail_scale_b, width=1.5)
    text(382, 298, "Lower right, above capacitor", 10.5)
    text(34, 291, "Original board photo; DC input wires at bottom.", 9.5)

    text(34, 263, "ONE RESISTOR ACROSS EACH CONNECTOR", 12, True)
    for y, line in ((245, "One lead to each circled contact's solder pad. Either direction."),
                    (229, "The underside pads are not shown; follow the two connector pins."),
                    (213, "Keep panel wires connected normally. Insulate exposed resistor leads."),
                    (197, "Leave W-1/W-2 unchanged. Do not connect to GND, OE or the ESP.")):
        text(34, y, line, 11.2)

    # Small, explicit flash note rather than duplicating the full bench procedure.
    c.setStrokeColor(colors.black)
    c.setLineWidth(1)
    c.roundRect(34, 99, 544, 78, 4, stroke=1, fill=0)
    text(46, 157, "WHILE OPEN: FLASH PREPARED FIRMWARE 0.1.4", 11.5, True)
    text(46, 139, "Lamp/bench power + VBUS-blocked USB. Preserve NVS / Home pairing.", 11)
    text(46, 123, "Check settled Off with status, registers and verify before fitting parts.", 11)
    text(46, 107, "If PWM is nonzero or a fault appears, fix that before adding resistors.", 11)

    text(34, 78, "Reassemble with power removed. Then check fades, low brightness", 11)
    text(34, 63, "and Off in a dark room. This is a trial; the glow fix is not yet proven.", 11)
    text(34, 36, "29 SEP 2026  ·  Contact reference, not an underside pad map", 9)
    c.save()
    print(OUTPUT)


if __name__ == "__main__":
    build()
