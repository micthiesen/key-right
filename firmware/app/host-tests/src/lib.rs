//! Runs the production hooks with the same Matter API and an in-memory store.
//! Recreate the SDK's Matter re-export without linking its ESP transport on a host.
extern crate self as rs_matter_embassy;
pub use rs_matter as matter;

#[path = "../../src/light.rs"]
pub mod light;

#[path = "../../src/presets.rs"]
pub mod presets;
#[path = "../../src/protocol.rs"]
pub mod protocol;
#[path = "../../src/recovery.rs"]
pub mod recovery;
#[path = "../../src/runtime.rs"]
pub mod runtime;

#[cfg(test)]
mod runtime_tests;

#[cfg(test)]
mod tests {
    use core::cell::{Cell, RefCell};
    use core::future::Future;
    use core::pin::pin;
    use core::task::{Context, Waker};

    use crate::light::BenchLight;
    use rs_matter::dm::clusters::app::on_off::{
        OnOffHandler, OnOffHooks, OutOfBandMessage, StartUpOnOffEnum,
    };
    use rs_matter::dm::Dataver;
    use rs_matter::error::{Error, ErrorCode};
    use rs_matter::persist::{KvBlobStore, KvBlobStoreAccess, KV_BUF_SIZE, VENDOR_KEYS_START};
    use rs_matter::tlv::Nullable;

    #[derive(Default)]
    struct Memory {
        data: RefCell<Option<Vec<u8>>>,
        writes: Cell<usize>,
        fail: Cell<bool>,
    }

    struct Store<'a>(&'a Memory);

    impl KvBlobStore for Store<'_> {
        fn load<'a>(&mut self, key: u16, buf: &'a mut [u8]) -> Result<Option<&'a [u8]>, Error> {
            assert_eq!(key, VENDOR_KEYS_START + 1);
            let data = self.0.data.borrow();
            Ok(data.as_ref().map(|data| {
                buf[..data.len()].copy_from_slice(data);
                &buf[..data.len()]
            }))
        }

        fn store(&mut self, key: u16, data: &[u8], _buf: &mut [u8]) -> Result<(), Error> {
            assert_eq!(key, VENDOR_KEYS_START + 1);
            if self.0.fail.get() {
                return Err(ErrorCode::InvalidData.into());
            }
            self.0.writes.set(self.0.writes.get() + 1);
            *self.0.data.borrow_mut() = Some(data.to_vec());
            Ok(())
        }

        fn remove(&mut self, _key: u16, _buf: &mut [u8]) -> Result<(), Error> {
            panic!("bench hooks must never erase NVS");
        }
    }

    impl KvBlobStoreAccess for Memory {
        fn access<F, R>(&self, f: F) -> R
        where
            F: FnOnce(&mut dyn KvBlobStore, &mut [u8]) -> R,
        {
            f(&mut Store(self), &mut [0; KV_BUF_SIZE])
        }
    }

    #[test]
    fn initial_boot_is_off_and_changed_intent_survives_reload() {
        let memory = Memory::default();
        let light = BenchLight::load(&memory).unwrap();
        assert!(!light.on_off());
        light.set_on_off(false);
        assert_eq!(memory.writes.get(), 0);
        light.set_on_off(true);
        assert!(light.on_off());
        assert_eq!(memory.writes.get(), 1);
        light.set_on_off(true);
        assert_eq!(memory.writes.get(), 1);
        assert!(BenchLight::load(&memory).unwrap().on_off());
        assert_eq!(*memory.data.borrow(), Some(vec![1, 1, 0, 0]));
    }

    #[test]
    fn failed_persistence_preserves_applied_intent_and_retries() {
        let memory = Memory::default();
        let light = BenchLight::load(&memory).unwrap();
        memory.fail.set(true);
        light.set_on_off(true);
        assert!(light.on_off());
        assert!(memory.data.borrow().is_none());
        memory.fail.set(false);
        // An unchanged command also retries the previously failed write.
        light.set_on_off(true);
        assert_eq!(memory.writes.get(), 1);
        assert!(BenchLight::load(&memory).unwrap().on_off());
    }

    #[test]
    fn stored_startup_toggle_is_applied_by_real_matter_handler() {
        let memory = Memory::default();
        let light = BenchLight::load(&memory).unwrap();
        light
            .set_start_up_on_off(Nullable::some(StartUpOnOffEnum::Toggle))
            .unwrap();
        let light = BenchLight::load(&memory).unwrap();
        let handler = OnOffHandler::new_standalone(Dataver::new(0), 1, &light);
        assert!(handler.on_off());
        assert!(BenchLight::load(&memory).unwrap().on_off());
        assert_eq!(
            light.start_up_on_off().into_option(),
            Some(StartUpOnOffEnum::Toggle)
        );
    }

    #[test]
    fn failed_startup_setting_is_not_reported_as_accepted() {
        let memory = Memory::default();
        let light = BenchLight::load(&memory).unwrap();
        memory.fail.set(true);
        assert!(light
            .set_start_up_on_off(Nullable::some(StartUpOnOffEnum::On))
            .is_err());
        assert_eq!(light.start_up_on_off().into_option(), None);
        assert!(memory.data.borrow().is_none());
    }

    #[test]
    fn hook_synchronizes_handler_state_at_each_transport_start() {
        let memory = Memory::default();
        let light = BenchLight::load(&memory).unwrap();
        let messages = RefCell::new(Vec::new());
        let mut run = pin!(light.run(|message| messages.borrow_mut().push(message)));
        assert!(run
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending());
        assert_eq!(*messages.borrow(), vec![OutOfBandMessage::Update]);
    }

    #[test]
    fn malformed_stored_records_are_preserved_and_rejected() {
        for data in [
            vec![],
            vec![1, 0, 0],
            vec![1, 0, 0, 0, 0],
            vec![2, 0, 0, 0],
            vec![1, 2, 0, 0],
            vec![1, 0, 2, 0],
            vec![1, 0, 0, 4],
        ] {
            let memory = Memory::default();
            *memory.data.borrow_mut() = Some(data.clone());
            assert!(BenchLight::load(&memory).is_err());
            assert_eq!(*memory.data.borrow(), Some(data));
            assert_eq!(memory.writes.get(), 0);
        }
    }
}
