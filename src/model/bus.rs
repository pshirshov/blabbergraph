use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::pw::message::{BusId, ChannelLayout};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BusConfig {
    pub id: BusId,
    pub name: String,
    pub channels: ChannelLayout,
    #[serde(default)]
    pub output_targets: HashSet<String>,
    pub volume: Vec<f32>,
    pub muted: bool,
}

impl BusConfig {
    pub fn new(id: BusId, name: String, channels: ChannelLayout) -> Self {
        let volume = match channels {
            ChannelLayout::Mono => vec![1.0],
            ChannelLayout::Stereo => vec![1.0, 1.0],
        };
        Self {
            id,
            name,
            channels,
            output_targets: HashSet::new(),
            volume,
            muted: false,
        }
    }
}
