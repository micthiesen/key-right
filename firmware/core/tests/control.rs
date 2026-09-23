use key_right_core::{Command, Controller, LightOutput, LightState, Preset};

#[derive(Default)]
struct Output {
    fail: bool,
    writes: usize,
    last_attempt: Option<LightState>,
}

impl LightOutput for Output {
    type Error = &'static str;

    fn apply(&mut self, state: LightState) -> Result<(), Self::Error> {
        self.writes += 1;
        self.last_attempt = Some(state);
        if self.fail {
            Err("I2C write failed")
        } else {
            Ok(())
        }
    }
}

#[test]
fn first_boot_requests_off_without_claiming_known_output() {
    let mut controller = Controller::new(None);
    assert_eq!(controller.intended(), LightState::default());
    assert_eq!(controller.applied(), None);

    let mut output = Output::default();
    assert_eq!(controller.reconcile(&mut output), Ok(true));
    assert_eq!(controller.applied(), Some(LightState::default()));
    assert_eq!(output.last_attempt.unwrap().brightness_percent(), 0);
}

#[test]
fn restored_intent_is_reapplied_on_boot() {
    let restored = LightState {
        on: true,
        preset: Preset::Two,
    };
    let mut controller = Controller::new(Some(restored));
    assert_eq!(controller.intended(), restored);
    assert_eq!(controller.applied(), None);

    let mut output = Output::default();
    controller.reconcile(&mut output).unwrap();
    assert_eq!(output.last_attempt, Some(restored));
    assert_eq!(controller.applied().unwrap().brightness_percent(), 3);
}

#[test]
fn preset_survives_off_and_on_without_changing_fixed_brightness() {
    let mut controller = Controller::new(None);
    assert!(controller.command(Command::SelectPreset(Preset::Two)));
    assert!(!controller.intended().on);
    assert!(controller.command(Command::SetPower(true)));
    assert_eq!(controller.intended().brightness_percent(), 3);
    assert!(controller.command(Command::SetPower(false)));
    assert_eq!(controller.intended().brightness_percent(), 0);
    assert!(controller.command(Command::SetPower(true)));
    assert_eq!(controller.intended().preset, Preset::Two);
    assert_eq!(controller.intended().brightness_percent(), 3);
}

#[test]
fn every_brightness_write_preserves_power_preset_and_output() {
    for on in [false, true] {
        for preset in [Preset::One, Preset::Two] {
            let state = LightState { on, preset };
            let mut controller = Controller::new(Some(state));
            let mut output = Output::default();
            controller.reconcile(&mut output).unwrap();

            for value in 0..=u8::MAX {
                assert!(!controller.command(Command::SetBrightness(value)));
                assert_eq!(controller.intended(), state);
                assert_eq!(controller.reconcile(&mut output), Ok(false));
                assert_eq!(controller.applied(), Some(state));
            }
            assert_eq!(output.writes, 1);
        }
    }
}

#[test]
fn repeated_commands_do_not_request_persistence_or_output_writes() {
    let mut controller = Controller::new(None);
    let mut output = Output::default();
    controller.reconcile(&mut output).unwrap();
    assert!(!controller.command(Command::SetPower(false)));
    assert!(!controller.command(Command::SelectPreset(Preset::One)));
    assert_eq!(controller.reconcile(&mut output), Ok(false));
    assert_eq!(output.writes, 1);
}

#[test]
fn pending_intent_does_not_claim_to_be_applied() {
    let mut controller = Controller::new(None);
    let mut output = Output::default();
    controller.reconcile(&mut output).unwrap();

    controller.command(Command::SetPower(true));
    assert!(controller.intended().on);
    assert!(!controller.applied().unwrap().on);
    controller.reconcile(&mut output).unwrap();
    assert!(controller.applied().unwrap().on);
}

#[test]
fn partial_write_failure_invalidates_acknowledgement_and_can_retry() {
    let mut controller = Controller::new(None);
    let mut output = Output::default();
    controller.reconcile(&mut output).unwrap();
    controller.command(Command::SetPower(true));
    controller.command(Command::SelectPreset(Preset::Two));

    output.fail = true;
    assert_eq!(controller.reconcile(&mut output), Err("I2C write failed"));
    assert_eq!(controller.applied(), None);
    assert!(controller.intended().on);
    assert_eq!(controller.intended().preset, Preset::Two);

    output.fail = false;
    assert_eq!(controller.reconcile(&mut output), Ok(true));
    assert_eq!(controller.applied(), Some(controller.intended()));
    assert_eq!(output.writes, 3);
}
