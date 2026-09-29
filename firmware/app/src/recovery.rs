//! Only local evidence drives recovery. An absent AP/controller/internet is idle.
/// Matter needs local IPv6. IPv4-only configuration cannot satisfy readiness;
/// static IPv6 configuration also does not establish readiness while link down.
pub fn usable_local_ipv6(operational: bool, addresses: &[core::net::Ipv6Addr]) -> bool {
    operational
        && addresses.iter().any(|address| {
            !address.is_unspecified() && !address.is_multicast() && !address.is_loopback()
        })
}

#[derive(Clone, Copy, Default)]
pub struct LocalNetHealth {
    missing_since: Option<u64>,
}
impl LocalNetHealth {
    pub fn restart_due(&mut self, now_ms: u64, associated: bool, local_ready: bool) -> bool {
        if !associated || local_ready {
            self.missing_since = None;
            return false;
        }
        let since = self.missing_since.get_or_insert(now_ms);
        now_ms.saturating_sub(*since) >= 60_000
    }
}

/// Consecutive internal driver failures need a fresh controller. Expected
/// association failures, including an absent AP, do not count as driver faults.
#[derive(Clone, Copy, Default)]
pub struct DriverErrors {
    consecutive: u8,
}
impl DriverErrors {
    pub fn observe(&mut self, internal_failure: bool) -> bool {
        self.consecutive = if internal_failure {
            self.consecutive.saturating_add(1)
        } else {
            0
        };
        self.consecutive >= 3
    }
}

/// Association is event-derived in the SDK. Repeated failure of the independent
/// radio query provides local evidence that the cached association is stale.
#[derive(Clone, Copy, Default)]
pub struct AssociatedHealth {
    failed_since: Option<u64>,
}
impl AssociatedHealth {
    pub fn restart_due(&mut self, now_ms: u64, associated: bool, radio_ok: bool) -> bool {
        if !associated || radio_ok {
            self.failed_since = None;
            return false;
        }
        let since = self.failed_since.get_or_insert(now_ms);
        now_ms.saturating_sub(*since) >= 60_000
    }
}

/// Bound repeated transport failures without treating time spent offline as a
/// successful run. The caller supplies continuously healthy operational time.
#[derive(Default)]
pub struct RestartBackoff {
    failures: u8,
}
impl RestartBackoff {
    pub fn next_delay_secs(&mut self, healthy_for_ms: u64) -> u64 {
        if healthy_for_ms >= 120_000 {
            self.failures = 0;
        }
        let seconds = (5_u64 << self.failures.min(4)).min(60);
        self.failures = self.failures.saturating_add(1);
        seconds
    }
}
