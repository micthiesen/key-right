use key_right_core::pca9635::{
    stock_frame, Delay, DriverError, OutputEnable, Pca9635, RegisterBus, ADDRESS, MODE2,
    TEMPERATURES_MIRED,
};
use key_right_core::{Command, Controller, LightOutput, LightState, Preset};
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
        // No physical effect: models the permitted OE-tied-low installation.
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
                preset: Preset::One,
            },
            [6, 2],
        ),
        (
            LightState {
                on: true,
                preset: Preset::Two,
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
            preset: Preset::Two,
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
            preset: Preset::One
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
        preset: Preset::Two,
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
        preset: Preset::Two,
    };
    driver.apply(state).unwrap();
    h.borrow_mut().registers.fill(0);
    assert!(driver.verify(state).is_err());
    driver.apply(state).unwrap();
    assert_eq!(h.borrow().registers[1], MODE2);
    assert_eq!(h.borrow().registers[2..], stock_frame(state));
}
