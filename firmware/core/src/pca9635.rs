//! PCA9635 register profiles and a transport-independent, readback-checked driver.
//!
//! Register semantics: NXP PCA9635 rev. 7.1, sections 7.2 through 7.4.
//! Profiles preserve stock-firmware evidence plus local operator attestations.
//! Structural validation and successful I/O do not prove physical illumination.
//! In particular, OE=HIGH is only lamp-off after the board's external circuitry
//! has been checked in both its power-on and configured states.

use crate::{LightOutput, LightState, Preset, FIXED_BRIGHTNESS_PERCENT};

pub const FRAME_LEN: usize = 22;
pub const PROFILE_LEN: usize = 80;
pub const REQUIRED_ATTESTATIONS: u8 = 0x0f;

/// Registers 0x02..=0x17: PWM[16], GRPPWM, GRPFREQ, LEDOUT[4].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Frame(pub [u8; FRAME_LEN]);

/// A stock configuration and per-light acceptance record. Candidates are uncommissioned.
/// Frames are ordered OFF, preset one, preset two.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Profile {
    address: u8,
    mode2: u8,
    temperatures_mired: [u16; 2],
    frames: [Frame; 3],
    attestations: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileError {
    Length,
    Version,
    Checksum,
    Address,
    Mode,
    Temperature,
    Brightness,
    Attestations,
    IdenticalPresets,
    Blinking,
    UnsupportedProfile,
}

impl Profile {
    /// Candidate for original Key Light board 53/build 222, nominal 3% at
    /// 3300 K (303 mired) and 5000 K (200 mired). Recovered by emulating the
    /// vendor's actual routines; see docs/references/firmware-analysis.md.
    /// This is NOT commissioned: board wiring and boot behaviour still need
    /// physical verification before unattended Matter output.
    pub fn stock_candidate() -> Self {
        let mut off = [0; FRAME_LEN];
        off[16] = 255; // Group dimming is unused with LEDOUT=0xaa.
        off[18..].fill(0xaa);
        let mut one = off;
        one[0] = 6;
        one[4] = 2;
        let mut two = off;
        two[0] = 3;
        two[4] = 6;
        Self::new(
            0x15,
            0x14,
            [303, 200],
            [Frame(off), Frame(one), Frame(two)],
            0,
        )
        .expect("stock candidate is structurally valid")
    }

    /// The direct-PWM build deliberately supports only the recovered stock
    /// profiles. It cannot interpret arbitrary PCA LEDOUT, inversion or group
    /// modes, and must not silently turn those into different physical output.
    pub fn require_stock_pwm(&self) -> Result<(), ProfileError> {
        let stock = Self::stock_candidate();
        if self.address == stock.address
            && self.mode2 == stock.mode2
            && self.temperatures_mired == stock.temperatures_mired
            && self.frames == stock.frames
        {
            Ok(())
        } else {
            Err(ProfileError::UnsupportedProfile)
        }
    }

    /// Warm/cool raw 8-bit duty counts, each divided by 256. Only this exact
    /// stock mode can be translated directly into ESP LEDC output.
    pub fn stock_pwm_duties(&self, state: LightState) -> Result<[u8; 2], ProfileError> {
        self.require_stock_pwm()?;
        let frame = self.frame(state);
        Ok([frame.0[0], frame.0[4]])
    }

    /// `attestations` uses four bits: verified wiring, disabled-output isolation,
    /// safe cold boot/reset, and both requested presets at the stock 3% level.
    /// These are the operator's assertions, not facts established by this parser.
    pub fn new(
        address: u8,
        mode2: u8,
        temperatures_mired: [u16; 2],
        frames: [Frame; 3],
        attestations: u8,
    ) -> Result<Self, ProfileError> {
        // Reject reserved bus addresses and the power-on All Call address.
        if !(0x08..=0x77).contains(&address) || address == 0x70 {
            return Err(ProfileError::Address);
        }
        // No reserved bits, blinking, per-byte output changes, or OUTNE=11.
        if mode2 & !0x17 != 0 || mode2 & 3 == 3 {
            return Err(ProfileError::Mode);
        }
        if temperatures_mired.contains(&0)
            || temperatures_mired.contains(&u16::MAX)
            || temperatures_mired[0] == temperatures_mired[1]
        {
            return Err(ProfileError::Temperature);
        }
        if attestations & !REQUIRED_ATTESTATIONS != 0 {
            return Err(ProfileError::Attestations);
        }
        if frames[1] == frames[2] || frames[0] == frames[1] || frames[0] == frames[2] {
            return Err(ProfileError::IdenticalPresets);
        }
        if frames.iter().any(|frame| frame.0[17] != 0) {
            return Err(ProfileError::Blinking);
        }
        Ok(Self {
            address,
            mode2,
            temperatures_mired,
            frames,
            attestations,
        })
    }

    pub const fn address(&self) -> u8 {
        self.address
    }

    pub const fn mode2(&self) -> u8 {
        self.mode2
    }

    pub const fn temperatures_mired(&self) -> [u16; 2] {
        self.temperatures_mired
    }

    pub const fn frames(&self) -> &[Frame; 3] {
        &self.frames
    }

    pub const fn attestations(&self) -> u8 {
        self.attestations
    }

    /// Production/Matter output must reject an uncommissioned candidate. Local
    /// controlled diagnostics may use it to carry out the checks first.
    pub fn require_commissioned(&self) -> Result<(), ProfileError> {
        if self.attestations == REQUIRED_ATTESTATIONS {
            Ok(())
        } else {
            Err(ProfileError::Attestations)
        }
    }

    pub fn with_attestations(mut self, attestations: u8) -> Result<Self, ProfileError> {
        if attestations & !REQUIRED_ATTESTATIONS != 0 {
            return Err(ProfileError::Attestations);
        }
        self.attestations = attestations;
        Ok(self)
    }

    pub fn frame(&self, state: LightState) -> &Frame {
        &self.frames[if !state.on {
            0
        } else if state.preset == Preset::One {
            1
        } else {
            2
        }]
    }

    /// Fixed format shared by the host tooling, USB console and flash storage.
    pub fn encode(&self) -> [u8; PROFILE_LEN] {
        let mut data = [0; PROFILE_LEN];
        data[..8].copy_from_slice(&[
            b'K',
            b'R',
            1,
            self.address,
            self.mode2,
            FIXED_BRIGHTNESS_PERCENT,
            self.attestations,
            0,
        ]);
        for (i, value) in self.temperatures_mired.iter().enumerate() {
            data[8 + i * 2..10 + i * 2].copy_from_slice(&value.to_le_bytes());
        }
        for (i, frame) in self.frames.iter().enumerate() {
            data[12 + i * FRAME_LEN..12 + (i + 1) * FRAME_LEN].copy_from_slice(&frame.0);
        }
        let crc = crc16(&data[..78]);
        data[78..].copy_from_slice(&crc.to_le_bytes());
        data
    }

    pub fn decode(data: &[u8]) -> Result<Self, ProfileError> {
        if data.len() != PROFILE_LEN {
            return Err(ProfileError::Length);
        }
        if data[..3] != [b'K', b'R', 1] || data[7] != 0 {
            return Err(ProfileError::Version);
        }
        if crc16(&data[..78]) != u16::from_le_bytes([data[78], data[79]]) {
            return Err(ProfileError::Checksum);
        }
        if data[5] != FIXED_BRIGHTNESS_PERCENT {
            return Err(ProfileError::Brightness);
        }
        let mut frames = [Frame([0; FRAME_LEN]); 3];
        for (i, frame) in frames.iter_mut().enumerate() {
            frame
                .0
                .copy_from_slice(&data[12 + i * FRAME_LEN..12 + (i + 1) * FRAME_LEN]);
        }
        Self::new(
            data[3],
            data[4],
            [
                u16::from_le_bytes([data[8], data[9]]),
                u16::from_le_bytes([data[10], data[11]]),
            ],
            frames,
            data[6],
        )
    }
}

/// CRC-16/CCITT-FALSE detects accidental profile corruption; not authentication.
pub fn crc16(bytes: &[u8]) -> u16 {
    let mut crc = 0xffff_u16;
    for &byte in bytes {
        crc ^= u16::from(byte) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

/// Implementations must bound each call with a hardware/software I/O deadline.
pub trait RegisterBus {
    type Error;
    fn write(&mut self, address: u8, bytes: &[u8]) -> Result<(), Self::Error>;
    fn write_read(&mut self, address: u8, bytes: &[u8], out: &mut [u8]) -> Result<(), Self::Error>;
}

/// `disabled=true` must isolate the downstream LED control signals. PCA OE high
/// alone is insufficient at power-on. This reference driver requires a board
/// adapter with an independently verified shutdown path.
pub trait OutputGate {
    type Error;
    fn set_disabled(&mut self, disabled: bool) -> Result<(), Self::Error>;
}

pub trait Delay {
    fn delay_us(&mut self, micros: u32);
}

#[derive(Debug, Eq, PartialEq)]
pub enum DriverError<B, G> {
    Bus(B),
    Gate(G),
    ReadbackMismatch,
}

pub struct Pca9635<B, G, D> {
    bus: B,
    gate: G,
    delay: D,
    profile: Profile,
    initialized: bool,
}

impl<B: RegisterBus, G: OutputGate, D: Delay> Pca9635<B, G, D> {
    pub fn new(bus: B, gate: G, delay: D, profile: Profile) -> Self {
        Self {
            bus,
            gate,
            delay,
            profile,
            initialized: false,
        }
    }

    pub const fn profile(&self) -> &Profile {
        &self.profile
    }

    pub fn into_parts(self) -> (B, G, D, Profile) {
        (self.bus, self.gate, self.delay, self.profile)
    }

    pub fn shutdown(&mut self) -> Result<(), DriverError<B::Error, G::Error>> {
        self.initialized = false;
        self.gate.set_disabled(true).map_err(DriverError::Gate)
    }

    /// Configure only behind the asserted gate, wake the oscillator, and verify
    /// OFF. Never releases OE, even with a valid saved ON intent.
    pub fn initialize_disabled(&mut self) -> Result<(), DriverError<B::Error, G::Error>> {
        self.shutdown()?;
        let result = self.initialize_inner();
        self.finish(result)
    }

    fn initialize_inner(&mut self) -> Result<(), DriverError<B::Error, G::Error>> {
        // Wake; disable all/sub-call responses to avoid address ambiguity.
        self.bus
            .write(self.profile.address, &[0x00, 0x00])
            .map_err(DriverError::Bus)?;
        self.delay.delay_us(500);
        self.bus
            .write(self.profile.address, &[0x01, self.profile.mode2])
            .map_err(DriverError::Bus)?;
        self.write_frame(LightState::default())?;
        self.verify(LightState::default())?;
        self.initialized = true;
        Ok(())
    }

    fn write_frame(&mut self, state: LightState) -> Result<(), DriverError<B::Error, G::Error>> {
        let mut bytes = [0; FRAME_LEN + 1];
        // Auto-increment through all registers, starting at PWM0. One STOP
        // updates all PWM and LEDOUT registers together (MODE2.OCH=0).
        bytes[0] = 0x82;
        bytes[1..].copy_from_slice(&self.profile.frame(state).0);
        self.bus
            .write(self.profile.address, &bytes)
            .map_err(DriverError::Bus)
    }

    pub fn read_registers(&mut self) -> Result<[u8; 24], DriverError<B::Error, G::Error>> {
        let mut data = [0; 24];
        self.bus
            .write_read(self.profile.address, &[0x80], &mut data)
            .map_err(DriverError::Bus)?;
        Ok(data)
    }

    /// Readback acknowledges registers only. The caller must invalidate its
    /// applied state and call shutdown when a periodic health check fails.
    pub fn verify(&mut self, state: LightState) -> Result<(), DriverError<B::Error, G::Error>> {
        let data = self.read_registers()?;
        // MODE1[7:5] mirrors the most recent control byte's auto-increment bits.
        if data[0] & 0x1f != 0
            || data[1] != self.profile.mode2
            || data[2..] != self.profile.frame(state).0
        {
            return Err(DriverError::ReadbackMismatch);
        }
        Ok(())
    }

    fn finish(
        &mut self,
        result: Result<(), DriverError<B::Error, G::Error>>,
    ) -> Result<(), DriverError<B::Error, G::Error>> {
        if result.is_err() {
            self.initialized = false;
            // A failed disable is more urgent than the original I/O error.
            self.gate.set_disabled(true).map_err(DriverError::Gate)?;
        }
        result
    }
}

impl<B: RegisterBus, G: OutputGate, D: Delay> LightOutput for Pca9635<B, G, D> {
    type Error = DriverError<B::Error, G::Error>;

    fn apply(&mut self, state: LightState) -> Result<(), Self::Error> {
        if let Err(error) = self.gate.set_disabled(true) {
            self.initialized = false;
            return Err(DriverError::Gate(error));
        }
        let result = (|| {
            if !self.initialized {
                self.initialize_disabled()?;
            }
            self.write_frame(state)?;
            self.verify(state)?;
            if state.on {
                self.gate.set_disabled(false).map_err(DriverError::Gate)?;
            }
            Ok(())
        })();
        self.finish(result)
    }
}
