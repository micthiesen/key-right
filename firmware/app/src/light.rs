//! Explicit in-memory output for the bench image; no GPIO or I2C is used.
use crate::runtime::{Hardware, OutputError};
use key_right_core::pca9635::{stock_frame, MODE2};
use key_right_core::{LightOutput, LightState};

#[derive(Default)]
pub struct SimulatedOutput {
    applied: LightState,
}
impl LightOutput for SimulatedOutput {
    type Error = OutputError;
    fn apply(&mut self, state: LightState) -> Result<(), OutputError> {
        self.applied = state;
        Ok(())
    }
}
impl Hardware for SimulatedOutput {
    const MODE: &'static str = "simulation";
    fn shutdown(&mut self) -> Result<(), OutputError> {
        self.applied.on = false;
        Ok(())
    }
    fn verify(&mut self, state: LightState) -> Result<(), OutputError> {
        if stock_frame(state) == stock_frame(self.applied) {
            Ok(())
        } else {
            Err(OutputError::Readback)
        }
    }
    fn registers(&mut self) -> Result<[u8; 24], OutputError> {
        let mut registers = [0; 24];
        registers[1] = MODE2;
        registers[2..].copy_from_slice(&stock_frame(self.applied));
        Ok(registers)
    }
}
