use iced::mouse;
use iced::widget::canvas::{self, Cache, Frame, Geometry, Path, Stroke, Text};
use iced::{Color, Point, Rectangle, Size, Vector};
use std::collections::HashMap;

use crate::config::{Config, NodeKey, Position};
use crate::layout;
use crate::pipewire_client::PipewireEvent;
use crate::Message;

pub const NODE_WIDTH: f32 = 180.0;
pub const NODE_HEADER_HEIGHT: f32 = 28.0;
pub const PORT_HEIGHT: f32 = 22.0;
pub const PORT_RADIUS: f32 = 6.0;
pub const PORT_SPACING: f32 = 4.0;

#[derive(Debug, Clone)]
pub enum GraphMessage {
    NodeDragged { node_id: u32, delta: Vector },
    NodeDragEnded { node_id: u32 },
    ConnectionStarted { node_id: u32, port_id: u32 },
    ConnectionEnded {
        from_node: u32,
        from_port: u32,
        to_node: u32,
        to_port: u32
    },
    ConnectionCancelled,
    DisconnectLink { link_id: u32, output_port: u32, input_port: u32 },
    Pan(Vector),
    Zoom { delta: f32, cursor: Point },
    AutoLayout,
    Undo,
    Redo,
    ToggleHelp,
    // Search
    SearchActivate,
    SearchInput { text: String },
    SearchBackspace,
    SearchClear,
    SearchCommit,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: u32,
    pub name: String,
    pub app_name: Option<String>,
    pub serial: Option<String>,
    pub object_path: Option<String>,
    pub index: u32,
    pub position: Point,
    pub has_saved_position: bool,
    pub input_ports: Vec<Port>,
    pub output_ports: Vec<Port>,
    /// Custom display name (from config or rename)
    pub custom_name: Option<String>,
    /// Node source (PipeWire or ALSA MIDI)
    pub source: NodeSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum NodeSource {
    #[default]
    PipeWire,
    AlsaMidi,
}

#[derive(Debug, Clone)]
pub struct Port {
    pub id: u32,
    pub name: String,
    pub direction: PortDirection,
    pub port_type: PortType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PortDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PortType {
    #[default]
    Audio,
    Midi,
    Video,
}

#[derive(Debug, Clone)]
pub struct Link {
    pub id: u32,
    pub output_node: u32,
    pub output_port: u32,
    pub input_node: u32,
    pub input_port: u32,
}

#[derive(Debug, Clone)]
pub enum UndoAction {
    Connect { output_port: u32, input_port: u32 },
    Disconnect { output_port: u32, input_port: u32 },
}

pub struct Graph {
    pub nodes: HashMap<u32, Node>,
    pub links: Vec<Link>,
    pub pan_offset: Vector,
    pub zoom: f32,
    cache: Cache,
    undo_stack: Vec<UndoAction>,
    redo_stack: Vec<UndoAction>,
    pub show_help: bool,

    // Search/filter state
    pub search_query: String,
    pub search_active: bool,
    pub filtered_nodes: std::collections::HashSet<u32>,

    // Preset state
    pub current_preset: Option<crate::preset::Preset>,
    pub preset_path: Option<std::path::PathBuf>,
    pub exclusive_mode: bool,

    // Node renaming state
    pub renaming_node: Option<u32>,
    pub rename_text: String,

    // Pinned connections (output_port_id, input_port_id)
    pub pinned_connections: std::collections::HashSet<(u32, u32)>,
}

impl Graph {
    pub fn new(config: &Config) -> Self {
        Self {
            nodes: HashMap::new(),
            links: Vec::new(),
            pan_offset: Vector::ZERO,
            zoom: 1.0,
            cache: Cache::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            show_help: false,
            search_query: String::new(),
            search_active: false,
            filtered_nodes: std::collections::HashSet::new(),
            current_preset: None,
            preset_path: None,
            exclusive_mode: config.exclusive_mode,
            renaming_node: None,
            rename_text: String::new(),
            pinned_connections: std::collections::HashSet::new(),
        }
    }

    pub fn update(&mut self, message: GraphMessage, config: &mut Config) {
        match message {
            GraphMessage::NodeDragged { node_id, delta } => {
                if let Some(node) = self.nodes.get_mut(&node_id) {
                    node.position = node.position + delta / self.zoom;
                    self.cache.clear();
                }
            }
            GraphMessage::NodeDragEnded { node_id } => {
                if let Some(node) = self.nodes.get_mut(&node_id) {
                    node.has_saved_position = true;
                    let key = NodeKey {
                        node_name: node.name.clone(),
                        app_name: node.app_name.clone(),
                        object_path: node.object_path.clone(),
                        index: Some(node.index),
                    };
                    config.set_position(
                        key,
                        Position {
                            x: node.position.x,
                            y: node.position.y,
                        },
                    );
                }
            }
            GraphMessage::ConnectionStarted { .. } => {
                // Visual feedback handled in draw
            }
            GraphMessage::ConnectionEnded { from_node, from_port, to_node: _, to_port } => {
                // Determine which is output and which is input
                let (output_port, input_port) = {
                    let from_is_output = self.nodes.get(&from_node)
                        .map(|n| n.output_ports.iter().any(|p| p.id == from_port))
                        .unwrap_or(false);

                    if from_is_output {
                        (from_port, to_port)
                    } else {
                        (to_port, from_port)
                    }
                };

                // Create connection and track for undo
                crate::pipewire_connect(output_port, input_port);
                self.undo_stack.push(UndoAction::Connect { output_port, input_port });
                self.redo_stack.clear(); // Clear redo on new action
            }
            GraphMessage::ConnectionCancelled => {
                self.cache.clear();
            }
            GraphMessage::DisconnectLink { link_id: _, output_port, input_port } => {
                // Disconnect and track for undo
                crate::pipewire_disconnect(output_port, input_port);
                self.undo_stack.push(UndoAction::Disconnect { output_port, input_port });
                self.redo_stack.clear(); // Clear redo on new action
            }
            GraphMessage::Pan(delta) => {
                self.pan_offset = self.pan_offset + delta;
                self.cache.clear();
            }
            GraphMessage::Zoom { delta, cursor } => {
                let old_zoom = self.zoom;
                self.zoom = (self.zoom * (1.0 + delta * 0.1)).clamp(0.25, 4.0);

                // Zoom towards cursor
                let cursor_world_x = (cursor.x - self.pan_offset.x) / old_zoom;
                let cursor_world_y = (cursor.y - self.pan_offset.y) / old_zoom;
                self.pan_offset.x = cursor.x - cursor_world_x * self.zoom;
                self.pan_offset.y = cursor.y - cursor_world_y * self.zoom;
                self.cache.clear();
            }
            GraphMessage::AutoLayout => {
                self.perform_auto_layout();
                self.cache.clear();
            }
            GraphMessage::Undo => {
                if let Some(action) = self.undo_stack.pop() {
                    match &action {
                        UndoAction::Connect { output_port, input_port } => {
                            // Undo a connect = disconnect
                            crate::pipewire_disconnect(*output_port, *input_port);
                        }
                        UndoAction::Disconnect { output_port, input_port } => {
                            // Undo a disconnect = reconnect
                            crate::pipewire_connect(*output_port, *input_port);
                        }
                    }
                    // Push inverse action to redo stack
                    let inverse = match action {
                        UndoAction::Connect { output_port, input_port } =>
                            UndoAction::Disconnect { output_port, input_port },
                        UndoAction::Disconnect { output_port, input_port } =>
                            UndoAction::Connect { output_port, input_port },
                    };
                    self.redo_stack.push(inverse);
                }
            }
            GraphMessage::Redo => {
                if let Some(action) = self.redo_stack.pop() {
                    match &action {
                        UndoAction::Connect { output_port, input_port } => {
                            crate::pipewire_disconnect(*output_port, *input_port);
                        }
                        UndoAction::Disconnect { output_port, input_port } => {
                            crate::pipewire_connect(*output_port, *input_port);
                        }
                    }
                    let inverse = match action {
                        UndoAction::Connect { output_port, input_port } =>
                            UndoAction::Disconnect { output_port, input_port },
                        UndoAction::Disconnect { output_port, input_port } =>
                            UndoAction::Connect { output_port, input_port },
                    };
                    self.undo_stack.push(inverse);
                }
            }
            GraphMessage::ToggleHelp => {
                self.show_help = !self.show_help;
                self.cache.clear();
            }
            GraphMessage::SearchActivate => {
                self.search_active = true;
                self.search_query.clear();
                self.filtered_nodes.clear();
                self.cache.clear();
            }
            GraphMessage::SearchInput { text } => {
                self.search_active = true;
                self.search_query.push_str(&text);
                self.update_search_filter();
                self.cache.clear();
            }
            GraphMessage::SearchBackspace => {
                self.search_query.pop();
                self.update_search_filter();
                self.cache.clear();
            }
            GraphMessage::SearchClear => {
                self.search_active = false;
                self.search_query.clear();
                self.filtered_nodes.clear();
                self.cache.clear();
            }
            GraphMessage::SearchCommit => {
                // Focus on first matching node
                if let Some(&node_id) = self.filtered_nodes.iter().next() {
                    if let Some(node) = self.nodes.get(&node_id) {
                        // Pan to center the node
                        self.pan_offset = Vector::new(
                            -node.position.x * self.zoom + 400.0,
                            -node.position.y * self.zoom + 300.0,
                        );
                    }
                }
                self.search_active = false;
                self.search_query.clear();
                self.filtered_nodes.clear();
                self.cache.clear();
            }
        }
    }

    /// Update the filtered nodes based on search query
    fn update_search_filter(&mut self) {
        self.filtered_nodes.clear();
        if self.search_query.is_empty() {
            return;
        }

        let query_lower = self.search_query.to_lowercase();
        for (&id, node) in &self.nodes {
            let display_name = node.custom_name.as_ref().unwrap_or(&node.name);
            if display_name.to_lowercase().contains(&query_lower) {
                self.filtered_nodes.insert(id);
            }
        }
    }

    /// Auto-layout: align connected nodes horizontally, isolate unconnected nodes
    fn perform_auto_layout(&mut self) {
        use std::collections::{HashMap, HashSet, VecDeque};

        const COL_WIDTH: f32 = 250.0;
        const START_X: f32 = 50.0;
        const START_Y: f32 = 50.0;
        const ROW_GAP: f32 = 25.0;  // Vertical spacing between nodes
        const ISOLATED_X: f32 = 50.0;
        const ISOLATED_GAP: f32 = 150.0;  // Extra gap between isolated and connected nodes

        // Reset all saved positions - L does a full re-layout
        for node in self.nodes.values_mut() {
            node.has_saved_position = false;
        }

        // Build connection maps
        let mut outgoing: HashMap<u32, Vec<u32>> = HashMap::new();
        let mut incoming: HashMap<u32, Vec<u32>> = HashMap::new();
        for link in &self.links {
            outgoing.entry(link.output_node).or_default().push(link.input_node);
            incoming.entry(link.input_node).or_default().push(link.output_node);
        }

        // Identify connected nodes (involved in at least one link)
        let mut connected_nodes: HashSet<u32> = HashSet::new();
        for link in &self.links {
            connected_nodes.insert(link.output_node);
            connected_nodes.insert(link.input_node);
        }

        // Separate isolated nodes (no connections at all)
        let mut isolated_nodes: Vec<u32> = Vec::new();
        for &id in self.nodes.keys() {
            if !connected_nodes.contains(&id) {
                isolated_nodes.push(id);
            }
        }
        isolated_nodes.sort();

        // Place isolated nodes in a column on the left, stacked vertically
        let mut isolated_y = START_Y;
        for &id in &isolated_nodes {
            if let Some(node) = self.nodes.get_mut(&id) {
                if !node.has_saved_position {
                    node.position = Point::new(ISOLATED_X, isolated_y);
                    isolated_y += Self::node_height(node) + ROW_GAP;
                }
            }
        }

        // Calculate the X offset for connected nodes (shift right if there are isolated nodes)
        let connected_start_x = if isolated_nodes.is_empty() {
            START_X
        } else {
            START_X + COL_WIDTH + ISOLATED_GAP  // Shift connected graph further right
        };

        // Classify connected nodes by ACTUAL connections (not just ports)
        let mut sources: Vec<u32> = Vec::new();
        let mut sinks: Vec<u32> = Vec::new();
        let mut processors: Vec<u32> = Vec::new();

        for &id in &connected_nodes {
            let has_incoming = incoming.contains_key(&id);
            let has_outgoing = outgoing.contains_key(&id);

            match (has_incoming, has_outgoing) {
                (false, true) => sources.push(id),   // Only outputs = source
                (true, false) => sinks.push(id),     // Only inputs = sink
                (true, true) => processors.push(id), // Both = processor
                (false, false) => {} // No connections (shouldn't happen for connected_nodes)
            }
        }
        sources.sort();
        sinks.sort();

        // Assign columns: Sources=0, Processors=BFS depth, Sinks=rightmost
        let mut node_col: HashMap<u32, usize> = HashMap::new();

        // Sources always column 0
        for &src in &sources {
            node_col.insert(src, 0);
        }

        // BFS to assign processor columns (starting from column 1)
        let mut queue: VecDeque<(u32, usize)> = VecDeque::new();
        for &src in &sources {
            queue.push_back((src, 0));
        }

        while let Some((node, col)) = queue.pop_front() {
            if let Some(targets) = outgoing.get(&node) {
                for &target in targets {
                    // Only assign BFS column to processors (not sinks)
                    if processors.contains(&target) {
                        let new_col = col + 1;
                        if new_col > node_col.get(&target).copied().unwrap_or(0) {
                            node_col.insert(target, new_col);
                            queue.push_back((target, new_col));
                        }
                    }
                }
            }
        }

        // Assign unvisited processors to column 1
        for &id in &processors {
            node_col.entry(id).or_insert(1);
        }

        // Find max processor column
        let max_processor_col = node_col.values().copied().max().unwrap_or(0);

        // Sinks go to rightmost column (max + 1)
        let sink_col = max_processor_col + 1;
        for &sink in &sinks {
            node_col.insert(sink, sink_col);
        }

        let max_col = sink_col;

        // Track which Y slots are used per column
        let mut col_slots: HashMap<usize, Vec<(f32, f32)>> = HashMap::new();
        let mut node_y: HashMap<u32, f32> = HashMap::new();

        // First pass: temporarily place sources to compute downstream positions
        let mut y = START_Y;
        for &src in &sources {
            let height = self.nodes.get(&src).map(|n| Self::node_height(n)).unwrap_or(80.0);
            node_y.insert(src, y);
            y += height + ROW_GAP;
        }

        // Compute initial Y positions for all non-source nodes
        for col in 1..=max_col {
            let mut col_nodes: Vec<u32> = node_col.iter()
                .filter(|&(_, &c)| c == col)
                .map(|(&id, _)| id)
                .collect();

            // Compute desired Y for each node (average Y of inputs from previous column)
            let mut node_desired: Vec<(u32, f32)> = col_nodes.iter().map(|&id| {
                let desired = incoming.get(&id).map(|ins| {
                    // Get all inputs from immediately previous column
                    let prev_col_ys: Vec<f32> = ins.iter()
                        .filter(|&&input_id| node_col.get(&input_id) == Some(&(col - 1)))
                        .filter_map(|&input_id| node_y.get(&input_id).copied())
                        .collect();

                    if !prev_col_ys.is_empty() {
                        prev_col_ys.iter().sum::<f32>() / prev_col_ys.len() as f32
                    } else {
                        ins.iter().filter_map(|&i| node_y.get(&i).copied()).next().unwrap_or(START_Y)
                    }
                }).unwrap_or(START_Y);
                (id, desired)
            }).collect();

            // Sort by desired Y
            node_desired.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

            let slots = col_slots.entry(col).or_default();
            for (id, desired_y) in node_desired {
                let height = self.nodes.get(&id).map(|n| Self::node_height(n)).unwrap_or(80.0);
                let final_y = Self::find_free_y(desired_y, height, slots, ROW_GAP, START_Y);
                node_y.insert(id, final_y);
                slots.push((final_y, height));
            }
        }

        // Store first-pass Y positions
        let first_pass_y: HashMap<u32, f32> = node_y.clone();

        // Reposition sources based on median Y of their outputs
        node_y.clear();
        col_slots.clear();

        let mut source_desired: Vec<(u32, f32, f32)> = Vec::new();
        for &src in &sources {
            let height = self.nodes.get(&src).map(|n| Self::node_height(n)).unwrap_or(80.0);
            let outputs = outgoing.get(&src).cloned().unwrap_or_default();
            let median_y = if !outputs.is_empty() {
                let mut ys: Vec<f32> = outputs.iter()
                    .filter_map(|&out| first_pass_y.get(&out).copied())
                    .collect();
                if !ys.is_empty() {
                    ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                    let mid = ys.len() / 2;
                    if ys.len() % 2 == 0 && mid > 0 {
                        (ys[mid - 1] + ys[mid]) / 2.0
                    } else {
                        ys[mid]
                    }
                } else {
                    START_Y
                }
            } else {
                START_Y
            };
            source_desired.push((src, median_y, height));
        }

        source_desired.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let source_slots = col_slots.entry(0).or_default();
        for (src, desired_y, height) in &source_desired {
            let final_y = Self::find_free_y(*desired_y, *height, source_slots, ROW_GAP, START_Y);
            node_y.insert(*src, final_y);
            source_slots.push((final_y, *height));
            source_slots.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        }

        // Second pass: recompute downstream positions
        col_slots.retain(|&k, _| k == 0);

        for col in 1..=max_col {
            let col_nodes: Vec<u32> = node_col.iter()
                .filter(|&(_, &c)| c == col)
                .map(|(&id, _)| id)
                .collect();

            // Compute desired Y and output group for each node
            let mut node_desired: Vec<(u32, f32, u32)> = col_nodes.iter().map(|&id| {
                let desired = incoming.get(&id).map(|ins| {
                    // Get all inputs from immediately previous column
                    let prev_col_ys: Vec<f32> = ins.iter()
                        .filter(|&&input_id| node_col.get(&input_id) == Some(&(col - 1)))
                        .filter_map(|&input_id| node_y.get(&input_id).copied())
                        .collect();

                    if !prev_col_ys.is_empty() {
                        prev_col_ys.iter().sum::<f32>() / prev_col_ys.len() as f32
                    } else {
                        ins.iter().filter_map(|&i| node_y.get(&i).copied()).next().unwrap_or(START_Y)
                    }
                }).unwrap_or(START_Y);

                // Get first output destination as group key (for grouping nodes with same output)
                let output_group = outgoing.get(&id)
                    .and_then(|outs| outs.first().copied())
                    .unwrap_or(u32::MAX);

                (id, desired, output_group)
            }).collect();

            // Sort by: 1) output group (to cluster nodes with same destination)
            //          2) desired Y within group
            node_desired.sort_by(|a, b| {
                // First compare output groups
                match a.2.cmp(&b.2) {
                    std::cmp::Ordering::Equal => {
                        // Same output group - sort by desired Y
                        a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
                    }
                    other => other
                }
            });

            let slots = col_slots.entry(col).or_default();
            for (id, desired_y, _) in node_desired {
                let height = self.nodes.get(&id).map(|n| Self::node_height(n)).unwrap_or(80.0);
                let final_y = Self::find_free_y(desired_y, height, slots, ROW_GAP, START_Y);
                node_y.insert(id, final_y);
                slots.push((final_y, height));
                slots.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            }
        }

        // Third pass: reposition sources one more time based on FINAL output positions
        // This minimizes line length after downstream nodes have been positioned
        let mut final_source_desired: Vec<(u32, f32, f32)> = sources.iter().map(|&src| {
            let height = self.nodes.get(&src).map(|n| Self::node_height(n)).unwrap_or(80.0);
            let outputs = outgoing.get(&src).cloned().unwrap_or_default();
            let target_y = if !outputs.is_empty() {
                // Use average Y of outputs (which are now in final positions)
                let sum: f32 = outputs.iter()
                    .filter_map(|&out| node_y.get(&out).copied())
                    .sum();
                let count = outputs.iter()
                    .filter(|&out| node_y.contains_key(out))
                    .count();
                if count > 0 { sum / count as f32 } else { START_Y }
            } else {
                START_Y
            };
            (src, target_y, height)
        }).collect();

        final_source_desired.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Clear and rebuild source positions
        let source_slots_final: &mut Vec<(f32, f32)> = col_slots.entry(0).or_default();
        source_slots_final.clear();

        for (src, desired_y, height) in final_source_desired {
            let final_y = Self::find_free_y(desired_y, height, source_slots_final, ROW_GAP, START_Y);
            node_y.insert(src, final_y);
            source_slots_final.push((final_y, height));
            source_slots_final.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        }

        // Apply positions to connected nodes
        for (&id, &col) in &node_col {
            if let Some(node) = self.nodes.get_mut(&id) {
                if !node.has_saved_position {
                    let y = node_y.get(&id).copied().unwrap_or(START_Y);
                    node.position = Point::new(connected_start_x + col as f32 * COL_WIDTH, y);
                }
            }
        }
    }

    /// Find a free Y position near the desired Y that doesn't overlap existing slots
    fn find_free_y(desired: f32, height: f32, slots: &[(f32, f32)], gap: f32, min_y: f32) -> f32 {
        if slots.is_empty() {
            return desired.max(min_y);
        }

        // Check if desired position works
        let overlaps = |y: f32| -> bool {
            for &(slot_y, slot_h) in slots {
                let top1 = y;
                let bot1 = y + height;
                let top2 = slot_y;
                let bot2 = slot_y + slot_h;
                if top1 < bot2 + gap && bot1 + gap > top2 {
                    return true;
                }
            }
            false
        };

        if !overlaps(desired) && desired >= min_y {
            return desired;
        }

        // Search for free slot in small increments (gap/2) to stay as close as possible
        let search_step = (gap / 2.0).max(5.0);
        for offset in 1..500 {
            let step = search_step * offset as f32;

            // Try below first (more natural flow)
            let try_below = desired + step;
            if !overlaps(try_below) {
                return try_below;
            }

            // Try above
            let try_above = desired - step;
            if try_above >= min_y && !overlaps(try_above) {
                return try_above;
            }
        }

        // Fallback: place at the bottom
        slots.iter()
            .map(|&(y, h)| y + h + gap)
            .fold(min_y, f32::max)
    }

    pub fn handle_pipewire_event(&mut self, event: PipewireEvent, config: &Config) {
        match event {
            PipewireEvent::NodeAdded { id, name, app_name, serial, object_path } => {
                // Count how many nodes with same name/app/path already exist (for indexing duplicates)
                let index = self.nodes.values()
                    .filter(|n| n.name == name && n.app_name == app_name && n.object_path == object_path)
                    .count() as u32;

                let key = NodeKey {
                    node_name: name.clone(),
                    app_name: app_name.clone(),
                    object_path: object_path.clone(),
                    index: Some(index),
                };

                let (base_position, has_saved_position) = config
                    .get_position(&key)
                    .map(|p| (Point::new(p.x, p.y), true))
                    .unwrap_or_else(|| (layout::auto_position(&self.nodes, id), false));

                // Get custom name from config if set
                let custom_name = config.get_node_rename(&key).cloned();

                // Offset if another node is already at this position
                let position = self.find_non_overlapping_position(base_position);

                self.nodes.insert(
                    id,
                    Node {
                        id,
                        name,
                        app_name,
                        serial,
                        object_path,
                        index,
                        position,
                        has_saved_position,
                        input_ports: Vec::new(),
                        output_ports: Vec::new(),
                        custom_name,
                        source: NodeSource::PipeWire,
                    },
                );
                self.cache.clear();
            }
            PipewireEvent::NodeRemoved { id } => {
                self.nodes.remove(&id);
                self.links.retain(|l| l.output_node != id && l.input_node != id);
                self.cache.clear();
            }
            PipewireEvent::PortAdded {
                node_id,
                port_id,
                name,
                direction,
                port_type,
            } => {
                // Check if this is the first port and node needs repositioning
                let should_reposition = self.nodes.get(&node_id)
                    .map(|n| !n.has_saved_position && n.input_ports.is_empty() && n.output_ports.is_empty())
                    .unwrap_or(false);

                // Add the port first
                if let Some(node) = self.nodes.get_mut(&node_id) {
                    let port = Port {
                        id: port_id,
                        name,
                        direction,
                        port_type,
                    };
                    match direction {
                        PortDirection::Input => node.input_ports.push(port),
                        PortDirection::Output => node.output_ports.push(port),
                    }
                }

                // Reposition based on node type (source/sink/processor)
                if should_reposition {
                    if let Some(node) = self.nodes.get(&node_id).cloned() {
                        let new_pos = layout::position_by_type(&self.nodes, &node);
                        let final_pos = self.find_non_overlapping_position(new_pos);
                        if let Some(node) = self.nodes.get_mut(&node_id) {
                            node.position = final_pos;
                        }
                    }
                }

                self.cache.clear();
            }
            PipewireEvent::PortRemoved { node_id, port_id } => {
                if let Some(node) = self.nodes.get_mut(&node_id) {
                    node.input_ports.retain(|p| p.id != port_id);
                    node.output_ports.retain(|p| p.id != port_id);
                    self.cache.clear();
                }
            }
            PipewireEvent::LinkAdded {
                id,
                output_node,
                output_port,
                input_node,
                input_port,
            } => {
                self.links.push(Link {
                    id,
                    output_node,
                    output_port,
                    input_node,
                    input_port,
                });
                self.cache.clear();
            }
            PipewireEvent::LinkRemoved { id } => {
                self.links.retain(|l| l.id != id);
                self.cache.clear();
            }
        }
    }

    fn node_height(node: &Node) -> f32 {
        let port_count = node.input_ports.len().max(node.output_ports.len());
        NODE_HEADER_HEIGHT + (port_count as f32 * (PORT_HEIGHT + PORT_SPACING)) + PORT_SPACING
    }

    fn port_position(node: &Node, port: &Port) -> Point {
        let ports = match port.direction {
            PortDirection::Input => &node.input_ports,
            PortDirection::Output => &node.output_ports,
        };
        let index = ports.iter().position(|p| p.id == port.id).unwrap_or(0);
        let x = match port.direction {
            PortDirection::Input => node.position.x,
            PortDirection::Output => node.position.x + NODE_WIDTH,
        };
        let y = node.position.y + NODE_HEADER_HEIGHT + PORT_SPACING + (index as f32 * (PORT_HEIGHT + PORT_SPACING)) + PORT_HEIGHT / 2.0;
        Point::new(x, y)
    }

    pub fn hit_test(&self, point: Point) -> HitResult {
        let world_point = self.screen_to_world(point);

        // Larger hit radius for ports (easier to click)
        const PORT_HIT_RADIUS: f32 = 15.0;

        // Check ports FIRST across all nodes (ports are on edges, may be outside node bounds)
        for node in self.nodes.values() {
            for port in node.input_ports.iter().chain(node.output_ports.iter()) {
                let port_pos = Self::port_position(node, port);
                let dist = ((world_point.x - port_pos.x).powi(2) + (world_point.y - port_pos.y).powi(2)).sqrt();
                if dist < PORT_HIT_RADIUS {
                    return HitResult::Port { node_id: node.id, port_id: port.id };
                }
            }
        }

        // Then check node bodies
        for node in self.nodes.values() {
            let height = Self::node_height(node);
            let bounds = Rectangle::new(node.position, Size::new(NODE_WIDTH, height));
            if bounds.contains(world_point) {
                return HitResult::Node(node.id);
            }
        }

        // Check links (sample points along bezier curve)
        for link in &self.links {
            if let Some(dist) = self.distance_to_link(world_point, link) {
                if dist < 8.0 {
                    return HitResult::Link {
                        link_id: link.id,
                        output_port: link.output_port,
                        input_port: link.input_port,
                    };
                }
            }
        }

        HitResult::None
    }

    fn screen_to_world(&self, point: Point) -> Point {
        Point::new(
            (point.x - self.pan_offset.x) / self.zoom,
            (point.y - self.pan_offset.y) / self.zoom,
        )
    }

    fn distance_to_link(&self, point: Point, link: &Link) -> Option<f32> {
        let out_node = self.nodes.get(&link.output_node)?;
        let in_node = self.nodes.get(&link.input_node)?;
        let out_port = out_node.output_ports.iter().find(|p| p.id == link.output_port)?;
        let in_port = in_node.input_ports.iter().find(|p| p.id == link.input_port)?;

        let start = Self::port_position(out_node, out_port);
        let end = Self::port_position(in_node, in_port);
        let control_offset = ((end.x - start.x).abs() / 2.0).max(60.0);
        let ctrl1 = Point::new(start.x + control_offset, start.y);
        let ctrl2 = Point::new(end.x - control_offset, end.y);

        // Sample points along the bezier curve
        let mut min_dist = f32::MAX;
        for i in 0..=20 {
            let t = i as f32 / 20.0;
            let bezier_point = Self::cubic_bezier(start, ctrl1, ctrl2, end, t);
            let dist = ((point.x - bezier_point.x).powi(2) + (point.y - bezier_point.y).powi(2)).sqrt();
            min_dist = min_dist.min(dist);
        }
        Some(min_dist)
    }

    fn cubic_bezier(p0: Point, p1: Point, p2: Point, p3: Point, t: f32) -> Point {
        let t2 = t * t;
        let t3 = t2 * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;
        let mt3 = mt2 * mt;

        Point::new(
            mt3 * p0.x + 3.0 * mt2 * t * p1.x + 3.0 * mt * t2 * p2.x + t3 * p3.x,
            mt3 * p0.y + 3.0 * mt2 * t * p1.y + 3.0 * mt * t2 * p2.y + t3 * p3.y,
        )
    }

    fn find_non_overlapping_position(&self, mut pos: Point) -> Point {
        const OFFSET: f32 = 30.0;
        let mut attempts = 0;
        while attempts < 20 {
            let overlaps = self.nodes.values().any(|node| {
                let dx = (node.position.x - pos.x).abs();
                let dy = (node.position.y - pos.y).abs();
                dx < NODE_WIDTH * 0.5 && dy < NODE_HEADER_HEIGHT * 2.0
            });
            if !overlaps {
                break;
            }
            pos.x += OFFSET;
            pos.y += OFFSET;
            attempts += 1;
        }
        pos
    }
}

#[derive(Debug, Clone, Copy)]
pub enum HitResult {
    None,
    Node(u32),
    Port { node_id: u32, port_id: u32 },
    Link { link_id: u32, output_port: u32, input_port: u32 },
}

impl canvas::Program<Message> for Graph {
    type State = Interaction;

    fn draw(
        &self,
        state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let content = self.cache.draw(renderer, bounds.size(), |frame| {
            // Background
            frame.fill_rectangle(
                Point::ORIGIN,
                bounds.size(),
                Color::from_rgb(0.075, 0.075, 0.085),
            );

            // Subtle dot grid pattern
            let grid_size = 40.0 * self.zoom;
            let dot_color = Color::from_rgba(1.0, 1.0, 1.0, 0.04);
            let offset_x = self.pan_offset.x % grid_size;
            let offset_y = self.pan_offset.y % grid_size;

            let cols = (bounds.width / grid_size) as i32 + 2;
            let rows = (bounds.height / grid_size) as i32 + 2;

            for row in 0..rows {
                for col in 0..cols {
                    let x = offset_x + col as f32 * grid_size;
                    let y = offset_y + row as f32 * grid_size;
                    let dot = Path::circle(Point::new(x, y), 1.0);
                    frame.fill(&dot, dot_color);
                }
            }

            frame.translate(self.pan_offset);
            frame.scale(self.zoom);

            // Draw links
            for link in &self.links {
                let output_node = self.nodes.get(&link.output_node);
                let input_node = self.nodes.get(&link.input_node);

                if let (Some(out_node), Some(in_node)) = (output_node, input_node) {
                    let out_port = out_node.output_ports.iter().find(|p| p.id == link.output_port);
                    let in_port = in_node.input_ports.iter().find(|p| p.id == link.input_port);

                    if let (Some(out_port), Some(_in_port)) = (out_port, in_port) {
                        let start = Self::port_position(out_node, out_port);
                        let end = Self::port_position(in_node, _in_port);
                        // Use output port's type for link color
                        draw_bezier_link(frame, start, end, out_port.port_type);
                    }
                }
            }

            // Draw nodes
            for node in self.nodes.values() {
                // Dim nodes that don't match search filter
                let dimmed = self.search_active && !self.search_query.is_empty()
                    && !self.filtered_nodes.contains(&node.id);
                draw_node(frame, node, dimmed);
            }
        });

        // Draw pending connection (not cached - follows cursor)
        let pending = Frame::new(renderer, bounds.size());
        let pending_geo = if let Interaction::CreatingConnection { from_node, from_port } = *state {
            if let Some(cursor_pos) = cursor.position_in(bounds) {
                let mut frame = Frame::new(renderer, bounds.size());
                frame.translate(self.pan_offset);
                frame.scale(self.zoom);

                // Find the source port position
                if let Some(node) = self.nodes.get(&from_node) {
                    let port = node.input_ports.iter()
                        .chain(node.output_ports.iter())
                        .find(|p| p.id == from_port);

                    if let Some(port) = port {
                        let start = Self::port_position(node, port);
                        let end = self.screen_to_world(cursor_pos);

                        // Draw with a pulsing/dashed style
                        draw_pending_link(&mut frame, start, end, port.direction, port.port_type);
                    }
                }
                frame.into_geometry()
            } else {
                pending.into_geometry()
            }
        } else {
            pending.into_geometry()
        };

        // Help overlay
        let help_geo = if self.show_help {
            let mut frame = Frame::new(renderer, bounds.size());
            draw_help_overlay(&mut frame, bounds.size());
            frame.into_geometry()
        } else {
            Frame::new(renderer, bounds.size()).into_geometry()
        };

        // Search overlay
        let search_geo = if self.search_active {
            let mut frame = Frame::new(renderer, bounds.size());
            draw_search_overlay(&mut frame, bounds.size(), &self.search_query, self.filtered_nodes.len());
            frame.into_geometry()
        } else {
            Frame::new(renderer, bounds.size()).into_geometry()
        };

        vec![content, pending_geo, help_geo, search_geo]
    }

    fn update(
        &self,
        state: &mut Self::State,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let cursor_position = cursor.position_in(bounds)?;

        match event {
            iced::Event::Mouse(mouse_event) => match mouse_event {
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    let hit = self.hit_test(cursor_position);
                    match hit {
                        HitResult::Port { node_id, port_id } => {
                            *state = Interaction::CreatingConnection { from_node: node_id, from_port: port_id };
                            Some(canvas::Action::publish(Message::Graph(
                                GraphMessage::ConnectionStarted { node_id, port_id }
                            )))
                        }
                        HitResult::Node(node_id) => {
                            *state = Interaction::Dragging { node_id, last_pos: cursor_position };
                            Some(canvas::Action::request_redraw())
                        }
                        HitResult::Link { .. } | HitResult::None => {
                            *state = Interaction::Panning { last_pos: cursor_position };
                            Some(canvas::Action::request_redraw())
                        }
                    }
                }
                mouse::Event::ButtonPressed(mouse::Button::Right) => {
                    let hit = self.hit_test(cursor_position);
                    if let HitResult::Link { link_id, output_port, input_port } = hit {
                        Some(canvas::Action::publish(Message::Graph(
                            GraphMessage::DisconnectLink { link_id, output_port, input_port }
                        )))
                    } else {
                        None
                    }
                }
                mouse::Event::ButtonReleased(mouse::Button::Left) => {
                    let action = match *state {
                        Interaction::Dragging { node_id, .. } => {
                            Some(canvas::Action::publish(Message::Graph(
                                GraphMessage::NodeDragEnded { node_id }
                            )))
                        }
                        Interaction::CreatingConnection { from_node, from_port } => {
                            let hit = self.hit_test(cursor_position);
                            if let HitResult::Port { node_id, port_id } = hit {
                                Some(canvas::Action::publish(Message::Graph(
                                    GraphMessage::ConnectionEnded {
                                        from_node,
                                        from_port,
                                        to_node: node_id,
                                        to_port: port_id,
                                    }
                                )))
                            } else {
                                Some(canvas::Action::publish(Message::Graph(
                                    GraphMessage::ConnectionCancelled
                                )))
                            }
                        }
                        _ => Some(canvas::Action::request_redraw()),
                    };
                    *state = Interaction::None;
                    action
                }
                mouse::Event::CursorMoved { .. } => {
                    match *state {
                        Interaction::Dragging { node_id, last_pos } => {
                            let delta = Vector::new(
                                cursor_position.x - last_pos.x,
                                cursor_position.y - last_pos.y,
                            );
                            *state = Interaction::Dragging { node_id, last_pos: cursor_position };
                            Some(canvas::Action::publish(Message::Graph(
                                GraphMessage::NodeDragged { node_id, delta }
                            )))
                        }
                        Interaction::Panning { last_pos } => {
                            let delta = Vector::new(
                                cursor_position.x - last_pos.x,
                                cursor_position.y - last_pos.y,
                            );
                            *state = Interaction::Panning { last_pos: cursor_position };
                            Some(canvas::Action::publish(Message::Graph(
                                GraphMessage::Pan(delta)
                            )))
                        }
                        Interaction::CreatingConnection { .. } => {
                            // Request redraw to update the pending connection line
                            Some(canvas::Action::request_redraw())
                        }
                        _ => None,
                    }
                }
                mouse::Event::WheelScrolled { delta } => {
                    let scroll = match delta {
                        mouse::ScrollDelta::Lines { y, .. } => *y,
                        mouse::ScrollDelta::Pixels { y, .. } => *y / 100.0,
                    };
                    Some(canvas::Action::publish(Message::Graph(
                        GraphMessage::Zoom { delta: scroll, cursor: cursor_position }
                    )))
                }
                _ => None,
            },
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, text, .. }) => {
                use iced::keyboard::Key;

                // When search is active, handle typing
                if self.search_active {
                    match key.as_ref() {
                        Key::Named(iced::keyboard::key::Named::Escape) => {
                            return Some(canvas::Action::publish(Message::Graph(GraphMessage::SearchClear)));
                        }
                        Key::Named(iced::keyboard::key::Named::Backspace) => {
                            return Some(canvas::Action::publish(Message::Graph(GraphMessage::SearchBackspace)));
                        }
                        Key::Named(iced::keyboard::key::Named::Enter) => {
                            return Some(canvas::Action::publish(Message::Graph(GraphMessage::SearchCommit)));
                        }
                        _ => {
                            // Handle text input
                            if let Some(txt) = text {
                                if !txt.is_empty() && !modifiers.control() && !modifiers.alt() {
                                    let input = txt.to_string();
                                    // Filter out control characters
                                    if input.chars().all(|c| !c.is_control()) {
                                        return Some(canvas::Action::publish(Message::Graph(
                                            GraphMessage::SearchInput { text: input }
                                        )));
                                    }
                                }
                            }
                            return None;
                        }
                    }
                }

                // Normal keyboard handling
                match key.as_ref() {
                    // Ctrl+F or / to activate search
                    Key::Character("f") | Key::Character("F") if modifiers.control() => {
                        Some(canvas::Action::publish(Message::Graph(GraphMessage::SearchActivate)))
                    }
                    Key::Character("/") if !modifiers.control() => {
                        Some(canvas::Action::publish(Message::Graph(GraphMessage::SearchActivate)))
                    }
                    Key::Character("l") | Key::Character("L") if !modifiers.control() => {
                        Some(canvas::Action::publish(Message::Graph(GraphMessage::AutoLayout)))
                    }
                    Key::Character("z") | Key::Character("Z") if modifiers.control() && !modifiers.shift() => {
                        Some(canvas::Action::publish(Message::Graph(GraphMessage::Undo)))
                    }
                    Key::Character("z") | Key::Character("Z") if modifiers.control() && modifiers.shift() => {
                        Some(canvas::Action::publish(Message::Graph(GraphMessage::Redo)))
                    }
                    Key::Character("y") | Key::Character("Y") if modifiers.control() => {
                        Some(canvas::Action::publish(Message::Graph(GraphMessage::Redo)))
                    }
                    Key::Character("?") | Key::Named(iced::keyboard::key::Named::F1) => {
                        Some(canvas::Action::publish(Message::Graph(GraphMessage::ToggleHelp)))
                    }
                    Key::Named(iced::keyboard::key::Named::Escape) => {
                        // Escape closes help if open
                        if self.show_help {
                            Some(canvas::Action::publish(Message::Graph(GraphMessage::ToggleHelp)))
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            },
            _ => None,
        }
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor.is_over(bounds) {
            match state {
                Interaction::Dragging { .. } => mouse::Interaction::Grabbing,
                Interaction::Panning { .. } => mouse::Interaction::Grabbing,
                Interaction::CreatingConnection { .. } => mouse::Interaction::Crosshair,
                Interaction::None => {
                    if let Some(pos) = cursor.position_in(bounds) {
                        match self.hit_test(pos) {
                            HitResult::Node(_) => mouse::Interaction::Grab,
                            HitResult::Port { .. } => mouse::Interaction::Crosshair,
                            HitResult::Link { .. } => mouse::Interaction::Pointer,
                            HitResult::None => mouse::Interaction::default(),
                        }
                    } else {
                        mouse::Interaction::default()
                    }
                }
            }
        } else {
            mouse::Interaction::default()
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub enum Interaction {
    #[default]
    None,
    Dragging { node_id: u32, last_pos: Point },
    Panning { last_pos: Point },
    CreatingConnection { from_node: u32, from_port: u32 },
}

// Color palette - Midnight Studio aesthetic
mod palette {
    use iced::Color;

    // Backgrounds
    pub const NODE_BG: Color = Color::from_rgb(0.11, 0.11, 0.13);
    pub const NODE_HEADER: Color = Color::from_rgb(0.15, 0.15, 0.18);
    pub const NODE_BORDER: Color = Color::from_rgb(0.22, 0.22, 0.26);
    pub const NODE_BORDER_HIGHLIGHT: Color = Color::from_rgb(0.30, 0.30, 0.36);

    // Accent colors - warm amber for output, cool cyan for input
    pub const ACCENT_OUTPUT: Color = Color::from_rgb(0.92, 0.65, 0.25);  // Warm amber/gold
    pub const ACCENT_INPUT: Color = Color::from_rgb(0.30, 0.75, 0.85);   // Cool cyan

    // Port type colors (matches qpwgraph conventions)
    pub const PORT_AUDIO: Color = Color::from_rgb(0.35, 0.75, 0.45);       // Green
    pub const PORT_AUDIO_GLOW: Color = Color::from_rgba(0.35, 0.75, 0.45, 0.25);
    pub const PORT_MIDI: Color = Color::from_rgb(0.85, 0.35, 0.35);        // Red
    pub const PORT_MIDI_GLOW: Color = Color::from_rgba(0.85, 0.35, 0.35, 0.25);
    pub const PORT_VIDEO: Color = Color::from_rgb(0.35, 0.55, 0.85);       // Blue
    pub const PORT_VIDEO_GLOW: Color = Color::from_rgba(0.35, 0.55, 0.85, 0.25);

    // Text
    pub const TEXT_PRIMARY: Color = Color::from_rgb(0.92, 0.92, 0.94);
    pub const TEXT_SECONDARY: Color = Color::from_rgb(0.55, 0.55, 0.60);

    // Links
    pub const LINK_COLOR: Color = Color::from_rgb(0.50, 0.70, 0.80);
    pub const LINK_GLOW: Color = Color::from_rgba(0.50, 0.70, 0.80, 0.15);
}

fn draw_rounded_rect(frame: &mut Frame, pos: Point, size: Size, radius: f32, color: Color) {
    let path = Path::new(|builder| {
        let r = radius.min(size.width / 2.0).min(size.height / 2.0);
        let x = pos.x;
        let y = pos.y;
        let w = size.width;
        let h = size.height;

        builder.move_to(Point::new(x + r, y));
        builder.line_to(Point::new(x + w - r, y));
        builder.arc_to(Point::new(x + w, y), Point::new(x + w, y + r), r);
        builder.line_to(Point::new(x + w, y + h - r));
        builder.arc_to(Point::new(x + w, y + h), Point::new(x + w - r, y + h), r);
        builder.line_to(Point::new(x + r, y + h));
        builder.arc_to(Point::new(x, y + h), Point::new(x, y + h - r), r);
        builder.line_to(Point::new(x, y + r));
        builder.arc_to(Point::new(x, y), Point::new(x + r, y), r);
        builder.close();
    });
    frame.fill(&path, color);
}

fn stroke_rounded_rect(frame: &mut Frame, pos: Point, size: Size, radius: f32, color: Color, width: f32) {
    let path = Path::new(|builder| {
        let r = radius.min(size.width / 2.0).min(size.height / 2.0);
        let x = pos.x;
        let y = pos.y;
        let w = size.width;
        let h = size.height;

        builder.move_to(Point::new(x + r, y));
        builder.line_to(Point::new(x + w - r, y));
        builder.arc_to(Point::new(x + w, y), Point::new(x + w, y + r), r);
        builder.line_to(Point::new(x + w, y + h - r));
        builder.arc_to(Point::new(x + w, y + h), Point::new(x + w - r, y + h), r);
        builder.line_to(Point::new(x + r, y + h));
        builder.arc_to(Point::new(x, y + h), Point::new(x, y + h - r), r);
        builder.line_to(Point::new(x, y + r));
        builder.arc_to(Point::new(x, y), Point::new(x + r, y), r);
        builder.close();
    });
    frame.stroke(&path, Stroke::default().with_color(color).with_width(width));
}

fn draw_node(frame: &mut Frame, node: &Node, dimmed: bool) {
    let height = Graph::node_height(node);
    let corner_radius = 8.0;

    // Opacity modifier for dimmed nodes
    let opacity = if dimmed { 0.25 } else { 1.0 };

    // Helper to apply dimming to a color
    let dim = |c: Color| -> Color {
        Color::from_rgba(c.r, c.g, c.b, c.a * opacity)
    };

    // Subtle outer glow/shadow
    draw_rounded_rect(
        frame,
        Point::new(node.position.x - 1.0, node.position.y - 1.0),
        Size::new(NODE_WIDTH + 2.0, height + 2.0),
        corner_radius + 1.0,
        dim(Color::from_rgba(0.0, 0.0, 0.0, 0.4)),
    );

    // Node background
    draw_rounded_rect(
        frame,
        node.position,
        Size::new(NODE_WIDTH, height),
        corner_radius,
        dim(palette::NODE_BG),
    );

    // Header background (with top corners rounded)
    let header_path = Path::new(|builder| {
        let r = corner_radius;
        let x = node.position.x;
        let y = node.position.y;
        let w = NODE_WIDTH;
        let h = NODE_HEADER_HEIGHT;

        builder.move_to(Point::new(x + r, y));
        builder.line_to(Point::new(x + w - r, y));
        builder.arc_to(Point::new(x + w, y), Point::new(x + w, y + r), r);
        builder.line_to(Point::new(x + w, y + h));
        builder.line_to(Point::new(x, y + h));
        builder.line_to(Point::new(x, y + r));
        builder.arc_to(Point::new(x, y), Point::new(x + r, y), r);
        builder.close();
    });
    frame.fill(&header_path, dim(palette::NODE_HEADER));

    // Accent line under header
    let accent_line = Path::line(
        Point::new(node.position.x, node.position.y + NODE_HEADER_HEIGHT),
        Point::new(node.position.x + NODE_WIDTH, node.position.y + NODE_HEADER_HEIGHT),
    );
    frame.stroke(
        &accent_line,
        Stroke::default()
            .with_color(dim(palette::NODE_BORDER))
            .with_width(1.0),
    );

    // Node border
    stroke_rounded_rect(
        frame,
        node.position,
        Size::new(NODE_WIDTH, height),
        corner_radius,
        dim(palette::NODE_BORDER),
        1.0,
    );

    // Node title (truncate if too long) - use custom_name if available
    let max_chars = 22;
    let name_to_display = node.custom_name.as_ref().unwrap_or(&node.name);
    let display_name = if name_to_display.len() > max_chars {
        format!("{}…", &name_to_display[..max_chars - 1])
    } else {
        name_to_display.clone()
    };
    let title = Text {
        content: display_name,
        position: Point::new(node.position.x + 12.0, node.position.y + 7.0),
        color: dim(palette::TEXT_PRIMARY),
        size: iced::Pixels(13.0),
        ..Text::default()
    };
    frame.fill_text(title);

    // Draw ports
    for port in node.input_ports.iter().chain(node.output_ports.iter()) {
        let pos = Graph::port_position(node, port);

        let (port_color, glow_color) = match port.port_type {
            PortType::Audio => (palette::PORT_AUDIO, palette::PORT_AUDIO_GLOW),
            PortType::Midi => (palette::PORT_MIDI, palette::PORT_MIDI_GLOW),
            PortType::Video => (palette::PORT_VIDEO, palette::PORT_VIDEO_GLOW),
        };

        // Outer glow
        let glow = Path::circle(pos, PORT_RADIUS + 3.0);
        frame.fill(&glow, dim(glow_color));

        // Port circle
        let circle = Path::circle(pos, PORT_RADIUS);
        frame.fill(&circle, dim(port_color));

        // Inner highlight
        let inner = Path::circle(pos, PORT_RADIUS - 2.0);
        frame.fill(&inner, dim(Color::from_rgba(1.0, 1.0, 1.0, 0.15)));

        // Port label (truncate if too long)
        let max_port_chars = 12;
        let port_display = if port.name.len() > max_port_chars {
            format!("{}…", &port.name[..max_port_chars - 1])
        } else {
            port.name.clone()
        };
        let label_x = match port.direction {
            PortDirection::Input => pos.x + PORT_RADIUS + 6.0,
            PortDirection::Output => pos.x - PORT_RADIUS - 65.0,
        };
        let label = Text {
            content: port_display,
            position: Point::new(label_x, pos.y - 5.0),
            color: dim(palette::TEXT_SECONDARY),
            size: iced::Pixels(10.0),
            ..Text::default()
        };
        frame.fill_text(label);
    }
}

fn draw_bezier_link(frame: &mut Frame, start: Point, end: Point, port_type: PortType) {
    let dx = end.x - start.x;
    let dy = (end.y - start.y).abs();

    // Reduce curve when nodes are nearly horizontally aligned
    // The more vertically aligned, the less curve we need
    let horizontal_dist = dx.abs();
    let alignment_factor = if horizontal_dist > 0.0 {
        (dy / horizontal_dist).min(1.0)  // 0 = perfectly aligned, 1 = very offset
    } else {
        1.0
    };

    // Base offset scales with horizontal distance, minimum depends on vertical offset
    let min_offset = 20.0 + 40.0 * alignment_factor;  // 20-60 based on alignment
    let control_offset = (horizontal_dist / 2.0).max(min_offset);

    let path = Path::new(|builder| {
        builder.move_to(start);
        builder.bezier_curve_to(
            Point::new(start.x + control_offset, start.y),
            Point::new(end.x - control_offset, end.y),
            end,
        );
    });

    // Color based on port type
    let (color, glow_color) = match port_type {
        PortType::Audio => (palette::PORT_AUDIO, palette::PORT_AUDIO_GLOW),
        PortType::Midi => (palette::PORT_MIDI, palette::PORT_MIDI_GLOW),
        PortType::Video => (palette::PORT_VIDEO, palette::PORT_VIDEO_GLOW),
    };

    // Outer glow layer
    frame.stroke(
        &path,
        Stroke::default()
            .with_color(glow_color)
            .with_width(8.0)
            .with_line_cap(canvas::LineCap::Round),
    );

    // Main cable
    frame.stroke(
        &path,
        Stroke::default()
            .with_color(color)
            .with_width(2.5)
            .with_line_cap(canvas::LineCap::Round),
    );

    // Inner highlight
    frame.stroke(
        &path,
        Stroke::default()
            .with_color(Color::from_rgba(1.0, 1.0, 1.0, 0.12))
            .with_width(1.0)
            .with_line_cap(canvas::LineCap::Round),
    );
}

fn draw_pending_link(frame: &mut Frame, start: Point, end: Point, direction: PortDirection, port_type: PortType) {
    // Determine control points based on direction
    let (ctrl_start, ctrl_end) = match direction {
        PortDirection::Output => {
            let offset = ((end.x - start.x).abs() / 2.0).max(60.0);
            (
                Point::new(start.x + offset, start.y),
                Point::new(end.x - offset, end.y),
            )
        }
        PortDirection::Input => {
            let offset = ((start.x - end.x).abs() / 2.0).max(60.0);
            (
                Point::new(start.x - offset, start.y),
                Point::new(end.x + offset, end.y),
            )
        }
    };

    let path = Path::new(|builder| {
        builder.move_to(start);
        builder.bezier_curve_to(ctrl_start, ctrl_end, end);
    });

    // Color based on port type
    let color = match port_type {
        PortType::Audio => palette::PORT_AUDIO,
        PortType::Midi => palette::PORT_MIDI,
        PortType::Video => palette::PORT_VIDEO,
    };

    // Outer glow - more prominent for pending
    frame.stroke(
        &path,
        Stroke::default()
            .with_color(Color::from_rgba(1.0, 1.0, 1.0, 0.1))
            .with_width(10.0)
            .with_line_cap(canvas::LineCap::Round),
    );

    // Main cable
    frame.stroke(
        &path,
        Stroke::default()
            .with_color(color)
            .with_width(3.0)
            .with_line_cap(canvas::LineCap::Round),
    );

    // Cursor endpoint indicator
    let cursor_dot = Path::circle(end, 6.0);
    frame.fill(&cursor_dot, Color::from_rgba(1.0, 1.0, 1.0, 0.3));
    let cursor_inner = Path::circle(end, 3.0);
    frame.fill(&cursor_inner, color);
}

fn draw_help_overlay(frame: &mut Frame, size: Size) {
    // Semi-transparent background
    frame.fill_rectangle(
        Point::ORIGIN,
        size,
        Color::from_rgba(0.0, 0.0, 0.0, 0.75),
    );

    let shortcuts = [
        ("L", "Auto-layout"),
        ("Ctrl+F  /  /", "Search nodes"),
        ("Ctrl+Z", "Undo"),
        ("Ctrl+Shift+Z", "Redo"),
        ("Ctrl+Y", "Redo"),
        ("?  /  F1", "Toggle help"),
        ("Esc", "Close overlay"),
        ("", ""),
        ("Mouse", ""),
        ("Drag port", "Connect"),
        ("Right-click link", "Disconnect"),
        ("Drag node", "Move"),
        ("Drag empty", "Pan"),
        ("Scroll", "Zoom"),
    ];

    let box_width = 280.0;
    let line_height = 24.0;
    let box_height = shortcuts.len() as f32 * line_height + 60.0;
    let box_x = (size.width - box_width) / 2.0;
    let box_y = (size.height - box_height) / 2.0;

    // Box background
    draw_rounded_rect(
        frame,
        Point::new(box_x, box_y),
        Size::new(box_width, box_height),
        12.0,
        Color::from_rgb(0.12, 0.12, 0.14),
    );

    // Title
    let title = Text {
        content: "Keyboard Shortcuts".to_string(),
        position: Point::new(box_x + 20.0, box_y + 20.0),
        color: palette::TEXT_PRIMARY,
        size: iced::Pixels(16.0),
        ..Text::default()
    };
    frame.fill_text(title);

    // Shortcuts
    for (i, (key, action)) in shortcuts.iter().enumerate() {
        let y = box_y + 55.0 + i as f32 * line_height;

        if !key.is_empty() {
            let key_text = Text {
                content: key.to_string(),
                position: Point::new(box_x + 20.0, y),
                color: palette::PORT_AUDIO,
                size: iced::Pixels(12.0),
                ..Text::default()
            };
            frame.fill_text(key_text);
        }

        if !action.is_empty() {
            let action_text = Text {
                content: action.to_string(),
                position: Point::new(box_x + 130.0, y),
                color: palette::TEXT_SECONDARY,
                size: iced::Pixels(12.0),
                ..Text::default()
            };
            frame.fill_text(action_text);
        }
    }

    // Press any key hint
    let hint = Text {
        content: "Press ? or F1 to close".to_string(),
        position: Point::new(box_x + 20.0, box_y + box_height - 25.0),
        color: Color::from_rgba(1.0, 1.0, 1.0, 0.4),
        size: iced::Pixels(10.0),
        ..Text::default()
    };
    frame.fill_text(hint);
}

fn draw_search_overlay(frame: &mut Frame, size: Size, query: &str, match_count: usize) {
    // Search bar at top center
    let bar_width = 320.0;
    let bar_height = 40.0;
    let bar_x = (size.width - bar_width) / 2.0;
    let bar_y = 20.0;

    // Background with shadow
    draw_rounded_rect(
        frame,
        Point::new(bar_x - 2.0, bar_y + 2.0),
        Size::new(bar_width + 4.0, bar_height),
        8.0,
        Color::from_rgba(0.0, 0.0, 0.0, 0.3),
    );

    // Main bar background
    draw_rounded_rect(
        frame,
        Point::new(bar_x, bar_y),
        Size::new(bar_width, bar_height),
        8.0,
        Color::from_rgb(0.12, 0.12, 0.14),
    );

    // Border
    stroke_rounded_rect(
        frame,
        Point::new(bar_x, bar_y),
        Size::new(bar_width, bar_height),
        8.0,
        palette::PORT_AUDIO,
        1.5,
    );

    // Search icon (magnifying glass represented as text)
    let icon = Text {
        content: "/".to_string(),
        position: Point::new(bar_x + 14.0, bar_y + 11.0),
        color: palette::TEXT_SECONDARY,
        size: iced::Pixels(14.0),
        ..Text::default()
    };
    frame.fill_text(icon);

    // Query text with cursor
    let display_text = if query.is_empty() {
        "Search nodes...".to_string()
    } else {
        format!("{}|", query) // Show cursor
    };
    let text_color = if query.is_empty() {
        palette::TEXT_SECONDARY
    } else {
        palette::TEXT_PRIMARY
    };
    let query_text = Text {
        content: display_text,
        position: Point::new(bar_x + 35.0, bar_y + 12.0),
        color: text_color,
        size: iced::Pixels(13.0),
        ..Text::default()
    };
    frame.fill_text(query_text);

    // Match count (right side)
    if !query.is_empty() {
        let count_text = if match_count == 1 {
            "1 match".to_string()
        } else {
            format!("{} matches", match_count)
        };
        let count = Text {
            content: count_text,
            position: Point::new(bar_x + bar_width - 75.0, bar_y + 12.0),
            color: if match_count > 0 { palette::PORT_AUDIO } else { palette::PORT_MIDI },
            size: iced::Pixels(11.0),
            ..Text::default()
        };
        frame.fill_text(count);
    }

    // Hint below
    let hint = Text {
        content: "Enter to focus • Esc to close".to_string(),
        position: Point::new(bar_x + (bar_width - 160.0) / 2.0, bar_y + bar_height + 8.0),
        color: Color::from_rgba(1.0, 1.0, 1.0, 0.4),
        size: iced::Pixels(10.0),
        ..Text::default()
    };
    frame.fill_text(hint);
}
