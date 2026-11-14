//! Detect a wedged Matter transport, and decide between an in-place restart
//! and a reset.
//!
//! The pinned stack (smoltcp 0.13.1, rs-matter 0.2.0, rs-matter-embassy
//! f31233a) can stop Matter while the radio, IPv6 and Wi-Fi driver queue all
//! stay healthy. smoltcp leaves a UDP packet whose next hop never answers
//! ARP/NDP at the head of the socket queue, retrying discovery every second
//! with no deadline. A subscribed controller that leaves the network or
//! changes its link-local address after its neighbor entry expires is enough.
//! Once the Matter socket's four queue slots fill, rs-matter's send waits
//! forever while holding the single transport TX buffer, and the built-in mDNS
//! responder waits for that same buffer. Home shows No Response until power is
//! cut, and none of the radio, IPv6 or transmit-queue monitors fire. Stillair,
//! on the same pinned stack, wedged this way on 2026-11-13.
//!
//! Only the TX buffer is watched: it is the one the wedge holds, while the RX
//! buffer is legitimately held for as long as a handler (an OTA chunk's flash
//! write, for example) works on a request.
//!
//! An in-place restart recreates the network stack and its sockets and frees
//! the buffer. Sessions and subscriptions survive it, so a second wedge soon
//! after means the stuck peer is still being sent to; only a reset clears them.

/// How often the transport buffers are sampled.
pub const SAMPLE_MS: u64 = 1_000;

/// How long the TX buffer may stay unavailable at every sample while
/// associated. A healthy send or receive holds one for milliseconds, and MRP
/// gives up on a peer well within a minute.
pub const STUCK_LIMIT_MS: u64 = 90_000;

/// This much continuous local network health before a wedge means the
/// previous restart worked; the wedge is a new incident.
pub const PRODUCTIVE_RUN_MS: u64 = 900_000;

/// How long a reset may wait for the accepted target to be saved before
/// falling back to another in-place restart.
pub const REBOOT_READY_WAIT_MS: u64 = 60_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sample {
    /// The transport TX buffer could be taken without waiting.
    pub tx_free: bool,
    pub associated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Wedge;

impl Wedge {
    pub const fn as_str(self) -> &'static str {
        "transport TX buffer held for 90 s"
    }
}

/// Tracks one transport run. Create a new one each time the stack starts.
#[derive(Clone, Copy, Debug)]
pub struct Watch {
    tx_free_at: u64,
}

impl Watch {
    pub const fn new(now_ms: u64) -> Self {
        Self { tx_free_at: now_ms }
    }

    pub fn observe(&mut self, now_ms: u64, sample: Sample) -> Option<Wedge> {
        // Off Wi-Fi every queued send waits on discovery for a reason a
        // restart cannot fix; the association and IPv6 monitors own that case.
        if sample.tx_free || !sample.associated {
            self.tx_free_at = now_ms;
        }
        (now_ms.saturating_sub(self.tx_free_at) >= STUCK_LIMIT_MS).then_some(Wedge)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Recovery {
    Restart,
    /// Reset once intent is durable; restart in place if it never becomes so.
    Reboot,
}

/// Restarts in place after a first wedge, and resets after a second one that
/// follows without a productive healthy period in between.
#[derive(Default)]
pub struct WedgeRecovery {
    restarted: bool,
}

impl WedgeRecovery {
    /// Call after every transport run, whatever ended it, with the run's
    /// current uninterrupted local network health. Wall time would let a long
    /// Wi-Fi outage after a restart pass for a productive run.
    pub fn after_run(&mut self, healthy_ms: u64, wedged: bool) -> Recovery {
        if healthy_ms >= PRODUCTIVE_RUN_MS {
            self.restarted = false;
        }
        if !wedged {
            return Recovery::Restart;
        }
        if self.restarted {
            Recovery::Reboot
        } else {
            self.restarted = true;
            Recovery::Restart
        }
    }
}
