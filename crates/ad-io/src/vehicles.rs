//! The marker references of the savegame's `vehicles.xml` (AD-17), patched in place (AD-6).
//!
//! `parkDestination`, `firstMarker` and `secondMarker` on a vehicle's AutoDrive element
//! name a marker by its 1-based position in the config's marker list. A load moves the
//! park destinations onto the markers and keeps the other two as references to the node
//! of their marker; a save writes the current positions into the original bytes and
//! changes nothing else.

use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;

use ad_core::network::{Network, NodeId};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use thiserror::Error;

use crate::config_xml::GameVersion;

const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
const PARK: &[u8] = b"parkDestination";
const FIRST: &[u8] = b"firstMarker";
const SECOND: &[u8] = b"secondMarker";

#[derive(Debug, Error)]
pub enum VehiclesError {
    #[error("XML error: {0}")]
    Xml(String),
    #[error("cannot write {}: {source}", path.display())]
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
}

fn xml_err(e: impl std::fmt::Display) -> VehiclesError {
    VehiclesError::Xml(e.to_string())
}

/// What a reference attribute follows.
#[derive(Debug, Clone, PartialEq)]
enum Target {
    /// `parkDestination`: the marker whose parked vehicles list this vehicle.
    Vehicle(String),
    /// `firstMarker` / `secondMarker`: the marker on this node.
    Node(NodeId),
    /// Not a marker position at load: never rewritten.
    Verbatim,
}

#[derive(Debug, Clone)]
struct Reference {
    /// The attribute with its leading whitespace, removed when a vehicle parks nowhere.
    attr: Range<usize>,
    /// The value between the quotes.
    value: Range<usize>,
    /// The position read at load.
    loaded: usize,
    target: Target,
}

/// A loaded `vehicles.xml`: the original bytes and its marker references.
#[derive(Debug, Clone)]
pub struct VehiclesFile {
    original: Vec<u8>,
    refs: Vec<Reference>,
}

impl VehiclesFile {
    /// The `firstMarker` and `secondMarker` attributes that follow a marker.
    pub fn marker_references(&self) -> usize {
        self.refs
            .iter()
            .filter(|r| matches!(r.target, Target::Node(_)))
            .count()
    }
}

/// One attribute of a start tag: name, the attribute with its leading whitespace and the
/// value between the quotes, all as absolute byte ranges.
struct RawAttr {
    name: Range<usize>,
    attr: Range<usize>,
    value: Range<usize>,
}

/// Splits the attributes of the tag whose content (between `<` and `>`) starts at `start`.
fn attributes(
    bytes: &[u8],
    start: usize,
    content_len: usize,
) -> Result<Vec<RawAttr>, VehiclesError> {
    let end = start + content_len;
    let ws = |b: u8| b.is_ascii_whitespace();
    let mut i = start;
    while i < end && !ws(bytes[i]) && bytes[i] != b'/' {
        i += 1;
    }
    let mut out = Vec::new();
    loop {
        let attr_start = i;
        while i < end && ws(bytes[i]) {
            i += 1;
        }
        if i >= end || bytes[i] == b'/' {
            return Ok(out);
        }
        let name_start = i;
        while i < end && bytes[i] != b'=' && !ws(bytes[i]) {
            i += 1;
        }
        let name = name_start..i;
        while i < end && ws(bytes[i]) {
            i += 1;
        }
        if i >= end || bytes[i] != b'=' {
            return Err(xml_err(format!(
                "attribute without value at byte {name_start}"
            )));
        }
        i += 1;
        while i < end && ws(bytes[i]) {
            i += 1;
        }
        let quote = match bytes.get(i) {
            Some(&q @ (b'"' | b'\'')) if i < end => q,
            _ => return Err(xml_err(format!("unquoted attribute at byte {name_start}"))),
        };
        let value_start = i + 1;
        let value_end = bytes[value_start..end]
            .iter()
            .position(|&b| b == quote)
            .map(|p| value_start + p)
            .ok_or_else(|| xml_err(format!("unclosed attribute at byte {name_start}")))?;
        i = value_end + 1;
        out.push(RawAttr {
            name,
            attr: attr_start..i,
            value: value_start..value_end,
        });
    }
}

fn text(bytes: &[u8], range: &Range<usize>) -> Result<String, VehiclesError> {
    let raw = std::str::from_utf8(&bytes[range.clone()]).map_err(xml_err)?;
    Ok(quick_xml::escape::unescape(raw)
        .map_err(xml_err)?
        .into_owned())
}

/// A reference attribute as read, before it is tied to the network.
struct Found {
    attr: Range<usize>,
    value: Range<usize>,
    /// `None` for `firstMarker` / `secondMarker`.
    vehicle: Option<String>,
    loaded: Option<usize>,
}

/// Whether the element just opened, with `stack` its ancestors, is a vehicle's AutoDrive
/// element: FS19 `vehicle/FS19_AutoDrive/AutoDriveVehicleData`, else `vehicle/AutoDrive`.
fn is_autodrive(name: &[u8], stack: &[Vec<u8>], version: GameVersion) -> bool {
    let parent = |k: usize| stack.len().checked_sub(k).map(|i| stack[i].as_slice());
    if version == GameVersion::FS19 {
        name == b"AutoDriveVehicleData"
            && parent(1) == Some(b"FS19_AutoDrive")
            && parent(2) == Some(b"vehicle")
    } else {
        name == b"AutoDrive" && parent(1) == Some(b"vehicle")
    }
}

/// Parses `bytes` with the id and element rules of `version`, then parks the vehicles on
/// the markers of `network` and ties `firstMarker` / `secondMarker` to their marker's node.
/// The network is untouched when the file does not parse.
pub fn load(
    bytes: &[u8],
    version: GameVersion,
    network: &mut Network,
) -> Result<VehiclesFile, VehiclesError> {
    let bom_len = if bytes.starts_with(BOM) { BOM.len() } else { 0 };
    let id_attr: &[u8] = if version == GameVersion::FS25 {
        b"uniqueId"
    } else {
        b"id"
    };
    let mut reader = Reader::from_reader(&bytes[bom_len..]);
    let mut stack: Vec<Vec<u8>> = Vec::new();
    // The id of each open `vehicle`, innermost last.
    let mut vehicles: Vec<Option<String>> = Vec::new();
    let mut found = Vec::new();
    loop {
        let before = reader.buffer_position() as usize + bom_len;
        let event = reader.read_event().map_err(xml_err)?;
        let (e, empty): (&BytesStart, bool) = match &event {
            Event::Start(e) => (e, false),
            Event::Empty(e) => (e, true),
            Event::End(_) => {
                if stack.pop().as_deref() == Some(b"vehicle") {
                    vehicles.pop();
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        let content = before + 1;
        if bytes.get(content..content + e.len()) != Some(e.as_bytes()) {
            return Err(xml_err(format!("unexpected tag layout at byte {before}")));
        }
        let name = e.name().as_ref().as_bytes().to_vec();
        let attrs = attributes(bytes, content, e.len())?;
        if name == b"vehicle" {
            let id = attrs
                .iter()
                .find(|a| &bytes[a.name.clone()] == id_attr)
                .map(|a| text(bytes, &a.value))
                .transpose()?;
            vehicles.push(id);
        } else if is_autodrive(&name, &stack, version) {
            let vehicle = vehicles.last().cloned().flatten();
            for a in attrs {
                let attr_name = &bytes[a.name.clone()];
                let park = attr_name == PARK;
                if !park && attr_name != FIRST && attr_name != SECOND {
                    continue;
                }
                let loaded = std::str::from_utf8(&bytes[a.value.clone()])
                    .ok()
                    .and_then(|s| s.trim().parse::<usize>().ok());
                found.push(Found {
                    attr: a.attr,
                    value: a.value,
                    vehicle: if park {
                        Some(vehicle.clone().unwrap_or_default())
                    } else {
                        None
                    },
                    loaded,
                });
            }
        }
        if empty {
            if name == b"vehicle" {
                vehicles.pop();
            }
        } else {
            stack.push(name);
        }
    }
    if let Some(open) = stack.last() {
        return Err(xml_err(format!(
            "unexpected end of file in <{}>",
            String::from_utf8_lossy(open)
        )));
    }

    let count = network.markers().len();
    let refs = found
        .into_iter()
        .map(|f| {
            let index = f.loaded.filter(|k| (1..=count).contains(k)).map(|k| k - 1);
            let target = match (index, f.vehicle) {
                // A vehicle without an id cannot be found again on save.
                (Some(i), Some(vehicle)) if !vehicle.is_empty() => {
                    network.park_vehicle(i, vehicle.clone());
                    Target::Vehicle(vehicle)
                }
                (Some(i), None) => Target::Node(network.markers()[i].node),
                _ => Target::Verbatim,
            };
            Reference {
                attr: f.attr,
                value: f.value,
                loaded: f.loaded.unwrap_or_default(),
                target,
            }
        })
        .collect();
    Ok(VehiclesFile {
        original: bytes.to_vec(),
        refs,
    })
}

/// The original bytes with every reference set to the current marker positions. A park
/// destination of a vehicle no marker lists is removed; a node without a marker gives 1.
pub fn patch(file: &VehiclesFile, network: &Network) -> Vec<u8> {
    // Positions as the config save writes them: markers whose node exists.
    let written = network
        .markers()
        .iter()
        .filter(|m| network.node(m.node).is_some());
    let mut by_node = HashMap::new();
    let mut by_vehicle = HashMap::new();
    for (i, m) in written.enumerate() {
        by_node.entry(m.node).or_insert(i + 1);
        for v in &m.parked_vehicles {
            by_vehicle.entry(v.as_str()).or_insert(i + 1);
        }
    }
    let orig = &file.original;
    let mut out = Vec::with_capacity(orig.len());
    let mut pos = 0;
    for r in &file.refs {
        let (range, new) = match &r.target {
            Target::Verbatim => continue,
            Target::Node(node) => (&r.value, Some(by_node.get(node).copied().unwrap_or(1))),
            Target::Vehicle(v) => match by_vehicle.get(v.as_str()) {
                Some(&k) => (&r.value, Some(k)),
                None => (&r.attr, None),
            },
        };
        if new == Some(r.loaded) {
            continue;
        }
        out.extend_from_slice(&orig[pos..range.start]);
        if let Some(k) = new {
            out.extend_from_slice(k.to_string().as_bytes());
        }
        pos = range.end;
    }
    out.extend_from_slice(&orig[pos..]);
    out
}

/// Patches and writes atomically to `target` (AD-6).
pub fn save_to(file: &VehiclesFile, network: &Network, target: &Path) -> Result<(), VehiclesError> {
    crate::atomic::write(target, &patch(file, network)).map_err(|source| VehiclesError::Io {
        path: target.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use ad_core::network::NodeFlag;

    use super::*;

    /// A network with `n` markers, one per node, named `m1`..`mn`.
    fn network(n: usize) -> Network {
        let mut net = Network::new();
        for i in 0..n {
            let id = net.add_node(i as f64, 0.0, 0.0, NodeFlag::REGULAR);
            net.set_marker(id, format!("m{}", i + 1), "All".into())
                .expect("marker");
        }
        net
    }

    fn parked(net: &Network) -> Vec<(usize, Vec<String>)> {
        net.markers()
            .iter()
            .enumerate()
            .filter(|(_, m)| !m.parked_vehicles.is_empty())
            .map(|(i, m)| (i + 1, m.parked_vehicles.clone()))
            .collect()
    }

    const FS22: &str = "<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"no\"?>\r\n\
        <vehicles>\r\n    <vehicle id=\"7\" filename=\"a.xml\">\r\n        \
        <AutoDrive mode=\"1\" firstMarker=\"3\" secondMarker='2' parkDestination=\"3\"/>\r\n    \
        </vehicle>\r\n    <vehicle id=\"8\">\r\n        <AutoDrive parkDestination=\"9\" firstMarker=\"x\"/>\r\n    \
        </vehicle>\r\n    <vehicle id=\"9\"><other><AutoDrive firstMarker=\"2\"/></other></vehicle>\r\n</vehicles>\r\n";

    const FS19: &str = "<vehicles>\n  <vehicle id=\"4\">\n    <FS19_AutoDrive>\n      \
        <AutoDriveVehicleData parkDestination=\"2\" firstMarker=\"1\" secondMarker=\"3\"/>\n    \
        </FS19_AutoDrive>\n    <AutoDrive parkDestination=\"1\"/>\n  </vehicle>\n</vehicles>\n";

    #[test]
    fn fs22_sample_parks_patches_and_keeps_the_rest() {
        let mut net = network(3);
        let file = load(FS22.as_bytes(), GameVersion::FS22, &mut net).expect("loads");
        assert_eq!(parked(&net), vec![(3, vec!["7".to_owned()])]);
        // 9 is out of range, "x" is no number, the nested AutoDrive is no vehicle's.
        assert_eq!(file.marker_references(), 2);
        assert_eq!(patch(&file, &net), FS22.as_bytes());
        // Delete marker 1: 3 -> 2, 2 -> 1; the out-of-range values stay.
        let first = net.markers()[0].node;
        net.remove_marker(first).expect("ok");
        let expected = FS22.replace(
            "firstMarker=\"3\" secondMarker='2' parkDestination=\"3\"",
            "firstMarker=\"2\" secondMarker='1' parkDestination=\"2\"",
        );
        assert_eq!(
            String::from_utf8(patch(&file, &net)).expect("utf8"),
            expected
        );
        // Delete the park target too: the attribute goes with its space, the node ref is 1.
        let third = net.markers()[1].node;
        net.remove_marker(third).expect("ok");
        let expected = FS22.replace(
            "firstMarker=\"3\" secondMarker='2' parkDestination=\"3\"",
            "firstMarker=\"1\" secondMarker='1'",
        );
        assert_eq!(
            String::from_utf8(patch(&file, &net)).expect("utf8"),
            expected
        );
    }

    #[test]
    fn fs19_sample_uses_its_own_element() {
        let mut net = network(3);
        let file = load(FS19.as_bytes(), GameVersion::FS19, &mut net).expect("loads");
        assert_eq!(parked(&net), vec![(2, vec!["4".to_owned()])]);
        assert_eq!(file.marker_references(), 2);
        assert_eq!(patch(&file, &net), FS19.as_bytes());
        let second = net.markers()[1].node;
        net.remove_marker(second).expect("ok");
        let expected = FS19.replace(
            "parkDestination=\"2\" firstMarker=\"1\" secondMarker=\"3\"",
            "firstMarker=\"1\" secondMarker=\"2\"",
        );
        assert_eq!(
            String::from_utf8(patch(&file, &net)).expect("utf8"),
            expected
        );
    }

    #[test]
    fn fs25_reads_unique_id_and_a_new_park_target_adds_no_attribute() {
        let text = "<vehicles><vehicle id=\"1\" uniqueId=\"veh_a\"><AutoDrive firstMarker=\"1\"/>\
                    </vehicle></vehicles>";
        let mut net = network(2);
        let file = load(text.as_bytes(), GameVersion::FS25, &mut net).expect("loads");
        assert!(parked(&net).is_empty());
        assert!(net.park_vehicle(1, "veh_a".into()));
        assert_eq!(patch(&file, &net), text.as_bytes());
        let text = text.replace("firstMarker", "parkDestination");
        let mut net = network(2);
        load(text.as_bytes(), GameVersion::FS25, &mut net).expect("loads");
        assert_eq!(parked(&net), vec![(1, vec!["veh_a".to_owned()])]);
    }

    #[test]
    fn broken_file_is_an_error_and_leaves_the_network() {
        for text in [
            "<vehicles><vehicle id=\"1\"><AutoDrive parkDestination=\"1\"/></vehicles>",
            "<vehicles><vehicle id=\"1\"><AutoDrive parkDestination=\"1\"/></vehicle>",
            "<vehicles><vehicle id=\"1\"><AutoDrive parkDestination=1/></vehicle></vehicles>",
        ] {
            let mut net = network(2);
            assert!(
                load(text.as_bytes(), GameVersion::FS22, &mut net).is_err(),
                "{text}"
            );
            assert!(parked(&net).is_empty(), "{text}");
        }
    }
}
