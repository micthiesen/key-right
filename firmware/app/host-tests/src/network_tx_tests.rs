use crate::network_tx::{GuardedDriver, QueueHealth, QueueRecovery};
use embassy_net_driver::{Capabilities, Driver, HardwareAddress, LinkState, RxToken, TxToken};
use std::cell::Cell;
use std::task::{Context, Waker};

#[test]
fn persistent_queue_stall_escalates_only_after_a_driver_recreation() {
    let mut recovery = QueueRecovery::default();
    assert!(!recovery.reboot_required());
    recovery.record_health(60_000);
    assert!(recovery.reboot_required());
    // A healthy run ending for an unrelated reason clears the old incident.
    recovery.record_health(120_000);
    recovery.record_health(0);
    assert!(!recovery.reboot_required());
    assert!(recovery.reboot_required());
}

#[test]
fn sixty_seconds_of_stuck_queue_cannot_complete_the_healthy_reset_window() {
    let mut recovery = QueueRecovery::default();
    assert!(!recovery.reboot_required());
    let healthy_ms = crate::network_tx::healthy_until(121_000, Some(61_000));
    recovery.record_health(healthy_ms);
    assert!(recovery.reboot_required());
    recovery.record_health(crate::network_tx::healthy_until(121_000, None));
    assert!(!recovery.reboot_required());
}

#[test]
fn only_sustained_local_queue_exhaustion_triggers_once() {
    let mut health = QueueHealth::default();
    assert!(!health.observe(0, false, false));
    assert!(!health.observe(3_600_000, false, false));
    assert!(!health.observe(3_600_000, true, false));
    assert!(!health.observe(3_659_999, true, false));
    assert!(health.observe(3_660_000, true, false));
    assert!(!health.observe(3_720_000, true, false));
    assert!(!health.observe(3_720_000, true, true));
    assert!(!health.observe(3_720_001, true, false));
    assert!(health.observe(3_780_001, true, false));
}

#[test]
fn capacity_or_link_loss_restarts_the_entire_grace_period() {
    let mut health = QueueHealth::default();
    assert!(!health.observe(0, true, false));
    assert!(!health.observe(59_999, true, true));
    assert!(!health.observe(60_000, true, false));
    assert!(!health.observe(119_999, false, false));
    assert!(!health.observe(120_000, true, false));
    assert!(!health.observe(179_999, true, false));
    assert!(health.observe(180_000, true, false));
}

struct Token<'a>(&'a Cell<usize>);
impl TxToken for Token<'_> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        self.0.set(self.0.get() + 1);
        f(&mut [0; 8][..len])
    }
}
struct Received;
impl RxToken for Received {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        f(&mut [42])
    }
}
struct FakeDriver<'a> {
    consumed: &'a Cell<usize>,
    probes: &'a Cell<usize>,
    capacity: &'a Cell<bool>,
}
impl Driver for FakeDriver<'_> {
    type RxToken<'a>
        = Received
    where
        Self: 'a;
    type TxToken<'a>
        = Token<'a>
    where
        Self: 'a;
    fn receive(&mut self, _: &mut Context<'_>) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        self.capacity
            .get()
            .then_some((Received, Token(self.consumed)))
    }
    fn transmit(&mut self, _: &mut Context<'_>) -> Option<Self::TxToken<'_>> {
        self.probes.set(self.probes.get() + 1);
        self.capacity.get().then_some(Token(self.consumed))
    }
    fn link_state(&mut self, _: &mut Context<'_>) -> LinkState {
        LinkState::Up
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }
    fn hardware_address(&self) -> HardwareAddress {
        HardwareAddress::Ethernet([1; 6])
    }
}

#[test]
fn guard_preserves_packet_tokens_and_does_not_transmit_its_probes() {
    let consumed = Cell::new(0);
    let probes = Cell::new(0);
    let capacity = Cell::new(true);
    let mut driver = GuardedDriver::new(
        FakeDriver {
            consumed: &consumed,
            probes: &probes,
            capacity: &capacity,
        },
        || panic!("healthy queue"),
    );
    let mut cx = Context::from_waker(Waker::noop());
    for _ in 0..100 {
        assert!(driver.link_state(&mut cx) == LinkState::Up);
    }
    assert_eq!(probes.get(), 100);
    assert_eq!(consumed.get(), 0);
    assert_eq!(driver.hardware_address(), HardwareAddress::Ethernet([1; 6]));
    driver
        .transmit(&mut cx)
        .unwrap()
        .consume(1, |bytes| bytes[0] = 7);
    let (rx, tx) = driver.receive(&mut cx).unwrap();
    assert_eq!(rx.consume(|bytes| bytes[0]), 42);
    tx.consume(1, |bytes| bytes[0] = 8);
    assert_eq!(consumed.get(), 2);
}

thread_local! { static NOW: Cell<u64> = const { Cell::new(0) }; }
fn now() -> u64 {
    NOW.with(Cell::get)
}

#[test]
fn healthy_packet_progress_prevents_false_stall_when_every_later_probe_is_full() {
    let consumed = Cell::new(0);
    let probes = Cell::new(0);
    let capacity = Cell::new(false);
    let stalls = Cell::new(0);
    let mut driver = GuardedDriver::with_clock(
        FakeDriver {
            consumed: &consumed,
            probes: &probes,
            capacity: &capacity,
        },
        || stalls.set(stalls.get() + 1),
        now,
    );
    let mut cx = Context::from_waker(Waker::noop());
    NOW.with(|clock| clock.set(0));
    driver.link_state(&mut cx);
    for tick in 1..=20 {
        NOW.with(|clock| clock.set(tick * 10_000));
        // Embassy consumes newly available capacity before calling link_state.
        capacity.set(true);
        if tick % 2 == 0 {
            driver.transmit(&mut cx).unwrap().consume(1, |_| ());
        } else {
            let (_, tx) = driver.receive(&mut cx).unwrap();
            tx.consume(1, |_| ());
        }
        capacity.set(false);
        driver.link_state(&mut cx);
    }
    assert_eq!(stalls.get(), 0);
    NOW.with(|clock| clock.set(260_000));
    driver.link_state(&mut cx);
    assert_eq!(
        stalls.get(),
        1,
        "only a subsequent interval with no progress stalls"
    );
}
