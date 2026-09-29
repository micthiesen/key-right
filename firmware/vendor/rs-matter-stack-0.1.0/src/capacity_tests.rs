use super::{MatterBuffers, MAX_IM_BUFFERS, MAX_RESPONDERS, MAX_SUBSCRIPTIONS};
use rs_matter::dm::clusters::basic_info::CapabilityMinima;
use rs_matter::utils::storage::pooled::Buffers;

#[test]
fn subscription_minimum_leaves_request_and_publishing_buffers() {
    assert_eq!(MAX_SUBSCRIPTIONS, 15);
    assert_eq!(MAX_RESPONDERS, 2);
    assert_eq!(MAX_IM_BUFFERS, 20);
    let promised = rs_matter::fabric::MAX_FABRICS
        * usize::from(CapabilityMinima::new().subscriptions_per_fabric);
    assert_eq!(MAX_SUBSCRIPTIONS, promised);

    let pool = MatterBuffers::<MAX_IM_BUFFERS>::new();
    let subscriptions: [_; MAX_SUBSCRIPTIONS] =
        core::array::from_fn(|_| pool.get_immediate().unwrap());
    let requests: [_; MAX_RESPONDERS * 2] = core::array::from_fn(|_| pool.get_immediate().unwrap());
    let publisher = pool.get_immediate().unwrap();
    assert!(pool.get_immediate().is_none());

    // Completing a report or request releases its slots while every promised
    // subscription remains held. No radio/controller traffic is needed here.
    drop(publisher);
    assert!(pool.get_immediate().is_some());
    drop(requests);
    let available: [_; 5] = core::array::from_fn(|_| pool.get_immediate().unwrap());
    assert!(pool.get_immediate().is_none());
    drop(available);
    drop(subscriptions);
}
