//! Commands and transactions (AD-4).
//!
//! `Network` is mutated only through `Command::apply` and `Command::revert`. Each command
//! keeps what it needs to undo itself exactly, and applying it again after a revert
//! reproduces the same result.

use crate::network::{
    ConnectionShape, LinkDelta, MarkerDelta, Network, NetworkError, NodeFlag, NodeId, RemovedNode,
};

/// A node position as `(x, y, z)`.
pub type Position = (f64, f64, f64);

/// Adds unconnected nodes in the given order, each as `(x, y, z, flag)`. A redo reinserts
/// each node under the id of the first apply.
#[derive(Debug, Clone, PartialEq)]
pub struct AddNodes {
    pub nodes: Vec<(f64, f64, f64, NodeFlag)>,
    ids: Vec<NodeId>,
    removed: Vec<RemovedNode>,
}

impl AddNodes {
    pub fn new(nodes: Vec<(f64, f64, f64, NodeFlag)>) -> Self {
        Self {
            nodes,
            ids: Vec::new(),
            removed: Vec::new(),
        }
    }

    /// The ids of the added nodes in order, known after the first apply.
    pub fn ids(&self) -> &[NodeId] {
        &self.ids
    }
}

/// Deletes nodes in the given order, with their links and markers.
#[derive(Debug, Clone, PartialEq)]
pub struct DeleteNodes {
    pub ids: Vec<NodeId>,
    removed: Vec<RemovedNode>,
}

impl DeleteNodes {
    pub fn new(ids: Vec<NodeId>) -> Self {
        Self {
            ids,
            removed: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Connect {
    pub a: NodeId,
    pub b: NodeId,
    pub shape: ConnectionShape,
    delta: LinkDelta,
}

impl Connect {
    pub fn new(a: NodeId, b: NodeId, shape: ConnectionShape) -> Self {
        Self {
            a,
            b,
            shape,
            delta: LinkDelta::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Disconnect {
    pub a: NodeId,
    pub b: NodeId,
    delta: LinkDelta,
}

impl Disconnect {
    pub fn new(a: NodeId, b: NodeId) -> Self {
        Self {
            a,
            b,
            delta: LinkDelta::default(),
        }
    }
}

/// Moves nodes between absolute positions: `(node, before, after)`.
#[derive(Debug, Clone, PartialEq)]
pub struct MoveNodes {
    pub moves: Vec<(NodeId, Position, Position)>,
}

/// Sets (`Some((name, group))`) or removes (`None`) the marker of a node.
#[derive(Debug, Clone, PartialEq)]
pub struct EditMarker {
    pub node: NodeId,
    pub marker: Option<(String, String)>,
    delta: MarkerDelta,
}

impl EditMarker {
    pub fn new(node: NodeId, marker: Option<(String, String)>) -> Self {
        Self {
            node,
            marker,
            delta: MarkerDelta::Unchanged,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    AddNodes(AddNodes),
    DeleteNodes(DeleteNodes),
    Connect(Connect),
    Disconnect(Disconnect),
    MoveNodes(MoveNodes),
    EditMarker(EditMarker),
}

fn move_to(net: &mut Network, id: NodeId, p: Position) -> Result<(), NetworkError> {
    net.move_node(id, p.0, p.1, p.2)
}

impl Command {
    /// Applies the command. On error the network is unchanged.
    pub fn apply(&mut self, net: &mut Network) -> Result<(), NetworkError> {
        match self {
            Command::AddNodes(c) => {
                if c.removed.is_empty() {
                    c.ids = c
                        .nodes
                        .iter()
                        .map(|&(x, y, z, flag)| net.add_node(x, y, z, flag))
                        .collect();
                } else {
                    // Revert pushed the nodes last first, so popping reinserts them in order.
                    while let Some(removed) = c.removed.pop() {
                        net.reinsert(removed);
                    }
                }
                Ok(())
            }
            Command::DeleteNodes(c) => {
                c.removed.clear();
                for id in &c.ids {
                    match net.remove_node(*id) {
                        Ok(removed) => c.removed.push(removed),
                        Err(e) => {
                            while let Some(removed) = c.removed.pop() {
                                net.reinsert(removed);
                            }
                            return Err(e);
                        }
                    }
                }
                Ok(())
            }
            Command::Connect(c) => {
                c.delta = net.connect(c.a, c.b, c.shape)?;
                Ok(())
            }
            Command::Disconnect(c) => {
                c.delta = net.disconnect(c.a, c.b)?;
                Ok(())
            }
            Command::MoveNodes(c) => {
                for (i, (id, _, after)) in c.moves.iter().enumerate() {
                    if let Err(e) = move_to(net, *id, *after) {
                        for (id, before, _) in c.moves[..i].iter().rev() {
                            move_to(net, *id, *before)?;
                        }
                        return Err(e);
                    }
                }
                Ok(())
            }
            Command::EditMarker(c) => {
                c.delta = match &c.marker {
                    Some((name, group)) => net.set_marker(c.node, name.clone(), group.clone())?,
                    None => net.remove_marker(c.node)?,
                };
                Ok(())
            }
        }
    }

    /// Undoes a successful `apply`.
    pub fn revert(&mut self, net: &mut Network) -> Result<(), NetworkError> {
        match self {
            Command::AddNodes(c) => {
                for id in c.ids.iter().rev() {
                    c.removed.push(net.remove_node(*id)?);
                }
                Ok(())
            }
            Command::DeleteNodes(c) => {
                while let Some(removed) = c.removed.pop() {
                    net.reinsert(removed);
                }
                Ok(())
            }
            Command::Connect(c) => {
                net.revert_links(&c.delta);
                Ok(())
            }
            Command::Disconnect(c) => {
                net.revert_links(&c.delta);
                Ok(())
            }
            Command::MoveNodes(c) => {
                for (id, before, _) in c.moves.iter().rev() {
                    move_to(net, *id, *before)?;
                }
                Ok(())
            }
            Command::EditMarker(c) => {
                net.revert_marker(&c.delta);
                Ok(())
            }
        }
    }
}

/// One user action: applied in order, reverted in reverse order, undone in one step.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Transaction(pub Vec<Command>);

impl Transaction {
    pub fn new(commands: Vec<Command>) -> Self {
        Self(commands)
    }

    /// Applies every command. If one fails, the applied ones are reverted, the network is
    /// unchanged and the error is returned.
    pub fn apply(&mut self, net: &mut Network) -> Result<(), NetworkError> {
        for i in 0..self.0.len() {
            let result = self.0[i].apply(net);
            if let Err(e) = result {
                for done in self.0[..i].iter_mut().rev() {
                    // Reverting a command that just applied cannot fail on a consistent network.
                    let _ = done.revert(net);
                }
                return Err(e);
            }
        }
        Ok(())
    }

    /// Reverts every command in reverse order. Attempts all, returns the first error.
    pub fn revert(&mut self, net: &mut Network) -> Result<(), NetworkError> {
        let mut first = Ok(());
        for c in self.0.iter_mut().rev() {
            if let Err(e) = c.revert(net)
                && first.is_ok()
            {
                first = Err(e);
            }
        }
        first
    }
}

#[cfg(test)]
pub(crate) mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::network::{Marker, NodeFlag};

    type SnapNode = (NodeId, Position, NodeFlag, Vec<NodeId>, Vec<NodeId>);

    /// Everything observable about a network, including both spatial indexes.
    /// `next_id` is left out on purpose: ids are never reused, so it only grows.
    #[derive(Debug, PartialEq)]
    pub(crate) struct Snapshot {
        order: Vec<NodeId>,
        nodes: Vec<SnapNode>,
        markers: Vec<Marker>,
        edges: Vec<(NodeId, NodeId)>,
        in_rect: Vec<NodeId>,
        nearest: Vec<Option<NodeId>>,
    }

    pub(crate) fn snap(n: &Network) -> Snapshot {
        const HUGE: f64 = 1e9;
        let mut nodes = Vec::new();
        for id in n.order() {
            if let Some(x) = n.node(*id) {
                nodes.push((
                    *id,
                    (x.x, x.y, x.z),
                    x.flag,
                    x.outgoing().to_vec(),
                    x.incoming().to_vec(),
                ));
            }
        }
        // Per-node probes catch an index entry at a stale position.
        let nearest = nodes.iter().map(|(_, p, ..)| n.nearest(p.0, p.2)).collect();
        Snapshot {
            order: n.order().to_vec(),
            nodes,
            markers: n.markers().to_vec(),
            edges: n.edges_near(0.0, 0.0, HUGE),
            in_rect: n.in_rect(-HUGE, -HUGE, HUGE, HUGE),
            nearest,
        }
    }

    /// a -> b -> c -> d chain, b <-> c dual, markers on b and c, plus a -> d.
    pub(crate) fn linked() -> (Network, [NodeId; 4]) {
        let mut n = Network::new();
        let ids = [
            n.add_node(0.0, 0.0, 0.0, NodeFlag::REGULAR),
            n.add_node(10.0, 1.0, 0.0, NodeFlag::SUB_PRIO),
            n.add_node(20.0, 2.0, 5.0, NodeFlag::REGULAR),
            n.add_node(30.0, 3.0, 5.0, NodeFlag(2)),
        ];
        let [a, b, c, d] = ids;
        n.connect(a, b, ConnectionShape::OneWay).expect("ok");
        n.connect(b, c, ConnectionShape::Dual).expect("ok");
        n.connect(c, d, ConnectionShape::OneWay).expect("ok");
        n.connect(a, d, ConnectionShape::Reverse).expect("ok");
        n.connect(d, b, ConnectionShape::OneWay).expect("ok");
        n.push_marker(Marker {
            node: b,
            name: "B".into(),
            group: "g".into(),
        });
        n.push_marker(Marker {
            node: c,
            name: "C".into(),
            group: "g".into(),
        });
        (n, ids)
    }

    /// Applies, then checks that undo and redo alternate between exactly two states.
    fn round_trip(mut net: Network, mut tx: Transaction) -> (Network, Transaction) {
        let before = snap(&net);
        tx.apply(&mut net).expect("apply");
        let after = snap(&net);
        assert_ne!(before, after);
        for _ in 0..3 {
            tx.revert(&mut net).expect("revert");
            assert_eq!(snap(&net), before);
            tx.apply(&mut net).expect("redo");
            assert_eq!(snap(&net), after);
        }
        (net, tx)
    }

    fn one(c: Command) -> Transaction {
        Transaction::new(vec![c])
    }

    #[test]
    fn round_trip_add() {
        let (n, _) = linked();
        let (n, tx) = round_trip(
            n,
            one(Command::AddNodes(AddNodes::new(vec![(
                5.0,
                0.0,
                5.0,
                NodeFlag::REGULAR,
            )]))),
        );
        assert_eq!(n.len(), 5);
        assert!(matches!(&tx.0[0], Command::AddNodes(a) if a.ids().len() == 1));
    }

    #[test]
    fn round_trip_delete_single_and_all_nodes() {
        let (n, [_, b, ..]) = linked();
        round_trip(n, one(Command::DeleteNodes(DeleteNodes::new(vec![b]))));
        let (n, ids) = linked();
        round_trip(n, one(Command::DeleteNodes(DeleteNodes::new(ids.to_vec()))));
    }

    #[test]
    fn round_trip_connect_disconnect() {
        let (n, [a, b, c, d]) = linked();
        let (n, _) = round_trip(
            n,
            one(Command::Connect(Connect::new(a, c, ConnectionShape::Dual))),
        );
        let (n, _) = round_trip(n, one(Command::Disconnect(Disconnect::new(b, c))));
        round_trip(n, one(Command::Disconnect(Disconnect::new(d, a))));
    }

    #[test]
    fn round_trip_move() {
        let (n, [a, b, ..]) = linked();
        round_trip(
            n,
            one(Command::MoveNodes(MoveNodes {
                moves: vec![
                    (a, (0.0, 0.0, 0.0), (50.0, 4.0, 60.0)),
                    (b, (10.0, 1.0, 0.0), (-3.0, 5.0, 7.0)),
                ],
            })),
        );
    }

    #[test]
    fn marker_edit_create_rename_remove() {
        let (mut n, [a, b, ..]) = linked();
        let set = |node, name: &str, group: &str| {
            Command::EditMarker(EditMarker::new(
                node,
                Some((name.to_string(), group.to_string())),
            ))
        };
        // create on a node without one (appended), rename in place, remove from the middle
        for c in [
            set(a, "A", "h"),
            set(b, "B2", "h2"),
            Command::EditMarker(EditMarker::new(b, None)),
        ] {
            let (net, _) = round_trip(n, one(c));
            n = net;
        }
        assert_eq!(n.markers().len(), 2);
    }

    #[test]
    fn removing_a_missing_marker_changes_nothing() {
        let (mut n, [a, ..]) = linked();
        let before = snap(&n);
        let mut tx = one(Command::EditMarker(EditMarker::new(a, None)));
        tx.apply(&mut n).expect("apply");
        assert_eq!(snap(&n), before);
        tx.revert(&mut n).expect("revert");
        assert_eq!(snap(&n), before);
    }

    #[test]
    fn delete_restores_order_neighbours_and_marker() {
        let (mut n, [a, b, c, d]) = linked();
        let before = snap(&n);
        let mut tx = one(Command::DeleteNodes(DeleteNodes::new(vec![b])));
        tx.apply(&mut n).expect("apply");
        assert_eq!(n.order(), &[a, c, d]);
        assert!(n.node(b).is_none());
        assert!(n.node(a).is_some_and(|x| x.outgoing() == [d]));
        assert_eq!(n.markers().len(), 1);
        tx.revert(&mut n).expect("revert");
        assert_eq!(snap(&n), before);
        assert_eq!(n.order(), &[a, b, c, d]);
        assert_eq!(n.markers()[0].node, b);
        // a.out was [b, d]: b must be back at index 0
        assert!(n.node(a).is_some_and(|x| x.outgoing() == [b, d]));
    }

    #[test]
    fn multi_node_delete_in_one_transaction() {
        let (mut n, [a, b, c, d]) = linked();
        let before = snap(&n);
        let mut tx = one(Command::DeleteNodes(DeleteNodes::new(vec![b, c])));
        tx.apply(&mut n).expect("apply");
        assert_eq!(n.order(), &[a, d]);
        assert!(n.edges_near(0.0, 0.0, 1e9) == vec![(a, d)]);
        tx.revert(&mut n).expect("revert");
        assert_eq!(snap(&n), before);
        // Same, as two commands in one transaction.
        let mut tx = Transaction::new(vec![
            Command::DeleteNodes(DeleteNodes::new(vec![c])),
            Command::DeleteNodes(DeleteNodes::new(vec![b])),
        ]);
        tx.apply(&mut n).expect("apply");
        tx.revert(&mut n).expect("revert");
        assert_eq!(snap(&n), before);
    }

    #[test]
    fn add_redo_keeps_id_and_ids_are_not_reused() {
        let (mut n, _) = linked();
        let mut tx = one(Command::AddNodes(AddNodes::new(vec![(
            1.0,
            1.0,
            1.0,
            NodeFlag(0),
        )])));
        tx.apply(&mut n).expect("apply");
        let id = match &tx.0[0] {
            Command::AddNodes(a) => *a.ids().first().expect("id"),
            _ => return,
        };
        tx.revert(&mut n).expect("revert");
        assert!(n.node(id).is_none());
        tx.apply(&mut n).expect("redo");
        assert!(n.node(id).is_some());
        assert_eq!(n.order().last(), Some(&id));
        tx.revert(&mut n).expect("revert");
        let fresh = n.add_node(2.0, 2.0, 2.0, NodeFlag(0));
        assert!(fresh > id);
    }

    #[test]
    fn add_several_redo_keeps_ids_and_order() {
        let (mut n, [a, b, c, d]) = linked();
        let nodes = vec![
            (1.0, 1.0, 1.0, NodeFlag::REGULAR),
            (2.0, 2.0, 2.0, NodeFlag::SUB_PRIO),
            (3.0, 3.0, 3.0, NodeFlag(2)),
        ];
        let mut tx = one(Command::AddNodes(AddNodes::new(nodes.clone())));
        tx.apply(&mut n).expect("apply");
        let ids = match &tx.0[0] {
            Command::AddNodes(add) => add.ids().to_vec(),
            _ => return,
        };
        tx.revert(&mut n).expect("revert");
        let (n, tx) = round_trip(n, tx);
        let Command::AddNodes(add) = &tx.0[0] else {
            return;
        };
        assert_eq!(add.ids(), ids.as_slice());
        assert_eq!(n.order(), &[a, b, c, d, ids[0], ids[1], ids[2]]);
        for (id, (x, y, z, flag)) in ids.iter().zip(nodes) {
            let node = n.node(*id).expect("added node");
            assert_eq!(((node.x, node.y, node.z), node.flag), ((x, y, z), flag));
        }
    }

    #[test]
    fn failing_transaction_leaves_network_unchanged() {
        let (mut n, [a, b, ..]) = linked();
        let before = snap(&n);
        let ghost = {
            let mut other = Network::new();
            for _ in 0..50 {
                other.add_node(0.0, 0.0, 0.0, NodeFlag(0));
            }
            other.order()[49]
        };
        let mut tx = Transaction::new(vec![
            Command::AddNodes(AddNodes::new(vec![(9.0, 9.0, 9.0, NodeFlag(0))])),
            Command::Disconnect(Disconnect::new(a, b)),
            Command::Connect(Connect::new(a, ghost, ConnectionShape::OneWay)),
        ]);
        assert_eq!(tx.apply(&mut n), Err(NetworkError::UnknownNode(ghost)));
        assert_eq!(snap(&n), before);
        // A failing delete after a successful one in the same command also rolls back.
        let mut del = Command::DeleteNodes(DeleteNodes::new(vec![a, ghost]));
        assert_eq!(del.apply(&mut n), Err(NetworkError::UnknownNode(ghost)));
        assert_eq!(snap(&n), before);
        let mut mv = Command::MoveNodes(MoveNodes {
            moves: vec![
                (a, (0.0, 0.0, 0.0), (1.0, 1.0, 1.0)),
                (ghost, (0.0, 0.0, 0.0), (1.0, 1.0, 1.0)),
            ],
        });
        assert_eq!(mv.apply(&mut n), Err(NetworkError::UnknownNode(ghost)));
        assert_eq!(snap(&n), before);
    }
}
