//! Public discovery identity and local setup payloads, separate from stored secrets.

use rs_matter_embassy::matter::dm::clusters::basic_info::BasicInfoConfig;
use rs_matter_embassy::matter::error::{Error, ErrorCode};
use rs_matter_embassy::matter::pairing::qr::{no_optional_data, CommFlowType, QrPayload};
use rs_matter_embassy::matter::pairing::DiscoveryCapabilities;
use rs_matter_embassy::matter::BasicCommData;

/// Four MAC suffix digits distinguish the lamps within the legacy BLE budget.
pub struct DiscoveryName([u8; 14]);

impl DiscoveryName {
    pub fn new(mac: [u8; 6]) -> Self {
        let mut name = *b"Key Right 0000";
        let hex = b"0123456789ABCDEF";
        for (index, byte) in mac[4..].iter().enumerate() {
            name[10 + index * 2] = hex[(byte >> 4) as usize];
            name[11 + index * 2] = hex[(byte & 0x0f) as usize];
        }
        Self(name)
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.0).expect("discovery name is ASCII")
    }
}

/// Encode the existing credential using the pinned SDK's standard BLE QR format.
pub fn qr_payload<'a>(
    commissioning: BasicCommData,
    device: &BasicInfoConfig<'_>,
    buffer: &'a mut [u8],
) -> Result<&'a str, Error> {
    let payload = QrPayload::new_from_basic_info(
        DiscoveryCapabilities::BLE,
        CommFlowType::Standard,
        commissioning,
        device,
        no_optional_data,
    );
    // The pinned SDK's as_str splits the buffer before checking its capacity.
    if payload.emit_chars().count() > buffer.len() {
        return Err(ErrorCode::BufferTooSmall.into());
    }
    payload.as_str(buffer).map(|(text, _)| text)
}
