//! ESP32-C3 Matter bench image. No PCA or application GPIO output is driven.
#![no_std]
#![no_main]

#[cfg(not(feature = "bench-light"))]
compile_error!("This is a simulated bench image. Build with --features bench-light.");

use esp_backtrace as _;
use esp_hal::ram;
use esp_hal::time::Duration;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::usb::usb_serial_jtag::UsbSerialJtag;
use esp_metadata_generated::memory_range;
use tinyrlibc as _;

mod console;
mod device_matter;
mod light;
mod network;
mod network_driver;
mod output;
mod presets;
mod recovery;
mod runtime;
mod storage;

esp_bootloader_esp_idf::esp_app_desc!();

const HEAP_SIZE: usize = 100 * 1024;
const RECLAIMED_RAM: usize =
    memory_range!("DRAM2_UNINIT").end - memory_range!("DRAM2_UNINIT").start;

#[esp_rtos::main]
async fn main(spawner: embassy_executor::Spawner) {
    output::init();
    let peripherals = esp_hal::init(esp_hal::Config::default());
    esp_alloc::heap_allocator!(size: HEAP_SIZE - RECLAIMED_RAM);
    esp_alloc::heap_allocator!(#[ram(reclaimed)] size: RECLAIMED_RAM);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let software_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, software_interrupt.software_interrupt0);
    let mut watchdog = timg0.wdt;
    watchdog.set_timeout(
        esp_hal::timer::timg::MwdtStage::Stage0,
        Duration::from_secs(15),
    );
    watchdog.enable();

    let (rx, tx) = UsbSerialJtag::new(peripherals.USB_DEVICE)
        .into_async()
        .split();
    spawner.spawn(output::writer_task(tx).expect("USB logger task"));
    log::warn!("Key Right Bench: simulated output; no physical light control");

    device_matter::run(
        (peripherals.RNG, peripherals.ADC1),
        peripherals.WIFI,
        peripherals.BT,
        peripherals.FLASH,
        light::SimulatedOutput::default(),
        rx,
        watchdog,
    )
    .await;
}
