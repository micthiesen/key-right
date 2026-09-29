//! Adapted from pinned rs-matter-embassy wifi/esp.rs, adding operation deadlines.
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{with_timeout, Duration};
use portable_atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering};
pub static RESTART: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static DISCONNECT: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static DISCONNECT_PENDING: AtomicBool = AtomicBool::new(false);
static TIMEOUTS: AtomicU32 = AtomicU32::new(0);
static ATTEMPTS: AtomicU32 = AtomicU32::new(0);
static CONNECTED: AtomicU32 = AtomicU32::new(0);
static RESTARTS: AtomicU32 = AtomicU32::new(0);
static RSSI: AtomicI32 = AtomicI32::new(i32::MIN);
static LOCAL_READY: AtomicU32 = AtomicU32::new(0);
static IPV4_READY: AtomicU32 = AtomicU32::new(0);
static IP_TIMEOUTS: AtomicU32 = AtomicU32::new(0);
static HEALTHY_SINCE: AtomicU64 = AtomicU64::new(u64::MAX);
static LONGEST_HEALTHY_MS: AtomicU64 = AtomicU64::new(0);
/// Queue a real link interruption for the local diagnostic. The normal Matter
/// network manager reconnects using the existing saved credentials.
pub fn request_disconnect() -> Result<(), Error> {
    if CONNECTED.load(Ordering::Relaxed) == 0 {
        return Err(ErrorCode::InvalidState.into());
    }
    DISCONNECT_PENDING
        .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
        .map_err(|_| Error::from(ErrorCode::Busy))?;
    DISCONNECT.signal(());
    Ok(())
}
/// Longest continuous healthy interval in this transport run, including a
/// completed interval before its failure. Call before `restarted()` clears it.
pub fn healthy_for_ms(now_ms: u64) -> u64 {
    let now_ms = crate::network_driver::healthy_until(now_ms);
    let since = HEALTHY_SINCE.load(Ordering::Relaxed);
    let current = if since == u64::MAX
        || CONNECTED.load(Ordering::Relaxed) == 0
        || LOCAL_READY.load(Ordering::Relaxed) == 0
        || RSSI.load(Ordering::Relaxed) == i32::MIN
    {
        0
    } else {
        now_ms.saturating_sub(since)
    };
    current.max(LONGEST_HEALTHY_MS.load(Ordering::Relaxed))
}
fn end_healthy_period(now_ms: u64) {
    let now_ms = crate::network_driver::healthy_until(now_ms);
    let since = HEALTHY_SINCE.swap(u64::MAX, Ordering::Relaxed);
    if since != u64::MAX {
        LONGEST_HEALTHY_MS.fetch_max(now_ms.saturating_sub(since), Ordering::Relaxed);
    }
}
pub fn local_metrics() -> (Option<i32>, bool, bool, u32) {
    let rssi = RSSI.load(Ordering::Relaxed);
    (
        (rssi != i32::MIN).then_some(rssi),
        LOCAL_READY.load(Ordering::Relaxed) != 0,
        IPV4_READY.load(Ordering::Relaxed) != 0,
        IP_TIMEOUTS.load(Ordering::Relaxed),
    )
}
pub fn metrics() -> (u32, u32, u32, bool) {
    (
        ATTEMPTS.load(Ordering::Relaxed),
        TIMEOUTS.load(Ordering::Relaxed),
        RESTARTS.load(Ordering::Relaxed),
        CONNECTED.load(Ordering::Relaxed) != 0,
    )
}
pub fn restarted() {
    crate::network_driver::reset_health();
    RESTARTS.fetch_add(1, Ordering::Relaxed);
    CONNECTED.store(0, Ordering::Relaxed);
    RSSI.store(i32::MIN, Ordering::Relaxed);
    LOCAL_READY.store(0, Ordering::Relaxed);
    IPV4_READY.store(0, Ordering::Relaxed);
    HEALTHY_SINCE.store(u64::MAX, Ordering::Relaxed);
    LONGEST_HEALTHY_MS.store(0, Ordering::Relaxed);
    DISCONNECT_PENDING.store(false, Ordering::Relaxed);
    DISCONNECT.reset();
}
async fn deadline<T>(
    seconds: u64,
    future: impl core::future::Future<Output = Result<T, NetCtlError>>,
) -> Result<T, NetCtlError> {
    match with_timeout(Duration::from_secs(seconds), future).await {
        Ok(result) => result,
        Err(_) => {
            TIMEOUTS.fetch_add(1, Ordering::Relaxed);
            RESTART.signal(());
            Err(NetCtlError::Other(ErrorCode::TxTimeout.into()))
        }
    }
}
use core::cell::Cell;

use esp_radio::wifi::scan::{ScanConfig, ScanTypeConfig};
use esp_radio::wifi::sta::{ScanMethod, StationConfig};
use esp_radio::wifi::{AuthenticationMethod, Config, WifiController, WifiError};

use rs_matter_embassy::matter::dm::clusters::net_comm::{
    NetCtl, NetCtlError, NetworkScanInfo, NetworkType, WiFiBandEnum, WiFiSecurityBitmap,
    WirelessCreds,
};
use rs_matter_embassy::matter::dm::clusters::wifi_diag::{
    SecurityTypeEnum, WiFiVersionEnum, WifiDiag, WirelessDiag,
};
use rs_matter_embassy::matter::dm::networks::NetChangeNotif;
use rs_matter_embassy::matter::error::{Error, ErrorCode};
use rs_matter_embassy::matter::tlv::Nullable;
use rs_matter_embassy::matter::utils::sync::blocking::Mutex;
use rs_matter_embassy::matter::utils::sync::{DynBase, IfMutex};

pub struct Controller<'a>(
    IfMutex<WifiController<'a>>,
    Mutex<Cell<bool>>,
    Mutex<Cell<crate::recovery::DriverErrors>>,
    Mutex<Cell<crate::recovery::AssociatedHealth>>,
);

// The SDK's 10–20 ms active dwell missed the installed AP during C3 bench
// commissioning. Keep discovery long enough to receive delayed probe replies.
fn discovery_scan() -> ScanConfig {
    ScanConfig::default()
        // ScanNetworks encodes one response, without chunking. The SDK returns
        // APs in RSSI order, so retain the strongest results within its budget.
        .with_max(crate::network_scan::MAX_SCAN_RESULTS)
        .with_scan_type(ScanTypeConfig::Active {
            min: esp_hal::time::Duration::from_millis(100),
            max: esp_hal::time::Duration::from_millis(300),
        })
}

impl<'a> Controller<'a> {
    pub fn new(controller: WifiController<'a>) -> Self {
        Self(
            IfMutex::new(controller),
            Mutex::new(Cell::new(false)),
            Mutex::new(Cell::new(crate::recovery::DriverErrors::default())),
            Mutex::new(Cell::new(crate::recovery::AssociatedHealth::default())),
        )
    }
}

impl Controller<'_> {
    fn update_connected(&self, new_connected: bool, rssi: Option<i32>) -> bool {
        CONNECTED.store(u32::from(new_connected), Ordering::Relaxed);
        RSSI.store(rssi.unwrap_or(i32::MIN), Ordering::Relaxed);
        if !new_connected {
            LOCAL_READY.store(0, Ordering::Relaxed);
            IPV4_READY.store(0, Ordering::Relaxed);
            self.3.lock(|health| health.set(Default::default()));
        }
        if !new_connected || rssi.is_none() {
            end_healthy_period(embassy_time::Instant::now().as_millis());
        }
        self.1.lock(|connected| {
            let changed = connected.get() != new_connected;
            if changed {
                log::info!("Wifi state changed: {} -> {new_connected}", connected.get());
            }
            connected.set(new_connected);
            changed
        })
    }

    fn record_connection_result(&self, result: &Result<(), NetCtlError>) {
        if result.is_err() {
            self.update_connected(false, None);
        }
        let internal_failure = matches!(
            result,
            Err(NetCtlError::Other(error)) if error.code() == ErrorCode::NoNetworkInterface
        );
        let restart = self.2.lock(|errors| {
            let mut current = errors.get();
            let restart = current.observe(internal_failure);
            errors.set(current);
            restart
        });
        if restart {
            log::error!("Repeated internal Wifi errors; recreating the transport");
            RESTART.signal(());
        }
    }

    async fn disconnect_inner(&self) -> Result<(), NetCtlError> {
        let mut ctl = self.0.lock().await;
        self.update_connected(false, None);
        if ctl.is_connected() {
            ctl.disconnect_async().await.map_err(to_ctl_err)?;
        }
        Ok(())
    }

    async fn scan_inner<F>(&self, network: Option<&[u8]>, mut f: F) -> Result<(), NetCtlError>
    where
        F: FnMut(&NetworkScanInfo) -> Result<(), Error>,
    {
        log::info!("Wifi scan request");

        let mut ctl = self.0.lock().await;

        ctl.set_config(&Config::Station(StationConfig::default()))
            .map_err(to_ctl_err)?;
        log::info!("Wifi configuration updated for scanning");

        let mut scan_config = discovery_scan();
        if let Some(network) = network.filter(|n| !n.is_empty()) {
            scan_config = scan_config.with_ssid(core::str::from_utf8(network).unwrap_or("???"));
        }

        let aps = ctl.scan_async(&scan_config).await.map_err(to_err)?;

        log::info!("Wifi scan complete, reporting {} results", aps.len());

        for (index, ap) in aps.into_iter().enumerate() {
            f(&NetworkScanInfo::Wifi {
                ssid: ap.ssid.as_str().as_bytes(),
                bssid: &ap.bssid,
                channel: ap.channel as _,
                rssi: ap.signal_strength,
                band: WiFiBandEnum::V2G4, // ESP32-C3 is a 2.4 GHz radio.
                security: match ap.auth_method {
                    Some(AuthenticationMethod::None) => WiFiSecurityBitmap::UNENCRYPTED,
                    Some(AuthenticationMethod::Wep) => WiFiSecurityBitmap::WEP,
                    Some(AuthenticationMethod::Wpa) => WiFiSecurityBitmap::WPA_PERSONAL,
                    Some(AuthenticationMethod::Wpa2Personal) => WiFiSecurityBitmap::WPA_2_PERSONAL,
                    Some(
                        AuthenticationMethod::Wpa3Personal
                        | AuthenticationMethod::Wpa3ExtPsk
                        | AuthenticationMethod::Wpa3ExtPskMixed,
                    ) => WiFiSecurityBitmap::WPA_3_PERSONAL,
                    Some(AuthenticationMethod::WpaWpa2Personal) => {
                        WiFiSecurityBitmap::WPA_PERSONAL | WiFiSecurityBitmap::WPA_2_PERSONAL
                    }
                    Some(AuthenticationMethod::Wpa2Wpa3Personal) => {
                        WiFiSecurityBitmap::WPA_2_PERSONAL | WiFiSecurityBitmap::WPA_3_PERSONAL
                    }
                    Some(AuthenticationMethod::Wpa2Enterprise) => {
                        WiFiSecurityBitmap::WPA_2_PERSONAL
                    }
                    _ => WiFiSecurityBitmap::WPA_2_PERSONAL, // Best guess
                },
            })
            .map_err(|error| {
                log::error!(
                    "Wifi scan response failed at result {}: {error:?}",
                    index + 1
                );
                error
            })?;
        }

        log::info!("Wifi scan complete");

        Ok(())
    }

    async fn connect_inner(&self, creds: &WirelessCreds<'_>) -> Result<(), NetCtlError> {
        let WirelessCreds::Wifi { ssid, pass } = creds else {
            return Err(NetCtlError::Other(ErrorCode::InvalidAction.into()));
        };

        let mut ctl = self.0.lock().await;

        let ssid = core::str::from_utf8(ssid).unwrap_or("???");
        let pass = core::str::from_utf8(pass).unwrap_or("???");

        ATTEMPTS.fetch_add(1, Ordering::Relaxed);
        self.update_connected(false, None);

        if ctl.is_connected() {
            log::info!("Wifi already connected, disconnecting first");
            let _ = ctl.disconnect_async().await;
        }

        // Find the requested network before the driver's association scan. A
        // channel is a starting hint, not a pinned BSSID; roaming remains possible.
        // Failed discovery still permits a normal all-channel connection attempt.
        let scan = discovery_scan().with_ssid(ssid);
        let channel = match ctl.scan_async(&scan).await {
            Ok(aps) => aps
                .into_iter()
                .max_by_key(|ap| ap.signal_strength)
                .map(|ap| {
                    log::info!(
                        "Wifi target found: channel={} rssi={}",
                        ap.channel,
                        ap.signal_strength
                    );
                    ap.channel
                }),
            Err(error) => {
                log::warn!("Wifi target scan failed: {error:?}; trying connection anyway");
                None
            }
        };
        if channel.is_none() {
            log::warn!("Wifi target not seen; trying all-channel connection");
        }
        let mut station = StationConfig::default()
            .with_ssid(ssid)
            .with_password(pass.into())
            .with_scan_method(ScanMethod::AllChannels);
        if let Some(channel) = channel {
            station = station.with_channel(channel);
        }
        ctl.set_config(&Config::Station(station))
            .map_err(to_ctl_err)?;
        log::info!("Wifi configuration updated");

        ctl.connect_async().await.map_err(to_ctl_err)?;

        self.update_connected(ctl.is_connected(), ctl.rssi().ok());

        log::info!("Wifi connected");

        Ok(())
    }
}

impl NetChangeNotif for Controller<'_> {
    async fn wait_changed(&self) {
        let fetch_connected = || async {
            let ctl = self.0.lock().await;

            let new_connected = ctl.is_connected();
            let rssi = new_connected.then(|| ctl.rssi().ok()).flatten();
            let changed = self.update_connected(new_connected, rssi);
            let restart = self.3.lock(|health| {
                let mut current = health.get();
                let restart = current.restart_due(
                    embassy_time::Instant::now().as_millis(),
                    new_connected,
                    rssi.is_some(),
                );
                health.set(current);
                restart
            });
            if restart {
                log::error!("Wifi association stayed stale for 60s; recreating the transport");
                RESTART.signal(());
            }
            changed
        };

        loop {
            if DISCONNECT_PENDING.load(Ordering::Relaxed) {
                let result = deadline(30, self.disconnect_inner()).await;
                self.record_connection_result(&result);
                DISCONNECT_PENDING.store(false, Ordering::Relaxed);
                DISCONNECT.reset();
                log::warn!("Local Wifi interruption completed: {result:?}");
                return;
            }
            if fetch_connected().await {
                return;
            }

            embassy_futures::select::select(
                embassy_time::Timer::after(Duration::from_secs(2)),
                DISCONNECT.wait(),
            )
            .await;
        }
    }
}

impl DynBase for Controller<'_> {}

impl WirelessDiag for Controller<'_> {
    fn connected(&self) -> Result<bool, Error> {
        Ok(self.1.lock(|connected| connected.get()))
    }
}

impl WifiDiag for Controller<'_> {
    fn bssid(&self, f: &mut dyn FnMut(Option<&[u8]>) -> Result<(), Error>) -> Result<(), Error> {
        f(None)
    }

    fn security_type(&self) -> Result<Nullable<SecurityTypeEnum>, Error> {
        Ok(Nullable::none())
    }

    fn wi_fi_version(&self) -> Result<Nullable<WiFiVersionEnum>, Error> {
        Ok(Nullable::none())
    }

    fn channel_number(&self) -> Result<Nullable<u16>, Error> {
        Ok(Nullable::none())
    }

    fn rssi(&self) -> Result<Nullable<i8>, Error> {
        Ok(self
            .0
            .try_lock()
            .ok()
            .and_then(|ctl| ctl.rssi().ok())
            .and_then(|value| i8::try_from(value).ok())
            .into())
    }
}

fn to_ctl_err(e: WifiError) -> NetCtlError {
    log::error!("Wifi error: {:?}", e);

    match e {
        WifiError::NotConnected | WifiError::Disconnected(_) => NetCtlError::OtherConnectionFailure,
        WifiError::Unsupported => NetCtlError::UnsupportedSecurity,
        _ => NetCtlError::Other(ErrorCode::NoNetworkInterface.into()),
    }
}

fn to_err(e: WifiError) -> Error {
    log::error!("Wifi error: {:?}", e);
    ErrorCode::NoNetworkInterface.into()
}

impl NetCtl for Controller<'_> {
    fn net_type(&self) -> NetworkType {
        NetworkType::Wifi
    }
    async fn scan<F>(&self, network: Option<&[u8]>, f: F) -> Result<(), NetCtlError>
    where
        F: FnMut(&NetworkScanInfo) -> Result<(), Error>,
    {
        let result = deadline(30, self.scan_inner(network, f)).await;
        if result.is_err() {
            self.update_connected(false, None);
        }
        result
    }
    async fn connect(&self, creds: &WirelessCreds<'_>) -> Result<(), NetCtlError> {
        let result = deadline(30, self.connect_inner(creds)).await;
        self.record_connection_result(&result);
        result
    }
}

/// Observe the real stack's interface configuration, independent of controller traffic.
/// This is not an end-to-end Matter request probe and cannot detect every logical stall.
pub struct InterfaceMonitor;
impl rs_matter_embassy::stack::UserTask for InterfaceMonitor {
    async fn run<S, N>(&mut self, _stack: S, netif: N) -> Result<(), Error>
    where
        S: rs_matter_embassy::stack::nal::NetStack,
        N: rs_matter_embassy::matter::dm::clusters::gen_diag::NetifDiag + NetChangeNotif,
    {
        let mut health = crate::recovery::LocalNetHealth::default();
        loop {
            let mut ready = false;
            let mut v4 = false;
            netif.netifs(&mut |info| {
                v4 |= info.operational && !info.ipv4_addrs.is_empty();
                ready |= crate::recovery::usable_local_ipv6(info.operational, info.ipv6_addrs);
                Ok(())
            })?;
            LOCAL_READY.store(u32::from(ready), Ordering::Relaxed);
            IPV4_READY.store(u32::from(v4), Ordering::Relaxed);
            let now = embassy_time::Instant::now().as_millis();
            let associated = CONNECTED.load(Ordering::Relaxed) != 0;
            if associated && ready && RSSI.load(Ordering::Relaxed) != i32::MIN {
                let _ = HEALTHY_SINCE.compare_exchange(
                    u64::MAX,
                    now,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                );
            } else {
                end_healthy_period(now);
            }
            if health.restart_due(now, associated, ready) {
                IP_TIMEOUTS.fetch_add(1, Ordering::Relaxed);
                return Err(ErrorCode::NoNetworkInterface.into());
            }
            embassy_time::Timer::after(Duration::from_secs(2)).await;
        }
    }
}
