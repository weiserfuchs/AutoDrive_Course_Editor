//! Shell layer: the eframe application.

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
    /// The action that continues once the save started from the prompt has finished.
    pending: Option<Pending>,
    /// The action whose unsaved-changes prompt is shown.
    prompt: Option<Pending>,
    /// Set once closing was agreed; the next close request is not intercepted.
    allow_close: bool,
    /// The title last sent to the window.
    title: String,
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
            pending: None,
            prompt: None,
            allow_close: false,
            title: window_title(None),
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
            Choice::Cancel => {}
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
            // A new document cancels the active tool (AD-18).
            if self.doc.as_ref().map(|d| d.epoch) != before {
                self.set_tool(None);
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
            }
            Action::Redo if self.editable() => {
                if let Some(tool) = &mut self.tool {
                    tool.cancel();
                }
                if let Some(doc) = &mut self.doc {
                    doc.redo();
                }
            }
            Action::DeleteHovered if self.editable() => self.delete_hovered(),
            Action::NodeTool
            | Action::MoveTool
            | Action::ConnectTool
            | Action::PriorityTool
            | Action::FlipTool
            | Action::Undo
            | Action::Redo
            | Action::DeleteHovered => {}
        }
    }

    /// Deletes the hovered node, if any, after cancelling the tool's preview (as Undo does).
    fn delete_hovered(&mut self) {
        let Some(doc) = &mut self.doc else {
            return;
        };
        // The hover belongs to the document the view last showed, and must still exist.
        let Some(id) = self
            .view
            .hovered_node
            .filter(|_| self.view_epoch == Some(doc.epoch))
            .filter(|&id| doc.config.network.node(id).is_some())
        else {
            return;
        };
        if let Some(tool) = &mut self.tool {
            tool.cancel();
        }
        let tx = Transaction::new(vec![Command::DeleteNodes(DeleteNodes::new(vec![id]))]);
        if let Err(e) = doc.apply(tx) {
            self.error = Some(e.to_string());
        }
    }

    /// Hands a primary click at map pixel `(px, py)` to the active tool and commits the
    /// transaction it returns. A read-only document gets no click.
    fn click(&mut self, px: f64, py: f64, snap: bool) {
        if !self.editable() {
            return;
        }
        let (Some(doc), Some(tool)) = (&mut self.doc, &mut self.tool) else {
            return;
        };
        let (x, z) = doc.map.geometry.to_world(px, py);
        let Some(tx) = tool.on_click(&tools::ToolContext {
            x,
            z,
            map: &doc.map,
            net: &doc.config.network,
            snap,
            hovered_node: self.view.hovered_node,
            hovered_edge: self.view.hovered_edge,
        }) else {
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
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.dirty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.prompt = Some(Pending::Close);
        }
        let title = window_title(self.doc.as_ref());
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
        // The prompt is modal: no shortcut reaches the map while it is shown.
        if self.prompt.is_none() {
            input::handle_shortcuts(self, &ctx);
        }
        ui::top_bar(self, ui, &ctx);
        egui::CentralPanel::default().show(ui, |ui| ui::map_panel(self, ui));
        ui::error_window(self, &ctx);
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
        let mut hooks =
            ui::ToolHooks::new(app.tool.as_deref_mut(), &doc.map, &doc.config.network, snap);
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
    fn snap_toggles_with_s() {
        let mut app = App::new(Settings::default());
        assert!(!app.snap);
        app.run(Action::ToggleSnap);
        assert!(app.snap);
    }
}
