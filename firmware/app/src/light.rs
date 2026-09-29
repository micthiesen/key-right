//! Explicit in-memory output for the bench image; no GPIO or I2C is used.
use crate::runtime::{Hardware, OutputError};
use key_right_core::pca9635::{output_frame, MODE2};
use key_right_core::{LightOutput, LightState, OutputFrame};

#[derive(Default)]
pub struct SimulatedOutput {
    applied: OutputFrame,
}
impl LightOutput for SimulatedOutput {
    type Error = OutputError;
    fn apply_frame(&mut self, frame: OutputFrame) -> Result<(), OutputError> {
        self.applied = frame;
        Ok(())
    }
}
impl Hardware for SimulatedOutput {
    const MODE: &'static str = "simulation";
    fn shutdown(&mut self) -> Result<(), OutputError> {
        self.applied = OutputFrame::from_state(LightState {
            on: false,
            ..self.applied.state
        });
        Ok(())
    }
    fn verify_frame(&mut self, frame: OutputFrame) -> Result<(), OutputError> {
        if output_frame(frame) == output_frame(self.applied) {
            Ok(())
        } else {
            Err(OutputError::Readback)
        }
    }
    fn registers(&mut self) -> Result<[u8; 24], OutputError> {
        let mut registers = [0; 24];
        registers[1] = MODE2;
        registers[2..].copy_from_slice(&output_frame(self.applied));
        Ok(registers)
    }
}
