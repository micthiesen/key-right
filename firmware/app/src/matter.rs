//! Wi-Fi/BLE Matter transport adapted from Stillair and rs-matter-embassy.
//! Development attestation is public test material; this is a bench image.

use core::fmt::Write as _;

use embassy_time::{Duration, Timer};
use esp_bootloader_esp_idf::partitions::PARTITION_TABLE_MAX_LEN;
use esp_hal::peripherals::{ADC1, BT, FLASH, RNG, WIFI};
use esp_hal::rng::{Trng, TrngSource};
use heapless::String;
use rs_matter_embassy::matter::crypto::{default_crypto, Crypto};
use rs_matter_embassy::matter::dm::clusters::app::on_off;
use rs_matter_embassy::matter::dm::clusters::basic_info::BasicInfoConfig;
use rs_matter_embassy::matter::dm::clusters::desc::{self, ClusterHandler as _};
use rs_matter_embassy::matter::dm::clusters::identify;
use rs_matter_embassy::matter::dm::devices::test::{DAC_PRIVKEY, TEST_DEV_ATT, TEST_PID, TEST_VID};
use rs_matter_embassy::matter::dm::devices::DEV_TYPE_ON_OFF_LIGHT;
use rs_matter_embassy::matter::dm::{Async, Dataver, EmptyHandler, Endpoint, EpClMatcher, Node};
use rs_matter_embassy::matter::error::Error;
use rs_matter_embassy::matter::utils::init::InitMaybeUninit;
use rs_matter_embassy::matter::{clusters, devices, BasicCommData};
use rs_matter_embassy::stack::rand::reseeding_csprng;
use rs_matter_embassy::wireless::esp::EspWifiDriver;
use rs_matter_embassy::wireless::{EmbassyWifi, EmbassyWifiMatterStack};
use static_cell::StaticCell;

use crate::light::{self, BenchLight};
use crate::storage::{commissioning_passcode, persistent_store};

// Large objects are initialized in place so no temporary overflows the task stack.
macro_rules! mk_static {
    ($t:ty) => {{
        static CELL: StaticCell<$t> = StaticCell::new();
        CELL.uninit()
    }};
}

const BUMP_SIZE: usize = 20_000;
const LIGHT_ENDPOINT: u16 = 1;
const RETRY_DELAY: Duration = Duration::from_secs(5);

const NODE: Node = Node {
    endpoints: &[
        EmbassyWifiMatterStack::<0, ()>::root_endpoint(),
        Endpoint::new(
            LIGHT_ENDPOINT,
            devices!(DEV_TYPE_ON_OFF_LIGHT),
            clusters!(
                desc::DescHandler::CLUSTER,
                identify::CLUSTER,
                light::CLUSTER
            ),
        ),
    ],
};

pub async fn run(
    rng: RNG<'static>,
    adc1: ADC1<'static>,
    wifi: WIFI<'static>,
    bt: BT<'static>,
    flash: FLASH<'static>,
) -> ! {
    let _entropy = TrngSource::new(rng, adc1);
    let trng = Trng::try_new().expect("TRNG source is initialized");
    let mut partition_table = [0; PARTITION_TABLE_MAX_LEN];
    let mut store = match persistent_store(flash, &mut partition_table) {
        Ok(store) => store,
        Err(error) => {
            halted(
                "NVS partition unavailable; flash with partitions.csv",
                error,
            )
            .await
        }
    };
    let passcode = match commissioning_passcode(&mut store, &trng) {
        Ok(passcode) => passcode,
        Err(error) => halted("commissioning credential storage failed", error).await,
    };

    let mac = esp_hal::efuse::base_mac_address();
    let mut identity = String::<32>::new();
    write!(identity, "KR-{mac}").expect("MAC identity fits");
    let identity = mk_static!(String<32>).write(identity).as_str();
    let device = mk_static!(BasicInfoConfig<'static>).write(BasicInfoConfig {
        vendor_name: "Key Right",
        product_name: "Key Right Bench",
        device_name: "Key Right Bench",
        serial_no: identity,
        unique_id: identity,
        vid: TEST_VID,
        pid: TEST_PID,
        hw_ver: 1,
        hw_ver_str: "ESP32-C6 bench",
        sw_ver: 1,
        sw_ver_str: env!("CARGO_PKG_VERSION"),
        device_type: Some(DEV_TYPE_ON_OFF_LIGHT.dtype),
        ..BasicInfoConfig::new()
    });
    let mac_bytes = mac.as_bytes();
    let commissioning = BasicCommData {
        password: passcode.to_le_bytes().into(),
        discriminator: u16::from_be_bytes([mac_bytes[4], mac_bytes[5]]) & 0x0fff,
    };
    let stack = mk_static!(EmbassyWifiMatterStack<BUMP_SIZE, ()>).init_with(
        EmbassyWifiMatterStack::init(device, commissioning, &TEST_DEV_ATT),
    );
    let crypto = default_crypto(
        reseeding_csprng(trng, 1000).expect("CSPRNG seeding from hardware entropy"),
        DAC_PRIVKEY,
    );
    let mut weak_rand = crypto.weak_rand().expect("weak RNG from crypto provider");

    if let Err(error) = stack.startup(&crypto, &mut store).await {
        halted(
            "Matter state could not be restored; NVS was preserved",
            error,
        )
        .await;
    }
    let kv = stack.matter().kv(store);
    let light = match BenchLight::load(&kv) {
        Ok(light) => light,
        Err(error) => {
            halted(
                "light state could not be restored; NVS was preserved",
                error,
            )
            .await
        }
    };
    let on_off = on_off::OnOffHandler::new_standalone(
        Dataver::new_rand(&mut weak_rand),
        LIGHT_ENDPOINT,
        light,
    );
    let handler = EmptyHandler
        .chain(
            EpClMatcher::new(Some(LIGHT_ENDPOINT), Some(light::CLUSTER.id)),
            on_off::HandlerAsyncAdaptor(&on_off),
        )
        .chain(
            EpClMatcher::new(Some(LIGHT_ENDPOINT), Some(identify::CLUSTER.id)),
            // No physical indicator in the bench image; IdentifyType is None.
            Async(identify::IdentifyHandler::new(Dataver::new_rand(
                &mut weak_rand,
            ))),
        )
        .chain(
            EpClMatcher::new(Some(LIGHT_ENDPOINT), Some(desc::DescHandler::CLUSTER.id)),
            Async(desc::DescHandler::new(Dataver::new_rand(&mut weak_rand)).adapt()),
        );
    let mut driver = EspWifiDriver::new(wifi, bt);

    log::warn!("identity={identity}; public development attestation; simulated output only");
    loop {
        // Reuse the stack, handler, intent, and NVS across transport restarts.
        // Actual AP loss and Apple Home recovery still require physical acceptance tests.
        log::info!("Matter starting; use the commissioning code printed by the stack");
        let result = stack
            .run_coex(
                EmbassyWifi::new(&mut driver, weak_rand, true, stack),
                &crypto,
                (NODE, &handler),
                &kv,
                (),
            )
            .await;
        log::error!("Matter stack exited: {result:?}; retry in 5 s, preserving intent");
        Timer::after(RETRY_DELAY).await;
    }
}

/// Report setup/storage faults continuously without erasing pairing or inventing state.
async fn halted(reason: &str, error: Error) -> ! {
    loop {
        log::error!("Matter unavailable: {reason}: {error:?}; repair then reboot");
        Timer::after(Duration::from_secs(30)).await;
    }
}
