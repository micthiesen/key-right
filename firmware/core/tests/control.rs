use key_right_core::{
    ColorTemperature, Command, Controller, Level, LightOutput, LightState, OutputBrightness,
    OutputFrame, Preset,
};

#[derive(Default)]
struct Output {
    fail: bool,
    writes: usize,
    last_attempt: Option<LightState>,
    last_frame: Option<OutputFrame>,
}

impl LightOutput for Output {
    type Error = &'static str;

    fn apply_frame(&mut self, frame: OutputFrame) -> Result<(), Self::Error> {
        self.writes += 1;
        self.last_attempt = Some(frame.state);
        self.last_frame = Some(frame);
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

#[test]
fn physical_brightness_covers_zero_and_round_trips_all_settled_levels() {
    assert_eq!(OutputBrightness::ZERO.get(), 0);
    assert_eq!(OutputBrightness::ZERO.to_level_clamped(), Level::MIN);
    assert_eq!(OutputBrightness::MAX.get(), 2530);
    for value in 0..=u16::MAX {
        assert_eq!(
            OutputBrightness::new(value).map(OutputBrightness::get),
            (value <= 2530).then_some(value)
        );
    }
    for value in 1..=254 {
        let level = Level::new(value).unwrap();
        let brightness = OutputBrightness::from_level(level);
        assert_eq!(brightness.to_level_clamped(), level);
        assert_eq!(
            (
                u32::from(brightness.get()),
                u32::from(OutputBrightness::DENOMINATOR)
            ),
            level.stock_brightness_ratio()
        );
    }
    for value in 0..=2530 {
        let brightness = OutputBrightness::new(value).unwrap();
        let nearest = OutputBrightness::from_level(brightness.to_level_clamped());
        if value < 253 {
            assert_eq!(nearest, OutputBrightness::from_level(Level::MIN));
        } else {
            assert!(value.abs_diff(nearest.get()) <= 4);
        }
    }
}

#[test]
fn subfloor_frame_is_acknowledged_exactly_without_changing_target() {
    let target = LightState {
        on: true,
        ..LightState::default()
    };
    let mut controller = Controller::new(Some(target));
    let mut output = Output::default();
    let frame = OutputFrame {
        state: LightState {
            level: Level::MIN,
            ..target
        },
        brightness: OutputBrightness::new(200).unwrap(),
    };
    assert_eq!(controller.reconcile_frame(&mut output, frame), Ok(true));
    assert_eq!(controller.intended(), target);
    assert_eq!(controller.applied(), Some(frame.state));
    assert_eq!(controller.applied_frame(), Some(frame));
    assert_eq!(output.last_frame, Some(frame));
    assert_eq!(controller.reconcile_frame(&mut output, frame), Ok(false));
    assert_eq!(output.writes, 1);

    let next = OutputFrame {
        brightness: OutputBrightness::new(201).unwrap(),
        ..frame
    };
    assert_eq!(controller.reconcile_frame(&mut output, next), Ok(true));
    assert_eq!(controller.applied_frame(), Some(next));
    controller.invalidate_applied();
    assert_eq!(controller.applied_frame(), None);
    assert_eq!(controller.applied(), None);
    assert_eq!(controller.intended(), target);
}

#[test]
fn failed_custom_frame_clears_both_acknowledgements_and_preserves_target() {
    let target = LightState {
        on: true,
        ..LightState::default()
    };
    let mut controller = Controller::new(Some(target));
    let mut output = Output::default();
    controller.reconcile(&mut output).unwrap();
    let frame = OutputFrame {
        state: LightState {
            level: Level::MIN,
            ..target
        },
        brightness: OutputBrightness::new(200).unwrap(),
    };
    output.fail = true;
    assert_eq!(
        controller.reconcile_frame(&mut output, frame),
        Err("I2C write failed")
    );
    assert_eq!(controller.applied(), None);
    assert_eq!(controller.applied_frame(), None);
    assert_eq!(controller.intended(), target);
    output.fail = false;
    controller.reconcile_frame(&mut output, frame).unwrap();
    assert_eq!(controller.applied_frame(), Some(frame));
}
