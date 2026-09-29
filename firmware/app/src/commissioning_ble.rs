//! Supply the discovery name where the pinned stack otherwise advertises "BT".

use rs_matter_embassy::matter::dm::clusters::gen_diag::NetifDiag;
use rs_matter_embassy::matter::dm::clusters::net_comm::NetCtl;
use rs_matter_embassy::matter::dm::clusters::wifi_diag::WifiDiag;
use rs_matter_embassy::matter::dm::networks::NetChangeNotif;
use rs_matter_embassy::matter::error::Error;
use rs_matter_embassy::matter::transport::network::btp::{AdvData, Btp};
use rs_matter_embassy::stack::ble::GattPeripheral;
use rs_matter_embassy::stack::mdns::Mdns;
use rs_matter_embassy::stack::nal::NetStack;
use rs_matter_embassy::stack::wireless::{WifiCoex, WifiCoexTask};

/// Leaves Wi-Fi, mDNS, and controller lifecycle with the existing Embassy adapter.
pub struct NamedBle<'a, W> {
    inner: W,
    name: &'a str,
}

impl<'a, W> NamedBle<'a, W> {
    pub fn new(inner: W, name: &'a str) -> Self {
        Self { inner, name }
    }
}

impl<W: WifiCoex> WifiCoex for NamedBle<'_, W> {
    async fn run<T: WifiCoexTask>(&mut self, task: T) -> Result<(), Error> {
        self.inner
            .run(NamedTask {
                inner: task,
                name: self.name,
            })
            .await
    }
}

struct NamedTask<'a, T> {
    inner: T,
    name: &'a str,
}

impl<T: WifiCoexTask> WifiCoexTask for NamedTask<'_, T> {
    async fn run<S, N, C, M, G>(
        &mut self,
        net_stack: S,
        netif: N,
        net_ctl: C,
        mdns: M,
        gatt: G,
    ) -> Result<(), Error>
    where
        S: NetStack,
        N: NetifDiag + NetChangeNotif,
        C: NetCtl + WifiDiag + NetChangeNotif,
        M: Mdns,
        G: GattPeripheral,
    {
        self.inner
            .run(
                net_stack,
                netif,
                net_ctl,
                mdns,
                NamedPeripheral {
                    inner: gatt,
                    name: self.name,
                },
            )
            .await
    }
}

struct NamedPeripheral<'a, G> {
    inner: G,
    name: &'a str,
}

impl<G: GattPeripheral> GattPeripheral for NamedPeripheral<'_, G> {
    async fn run(&mut self, btp: &Btp, _: &str, adv: &AdvData) -> Result<(), Error> {
        self.inner.run(btp, self.name, adv).await
    }
}
