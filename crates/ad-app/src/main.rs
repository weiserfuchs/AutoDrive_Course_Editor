//! Shell layer: the eframe application.

mod clipboard;
mod document;
mod hover;
mod icons;
mod input;
mod jobs;
mod lod;
mod look;
mod overlay;
mod startup;
mod tools;
mod ui;
mod view;
mod zip_import;

use std::sync::mpsc::{self, Receiver, Sender};

use eframe::egui;

use ad_core::command::{Command, DeleteNodes, Transaction};
use ad_core::geometry::MAP_PIXELS;
use ad_io::settings::Settings;
use document::Document;
use input::Action;
use jobs::JobResult;
use tools::Tool;
use view::MapView;

/// What waits for the document to be saved or discarded (R7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pending {
    Close,
    Open,
}

/// The user's answer to the unsaved-changes prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    Save,
    Discard,
    Cancel,
}

/// The window title: the open file's path, and ` *` while the document is dirty.
fn window_title(doc: Option<&Document>) -> String {
    match doc {
        None => "AutoDrive Course Editor".to_owned(),
        Some(doc) => format!(
            "AutoDrive Course Editor - {}{}",
            doc.path.display(),
            if doc.is_dirty() { " *" } else { "" }
        ),
    }
}

struct App {
    doc: Option<Document>,
    /// Epoch of the newest document; each new document takes the next value (AD-18).
    epoch_counter: u64,
    tx: Sender<JobResult>,
    rx: Receiver<JobResult>,
    loading: bool,
    saving: bool,
    /// A DDS or map mod zip import job is running.
    importing: bool,
    /// The map mod zip preview window, while it is shown.
    zip_preview: Option<zip_import::ZipPreview>,
    error: Option<String>,
    settings: Settings,
    view: MapView,
    /// Epoch of the document the view was last reset for.
    view_epoch: Option<u64>,
    /// The active tool, if any (AD-10).
    tool: Option<Box<dyn Tool>>,
    /// Grid snap for the tools; session only. Shift inverts it while held.
    snap: bool,
    /// Continuous connections for the connect tools; session only (Java default off).
    continuous: bool,
    /// The right drag draws a lasso instead of a rectangle; session only (Java default off).
    lasso: bool,
    /// The action that continues once the save started from the prompt has finished.
    pending: Option<Pending>,
    /// The action whose unsaved-changes prompt is shown.
    prompt: Option<Pending>,
    /// The Edit location dialog, while it is shown.
    location: Option<tools::edit_location::LocationDialog>,
    /// The marker dialog, while it is shown.
    marker: Option<tools::marker::MarkerDialog>,
    /// The groups window, while it is shown.
    groups: Option<tools::marker::GroupsWindow>,
    /// A tool's info for the toolbar's notice line, shown until the next click on the map.
    tool_notice: Option<&'static str>,
    /// Set once closing was agreed; the next close request is not intercepted.
    allow_close: bool,
    /// The title last sent to the window.
    title: String,
    /// The copied group; app-level, so it survives opening another config (AD-15).
    clipboard: Option<clipboard::Clipboard>,
    /// A note for the OS clipboard after a copy or cut, put there by the shortcut handler.
    copied: Option<String>,
}

impl App {
    fn new(settings: Settings) -> Self {
        let mut view = MapView::default();
        view.show_grid = settings.global.show_grid;
        let (tx, rx) = mpsc::channel();
        Self {
            doc: None,
            epoch_counter: 0,
            tx,
            rx,
            loading: false,
            saving: false,
            importing: false,
            zip_preview: None,
            error: None,
            settings,
            view,
            view_epoch: None,
            tool: None,
            snap: false,
            continuous: false,
            lasso: false,
            pending: None,
            prompt: None,
            location: None,
            marker: None,
            groups: None,
            tool_notice: None,
            allow_close: false,
            title: window_title(None),
            clipboard: None,
            copied: None,
        }
    }

    fn dirty(&self) -> bool {
        self.doc.as_ref().is_some_and(Document::is_dirty)
    }

    /// Runs `action` at once, or first asks Save, Discard or Cancel while the document is dirty.
    fn request(&mut self, action: Pending, ctx: &egui::Context) {
        if self.dirty() {
            self.prompt = Some(action);
        } else {
            self.proceed(action, ctx);
        }
    }

    /// Applies the user's answer to the prompt shown.
    fn choose(&mut self, choice: Choice, ctx: &egui::Context) {
        let Some(action) = self.prompt.take() else {
            return;
        };
        match choice {
            // The action continues only after this save has succeeded (see `resume`).
            Choice::Save => {
                if self.save(ctx) {
                    self.pending = Some(action);
                }
            }
            Choice::Discard => self.proceed(action, ctx),
            // Also drops an action still waiting for an earlier save.
            Choice::Cancel => self.pending = None,
        }
    }

    /// Continues the pending action once its save has finished. A failed save cleared it in
    /// `poll_jobs`; a save that left the document dirty (edits made while saving) asks again.
    fn resume(&mut self, ctx: &egui::Context) {
        if self.saving {
            return;
        }
        let Some(action) = self.pending.take() else {
            return;
        };
        if self.dirty() {
            self.prompt = Some(action);
        } else {
            self.proceed(action, ctx);
        }
    }

    /// Cancels a window close while the document is dirty and asks first.
    fn intercept_close(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.dirty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.prompt = Some(Pending::Close);
        }
    }

    fn proceed(&mut self, action: Pending, ctx: &egui::Context) {
        match action {
            Pending::Close => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Pending::Open => self.open(ctx),
        }
    }

    /// Applies finished jobs. Called once per frame on the UI thread.
    fn poll_jobs(&mut self) {
        while let Ok(result) = self.rx.try_recv() {
            match &result {
                JobResult::Loaded { .. } => self.loading = false,
                JobResult::Saved {
                    result, warning, ..
                } => {
                    self.saving = false;
                    // A failed save keeps the document and the window; so does a failed
                    // vehicles.xml save, so its message is seen.
                    if result.is_err() || warning.is_some() {
                        self.pending = None;
                    }
                }
                JobResult::Imported { .. } | JobResult::ZipSaved { .. } => self.importing = false,
                JobResult::ImageSaved { .. } => {}
                JobResult::ZipRead { .. } => {
                    self.importing = false;
                    if let JobResult::ZipRead { epoch, result } = result {
                        self.zip_read(epoch, result);
                    }
                    continue;
                }
            }
            let before = self.doc.as_ref().map(|d| d.epoch);
            if let Some(message) = jobs::apply_result(
                &mut self.doc,
                &mut self.epoch_counter,
                &self.settings,
                result,
            ) {
                self.error = Some(message);
            }
            // A new document cancels the active tool (AD-18) and the dialogs of an old node.
            if self.doc.as_ref().map(|d| d.epoch) != before {
                self.set_tool(None);
                self.location = None;
                self.marker = None;
                self.groups = None;
                self.tool_notice = None;
                self.zip_preview = None;
            }
        }
    }

    /// Shows the preview of a read zip, unless the document changed meanwhile (AD-18).
    fn zip_read(&mut self, epoch: Option<u64>, result: Result<Box<zip_import::ZipRead>, String>) {
        if self.doc.as_ref().map(|d| d.epoch) != epoch {
            return;
        }
        match result {
            Ok(read) => {
                let map_name = self.doc.as_ref().and_then(|d| d.config.map_name());
                self.zip_preview = Some(zip_import::ZipPreview::new(*read, map_name));
            }
            Err(message) => self.error = Some(message),
        }
    }

    /// Makes `tool` the active tool; the previous one is cancelled first (AD-10).
    fn set_tool(&mut self, tool: Option<Box<dyn Tool>>) {
        if let Some(old) = &mut self.tool {
            old.cancel();
        }
        self.tool = tool;
    }

    /// The line options the connect tools start with.
    fn line(&self) -> tools::connect::LineOptions {
        tools::connect::LineOptions::from_settings(&self.settings.global)
    }

    /// Whether the open document may be edited: tools, Undo and Redo need this.
    fn editable(&self) -> bool {
        self.doc.as_ref().is_some_and(document::Document::editable)
    }

    /// Whether a modal dialog (Edit location, marker, groups, the map image replace
    /// question or the zip preview) is shown.
    fn dialog_shown(&self) -> bool {
        self.location.is_some()
            || self.marker.is_some()
            || self.groups.is_some()
            || self.zip_preview.is_some()
            || self
                .doc
                .as_ref()
                .is_some_and(|d| d.image_to_replace.is_some())
    }

    /// Runs a toolbar or shortcut action.
    fn run(&mut self, action: Action) {
        match action {
            Action::ToggleNodeId => self.view.show_node_id = !self.view.show_node_id,
            Action::ToggleSnap => self.snap = !self.snap,
            Action::ToggleContinuous => {
                self.continuous = !self.continuous;
                if let Some(tool) = &mut self.tool {
                    tool.set_continuous(self.continuous);
                }
            }
            Action::ConnectTool if self.editable() => {
                let next =
                    tools::connect::cycled(self.tool.as_deref(), self.continuous, self.line());
                self.set_tool(Some(Box::new(next)));
            }
            Action::NodeTool if self.editable() => {
                let next = tools::node::cycled(self.tool.as_deref());
                self.set_tool(Some(Box::new(next)));
            }
            Action::MoveTool if self.editable() => {
                self.set_tool(Some(Box::new(tools::move_node::MoveTool::default())));
            }
            Action::RotateTool if self.editable() => {
                self.set_tool(Some(Box::new(tools::rotate::RotateTool::default())));
            }
            Action::AlignHorizontalTool if self.editable() => {
                let tool = tools::align::AlignTool::new(tools::align::Axis::Horizontal);
                self.set_tool(Some(Box::new(tool)));
            }
            Action::AlignVerticalTool if self.editable() => {
                let tool = tools::align::AlignTool::new(tools::align::Axis::Vertical);
                self.set_tool(Some(Box::new(tool)));
            }
            Action::PriorityTool if self.editable() => {
                self.set_tool(Some(Box::new(tools::priority::PriorityTool)));
            }
            Action::FlipTool if self.editable() => {
                self.set_tool(Some(Box::new(tools::flip::FlipTool)));
            }
            // A new Bézier tool, so a previewed curve is dropped (AD-10).
            Action::BezierTool if self.editable() => {
                self.set_tool(Some(Box::new(tools::bezier::BezierTool::default())));
            }
            Action::Undo if self.editable() => {
                // A preview built on the old network must not commit afterwards.
                if let Some(tool) = &mut self.tool {
                    tool.cancel();
                }
                if let Some(doc) = &mut self.doc {
                    doc.undo();
                }
                // The hover belongs to the old network; the next frame recomputes it, so a
                // Delete in the same frame cannot hit a node the undo moved away.
                self.view.clear_hover();
            }
            Action::Redo if self.editable() => {
                if let Some(tool) = &mut self.tool {
                    tool.cancel();
                }
                if let Some(doc) = &mut self.doc {
                    doc.redo();
                }
                self.view.clear_hover();
            }
            Action::DeleteHovered if self.editable() => self.delete_hovered(),
            Action::AddMarker if self.editable() => {
                self.marker = self
                    .marker_target()
                    .and_then(|(net, id)| tools::marker::MarkerDialog::open(net, id));
            }
            Action::EditMarker if self.editable() => {
                self.marker = self
                    .marker_target()
                    .and_then(|(net, id)| tools::marker::MarkerDialog::edit(net, id));
            }
            Action::DeleteMarker if self.editable() => self.delete_marker_hovered(),
            // Copy changes no data, so it works on a read-only document too.
            Action::Copy => {
                self.copy();
            }
            Action::Cut if self.editable() => self.cut(),
            Action::Paste if self.editable() => self.paste(false),
            Action::PasteInPlace if self.editable() => self.paste(true),
            Action::Cut
            | Action::Paste
            | Action::PasteInPlace
            | Action::NodeTool
            | Action::MoveTool
            | Action::RotateTool
            | Action::AlignHorizontalTool
            | Action::AlignVerticalTool
            | Action::ConnectTool
            | Action::PriorityTool
            | Action::FlipTool
            | Action::BezierTool
            | Action::Undo
            | Action::Redo
            | Action::DeleteHovered
            | Action::AddMarker
            | Action::EditMarker
            | Action::DeleteMarker => {}
        }
    }

    /// The network and the hovered node the marker keys (W, E, Shift+D) act on. None
    /// during a drag a tool claimed or a selection being drawn, which a modal would cut.
    fn marker_target(&self) -> Option<(&ad_core::network::Network, ad_core::network::NodeId)> {
        let doc = self.doc.as_ref()?;
        if self.view.selecting() || self.view.dragging() {
            return None;
        }
        // The hover belongs to the document the view last showed.
        let id = self
            .view
            .hovered_node
            .filter(|_| self.view_epoch == Some(doc.epoch))?;
        Some((&doc.config.network, id))
    }

    /// Removes the hovered node's marker in one transaction, if it has one (Shift+D).
    fn delete_marker_hovered(&mut self) {
        let Some(tx) = self
            .marker_target()
            .and_then(|(net, id)| tools::marker::delete_plan(net, id))
        else {
            return;
        };
        if let Some(doc) = &mut self.doc
            && let Err(e) = doc.apply(tx)
        {
            self.error = Some(e.to_string());
        }
    }

    /// Deletes the selection when it is not empty, else the hovered node, if any, after
    /// cancelling the tool's preview (as Undo does). Nothing while a selection is drawn.
    fn delete_hovered(&mut self) {
        let Some(doc) = &mut self.doc else {
            return;
        };
        if self.view.selecting() {
            return;
        }
        let ids = if doc.selection.is_empty() {
            // The hover belongs to the document the view last showed, and must still exist.
            let Some(id) = self
                .view
                .hovered_node
                .filter(|_| self.view_epoch == Some(doc.epoch))
                .filter(|&id| doc.config.network.node(id).is_some())
            else {
                return;
            };
            vec![id]
        } else {
            doc.selection.iter().copied().collect()
        };
        if let Some(tool) = &mut self.tool {
            tool.cancel();
        }
        let tx = Transaction::new(vec![Command::DeleteNodes(DeleteNodes::new(ids))]);
        if let Err(e) = doc.apply(tx) {
            self.error = Some(e.to_string());
        }
    }

    /// Copies the selection to the clipboard; the selection stays. Returns false, keeping
    /// the clipboard, when nothing is selected or a selection is being drawn.
    fn copy(&mut self) -> bool {
        let Some(doc) = &self.doc else {
            return false;
        };
        if self.view.selecting() {
            return false;
        }
        let Some(clip) = clipboard::copy(&doc.config.network, &doc.selection) else {
            return false;
        };
        self.copied = Some(format!(
            "AutoDrive Course Editor: {} nodes copied",
            clip.len()
        ));
        self.clipboard = Some(clip);
        true
    }

    /// Copies the selection, then deletes it in one transaction; undo reselects it.
    fn cut(&mut self) {
        if !self.copy() {
            return;
        }
        let Some(doc) = &mut self.doc else {
            return;
        };
        if let Some(tool) = &mut self.tool {
            tool.cancel();
        }
        let ids = doc.selection.iter().copied().collect();
        let tx = Transaction::new(vec![Command::DeleteNodes(DeleteNodes::new(ids))]);
        if let Err(e) = doc.apply_selecting(tx, |_| Default::default()) {
            self.error = Some(e.to_string());
        }
    }

    /// Pastes the clipboard in one transaction, centred on the view or, `in_place`, where
    /// it was copied; the pasted nodes become the selection. Nothing while a selection is
    /// drawn or the clipboard is empty.
    fn paste(&mut self, in_place: bool) {
        let (Some(doc), Some(clip)) = (&mut self.doc, &self.clipboard) else {
            return;
        };
        if self.view.selecting() {
            return;
        }
        let centre = (!in_place).then(|| {
            // The view may still show the previous document until the next frame.
            let (px, py) = if self.view_epoch == Some(doc.epoch) {
                self.view.centre()
            } else {
                (MAP_PIXELS / 2.0, MAP_PIXELS / 2.0)
            };
            doc.map.geometry.to_world(px, py)
        });
        if let Some(tool) = &mut self.tool {
            tool.cancel();
        }
        let tx = clipboard::paste(clip, centre, &doc.config.network, &doc.map);
        if let Err(e) = doc.apply_selecting(tx, clipboard::pasted) {
            self.error = Some(e.to_string());
        }
    }

    /// Hands a primary click at map pixel `(px, py)` to the active tool and commits the
    /// transaction it returns, or opens the dialog it asks for. A read-only document gets
    /// no click, nor does the map while a dialog is shown. A click ends the tool notice.
    fn click(&mut self, px: f64, py: f64, snap: bool) {
        if !self.editable() || self.dialog_shown() {
            return;
        }
        self.tool_notice = None;
        let (Some(doc), Some(tool)) = (&mut self.doc, &mut self.tool) else {
            return;
        };
        let (x, z) = doc.map.geometry.to_world(px, py);
        let tx = tool.on_click(&tools::ToolContext {
            x,
            z,
            map: &doc.map,
            net: &doc.config.network,
            selection: &doc.selection,
            snap,
            hovered_node: self.view.hovered_node,
            hovered_edge: self.view.hovered_edge,
        });
        let net = &doc.config.network;
        match tool.take_request() {
            Some(tools::ToolRequest::EditLocation(node)) => {
                self.location = tools::edit_location::LocationDialog::open(net, node);
            }
            Some(tools::ToolRequest::AddMarker(node)) if tools::marker::has_marker(net, node) => {
                self.tool_notice = Some(tools::marker::HAS_MARKER);
            }
            Some(tools::ToolRequest::AddMarker(node)) => {
                self.marker = tools::marker::MarkerDialog::open(net, node);
            }
            Some(tools::ToolRequest::EditMarker(node)) => {
                self.marker = tools::marker::MarkerDialog::edit(net, node);
                if self.marker.is_none() {
                    self.tool_notice = Some(tools::marker::NO_MARKER);
                }
            }
            Some(tools::ToolRequest::NoMarker(_)) => {
                self.tool_notice = Some(tools::marker::NO_MARKER);
            }
            None => {}
        }
        let Some(tx) = tx else {
            return;
        };
        match doc.apply(tx) {
            Ok(id) => {
                if let Some(tx) = doc.history.transaction(id) {
                    tool.committed(tx);
                }
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// Ends the previewed curve or arc: Commit applies it in one transaction (not on a
    /// read-only document), Cancel drops it. Either way the tool then waits for a new one.
    fn finish_curve(&mut self, choice: ui::CurveChoice) {
        let editable = self.editable();
        let Some(tool) = &mut self.tool else {
            return;
        };
        let tx = match (choice, &self.doc) {
            (ui::CurveChoice::Commit, Some(doc)) if editable => match tool.curve_mut() {
                Some(curve) => tools::bezier::plan(&doc.config.network, &doc.map, curve),
                None => tool.arc_mut().map(|arc| tools::arc::plan(&doc.map, arc)),
            },
            _ => None,
        };
        tool.cancel();
        if let (Some(tx), Some(doc)) = (tx, &mut self.doc)
            && let Err(e) = doc.apply(tx)
        {
            self.error = Some(e.to_string());
        }
    }

    /// Closes the Edit location dialog. With `ok`, sets its node to the typed position in
    /// one transaction, unless a field is invalid, nothing changed, the node is gone or
    /// the document is read-only.
    fn finish_location(&mut self, ok: bool) {
        let Some(dialog) = self.location.take() else {
            return;
        };
        if !ok || !self.editable() {
            return;
        }
        let Some(doc) = &mut self.doc else {
            return;
        };
        let [Ok(x), Ok(y), Ok(z)] = dialog.values(doc.map.geometry.scale) else {
            return;
        };
        let Some(tx) = tools::edit_location::plan(&doc.config.network, dialog.node, (x, y, z))
        else {
            return;
        };
        if let Err(e) = doc.apply(tx) {
            self.error = Some(e.to_string());
        }
    }

    /// Closes the marker dialog. With `ok`, sets its node's marker in one transaction,
    /// unless the name is refused, an edit changed nothing or the document is read-only.
    fn finish_marker(&mut self, ok: bool) {
        let Some(dialog) = self.marker.take() else {
            return;
        };
        if !ok || !self.editable() {
            return;
        }
        let Some(doc) = &mut self.doc else {
            return;
        };
        let Some(tx) = dialog.plan(&doc.config.network) else {
            return;
        };
        if let Err(e) = doc.apply(tx) {
            self.error = Some(e.to_string());
        }
    }

    /// Shows the groups window; not for a read-only document.
    fn open_groups(&mut self) {
        if self.editable() {
            self.groups = Some(tools::marker::GroupsWindow::default());
        }
    }

    /// Commits the groups window's open rename in one transaction and closes its text
    /// field; nothing while the name is refused or the document is read-only.
    fn finish_group_rename(&mut self) {
        let (Some(doc), Some(window)) = (&self.doc, &self.groups) else {
            return;
        };
        let Some(tx) = window
            .rename_plan(&doc.config.network)
            .filter(|_| self.editable())
        else {
            return;
        };
        self.apply_group_change(tx);
        if let Some(window) = &mut self.groups {
            window.rename = None;
        }
    }

    /// Deletes `group` from the groups window in one transaction: its markers stay, in
    /// "All". Nothing for "All" or on a read-only document.
    fn delete_group(&mut self, group: &str) {
        let Some(tx) = self
            .doc
            .as_ref()
            .filter(|_| self.editable() && self.groups.is_some())
            .and_then(|doc| tools::marker::delete_group_plan(&doc.config.network, group))
        else {
            return;
        };
        self.apply_group_change(tx);
    }

    fn apply_group_change(&mut self, tx: Transaction) {
        if let Some(doc) = &mut self.doc
            && let Err(e) = doc.apply(tx)
        {
            self.error = Some(e.to_string());
        }
    }

    fn open(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("XML", &["xml"])
            .pick_file()
        else {
            return;
        };
        self.loading = true;
        let ctx = ctx.clone();
        jobs::spawn_load(
            path,
            jobs::exe_dir(),
            self.settings.clone(),
            self.tx.clone(),
            move || {
                ctx.request_repaint();
            },
        );
    }

    /// Asks for a DDS and imports it as the open document's map image, cropped for
    /// `version`, the game the menu item names. Read-only documents too: no data changes.
    fn import(&mut self, version: ad_io::config_xml::GameVersion, ctx: &egui::Context) {
        let Some(doc) = &self.doc else {
            return;
        };
        let target = match jobs::import_target(doc, jobs::exe_dir().as_deref()) {
            Ok(target) => target,
            Err(message) => {
                self.error = Some(message);
                return;
            }
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("DDS", &["dds"])
            .pick_file()
        else {
            return;
        };
        let ctx = ctx.clone();
        self.start_import(path, version, target, move || ctx.request_repaint());
    }

    fn start_import(
        &mut self,
        path: std::path::PathBuf,
        version: ad_io::config_xml::GameVersion,
        target: std::path::PathBuf,
        notify: impl FnOnce() + Send + 'static,
    ) {
        let Some(doc) = &self.doc else {
            return;
        };
        self.importing = true;
        jobs::spawn_import(path, version, target, doc.epoch, self.tx.clone(), notify);
    }

    /// The answer to "Replace the existing map image?": Yes writes the shown image over it,
    /// No keeps it shown without saving.
    fn answer_replace(&mut self, replace: bool, notify: impl FnOnce() + Send + 'static) {
        let Some(save) = self.doc.as_mut().and_then(|d| d.image_to_replace.take()) else {
            return;
        };
        if replace {
            jobs::spawn_image_save(save, self.tx.clone(), notify);
        }
    }

    /// Asks for a map mod zip and reads it for the preview; no document is needed.
    fn import_zip(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Zip", &["zip"])
            .pick_file()
        else {
            return;
        };
        let ctx = ctx.clone();
        self.start_zip_read(path, move || ctx.request_repaint());
    }

    fn start_zip_read(&mut self, path: std::path::PathBuf, notify: impl FnOnce() + Send + 'static) {
        self.importing = true;
        let epoch = self.doc.as_ref().map(|d| d.epoch);
        zip_import::spawn_read(path, epoch, self.tx.clone(), notify);
    }

    /// The preview's Import: saves into `dir/mapImages/<name>/`. When files are there and
    /// `confirmed` is false, it asks first and saves nothing.
    fn zip_import(
        &mut self,
        dir: &std::path::Path,
        confirmed: bool,
        notify: impl FnOnce() + Send + 'static,
    ) {
        let Some(preview) = &mut self.zip_preview else {
            return;
        };
        let Ok(folder) = ad_io::mod_zip::folder_name(&preview.name) else {
            return;
        };
        if !confirmed {
            let existing: Vec<_> = preview
                .targets(dir, &folder)
                .into_iter()
                .filter(|p| p.exists())
                .collect();
            if !existing.is_empty() {
                preview.replace = Some(existing);
                return;
            }
        }
        let Some(preview) = self.zip_preview.take() else {
            return;
        };
        self.importing = true;
        let epoch = self.doc.as_ref().map(|d| d.epoch);
        zip_import::spawn_save(
            preview.read.zip,
            preview.version,
            dir.to_path_buf(),
            folder,
            epoch,
            self.tx.clone(),
            notify,
        );
    }

    /// Asks for a target and starts the save. Returns false when no save started.
    fn save(&mut self, ctx: &egui::Context) -> bool {
        let Some(doc) = &self.doc else {
            return false;
        };
        if doc.config.read_only().is_some() {
            return false;
        }
        let mut dialog = rfd::FileDialog::new().add_filter("XML", &["xml"]);
        if let Some(dir) = doc.path.parent() {
            dialog = dialog.set_directory(dir);
        }
        if let Some(name) = doc.path.file_name() {
            dialog = dialog.set_file_name(name.to_string_lossy());
        }
        let Some(target) = dialog.save_file() else {
            return false;
        };
        self.saving = true;
        let ctx = ctx.clone();
        jobs::spawn_save(
            doc.config.clone(),
            jobs::vehicles_to_patch(doc, &target),
            doc.epoch,
            doc.history.save_point(),
            target,
            self.tx.clone(),
            move || ctx.request_repaint(),
        );
        true
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_jobs();
        let ctx = ui.ctx().clone();
        self.resume(&ctx);
        self.intercept_close(&ctx);
        let title = window_title(self.doc.as_ref());
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
        input::handle_shortcuts(self, &ctx);
        ui::top_bar(self, ui, &ctx);
        egui::CentralPanel::default().show(ui, |ui| ui::map_panel(self, ui));
        ui::curve_panel(self, &ctx);
        ui::error_window(self, &ctx);
        // Before the prompt, so a prompt from a window close shows on top of it.
        ui::location_dialog(self, &ctx);
        ui::marker_dialog(self, &ctx);
        ui::groups_window(self, &ctx);
        ui::zip_preview_window(self, &ctx);
        ui::replace_image_prompt(self, &ctx);
        ui::unsaved_prompt(self, &ctx);
    }
}

fn main() -> eframe::Result {
    let startup = startup::init();
    let settings = startup.settings.clone();
    eframe::run_native(
        "AutoDrive Course Editor",
        eframe::NativeOptions::default(),
        Box::new(move |_cc| Ok(Box::new(App::new(settings)))),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use ad_core::command::Connect;
    use ad_core::network::{ConnectionKind, ConnectionShape, NodeFlag};
    use ad_io::{config_xml, map_image};

    use ad_core::network::NodeId;

    use super::*;
    use crate::tools::node::NodeTool;
    use crate::view::{DragEvent, ViewHooks};

    fn text(version: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"no\"?>\n<AutoDrive>\n    \
             {version}\n    <waypoints>\n        <id>1</id>\n        <x>1.000</x>\n        \
             <y>0.000</y>\n        <z>2.000</z>\n        <out>-1</out>\n        \
             <incoming>-1</incoming>\n        <flags>0</flags>\n    </waypoints>\n</AutoDrive>\n"
        )
    }

    /// Sends a load result for `version` through the app's channel and polls it.
    fn load(app: &mut App, version: &str) {
        let config = config_xml::load(text(version).as_bytes()).expect("loads");
        let result = JobResult::Loaded {
            path: PathBuf::from("a.xml"),
            result: Ok(Box::new((config, map_image::blank(), None, None, None))),
        };
        app.tx.send(result).expect("send");
        app.poll_jobs();
    }

    fn nodes(app: &App) -> usize {
        app.doc.as_ref().expect("doc").config.network.len()
    }

    #[test]
    fn a_new_document_cancels_the_active_tool() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        app.run(Action::NodeTool);
        assert!(app.tool.is_some());
        load(&mut app, "<version>3</version>");
        assert!(app.tool.is_none());
    }

    #[test]
    fn a_new_document_starts_with_an_empty_selection() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        let id = first(&app);
        app.doc.as_mut().expect("doc").selection.insert(id);
        load(&mut app, "<version>3</version>");
        assert!(app.doc.as_ref().expect("doc").selection.is_empty());
    }

    #[test]
    fn a_failed_load_keeps_the_tool() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        app.run(Action::NodeTool);
        app.tx
            .send(JobResult::Loaded {
                path: PathBuf::from("b.xml"),
                result: Err("broken".to_owned()),
            })
            .expect("send");
        app.poll_jobs();
        assert!(app.tool.is_some());
    }

    #[test]
    fn click_adds_a_node_and_undo_redo_run_through_actions() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        app.click(1024.0, 1024.0, false);
        assert_eq!(nodes(&app), 1, "no tool, no node");
        app.run(Action::NodeTool);
        app.run(Action::NodeTool);
        assert_eq!(app.tool.as_ref().map(|t| t.name()), Some("Sub-prio node"));
        // Map pixel (1034, 1019) is world (10, -5) at scale 1.
        app.click(1034.0, 1019.0, false);
        assert_eq!(nodes(&app), 2);
        let net = &app.doc.as_ref().expect("doc").config.network;
        let id = *net.order().last().expect("node");
        let n = net.node(id).expect("node");
        assert_eq!(
            (n.x, n.y, n.z, n.flag),
            (10.0, -1.0, -5.0, NodeFlag::SUB_PRIO)
        );
        app.run(Action::Undo);
        assert_eq!(nodes(&app), 1);
        app.run(Action::Redo);
        let net = &app.doc.as_ref().expect("doc").config.network;
        assert_eq!(net.order().last(), Some(&id));
    }

    #[test]
    fn read_only_document_disables_tools_and_clicks() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<markerID>1</markerID>");
        app.run(Action::NodeTool);
        assert!(app.tool.is_none());
        app.run(Action::MoveTool);
        assert!(app.tool.is_none());
        // Even a tool left active adds nothing.
        app.tool = Some(Box::new(NodeTool::new(NodeFlag::REGULAR)));
        app.click(1024.0, 1024.0, false);
        assert_eq!(nodes(&app), 1);
        assert!(app.error.is_none());
    }

    /// Hands `event` to the active tool as the map panel does, and commits what it returns.
    fn drag(app: &mut App, event: DragEvent, snap: bool) {
        let doc = app.doc.as_mut().expect("doc");
        let mut hooks = ui::ToolHooks::new(
            app.tool.as_deref_mut(),
            &doc.map,
            &doc.config.network,
            &doc.selection,
            snap,
        );
        hooks.drag(event);
        if let Some(tx) = hooks.commit.take() {
            doc.apply(tx).expect("applies");
        }
    }

    /// The loaded node's (x, y, z) and whether there is anything to undo.
    fn state(app: &App) -> ((f64, f64, f64), bool) {
        let doc = app.doc.as_ref().expect("doc");
        let net = &doc.config.network;
        let n = net.node(net.order()[0]).expect("node");
        ((n.x, n.y, n.z), doc.history.can_undo())
    }

    fn first(app: &App) -> NodeId {
        app.doc.as_ref().expect("doc").config.network.order()[0]
    }

    #[test]
    fn a_drag_commits_one_history_entry_and_undo_restores_the_start() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        app.run(Action::MoveTool);
        let id = first(&app);
        // The node is world (1, 2): map px (1025, 1026) at scale 1. Grab it 0.5 px off.
        let start = DragEvent::Start {
            at: (1025.5, 1026.0),
            node: Some(id),
        };
        drag(&mut app, start, false);
        drag(
            &mut app,
            DragEvent::Move {
                at: (1040.5, 1030.0),
            },
            false,
        );
        drag(
            &mut app,
            DragEvent::Move {
                at: (1050.5, 1010.0),
            },
            false,
        );
        assert_eq!(
            state(&app),
            ((1.0, 0.0, 2.0), false),
            "only the release commits"
        );
        drag(
            &mut app,
            DragEvent::End {
                at: (1060.5, 1000.0),
            },
            false,
        );
        assert_eq!(state(&app), ((36.0, 0.0, -24.0), true));
        app.run(Action::Undo);
        assert_eq!(state(&app), ((1.0, 0.0, 2.0), false), "one undo step");
    }

    #[test]
    fn a_group_drag_undoes_in_one_step_and_keeps_the_selection() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        app.run(Action::MoveTool);
        let id = first(&app);
        let doc = app.doc.as_mut().expect("doc");
        let b = doc
            .config
            .network
            .add_node(5.5, 3.0, -7.25, NodeFlag::REGULAR);
        let c = doc
            .config
            .network
            .add_node(-9.0, 1.0, 4.0, NodeFlag::REGULAR);
        doc.selection.extend([id, b, c]);
        let positions = |app: &App| {
            let net = &app.doc.as_ref().expect("doc").config.network;
            [id, b, c].map(|n| net.node(n).map(|n| (n.x, n.y, n.z)).expect("node"))
        };
        let start = positions(&app);
        // The loaded node is world (1, 2): map px (1025, 1026) at scale 1.
        let events = [
            DragEvent::Start {
                at: (1025.0, 1026.0),
                node: Some(id),
            },
            DragEvent::Move {
                at: (1040.0, 1030.0),
            },
            DragEvent::End {
                at: (1045.0, 1020.0),
            },
        ];
        for event in events {
            drag(&mut app, event, false);
        }
        assert_eq!(
            positions(&app),
            [(21.0, 0.0, -4.0), (25.5, 3.0, -13.25), (11.0, 1.0, -2.0)]
        );
        app.run(Action::Undo);
        let doc = app.doc.as_ref().expect("doc");
        assert!(!doc.history.can_undo(), "one undo step");
        assert_eq!(doc.selection, std::collections::BTreeSet::from([id, b, c]));
        assert_eq!(positions(&app), start);
    }

    #[test]
    fn a_rotation_of_the_selection_undoes_in_one_step() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        let id = first(&app);
        let doc = app.doc.as_mut().expect("doc");
        let b = doc
            .config
            .network
            .add_node(11.0, 3.0, 2.0, NodeFlag::REGULAR);
        doc.selection.extend([id, b]);
        // The tool has seen a view at zoom 4, as after one painted frame.
        let tool = tools::rotate::RotateTool::default();
        let mut t = crate::view::ViewTransform::new((512.0, 512.0), (1024.0, 1024.0));
        t.zoom = 4.0;
        tool.see(&t, doc.map.geometry, doc.map.node_size);
        let net = &doc.config.network;
        let (hx, hz) = tool.handle_at(net, &doc.selection).expect("handle");
        let geometry = doc.map.geometry;
        app.set_tool(Some(Box::new(tool)));
        let positions = |app: &App| {
            let net = &app.doc.as_ref().expect("doc").config.network;
            [id, b].map(|n| net.node(n).map(|n| (n.x, n.y, n.z)).expect("node"))
        };
        let start = positions(&app);
        // The centre is (6, 2); the pointer ends 90° clockwise from the handle.
        let radius = 2.0 - hz;
        let events = [
            DragEvent::Start {
                at: geometry.to_map(hx, hz),
                node: None,
            },
            DragEvent::End {
                at: geometry.to_map(6.0 + radius, 2.0),
            },
        ];
        for event in events {
            drag(&mut app, event, false);
        }
        let [(ax, ay, az), (bx, by, bz)] = positions(&app);
        assert!((ax - 6.0).abs() < 1e-9 && (az - -3.0).abs() < 1e-9);
        assert!((bx - 6.0).abs() < 1e-9 && (bz - 7.0).abs() < 1e-9);
        assert_eq!((ay, by), (0.0, 3.0));
        app.run(Action::Undo);
        let doc = app.doc.as_ref().expect("doc");
        assert!(!doc.history.can_undo(), "one undo step");
        assert_eq!(positions(&app), start);
    }

    /// The loaded node a (1, 0, 2) and b (41, 10, 2), the Bézier tool active and seen at
    /// zoom 4 (0.25 world units per px), and a curve previewed from a to b.
    fn bezier_curve(app: &mut App) -> [NodeId; 2] {
        let a = first(app);
        let doc = app.doc.as_mut().expect("doc");
        let b = doc
            .config
            .network
            .add_node(41.0, 10.0, 2.0, NodeFlag::REGULAR);
        let geometry = doc.map.geometry;
        app.run(Action::BezierTool);
        assert_eq!(active(app), Some(tools::bezier::NAME));
        let tool = tools::bezier::BezierTool::default();
        let mut t = crate::view::ViewTransform::new((512.0, 512.0), (1024.0, 1024.0));
        t.zoom = 4.0;
        tool.see(&t, geometry);
        app.set_tool(Some(Box::new(tool)));
        for id in [a, b] {
            app.view.hovered_node = Some(id);
            app.click(0.0, 0.0, false);
        }
        app.view.hovered_node = None;
        assert!(app.tool.as_mut().and_then(|t| t.curve_mut()).is_some());
        [a, b]
    }

    #[test]
    fn b_previews_a_curve_that_bends_commits_from_the_panel_and_undoes_in_one_step() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        let [a, b] = bezier_curve(&mut app);
        let before = snapshot(&app);
        // The handle sits at the midpoint (21, 2), map px (1045, 1026); drag it 10 down.
        for event in [
            DragEvent::Start {
                at: (1045.0, 1026.0),
                node: None,
            },
            DragEvent::Move {
                at: (1045.0, 1031.0),
            },
            DragEvent::End {
                at: (1045.0, 1036.0),
            },
        ] {
            drag(&mut app, event, false);
        }
        assert_eq!(snapshot(&app), before, "a handle drag changes no data");
        assert!(!app.doc.as_ref().expect("doc").history.can_undo());
        let curve = app
            .tool
            .as_mut()
            .and_then(|t| t.curve_mut())
            .expect("curve");
        assert_eq!(curve.control_points(), [(21.0, 32.0)]);
        curve.settings.points = 3;
        curve.settings.shape = ConnectionShape::Dual;
        app.finish_curve(ui::CurveChoice::Commit);
        assert!(app.error.is_none());
        assert!(app.tool.as_mut().and_then(|t| t.curve_mut()).is_none());
        assert_eq!(
            active(&app),
            Some(tools::bezier::NAME),
            "waits for a new start"
        );
        let net = &app.doc.as_ref().expect("doc").config.network;
        assert_eq!(net.len(), 5);
        let chain = [a, net.order()[2], net.order()[3], net.order()[4], b];
        for pair in chain.windows(2) {
            assert_eq!(net.connection_kind(pair[0], pair[1]), ConnectionKind::Dual);
        }
        // Bent off the line z = 2; heights linear from 0 to 10.
        let mid = net.node(chain[2]).expect("node");
        assert!(mid.z > 2.0);
        assert!((mid.y - 5.0).abs() < 1e-9);
        app.run(Action::Undo);
        assert_eq!(snapshot(&app), before);
        assert!(
            !app.doc.as_ref().expect("doc").history.can_undo(),
            "one step"
        );
    }

    #[test]
    fn the_curve_is_dropped_by_cancel_a_tool_switch_b_and_a_new_file() {
        let curve = |app: &mut App| app.tool.as_mut().and_then(|t| t.curve_mut()).is_some();
        let switches: [fn(&mut App); 4] = [
            |app| app.finish_curve(ui::CurveChoice::Cancel),
            |app| app.run(Action::MoveTool),
            |app| app.run(Action::BezierTool),
            |app| load(app, "<version>3</version>"),
        ];
        for switch in switches {
            let mut app = App::new(Settings::default());
            load(&mut app, "<version>3</version>");
            bezier_curve(&mut app);
            let before = snapshot(&app);
            switch(&mut app);
            assert!(!curve(&mut app));
            if app
                .doc
                .as_ref()
                .is_some_and(|d| d.config.network.len() == 2)
            {
                assert_eq!(snapshot(&app), before);
            }
            assert!(!app.doc.as_ref().expect("doc").history.can_undo());
        }
    }

    #[test]
    fn a_right_click_cancels_the_pending_curve_start() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        app.run(Action::BezierTool);
        let a = first(&app);
        app.view.hovered_node = Some(a);
        app.click(0.0, 0.0, false);
        let doc = app.doc.as_mut().expect("doc");
        let b = doc
            .config
            .network
            .add_node(41.0, 10.0, 2.0, NodeFlag::REGULAR);
        let mut hooks = ui::ToolHooks::new(
            app.tool.as_deref_mut(),
            &doc.map,
            &doc.config.network,
            &doc.selection,
            false,
        );
        hooks.select(crate::view::SelectEvent::Click { node: None });
        // The next click on a node starts a curve instead of ending one.
        app.view.hovered_node = Some(b);
        app.click(0.0, 0.0, false);
        assert!(app.tool.as_mut().and_then(|t| t.curve_mut()).is_none());
    }

    /// The arc tool active and seen at zoom 4 (0.25 world units per px), and an arc placed
    /// by a click at world (100, 50).
    fn arc_placed(app: &mut App) {
        let geometry = app.doc.as_ref().expect("doc").map.geometry;
        let tool = tools::arc::ArcTool::default();
        let mut t = crate::view::ViewTransform::new((512.0, 512.0), (1024.0, 1024.0));
        t.zoom = 4.0;
        tool.see(&t, geometry);
        app.set_tool(Some(Box::new(tool)));
        let (px, py) = geometry.to_map(100.0, 50.0);
        app.click(px, py, false);
        assert!(app.tool.as_mut().and_then(|t| t.arc_mut()).is_some());
    }

    #[test]
    fn an_arc_is_placed_shaped_by_its_handles_committed_and_undone_in_one_step() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        let len = nodes(&app);
        arc_placed(&mut app);
        let before = snapshot(&app);
        let geometry = app.doc.as_ref().expect("doc").map.geometry;
        // Centre (100, 50) to (110, 55); the start handle, now at (102, 55), to angle 0
        // (straight up at -z); the radius handle, now at (110, 51), to 6 from the centre.
        for (from, to) in [
            ((100.0, 50.0), (110.0, 55.0)),
            ((102.0, 55.0), (110.0, 40.0)),
            ((110.0, 51.0), (110.0, 49.0)),
        ] {
            let start = DragEvent::Start {
                at: geometry.to_map(from.0, from.1),
                node: None,
            };
            let end = DragEvent::End {
                at: geometry.to_map(to.0, to.1),
            };
            drag(&mut app, start, false);
            drag(&mut app, end, false);
        }
        assert_eq!(snapshot(&app), before, "a handle drag changes no data");
        assert!(!app.doc.as_ref().expect("doc").history.can_undo());
        let arc = app.tool.as_mut().and_then(|t| t.arc_mut()).expect("arc");
        assert_eq!((arc.centre, arc.start_deg), ((110.0, 55.0), 0.0));
        assert!((arc.radius - 12.0).abs() < 1e-9);
        arc.settings.points = 3;
        arc.swap();
        app.finish_curve(ui::CurveChoice::Commit);
        assert!(app.error.is_none());
        assert!(app.tool.as_mut().and_then(|t| t.arc_mut()).is_none());
        assert_eq!(active(&app), Some(tools::arc::NAME), "waits for a new arc");
        let net = &app.doc.as_ref().expect("doc").config.network;
        assert_eq!(net.len(), len + 5);
        let new = &net.order()[len..];
        // A quarter circle from (110, 43) round to (122, 55), no heightmap: y -1.
        let at = |i: usize| net.node(new[i]).map(|n| (n.x, n.y, n.z)).expect("node");
        let (x0, y0, z0) = at(0);
        let (x4, _, z4) = at(4);
        assert!((x0 - 110.0).abs() < 1e-9 && (z0 - 43.0).abs() < 1e-9);
        assert!((x4 - 122.0).abs() < 1e-9 && (z4 - 55.0).abs() < 1e-9);
        assert_eq!(y0, -1.0);
        // Swapped: one-way links from the last node back to the first, nothing else.
        for pair in new.windows(2) {
            assert_eq!(
                net.connection_kind(pair[1], pair[0]),
                ConnectionKind::Regular
            );
        }
        let links: usize = new
            .iter()
            .map(|id| net.node(*id).expect("node").outgoing().len())
            .sum();
        assert_eq!(links, 4);
        app.run(Action::Undo);
        assert_eq!(snapshot(&app), before);
        assert!(
            !app.doc.as_ref().expect("doc").history.can_undo(),
            "one step"
        );
    }

    #[test]
    fn the_arc_is_dropped_by_cancel_a_tool_switch_and_a_new_file() {
        let arc = |app: &mut App| app.tool.as_mut().and_then(|t| t.arc_mut()).is_some();
        let switches: [fn(&mut App); 4] = [
            |app| app.finish_curve(ui::CurveChoice::Cancel),
            |app| app.run(Action::MoveTool),
            |app| app.run(Action::BezierTool),
            |app| load(app, "<version>3</version>"),
        ];
        for switch in switches {
            let mut app = App::new(Settings::default());
            load(&mut app, "<version>3</version>");
            arc_placed(&mut app);
            let before = snapshot(&app);
            switch(&mut app);
            assert!(!arc(&mut app));
            assert_eq!(snapshot(&app), before);
            assert!(!app.doc.as_ref().expect("doc").history.can_undo());
        }
    }

    #[test]
    fn read_only_document_ignores_the_arc_tool() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<markerID>1</markerID>");
        assert!(!app.editable(), "the toolbar button is disabled");
        // Even a tool left active places nothing, and an arc it holds does not commit.
        app.tool = Some(Box::new(tools::arc::ArcTool::default()));
        app.click(1024.0, 1024.0, false);
        assert!(app.tool.as_mut().and_then(|t| t.arc_mut()).is_none());
        let mut tool = tools::arc::ArcTool::default();
        let doc = app.doc.as_ref().expect("doc");
        let ctx = tools::ToolContext {
            x: 0.0,
            z: 0.0,
            map: &doc.map,
            net: &doc.config.network,
            selection: &doc.selection,
            snap: false,
            hovered_node: None,
            hovered_edge: None,
        };
        assert!(tool.on_click(&ctx).is_none());
        app.tool = Some(Box::new(tool));
        app.finish_curve(ui::CurveChoice::Commit);
        assert_eq!(nodes(&app), 1);
        assert!(!app.doc.as_ref().expect("doc").history.can_undo());
    }

    #[test]
    fn read_only_document_ignores_the_bezier_tool() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<markerID>1</markerID>");
        app.run(Action::BezierTool);
        assert!(app.tool.is_none());
    }

    #[test]
    fn read_only_document_ignores_the_rotate_tool() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<markerID>1</markerID>");
        app.run(Action::RotateTool);
        assert!(app.tool.is_none());
    }

    #[test]
    fn shift_x_aligns_the_selection_to_a_clicked_node_and_undo_restores_it() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        let id = first(&app);
        let doc = app.doc.as_mut().expect("doc");
        let net = &mut doc.config.network;
        let b = net.add_node(11.0, 3.0, 2.0, NodeFlag::REGULAR);
        let c = net.add_node(-5.0, 1.0, 7.0, NodeFlag::REGULAR);
        let reference = net.add_node(40.0, 9.0, -12.5, NodeFlag::REGULAR);
        doc.selection.extend([id, b, c]);
        let positions = |app: &App| {
            let net = &app.doc.as_ref().expect("doc").config.network;
            [id, b, c, reference].map(|n| net.node(n).map(|n| (n.x, n.y, n.z)).expect("node"))
        };
        let start = positions(&app);
        app.run(Action::AlignHorizontalTool);
        assert_eq!(active(&app), Some("Align H"));
        app.view.hovered_node = Some(reference);
        app.click(0.0, 0.0, false);
        let now = positions(&app);
        for (i, (x, y, z)) in now[..3].iter().enumerate() {
            assert_eq!((*x, *y, *z), (start[i].0, start[i].1, -12.5));
        }
        assert_eq!(now[3], start[3]);
        let doc = app.doc.as_ref().expect("doc");
        assert_eq!(doc.selection, std::collections::BTreeSet::from([id, b, c]));
        app.run(Action::Undo);
        assert!(
            !app.doc.as_ref().expect("doc").history.can_undo(),
            "one undo step"
        );
        assert_eq!(positions(&app), start);
        app.run(Action::AlignVerticalTool);
        assert_eq!(active(&app), Some("Align V"));
    }

    #[test]
    fn read_only_document_ignores_the_align_tools() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<markerID>1</markerID>");
        app.run(Action::AlignHorizontalTool);
        app.run(Action::AlignVerticalTool);
        assert!(app.tool.is_none());
        let id = first(&app);
        let doc = app.doc.as_mut().expect("doc");
        let reference = doc
            .config
            .network
            .add_node(40.0, 9.0, -12.5, NodeFlag::REGULAR);
        doc.selection.insert(id);
        app.tool = Some(Box::new(tools::align::AlignTool::new(
            tools::align::Axis::Horizontal,
        )));
        app.view.hovered_node = Some(reference);
        app.click(0.0, 0.0, false);
        assert!(!app.doc.as_ref().expect("doc").history.can_undo());
    }

    /// Loads the one-node config of `version`, picks Edit location and clicks with the node
    /// hovered, or with nothing hovered when `on_node` is false.
    fn edit_location(version: &str, on_node: bool) -> App {
        let mut app = App::new(Settings::default());
        load(&mut app, version);
        app.tool = Some(Box::new(tools::edit_location::EditLocationTool::default()));
        app.view.hovered_node = on_node.then(|| first(&app));
        app.click(0.0, 0.0, false);
        app
    }

    #[test]
    fn an_edit_location_click_opens_the_dialog_for_the_hovered_node_only() {
        let app = edit_location("<version>3</version>", true);
        let dialog = app.location.as_ref().expect("dialog");
        assert_eq!(dialog.node, first(&app));
        assert_eq!(dialog.text, ["1.000", "0.000", "2.000"].map(String::from));
        assert!(!app.doc.as_ref().expect("doc").history.can_undo());
        let app = edit_location("<version>3</version>", false);
        assert!(app.location.is_none());
    }

    #[test]
    fn edit_location_ok_commits_one_move_and_one_undo_restores_it() {
        let mut app = edit_location("<version>3</version>", true);
        let id = first(&app);
        app.doc.as_mut().expect("doc").selection.insert(id);
        let start = state(&app).0;
        app.location.as_mut().expect("dialog").text =
            ["12.3456", "  4,5 ", "-7.0005"].map(String::from);
        app.finish_location(true);
        assert!(app.location.is_none());
        assert_eq!(state(&app), ((12.346, 4.5, -7.001), true));
        let doc = app.doc.as_ref().expect("doc");
        assert_eq!(doc.selection, std::collections::BTreeSet::from([id]));
        app.run(Action::Undo);
        assert_eq!(state(&app), (start, false), "one undo step");
    }

    #[test]
    fn edit_location_cancel_unchanged_or_invalid_commits_nothing() {
        let edits: [([&str; 3], bool); 4] = [
            (["5", "5", "5"], false),
            (["1.000", "0.000", "2.000"], true),
            (["1.0004", "0", "2"], true),
            (["5", "-1", "5"], true),
        ];
        for (text, ok) in edits {
            let mut app = edit_location("<version>3</version>", true);
            let start = state(&app);
            app.location.as_mut().expect("dialog").text = text.map(String::from);
            app.finish_location(ok);
            assert!(app.location.is_none(), "{text:?}");
            assert_eq!(state(&app), start, "{text:?}");
        }
    }

    #[test]
    fn read_only_document_ignores_the_edit_location_tool() {
        let mut app = edit_location("<markerID>1</markerID>", true);
        assert!(app.location.is_none(), "no dialog");
        // Even a dialog left open commits nothing.
        let net = &app.doc.as_ref().expect("doc").config.network;
        let mut dialog = tools::edit_location::LocationDialog::open(net, first(&app));
        dialog.as_mut().expect("dialog").text[0] = "5".to_owned();
        app.location = dialog;
        app.finish_location(true);
        assert!(!app.doc.as_ref().expect("doc").history.can_undo());
    }

    #[test]
    fn shortcuts_and_clicks_do_nothing_while_the_edit_location_dialog_is_shown() {
        let mut app = edit_location("<version>3</version>", true);
        // Something to undo: the node moved through the dialog, then the dialog again.
        app.location.as_mut().expect("dialog").text[0] = "5".to_owned();
        app.finish_location(true);
        app.click(0.0, 0.0, false);
        assert!(app.location.is_some());
        let before = state(&app);
        let ctx = egui::Context::default();
        let key = |key, modifiers| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        let ctrl = egui::Modifiers::CTRL.plus(egui::Modifiers::COMMAND);
        let input = egui::RawInput {
            events: vec![
                key(egui::Key::Z, ctrl),
                key(egui::Key::N, egui::Modifiers::NONE),
                key(egui::Key::S, egui::Modifiers::NONE),
            ],
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| input::handle_shortcuts(&mut app, ui.ctx()));
        out.textures_delta.clear();
        assert_eq!(state(&app), before, "no undo");
        assert_eq!(active(&app), Some("Edit location"));
        assert!(!app.snap);
        // A click on the map while it is shown opens nothing new and commits nothing.
        let shown = app.location.clone();
        app.click(0.0, 0.0, false);
        assert_eq!(app.location, shown);
    }

    #[test]
    fn a_new_document_closes_the_edit_location_dialog() {
        let mut app = edit_location("<version>3</version>", true);
        assert!(app.location.is_some());
        load(&mut app, "<version>3</version>");
        assert!(app.location.is_none());
    }

    /// Gives the loaded node the marker "Feld", outside the history.
    fn mark_first(app: &mut App) {
        let id = first(app);
        let net = &mut app.doc.as_mut().expect("doc").config.network;
        net.push_marker(ad_core::network::Marker {
            node: id,
            name: "Feld".to_owned(),
            group: "All".to_owned(),
            parked_vehicles: Vec::new(),
        });
    }

    /// Loads the one-node config of `version`, picks Add marker and clicks with the node
    /// hovered.
    fn add_marker(version: &str) -> App {
        let mut app = App::new(Settings::default());
        load(&mut app, version);
        app.tool = Some(Box::new(tools::marker::AddMarkerTool::default()));
        app.view.hovered_node = Some(first(&app));
        app.click(0.0, 0.0, false);
        app
    }

    #[test]
    fn add_marker_ok_commits_one_marker_and_one_undo_removes_it() {
        let mut app = add_marker("<version>3</version>");
        let id = first(&app);
        app.doc.as_mut().expect("doc").selection.insert(id);
        let dialog = app.marker.as_mut().expect("dialog");
        assert_eq!(dialog.node, id);
        dialog.name = " Hof ".to_owned();
        dialog.group = "Neu".to_owned();
        app.finish_marker(true);
        assert!(app.marker.is_none());
        let doc = app.doc.as_ref().expect("doc");
        let m = &doc.config.network.markers()[0];
        assert_eq!((m.node, &*m.name, &*m.group), (id, "Hof", "Neu"));
        assert!(m.parked_vehicles.is_empty());
        assert_eq!(doc.selection, BTreeSet::from([id]), "selection unaffected");
        app.run(Action::Undo);
        assert!(marker_names(&app).is_empty(), "one undo step");
        assert!(!app.doc.as_ref().expect("doc").history.can_undo());
    }

    #[test]
    fn add_marker_on_a_marked_node_shows_the_notice_until_the_next_click() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        mark_first(&mut app);
        app.tool = Some(Box::new(tools::marker::AddMarkerTool::default()));
        app.view.hovered_node = Some(first(&app));
        app.click(0.0, 0.0, false);
        assert!(app.marker.is_none(), "no dialog");
        assert_eq!(app.tool_notice, Some(tools::marker::HAS_MARKER));
        app.view.hovered_node = None;
        app.click(0.0, 0.0, false);
        assert_eq!(app.tool_notice, None);
        assert!(!app.doc.as_ref().expect("doc").history.can_undo());
    }

    #[test]
    fn add_marker_cancel_or_a_refused_name_commits_nothing() {
        for (name, ok) in [("Hof", false), ("  ", true), ("Feld", true)] {
            let mut app = App::new(Settings::default());
            load(&mut app, "<version>3</version>");
            app.tool = Some(Box::new(tools::marker::AddMarkerTool::default()));
            app.view.hovered_node = Some(first(&app));
            app.click(0.0, 0.0, false);
            // "Feld" is taken once another node has it; the dialog checks on OK too.
            if name == "Feld" {
                let doc = app.doc.as_mut().expect("doc");
                let other = doc
                    .config
                    .network
                    .add_node(0.0, 0.0, 0.0, NodeFlag::REGULAR);
                doc.config.network.push_marker(ad_core::network::Marker {
                    node: other,
                    name: "Feld".to_owned(),
                    group: "All".to_owned(),
                    parked_vehicles: Vec::new(),
                });
            }
            app.marker.as_mut().expect("dialog").name = name.to_owned();
            app.finish_marker(ok);
            assert!(app.marker.is_none(), "{name:?}");
            assert!(
                !app.doc.as_ref().expect("doc").history.can_undo(),
                "{name:?}"
            );
        }
    }

    #[test]
    fn w_opens_the_marker_dialog_for_a_hovered_unmarked_node_whichever_tool_is_active() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        // The view showed this document.
        app.view_epoch = app.doc.as_ref().map(|d| d.epoch);
        app.run(Action::NodeTool);
        app.run(Action::AddMarker);
        assert!(app.marker.is_none(), "over empty map");
        app.view.hovered_node = Some(first(&app));
        app.run(Action::AddMarker);
        assert_eq!(app.marker.as_ref().map(|d| d.node), Some(first(&app)));
        assert_eq!(active(&app), Some("Node"));
        app.finish_marker(false);
        mark_first(&mut app);
        app.run(Action::AddMarker);
        assert!(app.marker.is_none(), "over a marked node");
        assert_eq!(app.tool_notice, None);
    }

    #[test]
    fn read_only_document_ignores_the_add_marker_tool_and_w() {
        let mut app = add_marker("<markerID>1</markerID>");
        assert!(app.marker.is_none(), "no dialog from the tool");
        app.view_epoch = app.doc.as_ref().map(|d| d.epoch);
        app.run(Action::AddMarker);
        assert!(app.marker.is_none(), "no dialog from W");
        // Even a dialog left open commits nothing.
        let net = &app.doc.as_ref().expect("doc").config.network;
        let mut dialog = tools::marker::MarkerDialog::open(net, first(&app));
        dialog.as_mut().expect("dialog").name = "Hof".to_owned();
        app.marker = dialog;
        app.finish_marker(true);
        assert!(marker_names(&app).is_empty());
    }

    #[test]
    fn shortcuts_and_clicks_do_nothing_while_the_marker_dialog_is_shown() {
        let mut app = add_marker("<version>3</version>");
        app.view_epoch = app.doc.as_ref().map(|d| d.epoch);
        // Something to undo: a marker through the dialog, then the dialog for a new node.
        app.marker.as_mut().expect("dialog").name = "Hof".to_owned();
        app.finish_marker(true);
        let doc = app.doc.as_mut().expect("doc");
        let other = doc
            .config
            .network
            .add_node(5.0, 0.0, 5.0, NodeFlag::REGULAR);
        app.view.hovered_node = Some(other);
        app.click(0.0, 0.0, false);
        let shown = app.marker.clone();
        assert_eq!(shown.as_ref().map(|d| d.node), Some(other));
        let ctx = egui::Context::default();
        let key = |key, modifiers| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        let ctrl = egui::Modifiers::CTRL.plus(egui::Modifiers::COMMAND);
        let input = egui::RawInput {
            events: vec![
                key(egui::Key::Z, ctrl),
                key(egui::Key::N, egui::Modifiers::NONE),
                key(egui::Key::S, egui::Modifiers::NONE),
                key(egui::Key::W, egui::Modifiers::NONE),
            ],
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| input::handle_shortcuts(&mut app, ui.ctx()));
        out.textures_delta.clear();
        assert_eq!(marker_names(&app), ["Hof"], "no undo");
        assert_eq!(active(&app), Some("Add marker"));
        assert!(!app.snap);
        assert_eq!(app.marker, shown);
        // A click on the map while it is shown opens nothing new and commits nothing.
        app.view.hovered_node = Some(first(&app));
        app.click(0.0, 0.0, false);
        assert_eq!(app.marker, shown);
        assert_eq!(app.tool_notice, None);
    }

    #[test]
    fn a_new_document_closes_the_marker_dialog_and_the_notice() {
        let mut app = add_marker("<version>3</version>");
        assert!(app.marker.is_some());
        app.tool_notice = Some(tools::marker::HAS_MARKER);
        load(&mut app, "<version>3</version>");
        assert!(app.marker.is_none());
        assert!(app.tool_notice.is_none());
    }

    /// Loads the one-node config of `version`, gives the node the marker "Feld" with a
    /// parked vehicle and selects it; the view showed this document.
    fn marked(version: &str) -> App {
        let mut app = App::new(Settings::default());
        load(&mut app, version);
        mark_first(&mut app);
        let id = first(&app);
        let doc = app.doc.as_mut().expect("doc");
        assert!(doc.config.network.park_vehicle(0, "7".to_owned()));
        doc.selection.insert(id);
        app.view_epoch = Some(doc.epoch);
        app
    }

    fn markers(app: &App) -> Vec<ad_core::network::Marker> {
        app.doc
            .as_ref()
            .expect("doc")
            .config
            .network
            .markers()
            .to_vec()
    }

    #[test]
    fn edit_marker_ok_commits_one_edit_and_one_undo_restores_it() {
        let mut app = marked("<version>3</version>");
        let before = markers(&app);
        app.tool = Some(Box::new(tools::marker::EditMarkerTool::default()));
        app.view.hovered_node = Some(first(&app));
        app.click(0.0, 0.0, false);
        let dialog = app.marker.as_mut().expect("dialog");
        assert_eq!((&*dialog.name, &*dialog.group), ("Feld", "None"));
        dialog.name = "Hof".to_owned();
        dialog.group = "Neu".to_owned();
        app.finish_marker(true);
        let after = markers(&app);
        assert_eq!((&*after[0].name, &*after[0].group), ("Hof", "Neu"));
        assert_eq!(after[0].parked_vehicles, ["7"]);
        let doc = app.doc.as_ref().expect("doc");
        assert_eq!(doc.selection, BTreeSet::from([first(&app)]));
        app.run(Action::Undo);
        assert_eq!(markers(&app), before, "one undo step");
        assert!(!app.doc.as_ref().expect("doc").history.can_undo());
        // OK without a change commits nothing.
        app.click(0.0, 0.0, false);
        app.finish_marker(true);
        assert!(!app.doc.as_ref().expect("doc").history.can_undo());
    }

    #[test]
    fn delete_marker_removes_it_in_one_step_and_undo_brings_back_its_vehicles() {
        let mut app = marked("<version>3</version>");
        let before = markers(&app);
        app.tool = Some(Box::new(tools::marker::DeleteMarkerTool::default()));
        app.view.hovered_node = Some(first(&app));
        app.click(0.0, 0.0, false);
        assert!(markers(&app).is_empty());
        assert!(app.marker.is_none(), "no confirmation");
        assert_eq!(app.tool_notice, None);
        let doc = app.doc.as_ref().expect("doc");
        assert_eq!(doc.selection, BTreeSet::from([first(&app)]));
        app.run(Action::Undo);
        assert_eq!(markers(&app), before);
        assert!(!app.doc.as_ref().expect("doc").history.can_undo());
    }

    #[test]
    fn edit_or_delete_marker_on_a_plain_node_shows_the_notice_until_the_next_click() {
        let tools: [fn() -> Box<dyn Tool>; 2] = [
            || Box::new(tools::marker::EditMarkerTool::default()),
            || Box::new(tools::marker::DeleteMarkerTool::default()),
        ];
        for tool in tools {
            let mut app = App::new(Settings::default());
            load(&mut app, "<version>3</version>");
            app.tool = Some(tool());
            app.view.hovered_node = Some(first(&app));
            app.click(0.0, 0.0, false);
            assert!(app.marker.is_none(), "no dialog");
            assert_eq!(app.tool_notice, Some(tools::marker::NO_MARKER));
            assert!(!app.doc.as_ref().expect("doc").history.can_undo());
            app.view.hovered_node = None;
            app.click(0.0, 0.0, false);
            assert_eq!(app.tool_notice, None);
        }
    }

    #[test]
    fn e_and_shift_d_act_on_a_hovered_marked_node_whichever_tool_is_active() {
        let mut app = marked("<version>3</version>");
        let before = markers(&app);
        app.run(Action::NodeTool);
        // Over empty map: nothing.
        app.run(Action::EditMarker);
        app.run(Action::DeleteMarker);
        assert!(app.marker.is_none());
        assert_eq!(markers(&app), before);
        // Over the marked node: the prefilled dialog, then the delete.
        app.view.hovered_node = Some(first(&app));
        app.run(Action::EditMarker);
        let dialog = app.marker.as_ref().expect("dialog");
        assert_eq!((dialog.node, dialog.title()), (first(&app), "Edit marker"));
        app.finish_marker(false);
        app.run(Action::DeleteMarker);
        assert!(markers(&app).is_empty());
        assert_eq!(active(&app), Some("Node"));
        // Over a plain node: nothing, and no notice.
        app.run(Action::EditMarker);
        app.run(Action::DeleteMarker);
        assert!(app.marker.is_none());
        assert_eq!(app.tool_notice, None);
        app.run(Action::Undo);
        assert_eq!(markers(&app), before, "the delete was one step");
    }

    #[test]
    fn read_only_document_ignores_the_edit_and_delete_marker_tools_and_keys() {
        let mut app = marked("<markerID>1</markerID>");
        let before = markers(&app);
        app.view.hovered_node = Some(first(&app));
        app.run(Action::EditMarker);
        app.run(Action::DeleteMarker);
        assert!(app.marker.is_none());
        let tools: [Box<dyn Tool>; 2] = [
            Box::new(tools::marker::EditMarkerTool::default()),
            Box::new(tools::marker::DeleteMarkerTool::default()),
        ];
        for tool in tools {
            app.tool = Some(tool);
            app.click(0.0, 0.0, false);
            assert!(app.marker.is_none());
            assert_eq!(app.tool_notice, None);
        }
        assert_eq!(markers(&app), before);
        // Even an edit dialog left open commits nothing.
        let net = &app.doc.as_ref().expect("doc").config.network;
        let mut dialog = tools::marker::MarkerDialog::edit(net, first(&app));
        dialog.as_mut().expect("dialog").name = "Hof".to_owned();
        app.marker = dialog;
        app.finish_marker(true);
        assert_eq!(markers(&app), before);
    }

    #[test]
    fn e_and_shift_d_do_nothing_during_a_drawn_selection() {
        let mut app = marked("<version>3</version>");
        let before = markers(&app);
        app.view.hovered_node = Some(first(&app));
        app.view.start_select((1000.0, 1000.0));
        assert!(app.view.selecting());
        app.run(Action::EditMarker);
        app.run(Action::DeleteMarker);
        assert!(app.marker.is_none());
        assert_eq!(markers(&app), before);
    }

    /// Loads the one-node config of `version` plus two nodes, with markers "m0" and "m1" in
    /// "b" (a vehicle parked at "m0") and "m2" in "A", outside the history.
    fn grouped(version: &str) -> App {
        let mut app = App::new(Settings::default());
        load(&mut app, version);
        let net = &mut app.doc.as_mut().expect("doc").config.network;
        let mut ids = vec![net.order()[0]];
        ids.push(net.add_node(5.0, 0.0, 5.0, NodeFlag::REGULAR));
        ids.push(net.add_node(9.0, 0.0, 9.0, NodeFlag::REGULAR));
        for (i, (id, group)) in ids.into_iter().zip(["b", "b", "A"]).enumerate() {
            net.push_marker(ad_core::network::Marker {
                node: id,
                name: format!("m{i}"),
                group: group.to_owned(),
                parked_vehicles: Vec::new(),
            });
        }
        assert!(net.park_vehicle(0, "7".to_owned()));
        app
    }

    fn group_names(app: &App) -> Vec<String> {
        markers(app).into_iter().map(|m| m.group).collect()
    }

    #[test]
    fn the_groups_window_renames_and_deletes_each_in_one_undo_step() {
        let mut app = grouped("<version>3</version>");
        let before = markers(&app);
        app.open_groups();
        let window = app.groups.as_mut().expect("window");
        window.start_rename("b");
        window.rename.as_mut().expect("field").1 = " c ".to_owned();
        app.finish_group_rename();
        assert_eq!(group_names(&app), ["c", "c", "A"]);
        let window = app.groups.as_ref().expect("still open");
        assert_eq!(window.rename, None, "the field closes");
        app.delete_group("A");
        assert_eq!(group_names(&app), ["c", "c", "All"]);
        let after = markers(&app);
        assert_eq!(after[0].parked_vehicles, ["7"]);
        let names: Vec<_> = after.iter().map(|m| m.name.clone()).collect();
        assert_eq!(names, ["m0", "m1", "m2"]);
        app.groups = None;
        app.run(Action::Undo);
        assert_eq!(group_names(&app), ["c", "c", "A"]);
        app.run(Action::Undo);
        assert_eq!(markers(&app), before);
        app.run(Action::Redo);
        assert_eq!(group_names(&app), ["c", "c", "A"]);
    }

    #[test]
    fn a_refused_group_rename_commits_nothing_and_keeps_the_field() {
        let mut app = grouped("<version>3</version>");
        let before = markers(&app);
        app.open_groups();
        for text in ["", "  ", "b", "None"] {
            let window = app.groups.as_mut().expect("window");
            window.start_rename("b");
            window.rename.as_mut().expect("field").1 = text.to_owned();
            app.finish_group_rename();
            assert!(app.groups.as_ref().expect("window").rename.is_some());
            assert_eq!(markers(&app), before, "{text:?}");
        }
        app.delete_group("All");
        assert_eq!(markers(&app), before);
        assert!(!app.doc.as_ref().expect("doc").history.can_undo());
    }

    #[test]
    fn read_only_document_does_not_open_the_groups_window_or_change_groups() {
        let mut app = grouped("<markerID>1</markerID>");
        let before = markers(&app);
        app.open_groups();
        assert!(app.groups.is_none());
        // Even a window shown by force commits nothing.
        let mut window = tools::marker::GroupsWindow::default();
        window.start_rename("b");
        window.rename.as_mut().expect("field").1 = "c".to_owned();
        app.groups = Some(window);
        app.finish_group_rename();
        app.delete_group("A");
        assert_eq!(markers(&app), before);
    }

    #[test]
    fn shortcuts_and_clicks_do_nothing_while_the_groups_window_is_shown() {
        let mut app = grouped("<version>3</version>");
        app.view_epoch = app.doc.as_ref().map(|d| d.epoch);
        app.tool = Some(Box::new(NodeTool::new(NodeFlag::REGULAR)));
        // Something to undo: a delete through the window.
        app.open_groups();
        app.delete_group("A");
        let before = markers(&app);
        let nodes_before = nodes(&app);
        assert!(app.dialog_shown());
        let ctx = egui::Context::default();
        let key = |key, modifiers| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        let ctrl = egui::Modifiers::CTRL.plus(egui::Modifiers::COMMAND);
        let input = egui::RawInput {
            events: vec![
                key(egui::Key::Z, ctrl),
                key(egui::Key::S, egui::Modifiers::NONE),
                key(egui::Key::W, egui::Modifiers::NONE),
            ],
            ..Default::default()
        };
        app.view.hovered_node = Some(first(&app));
        let mut out = ctx.run_ui(input, |ui| input::handle_shortcuts(&mut app, ui.ctx()));
        out.textures_delta.clear();
        assert_eq!(markers(&app), before, "no undo");
        assert!(!app.snap);
        assert!(app.marker.is_none(), "no marker dialog from W");
        app.click(0.0, 0.0, false);
        assert_eq!(nodes(&app), nodes_before, "no node from a click");
        assert!(app.groups.is_some());
    }

    #[test]
    fn a_new_document_closes_the_groups_window() {
        let mut app = grouped("<version>3</version>");
        app.open_groups();
        app.groups.as_mut().expect("window").start_rename("b");
        load(&mut app, "<version>3</version>");
        assert!(app.groups.is_none());
    }

    #[test]
    fn a_snapped_drag_lands_on_the_grid() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        let grid = app.doc.as_ref().expect("doc").map.grid;
        app.run(Action::MoveTool);
        let start = DragEvent::Start {
            at: (1025.0, 1026.0),
            node: Some(first(&app)),
        };
        drag(&mut app, start, true);
        drag(
            &mut app,
            DragEvent::End {
                at: (1036.3, 1013.2),
            },
            true,
        );
        let ((x, _, z), undoable) = state(&app);
        assert!(undoable);
        let on_grid = |v: f64, g: f64| ((v / g).round() * g - v).abs() < 1e-9;
        assert!(
            on_grid(x, grid.0) && on_grid(z, grid.1),
            "({x}, {z}) on {grid:?}"
        );
        assert!((x - 12.3).abs() <= grid.0 / 2.0 && (z - -10.8).abs() <= grid.1 / 2.0);
    }

    #[test]
    fn a_tool_switch_mid_drag_leaves_the_network_unchanged() {
        for switch in [Action::MoveTool, Action::NodeTool] {
            let mut app = App::new(Settings::default());
            load(&mut app, "<version>3</version>");
            app.run(Action::MoveTool);
            let start = DragEvent::Start {
                at: (1025.0, 1026.0),
                node: Some(first(&app)),
            };
            drag(&mut app, start, false);
            drag(
                &mut app,
                DragEvent::Move {
                    at: (1100.0, 1100.0),
                },
                false,
            );
            app.run(switch);
            assert!(app.tool.as_ref().is_some_and(|t| t.preview().is_none()));
            // The rest of the drag reaches the new tool.
            drag(
                &mut app,
                DragEvent::Move {
                    at: (1110.0, 1100.0),
                },
                false,
            );
            drag(
                &mut app,
                DragEvent::End {
                    at: (1110.0, 1100.0),
                },
                false,
            );
            assert_eq!(state(&app), ((1.0, 0.0, 2.0), false), "{switch:?}");
        }
    }

    #[test]
    fn a_press_and_release_without_moving_commits_nothing() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        app.run(Action::MoveTool);
        // A press and release without a drag is a click; the move tool ignores it.
        app.click(1025.0, 1026.0, false);
        let start = DragEvent::Start {
            at: (1025.0, 1026.0),
            node: Some(first(&app)),
        };
        drag(&mut app, start, false);
        drag(
            &mut app,
            DragEvent::End {
                at: (1025.0, 1026.0),
            },
            false,
        );
        assert_eq!(state(&app), ((1.0, 0.0, 2.0), false));
    }

    #[test]
    fn undo_or_redo_mid_drag_drops_the_drag() {
        for action in [Action::Undo, Action::Redo] {
            let mut app = App::new(Settings::default());
            load(&mut app, "<version>3</version>");
            app.run(Action::NodeTool);
            // Map px (1034, 1019) is world (10, -5).
            app.click(1034.0, 1019.0, false);
            if action == Action::Redo {
                app.run(Action::Undo);
            }
            app.run(Action::MoveTool);
            // Undo removes the added node under the drag; redo re-adds it.
            let net = &app.doc.as_ref().expect("doc").config.network;
            let target = *net.order().last().expect("node");
            let start = DragEvent::Start {
                at: (1025.0, 1026.0),
                node: Some(target),
            };
            drag(&mut app, start, false);
            drag(
                &mut app,
                DragEvent::Move {
                    at: (1100.0, 1100.0),
                },
                false,
            );
            app.run(action);
            assert!(app.tool.as_ref().is_some_and(|t| t.preview().is_none()));
            // `drag` panics on an apply error; the release must not commit at all.
            drag(
                &mut app,
                DragEvent::End {
                    at: (1100.0, 1100.0),
                },
                false,
            );
            let doc = app.doc.as_ref().expect("doc");
            assert_eq!(state(&app).0, (1.0, 0.0, 2.0), "{action:?}");
            assert_eq!(doc.history.can_redo(), action == Action::Undo, "{action:?}");
            assert!(app.error.is_none());
        }
    }

    /// Marks the view as showing the open document, as the map panel does each frame.
    fn shown(app: &mut App) {
        app.view_epoch = app.doc.as_ref().map(|d| d.epoch);
    }

    /// Order, positions, links and markers of the open document.
    fn snapshot(app: &App) -> String {
        let net = &app.doc.as_ref().expect("doc").config.network;
        let nodes: Vec<_> = net
            .order()
            .iter()
            .map(|id| {
                let n = net.node(*id).expect("node");
                let links = (n.outgoing().to_vec(), n.incoming().to_vec());
                (*id, n.x, n.y, n.z, n.flag, links)
            })
            .collect();
        format!("{nodes:?} {:?}", net.markers())
    }

    #[test]
    fn the_delete_key_deletes_the_hovered_node_and_undo_restores_it() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        shown(&mut app);
        let before = snapshot(&app);
        app.run(Action::DeleteHovered);
        assert_eq!(nodes(&app), 1, "nothing without a hovered node");
        app.view.hovered_node = Some(first(&app));
        app.run(Action::DeleteHovered);
        assert_eq!(nodes(&app), 0);
        assert!(app.error.is_none());
        app.run(Action::Undo);
        assert_eq!(snapshot(&app), before);
    }

    #[test]
    fn the_delete_key_deletes_a_non_empty_selection_and_one_undo_restores_it() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        shown(&mut app);
        let a = first(&app);
        let doc = app.doc.as_mut().expect("doc");
        let net = &mut doc.config.network;
        let b = net.add_node(5.5, 3.0, -7.25, NodeFlag::REGULAR);
        let c = net.add_node(-9.0, 1.0, 4.0, NodeFlag::REGULAR);
        net.connect(a, b, ConnectionShape::Dual).expect("ok");
        net.connect(b, c, ConnectionShape::OneWay).expect("ok");
        doc.selection.extend([a, b]);
        let before = snapshot(&app);
        // The selection wins over the hovered node.
        app.view.hovered_node = Some(c);
        app.run(Action::DeleteHovered);
        let doc = app.doc.as_ref().expect("doc");
        assert_eq!(doc.config.network.order(), [c]);
        assert!(doc.selection.is_empty(), "pruned");
        assert!(app.error.is_none());
        app.run(Action::Undo);
        assert_eq!(snapshot(&app), before);
        assert!(!app.doc.as_ref().expect("doc").history.can_undo());
    }

    #[test]
    fn the_delete_key_does_nothing_while_a_selection_is_drawn() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        shown(&mut app);
        let id = first(&app);
        app.view.hovered_node = Some(id);
        app.view.start_select((1000.0, 1000.0));
        assert!(app.view.selecting());
        app.run(Action::DeleteHovered);
        app.doc.as_mut().expect("doc").selection.insert(id);
        app.run(Action::DeleteHovered);
        assert_eq!(nodes(&app), 1);
        assert!(!app.doc.as_ref().expect("doc").history.can_undo());
    }

    #[test]
    fn read_only_document_ignores_the_delete_key_on_a_selection() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<markerID>1</markerID>");
        shown(&mut app);
        let id = first(&app);
        app.doc.as_mut().expect("doc").selection.insert(id);
        app.run(Action::DeleteHovered);
        app.tool = Some(Box::new(tools::delete::DeleteTool));
        app.view.hovered_node = Some(id);
        app.click(0.0, 0.0, false);
        assert_eq!(nodes(&app), 1);
        assert!(app.error.is_none());
    }

    #[test]
    fn delete_priority_and_flip_on_a_selected_node_undo_the_group_in_one_step() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        shown(&mut app);
        let a = first(&app);
        let doc = app.doc.as_mut().expect("doc");
        let net = &mut doc.config.network;
        let b = net.add_node(5.5, 3.0, -7.25, NodeFlag::REGULAR);
        let c = net.add_node(-9.0, 1.0, 4.0, NodeFlag::SUB_PRIO);
        net.connect(a, b, ConnectionShape::OneWay).expect("ok");
        net.connect(b, c, ConnectionShape::Reverse).expect("ok");
        doc.selection.extend([a, b, c]);
        let before = snapshot(&app);
        let tools: [Box<dyn Tool>; 3] = [
            Box::new(tools::delete::DeleteTool),
            Box::new(tools::priority::PriorityTool),
            Box::new(tools::flip::FlipTool),
        ];
        for tool in tools {
            let name = tool.name();
            app.set_tool(Some(tool));
            app.view.hovered_node = Some(b);
            app.click(0.0, 0.0, false);
            assert_ne!(snapshot(&app), before, "{name}");
            app.run(Action::Undo);
            assert_eq!(snapshot(&app), before, "{name}");
            let doc = app.doc.as_mut().expect("doc");
            assert!(!doc.history.can_undo(), "{name}: one undo step");
            // A delete prunes the selection; undo does not reselect.
            doc.selection.extend([a, b, c]);
        }
        assert!(app.error.is_none());
    }

    #[test]
    fn the_delete_key_ignores_a_hover_from_another_document() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        shown(&mut app);
        app.view.hovered_node = Some(first(&app));
        // A new document whose view has not been shown yet.
        load(&mut app, "<version>3</version>");
        app.run(Action::DeleteHovered);
        assert_eq!(nodes(&app), 1);
        assert!(app.error.is_none());
    }

    #[test]
    fn read_only_document_ignores_the_delete_key_and_tool() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<markerID>1</markerID>");
        shown(&mut app);
        app.view.hovered_node = Some(first(&app));
        app.run(Action::DeleteHovered);
        app.tool = Some(Box::new(tools::delete::DeleteTool));
        app.click(1025.0, 1026.0, false);
        assert_eq!(nodes(&app), 1);
        assert!(app.error.is_none());
    }

    #[test]
    fn undo_and_delete_in_one_frame_delete_nothing() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        shown(&mut app);
        let id = first(&app);
        let doc = app.doc.as_mut().expect("doc");
        let before = doc
            .config
            .network
            .node(id)
            .map(|n| (n.x, n.y, n.z))
            .expect("node");
        let after = (before.0 + 50.0, before.1, before.2 + 50.0);
        doc.apply(Transaction::new(vec![Command::MoveNodes(
            ad_core::command::MoveNodes {
                moves: vec![(id, before, after)],
            },
        )]))
        .expect("move");
        // The hover was computed on the moved network; Undo and Delete arrive in one frame.
        app.view.hovered_node = Some(id);
        app.run(Action::Undo);
        app.run(Action::DeleteHovered);
        assert_eq!(nodes(&app), 1);
    }

    #[test]
    fn the_delete_key_mid_drag_cancels_the_drag() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        shown(&mut app);
        app.run(Action::MoveTool);
        let id = first(&app);
        let start = DragEvent::Start {
            at: (1025.0, 1026.0),
            node: Some(id),
        };
        drag(&mut app, start, false);
        drag(
            &mut app,
            DragEvent::Move {
                at: (1100.0, 1100.0),
            },
            false,
        );
        app.view.hovered_node = Some(id);
        app.run(Action::DeleteHovered);
        assert!(app.tool.as_ref().is_some_and(|t| t.preview().is_none()));
        assert_eq!(nodes(&app), 0);
        // `drag` panics on an apply error; the release must not commit a move.
        drag(
            &mut app,
            DragEvent::End {
                at: (1100.0, 1100.0),
            },
            false,
        );
        app.run(Action::Undo);
        assert_eq!(state(&app), ((1.0, 0.0, 2.0), false));
    }

    #[test]
    fn the_delete_tool_removes_a_node_and_a_connection_and_two_undos_restore_them() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        shown(&mut app);
        let a = first(&app);
        // Two more nodes: map px (1034, 1019) and (1044, 1019).
        app.run(Action::NodeTool);
        app.click(1034.0, 1019.0, false);
        app.click(1044.0, 1019.0, false);
        let doc = app.doc.as_mut().expect("doc");
        let [_, b, c] = doc.config.network.order()[..] else {
            panic!("three nodes");
        };
        for (from, to) in [(a, b), (b, c)] {
            let connect = Connect::new(from, to, ConnectionShape::Dual);
            doc.apply(Transaction::new(vec![Command::Connect(connect)]))
                .expect("connects");
        }
        let before = snapshot(&app);
        app.set_tool(Some(Box::new(tools::delete::DeleteTool)));
        // The click itself does not hit-test: the view's hover decides.
        app.view.hovered_edge = Some((b, a));
        app.click(0.0, 0.0, false);
        app.view.hovered_edge = None;
        app.view.hovered_node = Some(c);
        app.click(0.0, 0.0, false);
        let net = &app.doc.as_ref().expect("doc").config.network;
        assert_eq!(net.order(), [a, b]);
        assert_eq!(net.connection_kind(a, b), ConnectionKind::None);
        app.view.hovered_node = None;
        app.click(0.0, 0.0, false);
        assert_eq!(nodes(&app), 2, "an empty click does nothing");
        app.run(Action::Undo);
        app.run(Action::Undo);
        assert_eq!(snapshot(&app), before);
        assert!(app.error.is_none());
    }

    fn active(app: &App) -> Option<&'static str> {
        app.tool.as_ref().map(|t| t.name())
    }

    #[test]
    fn l_selects_and_cycles_the_connect_tools() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        app.run(Action::NodeTool);
        for name in ["Connect", "Dual", "Reverse", "Connect"] {
            app.run(Action::ConnectTool);
            assert_eq!(active(&app), Some(name));
        }
    }

    #[test]
    fn semicolon_toggles_continuous_for_the_session_and_the_active_tool() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        shown(&mut app);
        assert!(!app.continuous);
        app.run(Action::ToggleContinuous);
        assert!(app.continuous);
        // A connect tool picked now starts continuous; toggling reaches the active one.
        app.run(Action::NodeTool);
        app.click(1034.0, 1019.0, false);
        app.click(1044.0, 1019.0, false);
        let [a, b, c] = app.doc.as_ref().expect("doc").config.network.order()[..] else {
            panic!("three nodes");
        };
        app.run(Action::ConnectTool);
        for id in [a, b, c] {
            app.view.hovered_node = Some(id);
            app.click(0.0, 0.0, false);
        }
        let net = &app.doc.as_ref().expect("doc").config.network;
        assert_eq!(net.connection_kind(a, b), ConnectionKind::Regular);
        assert_eq!(net.connection_kind(b, c), ConnectionKind::Regular);
        app.run(Action::ToggleContinuous);
        assert!(!app.continuous);
        app.view.hovered_node = Some(a);
        app.click(0.0, 0.0, false);
        // c to a is about 20 units: a line through one new node.
        let net = &app.doc.as_ref().expect("doc").config.network;
        let mid = net.order()[3];
        assert_eq!(net.connection_kind(c, mid), ConnectionKind::Regular);
        assert_eq!(net.connection_kind(mid, a), ConnectionKind::Regular);
        // Not continuous any more: the next click picks a new start, nothing commits.
        let before = snapshot(&app);
        app.view.hovered_node = Some(b);
        app.click(0.0, 0.0, false);
        assert_eq!(snapshot(&app), before);
    }

    #[test]
    fn p_swaps_the_clicked_node_and_undo_restores_it() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        shown(&mut app);
        app.run(Action::PriorityTool);
        assert_eq!(active(&app), Some("Priority"));
        let id = first(&app);
        let flag = |app: &App| {
            app.doc
                .as_ref()
                .and_then(|d| d.config.network.node(id))
                .map(|n| n.flag)
        };
        let before = flag(&app);
        app.view.hovered_node = Some(id);
        app.click(0.0, 0.0, false);
        assert_ne!(flag(&app), before);
        app.run(Action::Undo);
        assert_eq!(flag(&app), before);
    }

    #[test]
    fn f_flips_the_clicked_connection_and_undo_restores_it() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        shown(&mut app);
        app.run(Action::NodeTool);
        app.click(1034.0, 1019.0, false);
        let [a, b] = app.doc.as_ref().expect("doc").config.network.order()[..] else {
            panic!("two nodes");
        };
        app.run(Action::ConnectTool);
        for id in [a, b] {
            app.view.hovered_node = Some(id);
            app.click(0.0, 0.0, false);
        }
        let kind = |app: &App, x, y| {
            app.doc
                .as_ref()
                .expect("doc")
                .config
                .network
                .connection_kind(x, y)
        };
        assert_eq!(kind(&app, a, b), ConnectionKind::Regular);
        app.run(Action::FlipTool);
        assert_eq!(active(&app), Some("Flip"));
        app.view.hovered_node = None;
        app.view.hovered_edge = Some((a, b));
        app.click(0.0, 0.0, false);
        assert_eq!(kind(&app, b, a), ConnectionKind::Regular);
        assert_eq!(kind(&app, a, b), ConnectionKind::None);
        app.run(Action::Undo);
        assert_eq!(kind(&app, a, b), ConnectionKind::Regular);
        assert_eq!(kind(&app, b, a), ConnectionKind::None);
    }

    #[test]
    fn read_only_document_ignores_the_flip_tool() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<markerID>1</markerID>");
        app.run(Action::FlipTool);
        assert!(app.tool.is_none());
    }

    #[test]
    fn read_only_document_ignores_the_priority_tool() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<markerID>1</markerID>");
        app.run(Action::PriorityTool);
        assert!(app.tool.is_none());
        app.tool = Some(Box::new(tools::priority::PriorityTool));
        app.view.hovered_node = Some(first(&app));
        app.click(0.0, 0.0, false);
        assert!(!app.doc.as_ref().expect("doc").history.can_undo());
        assert!(app.error.is_none());
    }

    #[test]
    fn read_only_document_ignores_the_connect_tools() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<markerID>1</markerID>");
        app.run(Action::ConnectTool);
        assert!(app.tool.is_none());
        let line = tools::connect::LineOptions {
            node_distance: 1.0,
            end_node: true,
        };
        app.tool = Some(Box::new(tools::connect::ConnectTool::new(
            ConnectionShape::OneWay,
            false,
            line,
        )));
        app.view.hovered_node = Some(first(&app));
        app.click(0.0, 0.0, false);
        // Not even a line to a new end node on empty map.
        app.view.hovered_node = None;
        app.click(500.0, 500.0, false);
        assert!(!app.doc.as_ref().expect("doc").history.can_undo());
        assert!(app.error.is_none());
    }

    #[test]
    fn a_continuous_line_to_a_new_end_node_starts_the_next_line_there() {
        let mut settings = Settings::default();
        settings.global.create_line_end_node = true;
        let mut app = App::new(settings);
        load(&mut app, "<version>3</version>");
        app.run(Action::ToggleContinuous);
        app.run(Action::ConnectTool);
        let a = first(&app);
        // The node is world (1, 2), map px (1025, 1026); each line runs 36 units: 3 nodes.
        app.view.hovered_node = Some(a);
        app.click(1025.0, 1026.0, false);
        app.view.hovered_node = None;
        app.click(1061.0, 1026.0, false);
        app.click(1061.0, 1062.0, false);
        assert!(app.error.is_none());
        let net = &app.doc.as_ref().expect("doc").config.network;
        let order = net.order().to_vec();
        assert_eq!(order.len(), 7);
        for pair in order.windows(2) {
            assert_eq!(
                net.connection_kind(pair[0], pair[1]),
                ConnectionKind::Regular
            );
        }
        let end = net.node(order[6]).expect("end");
        assert_eq!((end.x, end.z), (37.0, 38.0));
        app.run(Action::Undo);
        assert_eq!(nodes(&app), 4);
        app.run(Action::Undo);
        assert_eq!(nodes(&app), 1);
    }

    #[test]
    fn connect_reshape_and_remove_each_undo_in_one_step() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        shown(&mut app);
        app.run(Action::NodeTool);
        app.click(1034.0, 1019.0, false);
        let [a, b] = app.doc.as_ref().expect("doc").config.network.order()[..] else {
            panic!("two nodes");
        };
        let connect = |app: &mut App, action_steps: usize| {
            for _ in 0..action_steps {
                app.run(Action::ConnectTool);
            }
            for id in [a, b] {
                app.view.hovered_node = Some(id);
                app.click(0.0, 0.0, false);
            }
        };
        let kind = |app: &App| {
            let net = &app.doc.as_ref().expect("doc").config.network;
            (net.connection_kind(a, b), net.connection_kind(b, a))
        };
        // Regular a -> b, then dual (reshape), then dual again (remove).
        connect(&mut app, 1);
        assert_eq!(kind(&app), (ConnectionKind::Regular, ConnectionKind::None));
        let regular = snapshot(&app);
        connect(&mut app, 1);
        assert_eq!(active(&app), Some("Dual"));
        assert_eq!(kind(&app), (ConnectionKind::Dual, ConnectionKind::Dual));
        let dual = snapshot(&app);
        connect(&mut app, 0);
        assert_eq!(kind(&app), (ConnectionKind::None, ConnectionKind::None));
        app.run(Action::Undo);
        assert_eq!(snapshot(&app), dual);
        app.run(Action::Undo);
        assert_eq!(snapshot(&app), regular);
        // A pending start is dropped by Undo.
        app.view.hovered_node = Some(a);
        app.click(0.0, 0.0, false);
        app.run(Action::Undo);
        assert_eq!(kind(&app), (ConnectionKind::None, ConnectionKind::None));
        app.view.hovered_node = Some(b);
        app.click(0.0, 0.0, false);
        assert_eq!(kind(&app), (ConnectionKind::None, ConnectionKind::None));
        assert!(app.error.is_none());
    }

    /// Adds one node through the node tool: one history entry.
    fn edit(app: &mut App) {
        app.run(Action::NodeTool);
        app.click(1034.0, 1019.0, false);
        app.set_tool(None);
    }

    fn title(app: &App) -> String {
        window_title(app.doc.as_ref())
    }

    #[test]
    fn the_title_shows_the_path_and_a_star_while_dirty() {
        let mut app = App::new(Settings::default());
        assert_eq!(title(&app), "AutoDrive Course Editor");
        load(&mut app, "<version>3</version>");
        assert_eq!(title(&app), "AutoDrive Course Editor - a.xml");
        edit(&mut app);
        assert_eq!(title(&app), "AutoDrive Course Editor - a.xml *");
        app.run(Action::Undo);
        assert_eq!(title(&app), "AutoDrive Course Editor - a.xml");
        app.run(Action::Redo);
        let doc = app.doc.as_ref().expect("doc");
        let saved = JobResult::Saved {
            epoch: doc.epoch,
            point: doc.history.save_point(),
            target: PathBuf::from("b.xml"),
            result: Ok(()),
            warning: None,
        };
        app.tx.send(saved).expect("send");
        app.poll_jobs();
        assert_eq!(title(&app), "AutoDrive Course Editor - b.xml");
    }

    #[test]
    fn closing_a_clean_document_needs_no_prompt() {
        let ctx = egui::Context::default();
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        app.request(Pending::Close, &ctx);
        assert!(app.prompt.is_none());
        assert!(app.allow_close);
    }

    #[test]
    fn closing_a_dirty_document_asks_and_cancel_keeps_everything() {
        let ctx = egui::Context::default();
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        edit(&mut app);
        let before = snapshot(&app);
        app.request(Pending::Close, &ctx);
        assert_eq!(app.prompt, Some(Pending::Close));
        assert!(!app.allow_close);
        app.choose(Choice::Cancel, &ctx);
        assert!(app.prompt.is_none() && app.pending.is_none());
        assert!(!app.allow_close);
        assert_eq!(snapshot(&app), before);
        assert!(app.dirty());
    }

    #[test]
    fn discard_closes_a_dirty_document() {
        let ctx = egui::Context::default();
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        edit(&mut app);
        app.request(Pending::Close, &ctx);
        app.choose(Choice::Discard, &ctx);
        assert!(app.prompt.is_none());
        assert!(app.allow_close);
    }

    #[test]
    fn opening_over_a_dirty_document_asks_first() {
        let ctx = egui::Context::default();
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        edit(&mut app);
        app.request(Pending::Open, &ctx);
        assert_eq!(app.prompt, Some(Pending::Open));
        app.choose(Choice::Cancel, &ctx);
        assert!(app.prompt.is_none() && !app.loading);
    }

    #[test]
    fn a_dirty_read_only_document_asks_and_save_does_nothing() {
        let ctx = egui::Context::default();
        let mut app = App::new(Settings::default());
        load(&mut app, "<markerID>1</markerID>");
        // A read-only load resets no flags, so force the dirty state the prompt guards.
        app.doc.as_mut().expect("doc").history.mark_unsaved();
        app.request(Pending::Close, &ctx);
        assert_eq!(app.prompt, Some(Pending::Close));
        // Save is not offered; even if chosen, no save starts and nothing waits for it.
        app.choose(Choice::Save, &ctx);
        assert!(!app.saving && app.pending.is_none() && !app.allow_close);
    }

    #[test]
    fn a_failed_save_clears_the_pending_close() {
        let ctx = egui::Context::default();
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        edit(&mut app);
        // As if Save in the prompt had started a save.
        app.pending = Some(Pending::Close);
        app.saving = true;
        app.resume(&ctx);
        assert_eq!(app.pending, Some(Pending::Close), "waits for the save");
        let doc = app.doc.as_ref().expect("doc");
        let failed = JobResult::Saved {
            epoch: doc.epoch,
            point: doc.history.save_point(),
            target: PathBuf::from("b.xml"),
            result: Err("disk full".to_owned()),
            warning: None,
        };
        app.tx.send(failed).expect("send");
        app.poll_jobs();
        app.resume(&ctx);
        assert!(app.pending.is_none() && app.prompt.is_none());
        assert!(!app.allow_close);
        assert!(app.error.is_some() && app.dirty());
    }

    #[test]
    fn a_successful_save_continues_the_close() {
        let ctx = egui::Context::default();
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        edit(&mut app);
        app.pending = Some(Pending::Close);
        app.saving = true;
        let doc = app.doc.as_ref().expect("doc");
        let saved = JobResult::Saved {
            epoch: doc.epoch,
            point: doc.history.save_point(),
            target: PathBuf::from("b.xml"),
            result: Ok(()),
            warning: None,
        };
        app.tx.send(saved).expect("send");
        app.poll_jobs();
        app.resume(&ctx);
        assert!(app.pending.is_none());
        assert!(app.allow_close);
    }

    #[test]
    fn a_vehicles_warning_keeps_the_window_but_the_save_counts() {
        let ctx = egui::Context::default();
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        edit(&mut app);
        app.pending = Some(Pending::Close);
        app.saving = true;
        let doc = app.doc.as_ref().expect("doc");
        let saved = JobResult::Saved {
            epoch: doc.epoch,
            point: doc.history.save_point(),
            target: PathBuf::from("b.xml"),
            result: Ok(()),
            warning: Some("vehicles.xml was not saved".to_owned()),
        };
        app.tx.send(saved).expect("send");
        app.poll_jobs();
        app.resume(&ctx);
        assert!(app.pending.is_none() && !app.allow_close);
        assert_eq!(app.error.as_deref(), Some("vehicles.xml was not saved"));
        assert!(!app.dirty());
    }

    #[test]
    fn cancel_drops_a_close_waiting_for_a_running_save() {
        let ctx = egui::Context::default();
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        edit(&mut app);
        // Save was picked earlier; the save runs, and a second close is answered Cancel.
        app.pending = Some(Pending::Close);
        app.saving = true;
        app.request(Pending::Close, &ctx);
        app.choose(Choice::Cancel, &ctx);
        let doc = app.doc.as_ref().expect("doc");
        let saved = JobResult::Saved {
            epoch: doc.epoch,
            point: doc.history.save_point(),
            target: PathBuf::from("b.xml"),
            result: Ok(()),
            warning: None,
        };
        app.tx.send(saved).expect("send");
        app.poll_jobs();
        app.resume(&ctx);
        assert!(app.pending.is_none() && app.prompt.is_none());
        assert!(!app.allow_close);
    }

    /// Runs one frame with a close request for the root viewport through `intercept_close`;
    /// returns whether the frame sent `CancelClose`.
    fn close_frame(app: &mut App) -> bool {
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .events
            .push(egui::ViewportEvent::Close);
        let mut output = ctx.run_ui(input, |ui| app.intercept_close(ui.ctx()));
        // No painter applies the font texture here; epaint asserts it was handled.
        output.textures_delta.clear();
        output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .is_some_and(|v| v.commands.contains(&egui::ViewportCommand::CancelClose))
    }

    #[test]
    fn a_window_close_is_cancelled_only_while_dirty() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        assert!(!close_frame(&mut app));
        assert!(app.prompt.is_none());
        edit(&mut app);
        assert!(close_frame(&mut app));
        assert_eq!(app.prompt, Some(Pending::Close));
    }

    #[test]
    fn snap_toggles_with_s() {
        let mut app = App::new(Settings::default());
        assert!(!app.snap);
        app.run(Action::ToggleSnap);
        assert!(app.snap);
    }

    /// The loaded node `a` plus `b` and an outside node `c`: a -> b one-way, b -> c
    /// one-way, a marker "Hof" on b; a and b selected. Returns [a, b, c].
    fn group(app: &mut App) -> [NodeId; 3] {
        let a = first(app);
        let doc = app.doc.as_mut().expect("doc");
        let net = &mut doc.config.network;
        let b = net.add_node(5.0, 3.0, 2.0, NodeFlag::SUB_PRIO);
        let c = net.add_node(-9.0, 1.0, 4.0, NodeFlag::REGULAR);
        net.connect(a, b, ConnectionShape::OneWay).expect("ok");
        net.connect(b, c, ConnectionShape::OneWay).expect("ok");
        net.push_marker(ad_core::network::Marker {
            node: b,
            name: "Hof".into(),
            group: "All".into(),
            parked_vehicles: Vec::new(),
        });
        doc.selection.extend([a, b]);
        [a, b, c]
    }

    fn selection(app: &App) -> Vec<NodeId> {
        let doc = app.doc.as_ref().expect("doc");
        doc.selection.iter().copied().collect()
    }

    fn marker_names(app: &App) -> Vec<String> {
        let net = &app.doc.as_ref().expect("doc").config.network;
        net.markers().iter().map(|m| m.name.clone()).collect()
    }

    #[test]
    fn copy_keeps_the_selection_and_paste_selects_the_copy_centred_on_the_view() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        shown(&mut app);
        let [a, b, _] = group(&mut app);
        app.run(Action::Copy);
        assert_eq!(selection(&app), [a, b]);
        assert!(app.copied.is_some());
        // Before the first frame the view shows the map's centre, world (0, 0).
        app.run(Action::Paste);
        let net = &app.doc.as_ref().expect("doc").config.network;
        let new = selection(&app);
        assert_eq!(new.len(), 2);
        assert_eq!(net.len(), 5);
        // Box x 1..5, z 2..2: centre (3, 2).
        let pos = |id| net.node(id).map(|n| (n.x, n.y, n.z)).expect("node");
        assert_eq!(pos(new[0]), (-2.0, 0.0, 0.0));
        assert_eq!(pos(new[1]), (2.0, 3.0, 0.0));
        assert_eq!(net.connection_kind(new[0], new[1]), ConnectionKind::Regular);
        assert_eq!(net.node(new[1]).expect("b").outgoing(), &[] as &[NodeId]);
        assert_eq!(marker_names(&app), ["Hof", "Hof_2"]);
        app.run(Action::PasteInPlace);
        let net = &app.doc.as_ref().expect("doc").config.network;
        let new = selection(&app);
        let pos = |id| net.node(id).map(|n| (n.x, n.z)).expect("node");
        assert_eq!((pos(new[0]), pos(new[1])), ((1.0, 2.0), (5.0, 2.0)));
        assert_eq!(marker_names(&app), ["Hof", "Hof_2", "Hof_3"]);
        assert!(app.error.is_none());
    }

    #[test]
    fn undo_of_a_paste_restores_the_prior_selection_and_redo_reselects_the_same_ids() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        shown(&mut app);
        let [a, b, _] = group(&mut app);
        let before = snapshot(&app);
        app.run(Action::Copy);
        app.run(Action::Paste);
        let pasted = selection(&app);
        let after = snapshot(&app);
        app.run(Action::Undo);
        assert_eq!(snapshot(&app), before);
        assert_eq!(selection(&app), [a, b]);
        app.run(Action::Redo);
        assert_eq!(snapshot(&app), after);
        assert_eq!(selection(&app), pasted);
    }

    #[test]
    fn cut_deletes_the_group_in_one_step_and_undo_restores_it_with_the_selection() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        shown(&mut app);
        let [a, b, c] = group(&mut app);
        let before = snapshot(&app);
        app.run(Action::Cut);
        let doc = app.doc.as_ref().expect("doc");
        assert_eq!(doc.config.network.order(), [c]);
        assert!(doc.selection.is_empty());
        assert!(app.clipboard.is_some() && app.copied.is_some());
        app.run(Action::Undo);
        assert_eq!(snapshot(&app), before);
        assert_eq!(selection(&app), [a, b]);
        assert!(!app.doc.as_ref().expect("doc").history.can_undo());
        app.run(Action::Redo);
        assert!(selection(&app).is_empty());
        // Pasting the cut group keeps its free marker name.
        app.run(Action::PasteInPlace);
        assert_eq!(marker_names(&app), ["Hof"]);
        assert_eq!(nodes(&app), 3);
    }

    #[test]
    fn the_clipboard_survives_opening_another_file() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        shown(&mut app);
        group(&mut app);
        app.run(Action::Copy);
        load(&mut app, "<version>3</version>");
        assert_eq!(nodes(&app), 1);
        app.run(Action::Paste);
        assert_eq!(nodes(&app), 3);
        assert_eq!(selection(&app).len(), 2);
        assert_eq!(marker_names(&app), ["Hof"]);
    }

    #[test]
    fn nothing_without_a_selection_or_a_clipboard() {
        let mut app = App::new(Settings::default());
        for action in [
            Action::Copy,
            Action::Cut,
            Action::Paste,
            Action::PasteInPlace,
        ] {
            app.run(action);
        }
        load(&mut app, "<version>3</version>");
        shown(&mut app);
        for action in [
            Action::Paste,
            Action::PasteInPlace,
            Action::Copy,
            Action::Cut,
        ] {
            app.run(action);
        }
        assert!(app.clipboard.is_none() && app.copied.is_none());
        assert_eq!(nodes(&app), 1);
        assert!(!app.doc.as_ref().expect("doc").history.can_undo());
        // An empty selection keeps an earlier clipboard.
        group(&mut app);
        app.run(Action::Copy);
        let clip = app.clipboard.clone();
        app.doc.as_mut().expect("doc").selection.clear();
        app.run(Action::Copy);
        app.run(Action::Cut);
        assert_eq!(app.clipboard, clip);
        assert_eq!(nodes(&app), 3);
    }

    #[test]
    fn read_only_document_copies_but_does_not_cut_or_paste() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<markerID>1</markerID>");
        shown(&mut app);
        let id = first(&app);
        app.doc.as_mut().expect("doc").selection.insert(id);
        app.run(Action::Cut);
        assert!(app.clipboard.is_none());
        app.run(Action::Copy);
        assert!(app.clipboard.is_some());
        app.run(Action::Paste);
        app.run(Action::PasteInPlace);
        assert_eq!(nodes(&app), 1);
        assert!(app.error.is_none());
    }

    #[test]
    fn copy_cut_and_paste_do_nothing_while_a_selection_is_drawn() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        shown(&mut app);
        group(&mut app);
        app.view.start_select((1000.0, 1000.0));
        for action in [Action::Copy, Action::Cut] {
            app.run(action);
        }
        assert!(app.clipboard.is_none());
        app.view.reset();
        app.run(Action::Copy);
        app.view.start_select((1000.0, 1000.0));
        app.run(Action::Paste);
        app.run(Action::PasteInPlace);
        assert_eq!(nodes(&app), 3);
        assert!(!app.doc.as_ref().expect("doc").history.can_undo());
    }

    #[test]
    fn paste_and_cut_cancel_the_tool_preview() {
        for action in [Action::Paste, Action::Cut] {
            let mut app = App::new(Settings::default());
            load(&mut app, "<version>3</version>");
            shown(&mut app);
            let [a, ..] = group(&mut app);
            app.run(Action::Copy);
            app.run(Action::MoveTool);
            let start = DragEvent::Start {
                at: (1025.0, 1026.0),
                node: Some(a),
            };
            drag(&mut app, start, false);
            drag(
                &mut app,
                DragEvent::Move {
                    at: (1100.0, 1100.0),
                },
                false,
            );
            app.run(action);
            assert!(
                app.tool.as_ref().is_some_and(|t| t.preview().is_none()),
                "{action:?}"
            );
        }
    }

    /// One frame of `events` through the shortcut handler; returns the text it put on the
    /// OS clipboard, if any.
    fn clipboard_frame(app: &mut App, events: Vec<egui::Event>) -> Option<String> {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| input::handle_shortcuts(app, ui.ctx()));
        out.textures_delta.clear();
        out.platform_output.commands.iter().find_map(|c| match c {
            egui::OutputCommand::CopyText(text) => Some(text.clone()),
            _ => None,
        })
    }

    #[test]
    fn clipboard_events_copy_with_a_note_on_the_os_clipboard_and_paste() {
        let mut app = App::new(Settings::default());
        load(&mut app, "<version>3</version>");
        shown(&mut app);
        group(&mut app);
        let note = clipboard_frame(&mut app, vec![egui::Event::Copy]);
        assert!(note.is_some_and(|t| t.contains("2 nodes")));
        assert!(clipboard_frame(&mut app, vec![egui::Event::Paste("x".into())]).is_none());
        assert_eq!(nodes(&app), 5);
        assert_eq!(app.doc.as_ref().expect("doc").selection.len(), 2);
    }

    #[test]
    fn clipboard_events_do_nothing_while_the_edit_location_dialog_is_shown() {
        let mut app = edit_location("<version>3</version>", true);
        assert!(app.location.is_some());
        let id = first(&app);
        app.doc.as_mut().expect("doc").selection.insert(id);
        let events = vec![egui::Event::Copy, egui::Event::Cut];
        assert!(clipboard_frame(&mut app, events).is_none());
        assert!(app.clipboard.is_none());
        assert_eq!(nodes(&app), 1);
    }

    /// The FS25 fixture as the open document.
    fn load_fixture(app: &mut App) {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../AutoDrive_config.xml");
        let bytes = std::fs::read(&path).expect("fixture");
        let config = config_xml::load(&bytes).expect("loads");
        let result = JobResult::Loaded {
            path,
            result: Ok(Box::new((config, map_image::blank(), None, None, None))),
        };
        app.tx.send(result).expect("send");
        app.poll_jobs();
    }

    #[test]
    fn a_junction_of_the_fixture_pastes_twice_and_two_undos_remove_both() {
        let mut app = App::new(Settings::default());
        load_fixture(&mut app);
        shown(&mut app);
        let doc = app.doc.as_mut().expect("doc");
        let net = &doc.config.network;
        // A junction: a node with three or more distinct linked nodes, selected with them.
        let junction: BTreeSet<NodeId> = net
            .order()
            .iter()
            .filter_map(|id| net.node(*id).map(|n| (*id, n)))
            .map(|(id, n)| {
                let mut s: BTreeSet<NodeId> =
                    n.outgoing().iter().chain(n.incoming()).copied().collect();
                s.insert(id);
                s
            })
            .find(|s| s.len() >= 4)
            .expect("fixture has a junction");
        let inner = |net: &ad_core::network::Network, ids: &BTreeSet<NodeId>| -> usize {
            ids.iter()
                .filter_map(|id| net.node(*id))
                .map(|n| n.outgoing().iter().filter(|t| ids.contains(t)).count())
                .sum()
        };
        let links = inner(net, &junction);
        assert!(links >= 3);
        doc.selection = junction.clone();
        let before = snapshot(&app);
        app.run(Action::Copy);
        app.run(Action::Paste);
        let first_copy = app.doc.as_ref().expect("doc").selection.clone();
        app.run(Action::Paste);
        let doc = app.doc.as_ref().expect("doc");
        let second = doc.selection.clone();
        assert_eq!(first_copy.len(), junction.len());
        assert_eq!(second.len(), junction.len());
        assert!(first_copy.is_disjoint(&second) && first_copy.is_disjoint(&junction));
        assert_eq!(inner(&doc.config.network, &first_copy), links);
        assert_eq!(inner(&doc.config.network, &second), links);
        assert!(app.error.is_none());
        app.run(Action::Undo);
        app.run(Action::Undo);
        assert_eq!(snapshot(&app), before);
        assert_eq!(app.doc.as_ref().expect("doc").selection, junction);
    }
}

#[cfg(test)]
mod import_tests {
    #![allow(clippy::expect_used)]

    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use ad_io::config_xml::{self, GameVersion};
    use ad_io::map_image;

    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ad_app_import_{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        dir
    }

    fn text(version: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"no\"?>\n<AutoDrive>\n    \
             {version}\n    <waypoints>\n        <id>1</id>\n        <x>1.000</x>\n        \
             <y>0.000</y>\n        <z>2.000</z>\n        <out>-1</out>\n        \
             <incoming>-1</incoming>\n        <flags>0</flags>\n    </waypoints>\n</AutoDrive>\n"
        )
    }

    /// An app with a one-node config of `version`, its image already uploaded (taken).
    fn app(version: &str) -> App {
        let mut app = App::new(Settings::default());
        let config = config_xml::load(text(version).as_bytes()).expect("loads");
        app.tx
            .send(JobResult::Loaded {
                path: PathBuf::from("a.xml"),
                result: Ok(Box::new((config, map_image::blank(), None, None, None))),
            })
            .expect("send");
        app.poll_jobs();
        app.doc.as_mut().expect("doc").map_image = None;
        app
    }

    /// Writes a DDS of `rgba` to `dir` and imports it into `app` towards `target`.
    fn start(app: &mut App, dir: &Path, rgba: [u8; 4], target: &Path) {
        let dds = dir.join("map.dds");
        std::fs::write(&dds, jobs::test_dds(rgba)).expect("write");
        app.start_import(dds, GameVersion::FS25, target.to_path_buf(), || {});
        assert!(app.importing);
    }

    /// Waits for the next job result and applies it as a frame would.
    fn finish(app: &mut App) {
        let result = app.rx.recv_timeout(Duration::from_secs(60)).expect("job");
        app.tx.send(result).expect("send");
        app.poll_jobs();
    }

    fn shown(app: &App) -> Option<[u8; 4]> {
        let image = app.doc.as_ref().expect("doc").map_image.as_ref()?;
        Some([image.rgba[0], image.rgba[1], image.rgba[2], image.rgba[3]])
    }

    fn unchanged_data(app: &App) {
        let doc = app.doc.as_ref().expect("doc");
        assert!(!doc.is_dirty() && !doc.history.can_undo());
        assert_eq!(doc.config.network.len(), 1);
    }

    #[test]
    fn an_import_shows_the_image_and_saves_it() {
        let dir = temp_dir("new");
        let target = map_image::map_image_target(&dir, "M");
        let mut app = app("<version>3</version>");
        start(&mut app, &dir, [1, 2, 3, 255], &target);
        finish(&mut app);
        let saved = map_image::load_map_image(&target);
        std::fs::remove_dir_all(&dir).expect("cleanup");
        assert!(!app.importing && app.error.is_none());
        assert_eq!(shown(&app), Some([1, 2, 3, 255]));
        assert!(!app.dialog_shown());
        assert_eq!(saved.1, None);
        assert_eq!(&saved.0.rgba[..4], &[1, 2, 3, 255]);
        unchanged_data(&app);
    }

    #[test]
    fn an_existing_image_is_replaced_only_on_yes() {
        for replace in [true, false] {
            let dir = temp_dir(&format!("exists_{replace}"));
            let target = map_image::map_image_target(&dir, "M");
            std::fs::create_dir_all(target.parent().expect("dir")).expect("dir");
            std::fs::write(&target, b"old").expect("write");
            let mut app = app("<version>3</version>");
            start(&mut app, &dir, [4, 5, 6, 255], &target);
            finish(&mut app);
            // Shown while asking; nothing written yet.
            assert_eq!(shown(&app), Some([4, 5, 6, 255]));
            assert!(app.dialog_shown());
            assert_eq!(std::fs::read(&target).expect("read"), b"old");
            app.answer_replace(replace, || {});
            assert!(!app.dialog_shown());
            if replace {
                finish(&mut app);
            }
            let after = std::fs::read(&target).expect("read");
            std::fs::remove_dir_all(&dir).expect("cleanup");
            assert!(app.error.is_none());
            assert_eq!(after != b"old", replace, "replace {replace}");
            assert_eq!(shown(&app), Some([4, 5, 6, 255]));
            unchanged_data(&app);
        }
    }

    #[test]
    fn a_bad_file_shows_an_error_and_keeps_the_image() {
        let dir = temp_dir("bad");
        let target = map_image::map_image_target(&dir, "M");
        let mut app = app("<version>3</version>");
        let dds = dir.join("broken.dds");
        std::fs::write(&dds, &jobs::test_dds([1, 2, 3, 255])[..150]).expect("write");
        app.start_import(dds, GameVersion::FS19, target.clone(), || {});
        finish(&mut app);
        let written = target.exists();
        std::fs::remove_dir_all(&dir).expect("cleanup");
        assert!(!app.importing && !written);
        assert!(
            app.error
                .as_deref()
                .is_some_and(|e| e.contains("broken.dds"))
        );
        assert_eq!(shown(&app), None);
        unchanged_data(&app);
    }

    #[test]
    fn a_result_for_a_replaced_document_is_ignored() {
        let dir = temp_dir("replaced");
        let target = map_image::map_image_target(&dir, "M");
        std::fs::create_dir_all(target.parent().expect("dir")).expect("dir");
        std::fs::write(&target, b"old").expect("write");
        let mut app = app("<version>3</version>");
        start(&mut app, &dir, [7, 8, 9, 255], &target);
        // Another file opens before the job ends.
        let result = app.rx.recv_timeout(Duration::from_secs(60)).expect("job");
        let config = config_xml::load(text("<version>3</version>").as_bytes()).expect("loads");
        app.tx
            .send(JobResult::Loaded {
                path: PathBuf::from("b.xml"),
                result: Ok(Box::new((config, map_image::blank(), None, None, None))),
            })
            .expect("send");
        app.poll_jobs();
        app.doc.as_mut().expect("doc").map_image = None;
        app.tx.send(result).expect("send");
        app.poll_jobs();
        std::fs::remove_dir_all(&dir).expect("cleanup");
        assert!(!app.importing && app.error.is_none());
        assert_eq!(shown(&app), None);
        assert!(!app.dialog_shown());
    }

    #[test]
    fn a_read_only_document_takes_an_imported_image() {
        let dir = temp_dir("read_only");
        let target = map_image::map_image_target(&dir, "M");
        let mut app = app("<markerID>1</markerID>");
        assert!(!app.editable());
        start(&mut app, &dir, [1, 2, 3, 255], &target);
        finish(&mut app);
        let written = target.is_file();
        std::fs::remove_dir_all(&dir).expect("cleanup");
        assert!(app.error.is_none() && written);
        assert_eq!(shown(&app), Some([1, 2, 3, 255]));
        unchanged_data(&app);
    }
}

#[cfg(test)]
mod zip_tests {
    #![allow(clippy::expect_used)]

    use std::io::{Cursor, Write};
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use ad_io::config_xml::{self, GameVersion};
    use ad_io::map_image;

    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ad_app_zip_{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        dir
    }

    fn config_text(map_name: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"no\"?>\n<AutoDrive>\n    \
             <version>3</version>\n    <MapName>{map_name}</MapName>\n    <waypoints>\n        \
             <id>1</id>\n        <x>1.000</x>\n        <y>0.000</y>\n        <z>2.000</z>\n        \
             <out>-1</out>\n        <incoming>-1</incoming>\n        <flags>0</flags>\n    \
             </waypoints>\n</AutoDrive>\n"
        )
    }

    fn open(app: &mut App, loaded: jobs::Loaded) {
        app.tx
            .send(JobResult::Loaded {
                path: PathBuf::from("a.xml"),
                result: Ok(Box::new(loaded)),
            })
            .expect("send");
        app.poll_jobs();
        app.doc.as_mut().expect("doc").map_image = None;
    }

    /// An app with a one-node config whose map is `map_name`, its image already taken.
    fn app(map_name: &str) -> App {
        let mut app = App::new(Settings::default());
        let config = config_xml::load(config_text(map_name).as_bytes()).expect("loads");
        open(&mut app, (config, map_image::blank(), None, None, None));
        app
    }

    fn dem() -> Vec<u8> {
        let img = image::ImageBuffer::<image::Luma<u16>, _>::from_pixel(4, 4, image::Luma([25600]));
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png)
            .expect("encode");
        out.into_inner()
    }

    /// An FS25 mod zip (descVersion 92) with titles en "Foo Bar", de "Foo".
    fn zip_bytes(dds: &[u8], dem: Option<&[u8]>) -> Vec<u8> {
        let desc = "<modDesc descVersion=\"92\"><maps><map id=\"M\" configFilename=\"maps/map.xml\">\
                    <title><en>Foo Bar</en><de>Foo</de></title></map></maps></modDesc>";
        let map = "<map imageFilename=\"maps/overview.png\"/>";
        let mut entries: Vec<(&str, &[u8])> = vec![
            ("modDesc.xml", desc.as_bytes()),
            ("maps/map.xml", map.as_bytes()),
            ("maps/overview.dds", dds),
        ];
        if let Some(dem) = dem {
            entries.push(("maps/data/dem.png", dem));
        }
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .expect("start");
            writer.write_all(bytes).expect("write");
        }
        writer.finish().expect("finish").into_inner()
    }

    /// Waits for the next job result and applies it as a frame would.
    fn finish(app: &mut App) {
        let result = app.rx.recv_timeout(Duration::from_secs(120)).expect("job");
        app.tx.send(result).expect("send");
        app.poll_jobs();
    }

    /// Writes the zip to `dir` and reads it into `app`'s preview.
    fn read(app: &mut App, dir: &Path, bytes: &[u8]) {
        let path = dir.join("mod.zip");
        std::fs::write(&path, bytes).expect("write");
        app.start_zip_read(path, || {});
        assert!(app.importing);
        finish(app);
        assert!(!app.importing);
    }

    fn import(app: &mut App, dir: &Path, confirmed: bool) {
        app.zip_import(dir, confirmed, || {});
        if app.importing {
            finish(app);
        }
    }

    fn unchanged_data(app: &App) {
        let doc = app.doc.as_ref().expect("doc");
        assert!(!doc.is_dirty() && !doc.history.can_undo());
        assert_eq!(doc.config.network.len(), 1);
    }

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../AutoDrive_config.xml")
    }

    /// Needs the gitignored `pda_map_H.dds` and `dem.png` in the repo root; skipped when absent.
    #[test]
    fn the_fs25_sample_zip_previews_saves_and_attaches() {
        let root = fixture().with_file_name("");
        let (Ok(dds), Ok(dem)) = (
            std::fs::read(root.join("pda_map_H.dds")),
            std::fs::read(root.join("dem.png")),
        ) else {
            eprintln!("skipped: pda_map_H.dds or dem.png is absent");
            return;
        };
        let dir = temp_dir("fs25");
        let mut app = App::new(Settings::default());
        let loaded = jobs::load_file(&fixture(), None, &Settings::default()).expect("fixture");
        open(&mut app, loaded);
        let nodes = app.doc.as_ref().expect("doc").config.network.len();
        read(&mut app, &dir, &zip_bytes(&dds, Some(&dem)));
        assert!(app.error.is_none(), "{:?}", app.error);
        let preview = app.zip_preview.as_ref().expect("preview");
        assert_eq!(preview.version, GameVersion::FS25);
        assert_eq!(preview.name, "NF_Marsch_4fach");
        assert_eq!(preview.read.centre.dimensions(), (256, 256));
        assert!(preview.read.heightmap.is_some() && preview.read.zip.notice.is_none());
        assert!(app.dialog_shown());
        import(&mut app, &dir, false);
        assert!(
            app.error.is_none() && app.zip_preview.is_none(),
            "{:?}",
            app.error
        );
        let doc = app.doc.as_mut().expect("doc");
        let attached = doc.map_image.take().expect("image attached");
        let heightmap = doc.map.heightmap.clone().expect("heightmap attached");
        assert!(doc.map.height_map_scale >= 1.0);
        assert_eq!(doc.config.network.len(), nodes);
        assert!(!doc.is_dirty() && !doc.history.can_undo());
        // The heightmap is saved byte for byte; both are found again at the next load.
        let name = "NF_Marsch_4fach";
        let saved_dem = std::fs::read(map_image::heightmap_target(&dir, name)).expect("dem");
        let image_path = map_image::find_map_image(name, Some(&dir), None).expect("image");
        let height_path = map_image::find_heightmap(Some(name), Some(&dir), None).expect("hm");
        let (image, notice) = map_image::load_map_image(&image_path);
        let reloaded = map_image::load_heightmap(&height_path).expect("loads");
        let next = jobs::load_file(&fixture(), Some(&dir), &Settings::default()).expect("load");
        std::fs::remove_dir_all(&dir).expect("cleanup");
        assert_eq!(saved_dem, dem);
        assert_eq!((image, notice), (attached.clone(), None));
        assert_eq!(&reloaded, &*heightmap);
        assert_eq!(next.1, attached);
    }

    /// A DDS whose two-pixel border is red and whose centre is green.
    fn ringed() -> Vec<u8> {
        jobs::test_dds_of(8, |x, y| {
            if (2..6).contains(&x) && (2..6).contains(&y) {
                [0, 255, 0, 255]
            } else {
                [255, 0, 0, 255]
            }
        })
    }

    #[test]
    fn the_chosen_game_sets_the_crop() {
        for (version, corner_red) in [(GameVersion::FS19, true), (GameVersion::FS25, false)] {
            let dir = temp_dir(&format!("override_{version:?}"));
            let mut app = app("M");
            read(&mut app, &dir, &zip_bytes(&ringed(), None));
            let preview = app.zip_preview.as_mut().expect("preview");
            assert_eq!(preview.version, GameVersion::FS25);
            preview.version = version;
            import(&mut app, &dir, false);
            let saved = map_image::load_map_image(&map_image::map_image_target(&dir, "M"));
            std::fs::remove_dir_all(&dir).expect("cleanup");
            let (r, g) = (saved.0.rgba[0], saved.0.rgba[1]);
            assert_eq!(r > g, corner_red, "{version:?}: {r} {g}");
            assert!(app.error.is_none());
            unchanged_data(&app);
        }
    }

    #[test]
    fn existing_files_are_replaced_only_after_yes() {
        let dir = temp_dir("exists");
        let targets = [
            map_image::map_image_target(&dir, "M"),
            map_image::heightmap_target(&dir, "M"),
        ];
        std::fs::create_dir_all(targets[0].parent().expect("dir")).expect("dir");
        for target in &targets {
            std::fs::write(target, b"old").expect("write");
        }
        let mut app = app("M");
        let bytes = zip_bytes(&jobs::test_dds([1, 2, 3, 255]), Some(&dem()));
        read(&mut app, &dir, &bytes);
        import(&mut app, &dir, false);
        let preview = app.zip_preview.as_mut().expect("still shown");
        assert_eq!(preview.replace.as_deref(), Some(&targets[..]));
        assert!(!app.importing);
        // No: back to the preview, nothing written.
        preview.replace = None;
        assert!(
            targets
                .iter()
                .all(|t| std::fs::read(t).expect("read") == b"old")
        );
        import(&mut app, &dir, false);
        let preview = app.zip_preview.as_ref().expect("asks again");
        assert!(preview.replace.is_some());
        import(&mut app, &dir, true);
        let after: Vec<_> = targets
            .iter()
            .map(|t| std::fs::read(t).expect("read"))
            .collect();
        std::fs::remove_dir_all(&dir).expect("cleanup");
        assert!(app.zip_preview.is_none() && app.error.is_none());
        assert_eq!(after[1], dem());
        assert_ne!(after[0], b"old");
        let doc = app.doc.as_ref().expect("doc");
        assert!(doc.map_image.is_some() && doc.map.heightmap.is_some());
        unchanged_data(&app);
    }

    #[test]
    fn another_maps_name_or_no_document_saves_without_attaching() {
        let dir = temp_dir("other");
        let mut app = app("Other");
        read(
            &mut app,
            &dir,
            &zip_bytes(&jobs::test_dds([1, 2, 3, 255]), None),
        );
        let preview = app.zip_preview.as_mut().expect("preview");
        assert_eq!(preview.name, "Other");
        assert!(preview.read.zip.notice.is_some() && preview.read.heightmap.is_none());
        preview.name = "Foo Bar".to_owned();
        import(&mut app, &dir, false);
        let folder = map_image::map_image_target(&dir, "Foo_Bar").with_file_name("");
        let image = map_image::map_image_target(&dir, "Foo_Bar").is_file();
        let height = map_image::heightmap_target(&dir, "Foo_Bar").exists();
        assert!(image && !height && app.error.is_none());
        let notice = app
            .doc
            .as_ref()
            .and_then(|d| d.notice.clone())
            .unwrap_or_default();
        assert!(notice.contains("\"Foo_Bar\" saved"), "{notice}");
        let doc = app.doc.as_ref().expect("doc");
        assert!(doc.map_image.is_none() && doc.map.heightmap.is_none());
        unchanged_data(&app);
        // Without a document the name is the English title.
        std::fs::remove_dir_all(&folder).expect("clear");
        let mut app = App::new(Settings::default());
        let bytes = zip_bytes(&jobs::test_dds([1, 2, 3, 255]), Some(&dem()));
        read(&mut app, &dir, &bytes);
        assert_eq!(app.zip_preview.as_ref().expect("preview").name, "Foo Bar");
        import(&mut app, &dir, false);
        let both = map_image::map_image_target(&dir, "Foo_Bar").is_file()
            && map_image::heightmap_target(&dir, "Foo_Bar").is_file();
        std::fs::remove_dir_all(&dir).expect("cleanup");
        assert!(both && app.doc.is_none());
        let message = app.error.clone().unwrap_or_default();
        assert!(message.contains("\"Foo_Bar\" saved"), "{message}");
    }

    #[test]
    fn results_for_a_replaced_document_are_ignored() {
        let dir = temp_dir("replaced");
        let bytes = zip_bytes(&jobs::test_dds([1, 2, 3, 255]), Some(&dem()));
        let path = dir.join("mod.zip");
        std::fs::write(&path, &bytes).expect("write");
        // The read: another config opens before it ends.
        let mut app = app("M");
        app.start_zip_read(path, || {});
        let result = app.rx.recv_timeout(Duration::from_secs(60)).expect("job");
        let config = config_xml::load(config_text("M").as_bytes()).expect("loads");
        open(&mut app, (config, map_image::blank(), None, None, None));
        app.tx.send(result).expect("send");
        app.poll_jobs();
        assert!(app.zip_preview.is_none() && app.error.is_none() && !app.importing);
        // The save: the files are written, but not attached to the new document.
        read(&mut app, &dir, &bytes);
        app.zip_import(&dir, false, || {});
        let result = app.rx.recv_timeout(Duration::from_secs(60)).expect("job");
        let config = config_xml::load(config_text("M").as_bytes()).expect("loads");
        open(&mut app, (config, map_image::blank(), None, None, None));
        app.tx.send(result).expect("send");
        app.poll_jobs();
        let saved = map_image::map_image_target(&dir, "M").is_file();
        std::fs::remove_dir_all(&dir).expect("cleanup");
        assert!(saved && app.error.is_none() && !app.importing);
        let doc = app.doc.as_ref().expect("doc");
        assert!(doc.map_image.is_none() && doc.map.heightmap.is_none());
    }

    #[test]
    fn a_bad_zip_shows_an_error_and_changes_nothing() {
        let dir = temp_dir("bad");
        let mut app = app("M");
        read(&mut app, &dir, b"not a zip");
        let written = dir.join("mapImages").exists();
        std::fs::remove_dir_all(&dir).expect("cleanup");
        assert!(app.zip_preview.is_none() && !written);
        assert!(app.error.as_deref().is_some_and(|e| e.contains("mod.zip")));
        assert!(app.doc.as_ref().expect("doc").map_image.is_none());
        unchanged_data(&app);
    }
}
