//! Shell panels and windows around the map view.

use eframe::egui;

use crate::App;

/// The top bar: Open, Save, the Grid toggle and the status labels.
pub(crate) fn top_bar(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
    egui::Panel::top("top_bar").show(ui, |ui| {
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!app.loading && !app.saving, egui::Button::new("Open…"))
                .clicked()
            {
                app.open(ctx);
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

/// The map view of the open document, with the overlay hook tools draw into.
pub(crate) fn map_panel(app: &mut App, ui: &mut egui::Ui) {
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
    app.view.show(
        ui,
        &doc.config.network,
        doc.map.geometry,
        doc.map.node_size,
        doc.map_texture.as_ref().map(egui::TextureHandle::id),
        // Tools draw here, on top of the view.
        |_, _, _| {},
    );
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
