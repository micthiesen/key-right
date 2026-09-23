use std::cell::RefCell;
use std::rc::Rc;

use key_right_core::pca9635::{
    crc16, Delay, DriverError, Frame, OutputGate, Pca9635, Profile, ProfileError, RegisterBus,
    FRAME_LEN, REQUIRED_ATTESTATIONS,
};
use key_right_core::{Command, Controller, LightOutput, LightState, Preset};

fn profile(attestations: u8) -> Profile {
    let mut off = [0; FRAME_LEN];
    off[16] = 255;
    off[18..].fill(0xaa);
    let mut one = off;
    one[0] = 4;
    one[4] = 2;
    let mut two = off;
    two[0] = 1;
    two[4] = 5;
    // Deliberately synthetic fixture, never a production calibration.
    Profile::new(
        0x15,
        0x14,
        [303, 200],
        [Frame(off), Frame(one), Frame(two)],
        attestations,
    )
    .unwrap()
}

#[derive(Debug, PartialEq)]
enum Event {
    Gate(bool),
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
    corrupt_read: bool,
    disabled: bool,
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
        assert_eq!(address, 0x15);
        let mut h = self.0.borrow_mut();
        h.events.push(Event::Write(bytes.to_vec()));
        Self::step(&mut h)?;
        let start = usize::from(bytes[0] & 0x1f);
        h.registers[start..start + bytes.len() - 1].copy_from_slice(&bytes[1..]);
        Ok(())
    }
    fn write_read(&mut self, address: u8, bytes: &[u8], out: &mut [u8]) -> Result<(), Self::Error> {
        assert_eq!((address, bytes), (0x15, &[0x80][..]));
        let mut h = self.0.borrow_mut();
        h.events.push(Event::Read);
        Self::step(&mut h)?;
        out.copy_from_slice(&h.registers);
        out[0] |= 0x80;
        if h.corrupt_read {
            out[2] ^= 1;
        }
        Ok(())
    }
}

impl OutputGate for Mock {
    type Error = &'static str;
    fn set_disabled(&mut self, disabled: bool) -> Result<(), Self::Error> {
        let mut h = self.0.borrow_mut();
        h.events.push(Event::Gate(disabled));
        Self::step(&mut h)?;
        h.disabled = disabled;
        Ok(())
    }
}

impl Delay for Mock {
    fn delay_us(&mut self, micros: u32) {
        self.0.borrow_mut().events.push(Event::Delay(micros));
    }
}

fn fixture() -> (Rc<RefCell<Hardware>>, Pca9635<Mock, Mock, Mock>) {
    let h = Rc::new(RefCell::new(Hardware::default()));
    let driver = Pca9635::new(
        Mock(h.clone()),
        Mock(h.clone()),
        Mock(h.clone()),
        profile(15),
    );
    (h, driver)
}

#[test]
fn profile_roundtrip_and_every_single_bit_corruption_is_rejected() {
    let expected = profile(REQUIRED_ATTESTATIONS);
    let bytes = expected.encode();
    assert_eq!(Profile::decode(&bytes), Ok(expected));
    assert_eq!(crc16(b"123456789"), 0x29b1);
    for i in 0..bytes.len() {
        for bit in 0..8 {
            let mut corrupt = bytes;
            corrupt[i] ^= 1 << bit;
            assert!(Profile::decode(&corrupt).is_err(), "byte {i}, bit {bit}");
        }
    }
    assert_eq!(Profile::decode(&bytes[..79]), Err(ProfileError::Length));
}

#[test]
fn stock_candidate_preserves_real_wrapper_warm_cool_order() {
    let p = Profile::stock_candidate();
    assert_eq!((p.address(), p.mode2()), (0x15, 0x14));
    assert_eq!(p.temperatures_mired(), [303, 200]);
    assert_eq!((p.frames()[1].0[0], p.frames()[1].0[4]), (6, 2));
    assert_eq!((p.frames()[2].0[0], p.frames()[2].0[4]), (3, 6));
    for frame in p.frames() {
        assert_eq!(&frame.0[18..], &[0xaa; 4]);
        for channel in 0..16 {
            if channel != 0 && channel != 4 {
                assert_eq!(frame.0[channel], 0);
            }
        }
    }
    assert_eq!(p.attestations(), 0);
    assert!(p.require_commissioned().is_err());
    assert_eq!(p.stock_pwm_duties(LightState::default()), Ok([0, 0]));
    assert_eq!(
        p.stock_pwm_duties(LightState {
            on: true,
            preset: Preset::One
        }),
        Ok([6, 2])
    );
    assert_eq!(
        p.stock_pwm_duties(LightState {
            on: true,
            preset: Preset::Two
        }),
        Ok([3, 6])
    );
    assert!(p.with_attestations(15).unwrap().require_stock_pwm().is_ok());
    assert_eq!(
        profile(15).require_stock_pwm(),
        Err(ProfileError::UnsupportedProfile)
    );
}

#[test]
fn candidates_cannot_be_mistaken_for_commissioned_profiles() {
    for flags in 0..15 {
        assert!(profile(flags).require_commissioned().is_err());
    }
    assert!(profile(15).require_commissioned().is_ok());
    assert!(profile(0).with_attestations(16).is_err());
    assert_eq!(
        Profile::decode(&profile(0).encode())
            .unwrap()
            .attestations(),
        0
    );
}

#[test]
fn reserved_bus_modes_and_uncalibrated_presets_are_rejected() {
    let p = profile(15);
    for address in [0, 3, 7, 0x70, 0x78, 0xff] {
        assert_eq!(
            Profile::new(address, p.mode2(), [303, 200], *p.frames(), 15),
            Err(ProfileError::Address)
        );
    }
    for mode in [3, 0x1c, 0x34, 0x94] {
        assert_eq!(
            Profile::new(0x15, mode, [303, 200], *p.frames(), 15),
            Err(ProfileError::Mode)
        );
    }
    assert_eq!(
        Profile::new(0x15, 0x14, [303, 303], *p.frames(), 15),
        Err(ProfileError::Temperature)
    );
    let mut frames = *p.frames();
    frames[2] = frames[1];
    assert_eq!(
        Profile::new(0x15, 0x14, [303, 200], frames, 15),
        Err(ProfileError::IdenticalPresets)
    );
}

#[test]
fn initialization_wakes_before_pwm_and_never_releases_gate() {
    let (h, mut driver) = fixture();
    driver.initialize_disabled().unwrap();
    let h = h.borrow();
    assert!(h.disabled);
    assert_eq!(
        &h.events[..4],
        &[
            Event::Gate(true),
            Event::Write(vec![0, 0]),
            Event::Delay(500),
            Event::Write(vec![1, 0x14])
        ]
    );
    assert!(!h.events.contains(&Event::Gate(false)));
    assert_eq!(h.registers[2..], profile(15).frames()[0].0);
}

#[test]
fn enable_only_after_complete_readback_and_off_keeps_gate_asserted() {
    let (h, mut driver) = fixture();
    let on = LightState {
        on: true,
        preset: Preset::Two,
    };
    driver.apply(on).unwrap();
    {
        let h = h.borrow();
        assert_eq!(
            &h.events[h.events.len() - 2..],
            &[Event::Read, Event::Gate(false)]
        );
        assert_eq!(h.registers[2..], profile(15).frames()[2].0);
    }
    driver.apply(LightState::default()).unwrap();
    assert!(h.borrow().disabled);
    assert_eq!(h.borrow().registers[2..], profile(15).frames()[0].0);
}

#[test]
fn every_bus_failure_keeps_intent_unknown_and_retry_reinitializes() {
    // Successful first application consists of 9 fallible operations. Failure
    // at initial gate assertion is covered too; it cannot claim lamp-off.
    for fail_at in 0..9 {
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
        if fail_at != 0 {
            assert!(h.borrow().disabled);
        }
        h.borrow_mut().fail_at = None;
        controller.reconcile(&mut driver).unwrap();
        assert_eq!(controller.applied(), Some(controller.intended()));
    }
}

#[test]
fn corrupted_readback_never_enables_output() {
    let (h, mut driver) = fixture();
    h.borrow_mut().corrupt_read = true;
    assert_eq!(
        driver.apply(LightState {
            on: true,
            preset: Preset::One
        }),
        Err(DriverError::ReadbackMismatch)
    );
    assert!(h.borrow().disabled);
    assert!(!h.borrow().events.contains(&Event::Gate(false)));
}

#[test]
fn hardware_reset_is_detected_and_reconciled_without_losing_intent() {
    let (h, mut driver) = fixture();
    let mut controller = Controller::new(Some(LightState {
        on: true,
        preset: Preset::Two,
    }));
    controller.reconcile(&mut driver).unwrap();
    h.borrow_mut().registers.fill(0);
    assert!(driver.verify(controller.intended()).is_err());
    driver.shutdown().unwrap();
    controller.invalidate_applied();
    controller.reconcile(&mut driver).unwrap();
    assert_eq!(h.borrow().registers[2..], profile(15).frames()[2].0);
}
