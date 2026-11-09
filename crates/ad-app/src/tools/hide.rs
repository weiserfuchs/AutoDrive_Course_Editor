//! The hide tools (Java's Show/Hide buttons): a click hides or shows the node or the
//! connection under the cursor. Hiding changes no data and has no undo; it lasts until
//! another document opens.

use eframe::egui;

use ad_core::command::Transaction;
use ad_core::geometry::MapGeometry;
use ad_core::network::{Network, NodeId};

use super::{Tool, ToolContext, ToolRequest, acts_on_selection};
use crate::view::ViewTransform;

pub const NODE_NAME: &str = "Hide node";
pub const LINK_NAME: &str = "Hide connection";

/// Stroke width of the highlighted connection, in screen px.
const HIGHLIGHT_WIDTH_PX: f32 = 3.0;

/// Hides or shows the clicked node; a click on a selected node does it for the selection.
#[derive(Default)]
pub struct HideNodeTool {
    request: Option<ToolRequest>,
}

impl Tool for HideNodeTool {
    fn name(&self) -> &'static str {
        NODE_NAME
    }

    fn on_click(&mut self, ctx: &ToolContext) -> Option<Transaction> {
        let ids: Vec<NodeId> = match acts_on_selection(ctx) {
            Some(selection) => selection.iter().copied().collect(),
            None => ctx.hovered_node.into_iter().collect(),
        };
        if !ids.is_empty() {
            self.request = Some(ToolRequest::ToggleHiddenNodes(ids));
        }
        None
    }

    fn take_request(&mut self) -> Option<ToolRequest> {
        self.request.take()
    }

    fn acts_on_edge(&self, _net: &Network, _edge: (NodeId, NodeId)) -> bool {
        false
    }

    fn picks_hidden(&self) -> bool {
        true
    }

    /// The view's hover ring shows the node a click acts on.
    fn paint(
        &self,
        _painter: &egui::Painter,
        _t: &ViewTransform,
        _geometry: MapGeometry,
        _cursor: Option<egui::Pos2>,
        _snap_grid: Option<(f64, f64)>,
        _hovered_node: Option<(f64, f64)>,
        _hovered_edge: Option<(egui::Pos2, egui::Pos2)>,
    ) {
    }

    fn cancel(&mut self) {
        self.request = None;
    }
}

/// Hides or shows the clicked connection, both directions as one.
#[derive(Default)]
pub struct HideLinkTool {
    request: Option<ToolRequest>,
}

impl Tool for HideLinkTool {
    fn name(&self) -> &'static str {
        LINK_NAME
    }

    /// Only a connection: the view hovers none under a node.
    fn on_click(&mut self, ctx: &ToolContext) -> Option<Transaction> {
        if let (None, Some((a, b))) = (ctx.hovered_node, ctx.hovered_edge) {
            self.request = Some(ToolRequest::ToggleHiddenLink(a, b));
        }
        None
    }

    fn take_request(&mut self) -> Option<ToolRequest> {
        self.request.take()
    }

    fn picks_hidden(&self) -> bool {
        true
    }

    /// Highlights the connection a click would hide or show.
    fn paint(
        &self,
        painter: &egui::Painter,
        _t: &ViewTransform,
        _geometry: MapGeometry,
        _cursor: Option<egui::Pos2>,
        _snap_grid: Option<(f64, f64)>,
        _hovered_node: Option<(f64, f64)>,
        hovered_edge: Option<(egui::Pos2, egui::Pos2)>,
    ) {
        if let Some((a, b)) = hovered_edge {
            let colour = painter.ctx().global_style().visuals.selection.bg_fill;
            painter.line_segment([a, b], egui::Stroke::new(HIGHLIGHT_WIDTH_PX, colour));
        }
    }

    fn cancel(&mut self) {
        self.request = None;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::collections::BTreeSet;

    use ad_core::map_context::MapContext;
    use ad_core::network::NodeFlag;

    use super::*;

    fn ctx<'a>(
        net: &'a Network,
        map: &'a MapContext,
        selection: &'a BTreeSet<NodeId>,
        hovered_node: Option<NodeId>,
        hovered_edge: Option<(NodeId, NodeId)>,
    ) -> ToolContext<'a> {
        ToolContext {
            x: 0.0,
            z: 0.0,
            map,
            net,
            selection,
            snap: false,
            hovered_node,
            hovered_edge,
        }
    }

    #[test]
    fn a_click_asks_to_toggle_the_node_the_selection_or_the_connection() {
        let mut net = Network::new();
        let a = net.add_node(0.0, 0.0, 0.0, NodeFlag::REGULAR);
        let b = net.add_node(10.0, 0.0, 0.0, NodeFlag::REGULAR);
        let map = MapContext::new(None, 1, 2.0, (2.0, 2.0));
        let none = BTreeSet::new();
        let both: BTreeSet<_> = [a, b].into();
        let mut tool = HideNodeTool::default();
        assert!(tool.picks_hidden());
        assert!(
            tool.on_click(&ctx(&net, &map, &none, Some(a), None))
                .is_none()
        );
        assert_eq!(
            tool.take_request(),
            Some(ToolRequest::ToggleHiddenNodes(vec![a]))
        );
        tool.on_click(&ctx(&net, &map, &both, Some(b), None));
        assert_eq!(
            tool.take_request(),
            Some(ToolRequest::ToggleHiddenNodes(vec![a, b]))
        );
        tool.on_click(&ctx(&net, &map, &none, None, Some((a, b))));
        assert_eq!(tool.take_request(), None);
        let mut links = HideLinkTool::default();
        links.on_click(&ctx(&net, &map, &none, None, Some((b, a))));
        assert_eq!(
            links.take_request(),
            Some(ToolRequest::ToggleHiddenLink(b, a))
        );
        links.on_click(&ctx(&net, &map, &none, Some(a), Some((a, b))));
        assert_eq!(links.take_request(), None);
    }
}
