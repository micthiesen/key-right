use key_right_core::{
    ColorTemperature, Command, Controller, Level, LightOutput, LightState, Preset,
};

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
    assert!(!output.last_attempt.unwrap().on);
    assert_eq!(controller.intended().level, Level::DEFAULT);
    assert_eq!(controller.intended().temperature, ColorTemperature::DEFAULT);
}

#[test]
fn restored_intent_is_reapplied_on_boot() {
    let restored = LightState {
        on: true,
        level: Level::MAX,
        temperature: Preset::Two.temperature(),
    };
    let mut controller = Controller::new(Some(restored));
    assert_eq!(controller.intended(), restored);
    assert_eq!(controller.applied(), None);

    let mut output = Output::default();
    controller.reconcile(&mut output).unwrap();
    assert_eq!(output.last_attempt, Some(restored));
    assert_eq!(controller.applied().unwrap().level, Level::MAX);
}

#[test]
fn level_and_temperature_survive_off_and_on() {
    let mut controller = Controller::new(None);
    assert!(controller.command(Command::SelectPreset(Preset::Two)));
    assert!(!controller.intended().on);
    assert!(controller.command(Command::SetPower(true)));
    assert!(controller.command(Command::SetLevel(Level::MAX)));
    assert!(controller.command(Command::SetPower(false)));
    assert!(!controller.intended().on);
    assert!(controller.command(Command::SetPower(true)));
    assert_eq!(controller.intended().temperature, Preset::Two.temperature());
    assert_eq!(controller.intended().level, Level::MAX);
}

#[test]
fn level_and_temperature_writes_preserve_power_and_apply_new_settings() {
    for on in [false, true] {
        let mut controller = Controller::new(Some(LightState {
            on,
            ..LightState::default()
        }));
        let mut output = Output::default();
        controller.reconcile(&mut output).unwrap();
        assert!(controller.command(Command::SetLevel(Level::MIN)));
        assert!(controller.command(Command::SetTemperature(ColorTemperature::MIN)));
        assert_eq!(controller.intended().on, on);
        assert_eq!(controller.reconcile(&mut output), Ok(true));
        assert_eq!(controller.applied(), Some(controller.intended()));
        assert_eq!(output.writes, 2);
    }
}

#[test]
fn validated_types_reject_out_of_range_values() {
    for value in 0..=u8::MAX {
        assert_eq!(
            Level::new(value).map(Level::get),
            (1..=254).contains(&value).then_some(value)
        );
    }
    for value in 0..=u16::MAX {
        assert_eq!(
            ColorTemperature::new(value).map(ColorTemperature::get),
            (143..=344).contains(&value).then_some(value)
        );
    }
    assert_eq!(Level::MIN.stock_brightness_ratio(), (253, 253));
    assert_eq!(Level::MAX.stock_brightness_ratio(), (2530, 253));
    assert_eq!(Level::DEFAULT.stock_brightness_ratio(), (757, 253));
}

#[test]
fn repeated_commands_do_not_request_persistence_or_output_writes() {
    let mut controller = Controller::new(None);
    let mut output = Output::default();
    controller.reconcile(&mut output).unwrap();
    assert!(!controller.command(Command::SetPower(false)));
    assert!(!controller.command(Command::SelectPreset(Preset::One)));
    assert!(!controller.command(Command::SetLevel(Level::DEFAULT)));
    assert!(!controller.command(Command::SetTemperature(ColorTemperature::DEFAULT)));
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
    assert_eq!(controller.intended().temperature, Preset::Two.temperature());

    output.fail = false;
    assert_eq!(controller.reconcile(&mut output), Ok(true));
    assert_eq!(controller.applied(), Some(controller.intended()));
    assert_eq!(output.writes, 3);
}
