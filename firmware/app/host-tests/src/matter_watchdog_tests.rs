use crate::matter_watchdog::{
    Recovery, Sample, Watch, Wedge, WedgeRecovery, PRODUCTIVE_RUN_MS, SAMPLE_MS, STUCK_LIMIT_MS,
};

const HEALTHY: Sample = Sample {
    tx_free: true,
    associated: true,
};

const STUCK: Sample = Sample {
    tx_free: false,
    associated: true,
};

fn run(watch: &mut Watch, from_ms: u64, to_ms: u64, sample: Sample) -> Option<u64> {
    (from_ms..=to_ms)
        .step_by(SAMPLE_MS as usize)
        .find(|&t| watch.observe(t, sample).is_some())
}

#[test]
fn a_healthy_transport_is_never_restarted() {
    let mut watch = Watch::new(0);
    assert_eq!(run(&mut watch, 0, 24 * 3_600_000, HEALTHY), None);
}

#[test]
fn a_held_tx_buffer_restarts_after_ninety_seconds() {
    let mut watch = Watch::new(0);
    assert_eq!(run(&mut watch, 0, 60_000, HEALTHY), None);
    assert_eq!(watch.observe(60_000 + STUCK_LIMIT_MS - 1, STUCK), None);
    assert_eq!(
        run(&mut watch, 61_000, 400_000, STUCK),
        Some(60_000 + STUCK_LIMIT_MS)
    );
    assert_eq!(Wedge.as_str(), "transport TX buffer held for 90 s");
}

#[test]
fn a_buffer_free_at_any_sample_in_the_window_is_traffic() {
    let mut watch = Watch::new(0);
    for t in (0..3_600_000).step_by(SAMPLE_MS as usize) {
        let sample = Sample {
            tx_free: t % 60_000 == 0,
            associated: true,
        };
        assert_eq!(watch.observe(t, sample), None, "t={t}");
    }
}

#[test]
fn a_buffer_held_off_wifi_is_left_to_the_association_monitors() {
    let mut watch = Watch::new(0);
    let outage = Sample {
        tx_free: false,
        associated: false,
    };
    assert_eq!(run(&mut watch, 0, 3_600_000, outage), None);
    // Back on Wi-Fi with the buffer still held: the clock starts at the last
    // sample taken while disassociated.
    assert_eq!(
        run(&mut watch, 3_601_000, 4_000_000, STUCK),
        Some(3_600_000 + STUCK_LIMIT_MS)
    );
}

#[test]
fn a_first_wedge_restarts_and_a_quick_second_one_resets() {
    let mut recovery = WedgeRecovery::default();
    assert_eq!(recovery.after_run(STUCK_LIMIT_MS, true), Recovery::Restart);
    assert_eq!(recovery.after_run(STUCK_LIMIT_MS, true), Recovery::Reboot);
    // A reset deferred for an unsaved command stays due for the next wedge.
    assert_eq!(recovery.after_run(STUCK_LIMIT_MS, true), Recovery::Reboot);
}

#[test]
fn other_restarts_between_wedges_do_not_clear_the_incident() {
    let mut recovery = WedgeRecovery::default();
    assert_eq!(recovery.after_run(STUCK_LIMIT_MS, true), Recovery::Restart);
    // Includes a long Wi-Fi outage: the caller passes healthy time, not wall time.
    assert_eq!(recovery.after_run(0, false), Recovery::Restart);
    assert_eq!(recovery.after_run(STUCK_LIMIT_MS, true), Recovery::Reboot);
}

#[test]
fn fifteen_healthy_minutes_make_the_next_wedge_a_new_incident() {
    let mut recovery = WedgeRecovery::default();
    assert_eq!(recovery.after_run(STUCK_LIMIT_MS, true), Recovery::Restart);
    assert_eq!(
        recovery.after_run(PRODUCTIVE_RUN_MS - 1, true),
        Recovery::Reboot
    );
    let mut recovery = WedgeRecovery::default();
    assert_eq!(recovery.after_run(STUCK_LIMIT_MS, true), Recovery::Restart);
    assert_eq!(
        recovery.after_run(PRODUCTIVE_RUN_MS, true),
        Recovery::Restart
    );
    assert_eq!(
        recovery.after_run(PRODUCTIVE_RUN_MS, false),
        Recovery::Restart
    );
    assert_eq!(recovery.after_run(STUCK_LIMIT_MS, true), Recovery::Restart);
}

#[test]
fn non_wedge_stops_never_reset() {
    let mut recovery = WedgeRecovery::default();
    for _ in 0..100 {
        assert_eq!(recovery.after_run(1_000, false), Recovery::Restart);
    }
}
