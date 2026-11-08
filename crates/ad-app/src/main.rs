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
                JobResult::Saved { result, .. } => {
                    self.saving = false;
                    // A failed save keeps the document and the window.
                    if result.is_err() {
                        self.pending = None;
                    }
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
            // A new document cancels the active tool (AD-18) and the dialog of an old node.
            if self.doc.as_ref().map(|d| d.epoch) != before {
                self.set_tool(None);
                self.location = None;
            }
        }
    }

    /// Makes `tool` the active tool; the previous one is cancelled first (AD-10).
    fn set_tool(&mut self, tool: Option<Box<dyn Tool>>) {
        if let Some(old) = &mut self.tool {
            old.cancel();
        }
        self.tool = tool;
    }

    /// Whether the open document may be edited: tools, Undo and Redo need this.
    fn editable(&self) -> bool {
        self.doc.as_ref().is_some_and(document::Document::editable)
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
                let next = tools::connect::cycled(self.tool.as_deref(), self.continuous);
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
            | Action::Undo
            | Action::Redo
            | Action::DeleteHovered => {}
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
    /// transaction it returns, or opens the Edit location dialog it asks for. A read-only
    /// document gets no click, nor does the map while the dialog is shown.
    fn click(&mut self, px: f64, py: f64, snap: bool) {
        if !self.editable() || self.location.is_some() {
            return;
        }
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
        if let Some(node) = tool.take_location_request() {
            self.location = tools::edit_location::LocationDialog::open(&doc.config.network, node);
        }
        let Some(tx) = tx else {
            return;
        };
        if let Err(e) = doc.apply(tx) {
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

    fn open(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("XML", &["xml"])
            .pick_file()
        else {
            return;
        };
        self.loading = true;
        let ctx = ctx.clone();
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf));
        jobs::spawn_load(
            path,
            exe_dir,
            self.settings.clone(),
            self.tx.clone(),
            move || {
                ctx.request_repaint();
            },
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
        ui::error_window(self, &ctx);
        // Before the prompt, so a prompt from a window close shows on top of it.
        ui::location_dialog(self, &ctx);
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
            result: Ok(Box::new((config, map_image::blank(), None, None))),
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
        let net = &app.doc.as_ref().expect("doc").config.network;
        assert_eq!(net.connection_kind(c, a), ConnectionKind::Regular);
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
        app.tool = Some(Box::new(tools::connect::ConnectTool::new(
            ConnectionShape::OneWay,
            false,
        )));
        app.view.hovered_node = Some(first(&app));
        app.click(0.0, 0.0, false);
        assert!(!app.doc.as_ref().expect("doc").history.can_undo());
        assert!(app.error.is_none());
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
        };
        app.tx.send(saved).expect("send");
        app.poll_jobs();
        app.resume(&ctx);
        assert!(app.pending.is_none());
        assert!(app.allow_close);
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
            result: Ok(Box::new((config, map_image::blank(), None, None))),
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
