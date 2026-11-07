//! Keyboard shortcuts of the shell.

use eframe::egui;

use crate::App;

/// Handles the shortcuts pressed this frame.
pub(crate) fn handle_shortcuts(app: &mut App, ctx: &egui::Context) {
    if ctx.input(|i| i.key_pressed(egui::Key::F4)) {
        app.view.show_node_id = !app.view.show_node_id;
    }
}
