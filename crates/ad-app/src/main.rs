//! Shell layer: the eframe application.

struct App;

impl eframe::App for App {
    fn ui(&mut self, _ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {}
}

fn main() -> eframe::Result {
    eframe::run_native(
        "AutoDrive Course Editor",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(App))),
    )
}
