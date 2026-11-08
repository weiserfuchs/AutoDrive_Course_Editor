//! The Add marker tool (Java `AddMarkerButton`): a primary click on a node without a
//! marker asks the app to open the marker dialog, a name and a group. OK sets the marker
//! in one `EditMarker`, appended to the marker order (AD-17), so it undoes (AD-13).

use std::fmt;

use eframe::egui;

use ad_core::command::{Command, EditMarker, Transaction};
use ad_core::geometry::MapGeometry;
use ad_core::network::{Network, NodeId};

use super::{Tool, ToolContext, ToolRequest};
use crate::view::ViewTransform;

/// The group of a marker in no group.
pub const ALL: &str = "All";

/// The drop-down entry that means no group (Java's "None").
pub const NONE: &str = "None";

/// The toolbar notice for a click on a node that already has a marker.
pub const HAS_MARKER: &str = "This node already has a marker";

/// Why the dialog's name is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Note {
    /// Empty once trimmed.
    EmptyName,
    /// Another node's marker has this name.
    NameTaken,
}

impl fmt::Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Note::EmptyName => write!(f, "* Enter a name"),
            Note::NameTaken => write!(f, "* Another marker has this name"),
        }
    }
}

/// The groups of the network's markers without "All", each once, sorted ignoring case.
pub fn groups(net: &Network) -> Vec<String> {
    let mut groups: Vec<String> = net
        .markers()
        .iter()
        .map(|m| m.group.clone())
        .filter(|g| g != ALL)
        .collect();
    groups.sort_by(|a, b| {
        a.to_lowercase()
            .cmp(&b.to_lowercase())
            .then_with(|| a.cmp(b))
    });
    groups.dedup();
    groups
}

/// Whether `node` has a marker.
pub fn has_marker(net: &Network, node: NodeId) -> bool {
    net.markers().iter().any(|m| m.node == node)
}

/// The open dialog: the node it gives a marker and the text of the name and group fields.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkerDialog {
    pub node: NodeId,
    pub name: String,
    pub group: String,
    /// The drop-down entries: "None", then the groups when the dialog opened.
    pub choices: Vec<String>,
}

impl MarkerDialog {
    /// The dialog for `node`, empty and in no group; `None` when the node does not exist
    /// or already has a marker.
    pub fn open(net: &Network, node: NodeId) -> Option<Self> {
        if net.node(node).is_none() || has_marker(net, node) {
            return None;
        }
        let mut choices = vec![NONE.to_owned()];
        choices.extend(groups(net));
        Some(Self {
            node,
            name: String::new(),
            group: NONE.to_owned(),
            choices,
        })
    }

    /// The trimmed name and the group ("All" for "None" or an empty group), or why the
    /// name is refused: empty, or used by another node's marker (exact match).
    pub fn validate(&self, net: &Network) -> Result<(String, String), Note> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(Note::EmptyName);
        }
        if net
            .markers()
            .iter()
            .any(|m| m.node != self.node && m.name == name)
        {
            return Err(Note::NameTaken);
        }
        let group = self.group.trim();
        let group = if group.is_empty() || group == NONE {
            ALL
        } else {
            group
        };
        Ok((name.to_owned(), group.to_owned()))
    }

    /// The one `EditMarker` that sets the typed marker; `None` while the name is refused.
    pub fn plan(&self, net: &Network) -> Option<Transaction> {
        let marker = self.validate(net).ok()?;
        Some(Transaction::new(vec![Command::EditMarker(
            EditMarker::new(self.node, Some(marker)),
        )]))
    }
}

/// The tool: a click on a node leaves its id for the app to open the dialog with.
#[derive(Default)]
pub struct AddMarkerTool {
    requested: Option<NodeId>,
}

impl Tool for AddMarkerTool {
    fn name(&self) -> &'static str {
        "Add marker"
    }

    /// A click on a node asks for the dialog; nothing commits here.
    fn on_click(&mut self, ctx: &ToolContext) -> Option<Transaction> {
        self.requested = ctx.hovered_node;
        None
    }

    fn take_request(&mut self) -> Option<ToolRequest> {
        self.requested.take().map(ToolRequest::AddMarker)
    }

    fn paint(
        &self,
        _painter: &egui::Painter,
        _t: &ViewTransform,
        _geometry: MapGeometry,
        _cursor: Option<egui::Pos2>,
        _snap_grid: Option<(f64, f64)>,
        _hovered_node: Option<(f64, f64)>,
        _hovered_edge: Option<(egui::Pos2, egui::Pos2)>,
    ) {
    }

    fn cancel(&mut self) {
        self.requested = None;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::collections::BTreeSet;

    use ad_core::map_context::MapContext;
    use ad_core::network::{Marker, NodeFlag};

    use super::*;

    fn marker(node: NodeId, name: &str, group: &str) -> Marker {
        Marker {
            node,
            name: name.to_owned(),
            group: group.to_owned(),
            parked_vehicles: Vec::new(),
        }
    }

    /// A network with a marked node "Feld" in "Felder" and an unmarked node.
    fn setup() -> (Network, NodeId, NodeId) {
        let mut net = Network::new();
        let marked = net.add_node(0.0, 0.0, 0.0, NodeFlag::REGULAR);
        let free = net.add_node(1.0, 0.0, 1.0, NodeFlag::REGULAR);
        net.push_marker(marker(marked, "Feld", "Felder"));
        (net, marked, free)
    }

    fn dialog(net: &Network, node: NodeId, name: &str, group: &str) -> MarkerDialog {
        let mut d = MarkerDialog::open(net, node).expect("dialog");
        d.name = name.to_owned();
        d.group = group.to_owned();
        d
    }

    #[test]
    fn add_appends_the_marker_in_one_edit_marker_and_reverts() {
        let (mut net, marked, free) = setup();
        let d = dialog(&net, free, "  Hof ", "Felder");
        let mut tx = d.plan(&net).expect("valid");
        let [Command::EditMarker(e)] = tx.0.as_slice() else {
            panic!("one EditMarker");
        };
        assert_eq!(e.marker, Some(("Hof".to_owned(), "Felder".to_owned())));
        tx.apply(&mut net).expect("apply");
        assert_eq!(
            net.markers(),
            [
                marker(marked, "Feld", "Felder"),
                marker(free, "Hof", "Felder")
            ]
        );
        tx.revert(&mut net).expect("revert");
        assert_eq!(net.markers(), [marker(marked, "Feld", "Felder")]);
    }

    #[test]
    fn a_typed_group_is_kept_as_a_new_group() {
        let (net, _, free) = setup();
        let d = dialog(&net, free, "Hof", "Neu");
        assert_eq!(d.validate(&net), Ok(("Hof".to_owned(), "Neu".to_owned())));
    }

    #[test]
    fn none_or_an_empty_group_means_all() {
        let (net, _, free) = setup();
        let d = MarkerDialog::open(&net, free).expect("dialog");
        assert_eq!(d.group, NONE, "opens in no group");
        for group in [NONE, "", "  "] {
            let d = dialog(&net, free, "Hof", group);
            assert_eq!(
                d.validate(&net),
                Ok(("Hof".to_owned(), ALL.to_owned())),
                "{group:?}"
            );
        }
    }

    #[test]
    fn groups_are_sorted_ignoring_case_without_all_after_none() {
        let mut net = Network::new();
        for group in ["b", "A", "All", "a2", "b"] {
            let id = net.add_node(0.0, 0.0, 0.0, NodeFlag::REGULAR);
            net.push_marker(marker(id, group, group));
        }
        assert_eq!(groups(&net), ["A", "a2", "b"]);
        let free = net.add_node(0.0, 0.0, 0.0, NodeFlag::REGULAR);
        let d = MarkerDialog::open(&net, free).expect("dialog");
        assert_eq!(d.choices, ["None", "A", "a2", "b"]);
    }

    #[test]
    fn an_empty_or_taken_name_is_refused() {
        let (net, _, free) = setup();
        for name in ["", "   "] {
            let d = dialog(&net, free, name, "Felder");
            assert_eq!(d.validate(&net), Err(Note::EmptyName), "{name:?}");
            assert!(d.plan(&net).is_none());
        }
        let d = dialog(&net, free, " Feld ", "");
        assert_eq!(d.validate(&net), Err(Note::NameTaken));
        assert!(d.plan(&net).is_none());
        // Exact match only.
        let d = dialog(&net, free, "feld", "");
        assert!(d.validate(&net).is_ok());
    }

    #[test]
    fn no_dialog_for_a_marked_or_missing_node() {
        let (net, marked, free) = setup();
        assert!(MarkerDialog::open(&net, marked).is_none());
        assert!(MarkerDialog::open(&Network::new(), free).is_none());
    }

    #[test]
    fn a_click_asks_for_the_dialog_only_on_a_node() {
        let (net, marked, _) = setup();
        let map = MapContext::new(None, 1, 2.0, (2.0, 2.0));
        let selection = BTreeSet::new();
        let ctx = |hovered_node| ToolContext {
            x: 0.0,
            z: 0.0,
            map: &map,
            net: &net,
            selection: &selection,
            snap: false,
            hovered_node,
            hovered_edge: None,
        };
        let mut tool = AddMarkerTool::default();
        assert!(tool.on_click(&ctx(Some(marked))).is_none());
        assert_eq!(tool.take_request(), Some(ToolRequest::AddMarker(marked)));
        assert_eq!(tool.take_request(), None, "taken once");
        assert!(tool.on_click(&ctx(None)).is_none());
        assert_eq!(tool.take_request(), None);
    }
}
