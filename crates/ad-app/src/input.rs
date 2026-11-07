//! Keyboard shortcuts of the shell: the one fixed table (Java defaults, Redo per AD-13).

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};

use crate::App;

/// What a shortcut does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    NodeTool,
    Undo,
    Redo,
    ToggleNodeId,
}

/// The shortcut table. egui ignores extra Shift when matching, so a shortcut with Shift
/// comes before the same one without it: Ctrl+Shift+Z must not be read as Ctrl+Z.
pub(crate) const SHORTCUTS: [(Action, KeyboardShortcut); 4] = [
    (
        Action::Redo,
        KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z),
    ),
    (
        Action::Undo,
        KeyboardShortcut::new(Modifiers::COMMAND, Key::Z),
    ),
    (
        Action::NodeTool,
        KeyboardShortcut::new(Modifiers::NONE, Key::N),
    ),
    (
        Action::ToggleNodeId,
        KeyboardShortcut::new(Modifiers::NONE, Key::F4),
    ),
];

/// The actions pressed this frame, in table order; their key presses are consumed.
pub(crate) fn pressed(ctx: &egui::Context) -> Vec<Action> {
    ctx.input_mut(|i| {
        SHORTCUTS
            .iter()
            .filter(|(_, shortcut)| i.consume_shortcut(shortcut))
            .map(|(action, _)| *action)
            .collect()
    })
}

/// Handles the shortcuts pressed this frame.
pub(crate) fn handle_shortcuts(app: &mut App, ctx: &egui::Context) {
    for action in pressed(ctx) {
        app.run(action);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ctrl as eframe reports it on Windows: ctrl and command both set.
    const CTRL: Modifiers = Modifiers::CTRL.plus(Modifiers::COMMAND);

    /// One frame with `key` pressed under `modifiers`; returns the resolved actions.
    fn press(modifiers: Modifiers, key: Key) -> Vec<Action> {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
            ..Default::default()
        };
        let mut actions = Vec::new();
        let mut out = ctx.run_ui(input, |ui| actions = pressed(ui.ctx()));
        // No renderer here, so there is nothing to upload.
        out.textures_delta.clear();
        actions
    }

    #[test]
    fn table_maps_the_java_defaults() {
        assert_eq!(press(Modifiers::NONE, Key::N), [Action::NodeTool]);
        assert_eq!(press(CTRL, Key::Z), [Action::Undo]);
        assert_eq!(press(CTRL.plus(Modifiers::SHIFT), Key::Z), [Action::Redo]);
        assert_eq!(press(Modifiers::NONE, Key::F4), [Action::ToggleNodeId]);
    }

    #[test]
    fn plain_z_and_shift_z_do_nothing() {
        assert!(press(Modifiers::NONE, Key::Z).is_empty());
        assert!(press(Modifiers::SHIFT, Key::Z).is_empty());
    }
}
