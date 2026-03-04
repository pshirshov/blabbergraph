use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::pw::message::{BusId, ChannelLayout, StripId};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StripKind {
    HardwareInput { node_name: String },
    VirtualInput { name: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StripConfig {
    pub id: StripId,
    pub kind: StripKind,
    pub channels: ChannelLayout,
    pub volume: Vec<f32>,
    pub muted: bool,
    pub routed_to: HashSet<BusId>,
}

impl StripConfig {
    pub fn new_hardware(id: StripId, node_name: String, channels: ChannelLayout) -> Self {
        let volume = match channels {
            ChannelLayout::Mono => vec![1.0],
            ChannelLayout::Stereo => vec![1.0, 1.0],
        };
        Self {
            id,
            kind: StripKind::HardwareInput { node_name },
            channels,
            volume,
            muted: false,
            routed_to: HashSet::new(),
        }
    }

    pub fn new_virtual(id: StripId, name: String, channels: ChannelLayout) -> Self {
        let volume = match channels {
            ChannelLayout::Mono => vec![1.0],
            ChannelLayout::Stereo => vec![1.0, 1.0],
        };
        Self {
            id,
            kind: StripKind::VirtualInput { name },
            channels,
            volume,
            muted: false,
            routed_to: HashSet::new(),
        }
    }

    pub fn display_name(&self) -> &str {
        match &self.kind {
            StripKind::HardwareInput { node_name } => node_name,
            StripKind::VirtualInput { name } => name,
        }
    }

    pub fn is_virtual(&self) -> bool {
        matches!(self.kind, StripKind::VirtualInput { .. })
    }
}
