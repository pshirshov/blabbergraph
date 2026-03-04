use std::collections::HashMap;

use crate::pw::message::{LinkState, PortDirection};

#[derive(Clone, Debug)]
pub struct NodeInfo {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub media_class: String,
    pub volumes: Vec<f32>,
    pub muted: bool,
    pub properties: HashMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct PortInfo {
    pub id: u32,
    pub node_id: u32,
    pub name: String,
    pub direction: PortDirection,
}

#[derive(Clone, Debug)]
pub struct LinkInfo {
    pub id: u32,
    pub output_port_id: u32,
    pub input_port_id: u32,
    pub state: LinkState,
}

#[derive(Debug, Default)]
pub struct PwGraph {
    pub nodes: HashMap<u32, NodeInfo>,
    pub ports: HashMap<u32, PortInfo>,
    pub links: HashMap<u32, LinkInfo>,
    pub node_ports: HashMap<u32, Vec<u32>>,
}

impl PwGraph {
    pub fn add_node(&mut self, info: NodeInfo) {
        self.node_ports.entry(info.id).or_default();
        self.nodes.insert(info.id, info);
    }

    pub fn remove_node(&mut self, id: u32) {
        self.nodes.remove(&id);
        self.node_ports.remove(&id);
    }

    pub fn add_port(&mut self, info: PortInfo) {
        self.node_ports
            .entry(info.node_id)
            .or_default()
            .push(info.id);
        self.ports.insert(info.id, info);
    }

    pub fn remove_port(&mut self, id: u32) {
        if let Some(port) = self.ports.remove(&id) {
            if let Some(ports) = self.node_ports.get_mut(&port.node_id) {
                ports.retain(|&pid| pid != id);
            }
        }
    }

    pub fn add_link(&mut self, info: LinkInfo) {
        self.links.insert(info.id, info);
    }

    pub fn remove_link(&mut self, id: u32) {
        self.links.remove(&id);
    }

    pub fn ports_for_node(&self, node_id: u32) -> Vec<&PortInfo> {
        self.node_ports
            .get(&node_id)
            .map(|port_ids| {
                port_ids
                    .iter()
                    .filter_map(|pid| self.ports.get(pid))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn output_ports_for_node(&self, node_id: u32) -> Vec<&PortInfo> {
        self.ports_for_node(node_id)
            .into_iter()
            .filter(|p| p.direction == PortDirection::Output)
            .collect()
    }

    pub fn input_ports_for_node(&self, node_id: u32) -> Vec<&PortInfo> {
        self.ports_for_node(node_id)
            .into_iter()
            .filter(|p| p.direction == PortDirection::Input)
            .collect()
    }

    pub fn is_audio_node(&self, node_id: u32) -> bool {
        self.nodes
            .get(&node_id)
            .map(|n| n.media_class.starts_with("Audio/") || n.media_class.starts_with("Stream/"))
            .unwrap_or(false)
    }

    pub fn find_link(&self, output_port_id: u32, input_port_id: u32) -> Option<u32> {
        self.links
            .iter()
            .find(|(_, l)| l.output_port_id == output_port_id && l.input_port_id == input_port_id)
            .map(|(&id, _)| id)
    }

    pub fn audio_sink_nodes(&self) -> Vec<&NodeInfo> {
        self.nodes
            .values()
            .filter(|n| {
                n.media_class == "Audio/Sink"
                    || n.media_class == "Audio/Duplex"
            })
            .collect()
    }

    pub fn audio_source_nodes(&self) -> Vec<&NodeInfo> {
        self.nodes
            .values()
            .filter(|n| {
                n.media_class == "Audio/Source"
                    || n.media_class == "Audio/Duplex"
            })
            .collect()
    }
}
