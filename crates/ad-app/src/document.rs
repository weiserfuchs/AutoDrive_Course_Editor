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

    #[allow(dead_code)] // editing arrives with the edit stories; tests use it now
    pub fn apply(&mut self, tx: Transaction) -> Result<EntryId, NetworkError> {
        self.history.apply(&mut self.config.network, tx)
    }

    #[allow(dead_code)] // used by tests until the UI shows dirty state
    pub fn is_dirty(&self) -> bool {
        self.history.is_dirty()
    }
}
