//! Exercise the production control handlers against an in-memory adapter.
extern crate self as rs_matter_embassy;
pub use rs_matter as matter;

#[path = "../../src/boot.rs"]
pub mod boot;
#[path = "../../src/commissioning.rs"]
pub mod commissioning;
#[path = "../../src/light.rs"]
pub mod light;
#[path = "../../src/network_scan.rs"]
pub mod network_scan;
#[path = "../../src/network_tx.rs"]
pub mod network_tx;
#[path = "../../src/presets.rs"]
pub mod presets;
#[path = "../../src/recovery.rs"]
pub mod recovery;
#[path = "../../src/runtime.rs"]
pub mod runtime;

#[cfg(test)]
mod animation_tests;
#[cfg(test)]
mod boot_tests;
#[cfg(test)]
mod commissioning_ble_tests;
#[cfg(test)]
mod commissioning_tests;
#[cfg(test)]
mod control_tests;
#[cfg(test)]
mod matter_capacity_tests;
#[cfg(test)]
mod network_scan_tests;
#[cfg(test)]
mod network_tx_tests;
#[cfg(test)]
mod recovery_tests;
#[cfg(test)]
mod runtime_tests;
