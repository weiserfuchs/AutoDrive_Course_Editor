//! Background load and save jobs, and how their results apply (AD-9, AD-16, AD-18).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread;

use ad_core::geometry::MapGeometry;
use ad_core::heightmap::{Heightmap, calibrate};
use ad_core::history::SavePoint;
use ad_core::map_context::MapContext;
use ad_io::config_xml::{self, GameVersion, LoadedConfig};
use ad_io::dds;
use ad_io::map_image::{self, MapImage};
use ad_io::settings::Settings;
use ad_io::vehicles::{self, VehiclesFile};

use crate::document::Document;

/// A heightmap with its calibrated height scale.
pub type CalibratedHeightmap = (Arc<Heightmap>, f64);

/// A parsed config with its map image, the calibrated heightmap if one was found, the
/// notice about image, heightmap and `vehicles.xml`, if any, and the `vehicles.xml` beside
/// the config, if it was read.
pub type Loaded = (
    LoadedConfig,
    MapImage,
    Option<CalibratedHeightmap>,
    Option<String>,
    Option<Arc<VehiclesFile>>,
);

/// A `vehicles.xml` to patch after the config save, and where it is.
pub type VehiclesSave = (Arc<VehiclesFile>, PathBuf);

/// An encoded map image and the path it would replace.
pub type ImageSave = (PathBuf, Vec<u8>);

/// A DDS decoded, cropped and scaled to 2048x2048; `replace` is set when the target already
/// existed, so it was not written.
pub struct Imported {
    pub image: MapImage,
    pub replace: Option<ImageSave>,
}

pub enum JobResult {
    Loaded {
        path: PathBuf,
        result: Result<Box<Loaded>, String>,
    },
    Saved {
        epoch: u64,
        point: SavePoint,
        target: PathBuf,
        result: Result<(), String>,
        /// Why `vehicles.xml` was not saved after the config was.
        warning: Option<String>,
    },
    Imported {
        epoch: u64,
        result: Result<Box<Imported>, String>,
    },
    /// The write of an imported map image over an existing one.
    ImageSaved { result: Result<(), String> },
}

/// The folder of the running executable, where `mapImages` is looked up first (AD-14).
pub fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
}

/// Where an image imported for `doc` is saved; an error when the config names no map.
pub fn import_target(doc: &Document, exe_dir: Option<&Path>) -> Result<PathBuf, String> {
    let name = doc
        .config
        .map_name()
        .ok_or("Cannot import the map image: the config names no map")?;
    let dir = exe_dir.ok_or("Cannot import the map image: the program folder is unknown")?;
    Ok(map_image::map_image_target(dir, name))
}

/// Reads, decodes, crops (by `version`, AD-11) and scales the DDS, then saves it as PNG to
/// `target` unless a file is there already. Any failure names the file.
pub fn import_dds(path: &Path, version: GameVersion, target: &Path) -> Result<Imported, String> {
    let fail = |why: &dyn std::fmt::Display| format!("Cannot import {}: {why}", path.display());
    let bytes = std::fs::read(path).map_err(|e| fail(&e))?;
    let decoded = dds::decode(&bytes).map_err(|e| fail(&e))?;
    let image = map_image::to_2048(map_image::crop_for(version, decoded));
    let png = map_image::encode_png(&image).map_err(|e| fail(&e))?;
    if target.exists() {
        return Ok(Imported {
            image,
            replace: Some((target.to_path_buf(), png)),
        });
    }
    map_image::save_map_image(target, &png)
        .map_err(|e| format!("Cannot save {}: {e}", target.display()))?;
    Ok(Imported {
        image,
        replace: None,
    })
}

/// Runs `import_dds` on a background thread, then calls `notify`.
pub fn spawn_import(
    path: PathBuf,
    version: GameVersion,
    target: PathBuf,
    epoch: u64,
    tx: Sender<JobResult>,
    notify: impl FnOnce() + Send + 'static,
) {
    thread::spawn(move || {
        tracing::info!("importing {} as {version:?}", path.display());
        let result = import_dds(&path, version, &target).map(Box::new);
        match &result {
            Ok(_) => tracing::info!("imported {}", path.display()),
            Err(e) => tracing::warn!("{e}"),
        }
        let _ = tx.send(JobResult::Imported { epoch, result });
        notify();
    });
}

/// Writes an imported map image over the existing one on a background thread.
pub fn spawn_image_save(
    (target, png): ImageSave,
    tx: Sender<JobResult>,
    notify: impl FnOnce() + Send + 'static,
) {
    thread::spawn(move || {
        let result = map_image::save_map_image(&target, &png)
            .map_err(|e| format!("Cannot save {}: {e}", target.display()));
        match &result {
            Ok(()) => tracing::info!("saved {}", target.display()),
            Err(e) => tracing::warn!("{e}"),
        }
        let _ = tx.send(JobResult::ImageSaved { result });
        notify();
    });
}

/// Reads and parses the file, then finds and loads the map image and the heightmap. The error
/// message names the file and the config problem; image and heightmap problems never fail the
/// load (they become a notice).
pub fn load_file(
    path: &Path,
    exe_dir: Option<&Path>,
    settings: &Settings,
) -> Result<Loaded, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    let mut config =
        config_xml::load(&bytes).map_err(|e| format!("Cannot load {}: {e}", path.display()))?;
    let (vehicles, vehicles_notice) = load_vehicles(path, &mut config);
    let found = config
        .map_name()
        .and_then(|name| map_image::find_map_image(name, exe_dir, path.parent()));
    let (image, notice) = match found {
        Some(image_path) => map_image::load_map_image(&image_path),
        None => (map_image::blank(), None),
    };
    let (heightmap, height_notice) = load_heightmap(&config, exe_dir, path.parent(), settings);
    let notices: Vec<String> = [notice, height_notice, vehicles_notice]
        .into_iter()
        .flatten()
        .collect();
    let notice = (!notices.is_empty()).then(|| notices.join("\n"));
    Ok((config, image, heightmap, notice, vehicles))
}

/// Reads the `vehicles.xml` beside the config into its markers (AD-17). A missing file
/// gives none; one that cannot be read gives none and a notice, so it is never written.
fn load_vehicles(
    config_path: &Path,
    config: &mut LoadedConfig,
) -> (Option<Arc<VehiclesFile>>, Option<String>) {
    let path = config_path.with_file_name("vehicles.xml");
    if !path.is_file() {
        return (None, None);
    }
    let result = std::fs::read(&path)
        .map_err(|e| e.to_string())
        .and_then(|bytes| {
            vehicles::load(&bytes, config.game_version(), &mut config.network)
                .map_err(|e| e.to_string())
        });
    match result {
        Ok(file) => (Some(Arc::new(file)), None),
        Err(e) => {
            tracing::warn!("cannot read {}: {e}", path.display());
            (
                None,
                Some(format!(
                    "{} could not be read ({e}); parking is not shown and the file will not be saved",
                    path.display()
                )),
            )
        }
    }
}

/// Whether `a` and `b` name the same file: compared canonicalised when both exist.
fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// The `vehicles.xml` a save to `target` patches: only that of the opened config, and only
/// when the save goes to the opened path.
pub fn vehicles_to_patch(doc: &Document, target: &Path) -> Option<VehiclesSave> {
    let file = doc.vehicles.clone()?;
    same_file(target, &doc.opened_path)
        .then(|| (file, doc.opened_path.with_file_name("vehicles.xml")))
}

/// Finds and decodes the heightmap; a missing or broken one gives no heightmap and a notice.
fn load_heightmap(
    config: &LoadedConfig,
    exe_dir: Option<&Path>,
    config_dir: Option<&Path>,
    settings: &Settings,
) -> (Option<CalibratedHeightmap>, Option<String>) {
    let name = config.map_name();
    match map_image::find_heightmap(name, exe_dir, config_dir) {
        Some(path) => match map_image::load_heightmap(&path) {
            Ok(heightmap) => {
                let scale = name
                    .and_then(|n| settings.maps.get(n))
                    .map_or(1, |m| m.scale);
                let geometry = MapGeometry::new(f64::from(scale.max(1)));
                let height_scale = calibrate(&heightmap, &config.network, geometry);
                (Some((Arc::new(heightmap), height_scale)), None)
            }
            Err(message) => (None, Some(message)),
        },
        None => (
            None,
            Some(format!(
                "No heightmap found for {}; new nodes will have no height",
                name.unwrap_or("this map")
            )),
        ),
    }
}

/// Runs `load_file` on a background thread, then calls `notify` (a repaint request).
pub fn spawn_load(
    path: PathBuf,
    exe_dir: Option<PathBuf>,
    settings: Settings,
    tx: Sender<JobResult>,
    notify: impl FnOnce() + Send + 'static,
) {
    thread::spawn(move || {
        tracing::info!("loading {}", path.display());
        let result = load_file(&path, exe_dir.as_deref(), &settings).map(Box::new);
        match &result {
            Ok(_) => tracing::info!("loaded {}", path.display()),
            Err(e) => tracing::warn!("{e}"),
        }
        // The receiver is gone only when the app is closing.
        let _ = tx.send(JobResult::Loaded { path, result });
        notify();
    });
}

/// Saves a snapshot on a background thread, then patches `vehicles` if the config save
/// succeeded and the file is still there, then calls `notify`.
pub fn spawn_save(
    snapshot: LoadedConfig,
    vehicles: Option<VehiclesSave>,
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
        let warning = match (&result, vehicles) {
            (Ok(()), Some((file, path))) if path.is_file() => {
                match vehicles::save_to(&file, &snapshot.network, &path) {
                    Ok(()) => {
                        tracing::info!("saved {}", path.display());
                        None
                    }
                    Err(e) => {
                        tracing::warn!("{e}");
                        Some(format!(
                            "The config was saved, but vehicles.xml was not: {e}"
                        ))
                    }
                }
            }
            _ => None,
        };
        let _ = tx.send(JobResult::Saved {
            epoch,
            point,
            target,
            result,
            warning,
        });
        notify();
    });
}

/// The single place a document is replaced (AD-18). The new epoch makes `App::poll_jobs`
/// cancel the active tool; later stories clear selection and hover here.
pub fn replace_document(
    doc: &mut Option<Document>,
    epoch_counter: &mut u64,
    loaded: Loaded,
    path: PathBuf,
    settings: &Settings,
) {
    let (config, image, heightmap, notice, vehicles) = loaded;
    let map_settings = config
        .map_name()
        .and_then(|name| settings.maps.get(name))
        .cloned()
        .unwrap_or_default();
    let mut map = MapContext::new(
        config.map_name().map(str::to_owned),
        map_settings.scale,
        map_settings.node_size,
        (map_settings.grid_x, map_settings.grid_y),
    );
    if let Some((heightmap, height_map_scale)) = heightmap {
        map.heightmap = Some(heightmap);
        map.height_map_scale = height_map_scale;
    }
    *epoch_counter += 1;
    let mut new = Document::new(
        config,
        map,
        image,
        notice,
        path,
        *epoch_counter,
        settings.global.history_cap,
    );
    new.vehicles = vehicles;
    *doc = Some(new);
}

/// Applies a finished job on the UI thread. Returns an error message to show, if any.
/// `epoch_counter` holds the epoch of the newest document.
pub fn apply_result(
    doc: &mut Option<Document>,
    epoch_counter: &mut u64,
    settings: &Settings,
    result: JobResult,
) -> Option<String> {
    match result {
        JobResult::Loaded { path, result } => match result {
            Ok(config) => {
                replace_document(doc, epoch_counter, *config, path, settings);
                None
            }
            Err(message) => Some(message),
        },
        JobResult::Saved {
            epoch,
            point,
            target,
            result,
            warning,
        } => match result {
            Ok(()) => {
                if let Some(d) = doc.as_mut().filter(|d| d.epoch == epoch) {
                    d.history.mark_saved_at(point);
                    d.path = target;
                }
                warning
            }
            Err(message) => Some(message),
        },
        // The image changes neither the config nor the history; a result for a document
        // replaced meanwhile is dropped (AD-18).
        JobResult::Imported { epoch, result } => {
            let d = doc.as_mut().filter(|d| d.epoch == epoch)?;
            match result {
                Ok(imported) => {
                    d.map_image = Some(imported.image);
                    d.image_to_replace = imported.replace;
                    None
                }
                Err(message) => Some(message),
            }
        }
        JobResult::ImageSaved { result } => result.err(),
    }
}

/// An 8x8 A8R8G8B8 DDS filled with `rgba`.
#[cfg(test)]
pub(crate) fn test_dds([r, g, b, a]: [u8; 4]) -> Vec<u8> {
    let mut file = Vec::new();
    let mut put = |v: u32| file.extend_from_slice(&v.to_le_bytes());
    put(u32::from_le_bytes(*b"DDS "));
    put(124);
    put(0x1007); // CAPS | HEIGHT | WIDTH | PIXELFORMAT
    put(8);
    put(8);
    (0..14).for_each(|_| put(0)); // pitch, depth, mipmaps, reserved
    put(32);
    put(0x41); // RGB | ALPHAPIXELS
    put(0);
    put(32);
    [0xff_0000, 0xff00, 0xff, 0xff00_0000]
        .into_iter()
        .for_each(&mut put);
    put(0x1000); // caps: TEXTURE
    (0..4).for_each(|_| put(0));
    (0..64).for_each(|_| file.extend_from_slice(&[b, g, r, a]));
    file
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::sync::mpsc;
    use std::time::Duration;

    use ad_core::command::{AddNodes, Command, Transaction};
    use ad_core::heightmap::sample_height;
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

    fn settings(history_cap: usize) -> Settings {
        let mut s = Settings::default();
        s.global.history_cap = history_cap;
        s
    }

    fn loaded_config(config: LoadedConfig) -> Loaded {
        (config, map_image::blank(), None, None, None)
    }

    fn config() -> LoadedConfig {
        config_xml::load(inline_text("1.000,2.500").as_bytes()).expect("inline config loads")
    }

    fn edit() -> Transaction {
        Transaction::new(vec![Command::AddNodes(AddNodes::new(vec![(
            1.0,
            0.0,
            2.0,
            NodeFlag::REGULAR,
        )]))])
    }

    fn loaded(doc: &mut Option<Document>, counter: &mut u64) {
        let result = JobResult::Loaded {
            path: fixture_path(),
            result: Ok(Box::new(loaded_config(config()))),
        };
        assert_eq!(apply_result(doc, counter, &settings(500), result), None);
    }

    fn saved(doc: &Document, point: SavePoint, result: Result<(), String>) -> JobResult {
        JobResult::Saved {
            epoch: doc.epoch,
            point,
            target: PathBuf::from("target.xml"),
            result,
            warning: None,
        }
    }

    /// Loads `text` through `apply_result` into a fresh document.
    fn open(text: &str) -> Document {
        let config = config_xml::load(text.as_bytes()).expect("config loads");
        let (mut doc, mut counter) = (None, 0);
        let result = JobResult::Loaded {
            path: fixture_path(),
            result: Ok(Box::new(loaded_config(config))),
        };
        assert_eq!(
            apply_result(&mut doc, &mut counter, &settings(500), result),
            None
        );
        doc.expect("doc")
    }

    #[test]
    fn read_only_config_gets_a_gated_history() {
        let spline = inline_text("1.000,2.500").replace("<flags>0,1</flags>", "<flags>2,4</flags>");
        let cases = [
            (
                spline.replace("<version>3</version>", "<markerID>1</markerID>"),
                config_xml::ReadOnlyReason::Legacy,
            ),
            (
                spline.replace("<version>3</version>", "<version>9</version>"),
                config_xml::ReadOnlyReason::UnknownVersion("9".to_owned()),
            ),
        ];
        for (text, reason) in cases {
            let mut d = open(&text);
            assert_eq!(d.config.read_only(), Some(&reason));
            assert!(d.history.is_read_only());
            assert_eq!(d.apply(edit()), Err(NetworkError::ReadOnly));
            assert!(!d.history.can_undo() && !d.is_dirty(), "{reason}");
        }
    }

    #[test]
    fn flag_reset_on_load_starts_dirty() {
        let fs25 = inline_text("1.000,2.500");
        for flags in ["<flags>2,1</flags>", "<flags>0,4</flags>"] {
            let mut d = open(&fs25.replace("<flags>0,1</flags>", flags));
            assert!(d.is_dirty(), "{flags}");
            assert!(!d.history.can_undo());
            assert!(!d.history.undo(&mut d.config.network));
            assert!(d.is_dirty());
        }
        assert!(!open(&fs25).is_dirty());
        let fs19 = fs25
            .replace("<version>3</version>", "<version>1.1.0.8</version>")
            .replace("<flags>0,1</flags>", "<flags>2,1</flags>");
        assert!(!open(&fs19).is_dirty());
    }

    #[test]
    fn save_of_reset_document_makes_it_clean() {
        let text = inline_text("1.000,2.500").replace("<flags>0,1</flags>", "<flags>2,1</flags>");
        let mut doc = Some(open(&text));
        let mut counter = 1;
        let d = doc.as_ref().expect("doc");
        assert!(d.is_dirty());
        let result = saved(d, d.history.save_point(), Ok(()));
        assert_eq!(
            apply_result(&mut doc, &mut counter, &settings(500), result),
            None
        );
        assert!(!doc.expect("doc").is_dirty());
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
            result: Ok(Box::new(loaded_config(config()))),
        };
        assert_eq!(
            apply_result(&mut doc, &mut counter, &settings(1), result),
            None
        );
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
        let missing =
            load_file(Path::new("missing.xml"), None, &settings(500)).expect_err("missing file");
        assert!(missing.contains("missing.xml"));
        let dir = std::env::temp_dir().join("ad_app_jobs_broken");
        std::fs::create_dir_all(&dir).expect("dir");
        let broken = dir.join("broken.xml");
        std::fs::write(&broken, inline_text("abc,2.500")).expect("write");
        let loaded_err = load_file(&broken, None, &settings(500));
        std::fs::remove_dir_all(&dir).expect("cleanup");
        let message = loaded_err.expect_err("invalid value");
        assert!(message.contains("broken.xml"), "{message}");
        assert!(message.contains("<x>"), "{message}");
        let result = JobResult::Loaded {
            path: broken,
            result: Err(message.clone()),
        };
        assert_eq!(
            apply_result(&mut doc, &mut counter, &settings(500), result),
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
        assert_eq!(
            apply_result(&mut doc, &mut counter, &settings(500), result),
            None
        );
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
        assert_eq!(
            apply_result(&mut doc, &mut counter, &settings(500), result),
            None
        );
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
        assert_eq!(
            apply_result(&mut doc, &mut counter, &settings(500), result),
            None
        );
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
            apply_result(&mut doc, &mut counter, &settings(500), result),
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
        assert_eq!(
            apply_result(&mut doc, &mut counter, &settings(500), result),
            None
        );
        assert!(doc.as_ref().expect("doc").is_dirty());
    }

    #[test]
    fn load_through_channel() {
        let (tx, rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        spawn_load(fixture_path(), None, settings(500), tx, move || {
            let _ = done_tx.send(());
        });
        done_rx
            .recv_timeout(Duration::from_secs(30))
            .expect("job finished");
        let (mut doc, mut counter) = (None, 0);
        let result = rx.try_recv().expect("result queued before notify");
        assert_eq!(
            apply_result(&mut doc, &mut counter, &settings(500), result),
            None
        );
        assert!(doc.is_some_and(|d| !d.config.network.is_empty()));
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ad_app_jobs_{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        dir
    }

    fn named(text: &str) -> String {
        text.replace("<AutoDrive>", "<AutoDrive><MapName>Tiny</MapName>")
    }

    #[test]
    fn job_result_carries_the_image() {
        let dir = temp_dir("carries");
        let folder = dir.join("mapImages").join("Tiny");
        std::fs::create_dir_all(&folder).expect("folder");
        image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255]))
            .save(folder.join("Tiny.png"))
            .expect("png");
        let config = dir.join("AutoDrive_config.xml");
        std::fs::write(&config, named(&inline_text("1.000,2.500"))).expect("write");
        let loaded = load_file(&config, None, &settings(500));
        std::fs::remove_dir_all(&dir).expect("cleanup");
        let (_, image, _, notice, _) = loaded.expect("loads");
        assert_ne!(image, map_image::blank());
        assert_eq!(&image.rgba[..4], &[1, 2, 3, 255]);
        assert!(notice.expect("notice").contains("Tiny.png"));
    }

    #[test]
    fn missing_image_or_name_falls_back_to_blank() {
        let dir = temp_dir("fallback");
        let config = dir.join("AutoDrive_config.xml");
        for text in [
            named(&inline_text("1.000,2.500")),
            inline_text("1.000,2.500"),
        ] {
            std::fs::write(&config, text).expect("write");
            let (_, image, heightmap, notice, _) =
                load_file(&config, None, &settings(500)).expect("loads");
            assert_eq!((image, heightmap), (map_image::blank(), None));
            assert!(
                notice
                    .expect("notice")
                    .contains("new nodes will have no height")
            );
        }
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn scale_from_settings_reaches_the_map_context() {
        let text = inline_text("1.000,2.500")
            .replace("<AutoDrive>", "<AutoDrive><MapName>Marsch</MapName>");
        let mut s = settings(500);
        s.maps.insert(
            "Marsch".to_owned(),
            ad_io::settings::MapSettings {
                scale: 2,
                node_size: 3.0,
                grid_x: 4.0,
                grid_y: 0.0,
            },
        );
        let config = config_xml::load(text.as_bytes()).expect("config loads");
        let (mut doc, mut counter) = (None, 0);
        let result = JobResult::Loaded {
            path: fixture_path(),
            result: Ok(Box::new(loaded_config(config))),
        };
        assert_eq!(apply_result(&mut doc, &mut counter, &s, result), None);
        let d = doc.expect("doc");
        assert_eq!(d.map.geometry.scale, 2.0);
        assert_eq!(d.map.node_size, 3.0);
        assert_eq!(d.map.grid, (4.0, 0.1));
        assert_eq!(d.map.name.as_deref(), Some("Marsch"));
        assert!(d.map_image.is_some() && d.map.heightmap.is_none());
        // A missing name uses scale 1.
        let plain = open(&inline_text("1.000,2.500")).map;
        assert_eq!((plain.geometry.scale, plain.node_size), (1.0, 2.0));
        assert_eq!(plain.grid, (2.0, 2.0));
    }

    #[test]
    fn heightmap_beside_the_config_reaches_the_map_context() {
        let dir = temp_dir("heightmap");
        image::ImageBuffer::<image::Luma<u16>, _>::from_pixel(4, 4, image::Luma([25600]))
            .save(dir.join("terrain.heightmap.png"))
            .expect("png");
        let config = dir.join("AutoDrive_config.xml");
        std::fs::write(&config, inline_text("1.000,2.500")).expect("write");
        let loaded = load_file(&config, None, &settings(500));
        std::fs::remove_dir_all(&dir).expect("cleanup");
        let loaded = loaded.expect("loads");
        assert!(loaded.2.is_some() && loaded.3.is_none());
        let (mut doc, mut counter) = (None, 0);
        let result = JobResult::Loaded {
            path: config,
            result: Ok(Box::new(loaded)),
        };
        assert_eq!(
            apply_result(&mut doc, &mut counter, &settings(500), result),
            None
        );
        let d = doc.expect("doc");
        assert_eq!(sample_height(&d.map, 1.0, -3.25), Some(100.0));
        assert_eq!(d.map.height_map_scale, 1.0);
    }

    #[test]
    fn broken_heightmap_gives_a_notice_not_an_error() {
        let dir = temp_dir("heightmap_broken");
        std::fs::write(dir.join("terrain.heightmap.png"), b"nope").expect("write");
        let config = dir.join("AutoDrive_config.xml");
        std::fs::write(&config, inline_text("1.000,2.500")).expect("write");
        let loaded = load_file(&config, None, &settings(500));
        std::fs::remove_dir_all(&dir).expect("cleanup");
        let (_, _, heightmap, notice, _) = loaded.expect("loads");
        assert!(heightmap.is_none());
        assert!(notice.expect("notice").contains("terrain.heightmap.png"));
    }

    /// The FS25 inline config with markers 1 (node 1) and 2 (node 2).
    fn with_markers() -> String {
        inline_text("1.000,2.500").replace(
            "</AutoDrive>",
            "    <mapmarker><mm1><id>1</id><name>A</name><group>All</group></mm1>\
             <mm2><id>2</id><name>B</name><group>All</group></mm2></mapmarker>\n</AutoDrive>",
        )
    }

    const VEHICLES: &str = "<vehicles>\r\n    <vehicle uniqueId=\"v1\">\r\n        \
        <AutoDrive parkDestination=\"2\" firstMarker=\"2\" mode=\"1\"/>\r\n    </vehicle>\r\n</vehicles>\r\n";

    /// A folder with the config and, if given, `vehicles.xml`; opened through `load_file`.
    fn open_in(name: &str, vehicles: Option<&str>) -> (PathBuf, Document, Option<String>) {
        let dir = temp_dir(name);
        let config = dir.join("AutoDrive_config.xml");
        std::fs::write(&config, with_markers()).expect("write");
        if let Some(text) = vehicles {
            std::fs::write(dir.join("vehicles.xml"), text).expect("write");
        }
        let loaded = load_file(&config, None, &settings(500)).expect("loads");
        let notice = loaded.3.clone();
        let (mut doc, mut counter) = (None, 0);
        replace_document(&mut doc, &mut counter, loaded, config, &settings(500));
        (dir, doc.expect("doc"), notice)
    }

    /// Runs a save of `doc` to `target` as `App::save` does and applies its result.
    fn save_as(doc: Document, target: &Path) -> (Document, Option<String>) {
        let (tx, rx) = mpsc::channel();
        spawn_save(
            doc.config.clone(),
            vehicles_to_patch(&doc, target),
            doc.epoch,
            doc.history.save_point(),
            target.to_path_buf(),
            tx,
            || {},
        );
        let result = rx.recv_timeout(Duration::from_secs(30)).expect("saved");
        let (mut doc, mut counter) = (Some(doc), 1);
        let message = apply_result(&mut doc, &mut counter, &settings(500), result);
        (doc.expect("doc"), message)
    }

    fn remove_first_marker(doc: &mut Document) {
        let node = doc.config.network.markers()[0].node;
        let edit = ad_core::command::EditMarker::new(node, None);
        doc.apply(Transaction::new(vec![Command::EditMarker(edit)]))
            .expect("edit");
    }

    #[test]
    fn vehicles_are_patched_only_by_a_save_to_the_opened_path() {
        let (dir, mut doc, notice) = open_in("vehicles_save", Some(VEHICLES));
        assert!(notice.is_none_or(|n| !n.contains("vehicles.xml")));
        assert!(doc.vehicles.is_some());
        assert_eq!(doc.config.network.markers()[1].parked_vehicles, ["v1"]);
        remove_first_marker(&mut doc);
        let vehicles = dir.join("vehicles.xml");
        // Save as: the config goes elsewhere and vehicles.xml stays.
        let other = dir.join("other.xml");
        let (doc, message) = save_as(doc, &other);
        assert_eq!(message, None);
        assert!(other.is_file());
        assert_eq!(std::fs::read_to_string(&vehicles).expect("read"), VEHICLES);
        // A later save to the new path is still not to the opened config.
        let (doc, _) = save_as(doc, &other);
        assert_eq!(std::fs::read_to_string(&vehicles).expect("read"), VEHICLES);
        // Save to the opened path, spelled differently: patched.
        let opened = dir.join(".").join("AutoDrive_config.xml");
        let (doc, message) = save_as(doc, &opened);
        let saved = std::fs::read_to_string(&vehicles).expect("read");
        std::fs::remove_dir_all(&dir).expect("cleanup");
        assert_eq!(message, None);
        assert!(!doc.is_dirty());
        assert_eq!(
            saved,
            VEHICLES.replace(
                "parkDestination=\"2\" firstMarker=\"2\"",
                "parkDestination=\"1\" firstMarker=\"1\""
            )
        );
    }

    #[test]
    fn no_vehicles_file_loads_and_saves_as_before() {
        let (dir, mut doc, notice) = open_in("vehicles_none", None);
        assert!(notice.is_none_or(|n| !n.contains("vehicles.xml")));
        assert!(doc.vehicles.is_none());
        remove_first_marker(&mut doc);
        let config = dir.join("AutoDrive_config.xml");
        let (doc, message) = save_as(doc, &config);
        let written = dir.join("vehicles.xml").exists();
        std::fs::remove_dir_all(&dir).expect("cleanup");
        assert_eq!(message, None);
        assert!(!doc.is_dirty() && !written);
    }

    #[test]
    fn broken_vehicles_file_gives_a_notice_and_is_never_written() {
        let broken = VEHICLES.replace("</vehicle>", "");
        let (dir, mut doc, notice) = open_in("vehicles_broken", Some(&broken));
        let notice = notice.expect("notice");
        assert!(notice.contains("vehicles.xml"), "{notice}");
        assert_eq!(doc.notice.as_deref(), Some(notice.as_str()));
        assert!(doc.vehicles.is_none());
        assert!(doc.config.network.markers()[1].parked_vehicles.is_empty());
        remove_first_marker(&mut doc);
        let config = dir.join("AutoDrive_config.xml");
        let (_, message) = save_as(doc, &config);
        let after = std::fs::read_to_string(dir.join("vehicles.xml")).expect("read");
        std::fs::remove_dir_all(&dir).expect("cleanup");
        assert_eq!(message, None);
        assert_eq!(after, broken);
    }

    #[test]
    fn a_vehicles_warning_is_shown_and_the_config_save_still_counts() {
        let (mut doc, mut counter) = (None, 0);
        loaded(&mut doc, &mut counter);
        let d = doc.as_mut().expect("doc");
        d.apply(edit()).expect("edit");
        let result = JobResult::Saved {
            epoch: d.epoch,
            point: d.history.save_point(),
            target: PathBuf::from("target.xml"),
            result: Ok(()),
            warning: Some("vehicles.xml was not saved".to_owned()),
        };
        assert_eq!(
            apply_result(&mut doc, &mut counter, &settings(500), result),
            Some("vehicles.xml was not saved".to_owned())
        );
        assert!(!doc.expect("doc").is_dirty());
    }

    #[test]
    fn the_import_target_needs_a_map_name() {
        let exe = Path::new("exe");
        let message =
            import_target(&open(&inline_text("1.000,2.500")), Some(exe)).expect_err("no map name");
        assert!(message.contains("the config names no map"), "{message}");
        let doc = open(&named(&inline_text("1.000,2.500")));
        assert_eq!(
            import_target(&doc, Some(exe)),
            Ok(exe.join("mapImages").join("Tiny").join("Tiny.png"))
        );
        assert!(import_target(&doc, None).is_err());
    }

    #[test]
    fn an_import_saves_a_new_target_and_keeps_an_existing_one() {
        let dir = temp_dir("import");
        let dds = dir.join("map.dds");
        std::fs::write(&dds, test_dds([1, 2, 3, 255])).expect("write");
        let target = map_image::map_image_target(&dir, "Tiny");
        let imported = import_dds(&dds, GameVersion::FS19, &target).expect("imports");
        assert!(imported.replace.is_none());
        assert_eq!(map_image::load_map_image(&target), (imported.image, None));
        // Present now: not written, the PNG waits for the question.
        std::fs::write(&target, b"old").expect("write");
        let imported = import_dds(&dds, GameVersion::FS25, &target).expect("imports");
        let (path, png) = imported.replace.expect("asks");
        assert_eq!(path, target);
        assert_eq!(std::fs::read(&target).expect("read"), b"old");
        assert_eq!(&imported.image.rgba[..4], &[1, 2, 3, 255]);
        assert!(!png.is_empty());
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn a_bad_dds_is_an_error_naming_it_and_saves_nothing() {
        let dir = temp_dir("import_bad");
        let dds = dir.join("broken.dds");
        std::fs::write(&dds, &test_dds([1, 2, 3, 255])[..150]).expect("write");
        let target = map_image::map_image_target(&dir, "Tiny");
        let message = import_dds(&dds, GameVersion::FS25, &target)
            .err()
            .expect("truncated");
        let missing = import_dds(&dir.join("missing.dds"), GameVersion::FS25, &target).is_err();
        let written = target.exists();
        std::fs::remove_dir_all(&dir).expect("cleanup");
        assert!(message.contains("broken.dds"), "{message}");
        assert!(missing && !written);
    }

    /// Needs the gitignored `terrain.heightmap.png` beside the fixture.
    /// Run: `cargo test -p ad-app -- --ignored heightmap --nocapture`.
    #[test]
    #[ignore = "needs the fixture's terrain.heightmap.png"]
    fn fixture_heightmap_matches_node_heights() {
        let path = fixture_path();
        if !path.with_file_name("terrain.heightmap.png").is_file() {
            println!("terrain.heightmap.png is missing beside the fixture; skipping");
            return;
        }
        let mut s = settings(500);
        s.maps.insert(
            "NF_Marsch_4fach".to_owned(),
            ad_io::settings::MapSettings {
                scale: 2,
                ..Default::default()
            },
        );
        let loaded = load_file(&path, None, &s).expect("fixture loads");
        assert!(loaded.2.is_some(), "heightmap found: {:?}", loaded.3);
        let (mut doc, mut counter) = (None, 0);
        let result = JobResult::Loaded {
            path,
            result: Ok(Box::new(loaded)),
        };
        assert_eq!(apply_result(&mut doc, &mut counter, &s, result), None);
        let d = doc.expect("doc");
        let net = &d.config.network;
        let mut deviations: Vec<f64> = net
            .order()
            .iter()
            .filter_map(|&id| net.node(id))
            .filter(|n| n.y > 0.0)
            .filter_map(|n| sample_height(&d.map, n.x, n.z).map(|h| (h - n.y).abs()))
            .collect();
        assert!(!deviations.is_empty(), "no node with y > 0 got a sample");
        deviations.sort_by(f64::total_cmp);
        let median = deviations[deviations.len() / 2];
        let max = deviations.last().copied().unwrap_or(0.0);
        println!(
            "height_map_scale {}, nodes {}, median {median:.3}, max {max:.3}",
            d.map.height_map_scale,
            deviations.len()
        );
        assert!(median < 1.0, "median deviation {median}");
    }
}
