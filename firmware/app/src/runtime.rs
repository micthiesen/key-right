//! Durable intent and acknowledged stock PCA register state.
//! Failed I2C may leave the previous light output active; Off is best effort.
use core::cell::{Cell, RefCell};
use key_right_core::{Command, Controller, LightOutput, LightState, Preset};
use rs_matter_embassy::matter::dm::clusters::app::on_off::StartUpOnOffEnum;
use rs_matter_embassy::matter::error::{Error, ErrorCode};
use rs_matter_embassy::matter::persist::{KvBlobStoreAccess, VENDOR_KEYS_START};

pub const INTENT_KEY: u16 = VENDOR_KEYS_START + 3;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputError {
    Bus,
    Readback,
}
pub trait Hardware: LightOutput<Error = OutputError> {
    fn shutdown(&mut self) -> Result<(), OutputError>;
    fn verify(&mut self, state: LightState) -> Result<(), OutputError>;
    fn registers(&mut self) -> Result<[u8; 24], OutputError>;
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fault {
    None,
    Storage,
    InvalidRecord,
    Output,
    Rebooting,
}
#[derive(Clone, Copy, Debug)]
pub struct Snapshot {
    pub intended: LightState,
    pub applied: Option<LightState>,
    pub fault: Fault,
    pub output_failures: u32,
    pub storage_failures: u32,
    pub recoveries: u32,
    pub revision: u32,
}
struct State<H> {
    controller: Controller,
    hardware: H,
    fault: Fault,
    dirty: bool,
    startup: [u8; 2],
    load_fault: Option<Fault>,
    off_after_load: bool,
    next_verify: u64,
    retry_at: u64,
    failures: u32,
    storage_failures: u32,
    recoveries: u32,
    revision: u32,
}
#[derive(Default)]
struct SavedState {
    intended: LightState,
    startup: [u8; 2],
}
pub struct Runtime<K, H> {
    kv: K,
    state: RefCell<State<H>>,
    now: Cell<u64>,
}

pub fn endpoint_target(current: LightState, preset: Preset, on: bool) -> LightState {
    if on {
        LightState { on: true, preset }
    } else if current.preset == preset {
        LightState {
            on: false,
            ..current
        }
    } else {
        current
    }
}
fn encode_record(state: LightState, startup: [u8; 2]) -> [u8; 5] {
    [
        2,
        u8::from(state.on),
        u8::from(state.preset == Preset::Two),
        startup[0],
        startup[1],
    ]
}
fn decode_record(bytes: &[u8]) -> Result<SavedState, Fault> {
    match *bytes {
        [2, on @ 0..=1, preset @ 0..=1, one @ 0..=3, two @ 0..=3] => Ok(SavedState {
            intended: LightState {
                on: on != 0,
                preset: if preset == 0 {
                    Preset::One
                } else {
                    Preset::Two
                },
            },
            startup: [one, two],
        }),
        _ => Err(Fault::InvalidRecord),
    }
}
impl<K: KvBlobStoreAccess, H: Hardware> Runtime<K, H> {
    fn load_record(kv: &K) -> Result<SavedState, Fault> {
        kv.access(
            |store, buf| match store.load(INTENT_KEY, buf).map_err(|_| Fault::Storage)? {
                None => Ok(SavedState::default()),
                Some(bytes) => decode_record(bytes),
            },
        )
    }
    fn startup_intent(restored: LightState, startup: [u8; 2]) -> LightState {
        let mut intended = restored;
        // Apply in endpoint order; endpoint 2 wins conflicting ON policies.
        for (preset, policy) in [(Preset::One, startup[0]), (Preset::Two, startup[1])] {
            let on = match policy {
                0 => continue,
                1 => false,
                2 => true,
                _ => !(intended.on && intended.preset == preset),
            };
            intended = endpoint_target(intended, preset, on);
        }
        intended
    }
    pub fn load(kv: K, mut hardware: H) -> Self {
        let off_failed = hardware.shutdown().is_err();
        let saved = Self::load_record(&kv);
        let load_fault = saved.as_ref().err().copied();
        let saved = saved.unwrap_or_default();
        let intended = Self::startup_intent(saved.intended, saved.startup);
        Self {
            kv,
            now: Cell::new(0),
            state: RefCell::new(State {
                controller: Controller::new(Some(intended)),
                hardware,
                fault: load_fault.unwrap_or(if off_failed {
                    Fault::Output
                } else {
                    Fault::None
                }),
                dirty: intended != saved.intended,
                startup: saved.startup,
                load_fault,
                off_after_load: false,
                next_verify: 0,
                retry_at: if off_failed || load_fault == Some(Fault::Storage) {
                    5_000
                } else {
                    0
                },
                failures: u32::from(off_failed),
                storage_failures: u32::from(load_fault == Some(Fault::Storage)),
                recoveries: 0,
                revision: 0,
            }),
        }
    }
    pub fn snapshot(&self) -> Snapshot {
        let s = self.state.borrow();
        Snapshot {
            intended: s.controller.intended(),
            applied: s.controller.applied(),
            fault: s.fault,
            output_failures: s.failures,
            storage_failures: s.storage_failures,
            recoveries: s.recoveries,
            revision: s.revision,
        }
    }
    /// Best-effort Off before reset; never rewrites durable intent.
    pub fn prepare_reboot(&self) -> bool {
        let mut s = self.state.borrow_mut();
        let verified = s.hardware.shutdown().is_ok();
        s.controller.invalidate_applied();
        s.fault = Fault::Rebooting;
        verified
    }
    pub fn watchdog_test_ready(&self) -> Result<(), Error> {
        {
            let s = self.state.borrow();
            if s.dirty || s.fault != Fault::None {
                return Err(ErrorCode::InvalidState.into());
            }
        }
        self.verify()
    }
    pub fn endpoint_on(&self, preset: Preset) -> Result<bool, Error> {
        let s = self.state.borrow();
        let applied = s.controller.applied().ok_or(ErrorCode::InvalidState)?;
        Ok(applied.on && applied.preset == preset)
    }
    pub fn set_endpoint(&self, preset: Preset, on: bool) -> Result<(), Error> {
        let current = self.state.borrow().controller.intended();
        self.request(endpoint_target(current, preset, on))
    }
    pub fn startup(&self, preset: Preset) -> Option<StartUpOnOffEnum> {
        match self.state.borrow().startup[usize::from(preset == Preset::Two)] {
            0 => None,
            1 => Some(StartUpOnOffEnum::Off),
            2 => Some(StartUpOnOffEnum::On),
            _ => Some(StartUpOnOffEnum::Toggle),
        }
    }
    pub fn set_startup(
        &self,
        preset: Preset,
        value: Option<StartUpOnOffEnum>,
    ) -> Result<(), Error> {
        let mut s = self.state.borrow_mut();
        if s.load_fault.is_some() {
            return Err(ErrorCode::InvalidState.into());
        }
        let mut settings = s.startup;
        settings[usize::from(preset == Preset::Two)] = match value {
            None => 0,
            Some(StartUpOnOffEnum::Off) => 1,
            Some(StartUpOnOffEnum::On) => 2,
            Some(StartUpOnOffEnum::Toggle) => 3,
        };
        if settings == s.startup && !s.dirty {
            return Ok(());
        }
        let data = encode_record(s.controller.intended(), settings);
        if let Err(e) = self
            .kv
            .access(|store, buf| store.store(INTENT_KEY, &data, buf))
        {
            s.storage_failures = s.storage_failures.saturating_add(1);
            return Err(e);
        }
        s.startup = settings;
        s.dirty = false;
        s.revision = s.revision.wrapping_add(1);
        Ok(())
    }
    fn save_intent(&self, s: &mut State<H>) -> Result<(), Error> {
        if !s.dirty {
            return Ok(());
        }
        let data = encode_record(s.controller.intended(), s.startup);
        if let Err(e) = self
            .kv
            .access(|store, buf| store.store(INTENT_KEY, &data, buf))
        {
            s.storage_failures = s.storage_failures.saturating_add(1);
            s.fault = Fault::Storage;
            // A failed stop cannot promise physical Off; acknowledgement remains unknown.
            if s.hardware.shutdown().is_err() {
                s.failures = s.failures.saturating_add(1);
            }
            s.controller.invalidate_applied();
            s.revision = s.revision.wrapping_add(1);
            s.retry_at = self.now.get().saturating_add(5_000);
            return Err(e);
        }
        s.dirty = false;
        Ok(())
    }
    fn output_failed(s: &mut State<H>, now: u64) {
        let _ = s.hardware.shutdown();
        s.controller.invalidate_applied();
        s.fault = Fault::Output;
        s.failures = s.failures.saturating_add(1);
        s.retry_at = now.saturating_add(5_000);
        s.revision = s.revision.wrapping_add(1);
    }
    fn reconcile(s: &mut State<H>, now: u64) -> Result<(), Error> {
        let previous = s.controller.applied();
        if s.controller.reconcile(&mut s.hardware).is_err() {
            Self::output_failed(s, now);
            return Err(ErrorCode::Failure.into());
        }
        if s.fault == Fault::Output {
            s.recoveries = s.recoveries.saturating_add(1);
        }
        s.fault = Fault::None;
        s.next_verify = now.saturating_add(5_000);
        if previous != s.controller.applied() {
            s.revision = s.revision.wrapping_add(1);
        }
        Ok(())
    }
    pub fn request(&self, state: LightState) -> Result<(), Error> {
        let mut s = self.state.borrow_mut();
        if s.fault == Fault::Rebooting {
            return Err(ErrorCode::Busy.into());
        }
        if s.load_fault.is_some() {
            return Err(ErrorCode::InvalidState.into());
        }
        let changed = s.controller.command(Command::SelectPreset(state.preset));
        s.dirty |= changed | s.controller.command(Command::SetPower(state.on));
        self.save_intent(&mut s)?;
        Self::reconcile(&mut s, self.now.get())
    }
    /// USB Off always attempts stock zero-PWM, even when storage is unreadable.
    /// Explicit local Off also repairs a malformed intent record with safe defaults.
    pub fn off(&self) -> Result<(), Error> {
        let mut s = self.state.borrow_mut();
        let stopped = s.hardware.shutdown().is_ok();
        s.controller.invalidate_applied();
        s.dirty |= s.controller.command(Command::SetPower(false));
        if s.load_fault == Some(Fault::Storage) {
            s.off_after_load = true;
            s.dirty = false;
            if !stopped {
                s.failures = s.failures.saturating_add(1);
            }
            return if stopped {
                Ok(())
            } else {
                Err(ErrorCode::Failure.into())
            };
        }
        s.dirty |= s.load_fault == Some(Fault::InvalidRecord);
        self.save_intent(&mut s)?;
        s.load_fault = None;
        s.revision = s.revision.wrapping_add(1);
        if !stopped {
            Self::output_failed(&mut s, self.now.get());
            return Err(ErrorCode::Failure.into());
        }
        Self::reconcile(&mut s, self.now.get())
    }
    pub fn verify(&self) -> Result<(), Error> {
        let mut s = self.state.borrow_mut();
        let applied = s.controller.applied().ok_or(ErrorCode::InvalidState)?;
        if s.hardware.verify(applied).is_err() {
            Self::output_failed(&mut s, self.now.get());
            return Err(ErrorCode::Failure.into());
        }
        Ok(())
    }
    pub fn registers(&self) -> Result<[u8; 24], Error> {
        let mut s = self.state.borrow_mut();
        s.hardware.registers().map_err(|_| {
            Self::output_failed(&mut s, self.now.get());
            ErrorCode::Failure.into()
        })
    }
    pub fn tick(&self, now: u64) {
        self.now.set(now);
        let mut s = self.state.borrow_mut();
        if s.fault == Fault::Rebooting || now < s.retry_at {
            return;
        }
        if s.load_fault == Some(Fault::Storage) {
            match Self::load_record(&self.kv) {
                Ok(saved) => {
                    let mut intended = Self::startup_intent(saved.intended, saved.startup);
                    if s.off_after_load {
                        intended.on = false;
                    }
                    s.controller = Controller::new(Some(intended));
                    s.startup = saved.startup;
                    s.dirty = intended != saved.intended;
                    s.load_fault = None;
                    s.fault = Fault::None;
                    s.revision = s.revision.wrapping_add(1);
                }
                Err(fault) => {
                    s.load_fault = Some(fault);
                    s.fault = fault;
                    s.storage_failures = s
                        .storage_failures
                        .saturating_add(u32::from(fault == Fault::Storage));
                    s.retry_at = now.saturating_add(5_000);
                    return;
                }
            }
        }
        if s.load_fault.is_some() {
            return;
        }
        if self.save_intent(&mut s).is_err() {
            return;
        }
        if s.controller.applied().is_none() {
            let _ = Self::reconcile(&mut s, now);
        } else if now >= s.next_verify {
            s.next_verify = now.saturating_add(5_000);
            let applied = s.controller.applied().expect("checked above");
            if s.hardware.verify(applied).is_err() {
                Self::output_failed(&mut s, now);
            }
        }
    }
}
