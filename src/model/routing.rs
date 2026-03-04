use crate::pw::graph::PwGraph;

/// Compute port pairs to link for routing a strip's node output ports
/// to a bus's node input ports. Matches by port name suffix (FL/FR/MONO).
pub fn compute_link_pairs(
    graph: &PwGraph,
    source_node_id: u32,
    dest_node_id: u32,
) -> Vec<(u32, u32)> {
    let source_outputs = graph.output_ports_for_node(source_node_id);
    let dest_inputs = graph.input_ports_for_node(dest_node_id);

    if source_outputs.is_empty() || dest_inputs.is_empty() {
        return Vec::new();
    }

    let mut pairs = Vec::new();

    // Try matching by channel name suffix
    for out_port in &source_outputs {
        let out_channel = channel_suffix(&out_port.name);
        for in_port in &dest_inputs {
            let in_channel = channel_suffix(&in_port.name);
            if out_channel == in_channel {
                pairs.push((out_port.id, in_port.id));
            }
        }
    }

    // If no matches by name, try positional matching (mono to all)
    if pairs.is_empty() {
        if source_outputs.len() == 1 {
            // Mono source: link to all destination inputs
            for in_port in &dest_inputs {
                pairs.push((source_outputs[0].id, in_port.id));
            }
        } else {
            // Positional 1:1 matching
            for (out_port, in_port) in source_outputs.iter().zip(dest_inputs.iter()) {
                pairs.push((out_port.id, in_port.id));
            }
        }
    }

    pairs
}

/// Find existing links between two nodes and return their link IDs
pub fn find_links_between(
    graph: &PwGraph,
    source_node_id: u32,
    dest_node_id: u32,
) -> Vec<u32> {
    let source_output_ids: Vec<u32> = graph
        .output_ports_for_node(source_node_id)
        .iter()
        .map(|p| p.id)
        .collect();
    let dest_input_ids: Vec<u32> = graph
        .input_ports_for_node(dest_node_id)
        .iter()
        .map(|p| p.id)
        .collect();

    graph
        .links
        .iter()
        .filter(|(_, link)| {
            source_output_ids.contains(&link.output_port_id)
                && dest_input_ids.contains(&link.input_port_id)
        })
        .map(|(&id, _)| id)
        .collect()
}

fn channel_suffix(port_name: &str) -> &str {
    if let Some(pos) = port_name.rfind('_') {
        &port_name[pos + 1..]
    } else {
        port_name
    }
}
