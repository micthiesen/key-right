use crate::presets::{LightHandler, COLOR_CLUSTER, LEVEL_CLUSTER, ON_OFF_CLUSTER};
use crate::runtime::{Hardware, OffEffect, OutputError, Runtime};
use key_right_core::{ColorTemperature, Level, LightOutput, LightState};
use rs_matter::dm::clusters::app::{color_control as color, level_control as level};
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

#[derive(Default)]
struct Rig {
    records: RefCell<BTreeMap<u16, Vec<u8>>>,
    writes: Cell<usize>,
    fail_output: Cell<bool>,
    output: Cell<Option<LightState>>,
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
    fn apply(&mut self, state: LightState) -> Result<(), OutputError> {
        if self.0.fail_output.get() {
            return Err(OutputError::Bus);
        }
        self.0.output.set(Some(state));
        Ok(())
    }
}
impl Hardware for Output {
    fn shutdown(&mut self) -> Result<(), OutputError> {
        self.apply(LightState::default())
    }
    fn verify(&mut self, state: LightState) -> Result<(), OutputError> {
        if self.0.fail_output.get() || self.0.output.get() != Some(state) {
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
    assert!(runtime.acknowledged().unwrap().on);
    assert_eq!(runtime.acknowledged().unwrap().level.get(), 180);
}

#[test]
fn transition_stop_freezes_current_level_without_changing_other_axis() {
    let (h, runtime) = rig();
    let scenes = ScenesState::new();
    let light = handler(&runtime, &scenes);
    light.level_to(20, true, None, lo(), lo()).unwrap();
    light.level_to(220, false, Some(20), lo(), lo()).unwrap();
    light.color_to(143, 20, co(), co()).unwrap();
    let writes = h.writes.get();
    runtime.tick(500);
    let mid = runtime.acknowledged().unwrap();
    assert!(mid.level.get() > 20 && mid.level.get() < 220);
    assert!(mid.temperature.get() < 303 && mid.temperature.get() > 143);
    assert_eq!(
        h.writes.get(),
        writes,
        "intermediate frames must not write flash"
    );
    light.stop_level(false, lo(), lo()).unwrap();
    runtime.tick(2000);
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
    light
        .level_move(level::MoveModeEnum::Down, Some(50), true, lo(), lo())
        .unwrap();
    runtime.tick(500);
    assert!(runtime.acknowledged().unwrap().on);
    light
        .level_step(level::StepModeEnum::Up, 10, Some(0), false, lo(), lo())
        .unwrap();
    let stopped = runtime.acknowledged().unwrap().level;
    runtime.tick(3000);
    assert_eq!(runtime.acknowledged().unwrap().level, stopped);
    assert!(runtime.acknowledged().unwrap().on);
    light.level_to(0, true, Some(10), lo(), lo()).unwrap();
    runtime.tick(4000);
    assert!(!runtime.acknowledged().unwrap().on);
    assert!(runtime.acknowledged().unwrap().level >= Level::MIN);
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
    let original = runtime.acknowledged().unwrap();
    light.off_with_effect(OffEffect::SlowFade).unwrap();
    let writes = h.writes.get();
    runtime.tick(800);
    assert_eq!(runtime.acknowledged().unwrap().level.get(), 50);
    runtime.tick(12800);
    assert!(!runtime.acknowledged().unwrap().on);
    assert_eq!(runtime.acknowledged().unwrap().level, original.level);
    assert_eq!(h.writes.get(), writes);
    // Another Off must not overwrite the captured scene with the dark state.
    light.off_with_effect(OffEffect::NoFade).unwrap();
    let execute = color::OptionsBitmap::EXECUTE_IF_OFF;
    light.color_to(344, 0, execute, execute).unwrap();
    light.recall_global_scene().unwrap();
    assert_eq!(runtime.acknowledged().unwrap(), original);
    light.color_to(250, 0, co(), co()).unwrap();
    light.recall_global_scene().unwrap();
    assert_eq!(runtime.acknowledged().unwrap().temperature.get(), 250);
}

#[test]
fn repeated_on_preserves_dimming_but_cancels_a_pending_off_effect() {
    let (_, runtime) = rig();
    let scenes = ScenesState::new();
    let light = handler(&runtime, &scenes);
    light.level_to(100, true, None, lo(), lo()).unwrap();
    light.level_to(200, false, Some(20), lo(), lo()).unwrap();
    runtime.tick(500);
    light.power(true).unwrap();
    assert_eq!(runtime.level_remaining_ms(), 1500);
    light.off_with_effect(OffEffect::DyingLight).unwrap();
    runtime.tick(750);
    light.power(true).unwrap();
    assert_eq!(runtime.level_remaining_ms(), 0);
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
    assert_eq!(runtime.acknowledged().unwrap(), target);
}
