#![no_std]

//! Hardware-independent intended/applied state for the Key Right controller.
//!
//! Brightness is mapped into the original light's nominal 1..10% scale, never
//! measured brightness or raw PWM duty. Colour temperature uses stock mired units.

pub mod pca9635;

/// Nonzero Matter level mapped linearly to nominal stock brightness 1..10%.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Level(u8);

impl Level {
    pub const MIN: Self = Self(1);
    pub const MAX: Self = Self(254);
    /// Approximately stock 3%; preserves the former preset command bytes.
    pub const DEFAULT: Self = Self(57);

    pub const fn new(value: u8) -> Option<Self> {
        if value >= Self::MIN.0 && value <= Self::MAX.0 {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }

    /// Exact rational percentage in the stock API's scale, not optical output.
    pub const fn stock_brightness_ratio(self) -> (u32, u32) {
        (253 + 9 * (self.0 as u32 - 1), 253)
    }
}

impl Default for Level {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Physical transition brightness in 1/253 of a nominal stock percentage.
///
/// Unlike a Matter level, this can reach zero and pass below the normal 1%
/// floor. It remains bounded by the same nominal 10% ceiling, not a measured
/// optical brightness. Only settled targets belong in persistent light state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct OutputBrightness(u16);

impl OutputBrightness {
    pub const ZERO: Self = Self(0);
    pub const MAX: Self = Self(2530);
    pub const DENOMINATOR: u16 = 253;

    pub const fn new(value: u16) -> Option<Self> {
        if value <= Self::MAX.0 {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u16 {
        self.0
    }

    pub const fn from_level(level: Level) -> Self {
        Self(253 + 9 * (level.get() as u16 - 1))
    }

    /// Nearest representable logical level, clamped at the nonzero floor.
    /// Keep this display approximation separate from the exact output value.
    pub const fn to_level_clamped(self) -> Level {
        if self.0 <= 253 {
            Level::MIN
        } else {
            Level((1 + (self.0 - 253 + 4) / 9) as u8)
        }
    }
}

/// Stock colour-temperature command in mired, approximately 7000..2900 K.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ColorTemperature(u16);

impl ColorTemperature {
    pub const MIN: Self = Self(143);
    pub const MAX: Self = Self(344);
    pub const DEFAULT: Self = Self(303);

    pub const fn new(value: u16) -> Option<Self> {
        if value >= Self::MIN.0 && value <= Self::MAX.0 {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

impl Default for ColorTemperature {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Convenience names for the former 3300 K and 5000 K preset commands.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Preset {
    #[default]
    One,
    Two,
}

impl Preset {
    pub const fn temperature(self) -> ColorTemperature {
        match self {
            Self::One => ColorTemperature(303),
            Self::Two => ColorTemperature(200),
        }
    }
}

/// Logical intent. Switching off retains the selected level and temperature.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LightState {
    pub on: bool,
    pub level: Level,
    pub temperature: ColorTemperature,
}

/// Acknowledged output coordinates, distinct from the durable command target.
///
/// `state.level` describes the nearest logical level during a transition;
/// `brightness` retains the exact sub-floor output coordinate. Off always
/// suppresses output, even if a caller supplies a nonzero brightness.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OutputFrame {
    pub state: LightState,
    pub brightness: OutputBrightness,
}

impl OutputFrame {
    pub const fn from_state(state: LightState) -> Self {
        Self {
            state,
            brightness: if state.on {
                OutputBrightness::from_level(state.level)
            } else {
                OutputBrightness::ZERO
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    SetPower(bool),
    SetLevel(Level),
    SetTemperature(ColorTemperature),
    SelectPreset(Preset),
}

/// Boundary for output adapters; acknowledgement does not establish calibration.
pub trait LightOutput {
    type Error;

    /// Return success only when the complete state has been applied.
    ///
    /// An error may mean a partial write. Retrying must be safe and reapply the
    /// whole state. A successful write acknowledges I/O, not measured light output.
    fn apply(&mut self, state: LightState) -> Result<(), Self::Error> {
        self.apply_frame(OutputFrame::from_state(state))
    }

    /// Apply the exact physical frame, including brightness below the ordinary
    /// level floor. Adapters must not reconstruct PWM from `state.level`.
    fn apply_frame(&mut self, frame: OutputFrame) -> Result<(), Self::Error>;
}

#[derive(Debug)]
pub struct Controller {
    intended: LightState,
    applied: Option<OutputFrame>,
}

impl Controller {
    /// Restore validated intent, or start off at the default level/temperature.
    /// Physical output is unknown until the adapter acknowledges an application.
    pub fn new(restored: Option<LightState>) -> Self {
        Self {
            intended: restored.unwrap_or_default(),
            applied: None,
        }
    }

    pub const fn intended(&self) -> LightState {
        self.intended
    }

    /// Last acknowledged state, or unknown after boot or a failed output write.
    pub const fn applied(&self) -> Option<LightState> {
        match self.applied {
            Some(frame) => Some(frame.state),
            None => None,
        }
    }

    /// Last complete output coordinates acknowledged by the adapter.
    pub const fn applied_frame(&self) -> Option<OutputFrame> {
        self.applied
    }

    /// Discard acknowledgement after a hardware reset or failed health check.
    /// Intent survives and the next reconcile reapplies the complete output.
    pub fn invalidate_applied(&mut self) {
        self.applied = None;
    }

    /// Update intent and report whether it changed, so persistence can avoid
    /// redundant writes. This does not itself write flash or change hardware.
    pub fn command(&mut self, command: Command) -> bool {
        let previous = self.intended;
        match command {
            Command::SetPower(on) => self.intended.on = on,
            Command::SetLevel(level) => self.intended.level = level,
            Command::SetTemperature(temperature) => self.intended.temperature = temperature,
            Command::SelectPreset(preset) => self.intended.temperature = preset.temperature(),
        }
        self.intended != previous
    }

    /// Apply pending intent once. The caller owns retry timing and I/O deadlines.
    /// Returns false when there is nothing to write.
    pub fn reconcile<O: LightOutput>(&mut self, output: &mut O) -> Result<bool, O::Error> {
        self.reconcile_frame(output, OutputFrame::from_state(self.intended))
    }

    /// Apply an exact transition frame while retaining the caller's intent.
    /// Only a successful complete application establishes acknowledgement.
    pub fn reconcile_frame<O: LightOutput>(
        &mut self,
        output: &mut O,
        frame: OutputFrame,
    ) -> Result<bool, O::Error> {
        if self.applied == Some(frame) {
            return Ok(false);
        }

        // A failed write can leave output partially updated, so discard the old
        // acknowledgement before attempting I/O and preserve intent for retry.
        self.applied = None;
        output.apply_frame(frame)?;
        self.applied = Some(frame);
        Ok(true)
    }
}
