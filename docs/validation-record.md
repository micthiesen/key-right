# Key Right physical acceptance record

Copy this file to `local/LIGHT-NAME-validation.md` before filling it out.
`local/` is ignored by Git. Keep commissioning codes and Wi-Fi credentials out of
this record. Blank means **not tested**, never passed.

## Identity and assembly

- Date/operator/light label:
- Firmware commit and ELF SHA-256:
- XIAO identity (`status`):
- Stock board/revision and photo filenames:
- U3 physical pin-1 marker identified and photographed:
- Adapter voltage/polarity; stock VDD before/after module removal:
- Buck output before diode; loaded XIAO input; 3V3:
- Lifted pins 6/10 isolated from their pads; no bridges:
- Five 10 kΩ pulldowns/two 100 nF capacitors fitted:
- Two 1 kΩ series resistors between TXU outputs and driver pads:
- TXU pin-1/package orientation and point-to-point checks:
- Each disconnected driver pad with 10 kΩ to GND, powered: warm V / cool V:
- Each pad temporarily fed through 1 kΩ from stock VDD: warm V / cool V (both ≥0.88 × VDD); temporary feed removed:
- USB/buck jumper rules verified; strain relief/insulation/antenna placement:

## Output checks

Use a scope with short ground lead at the common low-voltage ground. Record probe,
bandwidth/sample rate, rail levels and saved captures. Never clip ground to +13 V.
A DMM average or camera cannot prove the absence of a brief full-output pulse.

| Test | Expected | Measured / pass / file |
| --- | --- | --- |
| Unconfigured / OE low | Both pads <0.1 V; LEDs off | |
| Off | OE low, both pads low | |
| 3300 K | Warm 6/256; cool 2/256; ~97.68 kHz | |
| 5000 K | Warm 3/256; cool 6/256; ~97.68 kHz | |
| Pulse integrity after 1 kΩ series resistors | Low near GND; high ≥0.88 × stock VDD even for ~80 ns pulse; no unexpected pulses | |
| Loaded 3300 / 5000 | Correct relative warmth; stock nominal 3% output | |
| Optical reference | Same fixed camera settings or light-meter positions | |
| Cold boot off / on | No excessive pulse; saved state resumes after startup | |
| XIAO RESET / USB reboot | Isolate during reset; saved state resumes | |
| Stock rail off/on with USB retained | Pads low while rail absent; valid PWM returns | |
| MCU supply off/on with stock rail retained | Pads low while MCU absent | |
| Deliberate watchdog test | Reset about 15 s; saved intent resumes | |
| Full supply interruption 10 times | No excessive pulse or lost pairing | |

The four profile attestations mean wiring, disabled-output isolation, startup/reset
behavior, and both nominal presets have been physically checked on this light.
They are operator statements, not automated measurements. Do not attest a failed
or skipped check. Optical adjustment, if required, needs a reviewed profile/code
change; current firmware deliberately rejects arbitrary duty edits.

## Network and Apple Home

Use an isolated test AP where possible. Record outage start, service return,
first successful control, counters and output behavior. Initial recovery target:
control restored within 60 seconds of usable network returning. Manual recovery
is failure. Label test-induced power cycles separately from unexpected resets.

| Test | Procedure | Result / seconds / logs |
| --- | --- | --- |
| Pair + cold boot | Pair in Home, switch presets, reboot; identity retained | |
| Closed housing | Wi-Fi + BLE commissioning with final antenna placement | |
| AP outage | Off for 30 s, 5 min and 1 hour, then restore | |
| Address change | Change test DHCP lease/address and restore routing | |
| Internet outage | Block WAN only; local light control stays available | |
| Controller restart | Restart Home hub/controller; correct state and control return | |
| Preset conflict | On1, On2, Off1: preset2 remains on; then Off2 | |
| Rapid commands | Alternate endpoints 100 times; then correct final state | |
| Timed command | If controller supports it, timed On ends at requested duration | |
| Two lights | Distinct MAC identities; pair/control/recover independently | |

## Seven-day soak

Use actual LED load, final housing and normal household networking. Capture daily
`status`, failures and any unsolicited reset. A recovered crash is still a defect.
After a material fix, repeat affected tests and restart the soak as appropriate.

| Day/date | Uptime/reset | Failures/recovery time | Output/temperature observation | Pass |
| --- | --- | --- | --- | --- |
| 1 | | | | |
| 2 | | | | |
| 3 | | | | |
| 4 | | | | |
| 5 | | | | |
| 6 | | | | |
| 7 | | | | |

- Remaining failures and corrective work:
- Final acceptance date/signature:
