//! The Add marker tool (Java `AddMarkerButton`): a primary click on a node without a
//! marker asks the app to open the marker dialog, a name and a group. OK sets the marker
//! in one `EditMarker`, appended to the marker order (AD-17), so it undoes (AD-13).
//!
//! The Edit marker tool (Java `EditMarkerButton`) opens the same dialog prefilled for a
//! node with a marker; OK replaces it in place, keeping its parked vehicles. The Delete
//! marker tool (Java `DeleteMarkerButton`) removes a node's marker in one `EditMarker`.

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

/// The toolbar notice for an Edit or Delete marker click on a node without a marker.
pub const NO_MARKER: &str = "This node has no marker";

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

/// The one `EditMarker` that removes the marker of `node`; `None` when it has none.
pub fn delete_plan(net: &Network, node: NodeId) -> Option<Transaction> {
    has_marker(net, node)
        .then(|| Transaction::new(vec![Command::EditMarker(EditMarker::new(node, None))]))
}

/// The open dialog: the node it gives a marker and the text of the name and group fields.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkerDialog {
    pub node: NodeId,
    pub name: String,
    pub group: String,
    /// The drop-down entries: "None", then the groups when the dialog opened.
    pub choices: Vec<String>,
    /// The marker's name and group when the dialog opened to edit it; `None` to add one.
    pub editing: Option<(String, String)>,
}

impl MarkerDialog {
    /// The dialog for `node`, empty and in no group; `None` when the node does not exist
    /// or already has a marker.
    pub fn open(net: &Network, node: NodeId) -> Option<Self> {
        if net.node(node).is_none() || has_marker(net, node) {
            return None;
        }
        Some(Self {
            node,
            name: String::new(),
            group: NONE.to_owned(),
            choices: choices(net),
            editing: None,
        })
    }

    /// The dialog that edits the marker of `node`, prefilled with its name and group
    /// ("All" shows as "None"); `None` when the node has no marker.
    pub fn edit(net: &Network, node: NodeId) -> Option<Self> {
        let marker = net.markers().iter().find(|m| m.node == node)?;
        let group = if marker.group == ALL {
            NONE
        } else {
            &marker.group
        };
        Some(Self {
            node,
            name: marker.name.clone(),
            group: group.to_owned(),
            choices: choices(net),
            editing: Some((marker.name.clone(), marker.group.clone())),
        })
    }

    /// The dialog's heading.
    pub fn title(&self) -> &'static str {
        if self.editing.is_some() {
            "Edit marker"
        } else {
            "Add marker"
        }
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

    /// The one `EditMarker` that sets the typed marker; `None` while the name is refused
    /// or, when editing, nothing changed.
    pub fn plan(&self, net: &Network) -> Option<Transaction> {
        let marker = self.validate(net).ok()?;
        if self.editing.as_ref() == Some(&marker) {
            return None;
        }
        Some(Transaction::new(vec![Command::EditMarker(
            EditMarker::new(self.node, Some(marker)),
        )]))
    }
}

/// "None", then the network's groups.
fn choices(net: &Network) -> Vec<String> {
    let mut choices = vec![NONE.to_owned()];
    choices.extend(groups(net));
    choices
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

/// The Edit marker tool: a click on a node leaves its id for the app to open the dialog
/// prefilled, or to show the notice when the node has no marker.
#[derive(Default)]
pub struct EditMarkerTool {
    requested: Option<NodeId>,
}

impl Tool for EditMarkerTool {
    fn name(&self) -> &'static str {
        "Edit marker"
    }

    /// A click on a node asks for the dialog; nothing commits here.
    fn on_click(&mut self, ctx: &ToolContext) -> Option<Transaction> {
        self.requested = ctx.hovered_node;
        None
    }

    fn take_request(&mut self) -> Option<ToolRequest> {
        self.requested.take().map(ToolRequest::EditMarker)
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

/// The Delete marker tool: a click on a node with a marker removes it in one `EditMarker`;
/// a click on a node without one leaves its id for the app to show the notice.
#[derive(Default)]
pub struct DeleteMarkerTool {
    unmarked: Option<NodeId>,
}

impl Tool for DeleteMarkerTool {
    fn name(&self) -> &'static str {
        "Delete marker"
    }

    fn on_click(&mut self, ctx: &ToolContext) -> Option<Transaction> {
        let node = ctx.hovered_node?;
        let tx = delete_plan(ctx.net, node);
        if tx.is_none() {
            self.unmarked = Some(node);
        }
        tx
    }

    fn take_request(&mut self) -> Option<ToolRequest> {
        self.unmarked.take().map(ToolRequest::NoMarker)
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
        self.unmarked = None;
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

    /// Markers "Hof" in "A" (two parked vehicles), "Feld" in "All" and "Wald" in "B".
    fn three_markers() -> (Network, [NodeId; 3]) {
        let mut net = Network::new();
        let ids = [0.0, 1.0, 2.0].map(|x| net.add_node(x, 0.0, 0.0, NodeFlag::REGULAR));
        net.push_marker(marker(ids[0], "Hof", "A"));
        net.push_marker(marker(ids[1], "Feld", ALL));
        net.push_marker(marker(ids[2], "Wald", "B"));
        assert!(net.park_vehicle(0, "1".to_owned()));
        assert!(net.park_vehicle(0, "2".to_owned()));
        (net, ids)
    }

    #[test]
    fn edit_replaces_name_and_group_in_place_keeping_parked_vehicles_and_reverts() {
        let (mut net, ids) = three_markers();
        let before = net.markers().to_vec();
        let mut d = MarkerDialog::edit(&net, ids[0]).expect("marked");
        assert_eq!(d.title(), "Edit marker");
        d.name = "Hof 2".to_owned();
        d.group = "B".to_owned();
        let mut tx = d.plan(&net).expect("changed");
        assert_eq!(tx.0.len(), 1);
        tx.apply(&mut net).expect("apply");
        let m = &net.markers()[0];
        assert_eq!((m.node, &*m.name, &*m.group), (ids[0], "Hof 2", "B"));
        assert_eq!(m.parked_vehicles, ["1", "2"]);
        assert_eq!(net.markers()[1..], before[1..]);
        tx.revert(&mut net).expect("revert");
        assert_eq!(net.markers(), before);
    }

    #[test]
    fn edit_prefills_name_and_group_with_all_as_none() {
        let (net, ids) = three_markers();
        let d = MarkerDialog::edit(&net, ids[1]).expect("marked");
        assert_eq!((&*d.name, &*d.group), ("Feld", NONE));
        assert_eq!(d.choices, ["None", "A", "B"]);
        let d = MarkerDialog::edit(&net, ids[0]).expect("marked");
        assert_eq!((&*d.name, &*d.group), ("Hof", "A"));
        let free = Network::new();
        assert!(MarkerDialog::edit(&free, ids[0]).is_none(), "missing node");
        assert_eq!(MarkerDialog::open(&net, ids[0]), None);
        let (net, _, unmarked) = setup();
        assert!(MarkerDialog::edit(&net, unmarked).is_none(), "no marker");
        let add = MarkerDialog::open(&net, unmarked).expect("dialog");
        assert_eq!(add.title(), "Add marker");
    }

    #[test]
    fn edit_keeping_its_own_name_with_a_new_group_is_accepted() {
        let (net, ids) = three_markers();
        let mut d = MarkerDialog::edit(&net, ids[0]).expect("marked");
        d.group = NONE.to_owned();
        assert_eq!(d.validate(&net), Ok(("Hof".to_owned(), ALL.to_owned())));
        assert!(d.plan(&net).is_some());
    }

    #[test]
    fn edit_to_another_markers_name_is_refused() {
        let (net, ids) = three_markers();
        let mut d = MarkerDialog::edit(&net, ids[0]).expect("marked");
        d.name = " Wald ".to_owned();
        assert_eq!(d.validate(&net), Err(Note::NameTaken));
        assert!(d.plan(&net).is_none());
    }

    #[test]
    fn edit_without_a_change_plans_nothing() {
        let (net, ids) = three_markers();
        for id in ids {
            let d = MarkerDialog::edit(&net, id).expect("marked");
            assert!(d.plan(&net).is_none(), "{id:?}");
        }
        // Only trimming or "None" for "All" is no change either.
        let mut d = MarkerDialog::edit(&net, ids[1]).expect("marked");
        d.name = " Feld ".to_owned();
        d.group = String::new();
        assert!(d.plan(&net).is_none());
    }

    #[test]
    fn delete_removes_the_marker_and_one_revert_restores_its_place_and_vehicles() {
        let (mut net, ids) = three_markers();
        let before = net.markers().to_vec();
        let mut tx = delete_plan(&net, ids[0]).expect("marked");
        let [Command::EditMarker(e)] = tx.0.as_slice() else {
            panic!("one EditMarker");
        };
        assert_eq!((e.node, &e.marker), (ids[0], &None));
        tx.apply(&mut net).expect("apply");
        assert_eq!(net.markers(), &before[1..], "later markers move up");
        tx.revert(&mut net).expect("revert");
        assert_eq!(net.markers(), before);
        let (net, _, unmarked) = setup();
        assert!(delete_plan(&net, unmarked).is_none());
    }

    #[test]
    fn edit_and_delete_marker_clicks_act_only_on_a_node() {
        let (net, marked, unmarked) = setup();
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
        let mut edit = EditMarkerTool::default();
        assert!(edit.on_click(&ctx(Some(marked))).is_none());
        assert_eq!(edit.take_request(), Some(ToolRequest::EditMarker(marked)));
        assert!(edit.on_click(&ctx(None)).is_none());
        assert_eq!(edit.take_request(), None);
        let mut delete = DeleteMarkerTool::default();
        assert_eq!(
            delete.on_click(&ctx(Some(marked))),
            delete_plan(&net, marked)
        );
        assert_eq!(delete.take_request(), None);
        assert!(delete.on_click(&ctx(Some(unmarked))).is_none());
        assert_eq!(delete.take_request(), Some(ToolRequest::NoMarker(unmarked)));
        assert_eq!(delete.take_request(), None, "taken once");
        assert!(delete.on_click(&ctx(None)).is_none());
        assert_eq!(delete.take_request(), None);
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
