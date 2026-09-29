use crate::runtime::{Fault, Hardware, OutputError, Runtime, INTENT_KEY};
use key_right_core::{ColorTemperature, Level, LightOutput, LightState, OutputFrame};
use rs_matter::error::{Error, ErrorCode};
use rs_matter::persist::{KvBlobStore, KvBlobStoreAccess, KV_BUF_SIZE};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
#[derive(Default)]
struct Rig {
    records: RefCell<BTreeMap<u16, Vec<u8>>>,
    events: RefCell<Vec<String>>,
    fail_store: Cell<bool>,
    fail_load: Cell<bool>,
    load_calls: Cell<u32>,
    fail_output: Cell<bool>,
    actual: Cell<Option<LightState>>,
    actual_frame: Cell<Option<OutputFrame>>,
}
struct Store(Rc<Rig>);
struct Access(Rc<Rig>);
impl KvBlobStore for Store {
    fn load<'a>(&mut self, key: u16, buf: &'a mut [u8]) -> Result<Option<&'a [u8]>, Error> {
        self.0.load_calls.set(self.0.load_calls.get() + 1);
        if self.0.fail_load.get() {
            return Err(ErrorCode::Failure.into());
        }
        Ok(self.0.records.borrow().get(&key).map(|data| {
            buf[..data.len()].copy_from_slice(data);
            &buf[..data.len()]
        }))
    }
    fn store(&mut self, key: u16, data: &[u8], _: &mut [u8]) -> Result<(), Error> {
        if self.0.fail_store.get() {
            return Err(ErrorCode::Failure.into());
        }
        self.0.events.borrow_mut().push(format!("store:{key}"));
        self.0.records.borrow_mut().insert(key, data.to_vec());
        Ok(())
    }
    fn remove(&mut self, _: u16, _: &mut [u8]) -> Result<(), Error> {
        panic!("no automatic erase")
    }
}
impl KvBlobStoreAccess for Access {
    fn access<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut dyn KvBlobStore, &mut [u8]) -> R,
    {
        f(&mut Store(self.0.clone()), &mut [0; KV_BUF_SIZE])
    }
}
struct Output(Rc<Rig>);
impl LightOutput for Output {
    type Error = OutputError;
    fn apply_frame(&mut self, frame: OutputFrame) -> Result<(), OutputError> {
        self.0.events.borrow_mut().push(format!("apply:{frame:?}"));
        if self.0.fail_output.get() {
            return Err(OutputError::Bus);
        }
        self.0.actual.set(Some(frame.state));
        self.0.actual_frame.set(Some(frame));
        Ok(())
    }
}
impl Hardware for Output {
    fn shutdown(&mut self) -> Result<(), OutputError> {
        self.0.events.borrow_mut().push("off".into());
        if self.0.fail_output.get() {
            return Err(OutputError::Bus);
        }
        self.0.actual.set(Some(LightState::default()));
        self.0.actual_frame.set(Some(OutputFrame::default()));
        Ok(())
    }
    fn verify_frame(&mut self, frame: OutputFrame) -> Result<(), OutputError> {
        if self.0.fail_output.get()
            || self.0.actual.get() != Some(frame.state)
            || self.0.actual_frame.get() != Some(frame)
        {
            Err(OutputError::Readback)
        } else {
            Ok(())
        }
    }
    fn registers(&mut self) -> Result<[u8; 24], OutputError> {
        if self.0.fail_output.get() {
            Err(OutputError::Bus)
        } else {
            Ok([0; 24])
        }
    }
}
fn rig() -> (Rc<Rig>, Runtime<Access, Output>) {
    let h = Rc::new(Rig::default());
    let r = Runtime::load(Access(h.clone()), Output(h.clone()));
    (h, r)
}

#[test]
fn first_boot_is_off_with_about_three_percent_and_3300k() {
    let (h, r) = rig();
    r.tick(0);
    assert_eq!(r.acknowledged().unwrap(), LightState::default());
    assert!(h.records.borrow().is_empty());
    let frame = key_right_core::pca9635::stock_frame(LightState {
        on: true,
        ..LightState::default()
    });
    assert_eq!([frame[0], frame[4]], [6, 2]);
}

#[test]
fn reported_destination_is_stable_while_acknowledged_frames_change() {
    let (_, r) = rig();
    assert!(r.reported().is_err());
    r.tick(0);
    r.set_power(true).unwrap();
    r.set_level_transition(254, false, 2000).unwrap();
    r.set_temperature_transition(ColorTemperature::MIN, 2000)
        .unwrap();
    let target = r.snapshot().intended;
    let revision = r.snapshot().revision;
    assert_eq!(r.reported().unwrap(), target);
    assert_ne!(r.acknowledged().unwrap(), target);
    r.tick(1000);
    assert_eq!(r.reported().unwrap(), target);
    assert_ne!(r.acknowledged().unwrap(), target);
    assert_eq!(r.snapshot().revision, revision);
    let frame = r.acknowledged().unwrap();
    r.stop_level_transition().unwrap();
    assert_eq!(r.reported().unwrap().level, frame.level);
    assert_eq!(r.reported().unwrap().temperature, ColorTemperature::MIN);
    r.stop_temperature_transition().unwrap();
    assert_eq!(r.reported().unwrap(), frame);
}

#[test]
fn reported_targets_remain_unavailable_after_failed_storage_output_or_reboot() {
    let (h, r) = rig();
    r.tick(0);
    h.fail_store.set(true);
    assert!(r.set_power(true).is_err());
    assert!(r.reported().is_err());
    h.fail_store.set(false);
    r.tick(5000);
    assert!(r.reported().unwrap().on);
    r.tick(5400);
    h.fail_output.set(true);
    r.set_power(false).unwrap();
    // Starting the fade retains the last frame; the next changed frame
    // detects the injected I/O failure and invalidates reported attributes.
    r.tick(5420);
    assert!(r.reported().is_err());
    h.fail_output.set(false);
    r.tick(10420);
    assert!(!r.reported().unwrap().on);
    let revision = r.snapshot().revision;
    assert!(r.prepare_reboot());
    assert!(r.reported().is_err());
    assert_ne!(r.snapshot().revision, revision);
}

#[test]
fn state_is_saved_before_io_and_repeated_commands_do_not_write_flash() {
    let (h, r) = rig();
    r.tick(0);
    h.events.borrow_mut().clear();
    r.set_power(true).unwrap();
    assert_eq!(h.events.borrow()[0], format!("store:{INTENT_KEY}"));
    assert!(h.events.borrow()[1].starts_with("apply:"));
    h.events.borrow_mut().clear();
    r.set_power(true).unwrap();
    assert!(h.events.borrow().is_empty());
    r.set_level(Level::MAX, false).unwrap();
    r.set_temperature(ColorTemperature::MIN).unwrap();
    let intended = r.snapshot().intended;
    let reboot = Runtime::load(Access(h.clone()), Output(h.clone()));
    reboot.tick(0);
    reboot.tick(400);
    assert_eq!(reboot.acknowledged().unwrap(), intended);
}

#[test]
fn absent_pca_preserves_commands_for_bounded_recovery() {
    let (h, _) = rig();
    h.fail_output.set(true);
    let r = Runtime::load(Access(h.clone()), Output(h.clone()));
    assert_eq!(r.snapshot().fault, Fault::Output);
    assert!(r.set_power(true).is_err());
    assert!(r.snapshot().intended.on);
    h.fail_output.set(false);
    r.tick(4999);
    assert!(r.acknowledged().is_err());
    r.tick(5000);
    assert!(!r.acknowledged().unwrap().on);
    r.tick(5400);
    assert!(r.acknowledged().unwrap().on);
}

#[test]
fn failed_save_never_applies_pending_on_and_retries_it() {
    let (h, r) = rig();
    r.tick(0);
    h.fail_store.set(true);
    assert!(r.set_power(true).is_err());
    assert_eq!(r.snapshot().fault, Fault::Storage);
    assert!(r.acknowledged().is_err());
    assert!(!h.actual.get().unwrap().on);
    h.fail_store.set(false);
    r.tick(4999);
    assert!(r.acknowledged().is_err());
    r.tick(5000);
    assert!(!r.acknowledged().unwrap().on);
    r.tick(5400);
    assert!(r.acknowledged().unwrap().on);
}

#[test]
fn failed_off_can_leave_physical_on_without_acknowledging_off() {
    let (h, r) = rig();
    r.tick(0);
    r.set_power(true).unwrap();
    r.tick(400);
    h.fail_output.set(true);
    assert!(r.off().is_err());
    assert!(!r.snapshot().intended.on);
    assert!(r.acknowledged().is_err());
    assert!(h.actual.get().unwrap().on);
    h.fail_output.set(false);
    r.tick(5400);
    assert!(!r.acknowledged().unwrap().on);
}

#[test]
fn periodic_readback_failure_marks_unknown_then_restores_all_settings() {
    let (h, r) = rig();
    r.tick(0);
    r.set_power(true).unwrap();
    r.set_level(Level::MAX, false).unwrap();
    r.set_temperature(ColorTemperature::MIN).unwrap();
    let intended = r.snapshot().intended;
    r.tick(400);
    h.actual.set(None);
    h.actual_frame.set(None);
    r.tick(5400);
    assert!(r.acknowledged().is_err());
    r.tick(10399);
    assert!(r.acknowledged().is_err());
    r.tick(10400);
    assert!(!r.acknowledged().unwrap().on);
    r.tick(10800);
    assert_eq!(r.acknowledged().unwrap(), intended);
    assert_eq!(r.snapshot().recoveries, 1);
}

#[test]
fn invalid_records_are_preserved_until_local_off_repairs_only_intent() {
    let malformed = [
        vec![],
        vec![3, 1, 0, 47, 1, 0, 255, 0, 0],
        vec![3, 2, 57, 47, 1, 0, 255, 0, 0],
        vec![3, 1, 57, 0, 0, 0, 255, 0, 0],
        vec![3, 1, 57, 47, 1, 4, 255, 0, 0],
        vec![3, 1, 57, 47, 1, 0, 255, 1, 0],
    ];
    for bytes in malformed {
        let (h, _) = rig();
        h.records.borrow_mut().insert(INTENT_KEY, bytes.clone());
        h.records.borrow_mut().insert(123, vec![1, 2, 3]);
        let r = Runtime::load(Access(h.clone()), Output(h.clone()));
        r.tick(0);
        assert_eq!(r.snapshot().fault, Fault::InvalidRecord);
        assert!(!h.actual.get().unwrap().on);
        assert!(r.reported().is_err());
        assert_eq!(h.records.borrow()[&INTENT_KEY], bytes);
        assert!(r.set_power(true).is_err());
        r.off().unwrap();
        assert_eq!(h.records.borrow()[&INTENT_KEY][0], 3);
        assert_eq!(h.records.borrow()[&123], vec![1, 2, 3]);
        r.set_power(true).unwrap();
    }
}

#[test]
fn legacy_presets_migrate_once_and_restore_the_selected_temperature() {
    let (h, _) = rig();
    h.records
        .borrow_mut()
        .insert(INTENT_KEY, vec![2, 1, 1, 2, 2]);
    let r = Runtime::load(Access(h.clone()), Output(h.clone()));
    r.tick(0);
    r.tick(400);
    let state = r.acknowledged().unwrap();
    assert!(state.on);
    assert_eq!(state.level, Level::DEFAULT);
    assert_eq!(state.temperature.get(), 200);
    assert_eq!(h.records.borrow()[&INTENT_KEY][0], 3);
    assert_eq!(r.startup(), None);
    r.set_power(false).unwrap();
    let reboot = Runtime::load(Access(h.clone()), Output(h.clone()));
    reboot.tick(0);
    reboot.tick(400);
    assert!(!reboot.acknowledged().unwrap().on);
    assert_eq!(reboot.acknowledged().unwrap().temperature.get(), 200);
}

#[test]
fn transient_read_failure_retries_and_applies_startup_only_once() {
    let (h, _) = rig();
    h.records
        .borrow_mut()
        .insert(INTENT_KEY, vec![3, 1, 57, 47, 1, 3, 255, 0, 0]);
    h.fail_load.set(true);
    let r = Runtime::load(Access(h.clone()), Output(h.clone()));
    assert_eq!(r.snapshot().fault, Fault::Storage);
    let calls = h.load_calls.get();
    r.tick(4999);
    assert_eq!(h.load_calls.get(), calls);
    r.tick(5000);
    assert_eq!(r.snapshot().storage_failures, 2);
    h.fail_load.set(false);
    r.tick(10000);
    assert!(!r.acknowledged().unwrap().on);
    r.tick(10400);
    assert!(r.acknowledged().unwrap().on);
    r.tick(15000);
    assert!(r.acknowledged().unwrap().on);
    assert_eq!(h.records.borrow()[&INTENT_KEY][1], 1);
    assert_eq!(h.records.borrow()[&INTENT_KEY][5], 0);
}

#[test]
fn local_off_during_failed_read_overrides_saved_on_after_recovery() {
    let (h, _) = rig();
    h.records
        .borrow_mut()
        .insert(INTENT_KEY, vec![2, 1, 1, 0, 0]);
    h.fail_load.set(true);
    let r = Runtime::load(Access(h.clone()), Output(h.clone()));
    r.off().unwrap();
    assert_eq!(h.records.borrow()[&INTENT_KEY], vec![2, 1, 1, 0, 0]);
    h.fail_load.set(false);
    r.tick(5000);
    assert!(!r.acknowledged().unwrap().on);
    assert_eq!(r.acknowledged().unwrap().temperature.get(), 200);
}

#[test]
fn repeated_local_off_checks_io_without_rewriting_flash() {
    let (h, r) = rig();
    r.tick(0);
    r.set_power(true).unwrap();
    r.off().unwrap();
    h.events.borrow_mut().clear();
    r.off().unwrap();
    assert_eq!(h.events.borrow().first().unwrap(), "off");
    assert!(!h.events.borrow().iter().any(|e| e.starts_with("store:")));
}

#[test]
fn startup_settings_are_atomic_durable_and_do_not_change_live_output() {
    use rs_matter::dm::clusters::app::on_off::StartUpOnOffEnum as Startup;
    let (h, r) = rig();
    r.tick(0);
    r.set_power(true).unwrap();
    r.tick(400);
    h.fail_store.set(true);
    assert!(r.set_startup(Some(Startup::Off)).is_err());
    assert!(r.set_startup_level(Some(254)).is_err());
    assert!(r
        .set_startup_temperature(Some(ColorTemperature::MIN))
        .is_err());
    assert_eq!(r.startup(), None);
    assert_eq!(r.startup_level(), None);
    assert_eq!(r.startup_temperature(), None);
    h.fail_store.set(false);
    r.set_startup(Some(Startup::Off)).unwrap();
    r.set_startup_level(Some(254)).unwrap();
    r.set_startup_temperature(Some(ColorTemperature::MIN))
        .unwrap();
    assert_eq!(
        r.acknowledged().unwrap(),
        LightState {
            on: true,
            ..LightState::default()
        }
    );
    h.fail_store.set(true);
    let reboot = Runtime::load(Access(h.clone()), Output(h.clone()));
    reboot.tick(0);
    reboot.tick(400);
    assert!(reboot.acknowledged().is_err());
    h.fail_store.set(false);
    reboot.tick(5000);
    assert_eq!(
        reboot.acknowledged().unwrap(),
        LightState {
            on: false,
            level: Level::MAX,
            temperature: ColorTemperature::MIN
        }
    );
}

#[test]
fn independent_fades_persist_destinations_not_intermediate_frames() {
    let (h, r) = rig();
    r.tick(0);
    r.set_power(true).unwrap();
    r.tick(400);
    r.set_level_transition(254, false, 2000).unwrap();
    r.set_temperature_transition(ColorTemperature::MIN, 1000)
        .unwrap();
    h.events.borrow_mut().clear();
    r.tick(900);
    let half = r.acknowledged().unwrap();
    assert!(half.level.get() > 57 && half.level.get() < 254);
    assert_eq!(half.temperature.get(), 223);
    assert_eq!(r.level_remaining_ms(), 1500);
    assert_eq!(r.temperature_remaining_ms(), 500);
    assert!(!h.events.borrow().iter().any(|e| e.starts_with("store:")));
    let reboot = Runtime::load(Access(h.clone()), Output(h.clone()));
    reboot.tick(0);
    reboot.tick(400);
    assert_eq!(
        reboot.acknowledged().unwrap(),
        LightState {
            on: true,
            level: Level::MAX,
            temperature: ColorTemperature::MIN
        }
    );
}

#[test]
fn stopped_fade_is_durable_and_does_not_stop_other_axis() {
    let (h, r) = rig();
    r.tick(0);
    r.set_power(true).unwrap();
    r.set_level_transition(254, false, 2000).unwrap();
    r.set_temperature_transition(ColorTemperature::MIN, 2000)
        .unwrap();
    r.tick(500);
    let level = r.acknowledged().unwrap().level;
    r.stop_level_transition().unwrap();
    r.tick(2000);
    assert_eq!(r.acknowledged().unwrap().level, level);
    assert_eq!(r.acknowledged().unwrap().temperature, ColorTemperature::MIN);
    let reboot = Runtime::load(Access(h.clone()), Output(h.clone()));
    reboot.tick(0);
    reboot.tick(400);
    assert_eq!(reboot.acknowledged().unwrap(), r.acknowledged().unwrap());
}

#[test]
fn fade_to_off_and_immediate_off_never_relight_from_a_pending_track() {
    let (_, r) = rig();
    r.tick(0);
    r.set_power(true).unwrap();
    r.tick(400);
    r.set_level_transition(0, true, 1000).unwrap();
    r.tick(900);
    assert!(r.acknowledged().unwrap().on);
    r.tick(1400);
    assert!(!r.acknowledged().unwrap().on);
    r.set_level_transition(254, true, 2000).unwrap();
    r.tick(1900);
    r.off().unwrap();
    r.tick(4000);
    assert!(!r.acknowledged().unwrap().on);
    assert_eq!(r.level_remaining_ms(), 0);
}

#[test]
fn out_of_band_settings_while_off_do_not_energize_output() {
    let (_, r) = rig();
    r.tick(0);
    r.set_level(Level::MAX, false).unwrap();
    r.set_temperature(ColorTemperature::MIN).unwrap();
    assert!(!r.acknowledged().unwrap().on);
    r.set_power(true).unwrap();
    r.tick(400);
    assert_eq!(r.acknowledged().unwrap().level, Level::MAX);
    assert_eq!(r.acknowledged().unwrap().temperature, ColorTemperature::MIN);
}

#[test]
fn failed_fade_recovers_to_its_durable_destination() {
    let (h, r) = rig();
    r.tick(0);
    r.set_power(true).unwrap();
    r.set_level_transition(254, false, 1000).unwrap();
    h.fail_output.set(true);
    r.tick(500);
    assert!(r.acknowledged().is_err());
    h.fail_output.set(false);
    r.tick(5500);
    assert!(!r.acknowledged().unwrap().on);
    r.tick(5900);
    assert_eq!(r.acknowledged().unwrap().level, Level::MAX);
    assert_eq!(r.level_remaining_ms(), 0);
}

#[test]
fn reboot_shutdown_keeps_target_and_cannot_reapply_during_usb_flush() {
    let (h, r) = rig();
    r.tick(0);
    r.set_power(true).unwrap();
    r.set_temperature(ColorTemperature::MIN).unwrap();
    r.tick(400);
    assert!(h.actual.get().unwrap().on);
    assert!(r.prepare_reboot());
    r.tick(60_000);
    assert!(!h.actual.get().unwrap().on);
    assert!(r.set_power(true).is_err());
    let reboot = Runtime::load(Access(h.clone()), Output(h.clone()));
    reboot.tick(0);
    reboot.tick(400);
    assert!(reboot.acknowledged().unwrap().on);
    assert_eq!(
        reboot.acknowledged().unwrap().temperature,
        ColorTemperature::MIN
    );
}

#[test]
fn watchdog_test_requires_settled_verified_output_without_changing_it() {
    let (h, r) = rig();
    assert!(r.watchdog_test_ready().is_err());
    r.tick(0);
    r.set_power(true).unwrap();
    r.tick(400);
    let state = h.actual.get();
    let records = h.records.borrow().clone();
    h.events.borrow_mut().clear();
    r.watchdog_test_ready().unwrap();
    assert_eq!(h.actual.get(), state);
    assert_eq!(*h.records.borrow(), records);
    assert!(h.events.borrow().is_empty());
    r.set_level_transition(254, false, 1000).unwrap();
    assert!(r.watchdog_test_ready().is_err());
    r.tick(1400);
    h.fail_output.set(true);
    assert!(r.watchdog_test_ready().is_err());
    assert!(r.acknowledged().is_err());
}

#[test]
fn failed_diagnostic_read_invalidates_acknowledgement() {
    let (h, r) = rig();
    r.tick(0);
    h.fail_output.set(true);
    assert!(r.registers().is_err());
    assert!(r.acknowledged().is_err());
}

#[test]
fn repeated_on_preserves_both_fades_and_successful_recovery_clears_backoff() {
    let (h, r) = rig();
    r.tick(0);
    r.set_power(true).unwrap();
    r.set_level_transition(254, false, 2000).unwrap();
    r.set_temperature_transition(ColorTemperature::MIN, 2000)
        .unwrap();
    r.tick(500);
    let intermediate = r.acknowledged().unwrap();
    r.set_power(true).unwrap();
    assert_eq!(r.acknowledged().unwrap(), intermediate);
    assert_eq!(r.level_remaining_ms(), 1500);
    assert_eq!(r.temperature_remaining_ms(), 1500);
    h.fail_output.set(true);
    r.tick(1000);
    assert!(r.acknowledged().is_err());
    h.fail_output.set(false);
    r.set_power(true).unwrap();
    let recovered = r.acknowledged().unwrap();
    assert!(!recovered.on);
    assert_eq!(recovered.temperature, ColorTemperature::MIN);
    r.tick(1500);
    assert!(r.acknowledged().unwrap().level > recovered.level);
    assert_eq!(r.acknowledged().unwrap().temperature, recovered.temperature);
    r.tick(2000);
    assert_eq!(r.acknowledged().unwrap().level, Level::MAX);
    assert_eq!(r.acknowledged().unwrap().temperature, ColorTemperature::MIN);
}

#[test]
fn scene_duration_limit_is_supported_and_out_of_range_rejected_without_mutation() {
    let (_, r) = rig();
    r.tick(0);
    r.set_power(true).unwrap();
    r.set_level_transition(254, false, 60_000_000).unwrap();
    r.set_temperature_transition(ColorTemperature::MIN, 60_000_000)
        .unwrap();
    assert!(r.set_level_transition(1, false, 60_000_001).is_err());
    assert!(r
        .set_temperature_transition(ColorTemperature::MAX, 60_000_001)
        .is_err());
    r.tick(30_000_000);
    assert!(r.acknowledged().unwrap().level < Level::MAX);
    r.tick(60_000_000);
    assert_eq!(r.acknowledged().unwrap().level, Level::MAX);
    assert_eq!(r.acknowledged().unwrap().temperature, ColorTemperature::MIN);
}

#[test]
fn off_effects_keep_the_saved_level_and_never_persist_intermediate_frames() {
    use crate::runtime::OffEffect;
    for (effect, midpoint_ms, midpoint_level, end_ms) in [
        (OffEffect::FastFade, 400, 36, 800),
        (OffEffect::SlowFade, 800, 36, 12_800),
        (OffEffect::DyingLight, 500, 125, 1_500),
    ] {
        let (h, r) = rig();
        r.tick(0);
        r.set_level_transition(100, true, 0).unwrap();
        r.tick(400);
        h.events.borrow_mut().clear();
        r.off_with_effect(effect).unwrap();
        assert!(!r.snapshot().intended.on);
        assert_eq!(r.snapshot().intended.level.get(), 100);
        assert_eq!(
            h.events
                .borrow()
                .iter()
                .filter(|e| e.starts_with("store:"))
                .count(),
            1
        );
        h.events.borrow_mut().clear();
        r.tick(400 + midpoint_ms);
        assert!(r.acknowledged().unwrap().on);
        assert_eq!(r.acknowledged().unwrap().level.get(), midpoint_level);
        r.tick(400 + end_ms);
        assert!(!r.acknowledged().unwrap().on);
        assert_eq!(r.acknowledged().unwrap().level.get(), 100);
        assert!(!h.events.borrow().iter().any(|e| e.starts_with("store:")));
        let reboot = Runtime::load(Access(h.clone()), Output(h.clone()));
        reboot.tick(0);
        reboot.tick(400);
        assert!(!reboot.acknowledged().unwrap().on);
        reboot.set_power(true).unwrap();
        reboot.tick(800);
        assert_eq!(reboot.acknowledged().unwrap().level.get(), 100);
    }
    let (_, r) = rig();
    r.tick(0);
    r.set_power(true).unwrap();
    r.tick(400);
    r.off_with_effect(OffEffect::NoFade).unwrap();
    assert!(!r.acknowledged().unwrap().on);
    assert_eq!(r.level_remaining_ms(), 0);
}

#[test]
fn scene_recall_commits_all_axes_once_before_output_and_restores_the_destination() {
    let (h, r) = rig();
    r.tick(0);
    let target = LightState {
        on: true,
        level: Level::MAX,
        temperature: ColorTemperature::MIN,
    };
    h.events.borrow_mut().clear();
    r.recall_scene(target, 1000).unwrap();
    assert_eq!(r.snapshot().intended, target);
    assert_eq!(h.events.borrow()[0], format!("store:{INTENT_KEY}"));
    assert_eq!(
        h.events
            .borrow()
            .iter()
            .filter(|e| e.starts_with("store:"))
            .count(),
        1
    );
    r.tick(500);
    assert!(r.acknowledged().unwrap().level < target.level);
    let reboot = Runtime::load(Access(h.clone()), Output(h.clone()));
    reboot.tick(0);
    reboot.tick(400);
    assert_eq!(reboot.acknowledged().unwrap(), target);
    h.fail_store.set(true);
    assert!(reboot.recall_scene(LightState::default(), 0).is_err());
    assert!(reboot.acknowledged().is_err());
    // No partially saved power/level/temperature tuple replaced the prior scene.
    h.fail_store.set(false);
    let next_boot = Runtime::load(Access(h.clone()), Output(h.clone()));
    next_boot.tick(0);
    next_boot.tick(400);
    assert_eq!(next_boot.acknowledged().unwrap(), target);
}

#[test]
fn reboot_restores_valid_power_and_settings_without_changing_other_records() {
    for on in [false, true] {
        let (h, r) = rig();
        r.tick(0);
        let settings = LightState {
            on,
            level: Level::new(180).unwrap(),
            temperature: ColorTemperature::new(200).unwrap(),
        };
        r.request(settings).unwrap();
        h.records.borrow_mut().insert(123, vec![1, 2, 3]);
        h.events.borrow_mut().clear();
        let reboot = Runtime::load(Access(h.clone()), Output(h.clone()));
        reboot.tick(0);
        reboot.tick(400);
        reboot.tick(60_000);
        assert_eq!(reboot.reported().unwrap(), settings);
        assert_eq!(h.records.borrow()[&123], vec![1, 2, 3]);
        if !on {
            assert!(!h
                .events
                .borrow()
                .iter()
                .any(|event| event.contains("on: true")));
        }
    }
}

#[test]
fn old_on_and_toggle_policies_migrate_to_restore_without_overriding_saved_off() {
    use rs_matter::dm::clusters::app::on_off::StartUpOnOffEnum as Startup;
    for power in 0..=3 {
        for saved_on in 0..=1 {
            let (h, _) = rig();
            h.records
                .borrow_mut()
                .insert(INTENT_KEY, vec![3, saved_on, 180, 200, 0, power, 255, 0, 0]);
            h.events.borrow_mut().clear();
            let r = Runtime::load(Access(h.clone()), Output(h.clone()));
            r.tick(0);
            let expected_on = saved_on != 0 && power != 1;
            assert_eq!(r.reported().unwrap().on, expected_on);
            assert_eq!(r.reported().unwrap().level.get(), 180);
            assert_eq!(r.reported().unwrap().temperature.get(), 200);
            assert_eq!(
                r.startup(),
                if power == 1 { Some(Startup::Off) } else { None }
            );
            if !expected_on {
                assert!(!h
                    .events
                    .borrow()
                    .iter()
                    .any(|event| event.contains("on: true")));
            }
            assert_eq!(
                h.records.borrow()[&INTENT_KEY],
                vec![
                    3,
                    u8::from(expected_on),
                    180,
                    200,
                    0,
                    u8::from(power == 1),
                    255,
                    0,
                    0
                ]
            );
        }
    }
}

#[test]
fn legacy_preset_startup_only_retains_an_explicit_off_for_the_selected_preset() {
    for preset in 0..=1 {
        for policy in 0..=3 {
            for saved_on in 0..=1 {
                let (h, _) = rig();
                h.records
                    .borrow_mut()
                    .insert(INTENT_KEY, vec![2, saved_on, preset, policy, policy]);
                let r = Runtime::load(Access(h.clone()), Output(h.clone()));
                r.tick(0);
                assert_eq!(r.reported().unwrap().on, saved_on != 0 && policy != 1);
                assert_eq!(
                    r.reported().unwrap().temperature.get(),
                    if preset == 0 { 303 } else { 200 }
                );
            }
        }
    }
}

#[test]
fn startup_migration_failures_stay_off_until_valid_saved_state_is_available() {
    for fail_load in [false, true] {
        let (h, _) = rig();
        h.records
            .borrow_mut()
            .insert(INTENT_KEY, vec![3, 1, 180, 200, 0, 2, 255, 0, 0]);
        h.fail_load.set(fail_load);
        h.fail_store.set(true);
        h.events.borrow_mut().clear();
        let r = Runtime::load(Access(h.clone()), Output(h.clone()));
        r.tick(0);
        assert!(r.reported().is_err());
        assert!(!h.actual.get().unwrap().on);
        h.fail_load.set(false);
        h.fail_store.set(false);
        r.tick(5000);
        assert!(r.reported().unwrap().on);
        assert_eq!(r.reported().unwrap().level.get(), 180);
    }
}

#[test]
fn local_network_deadline_ignores_absent_ap_and_accepts_ipv6_without_dhcp() {
    let mut health = crate::recovery::LocalNetHealth::default();
    assert!(!health.restart_due(0, false, false));
    assert!(!health.restart_due(3_600_000, false, false));
    assert!(!health.restart_due(3_600_000, true, false));
    assert!(!health.restart_due(3_659_999, true, false));
    assert!(health.restart_due(3_660_000, true, false));
    assert!(!health.restart_due(3_660_001, true, true));
    assert!(!health.restart_due(4_000_000, true, true));
}
