//! Observe local transmit capacity, never Internet or controller traffic.
use core::{future::Future, pin::Pin, task::Context};
use embassy_net_driver::{Capabilities, Driver, HardwareAddress, LinkState};
use embassy_time::{Duration, Instant, Timer};

const STALL_MS: u64 = 60_000;

/// The pinned radio's in-flight counter survives driver recreation. Escalate
/// only when that fresh driver still cannot transmit; ordinary outages never
/// enter this policy. Two healthy minutes allow a new independent incident.
#[derive(Default)]
pub struct QueueRecovery {
    recreated: bool,
}
impl QueueRecovery {
    pub fn record_health(&mut self, healthy_for_ms: u64) {
        if healthy_for_ms >= 120_000 {
            self.recreated = false;
        }
    }
    pub fn reboot_required(&mut self) -> bool {
        core::mem::replace(&mut self.recreated, true)
    }
}

#[derive(Default)]
pub struct QueueHealth {
    blocked_since: Option<u64>,
    reported: bool,
}

impl QueueHealth {
    pub fn observe(&mut self, now: u64, linked: bool, capacity: bool) -> bool {
        if !linked || capacity {
            self.blocked_since = None;
            self.reported = false;
            return false;
        }
        let since = *self.blocked_since.get_or_insert(now);
        if !self.reported && now.saturating_sub(since) >= STALL_MS {
            self.reported = true;
            return true;
        }
        false
    }

    fn remaining_ms(&self, now: u64) -> Option<u64> {
        self.blocked_since
            .filter(|_| !self.reported)
            .map(|since| STALL_MS.saturating_sub(now.saturating_sub(since)))
    }
}

/// Wrap the sole station interface. Probing uses the runner's own waker and
/// drops an unconsumed token, so it sends no packet and steals no wakeups.
pub struct GuardedDriver<D, F> {
    inner: D,
    health: QueueHealth,
    stalled: F,
    now: fn() -> u64,
    observe_health: fn(Option<u64>),
}

impl<D, F> GuardedDriver<D, F> {
    pub fn new(inner: D, stalled: F) -> Self {
        Self::with_clock(inner, stalled, || Instant::now().as_millis())
    }

    pub(crate) fn with_clock(inner: D, stalled: F, now: fn() -> u64) -> Self {
        Self {
            inner,
            health: QueueHealth::default(),
            stalled,
            now,
            observe_health: |_| {},
        }
    }

    pub fn with_health_observer(mut self, observer: fn(Option<u64>)) -> Self {
        self.observe_health = observer;
        self
    }
}

impl<D: Driver, F: FnMut()> Driver for GuardedDriver<D, F> {
    type RxToken<'a>
        = D::RxToken<'a>
    where
        Self: 'a;
    type TxToken<'a>
        = D::TxToken<'a>
    where
        Self: 'a;

    fn receive(&mut self, cx: &mut Context<'_>) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let tokens = self.inner.receive(cx);
        if tokens.is_some() {
            self.health.observe((self.now)(), true, true);
            (self.observe_health)(None);
        }
        tokens
    }

    fn transmit(&mut self, cx: &mut Context<'_>) -> Option<Self::TxToken<'_>> {
        let token = self.inner.transmit(cx);
        if token.is_some() {
            self.health.observe((self.now)(), true, true);
            (self.observe_health)(None);
        }
        token
    }

    fn link_state(&mut self, cx: &mut Context<'_>) -> LinkState {
        let link = self.inner.link_state(cx);
        let capacity = link == LinkState::Up && self.inner.transmit(cx).is_some();
        let now = (self.now)();
        let stalled = self.health.observe(now, link == LinkState::Up, capacity);
        (self.observe_health)(self.health.blocked_since);
        if stalled {
            (self.stalled)();
        }
        if let Some(remaining) = self.health.remaining_ms(now) {
            // Even a lost TX-completion wake must not leave the runner asleep
            // forever. Embassy's timer queue retains this scheduled wake.
            let mut alarm = Timer::after(Duration::from_millis(remaining));
            let _ = Pin::new(&mut alarm).poll(cx);
        }
        link
    }

    fn capabilities(&self) -> Capabilities {
        self.inner.capabilities()
    }
    fn hardware_address(&self) -> HardwareAddress {
        self.inner.hardware_address()
    }
}

/// Health time must stop at the beginning of known local backpressure, even
/// while the radio still claims association and has an IPv6 address.
pub fn healthy_until(now: u64, blocked_since: Option<u64>) -> u64 {
    blocked_since.map_or(now, |since| now.min(since))
}
