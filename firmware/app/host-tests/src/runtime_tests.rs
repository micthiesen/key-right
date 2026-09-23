use crate::runtime::{
    Fault, Hardware, OutputError, OutputRegisters, Runtime, INTENT_KEY, PROFILE_KEY,
    TEST_DURATION_MS,
};
use key_right_core::pca9635::Profile;
use key_right_core::{LightOutput, LightState, Preset};
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
    fail_load: Cell<Option<u16>>,
    load_calls: Cell<u32>,
    fail_output: Cell<bool>,
    actual: Cell<Option<LightState>>,
}
struct Store(Rc<Rig>);
struct Access(Rc<Rig>);
impl KvBlobStore for Store {
    fn load<'a>(&mut self, key: u16, buf: &'a mut [u8]) -> Result<Option<&'a [u8]>, Error> {
        self.0.load_calls.set(self.0.load_calls.get() + 1);
        if self.0.fail_load.get() == Some(key) {
            return Err(ErrorCode::Failure.into());
        }
        let records = self.0.records.borrow();
        Ok(records.get(&key).map(|data| {
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
        panic!("runtime must preserve unknown records")
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
    fn apply(&mut self, state: LightState) -> Result<(), OutputError> {
        self.0.events.borrow_mut().push(format!("apply:{state:?}"));
        if self.0.fail_output.get() {
            return Err(OutputError::Readback);
        }
        self.0.actual.set(Some(state));
        Ok(())
    }
}
impl Hardware for Output {
    fn configure(&mut self, _: Profile) {
        self.0.events.borrow_mut().push("configure".into());
        self.0.actual.set(None);
    }
    fn shutdown(&mut self) -> Result<(), OutputError> {
        self.0.events.borrow_mut().push("disabled".into());
        self.0.actual.set(None);
        Ok(())
    }
    fn verify(&mut self, state: LightState) -> Result<(), OutputError> {
        if self.0.fail_output.get() || self.0.actual.get() != Some(state) {
            Err(OutputError::Readback)
        } else {
            Ok(())
        }
    }
    fn outputs(&self) -> OutputRegisters {
        OutputRegisters::default()
    }
}
fn rig(commissioned: bool) -> (Rc<Rig>, Runtime<Access, Output>) {
    let rig = Rc::new(Rig::default());
    if commissioned {
        rig.records.borrow_mut().insert(
            PROFILE_KEY,
            Profile::stock_candidate()
                .with_attestations(15)
                .unwrap()
                .encode()
                .to_vec(),
        );
    }
    let runtime = Runtime::load(Access(rig.clone()), Output(rig.clone()));
    (rig, runtime)
}
#[test]
fn missing_profile_cannot_energize_or_claim_off() {
    let (rig, r) = rig(false);
    r.tick(1000);
    assert!(r.set_endpoint(Preset::One, true).is_err());
    assert!(r.endpoint_on(Preset::One).is_err());
    assert_eq!(r.snapshot().fault, Fault::MissingProfile);
    assert_eq!(*rig.events.borrow(), ["disabled"]);
    r.off().unwrap();
    assert_eq!(rig.actual.get(), None);
}
#[test]
fn switching_preset_is_durable_before_io_and_inactive_off_is_noop() {
    let (rig, r) = rig(true);
    r.tick(0);
    rig.events.borrow_mut().clear();
    r.set_endpoint(Preset::Two, true).unwrap();
    let events = rig.events.borrow();
    assert_eq!(events[0], format!("store:{INTENT_KEY}"));
    assert!(events[1].starts_with("apply:"));
    drop(events);
    assert!(!r.endpoint_on(Preset::One).unwrap());
    assert!(r.endpoint_on(Preset::Two).unwrap());
    rig.events.borrow_mut().clear();
    r.set_endpoint(Preset::One, false).unwrap();
    assert!(rig.events.borrow().is_empty());
    let reboot = Runtime::load(Access(rig.clone()), Output(rig.clone()));
    reboot.tick(0);
    assert!(reboot.endpoint_on(Preset::Two).unwrap());
}
#[test]
fn failed_store_does_not_energize_and_retries_pending_intent() {
    let (rig, r) = rig(true);
    r.tick(0);
    rig.fail_store.set(true);
    assert!(r.set_endpoint(Preset::One, true).is_err());
    assert_eq!(rig.actual.get(), None);
    assert_eq!(r.snapshot().applied, None);
    assert_eq!(r.snapshot().fault, Fault::Storage);
    rig.fail_store.set(false);
    r.tick(5000);
    assert!(r.endpoint_on(Preset::One).unwrap());
}
#[test]
fn failed_apply_and_periodic_readback_invalidate_then_recover() {
    let (rig, r) = rig(true);
    r.tick(0);
    rig.fail_output.set(true);
    assert!(r.set_endpoint(Preset::Two, true).is_err());
    assert!(r.endpoint_on(Preset::Two).is_err());
    rig.fail_output.set(false);
    r.tick(4999);
    assert!(r.endpoint_on(Preset::Two).is_err());
    r.tick(5000);
    assert!(r.endpoint_on(Preset::Two).unwrap());
    assert_eq!(r.snapshot().recoveries, 1);
    rig.actual.set(None);
    r.tick(10000);
    assert!(r.endpoint_on(Preset::Two).is_err());
    r.tick(15000);
    assert!(r.endpoint_on(Preset::Two).unwrap());
    assert_eq!(r.snapshot().recoveries, 2);
}
#[test]
fn candidate_is_local_bounded_and_never_persisted_without_attestation() {
    let (rig, r) = rig(false);
    r.stage(Profile::stock_candidate()).unwrap();
    assert!(r.commit().is_err());
    assert!(r.set_endpoint(Preset::One, true).is_err());
    r.test(LightState {
        on: true,
        preset: Preset::One,
    })
    .unwrap();
    assert!(rig.actual.get().unwrap().on);
    assert!(r.endpoint_on(Preset::One).is_err());
    assert!(!rig.records.borrow().contains_key(&PROFILE_KEY));
    r.tick(TEST_DURATION_MS);
    assert_eq!(rig.actual.get(), None);
    assert!(!r.snapshot().intended.on);
    r.attest(15).unwrap();
    r.commit().unwrap();
    assert!(!r.endpoint_on(Preset::One).unwrap());
    assert!(rig.records.borrow().contains_key(&PROFILE_KEY));
    r.set_endpoint(Preset::Two, true).unwrap();
    assert!(r.endpoint_on(Preset::Two).unwrap());
}
#[test]
fn corrupt_record_is_preserved_and_disabled_until_explicit_commit() {
    let (rig, _) = rig(true);
    rig.records.borrow_mut().insert(INTENT_KEY, vec![1, 9, 0]);
    let r = Runtime::load(Access(rig.clone()), Output(rig.clone()));
    r.tick(0);
    assert_eq!(r.snapshot().fault, Fault::InvalidRecord);
    assert_eq!(rig.actual.get(), None);
    assert_eq!(rig.records.borrow().get(&INTENT_KEY).unwrap(), &[1, 9, 0]);
    assert!(r.set_endpoint(Preset::One, true).is_err());
    let original = rig.records.borrow().clone();
    r.stage(Profile::stock_candidate()).unwrap();
    r.test(LightState {
        on: true,
        preset: Preset::One,
    })
    .unwrap();
    r.off().unwrap();
    assert_eq!(*rig.records.borrow(), original);
    assert_eq!(r.snapshot().fault, Fault::InvalidRecord);
    assert!(r.set_startup(Preset::One, None).is_err());
    r.attest(15).unwrap();
    r.commit().unwrap();
    assert!(!r.endpoint_on(Preset::One).unwrap());
    assert_eq!(
        rig.records.borrow().get(&INTENT_KEY).unwrap(),
        &[2, 0, 0, 0, 0]
    );
}

#[test]
fn transient_record_reads_retry_isolated_and_apply_startup_once_after_loading() {
    for failed_key in [PROFILE_KEY, INTENT_KEY] {
        let (rig, _) = rig(true);
        rig.records
            .borrow_mut()
            .insert(INTENT_KEY, vec![2, 1, 1, 0, 3]);
        let original = rig.records.borrow().clone();
        rig.fail_load.set(Some(failed_key));
        let r = Runtime::load(Access(rig.clone()), Output(rig.clone()));
        assert_eq!(r.snapshot().fault, Fault::Storage);
        assert_eq!(r.snapshot().storage_failures, 1);
        assert!(r.stage(Profile::stock_candidate()).is_err());
        let calls = rig.load_calls.get();
        r.tick(4999);
        assert_eq!(rig.load_calls.get(), calls);
        r.tick(5000);
        assert_eq!(r.snapshot().storage_failures, 2);
        assert_eq!(rig.actual.get(), None);
        assert_eq!(*rig.records.borrow(), original);
        rig.fail_load.set(None);
        r.tick(9999);
        assert_eq!(rig.actual.get(), None);
        r.tick(10000);
        assert_eq!(r.snapshot().fault, Fault::None);
        assert_eq!(
            rig.actual.get(),
            Some(LightState {
                on: false,
                preset: Preset::Two
            })
        );
        assert_eq!(
            rig.records.borrow().get(&INTENT_KEY).unwrap(),
            &[2, 0, 1, 0, 3]
        );
        r.tick(15000);
        assert!(!r.endpoint_on(Preset::Two).unwrap());
    }
}

#[test]
fn local_off_during_read_failure_prevents_reenergizing_when_storage_recovers() {
    let (rig, _) = rig(true);
    rig.records
        .borrow_mut()
        .insert(INTENT_KEY, vec![2, 1, 1, 0, 0]);
    rig.fail_load.set(Some(INTENT_KEY));
    let r = Runtime::load(Access(rig.clone()), Output(rig.clone()));
    r.off().unwrap();
    assert_eq!(
        rig.records.borrow().get(&INTENT_KEY).unwrap(),
        &[2, 1, 1, 0, 0]
    );
    rig.fail_load.set(None);
    r.tick(5000);
    assert!(!r.endpoint_on(Preset::Two).unwrap());
    assert_eq!(
        rig.records.borrow().get(&INTENT_KEY).unwrap(),
        &[2, 0, 1, 0, 0]
    );
}

#[test]
fn either_endpoint_off_stops_candidate_test_without_unblocking_on() {
    let (rig, r) = rig(true);
    r.tick(0);
    r.stage(Profile::stock_candidate()).unwrap();
    r.set_endpoint(Preset::Two, false).unwrap();
    r.test(LightState {
        on: true,
        preset: Preset::One,
    })
    .unwrap();
    assert!(rig.actual.get().unwrap().on);
    r.set_endpoint(Preset::Two, false).unwrap();
    assert!(!r.endpoint_on(Preset::One).unwrap());
    assert!(!rig.actual.get().unwrap().on);
    assert!(r.set_endpoint(Preset::One, true).is_err());
}

#[test]
fn timed_off_changes_intent_even_during_output_failure() {
    let (rig, r) = rig(true);
    r.tick(0);
    let handler =
        crate::presets::PresetHandler::new(&r, Preset::One, 1, rs_matter::dm::Dataver::new(0));
    rig.fail_output.set(true);
    assert!(handler.timed_on(false, 10, 0).is_err());
    assert!(r.snapshot().intended.on);
    handler.tick(10);
    assert!(!r.snapshot().intended.on);
    rig.fail_output.set(false);
    r.tick(5000);
    assert!(!r.endpoint_on(Preset::One).unwrap());
}

#[test]
fn timed_command_accept_only_on_is_noop_and_inactive_timer_cannot_turn_off_new_preset() {
    let (_rig, r) = rig(true);
    r.tick(0);
    let h = crate::presets::PresetHandler::new(&r, Preset::One, 1, rs_matter::dm::Dataver::new(0));
    h.timed_on(true, 10, 0).unwrap();
    assert!(!r.endpoint_on(Preset::One).unwrap());
    h.timed_on(false, 10, 0).unwrap();
    assert!(r.endpoint_on(Preset::One).unwrap());
    r.set_endpoint(Preset::Two, true).unwrap();
    h.tick(10);
    assert!(r.endpoint_on(Preset::Two).unwrap());
}

#[test]
fn profile_hex_parser_rejects_length_characters_corruption_and_wrong_temperatures() {
    let p = Profile::stock_candidate();
    let hex = p
        .encode()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    assert_eq!(crate::protocol::decode_profile(&hex).unwrap(), p);
    assert!(crate::protocol::decode_profile(&hex[..158]).is_err());
    assert!(crate::protocol::decode_profile(&hex.replacen('a', "z", 1)).is_err());
    let mut corrupt = hex.clone().into_bytes();
    corrupt[0] = b'0';
    assert!(crate::protocol::decode_profile(std::str::from_utf8(&corrupt).unwrap()).is_err());
    let wrong = Profile::new(p.address(), p.mode2(), [300, 200], *p.frames(), 0).unwrap();
    let (_, r) = rig(false);
    assert!(r.stage(wrong).is_err());
}

#[test]
fn intentional_reset_keeps_durable_intent_without_reenergizing_during_ack_flush() {
    let (rig, r) = rig(true);
    r.tick(0);
    r.set_endpoint(Preset::Two, true).unwrap();
    r.isolate();
    r.tick(60_000);
    assert_eq!(rig.actual.get(), None);
    assert!(r.set_endpoint(Preset::One, true).is_err());
    let reboot = Runtime::load(Access(rig.clone()), Output(rig.clone()));
    reboot.tick(0);
    assert!(reboot.endpoint_on(Preset::Two).unwrap());
}

#[test]
fn watchdog_test_requires_committed_verified_output_and_preserves_it() {
    let (_, missing) = rig(false);
    assert!(missing.watchdog_test_ready().is_err());
    let (rig, r) = rig(true);
    assert!(r.watchdog_test_ready().is_err()); // Boot output not acknowledged yet.
    r.tick(0);
    r.set_endpoint(Preset::Two, true).unwrap();
    let intended = r.snapshot().intended;
    let record = rig.records.borrow().get(&INTENT_KEY).unwrap().clone();
    rig.events.borrow_mut().clear();
    r.watchdog_test_ready().unwrap();
    assert_eq!(rig.actual.get(), Some(intended));
    assert_eq!(rig.records.borrow().get(&INTENT_KEY), Some(&record));
    assert!(rig.events.borrow().is_empty());
    rig.fail_output.set(true);
    assert!(r.watchdog_test_ready().is_err());
    assert_eq!(rig.actual.get(), None);
    rig.fail_output.set(false);
    r.tick(5000);
    r.stage(Profile::stock_candidate()).unwrap();
    assert!(r.watchdog_test_ready().is_err());
    r.test(LightState {
        on: true,
        preset: Preset::One,
    })
    .unwrap();
    assert!(r.watchdog_test_ready().is_err());
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
    assert!(!health.restart_due(5_000_000, true, false));
}

#[test]
fn startup_policies_are_durable_and_endpoint_two_wins_conflicting_on() {
    use rs_matter::dm::clusters::app::on_off::StartUpOnOffEnum as Startup;
    let (rig, r) = rig(true);
    r.tick(0);
    r.set_startup(Preset::One, Some(Startup::On)).unwrap();
    r.set_startup(Preset::Two, Some(Startup::On)).unwrap();
    assert!(!r.snapshot().intended.on); // A setting write does not change live power.
    let reboot = Runtime::load(Access(rig.clone()), Output(rig.clone()));
    assert_eq!(reboot.snapshot().applied, None);
    reboot.tick(0);
    assert!(!reboot.endpoint_on(Preset::One).unwrap());
    assert!(reboot.endpoint_on(Preset::Two).unwrap());
    assert_eq!(
        rig.records.borrow().get(&INTENT_KEY).unwrap(),
        &[2, 1, 1, 2, 2]
    );
    reboot.set_startup(Preset::One, None).unwrap();
    reboot
        .set_startup(Preset::Two, Some(Startup::Toggle))
        .unwrap();
    let again = Runtime::load(Access(rig.clone()), Output(rig.clone()));
    again.tick(0);
    assert!(!again.endpoint_on(Preset::Two).unwrap());
}

#[test]
fn startup_storage_failures_do_not_claim_setting_or_apply_unpersisted_boot_transition() {
    use rs_matter::dm::clusters::app::on_off::StartUpOnOffEnum as Startup;
    let (rig, r) = rig(true);
    r.tick(0);
    rig.fail_store.set(true);
    assert!(r.set_startup(Preset::Two, Some(Startup::On)).is_err());
    assert_eq!(r.startup(Preset::Two), None);
    rig.fail_store.set(false);
    r.set_startup(Preset::Two, Some(Startup::On)).unwrap();
    rig.fail_store.set(true);
    let reboot = Runtime::load(Access(rig.clone()), Output(rig.clone()));
    reboot.tick(0);
    assert_eq!(reboot.snapshot().applied, None);
    assert_eq!(rig.actual.get(), None);
    rig.fail_store.set(false);
    reboot.tick(5000);
    assert!(reboot.endpoint_on(Preset::Two).unwrap());
}

#[test]
fn repeated_local_off_is_physical_but_does_not_write_flash_again() {
    let (rig, r) = rig(true);
    r.tick(0);
    r.set_endpoint(Preset::One, true).unwrap();
    r.off().unwrap();
    rig.events.borrow_mut().clear();
    r.off().unwrap();
    assert_eq!(rig.events.borrow().first().unwrap(), "disabled");
    assert!(!rig.events.borrow().iter().any(|e| e.starts_with("store:")));
}

#[test]
fn legacy_intent_record_is_read_and_upgraded_on_next_change() {
    let (rig, _) = rig(true);
    rig.records.borrow_mut().insert(INTENT_KEY, vec![1, 1, 0]);
    let r = Runtime::load(Access(rig.clone()), Output(rig.clone()));
    r.tick(0);
    assert!(r.endpoint_on(Preset::One).unwrap());
    r.set_endpoint(Preset::Two, true).unwrap();
    assert_eq!(
        rig.records.borrow().get(&INTENT_KEY).unwrap(),
        &[2, 1, 1, 0, 0]
    );
}

#[test]
fn preset_deselection_clears_old_timed_duration_before_reselection() {
    let (_, r) = rig(true);
    r.tick(0);
    let h = crate::presets::PresetHandler::new(&r, Preset::One, 1, rs_matter::dm::Dataver::new(0));
    h.timed_on(false, 100, 0).unwrap();
    r.set_endpoint(Preset::Two, true).unwrap();
    h.tick(1);
    h.timed_on(false, 2, 0).unwrap();
    h.tick(2);
    assert!(!r.snapshot().intended.on);
}

#[test]
fn import_clears_attestations_and_failed_commit_cannot_activate_candidate() {
    let (rig, r) = rig(false);
    r.stage(Profile::stock_candidate().with_attestations(15).unwrap())
        .unwrap();
    assert_eq!(r.staged().unwrap().attestations(), 0);
    assert!(r.commit().is_err());
    r.attest(15).unwrap();
    rig.fail_store.set(true);
    assert!(r.commit().is_err());
    assert!(r.profile().is_none());
    assert!(r.snapshot().applied.is_none());
    assert_eq!(r.snapshot().storage_failures, 1);
    assert!(!rig.records.borrow().contains_key(&PROFILE_KEY));
    rig.fail_store.set(false);
    r.commit().unwrap();
    assert!(!r.endpoint_on(Preset::One).unwrap());
}
