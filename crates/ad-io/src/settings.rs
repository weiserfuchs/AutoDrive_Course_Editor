//! Editor settings stored as TOML.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ad_core::history::DEFAULT_CAP;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("cannot read or write settings file {}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("malformed settings file {}: {source}", path.display())]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("cannot serialize settings for {}: {source}", path.display())]
    Serialize {
        path: PathBuf,
        source: toml::ser::Error,
    },
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub global: GlobalSettings,
    pub maps: BTreeMap<String, MapSettings>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GlobalSettings {
    pub history_cap: usize,
    /// Whether the grid starts switched on (Java `bShowGrid`).
    pub show_grid: bool,
    /// Whether grid snap starts switched on (Java `GridSnapEnabled`).
    pub snap: bool,
    /// Whether the connect tools start in continuous mode (Java `Continuous_Connection`).
    pub continuous: bool,
    /// Whether the right drag starts as a lasso instead of a rectangle.
    pub lasso: bool,
    /// Spacing of the nodes a connect line puts in, in world units; at least 1 (Java
    /// `LinearLineNodeDistance`).
    #[serde(deserialize_with = "at_least_one")]
    pub line_node_distance: u32,
    /// Whether a connect click on empty map ends the line at a new node (Java
    /// `CreateLinearLineEndNode`).
    pub create_line_end_node: bool,
    /// Whether a missing map image is downloaded from the data repository (Java
    /// `Check_Online_MapImages`).
    pub online_map_images: bool,
    /// Whether unsaved edits are written to rotating side files (AD-16).
    pub autosave: bool,
    /// Minutes between autosaves; at least 1 (Java `autoSaveInterval`).
    #[serde(deserialize_with = "at_least_one")]
    pub autosave_minutes: u32,
    /// How many autosave files rotate; at least 1 (Java `maxAutoSaveSlots`).
    #[serde(deserialize_with = "at_least_one")]
    pub autosave_slots: u32,
    /// The highest zoom factor of the map view, 1 to 250 (Java `MaxZoomLevel`).
    pub max_zoom: f64,
    /// Light, dark or the system's look.
    pub theme: Theme,
    /// The highest number of interpolation points the curve panel offers, 2 to 50 (Java
    /// `CurveSliderMaximum`).
    pub curve_max_points: usize,
    /// Interpolation points a new curve or arc starts with (Java `CurveSliderDefault`).
    pub curve_default_points: usize,
    /// Whether a new curve's nodes are subprio (Java `CurveNodeDefaultPriority`).
    pub curve_default_subprio: bool,
    /// A new curve's connection type (Java `CurveDefaultConnection`).
    pub curve_default_kind: CurveKind,
    /// A handle drag moves its control point this many times as far, 1 to 10 (Java
    /// `ControlPointMoveScaler`).
    pub control_point_scaler: f64,
    /// The last opened or saved files, newest first, at most `RECENT_MAX` (Java recent
    /// files).
    pub recent_files: Vec<PathBuf>,
    /// The folder open and import dialogs start in (Java `lastUsedLocation`).
    pub last_folder: Option<PathBuf>,
    /// The window's place at the last exit.
    pub window: Option<WindowPlace>,
}

/// How many files File ▸ Open Recent keeps (Java keeps 10).
pub const RECENT_MAX: usize = 10;

impl GlobalSettings {
    /// Puts `path` first in the recent files, dropping an older entry of it and any beyond
    /// `RECENT_MAX`.
    pub fn add_recent(&mut self, path: &Path) {
        self.recent_files.retain(|p| p != path);
        self.recent_files.insert(0, path.to_path_buf());
        self.recent_files.truncate(RECENT_MAX);
    }
}

/// The look of the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Follows the system's light or dark mode.
    #[default]
    System,
    Light,
    Dark,
}

/// A new curve's connection type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CurveKind {
    #[default]
    Regular,
    Dual,
    Reverse,
}

/// The ranges the settings window offers; values outside are clamped where they are used.
pub const MAX_ZOOM_RANGE: std::ops::RangeInclusive<f64> = 1.0..=250.0;
pub const CURVE_MAX_POINTS_RANGE: std::ops::RangeInclusive<usize> = 2..=50;
pub const SCALER_RANGE: std::ops::RangeInclusive<f64> = 1.0..=10.0;

/// The window's outer position and inner size in points, and whether it was maximized.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowPlace {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub maximized: bool,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            history_cap: DEFAULT_CAP,
            show_grid: false,
            snap: false,
            continuous: false,
            lasso: false,
            line_node_distance: 12,
            create_line_end_node: false,
            online_map_images: true,
            autosave: true,
            autosave_minutes: 10,
            autosave_slots: 10,
            max_zoom: 30.0,
            theme: Theme::System,
            curve_max_points: 50,
            curve_default_points: 6,
            curve_default_subprio: false,
            curve_default_kind: CurveKind::Regular,
            control_point_scaler: 3.0,
            recent_files: Vec::new(),
            last_folder: None,
            window: None,
        }
    }
}

fn at_least_one<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    Ok(u32::deserialize(d)?.max(1))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MapSettings {
    pub scale: u32,
    /// Node size in map-view units (Java `nodeSize`); the on-screen diameter follows zoom.
    pub node_size: f64,
    /// Grid spacing in world units (Java `gridSpacingX`).
    pub grid_x: f64,
    /// Grid spacing in world units (Java `gridSpacingY`).
    pub grid_y: f64,
}

impl Default for MapSettings {
    fn default() -> Self {
        Self {
            scale: 1,
            node_size: 2.0,
            grid_x: 2.0,
            grid_y: 2.0,
        }
    }
}

/// Reads the settings. A missing file gives the defaults.
pub fn load(path: &Path) -> Result<Settings, SettingsError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Settings::default()),
        Err(source) => {
            return Err(SettingsError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    toml::from_str(&text).map_err(|source| SettingsError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

/// Writes the settings to a temp file in the target's directory and renames it over the
/// target, creating the directory if needed. On any error the temp file is removed.
pub fn save(settings: &Settings, path: &Path) -> Result<(), SettingsError> {
    let text = toml::to_string_pretty(settings).map_err(|source| SettingsError::Serialize {
        path: path.to_path_buf(),
        source,
    })?;
    let io_err = |source| SettingsError::Io {
        path: path.to_path_buf(),
        source,
    };
    // Without a file name `atomic::write` fails; create nothing on disk in that case.
    if let Some(dir) = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty() && path.file_name().is_some())
    {
        std::fs::create_dir_all(dir).map_err(io_err)?;
    }
    crate::atomic::write(path, text.as_bytes()).map_err(io_err)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn scratch_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ad_settings_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    #[test]
    fn round_trip() {
        let dir = scratch_dir("rt");
        let mut s = Settings::default();
        s.global.history_cap = 200;
        s.maps.insert(
            "Marsch".to_owned(),
            MapSettings {
                scale: 4,
                node_size: 3.0,
                grid_x: 5.0,
                grid_y: 0.5,
            },
        );
        let path = dir.join("settings.toml");
        save(&s, &path).expect("save");
        assert_eq!(load(&path).expect("load"), s);
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn recent_files_keep_the_newest_first_without_duplicates_up_to_ten() {
        let mut g = GlobalSettings::default();
        for i in 0..12 {
            g.add_recent(Path::new(&format!("f{i}.xml")));
        }
        g.add_recent(Path::new("f5.xml"));
        let names: Vec<_> = g
            .recent_files
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        assert_eq!(
            names,
            [
                "f5.xml", "f11.xml", "f10.xml", "f9.xml", "f8.xml", "f7.xml", "f6.xml", "f4.xml",
                "f3.xml", "f2.xml"
            ]
        );
    }

    #[test]
    fn recent_files_folder_and_window_round_trip() {
        let dir = scratch_dir("recent");
        let mut s = Settings::default();
        s.global
            .add_recent(Path::new("C:\\maps\\AutoDrive_config.xml"));
        s.global.last_folder = Some(PathBuf::from("C:\\maps"));
        s.global.window = Some(WindowPlace {
            x: 10.0,
            y: -4.0,
            width: 1600.0,
            height: 900.0,
            maximized: true,
        });
        s.maps.insert("Marsch".to_owned(), MapSettings::default());
        let path = dir.join("settings.toml");
        save(&s, &path).expect("save");
        assert_eq!(load(&path).expect("load"), s);
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn missing_file_gives_defaults() {
        let dir = scratch_dir("missing");
        let s = load(&dir.join("nope.toml")).expect("defaults");
        assert_eq!(s, Settings::default());
        assert_eq!((s.global.history_cap, s.maps.len()), (500, 0));
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn partial_files_take_defaults() {
        let dir = scratch_dir("partial");
        let path = dir.join("s.toml");
        std::fs::write(&path, "[global]\n").expect("write");
        assert_eq!(load(&path).expect("load"), Settings::default());
        std::fs::write(&path, "[maps.X]\nunknown = 1\n[other]\na = 2\n").expect("write");
        let s = load(&path).expect("load");
        assert_eq!(
            s.maps.get("X"),
            Some(&MapSettings {
                scale: 1,
                node_size: 2.0,
                grid_x: 2.0,
                grid_y: 2.0,
            })
        );
        assert_eq!(s.global.history_cap, 500);
        assert!(!s.global.show_grid);
        // A file from before `node_size` existed stays valid.
        std::fs::write(
            &path,
            "[maps.Y]
scale = 3
",
        )
        .expect("write");
        let s = load(&path).expect("load");
        assert_eq!((s.maps["Y"].scale, s.maps["Y"].node_size), (3, 2.0));
        assert_eq!((s.maps["Y"].grid_x, s.maps["Y"].grid_y), (2.0, 2.0));
        std::fs::write(
            &path,
            "[global]
show_grid = true
[maps.Y]
grid_x = 4.0
",
        )
        .expect("write");
        let s = load(&path).expect("load");
        assert!(s.global.show_grid);
        assert_eq!((s.maps["Y"].grid_x, s.maps["Y"].grid_y), (4.0, 2.0));
        std::fs::write(
            &path,
            "[maps.Y]
node_size = 3
",
        )
        .expect("write");
        assert_eq!(load(&path).expect("load").maps["Y"].node_size, 3.0);
        // Floats load too, as Java steps the size by 0.1.
        std::fs::write(
            &path,
            "[maps.Y]
node_size = 2.5
",
        )
        .expect("write");
        assert_eq!(load(&path).expect("load").maps["Y"].node_size, 2.5);
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn line_options_default_and_keep_the_minimum() {
        let g = GlobalSettings::default();
        assert_eq!((g.line_node_distance, g.create_line_end_node), (12, false));
        let dir = scratch_dir("line");
        let path = dir.join("s.toml");
        let line = |path: &Path| {
            let g = load(path).expect("load").global;
            (g.line_node_distance, g.create_line_end_node)
        };
        // A file from before the line options existed takes the defaults.
        std::fs::write(&path, "[global]\nshow_grid = true\n").expect("write");
        assert_eq!(line(&path), (12, false));
        std::fs::write(
            &path,
            "[global]\nline_node_distance = 0\ncreate_line_end_node = true\n",
        )
        .expect("write");
        assert_eq!(line(&path), (1, true), "0 is raised to the minimum");
        std::fs::write(&path, "[global]\nline_node_distance = 5\n").expect("write");
        assert_eq!(line(&path), (5, false));
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn online_map_images_defaults_on_and_loads_off() {
        assert!(GlobalSettings::default().online_map_images);
        let dir = scratch_dir("online");
        let path = dir.join("s.toml");
        // A file from before the option existed takes the default.
        std::fs::write(&path, "[global]\nshow_grid = true\n").expect("write");
        assert!(load(&path).expect("load").global.online_map_images);
        std::fs::write(&path, "[global]\nonline_map_images = false\n").expect("write");
        assert!(!load(&path).expect("load").global.online_map_images);
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn autosave_options_default_and_keep_the_minimum() {
        let g = GlobalSettings::default();
        assert_eq!(
            (g.autosave, g.autosave_minutes, g.autosave_slots),
            (true, 10, 10)
        );
        let dir = scratch_dir("autosave");
        let path = dir.join("s.toml");
        let autosave = |path: &Path| {
            let g = load(path).expect("load").global;
            (g.autosave, g.autosave_minutes, g.autosave_slots)
        };
        // A file from before the options existed takes the defaults.
        std::fs::write(&path, "[global]\nshow_grid = true\n").expect("write");
        assert_eq!(autosave(&path), (true, 10, 10));
        std::fs::write(
            &path,
            "[global]\nautosave = false\nautosave_minutes = 0\nautosave_slots = 0\n",
        )
        .expect("write");
        assert_eq!(autosave(&path), (false, 1, 1), "0 is raised to the minimum");
        std::fs::write(
            &path,
            "[global]\nautosave_minutes = 3\nautosave_slots = 4\n",
        )
        .expect("write");
        assert_eq!(autosave(&path), (true, 3, 4));
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn malformed_names_path() {
        let dir = scratch_dir("bad");
        let path = dir.join("s.toml");
        for text in ["global = 5\n", "[[[ not toml"] {
            std::fs::write(&path, text).expect("write");
            let err = load(&path).expect_err("malformed");
            assert!(err.to_string().contains("s.toml"), "{err}");
        }
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn save_creates_missing_dir_without_temp_file() {
        let dir = scratch_dir("mkdir");
        let sub = dir.join("a").join("b");
        save(&Settings::default(), &sub.join("settings.toml")).expect("save");
        let names: Vec<_> = std::fs::read_dir(&sub)
            .expect("read_dir")
            .map(|e| e.expect("entry").file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("settings.toml")]);
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn save_fails_when_parent_is_a_file() {
        let dir = scratch_dir("parentfile");
        let file = dir.join("f");
        std::fs::write(&file, "x").expect("write");
        let err = save(&Settings::default(), &file.join("settings.toml")).expect_err("fails");
        assert!(err.to_string().contains("settings.toml"), "{err}");
        assert_eq!(std::fs::read_to_string(&file).expect("read"), "x");
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}
