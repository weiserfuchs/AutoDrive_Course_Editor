//! Shell panels and windows around the map view.

use eframe::egui;

use ad_core::command::Transaction;
use ad_core::geometry::MapGeometry;
use ad_core::history::History;
use ad_core::map_context::MapContext;
use ad_core::network::{ConnectionShape, Network, NodeFlag, NodeId};

use crate::App;
use crate::input::{self, Action};
use crate::tools::connect::ConnectTool;
use crate::tools::delete::DeleteTool;
use crate::tools::flip::FlipTool;
use crate::tools::move_node::MoveTool;
use crate::tools::node::NodeTool;
use crate::tools::priority::PriorityTool;
use crate::tools::{Tool, ToolContext};
use crate::view::{DragEvent, MapPx, ViewHooks, ViewTransform};

/// The top bar: Open, Save, the Grid toggle and the status labels.
pub(crate) fn top_bar(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
    egui::Panel::top("top_bar").show(ui, |ui| {
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!app.loading && !app.saving, egui::Button::new("Open…"))
                .clicked()
            {
                app.open(ctx);
            }
            // Load and save never overlap: either could read or replace the same file.
            let editable = !app.saving
                && !app.loading
                && app
                    .doc
                    .as_ref()
                    .is_some_and(|d| d.config.read_only().is_none());
            if ui
                .add_enabled(editable, egui::Button::new("Save"))
                .clicked()
            {
                app.save(ctx);
            }
            ui.checkbox(&mut app.view.show_grid, "Grid");
            ui.separator();
            toolbar(app, ui);
            ui.separator();
            if app.loading {
                ui.label("Loading…");
            }
            if app.saving {
                ui.label("Saving…");
            }
            if let Some(reason) = app.doc.as_ref().and_then(|d| d.config.read_only()) {
                ui.colored_label(ui.visuals().warn_fg_color, reason.to_string());
            }
            if let Some(notice) = app.doc.as_ref().and_then(|d| d.notice.as_deref()) {
                ui.colored_label(ui.visuals().warn_fg_color, notice);
            }
        });
    });
}

/// The tool buttons, Undo and Redo; all disabled without an editable document (AD-11).
fn toolbar(app: &mut App, ui: &mut egui::Ui) {
    let editable = app.editable();
    let active = app.tool.as_ref().map(|t| t.name());
    let continuous = app.continuous;
    let tools: [Box<dyn Tool>; 9] = [
        Box::new(NodeTool::new(NodeFlag::REGULAR)),
        Box::new(NodeTool::new(NodeFlag::SUB_PRIO)),
        Box::new(MoveTool::default()),
        Box::new(DeleteTool),
        Box::new(PriorityTool),
        Box::new(FlipTool),
        Box::new(ConnectTool::new(ConnectionShape::OneWay, continuous)),
        Box::new(ConnectTool::new(ConnectionShape::Dual, continuous)),
        Box::new(ConnectTool::new(ConnectionShape::Reverse, continuous)),
    ];
    for tool in tools {
        let selected = active == Some(tool.name());
        let button = egui::Button::selectable(selected, tool.name());
        if ui.add_enabled(editable, button).clicked() {
            // A click on the active tool turns it off.
            app.set_tool((!selected).then_some(tool));
        }
    }
    let mut continuous = app.continuous;
    if ui
        .checkbox(&mut continuous, "Continuous")
        .on_hover_text("; (L cycles the connect tools)")
        .changed()
    {
        app.run(Action::ToggleContinuous);
    }
    ui.checkbox(&mut app.snap, "Snap")
        .on_hover_text("S; hold Shift to invert");
    let history = app.doc.as_ref().map(|d| &d.history);
    let can_undo = editable && history.is_some_and(History::can_undo);
    let can_redo = editable && history.is_some_and(History::can_redo);
    if ui
        .add_enabled(can_undo, egui::Button::new("Undo"))
        .on_hover_text("Ctrl+Z")
        .clicked()
    {
        app.run(Action::Undo);
    }
    if ui
        .add_enabled(can_redo, egui::Button::new("Redo"))
        .on_hover_text("Ctrl+Shift+Z")
        .clicked()
    {
        app.run(Action::Redo);
    }
}

/// Connects the active tool to the map view: drags go to the tool, its preview and paint
/// to the view. The transaction a released drag returns waits in `commit`.
pub(crate) struct ToolHooks<'a> {
    tool: Option<&'a mut (dyn Tool + 'static)>,
    map: &'a MapContext,
    net: &'a Network,
    snap: bool,
    pub commit: Option<Transaction>,
}

impl<'a> ToolHooks<'a> {
    pub fn new(
        tool: Option<&'a mut (dyn Tool + 'static)>,
        map: &'a MapContext,
        net: &'a Network,
        snap: bool,
    ) -> Self {
        Self {
            tool,
            map,
            net,
            snap,
            commit: None,
        }
    }
}

impl ViewHooks for ToolHooks<'_> {
    fn drag(&mut self, event: DragEvent) -> bool {
        let Some(tool) = self.tool.as_deref_mut() else {
            return false;
        };
        let ctx = |(px, py): MapPx| {
            let (x, z) = self.map.geometry.to_world(px, py);
            ToolContext {
                x,
                z,
                map: self.map,
                net: self.net,
                snap: self.snap,
                hovered_node: None,
                hovered_edge: None,
            }
        };
        match event {
            DragEvent::Start { at, node } => tool.on_drag_start(&ctx(at), node),
            DragEvent::Move { at } => {
                tool.on_drag(&ctx(at));
                true
            }
            DragEvent::End { at } => {
                self.commit = tool.on_drag_end(&ctx(at));
                true
            }
            DragEvent::Cancel => {
                tool.cancel();
                true
            }
        }
    }

    fn preview(&self) -> Option<(NodeId, f64, f64)> {
        self.tool.as_deref().and_then(Tool::preview)
    }

    fn highlights_edge(&self, edge: (NodeId, NodeId)) -> bool {
        self.tool
            .as_deref()
            .is_some_and(|tool| tool.acts_on_edge(self.net, edge))
    }

    fn paint(
        &self,
        painter: &egui::Painter,
        t: &ViewTransform,
        geometry: MapGeometry,
        cursor: Option<egui::Pos2>,
        hovered_edge: Option<(egui::Pos2, egui::Pos2)>,
    ) {
        if let Some(tool) = self.tool.as_deref() {
            let snap_grid = self.snap.then_some(self.map.grid);
            tool.paint(painter, t, geometry, cursor, snap_grid, hovered_edge);
        }
    }
}

/// The map view of the open document, with the overlay hook tools draw into.
pub(crate) fn map_panel(app: &mut App, ui: &mut egui::Ui) {
    let editable = app.editable();
    let snap = input::snap(app.snap, ui.ctx());
    let Some(doc) = &mut app.doc else {
        app.view.clear_hover();
        ui.allocate_space(ui.available_size());
        return;
    };
    if app.view_epoch != Some(doc.epoch) {
        app.view.reset();
        app.view_epoch = Some(doc.epoch);
    }
    // Upload once per document, then drop the CPU copy (AD-14).
    if let Some(image) = doc.map_image.take() {
        let color = egui::ColorImage::from_rgba_unmultiplied([2048, 2048], &image.rgba);
        doc.map_texture = Some(ui.ctx().load_texture(
            "map_image",
            color,
            egui::TextureOptions::LINEAR,
        ));
    }
    app.view.grid = doc.map.grid;
    // A read-only document gets no tool input (AD-11).
    let tool = app.tool.as_deref_mut().filter(|_| editable);
    let mut hooks = ToolHooks::new(tool, &doc.map, &doc.config.network, snap);
    let clicked = app.view.show(
        ui,
        &doc.config.network,
        doc.map.geometry,
        doc.map.node_size,
        doc.map_texture.as_ref().map(egui::TextureHandle::id),
        &mut hooks,
    );
    // The network changes only here, on release, through the history (AD-4).
    if let Some(tx) = hooks.commit.take()
        && let Err(e) = doc.apply(tx)
    {
        app.error = Some(e.to_string());
    }
    if let Some((px, py)) = clicked {
        app.click(px, py, snap);
    }
}

/// The error window, shown until the user closes it.
pub(crate) fn error_window(app: &mut App, ctx: &egui::Context) {
    if let Some(message) = app.error.clone() {
        let mut open = true;
        egui::Window::new("Error")
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(message);
            });
        if !open {
            app.error = None;
        }
    }
}
