# ESP32 controller replacement specification

## Outcome

Make the user's original full-size Elgato Key Lights reliably controllable through Apple Home, without manual power cycling to recover from Wi-Fi or communication failures. Replace the complete Dexatek/Realtek module with an ESP32 development board; retain the light's existing LED driver, output circuitry, LEDs, housing, and external power supply.

The original lights currently need approximately daily power cycles. Reliability and observable recovery are the main deliverables, not extra lighting features.

## Locked decisions

- Remove/desolder the Dexatek module. The user is comfortable doing this. Do not make reverse-engineering or reflashing the Realtek firmware the implementation path.
- Use the Seeed Studio XIAO ESP32-C6 (4 MB flash, native USB), reusing Stillair's compatible Rust stack.
- Write the custom application firmware in Rust. Vendor libraries or C bindings are acceptable where needed; a pure-Rust radio stack is not required.
- The external supply is **13 V DC, rated 4 A**. The user will use a **buck converter board to power the ESP**. This is decided, not an alternative awaiting approval. Match its regulated output to the chosen dev board's supported power input.
- Normal light output is **3% brightness**. The replacement uses a fixed brightness, with no user-facing brightness slider required.
- Provide on/off and **two colour-temperature presets**. The user selected **3300 K and 5000 K**. Signed stock-firmware emulation establishes warm/cool raw PWM counts **6/2** and **3/6** at nominal 3%, respectively. This is not an optical calibration.
- Use Matter over Wi-Fi with BLE commissioning, reusing the Rust connectivity setup in `../stillair`, for direct Apple Home access. This supersedes the original direct-HAP/Homebridge options.
- Use prior firmware research as reference material rather than directly forking that firmware project.
- No cloud dependency for ordinary operation.

## Scope and working agreement

The intended implementation is the complete working replacement, including hardware integration, firmware, HomeKit access, recovery, and validation. Organize work around validation steps, not separately approved limited milestones. Make routine reversible decisions and continue through fixable failures without repeatedly asking whether to proceed.

Physical soldering, connecting hardware, and other tasks the agent cannot perform require coordination with the user. Batch these into concrete instructions based on evidence. Do not invent pinouts or present unresolved electrical details as verified facts. The user declined manual continuity mapping during the initial discussion; prefer existing documentation, visual tracing, and evidence obtainable by the agent. If physical evidence is indispensable, explain exactly what remains unknown rather than guessing.

The current authorization covers the complete autonomous software/research work,
review, verification, commit/push and a printed field guide. Physical checks are
consolidated in that guide, with explicit stop conditions for a different board.

## Hardware architecture

```text
13 V supply -> existing LED power circuitry -> existing LED panels
           -> fused buck -> diode -> removable USB/buck jumper -> XIAO 5V

XIAO GPIO18/20 -> TXU0102 -> 1k series resistors -> warm/cool driver-input pads
XIAO GPIO21   -> TXU0102 output enable (external pulldown)
XIAO 3V3     -> TXU VCCA; stock logic rail -> TXU VCCB
XIAO         -> Matter over Wi-Fi -> Apple Home
USB          -> flashing, local setup, logs and recovery (buck jumper OPEN)
```

The photographed board has a DK-9169 V1.0/RTL8711AM module and PCA9635PW at U3.
Remove the Realtek module. Lift and insulate U3 pins 6 (warm) and 10 (cool), then
connect the translator to their vacated board pads. Retain the stock power stages.
PCA I²C and OE are not connected to the replacement MCU. This supersedes the
original PCA-bus plan because PCA POR can drive a high output even with OE high.

[Hardware details](hardware.md) own the exact BOM, five pulldowns, two series
resistors, two bypass capacitors, pin table and source links. The physical pin-1 mark must be identified;
photos do not support a trustworthy numbered overlay. Verify stock logic voltage,
disabled-pad voltage, pulse integrity and safe-off behavior in the guide.
Do not tie the XIAO and stock logic rails together. Open the buck jumper before
attaching USB; close it only with USB unplugged. Insulate and secure all added
parts, retain USB/BOOT access, and put the antenna outside continuous metal.

Four two-pin LED connectors were disconnected by the user. They can remain disconnected for initial controller/network work, but must be reconnected for output, load, thermal, and recovery-flicker validation. Preserve connector identity and polarity. The photographs show board labels W-1, W-2, F-1, and F-2; these are not yet an electrically verified channel map.

## Firmware behaviour

- Accept on/off and selection of either colour-temperature preset.
- Keep brightness fixed at the user's intended 3% level. Stock API brightness is not assumed to equal raw PWM duty: establish a comparable physical output and document the calibration.
- Keep preset changes at approximately consistent perceived brightness.
- Represent and report actual applied state consistently to the integration.
- Define and document power-on behaviour, state persistence, and behaviour during communication loss. Prefer retaining the last intended state, without unnecessary flash writes or unexpected full-brightness output.
- Keep lighting output stable during network recovery where the hardware permits. Test reset/startup separately; do not assume controller resets preserve output.
- Bound network operations, connection counts, queues, and retry work so failures cannot exhaust memory or prevent recovery.
- Reconnect automatically after Wi-Fi loss, AP restart, DHCP/address changes, and integration restarts.
- Escalate recovery from reconnecting to network-subsystem restart and, when necessary, controller reboot. Use watchdog protection for stalled execution.
- Do not reboot merely because the Internet is unavailable. An absent AP or HomeKit controller must not cause uncontrolled reboot loops.
- Include useful diagnostics: firmware identity, uptime, reset reason, Wi-Fi status/RSSI, reconnect counters, last failure, and recovery history. Keep secrets out of logs.
- Make firmware updates and failed-build recovery practical through the dev board's USB path. OTA can be added if useful but is not a substitute for recovery access.

## Apple Home integration

Reuse Stillair's `rs-matter-embassy` stack on ESP32-C6: BLE commissioning, Matter
over Wi-Fi, flash-backed fabrics/network state, and USB commissioning logs. The
user selected this path on 2026-09-23. Use the compatible dependency revisions
from that project and adapt its device identity and endpoint handlers for lights.

Stillair's own records include Apple Home commissioning and cold-boot restoration;
they do not establish that Key Right passes any hardware or recovery test. Bring
over the connectivity and persistence, then verify discovery, event delivery,
reconnects, and fault recovery here. The earlier Homebridge fork remains background
reference and is not a dependency of this implementation.

Expose two mutually exclusive on/off light endpoints labelled 3300 K and 5000 K. Turning either on selects it; turning the inactive endpoint off leaves the active one on. Preserve fixed brightness regardless of incidental brightness writes from an integration. Both lights must have distinct stable identities and independently recover from failures.

## Agent development and observation

Use a host that stays reachable independently of the light's Wi-Fi. The ESP's USB interface provides flashing and serial logs; verify the chosen board's bootloader/reset wiring permits automated recovery from broken application firmware. Do not assume every dev board supports this identically.

The useful test harness comprises:

- Repeatable build, flash, log capture, command execution, and recovery scripts.
- A controllable test AP/network for deliberate failures without disrupting household devices.
- Timestamped device logs and host-side connection/API observations.
- An independently controlled power switch if full power-cycle testing is needed.
- Physical observation of output, using a sensor or a camera with fixed exposure/white balance. A suitable faster sensor is needed if brief flashes cannot be resolved by the camera.
- A test Matter controller, plus validation in the user's actual Apple Home environment.

These describe capabilities, not an approved shopping list. Select concrete equipment around available hardware when implementation begins.

## Validation steps and acceptance

1. **Hardware control:** verify the TXU/driver-pad path and prove on/off, both presets, and nominal fixed brightness on the real LEDs. Record channel mapping, voltage, polarity, PWM and observed optical output.
2. **Safe startup/output:** observe cold boot, controller reset, firmware restart, and recovery. No unexpected full-brightness pulse or uncontrolled output. Document any unavoidable interruption.
3. **Wi-Fi failure injection:** repeatedly interrupt the AP, reject/drop traffic, renew/change DHCP addressing, and restore service. The light must reconnect and become controllable without touching it or re-pairing.
4. **Integration recovery:** restart/disconnect Matter controllers and interrupt discovery/event connections. Restore correct state and control without duplicate accessories or stale state.
5. **Resource resilience:** exercise repeated commands, connection churn, malformed requests where applicable, and prolonged outages. Show bounded resource use and recovery without an accumulating leak or deadlock.
6. **Watchdog/recovery:** deliberately stall the application or trigger a controlled fault. Verify recovery and diagnostics. Verify the host can restore a known-good build when application firmware cannot communicate.
7. **Real-environment operation:** verify pairing, both presets, on/off, and recovery through the user's actual Apple Home setup, with both lights when converted.
8. **Soak:** run at least seven consecutive days on the real network at intended brightness, recording failures and recovery. Include actual LED load and normal household network conditions. Repeat the relevant tests after material fixes.

For short recoverable network failures, use an initial target of restored control within 60 seconds after network service returns, and report measured times rather than silently weakening the target. Longer outages must also recover automatically when service returns.

Any harness-forced power cycle or manual intervention needed to restore ordinary operation is a test failure, not successful firmware recovery. Deliberate power interruptions used as test inputs are distinct and must be labelled. Automatically recovered firmware crashes remain reportable defects, even if they avoid manual intervention.

Deliver the completed firmware, reproducible build/flash instructions, wiring notes based on the actual modification, integration configuration, diagnostics, rollback/recovery procedure, and a concise validation record with remaining limitations. This is one complete outcome; these checks are not approval gates between partial deliverables.
