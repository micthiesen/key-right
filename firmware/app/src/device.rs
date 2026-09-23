//! XIAO ESP32C6 controlling the retained stock PCA9635 over I2C.
#![no_std]
#![no_main]
use esp_backtrace as _;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::i2c::master::{BusTimeout, Config as I2cConfig, I2c, SoftwareTimeout};
use esp_hal::time::{Duration, Rate};
use esp_hal::timer::timg::TimerGroup;
use esp_hal::usb::usb_serial_jtag::UsbSerialJtag;
use tinyrlibc as _;

mod console;
mod device_matter;
mod hardware;
mod network;
mod network_driver;
mod output;
mod presets;
mod recovery;
mod runtime;
mod storage;

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: embassy_executor::Spawner) {
    output::init();
    let p = esp_hal::init(esp_hal::Config::default());
    // Optional connection to stock active-low OE. HIGH is not a POR safety guarantee.
    let oe = Output::new(p.GPIO21, Level::High, OutputConfig::default());
    let bus = I2c::new(
        p.I2C0,
        I2cConfig::default()
            .with_frequency(Rate::from_khz(100))
            .with_timeout(BusTimeout::BusCycles(1000))
            .with_software_timeout(SoftwareTimeout::Transaction(Duration::from_millis(25))),
    )
    .expect("100 kHz I2C configuration")
    .with_sda(p.GPIO18)
    .with_scl(p.GPIO20);
    // Seeed XIAO ESP32C6 onboard antenna: switch enable low, select low.
    let _antenna_enable = Output::new(p.GPIO3, Level::Low, OutputConfig::default());
    let _antenna_select = Output::new(p.GPIO14, Level::Low, OutputConfig::default());
    esp_alloc::heap_allocator!(size: 100 * 1024);
    let timg0 = TimerGroup::new(p.TIMG0);
    let software_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(p.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, software_interrupt.software_interrupt0);
    let mut watchdog = timg0.wdt;
    watchdog.set_timeout(
        esp_hal::timer::timg::MwdtStage::Stage0,
        Duration::from_secs(15),
    );
    watchdog.enable();
    let (rx, tx) = UsbSerialJtag::new(p.USB_DEVICE).into_async().split();
    spawner.spawn(output::writer_task(tx).expect("USB writer"));
    log::warn!(
        "Key Right hardware mode; reset={:?}; no physical light acknowledgement is claimed",
        esp_hal::system::reset_reason()
    );
    let mut hardware = hardware::PhysicalOutput::new(bus, oe);
    // Try stock zero-PWM before Matter initialization. Absence of the PCA must
    // not block USB commissioning-code retrieval before the board is wired.
    use runtime::Hardware as _;
    let _ = hardware.shutdown();
    device_matter::run(
        (p.RNG, p.ADC1),
        p.WIFI,
        p.BT,
        p.FLASH,
        hardware,
        rx,
        watchdog,
    )
    .await;
}
