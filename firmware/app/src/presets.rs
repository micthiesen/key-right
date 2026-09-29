//! One color-temperature light. Runtime owns durable intent and verified register state.
use crate::runtime::{Hardware, OffEffect, Runtime};
use core::cell::Cell;
use embassy_time::{Duration, Instant, Timer};
use key_right_core::{ColorTemperature, Level, LightState};
use rs_matter_embassy::matter::dm::clusters::app::{
    color_control as color, level_control as level, on_off,
};
use rs_matter_embassy::matter::dm::clusters::decl::scenes_management::{
    AttributeValuePairStruct, AttributeValuePairStructArrayBuilder, CommandId as SceneCommandId,
};
use rs_matter_embassy::matter::dm::clusters::scenes::{
    SceneClusterHandler, SceneInvalidator, ScenesState,
};
use rs_matter_embassy::matter::dm::{
    AsyncHandler, AttrId, Cluster, Dataver, HandlerContext, InvokeContext, InvokeReply,
    MatchContext, ReadContext, ReadReply, WriteContext,
};
use rs_matter_embassy::matter::error::{Error, ErrorCode};
use rs_matter_embassy::matter::persist::KvBlobStoreAccess;
use rs_matter_embassy::matter::tlv::{Nullable, TLVArray, TLVBuilderParent};
use rs_matter_embassy::matter::with;

pub const ENDPOINT: u16 = 1;
pub const ON_OFF_CLUSTER: Cluster<'static> = on_off::FULL_CLUSTER.with_revision(6)
    .with_features(on_off::Feature::LIGHTING.bits())
    .with_attrs(with!(required; on_off::AttributeId::OnOff | on_off::AttributeId::GlobalSceneControl | on_off::AttributeId::OnTime | on_off::AttributeId::OffWaitTime | on_off::AttributeId::StartUpOnOff))
    .with_cmds(with!(on_off::CommandId::Off | on_off::CommandId::On | on_off::CommandId::Toggle | on_off::CommandId::OffWithEffect | on_off::CommandId::OnWithRecallGlobalScene | on_off::CommandId::OnWithTimedOff));
pub const LEVEL_CLUSTER: Cluster<'static> = level::FULL_CLUSTER.with_revision(6)
    .with_features(level::Feature::ON_OFF.bits() | level::Feature::LIGHTING.bits())
    .with_attrs(with!(required; level::AttributeId::CurrentLevel | level::AttributeId::RemainingTime | level::AttributeId::MinLevel | level::AttributeId::MaxLevel | level::AttributeId::StartUpCurrentLevel))
    .with_cmds(with!(level::CommandId::MoveToLevel | level::CommandId::Move | level::CommandId::Step | level::CommandId::Stop | level::CommandId::MoveToLevelWithOnOff | level::CommandId::MoveWithOnOff | level::CommandId::StepWithOnOff | level::CommandId::StopWithOnOff));
pub const COLOR_CLUSTER: Cluster<'static> = color::FULL_CLUSTER.with_revision(7)
    .with_features(color::Feature::COLOR_TEMPERATURE.bits())
    .with_attrs(with!(required; color::AttributeId::ColorTemperatureMireds | color::AttributeId::RemainingTime | color::AttributeId::ColorTempPhysicalMinMireds | color::AttributeId::ColorTempPhysicalMaxMireds | color::AttributeId::CoupleColorTempToLevelMinMireds | color::AttributeId::StartUpColorTemperatureMireds))
    .with_cmds(with!(color::CommandId::MoveToColorTemperature | color::CommandId::MoveColorTemperature | color::CommandId::StepColorTemperature | color::CommandId::StopMoveStep));

pub struct LightHandler<'a, K, H> {
    pub runtime: &'a Runtime<K, H>,
    scenes: &'a ScenesState<16>,
    datavers: [Dataver; 3],
    level_options: Cell<level::OptionsBitmap>,
    color_options: Cell<color::OptionsBitmap>,
    on_level: Cell<Option<u8>>,
    on_time: Cell<u16>,
    off_wait: Cell<u16>,
    timed: Cell<bool>,
    global_scene: Cell<bool>,
    global_scene_state: Cell<Option<LightState>>,
    scene_intent: Cell<Option<LightState>>,
    scene_draft: Cell<Option<SceneDraft>>,
    scene_revision: Cell<u32>,
    coupled_temperature: Cell<bool>,
}
#[derive(Clone, Copy)]
struct SceneDraft {
    target: LightState,
    duration_ms: u64,
    fields: u8,
}
impl<'a, K: KvBlobStoreAccess, H: Hardware> LightHandler<'a, K, H> {
    pub fn new(
        runtime: &'a Runtime<K, H>,
        scenes: &'a ScenesState<16>,
        datavers: [Dataver; 3],
    ) -> Self {
        Self {
            runtime,
            scenes,
            datavers,
            level_options: Cell::new(level::OptionsBitmap::empty()),
            color_options: Cell::new(color::OptionsBitmap::empty()),
            on_level: Cell::new(None),
            on_time: Cell::new(0),
            off_wait: Cell::new(0),
            timed: Cell::new(false),
            global_scene: Cell::new(true),
            global_scene_state: Cell::new(None),
            scene_intent: Cell::new(None),
            scene_draft: Cell::new(None),
            scene_revision: Cell::new(0),
            coupled_temperature: Cell::new(false),
        }
    }
    fn changed(&self) {
        self.scenes.scenable_attribute_changed(ENDPOINT);
        self.scene_intent.set(None);
        self.scene_revision
            .set(self.scene_revision.get().wrapping_add(1));
    }
    fn on(&self) -> Result<bool, Error> {
        Ok(self.runtime.acknowledged()?.on)
    }
    pub fn power(&self, on: bool) -> Result<(), Error> {
        self.changed();
        if on {
            self.global_scene.set(true);
            if self.on_time.get() == 0 {
                self.off_wait.set(0);
            }
            self.timed.set(
                self.on_time.get() > 0
                    && self.on_time.get() != u16::MAX
                    && self.off_wait.get() != u16::MAX,
            );
            if let Some(level) = self
                .on_level
                .get()
                .filter(|_| !self.runtime.acknowledged().is_ok_and(|state| state.on))
            {
                self.runtime.set_level_transition(level, true, 0)?;
            }
        } else {
            self.coupled_temperature.set(false);
            self.on_time.set(0);
            self.timed
                .set(self.off_wait.get() > 0 && self.off_wait.get() != u16::MAX);
        }
        self.datavers[0].changed();
        self.runtime.set_power(on)
    }
    /// Timed-off deadlines are independent of Matter transport restarts.
    pub fn tick(&self, deciseconds: u16) {
        if !self.timed.get() {
            return;
        }
        if self.runtime.snapshot().intended.on {
            self.on_time
                .set(self.on_time.get().saturating_sub(deciseconds));
            if self.on_time.get() == 0 {
                let _ = self.power(false);
                // Automatic expiry clears the guard. Early explicit Off keeps it.
                self.off_wait.set(0);
                self.timed.set(false);
            }
        } else {
            self.on_time.set(0);
            self.off_wait
                .set(self.off_wait.get().saturating_sub(deciseconds));
            if self.off_wait.get() == 0 {
                self.timed.set(false);
            }
        }
        self.datavers[0].changed();
    }
    pub fn timed_on(&self, accept_only_on: bool, on_time: u16, off_wait: u16) -> Result<(), Error> {
        if on_time == u16::MAX || off_wait == u16::MAX {
            return Err(ErrorCode::ConstraintError.into());
        }
        let on = self.on()?;
        if accept_only_on && !on {
            return Ok(());
        }
        if self.off_wait.get() > 0 && !on {
            self.off_wait.set(self.off_wait.get().min(off_wait));
            self.timed
                .set(self.off_wait.get() > 0 && self.off_wait.get() != u16::MAX);
            self.datavers[0].changed();
            return Ok(());
        }
        self.on_time.set(self.on_time.get().max(on_time));
        self.off_wait.set(off_wait);
        let result = self.power(true);
        self.timed
            .set(self.on_time.get() != u16::MAX && self.off_wait.get() != u16::MAX);
        result
    }
    fn save_global_scene(&self) -> Result<(), Error> {
        if self.global_scene.get() {
            self.global_scene_state
                .set(Some(self.runtime.acknowledged()?));
            self.global_scene.set(false);
        }
        Ok(())
    }
    pub fn off_with_effect(&self, effect: OffEffect) -> Result<(), Error> {
        self.save_global_scene()?;
        self.changed();
        self.coupled_temperature.set(false);
        self.on_time.set(0);
        self.timed
            .set(self.off_wait.get() > 0 && self.off_wait.get() != u16::MAX);
        self.datavers[0].changed();
        self.runtime.off_with_effect(effect)
    }
    pub fn recall_global_scene(&self) -> Result<(), Error> {
        if self.global_scene.get() {
            return Ok(());
        }
        let state = self
            .global_scene_state
            .get()
            .ok_or(ErrorCode::InvalidState)?;
        self.changed();
        self.runtime.request(state)?;
        self.global_scene.set(true);
        if self.on_time.get() == 0 {
            self.off_wait.set(0);
        }
        self.datavers[0].changed();
        Ok(())
    }
    pub(crate) fn begin_scene(&self) -> Result<(), Error> {
        if self.scene_draft.get().is_some() {
            return Err(ErrorCode::Busy.into());
        }
        self.scene_draft.set(Some(SceneDraft {
            target: self.runtime.acknowledged()?,
            duration_ms: 0,
            fields: 0,
        }));
        Ok(())
    }
    /// Validate every EFS before changing durable intent or hardware.
    pub(crate) fn stage_scene(
        &self,
        cluster: u32,
        avps: &TLVArray<'_, AttributeValuePairStruct<'_>>,
        duration_ms: u32,
    ) -> Result<(), Error> {
        let mut draft = self.scene_draft.get().ok_or(ErrorCode::InvalidState)?;
        if duration_ms > 60_000_000 {
            return Err(ErrorCode::ConstraintError.into());
        }
        draft.duration_ms = u64::from(duration_ms);
        for avp in avps.iter() {
            let avp = avp?;
            let id = avp.attribute_id()?;
            let field = match (cluster, id) {
                (6, 0) => {
                    draft.target.on = match avp.value_unsigned_8()? {
                        Some(0) => false,
                        Some(1) => true,
                        _ => return Err(ErrorCode::ConstraintError.into()),
                    };
                    1
                }
                (8, 0) => {
                    draft.target.level = avp
                        .value_unsigned_8()?
                        .and_then(Level::new)
                        .ok_or(ErrorCode::ConstraintError)?;
                    2
                }
                (0x300, 7) => {
                    draft.target.temperature = avp
                        .value_unsigned_16()?
                        .and_then(ColorTemperature::new)
                        .ok_or(ErrorCode::ConstraintError)?;
                    4
                }
                (0x300, 0x4001) => {
                    if avp.value_unsigned_8()?
                        != Some(color::EnhancedColorModeEnum::ColorTemperatureMireds as u8)
                    {
                        return Err(ErrorCode::ConstraintError.into());
                    }
                    8
                }
                _ => return Err(ErrorCode::InvalidCommand.into()),
            };
            if draft.fields & field != 0 {
                return Err(ErrorCode::ConstraintError.into());
            }
            draft.fields |= field;
        }
        self.scene_draft.set(Some(draft));
        Ok(())
    }
    pub(crate) fn finish_scene(&self) -> Result<(), Error> {
        let draft = self.scene_draft.take().ok_or(ErrorCode::InvalidState)?;
        self.coupled_temperature.set(false);
        self.on_time.set(0);
        self.off_wait.set(0);
        self.timed.set(false);
        if let Err(error) = self.runtime.recall_scene(draft.target, draft.duration_ms) {
            self.changed();
            return Err(error);
        }
        if draft.target.on {
            self.global_scene.set(true);
        }
        self.scene_intent.set(Some(draft.target));
        self.datavers[0].changed();
        Ok(())
    }
    pub(crate) fn abort_scene(&self) {
        self.scene_draft.set(None);
        self.changed();
    }
    fn execute_level(
        &self,
        with_on_off: bool,
        mask: level::OptionsBitmap,
        over: level::OptionsBitmap,
    ) -> Result<bool, Error> {
        Ok(with_on_off
            || self.on()?
            || if mask.contains(level::OptionsBitmap::EXECUTE_IF_OFF) {
                over.contains(level::OptionsBitmap::EXECUTE_IF_OFF)
            } else {
                self.level_options
                    .get()
                    .contains(level::OptionsBitmap::EXECUTE_IF_OFF)
            })
    }
    fn execute_color(
        &self,
        mask: color::OptionsBitmap,
        over: color::OptionsBitmap,
    ) -> Result<bool, Error> {
        Ok(self.on()?
            || if mask.contains(color::OptionsBitmap::EXECUTE_IF_OFF) {
                over.contains(color::OptionsBitmap::EXECUTE_IF_OFF)
            } else {
                self.color_options
                    .get()
                    .contains(color::OptionsBitmap::EXECUTE_IF_OFF)
            })
    }
    pub fn level_to(
        &self,
        raw: u8,
        with_on_off: bool,
        time_ds: Option<u16>,
        mask: level::OptionsBitmap,
        over: level::OptionsBitmap,
    ) -> Result<(), Error> {
        if raw == 255 {
            return Err(ErrorCode::ConstraintError.into());
        }
        if !self.execute_level(with_on_off, mask, over)? {
            return Ok(());
        }
        self.changed();
        let duration_ms = u64::from(time_ds.unwrap_or(0)) * 100;
        self.runtime
            .set_level_transition(raw, with_on_off, duration_ms)?;
        let options =
            (self.level_options.get().bits() & !mask.bits()) | (over.bits() & mask.bits());
        self.coupled_temperature.set(false);
        if options & level::OptionsBitmap::COUPLE_COLOR_TEMP_TO_LEVEL.bits() != 0 {
            let mireds = ColorTemperature::MAX.get()
                - (u16::from(raw.max(1)) - 1)
                    * (ColorTemperature::MAX.get() - ColorTemperature::MIN.get())
                    / 253;
            self.runtime.set_temperature_transition(
                ColorTemperature::new(mireds).expect("bounded temperature"),
                duration_ms,
            )?;
            self.coupled_temperature.set(true);
        }
        if with_on_off && raw > 0 {
            self.global_scene.set(true);
        }
        Ok(())
    }
    pub fn level_move(
        &self,
        mode: level::MoveModeEnum,
        rate: Option<u8>,
        with_on_off: bool,
        mask: level::OptionsBitmap,
        over: level::OptionsBitmap,
    ) -> Result<(), Error> {
        if !self.execute_level(with_on_off, mask, over)? {
            return Ok(());
        }
        let rate = rate.unwrap_or(254);
        if rate == 0 {
            return Err(ErrorCode::InvalidCommand.into());
        }
        let current = self.runtime.acknowledged()?.level.get();
        let target = match mode {
            level::MoveModeEnum::Up => 254,
            level::MoveModeEnum::Down => {
                if with_on_off {
                    0
                } else {
                    1
                }
            }
        };
        let ds = (u16::from(current.abs_diff(target)) * 10).div_ceil(u16::from(rate));
        self.level_to(target, with_on_off, Some(ds), mask, over)
    }
    pub fn level_step(
        &self,
        mode: level::StepModeEnum,
        step: u8,
        time_ds: Option<u16>,
        with_on_off: bool,
        mask: level::OptionsBitmap,
        over: level::OptionsBitmap,
    ) -> Result<(), Error> {
        if !self.execute_level(with_on_off, mask, over)? {
            return Ok(());
        }
        let current = self.runtime.acknowledged()?.level.get();
        let target = match mode {
            level::StepModeEnum::Up => current.saturating_add(step).min(254),
            level::StepModeEnum::Down => current.saturating_sub(step).max(u8::from(!with_on_off)),
        };
        self.level_to(target, with_on_off, time_ds, mask, over)
    }
    pub fn stop_level(
        &self,
        with_on_off: bool,
        mask: level::OptionsBitmap,
        over: level::OptionsBitmap,
    ) -> Result<(), Error> {
        if self.execute_level(with_on_off, mask, over)? {
            self.changed();
            self.runtime.stop_level_transition()?;
            if self.coupled_temperature.replace(false) {
                self.runtime.stop_temperature_transition()?;
            }
        }
        Ok(())
    }
    pub fn color_to(
        &self,
        mireds: u16,
        time_ds: u16,
        mask: color::OptionsBitmap,
        over: color::OptionsBitmap,
    ) -> Result<(), Error> {
        if mireds == 0 || mireds > 0xfeff {
            return Err(ErrorCode::ConstraintError.into());
        }
        if !self.execute_color(mask, over)? {
            return Ok(());
        }
        self.changed();
        self.coupled_temperature.set(false);
        let target = temperature(mireds);
        self.runtime
            .set_temperature_transition(target, u64::from(time_ds) * 100)
    }
    pub fn color_move(
        &self,
        mode: color::MoveModeEnum,
        rate: u16,
        bounds: (u16, u16),
        mask: color::OptionsBitmap,
        over: color::OptionsBitmap,
    ) -> Result<(), Error> {
        if mode == color::MoveModeEnum::Stop {
            return self.stop_color(mask, over);
        }
        if rate == 0 {
            return Err(ErrorCode::InvalidCommand.into());
        }
        if !self.execute_color(mask, over)? {
            return Ok(());
        }
        let (lo, hi) = color_bounds(bounds)?;
        let current = self.runtime.acknowledged()?.temperature.get();
        let target = match mode {
            color::MoveModeEnum::Up => hi,
            color::MoveModeEnum::Down => lo,
            color::MoveModeEnum::Stop => unreachable!(),
        };
        self.changed();
        self.coupled_temperature.set(false);
        self.runtime.set_temperature_transition(
            temperature(target),
            (u64::from(current.abs_diff(target)) * 1000).div_ceil(u64::from(rate)),
        )
    }
    pub fn color_step(
        &self,
        mode: color::StepModeEnum,
        step: u16,
        time_ds: u16,
        bounds: (u16, u16),
        mask: color::OptionsBitmap,
        over: color::OptionsBitmap,
    ) -> Result<(), Error> {
        if !self.execute_color(mask, over)? {
            return Ok(());
        }
        let (lo, hi) = color_bounds(bounds)?;
        let current = self.runtime.acknowledged()?.temperature.get();
        let target = match mode {
            color::StepModeEnum::Up => current.saturating_add(step),
            color::StepModeEnum::Down => current.saturating_sub(step),
        }
        .clamp(lo, hi);
        self.color_to(target, time_ds, mask, over)
    }
    pub fn stop_color(
        &self,
        mask: color::OptionsBitmap,
        over: color::OptionsBitmap,
    ) -> Result<(), Error> {
        if self.execute_color(mask, over)? {
            self.changed();
            self.coupled_temperature.set(false);
            self.runtime.stop_temperature_transition()?;
        }
        Ok(())
    }
    async fn report(
        &self,
        ctx: impl HandlerContext,
        index: usize,
        cluster: u32,
    ) -> Result<(), Error> {
        let initial = self.runtime.snapshot();
        let mut revision = initial.revision;
        let mut previous_output = (initial.intended, initial.applied);
        let mut scene_revision = self.scene_revision.get();
        let mut dataver = self.datavers[index].get();
        loop {
            Timer::after(Duration::from_millis(250)).await;
            let snapshot = self.runtime.snapshot();
            let next = snapshot.revision;
            if index == 0 {
                let output = (snapshot.intended, snapshot.applied);
                if output != previous_output
                    && !self.scene_intent.get().is_some_and(|expected| {
                        snapshot.intended == expected && snapshot.applied.is_some()
                    })
                {
                    self.changed();
                }
                previous_output = output;
                if scene_revision != self.scene_revision.get() {
                    ctx.notify_cluster_changed(
                        ENDPOINT,
                        rs_matter_embassy::matter::dm::clusters::scenes::FULL_CLUSTER.id,
                    );
                    scene_revision = self.scene_revision.get();
                }
            }
            if next != revision {
                self.datavers[index].changed();
                revision = next;
            }
            if dataver != self.datavers[index].get() {
                ctx.notify_cluster_changed(ENDPOINT, cluster);
                dataver = self.datavers[index].get();
            }
        }
    }
}
fn temperature(value: u16) -> ColorTemperature {
    ColorTemperature::new(value.clamp(ColorTemperature::MIN.get(), ColorTemperature::MAX.get()))
        .expect("bounded temperature")
}
fn color_bounds((min, max): (u16, u16)) -> Result<(u16, u16), Error> {
    let lo = if min == 0 {
        ColorTemperature::MIN.get()
    } else {
        min.max(ColorTemperature::MIN.get())
    };
    let hi = if max == 0 {
        ColorTemperature::MAX.get()
    } else {
        max.min(ColorTemperature::MAX.get())
    };
    if lo > hi {
        Err(ErrorCode::ConstraintError.into())
    } else {
        Ok((lo, hi))
    }
}
fn remaining(ms: u64) -> u16 {
    ms.div_ceil(100).min(65534) as u16
}

impl<K: KvBlobStoreAccess, H: Hardware> on_off::ClusterAsyncHandler for LightHandler<'_, K, H> {
    const CLUSTER: Cluster<'static> = ON_OFF_CLUSTER;
    fn dataver(&self) -> u32 {
        self.datavers[0].get()
    }
    fn dataver_changed(&self) {
        self.datavers[0].changed();
    }
    async fn run(&self, ctx: impl HandlerContext) -> Result<(), Error> {
        self.report(ctx, 0, ON_OFF_CLUSTER.id).await
    }
    async fn on_off(&self, _ctx: impl ReadContext) -> Result<bool, Error> {
        self.on()
    }
    async fn global_scene_control(&self, _ctx: impl ReadContext) -> Result<bool, Error> {
        Ok(self.global_scene.get())
    }
    async fn on_time(&self, _ctx: impl ReadContext) -> Result<u16, Error> {
        Ok(self.on_time.get())
    }
    async fn off_wait_time(&self, _ctx: impl ReadContext) -> Result<u16, Error> {
        Ok(self.off_wait.get())
    }
    async fn start_up_on_off(
        &self,
        _ctx: impl ReadContext,
    ) -> Result<Nullable<on_off::StartUpOnOffEnum>, Error> {
        Ok(self.runtime.startup().into())
    }
    async fn set_start_up_on_off(
        &self,
        ctx: impl WriteContext,
        value: Nullable<on_off::StartUpOnOffEnum>,
    ) -> Result<(), Error> {
        self.runtime.set_startup(value.into_option())?;
        ctx.notify_changed();
        Ok(())
    }
    async fn set_on_time(&self, ctx: impl WriteContext, value: u16) -> Result<(), Error> {
        self.on_time.set(value);
        ctx.notify_changed();
        Ok(())
    }
    async fn set_off_wait_time(&self, ctx: impl WriteContext, value: u16) -> Result<(), Error> {
        self.off_wait.set(value);
        ctx.notify_changed();
        Ok(())
    }
    async fn handle_off(&self, _ctx: impl InvokeContext) -> Result<(), Error> {
        self.power(false)
    }
    async fn handle_on(&self, _ctx: impl InvokeContext) -> Result<(), Error> {
        self.power(true)
    }
    async fn handle_toggle(&self, _ctx: impl InvokeContext) -> Result<(), Error> {
        self.power(!self.on()?)
    }
    async fn handle_off_with_effect(
        &self,
        _ctx: impl InvokeContext,
        r: on_off::OffWithEffectRequest<'_>,
    ) -> Result<(), Error> {
        let effect = match (r.effect_identifier()?, r.effect_variant()?) {
            (on_off::EffectIdentifierEnum::DelayedAllOff, 0) => OffEffect::FastFade,
            (on_off::EffectIdentifierEnum::DelayedAllOff, 1) => OffEffect::NoFade,
            (on_off::EffectIdentifierEnum::DelayedAllOff, 2) => OffEffect::SlowFade,
            (on_off::EffectIdentifierEnum::DyingLight, 0) => OffEffect::DyingLight,
            _ => return Err(ErrorCode::InvalidCommand.into()),
        };
        self.off_with_effect(effect)
    }
    async fn handle_on_with_recall_global_scene(
        &self,
        _ctx: impl InvokeContext,
    ) -> Result<(), Error> {
        self.recall_global_scene()
    }
    async fn handle_on_with_timed_off(
        &self,
        _ctx: impl InvokeContext,
        r: on_off::OnWithTimedOffRequest<'_>,
    ) -> Result<(), Error> {
        self.timed_on(
            r.on_off_control()?
                .contains(on_off::OnOffControlBitmap::ACCEPT_ONLY_WHEN_ON),
            r.on_time()?,
            r.off_wait_time()?,
        )
    }
}

impl<K: KvBlobStoreAccess, H: Hardware> level::ClusterAsyncHandler for LightHandler<'_, K, H> {
    const CLUSTER: Cluster<'static> = LEVEL_CLUSTER;
    fn dataver(&self) -> u32 {
        self.datavers[1].get()
    }
    fn dataver_changed(&self) {
        self.datavers[1].changed();
    }
    async fn run(&self, ctx: impl HandlerContext) -> Result<(), Error> {
        self.report(ctx, 1, LEVEL_CLUSTER.id).await
    }
    async fn current_level(&self, _ctx: impl ReadContext) -> Result<Nullable<u8>, Error> {
        Ok(Nullable::some(self.runtime.acknowledged()?.level.get()))
    }
    async fn remaining_time(&self, _ctx: impl ReadContext) -> Result<u16, Error> {
        Ok(remaining(self.runtime.level_remaining_ms()))
    }
    async fn min_level(&self, _ctx: impl ReadContext) -> Result<u8, Error> {
        Ok(Level::MIN.get())
    }
    async fn max_level(&self, _ctx: impl ReadContext) -> Result<u8, Error> {
        Ok(Level::MAX.get())
    }
    async fn options(&self, _ctx: impl ReadContext) -> Result<level::OptionsBitmap, Error> {
        Ok(self.level_options.get())
    }
    async fn on_level(&self, _ctx: impl ReadContext) -> Result<Nullable<u8>, Error> {
        Ok(self.on_level.get().into())
    }
    async fn start_up_current_level(&self, _ctx: impl ReadContext) -> Result<Nullable<u8>, Error> {
        Ok(self.runtime.startup_level().into())
    }
    async fn set_options(
        &self,
        ctx: impl WriteContext,
        value: level::OptionsBitmap,
    ) -> Result<(), Error> {
        if value.bits() & !3 != 0 {
            return Err(ErrorCode::ConstraintError.into());
        }
        self.level_options.set(value);
        ctx.notify_changed();
        Ok(())
    }
    async fn set_on_level(&self, ctx: impl WriteContext, value: Nullable<u8>) -> Result<(), Error> {
        let value = value.into_option();
        if value.is_some_and(|v| v == 0 || v == 255) {
            return Err(ErrorCode::ConstraintError.into());
        }
        self.on_level.set(value);
        ctx.notify_changed();
        Ok(())
    }
    async fn set_start_up_current_level(
        &self,
        ctx: impl WriteContext,
        value: Nullable<u8>,
    ) -> Result<(), Error> {
        self.runtime.set_startup_level(value.into_option())?;
        ctx.notify_changed();
        Ok(())
    }
    async fn handle_move_to_level(
        &self,
        _ctx: impl InvokeContext,
        r: level::MoveToLevelRequest<'_>,
    ) -> Result<(), Error> {
        self.level_to(
            r.level()?,
            false,
            r.transition_time()?.into_option(),
            r.options_mask()?,
            r.options_override()?,
        )
    }
    async fn handle_move(
        &self,
        _ctx: impl InvokeContext,
        r: level::MoveRequest<'_>,
    ) -> Result<(), Error> {
        self.level_move(
            r.move_mode()?,
            r.rate()?.into_option(),
            false,
            r.options_mask()?,
            r.options_override()?,
        )
    }
    async fn handle_step(
        &self,
        _ctx: impl InvokeContext,
        r: level::StepRequest<'_>,
    ) -> Result<(), Error> {
        self.level_step(
            r.step_mode()?,
            r.step_size()?,
            r.transition_time()?.into_option(),
            false,
            r.options_mask()?,
            r.options_override()?,
        )
    }
    async fn handle_stop(
        &self,
        _ctx: impl InvokeContext,
        r: level::StopRequest<'_>,
    ) -> Result<(), Error> {
        self.stop_level(false, r.options_mask()?, r.options_override()?)
    }
    async fn handle_move_to_level_with_on_off(
        &self,
        _ctx: impl InvokeContext,
        r: level::MoveToLevelWithOnOffRequest<'_>,
    ) -> Result<(), Error> {
        self.level_to(
            r.level()?,
            true,
            r.transition_time()?.into_option(),
            r.options_mask()?,
            r.options_override()?,
        )
    }
    async fn handle_move_with_on_off(
        &self,
        _ctx: impl InvokeContext,
        r: level::MoveWithOnOffRequest<'_>,
    ) -> Result<(), Error> {
        self.level_move(
            r.move_mode()?,
            r.rate()?.into_option(),
            true,
            r.options_mask()?,
            r.options_override()?,
        )
    }
    async fn handle_step_with_on_off(
        &self,
        _ctx: impl InvokeContext,
        r: level::StepWithOnOffRequest<'_>,
    ) -> Result<(), Error> {
        self.level_step(
            r.step_mode()?,
            r.step_size()?,
            r.transition_time()?.into_option(),
            true,
            r.options_mask()?,
            r.options_override()?,
        )
    }
    async fn handle_stop_with_on_off(
        &self,
        _ctx: impl InvokeContext,
        r: level::StopWithOnOffRequest<'_>,
    ) -> Result<(), Error> {
        self.stop_level(true, r.options_mask()?, r.options_override()?)
    }
    async fn handle_move_to_closest_frequency(
        &self,
        _ctx: impl InvokeContext,
        _r: level::MoveToClosestFrequencyRequest<'_>,
    ) -> Result<(), Error> {
        Err(ErrorCode::InvalidCommand.into())
    }
}
impl<K: KvBlobStoreAccess, H: Hardware> color::ClusterAsyncHandler for LightHandler<'_, K, H> {
    const CLUSTER: Cluster<'static> = COLOR_CLUSTER;
    fn dataver(&self) -> u32 {
        self.datavers[2].get()
    }
    fn dataver_changed(&self) {
        self.datavers[2].changed();
    }
    async fn run(&self, ctx: impl HandlerContext) -> Result<(), Error> {
        self.report(ctx, 2, COLOR_CLUSTER.id).await
    }
    async fn color_temperature_mireds(&self, _ctx: impl ReadContext) -> Result<u16, Error> {
        Ok(self.runtime.acknowledged()?.temperature.get())
    }
    async fn remaining_time(&self, _ctx: impl ReadContext) -> Result<u16, Error> {
        Ok(remaining(self.runtime.temperature_remaining_ms()))
    }
    async fn color_mode(&self, _ctx: impl ReadContext) -> Result<color::ColorModeEnum, Error> {
        Ok(color::ColorModeEnum::ColorTemperatureMireds)
    }
    async fn enhanced_color_mode(
        &self,
        _ctx: impl ReadContext,
    ) -> Result<color::EnhancedColorModeEnum, Error> {
        Ok(color::EnhancedColorModeEnum::ColorTemperatureMireds)
    }
    async fn options(&self, _ctx: impl ReadContext) -> Result<color::OptionsBitmap, Error> {
        Ok(self.color_options.get())
    }
    async fn number_of_primaries(&self, _ctx: impl ReadContext) -> Result<Nullable<u8>, Error> {
        Ok(Nullable::none())
    }
    async fn color_capabilities(
        &self,
        _ctx: impl ReadContext,
    ) -> Result<color::ColorCapabilitiesBitmap, Error> {
        Ok(color::ColorCapabilitiesBitmap::COLOR_TEMPERATURE)
    }
    async fn color_temp_physical_min_mireds(&self, _ctx: impl ReadContext) -> Result<u16, Error> {
        Ok(ColorTemperature::MIN.get())
    }
    async fn color_temp_physical_max_mireds(&self, _ctx: impl ReadContext) -> Result<u16, Error> {
        Ok(ColorTemperature::MAX.get())
    }
    async fn couple_color_temp_to_level_min_mireds(
        &self,
        _ctx: impl ReadContext,
    ) -> Result<u16, Error> {
        Ok(ColorTemperature::MIN.get())
    }
    async fn start_up_color_temperature_mireds(
        &self,
        _ctx: impl ReadContext,
    ) -> Result<Nullable<u16>, Error> {
        Ok(self
            .runtime
            .startup_temperature()
            .map(ColorTemperature::get)
            .into())
    }
    async fn set_options(
        &self,
        ctx: impl WriteContext,
        value: color::OptionsBitmap,
    ) -> Result<(), Error> {
        if value.bits() & !1 != 0 {
            return Err(ErrorCode::ConstraintError.into());
        }
        self.color_options.set(value);
        ctx.notify_changed();
        Ok(())
    }
    async fn set_start_up_color_temperature_mireds(
        &self,
        ctx: impl WriteContext,
        value: Nullable<u16>,
    ) -> Result<(), Error> {
        let value = value.into_option();
        if value.is_some_and(|v| v == 0 || v > 0xfeff) {
            return Err(ErrorCode::ConstraintError.into());
        }
        self.runtime
            .set_startup_temperature(value.map(temperature))?;
        ctx.notify_changed();
        Ok(())
    }
    async fn handle_move_to_color_temperature(
        &self,
        _ctx: impl InvokeContext,
        r: color::MoveToColorTemperatureRequest<'_>,
    ) -> Result<(), Error> {
        self.color_to(
            r.color_temperature_mireds()?,
            r.transition_time()?,
            r.options_mask()?,
            r.options_override()?,
        )
    }
    async fn handle_move_color_temperature(
        &self,
        _ctx: impl InvokeContext,
        r: color::MoveColorTemperatureRequest<'_>,
    ) -> Result<(), Error> {
        self.color_move(
            r.move_mode()?,
            r.rate()?,
            (
                r.color_temperature_minimum_mireds()?,
                r.color_temperature_maximum_mireds()?,
            ),
            r.options_mask()?,
            r.options_override()?,
        )
    }
    async fn handle_step_color_temperature(
        &self,
        _ctx: impl InvokeContext,
        r: color::StepColorTemperatureRequest<'_>,
    ) -> Result<(), Error> {
        self.color_step(
            r.step_mode()?,
            r.step_size()?,
            r.transition_time()?,
            (
                r.color_temperature_minimum_mireds()?,
                r.color_temperature_maximum_mireds()?,
            ),
            r.options_mask()?,
            r.options_override()?,
        )
    }
    async fn handle_stop_move_step(
        &self,
        _ctx: impl InvokeContext,
        r: color::StopMoveStepRequest<'_>,
    ) -> Result<(), Error> {
        self.stop_color(r.options_mask()?, r.options_override()?)
    }
    async fn handle_move_to_hue(
        &self,
        _ctx: impl InvokeContext,
        _r: color::MoveToHueRequest<'_>,
    ) -> Result<(), Error> {
        Err(ErrorCode::InvalidCommand.into())
    }
    async fn handle_move_hue(
        &self,
        _ctx: impl InvokeContext,
        _r: color::MoveHueRequest<'_>,
    ) -> Result<(), Error> {
        Err(ErrorCode::InvalidCommand.into())
    }
    async fn handle_step_hue(
        &self,
        _ctx: impl InvokeContext,
        _r: color::StepHueRequest<'_>,
    ) -> Result<(), Error> {
        Err(ErrorCode::InvalidCommand.into())
    }
    async fn handle_move_to_saturation(
        &self,
        _ctx: impl InvokeContext,
        _r: color::MoveToSaturationRequest<'_>,
    ) -> Result<(), Error> {
        Err(ErrorCode::InvalidCommand.into())
    }
    async fn handle_move_saturation(
        &self,
        _ctx: impl InvokeContext,
        _r: color::MoveSaturationRequest<'_>,
    ) -> Result<(), Error> {
        Err(ErrorCode::InvalidCommand.into())
    }
    async fn handle_step_saturation(
        &self,
        _ctx: impl InvokeContext,
        _r: color::StepSaturationRequest<'_>,
    ) -> Result<(), Error> {
        Err(ErrorCode::InvalidCommand.into())
    }
    async fn handle_move_to_hue_and_saturation(
        &self,
        _ctx: impl InvokeContext,
        _r: color::MoveToHueAndSaturationRequest<'_>,
    ) -> Result<(), Error> {
        Err(ErrorCode::InvalidCommand.into())
    }
    async fn handle_move_to_color(
        &self,
        _ctx: impl InvokeContext,
        _r: color::MoveToColorRequest<'_>,
    ) -> Result<(), Error> {
        Err(ErrorCode::InvalidCommand.into())
    }
    async fn handle_move_color(
        &self,
        _ctx: impl InvokeContext,
        _r: color::MoveColorRequest<'_>,
    ) -> Result<(), Error> {
        Err(ErrorCode::InvalidCommand.into())
    }
    async fn handle_step_color(
        &self,
        _ctx: impl InvokeContext,
        _r: color::StepColorRequest<'_>,
    ) -> Result<(), Error> {
        Err(ErrorCode::InvalidCommand.into())
    }
    async fn handle_enhanced_move_to_hue(
        &self,
        _ctx: impl InvokeContext,
        _r: color::EnhancedMoveToHueRequest<'_>,
    ) -> Result<(), Error> {
        Err(ErrorCode::InvalidCommand.into())
    }
    async fn handle_enhanced_move_hue(
        &self,
        _ctx: impl InvokeContext,
        _r: color::EnhancedMoveHueRequest<'_>,
    ) -> Result<(), Error> {
        Err(ErrorCode::InvalidCommand.into())
    }
    async fn handle_enhanced_step_hue(
        &self,
        _ctx: impl InvokeContext,
        _r: color::EnhancedStepHueRequest<'_>,
    ) -> Result<(), Error> {
        Err(ErrorCode::InvalidCommand.into())
    }
    async fn handle_enhanced_move_to_hue_and_saturation(
        &self,
        _ctx: impl InvokeContext,
        _r: color::EnhancedMoveToHueAndSaturationRequest<'_>,
    ) -> Result<(), Error> {
        Err(ErrorCode::InvalidCommand.into())
    }
    async fn handle_color_loop_set(
        &self,
        _ctx: impl InvokeContext,
        _r: color::ColorLoopSetRequest<'_>,
    ) -> Result<(), Error> {
        Err(ErrorCode::InvalidCommand.into())
    }
}

/// The SDK recalls clusters separately. Stage them and commit one complete light
/// target before returning the IM status, so a failed EFS cannot leave a hybrid.
pub struct AtomicScenes<'a, K, H, S> {
    pub inner: S,
    pub light: &'a LightHandler<'a, K, H>,
}
impl<K: KvBlobStoreAccess, H: Hardware, S: AsyncHandler> AsyncHandler
    for AtomicScenes<'_, K, H, S>
{
    async fn read(&self, ctx: impl ReadContext, reply: impl ReadReply) -> Result<(), Error> {
        self.inner.read(ctx, reply).await
    }
    async fn write(&self, ctx: impl WriteContext) -> Result<(), Error> {
        self.inner.write(ctx).await
    }
    async fn invoke(&self, ctx: impl InvokeContext, reply: impl InvokeReply) -> Result<(), Error> {
        if ctx.cmd().cmd_id != SceneCommandId::RecallScene as u32 {
            let stored = ctx.cmd().cmd_id == SceneCommandId::StoreScene as u32;
            let result = self.inner.invoke(ctx, reply).await;
            if stored && result.is_ok() {
                // A scene captured during a transition describes the current
                // frame, so later frames must invalidate it.
                self.light.scene_intent.set(None);
            }
            return result;
        }
        self.light.begin_scene()?;
        // All registry apply functions finish immediately. The SDK does not
        // write a RecallScene response; the IM status follows this result.
        if let Err(error) = self.inner.invoke(&ctx, reply).await {
            self.light.abort_scene();
            return Err(error);
        }
        self.light.finish_scene()
    }
    fn bump_dataver(&self, ctx: impl MatchContext) {
        self.inner.bump_dataver(ctx);
    }
    async fn run(&self, ctx: impl HandlerContext) -> Result<(), Error> {
        self.inner.run(ctx).await
    }
}

/// Scene adapters capture each cluster and stage recalls through AtomicScenes.
pub struct ScenePower<'a, K, H>(pub &'a LightHandler<'a, K, H>);
pub struct SceneLevel<'a, K, H>(pub &'a LightHandler<'a, K, H>);
pub struct SceneTemperature<'a, K, H>(pub &'a LightHandler<'a, K, H>);
impl<K: KvBlobStoreAccess, H: Hardware> SceneClusterHandler for ScenePower<'_, K, H> {
    const CLUSTER_ID: u32 = ON_OFF_CLUSTER.id;
    fn endpoint_id(&self) -> u16 {
        ENDPOINT
    }
    fn is_scenable_attribute(id: AttrId) -> bool {
        id == on_off::AttributeId::OnOff as _
    }
    fn capture<P: TLVBuilderParent>(
        &self,
        avps: AttributeValuePairStructArrayBuilder<P>,
    ) -> Result<AttributeValuePairStructArrayBuilder<P>, Error> {
        avps.push_u8(on_off::AttributeId::OnOff as _, u8::from(self.0.on()?))
    }
    async fn apply<C: HandlerContext>(
        &self,
        _ctx: &C,
        avps: &TLVArray<'_, AttributeValuePairStruct<'_>>,
        ms: u32,
    ) -> Result<(), Error> {
        self.0.stage_scene(Self::CLUSTER_ID, avps, ms)
    }
}
impl<K: KvBlobStoreAccess, H: Hardware> SceneClusterHandler for SceneLevel<'_, K, H> {
    const CLUSTER_ID: u32 = LEVEL_CLUSTER.id;
    fn endpoint_id(&self) -> u16 {
        ENDPOINT
    }
    fn is_scenable_attribute(id: AttrId) -> bool {
        id == level::AttributeId::CurrentLevel as _
    }
    fn capture<P: TLVBuilderParent>(
        &self,
        avps: AttributeValuePairStructArrayBuilder<P>,
    ) -> Result<AttributeValuePairStructArrayBuilder<P>, Error> {
        avps.push_u8(
            level::AttributeId::CurrentLevel as _,
            self.0.runtime.acknowledged()?.level.get(),
        )
    }
    async fn apply<C: HandlerContext>(
        &self,
        _ctx: &C,
        avps: &TLVArray<'_, AttributeValuePairStruct<'_>>,
        ms: u32,
    ) -> Result<(), Error> {
        self.0.stage_scene(Self::CLUSTER_ID, avps, ms)
    }
}
impl<K: KvBlobStoreAccess, H: Hardware> SceneClusterHandler for SceneTemperature<'_, K, H> {
    const CLUSTER_ID: u32 = COLOR_CLUSTER.id;
    fn endpoint_id(&self) -> u16 {
        ENDPOINT
    }
    fn is_scenable_attribute(id: AttrId) -> bool {
        id == color::AttributeId::ColorTemperatureMireds as _
            || id == color::AttributeId::EnhancedColorMode as _
    }
    fn capture<P: TLVBuilderParent>(
        &self,
        avps: AttributeValuePairStructArrayBuilder<P>,
    ) -> Result<AttributeValuePairStructArrayBuilder<P>, Error> {
        avps.push_u16(
            color::AttributeId::ColorTemperatureMireds as _,
            self.0.runtime.acknowledged()?.temperature.get(),
        )?
        .push_u8(
            color::AttributeId::EnhancedColorMode as _,
            color::EnhancedColorModeEnum::ColorTemperatureMireds as u8,
        )
    }
    async fn apply<C: HandlerContext>(
        &self,
        _ctx: &C,
        avps: &TLVArray<'_, AttributeValuePairStruct<'_>>,
        ms: u32,
    ) -> Result<(), Error> {
        self.0.stage_scene(Self::CLUSTER_ID, avps, ms)
    }
}
pub async fn maintenance<K: KvBlobStoreAccess, H: Hardware>(
    runtime: &Runtime<K, H>,
    light: &LightHandler<'_, K, H>,
    mut feed: impl FnMut(),
) -> ! {
    let mut last = Instant::now().as_millis();
    loop {
        let now = Instant::now().as_millis();
        let ticks = ((now - last) / 100).min(u16::MAX as u64) as u16;
        if ticks > 0 {
            light.tick(ticks);
            last += u64::from(ticks) * 100;
        }
        runtime.tick(now);
        feed();
        Timer::after(Duration::from_millis(100)).await;
    }
}
