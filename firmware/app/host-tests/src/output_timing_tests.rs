//! Exercise the production runtime and simulated PCA adapter: every command
//! applies at once, and only a Matter Move ramps.
use crate::light::SimulatedOutput;
use crate::runtime::{Runtime, INTENT_KEY};
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
fn power_level_and_temperature_apply_in_the_same_call_with_one_flash_write() {
    let (memory, light) = light();
    let writes = memory.writes.get();
    light.set_power(true).unwrap();
    assert_eq!(pwm(&light), [6, 2]);
    assert_eq!(
        frame(&light),
        OutputFrame::from_state(light.reported().unwrap())
    );
    assert_eq!(memory.writes.get(), writes + 1);
    light.set_level(Level::MAX, false).unwrap();
    light.set_temperature(ColorTemperature::MIN).unwrap();
    assert_eq!(pwm(&light), [0, 22]);
    assert_eq!(light.level_remaining_ms(), 0);
    assert_eq!(light.temperature_remaining_ms(), 0);
    assert_eq!(memory.writes.get(), writes + 3);
    light.set_power(false).unwrap();
    assert_eq!(pwm(&light), [0, 0]);
    assert_eq!(frame(&light).brightness, OutputBrightness::ZERO);
    // Later ticks change nothing: there is no animation left to run.
    let revision = light.snapshot().revision;
    for now in (20..=2000).step_by(20) {
        light.tick(now);
        assert_eq!(pwm(&light), [0, 0]);
    }
    assert_eq!(light.snapshot().revision, revision);
    assert_eq!(memory.writes.get(), writes + 4);
}

#[test]
fn a_matter_move_ramps_linearly_and_any_other_command_ends_it_at_once() {
    let (memory, light) = light();
    light.set_power(true).unwrap();
    let initial = frame(&light).brightness.get();
    let writes = memory.writes.get();
    light.set_level_rate(254, false, 1000).unwrap();
    light
        .set_temperature_rate(ColorTemperature::MIN, 1000)
        .unwrap();
    assert_eq!(frame(&light).brightness.get(), initial);
    light.tick(250);
    assert_eq!(
        frame(&light).brightness.get(),
        initial + (2530 - initial) / 4
    );
    assert_eq!(frame(&light).state.temperature.get(), 263);
    assert_eq!(light.level_remaining_ms(), 750);
    // Home reports the destination, and Move frames never write flash.
    assert_eq!(light.reported().unwrap().level, Level::MAX);
    assert_eq!(memory.writes.get(), writes + 2);
    light.set_level(Level::new(57).unwrap(), false).unwrap();
    light.set_temperature(ColorTemperature::DEFAULT).unwrap();
    assert_eq!(pwm(&light), [6, 2]);
    assert_eq!(light.level_remaining_ms(), 0);
    assert_eq!(light.temperature_remaining_ms(), 0);
}

#[test]
fn a_new_move_rate_restarts_from_the_acknowledged_frame_without_a_jump() {
    let (memory, light) = light();
    light.set_power(true).unwrap();
    light.set_level_rate(254, false, 2000).unwrap();
    light.tick(500);
    let before = frame(&light);
    let writes = memory.writes.get();
    light.set_level_rate(254, false, 500).unwrap();
    assert_eq!(frame(&light), before);
    assert_eq!(light.level_remaining_ms(), 500);
    assert_eq!(memory.writes.get(), writes);
    light.tick(1000);
    assert_eq!(frame(&light).brightness, OutputBrightness::MAX);
    // A second downward MoveWithOnOff also changes the rate of a running
    // Move to Off, even though the durable target already says Off.
    light.set_level_rate(0, true, 2000).unwrap();
    light.tick(1500);
    let before = frame(&light);
    light.set_level_rate(0, true, 500).unwrap();
    assert_eq!(frame(&light), before);
    assert_eq!(light.level_remaining_ms(), 500);
    light.tick(2000);
    assert_eq!(pwm(&light), [0, 0]);
}

#[test]
fn remembered_off_settings_never_restart_or_recolour_a_move_down_to_off() {
    let (_, light) = light();
    light.set_power(true).unwrap();
    light.set_level_rate(0, true, 1000).unwrap();
    light.tick(200);
    let before = frame(&light);
    light.set_level(Level::MAX, false).unwrap();
    light.set_temperature(ColorTemperature::MIN).unwrap();
    assert_eq!(frame(&light), before);
    assert_eq!(light.level_remaining_ms(), 800);
    for now in (220..1000).step_by(20) {
        light.tick(now);
        assert!(frame(&light).brightness <= before.brightness);
        assert_eq!(frame(&light).state.temperature, ColorTemperature::DEFAULT);
        assert!(!light.reported().unwrap().on);
    }
    light.tick(1000);
    assert_eq!(pwm(&light), [0, 0]);
    light.set_power(true).unwrap();
    assert_eq!(pwm(&light), [0, 22]);
}

#[test]
fn reboot_restores_the_saved_setting_at_once_and_saved_off_never_energizes() {
    let (memory, light) = light();
    light.set_power(true).unwrap();
    light.set_level(Level::MAX, false).unwrap();
    let target = light.reported().unwrap();
    let reboot = Runtime::load(memory.clone(), SimulatedOutput::default());
    // The first tick proves zero and then applies the saved setting directly.
    reboot.tick(0);
    assert_eq!(reboot.reported().unwrap(), target);
    assert_eq!(frame(&reboot), OutputFrame::from_state(target));
    assert_eq!(reboot.level_remaining_ms(), 0);
    // Power cut during a Move to Off: the persisted Off, not the
    // intermediate On frame, governs the next boot.
    reboot.set_level_rate(0, true, 1000).unwrap();
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
fn stop_in_the_subfloor_tail_and_local_off_cannot_promote_zero_to_minimum_on() {
    let (_, light) = light();
    light.set_power(true).unwrap();
    light.set_level_rate(0, true, 1000).unwrap();
    light.tick(950);
    assert_eq!(pwm(&light), [0, 0]);
    assert_ne!(frame(&light).brightness, OutputBrightness::ZERO);
    light.stop_level_transition().unwrap();
    assert!(!light.reported().unwrap().on);
    assert_eq!(pwm(&light), [0, 0]);
    light.set_power(true).unwrap();
    light.set_level(Level::MAX, false).unwrap();
    assert_ne!(pwm(&light), [0, 0]);
    light.off().unwrap();
    assert_eq!(pwm(&light), [0, 0]);
    assert_eq!(light.level_remaining_ms(), 0);
    assert!(!light.reported().unwrap().on);
}

#[test]
fn scene_recall_commits_all_targets_once_and_applies_both_axes_at_once() {
    let (memory, light) = light();
    light.set_power(true).unwrap();
    let target = LightState {
        on: true,
        level: Level::MAX,
        temperature: ColorTemperature::MIN,
    };
    let writes = memory.writes.get();
    light.request(target).unwrap();
    assert_eq!(memory.writes.get(), writes + 1);
    assert_eq!(frame(&light), OutputFrame::from_state(target));
    assert_eq!(light.level_remaining_ms(), 0);
    assert_eq!(light.temperature_remaining_ms(), 0);
}

#[test]
fn off_during_a_move_down_to_off_lands_at_once() {
    let (_, light) = light();
    light.set_power(true).unwrap();
    light.set_level_rate(0, true, 2000).unwrap();
    light.tick(500);
    assert_ne!(frame(&light).brightness, OutputBrightness::ZERO);
    light.set_power(false).unwrap();
    assert_eq!(pwm(&light), [0, 0]);
    assert_eq!(light.level_remaining_ms(), 0);
}
