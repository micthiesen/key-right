//! Boot storage retries, before transport or light output starts.
use rs_matter_embassy::matter::error::{Error, ErrorCode};
use rs_matter_embassy::matter::persist::{KvBlobStore, KV_BUF_SIZE, VENDOR_KEYS_START};

pub const COMMISSIONING_KEY: u16 = VENDOR_KEYS_START;

/// Retry adapter-reported StdIoError; decode/configuration errors remain faults.
/// The pinned adapter also maps backend corruption into StdIoError and may repair
/// pages on reads. This helper never requests an erase or replaces decoded records.
/// The one-second waits yield to the executor and keep the 15-second watchdog fed.
pub async fn retry_io<T>(
    mut operation: impl AsyncFnMut() -> Result<T, Error>,
    mut sleep: impl AsyncFnMut(u32),
    mut feed: impl FnMut(),
    mut report: impl FnMut(&Error, u32),
) -> Result<T, Error> {
    let mut delay_seconds = 5;
    loop {
        feed();
        match operation().await {
            Ok(value) => return Ok(value),
            Err(error) if error.code() != ErrorCode::StdIoError => return Err(error),
            Err(error) => {
                report(&error, delay_seconds);
                for _ in 0..delay_seconds {
                    sleep(1).await;
                    feed();
                }
                delay_seconds = (delay_seconds * 2).min(60);
            }
        }
    }
}

pub async fn restore<T>(
    reason: &str,
    operation: impl AsyncFnMut() -> Result<T, Error>,
    feed: impl FnMut(),
) -> Result<T, Error> {
    retry_io(
        operation,
        async |seconds| {
            embassy_time::Timer::after(embassy_time::Duration::from_secs(u64::from(seconds))).await;
        },
        feed,
        |error, seconds| {
            log::error!("{reason}: {error:?}; no reset/erase requested; retrying in {seconds}s");
        },
    )
    .await
}

/// Restore the existing credential before considering a fresh one. The caller supplies entropy.
pub fn commissioning_passcode(
    store: &mut impl KvBlobStore,
    mut random: impl FnMut() -> u32,
) -> Result<u32, Error> {
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
        let candidate = random() & 0x07ff_ffff;
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
