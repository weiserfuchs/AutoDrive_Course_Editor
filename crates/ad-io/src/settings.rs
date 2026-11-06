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
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            history_cap: DEFAULT_CAP,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MapSettings {
    pub scale: u32,
    /// Node size in map-view units (Java `nodeSize`); the on-screen diameter follows zoom.
    pub node_size: f64,
}

impl Default for MapSettings {
    fn default() -> Self {
        Self {
            scale: 1,
            node_size: 2.0,
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
            },
        );
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
                node_size: 2.0
            })
        );
        assert_eq!(s.global.history_cap, 500);
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
