//! Editing tools (AD-10): stateful controllers that turn input into one transaction and
//! draw their preview on top of the map view. Exactly one tool is active at a time.

pub mod node;

use eframe::egui;

use ad_core::command::Transaction;
use ad_core::geometry::MapGeometry;
use ad_core::map_context::MapContext;

use crate::view::ViewTransform;

/// What a tool sees of a click: the clicked world position and the map it lies on.
pub struct ToolContext<'a> {
    pub x: f64,
    pub z: f64,
    pub map: &'a MapContext,
}

pub trait Tool {
    /// The name shown in the toolbar.
    fn name(&self) -> &'static str;

    /// A primary click on the map; returns the transaction to commit, if any.
    fn on_click(&mut self, ctx: &ToolContext) -> Option<Transaction>;

    /// Draws the preview on top of the map view; `cursor` is the pointer over the view.
    fn paint(
        &self,
        painter: &egui::Painter,
        t: &ViewTransform,
        geometry: MapGeometry,
        cursor: Option<egui::Pos2>,
    );

    /// Drops any uncommitted preview; nothing commits.
    fn cancel(&mut self);
}
