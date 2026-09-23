use crate::runtime::{Fault, Hardware, OutputError, Runtime, INTENT_KEY};
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
    fail_load: Cell<bool>,
    load_calls: Cell<u32>,
    fail_output: Cell<bool>,
    actual: Cell<Option<LightState>>,
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
    fn apply(&mut self, state: LightState) -> Result<(), OutputError> {
        self.0.events.borrow_mut().push(format!("apply:{state:?}"));
        if self.0.fail_output.get() {
            return Err(OutputError::Bus);
        }
        self.0.actual.set(Some(state));
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
        Ok(())
    }
    fn verify(&mut self, state: LightState) -> Result<(), OutputError> {
        if self.0.fail_output.get() || self.0.actual.get() != Some(state) {
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
fn fresh_boot_uses_stock_output_without_profile_provisioning() {
    let (h, r) = rig();
    r.tick(0);
    assert!(!r.endpoint_on(Preset::One).unwrap());
    r.set_endpoint(Preset::Two, true).unwrap();
    assert!(r.endpoint_on(Preset::Two).unwrap());
    assert_eq!(h.records.borrow().len(), 1); // Only light intent; no profile blob.
}
#[test]
fn absent_pca_is_an_output_fault_not_a_provisioning_gate() {
    let (h, _) = rig();
    h.fail_output.set(true);
    let r = Runtime::load(Access(h.clone()), Output(h.clone()));
    assert_eq!(r.snapshot().fault, Fault::Output);
    assert_eq!(r.snapshot().applied, None);
    assert!(r.set_endpoint(Preset::Two, true).is_err());
    assert!(r.snapshot().intended.on);
    h.fail_output.set(false);
    r.tick(5000);
    assert!(r.endpoint_on(Preset::Two).unwrap());
}
#[test]
fn preset_selection_is_durable_before_io_and_inactive_off_is_noop() {
    let (h, r) = rig();
    r.tick(0);
    h.events.borrow_mut().clear();
    r.set_endpoint(Preset::Two, true).unwrap();
    assert_eq!(h.events.borrow()[0], format!("store:{INTENT_KEY}"));
    assert!(h.events.borrow()[1].starts_with("apply:"));
    h.events.borrow_mut().clear();
    r.set_endpoint(Preset::One, false).unwrap();
    assert!(h.events.borrow().is_empty());
    let reboot = Runtime::load(Access(h.clone()), Output(h.clone()));
    reboot.tick(0);
    assert!(reboot.endpoint_on(Preset::Two).unwrap());
}
#[test]
fn failed_save_never_applies_pending_on_and_retries_it() {
    let (h, r) = rig();
    r.tick(0);
    h.fail_store.set(true);
    assert!(r.set_endpoint(Preset::One, true).is_err());
    assert_eq!(r.snapshot().fault, Fault::Storage);
    assert_eq!(r.snapshot().applied, None);
    assert!(!h.actual.get().unwrap().on);
    h.fail_store.set(false);
    r.tick(4999);
    assert_eq!(r.snapshot().applied, None);
    r.tick(5000);
    assert!(r.endpoint_on(Preset::One).unwrap());
}
#[test]
fn failed_off_can_leave_light_on_but_never_claims_acknowledged_off() {
    let (h, r) = rig();
    r.tick(0);
    r.set_endpoint(Preset::Two, true).unwrap();
    h.fail_output.set(true);
    assert!(r.off().is_err());
    assert!(!r.snapshot().intended.on);
    assert_eq!(r.snapshot().applied, None);
    assert!(h.actual.get().unwrap().on);
    assert_eq!(r.snapshot().fault, Fault::Output);
    h.fail_output.set(false);
    r.tick(5000);
    assert!(!r.endpoint_on(Preset::Two).unwrap());
    assert!(!h.actual.get().unwrap().on);
}
#[test]
fn periodic_readback_failure_marks_unknown_then_restores_intent() {
    let (h, r) = rig();
    r.tick(0);
    r.set_endpoint(Preset::Two, true).unwrap();
    h.actual.set(None);
    r.tick(5000);
    assert!(r.endpoint_on(Preset::Two).is_err());
    r.tick(9999);
    assert_eq!(r.snapshot().applied, None);
    r.tick(10000);
    assert!(r.endpoint_on(Preset::Two).unwrap());
    assert_eq!(r.snapshot().recoveries, 1);
}
#[test]
fn invalid_record_is_preserved_until_explicit_local_off_repairs_it() {
    let (h, _) = rig();
    h.records
        .borrow_mut()
        .insert(INTENT_KEY, vec![2, 9, 0, 0, 0]);
    let r = Runtime::load(Access(h.clone()), Output(h.clone()));
    r.tick(0);
    assert_eq!(r.snapshot().fault, Fault::InvalidRecord);
    assert_eq!(h.records.borrow()[&INTENT_KEY], vec![2, 9, 0, 0, 0]);
    assert!(r.set_endpoint(Preset::One, true).is_err());
    r.off().unwrap();
    assert_eq!(h.records.borrow()[&INTENT_KEY], vec![2, 0, 0, 0, 0]);
    r.set_endpoint(Preset::One, true).unwrap();
}
#[test]
fn transient_read_failure_retries_and_applies_startup_only_once() {
    let (h, _) = rig();
    h.records
        .borrow_mut()
        .insert(INTENT_KEY, vec![2, 1, 1, 0, 3]);
    h.fail_load.set(true);
    let r = Runtime::load(Access(h.clone()), Output(h.clone()));
    assert_eq!(r.snapshot().fault, Fault::Storage);
    let calls = h.load_calls.get();
    r.tick(4999);
    assert_eq!(h.load_calls.get(), calls);
    r.tick(5000);
    assert_eq!(r.snapshot().storage_failures, 2);
    assert_eq!(h.records.borrow()[&INTENT_KEY], vec![2, 1, 1, 0, 3]);
    h.fail_load.set(false);
    r.tick(10000);
    assert!(!r.endpoint_on(Preset::Two).unwrap());
    r.tick(15000);
    assert!(!r.endpoint_on(Preset::Two).unwrap());
    assert_eq!(h.records.borrow()[&INTENT_KEY], vec![2, 0, 1, 0, 3]);
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
    assert!(!r.endpoint_on(Preset::Two).unwrap());
    assert_eq!(h.records.borrow()[&INTENT_KEY], vec![2, 0, 1, 0, 0]);
}
#[test]
fn repeated_local_off_checks_io_without_rewriting_flash() {
    let (h, r) = rig();
    r.tick(0);
    r.set_endpoint(Preset::One, true).unwrap();
    r.off().unwrap();
    h.events.borrow_mut().clear();
    r.off().unwrap();
    assert_eq!(h.events.borrow().first().unwrap(), "off");
    assert!(!h.events.borrow().iter().any(|e| e.starts_with("store:")));
}
#[test]
fn startup_policies_are_durable_and_endpoint_two_wins_conflicting_on() {
    use rs_matter::dm::clusters::app::on_off::StartUpOnOffEnum as Startup;
    let (h, r) = rig();
    r.tick(0);
    r.set_startup(Preset::One, Some(Startup::On)).unwrap();
    r.set_startup(Preset::Two, Some(Startup::On)).unwrap();
    assert!(!r.snapshot().intended.on);
    let reboot = Runtime::load(Access(h.clone()), Output(h.clone()));
    reboot.tick(0);
    assert!(!reboot.endpoint_on(Preset::One).unwrap());
    assert!(reboot.endpoint_on(Preset::Two).unwrap());
    assert_eq!(h.records.borrow()[&INTENT_KEY], vec![2, 1, 1, 2, 2]);
}
#[test]
fn startup_save_failures_do_not_claim_setting_or_apply_unsaved_boot_change() {
    use rs_matter::dm::clusters::app::on_off::StartUpOnOffEnum as Startup;
    let (h, r) = rig();
    r.tick(0);
    h.fail_store.set(true);
    assert!(r.set_startup(Preset::Two, Some(Startup::On)).is_err());
    assert_eq!(r.startup(Preset::Two), None);
    h.fail_store.set(false);
    r.set_startup(Preset::Two, Some(Startup::On)).unwrap();
    h.fail_store.set(true);
    let reboot = Runtime::load(Access(h.clone()), Output(h.clone()));
    reboot.tick(0);
    assert_eq!(reboot.snapshot().applied, None);
    h.fail_store.set(false);
    reboot.tick(5000);
    assert!(reboot.endpoint_on(Preset::Two).unwrap());
}
#[test]
fn timed_off_updates_intent_despite_bus_failure() {
    let (h, r) = rig();
    r.tick(0);
    let handler =
        crate::presets::PresetHandler::new(&r, Preset::One, 1, rs_matter::dm::Dataver::new(0));
    h.fail_output.set(true);
    assert!(handler.timed_on(false, 10, 0).is_err());
    assert!(r.snapshot().intended.on);
    handler.tick(10);
    assert!(!r.snapshot().intended.on);
    h.fail_output.set(false);
    r.tick(5000);
    assert!(!r.endpoint_on(Preset::One).unwrap());
}
#[test]
fn selecting_other_preset_clears_old_timed_duration() {
    let (_, r) = rig();
    r.tick(0);
    let h = crate::presets::PresetHandler::new(&r, Preset::One, 1, rs_matter::dm::Dataver::new(0));
    h.timed_on(true, 10, 0).unwrap();
    assert!(!r.endpoint_on(Preset::One).unwrap());
    h.timed_on(false, 100, 0).unwrap();
    r.set_endpoint(Preset::Two, true).unwrap();
    h.tick(1);
    assert!(r.endpoint_on(Preset::Two).unwrap());
    h.timed_on(false, 2, 0).unwrap();
    h.tick(2);
    assert!(!r.snapshot().intended.on);
}
#[test]
fn reboot_best_effort_off_keeps_intent_and_cannot_reapply_during_flush() {
    let (h, r) = rig();
    r.tick(0);
    r.set_endpoint(Preset::Two, true).unwrap();
    assert!(r.prepare_reboot());
    r.tick(60_000);
    assert!(!h.actual.get().unwrap().on);
    assert!(r.set_endpoint(Preset::One, true).is_err());
    let reboot = Runtime::load(Access(h.clone()), Output(h.clone()));
    reboot.tick(0);
    assert!(reboot.endpoint_on(Preset::Two).unwrap());
}
#[test]
fn watchdog_test_requires_verified_output_and_does_not_change_it() {
    let (h, r) = rig();
    assert!(r.watchdog_test_ready().is_err());
    r.tick(0);
    r.set_endpoint(Preset::Two, true).unwrap();
    let state = h.actual.get();
    let records = h.records.borrow().clone();
    h.events.borrow_mut().clear();
    r.watchdog_test_ready().unwrap();
    assert_eq!(h.actual.get(), state);
    assert_eq!(*h.records.borrow(), records);
    assert!(h.events.borrow().is_empty());
    h.fail_output.set(true);
    assert!(r.watchdog_test_ready().is_err());
    assert_eq!(r.snapshot().applied, None);
}
#[test]
fn failed_diagnostic_read_invalidates_acknowledged_output() {
    let (h, r) = rig();
    r.tick(0);
    h.fail_output.set(true);
    assert!(r.registers().is_err());
    assert_eq!(r.snapshot().applied, None);
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
