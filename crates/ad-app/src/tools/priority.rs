//! The priority tool: a primary click swaps the hovered node between regular and
//! sub-priority through `SetFlag` (Java `SwapNodePriorityButton`).

use eframe::egui;

use ad_core::command::{Command, SetFlag, Transaction};
use ad_core::geometry::MapGeometry;
use ad_core::network::NodeFlag;

use super::{Tool, ToolContext};
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

    fn on_click(&mut self, ctx: &ToolContext) -> Option<Transaction> {
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
        let map = MapContext::new(None, 1, 2.0, (2.0, 2.0));
        let ctx = ToolContext {
            x: 0.0,
            z: 0.0,
            map: &map,
            net,
            selection: &BTreeSet::new(),
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
}
