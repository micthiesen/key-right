//! Shared partition discovery and per-device commissioning credential.
use embassy_embedded_hal::adapter::BlockingAsync;
use esp_bootloader_esp_idf::partitions::{
    read_partition_table, DataPartitionSubType, PartitionType, PARTITION_TABLE_MAX_LEN,
};
use esp_hal::{peripherals::FLASH, rng::Trng};
use esp_storage::FlashStorage;
use rs_matter_embassy::matter::error::{Error, ErrorCode};
use rs_matter_embassy::matter::persist::{KvBlobStore, KV_BUF_SIZE, VENDOR_KEYS_START};
use rs_matter_embassy::persist::SeqMapKvBlobStore;
const COMMISSIONING_KEY: u16 = VENDOR_KEYS_START;
pub fn persistent_store<'d>(
    flash: FLASH<'d>,
    buf: &mut [u8; PARTITION_TABLE_MAX_LEN],
) -> Result<impl KvBlobStore + 'd, Error> {
    let mut flash = FlashStorage::new(flash);
    let table = read_partition_table(&mut flash, buf).map_err(|_| ErrorCode::InvalidData)?;
    let nvs = table
        .find_partition(PartitionType::Data(DataPartitionSubType::Nvs))
        .map_err(|_| ErrorCode::InvalidData)?
        .ok_or(ErrorCode::NotFound)?;
    let range = nvs.offset()..nvs.offset() + nvs.len();
    log::info!("Matter NVS: {:#x}..{:#x}", range.start, range.end);
    Ok(SeqMapKvBlobStore::new(BlockingAsync::new(flash), range))
}

/// One credential per fresh NVS, generated from entropy rather than a MAC or public test PIN.
pub fn commissioning_passcode(store: &mut impl KvBlobStore, trng: &Trng) -> Result<u32, Error> {
    // Compaction may move Matter records as well as this four-byte credential.
    let mut buf = [0; KV_BUF_SIZE];
    if let Some(data) = store.load(COMMISSIONING_KEY, &mut buf)? {
        let bytes: [u8; 4] = data.try_into().map_err(|_| ErrorCode::InvalidData)?;
        let passcode = u32::from_le_bytes(bytes);
        return valid_passcode(passcode)
            .then_some(passcode)
            .ok_or_else(|| ErrorCode::InvalidData.into());
    }

    let passcode = loop {
        // Rejection sampling is uniform over permitted Matter passcodes.
        let candidate = trng.random() & 0x07ff_ffff;
        if valid_passcode(candidate) {
            break candidate;
        }
    };
    store.store(COMMISSIONING_KEY, &passcode.to_le_bytes(), &mut buf)?;
    Ok(passcode)
}

fn valid_passcode(value: u32) -> bool {
    (1..=99_999_998).contains(&value)
        && !matches!(
            value,
            11_111_111
                | 22_222_222
                | 33_333_333
                | 44_444_444
                | 55_555_555
                | 66_666_666
                | 77_777_777
                | 88_888_888
                | 12_345_678
                | 87_654_321
        )
}
