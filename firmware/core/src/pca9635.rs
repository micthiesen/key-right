//! Stock Key Light PCA9635 configuration and readback-checked I2C output.
//!
//! OE is the PCA's existing output-enable input, not an independent isolation
//! circuit. Its power-on behavior differs from the configured stock mode. A bus
//! failure can leave the previous PWM state active, especially with OE tied low.
use crate::{LightOutput, LightState, Preset};

pub const ADDRESS: u8 = 0x15;
pub const MODE2: u8 = 0x14;
pub const FRAME_LEN: usize = 22;
pub const TEMPERATURES_MIRED: [u16; 2] = [303, 200];

/// Registers 0x02..=0x17: PWM[16], GRPPWM, GRPFREQ, LEDOUT[4].
/// Values reproduce original Key Light board 53/build 222 at nominal 3%.
pub fn stock_frame(state: LightState) -> [u8; FRAME_LEN] {
    let mut frame = [0; FRAME_LEN];
    frame[16] = 255;
    frame[18..].fill(0xaa);
    if state.on {
        let [warm, cool] = match state.preset {
            Preset::One => [6, 2],
            Preset::Two => [3, 6],
        };
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

/// Drives the existing active-low PCA OE input, when connected. Successful GPIO
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
}

impl<B: RegisterBus, O: OutputEnable, D: Delay> Pca9635<B, O, D> {
    pub fn new(bus: B, oe: O, delay: D) -> Self {
        Self { bus, oe, delay }
    }

    /// Attempt stock Off through I2C as well as OE, so normal Off also works
    /// when the board ties OE low. An error never establishes physical Off.
    pub fn shutdown(&mut self) -> Result<(), DriverError<B::Error, O::Error>> {
        self.initialize_off()
    }

    pub fn initialize_off(&mut self) -> Result<(), DriverError<B::Error, O::Error>> {
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
        self.bus
            .write_read(ADDRESS, &[0x80], &mut data)
            .map_err(DriverError::Bus)?;
        Ok(data)
    }

    /// Verifies register state, not voltage, emitted light, or OE wiring.
    pub fn verify(&mut self, state: LightState) -> Result<(), DriverError<B::Error, O::Error>> {
        let data = self.read_registers()?;
        // MODE1[7:5] reflects the latest auto-increment control byte.
        if data[0] & 0x1f != 0 || data[1] != MODE2 || data[2..] != stock_frame(state) {
            return Err(DriverError::ReadbackMismatch);
        }
        Ok(())
    }

    fn finish(
        &mut self,
        result: Result<(), DriverError<B::Error, O::Error>>,
    ) -> Result<(), DriverError<B::Error, O::Error>> {
        if result.is_err() {
            // Best effort only: OE may be unconnected/tied low, and the PCA may
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
            // The PCA rail can reset independently of the ESP. Reapply stock
            // configuration instead of trusting a cached initialized flag.
            self.initialize_off()?;
            if state.on {
                self.write_frame(state)?;
                self.verify(state)?;
                self.oe.set_disabled(false).map_err(DriverError::Enable)?;
            }
            Ok(())
        })();
        self.finish(result)
    }
}
