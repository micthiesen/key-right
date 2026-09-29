use crate::recovery::{
    usable_local_ipv6, AssociatedHealth, DriverErrors, LocalNetHealth, RestartBackoff,
};

#[test]
fn local_readiness_requires_an_operational_unicast_ipv6_address() {
    use core::net::Ipv6Addr;
    let local = "fe80::1234".parse().unwrap();
    assert!(usable_local_ipv6(true, &[local]));
    assert!(!usable_local_ipv6(false, &[local]));
    // An IPv4 address is insufficient when the IPv6 list is empty.
    assert!(!usable_local_ipv6(true, &[]));
    assert!(!usable_local_ipv6(
        true,
        &[
            Ipv6Addr::UNSPECIFIED,
            Ipv6Addr::LOCALHOST,
            "ff02::1".parse().unwrap(),
        ]
    ));
    assert!(usable_local_ipv6(true, &[Ipv6Addr::UNSPECIFIED, local]));
}

#[test]
fn internal_driver_errors_recreate_after_three_consecutive_failures() {
    let mut errors = DriverErrors::default();
    assert!(!errors.observe(true));
    assert!(!errors.observe(true));
    assert!(errors.observe(true));
    for _ in 0..300 {
        assert!(errors.observe(true));
    }
    assert!(!errors.observe(false));
    assert!(!errors.observe(true));
}

#[test]
fn expected_association_failures_and_success_break_internal_error_streaks() {
    let mut errors = DriverErrors::default();
    for _ in 0..100 {
        assert!(!errors.observe(true));
        assert!(!errors.observe(true));
        // An absent AP or a successful operation must prevent escalation.
        assert!(!errors.observe(false));
    }
}

#[test]
fn associated_radio_queries_must_fail_continuously_for_a_minute() {
    let mut health = AssociatedHealth::default();
    assert!(!health.restart_due(100, true, false));
    assert!(!health.restart_due(60_099, true, false));
    assert!(health.restart_due(60_100, true, false));
    assert!(!health.restart_due(60_101, true, true));
    assert!(!health.restart_due(120_101, true, false));
    assert!(!health.restart_due(120_102, false, false));
    assert!(!health.restart_due(999_999, false, false));
    assert!(!health.restart_due(1_000_000, true, false));
    assert!(health.restart_due(1_060_000, true, false));
}

#[test]
fn ipv6_loss_after_a_healthy_period_recovers_but_disassociation_is_idle() {
    let mut health = LocalNetHealth::default();
    assert!(!health.restart_due(0, true, true));
    assert!(!health.restart_due(10_000, true, false));
    assert!(!health.restart_due(69_999, true, false));
    assert!(health.restart_due(70_000, true, false));
    assert!(!health.restart_due(70_001, false, false));
    assert!(!health.restart_due(1_000_000, false, false));
    assert!(!health.restart_due(1_000_001, true, false));
    assert!(!health.restart_due(1_059_999, true, true));
}

#[test]
fn transport_restart_backoff_is_capped_and_only_healthy_operation_resets_it() {
    let mut backoff = RestartBackoff::default();
    for expected in [5, 10, 20, 40, 60, 60, 60] {
        assert_eq!(backoff.next_delay_secs(0), expected);
    }
    for _ in 0..300 {
        assert_eq!(backoff.next_delay_secs(119_999), 60);
    }
    assert_eq!(backoff.next_delay_secs(120_000), 5);
    assert_eq!(backoff.next_delay_secs(0), 10);
    assert_eq!(backoff.next_delay_secs(600_000), 5);
}
