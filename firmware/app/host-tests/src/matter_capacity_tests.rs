// Exercise the C3 profile's real pool implementation without adding the entire
// network SDK to host tests. The vendored stack checks these capacity relations
// against its actual feature-unified constants in every target build, and runs
// this same test against those constants when tested directly.
use rs_matter::transport::exchange::MatterBuffers;

const MAX_SUBSCRIPTIONS: usize = 15;
const MAX_IM_BUFFERS: usize = 20;
const MAX_RESPONDERS: usize = 2;

#[path = "../../../vendor/rs-matter-stack-0.1.0/src/capacity_tests.rs"]
mod pool_behavior;
