//! Durable light targets, independent transitions, and acknowledged PCA state.
//! Persist destinations once; interpolated frames never write flash.
use core::cell::{Cell, RefCell};
use key_right_core::{
    ColorTemperature, Command, Controller, Level, LightOutput, LightState, Preset,
};
use rs_matter_embassy::matter::dm::clusters::app::on_off::StartUpOnOffEnum;
use rs_matter_embassy::matter::error::{Error, ErrorCode};
use rs_matter_embassy::matter::persist::{KvBlobStoreAccess, VENDOR_KEYS_START};

pub const INTENT_KEY: u16 = VENDOR_KEYS_START + 3;
// Scenes Management supports longer transitions than Level/Color commands.
const MAX_TRANSITION_MS: u64 = 60_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputError {
    // The shared simulation runtime has no physical bus to raise this error.
    #[allow(dead_code)]
    Bus,
    Readback,
}
pub trait Hardware: LightOutput<Error = OutputError> {
    const MODE: &'static str = "hardware";
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
    /// Final durable destination, which may differ from an in-progress fade.
    pub intended: LightState,
    /// Last readback-acknowledged frame, not an optical measurement.
    pub applied: Option<LightState>,
    pub fault: Fault,
    pub output_failures: u32,
    pub storage_failures: u32,
    pub recoveries: u32,
    /// Logical attributes or output availability changed; excludes fade frames.
    pub revision: u32,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Startup {
    power: u8,
    level: Option<u8>,
    temperature: Option<ColorTemperature>,
}
#[derive(Clone, Copy)]
struct Transition {
    from: u16,
    to: u16,
    start: u64,
    duration: u64,
}
impl Transition {
    fn value(self, now: u64) -> u16 {
        let elapsed = now.saturating_sub(self.start).min(self.duration);
        if self.duration == 0 || elapsed == self.duration {
            return self.to;
        }
        (i64::from(self.from)
            + (i64::from(self.to) - i64::from(self.from)) * elapsed as i64 / self.duration as i64)
            as u16
    }
    fn remaining(self, now: u64) -> u64 {
        self.duration.saturating_sub(now.saturating_sub(self.start))
    }
}
#[derive(Clone, Copy)]
struct LevelTransition {
    ramp: Transition,
    then: Option<Transition>,
    on_during: bool,
}
impl LevelTransition {
    fn value(self, now: u64) -> u16 {
        self.then
            .filter(|next| now >= next.start)
            .unwrap_or(self.ramp)
            .value(now)
    }
    fn remaining(self, now: u64) -> u64 {
        let last = self.then.unwrap_or(self.ramp);
        last.start.saturating_add(last.duration).saturating_sub(now)
    }
}
#[derive(Clone, Copy, Debug)]
pub enum OffEffect {
    FastFade,
    NoFade,
    SlowFade,
    DyingLight,
}
struct State<H> {
    target: LightState,
    controller: Controller,
    hardware: H,
    level_transition: Option<LevelTransition>,
    temperature_transition: Option<Transition>,
    fault: Fault,
    dirty: bool,
    startup: Startup,
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
    startup: Startup,
    migrated: bool,
}
pub struct Runtime<K, H> {
    kv: K,
    state: RefCell<State<H>>,
    now: Cell<u64>,
}

fn encode_record(state: LightState, startup: Startup) -> [u8; 9] {
    let temperature = state.temperature.get().to_le_bytes();
    let startup_temperature = startup
        .temperature
        .map_or(0, ColorTemperature::get)
        .to_le_bytes();
    [
        3,
        u8::from(state.on),
        state.level.get(),
        temperature[0],
        temperature[1],
        startup.power,
        startup.level.unwrap_or(255),
        startup_temperature[0],
        startup_temperature[1],
    ]
}
fn decode_record(bytes: &[u8]) -> Result<SavedState, Fault> {
    match *bytes {
        [3, on @ 0..=1, level @ 1..=254, lo, hi, power @ 0..=3, startup_level, slo, shi] => {
            let temperature =
                ColorTemperature::new(u16::from_le_bytes([lo, hi])).ok_or(Fault::InvalidRecord)?;
            let startup_temperature = match u16::from_le_bytes([slo, shi]) {
                0 => None,
                value => Some(ColorTemperature::new(value).ok_or(Fault::InvalidRecord)?),
            };
            Ok(SavedState {
                intended: LightState {
                    on: on != 0,
                    level: Level::new(level).unwrap(),
                    temperature,
                },
                startup: Startup {
                    // Legacy On/Toggle policies must not turn a saved Off on.
                    power: if power == 1 { 1 } else { 0 },
                    level: (startup_level != 255).then_some(startup_level),
                    temperature: startup_temperature,
                },
                migrated: power > 1,
            })
        }
        // Convert the previous two-preset record without erasing Matter state.
        // Retain the saved power and selected temperature, not old On/Toggle overrides.
        [2, on @ 0..=1, preset @ 0..=1, one @ 0..=3, two @ 0..=3] => Ok(SavedState {
            intended: LightState {
                on: on != 0,
                temperature: if preset == 0 {
                    Preset::One.temperature()
                } else {
                    Preset::Two.temperature()
                },
                ..LightState::default()
            },
            startup: Startup {
                power: u8::from(if preset == 0 { one } else { two } == 1),
                ..Startup::default()
            },
            migrated: true,
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
    fn startup_intent(mut restored: LightState, startup: Startup) -> LightState {
        // Restore validated power intent unless the user explicitly chose Off.
        // Missing or unreadable intent never becomes an assumed On.
        if startup.power == 1 {
            restored.on = false;
        }
        if let Some(level) = startup.level {
            restored.level = Level::new(level.max(1)).expect("validated startup level");
        }
        if let Some(temperature) = startup.temperature {
            restored.temperature = temperature;
        }
        restored
    }
    pub fn load(kv: K, mut hardware: H) -> Self {
        let off_failed = hardware.shutdown().is_err();
        let saved = Self::load_record(&kv);
        let load_fault = saved.as_ref().err().copied();
        let saved = saved.unwrap_or_default();
        let target = Self::startup_intent(saved.intended, saved.startup);
        Self {
            kv,
            now: Cell::new(0),
            state: RefCell::new(State {
                target,
                controller: Controller::new(Some(target)),
                hardware,
                level_transition: None,
                temperature_transition: None,
                fault: load_fault.unwrap_or(if off_failed {
                    Fault::Output
                } else {
                    Fault::None
                }),
                dirty: saved.migrated || target != saved.intended,
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
            intended: s.target,
            applied: s.controller.applied(),
            fault: s.fault,
            output_failures: s.failures,
            storage_failures: s.storage_failures,
            recoveries: s.recoveries,
            revision: s.revision,
        }
    }
    pub fn acknowledged(&self) -> Result<LightState, Error> {
        self.state
            .borrow()
            .controller
            .applied()
            .ok_or_else(|| ErrorCode::InvalidState.into())
    }
    /// Accepted destination for controllers, while the output adapter is healthy.
    /// Fade frames remain available separately through `acknowledged` and `snapshot`.
    pub fn reported(&self) -> Result<LightState, Error> {
        let s = self.state.borrow();
        if s.fault != Fault::None || s.controller.applied().is_none() {
            return Err(ErrorCode::InvalidState.into());
        }
        Ok(s.target)
    }
    pub fn prepare_reboot(&self) -> bool {
        let mut s = self.state.borrow_mut();
        let verified = s.hardware.shutdown().is_ok();
        s.controller.invalidate_applied();
        s.fault = Fault::Rebooting;
        s.revision = s.revision.wrapping_add(1);
        verified
    }
    pub fn watchdog_test_ready(&self) -> Result<(), Error> {
        {
            let s = self.state.borrow();
            if s.dirty
                || s.fault != Fault::None
                || s.level_transition.is_some()
                || s.temperature_transition.is_some()
            {
                return Err(ErrorCode::InvalidState.into());
            }
        }
        self.verify()
    }
    fn writable(s: &State<H>) -> Result<(), Error> {
        if s.fault == Fault::Rebooting {
            return Err(ErrorCode::Busy.into());
        }
        if s.load_fault.is_some() {
            return Err(ErrorCode::InvalidState.into());
        }
        Ok(())
    }
    pub fn startup(&self) -> Option<StartUpOnOffEnum> {
        (self.state.borrow().startup.power == 1).then_some(StartUpOnOffEnum::Off)
    }
    pub fn set_startup(&self, value: Option<StartUpOnOffEnum>) -> Result<(), Error> {
        // Lighting requires StartUpOnOff. Null means Restore. Reject policies
        // which could turn a valid saved Off into On after a reset.
        let mut startup = self.state.borrow().startup;
        startup.power = match value {
            None => 0,
            Some(StartUpOnOffEnum::Off) => 1,
            Some(StartUpOnOffEnum::On | StartUpOnOffEnum::Toggle) => {
                return Err(ErrorCode::ConstraintError.into());
            }
        };
        self.save_startup(startup)
    }
    pub fn startup_level(&self) -> Option<u8> {
        self.state.borrow().startup.level
    }
    pub fn set_startup_level(&self, value: Option<u8>) -> Result<(), Error> {
        if value == Some(255) {
            return Err(ErrorCode::ConstraintError.into());
        }
        let mut startup = self.state.borrow().startup;
        startup.level = value;
        self.save_startup(startup)
    }
    pub fn startup_temperature(&self) -> Option<ColorTemperature> {
        self.state.borrow().startup.temperature
    }
    pub fn set_startup_temperature(&self, value: Option<ColorTemperature>) -> Result<(), Error> {
        let mut startup = self.state.borrow().startup;
        startup.temperature = value;
        self.save_startup(startup)
    }
    fn save_startup(&self, startup: Startup) -> Result<(), Error> {
        let mut s = self.state.borrow_mut();
        Self::writable(&s)?;
        if startup == s.startup && !s.dirty {
            return Ok(());
        }
        let data = encode_record(s.target, startup);
        if let Err(error) = self
            .kv
            .access(|store, buf| store.store(INTENT_KEY, &data, buf))
        {
            s.storage_failures = s.storage_failures.saturating_add(1);
            return Err(error);
        }
        s.startup = startup;
        s.dirty = false;
        s.revision = s.revision.wrapping_add(1);
        Ok(())
    }
    fn save_intent(&self, s: &mut State<H>) -> Result<(), Error> {
        if !s.dirty {
            return Ok(());
        }
        let data = encode_record(s.target, s.startup);
        if let Err(error) = self
            .kv
            .access(|store, buf| store.store(INTENT_KEY, &data, buf))
        {
            s.storage_failures = s.storage_failures.saturating_add(1);
            s.fault = Fault::Storage;
            if s.hardware.shutdown().is_err() {
                s.failures = s.failures.saturating_add(1);
            }
            s.controller.invalidate_applied();
            s.revision = s.revision.wrapping_add(1);
            s.retry_at = self.now.get().saturating_add(5_000);
            return Err(error);
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
    fn effective(s: &State<H>, now: u64) -> LightState {
        let mut state = s.target;
        if let Some(track) = s.level_transition {
            if track.remaining(now) != 0 {
                state.level = Level::new(track.value(now) as u8).expect("bounded level ramp");
                state.on = track.on_during;
            }
        }
        if let Some(track) = s.temperature_transition {
            state.temperature =
                ColorTemperature::new(track.value(now)).expect("bounded temperature ramp");
        }
        state
    }
    fn reconcile(s: &mut State<H>, now: u64) -> Result<(), Error> {
        let effective = Self::effective(s, now);
        s.controller.command(Command::SetLevel(effective.level));
        s.controller
            .command(Command::SetTemperature(effective.temperature));
        s.controller.command(Command::SetPower(effective.on));
        let previous = s.controller.applied();
        if s.controller.reconcile(&mut s.hardware).is_err() {
            Self::output_failed(s, now);
            return Err(ErrorCode::Failure.into());
        }
        if s.fault == Fault::Output {
            s.recoveries = s.recoveries.saturating_add(1);
        }
        s.fault = Fault::None;
        s.retry_at = 0;
        if previous != s.controller.applied() {
            s.next_verify = now.saturating_add(5_000);
            // Controllers report the destination, not each acknowledged fade frame.
            // Becoming available again still changes their readable attributes.
            if previous.is_none() {
                s.revision = s.revision.wrapping_add(1);
            }
        }
        if s.level_transition.is_some_and(|t| t.remaining(now) == 0) {
            s.level_transition = None;
        }
        if s.temperature_transition
            .is_some_and(|t| t.remaining(now) == 0)
        {
            s.temperature_transition = None;
        }
        Ok(())
    }
    fn commit(&self, s: &mut State<H>, target: LightState) -> Result<(), Error> {
        s.dirty |= s.target != target;
        s.target = target;
        s.revision = s.revision.wrapping_add(1);
        self.save_intent(s)?;
        Self::reconcile(s, self.now.get())
    }
    pub fn request(&self, target: LightState) -> Result<(), Error> {
        let mut s = self.state.borrow_mut();
        Self::writable(&s)?;
        s.level_transition = None;
        s.temperature_transition = None;
        self.commit(&mut s, target)
    }
    /// Commit a validated scene as one durable destination before any output.
    pub fn recall_scene(&self, target: LightState, duration_ms: u64) -> Result<(), Error> {
        if duration_ms > MAX_TRANSITION_MS {
            return Err(ErrorCode::ConstraintError.into());
        }
        let mut s = self.state.borrow_mut();
        Self::writable(&s)?;
        let current = s.controller.applied().unwrap_or(s.target);
        let animate = duration_ms != 0 && (current.on || target.on);
        s.level_transition = animate.then_some(LevelTransition {
            ramp: Transition {
                from: u16::from(if current.on { current.level.get() } else { 1 }),
                to: u16::from(if target.on { target.level.get() } else { 1 }),
                start: self.now.get(),
                duration: duration_ms,
            },
            then: None,
            on_during: true,
        });
        s.temperature_transition = animate.then_some(Transition {
            from: current.temperature.get(),
            to: target.temperature.get(),
            start: self.now.get(),
            duration: duration_ms,
        });
        self.commit(&mut s, target)
    }
    pub fn set_power(&self, on: bool) -> Result<(), Error> {
        let mut s = self.state.borrow_mut();
        Self::writable(&s)?;
        // Repeated On must not jump to the end of an active level/color fade.
        // Off always cancels pending work so a later tick cannot relight it.
        if !on || !s.target.on {
            s.level_transition = None;
            s.temperature_transition = None;
        }
        let target = LightState { on, ..s.target };
        self.commit(&mut s, target)
    }
    pub fn set_level(&self, level: Level, with_on_off: bool) -> Result<(), Error> {
        self.set_level_transition(level.get(), with_on_off, 0)
    }
    /// Effect frames are transient; the durable Off target retains its level.
    pub fn off_with_effect(&self, effect: OffEffect) -> Result<(), Error> {
        let mut s = self.state.borrow_mut();
        Self::writable(&s)?;
        let current = s.controller.applied().unwrap_or(s.target);
        let start = self.now.get();
        let from = u16::from(current.level.get());
        // Percentage effects are relative to the current logical level.
        let (middle, first_ms, last_ms) = match effect {
            OffEffect::FastFade => (1, 800, 0),
            OffEffect::NoFade => (1, 0, 0),
            OffEffect::SlowFade => ((from / 2).max(1), 800, 12_000),
            OffEffect::DyingLight => ((from * 6 / 5).min(254), 500, 1_000),
        };
        s.level_transition = (current.on && first_ms != 0).then_some(LevelTransition {
            ramp: Transition {
                from,
                to: middle,
                start,
                duration: first_ms,
            },
            then: (last_ms != 0).then_some(Transition {
                from: middle,
                to: 1,
                start: start + first_ms,
                duration: last_ms,
            }),
            on_during: true,
        });
        s.temperature_transition = None;
        let target = LightState {
            on: false,
            ..s.target
        };
        self.commit(&mut s, target)
    }
    pub fn set_temperature(&self, temperature: ColorTemperature) -> Result<(), Error> {
        self.set_temperature_transition(temperature, 0)
    }
    pub fn set_level_transition(
        &self,
        level: u8,
        with_on_off: bool,
        duration_ms: u64,
    ) -> Result<(), Error> {
        if level == 255 || duration_ms > MAX_TRANSITION_MS {
            return Err(ErrorCode::ConstraintError.into());
        }
        let mut s = self.state.borrow_mut();
        Self::writable(&s)?;
        let current = s.controller.applied().unwrap_or(s.target);
        let target = LightState {
            level: Level::new(level.max(1)).unwrap(),
            on: if with_on_off {
                level > Level::MIN.get()
            } else {
                s.target.on
            },
            ..s.target
        };
        let animate = duration_ms != 0 && (current.on || target.on);
        s.level_transition = animate.then_some(LevelTransition {
            ramp: Transition {
                from: u16::from(current.level.get()),
                to: u16::from(target.level.get()),
                start: self.now.get(),
                duration: duration_ms,
            },
            then: None,
            on_during: current.on || target.on,
        });
        self.commit(&mut s, target)
    }
    pub fn set_temperature_transition(
        &self,
        temperature: ColorTemperature,
        duration_ms: u64,
    ) -> Result<(), Error> {
        if duration_ms > MAX_TRANSITION_MS {
            return Err(ErrorCode::ConstraintError.into());
        }
        let mut s = self.state.borrow_mut();
        Self::writable(&s)?;
        let current = s.controller.applied().unwrap_or(s.target);
        s.temperature_transition = (duration_ms != 0 && current.on).then_some(Transition {
            from: current.temperature.get(),
            to: temperature.get(),
            start: self.now.get(),
            duration: duration_ms,
        });
        let target = LightState {
            temperature,
            ..s.target
        };
        self.commit(&mut s, target)
    }
    pub fn level_remaining_ms(&self) -> u64 {
        self.state
            .borrow()
            .level_transition
            .map_or(0, |t| t.remaining(self.now.get()))
    }
    pub fn temperature_remaining_ms(&self) -> u64 {
        self.state
            .borrow()
            .temperature_transition
            .map_or(0, |t| t.remaining(self.now.get()))
    }
    pub fn stop_level_transition(&self) -> Result<(), Error> {
        let mut s = self.state.borrow_mut();
        Self::writable(&s)?;
        if s.level_transition.is_none() {
            return Ok(());
        }
        let applied = s.controller.applied().ok_or(ErrorCode::InvalidState)?;
        s.level_transition = None;
        let target = LightState {
            level: applied.level,
            on: applied.on,
            ..s.target
        };
        self.commit(&mut s, target)
    }
    pub fn stop_temperature_transition(&self) -> Result<(), Error> {
        let mut s = self.state.borrow_mut();
        Self::writable(&s)?;
        if s.temperature_transition.is_none() {
            return Ok(());
        }
        let applied = s.controller.applied().ok_or(ErrorCode::InvalidState)?;
        s.temperature_transition = None;
        let target = LightState {
            temperature: applied.temperature,
            ..s.target
        };
        self.commit(&mut s, target)
    }
    /// Local Off attempts shutdown even if intent storage cannot be read.
    /// Explicit local Off repairs malformed intent, never Matter credentials.
    pub fn off(&self) -> Result<(), Error> {
        let mut s = self.state.borrow_mut();
        let stopped = s.hardware.shutdown().is_ok();
        s.controller.invalidate_applied();
        s.level_transition = None;
        s.temperature_transition = None;
        s.dirty |= s.target.on;
        s.target.on = false;
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
                    let mut target = Self::startup_intent(saved.intended, saved.startup);
                    if s.off_after_load {
                        target.on = false;
                    }
                    s.target = target;
                    s.controller = Controller::new(Some(target));
                    s.startup = saved.startup;
                    s.dirty = saved.migrated || target != saved.intended;
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
        if s.load_fault.is_some() || self.save_intent(&mut s).is_err() {
            return;
        }
        if s.controller.applied() != Some(Self::effective(&s, now)) {
            let _ = Self::reconcile(&mut s, now);
        } else {
            if s.level_transition.is_some_and(|t| t.remaining(now) == 0) {
                s.level_transition = None;
            }
            if s.temperature_transition
                .is_some_and(|t| t.remaining(now) == 0)
            {
                s.temperature_transition = None;
            }
            if now >= s.next_verify {
                s.next_verify = now.saturating_add(5_000);
                let applied = s
                    .controller
                    .applied()
                    .expect("effective state acknowledged");
                if s.hardware.verify(applied).is_err() {
                    Self::output_failed(&mut s, now);
                }
            }
        }
    }
}
