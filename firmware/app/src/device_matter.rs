//! Wi-Fi/BLE Matter transport adapted from Stillair and rs-matter-embassy.
//! Public development attestation and built-in stock PCA output settings.

use core::fmt::Write as _;

use crate::commissioning::{qr_payload, DiscoveryName};
use crate::commissioning_ble::NamedBle;
use crate::network_driver::EspWifiDriver;
use embassy_futures::join::join3;
use embassy_time::{Duration, Instant, Timer};
use esp_bootloader_esp_idf::partitions::PARTITION_TABLE_MAX_LEN;
use esp_hal::peripherals::{ADC1, BT, FLASH, RNG, WIFI};
use esp_hal::rng::{Trng, TrngSource};
use heapless::String;
use rs_matter_embassy::matter::crypto::{default_crypto, Crypto};
use rs_matter_embassy::matter::dm::clusters::app::{color_control, level_control, on_off};
use rs_matter_embassy::matter::dm::clusters::basic_info::BasicInfoConfig;
use rs_matter_embassy::matter::dm::clusters::desc::{self, ClusterHandler as _};
use rs_matter_embassy::matter::dm::clusters::groups::{self, GroupsHandler};
use rs_matter_embassy::matter::dm::clusters::identify;
use rs_matter_embassy::matter::dm::clusters::scenes::{
    self, SceneInvalidator, ScenesHandler, ScenesState,
};
use rs_matter_embassy::matter::dm::devices::test::{DAC_PRIVKEY, TEST_DEV_ATT, TEST_PID, TEST_VID};
use rs_matter_embassy::matter::dm::{
    Async, Dataver, DeviceType, EmptyHandler, Endpoint, EpClMatcher, Node,
};
use rs_matter_embassy::matter::error::{Error, ErrorCode};
use rs_matter_embassy::matter::utils::init::InitMaybeUninit;
use rs_matter_embassy::matter::{clusters, devices, BasicCommData};
use rs_matter_embassy::stack::rand::reseeding_csprng;
use rs_matter_embassy::wireless::{EmbassyWifi, EmbassyWifiMatterStack};
use static_cell::StaticCell;

use crate::presets::{self, AtomicScenes, LightHandler, SceneLevel, ScenePower, SceneTemperature};
use crate::runtime::Runtime;
use crate::storage::{commissioning_passcode, persistent_store};
use rs_matter_embassy::matter::persist::KV_BUF_SIZE;

// Large objects are initialized in place so no temporary overflows the task stack.
macro_rules! mk_static {
    ($t:ty) => {{
        static CELL: StaticCell<$t> = StaticCell::new();
        CELL.uninit()
    }};
}

// The complete light handler plus concurrent BLE/Wi-Fi transport exceeds the
// SDK example's 20,000-byte arena on C3 (first bench boot panicked there).
// Keep room for transport futures; the MCU gate separately checks stack space.
const BUMP_SIZE: usize = 32 * 1024;
const LIGHT_ENDPOINT: u16 = 1;
const COLOR_TEMPERATURE_LIGHT: DeviceType = DeviceType {
    dtype: 0x010c,
    drev: 4,
};

const NODE: Node = Node {
    endpoints: &[
        EmbassyWifiMatterStack::<0, ()>::root_endpoint(),
        Endpoint::new(
            LIGHT_ENDPOINT,
            devices!(COLOR_TEMPERATURE_LIGHT),
            clusters!(
                desc::DescHandler::CLUSTER,
                identify::CLUSTER,
                groups::FULL_CLUSTER.with_features(groups::Feature::GROUP_NAMES.bits()),
                scenes::FULL_CLUSTER,
                presets::ON_OFF_CLUSTER,
                presets::LEVEL_CLUSTER,
                presets::COLOR_CLUSTER
            ),
        ),
    ],
};

pub async fn run<H: crate::runtime::Hardware>(
    entropy: (RNG<'static>, ADC1<'static>),
    wifi: WIFI<'static>,
    bt: BT<'static>,
    flash: FLASH<'static>,
    hardware: H,
    rx: esp_hal::usb::usb_serial_jtag::UsbSerialJtagRx<'static, esp_hal::Async>,
    mut watchdog: esp_hal::timer::timg::Wdt<esp_hal::peripherals::TIMG0<'static>>,
) -> ! {
    let _entropy = TrngSource::new(entropy.0, entropy.1);
    let trng = Trng::try_new().expect("TRNG source is initialized");
    let mut partition_table = [0; PARTITION_TABLE_MAX_LEN];
    let mut store = match persistent_store(flash, &mut partition_table, || watchdog.feed()).await {
        Ok(store) => store,
        Err(error) => {
            halted(
                "NVS partition unavailable; flash with partitions.csv",
                error,
                &mut watchdog,
            )
            .await
        }
    };
    let passcode = match crate::boot::restore(
        "Commissioning credential storage failed",
        async || commissioning_passcode(&mut store, &trng),
        || watchdog.feed(),
    )
    .await
    {
        Ok(passcode) => passcode,
        Err(error) => {
            halted(
                "commissioning credential storage failed",
                error,
                &mut watchdog,
            )
            .await
        }
    };

    let mac = esp_hal::efuse::base_mac_address();
    let discovery_name = mk_static!(DiscoveryName).write(DiscoveryName::new(
        mac.as_bytes().try_into().expect("MAC address is six bytes"),
    ));
    let mut identity = String::<32>::new();
    write!(identity, "KR-{mac}").expect("MAC identity fits");
    let identity = mk_static!(String<32>).write(identity).as_str();
    let device = mk_static!(BasicInfoConfig<'static>).write(BasicInfoConfig {
        vendor_name: "Key Right",
        product_name: "Key Right",
        device_name: discovery_name.as_str(),
        serial_no: identity,
        unique_id: identity,
        vid: TEST_VID,
        pid: TEST_PID,
        hw_ver: 1,
        hw_ver_str: "ESP32-C3_MINI_V1",
        sw_ver: 2,
        sw_ver_str: env!("CARGO_PKG_VERSION"),
        device_type: Some(COLOR_TEMPERATURE_LIGHT.dtype),
        ..BasicInfoConfig::new()
    });
    let mac_bytes = mac.as_bytes();
    let commissioning = BasicCommData {
        password: passcode.to_le_bytes().into(),
        discriminator: u16::from_be_bytes([mac_bytes[4], mac_bytes[5]]) & 0x0fff,
    };
    let pairing_code = commissioning.compute_pairing_code();
    let pairing_qr = match qr_payload(
        commissioning.clone(),
        device,
        mk_static!([u8; 128]).write([0; 128]),
    ) {
        Ok(payload) => payload,
        Err(error) => halted("commissioning QR encoding failed", error, &mut watchdog).await,
    };
    let stack = mk_static!(EmbassyWifiMatterStack<BUMP_SIZE, ()>).init_with(
        EmbassyWifiMatterStack::init(device, commissioning, &TEST_DEV_ATT),
    );
    let crypto = default_crypto(
        reseeding_csprng(trng, 1000).expect("CSPRNG seeding from hardware entropy"),
        DAC_PRIVKEY,
    );
    let mut weak_rand = crypto.weak_rand().expect("weak RNG from crypto provider");

    // Retry before publishing the stack. Pinned loaders reset fabrics, basic
    // settings, time and networks before each reload; partial loads cannot
    // duplicate fabrics. No application erase is requested; the pinned storage
    // backend may repair pages while reading them.
    if let Err(error) = crate::boot::restore(
        "Matter state read failed",
        async || stack.load(&mut store).await,
        || watchdog.feed(),
    )
    .await
    {
        halted(
            "Matter state could not be restored; no application erase requested",
            error,
            &mut watchdog,
        )
        .await;
    }
    let scenes = mk_static!(ScenesState<16>).init_with(ScenesState::init());
    let mut scene_buf = [0; KV_BUF_SIZE];
    // Scenes decode into a temporary table, then replace the in-memory state.
    if let Err(error) = crate::boot::restore(
        "Scenes read failed",
        async || scenes.load_persist(&mut store, &mut scene_buf).await,
        || watchdog.feed(),
    )
    .await
    {
        halted(
            "Scenes could not be restored; no application erase requested",
            error,
            &mut watchdog,
        )
        .await;
    }
    // Startup policy and a reset during recall can differ from the saved scene.
    // Retain the table, but never restore a claim that its output is current.
    scenes.scenable_attribute_changed(LIGHT_ENDPOINT);
    let kv = stack.matter().kv(store);
    let runtime = Runtime::load(&kv, hardware);
    let light = LightHandler::new(
        &runtime,
        scenes,
        [
            Dataver::new_rand(&mut weak_rand),
            Dataver::new_rand(&mut weak_rand),
            Dataver::new_rand(&mut weak_rand),
        ],
    );
    let scene_handler = ScenesHandler::new(
        Dataver::new_rand(&mut weak_rand),
        scenes,
        (
            ScenePower(&light),
            (SceneLevel(&light), (SceneTemperature(&light), ())),
        ),
    );
    let handler = EmptyHandler
        .chain(
            EpClMatcher::new(Some(LIGHT_ENDPOINT), Some(presets::ON_OFF_CLUSTER.id)),
            on_off::HandlerAsyncAdaptor(&light),
        )
        .chain(
            EpClMatcher::new(Some(LIGHT_ENDPOINT), Some(presets::LEVEL_CLUSTER.id)),
            level_control::HandlerAsyncAdaptor(&light),
        )
        .chain(
            EpClMatcher::new(Some(LIGHT_ENDPOINT), Some(presets::COLOR_CLUSTER.id)),
            color_control::HandlerAsyncAdaptor(&light),
        )
        .chain(
            EpClMatcher::new(Some(LIGHT_ENDPOINT), Some(identify::CLUSTER.id)),
            Async(identify::IdentifyHandler::new(Dataver::new_rand(
                &mut weak_rand,
            ))),
        )
        .chain(
            EpClMatcher::new(Some(LIGHT_ENDPOINT), Some(groups::FULL_CLUSTER.id)),
            Async(GroupsHandler::new(Dataver::new_rand(&mut weak_rand)).adapt()),
        )
        .chain(
            EpClMatcher::new(Some(LIGHT_ENDPOINT), Some(scenes::FULL_CLUSTER.id)),
            AtomicScenes {
                inner: scene_handler.adapt(),
                light: &light,
            },
        )
        .chain(
            EpClMatcher::new(Some(LIGHT_ENDPOINT), Some(desc::DescHandler::CLUSTER.id)),
            Async(desc::DescHandler::new(Dataver::new_rand(&mut weak_rand)).adapt()),
        );
    let mut driver = EspWifiDriver::new(wifi, bt);

    log::warn!("identity={identity}; color-temperature light; public development attestation");
    let open_commissioning = || {
        if stack.is_commissioned() {
            return Err(Error::from(ErrorCode::InvalidState));
        }
        stack.matter().close_comm_window(&ProvisioningNotify)?;
        stack
            .matter()
            .open_basic_comm_window(900, &crypto, &ProvisioningNotify)
    };
    let transport = async {
        let mut backoff = crate::recovery::RestartBackoff::default();
        let mut queue_recovery = crate::network_tx::QueueRecovery::default();
        loop {
            if !stack.is_commissioned() {
                if let Err(e) = open_commissioning() {
                    log::error!("commissioning window: {e:?}");
                }
            }
            crate::network::RESTART.reset();
            crate::network_driver::TX_STALL.reset();
            let result = embassy_futures::select::select3(
                stack.run_coex(
                    NamedBle::new(
                        EmbassyWifi::new(&mut driver, weak_rand, true, stack),
                        discovery_name.as_str(),
                    ),
                    &crypto,
                    (NODE, &handler),
                    &kv,
                    crate::network::InterfaceMonitor,
                ),
                crate::network::RESTART.wait(),
                crate::network_driver::TX_STALL.wait(),
            )
            .await;
            let healthy_ms = crate::network::healthy_for_ms(Instant::now().as_millis());
            queue_recovery.record_health(healthy_ms);
            if matches!(result, embassy_futures::select::Either3::Third(()))
                && queue_recovery.reboot_required()
            {
                let off_verified = runtime.prepare_reboot();
                log::error!(
                    "Transmit queue remained stuck after radio recreation; rebooting with saved state/fabrics preserved; off_registers_verified={off_verified}"
                );
                Timer::after(Duration::from_millis(250)).await;
                esp_hal::system::software_reset();
            }
            let retry_secs = backoff.next_delay_secs(healthy_ms);
            crate::network::restarted();
            log::warn!("Matter transport exited: {result:?}; retry in {retry_secs}s; intent/fabrics preserved");
            Timer::after(Duration::from_secs(retry_secs)).await;
        }
    };
    join3(
        transport,
        crate::console::run(
            rx,
            &runtime,
            &pairing_code,
            pairing_qr,
            open_commissioning,
            || stack.is_commissioned(),
        ),
        presets::maintenance(&runtime, &light, || watchdog.feed()),
    )
    .await;
    unreachable!()
}

// Used only before any fabric exists; there are no commissioned subscribers to notify.
struct ProvisioningNotify;
impl rs_matter_embassy::matter::dm::AttrChangeNotifier for ProvisioningNotify {
    fn notify_attr_changed(&self, _endpoint: u16, _cluster: u32, _attribute: u32) {}
    fn notify_cluster_changed(&self, _endpoint: u16, _cluster: u32) {}
    fn notify_endpoint_changed(&self, _endpoint: u16) {}
    fn notify_all_changed(&self) {}
}

/// Report permanent setup/decoded-data faults without erasing pairing or inventing state.
/// Storage I/O failures retry before reaching this path.
async fn halted(
    reason: &str,
    error: Error,
    watchdog: &mut esp_hal::timer::timg::Wdt<esp_hal::peripherals::TIMG0<'static>>,
) -> ! {
    loop {
        log::error!("Matter unavailable: {reason}: {error:?}; repair then reboot");
        watchdog.feed();
        Timer::after(Duration::from_secs(5)).await;
    }
}
