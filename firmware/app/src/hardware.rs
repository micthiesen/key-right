//! XIAO ESP32C6 LEDC output through the TXU0102 isolation/level translator.
//! GPIO21 drives translator OE, with an external pulldown. No PCA9635 bus I/O.
use crate::runtime::{Hardware, OutputError, OutputRegisters};
use esp_hal::gpio::{interconnect::PeripheralOutput, DriveMode, Output, OutputSignal};
use esp_hal::ledc::{
    channel::{self, ChannelHW, ChannelIFace},
    timer::{self, TimerHW, TimerIFace},
    LSGlobalClkSource, Ledc, LowSpeed,
};
use esp_hal::peripherals::{GPIO, LEDC, PCR};
use esp_hal::time::Rate;
use key_right_core::pca9635::Profile;
use key_right_core::{LightOutput, LightState};
use static_cell::StaticCell;

const REQUESTED_HZ: u32 = 97_656;
const DUTY_BITS: u8 = 8;
const SETTLE_US: u32 = 50;

pub struct PhysicalOutput {
    ledc: Ledc<'static>,
    timer: &'static timer::Timer<'static, LowSpeed>,
    warm: channel::Channel<'static, LowSpeed>,
    cool: channel::Channel<'static, LowSpeed>,
    enable: Output<'static>,
    profile: Option<Profile>,
    clock_hz: u32,
    divider_q8: u32,
}
impl PhysicalOutput {
    pub fn new(
        peripheral: LEDC<'static>,
        warm: impl PeripheralOutput<'static>,
        cool: impl PeripheralOutput<'static>,
        mut enable: Output<'static>,
    ) -> Self {
        enable.set_low();
        let mut ledc = Ledc::new(peripheral);
        ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);
        static TIMER: StaticCell<timer::Timer<'static, LowSpeed>> = StaticCell::new();
        let timer = TIMER.init(ledc.timer(timer::Number::Timer0));
        timer
            .configure(timer::config::Config {
                duty: timer::config::Duty::Duty8Bit,
                clock_source: timer::LSClockSource::APBClk,
                frequency: Rate::from_hz(REQUESTED_HZ),
            })
            .expect("LEDC 8-bit carrier");
        let clock_hz = timer.freq().expect("configured source clock").as_hz();
        let divider_q8 = (((u64::from(clock_hz)) << 8) / u64::from(REQUESTED_HZ) / 256) as u32;
        let mut warm = ledc.channel(channel::Number::Channel0, warm);
        let mut cool = ledc.channel(channel::Number::Channel1, cool);
        // HAL sets duty before enabling each pin's peripheral output route.
        warm.configure(channel::config::Config {
            timer,
            duty_pct: 0,
            drive_mode: DriveMode::PushPull,
        })
        .expect("warm PWM");
        cool.configure(channel::config::Config {
            timer,
            duty_pct: 0,
            drive_mode: DriveMode::PushPull,
        })
        .expect("cool PWM");
        Self {
            ledc,
            timer,
            warm,
            cool,
            enable,
            profile: None,
            clock_hz,
            divider_q8,
        }
    }
    fn zero(&mut self) {
        self.enable.set_low();
        self.warm.set_duty_hw(0);
        self.cool.set_duty_hw(0);
        esp_hal::delay::Delay::new().delay_micros(SETTLE_US);
    }
    fn duties(&self, state: LightState) -> Result<[u8; 2], OutputError> {
        self.profile
            .ok_or(OutputError::Unconfigured)?
            .stock_pwm_duties(state)
            .map_err(|_| OutputError::Profile)
    }
    fn check(&self, duties: [u8; 2], enabled: bool) -> Result<(), OutputError> {
        let observed = self.outputs();
        if observed.enabled != enabled {
            return Err(OutputError::Gate);
        }
        let expected = [u32::from(duties[0]) << 4, u32::from(duties[1]) << 4];
        // Pinned esp-hal exposes read-only register access through its unstable
        // safe API. This adapter owns LEDC; no second writer or raw pointer is used.
        let regs = LEDC::regs();
        let timer = regs.timer(0).conf().read();
        let clock = PCR::regs().ledc_sclk_conf().read();
        if observed.warm_command != expected[0]
            || observed.cool_command != expected[1]
            || observed.warm_active != expected[0]
            || observed.cool_active != expected[1]
            || observed.duty_bits != DUTY_BITS
            || observed.divider_q8 != self.divider_q8
            || timer.pause().bit_is_set()
            || timer.rst().bit_is_set()
            || !clock.ledc_sclk_en().bit_is_set()
            || clock.ledc_sclk_sel().bits() != 1
        {
            return Err(OutputError::Readback);
        }
        for index in [0, 1] {
            let channel = regs.ch(index);
            let conf = channel.conf0().read();
            if !conf.sig_out_en().bit_is_set()
                || conf.timer_sel().bits() != 0
                || channel.hpoint().read().hpoint().bits() != 0
            {
                return Err(OutputError::Readback);
            }
        }
        let gpio = GPIO::regs();
        for (pin, signal) in [
            (18, OutputSignal::LEDC_LS_SIG0),
            (20, OutputSignal::LEDC_LS_SIG1),
            (21, OutputSignal::GPIO),
        ] {
            let route = gpio.func_out_sel_cfg(pin).read();
            if route.out_sel().bits() != signal as u8
                || route.inv_sel().bit_is_set()
                || route.oen_inv_sel().bit_is_set()
                || gpio.enable().read().bits() & (1 << pin) == 0
            {
                return Err(OutputError::Readback);
            }
        }
        Ok(())
    }
}
impl LightOutput for PhysicalOutput {
    type Error = OutputError;
    fn apply(&mut self, state: LightState) -> Result<(), OutputError> {
        self.enable.set_low();
        let result = (|| {
            let duties = self.duties(state)?;
            // Reassert peripheral configuration while isolated. This also recovers
            // a timer/channel configuration upset found by the periodic readback.
            self.ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);
            self.timer.configure_hw(self.divider_q8);
            self.timer.update_hw();
            self.warm
                .configure_hw()
                .map_err(|_| OutputError::Readback)?;
            self.cool
                .configure_hw()
                .map_err(|_| OutputError::Readback)?;
            self.warm.set_duty_hw(u32::from(duties[0]));
            self.cool.set_duty_hw(u32::from(duties[1]));
            esp_hal::delay::Delay::new().delay_micros(SETTLE_US);
            self.check(duties, false)?;
            if state.on {
                self.enable.set_high();
            }
            self.check(duties, state.on)
        })();
        if let Err(error) = result {
            self.zero();
            log::error!("PWM output rejected: {error:?}");
        }
        result
    }
}
impl Hardware for PhysicalOutput {
    fn configure(&mut self, profile: Profile) {
        self.zero();
        self.profile = Some(profile);
    }
    fn shutdown(&mut self) -> Result<(), OutputError> {
        self.zero();
        self.check([0, 0], false)
    }
    fn verify(&mut self, state: LightState) -> Result<(), OutputError> {
        self.check(self.duties(state)?, state.on)
    }
    fn outputs(&self) -> OutputRegisters {
        let regs = LEDC::regs();
        let timer = regs.timer(0).conf().read();
        OutputRegisters {
            warm_command: regs.ch(0).duty().read().duty().bits(),
            cool_command: regs.ch(1).duty().read().duty().bits(),
            warm_active: regs.ch(0).duty_r().read().duty_r().bits(),
            cool_active: regs.ch(1).duty_r().read().duty_r().bits(),
            duty_bits: timer.duty_res().bits(),
            divider_q8: timer.clk_div().bits(),
            clock_hz: self.clock_hz,
            enabled: self.enable.is_set_high(),
        }
    }
}
