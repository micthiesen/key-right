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
use rs_matter_embassy::matter::error::Error;
const SLOTS: usize = 20;
use rs_matter_embassy::wireless::{
    BleDriver, BleDriverTask, WifiCoexDriver, WifiCoexDriverTask, WifiDriver, WifiDriverTask,
};

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

        task.run(
            esp_radio::wifi::Interface::station(),
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

        task.run(
            esp_radio::wifi::Interface::station(),
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
