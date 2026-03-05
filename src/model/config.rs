use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::bus::BusConfig;
use super::strip::StripConfig;
use crate::pw::message::{BusId, ChannelLayout, VirtualOutputId};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AppConfig {
    pub buses: Vec<BusConfig>,
    pub strips: Vec<StripConfig>,
    pub next_bus_id: u32,
    pub next_strip_id: u32,
    #[serde(default)]
    pub next_voutput_id: u32,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let path = config_path();
        if path.exists() {
            let data = std::fs::read_to_string(&path)?;
            let config: AppConfig = serde_json::from_str(&data)?;
            Ok(config)
        } else {
            Ok(Self::default_config())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, data)?;
        Ok(())
    }

    pub fn delete() -> Result<()> {
        let path = config_path();
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }

    fn default_config() -> Self {
        let buses = vec![
            BusConfig::new(BusId(1), "A1".to_string(), ChannelLayout::Stereo),
            BusConfig::new(BusId(2), "B1".to_string(), ChannelLayout::Stereo),
        ];

        Self {
            buses,
            strips: Vec::new(),
            next_bus_id: 3,
            next_strip_id: 1,
            next_voutput_id: 1,
        }
    }

    pub fn allocate_bus_id(&mut self) -> BusId {
        let id = BusId(self.next_bus_id);
        self.next_bus_id += 1;
        id
    }

    pub fn allocate_strip_id(&mut self) -> crate::pw::message::StripId {
        let id = crate::pw::message::StripId(self.next_strip_id);
        self.next_strip_id += 1;
        id
    }

    pub fn allocate_voutput_id(&mut self) -> VirtualOutputId {
        let id = VirtualOutputId(self.next_voutput_id);
        self.next_voutput_id += 1;
        id
    }
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .expect("No XDG config directory available")
        .join("blabbergraph")
        .join("config.json")
}
