//! Exercise the production control handlers against an in-memory adapter.
extern crate self as rs_matter_embassy;
pub use rs_matter as matter;

#[path = "../../src/light.rs"]
pub mod light;
#[path = "../../src/presets.rs"]
pub mod presets;
#[path = "../../src/recovery.rs"]
pub mod recovery;
#[path = "../../src/runtime.rs"]
pub mod runtime;

#[cfg(test)]
mod control_tests;
#[cfg(test)]
mod runtime_tests;
