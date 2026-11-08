//! Network scans that change nothing: they report nodes for the user to look at.

use std::collections::BTreeSet;

use crate::command::{Command, Transaction};
use crate::network::{Network, NodeId};

/// Every node with at least one other node within `distance` of it in 3D (x, y and z),
/// inclusive (Java `ScanManager.scanNetworkForOverlapNodes`, which ignored the typed
/// distance).
pub fn overlaps(net: &Network, distance: f64) -> BTreeSet<NodeId> {
    net.order()
        .iter()
        .copied()
        .filter(|&id| overlapping(net, id, distance))
        .collect()
}

/// Whether another node lies within `distance` of `id` in 3D. The R-tree gives the
/// candidates within `distance` in (x, z). False for a missing node.
fn overlapping(net: &Network, id: NodeId, distance: f64) -> bool {
    let Some(n) = net.node(id) else {
        return false;
    };
    let limit = distance * distance;
    net.within(n.x, n.z, distance).into_iter().any(|other| {
        other != id
            && net.node(other).is_some_and(|o| {
                (o.x - n.x).powi(2) + (o.y - n.y).powi(2) + (o.z - n.z).powi(2) <= limit
            })
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
            | Command::SetFlag(_) => {}
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
    fn height_counts() {
        let (n, _) = net(&[(10.0, 0.0, 10.0), (10.0, 5.0, 10.0)]);
        assert!(overlaps(&n, 0.1).is_empty());
        // Only the height apart, within the distance.
        let (n, ids) = net(&[(10.0, 0.0, 10.0), (10.0, 0.05, 10.0)]);
        assert_eq!(overlaps(&n, 0.1), BTreeSet::from([ids[0], ids[1]]));
    }

    #[test]
    fn the_distance_decides() {
        let (n, ids) = net(&[(0.0, 0.0, 0.0), (0.3, 0.4, 0.0)]);
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

    #[test]
    fn an_empty_network_has_none() {
        assert!(overlaps(&Network::new(), 0.1).is_empty());
    }
}
