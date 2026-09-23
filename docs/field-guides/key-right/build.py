#!/usr/bin/env python3
"""Build the reviewed Letter field guide. Run with the pinned requirements."""
from pathlib import Path
from io import BytesIO
from PIL import Image
from reportlab.pdfgen import canvas
from reportlab.lib import colors
from reportlab.lib.styles import ParagraphStyle
from reportlab.platypus import Paragraph, Table, TableStyle
from reportlab.lib.utils import ImageReader
from pypdf import PdfReader

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
OUT = HERE / 'key-right-field-guide.pdf'
W, H, M = 612, 792, 42
WIDTH = W - 2*M
C = canvas.Canvas(str(OUT), pagesize=(W,H), invariant=1)
C.setTitle('Key Right | Assembly and commissioning field guide')
C.setAuthor('Key Right project')
C.setSubject('XIAO ESP32-C6, TXU0102, 3300 K / 5000 K at stock nominal 3%')
STYLE = ParagraphStyle('body', fontName='Helvetica', fontSize=11.3, leading=15, spaceAfter=6)
SMALL = ParagraphStyle('small', parent=STYLE, fontSize=9.7, leading=12.5)
CELL = ParagraphStyle('cell', parent=STYLE, fontSize=10.1, leading=12.6)
CODE = ParagraphStyle('code', parent=STYLE, fontName='Courier', fontSize=9.5, leading=13)
y = 0
PAGE = 0

def p(text, style=STYLE, gap=6, x=M, width=WIDTH):
    global y
    block=Paragraph(text, style)
    _,h=block.wrap(width,1000)
    if y-h < 47: raise ValueError(f'Page {PAGE} overflow: {text[:80]} at {y-h}')
    block.drawOn(C,x,y-h); y-=h+gap

def title(n, label, subtitle):
    global y,PAGE
    PAGE=n
    C.setFillColor(colors.black);C.setFont('Helvetica-Bold',10)
    C.drawString(M,H-33,'KEY RIGHT  /  FIELD GUIDE')
    C.setFont('Helvetica',9);C.drawRightString(W-M,H-33,'23 SEP 2026  |  REV 1')
    C.setFont('Helvetica-Bold',24);C.drawString(M,H-70,f'{n}. {label}')
    y=H-91;p(subtitle, SMALL, 12)

def end():
    C.setLineWidth(.5);C.line(M,38,W-M,38)
    C.setFont('Helvetica',8.5);C.drawString(M,24,'Original full-size Key Light, board 53 rev 1  |  Follow stop conditions')
    C.drawRightString(W-M,24,f'{PAGE} / 8');C.showPage()

def head(t):
    global y
    y-=3;p(t,ParagraphStyle('h',parent=STYLE,fontName='Helvetica-Bold',fontSize=13,leading=16),5)

def note(t):
    global y
    q=Paragraph(t,STYLE);_,h=q.wrap(WIDTH-18,1000)
    if y-h-16<47: raise ValueError(f'Note overflow page {PAGE}')
    C.setFillColor(colors.Color(.94,.94,.94));C.rect(M,y-h-14,WIDTH,h+14,fill=1,stroke=0)
    C.setFillColor(colors.black);q.drawOn(C,M+9,y-h-7);y-=h+23

def table(rows,widths):
    global y
    cells=[[Paragraph(str(v),CELL) for v in row] for row in rows]
    t=Table(cells,colWidths=widths,hAlign='LEFT')
    t.setStyle(TableStyle([('VALIGN',(0,0),(-1,-1),'TOP'),('BACKGROUND',(0,0),(-1,0),colors.Color(.9,.9,.9)),('LINEBELOW',(0,0),(-1,0),.7,colors.black),('LINEBELOW',(0,1),(-1,-1),.3,colors.Color(.7,.7,.7)),('LEFTPADDING',(0,0),(-1,-1),6),('RIGHTPADDING',(0,0),(-1,-1),6),('TOPPADDING',(0,0),(-1,-1),5),('BOTTOMPADDING',(0,0),(-1,-1),5)]))
    _,h=t.wrap(WIDTH,1000)
    if y-h<47: raise ValueError(f'Table overflow page {PAGE}: {y-h}')
    t.drawOn(C,M,y-h);y-=h+10

def code(lines):
    for line in lines:
        p(line.replace('&','&amp;').replace('<','&lt;').replace('>','&gt;'),CODE,1)
    global y
    y-=5

def checkbox(t): p('[  ] '+t)

def chip(x,top,w,h,n,left,right,label):
    C.setLineWidth(.8);C.rect(x,top-h,w,h)
    C.setFont('Helvetica-Bold',10);C.drawCentredString(x+w/2,top-h/2,label)
    C.circle(x+8,top-8,2.5,fill=1)
    step=(h-20)/(n-1)
    for i in range(n):
        yy=top-10-i*step
        C.line(x-8,yy,x,yy);C.line(x+w,yy,x+w+8,yy)
        C.setFont('Helvetica',9)
        C.drawRightString(x-11,yy-3,left[i]);C.drawString(x+w+11,yy-3,right[i])

# 1
title(1,'Parts and working rules','Two presets: 3300 K and 5000 K. Both reproduce the stock nominal 3% digital command.')
p('Use one kit per light. This guide is ready for assembly; electrical output, Apple Home operation and the seven-day soak have <b>not</b> been tested on a board.')
table([
['Qty','Buy / prepare','Use'],
['1','Seeed XIAO ESP32-C6<br/>SKU 113991254','4 MB, native USB controller'],
['1/1','TI TXU0102DCUR + Chip Quik PA0042C','VSSOP-8 translator + 0.5 mm adapter'],
['1','Pololu D24V25F5, item 2850','13 V to 5 V buck'],
['1','Vishay 1N5822-E3/54','3 A Schottky; band toward XIAO'],
['1','Littelfuse 0251002.MXL','2 A axial fuse in added 13 V branch'],
['1/1','Harwin M20-9990246 + M7567-05','2-pin header and removable shunt'],
['5','Vishay MRS25000C1002FCT00','10 kohm pulldowns'],
['2','Vishay MRS25000C1001FCT00','1 kohm series resistors / load screen'],
['2','Vishay K104K15X7RF5TL2','100 nF, 50 V ceramic bypass'],
['1','Taoglas FXP73.07.0100A','2.4 GHz flex antenna, U.FL-compatible'],
['Kit','22 AWG power wire; 30 AWG signal wire; isolated-pad perfboard; heat-shrink; polyimide tape; nylon mounts','Insulate, secure, provide strain relief'],
],[34,284,210])
head('Tools')
p('Fine soldering iron, flux, braid, magnification, tweezers, insulated probe tips, DMM and USB-C data cable. Use a scope with at least 100 MHz bandwidth and 200 MS/s to resolve the shortest ~80 ns pulse. Borrow one if needed; a DMM cannot validate startup flashes.')
note('<b>Every wiring change:</b> unplug both USB and the 13 V supply, verify rails are discharged. Open the buck shunt before USB is attached. Never connect 13 V to the XIAO.')
p('Parts/source details: <b>docs/hardware.md</b>. Build and console reference: <b>docs/development.md</b>. Work through pages 2-6 before pairing.',SMALL)
end()

# 2
title(2,'Identify the actual board','The source photograph locates parts. The package drawing establishes pin numbers only after you find the real pin-1 mark.')
photo=ROOT/'docs/references/photos/keylight-3623.png'
# Resample only for the PDF's 4-inch placement; keep the source photo untouched.
photo_image=Image.open(photo)
photo_image.thumbnail((1600,1200))
photo_bytes=BytesIO()
photo_image.convert('RGB').save(photo_bytes,format='JPEG',quality=92)
photo_bytes.seek(0)
C.drawImage(ImageReader(photo_bytes),M,y-218,width=291,height=218,preserveAspectRatio=True,mask='auto')
# Photo landmarks refer to visible parts, never individual chip legs.
C.setFillColor(colors.white);C.rect(M+3,y-215,285,22,fill=1,stroke=0);C.setFillColor(colors.black)
C.setFont('Helvetica',9);C.drawString(M+8,y-207,'Left: Realtek module. Right: U3 PCA9635.')
chip(427,y-7,66,189,14,[str(i) for i in range(1,15)],[str(i) for i in range(28,14,-1)],'U3')
C.setFont('Helvetica-Bold',10);C.drawCentredString(460,y-214,'TOP VIEW')
y-=237
note('<b>STOP if pin 1 is uncertain.</b> Locate the physical dot/notch under magnification. Rotate the drawing mentally to match the chip. Do not infer orientation from this photo, text direction, or nearby test pads.')
table([
['PCA9635 U3 pin','Role in this modification'],
['6 / LED0','Warm: lift IC leg; connect TXU to the vacated PCB pad'],
['10 / LED4','Cool: lift IC leg; connect TXU to the vacated PCB pad'],
['14 / VSS','Verified common ground reference'],
['28 / VDD','Measure stock logic rail; later feeds TXU VCCB'],
['23 / OE; 26 / SCL; 27 / SDA','Leave unchanged. Do not wire to the XIAO.'],
],[145,383])
checkbox('Photograph and label all four LED leads: W-1, W-2, F-1, F-2, including connector polarity. Keep them disconnected through page 5.')
checkbox('Label the stock input + and GND after checking the adapter polarity with a meter. Reuse its 13 V / 4 A supply; do not infer barrel polarity.')
p('Pin source: NXP PCA9635 datasheet, TSSOP28 top view. Local copy: docs/references/datasheets/NXP-PCA9635.pdf.',SMALL)
end()

# 3
title(3,'Measure, then isolate','Do these checks in order. Keep the original module intact until the first voltage measurement is recorded.')
head('A. Record the stock rails')
checkbox('With power removed, confirm U3 pin 14 connects to input GND. Clip the meter ground there. Cover the probe shaft; expose only its tip.')
checkbox('Power the stock board with LEDs disconnected. Measure U3 pin 28 relative to pin 14. Continue only at <b>2.3-5.5 V</b>. Record: stock VDD ______ V.')
p('If the old controller works, optionally capture its waveforms at 3%, 3300 K and 5000 K. Otherwise use the firmware evidence and controlled tests on pages 5-6.')
head('B. Remove and separate, with both supplies unplugged')
checkbox('Desolder the complete Realtek module without tearing pads or moving surrounding parts. Preserve it for physical rollback.')
checkbox('Reconfirm U3 pin 1. Using flux, gently lift <b>only legs 6 and 10</b> clear of their PCB pads. Avoid bending at the package body. Insulate the free legs so they cannot touch pads or wires.')
checkbox('Inspect for bridges. Verify each lifted leg is electrically separated from its original pad. Do not cut LED power traces or remove current-limit parts.')
checkbox('Fit a 10 kohm pulldown from each vacated pad to GND. Leave TXU outputs disconnected for this check.')
head('C. Confirm the board tolerates the new interface')
checkbox('Power the stock board, LEDs still disconnected. Remeasure VDD after module removal. It must remain stable and within 2.3-5.5 V.')
checkbox('Measure each pulled-down driver pad: warm ______ V; cool ______ V. Both must be <b>below 0.1 V</b>. Stop for a higher level, a short to 13 V, or an unidentified pad.')
checkbox('Power off. Temporarily feed one pad from stock VDD through <b>1 kohm</b>, keeping its 10 kohm pulldown. Power on: require pad voltage at least <b>0.88 x VDD</b> (2.90 V at 3.3 V). Repeat for the other pad. LEDs and TXU stay disconnected. Unplug, then remove the temporary VDD feed.')
p('Stop if either load screen fails or VDD disappears after module removal. These DC tests do not prove pulse shape or safe optical output; the next checks do.')
C.drawImage(ImageReader(str(HERE/'assets/probing-technique.png')),M,y-84,width=WIDTH,height=84,preserveAspectRatio=True,anchor='c',mask='auto')
y-=90
p('Generic probing technique illustration. It is not a board pinout.',SMALL)
end()

# 4
title(4,'Wire the replacement','TXU0102DCUR is a non-inverting A-to-B translator. Pin numbers below are for its VSSOP-8 package, viewed from above.')
chip(270,y-4,72,109,4,['1 B2Y','2 GND','3 VCCA','4 A2'],['8 B1Y','7 VCCB','6 OE','5 A1'],'TXU')
C.setFont('Helvetica',10);C.drawString(M,y-38,'Find its pin-1 mark.');C.drawString(M,y-55,'Match the adapter pads.');C.drawString(M,y-72,'Solder, inspect, then test.')
y-=128
table([
['From','To','Also connect'],
['XIAO D10 / GPIO18','TXU pin 5 / A1','10 kohm to GND'],
['XIAO D9 / GPIO20','TXU pin 4 / A2','10 kohm to GND'],
['XIAO D3 / GPIO21','TXU pin 6 / OE','10 kohm to GND'],
['TXU pin 8 / B1Y','1 kohm, then U3 pin 6 pad / WARM','10 kohm: pad side to GND'],
['TXU pin 1 / B2Y','1 kohm, then U3 pin 10 pad / COOL','10 kohm: pad side to GND'],
['XIAO 3V3','TXU pin 3 / VCCA','100 nF from pin 3 to GND'],
['Verified stock VDD','TXU pin 7 / VCCB','100 nF from pin 7 to GND'],
['Common GND','TXU pin 2 + XIAO + buck GND','Do not join the two logic rails'],
],[144,230,154])
head('Power branch')
code(['Verified +13 V -> 2 A fuse -> buck VIN',
      'buck 5V OUT -> diode anode -> banded end -> shunt -> XIAO 5V',
      'Verified GND -> buck GND -> XIAO GND -> TXU pin 2'])
p('Keep the fuse near the +13 V takeoff; insulate the inline diode. Put both capacitors close to TXU. Five pulldowns hold reset safely low. Keep both 1 kohm output resistors fitted: they limit current to 5.5 mA or less. Never bypass them to fix a failed pulse test.')
note('<b>USB connected = shunt OPEN.</b> USB can power the MCU while 13 V powers the stock board during tests. <b>Standalone = USB unplugged, shunt CLOSED.</b> Change the shunt with both supplies unplugged.')
p('Fit the Taoglas antenna to the XIAO U.FL port with power off. Place it at a plastic window or outside the metal cavity. Firmware selects the external antenna. Insulate all boards and preserve access to USB, BOOT and RESET.',SMALL)
end()

# 5
title(5,'First power and firmware','Keep all LED connectors disconnected. Run commands from the repository root. Replace PORT with the actual serial path.')
checkbox('Inspect every wire against page 4. Check no rail-to-GND short and no bridge from a lifted PCA leg to its pad. Check diode band direction and open shunt.')
checkbox('With XIAO disconnected, power the buck: output before diode ______ V (target 5 V). Unplug, connect XIAO, then use USB with shunt open. Verify XIAO 3V3 is stable near 3.3 V.')
head('Build and flash the real image')
code(['sh scripts/check.sh', 'sh scripts/check-firmware.sh',
      'python3 scripts/device.py --list', 'sh scripts/flash.sh PORT',
      'python3 scripts/device.py --port PORT status'])
p('Install espflash 4.5.0 if the script requests it. Expect <b>mode=hardware</b>, <b>configured=false</b>. Firmware starts isolated without a committed profile. A bench image is not suitable for these wires.')
checkbox('Power the stock board too. Verify TXU VCCA = XIAO 3V3, VCCB = stock VDD, OE low, and both driver pads below 0.1 V. Stop on any unexpected voltage or heating.')
head('Stage the candidate; test pulses without LED load')
code(['python3 scripts/device.py --port PORT profile stage \\',
      '  @hardware/profiles/stock-3300-5000.hex',
      'python3 scripts/device.py --port PORT profile test 1',
      'python3 scripts/device.py --port PORT outputs'])
p('Each test ends after about 10 seconds. Repeat with <b>profile test 2</b>, then <b>profile test off</b>. Use a short scope ground lead on common GND, never on +13 V. Keep probe tips clear of adjacent legs.')
table([
['Test','Warm / cool HIGH duty','outputs command_q4 / active_q4'],
['1: 3300 K','6/256 and 2/256','96 / 32 on both readbacks'],
['2: 5000 K','3/256 and 6/256','48 / 96 on both readbacks'],
['off','Both low; OE low','0 / 0'],
],[90,185,253])
p('Scope at the pads <b>after the 1 kohm resistors</b>: about 97.68 kHz, low near GND, high at least 0.88 x stock VDD, including the ~80 ns pulse. Stop if pulses are distorted. Readback: duty_bits=8, divider_q8=819, clock_hz=80000000. This is MCU state, not measured pad voltage.',SMALL)
end()

# 6
title(6,'Validate the light, then commit','Do not attest skipped or failed checks. The profile flags are your record of physical validation on this specific light.')
head('A. Connect the load and check each preset')
checkbox('Unplug both supplies. Reconnect each LED lead to its labelled original socket/polarity. Keep the buck shunt open for USB testing.')
checkbox('Apply USB and 13 V. Before testing, the light must remain off. Stage the profile again if the MCU restarted, using page 5.')
checkbox('Run <b>profile test 1</b>, then <b>profile test 2</b>. Each must produce a dim, steady light and stop after about 10 seconds. Preset 1 must be warmer than preset 2. Run <b>profile test off</b>; verify darkness.')
p('Nominal 3% means the exact stock digital settings, not 3% raw duty or measured lumens. Compare with the original light at fixed camera exposure/white balance or the same light-meter position. Record discrepancies; current firmware rejects arbitrary PWM edits.')
head('B. Check reset and both power orders before commitment')
checkbox('Scope the driver pads during a staged test while pressing XIAO RESET. They must become low during reset; no pulse above the tested duty. Repeat while turning only the stock 13 V supply off/on with USB retained.')
checkbox('With 13 V retained and shunt open, remove/reconnect USB. Pads must remain low while the MCU is absent. Confirm unloaded and loaded behavior. Restage after an MCU restart.')
checkbox('Try standalone power: unplug both, close shunt, connect only 13 V. Verify loaded XIAO input after diode and stable 3V3. Light stays off while unconfigured. Unplug, reopen shunt, reconnect USB and 13 V for the next commands.')
head('C. Record all four checks and enable normal operation')
p('Flags: <b>1</b> wiring/polarity; <b>2</b> disabled-output isolation; <b>4</b> safe boot/reset; <b>8</b> both nominal presets. All must pass to attest <b>15</b>. Staging a profile clears imported flags and persists OFF.')
code(['python3 scripts/device.py --port PORT profile stage \\',
      '  @hardware/profiles/stock-3300-5000.hex',
      'python3 scripts/device.py --port PORT profile attest 15',
      'python3 scripts/device.py --port PORT profile commit',
      'python3 scripts/device.py --port PORT on 1',
      'python3 scripts/device.py --port PORT verify'])
p('Commit starts OFF. Expect configured=true after commitment and a valid acknowledged state after On. Check <b>on 2</b>, <b>off</b>, then <b>on 1</b> followed by <b>reboot</b>: the last intended state should return after startup. Recheck boot waveforms with saved ON and OFF.')
note('<b>STOP:</b> unexpected bright output, light with OE low, unstable rails, excessive heat, wrong colour order or failed pulse checks. Unplug both supplies and preserve measurements/photos for diagnosis.')
end()

# 7
title(7,'Apple Home and recovery','Pair each light separately. Use a compatible Apple Home Matter controller and a 2.4 GHz network; keep setup codes private.')
code(['python3 scripts/device.py --port PORT commissioning code'])
p('After profile commit this opens a 15-minute commissioning window on an uncommissioned device and returns its 11-digit code. In Home, choose <b>Add Accessory</b>, use the manual code option and follow the prompts. Development attestation may produce an uncertified-accessory warning.')
p('Two on/off controls represent 3300 K and 5000 K. Rename them for the light/location. Turning one on selects it; turning the other, inactive control off must leave it on. Brightness is fixed. Each MCU MAC gives a distinct identity.')
head('Run with the final antenna placement and housing closed')
table([
['Test','Pass condition / record'],
['Saved ON / OFF; 10 supply cycles','No excessive pulse; intended state and pairing restored. An OFF interval during MCU reset is expected.'],
['USB test watchdog','After commitment, run the command below. About 15 s later MCU resets, saved intent returns; scope reset waveform and record reset reason.'],
['AP absent: 30 s, 5 min, 1 hour','Output stable; control returns without touching light or re-pairing. Initial target: within 60 s after usable network returns.'],
['DHCP address change; hub restart','Discovery, subscriptions and correct state recover. Use a test AP to avoid disrupting other devices.'],
['Internet blocked, LAN available','Local control stays available; no reboot caused merely by missing Internet.'],
['100 alternating preset commands','Correct final state; inactive Off never shuts active preset off. Repeat with both converted lights.'],
],[162,366])
code(['python3 scripts/device.py --port PORT test watchdog',
      'python3 scripts/device.py --port PORT status'])
p('Run status after USB reappears; use --list if the port changes. A deliberate watchdog test is a labelled fault input. Unexpected crashes remain defects even if they recover automatically.')
head('Seven consecutive days')
p('Run at intended brightness with actual LED load and normal household networking. Record daily uptime/reset, failures, recovery time and temperature observations in <b>docs/validation-record.md</b> (copy to ignored local/ first). Any manual rescue for ordinary operation is a failure.')
note('CPU watchdog, Wi-Fi deadlines and local-IP checks are implemented. A logically hung Matter task with a healthy CPU/IP may evade them. Real Home fault tests and the soak are required before calling recovery reliable.')
end()

# 8
title(8,'Quick reference and rescue','Keep this page beside the light. Commands below use: python3 scripts/device.py --port PORT COMMAND')
table([
['COMMAND','Meaning'],
['status / outputs / verify','Intent, MCU acknowledgement and counters / live LEDC values / check and reconcile output'],
['on 1 / on 2 / off','3300 K / 5000 K / durable OFF'],
['profile test 1 / 2 / off','Staged local test, about 10 s maximum; not a Matter acknowledgement'],
['commissioning code','Open setup window for an uncommissioned, configured light'],
['reboot','Isolate, preserve intent, reset; expect brief output interruption'],
['monitor','Read logs until Ctrl-C; no command writes'],
],[189,339])
head('If something goes wrong')
p('<b>No USB port:</b> open buck shunt; check USB data cable. Hold BOOT, press/release RESET, release BOOT, list ports and run <b>sh scripts/flash.sh PORT</b>. If needed, hold BOOT while attaching USB, then release. Reflash preserves NVS with this partition table.')
p('<b>Command timeout:</b> it may have executed. Inspect status before retrying a write. Use only one console/monitor at a time. Acknowledgement means MCU state was applied, not that the LEDs were measured.')
p('<b>Output fault:</b> disconnect power for wiring changes. Read status/outputs; check stock VDD and isolation. Firmware retries recoverable output/storage faults every 5 seconds. Never bypass pulldowns, fuse or output-enable checks.')
p('<b>Pairing lost / deliberate factory reset:</b> remove the accessory from Home first. With USB attached and shunt open, erase only NVS using the command below. This deletes pairing, Wi-Fi, code, intent and profile, requiring full provisioning again.')
code(['espflash erase-parts --port PORT \\',
      '  --partition-table firmware/app/partitions.csv nvs'])
p('<b>Physical rollback:</b> unplug both supplies, disconnect added outputs/power, restore lifted PCA legs to their original pads and refit the preserved module. Inspect for bridges before power. The old controller’s reliability problem may return.')
head('Record before closing the housing')
p('Light label __________________  Date __________  Firmware commit __________<br/>Stock VDD ______ V   XIAO loaded input ______ V   XIAO 3V3 ______ V<br/>Warm/cool disabled ______ / ______ V   PWM frequency ______ kHz<br/>3300 K checked [  ]   5000 K checked [  ]   Boot/rail loss checked [  ]<br/>Home + closed housing [  ]   Recovery tests [  ]   7-day soak ends __________')
p('Sources and rebuild instructions: docs/field-guides/key-right/README.md. Exact wiring: docs/hardware.md. Firmware evidence: docs/references/firmware-analysis.md. No private pairing code is printed in this guide.',SMALL)
end()
C.save()
reader=PdfReader(str(OUT))
assert len(reader.pages)==8
assert all(float(p.mediabox.width)==612 and float(p.mediabox.height)==792 for p in reader.pages)
print(f'{OUT}: {len(reader.pages)} Letter pages')
