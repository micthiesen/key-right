//! Driver peripheral lifecycle from the pinned rs-matter-embassy ESP adapter.
//! Recreating the controller clears operations cancelled by network deadlines.
macro_rules! must {
    ($value:expr) => {
        $value.map_err(|_| {
            rs_matter_embassy::matter::error::Error::from(
                rs_matter_embassy::matter::error::ErrorCode::NoNetworkInterface,
            )
        })?
    };
}
use bt_hci::controller::ExternalController;

use esp_radio::ble::controller::BleConnector;

use crate::network::Controller;
use crate::network_tx::GuardedDriver;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use portable_atomic::{AtomicU64, Ordering};
use rs_matter_embassy::matter::error::Error;
const SLOTS: usize = 20;
pub static TX_STALL: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static TX_BLOCKED_SINCE: AtomicU64 = AtomicU64::new(u64::MAX);

fn queue_health(blocked_since: Option<u64>) {
    TX_BLOCKED_SINCE.store(blocked_since.unwrap_or(u64::MAX), Ordering::Relaxed);
}

pub fn reset_health() {
    queue_health(None);
}

pub fn healthy_until(now: u64) -> u64 {
    let since = TX_BLOCKED_SINCE.load(Ordering::Relaxed);
    crate::network_tx::healthy_until(now, (since != u64::MAX).then_some(since))
}

fn queue_stalled() {
    log::error!("Wi-Fi transmit queue unavailable for 60s while link is up");
    TX_STALL.signal(());
}
use rs_matter_embassy::wireless::{
    BleDriver, BleDriverTask, WifiCoexDriver, WifiCoexDriverTask, WifiDriver, WifiDriverTask,
};

/// Optional bench evidence before commissioning, without credentials or a join.
#[cfg(feature = "radio-diagnostics")]
async fn diagnostic_scan(
    controller: &mut esp_radio::wifi::WifiController<'_>,
) -> Result<(), Error> {
    use embassy_time::{with_timeout, Duration, Timer};
    use esp_hal::time::Duration as RadioDuration;
    use esp_radio::wifi::{
        scan::{ScanConfig, ScanTypeConfig},
        sta::StationConfig,
        Config,
    };
    use rs_matter_embassy::matter::error::ErrorCode;

    must!(controller.set_config(&Config::Station(StationConfig::default())));
    let scans = [
        ("default", ScanConfig::default()),
        (
            "active-long",
            ScanConfig::default().with_scan_type(ScanTypeConfig::Active {
                min: RadioDuration::from_millis(100),
                max: RadioDuration::from_millis(300),
            }),
        ),
        (
            "passive",
            ScanConfig::default()
                .with_scan_type(ScanTypeConfig::Passive(RadioDuration::from_millis(300))),
        ),
    ];
    for (label, config) in scans {
        log::info!("RADIO {label} scan starting; no connection requested");
        let config = config.with_max(20);
        let aps = with_timeout(Duration::from_secs(15), controller.scan_async(&config))
            .await
            .map_err(|_| {
                log::error!("RADIO {label} scan timed out after 15s");
                Error::from(ErrorCode::TxTimeout)
            })?
            .map_err(|error| {
                log::error!("RADIO {label} scan failed: {error:?}");
                Error::from(ErrorCode::NoNetworkInterface)
            })?;
        log::info!("RADIO {label} scan: {} access points", aps.len());
        for ap in aps {
            log::info!(
                "RADIO {label} ssid={:?} bssid={:02x?} channel={} rssi={} security={:?}",
                ap.ssid,
                ap.bssid,
                ap.channel,
                ap.signal_strength,
                ap.auth_method
            );
            // Let the bounded USB log queue drain during a dense scan.
            Timer::after(Duration::from_millis(20)).await;
        }
    }
    Ok(())
}

/// A `WifiDriver` implementation for the ESP32 family of chips.
pub struct EspWifiDriver<'d> {
    wifi_peripheral: esp_hal::peripherals::WIFI<'d>,
    bt_peripheral: esp_hal::peripherals::BT<'d>,
}

impl<'d> EspWifiDriver<'d> {
    /// Create a new instance of the `Esp32WifiDriver` type.
    ///
    /// # Arguments
    /// - `controller` - The `esp-radio` controller instance.
    /// - `peripheral` - The Wifi peripheral instance.
    pub fn new(
        wifi_peripheral: esp_hal::peripherals::WIFI<'d>,
        bt_peripheral: esp_hal::peripherals::BT<'d>,
    ) -> Self {
        Self {
            wifi_peripheral,
            bt_peripheral,
        }
    }
}

impl WifiDriver for EspWifiDriver<'_> {
    type NetCtl<'a>
        = Controller<'a>
    where
        Self: 'a;

    async fn run<A>(&mut self, mut task: A) -> Result<(), Error>
    where
        A: WifiDriverTask,
    {
        let mut controller = must!(esp_radio::wifi::WifiController::new(
            self.wifi_peripheral.reborrow(),
            esp_radio::wifi::ControllerConfig::default(),
        ));

        // Keep the radio awake to avoid power-save receive latency.
        must!(controller.set_power_saving(esp_radio::wifi::PowerSaveMode::None));

        #[cfg(feature = "radio-diagnostics")]
        diagnostic_scan(&mut controller).await?;

        task.run(
            GuardedDriver::new(esp_radio::wifi::Interface::station(), queue_stalled)
                .with_health_observer(queue_health),
            Controller::new(controller),
        )
        .await
    }
}

impl WifiCoexDriver for EspWifiDriver<'_> {
    async fn run<A>(&mut self, mut task: A) -> Result<(), Error>
    where
        A: WifiCoexDriverTask,
    {
        let ble_ctl = ExternalController::<_, SLOTS>::new(must!(BleConnector::new(
            self.bt_peripheral.reborrow(),
            Default::default(),
        )));

        let mut controller = must!(esp_radio::wifi::WifiController::new(
            self.wifi_peripheral.reborrow(),
            esp_radio::wifi::ControllerConfig::default(),
        ));

        // Keep the radio awake to avoid power-save receive latency.
        must!(controller.set_power_saving(esp_radio::wifi::PowerSaveMode::None));

        #[cfg(feature = "radio-diagnostics")]
        diagnostic_scan(&mut controller).await?;

        task.run(
            GuardedDriver::new(esp_radio::wifi::Interface::station(), queue_stalled)
                .with_health_observer(queue_health),
            Controller::new(controller),
            ble_ctl,
        )
        .await
    }
}

impl BleDriver for EspWifiDriver<'_> {
    async fn run<A>(&mut self, mut task: A) -> Result<(), Error>
    where
        A: BleDriverTask,
    {
        let ble_controller = ExternalController::<_, SLOTS>::new(must!(BleConnector::new(
            self.bt_peripheral.reborrow(),
            Default::default(),
        )));

        task.run(ble_controller).await
    }
}
