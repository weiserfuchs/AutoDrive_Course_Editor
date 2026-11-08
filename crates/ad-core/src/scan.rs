//! Network scans that change nothing: they report nodes for the user to look at. The
//! merge of overlapping nodes, the move of nodes outside the map to its edge, the fill of
//! missing heights and the removal of broken connection entries are planned here as one
//! transaction each for the caller to apply.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::command::{
    Command, Connect, DeleteNodes, MoveNodes, Position, RemoveLinkEntries, Transaction,
};
use crate::geometry::{MAP_PIXELS, MapGeometry};
use crate::heightmap::sample_height;
use crate::map_context::MapContext;
use crate::network::{ConnectionShape, Network, NodeId, Side};

/// How far apart in height two overlapping nodes may be. Stored heights of a road and of a
/// copy pasted onto it differ by up to about 2 m (the heightmap sample is not the stored
/// height), while bridges and crossings on two levels are several metres apart.
pub const HEIGHT_TOLERANCE: f64 = 2.0;

/// Every node with at least one other node within `distance` of it in (x, z) and within
/// [`HEIGHT_TOLERANCE`] in height, both inclusive. Java (`ScanManager.
/// scanNetworkForOverlapNodes`) measured 3D and ignored the typed distance (AD-13).
pub fn overlaps(net: &Network, distance: f64) -> BTreeSet<NodeId> {
    net.order()
        .iter()
        .copied()
        .filter(|&id| overlapping(net, id, distance))
        .collect()
}

/// Whether another node lies within `distance` of `id` in (x, z), as the R-tree finds it,
/// and within [`HEIGHT_TOLERANCE`] in height. False for a missing node.
fn overlapping(net: &Network, id: NodeId, distance: f64) -> bool {
    let Some(n) = net.node(id) else {
        return false;
    };
    net.within(n.x, n.z, distance).into_iter().any(|other| {
        other != id
            && net
                .node(other)
                .is_some_and(|o| (o.y - n.y).abs() <= HEIGHT_TOLERANCE)
    })
}

/// The nodes `tx` adds, removes or moves, as far as their ids are known (the ids of added
/// nodes are known once it has been applied). The other commands move no node.
pub fn placed_nodes(tx: &Transaction) -> Vec<NodeId> {
    let mut ids = Vec::new();
    for command in &tx.0 {
        match command {
            Command::AddNodes(c) => ids.extend_from_slice(c.ids()),
            Command::PasteNodes(c) => ids.extend_from_slice(c.ids()),
            Command::DeleteNodes(c) => ids.extend_from_slice(&c.ids),
            Command::MoveNodes(c) => ids.extend(c.moves.iter().map(|m| m.0)),
            Command::Connect(_)
            | Command::Disconnect(_)
            | Command::EditMarker(_)
            | Command::RenameGroup(_)
            | Command::SetFlag(_)
            | Command::RemoveLinkEntries(_) => {}
        }
    }
    ids
}

/// `nodes` and every node within `distance` (in x, z) of where they are now: the nodes
/// whose mark can change when `nodes` move, appear or disappear. Taken once before and
/// once after the change, it covers their old and their new neighbours.
pub fn around(net: &Network, nodes: &[NodeId], distance: f64) -> BTreeSet<NodeId> {
    let mut out = BTreeSet::new();
    for &id in nodes {
        out.insert(id);
        if let Some(n) = net.node(id) {
            out.extend(net.within(n.x, n.z, distance));
        }
    }
    out
}

/// Recomputes the mark of each of `nodes` in `marks`; a missing node loses its mark. With
/// `nodes` from [`around`] before and after a change, `marks` then equals
/// `overlaps(net, distance)` when it did before.
pub fn remark(
    net: &Network,
    distance: f64,
    marks: &mut BTreeSet<NodeId>,
    nodes: impl IntoIterator<Item = NodeId>,
) {
    for id in nodes {
        if overlapping(net, id, distance) {
            marks.insert(id);
        } else {
            marks.remove(&id);
        }
    }
}

/// Marked nodes that overlap each other, joined transitively: `keep` stays, `removed` are
/// merged into it, in network order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub keep: NodeId,
    pub removed: Vec<NodeId>,
}

/// The root of `i` in a union-find forest, halving the path on the way.
fn root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// The groups of `marks`: two marked nodes are in one group when they overlap by the scan's
/// rule (within `distance` in x/z and [`HEIGHT_TOLERANCE`] in height), and so on
/// transitively. Each group keeps its first node in network order that has a marker, else
/// its first node. Groups come in network order of their first node; a marked node without
/// a marked neighbour is a group of its own with nothing removed.
pub fn overlap_groups(net: &Network, distance: f64, marks: &BTreeSet<NodeId>) -> Vec<Group> {
    let members: Vec<NodeId> = net
        .order()
        .iter()
        .copied()
        .filter(|id| marks.contains(id))
        .collect();
    let index: HashMap<NodeId, usize> =
        members.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let mut parent: Vec<usize> = (0..members.len()).collect();
    for (i, id) in members.iter().enumerate() {
        let Some(n) = net.node(*id) else {
            continue;
        };
        for other in net.within(n.x, n.z, distance) {
            let Some(&j) = index.get(&other) else {
                continue;
            };
            let close = net
                .node(other)
                .is_some_and(|o| (o.y - n.y).abs() <= HEIGHT_TOLERANCE);
            if j == i || !close {
                continue;
            }
            let (a, b) = (root(&mut parent, i), root(&mut parent, j));
            // The lower index becomes the root, so a group's root is its first node.
            parent[a.max(b)] = a.min(b);
        }
    }
    let with_marker: HashSet<NodeId> = net.markers().iter().map(|m| m.node).collect();
    let mut groups: Vec<Vec<NodeId>> = Vec::new();
    let mut slot: HashMap<usize, usize> = HashMap::new();
    for (i, id) in members.iter().enumerate() {
        let r = root(&mut parent, i);
        let g = *slot.entry(r).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[g].push(*id);
    }
    groups
        .into_iter()
        .filter_map(|nodes| {
            let keep = nodes
                .iter()
                .copied()
                .find(|id| with_marker.contains(id))
                .or_else(|| nodes.first().copied())?;
            Some(Group {
                keep,
                removed: nodes.into_iter().filter(|id| *id != keep).collect(),
            })
        })
        .collect()
}

/// The merge of `groups` as one transaction, and the names of the markers deleted with the
/// removed nodes, in marker order. Every link between a removed node and a node outside its
/// group is recreated with its direction and shape ([`Network::link_shape`]) between the
/// kept nodes of its ends (a node in no group stands for itself), unless it is there
/// already; links inside a group are dropped. Then the removed nodes are deleted with their
/// links and markers. No node moves.
pub fn merge_plan(net: &Network, groups: &[Group]) -> (Transaction, Vec<String>) {
    let kept: HashMap<NodeId, NodeId> = groups
        .iter()
        .flat_map(|g| g.removed.iter().map(move |r| (*r, g.keep)))
        .collect();
    let rep = |id: NodeId| kept.get(&id).copied().unwrap_or(id);
    let mut planned = HashSet::new();
    let mut commands = Vec::new();
    // Every node's outgoing list, as a reverse link into a removed node is listed only there.
    for &a in net.order() {
        let Some(node) = net.node(a) else {
            continue;
        };
        for &b in node.outgoing() {
            if !kept.contains_key(&a) && !kept.contains_key(&b) {
                continue;
            }
            let Some(shape) = net.link_shape(a, b) else {
                continue;
            };
            // A dual link is listed from both ends; it is taken once, from the lower id.
            if shape == ConnectionShape::Dual && a > b {
                continue;
            }
            let (ka, kb) = (rep(a), rep(b));
            if ka == kb || has(net, ka, kb, shape) {
                continue;
            }
            let key = match shape {
                ConnectionShape::Dual => (ka.min(kb), ka.max(kb), shape),
                _ => (ka, kb, shape),
            };
            if planned.insert(key) {
                commands.push(Command::Connect(Connect::new(ka, kb, shape)));
            }
        }
    }
    let removed: Vec<NodeId> = net
        .order()
        .iter()
        .copied()
        .filter(|id| kept.contains_key(id))
        .collect();
    let markers = net
        .markers()
        .iter()
        .filter(|m| kept.contains_key(&m.node))
        .map(|m| m.name.clone())
        .collect();
    if !removed.is_empty() {
        commands.push(Command::DeleteNodes(DeleteNodes::new(removed)));
    }
    (Transaction::new(commands), markers)
}

/// Whether every list entry `connect(a, b, shape)` would add is there already.
fn has(net: &Network, a: NodeId, b: NodeId, shape: ConnectionShape) -> bool {
    let out = |x: NodeId, y: NodeId| net.node(x).is_some_and(|n| n.outgoing().contains(&y));
    let inc = |x: NodeId, y: NodeId| net.node(x).is_some_and(|n| n.incoming().contains(&y));
    match shape {
        ConnectionShape::Reverse => out(a, b),
        ConnectionShape::OneWay => out(a, b) && inc(b, a),
        ConnectionShape::Dual => out(a, b) && inc(b, a) && out(b, a) && inc(a, b),
    }
}

/// The height of a node saved without one (AutoDrive's and Java's `-1`).
pub const NO_HEIGHT: f64 = -1.0;

/// `moves` as one transaction; empty when there are none.
fn moving(moves: Vec<(NodeId, Position, Position)>) -> Transaction {
    if moves.is_empty() {
        return Transaction::new(Vec::new());
    }
    Transaction::new(vec![Command::MoveNodes(MoveNodes { moves })])
}

/// Half the side of the map in world units: x and z run from minus this to this.
pub fn map_half_size(geometry: MapGeometry) -> f64 {
    MAP_PIXELS / 2.0 * geometry.scale
}

/// Moves every node outside the map (x or z beyond [`map_half_size`]) to the nearest point
/// of the map edge, x and z clamped and y kept, as one transaction of absolute moves in
/// network order; empty when every node is inside. A node on the edge is inside. Java
/// (`OutOfBoundsFixMenu`) moved them all to (0, 0).
pub fn out_of_bounds_plan(net: &Network, geometry: MapGeometry) -> Transaction {
    let half = map_half_size(geometry);
    let moves = net
        .order()
        .iter()
        .filter_map(|&id| {
            let n = net.node(id)?;
            let (x, z) = (n.x.clamp(-half, half), n.z.clamp(-half, half));
            ((x, z) != (n.x, n.z)).then_some((id, (n.x, n.y, n.z), (x, n.y, z)))
        })
        .collect();
    moving(moves)
}

/// Gives every node without a height ([`NO_HEIGHT`]) the terrain height at its place, as
/// one transaction of absolute moves in network order; empty when no node lacks a height
/// or the map has no heightmap. x and z stay.
pub fn missing_heights_plan(net: &Network, map: &MapContext) -> Transaction {
    let moves = net
        .order()
        .iter()
        .filter_map(|&id| {
            let n = net.node(id).filter(|n| n.y == NO_HEIGHT)?;
            let y = sample_height(map, n.x, n.z)?;
            Some((id, (n.x, n.y, n.z), (n.x, y, n.z)))
        })
        .collect();
    moving(moves)
}

/// The broken adjacency entries of a network, in network order. A reverse link (`a.out`
/// lists `b`, `b.in` does not list `a`) is valid and never reported.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Defects {
    /// `(b, a)`: `b.in` lists `a`, but `a.out` does not list `b` (or `a` is gone). Once
    /// per pair.
    pub dangling: Vec<(NodeId, NodeId)>,
    /// `(node, side, value)`: `value` more than once in the list. Once per value.
    pub duplicates: Vec<(NodeId, Side, NodeId)>,
    /// `(node, side)`: the node lists itself. Once per list.
    pub self_links: Vec<(NodeId, Side)>,
}

impl Defects {
    pub fn is_empty(&self) -> bool {
        self.dangling.is_empty() && self.duplicates.is_empty() && self.self_links.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Defect {
    SelfLink,
    Duplicate,
    Dangling,
}

/// Calls `f(node, side, index, value, defect, earlier)` for every broken entry; `earlier`
/// counts the entries before it in the same list that hold `value`. An entry has at most
/// one defect: a node's own id is a self-link, a repeat of another id a duplicate, and the
/// first occurrence of an incoming id may be dangling.
fn each_defect(net: &Network, mut f: impl FnMut(NodeId, Side, usize, NodeId, Defect, usize)) {
    for &id in net.order() {
        let Some(n) = net.node(id) else {
            continue;
        };
        for (side, list) in [(Side::Out, n.outgoing()), (Side::In, n.incoming())] {
            for (i, &value) in list.iter().enumerate() {
                let earlier = list[..i].iter().filter(|v| **v == value).count();
                let defect = if value == id {
                    Defect::SelfLink
                } else if earlier > 0 {
                    Defect::Duplicate
                } else if side == Side::In
                    && !net.node(value).is_some_and(|a| a.outgoing().contains(&id))
                {
                    Defect::Dangling
                } else {
                    continue;
                };
                f(id, side, i, value, defect, earlier);
            }
        }
    }
}

/// The dangling incoming, duplicate and self entries of `net` (AD-3: the loader keeps
/// them, this check reports them).
pub fn connection_defects(net: &Network) -> Defects {
    let mut d = Defects::default();
    each_defect(net, |node, side, _, value, defect, earlier| match defect {
        Defect::SelfLink if earlier == 0 => d.self_links.push((node, side)),
        Defect::Duplicate if earlier == 1 => d.duplicates.push((node, side, value)),
        Defect::Dangling => d.dangling.push((node, value)),
        _ => {}
    });
    d
}

/// Removes every entry [`connection_defects`] reports: self-links, every occurrence after
/// the first of a duplicate, and dangling incoming entries, as one transaction of one
/// [`RemoveLinkEntries`]; empty when there are none. Adds no link and removes no node.
pub fn connection_fix_plan(net: &Network) -> Transaction {
    let mut entries = Vec::new();
    each_defect(net, |node, side, index, _, _, _| {
        entries.push((node, side, index));
    });
    if entries.is_empty() {
        return Transaction::new(Vec::new());
    }
    Transaction::new(vec![Command::RemoveLinkEntries(RemoveLinkEntries::new(
        entries,
    ))])
}

/// The nodes whose lists `tx` changes through [`RemoveLinkEntries`], in id order.
pub fn relinked_nodes(tx: &Transaction) -> Vec<NodeId> {
    let mut ids: Vec<NodeId> =
        tx.0.iter()
            .filter_map(|c| match c {
                Command::RemoveLinkEntries(c) => Some(c.nodes()),
                _ => None,
            })
            .flatten()
            .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::network::NodeFlag;

    fn net(points: &[(f64, f64, f64)]) -> (Network, Vec<NodeId>) {
        let mut n = Network::new();
        let ids = points
            .iter()
            .map(|&(x, y, z)| n.add_node(x, y, z, NodeFlag::REGULAR))
            .collect();
        (n, ids)
    }

    #[test]
    fn a_pair_closer_than_the_distance_is_marked() {
        let (n, ids) = net(&[(10.0, 1.0, 10.0), (10.05, 1.0, 10.0), (20.0, 1.0, 20.0)]);
        assert_eq!(overlaps(&n, 0.1), BTreeSet::from([ids[0], ids[1]]));
    }

    #[test]
    fn height_counts_only_beyond_the_tolerance() {
        // Two levels (a bridge): apart.
        let (n, _) = net(&[(10.0, 0.0, 10.0), (10.0, 5.0, 10.0)]);
        assert!(overlaps(&n, 0.1).is_empty());
        let (n, _) = net(&[(10.0, 0.0, 10.0), (10.0, 2.001, 10.0)]);
        assert!(overlaps(&n, 0.1).is_empty());
        // A pasted copy whose height came from the heightmap (1.65 m off): overlapping.
        let (n, ids) = net(&[(10.0, 0.0, 10.0), (10.0, 1.65, 10.0)]);
        assert_eq!(overlaps(&n, 0.1), BTreeSet::from([ids[0], ids[1]]));
        let (n, ids) = net(&[(10.0, 0.0, 10.0), (10.05, 2.0, 10.0)]);
        assert_eq!(overlaps(&n, 0.1), BTreeSet::from([ids[0], ids[1]]));
    }

    #[test]
    fn the_distance_decides() {
        let (n, ids) = net(&[(0.0, 0.0, 0.0), (0.3, 0.4, 0.4)]);
        assert!(overlaps(&n, 0.1).is_empty());
        assert_eq!(overlaps(&n, 1.0), BTreeSet::from([ids[0], ids[1]]));
        // Inclusive: exactly 0.5 apart.
        assert_eq!(overlaps(&n, 0.5), BTreeSet::from([ids[0], ids[1]]));
    }

    #[test]
    fn a_group_is_marked_whole() {
        let (n, ids) = net(&[
            (5.0, 2.0, 5.0),
            (5.03, 2.0, 5.0),
            (5.0, 2.03, 5.03),
            (6.0, 2.0, 5.0),
        ]);
        assert_eq!(overlaps(&n, 0.1), BTreeSet::from([ids[0], ids[1], ids[2]]));
    }

    /// Marks kept with `around` before and after each apply and revert equal a full scan.
    #[test]
    fn marks_kept_around_placed_nodes_equal_a_full_scan() {
        use crate::command::{AddNodes, DeleteNodes, MoveNodes};
        let mut seed = 11u64;
        let mut next = |n: u64| {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) % n
        };
        // Coordinates on a 0.05 grid in a small square, so pairs within 0.1 are common.
        let coord = |next: &mut dyn FnMut(u64) -> u64| next(40) as f64 * 0.05;
        let d = 0.1;
        let mut n = Network::new();
        for _ in 0..60 {
            let (x, z) = (coord(&mut next), coord(&mut next));
            n.add_node(x, 0.0, z, NodeFlag::REGULAR);
        }
        let mut marks = overlaps(&n, d);
        let mut done: Vec<Transaction> = Vec::new();
        for step in 0..400 {
            let ids = n.order().to_vec();
            let pick = |next: &mut dyn FnMut(u64) -> u64| ids[next(ids.len() as u64) as usize];
            if step % 5 == 4
                && let Some(mut tx) = done.pop()
            {
                let before = around(&n, &placed_nodes(&tx), d);
                tx.revert(&mut n).expect("reverts");
                remark(&n, d, &mut marks, before);
                remark(&n, d, &mut marks, around(&n, &placed_nodes(&tx), d));
                assert_eq!(marks, overlaps(&n, d), "revert at step {step}");
                continue;
            }
            let command = match next(3) {
                0 => {
                    let (x, z) = (coord(&mut next), coord(&mut next));
                    Command::AddNodes(AddNodes::new(vec![(x, 0.0, z, NodeFlag::REGULAR)]))
                }
                1 if ids.len() > 10 => {
                    Command::DeleteNodes(DeleteNodes::new(vec![pick(&mut next)]))
                }
                _ => {
                    let id = pick(&mut next);
                    let p = n.node(id).map(|p| (p.x, p.y, p.z)).expect("node");
                    let to = (coord(&mut next), 0.0, coord(&mut next));
                    Command::MoveNodes(MoveNodes {
                        moves: vec![(id, p, to)],
                    })
                }
            };
            let mut tx = Transaction::new(vec![command]);
            let before = around(&n, &placed_nodes(&tx), d);
            tx.apply(&mut n).expect("applies");
            remark(&n, d, &mut marks, before);
            remark(&n, d, &mut marks, around(&n, &placed_nodes(&tx), d));
            assert_eq!(marks, overlaps(&n, d), "apply at step {step}");
            done.push(tx);
        }
        assert!(!marks.is_empty());
    }

    use crate::command::tests::{round_trip, snap};
    use crate::network::{ConnectionKind, ConnectionShape, Marker};

    /// Groups and plan of every node marked at 0.1.
    fn plan(n: &Network) -> (Vec<Group>, Transaction, Vec<String>) {
        let groups = overlap_groups(n, 0.1, &overlaps(n, 0.1));
        let (tx, markers) = merge_plan(n, &groups);
        (groups, tx, markers)
    }

    fn lists(n: &Network, id: NodeId) -> (Vec<NodeId>, Vec<NodeId>) {
        let x = n.node(id).expect("node");
        (x.outgoing().to_vec(), x.incoming().to_vec())
    }

    fn marker(n: &mut Network, node: NodeId, name: &str) {
        n.push_marker(Marker {
            node,
            name: name.into(),
            group: "g".into(),
            parked_vehicles: Vec::new(),
        });
    }

    #[test]
    fn a_chain_pasted_in_place_merges_into_the_original_without_new_links() {
        // Original A -> B -> C and A -> X; the copy a -> b -> c on top, added later.
        let (mut n, ids) = net(&[
            (0.0, 0.0, 0.0),
            (10.0, 0.0, 0.0),
            (20.0, 0.0, 0.0),
            (0.0, 0.0, 30.0),
            (0.0, 0.5, 0.0),
            (10.0, 0.5, 0.0),
            (20.0, 0.5, 0.0),
        ]);
        let [a0, b0, c0, x, a, b, c] = ids[..] else {
            panic!("seven nodes");
        };
        for (p, q) in [(a0, b0), (b0, c0), (a0, x), (a, b), (b, c)] {
            n.connect(p, q, ConnectionShape::OneWay).expect("ok");
        }
        let before: Vec<_> = [a0, b0, c0, x].iter().map(|&id| lists(&n, id)).collect();
        let (groups, tx, markers) = plan(&n);
        let keeps: Vec<NodeId> = groups.iter().map(|g| g.keep).collect();
        assert_eq!(keeps, [a0, b0, c0]);
        assert!(markers.is_empty());
        assert!(
            !tx.0.iter().any(|c| matches!(c, Command::Connect(_))),
            "every link is there already"
        );
        let (n, _) = round_trip(n, tx);
        assert_eq!(n.order(), &[a0, b0, c0, x]);
        let after: Vec<_> = [a0, b0, c0, x].iter().map(|&id| lists(&n, id)).collect();
        assert_eq!(after, before);
    }

    /// `k` with `r` on it; outside nodes p, q, s, t, u, v. r has one-way p -> r, one-way
    /// r -> q, reverse r -> s, reverse t -> r, dual r <-> u; k has its own one-way k -> v.
    /// Markers "K" on k (vehicle "v1" parked) and "R" on r (vehicle "v2").
    fn shapes() -> (Network, [NodeId; 8]) {
        let (mut n, ids) = net(&[
            (0.0, 0.0, 0.0),
            (0.05, 0.0, 0.0),
            (-10.0, 0.0, 0.0),
            (10.0, 0.0, 0.0),
            (0.0, 0.0, 10.0),
            (0.0, 0.0, -10.0),
            (10.0, 0.0, 10.0),
            (-10.0, 0.0, -10.0),
        ]);
        let [k, r, p, q, s, t, u, v] = ids[..] else {
            panic!("eight nodes");
        };
        n.connect(p, r, ConnectionShape::OneWay).expect("ok");
        n.connect(r, q, ConnectionShape::OneWay).expect("ok");
        n.connect(r, s, ConnectionShape::Reverse).expect("ok");
        n.connect(t, r, ConnectionShape::Reverse).expect("ok");
        n.connect(r, u, ConnectionShape::Dual).expect("ok");
        n.connect(k, v, ConnectionShape::OneWay).expect("ok");
        marker(&mut n, k, "K");
        marker(&mut n, r, "R");
        assert!(n.park_vehicle(0, "v1".into()));
        assert!(n.park_vehicle(1, "v2".into()));
        (n, [k, r, p, q, s, t, u, v])
    }

    #[test]
    fn every_link_shape_is_recreated_on_the_kept_node_and_reverts_exactly() {
        let (n, [k, r, p, q, s, t, u, v]) = shapes();
        let (groups, tx, markers) = plan(&n);
        assert_eq!(
            groups,
            [Group {
                keep: k,
                removed: vec![r]
            }]
        );
        assert_eq!(markers, ["R"]);
        // `round_trip`: undo restores every node, link, marker and parked vehicle exactly.
        let (n, _) = round_trip(n, tx);
        assert!(n.node(r).is_none());
        assert_eq!(n.connection_kind(p, k), ConnectionKind::Regular);
        assert_eq!(n.connection_kind(k, q), ConnectionKind::Regular);
        assert_eq!(n.connection_kind(k, s), ConnectionKind::Reverse);
        assert!(!lists(&n, s).1.contains(&k));
        assert_eq!(n.connection_kind(t, k), ConnectionKind::Reverse);
        assert!(!lists(&n, k).1.contains(&t));
        assert_eq!(n.connection_kind(k, u), ConnectionKind::Dual);
        assert_eq!(n.connection_kind(k, v), ConnectionKind::Regular);
        let (out, inc) = lists(&n, k);
        assert_eq!(out, [v, q, s, u]);
        assert_eq!(inc, [u, p]);
        assert_eq!(n.markers().len(), 1);
        assert_eq!(n.markers()[0].parked_vehicles, ["v1"]);
        assert!(
            n.node(k).is_some_and(|x| (x.x, x.z) == (0.0, 0.0)),
            "not moved"
        );
    }

    #[test]
    fn links_inside_a_group_are_dropped_without_self_links() {
        let (mut n, ids) = net(&[(0.0, 0.0, 0.0), (0.05, 0.0, 0.0), (0.0, 0.0, 0.05)]);
        let [k, r1, r2] = ids[..] else {
            panic!("three nodes");
        };
        n.connect(k, r1, ConnectionShape::Dual).expect("ok");
        n.connect(r1, r2, ConnectionShape::OneWay).expect("ok");
        n.connect(r2, k, ConnectionShape::Reverse).expect("ok");
        let (_, tx, _) = plan(&n);
        assert!(!tx.0.iter().any(|c| matches!(c, Command::Connect(_))));
        let (n, _) = round_trip(n, tx);
        assert_eq!(n.order(), &[k]);
        assert_eq!(lists(&n, k), (vec![], vec![]));
    }

    #[test]
    fn the_node_with_the_marker_is_kept_and_of_two_the_first() {
        let points = [(0.0, 0.0, 0.0), (0.05, 0.0, 0.0), (0.0, 0.0, 0.05)];
        let (mut n, ids) = net(&points);
        marker(&mut n, ids[1], "M");
        let (groups, _, markers) = plan(&n);
        assert_eq!(groups[0].keep, ids[1]);
        assert_eq!(groups[0].removed, [ids[0], ids[2]]);
        assert!(markers.is_empty());
        // Markers on the third and second node: the second (first in network order) stays.
        let (mut n, ids) = net(&points);
        marker(&mut n, ids[2], "Third");
        marker(&mut n, ids[1], "Second");
        let (groups, tx, markers) = plan(&n);
        assert_eq!(groups[0].keep, ids[1]);
        assert_eq!(markers, ["Third"]);
        let (n, _) = round_trip(n, tx);
        assert_eq!(n.markers().len(), 1);
        assert_eq!(
            (n.markers()[0].node, &*n.markers()[0].name),
            (ids[1], "Second")
        );
    }

    #[test]
    fn three_nodes_overlapping_transitively_are_one_group() {
        // The ends are 0.16 apart, each 0.08 from the middle one.
        let (n, ids) = net(&[(0.0, 0.0, 0.0), (0.08, 0.0, 0.0), (0.16, 0.0, 0.0)]);
        let (groups, tx, _) = plan(&n);
        assert_eq!(
            groups,
            [Group {
                keep: ids[0],
                removed: vec![ids[1], ids[2]]
            }]
        );
        let (n, _) = round_trip(n, tx);
        assert_eq!(n.order(), &[ids[0]]);
    }

    #[test]
    fn links_between_two_groups_join_their_kept_nodes_and_unmarked_nodes_stay() {
        // Pairs (k1, r1) and (k2, r2), r1 -> r2 one-way; w overlaps k1 but is not marked.
        let (mut n, ids) = net(&[
            (0.0, 0.0, 0.0),
            (10.0, 0.0, 0.0),
            (0.05, 0.0, 0.0),
            (10.05, 0.0, 0.0),
            (0.0, 0.0, 0.05),
        ]);
        let [k1, k2, r1, r2, w] = ids[..] else {
            panic!("five nodes");
        };
        n.connect(r1, r2, ConnectionShape::OneWay).expect("ok");
        let mut marks = overlaps(&n, 0.1);
        marks.remove(&w);
        let groups = overlap_groups(&n, 0.1, &marks);
        let (tx, _) = merge_plan(&n, &groups);
        let (n, _) = round_trip(n, tx);
        assert_eq!(n.order(), &[k1, k2, w]);
        assert_eq!(n.connection_kind(k1, k2), ConnectionKind::Regular);
    }

    #[test]
    fn an_empty_marking_plans_nothing() {
        let (n, _) = shapes();
        let groups = overlap_groups(&n, 0.1, &BTreeSet::new());
        assert!(groups.is_empty());
        let before = snap(&n);
        let (mut tx, markers) = merge_plan(&n, &groups);
        assert!(tx.0.is_empty() && markers.is_empty());
        let mut n = n;
        tx.apply(&mut n).expect("applies");
        assert_eq!(snap(&n), before);
    }

    #[test]
    fn an_empty_network_has_none() {
        assert!(overlaps(&Network::new(), 0.1).is_empty());
    }

    fn positions(n: &Network) -> Vec<Position> {
        n.order()
            .iter()
            .map(|id| n.node(*id).map(|p| (p.x, p.y, p.z)).expect("node"))
            .collect()
    }

    #[test]
    fn nodes_outside_the_map_move_to_its_edge_and_back_on_revert() {
        let (n, ids) = net(&[
            (1500.0, 5.0, 0.0),
            (-2000.0, 5.0, 300.0),
            (100.0, 5.0, 1100.0),
            (1200.0, 5.0, -1300.0),
            (10.0, 5.0, 10.0),
        ]);
        let tx = out_of_bounds_plan(&n, MapGeometry::new(1.0));
        assert_eq!(placed_nodes(&tx), ids[..4]);
        let (n, _) = round_trip(n, tx);
        assert_eq!(
            positions(&n),
            [
                (1024.0, 5.0, 0.0),
                (-1024.0, 5.0, 300.0),
                (100.0, 5.0, 1024.0),
                (1024.0, 5.0, -1024.0),
                (10.0, 5.0, 10.0),
            ]
        );
    }

    #[test]
    fn the_bounds_grow_with_the_scale_and_the_edge_is_inside() {
        let (n, _) = net(&[(2000.0, 0.0, 0.0)]);
        assert!(out_of_bounds_plan(&n, MapGeometry::new(2.0)).0.is_empty());
        assert!(!out_of_bounds_plan(&n, MapGeometry::new(1.0)).0.is_empty());
        let (n, _) = net(&[(1024.0, 0.0, -1024.0), (-1024.0, 0.0, 1024.0)]);
        assert!(out_of_bounds_plan(&n, MapGeometry::new(1.0)).0.is_empty());
    }

    #[test]
    fn missing_heights_are_sampled_and_other_heights_stay() {
        use crate::heightmap::Heightmap;
        let (n, ids) = net(&[(0.0, -1.0, 0.0), (5.0, 3.0, 5.0), (-5.0, -1.0, 7.0)]);
        let mut map = MapContext::new(None, 1, 2.0, (2.0, 2.0));
        // 100 m everywhere.
        let hm = Heightmap::new(4, 4, vec![25600; 16]).expect("size");
        map.heightmap = Some(std::sync::Arc::new(hm));
        let tx = missing_heights_plan(&n, &map);
        assert_eq!(placed_nodes(&tx), [ids[0], ids[2]]);
        let (n, _) = round_trip(n, tx);
        assert_eq!(
            positions(&n),
            [(0.0, 100.0, 0.0), (5.0, 3.0, 5.0), (-5.0, 100.0, 7.0)]
        );
        // Without a missing height, or without a heightmap, nothing is planned.
        let (n, _) = net(&[(0.0, 0.0, 0.0), (1.0, -0.5, 1.0)]);
        assert!(missing_heights_plan(&n, &map).0.is_empty());
        map.heightmap = None;
        let (n, _) = net(&[(0.0, -1.0, 0.0)]);
        assert!(missing_heights_plan(&n, &map).0.is_empty());
    }

    /// Nodes 10 apart on x with the given `(outgoing, incoming)` lists as order indexes.
    fn with_lists(nodes: &[(&[usize], &[usize])]) -> (Network, Vec<NodeId>) {
        use crate::network::LoadedNode;
        let loaded = nodes
            .iter()
            .enumerate()
            .map(|(i, (out, inc))| LoadedNode {
                x: 10.0 * i as f64,
                y: 0.0,
                z: 0.0,
                flag: NodeFlag::REGULAR,
                outgoing: out.to_vec(),
                incoming: inc.to_vec(),
            })
            .collect();
        let n = Network::from_loaded(loaded).expect("loads");
        let ids = n.order().to_vec();
        (n, ids)
    }

    #[test]
    fn a_dangling_incoming_entry_is_reported_and_removed() {
        // b.in = [a, c]: a does not list b, c does.
        let (n, ids) = with_lists(&[(&[], &[]), (&[], &[0, 2]), (&[1], &[])]);
        let [a, b, c] = [ids[0], ids[1], ids[2]];
        let d = connection_defects(&n);
        assert_eq!(d.dangling, [(b, a)]);
        assert!(d.duplicates.is_empty() && d.self_links.is_empty());
        let tx = connection_fix_plan(&n);
        assert_eq!(relinked_nodes(&tx), [b]);
        assert!(placed_nodes(&tx).is_empty());
        let (n, _) = round_trip(n, tx);
        assert_eq!(lists(&n, b), (vec![], vec![c]));
        assert_eq!(lists(&n, c), (vec![b], vec![]));
        assert!(connection_defects(&n).is_empty());
        // An entry naming a deleted node is dangling too.
        let (mut n, ids) = with_lists(&[(&[], &[]), (&[], &[0])]);
        n.remove_node(ids[0]).expect("remove");
        assert_eq!(connection_defects(&n).dangling, [(ids[1], ids[0])]);
        let (n, _) = round_trip(n.clone(), connection_fix_plan(&n));
        assert!(connection_defects(&n).is_empty());
    }

    #[test]
    fn reverse_one_way_and_dual_links_are_not_reported() {
        // a -> b reverse, b -> c one-way, c <-> d dual.
        let (n, _) = with_lists(&[(&[1], &[]), (&[2], &[]), (&[3], &[1, 3]), (&[2], &[2])]);
        assert_eq!(connection_defects(&n), Defects::default());
        assert!(connection_fix_plan(&n).0.is_empty());
    }

    #[test]
    fn a_duplicate_is_reported_once_and_only_its_repeats_are_removed() {
        // a.out = [b, c, b], b.in = [a]; a -> c is a reverse link.
        let (n, ids) = with_lists(&[(&[1, 2, 1], &[]), (&[], &[0]), (&[], &[])]);
        let [a, b, c] = [ids[0], ids[1], ids[2]];
        let d = connection_defects(&n);
        assert_eq!(d.duplicates, [(a, Side::Out, b)]);
        assert!(d.dangling.is_empty() && d.self_links.is_empty());
        let (n, _) = round_trip(n.clone(), connection_fix_plan(&n));
        assert_eq!(lists(&n, a), (vec![b, c], vec![]));
        assert_eq!(lists(&n, b), (vec![], vec![a]));
        assert_eq!(n.edges_near(0.0, 0.0, 1e9), [(a, b), (a, c)]);
        // Three times in an incoming list: once, and two repeats removed.
        let (n, ids) = with_lists(&[(&[1], &[]), (&[], &[0, 0, 0])]);
        assert_eq!(
            connection_defects(&n).duplicates,
            [(ids[1], Side::In, ids[0])]
        );
        let (n, _) = round_trip(n.clone(), connection_fix_plan(&n));
        assert_eq!(lists(&n, ids[1]), (vec![], vec![ids[0]]));
    }

    #[test]
    fn a_self_link_is_reported_and_removed() {
        // a.out = [a, b], b.in = [a]; c lists itself twice in both lists.
        let (n, ids) = with_lists(&[(&[0, 1], &[]), (&[], &[0]), (&[2, 2], &[2])]);
        let [a, b, c] = [ids[0], ids[1], ids[2]];
        let d = connection_defects(&n);
        assert_eq!(
            d.self_links,
            [(a, Side::Out), (c, Side::Out), (c, Side::In)]
        );
        assert!(d.dangling.is_empty() && d.duplicates.is_empty());
        let (n, _) = round_trip(n.clone(), connection_fix_plan(&n));
        assert_eq!(lists(&n, a), (vec![b], vec![]));
        assert_eq!(lists(&n, c), (vec![], vec![]));
        assert_eq!(n.edges_near(0.0, 0.0, 1e9), [(a, b)]);
        assert_eq!(n.connection_kind(a, b), ConnectionKind::Regular);
    }

    #[test]
    fn one_undo_restores_every_removed_entry_at_its_index() {
        // a.out = [b, a, c, b, c], b.in = [d, a, a], c.in = [a], d.in = [d].
        let (n, ids) = with_lists(&[
            (&[1, 0, 2, 1, 2], &[]),
            (&[], &[3, 0, 0]),
            (&[], &[0]),
            (&[], &[3]),
        ]);
        let [a, b, c, d] = [ids[0], ids[1], ids[2], ids[3]];
        let found = connection_defects(&n);
        assert_eq!(found.dangling, [(b, d)]);
        assert_eq!(
            found.duplicates,
            [(a, Side::Out, b), (a, Side::Out, c), (b, Side::In, a)]
        );
        assert_eq!(found.self_links, [(a, Side::Out), (d, Side::In)]);
        let tx = connection_fix_plan(&n);
        assert_eq!(relinked_nodes(&tx), [a, b, d]);
        // `round_trip`: undo gives the exact lists, in order, and the edge index back.
        let before = snap(&n);
        let (mut n, mut tx) = round_trip(n, tx);
        assert_eq!(lists(&n, a), (vec![b, c], vec![]));
        assert_eq!(lists(&n, b), (vec![], vec![a]));
        assert_eq!(lists(&n, d), (vec![], vec![]));
        assert!(connection_defects(&n).is_empty());
        tx.revert(&mut n).expect("undo");
        assert_eq!(snap(&n), before);
        assert_eq!(lists(&n, a), (vec![b, a, c, b, c], vec![]));
    }

    #[test]
    fn removing_entries_that_are_not_there_fails_and_changes_nothing() {
        let (mut n, ids) = with_lists(&[(&[1], &[]), (&[], &[0])]);
        let before = snap(&n);
        let gone = n.add_node(5.0, 0.0, 5.0, NodeFlag::REGULAR);
        n.remove_node(gone).expect("remove");
        for entries in [
            vec![(ids[0], Side::Out, 0), (ids[1], Side::In, 1)],
            vec![(ids[0], Side::Out, 0), (gone, Side::Out, 0)],
        ] {
            let mut tx = Transaction::new(vec![Command::RemoveLinkEntries(
                RemoveLinkEntries::new(entries),
            )]);
            assert!(tx.apply(&mut n).is_err());
            assert_eq!(snap(&n), before);
        }
    }
}
