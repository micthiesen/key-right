//! Bounded I2C to the retained PCA9635; GPIO21 optionally drives its stock OE.
use crate::runtime::{Hardware, OutputError};
use core::convert::Infallible;
use esp_hal::gpio::Output;
use esp_hal::i2c::master::{Error as I2cError, I2c};
use esp_hal::Blocking;
use key_right_core::pca9635::{Delay, DriverError, OutputEnable, Pca9635, RegisterBus};
use key_right_core::{LightOutput, LightState};

struct Bus(I2c<'static, Blocking>);
impl RegisterBus for Bus {
    type Error = I2cError;
    fn write(&mut self, address: u8, bytes: &[u8]) -> Result<(), Self::Error> {
        self.0.write(address, bytes)
    }
    fn write_read(&mut self, address: u8, bytes: &[u8], out: &mut [u8]) -> Result<(), Self::Error> {
        self.0.write_read(address, bytes, out)
    }
}
struct Oe(Output<'static>);
impl OutputEnable for Oe {
    type Error = Infallible;
    fn set_disabled(&mut self, disabled: bool) -> Result<(), Self::Error> {
        self.0.set_level(disabled.into());
        Ok(())
    }
}
struct Wait;
impl Delay for Wait {
    fn delay_us(&mut self, micros: u32) {
        esp_hal::delay::Delay::new().delay_micros(micros);
    }
}
pub struct PhysicalOutput(Pca9635<Bus, Oe, Wait>);
impl PhysicalOutput {
    pub fn new(bus: I2c<'static, Blocking>, oe: Output<'static>) -> Self {
        Self(Pca9635::new(Bus(bus), Oe(oe), Wait))
    }
    fn error(error: DriverError<I2cError, Infallible>) -> OutputError {
        log::error!("PCA operation failed: {error:?}; physical output unknown");
        match error {
            DriverError::Bus(_) => OutputError::Bus,
            DriverError::Enable(never) => match never {},
            DriverError::ReadbackMismatch => OutputError::Readback,
        }
    }
}
impl LightOutput for PhysicalOutput {
    type Error = OutputError;
    fn apply(&mut self, state: LightState) -> Result<(), OutputError> {
        self.0.apply(state).map_err(Self::error)
    }
}
impl Hardware for PhysicalOutput {
    fn shutdown(&mut self) -> Result<(), OutputError> {
        self.0.shutdown().map_err(Self::error)
    }
    fn verify(&mut self, state: LightState) -> Result<(), OutputError> {
        self.0.verify(state).map_err(Self::error)
    }
    fn registers(&mut self) -> Result<[u8; 24], OutputError> {
        self.0.read_registers().map_err(Self::error)
    }
}
