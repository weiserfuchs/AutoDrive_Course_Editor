//! Shell panels and windows around the map view.

use std::collections::BTreeSet;

use eframe::egui;

use ad_core::command::Transaction;
use ad_core::geometry::MapGeometry;
use ad_core::history::History;
use ad_core::map_context::MapContext;
use ad_core::network::{ConnectionShape, Network, NodeFlag, NodeId};
use ad_io::config_xml::GameVersion;

use crate::document::Fix;
use crate::input::{self, Action};
use crate::tools::align::{AlignTool, Axis};
use crate::tools::arc::ArcTool;
use crate::tools::bezier::{self, BezierTool, CurveSettings};
use crate::tools::connect::ConnectTool;
use crate::tools::delete::DeleteTool;
use crate::tools::edit_location::{self, EditLocationTool};
use crate::tools::flip::FlipTool;
use crate::tools::marker::{ALL, AddMarkerTool, DeleteMarkerTool, EditMarkerTool, group_counts};
use crate::tools::move_node::MoveTool;
use crate::tools::node::NodeTool;
use crate::tools::priority::PriorityTool;
use crate::tools::rotate::RotateTool;
use crate::tools::{Tool, ToolContext};
use crate::view::{DragEvent, MapPx, Preview, SelectEvent, ViewHooks, ViewTransform};
use crate::{App, Choice, Pending};

/// The top bar: Open, Save, the Import menu, the Grid toggle and the status labels.
pub(crate) fn top_bar(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
    egui::Panel::top("top_bar").show(ui, |ui| {
        // Wraps onto further rows when the window is narrower than all the buttons, so no
        // tool or notice is cut off at the right edge.
        ui.horizontal_wrapped(|ui| {
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
            // Not while downloading either: the download would replace the imported image.
            let can_import = !app.loading && !app.importing && !app.downloading_for_doc();
            let has_doc = app.doc.is_some();
            let has_heightmap = can_export_heightmap(app);
            ui.add_enabled_ui(can_import, |ui| {
                ui.menu_button("Import", |ui| {
                    if ui
                        .add_enabled(has_doc, egui::Button::new("DDS for FS19…"))
                        .clicked()
                    {
                        app.import(GameVersion::FS19, ctx);
                    }
                    if ui
                        .add_enabled(has_doc, egui::Button::new("DDS for FS22/FS25…"))
                        .clicked()
                    {
                        app.import(GameVersion::FS25, ctx);
                    }
                    if ui.button("Map mod zip…").clicked() {
                        app.import_zip(ctx);
                    }
                    ui.separator();
                    if ui
                        .add_enabled(has_doc, egui::Button::new("Load map image…"))
                        .clicked()
                    {
                        app.load_image(ctx);
                    }
                    if ui
                        .add_enabled(has_doc, egui::Button::new("Save map image…"))
                        .clicked()
                    {
                        app.save_image(ctx);
                    }
                    if ui
                        .add_enabled(has_doc, egui::Button::new("Import heightmap…"))
                        .clicked()
                    {
                        app.import_heightmap(ctx);
                    }
                    if ui
                        .add_enabled(has_heightmap, egui::Button::new("Export heightmap…"))
                        .clicked()
                    {
                        app.export_heightmap(ctx);
                    }
                });
            });
            let has_marks = app.doc.as_ref().is_some_and(|d| d.overlap.is_some());
            ui.add_enabled_ui(has_doc, |ui| {
                ui.menu_button("Scan", |ui| {
                    if ui.button("Find overlapping nodes…").clicked() {
                        app.overlap_dialog = Some(crate::OVERLAP_DEFAULT.to_owned());
                    }
                    if ui
                        .add_enabled(has_marks, egui::Button::new("Clear overlap marks"))
                        .clicked()
                        && let Some(doc) = &mut app.doc
                    {
                        doc.clear_overlaps();
                    }
                    if ui
                        .add_enabled(
                            app.can_merge(),
                            egui::Button::new("Merge overlapping nodes…"),
                        )
                        .clicked()
                    {
                        app.ask_merge();
                    }
                    ui.separator();
                    for (fix, label) in [
                        (Fix::OutOfBounds, "Fix nodes outside the map…"),
                        (Fix::MissingHeights, "Fix missing node heights…"),
                    ] {
                        if ui
                            .add_enabled(app.can_fix(fix), egui::Button::new(label))
                            .clicked()
                        {
                            app.ask_fix(fix);
                        }
                    }
                    ui.separator();
                    if ui.button("Check connections…").clicked() {
                        app.check_connections();
                    }
                });
            });
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
            if app.merging {
                ui.label("Writing merge backup…");
            }
            if app.importing {
                ui.label("Importing…");
            }
            if app.exporting {
                ui.label("Saving image…");
            }
            if let Some(label) = download_label(app) {
                ui.label(label);
            }
            if let Some(reason) = app.doc.as_ref().and_then(|d| d.config.read_only()) {
                ui.colored_label(ui.visuals().warn_fg_color, reason.to_string());
            }
            if let Some(notice) = app.doc.as_ref().and_then(|d| d.notice.as_deref()) {
                ui.colored_label(ui.visuals().warn_fg_color, notice);
            }
            if let Some(notice) = app.tool_notice {
                ui.label(notice);
            }
        });
    });
}

/// Whether Export heightmap is enabled: the open document has a heightmap.
pub(crate) fn can_export_heightmap(app: &App) -> bool {
    app.doc.as_ref().is_some_and(|d| d.map.heightmap.is_some())
}

/// The top bar label while a map image downloads for the open document.
pub(crate) fn download_label(app: &App) -> Option<&'static str> {
    app.downloading_for_doc()
        .then_some("Downloading map image…")
}

/// The tool buttons, Undo and Redo; all disabled without an editable document (AD-11).
fn toolbar(app: &mut App, ui: &mut egui::Ui) {
    let editable = app.editable();
    let active = app.tool.as_ref().map(|t| t.name());
    let continuous = app.continuous;
    let line = app.line();
    let tools: [Box<dyn Tool>; 19] = [
        Box::new(NodeTool::new(NodeFlag::REGULAR)),
        Box::new(NodeTool::new(NodeFlag::SUB_PRIO)),
        Box::new(MoveTool::default()),
        Box::new(RotateTool::default()),
        Box::new(AlignTool::new(Axis::Horizontal)),
        Box::new(AlignTool::new(Axis::Vertical)),
        Box::new(AlignTool::new(Axis::Depth)),
        Box::new(EditLocationTool::default()),
        Box::new(DeleteTool),
        Box::new(PriorityTool),
        Box::new(FlipTool),
        Box::new(ConnectTool::new(ConnectionShape::OneWay, continuous, line)),
        Box::new(ConnectTool::new(ConnectionShape::Dual, continuous, line)),
        Box::new(ConnectTool::new(ConnectionShape::Reverse, continuous, line)),
        Box::new(BezierTool::default()),
        Box::new(ArcTool::default()),
        Box::new(AddMarkerTool::default()),
        Box::new(EditMarkerTool::default()),
        Box::new(DeleteMarkerTool::default()),
    ];
    for tool in tools {
        let selected = active == Some(tool.name());
        let button = egui::Button::selectable(selected, tool.name());
        if ui.add_enabled(editable, button).clicked() {
            // A click on the active tool turns it off.
            app.set_tool((!selected).then_some(tool));
        }
    }
    if ui
        .add_enabled(editable, egui::Button::new("Groups"))
        .on_hover_text("Rename or delete marker groups")
        .clicked()
    {
        app.open_groups();
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
    /// The nodes the overlap scan marks, handed to the view.
    pub overlap: Option<&'a BTreeSet<NodeId>>,
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
            overlap: None,
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
        if let (SelectEvent::Click { .. }, Some(tool)) = (&event, self.tool.as_deref_mut()) {
            tool.on_secondary_click();
        }
        self.select_event = Some(event);
    }

    fn lasso(&self) -> bool {
        self.lasso
    }

    fn selection(&self) -> Option<&BTreeSet<NodeId>> {
        Some(self.selection)
    }

    fn overlap(&self) -> Option<&BTreeSet<NodeId>> {
        self.overlap
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
                tool.on_drag_cancel();
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
        hovered_node: Option<NodeId>,
        hovered_edge: Option<(egui::Pos2, egui::Pos2)>,
    ) {
        if let Some(tool) = self.tool.as_deref() {
            tool.paint_selection(
                painter,
                t,
                self.map,
                self.net,
                self.selection,
                hovered_node,
                self.revision,
            );
            let snap_grid = self.snap.then_some(self.map.grid);
            let hovered_node = hovered_node
                .and_then(|id| self.net.node(id))
                .map(|n| (n.x, n.z));
            tool.paint(
                painter,
                t,
                geometry,
                cursor,
                snap_grid,
                hovered_node,
                hovered_edge,
            );
        }
    }
}

/// The map view of the open document, with the overlay hook tools draw into.
pub(crate) fn map_panel(app: &mut App, ui: &mut egui::Ui) {
    let editable = app.editable();
    // A node dialog is modal: the map takes no tool input or selection then.
    let modal = app.dialog_shown();
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
    let tool = app.tool.as_deref_mut().filter(|_| editable && !modal);
    let mut hooks = ToolHooks::new(tool, &doc.map, &doc.config.network, &doc.selection, snap);
    hooks.lasso = app.lasso;
    hooks.revision = doc.revision;
    hooks.overlap = doc.overlap.as_ref().map(|(_, marks)| marks);
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
    if let Some(event) = select_event.filter(|_| !modal) {
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

/// What the curve panel's buttons ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CurveChoice {
    Commit,
    Cancel,
}

/// The curve panel, a non-modal window shown while the Bézier tool previews a curve or the
/// arc tool an arc: it edits the preview directly (the preview follows), Commit applies
/// it, Cancel drops it. Only a Bézier curve has the control-point setting.
pub(crate) fn curve_panel(app: &mut App, ctx: &egui::Context) {
    let Some(tool) = app.tool.as_deref_mut() else {
        return;
    };
    let window = |title: &str| egui::Window::new(title).collapsible(false).resizable(false);
    let (mut choice, mut swap) = (None, false);
    if let Some(curve) = tool.curve_mut() {
        window("Bézier curve").show(ctx, |ui| {
            let controls = Some(&mut curve.controls);
            choice = curve_controls(ui, &mut curve.settings, controls, &mut swap);
        });
        if swap {
            curve.swap();
        }
    } else if let Some(arc) = tool.arc_mut() {
        window("Arc").show(ctx, |ui| {
            choice = curve_controls(ui, &mut arc.settings, None, &mut swap);
        });
        if swap {
            arc.swap();
        }
    }
    if let Some(choice) = choice {
        app.finish_curve(choice);
    }
}

/// The curve panel's controls (Java `CurvePanel`), with the control-point setting when
/// `controls` is given; sets `swap` when "Swap direction" is pressed. Returns Commit or
/// Cancel when pressed.
fn curve_controls(
    ui: &mut egui::Ui,
    settings: &mut CurveSettings,
    controls: Option<&mut usize>,
    swap: &mut bool,
) -> Option<CurveChoice> {
    ui.add(
        egui::Slider::new(&mut settings.points, 1..=bezier::MAX_POINTS)
            .text("Interpolation points"),
    );
    if let Some(controls) = controls {
        ui.add(egui::Slider::new(controls, 1..=bezier::MAX_CONTROLS).text("Control points"));
    }
    ui.horizontal(|ui| {
        ui.label("Priority");
        ui.selectable_value(&mut settings.flag, NodeFlag::REGULAR, "Regular");
        ui.selectable_value(&mut settings.flag, NodeFlag::SUB_PRIO, "Sub-priority");
    });
    ui.horizontal(|ui| {
        ui.label("Connection");
        ui.selectable_value(&mut settings.shape, ConnectionShape::OneWay, "Regular");
        ui.selectable_value(&mut settings.shape, ConnectionShape::Dual, "Dual");
        ui.selectable_value(&mut settings.shape, ConnectionShape::Reverse, "Reverse");
    });
    *swap = ui.button("Swap direction").clicked();
    let mut choice = None;
    ui.horizontal(|ui| {
        if ui.button("Commit").clicked() {
            choice = Some(CurveChoice::Commit);
        }
        if ui.button("Cancel").clicked() {
            choice = Some(CurveChoice::Cancel);
        }
    });
    choice
}

/// The Edit location dialog: x, y and z fields, each with a red note while its text is
/// refused. OK or Enter, with every field valid, commits; Cancel, Escape or a click
/// beside the dialog commits nothing.
pub(crate) fn location_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(scale) = app.doc.as_ref().map(|d| d.map.geometry.scale) else {
        return;
    };
    let Some(dialog) = &mut app.location else {
        return;
    };
    let (mut ok, mut cancel) = (false, false);
    let modal = egui::Modal::new(egui::Id::unique("edit_location")).show(ctx, |ui| {
        ui.heading("Edit location");
        let mut valid = true;
        for (i, axis) in edit_location::AXES.iter().enumerate() {
            ui.label(*axis);
            ui.text_edit_singleline(&mut dialog.text[i]);
            if let Err(note) = &dialog.values(scale)[i] {
                ui.colored_label(ui.visuals().error_fg_color, note.to_string());
                valid = false;
            }
        }
        ui.horizontal(|ui| {
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            ok = ui.add_enabled(valid, egui::Button::new("OK")).clicked() || (valid && enter);
            cancel = ui.button("Cancel").clicked();
        });
    });
    // Always asked, so Escape is consumed while the dialog is on top.
    let closed = modal.should_close();
    if ok {
        app.finish_location(true);
    } else if cancel || closed {
        app.finish_location(false);
    }
}

/// The overlap scan dialog: a distance field with a red note while its text is refused.
/// OK or Enter, with the distance valid, scans; Cancel, Escape or a click beside the
/// dialog scans nothing.
pub(crate) fn overlap_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(text) = &mut app.overlap_dialog else {
        return;
    };
    let (mut ok, mut cancel) = (false, false);
    let modal = egui::Modal::new(egui::Id::unique("overlap_scan")).show(ctx, |ui| {
        ui.heading("Find overlapping nodes");
        ui.label("Distance");
        ui.text_edit_singleline(text);
        let valid = match crate::overlap_distance(text) {
            Ok(_) => true,
            Err(note) => {
                ui.colored_label(ui.visuals().error_fg_color, note.to_string());
                false
            }
        };
        ui.horizontal(|ui| {
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            ok = ui.add_enabled(valid, egui::Button::new("OK")).clicked() || (valid && enter);
            cancel = ui.button("Cancel").clicked();
        });
    });
    // Always asked, so Escape is consumed while the dialog is on top.
    let closed = modal.should_close();
    if ok {
        app.finish_overlap(true);
    } else if cancel || closed {
        app.finish_overlap(false);
    }
}

/// "Merge N marked nodes into M nodes?" with the backup's path. Merge writes the backup,
/// then merges; Cancel, Escape or a click beside it merges nothing.
pub(crate) fn merge_prompt(app: &mut App, ctx: &egui::Context) {
    let (Some((marked, kept)), Some(doc)) = (app.merge_prompt, &app.doc) else {
        return;
    };
    let backup = crate::jobs::merge_backup_path(&doc.path);
    let mut answer = None;
    let modal = egui::Modal::new(egui::Id::unique("merge_overlaps")).show(ctx, |ui| {
        ui.label(format!(
            "Merge {marked} marked nodes into {kept} nodes? A backup is written first."
        ));
        ui.label(backup.display().to_string());
        ui.horizontal(|ui| {
            if ui.button("Merge").clicked() {
                answer = Some(true);
            }
            if ui.button("Cancel").clicked() {
                answer = Some(false);
            }
        });
    });
    if modal.should_close() && answer.is_none() {
        answer = Some(false);
    }
    if let Some(ok) = answer {
        let ctx = ctx.clone();
        app.finish_merge(ok, move || ctx.request_repaint());
    }
}

/// The question of a Scan menu fix with its count (and, for nodes outside the map, the
/// bounds). OK fixes in one undoable entry; Cancel, Escape or a click beside it fixes
/// nothing.
pub(crate) fn fix_prompt(app: &mut App, ctx: &egui::Context) {
    let (Some((fix, count)), Some(doc)) = (app.fix_prompt, &app.doc) else {
        return;
    };
    let half = ad_core::scan::map_half_size(doc.map.geometry);
    let mut answer = None;
    let modal = egui::Modal::new(egui::Id::unique("fix_nodes")).show(ctx, |ui| {
        match fix {
            Fix::OutOfBounds => {
                ui.label(format!(
                    "Move {count} nodes outside the map to the nearest point of its edge?"
                ));
                ui.label(format!("X: {} to {half}\nZ: {} to {half}", -half, -half));
            }
            Fix::MissingHeights => {
                ui.label(format!(
                    "Set the height of {count} nodes without one from the heightmap?"
                ));
            }
        }
        ui.horizontal(|ui| {
            if ui.button("OK").clicked() {
                answer = Some(true);
            }
            if ui.button("Cancel").clicked() {
                answer = Some(false);
            }
        });
    });
    if modal.should_close() && answer.is_none() {
        answer = Some(false);
    }
    if let Some(ok) = answer {
        app.finish_fix(ok);
    }
}

/// The connection check's counts per kind. With an editable document, Fix removes the
/// entries in one undoable entry; Close, Escape or a click beside it changes nothing.
pub(crate) fn connection_report(app: &mut App, ctx: &egui::Context) {
    let Some(defects) = &app.connection_report else {
        return;
    };
    let editable = app.editable();
    let mut answer = None;
    let modal = egui::Modal::new(egui::Id::unique("connection_report")).show(ctx, |ui| {
        ui.heading("Connection check");
        ui.label(format!(
            "Dangling incoming entries: {}\nDuplicate entries: {}\nSelf-links: {}",
            defects.dangling.len(),
            defects.duplicates.len(),
            defects.self_links.len()
        ));
        if !editable {
            ui.label("The config is read-only.");
        }
        ui.horizontal(|ui| {
            if editable && ui.button("Fix").clicked() {
                answer = Some(true);
            }
            if ui.button("Close").clicked() {
                answer = Some(false);
            }
        });
    });
    if modal.should_close() && answer.is_none() {
        answer = Some(false);
    }
    if let Some(fix) = answer {
        app.finish_connections(fix);
    }
}

/// The marker dialog: a name field with a red note while the name is refused, and a group
/// field with a drop-down of "None" and the existing groups, headed "Add marker" or "Edit
/// marker". OK or Enter, with the name valid, commits; Cancel, Escape or a click beside
/// the dialog commits nothing.
pub(crate) fn marker_dialog(app: &mut App, ctx: &egui::Context) {
    let (Some(doc), Some(dialog)) = (&app.doc, &mut app.marker) else {
        return;
    };
    let net = &doc.config.network;
    let (mut ok, mut cancel) = (false, false);
    let modal = egui::Modal::new(egui::Id::unique("marker")).show(ctx, |ui| {
        ui.heading(dialog.title());
        ui.label("Name");
        ui.text_edit_singleline(&mut dialog.name);
        let valid = match dialog.validate(net) {
            Ok(_) => true,
            Err(note) => {
                ui.colored_label(ui.visuals().error_fg_color, note.to_string());
                false
            }
        };
        ui.label("Group");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut dialog.group);
            egui::ComboBox::from_id_salt("marker_group")
                .selected_text("Groups")
                .show_ui(ui, |ui| {
                    for choice in &dialog.choices {
                        if ui
                            .selectable_label(dialog.group == *choice, choice)
                            .clicked()
                        {
                            dialog.group.clone_from(choice);
                        }
                    }
                });
        });
        ui.horizontal(|ui| {
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            ok = ui.add_enabled(valid, egui::Button::new("OK")).clicked() || (valid && enter);
            cancel = ui.button("Cancel").clicked();
        });
    });
    // Always asked, so Escape is consumed while the dialog is on top.
    let closed = modal.should_close();
    if ok {
        app.finish_marker(true);
    } else if cancel || closed {
        app.finish_marker(false);
    }
}

/// The groups window: one row per group with its marker count, "All" first. Rename opens
/// a text field in the row, with a red note while the name is refused; OK or Enter commits
/// one `RenameGroup`, Cancel closes the field. Delete moves the group's markers to "All".
/// "All" has neither button. Close, Escape or a click beside the window closes it.
pub(crate) fn groups_window(app: &mut App, ctx: &egui::Context) {
    let (Some(doc), Some(window)) = (&app.doc, &mut app.groups) else {
        return;
    };
    let rows = group_counts(&doc.config.network);
    let field = egui::Id::unique("group_rename");
    let (mut ok, mut delete, mut close) = (false, None, false);
    let modal = egui::Modal::new(egui::Id::unique("groups")).show(ctx, |ui| {
        ui.heading("Groups");
        egui::ScrollArea::vertical()
            .max_height(400.0)
            .show(ui, |ui| {
                for (group, count) in &rows {
                    let renaming = window
                        .rename
                        .as_ref()
                        .is_some_and(|(from, _)| from == group);
                    if !renaming {
                        ui.horizontal(|ui| {
                            ui.label(format!("{group} ({count})"));
                            if group != ALL {
                                if ui.button("Rename").clicked() {
                                    window.start_rename(group);
                                    ui.memory_mut(|m| m.request_focus(field));
                                }
                                if ui.button("Delete").clicked() {
                                    delete = Some(group.clone());
                                }
                            }
                        });
                        continue;
                    }
                    let note = window.validate().and_then(Result::err);
                    ui.horizontal(|ui| {
                        if let Some((_, text)) = &mut window.rename {
                            ui.add(egui::TextEdit::singleline(text).id(field));
                        }
                        ui.label(format!("({count})"));
                        let valid = note.is_none();
                        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                        ok = ui.add_enabled(valid, egui::Button::new("OK")).clicked()
                            || (valid && enter);
                        if ui.button("Cancel").clicked() {
                            window.rename = None;
                        }
                    });
                    if let Some(note) = note {
                        ui.colored_label(ui.visuals().error_fg_color, note.to_string());
                    }
                }
            });
        close = ui.button("Close").clicked();
    });
    // Always asked, so Escape is consumed while the window is on top.
    let closed = modal.should_close();
    if ok {
        app.finish_group_rename();
    }
    if let Some(group) = delete {
        app.delete_group(&group);
    }
    if close || closed {
        app.groups = None;
    }
}

/// "Replace the existing map image?" after an import whose target exists. Yes saves the
/// shown image over it; No or Escape keeps it shown without saving.
pub(crate) fn replace_image_prompt(app: &mut App, ctx: &egui::Context) {
    let Some((target, _)) = app.doc.as_ref().and_then(|d| d.image_to_replace.as_ref()) else {
        return;
    };
    let target = target.display().to_string();
    let mut answer = None;
    let modal = egui::Modal::new(egui::Id::unique("replace_image")).show(ctx, |ui| {
        ui.label("Replace the existing map image?");
        ui.label(target);
        ui.horizontal(|ui| {
            if ui.button("Yes").clicked() {
                answer = Some(true);
            }
            if ui.button("No").clicked() {
                answer = Some(false);
            }
        });
    });
    if modal.should_close() && answer.is_none() {
        answer = Some(false);
    }
    if let Some(replace) = answer {
        let ctx = ctx.clone();
        app.answer_replace(replace, move || ctx.request_repaint());
    }
}

/// What the zip preview's buttons ask for.
enum ZipChoice {
    Import { confirmed: bool },
    KeepExisting,
    Cancel,
}

/// The map mod zip preview: map and heightmap previews, the game (whose crop Import
/// uses), the map name, Import and Cancel. When Import would replace files, it asks first;
/// No returns to the preview. Escape or a click beside it cancels.
pub(crate) fn zip_preview_window(app: &mut App, ctx: &egui::Context) {
    let Some(preview) = &mut app.zip_preview else {
        return;
    };
    let read = &preview.read;
    let textures = preview.textures.get_or_insert_with(|| {
        let load = |name: &str, image: &image::RgbaImage| {
            let size = [image.width() as usize, image.height() as usize];
            let color = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
            ctx.load_texture(name, color, egui::TextureOptions::LINEAR)
        };
        crate::zip_import::PreviewTextures {
            whole: load("zip_preview_whole", &read.whole),
            centre: load("zip_preview_centre", &read.centre),
            heightmap: read
                .heightmap
                .as_ref()
                .map(|h| load("zip_preview_heightmap", h)),
        }
    });
    let mut choice = None;
    let side = egui::vec2(256.0, 256.0);
    let modal = egui::Modal::new(egui::Id::unique("zip_preview")).show(ctx, |ui| {
        ui.heading("Import map mod zip");
        if let Some(existing) = &preview.replace {
            ui.label("Replace the existing files?");
            for path in existing {
                ui.label(path.display().to_string());
            }
            ui.horizontal(|ui| {
                if ui.button("Yes").clicked() {
                    choice = Some(ZipChoice::Import { confirmed: true });
                }
                if ui.button("No").clicked() {
                    choice = Some(ZipChoice::KeepExisting);
                }
            });
            return;
        }
        ui.horizontal(|ui| {
            let map = if preview.version == GameVersion::FS19 {
                &textures.whole
            } else {
                &textures.centre
            };
            ui.add(egui::Image::new((map.id(), side)));
            match &textures.heightmap {
                Some(h) => {
                    ui.add(egui::Image::new((h.id(), side)));
                }
                None => {
                    ui.allocate_ui(side, |ui| ui.label("No heightmap"));
                }
            }
        });
        if let Some(notice) = &read.zip.notice {
            ui.colored_label(ui.visuals().warn_fg_color, notice);
        }
        ui.horizontal(|ui| {
            ui.label("Game");
            for (version, label) in [
                (GameVersion::FS19, "FS19"),
                (GameVersion::FS22, "FS22"),
                (GameVersion::FS25, "FS25"),
            ] {
                ui.selectable_value(&mut preview.version, version, label);
            }
            if preview.version != read.zip.version {
                ui.label(format!("(detected {:?})", read.zip.version));
            }
        });
        ui.horizontal(|ui| {
            ui.label("Map name");
            ui.text_edit_singleline(&mut preview.name);
        });
        let folder = ad_io::mod_zip::folder_name(&preview.name);
        match &folder {
            Ok(folder) => ui.label(format!("Saved as mapImages/{folder}/{folder}.png")),
            Err(note) => ui.colored_label(ui.visuals().error_fg_color, *note),
        };
        ui.horizontal(|ui| {
            if ui
                .add_enabled(folder.is_ok(), egui::Button::new("Import"))
                .clicked()
            {
                choice = Some(ZipChoice::Import { confirmed: false });
            }
            if ui.button("Cancel").clicked() {
                choice = Some(ZipChoice::Cancel);
            }
        });
    });
    if modal.should_close() && choice.is_none() {
        choice = Some(if preview.replace.is_some() {
            ZipChoice::KeepExisting
        } else {
            ZipChoice::Cancel
        });
    }
    match choice {
        Some(ZipChoice::Import { confirmed }) => match crate::jobs::exe_dir() {
            Some(dir) => {
                let ctx = ctx.clone();
                app.zip_import(&dir, confirmed, move || ctx.request_repaint());
            }
            None => {
                app.error = Some("Cannot import: the program folder is unknown".to_owned());
            }
        },
        Some(ZipChoice::KeepExisting) => preview.replace = None,
        Some(ZipChoice::Cancel) => app.zip_preview = None,
        None => {}
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
    use crate::tools::connect::LineOptions;

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

    #[test]
    fn the_align_guide_follows_the_hover_of_the_same_frame() {
        let ctx = egui::Context::default();
        let mut app = app("<version>3</version>");
        let doc = app.doc.as_mut().expect("doc");
        let id = doc.config.network.order()[0];
        let reference = doc
            .config
            .network
            .add_node(30.0, 0.0, -20.0, NodeFlag::REGULAR);
        doc.selection.insert(id);
        let geometry = doc.map.geometry;
        app.tool = Some(Box::new(AlignTool::new(Axis::Horizontal)));
        let frame = |app: &mut App, pos: egui::Pos2| {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1024.0, 1024.0),
                )),
                time: Some(ctx.input(|i| i.time) + 0.016),
                events: vec![egui::Event::PointerMoved(pos)],
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| map_panel(app, ui));
            out.textures_delta.clear();
            (out.shapes, app.view.hovered_node)
        };
        let off = egui::pos2(5.0, 5.0);
        frame(&mut app, off);
        let t = app.view.transform_mut().expect("shown");
        t.zoom = 10.0;
        let (mx, my) = geometry.to_map(30.0, -20.0);
        let (sx, sy) = t.map_to_screen(mx, my);
        let at = egui::pos2(sx as f32, sy as f32);
        // Dashes of the guide: 3 px segments on the reference's screen row.
        let guide = |shapes: &[egui::epaint::ClippedShape]| {
            shapes
                .iter()
                .filter(|s| match &s.shape {
                    egui::Shape::LineSegment { points, stroke } => {
                        stroke.width == 3.0 && points.iter().all(|p| (p.y - at.y).abs() < 1e-3)
                    }
                    _ => false,
                })
                .count()
        };
        // The pointer is off the reference, then comes to rest on it in a single frame.
        let (shapes, hovered) = frame(&mut app, off);
        assert_eq!((guide(&shapes), hovered), (0, None));
        let (shapes, hovered) = frame(&mut app, at);
        assert_eq!(hovered, Some(reference));
        assert!(guide(&shapes) > 0, "drawn the frame the pointer arrives");
        let (shapes, hovered) = frame(&mut app, off);
        assert_eq!(
            (guide(&shapes), hovered),
            (0, None),
            "gone the frame it leaves"
        );
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

    /// One frame of the Edit location dialog with `keys` pressed.
    fn dialog_frame(ctx: &egui::Context, app: &mut App, keys: &[egui::Key]) {
        let events = keys
            .iter()
            .map(|&key| egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            })
            .collect();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1024.0, 1024.0),
            )),
            time: Some(ctx.input(|i| i.time) + 0.016),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| location_dialog(app, ui.ctx()));
        out.textures_delta.clear();
    }

    /// The node's position and whether there is anything to undo.
    fn node_state(app: &App) -> ((f64, f64, f64), bool) {
        let doc = app.doc.as_ref().expect("doc");
        let net = &doc.config.network;
        let n = net.node(net.order()[0]).expect("node");
        ((n.x, n.y, n.z), doc.history.can_undo())
    }

    #[test]
    fn enter_commits_a_valid_dialog_and_escape_cancels_it() {
        let ctx = egui::Context::default();
        let mut app = app("<version>3</version>");
        let id = app.doc.as_ref().expect("doc").config.network.order()[0];
        let open = |app: &mut App, x: &str| {
            let net = &app.doc.as_ref().expect("doc").config.network;
            let mut dialog = edit_location::LocationDialog::open(net, id).expect("node");
            dialog.text[0] = x.to_owned();
            app.location = Some(dialog);
        };
        // Enter with a field refused keeps the dialog and commits nothing.
        open(&mut app, "abc");
        dialog_frame(&ctx, &mut app, &[]);
        dialog_frame(&ctx, &mut app, &[egui::Key::Enter]);
        assert!(app.location.is_some());
        assert_eq!(node_state(&app), ((1.0, 0.0, 2.0), false));
        // Escape closes it and commits nothing.
        open(&mut app, "8");
        dialog_frame(&ctx, &mut app, &[egui::Key::Escape]);
        assert!(app.location.is_none());
        assert_eq!(node_state(&app), ((1.0, 0.0, 2.0), false));
        // Enter with every field valid commits.
        open(&mut app, "8,25");
        dialog_frame(&ctx, &mut app, &[]);
        dialog_frame(&ctx, &mut app, &[egui::Key::Enter]);
        assert!(app.location.is_none());
        assert_eq!(node_state(&app), ((8.25, 0.0, 2.0), true));
    }

    /// One frame of the marker dialog with `events`.
    fn marker_frame(ctx: &egui::Context, app: &mut App, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1024.0, 1024.0),
            )),
            time: Some(ctx.input(|i| i.time) + 0.016),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| marker_dialog(app, ui.ctx()));
        out.textures_delta.clear();
    }

    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[test]
    fn the_marker_dialog_commits_on_enter_and_escape_or_a_click_beside_cancels() {
        let ctx = egui::Context::default();
        let mut app = app("<version>3</version>");
        let id = app.doc.as_ref().expect("doc").config.network.order()[0];
        let open = |app: &mut App, name: &str| {
            let net = &app.doc.as_ref().expect("doc").config.network;
            let mut dialog = crate::tools::marker::MarkerDialog::open(net, id).expect("node");
            dialog.name = name.to_owned();
            app.marker = Some(dialog);
        };
        let markers = |app: &App| {
            app.doc
                .as_ref()
                .expect("doc")
                .config
                .network
                .markers()
                .len()
        };
        // Enter with the name refused keeps the dialog and commits nothing.
        open(&mut app, "  ");
        marker_frame(&ctx, &mut app, vec![]);
        marker_frame(&ctx, &mut app, vec![key(egui::Key::Enter)]);
        assert!(app.marker.is_some());
        assert_eq!(markers(&app), 0);
        // Escape closes it and commits nothing.
        open(&mut app, "Hof");
        marker_frame(&ctx, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.marker.is_none());
        assert_eq!(markers(&app), 0);
        // A click beside it closes it and commits nothing.
        open(&mut app, "Hof");
        let beside = egui::pos2(5.0, 5.0);
        let button = |pressed| egui::Event::PointerButton {
            pos: beside,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        marker_frame(&ctx, &mut app, vec![egui::Event::PointerMoved(beside)]);
        marker_frame(&ctx, &mut app, vec![button(true)]);
        marker_frame(&ctx, &mut app, vec![button(false)]);
        assert!(app.marker.is_none());
        assert_eq!(markers(&app), 0);
        // Enter with the name valid commits.
        open(&mut app, "Hof");
        marker_frame(&ctx, &mut app, vec![]);
        marker_frame(&ctx, &mut app, vec![key(egui::Key::Enter)]);
        assert!(app.marker.is_none());
        let net = &app.doc.as_ref().expect("doc").config.network;
        assert_eq!(net.markers()[0].name, "Hof");
        assert_eq!(net.markers()[0].group, "All");
    }

    /// One frame of the overlap scan dialog with `events`; returns the texts it painted.
    fn overlap_frame(ctx: &egui::Context, app: &mut App, events: Vec<egui::Event>) -> String {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1024.0, 1024.0),
            )),
            time: Some(ctx.input(|i| i.time) + 0.016),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| overlap_dialog(app, ui.ctx()));
        out.textures_delta.clear();
        let mut texts = Vec::new();
        for shape in &out.shapes {
            if let egui::Shape::Text(t) = &shape.shape {
                texts.push(t.galley.text().to_owned());
            }
        }
        texts.join("|")
    }

    #[test]
    fn the_overlap_dialog_refuses_an_invalid_distance_and_scans_a_valid_one() {
        let ctx = egui::Context::default();
        let mut app = app("<version>3</version>");
        for (text, note) in [
            ("abc", "* Not a number"),
            ("0", "* Must be between 0.001 and 100"),
            ("1000", "* Must be between 0.001 and 100"),
        ] {
            app.overlap_dialog = Some(text.to_owned());
            overlap_frame(&ctx, &mut app, vec![]);
            assert!(
                overlap_frame(&ctx, &mut app, vec![]).contains(note),
                "{text}"
            );
            // Enter with the distance refused keeps the dialog and scans nothing.
            overlap_frame(&ctx, &mut app, vec![key(egui::Key::Enter)]);
            assert!(app.overlap_dialog.is_some(), "{text}");
            assert!(app.doc.as_ref().expect("doc").overlap.is_none(), "{text}");
        }
        // Escape closes it and scans nothing.
        app.overlap_dialog = Some("0.1".to_owned());
        overlap_frame(&ctx, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.overlap_dialog.is_none());
        assert!(app.doc.as_ref().expect("doc").overlap.is_none());
        // A valid distance, with a comma, shows no note; Enter scans with it.
        app.overlap_dialog = Some("0,5".to_owned());
        overlap_frame(&ctx, &mut app, vec![]);
        let texts = overlap_frame(&ctx, &mut app, vec![]);
        assert!(
            texts.contains("Distance") && !texts.contains('*'),
            "{texts}"
        );
        overlap_frame(&ctx, &mut app, vec![key(egui::Key::Enter)]);
        assert!(app.overlap_dialog.is_none());
        let doc = app.doc.as_ref().expect("doc");
        assert_eq!(doc.overlap.as_ref().map(|(d, _)| *d), Some(0.5));
        assert_eq!(doc.notice.as_deref(), Some("No overlapping nodes"));
    }

    /// One frame of the groups window with `events`; returns the texts it painted.
    fn groups_frame(ctx: &egui::Context, app: &mut App, events: Vec<egui::Event>) -> String {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1024.0, 1024.0),
            )),
            time: Some(ctx.input(|i| i.time) + 0.016),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| groups_window(app, ui.ctx()));
        out.textures_delta.clear();
        let mut texts = Vec::new();
        for shape in &out.shapes {
            if let egui::Shape::Text(t) = &shape.shape {
                texts.push(t.galley.text().to_owned());
            }
        }
        texts.join("|")
    }

    #[test]
    fn the_top_bar_wraps_so_every_button_stays_inside_a_narrow_window() {
        let ctx = egui::Context::default();
        let mut app = app("<version>3</version>");
        let width = 800.0;
        let mut texts = Vec::new();
        for _ in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 600.0),
                )),
                time: Some(ctx.input(|i| i.time) + 0.016),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| top_bar(&mut app, ui, &ctx.clone()));
            out.textures_delta.clear();
            texts = out
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Text(t) => Some((t.galley.text().to_owned(), t.pos.x)),
                    _ => None,
                })
                .collect();
        }
        for name in ["Open…", "Node", "Delete marker", "Groups"] {
            let (_, x) = texts
                .iter()
                .find(|(t, _)| t == name)
                .unwrap_or_else(|| panic!("{name} drawn: {texts:?}"));
            assert!(*x < width, "{name} at x = {x}");
        }
    }

    #[test]
    fn the_groups_window_lists_counts_commits_on_enter_and_closes_on_escape() {
        let ctx = egui::Context::default();
        let mut app = app("<version>3</version>");
        let id = app.doc.as_ref().expect("doc").config.network.order()[0];
        let net = &mut app.doc.as_mut().expect("doc").config.network;
        net.push_marker(ad_core::network::Marker {
            node: id,
            name: "Hof".to_owned(),
            group: "b".to_owned(),
            parked_vehicles: Vec::new(),
        });
        app.open_groups();
        // The first frame only sizes the window.
        groups_frame(&ctx, &mut app, vec![]);
        let texts = groups_frame(&ctx, &mut app, vec![]);
        assert!(texts.contains("All (0)|"), "{texts}");
        assert!(texts.contains("b (1)|Rename|Delete"), "{texts}");
        // A refused name shows the note, and Enter keeps the field.
        let window = app.groups.as_mut().expect("window");
        window.start_rename("b");
        window.rename.as_mut().expect("field").1 = "None".to_owned();
        let texts = groups_frame(&ctx, &mut app, vec![key(egui::Key::Enter)]);
        assert!(texts.contains("cannot name a group"), "{texts}");
        assert!(app.groups.as_ref().expect("window").rename.is_some());
        // A valid one commits on Enter; the window stays open.
        app.groups.as_mut().expect("window").rename = Some(("b".into(), "c".into()));
        groups_frame(&ctx, &mut app, vec![]);
        groups_frame(&ctx, &mut app, vec![key(egui::Key::Enter)]);
        let net = &app.doc.as_ref().expect("doc").config.network;
        assert_eq!(net.markers()[0].group, "c");
        assert_eq!(app.groups, Some(Default::default()));
        let texts = groups_frame(&ctx, &mut app, vec![]);
        assert!(texts.contains("c (1)"), "{texts}");
        groups_frame(&ctx, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.groups.is_none());
    }

    #[test]
    fn a_click_with_edit_location_opens_the_dialog_and_the_map_then_ignores_selection() {
        let ctx = egui::Context::default();
        let mut app = app("<version>3</version>");
        let id = app.doc.as_ref().expect("doc").config.network.order()[0];
        app.tool = Some(Box::new(EditLocationTool::default()));
        let primary = egui::PointerButton::Primary;
        // Zoom 2 so the node is large enough to hit: world (1, 2) is at screen (514, 516).
        frames(&ctx, &mut app, primary, &[&[]]);
        app.view.transform_mut().expect("set").zoom = 2.0;
        let node = egui::pos2(514.0, 516.0);
        frames(
            &ctx,
            &mut app,
            primary,
            &[
                &[(node, None)],
                &[(node, Some(true))],
                &[(node, Some(false))],
            ],
        );
        assert_eq!(app.location.as_ref().map(|d| d.node), Some(id));
        let doc = app.doc.as_ref().expect("doc");
        assert!(doc.selection.is_empty(), "the selection is unaffected");
        assert!(!doc.history.can_undo());
        // A right click on the node would select it, but not while the dialog is shown.
        frames(
            &ctx,
            &mut app,
            egui::PointerButton::Secondary,
            &[&[(node, Some(true))], &[(node, Some(false))], &[]],
        );
        assert!(app.doc.as_ref().expect("doc").selection.is_empty());
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
            Some(Box::new(ConnectTool::new(
                ConnectionShape::OneWay,
                false,
                LineOptions::from_settings(&Default::default()),
            ))),
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
