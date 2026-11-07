//! Shell panels and windows around the map view.

use std::collections::BTreeSet;

use eframe::egui;

use ad_core::command::Transaction;
use ad_core::geometry::MapGeometry;
use ad_core::history::History;
use ad_core::map_context::MapContext;
use ad_core::network::{ConnectionShape, Network, NodeFlag, NodeId};

use crate::input::{self, Action};
use crate::tools::connect::ConnectTool;
use crate::tools::delete::DeleteTool;
use crate::tools::flip::FlipTool;
use crate::tools::move_node::MoveTool;
use crate::tools::node::NodeTool;
use crate::tools::priority::PriorityTool;
use crate::tools::{Tool, ToolContext};
use crate::view::{DragEvent, MapPx, SelectEvent, ViewHooks, ViewTransform};
use crate::{App, Choice, Pending};

/// The top bar: Open, Save, the Grid toggle and the status labels.
pub(crate) fn top_bar(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
    egui::Panel::top("top_bar").show(ui, |ui| {
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!app.loading && !app.saving, egui::Button::new("Open…"))
                .clicked()
            {
                app.request(Pending::Open, ctx);
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
/// to the view. The transaction a released drag returns waits in `commit`, a selection
/// gesture in `select_event`; both apply after the view has shown.
pub(crate) struct ToolHooks<'a> {
    tool: Option<&'a mut (dyn Tool + 'static)>,
    map: &'a MapContext,
    net: &'a Network,
    selection: &'a BTreeSet<NodeId>,
    snap: bool,
    pub commit: Option<Transaction>,
    pub select_event: Option<SelectEvent>,
}

impl<'a> ToolHooks<'a> {
    pub fn new(
        tool: Option<&'a mut (dyn Tool + 'static)>,
        map: &'a MapContext,
        net: &'a Network,
        selection: &'a BTreeSet<NodeId>,
        snap: bool,
    ) -> Self {
        Self {
            tool,
            map,
            net,
            selection,
            snap,
            commit: None,
            select_event: None,
        }
    }
}

/// Flips the selection state of each of `ids`.
fn toggle(selection: &mut BTreeSet<NodeId>, ids: impl IntoIterator<Item = NodeId>) {
    for id in ids {
        if !selection.remove(&id) {
            selection.insert(id);
        }
    }
}

/// Applies a selection gesture (AD-15): a rectangle toggles every node it covers, edges
/// included (AD-5); a click on a node toggles it; a click on empty map clears the selection.
/// The network never changes.
pub(crate) fn select(
    selection: &mut BTreeSet<NodeId>,
    net: &Network,
    geometry: MapGeometry,
    event: SelectEvent,
) {
    match event {
        SelectEvent::Rect { a, b } => {
            let (ax, az) = geometry.to_world(a.0, a.1);
            let (bx, bz) = geometry.to_world(b.0, b.1);
            let mut ids = Vec::new();
            net.nodes_in_rect_into(ax.min(bx), az.min(bz), ax.max(bx), az.max(bz), &mut ids);
            toggle(selection, ids);
        }
        SelectEvent::Click { node: Some(id) } => toggle(selection, [id]),
        SelectEvent::Click { node: None } => selection.clear(),
    }
}

impl ViewHooks for ToolHooks<'_> {
    fn select(&mut self, event: SelectEvent) {
        self.select_event = Some(event);
    }

    fn selection(&self) -> Option<&BTreeSet<NodeId>> {
        Some(self.selection)
    }

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
    // Selection changes no data, so it works on a read-only document too.
    let tool = app.tool.as_deref_mut().filter(|_| editable);
    let mut hooks = ToolHooks::new(tool, &doc.map, &doc.config.network, &doc.selection, snap);
    let clicked = app.view.show(
        ui,
        &doc.config.network,
        doc.map.geometry,
        doc.map.node_size,
        doc.map_texture.as_ref().map(egui::TextureHandle::id),
        &mut hooks,
    );
    let (commit, select_event) = (hooks.commit.take(), hooks.select_event.take());
    // The network changes only here, on release, through the history (AD-4).
    if let Some(tx) = commit
        && let Err(e) = doc.apply(tx)
    {
        app.error = Some(e.to_string());
    }
    if let Some(event) = select_event {
        select(
            &mut doc.selection,
            &doc.config.network,
            doc.map.geometry,
            event,
        );
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

/// The unsaved-changes prompt: Save (not for a read-only document), Discard or Cancel.
/// Its backdrop keeps the pointer off the map while it is shown.
pub(crate) fn unsaved_prompt(app: &mut App, ctx: &egui::Context) {
    if app.prompt.is_none() {
        return;
    }
    let editable = app.editable();
    let can_save = !app.saving && !app.loading;
    let mut choice = None;
    egui::Modal::new(egui::Id::unique("unsaved_prompt")).show(ctx, |ui| {
        ui.label("The document has unsaved changes.");
        ui.horizontal(|ui| {
            if editable
                && ui
                    .add_enabled(can_save, egui::Button::new("Save"))
                    .clicked()
            {
                choice = Some(Choice::Save);
            }
            if ui.button("Discard").clicked() {
                choice = Some(Choice::Discard);
            }
            if ui.button("Cancel").clicked() {
                choice = Some(Choice::Cancel);
            }
        });
    });
    if let Some(choice) = choice {
        app.choose(choice, ctx);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::path::PathBuf;

    use ad_io::settings::Settings;
    use ad_io::{config_xml, map_image};

    use super::*;
    use crate::document::Document;

    fn sorted(selection: &BTreeSet<NodeId>) -> Vec<NodeId> {
        selection.iter().copied().collect()
    }

    /// Nodes at world (0, 0), (10, 0) and (20, 5) on a scale-1 map, where map px = world + 1024.
    fn net3() -> (Network, [NodeId; 3]) {
        let mut net = Network::new();
        let a = net.add_node(0.0, 0.0, 0.0, NodeFlag::REGULAR);
        let b = net.add_node(10.0, 0.0, 0.0, NodeFlag::REGULAR);
        let c = net.add_node(20.0, 0.0, 5.0, NodeFlag::REGULAR);
        (net, [a, b, c])
    }

    fn rect(a: (f64, f64), b: (f64, f64)) -> SelectEvent {
        let px = |(x, z): (f64, f64)| (x + 1024.0, z + 1024.0);
        SelectEvent::Rect { a: px(a), b: px(b) }
    }

    #[test]
    fn a_rectangle_selects_the_nodes_it_covers() {
        let (net, [a, b, _]) = net3();
        let g = MapGeometry::new(1.0);
        let mut selection = BTreeSet::new();
        // Dragged from bottom right to top left: the corners are normalised.
        select(&mut selection, &net, g, rect((15.0, 2.0), (-1.0, -2.0)));
        assert_eq!(sorted(&selection), [a, b]);
    }

    #[test]
    fn a_node_on_the_rectangle_edge_is_selected() {
        let (net, [a, b, c]) = net3();
        let g = MapGeometry::new(1.0);
        let mut selection = BTreeSet::new();
        // a on the left edge, b on the right edge, c on the bottom-right corner.
        select(&mut selection, &net, g, rect((0.0, -3.0), (10.0, 0.0)));
        assert_eq!(sorted(&selection), [a, b]);
        let mut selection = BTreeSet::new();
        select(&mut selection, &net, g, rect((20.0, 5.0), (12.0, 1.0)));
        assert_eq!(sorted(&selection), [c]);
    }

    #[test]
    fn a_rectangle_toggles_each_covered_node() {
        let (net, [a, b, c]) = net3();
        let g = MapGeometry::new(1.0);
        let mut selection = BTreeSet::from([a, c]);
        select(&mut selection, &net, g, rect((-1.0, -1.0), (11.0, 1.0)));
        assert_eq!(sorted(&selection), [b, c]);
    }

    #[test]
    fn a_click_toggles_a_node_and_a_click_on_nothing_clears() {
        let (net, [a, b, _]) = net3();
        let g = MapGeometry::new(1.0);
        let mut selection = BTreeSet::from([a]);
        select(
            &mut selection,
            &net,
            g,
            SelectEvent::Click { node: Some(b) },
        );
        assert_eq!(sorted(&selection), [a, b]);
        select(
            &mut selection,
            &net,
            g,
            SelectEvent::Click { node: Some(a) },
        );
        assert_eq!(sorted(&selection), [b]);
        select(&mut selection, &net, g, SelectEvent::Click { node: None });
        assert!(selection.is_empty());
    }

    #[test]
    fn toggle_flips_each_id() {
        let (_, [a, b, c]) = net3();
        let mut selection = BTreeSet::from([a, b]);
        toggle(&mut selection, [b, c]);
        assert_eq!(sorted(&selection), [a, c]);
    }

    /// An app with the one-node config of `version`: the node is world (1, 2), at screen
    /// (512.5, 513) on the 1024 px view at minimum zoom 0.5.
    fn app(version: &str) -> App {
        let text = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"no\"?>\n<AutoDrive>\n    \
             {version}\n    <waypoints>\n        <id>1</id>\n        <x>1.000</x>\n        \
             <y>0.000</y>\n        <z>2.000</z>\n        <out>-1</out>\n        \
             <incoming>-1</incoming>\n        <flags>0</flags>\n    </waypoints>\n</AutoDrive>\n"
        );
        let config = config_xml::load(text.as_bytes()).expect("loads");
        let map = MapContext::new(None, 1, 2.0, (2.0, 2.0));
        let doc = Document::new(
            config,
            map,
            map_image::blank(),
            None,
            PathBuf::from("a.xml"),
            1,
            100,
        );
        let mut app = App::new(Settings::default());
        app.doc = Some(doc);
        app
    }

    /// Runs one map-panel frame per event list, pressing `button`.
    fn frames(
        ctx: &egui::Context,
        app: &mut App,
        button: egui::PointerButton,
        frames: &[&[(egui::Pos2, Option<bool>)]],
    ) {
        for events in frames {
            let events = events
                .iter()
                .map(|&(pos, pressed)| match pressed {
                    None => egui::Event::PointerMoved(pos),
                    Some(pressed) => egui::Event::PointerButton {
                        pos,
                        button,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                })
                .collect();
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1024.0, 1024.0),
                )),
                // After the previous frame, also across calls on the same context.
                time: Some(ctx.input(|i| i.time) + 0.016),
                events,
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| map_panel(app, ui));
            out.textures_delta.clear();
        }
    }

    /// A right drag over the node, then a right click on empty map.
    fn select_and_clear(app: &mut App) -> (Vec<NodeId>, Vec<NodeId>) {
        let ctx = egui::Context::default();
        let secondary = egui::PointerButton::Secondary;
        let (a, b) = (egui::pos2(400.0, 400.0), egui::pos2(600.0, 600.0));
        frames(
            &ctx,
            app,
            secondary,
            &[
                &[(a, None)],
                &[(a, Some(true))],
                &[(b, None)],
                &[(b, Some(false))],
                &[],
            ],
        );
        let selected = sorted(&app.doc.as_ref().expect("doc").selection);
        let empty = egui::pos2(100.0, 100.0);
        frames(
            &ctx,
            app,
            secondary,
            &[
                &[(empty, None)],
                &[(empty, Some(true))],
                &[(empty, Some(false))],
                &[],
            ],
        );
        (selected, sorted(&app.doc.as_ref().expect("doc").selection))
    }

    #[test]
    fn the_left_button_is_unchanged_while_nodes_are_selected() {
        let primary = egui::PointerButton::Primary;
        // A left click with the node tool adds a node.
        let ctx = egui::Context::default();
        let mut clicker = app("<version>3</version>");
        let id = clicker.doc.as_ref().expect("doc").config.network.order()[0];
        clicker.doc.as_mut().expect("doc").selection.insert(id);
        clicker.tool = Some(Box::new(NodeTool::new(NodeFlag::REGULAR)));
        let p = egui::pos2(612.0, 412.0);
        frames(
            &ctx,
            &mut clicker,
            primary,
            &[&[(p, None)], &[(p, Some(true))], &[(p, Some(false))], &[]],
        );
        let doc = clicker.doc.as_ref().expect("doc");
        assert_eq!(doc.config.network.len(), 2);
        assert_eq!(sorted(&doc.selection), [id]);

        // A left drag with no tool pans; zoom 2 so the map can move.
        let ctx = egui::Context::default();
        let mut panner = app("<version>3</version>");
        let id = panner.doc.as_ref().expect("doc").config.network.order()[0];
        panner.doc.as_mut().expect("doc").selection.insert(id);
        frames(&ctx, &mut panner, primary, &[&[]]);
        panner.view.transform_mut().expect("set").zoom = 2.0;
        let (a, b) = (egui::pos2(500.0, 500.0), egui::pos2(560.0, 500.0));
        frames(
            &ctx,
            &mut panner,
            primary,
            &[
                &[(a, None)],
                &[(a, Some(true))],
                &[(b, None)],
                &[(b, Some(false))],
                &[],
            ],
        );
        let centre = panner.view.transform_mut().expect("set").center;
        assert_eq!(centre, (994.0, 1024.0));
        let doc = panner.doc.as_ref().expect("doc");
        assert_eq!(doc.config.network.len(), 1);
        assert_eq!(sorted(&doc.selection), [id]);
    }

    #[test]
    fn selection_works_on_a_read_only_document() {
        let mut app = app("<markerID>1</markerID>");
        assert!(!app.editable());
        let id = app.doc.as_ref().expect("doc").config.network.order()[0];
        assert_eq!(select_and_clear(&mut app), (vec![id], vec![]));
    }

    #[test]
    fn selection_works_whichever_tool_is_active() {
        let tools: Vec<Option<Box<dyn Tool>>> = vec![
            None,
            Some(Box::new(NodeTool::new(NodeFlag::REGULAR))),
            Some(Box::new(MoveTool::default())),
            Some(Box::new(DeleteTool)),
            Some(Box::new(PriorityTool)),
            Some(Box::new(FlipTool)),
            Some(Box::new(ConnectTool::new(ConnectionShape::OneWay, false))),
        ];
        for tool in tools {
            let mut app = app("<version>3</version>");
            let name = tool.as_ref().map(|t| t.name());
            app.tool = tool;
            let id = app.doc.as_ref().expect("doc").config.network.order()[0];
            assert_eq!(select_and_clear(&mut app), (vec![id], vec![]), "{name:?}");
            // Selection never changes the network.
            let doc = app.doc.as_ref().expect("doc");
            assert_eq!(doc.config.network.len(), 1, "{name:?}");
            assert!(!doc.history.can_undo(), "{name:?}");
        }
    }
}
