//! Shared partition discovery and per-device commissioning credential.
use embassy_embedded_hal::adapter::BlockingAsync;
use esp_bootloader_esp_idf::partitions::{
    read_partition_table, DataPartitionSubType, Error as PartitionError, PartitionType,
    PARTITION_TABLE_MAX_LEN,
};
use esp_hal::{peripherals::FLASH, rng::Trng};
use esp_storage::FlashStorage;
use rs_matter_embassy::matter::error::{Error, ErrorCode};
use rs_matter_embassy::matter::persist::KvBlobStore;
use rs_matter_embassy::persist::SeqMapKvBlobStore;
pub async fn persistent_store<'d>(
    flash: FLASH<'d>,
    buf: &mut [u8; PARTITION_TABLE_MAX_LEN],
    feed: impl FnMut(),
) -> Result<impl KvBlobStore + 'd, Error> {
    let mut flash = FlashStorage::new(flash);
    let range = crate::boot::restore(
        "NVS partition read failed",
        async || {
            let table = read_partition_table(&mut flash, buf).map_err(|error| match error {
                PartitionError::StorageError => ErrorCode::StdIoError,
                _ => ErrorCode::InvalidData,
            })?;
            let nvs = table
                .find_partition(PartitionType::Data(DataPartitionSubType::Nvs))
                .map_err(|_| ErrorCode::InvalidData)?
                .ok_or(ErrorCode::NotFound)?;
            Ok(nvs.offset()..nvs.offset() + nvs.len())
        },
        feed,
    )
    .await?;
    log::info!("Matter NVS: {:#x}..{:#x}", range.start, range.end);
    Ok(SeqMapKvBlobStore::new(BlockingAsync::new(flash), range))
}

/// One credential per fresh NVS, generated from entropy rather than a MAC or public test PIN.
pub fn commissioning_passcode(store: &mut impl KvBlobStore, trng: &Trng) -> Result<u32, Error> {
    crate::boot::commissioning_passcode(store, || trng.random())
}
