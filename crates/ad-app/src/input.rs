//! Keyboard shortcuts of the shell: the default table (Java defaults, Redo per AD-13) and
//! the overrides from the settings (story 9.5).

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
    /// Opens the marker dialog prefilled for the hovered node's marker (Java's E).
    EditMarker,
    /// Removes the hovered node's marker (Java's Shift+D).
    DeleteMarker,
    /// Copies the selection to the app clipboard.
    Copy,
    /// Copies the selection, then deletes it.
    Cut,
    /// Pastes the clipboard centred on the view.
    Paste,
    /// Pastes the clipboard at the place it was copied from.
    PasteInPlace,
    /// File ▸ Open…, after the unsaved-changes prompt.
    Open,
    /// File ▸ Save: writes to the document's own path.
    Save,
    /// File ▸ Save As…: asks for the target.
    SaveAs,
    /// File ▸ Exit, after the unsaved-changes prompt.
    Exit,
}

/// The shortcut table. Modifiers match exactly, so Ctrl+Shift+Z is never read as Ctrl+Z
/// and Shift+N or Alt+N do nothing.
pub(crate) const SHORTCUTS: [(Action, KeyboardShortcut); 27] = [
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
    (
        Action::EditMarker,
        KeyboardShortcut::new(Modifiers::NONE, Key::E),
    ),
    (
        Action::DeleteMarker,
        KeyboardShortcut::new(Modifiers::SHIFT, Key::D),
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
    (
        Action::SaveAs,
        KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::S),
    ),
    (
        Action::Save,
        KeyboardShortcut::new(Modifiers::COMMAND, Key::S),
    ),
    (
        Action::Open,
        KeyboardShortcut::new(Modifiers::COMMAND, Key::O),
    ),
    (
        Action::Exit,
        KeyboardShortcut::new(Modifiers::COMMAND, Key::Q),
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

/// The actions pressed this frame, in `bindings` order, then the clipboard events; their key
/// presses and events are consumed.
pub(crate) fn pressed(ctx: &egui::Context, bindings: &[(Action, KeyboardShortcut)]) -> Vec<Action> {
    ctx.input_mut(|i| {
        let mut actions: Vec<Action> = bindings
            .iter()
            .filter(|(_, shortcut)| consume_exact(i, shortcut))
            .map(|(action, _)| *action)
            .collect();
        actions.extend(clipboard_actions(i));
        actions
    })
}

/// Handles the shortcuts pressed this frame. The unsaved prompt, the Edit location and
/// marker dialogs and the groups window are modal: no shortcut acts, nor is its key
/// consumed, while one is shown, so a text field there keeps its own copy and paste.
pub(crate) fn handle_shortcuts(app: &mut App, ctx: &egui::Context) {
    if app.prompt.is_some() || app.dialog_shown() {
        return;
    }
    for action in pressed(ctx, &app.bindings) {
        perform(app, action, ctx);
    }
}

/// The actions the Key bindings tab lists: (action, settings id, label). Copy, Cut and
/// Paste are not listed: Ctrl+C, Ctrl+X and Ctrl+V reach the app as clipboard events.
pub(crate) const CONFIGURABLE: [(Action, &str, &str); 23] = [
    (Action::Open, "open", "Open…"),
    (Action::Save, "save", "Save"),
    (Action::SaveAs, "save_as", "Save As…"),
    (Action::Exit, "exit", "Exit"),
    (Action::Undo, "undo", "Undo"),
    (Action::Redo, "redo", "Redo"),
    (Action::PasteInPlace, "paste_in_place", "Paste in place"),
    (Action::NodeTool, "node_tool", "Node tool"),
    (Action::MoveTool, "move_tool", "Move tool"),
    (Action::RotateTool, "rotate_tool", "Rotate tool"),
    (
        Action::AlignHorizontalTool,
        "align_horizontal",
        "Align horizontal",
    ),
    (
        Action::AlignVerticalTool,
        "align_vertical",
        "Align vertical",
    ),
    (Action::ConnectTool, "connect_tool", "Connect tool"),
    (Action::PriorityTool, "priority_tool", "Priority tool"),
    (Action::FlipTool, "flip_tool", "Flip tool"),
    (Action::BezierTool, "bezier_tool", "Bézier tool"),
    (Action::ToggleSnap, "toggle_snap", "Grid snap"),
    (
        Action::ToggleContinuous,
        "toggle_continuous",
        "Continuous connections",
    ),
    (Action::ToggleNodeId, "toggle_node_id", "Selected node id"),
    (
        Action::DeleteHovered,
        "delete_hovered",
        "Delete hovered node",
    ),
    (Action::AddMarker, "add_marker", "Add marker"),
    (Action::EditMarker, "edit_marker", "Edit marker"),
    (Action::DeleteMarker, "delete_marker", "Delete marker"),
];

/// The settings id of `action`, if the Key bindings tab lists it.
pub(crate) fn action_id(action: Action) -> Option<&'static str> {
    CONFIGURABLE
        .iter()
        .find(|(a, ..)| *a == action)
        .map(|(_, id, _)| *id)
}

/// The shortcut table in use: the defaults, with each action named in `overrides` (settings
/// id → key text) bound to that key instead, or to none for an empty text. A text that is
/// no key, or an unknown id, is ignored (logged), so the default stays.
pub(crate) fn bindings(
    overrides: &std::collections::BTreeMap<String, String>,
) -> Vec<(Action, KeyboardShortcut)> {
    let mut replaced: Vec<(Action, Option<KeyboardShortcut>)> = Vec::new();
    for (id, text) in overrides {
        let Some((action, ..)) = CONFIGURABLE.iter().find(|(_, i, _)| i == id) else {
            tracing::warn!("unknown shortcut action {id:?} in the settings; ignored");
            continue;
        };
        match parse_shortcut(text) {
            Some(shortcut) => replaced.push((*action, Some(shortcut))),
            None if text.trim().is_empty() => replaced.push((*action, None)),
            None => tracing::warn!("shortcut {text:?} for {id} is no key; the default stays"),
        }
    }
    let mut out: Vec<(Action, KeyboardShortcut)> = SHORTCUTS
        .iter()
        .filter(|(a, _)| !replaced.iter().any(|(r, _)| r == a))
        .copied()
        .collect();
    out.extend(replaced.into_iter().filter_map(|(a, s)| s.map(|s| (a, s))));
    out
}

/// "Ctrl+Shift+Alt+Key" (each modifier only when set) as stored in the settings. Ctrl stands
/// for the command modifier, as in the default table.
pub(crate) fn shortcut_to_text(shortcut: &KeyboardShortcut) -> String {
    let m = shortcut.modifiers;
    let mut text = String::new();
    if m.command || m.ctrl {
        text.push_str("Ctrl+");
    }
    if m.shift {
        text.push_str("Shift+");
    }
    if m.alt {
        text.push_str("Alt+");
    }
    text.push_str(shortcut.logical_key.name());
    text
}

/// The shortcut a settings text names, or `None` when it names no key.
pub(crate) fn parse_shortcut(text: &str) -> Option<KeyboardShortcut> {
    let mut parts: Vec<&str> = text.split('+').map(str::trim).collect();
    // "Ctrl++" names the Plus key: an empty last part after a '+'.
    if text.trim_end().ends_with("++") {
        parts.pop();
        if let Some(last) = parts.last_mut() {
            *last = "+";
        }
    }
    let key = Key::from_name(parts.pop()?)?;
    let mut modifiers = Modifiers::NONE;
    for part in parts {
        modifiers = modifiers.plus(match part.to_ascii_lowercase().as_str() {
            "ctrl" | "cmd" | "command" => Modifiers::COMMAND,
            "shift" => Modifiers::SHIFT,
            "alt" => Modifiers::ALT,
            _ => return None,
        });
    }
    Some(KeyboardShortcut::new(modifiers, key))
}

/// The shortcut a captured key press gives: Ctrl (or the Mac command key) as the command
/// modifier, Shift and Alt as pressed.
pub(crate) fn captured(modifiers: Modifiers, key: Key) -> KeyboardShortcut {
    let mut m = Modifiers::NONE;
    if modifiers.command || modifiers.ctrl || modifiers.mac_cmd {
        m = m.plus(Modifiers::COMMAND);
    }
    if modifiers.shift {
        m = m.plus(Modifiers::SHIFT);
    }
    if modifiers.alt {
        m = m.plus(Modifiers::ALT);
    }
    KeyboardShortcut::new(m, key)
}

/// The other action `shortcut` is bound to in `bindings`, if any.
pub(crate) fn bound_elsewhere(
    bindings: &[(Action, KeyboardShortcut)],
    shortcut: &KeyboardShortcut,
    action: Action,
) -> Option<Action> {
    bindings
        .iter()
        .find(|(a, s)| *a != action && s == shortcut)
        .map(|(a, _)| *a)
}

/// The label of `action` in the Key bindings tab.
pub(crate) fn action_label(action: Action) -> &'static str {
    CONFIGURABLE
        .iter()
        .find(|(a, ..)| *a == action)
        .map_or("", |(_, _, label)| *label)
}

/// Runs `action` from a shortcut or a menu item.
pub(crate) fn perform(app: &mut App, action: Action, ctx: &egui::Context) {
    if !app.run_file(action, ctx) {
        app.run(action);
    }
    // Text on the OS clipboard, so the next Ctrl+V reaches the app as a paste event.
    if let Some(text) = app.copied.take() {
        ctx.copy_text(text);
    }
}

/// The first shortcut of `action` in `bindings` as egui writes it (e.g. "Ctrl+S"), for menu
/// items and the Key bindings tab; empty when it has none.
pub(crate) fn shortcut_text(
    ctx: &egui::Context,
    bindings: &[(Action, KeyboardShortcut)],
    action: Action,
) -> String {
    bindings
        .iter()
        .find(|(a, _)| *a == action)
        .map(|(_, shortcut)| ctx.format_shortcut(shortcut))
        .unwrap_or_default()
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
        let mut out = ctx.run_ui(input, |ui| actions = pressed(ui.ctx(), &SHORTCUTS));
        // No renderer here, so there is nothing to upload.
        out.textures_delta.clear();
        actions
    }

    /// Like `press`, with the bindings built from `overrides`.
    fn press_with(overrides: &[(&str, &str)], modifiers: Modifiers, key: Key) -> Vec<Action> {
        let overrides = overrides
            .iter()
            .map(|(id, text)| ((*id).to_owned(), (*text).to_owned()))
            .collect();
        let table = bindings(&overrides);
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
        let mut out = ctx.run_ui(input, |ui| actions = pressed(ui.ctx(), &table));
        out.textures_delta.clear();
        actions
    }

    #[test]
    fn every_default_shortcut_survives_its_settings_text() {
        for (action, shortcut) in SHORTCUTS {
            let text = shortcut_to_text(&shortcut);
            assert_eq!(parse_shortcut(&text), Some(shortcut), "{action:?}: {text}");
        }
        assert_eq!(
            parse_shortcut("ctrl + shift + Z"),
            Some(KeyboardShortcut::new(
                Modifiers::COMMAND.plus(Modifiers::SHIFT),
                Key::Z
            ))
        );
        for bad in ["", "Ctrl+", "Hyper+N", "Nope"] {
            assert_eq!(parse_shortcut(bad), None, "{bad}");
        }
        assert_eq!(
            captured(CTRL.plus(Modifiers::SHIFT), Key::S),
            KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::S)
        );
        // Every listed action has a distinct id and a default key.
        for (action, id, _) in CONFIGURABLE {
            assert_eq!(CONFIGURABLE.iter().filter(|(_, i, _)| *i == id).count(), 1);
            assert!(SHORTCUTS.iter().any(|(a, _)| *a == action), "{id}");
        }
    }

    #[test]
    fn overrides_rebind_clear_or_are_ignored_when_they_name_no_key() {
        let rebound = [("node_tool", "Shift+N")];
        assert_eq!(
            press_with(&rebound, Modifiers::SHIFT, Key::N),
            [Action::NodeTool]
        );
        assert!(press_with(&rebound, Modifiers::NONE, Key::N).is_empty());
        // An empty text clears both of the action's default keys.
        let cleared = [("toggle_continuous", "")];
        assert!(press_with(&cleared, Modifiers::NONE, Key::Semicolon).is_empty());
        assert!(press_with(&cleared, Modifiers::SHIFT, Key::Semicolon).is_empty());
        // No key, or an unknown action: the defaults stay.
        let ignored = [("node_tool", "Ctrl+Nope"), ("no_such_action", "Q")];
        assert_eq!(
            press_with(&ignored, Modifiers::NONE, Key::N),
            [Action::NodeTool]
        );
        assert!(press_with(&ignored, Modifiers::NONE, Key::Q).is_empty());
        let table = bindings(&Default::default());
        let save = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
        assert_eq!(
            bound_elsewhere(&table, &save, Action::Open),
            Some(Action::Save)
        );
        assert_eq!(bound_elsewhere(&table, &save, Action::Save), None);
    }

    #[test]
    fn file_shortcuts_use_ctrl_and_leave_the_plain_keys_alone() {
        assert_eq!(press(CTRL, Key::O), [Action::Open]);
        assert_eq!(press(CTRL, Key::S), [Action::Save]);
        assert_eq!(press(CTRL.plus(Modifiers::SHIFT), Key::S), [Action::SaveAs]);
        assert_eq!(press(CTRL, Key::Q), [Action::Exit]);
        assert_eq!(press(Modifiers::NONE, Key::S), [Action::ToggleSnap]);
        assert!(press(Modifiers::NONE, Key::O).is_empty());
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
        assert_eq!(press(Modifiers::NONE, Key::E), [Action::EditMarker]);
        assert_eq!(press(Modifiers::SHIFT, Key::D), [Action::DeleteMarker]);
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
        let mut out = ctx.run_ui(input, |ui| actions = pressed(ui.ctx(), &SHORTCUTS));
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
        // Ctrl+S saves; it never toggles snap.
        assert_eq!(press(CTRL, Key::S), [Action::Save]);
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
        assert!(press(Modifiers::SHIFT, Key::E).is_empty());
        assert!(press(CTRL, Key::E).is_empty());
        assert!(press(Modifiers::ALT, Key::E).is_empty());
        assert!(press(Modifiers::NONE, Key::D).is_empty());
        assert!(press(CTRL.plus(Modifiers::SHIFT), Key::D).is_empty());
        assert!(press(Modifiers::SHIFT.plus(Modifiers::ALT), Key::D).is_empty());
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
