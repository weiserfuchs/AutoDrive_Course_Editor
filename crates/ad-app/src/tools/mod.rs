//! Editing tools (AD-10): stateful controllers that turn input into one transaction and
//! draw their preview on top of the map view. Exactly one tool is active at a time.

pub mod align;
pub mod arc;
pub mod bezier;
pub mod connect;
pub mod delete;
pub mod edit_location;
pub mod flip;
pub mod hide;
pub mod marker;
pub mod move_node;
pub mod node;
pub mod priority;
pub mod rotate;

use std::collections::BTreeSet;

use eframe::egui;

use ad_core::command::Transaction;
use ad_core::geometry::MapGeometry;
use ad_core::map_context::MapContext;
use ad_core::network::{Network, NodeId};

use crate::view::{Preview, ViewTransform};

/// What a tool sees of a click or drag: the pointer's world position, the map it lies on,
/// the network, the selection, and whether grid snap applies (the toggle, already inverted
/// by Shift).
pub struct ToolContext<'a> {
    pub x: f64,
    pub z: f64,
    pub map: &'a MapContext,
    pub net: &'a Network,
    pub selection: &'a BTreeSet<NodeId>,
    pub snap: bool,
    /// The node under the pointer for a click (a drag gets its node in `on_drag_start`).
    pub hovered_node: Option<NodeId>,
    /// The connection under the pointer for a click; never set while a node is hovered.
    pub hovered_edge: Option<(NodeId, NodeId)>,
}

/// The selection a click acts on as a whole: `Some` when the clicked node is selected,
/// `None` when it is not (or no node is hovered), so the click acts on that target alone.
pub fn acts_on_selection<'a>(ctx: &ToolContext<'a>) -> Option<&'a BTreeSet<NodeId>> {
    ctx.hovered_node
        .filter(|id| ctx.selection.contains(id))
        .map(|_| ctx.selection)
}

/// (x, z) rounded to the nearest multiple of the grid spacing per axis (Java
/// `Math.round(pos / grid) * grid`, which rounds halves up).
pub fn snap_to_grid((x, z): (f64, f64), (gx, gz): (f64, f64)) -> (f64, f64) {
    let axis = |v: f64, g: f64| (v / g + 0.5).floor() * g;
    (axis(x, gx), axis(z, gz))
}

/// A dialog a tool's click asks the app to open for a node, or a notice to show for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolRequest {
    EditLocation(NodeId),
    AddMarker(NodeId),
    EditMarker(NodeId),
    /// A Delete marker click on a node without a marker.
    NoMarker(NodeId),
    /// A Hide node click: hide the shown and show the hidden of these nodes.
    ToggleHiddenNodes(Vec<NodeId>),
    /// A Hide connection click on this connection.
    ToggleHiddenLink(NodeId, NodeId),
}

pub trait Tool {
    /// The name shown in the toolbar.
    fn name(&self) -> &'static str;

    /// A primary click on the map; returns the transaction to commit, if any.
    fn on_click(&mut self, ctx: &ToolContext) -> Option<Transaction>;

    /// The dialog the last click asked for; the app takes it right after the click. Only
    /// the Edit location and marker tools ask.
    fn take_request(&mut self) -> Option<ToolRequest> {
        None
    }

    /// A right click on the map (which also selects, AD-15); only the Bézier tool uses it,
    /// to cancel its pending start.
    fn on_secondary_click(&mut self) {}

    /// The previewed curve the curve panel edits; only the Bézier tool has one.
    fn curve_mut(&mut self) -> Option<&mut bezier::Curve> {
        None
    }

    /// The previewed arc the curve panel edits; only the arc tool has one.
    fn arc_mut(&mut self) -> Option<&mut arc::Arc> {
        None
    }

    /// A primary drag starts at `ctx`, on `hovered` if it started on a node. Returning
    /// true claims the drag: the view does not pan it and reports its moves and end here.
    fn on_drag_start(&mut self, _ctx: &ToolContext, _hovered: Option<NodeId>) -> bool {
        false
    }

    /// The pointer of a claimed drag moved to `ctx`.
    fn on_drag(&mut self, _ctx: &ToolContext) {}

    /// A claimed drag was released at `ctx`; returns the transaction to commit, if any.
    fn on_drag_end(&mut self, _ctx: &ToolContext) -> Option<Transaction> {
        None
    }

    /// A claimed drag ended without a release position (the pointer was lost); by default
    /// the tool's whole preview is cancelled.
    fn on_drag_cancel(&mut self) {
        self.cancel();
    }

    /// The nodes the view draws, with their connections, at world (x, z) instead of their
    /// stored positions while the tool previews a move.
    fn preview(&self) -> Option<&Preview> {
        None
    }

    /// Draws what the tool shows for the selection (the rotate handle) on top of the map
    /// view, before [`Tool::paint`]; `map` gives the geometry and the node size. `hovered`
    /// is this frame's node under the pointer. `revision` changes whenever the
    /// network or the selection may have changed.
    #[allow(clippy::too_many_arguments)]
    fn paint_selection(
        &self,
        _painter: &egui::Painter,
        _t: &ViewTransform,
        _map: &MapContext,
        _net: &Network,
        _selection: &BTreeSet<NodeId>,
        _hovered: Option<NodeId>,
        _revision: u64,
    ) {
    }

    /// Draws the preview on top of the map view; `cursor` is the pointer over the view.
    /// `snap_grid` is the grid spacing when snap applies this frame (Shift already resolved).
    /// `hovered_node` is the world (x, z) of the node under the pointer.
    /// `hovered_edge` is the drawn screen segment of the hovered connection.
    #[allow(clippy::too_many_arguments)]
    fn paint(
        &self,
        painter: &egui::Painter,
        t: &ViewTransform,
        geometry: MapGeometry,
        cursor: Option<egui::Pos2>,
        snap_grid: Option<(f64, f64)>,
        hovered_node: Option<(f64, f64)>,
        hovered_edge: Option<(egui::Pos2, egui::Pos2)>,
    );

    /// The transaction the last click returned was applied as `tx`; only the connect
    /// tools use it, to start the next line at a node the line created.
    fn committed(&mut self, _tx: &Transaction) {}

    /// Continuous connections were turned on or off; only the connect tools use it.
    fn set_continuous(&mut self, _on: bool) {}

    /// The rotation snap step (`None`: whole degrees); only the rotate tool uses it.
    fn set_rotation_step(&mut self, _step: Option<f64>) {}

    /// Whether a click on the connection `(from, to)` would act on it; only such a hovered
    /// connection is highlighted.
    fn acts_on_edge(&self, _net: &Network, _edge: (NodeId, NodeId)) -> bool {
        true
    }

    /// Whether the tool hovers and clicks hidden nodes and connections even without
    /// Select hidden; only the hide tools do (Java `alwaysSelectHidden`).
    fn picks_hidden(&self) -> bool {
        false
    }

    /// Whether the tool holds an uncommitted preview or a pending start (a connect start,
    /// a curve or an arc); autosave waits for it to end (AD-16).
    fn in_progress(&self) -> bool {
        self.preview().is_some()
    }

    /// Drops any uncommitted preview; nothing commits.
    fn cancel(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_rounds_to_the_nearest_grid_point_per_axis() {
        assert_eq!(snap_to_grid((2.9, -4.4), (2.0, 4.0)), (2.0, -4.0));
        assert_eq!(snap_to_grid((3.1, -6.1), (2.0, 4.0)), (4.0, -8.0));
        // Halves round up, as Java's Math.round.
        assert_eq!(snap_to_grid((1.0, -2.0), (2.0, 4.0)), (2.0, 0.0));
    }
}
