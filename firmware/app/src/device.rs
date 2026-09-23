//! Real XIAO ESP32C6 image. No calibrated output profile is shipped.
#![no_std]
#![no_main]
use esp_backtrace as _;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::time::Duration;
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
mod protocol;
mod recovery;
mod runtime;
mod storage;

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: embassy_executor::Spawner) {
    output::init();
    let p = esp_hal::init(esp_hal::Config::default());
    // TXU0102 OE is held low by an external pulldown until PWM is verified.
    let interlock = Output::new(p.GPIO21, Level::Low, OutputConfig::default());
    let warm = Output::new(p.GPIO18, Level::Low, OutputConfig::default());
    let cool = Output::new(p.GPIO20, Level::Low, OutputConfig::default());
    // Seeed XIAO ESP32C6 external antenna switch: enable low, select high.
    let _antenna_enable = Output::new(p.GPIO3, Level::Low, OutputConfig::default());
    let _antenna_select = Output::new(p.GPIO14, Level::High, OutputConfig::default());
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
    device_matter::run(
        (p.RNG, p.ADC1),
        p.WIFI,
        p.BT,
        p.FLASH,
        hardware::PhysicalOutput::new(
            p.LEDC,
            warm.into_peripheral_output(),
            cool.into_peripheral_output(),
            interlock,
        ),
        rx,
        watchdog,
    )
    .await;
}
