//! Exercise the production runtime and simulated PCA adapter, not a second fade model.
use crate::light::SimulatedOutput;
use crate::runtime::{OffEffect, Runtime, DEFAULT_TRANSITION_MS, INTENT_KEY};
use key_right_core::{ColorTemperature, Level, LightState, OutputBrightness, OutputFrame};
use rs_matter::error::Error;
use rs_matter::persist::{KvBlobStore, KvBlobStoreAccess, KV_BUF_SIZE};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Clone, Default)]
struct Memory {
    record: Rc<RefCell<Option<Vec<u8>>>>,
    writes: Rc<Cell<u32>>,
}
impl KvBlobStore for Memory {
    fn load<'a>(&mut self, key: u16, buf: &'a mut [u8]) -> Result<Option<&'a [u8]>, Error> {
        assert_eq!(key, INTENT_KEY);
        Ok(self.record.borrow().as_ref().map(|data| {
            buf[..data.len()].copy_from_slice(data);
            &buf[..data.len()]
        }))
    }
    fn store(&mut self, key: u16, data: &[u8], _: &mut [u8]) -> Result<(), Error> {
        assert_eq!(key, INTENT_KEY);
        self.writes.set(self.writes.get() + 1);
        *self.record.borrow_mut() = Some(data.to_vec());
        Ok(())
    }
    fn remove(&mut self, _: u16, _: &mut [u8]) -> Result<(), Error> {
        panic!("animation cannot erase storage")
    }
}
impl KvBlobStoreAccess for Memory {
    fn access<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut dyn KvBlobStore, &mut [u8]) -> R,
    {
        f(&mut self.clone(), &mut [0; KV_BUF_SIZE])
    }
}
type Light = Runtime<Memory, SimulatedOutput>;
fn light() -> (Memory, Light) {
    let memory = Memory::default();
    let runtime = Runtime::load(memory.clone(), SimulatedOutput::default());
    runtime.tick(0);
    (memory, runtime)
}
fn frame(light: &Light) -> OutputFrame {
    light.snapshot().applied_frame.unwrap()
}
fn pwm(light: &Light) -> [u8; 2] {
    let registers = light.registers().unwrap();
    [registers[2], registers[6]]
}

#[test]
fn default_power_fades_ease_from_true_zero_without_reporting_or_flash_churn() {
    let (memory, light) = light();
    light.set_power(true).unwrap();
    let target = light.reported().unwrap();
    let revision = light.snapshot().revision;
    let writes = memory.writes.get();
    assert_eq!(frame(&light).brightness, OutputBrightness::ZERO);
    assert_eq!(pwm(&light), [0, 0]);
    light.tick(100);
    // smoothstep(0.25) = 0.15625, not the linear 0.25. This is below
    // stock 1%, which cannot be represented by an ordinary Matter level.
    assert_eq!(frame(&light).brightness.get(), 118);
    assert_eq!(pwm(&light), [0, 0]);
    for now in (120..=400).step_by(20) {
        light.tick(now);
        light.verify().unwrap();
        assert_eq!(light.reported().unwrap(), target);
        assert_eq!(light.snapshot().revision, revision);
        assert_eq!(memory.writes.get(), writes);
    }
    assert_eq!(frame(&light), OutputFrame::from_state(target));
    assert_eq!(pwm(&light), [6, 2]);
    light.set_power(false).unwrap();
    assert!(!light.reported().unwrap().on);
    assert_eq!(pwm(&light), [6, 2]);
    let writes = memory.writes.get();
    for now in (420..=800).step_by(20) {
        light.tick(now);
        assert!(!light.reported().unwrap().on);
        assert_eq!(memory.writes.get(), writes);
    }
    assert_eq!(frame(&light).brightness, OutputBrightness::ZERO);
    assert_eq!(pwm(&light), [0, 0]);
    assert_eq!(light.level_remaining_ms(), 0);
}

#[test]
fn reversals_and_rapid_retargeting_start_at_the_exact_acknowledged_frame() {
    let (_, light) = light();
    light.set_power(true).unwrap();
    light.tick(100);
    let subfloor = frame(&light);
    light.set_power(false).unwrap();
    assert_eq!(frame(&light), subfloor);
    light.tick(200);
    assert!(frame(&light).brightness < subfloor.brightness);
    let reversing = frame(&light);
    light.set_power(true).unwrap();
    assert_eq!(frame(&light), reversing);
    light.set_level(Level::MAX, false).unwrap();
    assert_eq!(frame(&light), reversing);
    light.tick(300);
    let rising = frame(&light);
    assert!(rising.brightness > reversing.brightness);
    light.set_level(Level::new(100).unwrap(), false).unwrap();
    assert_eq!(frame(&light), rising);
    light.tick(700);
    assert_eq!(
        frame(&light),
        OutputFrame::from_state(light.reported().unwrap())
    );
}

#[test]
fn remembered_off_settings_never_restart_or_recolour_a_fade_out() {
    let (_, light) = light();
    light.set_power(true).unwrap();
    light.tick(400);
    light.set_power(false).unwrap();
    light.tick(500);
    let before = frame(&light);
    light.set_level(Level::MAX, false).unwrap();
    light.set_temperature(ColorTemperature::MIN).unwrap();
    assert_eq!(frame(&light), before);
    assert_eq!(light.level_remaining_ms(), 300);
    assert_eq!(light.temperature_remaining_ms(), 0);
    for now in (520..800).step_by(20) {
        light.tick(now);
        assert!(frame(&light).brightness <= before.brightness);
        assert_eq!(frame(&light).state.temperature, ColorTemperature::DEFAULT);
        assert!(!light.reported().unwrap().on);
    }
    light.tick(800);
    assert_eq!(pwm(&light), [0, 0]);
    assert_eq!(frame(&light).state.temperature, ColorTemperature::MIN);
    light.set_power(true).unwrap();
    light.tick(1200);
    assert_eq!(pwm(&light), [0, 22]);
}

#[test]
fn repeating_a_target_keeps_its_deadline_and_independent_colour_track() {
    let (_, light) = light();
    light.set_power(true).unwrap();
    light.tick(400);
    light.set_level_transition(254, false, 2000).unwrap();
    light
        .set_temperature_transition(ColorTemperature::MIN, 1000)
        .unwrap();
    light.tick(600);
    let current = frame(&light);
    light.set_level_transition(254, false, 9000).unwrap();
    light.set_temperature(ColorTemperature::MIN).unwrap();
    assert_eq!(frame(&light), current);
    assert_eq!(light.level_remaining_ms(), 1800);
    assert_eq!(light.temperature_remaining_ms(), 800);
    light.tick(1400);
    assert_eq!(frame(&light).state.temperature, ColorTemperature::MIN);
    assert_ne!(frame(&light).brightness, OutputBrightness::MAX);
    light.tick(2400);
    assert_eq!(pwm(&light), [0, 22]);
}

#[test]
fn explicit_move_rates_remain_linear_while_move_to_eases() {
    let (_, light) = light();
    light.set_power(true).unwrap();
    light.tick(400);
    let initial = frame(&light).brightness.get();
    light.set_level_rate(254, false, 1000).unwrap();
    light
        .set_temperature_rate(ColorTemperature::MIN, 1000)
        .unwrap();
    light.tick(650);
    assert_eq!(
        frame(&light).brightness.get(),
        initial + (2530 - initial) / 4
    );
    assert_eq!(frame(&light).state.temperature.get(), 263);
    light.tick(1400);
    light.set_level_transition(57, false, 1000).unwrap();
    light
        .set_temperature_transition(ColorTemperature::DEFAULT, 1000)
        .unwrap();
    light.tick(1650);
    assert_eq!(
        frame(&light).brightness.get(),
        2530 - (2530 - initial) * 10 / 64
    );
    assert_eq!(frame(&light).state.temperature.get(), 168);
}

#[test]
fn reboot_restores_destination_with_zero_first_and_saved_off_never_energizes() {
    let (memory, light) = light();
    light.set_power(true).unwrap();
    light.set_level(Level::MAX, false).unwrap();
    light.tick(100);
    let target = light.reported().unwrap();
    let reboot = Runtime::load(memory.clone(), SimulatedOutput::default());
    reboot.tick(0);
    assert_eq!(reboot.reported().unwrap(), target);
    assert_eq!(pwm(&reboot), [0, 0]);
    reboot.tick(DEFAULT_TRANSITION_MS);
    assert_eq!(frame(&reboot), OutputFrame::from_state(target));
    reboot.set_power(false).unwrap();
    // Cut power during fade-out. Persisted Off, not the intermediate On frame,
    // governs the next boot.
    reboot.tick(500);
    assert_ne!(frame(&reboot).brightness, OutputBrightness::ZERO);
    let off_reboot = Runtime::load(memory, SimulatedOutput::default());
    for now in [0, 20, 400, 5000] {
        off_reboot.tick(now);
        assert!(!off_reboot.reported().unwrap().on);
        assert_eq!(pwm(&off_reboot), [0, 0]);
    }
}

#[test]
fn stop_subfloor_and_local_safety_off_cannot_promote_zero_to_minimum_on() {
    let (_, light) = light();
    light.set_power(true).unwrap();
    light.tick(100);
    assert_eq!(pwm(&light), [0, 0]);
    light.stop_level_transition().unwrap();
    assert!(!light.reported().unwrap().on);
    assert_eq!(pwm(&light), [0, 0]);
    light.set_power(true).unwrap();
    light.tick(500);
    light.set_level(Level::MAX, false).unwrap();
    light.tick(700);
    assert_ne!(pwm(&light), [0, 0]);
    light.off().unwrap();
    assert_eq!(pwm(&light), [0, 0]);
    assert_eq!(light.level_remaining_ms(), 0);
    assert!(!light.reported().unwrap().on);
}

#[test]
fn scene_recall_commits_all_targets_once_and_fades_both_axes() {
    let (memory, light) = light();
    light.set_power(true).unwrap();
    light.tick(400);
    let before = frame(&light);
    let target = LightState {
        on: true,
        level: Level::MAX,
        temperature: ColorTemperature::MIN,
    };
    let writes = memory.writes.get();
    light.recall_scene(target, 0).unwrap();
    assert_eq!(memory.writes.get(), writes + 1);
    assert_eq!(frame(&light), before);
    assert_eq!(light.reported().unwrap(), target);
    light.tick(600);
    assert!(frame(&light).brightness > before.brightness);
    assert!(frame(&light).brightness < OutputBrightness::MAX);
    assert!(frame(&light).state.temperature > target.temperature);
    assert!(frame(&light).state.temperature < before.state.temperature);
    light.tick(800);
    assert_eq!(frame(&light), OutputFrame::from_state(target));
    assert_eq!(memory.writes.get(), writes + 1);
}

#[test]
fn new_move_rates_replace_same_endpoint_tracks_without_a_jump_or_new_flash_write() {
    let (memory, light) = light();
    light.set_power(true).unwrap();
    light.tick(400);
    light.set_level_transition(254, false, 2000).unwrap();
    light
        .set_temperature_transition(ColorTemperature::MIN, 2000)
        .unwrap();
    light.tick(900);
    let before = frame(&light);
    let writes = memory.writes.get();
    light.set_level_rate(254, false, 500).unwrap();
    light
        .set_temperature_rate(ColorTemperature::MIN, 500)
        .unwrap();
    assert_eq!(frame(&light), before);
    assert_eq!(light.level_remaining_ms(), 500);
    assert_eq!(light.temperature_remaining_ms(), 500);
    assert_eq!(memory.writes.get(), writes);
    light.tick(1025);
    assert_eq!(
        frame(&light).brightness.get(),
        before.brightness.get() + (2530 - before.brightness.get()) / 4
    );
    let before = frame(&light);
    light.set_level_rate(254, false, 1000).unwrap();
    light
        .set_temperature_rate(ColorTemperature::MIN, 1000)
        .unwrap();
    assert_eq!(frame(&light), before);
    assert_eq!(light.level_remaining_ms(), 1000);
    assert_eq!(light.temperature_remaining_ms(), 1000);
    light.tick(2025);
    assert_eq!(pwm(&light), [0, 22]);
    // A second downward MoveWithOnOff must also change the ongoing Off rate,
    // even though the durable target already says Off.
    light.set_level_rate(0, true, 2000).unwrap();
    light.tick(2525);
    let before = frame(&light);
    light.set_level_rate(0, true, 500).unwrap();
    assert_eq!(frame(&light), before);
    assert_eq!(light.level_remaining_ms(), 500);
    light.tick(3025);
    assert_eq!(pwm(&light), [0, 0]);
}

#[test]
fn slow_off_effect_never_brightens_even_below_the_minimum_on_level() {
    let (_, light) = light();
    light.set_power(true).unwrap();
    light.tick(100);
    assert_eq!(frame(&light).brightness.get(), 118);
    assert_eq!(pwm(&light), [0, 0]);
    light.off_with_effect(OffEffect::SlowFade).unwrap();
    let mut previous = frame(&light).brightness;
    for now in (120..=12900).step_by(20) {
        light.tick(now);
        let current = frame(&light).brightness;
        assert!(current <= previous);
        assert_eq!(pwm(&light), [0, 0]);
        previous = current;
    }
    assert_eq!(previous, OutputBrightness::ZERO);
    assert!(!light.reported().unwrap().on);
}
