//! The open document: config, history, path and epoch (AD-9, AD-18).

use std::path::PathBuf;

use ad_core::command::Transaction;
use ad_core::history::{EntryId, History};
use ad_core::map_context::MapContext;
use ad_core::network::NetworkError;
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
        }
    }

    pub fn apply(&mut self, tx: Transaction) -> Result<EntryId, NetworkError> {
        self.history.apply(&mut self.config.network, tx)
    }

    /// Returns false when there is nothing to undo or the history is read-only.
    pub fn undo(&mut self) -> bool {
        self.history.undo(&mut self.config.network)
    }

    /// Returns false when there is nothing to redo or the history is read-only.
    pub fn redo(&mut self) -> bool {
        self.history.redo(&mut self.config.network)
    }

    /// Whether tools, Undo and Redo may change this document (AD-11).
    pub fn editable(&self) -> bool {
        self.config.read_only().is_none()
    }

    #[allow(dead_code)] // used by tests until the UI shows dirty state
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
