use iced::Point;
use std::collections::HashMap;

use crate::graph::Node;

const GRID_SPACING_X: f32 = 250.0;
const GRID_SPACING_Y: f32 = 150.0;
const INITIAL_Y: f32 = 50.0;

// Column positions for different node types
const SOURCE_X: f32 = 50.0;       // Left - output only nodes
const PROCESSOR_X: f32 = 350.0;   // Middle - nodes with both
const SINK_X: f32 = 650.0;        // Right - input only nodes

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeType {
    Source,     // Only outputs
    Sink,       // Only inputs
    Processor,  // Both inputs and outputs
    Unknown,    // No ports yet
}

impl NodeType {
    pub fn from_node(node: &Node) -> Self {
        let has_inputs = !node.input_ports.is_empty();
        let has_outputs = !node.output_ports.is_empty();

        match (has_inputs, has_outputs) {
            (false, true) => NodeType::Source,
            (true, false) => NodeType::Sink,
            (true, true) => NodeType::Processor,
            (false, false) => NodeType::Unknown,
        }
    }

    pub fn base_x(&self) -> f32 {
        match self {
            NodeType::Source => SOURCE_X,
            NodeType::Sink => SINK_X,
            NodeType::Processor => PROCESSOR_X,
            NodeType::Unknown => PROCESSOR_X,
        }
    }
}

/// Calculate an automatic position for a new node
pub fn auto_position(existing_nodes: &HashMap<u32, Node>, _node_id: u32) -> Point {
    // Initially place in processor column, will be repositioned when ports are added
    let base_x = PROCESSOR_X;

    find_free_position(existing_nodes, base_x)
}

/// Calculate position for a node based on its type (call after ports are known)
pub fn position_by_type(existing_nodes: &HashMap<u32, Node>, node: &Node) -> Point {
    let node_type = NodeType::from_node(node);
    let base_x = node_type.base_x();

    find_free_position(existing_nodes, base_x)
}

fn find_free_position(existing_nodes: &HashMap<u32, Node>, base_x: f32) -> Point {
    // Find a free vertical position in the column
    for row in 0..50 {
        let candidate = Point::new(
            base_x,
            INITIAL_Y + row as f32 * GRID_SPACING_Y,
        );

        let overlaps = existing_nodes.values().any(|node| {
            let dx = (node.position.x - candidate.x).abs();
            let dy = (node.position.y - candidate.y).abs();
            dx < GRID_SPACING_X * 0.8 && dy < GRID_SPACING_Y * 0.6
        });

        if !overlaps {
            return candidate;
        }
    }

    Point::new(base_x, INITIAL_Y)
}
