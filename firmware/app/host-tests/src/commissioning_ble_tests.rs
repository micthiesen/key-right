use embassy_sync_07::blocking_mutex::raw::NoopRawMutex;
use trouble_host::prelude::{
    appearance, AttributeTable, CentralConfig, GapConfig, PeripheralConfig,
};

type Table<'a> = AttributeTable<'a, NoopRawMutex, 6>;

fn config(name: &str, peripheral: bool) -> GapConfig<'_> {
    if peripheral {
        GapConfig::Peripheral(PeripheralConfig {
            name,
            appearance: &appearance::power_device::GENERIC_POWER_DEVICE,
        })
    } else {
        GapConfig::Central(CentralConfig {
            name,
            appearance: &appearance::UNKNOWN,
        })
    }
}

fn read_name(table: &Table<'_>) -> Vec<u8> {
    // GAP service = 1, device-name declaration = 2, device-name value = 3.
    let mut bytes = [0; 22];
    let count = table.read(3, 0, &mut bytes).unwrap();
    bytes[..count].to_vec()
}

#[test]
fn peripheral_gap_restarts_preserve_each_borrowed_device_name() {
    repeated_gap_builds(true);
}

#[test]
fn central_gap_restarts_preserve_each_borrowed_device_name() {
    repeated_gap_builds(false);
}

fn repeated_gap_builds(peripheral: bool) {
    let first_name = String::from("Key Right first");
    let mut first = Table::new();
    config(&first_name, peripheral).build(&mut first).unwrap();
    for attempt in 0..10 {
        let next_name = format!("Key Right {attempt}");
        let mut next = Table::new();
        config(&next_name, peripheral).build(&mut next).unwrap();
        assert_eq!(read_name(&next), next_name.as_bytes());
        assert_eq!(read_name(&first), first_name.as_bytes());
        assert!(next.write(3, 0, b"changed").is_err());
    }
}

#[test]
fn gap_name_limit_remains_22_bytes_and_rejection_does_not_poison_retry() {
    for peripheral in [true, false] {
        for name in ["", "1234567890123456789012", "ééééééééééé"] {
            let mut table = Table::new();
            config(name, peripheral).build(&mut table).unwrap();
            assert_eq!(read_name(&table), name.as_bytes());
        }
        let mut table = Table::new();
        assert_eq!(
            config("12345678901234567890123", peripheral).build(&mut table),
            Err("Device name is too long. Max length is 22 bytes")
        );
        config("retry", peripheral).build(&mut table).unwrap();
        assert_eq!(read_name(&table), b"retry");
    }
}
