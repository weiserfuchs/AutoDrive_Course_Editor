//! The open document: config, history, selection, path and epoch (AD-9, AD-15, AD-18).

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use ad_core::command::Transaction;
use ad_core::history::{EntryId, History, SavePoint};
use ad_core::map_context::MapContext;
use ad_core::network::{NetworkError, NodeId};
use ad_core::scan;
use ad_io::config_xml::LoadedConfig;
use ad_io::map_image::MapImage;
use ad_io::vehicles::VehiclesFile;

/// The notice line of a scan that marked no node.
const NO_OVERLAPS: &str = "No overlapping nodes";
/// The notice line of a scan that marked nodes, after their count.
const OVERLAPS_FOUND_SUFFIX: &str = " overlapping nodes found";
/// The notice line naming the markers deleted with merged nodes, before their names.
pub const MARKERS_MERGED_PREFIX: &str = "Markers deleted with merged nodes: ";
/// The notice lines of the two fixes (story 8.3): nothing to fix, and after the count of
/// fixed nodes.
const NO_OUTSIDE: &str = "No nodes outside the map";
const OUTSIDE_MOVED_SUFFIX: &str = " nodes moved to the map edge";
const NO_MISSING_HEIGHT: &str = "No nodes without height";
const HEIGHTS_FILLED_SUFFIX: &str = " node heights filled";
/// The notice line of a connection check that found no broken entry (story 8.4).
const CONNECTIONS_CONSISTENT: &str = "Connections are consistent";

/// A fix of the Scan menu: nodes outside the map move to its edge, or nodes without a
/// height get the terrain height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fix {
    OutOfBounds,
    MissingHeights,
}

/// [`scan::around`] `nodes`, or `None` (rescan the whole network) when they are more than 64
/// and an eighth of it: remarking around every node of the fixture costs about 43 ms, a full
/// scan 9 ms.
fn around_or_rescan(
    net: &ad_core::network::Network,
    nodes: &[NodeId],
    distance: f64,
) -> Option<BTreeSet<NodeId>> {
    (nodes.len() <= (net.len() / 8).max(64)).then(|| scan::around(net, nodes, distance))
}

pub struct Document {
    pub config: LoadedConfig,
    pub history: History,
    pub path: PathBuf,
    /// The path the config was opened from; only a save to it writes `vehicles.xml`.
    pub opened_path: PathBuf,
    /// The `vehicles.xml` beside the opened config, when one was read (AD-17).
    pub vehicles: Option<Arc<VehiclesFile>>,
    /// The buildings of the `placeables.xml` beside the opened config, shown on the map; read only.
    pub placeables: Option<Arc<[ad_io::placeables::Placeable]>>,
    pub epoch: u64,
    pub map: MapContext,
    /// Decoded image waiting for the UI thread to upload it; taken right after (AD-14).
    pub map_image: Option<MapImage>,
    /// PNG bytes of the shown image, kept after the upload so it can be saved (AD-8).
    pub map_png: Arc<[u8]>,
    pub map_texture: Option<eframe::egui::TextureHandle>,
    /// Whether a real map image is shown; false while the blank stands in (Map info).
    pub has_image: bool,
    /// Nodes and connections the hide tools hid; session only, a new document has none.
    pub hidden: crate::view::Hidden,
    /// An imported map image, shown, whose save waits for "Replace the existing map image?".
    pub image_to_replace: Option<crate::jobs::ImageSave>,
    /// A warning about the map image, shown until the next document loads.
    pub notice: Option<String>,
    /// Selected nodes, kept outside the history and pruned to existing ids after every
    /// apply, undo and redo (AD-15). A new document starts with none (AD-18).
    pub selection: BTreeSet<NodeId>,
    /// Bumped by every apply, undo and redo and by the map panel after a selection
    /// gesture; tools key per-frame caches of the network or selection on it.
    pub revision: u64,
    /// The selection before and after each entry applied with `apply_selecting`; undo and
    /// redo of such an entry restore it. Records of entries gone from the history are
    /// dropped at the next `apply_selecting`.
    selections: BTreeMap<EntryId, (BTreeSet<NodeId>, BTreeSet<NodeId>)>,
    /// The overlap scan's distance and the nodes it marks, while marks are shown; kept up
    /// to date after every apply, undo and redo. A new document starts with none.
    pub overlap: Option<(f64, BTreeSet<NodeId>)>,
    /// When the autosave timer last started; `None` starts it at the next check (a new
    /// document, a save, AD-16).
    pub autosave_since: Option<Instant>,
    /// The history position the last autosave wrote; an autosave never repeats it.
    pub autosaved: Option<SavePoint>,
}

impl Document {
    /// A document with a fresh history, gated read-only when the config requires it.
    /// It starts clean unless the load reset flags.
    pub fn new(
        config: LoadedConfig,
        map: MapContext,
        map_image: MapImage,
        notice: Option<String>,
        path: PathBuf,
        epoch: u64,
        history_cap: usize,
    ) -> Self {
        let mut history = History::new(history_cap);
        history.set_read_only(config.read_only().is_some());
        // The load already changed the flags, so a save writes different bytes (AD-11).
        if config.flags_reset() {
            history.mark_unsaved();
        }
        Self {
            config,
            history,
            opened_path: path.clone(),
            vehicles: None,
            placeables: None,
            path,
            epoch,
            map,
            map_png: map_image.png.clone(),
            map_image: Some(map_image),
            map_texture: None,
            has_image: false,
            hidden: crate::view::Hidden::default(),
            image_to_replace: None,
            notice,
            selection: BTreeSet::new(),
            revision: 0,
            selections: BTreeMap::new(),
            overlap: None,
            autosave_since: None,
            autosaved: None,
        }
    }

    /// Applies `tx`, then selects what `select` picks from the applied transaction (AD-15
    /// `select_after`). Undo of the entry restores the selection from before it, redo the
    /// one after it.
    pub fn apply_selecting(
        &mut self,
        tx: Transaction,
        select: impl FnOnce(&Transaction) -> BTreeSet<NodeId>,
    ) -> Result<EntryId, NetworkError> {
        let near = self.overlap_around(Some(&tx));
        let before = self.selection.clone();
        let id = self.history.apply(&mut self.config.network, tx);
        if let Ok(id) = id {
            let history = &self.history;
            self.selections
                .retain(|entry, _| history.transaction(*entry).is_some());
            let after = history.transaction(id).map(select).unwrap_or_default();
            self.selection = after.clone();
            self.selections.insert(id, (before, after));
        }
        self.follow_overlap(near, id.as_ref().ok().copied());
        self.prune_selection();
        id
    }

    pub fn apply(&mut self, tx: Transaction) -> Result<EntryId, NetworkError> {
        let near = self.overlap_around(Some(&tx));
        let result = self.history.apply(&mut self.config.network, tx);
        self.follow_overlap(near, result.as_ref().ok().copied());
        self.prune_selection();
        result
    }

    /// Returns false when there is nothing to undo or the history is read-only.
    pub fn undo(&mut self) -> bool {
        let target = self.history.undo_target();
        let near = self.overlap_around(target.and_then(|id| self.history.transaction(id)));
        let done = self.history.undo(&mut self.config.network);
        self.follow_overlap(near, target);
        if done && let Some((before, _)) = target.and_then(|id| self.selections.get(&id)) {
            self.selection = before.clone();
        }
        self.prune_selection();
        done
    }

    /// Returns false when there is nothing to redo or the history is read-only.
    pub fn redo(&mut self) -> bool {
        let target = self.history.redo_target();
        let near = self.overlap_around(target.and_then(|id| self.history.transaction(id)));
        let done = self.history.redo(&mut self.config.network);
        self.follow_overlap(near, target);
        if done && let Some((_, after)) = target.and_then(|id| self.selections.get(&id)) {
            self.selection = after.clone();
        }
        self.prune_selection();
        done
    }

    /// Drops selected ids whose node no longer exists (AD-15) and bumps the revision; every
    /// apply, undo and redo ends here.
    fn prune_selection(&mut self) {
        self.revision += 1;
        let net = &self.config.network;
        self.selection.retain(|&id| net.node(id).is_some());
    }

    /// Shows `image` instead of the current one: it waits for the upload, its PNG stays.
    pub fn show(&mut self, image: MapImage) {
        self.map_png = image.png.clone();
        self.map_image = Some(image);
        self.has_image = true;
    }

    /// Adds `line` below the notice.
    pub fn add_notice(&mut self, line: String) {
        self.notice = Some(match self.notice.take() {
            Some(old) => format!("{old}\n{line}"),
            None => line,
        });
    }

    /// Drops the notice lines that start with `prefix`; keeps any other line.
    pub fn drop_notice_lines(&mut self, prefix: &str) {
        self.drop_notice_lines_where(|line| line.starts_with(prefix));
    }

    fn drop_notice_lines_where(&mut self, drop: impl Fn(&str) -> bool) {
        self.notice = self.notice.take().and_then(|notice| {
            let rest: Vec<&str> = notice.lines().filter(|line| !drop(line)).collect();
            (!rest.is_empty()).then(|| rest.join("\n"))
        });
    }

    /// Marks every node with another node within `distance` (3D) and shows how many; the
    /// marks then follow every apply, undo and redo. Changes no data, so it works on a
    /// read-only document too.
    pub fn scan_overlaps(&mut self, distance: f64) {
        self.overlap = Some((distance, scan::overlaps(&self.config.network, distance)));
        self.overlap_notice();
    }

    /// Removes the overlap marks and their notice.
    pub fn clear_overlaps(&mut self) {
        self.overlap = None;
        self.overlap_notice();
    }

    /// How many nodes are marked and how many stay after their merge; `None` without marks.
    pub fn merge_counts(&self) -> Option<(usize, usize)> {
        let (distance, marks) = self.overlap.as_ref().filter(|(_, m)| !m.is_empty())?;
        let groups = scan::overlap_groups(&self.config.network, *distance, marks);
        Some((marks.len(), groups.len()))
    }

    /// Merges the marked overlapping nodes in one entry (story 8.2): each group keeps one
    /// node, which takes over the links of the others, and the kept nodes become the
    /// selection. A notice names the markers deleted with merged nodes. Without marks,
    /// nothing happens.
    pub fn merge_overlaps(&mut self) -> Result<(), NetworkError> {
        let Some((distance, marks)) = &self.overlap else {
            return Ok(());
        };
        let net = &self.config.network;
        let groups = scan::overlap_groups(net, *distance, marks);
        let (tx, removed_markers) = scan::merge_plan(net, &groups);
        if tx.0.is_empty() {
            return Ok(());
        }
        let kept: BTreeSet<NodeId> = groups.iter().map(|g| g.keep).collect();
        self.apply_selecting(tx, |_| kept)?;
        if !removed_markers.is_empty() {
            self.add_notice(format!(
                "{MARKERS_MERGED_PREFIX}{}",
                removed_markers.join(", ")
            ));
        }
        Ok(())
    }

    /// The moves of `fix` on the current network; empty when it has nothing to fix.
    fn fix_plan(&self, fix: Fix) -> Transaction {
        let net = &self.config.network;
        match fix {
            Fix::OutOfBounds => scan::out_of_bounds_plan(net, self.map.geometry),
            Fix::MissingHeights => scan::missing_heights_plan(net, &self.map),
        }
    }

    /// How many nodes `fix` would change now.
    pub fn fix_count(&self, fix: Fix) -> usize {
        scan::placed_nodes(&self.fix_plan(fix)).len()
    }

    /// Runs `fix` in one entry; the changed nodes become the selection (undo restores the
    /// one before) and a notice gives their count. With nothing to fix, a notice says so
    /// and nothing is committed. Returns how many nodes changed.
    pub fn apply_fix(&mut self, fix: Fix) -> Result<usize, NetworkError> {
        let tx = self.fix_plan(fix);
        let count = scan::placed_nodes(&tx).len();
        if count > 0 {
            self.apply_selecting(tx, |tx| scan::placed_nodes(tx).into_iter().collect())?;
        }
        self.fix_notice(fix, count);
        Ok(count)
    }

    /// Replaces the notice line of an earlier fix with the result of `fix`: `count` nodes
    /// fixed, or none to fix.
    pub fn fix_notice(&mut self, fix: Fix, count: usize) {
        self.drop_notice_lines_where(|line| {
            line == NO_OUTSIDE
                || line == NO_MISSING_HEIGHT
                || line.ends_with(OUTSIDE_MOVED_SUFFIX)
                || line.ends_with(HEIGHTS_FILLED_SUFFIX)
        });
        let line = match (fix, count) {
            (Fix::OutOfBounds, 0) => NO_OUTSIDE.to_owned(),
            (Fix::OutOfBounds, n) => format!("{n}{OUTSIDE_MOVED_SUFFIX}"),
            (Fix::MissingHeights, 0) => NO_MISSING_HEIGHT.to_owned(),
            (Fix::MissingHeights, n) => format!("{n}{HEIGHTS_FILLED_SUFFIX}"),
        };
        self.add_notice(line);
    }

    /// The dangling incoming, duplicate and self entries of the network. With none, the
    /// notice says the connections are consistent (replacing that line of an earlier
    /// check). Changes no data, so it works on a read-only document too.
    pub fn check_connections(&mut self) -> scan::Defects {
        let defects = scan::connection_defects(&self.config.network);
        if defects.is_empty() {
            self.drop_notice_lines_where(|line| line == CONNECTIONS_CONSISTENT);
            self.add_notice(CONNECTIONS_CONSISTENT.to_owned());
        }
        defects
    }

    /// Removes every entry the connection check reports in one entry; the nodes whose
    /// lists changed become the selection (undo restores the one before). With none,
    /// nothing is committed.
    pub fn fix_connections(&mut self) -> Result<(), NetworkError> {
        let tx = scan::connection_fix_plan(&self.config.network);
        if !tx.0.is_empty() {
            self.apply_selecting(tx, |tx| scan::relinked_nodes(tx).into_iter().collect())?;
        }
        Ok(())
    }

    /// The nodes whose overlap mark `tx` can change, from where its nodes are now; none
    /// while no marks are shown. `None` asks for a full rescan instead.
    fn overlap_around(&self, tx: Option<&Transaction>) -> Option<BTreeSet<NodeId>> {
        match (&self.overlap, tx) {
            (Some((distance, _)), Some(tx)) => {
                around_or_rescan(&self.config.network, &scan::placed_nodes(tx), *distance)
            }
            _ => Some(BTreeSet::new()),
        }
    }

    /// After an apply, undo or redo of `entry`: remarks `near` (from [`Self::overlap_around`]
    /// before it) and the nodes around the entry's nodes now, so the marks equal a full
    /// rescan without one (a full scan of the fixture costs about 9 ms, a moved node a few
    /// µs). An entry that places many nodes gets the full rescan.
    fn follow_overlap(&mut self, near: Option<BTreeSet<NodeId>>, entry: Option<EntryId>) {
        let Some((distance, marks)) = &mut self.overlap else {
            return;
        };
        let net = &self.config.network;
        let after = match entry.and_then(|id| self.history.transaction(id)) {
            Some(tx) => around_or_rescan(net, &scan::placed_nodes(tx), *distance),
            None => Some(BTreeSet::new()),
        };
        match (near, after) {
            (Some(near), Some(after)) => {
                scan::remark(net, *distance, marks, near.into_iter().chain(after));
            }
            _ => *marks = scan::overlaps(net, *distance),
        }
        self.overlap_notice();
    }

    /// Replaces the overlap line of the notice with the current count; drops it while no
    /// marks are shown.
    fn overlap_notice(&mut self) {
        self.drop_notice_lines_where(|line| {
            line == NO_OVERLAPS || line.ends_with(OVERLAPS_FOUND_SUFFIX)
        });
        match self.overlap.as_ref().map(|(_, marks)| marks.len()) {
            Some(0) => self.add_notice(NO_OVERLAPS.to_owned()),
            Some(n) => self.add_notice(format!("{n}{OVERLAPS_FOUND_SUFFIX}")),
            None => {}
        }
    }

    /// Whether tools, Undo and Redo may change this document (AD-11).
    pub fn editable(&self) -> bool {
        self.config.read_only().is_none()
    }

    pub fn is_dirty(&self) -> bool {
        self.history.is_dirty()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use ad_core::map_context::MapContext;
    use ad_core::network::{NetworkError, NodeFlag};
    use ad_io::config_xml;
    use ad_io::map_image;

    use super::*;
    use crate::tools::node::NodeTool;
    use crate::tools::{Tool, ToolContext};

    fn doc(version: &str) -> Document {
        let text = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"no\"?>\n<AutoDrive>\n    \
             {version}\n    <waypoints>\n        <id>1</id>\n        <x>1.000</x>\n        \
             <y>0.000</y>\n        <z>2.000</z>\n        <out>-1</out>\n        \
             <incoming>-1</incoming>\n        <flags>0</flags>\n    </waypoints>\n</AutoDrive>\n"
        );
        let config = config_xml::load(text.as_bytes()).expect("loads");
        let map = MapContext::new(None, 1, 2.0, (2.0, 2.0));
        Document::new(
            config,
            map,
            map_image::blank(),
            None,
            PathBuf::from("a.xml"),
            1,
            100,
        )
    }

    fn node_tx(d: &Document) -> Transaction {
        NodeTool::new(NodeFlag::REGULAR)
            .on_click(&ToolContext {
                x: 5.0,
                z: 6.0,
                map: &d.map,
                net: &d.config.network,
                selection: &d.selection,
                snap: false,
                hovered_node: None,
                hovered_edge: None,
            })
            .expect("a transaction")
    }

    fn delete_tx(id: NodeId) -> Transaction {
        use ad_core::command::{Command, DeleteNodes};
        Transaction::new(vec![Command::DeleteNodes(DeleteNodes::new(vec![id]))])
    }

    #[test]
    fn undo_prunes_a_selected_node_the_edit_added() {
        let mut d = doc("<version>3</version>");
        let tx = node_tx(&d);
        d.apply(tx).expect("applies");
        let first = d.config.network.order()[0];
        let added = *d.config.network.order().last().expect("node");
        d.selection.extend([first, added]);
        assert!(d.undo());
        assert_eq!(d.selection.iter().copied().collect::<Vec<_>>(), [first]);
        // Redo brings the node back, not its selection.
        assert!(d.redo());
        assert_eq!(d.selection.iter().copied().collect::<Vec<_>>(), [first]);
    }

    #[test]
    fn delete_prunes_the_selection_and_undo_does_not_reselect_it() {
        let mut d = doc("<version>3</version>");
        let id = d.config.network.order()[0];
        d.selection.insert(id);
        d.apply(delete_tx(id)).expect("applies");
        assert!(d.selection.is_empty());
        assert!(d.undo());
        assert!(d.config.network.node(id).is_some());
        assert!(d.selection.is_empty());
    }

    #[test]
    fn a_selecting_entry_restores_its_selections_on_undo_and_redo_only() {
        use ad_core::command::{Command, PasteNodes};
        let mut d = doc("<version>3</version>");
        let first = d.config.network.order()[0];
        d.selection.insert(first);
        let paste = Transaction::new(vec![Command::PasteNodes(PasteNodes::new(
            vec![(3.0, 0.0, 3.0, NodeFlag::REGULAR)],
            Vec::new(),
            Vec::new(),
        ))]);
        d.apply_selecting(paste, crate::clipboard::pasted)
            .expect("applies");
        let pasted = *d.config.network.order().last().expect("pasted");
        assert_eq!(d.selection.iter().copied().collect::<Vec<_>>(), [pasted]);
        // A plain entry on top keeps today's rule: no selection change on undo or redo.
        let tx = node_tx(&d);
        d.apply(tx).expect("applies");
        d.selection.clear();
        assert!(d.undo());
        assert!(d.selection.is_empty());
        assert!(d.undo());
        assert_eq!(d.selection.iter().copied().collect::<Vec<_>>(), [first]);
        assert!(d.redo());
        assert_eq!(d.selection.iter().copied().collect::<Vec<_>>(), [pasted]);
        // A cut: undo reselects the cut nodes, redo selects nothing.
        let cut = delete_tx(pasted);
        d.apply_selecting(cut, |_| BTreeSet::new())
            .expect("applies");
        assert!(d.selection.is_empty());
        assert!(d.undo());
        assert_eq!(d.selection.iter().copied().collect::<Vec<_>>(), [pasted]);
        assert!(d.redo());
        assert!(d.selection.is_empty());
    }

    #[test]
    fn a_read_only_document_rejects_a_selecting_entry_and_keeps_the_selection() {
        let mut d = doc("<markerID>1</markerID>");
        let first = d.config.network.order()[0];
        d.selection.insert(first);
        assert_eq!(
            d.apply_selecting(delete_tx(first), |_| BTreeSet::new()),
            Err(NetworkError::ReadOnly)
        );
        assert_eq!(d.selection.iter().copied().collect::<Vec<_>>(), [first]);
    }

    #[test]
    fn a_new_document_starts_with_no_selection() {
        assert!(doc("<version>3</version>").selection.is_empty());
    }

    #[test]
    fn read_only_document_rejects_the_node_tool() {
        for version in ["<markerID>1</markerID>", "<version>9</version>"] {
            let mut d = doc(version);
            assert!(!d.editable());
            let tx = node_tx(&d);
            assert_eq!(d.apply(tx), Err(NetworkError::ReadOnly));
            assert_eq!(d.config.network.len(), 1);
            assert!(!d.undo() && !d.redo());
        }
    }

    #[test]
    fn undo_and_redo_keep_the_node_id() {
        let mut d = doc("<version>3</version>");
        assert!(d.editable());
        let tx = node_tx(&d);
        d.apply(tx).expect("applies");
        let id = *d.config.network.order().last().expect("node");
        assert_eq!(d.config.network.len(), 2);
        assert!(d.undo());
        assert_eq!(d.config.network.len(), 1);
        assert!(d.config.network.node(id).is_none());
        assert!(!d.undo());
        assert!(d.redo());
        assert_eq!(d.config.network.order().last(), Some(&id));
        let n = d.config.network.node(id).expect("back");
        assert_eq!((n.x, n.y, n.z), (5.0, -1.0, 6.0));
        assert!(!d.redo());
    }

    /// The loaded node at (1, 0, 2) and a node added 0.05 from it.
    fn pair() -> (Document, NodeId, NodeId) {
        use ad_core::command::{AddNodes, Command};
        let mut d = doc("<version>3</version>");
        let a = d.config.network.order()[0];
        let add = Transaction::new(vec![Command::AddNodes(AddNodes::new(vec![(
            1.05,
            0.0,
            2.0,
            NodeFlag::REGULAR,
        )]))]);
        d.apply(add).expect("applies");
        let b = *d.config.network.order().last().expect("added");
        (d, a, b)
    }

    fn marks(d: &Document) -> Option<Vec<NodeId>> {
        d.overlap
            .as_ref()
            .map(|(_, marks)| marks.iter().copied().collect())
    }

    #[test]
    fn overlap_marks_follow_a_move_and_its_undo_and_redo() {
        use ad_core::command::{Command, MoveNodes};
        let (mut d, a, b) = pair();
        d.scan_overlaps(0.1);
        assert_eq!(marks(&d), Some(vec![a, b]));
        assert_eq!(d.notice.as_deref(), Some("2 overlapping nodes found"));
        let away = Transaction::new(vec![Command::MoveNodes(MoveNodes {
            moves: vec![(b, (1.05, 0.0, 2.0), (30.0, 0.0, 2.0))],
        })]);
        d.apply(away).expect("applies");
        assert_eq!(marks(&d), Some(vec![]));
        assert_eq!(d.notice.as_deref(), Some("No overlapping nodes"));
        assert!(d.undo());
        assert_eq!(marks(&d), Some(vec![a, b]));
        assert!(d.redo());
        assert_eq!(marks(&d), Some(vec![]));
        assert!(d.undo());
        // Deleting one of the pair unmarks the other; undo brings both marks back.
        d.apply(delete_tx(b)).expect("applies");
        assert_eq!(marks(&d), Some(vec![]));
        assert!(d.undo());
        assert_eq!(marks(&d), Some(vec![a, b]));
        // The history and the network are untouched by the scan itself.
        assert!(d.undo());
        assert_eq!(marks(&d), Some(vec![]));
        assert!(!d.history.can_undo());
    }

    #[test]
    fn a_new_node_on_another_is_marked_while_marks_are_shown() {
        use ad_core::command::{AddNodes, Command};
        let mut d = doc("<version>3</version>");
        let a = d.config.network.order()[0];
        d.scan_overlaps(0.1);
        assert_eq!(d.notice.as_deref(), Some("No overlapping nodes"));
        // 0.05 above the loaded node.
        let add = Transaction::new(vec![Command::AddNodes(AddNodes::new(vec![(
            1.0,
            0.05,
            2.0,
            NodeFlag::REGULAR,
        )]))]);
        d.apply(add).expect("applies");
        let added = *d.config.network.order().last().expect("added");
        assert_eq!(marks(&d), Some(vec![a, added]));
        assert!(d.undo());
        assert_eq!(marks(&d), Some(vec![]));
    }

    #[test]
    fn an_edit_placing_many_nodes_rescans_and_matches_a_full_scan() {
        use ad_core::command::{AddNodes, Command};
        let mut d = doc("<version>3</version>");
        d.scan_overlaps(0.1);
        // 70 nodes, more than the 64 remarked around: one pair at the loaded node, the
        // rest 1 apart.
        let nodes = (0..70)
            .map(|i| {
                let x = if i == 0 { 1.02 } else { 10.0 + f64::from(i) };
                (x, 0.0, 2.0, NodeFlag::REGULAR)
            })
            .collect();
        d.apply(Transaction::new(vec![Command::AddNodes(AddNodes::new(
            nodes,
        ))]))
        .expect("applies");
        let want = scan::overlaps(&d.config.network, 0.1);
        assert_eq!(want.len(), 2);
        assert_eq!(d.overlap.as_ref().map(|(_, m)| m), Some(&want));
        assert!(d.undo());
        assert_eq!(marks(&d), Some(vec![]));
    }

    #[test]
    fn clearing_overlap_marks_drops_them_and_their_notice_only() {
        let (mut d, _, _) = pair();
        d.notice = Some("Map image missing".to_owned());
        d.scan_overlaps(0.1);
        assert_eq!(
            d.notice.as_deref(),
            Some("Map image missing\n2 overlapping nodes found")
        );
        // A second scan replaces the line.
        d.scan_overlaps(0.01);
        assert_eq!(
            d.notice.as_deref(),
            Some("Map image missing\nNo overlapping nodes")
        );
        d.clear_overlaps();
        assert_eq!(marks(&d), None);
        assert_eq!(d.notice.as_deref(), Some("Map image missing"));
        // Without marks, an edit computes none.
        let tx = node_tx(&d);
        d.apply(tx).expect("applies");
        assert_eq!(marks(&d), None);
    }

    #[test]
    fn the_overlap_scan_works_on_a_read_only_document() {
        let mut d = doc("<markerID>1</markerID>");
        assert!(!d.editable());
        d.scan_overlaps(0.1);
        assert_eq!(marks(&d), Some(vec![]));
        assert_eq!(d.notice.as_deref(), Some("No overlapping nodes"));
        assert!(!d.is_dirty() && !d.history.can_undo());
    }
}
