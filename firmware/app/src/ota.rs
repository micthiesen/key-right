//! esp-lan-ota glue for the real lamp image: Matter-delivered updates written
//! to the inactive slot, confirmed only after the lamp proves itself healthy.
use embassy_futures::join::join3;
use embassy_time::{Duration, Instant, Timer};
use esp_bootloader_esp_idf::partitions::{read_partition_table, PARTITION_TABLE_MAX_LEN};
use esp_hal::peripherals::FLASH;
use esp_lan_ota::matter::{announce_enrollment, open_enrollment_window, OtaCluster, OtaService};
use esp_lan_ota::{Chip, Identity, Name, Ota};
use esp_storage::FlashStorage;
use rs_matter_embassy::matter::crypto::Crypto;
use rs_matter_embassy::matter::dm::{
    Cluster, Dataver, Handler, InvokeContext, InvokeReply, MatchContext, NonBlockingHandler,
    ReadContext, ReadReply,
};
use rs_matter_embassy::matter::error::{Error, ErrorCode};
use rs_matter_embassy::matter::Matter;
use static_cell::StaticCell;

use crate::ota_health::{ota_verdict, OtaVerdict};

pub type Flash = FlashStorage<'static>;
pub type Service = OtaService<Flash>;
pub use esp_lan_ota::matter::CLUSTER_ID;
pub const CLUSTER: Cluster<'static> = OtaCluster::<'static, Flash>::CLUSTER;

/// Starts OTA bookkeeping on a second flash handle. `buf` is the caller's
/// existing partition-table buffer, reused so this adds no 3 KiB stack array.
/// Returns `None` (logged) when this boot cannot support updates; the light
/// still runs, and the cluster then refuses update commands.
pub fn start(buf: &mut [u8; PARTITION_TABLE_MAX_LEN]) -> Option<&'static Service> {
    // SAFETY: esp-storage serializes flash operations in a critical section,
    // and both handles run on the same single-core executor.
    let mut flash = FlashStorage::new(unsafe { FLASH::steal() });
    let booted = match read_partition_table(&mut flash, buf).and_then(|t| t.booted_partition()) {
        Ok(Some(partition)) => partition.offset(),
        Ok(None) => {
            log::error!("OTA unavailable: running image is not in a partition");
            return None;
        }
        Err(error) => {
            log::error!("OTA unavailable: partition table: {error:?}");
            return None;
        }
    };
    let desc = &crate::ESP_APP_DESC;
    let (Some(project), Some(version)) =
        (Name::new(desc.project_name()), Name::new(desc.version()))
    else {
        log::error!("OTA unavailable: app descriptor name or version is not a valid OTA name");
        return None;
    };
    let identity = Identity {
        chip: Chip::Esp32c3,
        project,
        version,
    };
    let ota = match Ota::start(flash, booted, identity) {
        Ok(ota) => ota,
        Err(error) => {
            log::error!("OTA unavailable at {booted:#x}: {error:?}; flash with scripts/flash.sh");
            return None;
        }
    };
    let boot = ota.boot_info();
    log::warn!(
        "OTA slot={:?} state={:?} rollback={} enrollment_window={}",
        boot.booted,
        boot.state,
        boot.rollback.name(),
        boot.enrollment_window
    );
    static SERVICE: StaticCell<Service> = StaticCell::new();
    Some(SERVICE.init(OtaService::new(ota)))
}

/// The cluster is always declared on the light endpoint. Without a working
/// OTA service it still answers its global attributes and refuses commands.
pub enum OtaHandler {
    Ready(OtaCluster<'static, Flash>),
    Unavailable(Dataver),
}

impl OtaHandler {
    pub fn new(service: Option<&'static Service>, dataver: Dataver) -> Self {
        match service {
            Some(service) => Self::Ready(OtaCluster::new(service, dataver)),
            None => Self::Unavailable(dataver),
        }
    }
}

impl Handler for OtaHandler {
    fn read(&self, ctx: impl ReadContext, reply: impl ReadReply) -> Result<(), Error> {
        match self {
            Self::Ready(cluster) => cluster.read(ctx, reply),
            Self::Unavailable(dataver) => {
                let attr = ctx.attr();
                let Some(writer) = reply.with_dataver(dataver.get())? else {
                    return Ok(());
                };
                if attr.is_system() {
                    CLUSTER.read(attr, writer)
                } else {
                    Err(ErrorCode::AttributeNotFound.into())
                }
            }
        }
    }

    fn invoke(&self, ctx: impl InvokeContext, reply: impl InvokeReply) -> Result<(), Error> {
        match self {
            Self::Ready(cluster) => cluster.invoke(ctx, reply),
            Self::Unavailable(_) => Err(ErrorCode::Failure.into()),
        }
    }

    fn bump_dataver(&self, ctx: impl MatchContext) {
        match self {
            Self::Ready(cluster) => cluster.bump_dataver(ctx),
            Self::Unavailable(dataver) => {
                dataver.changed();
            }
        }
    }
}

impl NonBlockingHandler for OtaHandler {}

/// Opens the post-USB-flash enrollment window. Call after Matter state loads.
pub fn open_window<C: Crypto>(
    service: Option<&'static Service>,
    matter: &Matter<'_>,
    crypto: C,
) -> Option<esp_lan_ota::matter::Enrollment> {
    match open_enrollment_window(&service?.boot_info(), matter, crypto) {
        Ok(enrollment) => enrollment,
        Err(error) => {
            log::error!("OTA enrollment window: {error:?}");
            None
        }
    }
}

/// Runs for the life of the image: announces enrollment, confirms or rolls
/// back a new image, and reboots into a staged update. `prepare_reboot`
/// drives verified zero output without changing durable intent; the next
/// boot restores saved power, level and temperature as after any reset.
pub async fn supervise(
    service: Option<&'static Service>,
    enrollment: Option<esp_lan_ota::matter::Enrollment>,
    commissioned: impl Fn() -> bool,
    mut output_verified: impl FnMut() -> bool,
    prepare_reboot: impl Fn() -> bool,
) -> ! {
    let Some(service) = service else {
        core::future::pending().await
    };
    let announce = async {
        if let Some(enrollment) = enrollment {
            // Ends the enrollment period after the window's 15 minutes.
            announce_enrollment(service, enrollment, |line| log::warn!("{line}")).await;
        }
    };
    let confirm = async {
        while service.is_unconfirmed() {
            let uptime_ms = Instant::now().as_millis();
            let verdict = ota_verdict(
                uptime_ms,
                commissioned(),
                crate::network::current_healthy_ms(uptime_ms),
                &mut output_verified,
            );
            match verdict {
                OtaVerdict::Wait => {}
                OtaVerdict::Confirm => match service.confirm() {
                    Ok(_) => log::warn!("OTA image confirmed after {uptime_ms} ms"),
                    Err(error) => log::error!("OTA confirm write failed: {error:?}; retrying"),
                },
                OtaVerdict::RollBack => {
                    let off = prepare_reboot();
                    log::error!(
                        "OTA image unhealthy after {uptime_ms} ms; resetting unconfirmed to roll back; off_registers_verified={off}"
                    );
                    Timer::after(Duration::from_millis(250)).await;
                    esp_hal::system::software_reset();
                }
            }
            // Every pass yields, so a failed confirm cannot starve the watchdog.
            Timer::after(Duration::from_secs(5)).await;
        }
    };
    let reboot = async {
        service.wait_reboot().await;
        let off = prepare_reboot();
        log::warn!(
            "OTA update staged; rebooting with intent/fabrics preserved; off_registers_verified={off}"
        );
        Timer::after(Duration::from_millis(250)).await;
        esp_hal::system::software_reset();
    };
    join3(announce, confirm, reboot).await;
    unreachable!("the reboot future never completes")
}
