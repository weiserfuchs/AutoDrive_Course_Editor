//! Road network model (AD-2, AD-3, AD-5).
//!
//! Adjacency, positions and flags change only through `Network` methods, and every
//! one of them keeps the spatial indexes in sync.

use std::collections::HashMap;
use std::fmt;

use rstar::primitives::{GeomWithData, Line};
use rstar::{AABB, RTree};
use thiserror::Error;

/// Stable node identity. Monotonically increasing and never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(u64);

impl NodeId {
    pub fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "node {}", self.0)
    }
}

/// Node flag as stored in the config file. Any file value is preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NodeFlag(pub u8);

impl NodeFlag {
    pub const REGULAR: NodeFlag = NodeFlag(0);
    pub const SUB_PRIO: NodeFlag = NodeFlag(1);
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub flag: NodeFlag,
    outgoing: Vec<NodeId>,
    incoming: Vec<NodeId>,
}

impl Node {
    /// Targets in file order.
    pub fn outgoing(&self) -> &[NodeId] {
        &self.outgoing
    }

    /// Sources in file order.
    pub fn incoming(&self) -> &[NodeId] {
        &self.incoming
    }
}

/// A node as read from a file. Adjacency entries are indexes into the loaded list.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedNode {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub flag: NodeFlag,
    pub outgoing: Vec<usize>,
    pub incoming: Vec<usize>,
}

/// What `connect` and `set_shape` create between `a` and `b`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionShape {
    /// a.out + b, b.in + a.
    OneWay,
    /// OneWay in both directions.
    Dual,
    /// a.out + b only.
    Reverse,
}

/// The displayed kind of the link from a start node to an end node (derived, never stored).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionKind {
    None,
    Regular,
    SubPrio,
    Dual,
    Reverse,
    CrossedRegular,
    CrossedReverse,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NetworkError {
    #[error("{0} does not exist")]
    UnknownNode(NodeId),
    #[error("{0} cannot be connected to itself")]
    SelfConnection(NodeId),
    #[error("adjacency index {index} is out of range for {len} loaded nodes")]
    IndexOutOfRange { index: usize, len: usize },
    #[error("the config is read-only")]
    ReadOnly,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Marker {
    pub node: NodeId,
    pub name: String,
    pub group: String,
}

/// Which adjacency list of a node a record refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Out,
    In,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkChange {
    Added,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LinkRecord {
    node: NodeId,
    side: Side,
    index: usize,
    value: NodeId,
    change: LinkChange,
}

/// The list entries a link operation added or removed, with their positions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkDelta(Vec<LinkRecord>);

impl LinkDelta {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A node taken out of the network, with what `Network::reinsert` needs to restore it.
#[derive(Debug, Clone, PartialEq)]
pub struct RemovedNode {
    id: NodeId,
    node: Node,
    order_pos: usize,
    links: LinkDelta,
    markers: Vec<(usize, Marker)>,
}

impl RemovedNode {
    pub fn id(&self) -> NodeId {
        self.id
    }
}

/// What a marker edit changed.
#[derive(Debug, Clone, PartialEq)]
pub enum MarkerDelta {
    Unchanged,
    Added { index: usize },
    Replaced { index: usize, old: Marker },
    Removed { index: usize, marker: Marker },
}

type NodeEntry = GeomWithData<[f64; 2], NodeId>;
type EdgeEntry = GeomWithData<Line<[f64; 2]>, (NodeId, NodeId)>;

#[derive(Debug, Clone, Default)]
pub struct Network {
    nodes: HashMap<NodeId, Node>,
    order: Vec<NodeId>,
    markers: Vec<Marker>,
    next_id: u64,
    node_index: RTree<NodeEntry>,
    edge_index: RTree<EdgeEntry>,
}

fn edge_entry(from: &Node, to: &Node, ids: (NodeId, NodeId)) -> EdgeEntry {
    GeomWithData::new(Line::new([from.x, from.z], [to.x, to.z]), ids)
}

/// Even-odd ray casting.
fn point_in_polygon(x: f64, z: f64, polygon: &[(f64, f64)]) -> bool {
    let mut inside = false;
    let mut j = polygon.len().wrapping_sub(1);
    for (i, &(xi, zi)) in polygon.iter().enumerate() {
        let (xj, zj) = polygon[j];
        if (zi > z) != (zj > z) && x < (xj - xi) * (z - zi) / (zj - zi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

impl Network {
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a network from loaded nodes, assigning ids in order. Adjacency is kept verbatim.
    pub fn from_loaded(loaded: Vec<LoadedNode>) -> Result<Self, NetworkError> {
        let len = loaded.len();
        let ids: Vec<NodeId> = (0..len as u64).map(NodeId).collect();
        let map = |list: &[usize]| -> Result<Vec<NodeId>, NetworkError> {
            list.iter()
                .map(|&index| {
                    ids.get(index)
                        .copied()
                        .ok_or(NetworkError::IndexOutOfRange { index, len })
                })
                .collect()
        };
        let mut nodes = HashMap::with_capacity(len);
        for (l, id) in loaded.iter().zip(&ids) {
            nodes.insert(
                *id,
                Node {
                    x: l.x,
                    y: l.y,
                    z: l.z,
                    flag: l.flag,
                    outgoing: map(&l.outgoing)?,
                    incoming: map(&l.incoming)?,
                },
            );
        }
        let points = ids
            .iter()
            .filter_map(|id| nodes.get(id).map(|n| GeomWithData::new([n.x, n.z], *id)))
            .collect();
        let mut edges = Vec::new();
        for id in &ids {
            let Some(from) = nodes.get(id) else { continue };
            for to_id in &from.outgoing {
                if let Some(to) = nodes.get(to_id) {
                    edges.push(edge_entry(from, to, (*id, *to_id)));
                }
            }
        }
        Ok(Self {
            nodes,
            order: ids,
            markers: Vec::new(),
            next_id: len as u64,
            node_index: RTree::bulk_load(points),
            edge_index: RTree::bulk_load(edges),
        })
    }

    /// Appends an unconnected node at the end of the order and returns its new id.
    pub fn add_node(&mut self, x: f64, y: f64, z: f64, flag: NodeFlag) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        self.nodes.insert(
            id,
            Node {
                x,
                y,
                z,
                flag,
                outgoing: Vec::new(),
                incoming: Vec::new(),
            },
        );
        self.order.push(id);
        self.node_index.insert(GeomWithData::new([x, z], id));
        id
    }

    pub fn push_marker(&mut self, marker: Marker) {
        self.markers.push(marker);
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id)
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

    fn require(&self, id: NodeId) -> Result<&Node, NetworkError> {
        self.nodes.get(&id).ok_or(NetworkError::UnknownNode(id))
    }

    fn check_pair(&self, a: NodeId, b: NodeId) -> Result<(), NetworkError> {
        self.require(a)?;
        self.require(b)?;
        if a == b {
            return Err(NetworkError::SelfConnection(a));
        }
        Ok(())
    }

    /// Moves a node and updates the node and edge indexes.
    pub fn move_node(&mut self, id: NodeId, x: f64, y: f64, z: f64) -> Result<(), NetworkError> {
        let old = self.require(id)?;
        let old_point = [old.x, old.z];
        let touching: Vec<EdgeEntry> = self
            .edge_index
            .locate_in_envelope_intersecting(AABB::from_point(old_point))
            .filter(|e| e.data.0 == id || e.data.1 == id)
            .cloned()
            .collect();
        for e in &touching {
            self.edge_index.remove(e);
        }
        self.node_index.remove(&GeomWithData::new(old_point, id));
        if let Some(node) = self.nodes.get_mut(&id) {
            node.x = x;
            node.y = y;
            node.z = z;
        }
        self.node_index.insert(GeomWithData::new([x, z], id));
        for e in touching {
            let (a, b) = e.data;
            if let (Some(from), Some(to)) = (self.nodes.get(&a), self.nodes.get(&b)) {
                self.edge_index.insert(edge_entry(from, to, (a, b)));
            }
        }
        Ok(())
    }

    pub fn set_flag(&mut self, id: NodeId, flag: NodeFlag) -> Result<(), NetworkError> {
        self.nodes
            .get_mut(&id)
            .ok_or(NetworkError::UnknownNode(id))?
            .flag = flag;
        Ok(())
    }

    fn list_mut(&mut self, node: NodeId, side: Side) -> Option<&mut Vec<NodeId>> {
        self.nodes.get_mut(&node).map(|n| match side {
            Side::Out => &mut n.outgoing,
            Side::In => &mut n.incoming,
        })
    }

    /// Removes one edge-index entry for `a -> b` when both nodes exist.
    fn drop_edge(&mut self, a: NodeId, b: NodeId) {
        if let (Some(from), Some(to)) = (self.nodes.get(&a), self.nodes.get(&b)) {
            let entry = edge_entry(from, to, (a, b));
            self.edge_index.remove(&entry);
        }
    }

    /// Inserts an edge-index entry for `a -> b` when both nodes exist.
    fn put_edge(&mut self, a: NodeId, b: NodeId) {
        if let (Some(from), Some(to)) = (self.nodes.get(&a), self.nodes.get(&b)) {
            self.edge_index.insert(edge_entry(from, to, (a, b)));
        }
    }

    /// Adds `a.out + b` (and optionally `b.in + a`) when missing. Both nodes must exist.
    fn add_link(&mut self, a: NodeId, b: NodeId, with_incoming: bool, delta: &mut LinkDelta) {
        let mut added = false;
        if let Some(node) = self.nodes.get_mut(&a)
            && !node.outgoing.contains(&b)
        {
            delta.0.push(LinkRecord {
                node: a,
                side: Side::Out,
                index: node.outgoing.len(),
                value: b,
                change: LinkChange::Added,
            });
            node.outgoing.push(b);
            added = true;
        }
        if with_incoming
            && let Some(node) = self.nodes.get_mut(&b)
            && !node.incoming.contains(&a)
        {
            delta.0.push(LinkRecord {
                node: b,
                side: Side::In,
                index: node.incoming.len(),
                value: a,
                change: LinkChange::Added,
            });
            node.incoming.push(a);
        }
        if added {
            self.put_edge(a, b);
        }
    }

    /// Removes every `value` from the list, highest index first, and records each removal.
    fn take_entries(&mut self, node: NodeId, side: Side, value: NodeId, delta: &mut LinkDelta) {
        let Some(list) = self.list_mut(node, side) else {
            return;
        };
        let hits: Vec<usize> = list
            .iter()
            .enumerate()
            .filter(|(_, v)| **v == value)
            .map(|(i, _)| i)
            .collect();
        for &index in hits.iter().rev() {
            list.remove(index);
            delta.0.push(LinkRecord {
                node,
                side,
                index,
                value,
                change: LinkChange::Removed,
            });
        }
        if side == Side::Out {
            for _ in &hits {
                self.drop_edge(node, value);
            }
        }
    }

    /// Undoes a delta exactly: records are replayed backwards.
    pub fn revert_links(&mut self, delta: &LinkDelta) {
        for r in delta.0.iter().rev() {
            if let Some(list) = self.list_mut(r.node, r.side) {
                match r.change {
                    LinkChange::Added => {
                        if r.index < list.len() {
                            list.remove(r.index);
                        }
                    }
                    LinkChange::Removed => list.insert(r.index.min(list.len()), r.value),
                }
            }
            if r.side == Side::Out {
                match r.change {
                    LinkChange::Added => self.drop_edge(r.node, r.value),
                    LinkChange::Removed => self.put_edge(r.node, r.value),
                }
            }
        }
    }

    /// Adds the links `shape` describes; never removes anything.
    pub fn connect(
        &mut self,
        a: NodeId,
        b: NodeId,
        shape: ConnectionShape,
    ) -> Result<LinkDelta, NetworkError> {
        self.check_pair(a, b)?;
        let mut delta = LinkDelta::default();
        match shape {
            ConnectionShape::OneWay => self.add_link(a, b, true, &mut delta),
            ConnectionShape::Reverse => self.add_link(a, b, false, &mut delta),
            ConnectionShape::Dual => {
                self.add_link(a, b, true, &mut delta);
                self.add_link(b, a, true, &mut delta);
            }
        }
        Ok(delta)
    }

    /// Removes every link between `a` and `b`, in both directions.
    pub fn disconnect(&mut self, a: NodeId, b: NodeId) -> Result<LinkDelta, NetworkError> {
        self.check_pair(a, b)?;
        let mut delta = LinkDelta::default();
        for (from, to) in [(a, b), (b, a)] {
            self.take_entries(from, Side::Out, to, &mut delta);
            self.take_entries(from, Side::In, to, &mut delta);
        }
        Ok(delta)
    }

    /// Makes the link between `a` and `b` exactly `shape`.
    pub fn set_shape(
        &mut self,
        a: NodeId,
        b: NodeId,
        shape: ConnectionShape,
    ) -> Result<LinkDelta, NetworkError> {
        let mut delta = self.disconnect(a, b)?;
        delta.0.extend(self.connect(a, b, shape)?.0);
        Ok(delta)
    }

    /// Removes a node, every list entry that points at it and its markers. The record
    /// restores all of it through `reinsert`, provided later removals were reinserted first.
    pub fn remove_node(&mut self, id: NodeId) -> Result<RemovedNode, NetworkError> {
        let node = self.require(id)?;
        let own_out = node.outgoing.clone();
        let mut neighbours: Vec<NodeId> = node
            .outgoing
            .iter()
            .chain(&node.incoming)
            .copied()
            .filter(|n| *n != id)
            .collect();
        // Sources of one-way (Reverse) links are not in the node's own lists.
        neighbours.extend(
            self.edge_index
                .locate_in_envelope_intersecting(AABB::from_point([node.x, node.z]))
                .filter(|e| e.data.1 == id && e.data.0 != id)
                .map(|e| e.data.0),
        );
        neighbours.sort();
        neighbours.dedup();
        for t in own_out {
            self.drop_edge(id, t);
        }
        let mut links = LinkDelta::default();
        for n in neighbours {
            self.take_entries(n, Side::Out, id, &mut links);
            self.take_entries(n, Side::In, id, &mut links);
        }
        let (point, node) = match self.nodes.remove(&id) {
            Some(node) => ([node.x, node.z], node),
            None => return Err(NetworkError::UnknownNode(id)),
        };
        self.node_index.remove(&GeomWithData::new(point, id));
        let order_pos = self.order.iter().position(|o| *o == id).unwrap_or(0);
        if order_pos < self.order.len() {
            self.order.remove(order_pos);
        }
        let mut markers = Vec::new();
        for index in (0..self.markers.len()).rev() {
            if self.markers[index].node == id {
                markers.push((index, self.markers.remove(index)));
            }
        }
        markers.reverse();
        Ok(RemovedNode {
            id,
            node,
            order_pos,
            links,
            markers,
        })
    }

    /// Restores a removed node under its original id, at its original order position.
    pub fn reinsert(&mut self, removed: RemovedNode) {
        let RemovedNode {
            id,
            node,
            order_pos,
            links,
            markers,
        } = removed;
        let point = [node.x, node.z];
        let own_out = node.outgoing.clone();
        self.nodes.insert(id, node);
        self.order.insert(order_pos.min(self.order.len()), id);
        self.node_index.insert(GeomWithData::new(point, id));
        self.revert_links(&links);
        for t in own_out {
            self.put_edge(id, t);
        }
        for (index, marker) in markers {
            self.markers.insert(index.min(self.markers.len()), marker);
        }
        self.next_id = self.next_id.max(id.0 + 1);
    }

    /// Sets the marker of `node`, replacing it in place or appending a new one.
    pub fn set_marker(
        &mut self,
        node: NodeId,
        name: String,
        group: String,
    ) -> Result<MarkerDelta, NetworkError> {
        self.require(node)?;
        let marker = Marker { node, name, group };
        if let Some(index) = self.markers.iter().position(|m| m.node == node) {
            let old = std::mem::replace(&mut self.markers[index], marker);
            Ok(MarkerDelta::Replaced { index, old })
        } else {
            self.markers.push(marker);
            Ok(MarkerDelta::Added {
                index: self.markers.len() - 1,
            })
        }
    }

    /// Removes the marker of `node`, if it has one.
    pub fn remove_marker(&mut self, node: NodeId) -> Result<MarkerDelta, NetworkError> {
        self.require(node)?;
        Ok(match self.markers.iter().position(|m| m.node == node) {
            Some(index) => MarkerDelta::Removed {
                index,
                marker: self.markers.remove(index),
            },
            None => MarkerDelta::Unchanged,
        })
    }

    pub fn revert_marker(&mut self, delta: &MarkerDelta) {
        match delta {
            MarkerDelta::Unchanged => {}
            MarkerDelta::Added { index } => {
                if *index < self.markers.len() {
                    self.markers.remove(*index);
                }
            }
            MarkerDelta::Replaced { index, old } => {
                if let Some(slot) = self.markers.get_mut(*index) {
                    *slot = old.clone();
                }
            }
            MarkerDelta::Removed { index, marker } => {
                self.markers
                    .insert((*index).min(self.markers.len()), marker.clone());
            }
        }
    }

    /// The displayed kind of the link from `a` to `b` (Java `RoadMap.getConnection`).
    pub fn connection_kind(&self, a: NodeId, b: NodeId) -> ConnectionKind {
        let (Some(na), Some(nb)) = (self.nodes.get(&a), self.nodes.get(&b)) else {
            return ConnectionKind::None;
        };
        if !na.outgoing.contains(&b) {
            return ConnectionKind::None;
        }
        let p = nb.incoming.contains(&a);
        let q = nb.outgoing.contains(&a);
        let r = na.incoming.contains(&b);
        match (p, q, r) {
            (true, true, false) => ConnectionKind::CrossedRegular,
            (false, true, true) => ConnectionKind::CrossedReverse,
            (true, true, true) => ConnectionKind::Dual,
            (false, false, false) => ConnectionKind::Reverse,
            (true, false, false) => match na.flag {
                NodeFlag::SUB_PRIO => ConnectionKind::SubPrio,
                NodeFlag::REGULAR => ConnectionKind::Regular,
                _ => ConnectionKind::Unknown,
            },
            _ => ConnectionKind::Unknown,
        }
    }

    /// The node closest to (x, z).
    pub fn nearest(&self, x: f64, z: f64) -> Option<NodeId> {
        self.node_index.nearest_neighbor([x, z]).map(|e| e.data)
    }

    /// Nodes inside the rectangle, bounds inclusive. Sorted by id.
    pub fn in_rect(&self, x0: f64, z0: f64, x1: f64, z1: f64) -> Vec<NodeId> {
        let aabb = AABB::from_corners([x0, z0], [x1, z1]);
        let mut ids: Vec<NodeId> = self
            .node_index
            .locate_in_envelope(aabb)
            .map(|e| e.data)
            .collect();
        ids.sort();
        ids
    }

    /// Nodes inside the polygon (even-odd rule). Sorted by id.
    pub fn in_polygon(&self, polygon: &[(f64, f64)]) -> Vec<NodeId> {
        if polygon.len() < 3 {
            return Vec::new();
        }
        let aabb = AABB::from_points(
            polygon
                .iter()
                .map(|&(x, z)| [x, z])
                .collect::<Vec<_>>()
                .iter(),
        );
        let mut ids: Vec<NodeId> = self
            .node_index
            .locate_in_envelope(aabb)
            .filter(|e| point_in_polygon(e.geom()[0], e.geom()[1], polygon))
            .map(|e| e.data)
            .collect();
        ids.sort();
        ids
    }

    /// Nodes within `distance` of (x, z), inclusive. Sorted by id.
    pub fn within(&self, x: f64, z: f64, distance: f64) -> Vec<NodeId> {
        let mut ids: Vec<NodeId> = self
            .node_index
            .locate_within_distance([x, z], distance * distance)
            .map(|e| e.data)
            .collect();
        ids.sort();
        ids
    }

    /// `(from, to)` of every edge whose segment lies within `distance` of (x, z), inclusive.
    /// Sorted.
    pub fn edges_near(&self, x: f64, z: f64, distance: f64) -> Vec<(NodeId, NodeId)> {
        let mut edges: Vec<(NodeId, NodeId)> = self
            .edge_index
            .locate_within_distance([x, z], distance * distance)
            .map(|e| e.data)
            .collect();
        edges.sort();
        edges
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    /// Small deterministic generator, so the tests need no extra dependency.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 33
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
        fn coord(&mut self) -> f64 {
            self.below(200) as f64
        }
    }

    fn two() -> (Network, NodeId, NodeId) {
        let mut n = Network::new();
        let a = n.add_node(0.0, 0.0, 0.0, NodeFlag::REGULAR);
        let b = n.add_node(10.0, 0.0, 0.0, NodeFlag::REGULAR);
        (n, a, b)
    }

    fn out(n: &Network, id: NodeId) -> Vec<NodeId> {
        n.node(id)
            .map(|x| x.outgoing().to_vec())
            .unwrap_or_default()
    }

    fn inc(n: &Network, id: NodeId) -> Vec<NodeId> {
        n.node(id)
            .map(|x| x.incoming().to_vec())
            .unwrap_or_default()
    }

    const HUGE: f64 = 1e6;

    #[test]
    fn connect_one_way() {
        let (mut n, a, b) = two();
        n.connect(a, b, ConnectionShape::OneWay).expect("ok");
        assert_eq!((out(&n, a), inc(&n, b)), (vec![b], vec![a]));
        assert!(out(&n, b).is_empty() && inc(&n, a).is_empty());
        assert_eq!(n.connection_kind(a, b), ConnectionKind::Regular);
        n.set_flag(a, NodeFlag::SUB_PRIO).expect("ok");
        assert_eq!(n.connection_kind(a, b), ConnectionKind::SubPrio);
    }

    #[test]
    fn connect_dual() {
        let (mut n, a, b) = two();
        n.connect(a, b, ConnectionShape::Dual).expect("ok");
        assert_eq!((out(&n, a), inc(&n, a)), (vec![b], vec![b]));
        assert_eq!((out(&n, b), inc(&n, b)), (vec![a], vec![a]));
        assert_eq!(n.connection_kind(a, b), ConnectionKind::Dual);
        assert_eq!(n.connection_kind(b, a), ConnectionKind::Dual);
    }

    #[test]
    fn connect_reverse() {
        let (mut n, a, b) = two();
        n.connect(a, b, ConnectionShape::Reverse).expect("ok");
        assert_eq!(out(&n, a), vec![b]);
        assert!(inc(&n, b).is_empty() && out(&n, b).is_empty() && inc(&n, a).is_empty());
        assert_eq!(n.connection_kind(a, b), ConnectionKind::Reverse);
    }

    #[test]
    fn idempotent() {
        let (mut n, a, b) = two();
        for shape in [
            ConnectionShape::OneWay,
            ConnectionShape::Dual,
            ConnectionShape::Reverse,
        ] {
            n.connect(a, b, shape).expect("ok");
            let snapshot = n.clone();
            n.connect(a, b, shape).expect("ok");
            assert_eq!(n.nodes, snapshot.nodes);
            assert_eq!(
                n.edges_near(0.0, 0.0, HUGE),
                snapshot.edges_near(0.0, 0.0, HUGE)
            );
        }
        n.disconnect(a, b).expect("ok");
        let snapshot = n.clone();
        n.disconnect(a, b).expect("ok");
        assert_eq!(n.nodes, snapshot.nodes);
    }

    #[test]
    fn connect_adds_to_existing() {
        let (mut n, a, b) = two();
        n.connect(a, b, ConnectionShape::OneWay).expect("ok");
        n.connect(a, b, ConnectionShape::Dual).expect("ok");
        assert_eq!(n.connection_kind(a, b), ConnectionKind::Dual);
        n.connect(a, b, ConnectionShape::OneWay).expect("ok");
        assert_eq!(n.connection_kind(a, b), ConnectionKind::Dual);
    }

    #[test]
    fn set_shape_replaces() {
        let (mut n, a, b) = two();
        n.connect(a, b, ConnectionShape::Dual).expect("ok");
        n.set_shape(a, b, ConnectionShape::OneWay).expect("ok");
        assert_eq!((out(&n, a), inc(&n, b)), (vec![b], vec![a]));
        assert!(out(&n, b).is_empty() && inc(&n, a).is_empty());
        assert_eq!(n.edges_near(0.0, 0.0, HUGE), vec![(a, b)]);
        n.set_shape(a, b, ConnectionShape::Reverse).expect("ok");
        assert_eq!(n.connection_kind(a, b), ConnectionKind::Reverse);
    }

    #[test]
    fn disconnect_removes_everything() {
        let (mut n, a, b) = two();
        n.connect(a, b, ConnectionShape::Dual).expect("ok");
        n.disconnect(b, a).expect("ok");
        assert!(out(&n, a).is_empty() && inc(&n, a).is_empty());
        assert!(out(&n, b).is_empty() && inc(&n, b).is_empty());
        assert!(n.edges_near(0.0, 0.0, HUGE).is_empty());
        assert_eq!(n.connection_kind(a, b), ConnectionKind::None);
    }

    #[test]
    fn self_and_unknown_are_errors_and_change_nothing() {
        let (mut n, a, b) = two();
        let ghost = NodeId(99);
        let before = n.nodes.clone();
        assert_eq!(
            n.connect(a, a, ConnectionShape::OneWay),
            Err(NetworkError::SelfConnection(a))
        );
        assert_eq!(
            n.connect(a, ghost, ConnectionShape::Dual),
            Err(NetworkError::UnknownNode(ghost))
        );
        assert_eq!(
            n.disconnect(ghost, b),
            Err(NetworkError::UnknownNode(ghost))
        );
        assert_eq!(
            n.set_shape(a, ghost, ConnectionShape::Reverse),
            Err(NetworkError::UnknownNode(ghost))
        );
        assert_eq!(
            n.move_node(ghost, 1.0, 1.0, 1.0),
            Err(NetworkError::UnknownNode(ghost))
        );
        assert_eq!(
            n.set_flag(ghost, NodeFlag(2)),
            Err(NetworkError::UnknownNode(ghost))
        );
        assert_eq!(n.nodes, before);
        assert!(n.edges_near(0.0, 0.0, HUGE).is_empty());
        assert!(ghost.to_string().contains("99"));
    }

    #[test]
    fn link_deltas_revert_exactly() {
        let (mut n, a, b) = two();
        let c = n.add_node(5.0, 0.0, 5.0, NodeFlag::REGULAR);
        n.connect(a, b, ConnectionShape::Dual).expect("ok");
        n.connect(c, a, ConnectionShape::OneWay).expect("ok");
        n.connect(a, c, ConnectionShape::Reverse).expect("ok");
        let before = (n.nodes.clone(), n.edges_near(0.0, 0.0, HUGE));
        let d1 = n.set_shape(a, b, ConnectionShape::Reverse).expect("ok");
        let d2 = n.disconnect(a, c).expect("ok");
        let d3 = n.connect(b, c, ConnectionShape::Dual).expect("ok");
        assert!(!d1.is_empty() && !d2.is_empty() && !d3.is_empty());
        for d in [&d3, &d2, &d1] {
            n.revert_links(d);
        }
        assert_eq!((n.nodes.clone(), n.edges_near(0.0, 0.0, HUGE)), before);
    }

    #[test]
    fn remove_and_reinsert_restore_everything() {
        let (mut n, a, b) = two();
        let c = n.add_node(5.0, 0.0, 5.0, NodeFlag(3));
        n.connect(a, b, ConnectionShape::Dual).expect("ok");
        n.connect(c, b, ConnectionShape::OneWay).expect("ok");
        n.connect(b, c, ConnectionShape::Reverse).expect("ok");
        n.push_marker(Marker {
            node: a,
            name: "A".into(),
            group: "g".into(),
        });
        n.push_marker(Marker {
            node: b,
            name: "B".into(),
            group: "g".into(),
        });
        let before = (
            n.nodes.clone(),
            n.order.clone(),
            n.markers.clone(),
            n.edges_near(0.0, 0.0, HUGE),
            n.in_rect(-HUGE, -HUGE, HUGE, HUGE),
        );
        let removed = n.remove_node(a).expect("ok");
        assert_eq!(removed.id(), a);
        assert!(n.node(a).is_none() && n.order() == [b, c]);
        assert_eq!(n.markers().len(), 1);
        assert_eq!(n.in_rect(-HUGE, -HUGE, HUGE, HUGE), vec![b, c]);
        assert_eq!(n.remove_node(a).err(), Some(NetworkError::UnknownNode(a)));
        n.reinsert(removed);
        assert_eq!(
            (
                n.nodes.clone(),
                n.order.clone(),
                n.markers.clone(),
                n.edges_near(0.0, 0.0, HUGE),
                n.in_rect(-HUGE, -HUGE, HUGE, HUGE),
            ),
            before
        );
        assert_eq!(n.nearest(0.1, 0.1), Some(a));
    }

    #[test]
    fn marker_deltas_revert() {
        let (mut n, a, b) = two();
        let d1 = n.set_marker(a, "A".into(), "g".into()).expect("ok");
        let d2 = n.set_marker(b, "B".into(), "g".into()).expect("ok");
        let d3 = n.set_marker(a, "A2".into(), "h".into()).expect("ok");
        let d4 = n.remove_marker(a).expect("ok");
        assert_eq!(n.markers().len(), 1);
        assert_eq!(n.remove_marker(a).expect("ok"), MarkerDelta::Unchanged);
        for d in [&d4, &d3] {
            n.revert_marker(d);
        }
        assert_eq!(n.markers()[0].name, "A");
        n.revert_marker(&d2);
        n.revert_marker(&d1);
        assert!(n.markers().is_empty());
        let ghost = NodeId(99);
        assert_eq!(
            n.set_marker(ghost, String::new(), String::new()),
            Err(NetworkError::UnknownNode(ghost))
        );
    }

    #[test]
    fn deleting_the_target_of_a_reverse_link_leaves_no_reference() {
        let (mut n, a, d) = two();
        n.connect(a, d, ConnectionShape::Reverse).expect("ok");
        let snap = |n: &Network| {
            (
                n.nodes.clone(),
                n.order.clone(),
                n.edges_near(0.0, 0.0, HUGE),
                n.in_rect(-HUGE, -HUGE, HUGE, HUGE),
            )
        };
        let before = snap(&n);
        let removed = n.remove_node(d).expect("ok");
        assert!(out(&n, a).is_empty());
        assert!(n.edges_near(0.0, 0.0, HUGE).is_empty());
        let after = snap(&n);
        n.reinsert(removed);
        assert_eq!(snap(&n), before);
        let removed = n.remove_node(d).expect("ok");
        assert_eq!(snap(&n), after);
        n.reinsert(removed);
        assert_eq!(snap(&n), before);
    }

    #[test]
    fn polygon_literal_shapes() {
        let mut n = Network::new();
        let mut add = |x: f64, z: f64| n.add_node(x, 0.0, z, NodeFlag::REGULAR);
        let inside_sq = add(5.0, 5.0);
        let outside_sq = add(15.0, 5.0);
        let left_arm = add(1.0, 5.0);
        let right_arm = add(9.0, 5.0);
        let notch = add(5.0, 8.0);
        let bottom = add(5.0, 1.0);
        let square = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        assert_eq!(
            n.in_polygon(&square),
            vec![inside_sq, left_arm, right_arm, notch, bottom]
        );
        // U shape opening towards +z: arms x 0..3 and 7..10, bottom z 0..3, notch x 3..7, z 3..10.
        let u = [
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 10.0),
            (7.0, 10.0),
            (7.0, 3.0),
            (3.0, 3.0),
            (3.0, 10.0),
            (0.0, 10.0),
        ];
        assert_eq!(n.in_polygon(&u), vec![left_arm, right_arm, bottom]);
        assert!(!n.in_polygon(&u).contains(&notch));
        assert!(!n.in_polygon(&u).contains(&outside_sq));
    }

    #[test]
    fn distance_bounds_are_inclusive() {
        let mut n = Network::new();
        let a = n.add_node(3.0, 0.0, 4.0, NodeFlag::REGULAR);
        let b = n.add_node(30.0, 0.0, 4.0, NodeFlag::REGULAR);
        let c = n.add_node(30.0, 0.0, 40.0, NodeFlag::REGULAR);
        n.connect(b, c, ConnectionShape::OneWay).expect("ok");
        assert_eq!(n.within(0.0, 0.0, 5.0), vec![a]);
        assert!(n.within(0.0, 0.0, 4.999).is_empty());
        // The segment b-c (x = 30) is exactly 25 away from (5, 20).
        assert_eq!(n.edges_near(5.0, 20.0, 25.0), vec![(b, c)]);
        assert!(n.edges_near(5.0, 20.0, 24.999).is_empty());
    }

    #[test]
    fn flag_keeps_any_value() {
        let mut n = Network::new();
        let a = n.add_node(0.0, 0.0, 0.0, NodeFlag(7));
        assert_eq!(n.node(a).map(|x| x.flag), Some(NodeFlag(7)));
    }

    #[test]
    fn ids_are_monotonic() {
        let (mut n, a, b) = two();
        let c = n.add_node(1.0, 1.0, 1.0, NodeFlag::REGULAR);
        assert!(a < b && b < c);
        assert_eq!(n.order(), &[a, b, c]);
    }

    #[test]
    fn from_loaded_rejects_bad_index_and_keeps_inconsistency() {
        let node = |out: Vec<usize>, inc: Vec<usize>| LoadedNode {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            flag: NodeFlag(0),
            outgoing: out,
            incoming: inc,
        };
        assert_eq!(
            Network::from_loaded(vec![node(vec![3], vec![])]).err(),
            Some(NetworkError::IndexOutOfRange { index: 3, len: 1 })
        );
        let n =
            Network::from_loaded(vec![node(vec![1, 1], vec![]), node(vec![], vec![])]).expect("ok");
        let a = n.order()[0];
        assert_eq!(out(&n, a).len(), 2);
        assert_eq!(n.edges_near(0.0, 0.0, HUGE).len(), 2);
    }

    /// Reference for Java's `getConnection`.
    fn expected_kind(p: bool, q: bool, r: bool, flag: u8) -> ConnectionKind {
        let crossed_regular = p && q && !r;
        let crossed_reverse = !p && q && r;
        let dual = p && q && r;
        let reverse = !p && !q && !r;
        let regular = p && !q && !r;
        if crossed_regular {
            ConnectionKind::CrossedRegular
        } else if crossed_reverse {
            ConnectionKind::CrossedReverse
        } else if dual {
            ConnectionKind::Dual
        } else if reverse {
            ConnectionKind::Reverse
        } else if regular && flag == 1 {
            ConnectionKind::SubPrio
        } else if regular && flag == 0 {
            ConnectionKind::Regular
        } else {
            ConnectionKind::Unknown
        }
    }

    #[test]
    fn kind_for_every_membership_combination() {
        for bits in 0..16u8 {
            let (a_out_b, p, q, r) = (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0);
            for flag in [0u8, 1, 7] {
                let mk = |f: u8| LoadedNode {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    flag: NodeFlag(f),
                    outgoing: vec![],
                    incoming: vec![],
                };
                // Build explicit lists: node 0 is a, node 1 is b.
                let mut a = mk(flag);
                let mut b = mk(0);
                if a_out_b {
                    a.outgoing.push(1);
                }
                if p {
                    b.incoming.push(0);
                }
                if q {
                    b.outgoing.push(0);
                }
                if r {
                    a.incoming.push(1);
                }
                let n = Network::from_loaded(vec![a, b]).expect("ok");
                let (ia, ib) = (n.order()[0], n.order()[1]);
                let want = if a_out_b {
                    expected_kind(p, q, r, flag)
                } else {
                    ConnectionKind::None
                };
                assert_eq!(
                    n.connection_kind(ia, ib),
                    want,
                    "bits {bits:04b} flag {flag}"
                );
            }
        }
    }

    #[test]
    fn crossed_and_unknown_examples() {
        let ld = |flag: u8, out: Vec<usize>, inc: Vec<usize>| LoadedNode {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            flag: NodeFlag(flag),
            outgoing: out,
            incoming: inc,
        };
        let kind = |a: LoadedNode, b: LoadedNode| {
            let n = Network::from_loaded(vec![a, b]).expect("ok");
            n.connection_kind(n.order()[0], n.order()[1])
        };
        assert_eq!(
            kind(ld(0, vec![1], vec![]), ld(0, vec![0], vec![0])),
            ConnectionKind::CrossedRegular
        );
        assert_eq!(
            kind(ld(0, vec![1], vec![1]), ld(0, vec![0], vec![])),
            ConnectionKind::CrossedReverse
        );
        assert_eq!(
            kind(ld(0, vec![1], vec![1]), ld(0, vec![], vec![])),
            ConnectionKind::Unknown
        );
        assert_eq!(
            kind(ld(7, vec![1], vec![]), ld(0, vec![], vec![0])),
            ConnectionKind::Unknown
        );
        assert_eq!(
            kind(ld(0, vec![], vec![]), ld(0, vec![], vec![])),
            ConnectionKind::None
        );
    }

    fn seg_dist2(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
        let (dx, dz) = (b.0 - a.0, b.1 - a.1);
        let len2 = dx * dx + dz * dz;
        let t = if len2 == 0.0 {
            0.0
        } else {
            (((p.0 - a.0) * dx + (p.1 - a.1) * dz) / len2).clamp(0.0, 1.0)
        };
        let (cx, cz) = (a.0 + t * dx, a.1 + t * dz);
        (p.0 - cx).powi(2) + (p.1 - cz).powi(2)
    }

    fn check_queries(n: &Network, rng: &mut Lcg) {
        let pos = |id: NodeId| n.node(id).map(|x| (x.x, x.z)).unwrap_or((0.0, 0.0));
        let mut all_edges = Vec::new();
        for id in n.order() {
            for t in n
                .node(*id)
                .map(|x| x.outgoing().to_vec())
                .unwrap_or_default()
            {
                all_edges.push((*id, t));
            }
        }
        all_edges.sort();
        assert_eq!(n.edges_near(0.0, 0.0, HUGE), all_edges);

        for _ in 0..40 {
            let (px, pz) = (rng.coord() + 0.37, rng.coord() + 0.61);
            let want = n
                .order()
                .iter()
                .map(|id| {
                    let (x, z) = pos(*id);
                    (x - px).powi(2) + (z - pz).powi(2)
                })
                .fold(f64::INFINITY, f64::min);
            let got = n.nearest(px, pz).map(|id| {
                let (x, z) = pos(id);
                (x - px).powi(2) + (z - pz).powi(2)
            });
            assert_eq!(got, Some(want));

            let (x0, z0, x1, z1) = (rng.coord(), rng.coord(), rng.coord(), rng.coord());
            let (lo_x, hi_x, lo_z, hi_z) = (x0.min(x1), x0.max(x1), z0.min(z1), z0.max(z1));
            let want: Vec<NodeId> = n
                .order()
                .iter()
                .copied()
                .filter(|id| {
                    let (x, z) = pos(*id);
                    x >= lo_x && x <= hi_x && z >= lo_z && z <= hi_z
                })
                .collect();
            let mut want = want;
            want.sort();
            assert_eq!(n.in_rect(x0, z0, x1, z1), want);

            let poly: Vec<(f64, f64)> = (0..5)
                .map(|_| (rng.coord() + 0.5, rng.coord() + 0.5))
                .collect();
            let mut want: Vec<NodeId> = n
                .order()
                .iter()
                .copied()
                .filter(|id| {
                    let (x, z) = pos(*id);
                    point_in_polygon(x, z, &poly)
                })
                .collect();
            want.sort();
            assert_eq!(n.in_polygon(&poly), want);

            let d = 7.3 + rng.below(20) as f64;
            let mut want: Vec<NodeId> = n
                .order()
                .iter()
                .copied()
                .filter(|id| {
                    let (x, z) = pos(*id);
                    (x - px).powi(2) + (z - pz).powi(2) <= d * d
                })
                .collect();
            want.sort();
            assert_eq!(n.within(px, pz, d), want);

            let want: Vec<(NodeId, NodeId)> = all_edges
                .iter()
                .copied()
                .filter(|(a, b)| seg_dist2((px, pz), pos(*a), pos(*b)) <= d * d)
                .collect();
            assert_eq!(n.edges_near(px, pz, d), want);
        }
    }

    #[test]
    fn queries_match_brute_force_after_connects_and_moves() {
        let mut rng = Lcg(42);
        let mut n = Network::new();
        let ids: Vec<NodeId> = (0..500)
            .map(|_| n.add_node(rng.coord(), 0.0, rng.coord(), NodeFlag::REGULAR))
            .collect();
        let pick = |rng: &mut Lcg| ids[rng.below(ids.len() as u64) as usize];
        for _ in 0..800 {
            let (a, b) = (pick(&mut rng), pick(&mut rng));
            let shape = match rng.below(3) {
                0 => ConnectionShape::OneWay,
                1 => ConnectionShape::Dual,
                _ => ConnectionShape::Reverse,
            };
            let _ = n.connect(a, b, shape);
        }
        check_queries(&n, &mut rng);
        for _ in 0..200 {
            let id = pick(&mut rng);
            let _ = n.move_node(id, rng.coord(), 1.0, rng.coord());
        }
        for _ in 0..100 {
            let (a, b) = (pick(&mut rng), pick(&mut rng));
            let _ = n.disconnect(a, b);
            let (a, b) = (pick(&mut rng), pick(&mut rng));
            let _ = n.set_shape(a, b, ConnectionShape::OneWay);
        }
        check_queries(&n, &mut rng);

        let loaded: Vec<LoadedNode> = n
            .order()
            .iter()
            .filter_map(|id| n.node(*id))
            .map(|x| LoadedNode {
                x: x.x,
                y: x.y,
                z: x.z,
                flag: x.flag,
                outgoing: x.outgoing().iter().map(|t| t.get() as usize).collect(),
                incoming: x.incoming().iter().map(|t| t.get() as usize).collect(),
            })
            .collect();
        let reloaded = Network::from_loaded(loaded).expect("ok");
        check_queries(&reloaded, &mut rng);
    }
}
