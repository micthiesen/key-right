use crate::commissioning::{qr_payload, DiscoveryName};
use rs_matter::dm::clusters::basic_info::BasicInfoConfig;
use rs_matter::error::ErrorCode;
use rs_matter::transport::network::btp::AdvData;
use rs_matter::BasicCommData;

#[test]
fn per_lamp_name_fits_alongside_the_matter_ble_service() {
    let name = DiscoveryName::new([0x10, 0x20, 0x30, 0x40, 0xec, 0xf4]);
    assert_eq!(name.as_str(), "Key Right ECF4");
    assert_eq!(
        DiscoveryName::new([0, 0, 0, 0, 0, 1]).as_str(),
        "Key Right 0001"
    );
    let info = BasicInfoConfig::new();
    let adv = AdvData::new(&info, 0xfff);
    // Match the pinned Embassy adapter: flags, Matter service data, complete name.
    // The name record adds its length and type bytes to the UTF-8 name bytes.
    assert_eq!(adv.iter().count() + 2 + name.as_str().len(), 31);
}

#[test]
fn qr_matches_the_pinned_sdk_standard_ble_vector() {
    let commissioning = BasicCommData {
        password: 34567890_u32.to_le_bytes().into(),
        discriminator: 2976,
    };
    let info = BasicInfoConfig {
        vid: 9050,
        pid: 65279,
        ..BasicInfoConfig::new()
    };
    let mut buffer = [0; 128];
    assert_eq!(
        qr_payload(commissioning.clone(), &info, &mut buffer).unwrap(),
        "MT:YNJV7VSC00CMVH7SR00"
    );
    assert_eq!(
        qr_payload(commissioning, &info, &mut [0; 4])
            .unwrap_err()
            .code(),
        ErrorCode::BufferTooSmall
    );
}

#[test]
fn qr_with_full_device_identity_fits_the_firmware_buffer() {
    let info = BasicInfoConfig {
        vid: 0xfff1,
        pid: 0x8000,
        serial_no: "KR-10:20:30:40:EC:F4",
        ..BasicInfoConfig::new()
    };
    let credential = BasicCommData {
        password: 34567890_u32.to_le_bytes().into(),
        discriminator: 0xcf4,
    };
    let mut buffer = [0; 128];
    assert!(qr_payload(credential, &info, &mut buffer)
        .unwrap()
        .starts_with("MT:"));
}
