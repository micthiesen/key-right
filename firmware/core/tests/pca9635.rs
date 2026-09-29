use key_right_core::pca9635::{
    output_frame, output_pwm_pair, stock_frame, stock_pwm_pair, Delay, DriverError, OutputEnable,
    Pca9635, RegisterBus, ADDRESS, MODE2, TEMPERATURES_MIRED,
};
use key_right_core::{
    ColorTemperature, Command, Controller, Level, LightOutput, LightState, OutputBrightness,
    OutputFrame, Preset,
};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, PartialEq)]
enum Event {
    Oe(bool),
    Write(Vec<u8>),
    Read,
    Delay(u32),
}
#[derive(Default)]
struct Hardware {
    registers: [u8; 24],
    events: Vec<Event>,
    fail_at: Option<usize>,
    operations: usize,
    fail_bus: bool,
    corrupt_read: bool,
    reset_after_write: bool,
}
#[derive(Clone)]
struct Mock(Rc<RefCell<Hardware>>);
impl Mock {
    fn step(h: &mut Hardware) -> Result<(), &'static str> {
        let operation = h.operations;
        h.operations += 1;
        if h.fail_at == Some(operation) {
            Err("injected")
        } else {
            Ok(())
        }
    }
}
impl RegisterBus for Mock {
    type Error = &'static str;
    fn write(&mut self, address: u8, bytes: &[u8]) -> Result<(), Self::Error> {
        assert_eq!(address, ADDRESS);
        let mut h = self.0.borrow_mut();
        h.events.push(Event::Write(bytes.to_vec()));
        Self::step(&mut h)?;
        if h.fail_bus {
            return Err("bus unavailable");
        }
        let start = usize::from(bytes[0] & 0x1f);
        h.registers[start..start + bytes.len() - 1].copy_from_slice(&bytes[1..]);
        if h.reset_after_write {
            h.registers.fill(0);
            h.reset_after_write = false;
        }
        Ok(())
    }
    fn write_read(&mut self, address: u8, bytes: &[u8], out: &mut [u8]) -> Result<(), Self::Error> {
        assert_eq!((address, bytes), (ADDRESS, &[0x80][..]));
        let mut h = self.0.borrow_mut();
        h.events.push(Event::Read);
        Self::step(&mut h)?;
        if h.fail_bus {
            return Err("bus unavailable");
        }
        out.copy_from_slice(&h.registers);
        out[0] |= 0x80;
        if h.corrupt_read {
            out[2] ^= 1;
        }
        Ok(())
    }
}
impl OutputEnable for Mock {
    type Error = &'static str;
    fn set_disabled(&mut self, disabled: bool) -> Result<(), Self::Error> {
        let mut h = self.0.borrow_mut();
        h.events.push(Event::Oe(disabled));
        Self::step(&mut h)
        // No physical effect: models an ineffective OE path as a fault case.
    }
}
impl Delay for Mock {
    fn delay_us(&mut self, micros: u32) {
        self.0.borrow_mut().events.push(Event::Delay(micros));
    }
}
fn fixture() -> (Rc<RefCell<Hardware>>, Pca9635<Mock, Mock, Mock>) {
    let h = Rc::new(RefCell::new(Hardware::default()));
    let driver = Pca9635::new(Mock(h.clone()), Mock(h.clone()), Mock(h.clone()));
    (h, driver)
}
#[test]
fn stock_frames_preserve_wrapper_channel_order_and_nominal_three_percent() {
    assert_eq!(
        (ADDRESS, MODE2, TEMPERATURES_MIRED),
        (0x15, 0x14, [303, 200])
    );
    for (state, pair) in [
        (LightState::default(), [0, 0]),
        (
            LightState {
                on: true,
                ..LightState::default()
            },
            [6, 2],
        ),
        (
            LightState {
                on: true,
                temperature: Preset::Two.temperature(),
                ..LightState::default()
            },
            [3, 6],
        ),
    ] {
        let frame = stock_frame(state);
        assert_eq!([frame[0], frame[4]], pair);
        assert_eq!(&frame[16..], &[255, 0, 0xaa, 0xaa, 0xaa, 0xaa]);
        for (channel, duty) in frame[..16].iter().enumerate() {
            if channel != 0 && channel != 4 {
                assert_eq!(*duty, 0);
            }
        }
    }
}
#[test]
fn initialization_writes_stock_off_before_waking_and_reads_it_back() {
    let (h, mut driver) = fixture();
    driver.initialize_off().unwrap();
    let h = h.borrow();
    assert_eq!(h.events[0], Event::Oe(true));
    assert_eq!(h.events[1], Event::Write(vec![1, MODE2]));
    assert_eq!(h.events[3], Event::Write(vec![0, 0]));
    assert_eq!(h.events[4], Event::Delay(500));
    assert_eq!(h.events[5], Event::Read);
    assert!(!h.events.contains(&Event::Oe(false)));
    assert_eq!(h.registers[2..], stock_frame(LightState::default()));
}
#[test]
fn on_is_read_back_before_oe_enable_and_off_writes_zero_even_without_oe() {
    let (h, mut driver) = fixture();
    driver
        .apply(LightState {
            on: true,
            temperature: Preset::Two.temperature(),
            ..LightState::default()
        })
        .unwrap();
    {
        let h = h.borrow();
        assert_eq!(
            &h.events[h.events.len() - 2..],
            &[Event::Read, Event::Oe(false)]
        );
    }
    driver.shutdown().unwrap();
    assert_eq!(
        h.borrow().registers[2..],
        stock_frame(LightState::default())
    );
}
#[test]
fn each_io_failure_invalidates_acknowledgement_and_retry_reinitializes() {
    for fail_at in 0..8 {
        let (h, mut driver) = fixture();
        h.borrow_mut().fail_at = Some(fail_at);
        let mut controller = Controller::new(None);
        controller.command(Command::SetPower(true));
        assert!(
            controller.reconcile(&mut driver).is_err(),
            "operation {fail_at}"
        );
        assert_eq!(controller.applied(), None);
        assert!(controller.intended().on);
        h.borrow_mut().fail_at = None;
        controller.reconcile(&mut driver).unwrap();
        assert_eq!(controller.applied(), Some(controller.intended()));
    }
}
#[test]
fn corrupted_readback_does_not_release_oe_or_claim_success() {
    let (h, mut driver) = fixture();
    h.borrow_mut().corrupt_read = true;
    assert_eq!(
        driver.apply(LightState {
            on: true,
            ..LightState::default()
        }),
        Err(DriverError::ReadbackMismatch)
    );
    assert!(!h.borrow().events.contains(&Event::Oe(false)));
}
#[test]
fn lost_bus_can_leave_previous_pwm_active_and_off_is_not_acknowledged() {
    let (h, mut driver) = fixture();
    let on = LightState {
        on: true,
        temperature: Preset::Two.temperature(),
        ..LightState::default()
    };
    let mut controller = Controller::new(Some(on));
    controller.reconcile(&mut driver).unwrap();
    h.borrow_mut().fail_bus = true;
    controller.command(Command::SetPower(false));
    assert!(controller.reconcile(&mut driver).is_err());
    assert!(driver.shutdown().is_err());
    assert_eq!(controller.applied(), None);
    assert_eq!(h.borrow().registers[2..], stock_frame(on));
    h.borrow_mut().fail_bus = false;
    controller.reconcile(&mut driver).unwrap();
    assert_eq!(
        h.borrow().registers[2..],
        stock_frame(LightState::default())
    );
}
#[test]
fn pca_reset_is_detected_and_each_apply_restores_full_configuration() {
    let (h, mut driver) = fixture();
    let state = LightState {
        on: true,
        temperature: Preset::Two.temperature(),
        ..LightState::default()
    };
    driver.apply(state).unwrap();
    h.borrow_mut().registers.fill(0);
    assert!(driver.verify(state).is_err());
    driver.apply(state).unwrap();
    assert_eq!(h.borrow().registers[1], MODE2);
    assert_eq!(h.borrow().registers[2..], stock_frame(state));
}

#[test]
fn low_light_range_is_bounded_and_monotonic_with_stock_temperature_direction() {
    for mired in 143..=344 {
        let temperature = ColorTemperature::new(mired).unwrap();
        let mut previous = [0, 0];
        for value in 1..=254 {
            let pair = stock_pwm_pair(Level::new(value).unwrap(), temperature);
            assert!(pair[0] >= previous[0] && pair[1] >= previous[1]);
            assert!(pair[0] <= 22 && pair[1] <= 22);
            assert!(pair[0] > 0 || pair[1] > 0);
            previous = pair;
        }
    }
    assert_eq!(stock_pwm_pair(Level::MIN, ColorTemperature::MIN), [0, 1]);
    assert_eq!(stock_pwm_pair(Level::MAX, ColorTemperature::MIN), [0, 22]);
    assert_eq!(stock_pwm_pair(Level::MIN, ColorTemperature::MAX), [1, 0]);
    assert_eq!(stock_pwm_pair(Level::MAX, ColorTemperature::MAX), [22, 0]);
    assert_eq!(
        stock_pwm_pair(Level::MAX, Preset::One.temperature()),
        [22, 9]
    );
    assert_eq!(
        stock_pwm_pair(Level::MAX, Preset::Two.temperature()),
        [12, 22]
    );
    // The stock mix reaches 100/100; it is not a constant total-duty budget.
    assert_eq!(
        stock_pwm_pair(Level::MAX, ColorTemperature::new(244).unwrap()),
        [22, 22]
    );
}

#[test]
fn changed_frame_is_verified_before_and_after_without_parking() {
    let (h, mut driver) = fixture();
    let on = LightState {
        on: true,
        ..LightState::default()
    };
    driver.apply(on).unwrap();
    h.borrow_mut().events.clear();
    let brighter = LightState {
        level: Level::MAX,
        ..on
    };
    driver.apply(brighter).unwrap();
    let h = h.borrow();
    assert_eq!(h.events.len(), 3);
    assert_eq!(h.events[0], Event::Read);
    assert!(
        matches!(&h.events[1], Event::Write(bytes) if bytes[0] == 0x82 && bytes[1..] == stock_frame(brighter))
    );
    assert_eq!(h.events[2], Event::Read);
}

#[test]
fn quantized_identical_frame_requires_only_readback_without_oe_glitches() {
    let (h, mut driver) = fixture();
    let on = LightState {
        on: true,
        ..LightState::default()
    };
    let adjacent = LightState {
        level: Level::new(58).unwrap(),
        ..on
    };
    assert_ne!(on, adjacent);
    assert_eq!(stock_frame(on), stock_frame(adjacent));
    driver.apply(on).unwrap();
    h.borrow_mut().events.clear();
    driver.apply(adjacent).unwrap();
    assert_eq!(h.borrow().events, [Event::Read]);
}

#[test]
fn reset_during_fast_path_recovers_full_configuration_before_enabling() {
    let (h, mut driver) = fixture();
    let on = LightState {
        on: true,
        ..LightState::default()
    };
    driver.apply(on).unwrap();
    {
        let mut h = h.borrow_mut();
        h.events.clear();
        h.registers.fill(0);
    }
    driver.apply(on).unwrap();
    let h = h.borrow();
    assert_eq!(&h.events[..2], &[Event::Read, Event::Oe(true)]);
    assert_eq!(
        &h.events[h.events.len() - 2..],
        &[Event::Read, Event::Oe(false)]
    );
    assert_eq!(h.registers[1], MODE2);
    assert_eq!(h.registers[2..], stock_frame(on));
}

#[test]
fn every_fast_path_bus_failure_disables_and_requires_full_retry() {
    for failure in 0..3 {
        let (h, mut driver) = fixture();
        let on = LightState {
            on: true,
            ..LightState::default()
        };
        let brighter = LightState {
            level: Level::MAX,
            ..on
        };
        driver.apply(on).unwrap();
        {
            let mut h = h.borrow_mut();
            h.fail_at = Some(h.operations + failure);
            h.events.clear();
        }
        assert_eq!(driver.apply(brighter), Err(DriverError::Bus("injected")));
        assert_eq!(h.borrow().events.last(), Some(&Event::Oe(true)));
        {
            let mut h = h.borrow_mut();
            h.fail_at = None;
            h.events.clear();
        }
        driver.apply(brighter).unwrap();
        assert_eq!(h.borrow().events[0], Event::Oe(true));
        assert_eq!(h.borrow().registers[2..], stock_frame(brighter));
    }
}

#[test]
fn reset_between_frame_write_and_readback_never_acknowledges_output() {
    let (h, mut driver) = fixture();
    let on = LightState {
        on: true,
        ..LightState::default()
    };
    let brighter = LightState {
        level: Level::MAX,
        ..on
    };
    driver.apply(on).unwrap();
    h.borrow_mut().reset_after_write = true;
    assert_eq!(driver.apply(brighter), Err(DriverError::ReadbackMismatch));
    assert_eq!(h.borrow().events.last(), Some(&Event::Oe(true)));
    h.borrow_mut().events.clear();
    driver.apply(brighter).unwrap();
    assert_eq!(h.borrow().events[0], Event::Oe(true));
    assert_eq!(h.borrow().registers[2..], stock_frame(brighter));
}

#[test]
fn off_and_shutdown_invalidate_fast_path_and_reinitialize() {
    for shutdown in [false, true] {
        let (h, mut driver) = fixture();
        let on = LightState {
            on: true,
            ..LightState::default()
        };
        driver.apply(on).unwrap();
        h.borrow_mut().events.clear();
        if shutdown {
            driver.shutdown().unwrap();
        } else {
            driver.apply(LightState { on: false, ..on }).unwrap();
        }
        assert_eq!(h.borrow().events[0], Event::Oe(true));
        assert_eq!(
            h.borrow().registers[2..],
            stock_frame(LightState::default())
        );
        h.borrow_mut().events.clear();
        driver.apply(on).unwrap();
        assert_eq!(h.borrow().events[0], Event::Oe(true));
    }
}

#[test]
fn physical_transition_brightness_reaches_zero_without_changing_settled_pwm() {
    for mired in 143..=344 {
        let temperature = ColorTemperature::new(mired).unwrap();
        assert_eq!(output_pwm_pair(OutputBrightness::ZERO, temperature), [0, 0]);
        let mut previous = [0, 0];
        for value in 0..=2530 {
            let brightness = OutputBrightness::new(value).unwrap();
            let pair = output_pwm_pair(brightness, temperature);
            assert!(pair[0] >= previous[0] && pair[1] >= previous[1]);
            assert!(pair[0] <= 22 && pair[1] <= 22);
            previous = pair;
        }
        for value in 1..=254 {
            let level = Level::new(value).unwrap();
            let state = LightState {
                on: true,
                level,
                temperature,
            };
            assert_eq!(
                output_frame(OutputFrame::from_state(state)),
                stock_frame(state)
            );
        }
    }
    let warmest = ColorTemperature::MAX;
    assert_eq!(
        output_pwm_pair(OutputBrightness::new(100).unwrap(), warmest),
        [0, 0]
    );
    assert_eq!(
        output_pwm_pair(OutputBrightness::new(200).unwrap(), warmest),
        [1, 0]
    );
    assert_eq!(
        output_pwm_pair(
            OutputBrightness::from_level(Level::DEFAULT),
            ColorTemperature::DEFAULT
        ),
        [6, 2]
    );
    assert_eq!(
        output_pwm_pair(
            OutputBrightness::from_level(Level::DEFAULT),
            Preset::Two.temperature()
        ),
        [3, 6]
    );
}

#[test]
fn off_frame_suppresses_even_inconsistent_nonzero_brightness() {
    let frame = OutputFrame {
        state: LightState::default(),
        brightness: OutputBrightness::MAX,
    };
    assert_eq!(output_frame(frame), stock_frame(LightState::default()));
    let (h, mut driver) = fixture();
    driver.apply_frame(frame).unwrap();
    assert!(!h.borrow().events.contains(&Event::Oe(false)));
    assert_eq!(
        h.borrow().registers[2..],
        stock_frame(LightState::default())
    );
}

#[test]
fn subfloor_frame_readback_uses_exact_brightness_and_not_logical_level() {
    let (h, mut driver) = fixture();
    let state = LightState {
        on: true,
        level: Level::MIN,
        temperature: ColorTemperature::MAX,
    };
    let frame = OutputFrame {
        state,
        brightness: OutputBrightness::new(100).unwrap(),
    };
    driver.apply_frame(frame).unwrap();
    assert_eq!([h.borrow().registers[2], h.borrow().registers[6]], [0, 0]);
    driver.verify_frame(frame).unwrap();
    assert_eq!(driver.verify(state), Err(DriverError::ReadbackMismatch));
    driver.apply_frame(frame).unwrap();
    driver.verify_frame(frame).unwrap();
}

#[test]
fn quantized_transition_changes_only_read_back_without_output_or_oe_writes() {
    let (h, mut driver) = fixture();
    let state = LightState {
        on: true,
        level: Level::MIN,
        temperature: ColorTemperature::MAX,
    };
    let first = OutputFrame {
        state,
        brightness: OutputBrightness::new(200).unwrap(),
    };
    let next = OutputFrame {
        brightness: OutputBrightness::new(201).unwrap(),
        ..first
    };
    assert_eq!(output_frame(first), output_frame(next));
    let mut controller = Controller::new(Some(state));
    controller.reconcile_frame(&mut driver, first).unwrap();
    h.borrow_mut().events.clear();
    assert_eq!(controller.reconcile_frame(&mut driver, next), Ok(true));
    assert_eq!(controller.applied_frame(), Some(next));
    assert_eq!(h.borrow().events, [Event::Read]);
    h.borrow_mut().events.clear();
    assert_eq!(controller.reconcile_frame(&mut driver, next), Ok(false));
    assert!(h.borrow().events.is_empty());
}

#[test]
fn failed_subfloor_write_verification_cannot_acknowledge_a_frame() {
    let (h, mut driver) = fixture();
    let target = LightState {
        on: true,
        ..LightState::default()
    };
    let mut controller = Controller::new(Some(target));
    controller.reconcile(&mut driver).unwrap();
    let frame = OutputFrame {
        state: LightState {
            level: Level::MIN,
            ..target
        },
        brightness: OutputBrightness::new(100).unwrap(),
    };
    h.borrow_mut().reset_after_write = true;
    assert_eq!(
        controller.reconcile_frame(&mut driver, frame),
        Err(DriverError::ReadbackMismatch)
    );
    assert_eq!(controller.applied(), None);
    assert_eq!(controller.applied_frame(), None);
    assert_eq!(controller.intended(), target);
    assert_eq!(h.borrow().events.last(), Some(&Event::Oe(true)));
    controller.reconcile_frame(&mut driver, frame).unwrap();
    assert_eq!(controller.applied_frame(), Some(frame));
    assert_eq!(h.borrow().registers[2..], output_frame(frame));
}
