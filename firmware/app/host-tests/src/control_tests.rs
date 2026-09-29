use crate::presets::{LightHandler, COLOR_CLUSTER, LEVEL_CLUSTER, ON_OFF_CLUSTER};
use crate::runtime::{Hardware, OffEffect, OutputError, Runtime};
use key_right_core::{ColorTemperature, Level, LightOutput, LightState, OutputFrame};
use rs_matter::dm::clusters::app::{color_control as color, level_control as level, on_off};
use rs_matter::dm::clusters::decl::scenes_management::AttributeValuePairStruct;
use rs_matter::dm::clusters::scenes::ScenesState;
use rs_matter::dm::Dataver;
use rs_matter::error::{Error, ErrorCode};
use rs_matter::persist::{KvBlobStore, KvBlobStoreAccess, KV_BUF_SIZE};
use rs_matter::tlv::{TLVArray, TLVElement, TLVTag, TLVWrite};
use rs_matter::utils::storage::WriteBuf;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

// Attribute getters are synchronous futures. Exercise the production trait wiring
// without constructing a transport or using a radio.
fn ready<F: std::future::Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
    {
        std::task::Poll::Ready(value) => value,
        std::task::Poll::Pending => panic!("attribute getter unexpectedly awaited I/O"),
    }
}
fn unused<T>() -> T {
    panic!("attribute getter unexpectedly used the transport context")
}
struct TestContext<'a> {
    light: &'a LightHandler<'a, Access, Output>,
    notifications: RefCell<Vec<(u32, Option<u32>)>>,
}
impl<'a> TestContext<'a> {
    fn new(light: &'a LightHandler<'a, Access, Output>) -> Self {
        Self {
            light,
            notifications: RefCell::new(Vec::new()),
        }
    }
    fn notify(&self, cluster: u32, attribute: Option<u32>) {
        // Match rs-matter 0.2.0's InteractionModel notification semantics.
        match cluster {
            6 => on_off::ClusterAsyncHandler::dataver_changed(self.light),
            8 => level::ClusterAsyncHandler::dataver_changed(self.light),
            0x300 => color::ClusterAsyncHandler::dataver_changed(self.light),
            _ => (),
        }
        self.notifications.borrow_mut().push((cluster, attribute));
    }
}
impl rs_matter::dm::AttrChangeNotifier for TestContext<'_> {
    fn notify_attr_changed(&self, endpoint: u16, cluster: u32, attribute: u32) {
        assert_eq!(endpoint, 1);
        self.notify(cluster, Some(attribute));
    }
    fn notify_cluster_changed(&self, endpoint: u16, cluster: u32) {
        assert_eq!(endpoint, 1);
        self.notify(cluster, None);
    }
    fn notify_endpoint_changed(&self, _: u16) {
        unused()
    }
    fn notify_all_changed(&self) {
        unused()
    }
}
impl rs_matter::dm::EventEmitter for TestContext<'_> {
    fn emit_event<F>(
        &self,
        _: u16,
        _: u32,
        _: u32,
        _: rs_matter::im::EventPriority,
        _: F,
    ) -> Result<rs_matter::dm::EventNumber, Error>
    where
        F: FnOnce(rs_matter::im::events::EventTLVWrite<'_>) -> Result<(), Error>,
    {
        unused()
    }
}
impl rs_matter::dm::HandlerContext for TestContext<'_> {
    fn matter(&self) -> &rs_matter::Matter<'_> {
        unused()
    }
    fn crypto(&self) -> impl rs_matter::crypto::Crypto + '_ {
        unused::<
            rs_matter::crypto::backend::rustcrypto::RustCrypto<
                '_,
                rs_matter::crypto::WeakTestOnlyRand,
            >,
        >()
    }
    fn kv(&self) -> impl KvBlobStoreAccess + '_ {
        unused::<Access>()
    }
    fn networks(&self) -> impl rs_matter::dm::clusters::net_comm::NetworksAccess + '_ {
        rs_matter::dm::clusters::net_comm::DummyNetworkAccess
    }
    fn metadata(&self) -> impl rs_matter::dm::Metadata + '_ {
        unused::<rs_matter::dm::Node<'_>>()
    }
    fn handler(&self) -> impl rs_matter::dm::AsyncHandler + '_ {
        rs_matter::dm::EmptyHandler
    }
    fn buffers(
        &self,
    ) -> impl rs_matter::utils::storage::pooled::Buffers<rs_matter::im::IMBuffer> + '_ {
        unused::<rs_matter::utils::storage::pooled::PooledBuffers<rs_matter::im::IMBuffer, 1>>()
    }
}
impl rs_matter::dm::MatchContext for TestContext<'_> {
    fn endpt(&self) -> Option<u16> {
        Some(1)
    }
    fn cluster(&self) -> Option<u32> {
        unused()
    }
}
impl rs_matter::dm::OwnAttrChangeNotifier for TestContext<'_> {
    fn notify_own_attr_changed(&self, _: u32) {
        unused()
    }
    fn notify_own_cluster_changed(&self) {
        unused()
    }
    fn notify_own_endpoint_changed(&self) {
        unused()
    }
}
impl rs_matter::dm::OwnEventEmitter for TestContext<'_> {
    fn emit_own_event<F>(
        &self,
        _: u32,
        _: rs_matter::im::EventPriority,
        _: F,
    ) -> Result<rs_matter::dm::EventNumber, Error>
    where
        F: FnOnce(rs_matter::im::events::EventTLVWrite<'_>) -> Result<(), Error>,
    {
        unused()
    }
}
impl rs_matter::dm::OperationContext for TestContext<'_> {
    fn exchange(&self) -> &rs_matter::transport::exchange::Exchange<'_> {
        unused()
    }
}
impl rs_matter::dm::ReadContext for TestContext<'_> {
    fn attr(&self) -> &rs_matter::dm::AttrDetails {
        unused()
    }
}
impl rs_matter::dm::WriteContext for TestContext<'_> {
    fn attr(&self) -> &rs_matter::dm::AttrDetails {
        unused()
    }
    fn data(&self) -> &TLVElement<'_> {
        unused()
    }
    fn notify_changed(&self) {
        self.notify(6, Some(on_off::AttributeId::StartUpOnOff as u32));
    }
}

#[derive(Default)]
struct Rig {
    records: RefCell<BTreeMap<u16, Vec<u8>>>,
    writes: Cell<usize>,
    fail_output: Cell<bool>,
    output: Cell<Option<LightState>>,
    output_frame: Cell<Option<OutputFrame>>,
}
struct Access(Rc<Rig>);
impl KvBlobStore for Access {
    fn load<'a>(&mut self, key: u16, buf: &'a mut [u8]) -> Result<Option<&'a [u8]>, Error> {
        Ok(self.0.records.borrow().get(&key).map(|data| {
            buf[..data.len()].copy_from_slice(data);
            &buf[..data.len()]
        }))
    }
    fn store(&mut self, key: u16, data: &[u8], _: &mut [u8]) -> Result<(), Error> {
        self.0.writes.set(self.0.writes.get() + 1);
        self.0.records.borrow_mut().insert(key, data.to_vec());
        Ok(())
    }
    fn remove(&mut self, _: u16, _: &mut [u8]) -> Result<(), Error> {
        Err(ErrorCode::InvalidAction.into())
    }
}
impl KvBlobStoreAccess for Access {
    fn access<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut dyn KvBlobStore, &mut [u8]) -> R,
    {
        f(&mut Access(self.0.clone()), &mut [0; KV_BUF_SIZE])
    }
}
struct Output(Rc<Rig>);
impl LightOutput for Output {
    type Error = OutputError;
    fn apply_frame(&mut self, frame: OutputFrame) -> Result<(), OutputError> {
        if self.0.fail_output.get() {
            return Err(OutputError::Bus);
        }
        self.0.output.set(Some(frame.state));
        self.0.output_frame.set(Some(frame));
        Ok(())
    }
}
impl Hardware for Output {
    fn shutdown(&mut self) -> Result<(), OutputError> {
        self.apply(LightState::default())
    }
    fn verify_frame(&mut self, frame: OutputFrame) -> Result<(), OutputError> {
        if self.0.fail_output.get()
            || self.0.output.get() != Some(frame.state)
            || self.0.output_frame.get() != Some(frame)
        {
            Err(OutputError::Readback)
        } else {
            Ok(())
        }
    }
    fn registers(&mut self) -> Result<[u8; 24], OutputError> {
        Ok([0; 24])
    }
}
fn rig() -> (Rc<Rig>, Runtime<Access, Output>) {
    let h = Rc::new(Rig::default());
    let runtime = Runtime::load(Access(h.clone()), Output(h.clone()));
    runtime.tick(0);
    (h, runtime)
}
fn handler<'a>(
    runtime: &'a Runtime<Access, Output>,
    scenes: &'a ScenesState<16>,
) -> LightHandler<'a, Access, Output> {
    LightHandler::new(
        runtime,
        scenes,
        [Dataver::new(1), Dataver::new(2), Dataver::new(3)],
    )
}
fn lo() -> level::OptionsBitmap {
    level::OptionsBitmap::empty()
}
fn co() -> color::OptionsBitmap {
    color::OptionsBitmap::empty()
}
fn scene_values(values: &[(u32, u8, u16)]) -> Vec<u8> {
    let mut bytes = [0; 128];
    let mut writer = WriteBuf::new(&mut bytes);
    writer.start_array(&TLVTag::Anonymous).unwrap();
    for &(attribute, tag, value) in values {
        writer.start_struct(&TLVTag::Anonymous).unwrap();
        writer.u32(&TLVTag::Context(0), attribute).unwrap();
        writer.u16(&TLVTag::Context(tag), value).unwrap();
        writer.end_container().unwrap();
    }
    writer.end_container().unwrap();
    writer.as_slice().to_vec()
}
fn avps(bytes: &[u8]) -> TLVArray<'_, AttributeValuePairStruct<'_>> {
    TLVArray::new_unchecked(TLVElement::new(bytes))
}

#[test]
fn ordinary_commands_respect_off_and_execute_if_off_never_turns_on() {
    let (_, runtime) = rig();
    let scenes = ScenesState::new();
    let light = handler(&runtime, &scenes);
    let initial = runtime.snapshot().intended;
    light.level_to(200, false, None, lo(), lo()).unwrap();
    light.color_to(200, 0, co(), co()).unwrap();
    assert_eq!(runtime.snapshot().intended, initial);
    let execute = color::OptionsBitmap::EXECUTE_IF_OFF;
    light.color_to(200, 0, execute, execute).unwrap();
    assert_eq!(runtime.acknowledged().unwrap().temperature.get(), 200);
    assert!(!runtime.acknowledged().unwrap().on);
    light.level_to(180, true, None, lo(), lo()).unwrap();
    runtime.tick(400);
    assert!(runtime.acknowledged().unwrap().on);
    assert_eq!(runtime.acknowledged().unwrap().level.get(), 180);
}

#[test]
fn transition_stop_freezes_current_level_without_changing_other_axis() {
    let (h, runtime) = rig();
    let scenes = ScenesState::new();
    let light = handler(&runtime, &scenes);
    light.level_to(20, true, None, lo(), lo()).unwrap();
    runtime.tick(400);
    light.level_to(220, false, Some(20), lo(), lo()).unwrap();
    light.color_to(143, 20, co(), co()).unwrap();
    let writes = h.writes.get();
    runtime.tick(900);
    let mid = runtime.acknowledged().unwrap();
    assert!(mid.level.get() > 20 && mid.level.get() < 220);
    assert!(mid.temperature.get() < 303 && mid.temperature.get() > 143);
    assert_eq!(
        h.writes.get(),
        writes,
        "intermediate frames must not write flash"
    );
    light.stop_level(false, lo(), lo()).unwrap();
    runtime.tick(2400);
    assert_eq!(runtime.acknowledged().unwrap().level, mid.level);
    assert_eq!(
        runtime.acknowledged().unwrap().temperature,
        ColorTemperature::MIN
    );
}

#[test]
fn with_on_off_move_reaches_off_and_step_cancels_an_earlier_move() {
    let (_, runtime) = rig();
    let scenes = ScenesState::new();
    let light = handler(&runtime, &scenes);
    light.level_to(100, true, None, lo(), lo()).unwrap();
    runtime.tick(400);
    light
        .level_move(level::MoveModeEnum::Down, Some(50), true, lo(), lo())
        .unwrap();
    runtime.tick(900);
    assert!(runtime.acknowledged().unwrap().on);
    light
        .level_step(level::StepModeEnum::Up, 10, Some(0), true, lo(), lo())
        .unwrap();
    let stopped = runtime.reported().unwrap().level;
    runtime.tick(3000);
    assert_eq!(runtime.acknowledged().unwrap().level, stopped);
    assert!(runtime.acknowledged().unwrap().on);
    light.level_to(0, true, Some(10), lo(), lo()).unwrap();
    runtime.tick(4000);
    assert!(!runtime.acknowledged().unwrap().on);
    assert!(runtime.acknowledged().unwrap().level >= Level::MIN);
}

#[test]
fn matter_move_rates_remain_linear_while_targets_report_the_destination() {
    let (_, runtime) = rig();
    let scenes = ScenesState::new();
    let light = handler(&runtime, &scenes);
    light.level_to(54, true, None, lo(), lo()).unwrap();
    runtime.tick(400);
    light
        .level_move(level::MoveModeEnum::Up, Some(100), false, lo(), lo())
        .unwrap();
    light
        .color_move(color::MoveModeEnum::Down, 80, (143, 344), co(), co())
        .unwrap();
    runtime.tick(900);
    // A quarter of each two-second Move is a quarter of the distance, not
    // the 15.625% used by a smoothstep transition at the same elapsed time.
    assert_eq!(runtime.acknowledged().unwrap().level.get(), 104);
    assert_eq!(runtime.acknowledged().unwrap().temperature.get(), 263);
    assert_eq!(runtime.reported().unwrap().level, Level::MAX);
    assert_eq!(
        runtime.reported().unwrap().temperature,
        ColorTemperature::MIN
    );
    runtime.tick(2400);
    assert_eq!(runtime.acknowledged().unwrap(), runtime.reported().unwrap());
}

#[test]
fn color_bounds_reject_inverted_ranges_and_move_stops_at_limit() {
    let (_, runtime) = rig();
    let scenes = ScenesState::new();
    let light = handler(&runtime, &scenes);
    light.power(true).unwrap();
    assert!(light
        .color_move(color::MoveModeEnum::Up, 10, (340, 200), co(), co())
        .is_err());
    assert!(light
        .color_step(color::StepModeEnum::Up, 5, 0, (500, 600), co(), co())
        .is_err());
    assert!(light.color_to(0, 0, co(), co()).is_err());
    light.color_to(600, 0, co(), co()).unwrap();
    runtime.tick(400);
    assert_eq!(
        runtime.acknowledged().unwrap().temperature,
        ColorTemperature::MAX
    );
    light
        .color_move(color::MoveModeEnum::Down, 100, (200, 320), co(), co())
        .unwrap();
    runtime.tick(2000);
    assert_eq!(runtime.acknowledged().unwrap().temperature.get(), 200);
}

#[test]
fn failed_actuation_is_not_acknowledged_and_timed_off_survives_pending_transition() {
    let (h, runtime) = rig();
    let scenes = ScenesState::new();
    let light = handler(&runtime, &scenes);
    h.fail_output.set(true);
    assert!(light.level_to(100, true, None, lo(), lo()).is_err());
    assert!(runtime.acknowledged().is_err());
    h.fail_output.set(false);
    runtime.tick(5000);
    light.timed_on(false, 10, 0).unwrap();
    light.level_to(200, true, Some(100), lo(), lo()).unwrap();
    light.tick(10);
    runtime.tick(6000);
    assert!(!runtime.acknowledged().unwrap().on);
    runtime.tick(20000);
    assert!(!runtime.acknowledged().unwrap().on);
}

#[test]
fn metadata_exposes_only_temperature_lighting_commands() {
    assert_eq!(ON_OFF_CLUSTER.id, 6);
    assert_eq!(
        LEVEL_CLUSTER.feature_map,
        level::Feature::ON_OFF.bits() | level::Feature::LIGHTING.bits()
    );
    assert_eq!(
        COLOR_CLUSTER.feature_map,
        color::Feature::COLOR_TEMPERATURE.bits()
    );
    assert!(COLOR_CLUSTER
        .command(color::CommandId::MoveToColorTemperature as _)
        .is_some());
    assert!(COLOR_CLUSTER
        .command(color::CommandId::StopMoveStep as _)
        .is_some());
    assert!(COLOR_CLUSTER
        .command(color::CommandId::MoveToHue as _)
        .is_none());
    assert!(COLOR_CLUSTER
        .attribute(color::AttributeId::CurrentHue as _)
        .is_none());
    assert!(LEVEL_CLUSTER
        .command(level::CommandId::MoveToClosestFrequency as _)
        .is_none());
}

#[test]
fn off_effect_and_global_scene_recall_restore_full_settings_once() {
    let (h, runtime) = rig();
    let scenes = ScenesState::new();
    let light = handler(&runtime, &scenes);
    light.level_to(100, true, None, lo(), lo()).unwrap();
    light.color_to(200, 0, co(), co()).unwrap();
    runtime.tick(400);
    let original = runtime.acknowledged().unwrap();
    light.off_with_effect(OffEffect::SlowFade).unwrap();
    let writes = h.writes.get();
    runtime.tick(1200);
    // Half physical output, accounting for the normal nonzero level floor.
    assert_eq!(runtime.acknowledged().unwrap().level.get(), 36);
    runtime.tick(13200);
    assert!(!runtime.acknowledged().unwrap().on);
    assert_eq!(runtime.acknowledged().unwrap().level, original.level);
    assert_eq!(h.writes.get(), writes);
    // Another Off must not overwrite the captured scene with the dark state.
    light.off_with_effect(OffEffect::NoFade).unwrap();
    let execute = color::OptionsBitmap::EXECUTE_IF_OFF;
    light.color_to(344, 0, execute, execute).unwrap();
    light.recall_global_scene().unwrap();
    runtime.tick(13600);
    assert_eq!(runtime.acknowledged().unwrap(), original);
    light.color_to(250, 0, co(), co()).unwrap();
    runtime.tick(14000);
    light.recall_global_scene().unwrap();
    assert_eq!(runtime.acknowledged().unwrap().temperature.get(), 250);
}

#[test]
fn repeated_on_preserves_dimming_but_cancels_a_pending_off_effect() {
    let (_, runtime) = rig();
    let scenes = ScenesState::new();
    let light = handler(&runtime, &scenes);
    light.level_to(100, true, None, lo(), lo()).unwrap();
    runtime.tick(400);
    light.level_to(200, false, Some(20), lo(), lo()).unwrap();
    runtime.tick(900);
    light.power(true).unwrap();
    assert_eq!(runtime.level_remaining_ms(), 1500);
    light.off_with_effect(OffEffect::DyingLight).unwrap();
    runtime.tick(1150);
    light.power(true).unwrap();
    assert_eq!(runtime.level_remaining_ms(), 400);
    runtime.tick(5000);
    assert!(runtime.acknowledged().unwrap().on);
}

#[test]
fn malformed_scene_does_not_partially_persist_or_actuate() {
    let (h, runtime) = rig();
    let scenes = ScenesState::new();
    let light = handler(&runtime, &scenes);
    let original = runtime.acknowledged().unwrap();
    let writes = h.writes.get();
    light.begin_scene().unwrap();
    light
        .stage_scene(6, &avps(&scene_values(&[(0, 1, 1)])), 1000)
        .unwrap();
    light
        .stage_scene(8, &avps(&scene_values(&[(0, 1, 200)])), 1000)
        .unwrap();
    assert!(light
        .stage_scene(0x300, &avps(&scene_values(&[(7, 3, 0)])), 1000)
        .is_err());
    light.abort_scene();
    assert_eq!(runtime.acknowledged().unwrap(), original);
    assert_eq!(runtime.snapshot().intended, original);
    assert_eq!(h.writes.get(), writes);
    // Duplicate attributes cannot silently choose a last value.
    light.begin_scene().unwrap();
    assert!(light
        .stage_scene(8, &avps(&scene_values(&[(0, 1, 100), (0, 1, 200)])), 0)
        .is_err());
    light.abort_scene();
    assert_eq!(h.writes.get(), writes);
}

#[test]
fn full_scene_commits_once_resets_timers_and_refreshes_global_scene() {
    let (h, runtime) = rig();
    let scenes = ScenesState::new();
    let light = handler(&runtime, &scenes);
    light.power(true).unwrap();
    light.off_with_effect(OffEffect::NoFade).unwrap();
    light.timed_on(false, 10, 0).unwrap();
    light.off_with_effect(OffEffect::NoFade).unwrap();
    let writes = h.writes.get();
    light.begin_scene().unwrap();
    light
        .stage_scene(6, &avps(&scene_values(&[(0, 1, 1)])), 1000)
        .unwrap();
    light
        .stage_scene(8, &avps(&scene_values(&[(0, 1, 200)])), 1000)
        .unwrap();
    light
        .stage_scene(
            0x300,
            &avps(&scene_values(&[(7, 3, 200), (0x4001, 1, 2)])),
            1000,
        )
        .unwrap();
    assert_eq!(h.writes.get(), writes);
    light.finish_scene().unwrap();
    assert_eq!(h.writes.get(), writes + 1);
    light.tick(100);
    runtime.tick(2000);
    let recalled = runtime.acknowledged().unwrap();
    assert!(recalled.on);
    assert_eq!(recalled.level.get(), 200);
    assert_eq!(recalled.temperature.get(), 200);
    light.off_with_effect(OffEffect::NoFade).unwrap();
    light.recall_global_scene().unwrap();
    runtime.tick(2400);
    assert_eq!(runtime.acknowledged().unwrap(), recalled);
}

#[test]
fn failed_scene_actuation_returns_error_with_complete_target_for_recovery() {
    let (h, runtime) = rig();
    let scenes = ScenesState::new();
    let light = handler(&runtime, &scenes);
    light.begin_scene().unwrap();
    light
        .stage_scene(6, &avps(&scene_values(&[(0, 1, 1)])), 0)
        .unwrap();
    light
        .stage_scene(8, &avps(&scene_values(&[(0, 1, 200)])), 0)
        .unwrap();
    light
        .stage_scene(0x300, &avps(&scene_values(&[(7, 3, 200)])), 0)
        .unwrap();
    h.fail_output.set(true);
    assert!(light.finish_scene().is_err());
    assert!(runtime.acknowledged().is_err());
    let target = runtime.snapshot().intended;
    assert!(target.on);
    assert_eq!(target.level.get(), 200);
    assert_eq!(target.temperature.get(), 200);
    h.fail_output.set(false);
    runtime.tick(5000);
    assert!(!runtime.acknowledged().unwrap().on);
    runtime.tick(5400);
    assert_eq!(runtime.acknowledged().unwrap(), target);
}

#[test]
fn matter_attributes_report_targets_through_fades_and_fail_when_output_is_unknown() {
    let (h, runtime) = rig();
    let scenes = ScenesState::new();
    let light = handler(&runtime, &scenes);
    let ctx = TestContext::new(&light);
    light.level_to(200, true, Some(20), lo(), lo()).unwrap();
    light.color_to(200, 20, co(), co()).unwrap();
    runtime.tick(500);
    assert!(ready(on_off::ClusterAsyncHandler::on_off(&light, &ctx)).unwrap());
    assert_eq!(
        ready(level::ClusterAsyncHandler::current_level(&light, &ctx))
            .unwrap()
            .into_option(),
        Some(200)
    );
    assert_eq!(
        ready(color::ClusterAsyncHandler::color_temperature_mireds(
            &light, &ctx
        ))
        .unwrap(),
        200
    );
    assert_ne!(runtime.acknowledged().unwrap(), runtime.reported().unwrap());
    light.off_with_effect(OffEffect::FastFade).unwrap();
    assert!(!ready(on_off::ClusterAsyncHandler::on_off(&light, &ctx)).unwrap());
    assert!(runtime.acknowledged().unwrap().on);
    assert_eq!(
        ready(level::ClusterAsyncHandler::current_level(&light, &ctx))
            .unwrap()
            .into_option(),
        Some(200)
    );
    runtime.tick(1300);
    light.recall_global_scene().unwrap();
    runtime.tick(1700);
    assert_eq!(runtime.acknowledged().unwrap().level.get(), 200);
    assert_eq!(runtime.acknowledged().unwrap().temperature.get(), 200);
    h.fail_output.set(true);
    light.power(false).unwrap();
    // The first changed physical frame exposes the failed adapter.
    runtime.tick(1720);
    assert!(ready(on_off::ClusterAsyncHandler::on_off(&light, &ctx)).is_err());
    assert!(ready(level::ClusterAsyncHandler::current_level(&light, &ctx)).is_err());
    assert!(ready(color::ClusterAsyncHandler::color_temperature_mireds(
        &light, &ctx
    ))
    .is_err());
}

#[test]
fn minimum_with_on_off_is_off_but_plain_level_and_on_can_use_minimum() {
    let (_, runtime) = rig();
    let scenes = ScenesState::new();
    let light = handler(&runtime, &scenes);
    for raw in [0, 1] {
        light.level_to(100, true, None, lo(), lo()).unwrap();
        light.level_to(raw, true, None, lo(), lo()).unwrap();
        assert!(!runtime.reported().unwrap().on);
        assert_eq!(runtime.reported().unwrap().level, Level::MIN);
    }
    light.power(true).unwrap();
    light.level_to(1, false, None, lo(), lo()).unwrap();
    assert!(runtime.reported().unwrap().on);
    light.level_to(2, true, None, lo(), lo()).unwrap();
    light
        .level_step(level::StepModeEnum::Down, 1, None, true, lo(), lo())
        .unwrap();
    assert!(!runtime.reported().unwrap().on);
}

#[test]
fn ordinary_level_commands_never_resurrect_an_off_fade() {
    let (_, runtime) = rig();
    let scenes = ScenesState::new();
    let light = handler(&runtime, &scenes);
    light.level_to(100, true, None, lo(), lo()).unwrap();
    runtime.tick(400);
    light.off_with_effect(OffEffect::SlowFade).unwrap();
    runtime.tick(800);
    assert!(runtime.acknowledged().unwrap().on);
    light.level_to(180, false, None, lo(), lo()).unwrap();
    assert!(!runtime.reported().unwrap().on);
    assert_eq!(runtime.reported().unwrap().level.get(), 100);
    let execute = level::OptionsBitmap::EXECUTE_IF_OFF;
    light.level_to(180, false, None, execute, execute).unwrap();
    assert!(!runtime.reported().unwrap().on);
    assert_eq!(runtime.reported().unwrap().level.get(), 180);
    runtime.tick(20000);
    assert!(!runtime.acknowledged().unwrap().on);
}

#[test]
fn both_stop_variants_freeze_an_off_fade_at_the_acknowledged_frame() {
    for with_on_off in [false, true] {
        let (_, runtime) = rig();
        let scenes = ScenesState::new();
        let light = handler(&runtime, &scenes);
        light.level_to(100, true, None, lo(), lo()).unwrap();
        runtime.tick(400);
        light.level_to(0, true, Some(20), lo(), lo()).unwrap();
        runtime.tick(900);
        let frame = runtime.acknowledged().unwrap();
        assert!(frame.on);
        assert!(!runtime.reported().unwrap().on);
        light.stop_level(with_on_off, lo(), lo()).unwrap();
        assert_eq!(runtime.reported().unwrap(), frame);
        runtime.tick(3000);
        assert_eq!(runtime.acknowledged().unwrap(), frame);
    }
}

#[test]
fn fades_notify_only_remaining_time_then_targets_and_faults_notify_clusters() {
    let (h, runtime) = rig();
    let scenes = ScenesState::new();
    let light = handler(&runtime, &scenes);
    let ctx = TestContext::new(&light);
    let clusters = [6, 8, 0x300];
    let mut reports = core::array::from_fn::<_, 3, _>(|index| light.report_state(index));
    light.level_to(200, true, Some(20), lo(), lo()).unwrap();
    light.color_to(200, 20, co(), co()).unwrap();
    for (index, report) in reports.iter_mut().enumerate() {
        light.poll_report(&ctx, index, clusters[index], report);
    }
    for cluster in clusters {
        assert!(ctx.notifications.borrow().contains(&(cluster, None)));
    }
    ctx.notifications.borrow_mut().clear();
    let level_version = level::ClusterAsyncHandler::dataver(&light);
    runtime.tick(500);
    for (index, report) in reports.iter_mut().enumerate() {
        light.poll_report(&ctx, index, clusters[index], report);
    }
    assert_eq!(
        *ctx.notifications.borrow(),
        vec![(8, Some(1)), (0x300, Some(2))]
    );
    assert_ne!(level::ClusterAsyncHandler::dataver(&light), level_version);
    ctx.notifications.borrow_mut().clear();
    for (index, report) in reports.iter_mut().enumerate() {
        light.poll_report(&ctx, index, clusters[index], report);
    }
    assert!(ctx.notifications.borrow().is_empty());
    runtime.tick(2000);
    for (index, report) in reports.iter_mut().enumerate() {
        light.poll_report(&ctx, index, clusters[index], report);
    }
    assert_eq!(
        *ctx.notifications.borrow(),
        vec![(8, Some(1)), (0x300, Some(2))]
    );
    ctx.notifications.borrow_mut().clear();
    runtime.off().unwrap();
    for (index, report) in reports.iter_mut().enumerate() {
        light.poll_report(&ctx, index, clusters[index], report);
    }
    for cluster in clusters {
        assert!(ctx.notifications.borrow().contains(&(cluster, None)));
    }
    ctx.notifications.borrow_mut().clear();
    h.fail_output.set(true);
    assert!(runtime.verify().is_err());
    for (index, report) in reports.iter_mut().enumerate() {
        light.poll_report(&ctx, index, clusters[index], report);
    }
    for cluster in clusters {
        assert!(ctx.notifications.borrow().contains(&(cluster, None)));
    }
    h.fail_output.set(false);
    runtime.tick(7000);
    ctx.notifications.borrow_mut().clear();
    for (index, report) in reports.iter_mut().enumerate() {
        light.poll_report(&ctx, index, clusters[index], report);
    }
    for cluster in clusters {
        assert!(ctx.notifications.borrow().contains(&(cluster, None)));
    }
}

#[test]
fn identical_target_preserves_deadline_and_reports_completion() {
    let (_, runtime) = rig();
    let scenes = ScenesState::new();
    let light = handler(&runtime, &scenes);
    let ctx = TestContext::new(&light);
    light.level_to(100, true, None, lo(), lo()).unwrap();
    light.level_to(100, false, Some(100), lo(), lo()).unwrap();
    let mut report = light.report_state(1);
    assert_eq!(
        ready(level::ClusterAsyncHandler::remaining_time(&light, &ctx)).unwrap(),
        4
    );
    runtime.tick(400);
    light.poll_report(&ctx, 1, 8, &mut report);
    assert_eq!(*ctx.notifications.borrow(), vec![(8, Some(1))]);
    assert_eq!(
        ready(level::ClusterAsyncHandler::remaining_time(&light, &ctx)).unwrap(),
        0
    );
}

#[test]
fn startup_power_attribute_accepts_restore_or_off_and_rejects_on_or_toggle() {
    use on_off::StartUpOnOffEnum as Startup;
    use rs_matter::tlv::Nullable;
    let (h, runtime) = rig();
    let scenes = ScenesState::new();
    let light = handler(&runtime, &scenes);
    let ctx = TestContext::new(&light);
    assert_eq!(
        ready(on_off::ClusterAsyncHandler::start_up_on_off(&light, &ctx))
            .unwrap()
            .into_option(),
        None
    );
    light.power(true).unwrap();
    let writes = h.writes.get();
    for unsupported in [Startup::On, Startup::Toggle] {
        let error = ready(on_off::ClusterAsyncHandler::set_start_up_on_off(
            &light,
            &ctx,
            Nullable::some(unsupported),
        ))
        .unwrap_err();
        assert_eq!(error.code(), ErrorCode::ConstraintError);
        assert_eq!(h.writes.get(), writes);
        assert!(ctx.notifications.borrow().is_empty());
    }
    ready(on_off::ClusterAsyncHandler::set_start_up_on_off(
        &light,
        &ctx,
        Nullable::some(Startup::Off),
    ))
    .unwrap();
    assert_eq!(
        ready(on_off::ClusterAsyncHandler::start_up_on_off(&light, &ctx))
            .unwrap()
            .into_option(),
        Some(Startup::Off)
    );
    assert!(runtime.reported().unwrap().on);
    ready(on_off::ClusterAsyncHandler::set_start_up_on_off(
        &light,
        &ctx,
        Nullable::none(),
    ))
    .unwrap();
    assert_eq!(
        ready(on_off::ClusterAsyncHandler::start_up_on_off(&light, &ctx))
            .unwrap()
            .into_option(),
        None
    );
    assert!(runtime.reported().unwrap().on);
    assert_eq!(
        *ctx.notifications.borrow(),
        vec![(6, Some(0x4003)), (6, Some(0x4003))]
    );
}
