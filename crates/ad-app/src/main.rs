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

use ad_io::settings::Settings;
use document::Document;
use input::Action;
use jobs::JobResult;
use tools::Tool;
use view::MapView;

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
        }
    }

    /// Applies finished jobs. Called once per frame on the UI thread.
    fn poll_jobs(&mut self) {
        while let Ok(result) = self.rx.try_recv() {
            match &result {
                JobResult::Loaded { .. } => self.loading = false,
                JobResult::Saved { .. } => self.saving = false,
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
            Action::NodeTool if self.editable() => {
                let next = tools::node::cycled(self.tool.as_deref());
                self.set_tool(Some(Box::new(next)));
            }
            Action::MoveTool if self.editable() => {
                self.set_tool(Some(Box::new(tools::move_node::MoveTool::default())));
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
            Action::NodeTool | Action::MoveTool | Action::Undo | Action::Redo => {}
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

    fn save(&mut self, ctx: &egui::Context) {
        let Some(doc) = &self.doc else {
            return;
        };
        if doc.config.read_only().is_some() {
            return;
        }
        let mut dialog = rfd::FileDialog::new().add_filter("XML", &["xml"]);
        if let Some(dir) = doc.path.parent() {
            dialog = dialog.set_directory(dir);
        }
        if let Some(name) = doc.path.file_name() {
            dialog = dialog.set_file_name(name.to_string_lossy());
        }
        let Some(target) = dialog.save_file() else {
            return;
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
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_jobs();
        let ctx = ui.ctx().clone();
        input::handle_shortcuts(self, &ctx);
        ui::top_bar(self, ui, &ctx);
        egui::CentralPanel::default().show(ui, |ui| ui::map_panel(self, ui));
        ui::error_window(self, &ctx);
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

    use ad_core::network::NodeFlag;
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

    #[test]
    fn snap_toggles_with_s() {
        let mut app = App::new(Settings::default());
        assert!(!app.snap);
        app.run(Action::ToggleSnap);
        assert!(app.snap);
    }
}
