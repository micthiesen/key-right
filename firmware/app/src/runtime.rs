//! Thread-mode control, persistence and bounded local calibration.
//! Successful output means register readback acknowledged, never measured light.
use core::cell::{Cell, RefCell};
use key_right_core::pca9635::Profile;
use key_right_core::{Command, Controller, LightOutput, LightState, Preset};
use rs_matter_embassy::matter::dm::clusters::app::on_off::StartUpOnOffEnum;
use rs_matter_embassy::matter::error::{Error, ErrorCode};
use rs_matter_embassy::matter::persist::{KvBlobStoreAccess, VENDOR_KEYS_START};

pub const PROFILE_KEY: u16 = VENDOR_KEYS_START + 2;
pub const INTENT_KEY: u16 = VENDOR_KEYS_START + 3;
pub const TEST_DURATION_MS: u64 = 10_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputError {
    Unconfigured,
    Profile,
    Gate,
    Readback,
}

pub trait Hardware: LightOutput<Error = OutputError> {
    fn configure(&mut self, profile: Profile);
    fn shutdown(&mut self) -> Result<(), OutputError>;
    fn verify(&mut self, state: LightState) -> Result<(), OutputError>;
    fn outputs(&self) -> OutputRegisters;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct OutputRegisters {
    pub warm_command: u32,
    pub cool_command: u32,
    pub warm_active: u32,
    pub cool_active: u32,
    pub duty_bits: u8,
    pub divider_q8: u32,
    pub clock_hz: u32,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fault {
    None,
    MissingProfile,
    Storage,
    InvalidRecord,
    Output,
    Testing,
    Rebooting,
}

#[derive(Clone, Copy, Debug)]
pub struct Snapshot {
    pub intended: LightState,
    pub applied: Option<LightState>,
    pub fault: Fault,
    pub configured: bool,
    pub output_failures: u32,
    pub storage_failures: u32,
    pub recoveries: u32,
    pub revision: u32,
}

struct State<H> {
    controller: Controller,
    hardware: H,
    active: Option<Profile>,
    staged: Option<Profile>,
    fault: Fault,
    dirty: bool,
    startup: [u8; 2],
    load_fault: Option<Fault>,
    off_after_load: bool,
    test_until: Option<u64>,
    next_verify: u64,
    retry_at: u64,
    failures: u32,
    storage_failures: u32,
    recoveries: u32,
    revision: u32,
}

#[derive(Default)]
struct SavedState {
    profile: Option<Profile>,
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
fn decode_record(bytes: &[u8]) -> Result<(LightState, [u8; 2]), Error> {
    let (on, preset, startup) = match *bytes {
        [1, on @ 0..=1, preset @ 0..=1] => (on, preset, [0, 0]),
        [2, on @ 0..=1, preset @ 0..=1, one @ 0..=3, two @ 0..=3] => (on, preset, [one, two]),
        _ => return Err(ErrorCode::InvalidData.into()),
    };
    Ok((
        LightState {
            on: on != 0,
            preset: if preset == 0 {
                Preset::One
            } else {
                Preset::Two
            },
        },
        startup,
    ))
}

impl<K: KvBlobStoreAccess, H: Hardware> Runtime<K, H> {
    fn load_records(kv: &K) -> Result<SavedState, Fault> {
        let profile = kv.access(|store, buf| {
            store
                .load(PROFILE_KEY, buf)
                .map_err(|_| Fault::Storage)?
                .map(|bytes| {
                    let p = Profile::decode(bytes).map_err(|_| Fault::InvalidRecord)?;
                    Self::check_profile(p).map_err(|_| Fault::InvalidRecord)?;
                    p.require_commissioned().map_err(|_| Fault::InvalidRecord)?;
                    Ok(p)
                })
                .transpose()
        })?;
        let (intended, startup) = kv.access(|store, buf| {
            match store.load(INTENT_KEY, buf).map_err(|_| Fault::Storage)? {
                None => Ok((LightState::default(), [0, 0])),
                Some(bytes) => decode_record(bytes).map_err(|_| Fault::InvalidRecord),
            }
        })?;
        Ok(SavedState {
            profile,
            intended,
            startup,
        })
    }

    fn startup_intent(restored: LightState, startup: [u8; 2]) -> LightState {
        let mut intended = restored;
        // Two mutually exclusive endpoints: apply in ID order, so endpoint 2 wins
        // when both policies request ON. Null leaves the previous result intact.
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
        let _ = hardware.shutdown();
        let saved = Self::load_records(&kv);
        let load_fault = saved.as_ref().err().copied();
        let saved = saved.unwrap_or_default();
        let active = saved.profile;
        if let Some(p) = active {
            hardware.configure(p);
        }
        let intended = Self::startup_intent(saved.intended, saved.startup);
        Self {
            kv,
            now: Cell::new(0),
            state: RefCell::new(State {
                controller: Controller::new(Some(intended)),
                hardware,
                active,
                staged: None,
                fault: load_fault.unwrap_or(if active.is_none() {
                    Fault::MissingProfile
                } else {
                    Fault::None
                }),
                dirty: intended != saved.intended,
                startup: saved.startup,
                load_fault,
                off_after_load: false,
                test_until: None,
                next_verify: 0,
                retry_at: if load_fault == Some(Fault::Storage) {
                    5_000
                } else {
                    0
                },
                failures: 0,
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
            configured: s.active.is_some(),
            output_failures: s.failures,
            storage_failures: s.storage_failures,
            recoveries: s.recoveries,
            revision: s.revision,
        }
    }

    /// Stop physical output before a deliberate reset without rewriting durable intent.
    pub fn isolate(&self) {
        let mut s = self.state.borrow_mut();
        let _ = s.hardware.shutdown();
        s.controller.invalidate_applied();
        s.fault = Fault::Rebooting;
    }

    /// Admit a local watchdog fault injection only with verified normal output.
    /// Unlike `isolate`, this leaves the output and durable intent untouched.
    pub fn watchdog_test_ready(&self) -> Result<(), Error> {
        {
            let s = self.state.borrow();
            if s.active.is_none()
                || s.staged.is_some()
                || s.test_until.is_some()
                || s.dirty
                || s.fault != Fault::None
            {
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
        // Either Matter endpoint can stop a local candidate test. Outside local
        // provisioning, retain the normal inactive-endpoint OFF no-op semantics.
        if !on && {
            let s = self.state.borrow();
            s.staged.is_some() || s.test_until.is_some()
        } {
            return self.off();
        }
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
        let state = s.controller.intended();
        let data = encode_record(state, s.startup);
        if let Err(e) = self
            .kv
            .access(|store, buf| store.store(INTENT_KEY, &data, buf))
        {
            s.storage_failures = s.storage_failures.saturating_add(1);
            s.fault = Fault::Storage;
            let _ = s.hardware.shutdown();
            s.controller.invalidate_applied();
            s.revision = s.revision.wrapping_add(1);
            return Err(e);
        }
        s.dirty = false;
        Ok(())
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

    fn output_failed(s: &mut State<H>, now: u64) {
        let _ = s.hardware.shutdown();
        s.controller.invalidate_applied();
        s.fault = Fault::Output;
        s.failures = s.failures.saturating_add(1);
        s.retry_at = now.saturating_add(5_000);
        s.revision = s.revision.wrapping_add(1);
    }

    pub fn request(&self, state: LightState) -> Result<(), Error> {
        let mut s = self.state.borrow_mut();
        if s.test_until.is_some() || s.staged.is_some() || s.fault == Fault::Rebooting {
            return Err(ErrorCode::Busy.into());
        }
        if s.active.is_none() || s.load_fault.is_some() {
            return Err(ErrorCode::InvalidState.into());
        }
        let changed = s.controller.command(Command::SelectPreset(state.preset));
        s.dirty |= changed | s.controller.command(Command::SetPower(state.on));
        self.save_intent(&mut s)?;
        Self::reconcile(&mut s, self.now.get())
    }

    /// Local emergency stop works even before a profile is provisioned.
    pub fn off(&self) -> Result<(), Error> {
        let mut s = self.state.borrow_mut();
        let was_test = s.test_until.take().is_some();
        let gate_ok = s.hardware.shutdown().is_ok();
        s.controller.invalidate_applied();
        s.dirty |= s.controller.command(Command::SetPower(false));
        if was_test {
            if let Some(p) = s.active {
                s.hardware.configure(p);
            }
        }
        // Missing/unreadable records are never replaced by a provisional default.
        // Invalid bytes remain available until the explicit profile commit repairs them.
        if let Some(fault) = s.load_fault {
            s.off_after_load |= fault == Fault::Storage;
            s.fault = fault;
            s.dirty = false;
            s.revision = s.revision.wrapping_add(1);
            return if gate_ok {
                Ok(())
            } else {
                Err(ErrorCode::Failure.into())
            };
        }
        self.save_intent(&mut s)?;
        s.revision = s.revision.wrapping_add(1);
        if !gate_ok {
            Self::output_failed(&mut s, self.now.get());
            return Err(ErrorCode::Failure.into());
        }
        if s.active.is_some() {
            Self::reconcile(&mut s, self.now.get())
        } else {
            s.fault = Fault::MissingProfile;
            Ok(())
        }
    }

    pub fn stage(&self, profile: Profile) -> Result<(), Error> {
        Self::check_profile(profile)?;
        if self.state.borrow().load_fault == Some(Fault::Storage) {
            return Err(ErrorCode::Busy.into());
        }
        self.off()?;
        // Imported attestations belong to the previous physical installation.
        self.state.borrow_mut().staged = Some(
            profile
                .with_attestations(0)
                .map_err(|_| ErrorCode::InvalidData)?,
        );
        Ok(())
    }
    fn check_profile(profile: Profile) -> Result<(), Error> {
        // This backend reproduces only the decoded stock presets, without
        // pretending to implement arbitrary PCA modes or uncalibrated levels.
        profile
            .require_stock_pwm()
            .map_err(|_| ErrorCode::InvalidData.into())
    }
    pub fn staged(&self) -> Option<Profile> {
        self.state.borrow().staged
    }
    pub fn profile(&self) -> Option<Profile> {
        self.state.borrow().active
    }
    pub fn attest(&self, bits: u8) -> Result<(), Error> {
        let mut s = self.state.borrow_mut();
        let p = s.staged.ok_or(ErrorCode::InvalidState)?;
        s.staged = Some(
            p.with_attestations(bits)
                .map_err(|_| ErrorCode::InvalidData)?,
        );
        Ok(())
    }
    pub fn test(&self, state: LightState) -> Result<(), Error> {
        self.off()?;
        let mut s = self.state.borrow_mut();
        let p = s.staged.ok_or(ErrorCode::InvalidState)?;
        s.hardware.configure(p);
        s.controller.invalidate_applied();
        // Calibration state never becomes durable intent or a Matter acknowledgement.
        s.test_until = Some(self.now.get().saturating_add(TEST_DURATION_MS));
        s.fault = Fault::Testing;
        if s.hardware.apply(state).is_err() {
            let _ = s.hardware.shutdown();
            s.failures = s.failures.saturating_add(1);
            return Err(ErrorCode::Failure.into());
        }
        Ok(())
    }
    pub fn commit(&self) -> Result<(), Error> {
        let p = self.staged().ok_or(ErrorCode::InvalidState)?;
        Self::check_profile(p)?;
        p.require_commissioned()
            .map_err(|_| ErrorCode::InvalidState)?;
        self.off()?;
        {
            let mut s = self.state.borrow_mut();
            // Explicit commit is the repair boundary for malformed intent records.
            s.dirty |= s.load_fault == Some(Fault::InvalidRecord);
            self.save_intent(&mut s)?;
        }
        if let Err(error) = self
            .kv
            .access(|store, buf| store.store(PROFILE_KEY, &p.encode(), buf))
        {
            let mut s = self.state.borrow_mut();
            s.storage_failures = s.storage_failures.saturating_add(1);
            s.fault = Fault::Storage;
            return Err(error);
        }
        let mut s = self.state.borrow_mut();
        s.active = Some(p);
        s.load_fault = None;
        s.staged = None;
        s.hardware.configure(p);
        s.controller.invalidate_applied();
        s.fault = Fault::None;
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
    pub fn outputs(&self) -> OutputRegisters {
        self.state.borrow().hardware.outputs()
    }
    pub fn tick(&self, now: u64) {
        self.now.set(now);
        if self
            .state
            .borrow()
            .test_until
            .is_some_and(|until| now >= until)
        {
            let _ = self.off();
        }
        let mut s = self.state.borrow_mut();
        if s.load_fault == Some(Fault::Storage) && s.fault != Fault::Rebooting {
            if now < s.retry_at {
                return;
            }
            match Self::load_records(&self.kv) {
                Ok(saved) => {
                    let mut intended = Self::startup_intent(saved.intended, saved.startup);
                    if s.off_after_load {
                        intended.on = false;
                    }
                    s.controller = Controller::new(Some(intended));
                    s.active = saved.profile;
                    if let Some(profile) = saved.profile {
                        s.hardware.configure(profile);
                    }
                    s.startup = saved.startup;
                    s.dirty = intended != saved.intended;
                    s.load_fault = None;
                    s.fault = if s.active.is_some() {
                        Fault::None
                    } else {
                        Fault::MissingProfile
                    };
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
        if s.test_until.is_some()
            || s.staged.is_some()
            || s.active.is_none()
            || s.load_fault.is_some()
            || s.fault == Fault::Rebooting
        {
            return;
        }
        if now < s.retry_at {
            return;
        }
        if self.save_intent(&mut s).is_err() {
            s.retry_at = now.saturating_add(5_000);
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
