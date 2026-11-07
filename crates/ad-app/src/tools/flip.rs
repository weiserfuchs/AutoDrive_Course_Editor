//! The flip tool: a primary click reverses the direction of the connection under the cursor
//! in one transaction (Java `FlipDirectionButton`, without its four-pass workaround). Dual
//! connections stay as they are, as Java filters them out.

use eframe::egui;

use ad_core::command::{Command, Connect, Disconnect, Transaction};
use ad_core::geometry::MapGeometry;
use ad_core::network::{ConnectionShape, Network, NodeId};

use super::connect::is_exactly;
use super::{Tool, ToolContext};
use crate::view::ViewTransform;

pub const NAME: &str = "Flip";

/// Stroke width of the highlighted connection, in screen px.
const HIGHLIGHT_WIDTH_PX: f32 = 3.0;

/// The flip of the link `a -> b`: `Disconnect` then `Connect` from `b` to `a` with the same
/// shape. `None` for a dual link or links that are not exactly one-way or reverse.
pub fn plan(net: &Network, a: NodeId, b: NodeId) -> Option<Transaction> {
    [ConnectionShape::OneWay, ConnectionShape::Reverse]
        .into_iter()
        .find(|shape| is_exactly(net, a, b, *shape))
        .map(|shape| {
            Transaction::new(vec![
                Command::Disconnect(Disconnect::new(a, b)),
                Command::Connect(Connect::new(b, a, shape)),
            ])
        })
}

#[derive(Default)]
pub struct FlipTool;

impl Tool for FlipTool {
    fn name(&self) -> &'static str {
        NAME
    }

    /// The view hovers no connection under a node (story 3.4 rule), so a node click is a no-op.
    fn on_click(&mut self, ctx: &ToolContext) -> Option<Transaction> {
        if ctx.hovered_node.is_some() {
            return None;
        }
        let (a, b) = ctx.hovered_edge?;
        plan(ctx.net, a, b)
    }

    /// Only a link that is exactly one-way or reverse is flipped, so only that is highlighted.
    fn acts_on_edge(&self, net: &Network, (a, b): (NodeId, NodeId)) -> bool {
        plan(net, a, b).is_some()
    }

    /// Highlights the connection a click would act on.
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
    use ad_core::network::{ConnectionKind, NodeFlag};

    use super::*;

    fn pair() -> (Network, NodeId, NodeId) {
        let mut net = Network::new();
        let a = net.add_node(0.0, 1.0, 0.0, NodeFlag::REGULAR);
        let b = net.add_node(10.0, 1.0, 0.0, NodeFlag::REGULAR);
        (net, a, b)
    }

    fn click(
        net: &Network,
        node: Option<NodeId>,
        edge: Option<(NodeId, NodeId)>,
    ) -> Option<Transaction> {
        let map = MapContext::new(None, 1, 2.0, (2.0, 2.0));
        let ctx = ToolContext {
            x: 0.0,
            z: 0.0,
            map: &map,
            net,
            selection: &BTreeSet::new(),
            snap: false,
            hovered_node: node,
            hovered_edge: edge,
        };
        FlipTool.on_click(&ctx)
    }

    fn links(net: &Network, id: NodeId) -> (Vec<NodeId>, Vec<NodeId>) {
        let n = net.node(id).expect("node");
        (n.outgoing().to_vec(), n.incoming().to_vec())
    }

    #[test]
    fn one_way_and_reverse_flip_and_undo_in_one_step() {
        for shape in [ConnectionShape::OneWay, ConnectionShape::Reverse] {
            let (mut net, a, b) = pair();
            net.connect(a, b, shape).expect("ok");
            let before = (links(&net, a), links(&net, b));
            let kind = net.connection_kind(a, b);

            let mut tx = click(&net, None, Some((a, b))).expect("flip");
            tx.apply(&mut net).expect("apply");
            assert_eq!(net.connection_kind(b, a), kind);
            assert_eq!(net.connection_kind(a, b), ConnectionKind::None);
            assert!(is_exactly(&net, b, a, shape));

            tx.revert(&mut net).expect("revert");
            assert_eq!((links(&net, a), links(&net, b)), before);
        }
    }

    #[test]
    fn a_dual_link_is_left_unchanged_and_not_highlighted() {
        let (mut net, a, b) = pair();
        net.connect(a, b, ConnectionShape::Dual).expect("ok");
        assert!(click(&net, None, Some((a, b))).is_none());
        assert!(click(&net, None, Some((b, a))).is_none());
        assert!(!FlipTool.acts_on_edge(&net, (a, b)));
        assert!(!FlipTool.acts_on_edge(&net, (b, a)));
        let (mut net, a, b) = pair();
        net.connect(a, b, ConnectionShape::OneWay).expect("ok");
        assert!(FlipTool.acts_on_edge(&net, (a, b)));
    }

    #[test]
    fn a_node_or_empty_click_does_nothing() {
        let (mut net, a, b) = pair();
        net.connect(a, b, ConnectionShape::OneWay).expect("ok");
        assert!(click(&net, Some(a), Some((a, b))).is_none());
        assert!(click(&net, None, None).is_none());
    }
}
