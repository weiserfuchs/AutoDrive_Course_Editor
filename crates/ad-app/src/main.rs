//! Shell layer: the eframe application.

use std::path::PathBuf;

use ad_io::config_xml::{self, LoadedConfig};
use eframe::egui;

#[derive(Default)]
struct App {
    config: Option<LoadedConfig>,
    path: Option<PathBuf>,
    error: Option<String>,
}

impl App {
    fn open(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("XML", &["xml"])
            .pick_file()
        else {
            return;
        };
        let result = std::fs::read(&path)
            .map_err(|e| format!("Cannot read {}: {e}", path.display()))
            .and_then(|bytes| {
                config_xml::load(&bytes).map_err(|e| format!("Cannot load {}: {e}", path.display()))
            });
        match result {
            Ok(config) => {
                self.config = Some(config);
                self.path = Some(path);
            }
            Err(message) => self.error = Some(message),
        }
    }

    fn save(&mut self) {
        let Some(config) = &self.config else {
            return;
        };
        let mut dialog = rfd::FileDialog::new().add_filter("XML", &["xml"]);
        if let Some(path) = &self.path {
            if let Some(dir) = path.parent() {
                dialog = dialog.set_directory(dir);
            }
            if let Some(name) = path.file_name() {
                dialog = dialog.set_file_name(name.to_string_lossy());
            }
        }
        let Some(target) = dialog.save_file() else {
            return;
        };
        if let Err(e) = config_xml::save_to(config, &target) {
            self.error = Some(e.to_string());
        }
    }

    fn paint_network(&self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::hover());
        let Some(config) = &self.config else {
            return;
        };
        let net = &config.network;
        let (mut min_x, mut max_x, mut min_z, mut max_z) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for id in net.order() {
            if let Some(n) = net.node(*id) {
                min_x = min_x.min(n.x);
                max_x = max_x.max(n.x);
                min_z = min_z.min(n.z);
                max_z = max_z.max(n.z);
            }
        }
        if net.is_empty() {
            return;
        }
        let rect = response.rect.shrink(8.0);
        let span_x = (max_x - min_x).max(1e-6);
        let span_z = (max_z - min_z).max(1e-6);
        let scale = (f64::from(rect.width()) / span_x).min(f64::from(rect.height()) / span_z);
        let center = rect.center();
        let to_screen = |x: f64, z: f64| {
            egui::pos2(
                center.x + ((x - (min_x + max_x) / 2.0) * scale) as f32,
                center.y + ((z - (min_z + max_z) / 2.0) * scale) as f32,
            )
        };

        let line = egui::Stroke::new(1.0, egui::Color32::from_gray(120));
        let dot = egui::Color32::from_rgb(80, 200, 120);
        for id in net.order() {
            let Some(node) = net.node(*id) else {
                continue;
            };
            let from = to_screen(node.x, node.z);
            for target in node.outgoing() {
                if let Some(t) = net.node(*target) {
                    painter.line_segment([from, to_screen(t.x, t.z)], line);
                }
            }
        }
        for id in net.order() {
            if let Some(node) = net.node(*id) {
                painter.circle_filled(to_screen(node.x, node.z), 1.5, dot);
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("top_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open…").clicked() {
                    self.open();
                }
                if ui
                    .add_enabled(self.config.is_some(), egui::Button::new("Save"))
                    .clicked()
                {
                    self.save();
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
    eframe::run_native(
        "AutoDrive Course Editor",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(App::default()))),
    )
}
