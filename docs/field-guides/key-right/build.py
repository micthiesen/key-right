#!/usr/bin/env python3
"""Two-page DIY guide. Geometry: source photo, NXP and Dexatek datasheets.

Pin drawings are top views, not drilling/soldering templates. Source references
and the copy/visual review record are in README.md. No generated board pinouts.
"""
from pathlib import Path
from io import BytesIO
from xml.sax.saxutils import escape
from PIL import Image
from reportlab.pdfgen import canvas
from reportlab.lib import colors
from reportlab.lib.styles import ParagraphStyle
from reportlab.platypus import Paragraph
from reportlab.lib.utils import ImageReader
from pypdf import PdfReader

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
OUT = HERE / "key-right-field-guide.pdf"
W, H = 612, 792
C = canvas.Canvas(str(OUT), pagesize=(W, H), invariant=1)
C.setTitle("Key Right | Simple controller swap")
C.setAuthor("Key Right project")
C.setSubject("Two-page DIY guide: ESP32-C6, buck converter and retained PCA9635")
BODY = ParagraphStyle("body", fontName="Helvetica", fontSize=10.7, leading=13.6)
SMALL = ParagraphStyle("small", parent=BODY, fontSize=9, leading=11.5)
CODE = ParagraphStyle("code", parent=BODY, fontName="Courier", fontSize=9.5, leading=13)


def text(x, y, value, width, style=BODY):
    p = Paragraph(value, style)
    _, height = p.wrap(width, 900)
    if y - height < 32:
        raise ValueError(f"Page overflow: {value[:60]}")
    p.drawOn(C, x, y - height)
    return height


def label(x, y, value, size=10, bold=False, align="left"):
    C.setFillColor(colors.black)
    C.setFont("Helvetica-Bold" if bold else "Helvetica", size)
    draw = {"left": C.drawString, "right": C.drawRightString, "center": C.drawCentredString}[align]
    draw(x, y, value)


def header(page, title, subtitle):
    label(36, 763, "KEY RIGHT  /  SIMPLE CONTROLLER SWAP", 9, True)
    label(576, 763, "REV 2  |  23 SEP 2026", 8, align="right")
    label(36, 731, title, 25, True)
    text(36, 718, subtitle, 540, SMALL)
    C.setLineWidth(.5)
    C.line(36, 31, 576, 31)
    label(36, 18, "Original full-size Key Light  |  3300 K / 5000 K  |  stock nominal 3%", 8)
    label(576, 18, f"{page} / 2", 8, align="right")


def step(n, x, y, title, body, width=258):
    C.setLineWidth(.9)
    C.circle(x + 9, y - 9, 9, fill=0)
    label(x + 9, y - 12.4, str(n), 10, True, "center")
    label(x + 24, y - 11, title, 11, True)
    return 19 + text(x + 24, y - 19, body, width - 24)


def photo_crop(x, y, w, h):
    # Clip the original photo in PDF coordinates; preserve its geometry. The
    # crop includes the controller, U3 and J2 input. Originals stay intact.
    source = Image.open(ROOT / "docs/references/photos/keylight-3623.png")
    stream = BytesIO()
    source.convert("RGB").save(stream, format="JPEG", quality=94)
    stream.seek(0)
    iw, ih = source.size
    unit = iw / 1824
    crop_x, crop_y, crop_w = 450 * unit, 480 * unit, 1250 * unit
    scale = w / crop_w
    C.saveState()
    path = C.beginPath()
    path.rect(x, y, w, h)
    C.clipPath(path, stroke=0, fill=0)
    C.drawImage(ImageReader(stream), x - crop_x * scale,
                y + h - (ih - crop_y) * scale, width=iw * scale, height=ih * scale)
    C.restoreState()
    C.setLineWidth(.6)
    C.rect(x, y, w, h)


def module_map():
    # Dexatek DK9169 p4 rotated 90 degrees counterclockwise: antenna at left,
    # pins 9..18 bottom-to-top on the right edge, matching the source photo.
    x, y, w, h = 54, 341, 222, 119
    C.setLineWidth(.7)
    C.rect(x, y, w, h)
    C.line(x + 43, y, x + 43, y + h)
    text(x + 5, y + 71, "ANT<br/>end", 35, SMALL)
    label(x + 130, y + 65, "OLD CONTROLLER", 10, True, "center")
    label(x + 130, y + 47, "DK9169", 10, align="center")
    for i in range(10):
        yy = y + 6 + i * 11.8
        pin = i + 9
        C.circle(x + w, yy, 4, fill=0)
        value = {9: "9  SCL?", 10: "10  SDA?", 14: "14  EN?"}.get(pin, str(pin))
        label(x + w + 9, yy - 3, value, 8.7, pin in (9, 10, 14))
    positions = [(32, 13), (33, 32)] + [(i, 66 + (i - 1) * 20) for i in range(1, 9)]
    for pin, dx in positions:
        C.circle(x + dx, y, 3, fill=0)
        label(x + dx, y - 13, str(pin), 7.5, align="center")
    positions = [(31 - i, 12 + i * 20) for i in range(5)] + [(26 - i, 110 + i * 15) for i in range(8)]
    for pin, dx in positions:
        C.circle(x + dx, y + h, 3, fill=0)
        if pin in (31, 27, 26, 19):
            label(x + dx, y + h + 9, str(pin), 7.5, align="center")
    text(40, 318, "<b>Circle confirmed pads.</b> The ? labels are candidates; check them with the meter.", 305, SMALL)


def pca_map():
    x, y, w, h = 412, 335, 60, 137
    C.setLineWidth(.7)
    C.rect(x, y, w, h)
    C.circle(x + 8, y + h - 8, 2.2, fill=1)
    label(x + w / 2, y + 73, "U3", 12, True, "center")
    label(x + w / 2, y + 56, "PCA9635", 8, align="center")
    for i in range(14):
        yy = y + h - 5 - i * 9.8
        left, right = i + 1, 28 - i
        C.line(x - 6, yy, x, yy)
        C.line(x + w, yy, x + w + 6, yy)
        label(x - 9, yy - 2.5, "14 GND" if left == 14 else str(left), 7.7, left in (1, 14), "right")
        role = {28: "VDD", 27: "SDA", 26: "SCL", 23: "OE"}.get(right, "")
        label(x + w + 9, yy - 2.5, f"{right} {role}", 8, bool(role))
    text(371, 318, "<b>Top view.</b> Match the dot/notch on the real chip before counting pins.", 205, SMALL)


header(1, "Find and circle the pins", "Confirm each connection with a multimeter, then circle its pad on the photo and map.")
photo_crop(36, 489, 475, 207)
label(529, 668, "CIRCLE", 9, True)
for i, word in enumerate(["SDA", "SCL", "EN", "J2 +", "J2 -"]):
    label(529, 642 - 29 * i, word, 10)
module_map()
pca_map()
C.setLineWidth(.4)
C.line(36, 284, 576, 284)
step(1, 36, 272, "Open the lamp", "Unplug it. Circle <b>J2 + and -</b> in the photo; these are the power input pads. Find the Realtek module and U3.")
step(2, 36, 201, "Find U3 pin 1", "Match its dot/notch to the drawing. Circle pins <b>27 SDA, 26 SCL and 23 OE</b> on the map.")
step(3, 36, 130, "Find SDA and SCL pads", "With power off, check continuity: module <b>10 to U3 27</b>, and <b>9 to U3 26</b>. Circle the confirmed module pads.")
step(4, 318, 272, "Find the enable wire", "Trace <b>U3 23 (OE)</b> to the module; try <b>pad 14</b>. Circle that pad as EN. If OE is tied to GND, write <b>NO EN WIRE</b>.")
step(5, 318, 185, "Check the voltages", "Power the stock board. J2 + to -: ____ V (about <b>13 V</b>). SDA / SCL to J2 -: ____ / ____ V (about <b>3.3 V</b>). Unplug.")
step(6, 318, 111, "Remove the controller", "Desolder only the Realtek module. Keep U3 and its pins intact. Use the pads you circled for the new wires.")
text(36, 51, "<b>If a connection is unclear or the signal voltage is not about 3.3 V, stop and record it before wiring the ESP.</b>", 540, SMALL)
C.showPage()

header(2, "Wire it, pair it, try it", "Parts: XIAO ESP32-C6 + Pololu D24V25F5 (5 V buck), wire and insulated mounting. Keep the stock LED circuitry.")

# Wiring overview, drawn from exact named terminals rather than arbitrary PCB
# pad coordinates. It is a connection diagram, not a board footprint.
C.setLineWidth(.8)
C.roundRect(36, 574, 136, 112, 4)
C.roundRect(207, 574, 132, 112, 4)
C.roundRect(374, 551, 202, 135, 4)
label(104, 668, "STOCK J2 INPUT", 11, True, "center")
label(273, 668, "5 V BUCK", 11, True, "center")
label(475, 668, "XIAO ESP32-C6", 11, True, "center")
for yy, left, right in [(640, "J2 + / 13 V", "VIN"), (610, "J2 - / GND", "GND")]:
    label(163, yy - 3, left, 10, align="right")
    label(216, yy - 3, right, 10)
    C.line(172, yy, 207, yy)
label(330, 640 - 3, "5V OUT", 10, align="right")
label(383, 640 - 3, "5V", 10)
C.line(339, 640, 374, 640)
label(330, 610 - 3, "GND", 10, align="right")
label(383, 610 - 3, "GND", 10)
C.line(339, 610, 374, 610)
label(475, 583, "USB unplugged when wired", 9, align="center")
label(475, 568, "to the buck", 9, align="center")
text(36, 557, "<b>Your confirmed stock pads</b><br/>SDA: ______  SCL: ______  EN: ______", 320, BODY)
# Connection rows leave the user's recorded source pads directly above.
for i, (name, dest) in enumerate([("SDA", "D10 / GPIO18"), ("SCL", "D9 / GPIO20"), ("EN / OE*", "D3 / GPIO21")]):
    yy = 507 - i * 23
    label(50, yy - 3, name, 11, True)
    C.line(123, yy, 390, yy)
    C.line(385, yy + 3, 390, yy)
    C.line(385, yy - 3, 390, yy)
    label(400, yy - 3, dest, 11, True)
text(36, 438, "*Connect EN only to the controller pad you traced to OE. If OE is tied to GND, leave D3 unconnected.", 540, SMALL)
C.line(36, 410, 576, 410)

step(7, 36, 398, "Flash before wiring", "Connect only the ESP to USB. From the repo, list its port, flash, then read its pairing code:", 540)
for i, line in enumerate([
    "python3 scripts/device.py --list",
    "sh scripts/flash.sh /dev/cu.usbmodemPORT",
    "python3 scripts/device.py --port /dev/cu.usbmodemPORT commissioning code",
]):
    text(60, 356 - i * 14, escape(line), 510, CODE)
text(60, 311, "Replace the port above. Pairing code: ____________________  Then unplug USB.", 510, BODY)
step(8, 36, 283, "Connect power", "Wire the buck to J2 + and -. Power the lamp and check for <b>5 V</b> at the buck output. Unplug, then connect the ESP.", 540)
step(9, 36, 230, "Connect the control wires", "Solder SDA, SCL and the identified EN wire as drawn. Reconnect any unplugged LED leads to their original sockets.", 540)
step(10, 36, 177, "Pair in Apple Home", "Power the lamp. In Home, add an accessory using the code above. Pair within <b>15 minutes</b>; power-cycle to reopen the window.", 540)
step(11, 36, 123, "Try both presets", "Try both presets and Off. Rename them 3300 K (warm) and 5000 K (cool). Turn one on; power-cycle and check it returns.", 540)
step(12, 36, 70, "Mount and close", "Unplug. Insulate and secure the ESP and buck, then close the lamp. Check Home still reaches it.", 540)
C.showPage()
C.save()
reader = PdfReader(OUT)
assert len(reader.pages) == 2
content = "\n".join(page.extract_text() for page in reader.pages)
for needed in ["3300 K", "5000 K", "GPIO18", "GPIO20", "GPIO21", "commissioning code", "NO EN WIRE"]:
    assert needed in content, needed
print(f"Built {OUT} ({len(reader.pages)} pages)")
