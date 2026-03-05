use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BusId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StripId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VirtualOutputId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelLayout {
    Mono,
    Stereo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortDirection {
    Input,
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkState {
    Negotiating,
    Allocating,
    Paused,
    Active,
    Error,
    Unlinked,
}

#[derive(Debug)]
pub enum PwCommand {
    CreateBus {
        bus_id: BusId,
        name: String,
        channels: ChannelLayout,
    },
    DestroyBus {
        bus_id: BusId,
    },
    CreateVirtualInput {
        strip_id: StripId,
        name: String,
        channels: ChannelLayout,
    },
    DestroyVirtualInput {
        strip_id: StripId,
    },
    CreateVirtualOutput {
        voutput_id: VirtualOutputId,
        name: String,
        channels: ChannelLayout,
    },
    DestroyVirtualOutput {
        voutput_id: VirtualOutputId,
    },
    CreateLink {
        output_port_id: u32,
        input_port_id: u32,
    },
    DestroyLink {
        link_id: u32,
    },
    DestroyGlobal {
        id: u32,
    },
    SetVolume {
        node_id: u32,
        volumes: Vec<f32>,
    },
    SetMute {
        node_id: u32,
        muted: bool,
    },
    SetMonitoredNodes {
        /// (node_id, node_name, capture_sink)
        nodes: Vec<(u32, String, bool)>,
    },
    Terminate,
}

#[derive(Debug, Clone)]
pub enum PwEvent {
    NodeAdded {
        id: u32,
        name: String,
        media_class: String,
        description: String,
        properties: HashMap<String, String>,
    },
    NodeRemoved {
        id: u32,
    },
    PortAdded {
        id: u32,
        node_id: u32,
        name: String,
        direction: PortDirection,
    },
    PortRemoved {
        id: u32,
    },
    LinkAdded {
        id: u32,
        output_port_id: u32,
        input_port_id: u32,
        state: LinkState,
    },
    LinkStateChanged {
        id: u32,
        state: LinkState,
    },
    LinkRemoved {
        id: u32,
    },
    ParamsChanged {
        node_id: u32,
        volumes: Option<Vec<f32>>,
        muted: Option<bool>,
        soft_volumes: Option<Vec<f32>>,
        monitor_volumes: Option<Vec<f32>>,
    },
    BusCreated {
        bus_id: BusId,
        node_id: u32,
    },
    VirtualInputCreated {
        strip_id: StripId,
        node_id: u32,
    },
    VirtualOutputCreated {
        voutput_id: VirtualOutputId,
        node_id: u32,
    },
    PeakLevel {
        node_id: u32,
        peaks: Vec<f32>,
    },
}
