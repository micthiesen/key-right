//! Adapted from pinned rs-matter-embassy wifi/esp.rs, adding operation deadlines.
use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{with_timeout, Duration};
pub static RESTART: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static TIMEOUTS: AtomicU32 = AtomicU32::new(0);
static ATTEMPTS: AtomicU32 = AtomicU32::new(0);
static CONNECTED: AtomicU32 = AtomicU32::new(0);
static RESTARTS: AtomicU32 = AtomicU32::new(0);
static RSSI: AtomicI32 = AtomicI32::new(i32::MIN);
static LOCAL_READY: AtomicU32 = AtomicU32::new(0);
static IPV4_READY: AtomicU32 = AtomicU32::new(0);
static IP_TIMEOUTS: AtomicU32 = AtomicU32::new(0);
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
    RESTARTS.fetch_add(1, Ordering::Relaxed);
    CONNECTED.store(0, Ordering::Relaxed);
    RSSI.store(i32::MIN, Ordering::Relaxed);
    LOCAL_READY.store(0, Ordering::Relaxed);
    IPV4_READY.store(0, Ordering::Relaxed);
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

use esp_radio::wifi::scan::ScanConfig;
use esp_radio::wifi::sta::StationConfig;
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

pub struct Controller<'a>(IfMutex<WifiController<'a>>, Mutex<Cell<bool>>);

impl<'a> Controller<'a> {
    pub const fn new(controller: WifiController<'a>) -> Self {
        Self(IfMutex::new(controller), Mutex::new(Cell::new(false)))
    }
}

impl Controller<'_> {
    async fn scan_inner<F>(&self, network: Option<&[u8]>, mut f: F) -> Result<(), NetCtlError>
    where
        F: FnMut(&NetworkScanInfo) -> Result<(), Error>,
    {
        log::info!("Wifi scan request");

        let mut ctl = self.0.lock().await;

        ctl.set_config(&Config::Station(StationConfig::default()))
            .map_err(to_ctl_err)?;
        log::info!("Wifi configuration updated for scanning");

        let mut scan_config = ScanConfig::default();
        if let Some(network) = network.filter(|n| !n.is_empty()) {
            scan_config = scan_config.with_ssid(core::str::from_utf8(network).unwrap_or("???"));
        }

        let aps = ctl.scan_async(&scan_config).await.map_err(to_err)?;

        log::info!("Wifi scan complete, reporting {} results", aps.len());

        for ap in aps {
            f(&NetworkScanInfo::Wifi {
                ssid: ap.ssid.as_str().as_bytes(),
                bssid: &ap.bssid,
                channel: ap.channel as _,
                rssi: ap.signal_strength,
                band: WiFiBandEnum::V2G4, // ESP32-C6 is a 2.4 GHz radio.
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

        if ctl.is_connected() {
            log::info!("Wifi already connected, disconnecting first");
            let _ = ctl.disconnect_async().await;

            self.1.lock(|connected| {
                connected.set(false);
            });
        }

        ctl.set_config(&Config::Station(
            StationConfig::default()
                .with_ssid(ssid)
                .with_password(pass.into()),
        ))
        .map_err(to_ctl_err)?;
        log::info!("Wifi configuration updated");

        ctl.connect_async().await.map_err(to_ctl_err)?;

        self.1.lock(|connected| {
            log::info!("Wifi state updated: {} -> {}", connected.get(), true);
            CONNECTED.store(1, Ordering::Relaxed);
            connected.set(true);
        });

        log::info!("Wifi connected");

        Ok(())
    }
}

impl NetChangeNotif for Controller<'_> {
    async fn wait_changed(&self) {
        let fetch_connected = || async {
            let ctl = self.0.lock().await;

            let new_connected = ctl.is_connected();
            RSSI.store(
                if new_connected {
                    ctl.rssi().unwrap_or(i32::MIN)
                } else {
                    i32::MIN
                },
                Ordering::Relaxed,
            );
            self.1.lock(|connected| {
                if connected.get() != new_connected {
                    log::warn!(
                        "Wifi state changed: {} -> {}",
                        connected.get(),
                        new_connected
                    );

                    CONNECTED.store(u32::from(new_connected), Ordering::Relaxed);
                    connected.set(new_connected);
                    true
                } else {
                    false
                }
            })
        };

        loop {
            if fetch_connected().await {
                return;
            }

            embassy_time::Timer::after(embassy_time::Duration::from_secs(2)).await;
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
        WifiError::NotConnected => NetCtlError::OtherConnectionFailure,
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
        deadline(30, self.scan_inner(network, f)).await
    }
    async fn connect(&self, creds: &WirelessCreds<'_>) -> Result<(), NetCtlError> {
        deadline(30, self.connect_inner(creds)).await
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
                ready |= info.operational
                    && (!info.ipv4_addrs.is_empty() || !info.ipv6_addrs.is_empty());
                Ok(())
            })?;
            LOCAL_READY.store(u32::from(ready), Ordering::Relaxed);
            IPV4_READY.store(u32::from(v4), Ordering::Relaxed);
            if health.restart_due(
                embassy_time::Instant::now().as_millis(),
                CONNECTED.load(Ordering::Relaxed) != 0,
                ready,
            ) {
                IP_TIMEOUTS.fetch_add(1, Ordering::Relaxed);
                return Err(ErrorCode::NoNetworkInterface.into());
            }
            embassy_time::Timer::after(Duration::from_secs(2)).await;
        }
    }
}
