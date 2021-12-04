use std::{
    collections::BTreeMap,
    sync::{
        atomic::{self, AtomicBool},
        Arc,
    },
};

use parking_lot::RwLock;
use serde::Serialize;

use crate::config::{self, Config};

#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "kebab-case")]
pub struct BusState {
    pub devices: BTreeMap<String, Arc<DeviceState>>,
    pub coils: BTreeMap<String, Arc<CoilState>>,
}

#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "kebab-case")]
pub struct DeviceState {
    #[serde(skip)]
    pub name: String,
    #[serde(skip)]
    pub config: config::Device,
    pub version: RwLock<Option<u16>>,
    pub seen: AtomicBool,
}

impl DeviceState {
    /// Reset the state of a device
    pub fn reset(&self) {
        *self.version.write() = None;
        self.seen.store(false, atomic::Ordering::Relaxed);
    }
}

#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "kebab-case")]
pub struct CoilState {
    #[serde(skip)]
    pub name: String,
    #[serde(skip)]
    pub config: config::Coil,
    #[serde(skip)]
    pub device: Arc<DeviceState>,
    pub status: RwLock<CoilValue>,
}

impl CoilState {
    /// Reset the state of the coil
    pub fn reset(&self) {
        *self.status.write() = CoilValue::Unknown;
    }
}

#[derive(Serialize, Debug, Copy, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum CoilValue {
    On,
    Off,
    Unknown,
}

impl Default for CoilValue {
    fn default() -> Self {
        CoilValue::Unknown
    }
}

impl TryFrom<&Config> for BusState {
    type Error = anyhow::Error;

    fn try_from(config: &Config) -> Result<Self, Self::Error> {
        let devices: BTreeMap<_, _> = config
            .devices
            .iter()
            .map(|(name, device)| {
                (
                    name.to_owned(),
                    Arc::new(DeviceState {
                        name: name.clone(),
                        config: device.clone(),
                        version: RwLock::new(None),
                        seen: AtomicBool::from(false),
                    }),
                )
            })
            .collect();

        let coils_res: anyhow::Result<BTreeMap<_, _>> = config
            .coils
            .iter()
            .map(|(name, coil)| {
                let device = devices
                    .get(&coil.device)
                    .ok_or_else(|| {
                        anyhow::Error::msg(format!(
                            "coil {} is member of device {} which does not exist",
                            name, coil.device,
                        ))
                    })?
                    .clone();
                Ok((
                    name.to_owned(),
                    Arc::new(CoilState {
                        name: name.clone(),
                        config: coil.to_owned(),
                        device,
                        status: Default::default(),
                    }),
                ))
            })
            .collect();

        let coils = coils_res?;

        Ok(BusState { devices, coils })
    }
}

impl BusState {
    /// Reset the state of all devices and coils
    ///
    /// Call this after a powerloss on modbus.
    pub fn reset(&self) {
        self.devices.values().for_each(|state| state.reset());
        self.coils.values().for_each(|state| state.reset());
    }
}
