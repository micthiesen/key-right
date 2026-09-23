//! Bounded asynchronous USB logging, adapted from Stillair's output.rs.
//! A disconnected USB host can drop logs without blocking the Matter task.

use core::fmt::Write as _;
use core::sync::atomic::{AtomicU32, Ordering};

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embedded_io_async::Write as _;
use esp_hal::usb::usb_serial_jtag::UsbSerialJtagTx;
use esp_hal::Async;
use heapless::String;

type Line = String<1024>;
struct Record {
    line: Line,
    flush: bool,
}
static LINES: Channel<CriticalSectionRawMutex, Record, 16> = Channel::new();
static DROPPED: AtomicU32 = AtomicU32::new(0);
static TOTAL_DROPPED: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "hardware-light")]
static FLUSHED: embassy_sync::signal::Signal<CriticalSectionRawMutex, ()> =
    embassy_sync::signal::Signal::new();

struct QueueLogger;

impl log::Log for QueueLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Info
            // Hardware commissioning credentials are returned only by an explicit USB command.
            && !(cfg!(feature = "hardware-light") && metadata.target() == "rs_matter")
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let mut text = Line::new();
        let _ = write!(text, "[{}] {}", record.level(), record.args());
        if LINES
            .try_send(Record {
                line: text,
                flush: false,
            })
            .is_err()
        {
            DROPPED.fetch_add(1, Ordering::Relaxed);
            TOTAL_DROPPED.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn flush(&self) {}
}

static LOGGER: QueueLogger = QueueLogger;

pub fn init() {
    log::set_logger(&LOGGER).expect("install USB logger once");
    log::set_max_level(log::LevelFilter::Info);
}

#[cfg(feature = "hardware-light")]
pub async fn response(line: Line) {
    LINES.send(Record { line, flush: false }).await;
}

#[cfg(feature = "hardware-light")]
pub async fn response_and_flush(line: Line) {
    FLUSHED.reset();
    let _ = embassy_time::with_timeout(embassy_time::Duration::from_secs(1), async {
        LINES.send(Record { line, flush: true }).await;
        FLUSHED.wait().await;
    })
    .await;
}

#[cfg(feature = "hardware-light")]
pub fn dropped() -> u32 {
    TOTAL_DROPPED.load(Ordering::Relaxed)
}

#[embassy_executor::task]
pub async fn writer_task(mut tx: UsbSerialJtagTx<'static, Async>) {
    loop {
        let record = LINES.receive().await;
        let dropped = DROPPED.swap(0, Ordering::Relaxed);
        if dropped != 0 {
            let mut notice = String::<64>::new();
            let _ = writeln!(notice, "[WARN] USB logs dropped: {dropped}");
            let _ = tx.write_all(notice.as_bytes()).await;
        }
        let _ = tx.write_all(record.line.as_bytes()).await;
        let _ = tx.write_all(b"\n").await;
        if record.flush {
            let _ = tx.flush().await;
            #[cfg(feature = "hardware-light")]
            FLUSHED.signal(());
        }
    }
}
