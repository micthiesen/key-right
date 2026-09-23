//! ESP32-C6 Matter bench image. No LED driver or GPIO output is connected.
#![no_std]
#![no_main]

#[cfg(not(feature = "bench-light"))]
compile_error!("This is a simulated bench image. Build with --features bench-light.");

use esp_backtrace as _;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::usb::usb_serial_jtag::UsbSerialJtag;
use tinyrlibc as _;

mod light;
mod matter;
mod output;
mod storage;

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: embassy_executor::Spawner) {
    output::init();
    let peripherals = esp_hal::init(esp_hal::Config::default());
    esp_alloc::heap_allocator!(size: 100 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let software_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, software_interrupt.software_interrupt0);

    let (_rx, tx) = UsbSerialJtag::new(peripherals.USB_DEVICE)
        .into_async()
        .split();
    spawner.spawn(output::writer_task(tx).expect("USB logger task"));
    log::warn!("Key Right Bench: simulated output; no physical light control");

    matter::run(
        peripherals.RNG,
        peripherals.ADC1,
        peripherals.WIFI,
        peripherals.BT,
        peripherals.FLASH,
    )
    .await;
}
