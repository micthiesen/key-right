//! Two mutually exclusive standard On/Off lights backed by acknowledged output.
use crate::runtime::{Hardware, Runtime};
use core::cell::Cell;
use embassy_time::{Duration, Instant, Timer};
use key_right_core::Preset;
use rs_matter_embassy::matter::dm::clusters::app::on_off::{
    self, ClusterAsyncHandler, EffectIdentifierEnum, OffWithEffectRequest, OnOffControlBitmap,
    OnWithTimedOffRequest, StartUpOnOffEnum,
};
use rs_matter_embassy::matter::dm::{
    Cluster, Dataver, HandlerContext, InvokeContext, ReadContext, WriteContext,
};
use rs_matter_embassy::matter::error::{Error, ErrorCode};
use rs_matter_embassy::matter::persist::KvBlobStoreAccess;
use rs_matter_embassy::matter::tlv::Nullable;
use rs_matter_embassy::matter::with;

pub const CLUSTER: Cluster<'static> = on_off::FULL_CLUSTER.with_revision(6)
    .with_features(on_off::Feature::LIGHTING.bits())
    .with_attrs(with!(required; on_off::AttributeId::OnOff | on_off::AttributeId::GlobalSceneControl | on_off::AttributeId::OnTime | on_off::AttributeId::OffWaitTime | on_off::AttributeId::StartUpOnOff))
    .with_cmds(with!(on_off::CommandId::Off | on_off::CommandId::On | on_off::CommandId::Toggle |
        on_off::CommandId::OffWithEffect | on_off::CommandId::OnWithRecallGlobalScene | on_off::CommandId::OnWithTimedOff));

pub struct PresetHandler<'a, K, H> {
    pub runtime: &'a Runtime<K, H>,
    pub preset: Preset,
    pub endpoint: u16,
    pub dataver: Dataver,
    on_time: Cell<u16>,
    off_wait: Cell<u16>,
    timed: Cell<bool>,
    global_scene: Cell<bool>,
}
impl<'a, K: KvBlobStoreAccess, H: Hardware> PresetHandler<'a, K, H> {
    pub fn new(
        runtime: &'a Runtime<K, H>,
        preset: Preset,
        endpoint: u16,
        dataver: Dataver,
    ) -> Self {
        Self {
            runtime,
            preset,
            endpoint,
            dataver,
            on_time: Cell::new(0),
            off_wait: Cell::new(0),
            timed: Cell::new(false),
            global_scene: Cell::new(true),
        }
    }
    fn power(&self, on: bool) -> Result<(), Error> {
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
        } else {
            self.on_time.set(0);
            self.timed
                .set(self.off_wait.get() > 0 && self.off_wait.get() != u16::MAX);
        }
        self.dataver.changed();
        self.runtime.set_endpoint(self.preset, on)
    }
    /// Runs independently of the transport so a network restart cannot extend a timer.
    pub fn tick(&self, deciseconds: u16) {
        if !self.timed.get() {
            return;
        }
        let intended = self.runtime.snapshot().intended;
        match intended.on && intended.preset == self.preset {
            true => {
                self.on_time
                    .set(self.on_time.get().saturating_sub(deciseconds));
                if self.on_time.get() == 0 && self.power(false).is_ok() {
                    self.off_wait.set(0);
                    self.timed.set(false);
                }
            }
            false => {
                // Selecting the other endpoint cancels this endpoint's timed ON.
                // Do not carry its old duration into a later timed command.
                self.on_time.set(0);
                self.off_wait
                    .set(self.off_wait.get().saturating_sub(deciseconds));
                if self.off_wait.get() == 0 {
                    self.timed.set(false);
                }
            }
        }
        self.dataver.changed();
    }
    pub fn timed_on(&self, accept_only_on: bool, on_time: u16, off_wait: u16) -> Result<(), Error> {
        let on = self.runtime.endpoint_on(self.preset)?;
        if accept_only_on && !on {
            return Ok(());
        }
        if self.off_wait.get() > 0 && !on {
            self.off_wait.set(self.off_wait.get().min(off_wait));
            self.timed
                .set(self.on_time.get() != u16::MAX && self.off_wait.get() != u16::MAX);
            self.dataver.changed();
            return Ok(());
        }
        // Record the cutoff even when applying ON fails and is retried later.
        self.on_time.set(self.on_time.get().max(on_time));
        self.off_wait.set(off_wait);
        self.timed
            .set(self.on_time.get() != u16::MAX && self.off_wait.get() != u16::MAX);
        self.dataver.changed();
        self.global_scene.set(true);
        self.runtime.set_endpoint(self.preset, true)
    }
}
impl<K: KvBlobStoreAccess, H: Hardware> ClusterAsyncHandler for PresetHandler<'_, K, H> {
    const CLUSTER: Cluster<'static> = CLUSTER;
    fn dataver(&self) -> u32 {
        self.dataver.get()
    }
    fn dataver_changed(&self) {
        self.dataver.changed();
    }
    async fn run(&self, ctx: impl HandlerContext) -> Result<(), Error> {
        let mut revision = self.runtime.snapshot().revision;
        let mut dataver = self.dataver.get();
        loop {
            Timer::after(Duration::from_millis(250)).await;
            let new = self.runtime.snapshot().revision;
            if new != revision {
                self.dataver.changed();
                revision = new;
            }
            if dataver != self.dataver.get() {
                dataver = self.dataver.get();
                ctx.notify_cluster_changed(self.endpoint, CLUSTER.id);
            }
        }
    }
    async fn on_off(&self, _ctx: impl ReadContext) -> Result<bool, Error> {
        self.runtime.endpoint_on(self.preset)
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
    ) -> Result<Nullable<StartUpOnOffEnum>, Error> {
        Ok(self.runtime.startup(self.preset).into())
    }
    async fn set_start_up_on_off(
        &self,
        ctx: impl WriteContext,
        value: Nullable<StartUpOnOffEnum>,
    ) -> Result<(), Error> {
        self.runtime.set_startup(self.preset, value.into_option())?;
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
        self.power(!self.runtime.endpoint_on(self.preset)?)
    }
    async fn handle_off_with_effect(
        &self,
        _ctx: impl InvokeContext,
        request: OffWithEffectRequest<'_>,
    ) -> Result<(), Error> {
        let valid = match request.effect_identifier()? {
            EffectIdentifierEnum::DelayedAllOff => request.effect_variant()? <= 2,
            EffectIdentifierEnum::DyingLight => request.effect_variant()? == 0,
        };
        if !valid {
            return Err(ErrorCode::InvalidCommand.into());
        }
        // Fixed calibrated output has no intermediate fade levels. End at verified OFF.
        self.power(false)?;
        self.global_scene.set(false);
        Ok(())
    }
    async fn handle_on_with_recall_global_scene(
        &self,
        _ctx: impl InvokeContext,
    ) -> Result<(), Error> {
        if self.global_scene.get() {
            Ok(())
        } else {
            self.power(true)
        }
    }
    async fn handle_on_with_timed_off(
        &self,
        _ctx: impl InvokeContext,
        request: OnWithTimedOffRequest<'_>,
    ) -> Result<(), Error> {
        self.timed_on(
            request
                .on_off_control()?
                .contains(OnOffControlBitmap::ACCEPT_ONLY_WHEN_ON),
            request.on_time()?,
            request.off_wait_time()?,
        )
    }
}

pub async fn maintenance<K: KvBlobStoreAccess, H: Hardware>(
    runtime: &Runtime<K, H>,
    handlers: [&PresetHandler<'_, K, H>; 2],
    mut feed: impl FnMut(),
) -> ! {
    let mut last = Instant::now().as_millis();
    loop {
        let now = Instant::now().as_millis();
        let ticks = ((now - last) / 100).min(u16::MAX as u64) as u16;
        if ticks > 0 {
            for h in handlers {
                h.tick(ticks);
            }
            last += u64::from(ticks) * 100;
        }
        runtime.tick(now);
        feed();
        Timer::after(Duration::from_millis(100)).await;
    }
}
