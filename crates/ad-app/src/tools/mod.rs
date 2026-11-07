//! Editing tools (AD-10): stateful controllers that turn input into one transaction and
//! draw their preview on top of the map view. Exactly one tool is active at a time.

pub mod move_node;
pub mod node;

use eframe::egui;

use ad_core::command::Transaction;
use ad_core::geometry::MapGeometry;
use ad_core::map_context::MapContext;
use ad_core::network::{Network, NodeId};

use crate::view::ViewTransform;

/// What a tool sees of a click or drag: the pointer's world position, the map it lies on,
/// the network, and whether grid snap applies (the toggle, already inverted by Shift).
pub struct ToolContext<'a> {
    pub x: f64,
    pub z: f64,
    pub map: &'a MapContext,
    pub net: &'a Network,
    pub snap: bool,
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

    /// A node the view draws, with its connections, at world (x, z) instead of its stored
    /// position while the tool previews a move.
    fn preview(&self) -> Option<(NodeId, f64, f64)> {
        None
    }

    /// Draws the preview on top of the map view; `cursor` is the pointer over the view.
    /// `snap_grid` is the grid spacing when snap applies this frame (Shift already resolved).
    fn paint(
        &self,
        painter: &egui::Painter,
        t: &ViewTransform,
        geometry: MapGeometry,
        cursor: Option<egui::Pos2>,
        snap_grid: Option<(f64, f64)>,
    );

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
