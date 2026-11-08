//! The delete tool: a primary click deletes the node under the cursor, or, with no node
//! there, removes the connection under it in both directions (AD-4, AD-10).

use eframe::egui;

use ad_core::command::{Command, DeleteNodes, Disconnect, Transaction};
use ad_core::geometry::MapGeometry;

use super::{Tool, ToolContext, acts_on_selection};
use crate::view::ViewTransform;

pub const NAME: &str = "Delete";

/// Stroke width of the highlighted connection, in screen px.
const HIGHLIGHT_WIDTH_PX: f32 = 3.0;

#[derive(Default)]
pub struct DeleteTool;

impl Tool for DeleteTool {
    fn name(&self) -> &'static str {
        NAME
    }

    /// A click on a selected node deletes the whole selection, in id order.
    fn on_click(&mut self, ctx: &ToolContext) -> Option<Transaction> {
        if let Some(selection) = acts_on_selection(ctx) {
            let ids = selection.iter().copied().collect();
            return Some(Transaction::new(vec![Command::DeleteNodes(
                DeleteNodes::new(ids),
            )]));
        }
        let command = match (ctx.hovered_node, ctx.hovered_edge) {
            (Some(id), _) => Command::DeleteNodes(DeleteNodes::new(vec![id])),
            (None, Some((a, b))) => Command::Disconnect(Disconnect::new(a, b)),
            (None, None) => return None,
        };
        Some(Transaction::new(vec![command]))
    }

    /// Highlights the connection a click would remove; the view hovers none under a node.
    fn paint(
        &self,
        painter: &egui::Painter,
        _t: &ViewTransform,
        _geometry: MapGeometry,
        _cursor: Option<egui::Pos2>,
        _snap_grid: Option<(f64, f64)>,
        hovered_edge: Option<(egui::Pos2, egui::Pos2)>,
    ) {
        if let Some((a, b)) = hovered_edge {
            let colour = painter.ctx().global_style().visuals.warn_fg_color;
            painter.line_segment([a, b], egui::Stroke::new(HIGHLIGHT_WIDTH_PX, colour));
        }
    }

    fn cancel(&mut self) {}
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::collections::BTreeSet;

    use ad_core::map_context::MapContext;
    use ad_core::network::{ConnectionKind, ConnectionShape, Network, NodeFlag, NodeId};

    use super::*;

    /// a <-> b dual, b -> c one-way, and a marker on b.
    fn setup() -> (Network, [NodeId; 3]) {
        let mut net = Network::new();
        let a = net.add_node(0.0, 1.0, 0.0, NodeFlag::REGULAR);
        let b = net.add_node(10.0, 2.0, 0.0, NodeFlag::REGULAR);
        let c = net.add_node(20.0, 3.0, 0.0, NodeFlag::SUB_PRIO);
        net.connect(a, b, ConnectionShape::Dual).expect("ok");
        net.connect(b, c, ConnectionShape::OneWay).expect("ok");
        net.set_marker(b, "B".to_owned(), "All".to_owned())
            .expect("marker");
        (net, [a, b, c])
    }

    fn click(
        net: &Network,
        node: Option<NodeId>,
        edge: Option<(NodeId, NodeId)>,
    ) -> Option<Transaction> {
        click_with(net, &BTreeSet::new(), node, edge)
    }

    fn click_with(
        net: &Network,
        selection: &BTreeSet<NodeId>,
        node: Option<NodeId>,
        edge: Option<(NodeId, NodeId)>,
    ) -> Option<Transaction> {
        let map = MapContext::new(None, 1, 2.0, (2.0, 2.0));
        DeleteTool.on_click(&ToolContext {
            x: 0.0,
            z: 0.0,
            map: &map,
            net,
            selection,
            snap: false,
            hovered_node: node,
            hovered_edge: edge,
        })
    }

    /// Everything the delete must restore: order, positions, links, markers.
    fn snapshot(net: &Network) -> String {
        let nodes: Vec<_> = net
            .order()
            .iter()
            .map(|id| {
                let n = net.node(*id).expect("node");
                (
                    *id,
                    n.x,
                    n.y,
                    n.z,
                    n.flag,
                    n.outgoing().to_vec(),
                    n.incoming().to_vec(),
                )
            })
            .collect();
        format!("{nodes:?} {:?}", net.markers())
    }

    #[test]
    fn a_click_on_a_node_deletes_it_with_its_links_and_marker() {
        let (mut net, [a, b, c]) = setup();
        let before = snapshot(&net);
        let mut tx = click(&net, Some(b), None).expect("a transaction");
        tx.apply(&mut net).expect("applies");
        assert!(net.node(b).is_none());
        assert_eq!(net.order(), [a, c]);
        assert!(net.markers().is_empty());
        assert_eq!(net.connection_kind(a, c), ConnectionKind::None);
        assert!(net.node(a).expect("a").outgoing().is_empty());
        assert!(net.node(c).expect("c").incoming().is_empty());
        tx.revert(&mut net).expect("reverts");
        assert_eq!(snapshot(&net), before);
    }

    #[test]
    fn a_hovered_node_wins_over_a_hovered_connection() {
        let (mut net, [a, b, c]) = setup();
        let mut tx = click(&net, Some(c), Some((a, b))).expect("a transaction");
        tx.apply(&mut net).expect("applies");
        assert!(net.node(c).is_none());
        assert_eq!(net.connection_kind(a, b), ConnectionKind::Dual);
    }

    #[test]
    fn a_click_on_a_connection_removes_it_both_ways_and_keeps_the_nodes() {
        let (mut net, [a, b, c]) = setup();
        let before = snapshot(&net);
        // The hovered direction of a dual link is either one; both go.
        let mut tx = click(&net, None, Some((b, a))).expect("a transaction");
        tx.apply(&mut net).expect("applies");
        assert_eq!(net.order(), [a, b, c]);
        assert_eq!(net.connection_kind(a, b), ConnectionKind::None);
        assert_eq!(net.connection_kind(b, a), ConnectionKind::None);
        assert_eq!(net.connection_kind(b, c), ConnectionKind::Regular);
        tx.revert(&mut net).expect("reverts");
        assert_eq!(snapshot(&net), before);
        assert_eq!(net.connection_kind(a, b), ConnectionKind::Dual);
    }

    #[test]
    fn a_click_on_empty_map_does_nothing() {
        let (net, _) = setup();
        assert!(click(&net, None, None).is_none());
    }

    #[test]
    fn a_click_on_a_selected_node_deletes_the_whole_selection_in_one_undo() {
        let (mut net, [a, b, c]) = setup();
        let d = net.add_node(30.0, 4.0, 0.0, NodeFlag::REGULAR);
        net.connect(c, d, ConnectionShape::OneWay).expect("ok");
        let before = snapshot(&net);
        let selection = BTreeSet::from([a, b, c]);
        let mut tx = click_with(&net, &selection, Some(b), None).expect("a transaction");
        assert_eq!(tx.0.len(), 1, "one DeleteNodes");
        tx.apply(&mut net).expect("applies");
        assert_eq!(net.order(), [d]);
        assert!(net.markers().is_empty());
        assert!(net.node(d).expect("d").incoming().is_empty());
        tx.revert(&mut net).expect("reverts");
        assert_eq!(snapshot(&net), before);
    }

    #[test]
    fn a_click_off_the_selection_deletes_that_target_alone() {
        let (mut net, [a, b, c]) = setup();
        let selection = BTreeSet::from([a, b]);
        let mut tx = click_with(&net, &selection, Some(c), None).expect("a transaction");
        tx.apply(&mut net).expect("applies");
        assert_eq!(net.order(), [a, b]);
        tx.revert(&mut net).expect("reverts");
        // A connection inside the selection is removed alone too.
        let mut tx = click_with(&net, &selection, None, Some((a, b))).expect("a transaction");
        tx.apply(&mut net).expect("applies");
        assert_eq!(net.order(), [a, b, c]);
        assert_eq!(net.connection_kind(a, b), ConnectionKind::None);
    }
}
