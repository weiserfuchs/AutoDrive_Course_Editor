//! The open document: config, history, path and epoch (AD-9, AD-18).

use std::path::PathBuf;

use ad_core::command::Transaction;
use ad_core::history::{EntryId, History};
use ad_core::network::NetworkError;
use ad_io::config_xml::LoadedConfig;

pub struct Document {
    pub config: LoadedConfig,
    pub history: History,
    pub path: PathBuf,
    pub epoch: u64,
}

impl Document {
    /// A clean document with a fresh history, gated read-only when the config requires it.
    pub fn new(config: LoadedConfig, path: PathBuf, epoch: u64, history_cap: usize) -> Self {
        let mut history = History::new(history_cap);
        history.set_read_only(config.read_only().is_some());
        Self {
            config,
            history,
            path,
            epoch,
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
