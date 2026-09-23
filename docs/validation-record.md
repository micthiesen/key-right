# Physical validation record

Copy this file to `local/LIGHT-NAME-validation.md` before recording a specific
light. Keep pairing codes and Wi-Fi credentials out of the copy. Blank means
untested. Do not report software register readback as measured light output.

## Assembly

- Date, operator, light label:
- Firmware commit:
- XIAO model/revision and stock-board revision:
- Adapter polarity and buck output measured:
- PCA U3 pin-1 orientation and verified SDA/SCL connection points:
- SDA/SCL idle level (nominally 3.3 V):
- OE disposition: verified tied low / verified to D3 / unresolved:
- USB/buck exclusion followed (5 V lead disconnected before USB):

## Output and operation

| Check | Observation |
| --- | --- |
| Off | |
| 3300 K, nominal 3% | |
| 5000 K, nominal 3% | |
| Cold power-up/controller reset | |
| Closed-housing Apple Home pairing and control | |
| Short Wi-Fi interruption and recovery | |

Keep observations direct and specific, including any startup flash, unexpected
output, bus failure, or radio failure. A blank row is not a pass.

## Limits

The offline stock-firmware emulator establishes intended PCA register values, not
optical brightness or color temperature. No physical results are recorded here
until someone tests an actual modified light. Do not claim long-term reliability
or safe startup from a successful host build or PCA readback.
