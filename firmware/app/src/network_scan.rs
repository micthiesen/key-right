//! Bounds for Matter Network Commissioning responses.

// The pinned SDK encodes ScanNetworks into one 1,178-byte exchange payload,
// without chunking. Ten results with maximum-length SSIDs leave room for the
// command/response envelope; host tests encode that envelope with the real SDK.
pub const MAX_SCAN_RESULTS: usize = 10;
