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
use crate::tools::rotate::RotateTool;
use crate::tools::{Tool, ToolContext};
use crate::view::{DragEvent, MapPx, Preview, SelectEvent, ViewHooks, ViewTransform};
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
    let tools: [Box<dyn Tool>; 10] = [
        Box::new(NodeTool::new(NodeFlag::REGULAR)),
        Box::new(NodeTool::new(NodeFlag::SUB_PRIO)),
        Box::new(MoveTool::default()),
        Box::new(RotateTool::default()),
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
    // The shape of the right-drag selection; selection works without an editable document.
    ui.selectable_value(&mut app.lasso, false, "Rect")
        .on_hover_text("Right drag selects a rectangle");
    ui.selectable_value(&mut app.lasso, true, "Lasso")
        .on_hover_text("Right drag selects inside a drawn outline");
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
    /// The right drag draws a lasso instead of a rectangle.
    pub lasso: bool,
    /// The document revision, handed to the tool's `paint_selection`.
    pub revision: u64,
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
            lasso: false,
            revision: 0,
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

/// Most outline points a lasso query uses; a longer path is thinned evenly.
const LASSO_MAX_POINTS: usize = 256;

/// Applies a selection gesture (AD-15): a rectangle toggles every node it covers, edges
/// included (AD-5); a lasso toggles every node inside its outline; a click on a node
/// toggles it; a click on empty map clears the selection. The network never changes.
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
        SelectEvent::Lasso { points } => {
            // The point-in-polygon test costs nodes-in-box × outline length, so a long
            // outline is thinned to at most LASSO_MAX_POINTS evenly spaced points.
            let step = points.len().div_ceil(LASSO_MAX_POINTS).max(1);
            let outline: Vec<(f64, f64)> = points
                .iter()
                .step_by(step)
                .map(|&(px, py)| geometry.to_world(px, py))
                .collect();
            // Even-odd (AD-5); an outline of fewer than 3 points selects nothing.
            toggle(selection, net.in_polygon(&outline));
        }
        SelectEvent::Click { node: Some(id) } => toggle(selection, [id]),
        SelectEvent::Click { node: None } => selection.clear(),
    }
}

impl ViewHooks for ToolHooks<'_> {
    fn select(&mut self, event: SelectEvent) {
        self.select_event = Some(event);
    }

    fn lasso(&self) -> bool {
        self.lasso
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
                selection: self.selection,
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

    fn preview(&self) -> Option<&Preview> {
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
            tool.paint_selection(
                painter,
                t,
                self.map,
                self.net,
                self.selection,
                self.revision,
            );
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
    hooks.lasso = app.lasso;
    hooks.revision = doc.revision;
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
        doc.revision += 1;
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

    /// A lasso through the world points `outline`.
    fn lasso(outline: &[(f64, f64)]) -> SelectEvent {
        let points = outline
            .iter()
            .map(|&(x, z)| (x + 1024.0, z + 1024.0))
            .collect();
        SelectEvent::Lasso { points }
    }

    #[test]
    fn a_lasso_toggles_the_nodes_inside_its_outline() {
        // A curved road: a quarter circle of radius 50, and a node at its centre.
        let mut net = Network::new();
        let arc: Vec<NodeId> = (0..=6)
            .map(|i| {
                let t = f64::from(i) * std::f64::consts::FRAC_PI_2 / 6.0;
                net.add_node(50.0 * t.cos(), 0.0, 50.0 * t.sin(), NodeFlag::REGULAR)
            })
            .collect();
        let centre = net.add_node(0.0, 0.0, 0.0, NodeFlag::REGULAR);
        let g = MapGeometry::new(1.0);
        // A band from radius 40 to 60 around the arc, sampled every 15 degrees.
        let ring = |r: f64| {
            (0..=8).map(move |i| {
                let t = (f64::from(i) * 15.0 - 15.0).to_radians();
                (r * t.cos(), r * t.sin())
            })
        };
        let outline: Vec<_> = ring(60.0).chain(ring(40.0).rev()).collect();
        let mut selection = BTreeSet::from([centre, arc[0]]);
        select(&mut selection, &net, g, lasso(&outline));
        // Every arc node flips (arc[0] off), the centre outside the band keeps its state.
        let mut want: Vec<NodeId> = arc[1..].to_vec();
        want.push(centre);
        want.sort();
        assert_eq!(sorted(&selection), want);
    }

    #[test]
    fn a_concave_lasso_leaves_the_node_in_its_notch() {
        let mut net = Network::new();
        let top = net.add_node(5.0, 0.0, 2.0, NodeFlag::REGULAR);
        let notch = net.add_node(15.0, 0.0, 10.0, NodeFlag::REGULAR);
        let bottom = net.add_node(5.0, 0.0, 18.0, NodeFlag::REGULAR);
        let g = MapGeometry::new(1.0);
        // A C open to the right: the notch is x 10..20, z 5..15.
        let c = [
            (0.0, 0.0),
            (20.0, 0.0),
            (20.0, 5.0),
            (10.0, 5.0),
            (10.0, 15.0),
            (20.0, 15.0),
            (20.0, 20.0),
            (0.0, 20.0),
        ];
        let mut selection = BTreeSet::new();
        select(&mut selection, &net, g, lasso(&c));
        assert_eq!(sorted(&selection), [top, bottom]);
        assert!(!selection.contains(&notch));
    }

    #[test]
    fn a_lasso_of_fewer_than_three_distinct_points_toggles_nothing() {
        let (net, [a, _, _]) = net3();
        let g = MapGeometry::new(1.0);
        for outline in [
            vec![],
            vec![(0.0, 0.0)],
            vec![(-1.0, -1.0), (11.0, 1.0)],
            // Three points, two of them the same: a line through a and b.
            vec![(-1.0, 0.0), (-1.0, 0.0), (11.0, 0.0)],
        ] {
            let mut selection = BTreeSet::from([a]);
            select(&mut selection, &net, g, lasso(&outline));
            assert_eq!(sorted(&selection), [a], "{outline:?}");
        }
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

    /// A right drag through `path` on the map panel.
    fn right_drag(ctx: &egui::Context, app: &mut App, path: &[egui::Pos2]) {
        let (first, last) = (path[0], path[path.len() - 1]);
        let mut events: Vec<Vec<(egui::Pos2, Option<bool>)>> =
            vec![vec![(first, None)], vec![(first, Some(true))]];
        events.extend(path[1..].iter().map(|&p| vec![(p, None)]));
        events.push(vec![(last, Some(false))]);
        events.push(vec![]);
        let frames_ref: Vec<&[(egui::Pos2, Option<bool>)]> =
            events.iter().map(Vec::as_slice).collect();
        frames(ctx, app, egui::PointerButton::Secondary, &frames_ref);
    }

    #[test]
    fn the_switch_picks_the_shape_of_the_next_right_drag() {
        let ctx = egui::Context::default();
        let mut app = app("<version>3</version>");
        let id = app.doc.as_ref().expect("doc").config.network.order()[0];
        let selection = |app: &App| sorted(&app.doc.as_ref().expect("doc").selection);
        // The node is at screen (512.5, 513). A straight drag across it is a rectangle
        // over it, but as a lasso a line around nothing.
        let diagonal = [egui::pos2(400.0, 400.0), egui::pos2(600.0, 600.0)];
        assert!(!app.lasso, "rectangle by default");
        right_drag(&ctx, &mut app, &diagonal);
        assert_eq!(selection(&app), [id]);
        app.lasso = true;
        right_drag(&ctx, &mut app, &diagonal);
        assert_eq!(selection(&app), [id]);
        // A lasso drawn around the node toggles it off.
        let around = [
            egui::pos2(400.0, 400.0),
            egui::pos2(600.0, 400.0),
            egui::pos2(600.0, 600.0),
            egui::pos2(400.0, 600.0),
        ];
        right_drag(&ctx, &mut app, &around);
        assert!(selection(&app).is_empty());
        // And back: the diagonal is a rectangle again.
        app.lasso = false;
        right_drag(&ctx, &mut app, &diagonal);
        assert_eq!(selection(&app), [id]);
        let doc = app.doc.as_ref().expect("doc");
        assert_eq!(doc.config.network.len(), 1);
        assert!(!doc.history.can_undo());
    }

    #[test]
    fn a_right_click_toggles_and_clears_in_lasso_mode() {
        let ctx = egui::Context::default();
        let mut app = app("<version>3</version>");
        app.lasso = true;
        let id = app.doc.as_ref().expect("doc").config.network.order()[0];
        let secondary = egui::PointerButton::Secondary;
        let click = |app: &mut App, p: egui::Pos2| {
            frames(
                &ctx,
                app,
                secondary,
                &[&[(p, None)], &[(p, Some(true))], &[(p, Some(false))], &[]],
            );
            sorted(&app.doc.as_ref().expect("doc").selection)
        };
        // Zoom 2 so the node is large enough to hit: world (1, 2) is at screen (514, 516).
        frames(&ctx, &mut app, secondary, &[&[]]);
        app.view.transform_mut().expect("set").zoom = 2.0;
        let node = egui::pos2(514.0, 516.0);
        assert_eq!(click(&mut app, node), [id]);
        assert_eq!(click(&mut app, node), []);
        assert_eq!(click(&mut app, node), [id]);
        assert_eq!(click(&mut app, egui::pos2(100.0, 100.0)), []);
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
