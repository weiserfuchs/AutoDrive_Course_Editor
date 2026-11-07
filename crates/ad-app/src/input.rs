//! Keyboard shortcuts of the shell: the one fixed table (Java defaults, Redo per AD-13).

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};

use crate::App;

/// What a shortcut does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    NodeTool,
    MoveTool,
    /// Selects the regular connect tool, then cycles regular, dual, reverse.
    ConnectTool,
    PriorityTool,
    FlipTool,
    ToggleSnap,
    /// Continuous connections: after a commit the end node starts the next link.
    ToggleContinuous,
    Undo,
    Redo,
    ToggleNodeId,
    /// Deletes the hovered node, whichever tool is active (Java's Delete key).
    DeleteHovered,
}

/// The shortcut table. Modifiers match exactly, so Ctrl+Shift+Z is never read as Ctrl+Z
/// and Shift+N or Alt+N do nothing.
pub(crate) const SHORTCUTS: [(Action, KeyboardShortcut); 12] = [
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
        Action::MoveTool,
        KeyboardShortcut::new(Modifiers::NONE, Key::M),
    ),
    (
        Action::ConnectTool,
        KeyboardShortcut::new(Modifiers::NONE, Key::L),
    ),
    (
        Action::PriorityTool,
        KeyboardShortcut::new(Modifiers::NONE, Key::P),
    ),
    (
        Action::FlipTool,
        KeyboardShortcut::new(Modifiers::NONE, Key::F),
    ),
    (
        Action::ToggleSnap,
        KeyboardShortcut::new(Modifiers::NONE, Key::S),
    ),
    (
        Action::ToggleContinuous,
        KeyboardShortcut::new(Modifiers::NONE, Key::Semicolon),
    ),
    // Layouts such as German type `;` with Shift, and egui reports it with Shift held.
    (
        Action::ToggleContinuous,
        KeyboardShortcut::new(Modifiers::SHIFT, Key::Semicolon),
    ),
    (
        Action::ToggleNodeId,
        KeyboardShortcut::new(Modifiers::NONE, Key::F4),
    ),
    (
        Action::DeleteHovered,
        KeyboardShortcut::new(Modifiers::NONE, Key::Delete),
    ),
];

/// Whether grid snap applies this frame: the toggle, inverted while Shift is held.
pub(crate) fn snap(toggle: bool, ctx: &egui::Context) -> bool {
    resolve_snap(toggle, ctx.input(|i| i.modifiers.shift))
}

fn resolve_snap(toggle: bool, shift: bool) -> bool {
    toggle != shift
}

/// Consumes the presses of `shortcut` whose modifiers match it exactly (egui's
/// `consume_shortcut` ignores extra Shift and Alt). Returns whether there was one.
fn consume_exact(i: &mut egui::InputState, shortcut: &KeyboardShortcut) -> bool {
    let before = i.events.len();
    i.events.retain(|e| {
        !matches!(e, egui::Event::Key { key, modifiers, pressed: true, .. }
            if *key == shortcut.logical_key && modifiers.matches_exact(shortcut.modifiers))
    });
    i.events.len() != before
}

/// The actions pressed this frame, in table order; their key presses are consumed.
pub(crate) fn pressed(ctx: &egui::Context) -> Vec<Action> {
    ctx.input_mut(|i| {
        SHORTCUTS
            .iter()
            .filter(|(_, shortcut)| consume_exact(i, shortcut))
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
        assert_eq!(press(Modifiers::NONE, Key::M), [Action::MoveTool]);
        assert_eq!(press(Modifiers::NONE, Key::P), [Action::PriorityTool]);
        assert_eq!(press(Modifiers::NONE, Key::F), [Action::FlipTool]);
        assert_eq!(press(Modifiers::NONE, Key::S), [Action::ToggleSnap]);
        assert_eq!(press(Modifiers::NONE, Key::Delete), [Action::DeleteHovered]);
        assert_eq!(press(Modifiers::NONE, Key::L), [Action::ConnectTool]);
        assert_eq!(
            press(Modifiers::NONE, Key::Semicolon),
            [Action::ToggleContinuous]
        );
        assert_eq!(
            press(Modifiers::SHIFT, Key::Semicolon),
            [Action::ToggleContinuous]
        );
    }

    #[test]
    fn shift_inverts_snap_while_held() {
        assert!(resolve_snap(true, false));
        assert!(!resolve_snap(true, true));
        assert!(!resolve_snap(false, false));
        assert!(resolve_snap(false, true));
    }

    #[test]
    fn plain_z_and_shift_z_do_nothing() {
        assert!(press(Modifiers::NONE, Key::Z).is_empty());
        assert!(press(Modifiers::SHIFT, Key::Z).is_empty());
        assert!(press(CTRL.plus(Modifiers::ALT), Key::Z).is_empty());
    }

    #[test]
    fn extra_modifiers_do_not_fire_n_or_f4() {
        assert!(press(Modifiers::SHIFT, Key::N).is_empty());
        assert!(press(Modifiers::ALT, Key::N).is_empty());
        assert!(press(CTRL, Key::N).is_empty());
        assert!(press(Modifiers::SHIFT, Key::F4).is_empty());
        // Ctrl+S stays free for saving.
        assert!(press(CTRL, Key::S).is_empty());
        assert!(press(Modifiers::SHIFT, Key::M).is_empty());
        assert!(press(CTRL, Key::Delete).is_empty());
        assert!(press(Modifiers::SHIFT, Key::L).is_empty());
        assert!(press(CTRL, Key::L).is_empty());
        assert!(press(CTRL, Key::Semicolon).is_empty());
    }
}
