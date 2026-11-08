//! `vehicles.xml` beside the FS25 fixture: load, edits, save (AD-6, AD-17). Both fixtures
//! are copied to a temp folder; the repo files are only read.

use std::path::PathBuf;

use ad_core::command::{Command, DeleteNodes, EditMarker, Transaction};
use ad_io::config_xml::{self, LoadedConfig};
use ad_io::vehicles::{self, VehiclesFile};

type Error = Box<dyn std::error::Error>;

const NAMES: [&str; 3] = ["parkDestination", "firstMarker", "secondMarker"];

fn repo(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(name)
}

/// A temp folder holding copies of both fixtures.
struct Copies {
    dir: PathBuf,
}

impl Copies {
    fn new(tag: &str) -> Result<Self, Error> {
        let dir = std::env::temp_dir().join(format!("ad_vehicles_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        for name in ["AutoDrive_config.xml", "vehicles.xml"] {
            std::fs::copy(repo(name), dir.join(name))?;
        }
        Ok(Self { dir })
    }

    fn config(&self) -> PathBuf {
        self.dir.join("AutoDrive_config.xml")
    }

    fn vehicles(&self) -> PathBuf {
        self.dir.join("vehicles.xml")
    }

    fn open(&self) -> Result<(LoadedConfig, VehiclesFile), Error> {
        let mut cfg = config_xml::load(&std::fs::read(self.config())?)?;
        let version = cfg.game_version();
        let file = vehicles::load(&std::fs::read(self.vehicles())?, version, &mut cfg.network)?;
        Ok((cfg, file))
    }

    /// Saves both files in place, config first, and returns the new `vehicles.xml`.
    fn save(&self, cfg: &LoadedConfig, file: &VehiclesFile) -> Result<Vec<u8>, Error> {
        config_xml::save_to(cfg, &self.config())?;
        vehicles::save_to(file, &cfg.network, &self.vehicles())?;
        Ok(std::fs::read(self.vehicles())?)
    }
}

impl Drop for Copies {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The reference attributes in file order as `(name, value)`, and the bytes with every
/// reference attribute removed.
fn split(bytes: &[u8]) -> (Vec<(String, usize)>, Vec<u8>) {
    let mut refs = Vec::new();
    let mut rest = Vec::with_capacity(bytes.len());
    let mut i = 0;
    'outer: while i < bytes.len() {
        for name in NAMES {
            let head = format!(" {name}=\"");
            if bytes[i..].starts_with(head.as_bytes()) {
                let start = i + head.len();
                if let Some(len) = bytes[start..].iter().position(|&b| b == b'"') {
                    let value = std::str::from_utf8(&bytes[start..start + len])
                        .ok()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                    refs.push((name.to_owned(), value));
                    i = start + len + 1;
                    continue 'outer;
                }
            }
        }
        rest.push(bytes[i]);
        i += 1;
    }
    (refs, rest)
}

/// What the references become after deleting the markers at the 1-based `deleted`.
fn expected(refs: &[(String, usize)], deleted: &[usize]) -> Vec<(String, usize)> {
    refs.iter()
        .filter_map(|(name, v)| {
            if deleted.contains(v) {
                (name != "parkDestination").then(|| (name.clone(), 1))
            } else {
                let shift = deleted.iter().filter(|d| **d < *v).count();
                Some((name.clone(), v - shift))
            }
        })
        .collect()
}

fn remove_markers(cfg: &mut LoadedConfig, indexes: &[usize]) -> Result<Transaction, Error> {
    let nodes = indexes
        .iter()
        .map(|k| cfg.network.markers()[k - 1].node)
        .collect::<Vec<_>>();
    let mut tx = Transaction::new(
        nodes
            .into_iter()
            .map(|node| Command::EditMarker(EditMarker::new(node, None)))
            .collect(),
    );
    tx.apply(&mut cfg.network)?;
    Ok(tx)
}

fn parked_markers(cfg: &LoadedConfig) -> Vec<(usize, usize)> {
    cfg.network
        .markers()
        .iter()
        .enumerate()
        .filter(|(_, m)| !m.parked_vehicles.is_empty())
        .map(|(i, m)| (i + 1, m.parked_vehicles.len()))
        .collect()
}

#[test]
fn fixture_loads_parking_and_references() -> Result<(), Error> {
    let copies = Copies::new("load")?;
    let (cfg, file) = copies.open()?;
    assert_eq!(cfg.network.markers().len(), 223);
    let parked: Vec<usize> = parked_markers(&cfg).iter().map(|(k, _)| *k).collect();
    assert_eq!(parked, [161, 163, 164, 165, 167, 178]);
    assert!(parked_markers(&cfg).iter().all(|(_, n)| *n == 1));
    // 25 AutoDrive elements with both firstMarker and secondMarker.
    assert_eq!(file.marker_references(), 50);
    Ok(())
}

#[test]
fn unchanged_save_is_byte_identical() -> Result<(), Error> {
    let copies = Copies::new("unchanged")?;
    let (cfg, file) = copies.open()?;
    assert_eq!(
        copies.save(&cfg, &file)?,
        std::fs::read(repo("vehicles.xml"))?
    );
    Ok(())
}

#[test]
fn deleting_a_marker_before_shifts_only_the_later_references() -> Result<(), Error> {
    let copies = Copies::new("before")?;
    let (mut cfg, file) = copies.open()?;
    remove_markers(&mut cfg, &[100])?;
    let (refs, rest) = split(&std::fs::read(repo("vehicles.xml"))?);
    let (new_refs, new_rest) = split(&copies.save(&cfg, &file)?);
    assert_eq!(new_rest, rest, "only reference values change");
    assert_eq!(new_refs, expected(&refs, &[100]));
    assert!(
        new_refs
            .iter()
            .any(|(n, v)| n == "parkDestination" && *v == 160)
    );
    Ok(())
}

#[test]
fn deleting_referenced_markers_drops_parking_and_resets_references() -> Result<(), Error> {
    let copies = Copies::new("referenced")?;
    let (mut cfg, file) = copies.open()?;
    remove_markers(&mut cfg, &[161, 180])?;
    let original = std::fs::read(repo("vehicles.xml"))?;
    let saved = copies.save(&cfg, &file)?;
    let (refs, rest) = split(&original);
    let (new_refs, new_rest) = split(&saved);
    assert_eq!(new_rest, rest, "only reference attributes change");
    assert_eq!(new_refs, expected(&refs, &[161, 180]));
    let count = |refs: &[(String, usize)], name: &str| refs.iter().filter(|r| r.0 == name).count();
    assert_eq!(count(&new_refs, "parkDestination"), 5);
    // The standalone element of the vehicle parked at 161 keeps no attribute.
    let occurrences =
        |bytes: &[u8], needle: &[u8]| bytes.windows(needle.len()).filter(|w| *w == needle).count();
    assert_eq!(
        occurrences(&original, b"<AutoDrive parkDestination=\"161\"/>"),
        1
    );
    assert_eq!(
        occurrences(&saved, b"<AutoDrive/>"),
        occurrences(&original, b"<AutoDrive/>") + 1
    );
    // The five firstMarker="180" became 1; 186 is now 184.
    assert_eq!(
        new_refs
            .iter()
            .filter(|r| r.0 == "secondMarker" && r.1 == 184)
            .count(),
        3
    );
    Ok(())
}

#[test]
fn undo_of_a_marker_delete_saves_byte_identical() -> Result<(), Error> {
    let copies = Copies::new("undo")?;
    let (mut cfg, file) = copies.open()?;
    let mut tx = remove_markers(&mut cfg, &[161])?;
    tx.revert(&mut cfg.network)?;
    assert_eq!(
        copies.save(&cfg, &file)?,
        std::fs::read(repo("vehicles.xml"))?
    );
    Ok(())
}

#[test]
fn node_delete_and_undo_bring_the_parked_vehicle_back() -> Result<(), Error> {
    let copies = Copies::new("node")?;
    let (mut cfg, file) = copies.open()?;
    let marker = cfg.network.markers()[160].clone();
    assert_eq!(marker.parked_vehicles.len(), 1);
    let mut tx = Transaction::new(vec![Command::DeleteNodes(DeleteNodes::new(vec![
        marker.node,
    ]))]);
    tx.apply(&mut cfg.network)?;
    assert!(!parked_markers(&cfg).iter().any(|(k, _)| *k == 161));
    tx.revert(&mut cfg.network)?;
    assert_eq!(cfg.network.markers()[160], marker);
    assert_eq!(
        copies.save(&cfg, &file)?,
        std::fs::read(repo("vehicles.xml"))?
    );
    Ok(())
}
