#![no_std]

//! Hardware-independent intended/applied state for the Key Right controller.
//!
//! Brightness is a nominal stock-light setting, never a raw PWM duty. The board
//! adapter must supply measured calibration and translate the two preset slots.

pub mod pca9635;

/// Intended brightness while on, in the original light's percentage scale.
pub const FIXED_BRIGHTNESS_PERCENT: u8 = 3;

/// Slots whose actual colour temperatures are still to be supplied.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Preset {
    #[default]
    One,
    Two,
}

/// Logical state; there is deliberately no adjustable brightness field.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LightState {
    pub on: bool,
    pub preset: Preset,
}

impl LightState {
    pub const fn brightness_percent(self) -> u8 {
        if self.on {
            FIXED_BRIGHTNESS_PERCENT
        } else {
            0
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    SetPower(bool),
    SelectPreset(Preset),
    /// Integration compatibility: any brightness write leaves state unchanged.
    SetBrightness(u8),
}

/// Boundary for a future calibrated PCA9635 adapter.
pub trait LightOutput {
    type Error;

    /// Return success only when the complete state has been applied.
    ///
    /// An error may mean a partial write. Retrying must be safe and reapply the
    /// whole state. A successful write acknowledges I/O, not measured light output.
    fn apply(&mut self, state: LightState) -> Result<(), Self::Error>;
}

#[derive(Debug)]
pub struct Controller {
    intended: LightState,
    applied: Option<LightState>,
}

impl Controller {
    /// Restore validated intent, or start off at preset one on first boot.
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
            Command::SelectPreset(preset) => self.intended.preset = preset,
            Command::SetBrightness(_) => {}
        }
        self.intended != previous
    }

    /// Apply pending intent once. The caller owns retry timing and I/O deadlines.
    /// Returns false when there is nothing to write.
    pub fn reconcile<O: LightOutput>(&mut self, output: &mut O) -> Result<bool, O::Error> {
        if self.applied == Some(self.intended) {
            return Ok(false);
        }

        // A failed write can leave output partially updated, so discard the old
        // acknowledgement before attempting I/O and preserve intent for retry.
        self.applied = None;
        output.apply(self.intended)?;
        self.applied = Some(self.intended);
        Ok(true)
    }
}
