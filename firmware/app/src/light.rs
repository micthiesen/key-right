//! On/Off hooks for the simulated light. The portable controller owns intent.
//! These hooks cannot report output errors, so they are suitable only for this
//! infallible bench adapter. A real PCA9635 bridge must surface failed writes.

use core::cell::{Cell, RefCell};
use core::convert::Infallible;

use embassy_time::{Duration, Timer};
use key_right_core::{Command, Controller, LightOutput, LightState, Preset};
use rs_matter_embassy::matter::dm::clusters::app::on_off::{
    self, EffectVariantEnum, OnOffHooks, OutOfBandMessage, StartUpOnOffEnum,
};
use rs_matter_embassy::matter::dm::Cluster;
use rs_matter_embassy::matter::error::{Error, ErrorCode};
use rs_matter_embassy::matter::persist::{KvBlobStoreAccess, VENDOR_KEYS_START};
use rs_matter_embassy::matter::tlv::Nullable;
use rs_matter_embassy::matter::with;

const LIGHT_STATE_KEY: u16 = VENDOR_KEYS_START + 1;

pub const CLUSTER: Cluster<'static> = on_off::FULL_CLUSTER
    .with_revision(6)
    .with_features(on_off::Feature::LIGHTING.bits())
    .with_attrs(with!(
        required;
        on_off::AttributeId::OnOff
            | on_off::AttributeId::GlobalSceneControl
            | on_off::AttributeId::OnTime
            | on_off::AttributeId::OffWaitTime
            | on_off::AttributeId::StartUpOnOff
    ))
    .with_cmds(with!(
        on_off::CommandId::Off
            | on_off::CommandId::On
            | on_off::CommandId::Toggle
            | on_off::CommandId::OffWithEffect
            | on_off::CommandId::OnWithRecallGlobalScene
            | on_off::CommandId::OnWithTimedOff
    ));

/// Acknowledges only an in-memory application, never measured physical output.
struct SimulatedOutput;

impl LightOutput for SimulatedOutput {
    type Error = Infallible;

    fn apply(&mut self, state: LightState) -> Result<(), Self::Error> {
        log::info!(
            "mode=simulation output_ack_on={} preset={:?} nominal_brightness_percent={} physical_output=unknown",
            state.on,
            state.preset,
            state.brightness_percent()
        );
        Ok(())
    }
}

pub struct BenchLight<K> {
    controller: RefCell<Controller>,
    startup: Cell<Option<StartUpOnOffEnum>>,
    dirty: Cell<bool>,
    kv: K,
}

impl<K: KvBlobStoreAccess> BenchLight<K> {
    pub fn load(kv: K) -> Result<Self, Error> {
        let (state, startup) = kv.access(|store, buf| {
            let Some(data) = store.load(LIGHT_STATE_KEY, buf)? else {
                return Ok((LightState::default(), None));
            };
            match *data {
                [1, on @ 0..=1, preset @ 0..=1, startup @ 0..=3] => Ok((
                    LightState {
                        on: on != 0,
                        preset: if preset == 0 {
                            Preset::One
                        } else {
                            Preset::Two
                        },
                    },
                    match startup {
                        0 => None,
                        1 => Some(StartUpOnOffEnum::Off),
                        2 => Some(StartUpOnOffEnum::On),
                        _ => Some(StartUpOnOffEnum::Toggle),
                    },
                )),
                _ => Err(Error::from(ErrorCode::InvalidData)),
            }
        })?;

        let mut controller = Controller::new(Some(state));
        controller
            .reconcile(&mut SimulatedOutput)
            .expect("simulation is infallible");
        Ok(Self {
            controller: RefCell::new(controller),
            startup: Cell::new(startup),
            dirty: Cell::new(false),
            kv,
        })
    }

    fn persist(&self, startup: Option<StartUpOnOffEnum>) -> Result<(), Error> {
        let state = self.controller.borrow().intended();
        let data = [
            1, // Schema version; invalid records fail closed during startup.
            u8::from(state.on),
            match state.preset {
                Preset::One => 0,
                Preset::Two => 1,
            },
            match startup {
                None => 0,
                Some(StartUpOnOffEnum::Off) => 1,
                Some(StartUpOnOffEnum::On) => 2,
                Some(StartUpOnOffEnum::Toggle) => 3,
            },
        ];
        self.kv
            .access(|store, buf| store.store(LIGHT_STATE_KEY, &data, buf))?;
        self.dirty.set(false);
        Ok(())
    }

    fn retry_persistence(&self) {
        if self.dirty.get() {
            if let Err(error) = self.persist(self.startup.get()) {
                log::error!("light intent persistence failed: {error:?}; will retry");
            }
        }
    }
}

impl<K: KvBlobStoreAccess> OnOffHooks for BenchLight<K> {
    const CLUSTER: Cluster<'static> = CLUSTER;

    fn on_off(&self) -> bool {
        self.controller
            .borrow()
            .applied()
            .expect("simulation acknowledges every application")
            .on
    }

    fn set_on_off(&self, on: bool) {
        let changed = {
            let mut controller = self.controller.borrow_mut();
            let changed = controller.command(Command::SetPower(on));
            controller
                .reconcile(&mut SimulatedOutput)
                .expect("simulation is infallible");
            changed
        };
        if changed {
            self.dirty.set(true);
        }
        self.retry_persistence();
    }

    fn start_up_on_off(&self) -> Nullable<StartUpOnOffEnum> {
        self.startup.get().into()
    }

    fn set_start_up_on_off(&self, value: Nullable<StartUpOnOffEnum>) -> Result<(), Error> {
        let value = value.into_option();
        if value != self.startup.get() {
            self.persist(value)?;
            self.startup.set(value);
        }
        Ok(())
    }

    async fn handle_off_with_effect(&self, _effect: EffectVariantEnum) {
        // Fixed nominal brightness: the library turns off when this hook completes.
        log::info!("mode=simulation off effect completes immediately");
    }

    async fn run<F: Fn(OutOfBandMessage)>(&self, notify: F) {
        // The pinned handler snapshots power before applying StartUpOnOff.
        // Synchronize its command state machine with the resulting controller state.
        notify(OutOfBandMessage::Update);
        loop {
            Timer::after(Duration::from_secs(5)).await;
            self.retry_persistence();
        }
    }
}
