//! Minimal road network model (AD-2). Story 3 completes this API.

use std::collections::HashMap;

/// Stable node identity. Monotonically increasing and never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(u64);

impl NodeId {
    pub fn get(self) -> u64 {
        self.0
    }
}

/// Node flag as stored in the config file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NodeFlag(pub u8);

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub flag: NodeFlag,
    /// Targets in file order.
    pub outgoing: Vec<NodeId>,
    /// Sources in file order.
    pub incoming: Vec<NodeId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Marker {
    pub node: NodeId,
    pub name: String,
    pub group: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Network {
    nodes: HashMap<NodeId, Node>,
    order: Vec<NodeId>,
    markers: Vec<Marker>,
    next_id: u64,
}

impl Network {
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a node at the end of the order and returns its new id.
    pub fn push_node(&mut self, node: Node) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        self.nodes.insert(id, node);
        self.order.push(id);
        id
    }

    pub fn push_marker(&mut self, marker: Marker) {
        self.markers.push(marker);
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(&id)
    }

    /// Node ids in file order.
    pub fn order(&self) -> &[NodeId] {
        &self.order
    }

    pub fn markers(&self) -> &[Marker] {
        &self.markers
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
}
