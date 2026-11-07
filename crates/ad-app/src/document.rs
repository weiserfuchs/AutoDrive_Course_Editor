//! The open document: config, history, selection, path and epoch (AD-9, AD-15, AD-18).

use std::collections::BTreeSet;
use std::path::PathBuf;

use ad_core::command::Transaction;
use ad_core::history::{EntryId, History};
use ad_core::map_context::MapContext;
use ad_core::network::{NetworkError, NodeId};
use ad_io::config_xml::LoadedConfig;
use ad_io::map_image::MapImage;

pub struct Document {
    pub config: LoadedConfig,
    pub history: History,
    pub path: PathBuf,
    pub epoch: u64,
    pub map: MapContext,
    /// Decoded image waiting for the UI thread to upload it; taken right after (AD-14).
    pub map_image: Option<MapImage>,
    pub map_texture: Option<eframe::egui::TextureHandle>,
    /// A warning about the map image, shown until the next document loads.
    pub notice: Option<String>,
    /// Selected nodes, kept outside the history and pruned to existing ids after every
    /// apply, undo and redo (AD-15). A new document starts with none (AD-18).
    pub selection: BTreeSet<NodeId>,
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
            path,
            epoch,
            map,
            map_image: Some(map_image),
            map_texture: None,
            notice,
            selection: BTreeSet::new(),
        }
    }

    pub fn apply(&mut self, tx: Transaction) -> Result<EntryId, NetworkError> {
        let result = self.history.apply(&mut self.config.network, tx);
        self.prune_selection();
        result
    }

    /// Returns false when there is nothing to undo or the history is read-only.
    pub fn undo(&mut self) -> bool {
        let done = self.history.undo(&mut self.config.network);
        self.prune_selection();
        done
    }

    /// Returns false when there is nothing to redo or the history is read-only.
    pub fn redo(&mut self) -> bool {
        let done = self.history.redo(&mut self.config.network);
        self.prune_selection();
        done
    }

    /// Drops selected ids whose node no longer exists (AD-15).
    fn prune_selection(&mut self) {
        let net = &self.config.network;
        self.selection.retain(|&id| net.node(id).is_some());
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
}
