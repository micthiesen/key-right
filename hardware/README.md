# Hardware

Key Right has two hardware generations. Firmware versions and hardware tags
(`hw-v1.0`, ...) are numbered separately.

| | v1, hand-wired (in use) | v2, drop-in board (planning) |
| --- | --- | --- |
| Controller | Off-the-shelf ESP32-C3_MINI_V1 board | ESP32-C3-MINI-1-N4 module on a custom PCB |
| Stock board | Kept; Realtek module replaced | Replaced entirely, including the resistor array |
| Install | Five wires to the stock PCA9635 (GPIO4/5/6, power, ground) | Unscrew, swap, plug in the LED and DC cables |
| Off-glow | Faint residual glow; cause unconfirmed | Hardware LED-rail switch, default off |
| USB with lamp power | Only with VBUS blocked | Safe; VBUS cannot reach the LED rail |

v1 is documented in [docs/hardware.md](../docs/hardware.md), the
[bench bring-up guide](../docs/bench-bring-up.md) and the
[validation record](../docs/validation-record.md).

v2 is a planning draft: goals, measurements still needed, block diagram, parts,
cost estimates and open questions are in [SPEC.md](SPEC.md). No schematic or
layout exists yet. KiCad files, BOM and fabrication outputs will live here, and
firmware will select the board at build time.

The licence for `hardware/` is undecided (CERN-OHL-P-2.0 is proposed; see
SPEC.md, open question 4).
