# ESP32 controller replacement specification

## Outcome

Replace only the Realtek controller in the user's original full-size Elgato Key
Light with a small ESP32 board. Keep the PCA9635, its stock LED driver/current
limiting circuitry, LED panels, housing, and 13 V / 4 A supply. Provide direct
local Apple Home control and automatic recovery without relying on cloud services.

## Locked decisions

- Use a Seeed Studio XIAO ESP32-C6 with 4 MB flash and native USB.
- Power it from the existing 13 V supply through the fixed-5 V Pololu D24V25F5
  buck converter. Flash the XIAO and request its pairing code over USB before
  connecting the buck. For later USB service, disconnect the buck's 5 V lead
  from the XIAO first;
  unplug USB before reconnecting it.
- Communicate with the retained PCA9635 over I²C. Project GPIO mapping is
  D10/GPIO18 SDA and D9/GPIO20 SCL. D3/GPIO21 may control active-low PCA OE only
  if its connection to the actual board OE net is verified.
- Preserve the user's fixed nominal **3% brightness**. Provide two fixed presets:
  **3300 K** and **5000 K**, with no user-facing brightness or continuous-CCT
  control. Signed stock firmware emulation produces warm/cool PCA values 6/2 and
  3/6 at those presets; these values are not optical calibration.
- Use Matter over Wi-Fi with BLE commissioning, adapting the compatible Rust
  connectivity stack from `../stillair`.
- Do not add circuitry unless measurements establish it is necessary to operate
  the bus or power the chosen board. Do not lift PCA pins or assume OE is a
  startup interlock.

## Physical boundary and limits

Board access points, SDA/SCL idle voltages and pull-ups, PCA OE routing, supply
polarity, and LED output behavior have not been established from the available
photos. Identify them using the field guide before wiring; stop on an unknown or
ESP-incompatible bus voltage. Never invent board-pad numbers or net assignments.

The PCA9635 power-on output configuration differs from its configured stock
mode. This minimal design has no independent power/gate cutoff. Firmware control
of OE, where physically verified, is not a promise of safe output during PCA
power-on, MCU reset, brownout, or failed bus operations. No production-safe
startup or physical performance claim is allowed before direct observation of
the assembled light.

## Firmware behaviour

- Expose two mutually exclusive On/Off controls for the 3300 K and 5000 K presets.
  Turning one on selects it. Turning off the inactive one does not stop the active
  preset. Brightness writes do not change the fixed output.
- Preserve last intended state and distinguish it from confirmed PCA register
  state and measured light output. A register write/readback is not a physical
  acknowledgement.
- Store state and Matter fabrics as appropriate; avoid unnecessary flash writes.
- Recover automatically from Wi-Fi/AP/address and Matter transport interruptions
  without discarding user intent. Do not reboot only because internet or a Home
  controller is absent. Bound retries and operations and retain useful diagnostics.
- Keep all hardware, network, time, and persistence I/O outside the portable core.

## Minimum acceptance

1. Verify the buck output and identify the actual PCA bus/OE connections. Bus
   voltage and pull-ups must suit XIAO GPIO; stop if uncertain or incompatible.
2. Confirm Off and both presets on the assembled lamp. Record the physical result
   and cold power-up/controller-reset behavior.
3. Pair with Apple Home using the closed housing and onboard antenna. Verify both
   presets and recovery after a short Wi-Fi interruption.

These checks do not establish long-term reliability. Record skipped and failed
checks in [the validation record](validation-record.md).

## Agent and host development

The host running development must remain reachable independently of the lamp's
Wi-Fi. Support reproducible build, flash, serial diagnostics, and USB bootloader
recovery. Test Wi-Fi and BLE in the completed metal housing. Keep credentials out
of the repository. Host tests cannot establish wiring, optical output, safe
startup, radio performance, or end-to-end recovery.

The complete authorized outcome includes firmware, build/flash instructions,
concise wiring and recovery guidance, Matter integration, diagnostics, and a
validation record with actual results and remaining limitations.
