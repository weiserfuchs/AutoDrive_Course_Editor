//! Background load and save jobs, and how their results apply (AD-9, AD-16, AD-18).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread;

use ad_core::geometry::MapGeometry;
use ad_core::heightmap::{Heightmap, calibrate};
use ad_core::history::SavePoint;
use ad_core::map_context::MapContext;
use ad_core::network::NodeId;
use ad_io::config_xml::{self, GameVersion, LoadedConfig};
use ad_io::dds;
use ad_io::download::{self, DownloadError};
use ad_io::map_image::{self, MapImage};
use ad_io::routes_xml;
use ad_io::settings::Settings;
use ad_io::vehicles::{self, VehiclesFile};

use crate::document::Document;
use crate::zip_import;

/// A heightmap with its calibrated height scale.
pub type CalibratedHeightmap = (Arc<Heightmap>, f64);

/// A parsed config with its map image, the calibrated heightmap if one was found, the
/// notice about image, heightmap and `vehicles.xml`, if any, and the `vehicles.xml` beside
/// the config, if it was read, and whether a local map image was found (false: the image
/// is Blank and may be downloaded).
pub type Loaded = (
    LoadedConfig,
    MapImage,
    Option<CalibratedHeightmap>,
    Option<String>,
    Option<Arc<VehiclesFile>>,
    bool,
);

/// A downloaded map image, decoded and scaled, and its notice (scaled, or not saved).
pub struct Downloaded {
    pub image: MapImage,
    pub notice: Option<String>,
}

/// Where a missing map image is downloaded from and saved to.
#[derive(Debug, Clone)]
pub struct DownloadSource {
    pub base_url: String,
    pub limits: download::Limits,
    /// `mapImages/<name>/` is saved here; without it nothing is downloaded.
    pub exe_dir: Option<PathBuf>,
}

impl Default for DownloadSource {
    fn default() -> Self {
        Self {
            base_url: download::MAP_IMAGES_URL.to_owned(),
            limits: download::Limits::default(),
            exe_dir: exe_dir(),
        }
    }
}

/// A `vehicles.xml` to patch after the config save, and where it is.
pub type VehiclesSave = (Arc<VehiclesFile>, PathBuf);

/// An encoded map image and the path it would replace.
pub type ImageSave = (PathBuf, Arc<[u8]>);

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
    /// A missing map image downloaded for the document of `epoch`.
    Downloaded {
        epoch: u64,
        result: Result<Box<Downloaded>, String>,
    },
    /// A map mod zip read for the preview; `epoch` is the open document's, if any.
    ZipRead {
        epoch: Option<u64>,
        result: Result<Box<zip_import::ZipRead>, String>,
    },
    /// The map mod images saved into `mapImages/<folder>/`.
    ZipSaved {
        epoch: Option<u64>,
        folder: String,
        result: Result<Box<zip_import::ZipSaved>, String>,
    },
    /// A PNG picked as map image, decoded with its scaling notice, for the document of
    /// `epoch`.
    ImageLoaded {
        epoch: u64,
        result: Result<Box<(MapImage, Option<String>)>, String>,
    },
    /// A PNG picked as heightmap, decoded, for the document of `epoch`.
    HeightmapLoaded {
        epoch: u64,
        result: Result<Box<Heightmap>, String>,
    },
    /// The shown map image (`image`, its PNG) or the heightmap (`image` none) written.
    Exported {
        image: Option<Arc<[u8]>>,
        result: Result<(), String>,
    },
    /// The backup before a merge of overlapping nodes written; the merge waits for it.
    MergeBackup {
        start: MergeStart,
        result: Result<(), String>,
    },
    /// An autosave of the document of `epoch` at `point`: the file written, or why not.
    Autosaved {
        epoch: u64,
        point: SavePoint,
        result: Result<PathBuf, String>,
    },
    /// The update check's answer: the latest release, or why there is none.
    UpdateChecked {
        result: Result<ad_io::update::Release, String>,
    },
    /// A `routes.xml` read for the routes window: its routes sorted by name.
    RoutesRead {
        path: PathBuf,
        result: Result<Vec<routes_xml::RouteEntry>, String>,
    },
}

/// Reads the routes of the `routes.xml` at `path`, sorted by name ignoring case.
pub fn read_routes_list(path: &Path) -> Result<Vec<routes_xml::RouteEntry>, String> {
    let cannot = |e: &dyn std::fmt::Display| format!("Cannot read {}: {e}", path.display());
    let bytes = std::fs::read(path).map_err(|e| cannot(&e))?;
    let mut routes = routes_xml::load_list(&bytes).map_err(|e| cannot(&e))?;
    routes.sort_by_cached_key(|r| r.name.to_lowercase());
    Ok(routes)
}

/// Reads the `routes.xml` at `path` on a background thread, then calls `notify`.
pub fn spawn_routes_read(
    path: PathBuf,
    tx: Sender<JobResult>,
    notify: impl FnOnce() + Send + 'static,
) {
    thread::spawn(move || {
        let result = read_routes_list(&path);
        let _ = tx.send(JobResult::RoutesRead { path, result });
        notify();
    });
}

/// Asks `url` for the latest release on a background thread, then calls `notify`.
pub fn spawn_update_check(
    url: String,
    tx: Sender<JobResult>,
    notify: impl FnOnce() + Send + 'static,
) {
    thread::spawn(move || {
        let result = ad_io::update::fetch_latest(&url);
        let _ = tx.send(JobResult::UpdateChecked { result });
        notify();
    });
}

/// The route file a `routes.xml` entry names: `<list folder>/routes/<fileName>`.
pub fn route_file(list: &Path, entry: &routes_xml::RouteEntry) -> PathBuf {
    list.with_file_name("routes").join(&entry.file_name)
}

/// The document state a merge's backup was written from: the merge applies only while it
/// is still the document's.
#[derive(Debug, Clone, PartialEq)]
pub struct MergeStart {
    pub epoch: u64,
    pub revision: u64,
    /// The overlap distance and marks the user confirmed.
    pub overlap: (f64, BTreeSet<NodeId>),
}

/// The notice line when a merge was not applied because the document changed meanwhile.
pub const MERGE_CHANGED: &str =
    "Merge not applied: the document changed while the backup was written";

/// `<config dir>/<config stem>_mergeBackup.xml` (Java `saveMergeBackupConfigFile`).
pub fn merge_backup_path(config: &Path) -> PathBuf {
    let stem = config
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    config.with_file_name(format!("{stem}_mergeBackup.xml"))
}

/// Writes the backup of `snapshot` before a merge on a background thread, atomically
/// (AD-16); path, saved state and `vehicles.xml` stay as they are. Then calls `notify`.
pub fn spawn_merge_backup(
    snapshot: LoadedConfig,
    start: MergeStart,
    target: PathBuf,
    tx: Sender<JobResult>,
    notify: impl FnOnce() + Send + 'static,
) {
    thread::spawn(move || {
        tracing::info!("saving the merge backup {}", target.display());
        let result = config_xml::save_to(&snapshot, &target).map_err(|e| {
            format!(
                "Merge cancelled: cannot write the backup {}: {e}",
                target.display()
            )
        });
        match &result {
            Ok(()) => tracing::info!("saved {}", target.display()),
            Err(e) => tracing::warn!("{e}"),
        }
        let _ = tx.send(JobResult::MergeBackup { start, result });
        notify();
    });
}

/// The notice line of a failed autosave, before the reason.
pub const AUTOSAVE_FAILED: &str = "Autosave failed: ";

/// `<config dir>/<config stem>_autosave_<slot>.xml` (Java `autoSaveGameConfigFile`).
pub fn autosave_path(config: &Path, slot: u32) -> PathBuf {
    let stem = config
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    config.with_file_name(format!("{stem}_autosave_{slot}.xml"))
}

/// The slot the next autosave of `config` writes: the one after the newest existing
/// autosave file of slots 1 to `slots` (by modification time), wrapping after `slots`;
/// 1 when none exists.
pub fn next_slot(config: &Path, slots: u32) -> u32 {
    let slots = slots.max(1);
    let newest = (1..=slots)
        .filter_map(|slot| {
            let modified = std::fs::metadata(autosave_path(config, slot))
                .and_then(|m| m.modified())
                .ok()?;
            Some((modified, slot))
        })
        .max();
    newest.map_or(1, |(_, slot)| slot % slots + 1)
}

/// Writes `snapshot` to the next autosave slot of `config` on a background thread,
/// atomically (AD-16); path, saved state, `vehicles.xml` and the config itself stay as
/// they are. Then calls `notify`.
pub fn spawn_autosave(
    snapshot: LoadedConfig,
    epoch: u64,
    point: SavePoint,
    config: PathBuf,
    slots: u32,
    tx: Sender<JobResult>,
    notify: impl FnOnce() + Send + 'static,
) {
    thread::spawn(move || {
        let target = autosave_path(&config, next_slot(&config, slots));
        tracing::info!("autosaving {}", target.display());
        let result = match config_xml::save_to(&snapshot, &target) {
            Ok(()) => {
                tracing::info!("autosaved {}", target.display());
                Ok(target)
            }
            Err(e) => {
                tracing::warn!("autosave failed: {e}");
                Err(e.to_string())
            }
        };
        let _ = tx.send(JobResult::Autosaved {
            epoch,
            point,
            result,
        });
        notify();
    });
}

/// What an export writes.
pub enum Export {
    /// The shown map image's PNG bytes, written as they are.
    MapImage(Arc<[u8]>),
    /// The heightmap, encoded as 16-bit grey PNG.
    Heightmap(Arc<Heightmap>),
}

/// The notice line after a map image was loaded by hand; a save of that image drops it.
pub const NOT_SAVED: &str = "Map image loaded, not saved — use Save map image to keep it";

/// How the load's notice for a route without a map name starts; the reason follows.
pub const NO_ROUTE_MAP_NAME: &str = "No map name for this route";

/// How the load's notice for a missing heightmap starts; an attached heightmap drops it.
pub const NO_HEIGHTMAP: &str = "No heightmap found";

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

/// Where Save map image (`heightmap` false) or Export heightmap starts the dialog:
/// `mapImages/<name>/<name>.png` or `<name>_HeightMap.png` in `exe_dir`; without a map name
/// `mapImages/unknown.png` or `unknown_HeightMap.png` (Java).
pub fn export_target(exe_dir: &Path, name: Option<&str>, heightmap: bool) -> PathBuf {
    match (name, heightmap) {
        (Some(name), false) => map_image::map_image_target(exe_dir, name),
        (Some(name), true) => map_image::heightmap_target(exe_dir, name),
        (None, false) => exe_dir.join("mapImages").join("unknown.png"),
        (None, true) => exe_dir.join("mapImages").join("unknown_HeightMap.png"),
    }
}

/// `path`, with `.png` added when it has another or no extension.
pub fn with_png_extension(path: PathBuf) -> PathBuf {
    let is_png = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("png"));
    if is_png {
        return path;
    }
    let mut name = path.into_os_string();
    name.push(".png");
    PathBuf::from(name)
}

/// Reads and decodes a PNG as the map image, scaled as a local image is (AD-14). Not saved.
pub fn load_image_file(path: &Path) -> Result<(MapImage, Option<String>), String> {
    let fail =
        |why: &dyn std::fmt::Display| format!("Cannot load {} as map image: {why}", path.display());
    let bytes = std::fs::read(path).map_err(|e| fail(&e))?;
    map_image::decode_map_image(&bytes, &path.display().to_string()).map_err(|e| fail(&e))
}

/// Runs `load_image_file` on a background thread, then calls `notify`.
pub fn spawn_image_load(
    path: PathBuf,
    epoch: u64,
    tx: Sender<JobResult>,
    notify: impl FnOnce() + Send + 'static,
) {
    thread::spawn(move || {
        tracing::info!("loading map image {}", path.display());
        let result = load_image_file(&path).map(Box::new);
        if let Err(e) = &result {
            tracing::warn!("{e}");
        }
        let _ = tx.send(JobResult::ImageLoaded { epoch, result });
        notify();
    });
}

/// Decodes a PNG as heightmap on a background thread, then calls `notify`. Not saved.
pub fn spawn_heightmap_load(
    path: PathBuf,
    epoch: u64,
    tx: Sender<JobResult>,
    notify: impl FnOnce() + Send + 'static,
) {
    thread::spawn(move || {
        tracing::info!("loading heightmap {}", path.display());
        let result = map_image::load_heightmap(&path).map(Box::new);
        if let Err(e) = &result {
            tracing::warn!("{e}");
        }
        let _ = tx.send(JobResult::HeightmapLoaded { epoch, result });
        notify();
    });
}

/// Writes `what` to `target` atomically, creating its folders. The error names the file.
pub fn export(what: &Export, target: &Path) -> Result<(), String> {
    let fail = |why: &dyn std::fmt::Display| format!("Cannot save {}: {why}", target.display());
    match what {
        Export::MapImage(png) => map_image::save_map_image(target, png),
        Export::Heightmap(heightmap) => {
            let png = map_image::encode_heightmap(heightmap).map_err(|e| fail(&e))?;
            map_image::save_map_image(target, &png)
        }
    }
    .map_err(|e| fail(&e))
}

/// Runs `export` on a background thread, then calls `notify`.
pub fn spawn_export(
    what: Export,
    target: PathBuf,
    tx: Sender<JobResult>,
    notify: impl FnOnce() + Send + 'static,
) {
    thread::spawn(move || {
        let result = export(&what, &target);
        match &result {
            Ok(()) => tracing::info!("saved {}", target.display()),
            Err(e) => tracing::warn!("{e}"),
        }
        let image = match what {
            Export::MapImage(png) => Some(png),
            Export::Heightmap(_) => None,
        };
        let _ = tx.send(JobResult::Exported { image, result });
        notify();
    });
}

/// Reads, decodes, crops (by `version`, AD-11) and scales the DDS, then saves it as PNG to
/// `target` unless a file is there already. Any failure names the file.
pub fn import_dds(path: &Path, version: GameVersion, target: &Path) -> Result<Imported, String> {
    let fail = |why: &dyn std::fmt::Display| format!("Cannot import {}: {why}", path.display());
    let bytes = std::fs::read(path).map_err(|e| fail(&e))?;
    let decoded = dds::decode(&bytes).map_err(|e| fail(&e))?;
    let image = map_image::to_2048(map_image::crop_for(version, decoded)).map_err(|e| fail(&e))?;
    let png = image.png.clone();
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

/// Reads and parses the file (a config, or a route when its root is `routeExport`, its map
/// name from `routes.xml`), then finds and loads the map image and the heightmap. The error
/// message names the file and the config problem; image and heightmap problems never fail the
/// load (they become a notice).
pub fn load_file(
    path: &Path,
    exe_dir: Option<&Path>,
    settings: &Settings,
) -> Result<Loaded, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    let cannot_load = |e: &dyn std::fmt::Display| format!("Cannot load {}: {e}", path.display());
    // A route has no vehicles.xml; its notice says why it has no map name.
    let (config, vehicles, file_notice) = if routes_xml::is_route(&bytes) {
        let route = routes_xml::load(&bytes).map_err(|e| cannot_load(&e))?;
        let (map_name, notice) = match routes_xml::map_name_for(path) {
            Ok(name) => (Some(name), None),
            Err(why) => (None, Some(format!("{NO_ROUTE_MAP_NAME} ({why})"))),
        };
        (LoadedConfig::for_route(route, map_name), None, notice)
    } else {
        let mut config = config_xml::load(&bytes).map_err(|e| cannot_load(&e))?;
        let (vehicles, notice) = load_vehicles(path, &mut config);
        (config, vehicles, notice)
    };
    let found = config
        .map_name()
        .and_then(|name| map_image::find_map_image(name, exe_dir, path.parent()));
    let image_found = found.is_some();
    let (image, notice) = match found {
        Some(image_path) => map_image::load_map_image(&image_path),
        None => (map_image::blank(), None),
    };
    let (heightmap, height_notice) = load_heightmap(&config, exe_dir, path.parent(), settings);
    let notices: Vec<String> = [notice, height_notice, file_notice]
        .into_iter()
        .flatten()
        .collect();
    let notice = (!notices.is_empty()).then(|| notices.join("\n"));
    Ok((config, image, heightmap, notice, vehicles, image_found))
}

/// The map name to download an image for after `loaded`: none when a local image was found,
/// the setting is off or the config names no map (AD-14 step 3).
pub fn download_wanted<'a>(loaded: &'a Loaded, settings: &Settings) -> Option<&'a str> {
    let (config, .., image_found) = loaded;
    (!image_found && settings.global.online_map_images)
        .then(|| config.map_name())
        .flatten()
}

/// Fetches the map image of `name`, decodes it as a local image is, then saves the fetched
/// bytes as `<exe dir>/mapImages/<name>/<name>.png`. A failure to save is a notice only.
pub fn download_map_image(name: &str, source: &DownloadSource) -> Result<Downloaded, String> {
    let fail = |why: &dyn std::fmt::Display| {
        format!("Map image for {name} could not be downloaded ({why})")
    };
    let dir = (source.exe_dir.as_deref()).ok_or_else(|| fail(&"the program folder is unknown"))?;
    let bytes =
        download::fetch_map_image(&source.base_url, name, source.limits).map_err(|e| match e {
            DownloadError::NotFound => format!("No map image online for {name}"),
            e => fail(&e),
        })?;
    let target = map_image::map_image_target(dir, name);
    let (image, scaled) = map_image::decode_map_image(&bytes, &target.display().to_string())
        .map_err(|e| fail(&format!("not a PNG: {e}")))?;
    // A download starts only when no local image exists; one that appeared meanwhile (an
    // import for the same map) is newer and is never overwritten.
    let unsaved = if target.exists() {
        None
    } else {
        map_image::save_map_image(&target, &bytes).err().map(|e| {
            format!(
                "The downloaded map image could not be saved to {}: {e}",
                target.display()
            )
        })
    };
    let notices: Vec<String> = [scaled, unsaved].into_iter().flatten().collect();
    Ok(Downloaded {
        image,
        notice: (!notices.is_empty()).then(|| notices.join("\n")),
    })
}

/// Runs `download_map_image` on a background thread, then calls `notify`.
pub fn spawn_download(
    name: String,
    source: DownloadSource,
    epoch: u64,
    tx: Sender<JobResult>,
    notify: impl FnOnce() + Send + 'static,
) {
    thread::spawn(move || {
        tracing::info!("downloading the map image for {name}");
        let result = download_map_image(&name, &source).map(Box::new);
        match &result {
            Ok(_) => tracing::info!("downloaded the map image for {name}"),
            Err(e) => tracing::warn!("{e}"),
        }
        let _ = tx.send(JobResult::Downloaded { epoch, result });
        notify();
    });
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
                "{NO_HEIGHTMAP} for {}; new nodes will have no height",
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
    let (config, image, heightmap, notice, vehicles, found) = loaded;
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
    map.subdivisions = map_settings
        .grid_subdivisions
        .clamp(1, ad_core::map_context::MAX_SUBDIVISIONS);
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
    new.has_image = found;
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
                    // A save restarts the autosave timer.
                    d.autosave_since = None;
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
                    d.show(imported.image);
                    d.image_to_replace = imported.replace;
                    None
                }
                Err(message) => Some(message),
            }
        }
        JobResult::ImageSaved { result } => result.err(),
        // Like an import: only the image and the notice change, and only for the document
        // it was started for (the file is saved either way).
        JobResult::Downloaded { epoch, result } => {
            let d = doc.as_mut().filter(|d| d.epoch == epoch)?;
            let notice = match result {
                Ok(downloaded) => {
                    d.show(downloaded.image);
                    downloaded.notice
                }
                Err(message) => Some(message),
            };
            if let Some(notice) = notice {
                d.add_notice(notice);
            }
            None
        }
        // Shown, not saved: only the image and the notice change, for its document only.
        JobResult::ImageLoaded { epoch, result } => {
            let d = doc.as_mut().filter(|d| d.epoch == epoch)?;
            match result {
                Ok(loaded) => {
                    let (image, scaled) = *loaded;
                    d.show(image);
                    // Lines about the image shown before (scaled, failed, not saved) go.
                    d.drop_notice_lines("Map image ");
                    if let Some(scaled) = scaled {
                        d.add_notice(scaled);
                    }
                    d.add_notice(NOT_SAVED.to_owned());
                    None
                }
                Err(message) => Some(message),
            }
        }
        JobResult::HeightmapLoaded { epoch, result } => {
            let d = doc.as_mut().filter(|d| d.epoch == epoch)?;
            match result {
                Ok(heightmap) => {
                    zip_import::attach_heightmap(d, *heightmap);
                    None
                }
                Err(message) => Some(message),
            }
        }
        // A saved map image that is still the shown one is no longer "not saved".
        JobResult::Exported { image, result } => match result {
            Ok(()) => {
                if let (Some(d), Some(png)) = (doc.as_mut(), image)
                    && Arc::ptr_eq(&d.map_png, &png)
                {
                    d.drop_notice_lines(NOT_SAVED);
                }
                None
            }
            Err(message) => Some(message),
        },
        // Taken by `App::poll_jobs` before this is called.
        JobResult::ZipRead { .. }
        | JobResult::RoutesRead { .. }
        | JobResult::UpdateChecked { .. } => None,
        // The merge runs only after its backup was written, on the document, revision and
        // marks the backup was taken of (AD-18).
        JobResult::MergeBackup { start, result } => {
            if let Err(message) = result {
                return Some(message);
            }
            let d = doc.as_mut().filter(|d| d.epoch == start.epoch)?;
            if d.revision != start.revision || d.overlap.as_ref() != Some(&start.overlap) {
                d.add_notice(MERGE_CHANGED.to_owned());
                return None;
            }
            d.merge_overlaps().err().map(|e| e.to_string())
        }
        // Only the record of what was autosaved or a notice changes, for its document
        // only (AD-16, AD-18); `App::poll_jobs` shows the success.
        JobResult::Autosaved {
            epoch,
            point,
            result,
        } => {
            let d = doc.as_mut().filter(|d| d.epoch == epoch)?;
            match result {
                Ok(_) => d.autosaved = Some(point),
                Err(reason) => d.add_notice(format!("{AUTOSAVE_FAILED}{reason}")),
            }
            None
        }
        // The files are saved either way; they attach only to the document they were
        // imported for, and only when its map name is the folder name.
        JobResult::ZipSaved {
            epoch,
            folder,
            result,
        } => match result {
            Ok(saved) => {
                let same = |d: &Document| {
                    Some(d.epoch) == epoch && d.config.map_name() == Some(folder.as_str())
                };
                match doc.as_mut() {
                    Some(d) if same(d) => zip_import::attach(d, *saved),
                    // Saved for another map: say where, so the import is not silent.
                    Some(d) => {
                        d.notice = Some(format!(
                            "Map images for \"{folder}\" saved; they load with a config of that map"
                        ));
                    }
                    None => {
                        return Some(format!(
                            "Map images for \"{folder}\" saved; they load with a config of that map"
                        ));
                    }
                }
                None
            }
            Err(message) => Some(message),
        },
    }
}

/// `network` as a `routeExport` file written by the route writer, with "All" first as id 1
/// when a marker uses it (the other groups follow in marker order). Test only.
#[cfg(test)]
pub(crate) fn test_route_text(network: &ad_core::network::Network) -> Vec<u8> {
    let all = network.markers().iter().any(|m| m.group == "All");
    let groups = if all {
        vec![("All".to_owned(), 1)]
    } else {
        Vec::new()
    };
    routes_xml::save(network, &groups)
}

/// Writes `<dir>/routesManager/routes.xml`, listing `TestRoute0000001.xml` with `map`, and
/// `<dir>/routesManager/routes/TestRoute0000001.xml` holding `network`; returns the route
/// file. Test only.
#[cfg(test)]
pub(crate) fn test_route_manager(
    dir: &Path,
    network: &ad_core::network::Network,
    map: &str,
) -> std::io::Result<PathBuf> {
    let manager = dir.join("routesManager");
    std::fs::create_dir_all(manager.join("routes"))?;
    let list = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"no\"?>\n\
         <autoDriveRoutesManager>\n    <routes>\n        <route name=\"Test route\" \
         fileName=\"TestRoute0000001.xml\" map=\"{map}\" revision=\"1\" date=\"2026/11/08 12:00:00\">\n            \
         <serverId></serverId>\n        </route>\n    </routes>\n</autoDriveRoutesManager>\n"
    );
    std::fs::write(manager.join("routes.xml"), list)?;
    let file = manager.join("routes").join("TestRoute0000001.xml");
    std::fs::write(&file, test_route_text(network))?;
    Ok(file)
}

/// An 8x8 A8R8G8B8 DDS filled with `rgba`.
#[cfg(test)]
pub(crate) fn test_dds(rgba: [u8; 4]) -> Vec<u8> {
    test_dds_of(8, |_, _| rgba)
}

/// A `side` x `side` A8R8G8B8 DDS whose pixel (x, y) is `pixel(x, y)` as RGBA.
#[cfg(test)]
pub(crate) fn test_dds_of(side: u32, pixel: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    let mut file = Vec::new();
    let mut put = |v: u32| file.extend_from_slice(&v.to_le_bytes());
    put(u32::from_le_bytes(*b"DDS "));
    put(124);
    put(0x1007); // CAPS | HEIGHT | WIDTH | PIXELFORMAT
    put(side);
    put(side);
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
    for y in 0..side {
        for x in 0..side {
            let [r, g, b, a] = pixel(x, y);
            file.extend_from_slice(&[b, g, r, a]);
        }
    }
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
        (config, map_image::blank(), None, None, None, true)
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
        let (_, image, _, notice, _, _) = loaded.expect("loads");
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
            let (_, image, heightmap, notice, _, found) =
                load_file(&config, None, &settings(500)).expect("loads");
            assert_eq!((image, heightmap, found), (map_image::blank(), None, false));
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
                grid_subdivisions: 40,
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
        assert_eq!(d.map.subdivisions, 32, "clamped to the most");
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
        let (_, _, heightmap, notice, _, _) = loaded.expect("loads");
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

    use ad_io::download::test_server::{not_found, ok, png, serve};

    /// A download source for `base` saving into `exe`, with short timeouts and a 1 MiB cap.
    fn source(base: String, exe: &Path) -> DownloadSource {
        DownloadSource {
            base_url: base,
            limits: download::Limits {
                connect: Duration::from_secs(2),
                global: Duration::from_millis(500),
                max_bytes: 1 << 20,
            },
            exe_dir: Some(exe.to_path_buf()),
        }
    }

    /// A Tiny document of epoch 1 with no image and the load's heightmap notice.
    fn tiny() -> Option<Document> {
        let mut doc = Some(open(&named(&inline_text("1.000,2.500"))));
        if let Some(d) = doc.as_mut() {
            d.map_image = None;
            d.notice = Some(format!("{NO_HEIGHTMAP} for Tiny"));
        }
        doc
    }

    fn apply_download(doc: &mut Option<Document>, result: Result<Downloaded, String>) {
        let result = JobResult::Downloaded {
            epoch: 1,
            result: result.map(Box::new),
        };
        assert_eq!(apply_result(doc, &mut 1, &settings(500), result), None);
    }

    #[test]
    fn a_download_is_wanted_only_without_a_local_image_with_the_setting_and_a_name() {
        let dir = temp_dir("wanted");
        let config = dir.join("AutoDrive_config.xml");
        std::fs::write(&config, named(&inline_text("1.000,2.500"))).expect("write");
        let mut off = settings(500);
        off.global.online_map_images = false;
        let missing = load_file(&config, None, &settings(500)).expect("loads");
        assert_eq!(download_wanted(&missing, &settings(500)), Some("Tiny"));
        // Setting off.
        assert_eq!(download_wanted(&missing, &off), None);
        // Local image.
        let folder = dir.join("mapImages").join("Tiny");
        std::fs::create_dir_all(&folder).expect("folder");
        std::fs::write(folder.join("Tiny.png"), png(4, 4, [1, 2, 3, 255])).expect("write");
        let local = load_file(&config, None, &settings(500)).expect("loads");
        assert!(local.5);
        assert_eq!(download_wanted(&local, &settings(500)), None);
        // No map name.
        std::fs::write(&config, inline_text("1.000,2.500")).expect("write");
        let unnamed = load_file(&config, None, &settings(500)).expect("loads");
        std::fs::remove_dir_all(&dir).expect("cleanup");
        assert_eq!(download_wanted(&unnamed, &settings(500)), None);
    }

    #[test]
    fn a_downloaded_image_is_attached_saved_and_found_at_the_next_load() {
        let exe = temp_dir("download_ok");
        let (base, path) = serve(ok(png(4, 4, [7, 8, 9, 255]), "image/png"));
        let downloaded = download_map_image("Tiny", &source(base, &exe));
        let requested = path.recv().expect("path");
        let target = map_image::map_image_target(&exe, "Tiny");
        let found = map_image::find_map_image("Tiny", Some(&exe), None);
        let saved = map_image::load_map_image(&target);
        std::fs::remove_dir_all(&exe).expect("cleanup");
        assert_eq!(requested, "/mapImages/Tiny/Tiny.png");
        assert_eq!(found, Some(target));
        assert_eq!(&saved.0.rgba[..4], &[7, 8, 9, 255]);
        let downloaded = downloaded.expect("downloads");
        // 4x4 is scaled, as a local image would be.
        let scaled = downloaded.notice.clone().expect("scaled notice");
        assert!(scaled.contains("4x4"), "{scaled}");
        let mut doc = tiny();
        apply_download(&mut doc, Ok(downloaded));
        let d = doc.expect("doc");
        assert_eq!(
            &d.map_image.as_ref().expect("attached").rgba[..4],
            &[7, 8, 9, 255]
        );
        let notice = d.notice.as_deref().expect("notice");
        assert!(
            notice.starts_with(NO_HEIGHTMAP) && notice.contains("4x4"),
            "{notice}"
        );
        assert!(!d.is_dirty() && !d.history.can_undo());
    }

    #[test]
    fn a_download_never_overwrites_an_image_saved_meanwhile() {
        let exe = temp_dir("download_late");
        let target = map_image::map_image_target(&exe, "Tiny");
        let imported = png(2048, 2048, [1, 2, 3, 255]);
        map_image::save_map_image(&target, &imported).expect("an import saved it first");
        let (base, _path) = serve(ok(png(4, 4, [7, 8, 9, 255]), "image/png"));
        let downloaded = download_map_image("Tiny", &source(base, &exe));
        let kept = std::fs::read(&target).expect("kept");
        std::fs::remove_dir_all(&exe).expect("cleanup");
        assert!(downloaded.is_ok());
        assert_eq!(kept, imported);
    }

    /// Runs a download of Tiny against `respond`; returns the error and whether a file was saved.
    fn failed_download(
        tag: &str,
        respond: impl FnOnce(&mut std::net::TcpStream) + Send + 'static,
    ) -> (String, bool) {
        let exe = temp_dir(tag);
        let (base, _) = serve(respond);
        let result = download_map_image("Tiny", &source(base, &exe));
        let saved = exe.join("mapImages").exists();
        std::fs::remove_dir_all(&exe).expect("cleanup");
        (result.err().expect("fails"), saved)
    }

    #[test]
    fn a_404_gives_the_no_image_online_notice_and_keeps_the_blank() {
        let (message, saved) = failed_download("download_404", not_found);
        assert_eq!(message, "No map image online for Tiny");
        assert!(!saved);
        let mut doc = tiny();
        apply_download(&mut doc, Err(message));
        let d = doc.expect("doc");
        assert!(d.map_image.is_none());
        let notice = d.notice.expect("notice");
        assert!(
            notice.ends_with("\nNo map image online for Tiny"),
            "{notice}"
        );
    }

    #[test]
    fn a_timeout_oversize_or_html_body_names_the_reason_and_saves_nothing() {
        let html = b"<html><body>Rate limited</body></html>".to_vec();
        let results = [
            (
                failed_download("download_slow", |_| {
                    std::thread::sleep(Duration::from_secs(3))
                }),
                "timed out",
            ),
            (
                failed_download("download_big", ok(vec![0; (1 << 20) + 1], "image/png")),
                "larger than",
            ),
            (
                failed_download("download_html", ok(html, "text/html")),
                "not a PNG",
            ),
        ];
        for ((message, saved), reason) in results {
            assert!(message.starts_with("Map image for Tiny"), "{message}");
            assert!(message.contains(reason), "{message}");
            assert!(!saved, "{message}");
        }
    }

    #[test]
    fn a_download_for_a_replaced_document_is_not_attached() {
        let mut doc = tiny();
        let downloaded = Downloaded {
            image: map_image::blank(),
            notice: None,
        };
        let stale = JobResult::Downloaded {
            epoch: 0,
            result: Ok(Box::new(downloaded)),
        };
        assert_eq!(apply_result(&mut doc, &mut 1, &settings(500), stale), None);
        let d = doc.expect("doc");
        assert!(d.map_image.is_none());
        assert_eq!(d.notice, Some(format!("{NO_HEIGHTMAP} for Tiny")));
    }

    #[test]
    fn without_the_program_folder_nothing_is_requested() {
        let (base, path) = serve(not_found);
        let mut source = source(base, Path::new("unused"));
        source.exe_dir = None;
        let message = download_map_image("Tiny", &source).err().expect("fails");
        assert!(message.contains("program folder"), "{message}");
        assert!(path.recv_timeout(Duration::from_millis(200)).is_err());
    }

    /// Downloads the fixture's map image from the data repository into a temp folder.
    /// Run: `cargo test -p ad-app -- --ignored real_map_image_download --nocapture`.
    #[test]
    #[ignore = "needs the network"]
    fn real_map_image_download() {
        let exe = temp_dir("download_real");
        let source = DownloadSource {
            exe_dir: Some(exe.clone()),
            ..DownloadSource::default()
        };
        let result = download_map_image("NF_Marsch_4fach", &source);
        let found = map_image::find_map_image("NF_Marsch_4fach", Some(&exe), None);
        std::fs::remove_dir_all(&exe).expect("cleanup");
        let downloaded = result.expect("downloads");
        println!("notice {:?}, saved {found:?}", downloaded.notice);
        assert_eq!(downloaded.image.rgba.len(), 2048 * 2048 * 4);
        assert!(found.is_some());
    }

    #[test]
    fn autosave_path_is_beside_the_config_with_the_slot() {
        assert_eq!(
            autosave_path(Path::new("dir/AutoDrive_config.xml"), 3),
            Path::new("dir/AutoDrive_config_autosave_3.xml")
        );
        assert_eq!(
            autosave_path(Path::new("routes"), 1),
            Path::new("routes_autosave_1.xml")
        );
    }

    /// Writes the autosave files of `slots` beside `config`, each newer than the one before.
    fn autosaves(config: &Path, slots: &[u32]) {
        let start = std::time::SystemTime::now() - Duration::from_secs(3600);
        for (i, slot) in slots.iter().enumerate() {
            let path = autosave_path(config, *slot);
            std::fs::write(&path, "x").expect("write");
            let file = std::fs::File::options()
                .write(true)
                .open(&path)
                .expect("open");
            let modified = start + Duration::from_secs(60 * i as u64);
            file.set_modified(modified).expect("set mtime");
        }
    }

    #[test]
    fn the_next_slot_follows_the_newest_autosave() {
        let dir = temp_dir("autosave_rotation");
        let config = dir.join("AutoDrive_config.xml");
        assert_eq!(next_slot(&config, 10), 1, "none exists");
        // Slots 1..3 exist, 3 newest: slot 4.
        autosaves(&config, &[1, 2, 3]);
        assert_eq!(next_slot(&config, 10), 4);
        // The modification time counts, not the number: 2 rewritten last gives 3.
        autosaves(&config, &[1, 3, 2]);
        assert_eq!(next_slot(&config, 10), 3);
        // Another config's autosaves do not count.
        autosaves(&dir.join("other.xml"), &[7]);
        assert_eq!(next_slot(&config, 10), 3);
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn the_next_slot_wraps_after_the_last() {
        let dir = temp_dir("autosave_wrap");
        let config = dir.join("AutoDrive_config.xml");
        autosaves(&config, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        assert_eq!(next_slot(&config, 10), 1, "slot 10 newest");
        // With fewer slots, files of higher slots are ignored and 3 wraps to 1.
        autosaves(&config, &[1, 2, 3, 10]);
        assert_eq!(next_slot(&config, 3), 1);
        assert_eq!(next_slot(&config, 1), 1);
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}

#[cfg(test)]
mod route_tests {
    #![allow(clippy::expect_used)]

    use std::collections::HashMap;

    use ad_core::network::Network;

    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ad_app_routes_{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        dir
    }

    fn fixture() -> LoadedConfig {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../AutoDrive_config.xml");
        config_xml::load(&std::fs::read(path).expect("fixture")).expect("fixture loads")
    }

    fn two_nodes() -> Network {
        let config = "<AutoDrive><version>3</version><waypoints><id>1,2</id><x>1,2</x>\
            <y>0,0</y><z>3,4</z><out>2;-1</out><incoming>-1;1</incoming><flags>0,0</flags>\
            </waypoints></AutoDrive>";
        config_xml::load(config.as_bytes()).expect("loads").network
    }

    fn open(path: &Path) -> Document {
        let (mut doc, mut counter) = (None, 0);
        let result = JobResult::Loaded {
            path: path.to_path_buf(),
            result: load_file(path, None, &Settings::default()).map(Box::new),
        };
        assert_eq!(
            apply_result(&mut doc, &mut counter, &Settings::default(), result),
            None
        );
        doc.expect("doc")
    }

    #[test]
    fn open_detects_a_config_or_a_route() {
        let dir = temp_dir("detect");
        let config = dir.join("AutoDrive_config.xml");
        std::fs::write(
            &config,
            "<AutoDrive><version>3</version><waypoints><id>1</id><x>1</x><y>0</y><z>2</z>\
             <out>-1</out><incoming>-1</incoming><flags>0</flags></waypoints></AutoDrive>",
        )
        .expect("write");
        let doc = open(&config);
        assert!(!doc.config.is_route());
        assert_eq!(doc.config.network.len(), 1);
        let route = test_route_manager(&dir, &two_nodes(), "M").expect("route");
        let doc = open(&route);
        assert!(doc.config.is_route() && doc.editable());
        assert_eq!(doc.config.network.len(), 2);
        assert_eq!(doc.config.route_groups(), Some(&[][..]));
        // A broken route fails like a broken config, naming the file and the element.
        std::fs::write(
            &route,
            "<routeExport><waypoints c=\"2\"><x>1</x></waypoints></routeExport>",
        )
        .expect("write");
        let message = load_file(&route, None, &Settings::default()).expect_err("fails");
        assert!(
            message.contains("TestRoute0000001.xml") && message.contains("<x>"),
            "{message}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_route_without_routes_xml_has_a_notice_a_blank_map_and_no_vehicles() {
        let dir = temp_dir("no_list");
        let route = dir.join("Lonely.xml");
        std::fs::write(&route, test_route_text(&two_nodes())).expect("write");
        // A vehicles.xml beside a route is not read.
        std::fs::write(
            dir.join("vehicles.xml"),
            "<vehicles>\n    <vehicle id=\"1\" parkDestination=\"1\"/>\n</vehicles>\n",
        )
        .expect("write");
        let (config, image, _, notice, vehicles, found) =
            load_file(&route, None, &Settings::default()).expect("loads");
        assert!(config.is_route());
        assert_eq!(config.map_name(), None);
        assert_eq!((image, found), (map_image::blank(), false));
        assert!(vehicles.is_none());
        let notice = notice.expect("notice");
        assert!(
            notice.contains("No map name for this route (routes.xml not found)"),
            "{notice}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_fixture_as_a_route_opens_with_its_network_markers_and_map() {
        let source = fixture();
        let dir = temp_dir("fixture");
        let route = test_route_manager(&dir, &source.network, "NF_Marsch_4fach").expect("route");
        let doc = open(&route);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(doc.config.is_route() && doc.editable());
        assert_eq!(doc.config.map_name(), Some("NF_Marsch_4fach"));
        assert_eq!(doc.map.name.as_deref(), Some("NF_Marsch_4fach"));
        let (a, b) = (&source.network, &doc.config.network);
        assert_eq!(a.len(), b.len());
        let index = |net: &Network| -> HashMap<NodeId, usize> {
            net.order()
                .iter()
                .enumerate()
                .map(|(i, id)| (*id, i))
                .collect()
        };
        let (ia, ib) = (index(a), index(b));
        let links = |list: &[NodeId], index: &HashMap<NodeId, usize>| -> Vec<usize> {
            list.iter().filter_map(|t| index.get(t).copied()).collect()
        };
        for (na, nb) in a.order().iter().zip(b.order()) {
            let (na, nb) = (a.node(*na).expect("node"), b.node(*nb).expect("node"));
            for (va, vb) in [(na.x, nb.x), (na.y, nb.y), (na.z, nb.z)] {
                assert!((va - vb).abs() <= 0.005 + 1e-9, "{va} vs {vb}");
            }
            assert_eq!(na.flag, nb.flag);
            assert_eq!(links(na.outgoing(), &ia), links(nb.outgoing(), &ib));
            assert_eq!(links(na.incoming(), &ia), links(nb.incoming(), &ib));
        }
        let markers =
            |net: &Network, index: &HashMap<NodeId, usize>| -> Vec<(usize, String, String)> {
                net.markers()
                    .iter()
                    .map(|m| (index[&m.node], m.name.clone(), m.group.clone()))
                    .collect()
            };
        assert_eq!(markers(a, &ia), markers(b, &ib));
        assert!(!a.markers().is_empty());
        let groups = doc.config.route_groups().expect("groups");
        assert!(
            groups
                .iter()
                .all(|(g, _)| a.markers().iter().any(|m| &m.group == g))
        );
        let ids: Vec<u32> = groups.iter().map(|(_, i)| *i).collect();
        assert_eq!(ids, (1..=groups.len() as u32).collect::<Vec<_>>());
    }

    /// Writes the hand-test files `target/handtest/routesManager/routes.xml` and
    /// `.../routes/TestRoute0000001.xml` from the fixture (never committed).
    #[test]
    #[ignore = "writes the hand-test files"]
    fn write_handtest_route_manager() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/handtest");
        let route = test_route_manager(&dir, &fixture().network, "NF_Marsch_4fach").expect("route");
        let (config, ..) = load_file(&route, None, &Settings::default()).expect("loads");
        assert_eq!(config.map_name(), Some("NF_Marsch_4fach"));
        println!("wrote {}", route.display());
    }
}
