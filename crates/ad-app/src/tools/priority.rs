//! The priority tool: a primary click swaps the hovered node between regular and
//! sub-priority through `SetFlag` (Java `SwapNodePriorityButton`).

use eframe::egui;

use ad_core::command::{Command, SetFlag, Transaction};
use ad_core::geometry::MapGeometry;
use ad_core::network::NodeFlag;

use super::{Tool, ToolContext, acts_on_selection};
use crate::view::ViewTransform;

pub const NAME: &str = "Priority";

/// The flag a swap sets. Java computes `1 - flag`, which turns any other file value into
/// garbage; here sub-priority becomes regular and everything else becomes sub-priority.
pub fn swapped(flag: NodeFlag) -> NodeFlag {
    if flag == NodeFlag::SUB_PRIO {
        NodeFlag::REGULAR
    } else {
        NodeFlag::SUB_PRIO
    }
}

#[derive(Default)]
pub struct PriorityTool;

impl Tool for PriorityTool {
    fn name(&self) -> &'static str {
        NAME
    }

    /// A click on a selected node swaps every selected node, one `SetFlag` each.
    fn on_click(&mut self, ctx: &ToolContext) -> Option<Transaction> {
        if let Some(selection) = acts_on_selection(ctx) {
            let commands = selection
                .iter()
                .filter_map(|&id| {
                    let flag = ctx.net.node(id)?.flag;
                    Some(Command::SetFlag(SetFlag::new(id, swapped(flag))))
                })
                .collect();
            return Some(Transaction::new(commands));
        }
        let id = ctx.hovered_node?;
        let flag = ctx.net.node(id)?.flag;
        Some(Transaction::new(vec![Command::SetFlag(SetFlag::new(
            id,
            swapped(flag),
        ))]))
    }

    fn paint(
        &self,
        _painter: &egui::Painter,
        _t: &ViewTransform,
        _geometry: MapGeometry,
        _cursor: Option<egui::Pos2>,
        _snap_grid: Option<(f64, f64)>,
        _hovered_edge: Option<(egui::Pos2, egui::Pos2)>,
    ) {
    }

    fn cancel(&mut self) {}
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::collections::BTreeSet;

    use ad_core::map_context::MapContext;
    use ad_core::network::{ConnectionKind, ConnectionShape, Network, NodeId};

    use super::*;

    fn click(net: &Network, node: Option<NodeId>) -> Option<Transaction> {
        click_with(net, &BTreeSet::new(), node)
    }

    fn click_with(
        net: &Network,
        selection: &BTreeSet<NodeId>,
        node: Option<NodeId>,
    ) -> Option<Transaction> {
        let map = MapContext::new(None, 1, 2.0, (2.0, 2.0));
        let ctx = ToolContext {
            x: 0.0,
            z: 0.0,
            map: &map,
            net,
            selection,
            snap: false,
            hovered_node: node,
            hovered_edge: None,
        };
        PriorityTool.on_click(&ctx)
    }

    #[test]
    fn a_click_swaps_regular_and_sub_priority_and_undo_restores() {
        let mut net = Network::new();
        let a = net.add_node(0.0, 1.0, 0.0, NodeFlag::REGULAR);
        let b = net.add_node(10.0, 1.0, 0.0, NodeFlag::REGULAR);
        net.connect(a, b, ConnectionShape::OneWay).expect("ok");
        assert_eq!(net.connection_kind(a, b), ConnectionKind::Regular);

        let mut tx = click(&net, Some(a)).expect("swap");
        tx.apply(&mut net).expect("apply");
        assert_eq!(net.node(a).map(|n| n.flag), Some(NodeFlag::SUB_PRIO));
        // The colour follows from the derived kind.
        assert_eq!(net.connection_kind(a, b), ConnectionKind::SubPrio);

        let mut back = click(&net, Some(a)).expect("swap back");
        back.apply(&mut net).expect("apply");
        assert_eq!(net.node(a).map(|n| n.flag), Some(NodeFlag::REGULAR));
        back.revert(&mut net).expect("revert");
        tx.revert(&mut net).expect("revert");
        assert_eq!(net.node(a).map(|n| n.flag), Some(NodeFlag::REGULAR));
        assert_eq!(net.connection_kind(a, b), ConnectionKind::Regular);
    }

    #[test]
    fn another_file_value_becomes_sub_priority_and_undo_restores_it() {
        let mut net = Network::new();
        let a = net.add_node(0.0, 1.0, 0.0, NodeFlag(4));
        let mut tx = click(&net, Some(a)).expect("swap");
        tx.apply(&mut net).expect("apply");
        assert_eq!(net.node(a).map(|n| n.flag), Some(NodeFlag::SUB_PRIO));
        tx.revert(&mut net).expect("revert");
        assert_eq!(net.node(a).map(|n| n.flag), Some(NodeFlag(4)));
    }

    #[test]
    fn a_click_off_a_node_does_nothing() {
        let net = Network::new();
        assert!(click(&net, None).is_none());
    }

    fn flags(net: &Network) -> Vec<NodeFlag> {
        net.order()
            .iter()
            .filter_map(|id| net.node(*id).map(|n| n.flag))
            .collect()
    }

    #[test]
    fn a_click_on_a_selected_node_swaps_every_selected_node_in_one_undo() {
        let mut net = Network::new();
        let a = net.add_node(0.0, 1.0, 0.0, NodeFlag::REGULAR);
        let b = net.add_node(10.0, 1.0, 0.0, NodeFlag::SUB_PRIO);
        let c = net.add_node(20.0, 1.0, 0.0, NodeFlag(4));
        let d = net.add_node(30.0, 1.0, 0.0, NodeFlag::REGULAR);
        let before = flags(&net);
        let selection = BTreeSet::from([a, b, c]);
        let mut tx = click_with(&net, &selection, Some(b)).expect("swap");
        tx.apply(&mut net).expect("apply");
        let (r, s) = (NodeFlag::REGULAR, NodeFlag::SUB_PRIO);
        assert_eq!(flags(&net), [s, r, s, r]);
        tx.revert(&mut net).expect("revert");
        assert_eq!(flags(&net), before);
        // A click on the unselected node swaps it alone.
        let mut tx = click_with(&net, &selection, Some(d)).expect("swap");
        tx.apply(&mut net).expect("apply");
        assert_eq!(flags(&net), [r, s, NodeFlag(4), s]);
    }
}
