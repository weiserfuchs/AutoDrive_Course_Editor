//! Editing tools (AD-10): stateful controllers that turn input into one transaction and
//! draw their preview on top of the map view. Exactly one tool is active at a time.

pub mod connect;
pub mod delete;
pub mod flip;
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

pub trait Tool {
    /// The name shown in the toolbar.
    fn name(&self) -> &'static str;

    /// A primary click on the map; returns the transaction to commit, if any.
    fn on_click(&mut self, ctx: &ToolContext) -> Option<Transaction>;

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

    /// The nodes the view draws, with their connections, at world (x, z) instead of their
    /// stored positions while the tool previews a move.
    fn preview(&self) -> Option<&Preview> {
        None
    }

    /// Draws what the tool shows for the selection (the rotate handle) on top of the map
    /// view, before [`Tool::paint`]; `map` gives the geometry and the node size. `revision`
    /// changes whenever the network or the selection may have changed.
    fn paint_selection(
        &self,
        _painter: &egui::Painter,
        _t: &ViewTransform,
        _map: &MapContext,
        _net: &Network,
        _selection: &BTreeSet<NodeId>,
        _revision: u64,
    ) {
    }

    /// Draws the preview on top of the map view; `cursor` is the pointer over the view.
    /// `snap_grid` is the grid spacing when snap applies this frame (Shift already resolved).
    /// `hovered_edge` is the drawn screen segment of the hovered connection.
    fn paint(
        &self,
        painter: &egui::Painter,
        t: &ViewTransform,
        geometry: MapGeometry,
        cursor: Option<egui::Pos2>,
        snap_grid: Option<(f64, f64)>,
        hovered_edge: Option<(egui::Pos2, egui::Pos2)>,
    );

    /// Continuous connections were turned on or off; only the connect tools use it.
    fn set_continuous(&mut self, _on: bool) {}

    /// Whether a click on the connection `(from, to)` would act on it; only such a hovered
    /// connection is highlighted.
    fn acts_on_edge(&self, _net: &Network, _edge: (NodeId, NodeId)) -> bool {
        true
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
