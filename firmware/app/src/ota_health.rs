//! When an image started from a new OTA slot may confirm itself.

/// Bound for an updated image's self-test. A broken build rolls back within
/// five minutes; a sound one has time for several 5–60 s transport retries.
pub const OTA_CONFIRM_DEADLINE_MS: u64 = 300_000;
/// Current uninterrupted associated Wi-Fi plus usable local IPv6 required.
pub const OTA_HEALTHY_MS: u64 = 30_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OtaVerdict {
    Wait,
    Confirm,
    /// Reset unconfirmed so the bootloader returns to the previous image.
    RollBack,
}

/// Decide whether a newly updated image is healthy. Output readback runs last,
/// and only once the Matter and network evidence already passes.
pub fn ota_verdict(
    uptime_ms: u64,
    commissioned: bool,
    network_healthy_ms: u64,
    output_verified: impl FnOnce() -> bool,
) -> OtaVerdict {
    if commissioned && network_healthy_ms >= OTA_HEALTHY_MS && output_verified() {
        OtaVerdict::Confirm
    } else if uptime_ms >= OTA_CONFIRM_DEADLINE_MS {
        OtaVerdict::RollBack
    } else {
        OtaVerdict::Wait
    }
}
