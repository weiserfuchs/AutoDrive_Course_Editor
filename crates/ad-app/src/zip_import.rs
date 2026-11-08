//! Map mod zip import: the read job, the preview state, the save job and the attach to an
//! open document whose map the files are for (AD-9, AD-14, AD-18).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread;

use ad_core::heightmap::{Heightmap, calibrate};
use ad_io::config_xml::GameVersion;
use ad_io::map_image::{self, MapImage};
use ad_io::mod_zip::{self, ModZip};
use eframe::egui;
use image::RgbaImage;

use crate::document::Document;
use crate::jobs::JobResult;

/// A read zip with its 256x256 previews.
pub struct ZipRead {
    pub zip: ModZip,
    /// The map as FS19 crops it (whole) and as FS22/FS25 crop it (centre half).
    pub whole: RgbaImage,
    pub centre: RgbaImage,
    pub heightmap: Option<RgbaImage>,
}

/// The preview window's state, while it is shown.
pub struct ZipPreview {
    pub read: ZipRead,
    /// The game whose crop the import uses; starts as detected.
    pub version: GameVersion,
    pub name: String,
    /// The existing files an Import would replace, while that is asked.
    pub replace: Option<Vec<PathBuf>>,
    pub textures: Option<PreviewTextures>,
}

pub struct PreviewTextures {
    pub whole: egui::TextureHandle,
    pub centre: egui::TextureHandle,
    pub heightmap: Option<egui::TextureHandle>,
}

impl ZipPreview {
    /// The name starts as the open config's map name, else the English or first title.
    pub fn new(read: ZipRead, map_name: Option<&str>) -> Self {
        Self {
            version: read.zip.version,
            name: mod_zip::default_name(&read.zip.titles, map_name),
            read,
            replace: None,
            textures: None,
        }
    }

    /// `mapImages/<name>/<name>.png` and, with a heightmap, `<name>_HeightMap.png` in `dir`.
    pub fn targets(&self, dir: &Path, folder: &str) -> Vec<PathBuf> {
        let mut targets = vec![map_image::map_image_target(dir, folder)];
        if self.read.zip.dem.is_some() {
            targets.push(map_image::heightmap_target(dir, folder));
        }
        targets
    }
}

/// The saved map image and heightmap, for the attach.
pub struct ZipSaved {
    pub image: MapImage,
    pub heightmap: Option<Heightmap>,
}

/// Reads the zip and makes the previews. The error names the zip and the problem.
pub fn read(path: &Path) -> Result<ZipRead, String> {
    let fail = |why: &dyn std::fmt::Display| format!("Cannot import {}: {why}", path.display());
    let file = std::fs::File::open(path).map_err(|e| fail(&e))?;
    let zip = mod_zip::read(std::io::BufReader::new(file)).map_err(|e| fail(&e))?;
    Ok(ZipRead {
        whole: map_image::preview(GameVersion::FS19, &zip.image),
        centre: map_image::preview(GameVersion::FS25, &zip.image),
        heightmap: zip
            .dem
            .as_ref()
            .map(|d| map_image::heightmap_preview(&d.heightmap)),
        zip,
    })
}

/// Runs `read` on a background thread, then calls `notify`. `epoch` is that of the open
/// document, if any.
pub fn spawn_read(
    path: PathBuf,
    epoch: Option<u64>,
    tx: Sender<JobResult>,
    notify: impl FnOnce() + Send + 'static,
) {
    thread::spawn(move || {
        tracing::info!("reading map mod zip {}", path.display());
        let result = read(&path).map(Box::new);
        if let Err(e) = &result {
            tracing::warn!("{e}");
        }
        let _ = tx.send(JobResult::ZipRead { epoch, result });
        notify();
    });
}

/// Crops (by `version`, AD-11), scales and saves the map image, then saves the heightmap's
/// bytes unchanged, both into `dir/mapImages/<folder>/`, replacing what is there.
pub fn save(
    zip: ModZip,
    version: GameVersion,
    dir: &Path,
    folder: &str,
) -> Result<ZipSaved, String> {
    let image = map_image::to_2048(map_image::crop_for(version, zip.image));
    let png =
        map_image::encode_png(&image).map_err(|e| format!("Cannot encode the map image: {e}"))?;
    let write = |target: PathBuf, bytes: &[u8]| {
        map_image::save_map_image(&target, bytes)
            .map_err(|e| format!("Cannot save {}: {e}", target.display()))
    };
    write(map_image::map_image_target(dir, folder), &png)?;
    let heightmap = match zip.dem {
        Some(dem) => {
            write(map_image::heightmap_target(dir, folder), &dem.png)?;
            Some(dem.heightmap)
        }
        None => None,
    };
    Ok(ZipSaved { image, heightmap })
}

/// Runs `save` on a background thread, then calls `notify`. `epoch` is that of the open
/// document, if any; the result attaches only to it.
pub fn spawn_save(
    zip: ModZip,
    version: GameVersion,
    dir: PathBuf,
    folder: String,
    epoch: Option<u64>,
    tx: Sender<JobResult>,
    notify: impl FnOnce() + Send + 'static,
) {
    thread::spawn(move || {
        tracing::info!("saving map mod images for {folder} as {version:?}");
        let result = save(zip, version, &dir, &folder).map(Box::new);
        match &result {
            Ok(_) => tracing::info!("saved map mod images for {folder}"),
            Err(e) => tracing::warn!("{e}"),
        }
        let _ = tx.send(JobResult::ZipSaved {
            epoch,
            folder,
            result,
        });
        notify();
    });
}

/// Shows the image and, if saved, takes the heightmap with its scale calibrated as a load
/// does. Neither the config nor the history changes.
pub fn attach(doc: &mut Document, saved: ZipSaved) {
    doc.map_image = Some(saved.image);
    if let Some(heightmap) = saved.heightmap {
        doc.map.height_map_scale = calibrate(&heightmap, &doc.config.network, doc.map.geometry);
        doc.map.heightmap = Some(Arc::new(heightmap));
    }
}
