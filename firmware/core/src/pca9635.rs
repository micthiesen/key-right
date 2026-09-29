//! Stock Key Light PCA9635 configuration and readback-checked I2C output.
//!
//! OE is the PCA's existing output-enable input, not an independent isolation
//! circuit. Its power-on behavior differs from the configured stock mode. A bus
//! failure can leave the previous PWM state active if the OE path is ineffective.
use crate::{ColorTemperature, Level, LightOutput, LightState};

pub const ADDRESS: u8 = 0x15;
pub const MODE2: u8 = 0x14;
pub const FRAME_LEN: usize = 22;
pub const TEMPERATURES_MIRED: [u16; 2] = [303, 200];

/// Warm/cool bytes following the stock settled arithmetic and channel order.
///
/// The integer stock brightness inputs reproduce board 53/build 222's formula.
/// Fractional brightness is this application's linear extension of that formula;
/// it is not an optical calibration or the stock firmware's fade algorithm.
pub fn stock_pwm_pair(level: Level, temperature: ColorTemperature) -> [u8; 2] {
    let mired = u32::from(temperature.get());
    let [warm, cool] = match mired {
        143 => [0, 100],
        144..=243 => [(mired - 143) * 100 / 101, 100],
        244 => [100, 100],
        _ => [100, 344 - mired],
    };
    let (numerator, denominator) = level.stock_brightness_ratio();
    [
        stock_pwm(warm, numerator, denominator),
        stock_pwm(cool, numerator, denominator),
    ]
}

fn stock_pwm(mix: u32, numerator: u32, denominator: u32) -> u8 {
    // floor(floor(mix * 0.4095 * brightness) / 16) * 0.9,
    // with a final truncation. Combining only the first two floors is exact.
    // The largest numerator is 100 * 4095 * 2530, which fits in u32.
    let steps = mix * 4095 * numerator / (160_000 * denominator);
    (steps * 9 / 10) as u8
}

/// Registers 0x02..=0x17: PWM[16], GRPPWM, GRPFREQ, LEDOUT[4].
/// All unused channels stay zero; only individual PWM is selected.
pub fn stock_frame(state: LightState) -> [u8; FRAME_LEN] {
    let mut frame = [0; FRAME_LEN];
    frame[16] = 255;
    frame[18..].fill(0xaa);
    if state.on {
        let [warm, cool] = stock_pwm_pair(state.level, state.temperature);
        frame[0] = warm;
        frame[4] = cool;
    }
    frame
}

/// Each transaction must have a finite I/O timeout.
pub trait RegisterBus {
    type Error;
    fn write(&mut self, address: u8, bytes: &[u8]) -> Result<(), Self::Error>;
    fn write_read(&mut self, address: u8, bytes: &[u8], out: &mut [u8]) -> Result<(), Self::Error>;
}

/// Drives the existing active-low PCA OE input. Successful GPIO
/// output does not prove that OE is connected or that the physical lamp is off.
pub trait OutputEnable {
    type Error;
    fn set_disabled(&mut self, disabled: bool) -> Result<(), Self::Error>;
}

pub trait Delay {
    fn delay_us(&mut self, micros: u32);
}

#[derive(Debug, Eq, PartialEq)]
pub enum DriverError<B, O> {
    Bus(B),
    Enable(O),
    ReadbackMismatch,
}

pub struct Pca9635<B, O, D> {
    bus: B,
    oe: O,
    delay: D,
    verified_on: Option<LightState>,
}

impl<B: RegisterBus, O: OutputEnable, D: Delay> Pca9635<B, O, D> {
    pub fn new(bus: B, oe: O, delay: D) -> Self {
        Self {
            bus,
            oe,
            delay,
            verified_on: None,
        }
    }

    /// Attempt stock Off through I2C as well as OE, so normal Off also works
    /// when OE is ineffective. An error never establishes physical Off.
    pub fn shutdown(&mut self) -> Result<(), DriverError<B::Error, O::Error>> {
        self.initialize_off()
    }

    pub fn initialize_off(&mut self) -> Result<(), DriverError<B::Error, O::Error>> {
        self.verified_on = None;
        self.oe.set_disabled(true).map_err(DriverError::Enable)?;
        let result = (|| {
            // Configure stock output polarity and zero PWM before waking. This
            // reduces initialization output but cannot guarantee glitch-free POR.
            self.bus
                .write(ADDRESS, &[0x01, MODE2])
                .map_err(DriverError::Bus)?;
            self.write_frame(LightState::default())?;
            // Wake oscillator; disable all/sub-call responses.
            self.bus
                .write(ADDRESS, &[0x00, 0x00])
                .map_err(DriverError::Bus)?;
            self.delay.delay_us(500);
            self.verify(LightState::default())?;
            Ok(())
        })();
        self.finish(result)
    }

    fn write_frame(&mut self, state: LightState) -> Result<(), DriverError<B::Error, O::Error>> {
        let mut bytes = [0; FRAME_LEN + 1];
        bytes[0] = 0x82; // Auto-increment from PWM0; MODE2.OCH=0 updates on STOP.
        bytes[1..].copy_from_slice(&stock_frame(state));
        self.bus.write(ADDRESS, &bytes).map_err(DriverError::Bus)
    }

    pub fn read_registers(&mut self) -> Result<[u8; 24], DriverError<B::Error, O::Error>> {
        let mut data = [0; 24];
        if let Err(error) = self.bus.write_read(ADDRESS, &[0x80], &mut data) {
            self.verified_on = None;
            return Err(DriverError::Bus(error));
        }
        Ok(data)
    }

    /// Verifies register state, not voltage, emitted light, or OE wiring.
    pub fn verify(&mut self, state: LightState) -> Result<(), DriverError<B::Error, O::Error>> {
        let data = self.read_registers()?;
        // MODE1[7:5] reflects the latest auto-increment control byte.
        if data[0] & 0x1f != 0 || data[1] != MODE2 || data[2..] != stock_frame(state) {
            self.verified_on = None;
            return Err(DriverError::ReadbackMismatch);
        }
        Ok(())
    }

    fn finish(
        &mut self,
        result: Result<(), DriverError<B::Error, O::Error>>,
    ) -> Result<(), DriverError<B::Error, O::Error>> {
        if result.is_err() {
            self.verified_on = None;
            // Best effort only: the OE path may be ineffective, and the PCA may
            // have reset to a mode where OE high does not produce lamp-off.
            self.oe.set_disabled(true).map_err(DriverError::Enable)?;
        }
        result
    }
}

impl<B: RegisterBus, O: OutputEnable, D: Delay> LightOutput for Pca9635<B, O, D> {
    type Error = DriverError<B::Error, O::Error>;
    fn apply(&mut self, state: LightState) -> Result<(), Self::Error> {
        let result = (|| {
            if state.on {
                if let Some(previous) = self.verified_on {
                    match self.verify(previous) {
                        Ok(()) => {
                            // Different logical levels/temperatures can encode the
                            // same low-duty frame. Readback acknowledges those
                            // without an output write or an OE interruption.
                            if stock_frame(previous) != stock_frame(state) {
                                self.write_frame(state)?;
                                self.verify(state)?;
                            }
                            self.verified_on = Some(state);
                            return Ok(());
                        }
                        Err(DriverError::ReadbackMismatch) => {
                            // The PCA can reset independently of the ESP. Recover
                            // with the complete parked initialization below.
                        }
                        Err(error) => return Err(error),
                    }
                }
            }
            // The PCA rail can reset independently of the ESP. Reapply stock
            // configuration when not verified or after detecting a mismatch.
            self.initialize_off()?;
            if state.on {
                self.write_frame(state)?;
                self.verify(state)?;
                self.oe.set_disabled(false).map_err(DriverError::Enable)?;
                self.verified_on = Some(state);
            }
            Ok(())
        })();
        self.finish(result)
    }
}

#[cfg(test)]
mod tests {
    use super::stock_pwm;

    #[test]
    fn integer_stock_brightness_preserves_both_truncations() {
        for mix in 0..=100 {
            for brightness in 1..=10 {
                let original = (((f64::from(mix) * 0.4095 * f64::from(brightness)) as u32 / 16)
                    as f64
                    * 0.9) as u8;
                assert_eq!(stock_pwm(mix, brightness, 1), original);
            }
        }
        assert_eq!(
            (
                stock_pwm(100, 1, 1),
                stock_pwm(100, 2, 1),
                stock_pwm(100, 3, 1)
            ),
            (1, 4, 6)
        );
    }
}
