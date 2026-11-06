//! Background load and save jobs, and how their results apply (AD-9, AD-16, AD-18).

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::thread;

use ad_core::history::SavePoint;
use ad_io::config_xml::{self, LoadedConfig};

use crate::document::Document;

pub enum JobResult {
    Loaded {
        path: PathBuf,
        result: Result<Box<LoadedConfig>, String>,
    },
    Saved {
        epoch: u64,
        point: SavePoint,
        target: PathBuf,
        result: Result<(), String>,
    },
}

/// Reads and parses the file; the error message names the file and the config problem.
pub fn load_file(path: &Path) -> Result<LoadedConfig, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    config_xml::load(&bytes).map_err(|e| format!("Cannot load {}: {e}", path.display()))
}

/// Runs `load_file` on a background thread, then calls `notify` (a repaint request).
pub fn spawn_load(path: PathBuf, tx: Sender<JobResult>, notify: impl FnOnce() + Send + 'static) {
    thread::spawn(move || {
        tracing::info!("loading {}", path.display());
        let result = load_file(&path).map(Box::new);
        match &result {
            Ok(_) => tracing::info!("loaded {}", path.display()),
            Err(e) => tracing::warn!("{e}"),
        }
        // The receiver is gone only when the app is closing.
        let _ = tx.send(JobResult::Loaded { path, result });
        notify();
    });
}

/// Saves a snapshot on a background thread, then calls `notify`.
pub fn spawn_save(
    snapshot: LoadedConfig,
    epoch: u64,
    point: SavePoint,
    target: PathBuf,
    tx: Sender<JobResult>,
    notify: impl FnOnce() + Send + 'static,
) {
    thread::spawn(move || {
        tracing::info!("saving {}", target.display());
        let result = config_xml::save_to(&snapshot, &target)
            .map_err(|e| format!("Cannot save {}: {e}", target.display()));
        match &result {
            Ok(()) => tracing::info!("saved {}", target.display()),
            Err(e) => tracing::warn!("{e}"),
        }
        let _ = tx.send(JobResult::Saved {
            epoch,
            point,
            target,
            result,
        });
        notify();
    });
}

/// The single place a document is replaced (AD-18). Later stories clear tool preview,
/// selection and hover here.
pub fn replace_document(
    doc: &mut Option<Document>,
    epoch_counter: &mut u64,
    config: LoadedConfig,
    path: PathBuf,
    history_cap: usize,
) {
    *epoch_counter += 1;
    *doc = Some(Document::new(config, path, *epoch_counter, history_cap));
}

/// Applies a finished job on the UI thread. Returns an error message to show, if any.
/// `epoch_counter` holds the epoch of the newest document.
pub fn apply_result(
    doc: &mut Option<Document>,
    epoch_counter: &mut u64,
    history_cap: usize,
    result: JobResult,
) -> Option<String> {
    match result {
        JobResult::Loaded { path, result } => match result {
            Ok(config) => {
                replace_document(doc, epoch_counter, *config, path, history_cap);
                None
            }
            Err(message) => Some(message),
        },
        JobResult::Saved {
            epoch,
            point,
            target,
            result,
        } => match result {
            Ok(()) => {
                if let Some(d) = doc.as_mut().filter(|d| d.epoch == epoch) {
                    d.history.mark_saved_at(point);
                    d.path = target;
                }
                None
            }
            Err(message) => Some(message),
        },
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::sync::mpsc;
    use std::time::Duration;

    use ad_core::command::{AddNode, Command, Transaction};
    use ad_core::history::History;
    use ad_core::network::{NetworkError, NodeFlag};

    use super::*;

    fn fixture_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../AutoDrive_config.xml")
    }

    fn inline_text(x: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"no\"?>\n<AutoDrive>\n    \
             <version>3</version>\n    <waypoints>\n        <id>1,2</id>\n        \
             <x>{x}</x>\n        <y>0.000,0.000</y>\n        <z>-3.250,4.000</z>\n        \
             <out>2;-1</out>\n        <incoming>-1;1</incoming>\n        \
             <flags>0,1</flags>\n    </waypoints>\n</AutoDrive>\n"
        )
    }

    fn config() -> LoadedConfig {
        config_xml::load(inline_text("1.000,2.500").as_bytes()).expect("inline config loads")
    }

    fn edit() -> Transaction {
        Transaction::new(vec![Command::AddNode(AddNode::new(
            1.0,
            0.0,
            2.0,
            NodeFlag::REGULAR,
        ))])
    }

    fn loaded(doc: &mut Option<Document>, counter: &mut u64) {
        let result = JobResult::Loaded {
            path: fixture_path(),
            result: Ok(Box::new(config())),
        };
        assert_eq!(apply_result(doc, counter, 500, result), None);
    }

    fn saved(doc: &Document, point: SavePoint, result: Result<(), String>) -> JobResult {
        JobResult::Saved {
            epoch: doc.epoch,
            point,
            target: PathBuf::from("target.xml"),
            result,
        }
    }

    #[test]
    fn read_only_config_gets_a_gated_history() {
        let legacy =
            inline_text("1.000,2.500").replace("<version>3</version>", "<markerID>1</markerID>");
        let config = config_xml::load(legacy.as_bytes()).expect("legacy loads");
        assert_eq!(
            config.read_only(),
            Some(&config_xml::ReadOnlyReason::Legacy)
        );
        let (mut doc, mut counter) = (None, 0);
        let result = JobResult::Loaded {
            path: fixture_path(),
            result: Ok(Box::new(config)),
        };
        assert_eq!(apply_result(&mut doc, &mut counter, 500, result), None);
        let d = doc.as_mut().expect("doc");
        assert!(d.history.is_read_only());
        assert_eq!(d.apply(edit()), Err(NetworkError::ReadOnly));
        assert!(!d.history.can_undo() && !d.is_dirty());
    }

    #[test]
    fn load_ok_replaces_document() {
        let (mut doc, mut counter) = (None, 0);
        loaded(&mut doc, &mut counter);
        let first = doc.as_mut().expect("doc");
        assert_eq!((first.epoch, first.is_dirty()), (1, false));
        assert!(first.config.read_only().is_none());
        first.apply(edit()).expect("edit");
        loaded(&mut doc, &mut counter);
        let second = doc.as_ref().expect("doc");
        assert_eq!((second.epoch, second.is_dirty()), (2, false));
        assert!(!second.history.can_undo());
    }

    #[test]
    fn history_cap_reaches_new_document() {
        let (mut doc, mut counter) = (None, 0);
        let result = JobResult::Loaded {
            path: fixture_path(),
            result: Ok(Box::new(config())),
        };
        assert_eq!(apply_result(&mut doc, &mut counter, 1, result), None);
        let d = doc.as_mut().expect("doc");
        d.apply(edit()).expect("edit");
        d.apply(edit()).expect("edit");
        assert!(d.history.undo(&mut d.config.network));
        assert!(!d.history.can_undo());
    }

    #[test]
    fn load_fail_keeps_document() {
        let (mut doc, mut counter) = (None, 0);
        loaded(&mut doc, &mut counter);
        let missing = load_file(Path::new("missing.xml")).expect_err("missing file");
        assert!(missing.contains("missing.xml"));
        let dir = std::env::temp_dir().join("ad_app_jobs_broken");
        std::fs::create_dir_all(&dir).expect("dir");
        let broken = dir.join("broken.xml");
        std::fs::write(&broken, inline_text("abc,2.500")).expect("write");
        let loaded_err = load_file(&broken);
        std::fs::remove_dir_all(&dir).expect("cleanup");
        let message = loaded_err.expect_err("invalid value");
        assert!(message.contains("broken.xml"), "{message}");
        assert!(message.contains("<x>"), "{message}");
        let result = JobResult::Loaded {
            path: broken,
            result: Err(message.clone()),
        };
        assert_eq!(
            apply_result(&mut doc, &mut counter, 500, result),
            Some(message)
        );
        let d = doc.as_mut().expect("doc");
        assert_eq!((d.epoch, counter, d.is_dirty()), (1, 1, false));
        assert_eq!(d.path, fixture_path());
        d.apply(edit()).expect("still editable");
    }

    #[test]
    fn save_ok_marks_clean_and_sets_path() {
        let (mut doc, mut counter) = (None, 0);
        loaded(&mut doc, &mut counter);
        let d = doc.as_mut().expect("doc");
        d.apply(edit()).expect("edit");
        let point = d.history.save_point();
        let result = saved(d, point, Ok(()));
        assert_eq!(apply_result(&mut doc, &mut counter, 500, result), None);
        let d = doc.as_ref().expect("doc");
        assert!(!d.is_dirty());
        assert_eq!(d.path, PathBuf::from("target.xml"));
    }

    #[test]
    fn save_then_edit_stays_dirty() {
        let (mut doc, mut counter) = (None, 0);
        loaded(&mut doc, &mut counter);
        let d = doc.as_mut().expect("doc");
        d.apply(edit()).expect("edit");
        let point = d.history.save_point();
        let result = saved(d, point, Ok(()));
        d.apply(edit()).expect("edit");
        assert_eq!(apply_result(&mut doc, &mut counter, 500, result), None);
        assert!(doc.as_ref().expect("doc").is_dirty());
    }

    #[test]
    fn stale_epoch_is_dropped() {
        let (mut doc, mut counter) = (None, 0);
        loaded(&mut doc, &mut counter);
        let d = doc.as_mut().expect("doc");
        d.apply(edit()).expect("edit");
        let point = d.history.save_point();
        let result = saved(d, point, Ok(()));
        loaded(&mut doc, &mut counter);
        let d = doc.as_mut().expect("doc");
        d.apply(edit()).expect("edit");
        assert_eq!(apply_result(&mut doc, &mut counter, 500, result), None);
        let d = doc.as_ref().expect("doc");
        assert!(d.is_dirty());
        assert_eq!(d.path, fixture_path());
    }

    #[test]
    fn save_fail_keeps_saved_state() {
        let (mut doc, mut counter) = (None, 0);
        loaded(&mut doc, &mut counter);
        let d = doc.as_mut().expect("doc");
        d.apply(edit()).expect("edit");
        let point = d.history.save_point();
        let result = saved(d, point, Err("disk full".to_owned()));
        assert_eq!(
            apply_result(&mut doc, &mut counter, 500, result),
            Some("disk full".to_owned())
        );
        let d = doc.as_ref().expect("doc");
        assert!(d.is_dirty());
        assert_eq!(d.path, fixture_path());
    }

    #[test]
    fn saved_base_evicted_stays_dirty() {
        let (mut doc, mut counter) = (None, 0);
        loaded(&mut doc, &mut counter);
        let d = doc.as_mut().expect("doc");
        d.history = History::new(1);
        let point = d.history.save_point();
        d.apply(edit()).expect("edit");
        d.apply(edit()).expect("edit");
        let result = saved(d, point, Ok(()));
        assert_eq!(apply_result(&mut doc, &mut counter, 500, result), None);
        assert!(doc.as_ref().expect("doc").is_dirty());
    }

    #[test]
    fn load_through_channel() {
        let (tx, rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        spawn_load(fixture_path(), tx, move || {
            let _ = done_tx.send(());
        });
        done_rx
            .recv_timeout(Duration::from_secs(30))
            .expect("job finished");
        let (mut doc, mut counter) = (None, 0);
        let result = rx.try_recv().expect("result queued before notify");
        assert_eq!(apply_result(&mut doc, &mut counter, 500, result), None);
        assert!(doc.is_some_and(|d| !d.config.network.is_empty()));
    }
}
