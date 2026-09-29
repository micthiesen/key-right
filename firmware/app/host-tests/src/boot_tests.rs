use crate::boot::{commissioning_passcode, retry_io, COMMISSIONING_KEY};
use rs_matter::dm::clusters::scenes::ScenesState;
use rs_matter::error::{Error, ErrorCode};
use rs_matter::fabric::Fabrics;
use rs_matter::persist::{KvBlobStore, FABRIC_KEYS_START, KV_BUF_SIZE, SCENES_KEY};
use rs_matter::tlv::{TLVTag, TLVWrite, ToTLV};
use rs_matter::utils::storage::WriteBuf;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

fn run<T>(future: impl Future<Output = T>) -> T {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

#[derive(Default)]
struct Store {
    records: BTreeMap<u16, Vec<u8>>,
    fail_once: Option<u16>,
    loads: usize,
}
impl KvBlobStore for Store {
    fn load<'a>(&mut self, key: u16, buf: &'a mut [u8]) -> Result<Option<&'a [u8]>, Error> {
        self.loads += 1;
        if self.fail_once == Some(key) {
            self.fail_once = None;
            return Err(ErrorCode::StdIoError.into());
        }
        Ok(self.records.get(&key).map(|data| {
            buf[..data.len()].copy_from_slice(data);
            &buf[..data.len()]
        }))
    }
    fn store(&mut self, _: u16, _: &[u8], _: &mut [u8]) -> Result<(), Error> {
        panic!("boot recovery must preserve existing records")
    }
    fn remove(&mut self, _: u16, _: &mut [u8]) -> Result<(), Error> {
        panic!("boot recovery must never erase records")
    }
}

#[test]
fn transient_credential_read_preserves_the_original_passcode() {
    let expected = 34_567_890_u32;
    let mut store = Store::default();
    store
        .records
        .insert(COMMISSIONING_KEY, expected.to_le_bytes().to_vec());
    let before = store.records.clone();
    store.fail_once = Some(COMMISSIONING_KEY);
    let mut delays = Vec::new();
    let passcode = run(retry_io(
        async || commissioning_passcode(&mut store, || panic!("must not generate a new passcode")),
        async |_| {},
        || {},
        |_, seconds| delays.push(seconds),
    ))
    .unwrap();
    assert_eq!(passcode, expected);
    assert_eq!(store.loads, 2);
    assert_eq!(delays, [5]);
    assert_eq!(store.records, before);
}

#[test]
fn retry_after_a_partial_fabric_load_restores_both_fabrics_once() {
    let mut original = Fabrics::new();
    // Synthetic fabrics exercise persistence only; no operational credentials are used.
    original.add_with_post_init(|_| Ok(())).unwrap();
    original.add_with_post_init(|_| Ok(())).unwrap();
    let mut store = Store::default();
    for fabric in original.iter() {
        let mut bytes = [0; KV_BUF_SIZE];
        let mut writer = WriteBuf::new(&mut bytes);
        fabric.to_tlv(&TLVTag::Anonymous, &mut writer).unwrap();
        store.records.insert(
            FABRIC_KEYS_START + u16::from(fabric.fab_idx().get()),
            writer.as_slice().to_vec(),
        );
    }
    let before = store.records.clone();
    store.fail_once = Some(FABRIC_KEYS_START + 2);
    let mut restored = Fabrics::new();
    let mut buf = [0; KV_BUF_SIZE];
    let mut delays = Vec::new();
    run(retry_io(
        async || restored.load_persist(&mut store, &mut buf),
        async |_| {},
        || {},
        |_, seconds| delays.push(seconds),
    ))
    .unwrap();
    let ids: Vec<_> = restored
        .iter()
        .map(|fabric| fabric.fab_idx().get())
        .collect();
    assert_eq!(ids, [1, 2]);
    assert_eq!(delays, [5]);
    assert_eq!(store.records, before);
    for fabric in restored.iter() {
        let mut bytes = [0; KV_BUF_SIZE];
        let mut writer = WriteBuf::new(&mut bytes);
        fabric.to_tlv(&TLVTag::Anonymous, &mut writer).unwrap();
        assert_eq!(
            writer.as_slice(),
            before[&(FABRIC_KEYS_START + u16::from(fabric.fab_idx().get()))]
        );
    }
}

fn scene_record() -> Vec<u8> {
    let mut bytes = [0; 256];
    let mut writer = WriteBuf::new(&mut bytes);
    writer.start_struct(&TLVTag::Anonymous).unwrap();
    writer.start_array(&TLVTag::Context(0)).unwrap();
    for fabric in 1..=2 {
        writer.start_struct(&TLVTag::Anonymous).unwrap();
        writer.u8(&TLVTag::Context(0), fabric).unwrap();
        writer.u16(&TLVTag::Context(1), 1).unwrap();
        writer.u16(&TLVTag::Context(2), 0).unwrap();
        writer.u8(&TLVTag::Context(3), fabric).unwrap();
        writer.u32(&TLVTag::Context(4), 1000).unwrap();
        writer.str(&TLVTag::Context(5), &[]).unwrap();
        writer.end_container().unwrap();
    }
    writer.end_container().unwrap();
    writer.start_array(&TLVTag::Context(1)).unwrap();
    writer.end_container().unwrap();
    writer.end_container().unwrap();
    writer.as_slice().to_vec()
}

#[test]
fn transient_scene_read_retries_the_real_loader_without_modifying_records() {
    let mut store = Store::default();
    store.records.insert(SCENES_KEY, scene_record());
    let before = store.records.clone();
    store.fail_once = Some(SCENES_KEY);
    let scenes = ScenesState::<16>::new();
    let mut buf = [0; KV_BUF_SIZE];
    let mut delays = Vec::new();
    run(retry_io(
        async || scenes.load_persist(&mut store, &mut buf).await,
        async |_| {},
        || {},
        |_, seconds| delays.push(seconds),
    ))
    .unwrap();
    assert_eq!(delays, [5]);
    assert_eq!(store.loads, 2);
    assert_eq!(store.records, before);
}

#[test]
fn malformed_credentials_and_scenes_remain_faulted_without_retry_or_erase() {
    let mut store = Store::default();
    store.records.insert(COMMISSIONING_KEY, vec![0]);
    store.records.insert(SCENES_KEY, vec![0]);
    let before = store.records.clone();
    let error = run(retry_io(
        async || commissioning_passcode(&mut store, || panic!("must preserve malformed record")),
        async |_| panic!("decode faults must not retry"),
        || {},
        |_, _| panic!("decode faults must not retry"),
    ))
    .unwrap_err();
    assert_eq!(error.code(), ErrorCode::InvalidData);
    let scenes = ScenesState::<16>::new();
    let mut buf = [0; KV_BUF_SIZE];
    let error = run(retry_io(
        async || scenes.load_persist(&mut store, &mut buf).await,
        async |_| panic!("decode faults must not retry"),
        || {},
        |_, _| panic!("decode faults must not retry"),
    ))
    .unwrap_err();
    assert_ne!(error.code(), ErrorCode::StdIoError);
    assert_eq!(store.loads, 2);
    assert_eq!(store.records, before);
}

#[test]
fn repeated_io_failures_back_off_and_yield_with_watchdog_feeds_each_second() {
    let attempts = Cell::new(0);
    let feeds = Cell::new(0);
    let last_sleep_feed = Cell::new(0);
    let mut delays = Vec::new();
    let mut sleeps = 0;
    run(retry_io(
        async || {
            attempts.set(attempts.get() + 1);
            if attempts.get() <= 7 {
                Err(ErrorCode::StdIoError.into())
            } else {
                Ok(())
            }
        },
        async |seconds| {
            assert_eq!(seconds, 1);
            assert!(feeds.get() > last_sleep_feed.get());
            last_sleep_feed.set(feeds.get());
            sleeps += 1;
            let mut yielded = false;
            std::future::poll_fn(|context| {
                if yielded {
                    Poll::Ready(())
                } else {
                    yielded = true;
                    context.waker().wake_by_ref();
                    Poll::Pending
                }
            })
            .await;
        },
        || feeds.set(feeds.get() + 1),
        |_, seconds| delays.push(seconds),
    ))
    .unwrap();
    assert_eq!(delays, [5, 10, 20, 40, 60, 60, 60]);
    assert_eq!(sleeps, delays.iter().sum::<u32>());
    assert_eq!(feeds.get(), sleeps + attempts.get());
}
