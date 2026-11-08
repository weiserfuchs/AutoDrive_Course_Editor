//! Keyboard shortcuts of the shell: the one fixed table (Java defaults, Redo per AD-13).

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};

use crate::App;

/// What a shortcut does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    NodeTool,
    MoveTool,
    RotateTool,
    /// Aligns the selection's z to a clicked node.
    AlignHorizontalTool,
    /// Aligns the selection's x to a clicked node.
    AlignVerticalTool,
    /// Selects the regular connect tool, then cycles regular, dual, reverse.
    ConnectTool,
    PriorityTool,
    FlipTool,
    /// Selects the Bézier curve tool; a previewed curve is dropped.
    BezierTool,
    ToggleSnap,
    /// Continuous connections: after a commit the end node starts the next link.
    ToggleContinuous,
    Undo,
    Redo,
    ToggleNodeId,
    /// Deletes the hovered node, whichever tool is active (Java's Delete key).
    DeleteHovered,
    /// Opens the marker dialog for the hovered node, whichever tool is active (Java's W).
    AddMarker,
    /// Copies the selection to the app clipboard.
    Copy,
    /// Copies the selection, then deletes it.
    Cut,
    /// Pastes the clipboard centred on the view.
    Paste,
    /// Pastes the clipboard at the place it was copied from.
    PasteInPlace,
}

/// The shortcut table. Modifiers match exactly, so Ctrl+Shift+Z is never read as Ctrl+Z
/// and Shift+N or Alt+N do nothing.
pub(crate) const SHORTCUTS: [(Action, KeyboardShortcut); 21] = [
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
        Action::RotateTool,
        KeyboardShortcut::new(Modifiers::NONE, Key::R),
    ),
    (
        Action::AlignHorizontalTool,
        KeyboardShortcut::new(Modifiers::SHIFT, Key::X),
    ),
    (
        Action::AlignVerticalTool,
        KeyboardShortcut::new(Modifiers::SHIFT, Key::Y),
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
        Action::BezierTool,
        KeyboardShortcut::new(Modifiers::NONE, Key::B),
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
    (
        Action::AddMarker,
        KeyboardShortcut::new(Modifiers::NONE, Key::W),
    ),
    // eframe turns Ctrl+C, X and V into clipboard events (see `clipboard_actions`); these
    // key forms are kept for a backend that passes them on as keys.
    (
        Action::Copy,
        KeyboardShortcut::new(Modifiers::COMMAND, Key::C),
    ),
    (
        Action::Cut,
        KeyboardShortcut::new(Modifiers::COMMAND, Key::X),
    ),
    (
        Action::Paste,
        KeyboardShortcut::new(Modifiers::COMMAND, Key::V),
    ),
    (
        Action::PasteInPlace,
        KeyboardShortcut::new(Modifiers::SHIFT, Key::V),
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

/// Consumes the clipboard events egui-winit sends instead of key presses: Ctrl+C, Ctrl+X
/// and Ctrl+V (also Ctrl+Insert, Shift+Delete, Shift+Insert and the Copy, Cut and Paste
/// keys) arrive as `Event::Copy`, `Event::Cut` and `Event::Paste(text)`. A paste arrives
/// only while the OS clipboard holds text (or an image, as `Event::PasteImage`), so a copy
/// puts a note there (see `handle_shortcuts`).
fn clipboard_actions(i: &mut egui::InputState) -> Vec<Action> {
    let mut actions = Vec::new();
    i.events.retain(|e| {
        let action = match e {
            egui::Event::Copy => Action::Copy,
            egui::Event::Cut => Action::Cut,
            egui::Event::Paste(_) | egui::Event::PasteImage(_) => Action::Paste,
            _ => return true,
        };
        actions.push(action);
        false
    });
    actions
}

/// The actions pressed this frame, in table order, then the clipboard events; their key
/// presses and events are consumed.
pub(crate) fn pressed(ctx: &egui::Context) -> Vec<Action> {
    ctx.input_mut(|i| {
        let mut actions: Vec<Action> = SHORTCUTS
            .iter()
            .filter(|(_, shortcut)| consume_exact(i, shortcut))
            .map(|(action, _)| *action)
            .collect();
        actions.extend(clipboard_actions(i));
        actions
    })
}

/// Handles the shortcuts pressed this frame. The unsaved prompt and the Edit location and
/// marker dialogs are modal: no shortcut acts, nor is its key consumed, while one is
/// shown, so a text field there keeps its own copy and paste.
pub(crate) fn handle_shortcuts(app: &mut App, ctx: &egui::Context) {
    if app.prompt.is_some() || app.dialog_shown() {
        return;
    }
    for action in pressed(ctx) {
        app.run(action);
        // Text on the OS clipboard, so the next Ctrl+V reaches the app as a paste event.
        if let Some(text) = app.copied.take() {
            ctx.copy_text(text);
        }
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
        assert_eq!(press(Modifiers::NONE, Key::R), [Action::RotateTool]);
        assert_eq!(press(Modifiers::NONE, Key::P), [Action::PriorityTool]);
        assert_eq!(press(Modifiers::NONE, Key::F), [Action::FlipTool]);
        assert_eq!(press(Modifiers::NONE, Key::B), [Action::BezierTool]);
        assert_eq!(press(Modifiers::NONE, Key::S), [Action::ToggleSnap]);
        assert_eq!(press(Modifiers::NONE, Key::Delete), [Action::DeleteHovered]);
        assert_eq!(press(Modifiers::NONE, Key::W), [Action::AddMarker]);
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

    /// One frame with `events`; returns the resolved actions.
    fn send(events: Vec<egui::Event>) -> Vec<Action> {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            events,
            ..Default::default()
        };
        let mut actions = Vec::new();
        let mut out = ctx.run_ui(input, |ui| actions = pressed(ui.ctx()));
        out.textures_delta.clear();
        actions
    }

    #[test]
    fn copy_cut_and_paste_keys_and_shift_v() {
        assert_eq!(press(CTRL, Key::C), [Action::Copy]);
        assert_eq!(press(CTRL, Key::X), [Action::Cut]);
        assert_eq!(press(CTRL, Key::V), [Action::Paste]);
        assert_eq!(press(Modifiers::SHIFT, Key::V), [Action::PasteInPlace]);
        for key in [Key::C, Key::V] {
            assert!(press(Modifiers::NONE, key).is_empty());
            assert!(press(CTRL.plus(Modifiers::SHIFT), key).is_empty());
        }
    }

    #[test]
    fn clipboard_events_map_to_copy_cut_and_paste() {
        assert_eq!(
            send(vec![
                egui::Event::Copy,
                egui::Event::Cut,
                egui::Event::Paste("note".into()),
            ]),
            [Action::Copy, Action::Cut, Action::Paste]
        );
        assert!(send(vec![egui::Event::Text("v".into())]).is_empty());
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
        assert!(press(CTRL, Key::R).is_empty());
        assert!(press(CTRL, Key::Delete).is_empty());
        assert!(press(Modifiers::SHIFT, Key::L).is_empty());
        assert!(press(CTRL, Key::L).is_empty());
        assert!(press(CTRL, Key::Semicolon).is_empty());
        assert!(press(Modifiers::SHIFT, Key::B).is_empty());
        assert!(press(CTRL, Key::B).is_empty());
        assert!(press(Modifiers::ALT, Key::B).is_empty());
        assert!(press(Modifiers::SHIFT, Key::W).is_empty());
        assert!(press(CTRL, Key::W).is_empty());
        assert!(press(Modifiers::ALT, Key::W).is_empty());
    }

    #[test]
    fn shift_x_and_shift_y_pick_the_align_tools_and_plain_x_y_do_nothing() {
        assert_eq!(
            press(Modifiers::SHIFT, Key::X),
            [Action::AlignHorizontalTool]
        );
        assert_eq!(press(Modifiers::SHIFT, Key::Y), [Action::AlignVerticalTool]);
        for key in [Key::X, Key::Y] {
            assert!(press(Modifiers::NONE, key).is_empty());
            assert!(press(CTRL.plus(Modifiers::SHIFT), key).is_empty());
            assert!(press(Modifiers::SHIFT.plus(Modifiers::ALT), key).is_empty());
        }
    }
}
