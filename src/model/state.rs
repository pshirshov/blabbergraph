use std::collections::HashMap;

use crate::pw::graph::PwGraph;
use crate::pw::message::{BusId, StripId};

use super::config::AppConfig;

pub struct AppState {
    pub graph: PwGraph,
    pub config: AppConfig,
    /// Maps our logical BusId to PipeWire node id
    pub bus_node_map: HashMap<BusId, u32>,
    /// Maps our logical StripId to PipeWire node id (for virtual inputs)
    pub strip_node_map: HashMap<StripId, u32>,
    /// Reverse: PW node id -> BusId
    pub node_bus_map: HashMap<u32, BusId>,
    /// Reverse: PW node id -> StripId
    pub node_strip_map: HashMap<u32, StripId>,
    /// Whether initial restore has been triggered
    pub restore_pending: bool,
    /// Whether stale node cleanup has been done
    pub cleanup_done: bool,
    /// Save debounce source id
    pub save_timeout_id: Option<glib::SourceId>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        Self {
            graph: PwGraph::default(),
            config,
            bus_node_map: HashMap::new(),
            strip_node_map: HashMap::new(),
            node_bus_map: HashMap::new(),
            node_strip_map: HashMap::new(),
            restore_pending: true,
            cleanup_done: false,
            save_timeout_id: None,
        }
    }

    pub fn register_bus(&mut self, bus_id: BusId, node_id: u32) {
        self.bus_node_map.insert(bus_id, node_id);
        self.node_bus_map.insert(node_id, bus_id);
    }

    pub fn register_strip(&mut self, strip_id: StripId, node_id: u32) {
        self.strip_node_map.insert(strip_id, node_id);
        self.node_strip_map.insert(node_id, strip_id);
    }

    pub fn bus_node_id(&self, bus_id: BusId) -> Option<u32> {
        self.bus_node_map.get(&bus_id).copied()
    }

    pub fn strip_node_id(&self, strip_id: StripId) -> Option<u32> {
        self.strip_node_map.get(&strip_id).copied()
    }

    pub fn find_hardware_node_by_name(&self, node_name: &str) -> Option<u32> {
        self.graph
            .nodes
            .values()
            .find(|n| n.name == node_name)
            .map(|n| n.id)
    }
}
