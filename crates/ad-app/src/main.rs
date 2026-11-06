//! Shell layer: the eframe application.

mod document;
mod icons;
mod jobs;
mod lod;
mod look;
mod overlay;
mod startup;
mod view;

use std::sync::mpsc::{self, Receiver, Sender};

use eframe::egui;

use ad_io::settings::Settings;
use document::Document;
use jobs::JobResult;
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
        }
    }

    /// Applies finished jobs. Called once per frame on the UI thread.
    fn poll_jobs(&mut self) {
        while let Ok(result) = self.rx.try_recv() {
            match &result {
                JobResult::Loaded { .. } => self.loading = false,
                JobResult::Saved { .. } => self.saving = false,
            }
            if let Some(message) = jobs::apply_result(
                &mut self.doc,
                &mut self.epoch_counter,
                &self.settings,
                result,
            ) {
                self.error = Some(message);
            }
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
        jobs::spawn_load(path, exe_dir, self.tx.clone(), move || {
            ctx.request_repaint();
        });
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

    fn paint_network(&mut self, ui: &mut egui::Ui) {
        let Some(doc) = &mut self.doc else {
            ui.allocate_space(ui.available_size());
            return;
        };
        if self.view_epoch != Some(doc.epoch) {
            self.view.reset();
            self.view_epoch = Some(doc.epoch);
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
        self.view.grid = doc.map.grid;
        self.view.show(
            ui,
            &doc.config.network,
            doc.map.geometry,
            doc.map.node_size,
            doc.map_texture.as_ref().map(egui::TextureHandle::id),
        );
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_jobs();
        let ctx = ui.ctx().clone();
        egui::Panel::top("top_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!self.loading && !self.saving, egui::Button::new("Open…"))
                    .clicked()
                {
                    self.open(&ctx);
                }
                // Load and save never overlap: either could read or replace the same file.
                let editable = !self.saving
                    && !self.loading
                    && self
                        .doc
                        .as_ref()
                        .is_some_and(|d| d.config.read_only().is_none());
                if ui
                    .add_enabled(editable, egui::Button::new("Save"))
                    .clicked()
                {
                    self.save(&ctx);
                }
                ui.checkbox(&mut self.view.show_grid, "Grid");
                if self.loading {
                    ui.label("Loading…");
                }
                if self.saving {
                    ui.label("Saving…");
                }
                if let Some(reason) = self.doc.as_ref().and_then(|d| d.config.read_only()) {
                    ui.colored_label(ui.visuals().warn_fg_color, reason.to_string());
                }
                if let Some(notice) = self.doc.as_ref().and_then(|d| d.notice.as_deref()) {
                    ui.colored_label(ui.visuals().warn_fg_color, notice);
                }
            });
        });
        egui::CentralPanel::default().show(ui, |ui| self.paint_network(ui));

        if let Some(message) = self.error.clone() {
            let mut open = true;
            egui::Window::new("Error")
                .open(&mut open)
                .collapsible(false)
                .show(ui.ctx(), |ui| {
                    ui.label(message);
                });
            if !open {
                self.error = None;
            }
        }
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
