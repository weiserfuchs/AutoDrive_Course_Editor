//! Lossless load and save of `AutoDrive_config.xml` by byte splicing (AD-6).
//!
//! quick-xml only parses the modelled elements and locates the byte ranges of
//! `<waypoints>` and `<mapmarker>`. Save regenerates those two blocks and
//! splices them into the original bytes.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::ops::Range;
use std::path::{Path, PathBuf};

use ad_core::network::{LoadedNode, Marker, Network, NetworkError, Node, NodeFlag, NodeId};
use quick_xml::events::Event;
use quick_xml::{Reader, XmlVersion};
use thiserror::Error;

const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
const ROOT: &str = "AutoDrive";

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("XML error: {0}")]
    Xml(String),
    #[error("unexpected root element <{0}>, expected <{ROOT}>")]
    UnexpectedRoot(String),
    #[error("missing root element <{ROOT}>")]
    MissingRoot,
    #[error("missing element <{0}>")]
    MissingElement(String),
    #[error("unexpected element <{0}> in <{1}>")]
    UnexpectedElement(String, String),
    #[error("invalid value {value:?} in <{element}>")]
    InvalidValue { element: String, value: String },
    #[error("<{element}> has {found} values, expected {expected}")]
    CountMismatch {
        element: String,
        found: usize,
        expected: usize,
    },
    #[error("<{element}> refers to unknown node {value:?}")]
    UnknownNode { element: String, value: String },
    #[error(transparent)]
    Network(#[from] NetworkError),
    #[error("cannot write {}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

fn xml_err(e: impl std::fmt::Display) -> ConfigError {
    ConfigError::Xml(e.to_string())
}

fn invalid(element: &str, value: &str) -> ConfigError {
    ConfigError::InvalidValue {
        element: element.to_owned(),
        value: value.to_owned(),
    }
}

/// A parsed config: the original bytes plus what is needed to regenerate the
/// two modelled blocks.
#[derive(Debug, Clone)]
pub struct LoadedConfig {
    original: Vec<u8>,
    waypoints: Range<usize>,
    mapmarker: Option<Range<usize>>,
    /// Start of the root end tag; where a missing `<mapmarker>` is inserted.
    root_end: usize,
    newline: String,
    base_indent: String,
    unit: String,
    child_order: Vec<String>,
    pub network: Network,
}

/// Minimal element tree for the two modelled blocks.
struct Elem {
    name: String,
    text: String,
    children: Vec<Elem>,
}

fn name_of(raw: &str) -> String {
    raw.to_owned()
}

/// Reads the element whose start tag was just consumed, through its end tag.
fn read_elem(reader: &mut Reader<&[u8]>, name: String) -> Result<Elem, ConfigError> {
    let mut elem = Elem {
        name,
        text: String::new(),
        children: Vec::new(),
    };
    loop {
        match reader.read_event().map_err(xml_err)? {
            Event::Text(t) => elem.text.push_str(&t.xml_content(XmlVersion::Implicit1_0)),
            Event::CData(t) => elem.text.push_str(&t.xml_content(XmlVersion::Implicit1_0)),
            Event::GeneralRef(r) => {
                if let Some(c) = r.resolve_char_ref().map_err(xml_err)? {
                    elem.text.push(c);
                } else {
                    let n = r.into_inner();
                    match quick_xml::escape::resolve_predefined_entity(&n) {
                        Some(s) => elem.text.push_str(s),
                        None => return Err(invalid(&elem.name, &format!("&{n};"))),
                    }
                }
            }
            Event::Start(e) => {
                let child = read_elem(reader, name_of(e.name().as_ref()))?;
                elem.children.push(child);
            }
            Event::Empty(e) => elem.children.push(Elem {
                name: name_of(e.name().as_ref()),
                text: String::new(),
                children: Vec::new(),
            }),
            Event::End(_) => return Ok(elem),
            Event::Eof => {
                return Err(xml_err(format!(
                    "unexpected end of file in <{}>",
                    elem.name
                )));
            }
            _ => {}
        }
    }
}

fn require<'a>(elem: &'a Elem, name: &str) -> Result<&'a str, ConfigError> {
    elem.children
        .iter()
        .find(|c| c.name == name)
        .map(|c| c.text.as_str())
        .ok_or_else(|| ConfigError::MissingElement(name.to_owned()))
}

fn count_error(element: &str, found: usize, expected: usize) -> ConfigError {
    ConfigError::CountMismatch {
        element: element.to_owned(),
        found,
        expected,
    }
}

fn parse_list<T>(
    element: &str,
    text: &str,
    expected: Option<usize>,
    parse: impl Fn(&str) -> Option<T>,
) -> Result<Vec<T>, ConfigError> {
    let mut out = Vec::new();
    if !text.trim().is_empty() {
        for part in text.split(',') {
            let part = part.trim();
            out.push(parse(part).ok_or_else(|| invalid(element, part))?);
        }
    }
    match expected {
        Some(n) if n != out.len() => Err(count_error(element, out.len(), n)),
        _ => Ok(out),
    }
}

fn parse_adjacency(
    element: &str,
    text: &str,
    expected: usize,
    by_file_id: &HashMap<i64, usize>,
) -> Result<Vec<Vec<usize>>, ConfigError> {
    let mut out = Vec::with_capacity(expected);
    if !text.trim().is_empty() {
        for group in text.split(';') {
            let group = group.trim();
            let mut ids = Vec::new();
            if group != "-1" {
                for part in group.split(',') {
                    let part = part.trim();
                    let file_id: i64 = part.parse().map_err(|_| invalid(element, part))?;
                    let id = by_file_id
                        .get(&file_id)
                        .ok_or_else(|| ConfigError::UnknownNode {
                            element: element.to_owned(),
                            value: part.to_owned(),
                        })?;
                    ids.push(*id);
                }
            }
            out.push(ids);
        }
    }
    if out.len() != expected {
        return Err(count_error(element, out.len(), expected));
    }
    Ok(out)
}

const WAYPOINT_ELEMENTS: [&str; 7] = ["id", "x", "y", "z", "out", "incoming", "flags"];

fn build_network(waypoints: &Elem, mapmarker: Option<&Elem>) -> Result<Network, ConfigError> {
    for c in &waypoints.children {
        if !WAYPOINT_ELEMENTS.contains(&c.name.as_str()) {
            return Err(ConfigError::UnexpectedElement(
                c.name.clone(),
                waypoints.name.clone(),
            ));
        }
    }
    let ids = parse_list("id", require(waypoints, "id")?, None, |s| {
        s.parse::<i64>().ok()
    })?;
    let n = ids.len();
    let xs = parse_list("x", require(waypoints, "x")?, Some(n), |s| {
        s.parse::<f64>().ok().filter(|v| v.is_finite())
    })?;
    let ys = parse_list("y", require(waypoints, "y")?, Some(n), |s| {
        s.parse::<f64>().ok().filter(|v| v.is_finite())
    })?;
    let zs = parse_list("z", require(waypoints, "z")?, Some(n), |s| {
        s.parse::<f64>().ok().filter(|v| v.is_finite())
    })?;
    let flags = match require(waypoints, "flags") {
        Ok(t) => parse_list("flags", t, Some(n), |s| s.parse::<u8>().ok())?,
        Err(_) => vec![0; n],
    };

    let mut by_file_id = HashMap::with_capacity(n);
    for (i, file_id) in ids.iter().enumerate() {
        by_file_id.insert(*file_id, i);
    }
    let outs = parse_adjacency("out", require(waypoints, "out")?, n, &by_file_id)?;
    let ins = parse_adjacency("incoming", require(waypoints, "incoming")?, n, &by_file_id)?;
    let loaded = outs
        .into_iter()
        .zip(ins)
        .enumerate()
        .map(|(i, (outgoing, incoming))| LoadedNode {
            x: xs[i],
            y: ys[i],
            z: zs[i],
            flag: NodeFlag(flags[i]),
            outgoing,
            incoming,
        })
        .collect();
    let mut network = Network::from_loaded(loaded)?;
    let by_file_id: HashMap<i64, NodeId> = by_file_id
        .into_iter()
        .map(|(file_id, i)| (file_id, network.order()[i]))
        .collect();

    for entry in mapmarker.map(|m| m.children.as_slice()).unwrap_or_default() {
        let id_text = require(entry, "id")?;
        let file_id = id_text
            .trim()
            .parse::<f64>()
            .map_err(|_| invalid("id", id_text))?;
        let node = by_file_id
            .get(&(file_id as i64))
            .ok_or_else(|| ConfigError::UnknownNode {
                element: "id".to_owned(),
                value: id_text.to_owned(),
            })?;
        network.push_marker(Marker {
            node: *node,
            name: require(entry, "name")?.to_owned(),
            group: require(entry, "group")?.to_owned(),
        });
    }
    Ok(network)
}

/// Parses a config and remembers the original bytes.
pub fn load(bytes: &[u8]) -> Result<LoadedConfig, ConfigError> {
    let bom_len = if bytes.starts_with(BOM) { BOM.len() } else { 0 };
    let mut reader = Reader::from_reader(&bytes[bom_len..]);

    let mut depth = 0usize;
    let mut waypoints: Option<(Range<usize>, Elem)> = None;
    let mut mapmarker: Option<(Range<usize>, Elem)> = None;
    let mut root_end = None;

    loop {
        let before = reader.buffer_position() as usize + bom_len;
        match reader.read_event().map_err(xml_err)? {
            Event::Start(e) => {
                let name = name_of(e.name().as_ref());
                if depth == 0 {
                    if name != ROOT {
                        return Err(ConfigError::UnexpectedRoot(name));
                    }
                    depth = 1;
                } else if name == "waypoints" && waypoints.is_none() {
                    let elem = read_elem(&mut reader, name)?;
                    waypoints = Some((before..reader.buffer_position() as usize + bom_len, elem));
                } else if name == "mapmarker" && mapmarker.is_none() {
                    let elem = read_elem(&mut reader, name)?;
                    mapmarker = Some((before..reader.buffer_position() as usize + bom_len, elem));
                } else {
                    reader.read_to_end(e.name()).map_err(xml_err)?;
                }
            }
            Event::Empty(e) if depth == 0 => {
                return Err(ConfigError::UnexpectedRoot(name_of(e.name().as_ref())));
            }
            Event::End(_) => {
                root_end = Some(before);
                break;
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if depth == 0 {
        return Err(ConfigError::MissingRoot);
    }
    let root_end = root_end.ok_or_else(|| xml_err("unexpected end of file in <AutoDrive>"))?;
    let (wp_range, wp_elem) =
        waypoints.ok_or_else(|| ConfigError::MissingElement("waypoints".to_owned()))?;
    let network = build_network(&wp_elem, mapmarker.as_ref().map(|(_, e)| e))?;

    let mut child_order: Vec<String> = wp_elem.children.iter().map(|c| c.name.clone()).collect();
    if !child_order.iter().any(|n| n == "flags") {
        child_order.push("flags".to_owned());
    }

    // Newline and indentation come from the whitespace in front of <waypoints>.
    let before_wp = &bytes[..wp_range.start];
    let line_start = before_wp
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |p| p + 1);
    let newline = if before_wp[..line_start].ends_with(b"\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let base_indent: String = before_wp[line_start..]
        .iter()
        .take_while(|&&b| b == b' ' || b == b'\t')
        .map(|&b| b as char)
        .collect();
    let unit = if base_indent.is_empty() {
        "    ".to_owned()
    } else {
        base_indent.clone()
    };

    Ok(LoadedConfig {
        original: bytes.to_vec(),
        waypoints: wp_range,
        mapmarker: mapmarker.map(|(r, _)| r),
        root_end,
        newline: newline.to_owned(),
        base_indent,
        unit,
        child_order,
        network,
    })
}

fn escape_text(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
}

/// The dense 1-based file ids of one save, assigned from `Network::order` (AD-2).
/// It is the only source of ids written to the file.
#[derive(Debug, Clone)]
pub struct SaveIdMap(HashMap<NodeId, usize>);

impl SaveIdMap {
    pub fn from_network(network: &Network) -> Self {
        Self(
            network
                .order()
                .iter()
                .enumerate()
                .map(|(i, id)| (*id, i + 1))
                .collect(),
        )
    }

    pub fn get(&self, id: NodeId) -> Option<usize> {
        self.0.get(&id).copied()
    }
}

/// Joins one adjacency list per node. Entries that refer to a node no longer in the
/// network are skipped; an empty list is written as `-1`.
fn join_nodes(
    network: &Network,
    file_ids: &SaveIdMap,
    pick: impl Fn(&Node) -> &[NodeId],
) -> String {
    let mut out = String::new();
    for (i, id) in network.order().iter().enumerate() {
        if i > 0 {
            out.push(';');
        }
        let targets: Vec<usize> = network
            .node(*id)
            .map(&pick)
            .unwrap_or_default()
            .iter()
            .filter_map(|t| file_ids.get(*t))
            .collect();
        if targets.is_empty() {
            out.push_str("-1");
        }
        for (j, file_id) in targets.iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            let _ = write!(out, "{file_id}");
        }
    }
    out
}

fn join_nodes_values(network: &Network, f: impl Fn(&Node) -> String) -> String {
    let parts: Vec<String> = network
        .order()
        .iter()
        .filter_map(|id| network.node(*id))
        .map(f)
        .collect();
    parts.join(",")
}

fn waypoints_block(cfg: &LoadedConfig, file_ids: &SaveIdMap) -> String {
    let net = &cfg.network;
    let nl = &cfg.newline;
    let child_indent = format!("{}{}", cfg.base_indent, cfg.unit);
    let mut out = String::from("<waypoints>");
    out.push_str(nl);
    for name in &cfg.child_order {
        let value = match name.as_str() {
            "id" => (1..=net.len())
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(","),
            "x" => join_nodes_values(net, |n| format!("{:.3}", n.x)),
            "y" => join_nodes_values(net, |n| format!("{:.3}", n.y)),
            "z" => join_nodes_values(net, |n| format!("{:.3}", n.z)),
            "out" => join_nodes(net, file_ids, |n| n.outgoing()),
            "incoming" => join_nodes(net, file_ids, |n| n.incoming()),
            "flags" => join_nodes_values(net, |n| n.flag.0.to_string()),
            _ => continue,
        };
        let _ = write!(out, "{child_indent}<{name}>{value}</{name}>{nl}");
    }
    out.push_str(&cfg.base_indent);
    out.push_str("</waypoints>");
    out
}

fn mapmarker_block(cfg: &LoadedConfig, file_ids: &SaveIdMap) -> String {
    let nl = &cfg.newline;
    let i1 = format!("{}{}", cfg.base_indent, cfg.unit);
    let i2 = format!("{i1}{}", cfg.unit);
    let mut out = String::from("<mapmarker>");
    out.push_str(nl);
    // Markers whose node is gone are skipped, so the `mmN` numbering stays dense.
    let written = cfg
        .network
        .markers()
        .iter()
        .filter_map(|m| file_ids.get(m.node).map(|file_id| (file_id, m)));
    for (i, (file_id, m)) in written.enumerate() {
        let n = i + 1;
        let _ = write!(
            out,
            "{i1}<mm{n}>{nl}{i2}<id>{file_id}.000000</id>{nl}{i2}<name>"
        );
        escape_text(&m.name, &mut out);
        let _ = write!(out, "</name>{nl}{i2}<group>");
        escape_text(&m.group, &mut out);
        let _ = write!(out, "</group>{nl}{i1}</mm{n}>{nl}");
    }
    out.push_str(&cfg.base_indent);
    out.push_str("</mapmarker>");
    out
}

/// Regenerates the two modelled blocks and splices them into the original bytes.
pub fn save(cfg: &LoadedConfig) -> Vec<u8> {
    let file_ids = SaveIdMap::from_network(&cfg.network);
    let orig = &cfg.original;
    // Replacements as (source range, new bytes), spliced in source-byte order.
    let mut parts: Vec<(Range<usize>, String)> =
        vec![(cfg.waypoints.clone(), waypoints_block(cfg, &file_ids))];
    match &cfg.mapmarker {
        Some(mm) => parts.push((mm.clone(), mapmarker_block(cfg, &file_ids))),
        None if cfg.network.markers().is_empty() => {}
        None => {
            // Insert before the root end tag, which sits at column 0 or on its own line.
            let block = format!(
                "{}{}{}",
                cfg.base_indent,
                mapmarker_block(cfg, &file_ids),
                cfg.newline
            );
            parts.push((cfg.root_end..cfg.root_end, block));
        }
    }
    parts.sort_by_key(|(r, _)| r.start);
    let mut out = Vec::with_capacity(orig.len());
    let mut pos = 0;
    for (range, block) in &parts {
        out.extend_from_slice(&orig[pos..range.start]);
        out.extend_from_slice(block.as_bytes());
        pos = range.end;
    }
    out.extend_from_slice(&orig[pos..]);
    out
}

/// Writes the saved bytes to a temp file in the target's directory and renames it over
/// the target. On any error the target is untouched and the temp file is removed.
pub fn save_to(cfg: &LoadedConfig, target: &Path) -> Result<(), ConfigError> {
    let io_err = |source| ConfigError::Io {
        path: target.to_path_buf(),
        source,
    };
    let dir = match target.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let name = target.file_name().ok_or_else(|| {
        io_err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "path has no file name",
        ))
    })?;
    let mut tmp_name = std::ffi::OsString::from(".");
    tmp_name.push(name);
    tmp_name.push(format!(".{}.tmp", std::process::id()));
    let tmp = dir.join(tmp_name);
    let result = std::fs::File::create(&tmp)
        .and_then(|mut file| {
            std::io::Write::write_all(&mut file, &save(cfg))?;
            file.sync_all()
        })
        .and_then(|()| std::fs::rename(&tmp, target));
    if let Err(source) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(io_err(source));
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn lines(nl: &str, lines: &[&str]) -> String {
        lines.iter().map(|l| format!("{l}{nl}")).collect()
    }

    fn config(nl: &str, bom: bool, markers: &str) -> Vec<u8> {
        let head = lines(
            nl,
            &[
                "<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"no\"?>",
                "<AutoDrive>",
                "    <version>3</version>",
                "    <waypoints>",
                "        <id>1,2</id>",
                "        <x>1.000,2.500</x>",
                "        <y>0.000,0.000</y>",
                "        <z>-3.250,4.000</z>",
                "        <out>2;-1</out>",
                "        <incoming>-1;1</incoming>",
                "        <flags>0,1</flags>",
                "    </waypoints>",
            ],
        );
        let mut v = if bom { BOM.to_vec() } else { Vec::new() };
        v.extend_from_slice(format!("{head}{markers}</AutoDrive>{nl}").as_bytes());
        v
    }

    fn marker_block(nl: &str, name: &str) -> String {
        lines(
            nl,
            &[
                "    <mapmarker>",
                "        <mm1>",
                "            <id>2.000000</id>",
                &format!("            <name>{name}</name>"),
                "            <group>All</group>",
                "        </mm1>",
                "    </mapmarker>",
            ],
        )
    }

    fn roundtrip(bytes: &[u8]) -> LoadedConfig {
        let cfg = load(bytes).expect("loads");
        assert_eq!(
            String::from_utf8_lossy(&save(&cfg)),
            String::from_utf8_lossy(bytes)
        );
        cfg
    }

    #[test]
    fn unusual_flag_value_roundtrips() {
        let text = String::from_utf8(config("\n", false, ""))
            .expect("utf8")
            .replace("<flags>0,1</flags>", "<flags>0,7</flags>");
        let cfg = roundtrip(text.as_bytes());
        let second = cfg.network.order()[1];
        assert_eq!(cfg.network.node(second).map(|n| n.flag), Some(NodeFlag(7)));
    }

    #[test]
    fn small_lf_file_roundtrips() {
        let cfg = roundtrip(&config("\n", false, &marker_block("\n", "Shop")));
        assert_eq!(cfg.network.len(), 2);
        assert_eq!(cfg.network.markers().len(), 1);
    }

    #[test]
    fn crlf_with_bom_roundtrips() {
        roundtrip(&config("\r\n", true, &marker_block("\r\n", "Shop")));
    }

    #[test]
    fn escaped_name_roundtrips() {
        let cfg = roundtrip(&config("\n", false, &marker_block("\n", "A &amp; B")));
        assert_eq!(cfg.network.markers()[0].name, "A & B");
    }

    #[test]
    fn empty_adjacency_roundtrips() {
        let cfg = roundtrip(&config("\n", false, ""));
        let first = cfg.network.order()[1];
        assert!(cfg.network.node(first).expect("node").outgoing().is_empty());
    }

    #[test]
    fn mapmarker_before_waypoints_roundtrips() {
        let text = String::from_utf8(config(
            "
",
            false,
            &marker_block(
                "
", "Shop",
            ),
        ))
        .expect("utf8");
        let wp_start = text.find("    <waypoints>").expect("wp");
        let wp_end = text.find("    <mapmarker>").expect("mm");
        let mm_end = text.find("</AutoDrive>").expect("root");
        let swapped = format!(
            "{}{}{}{}",
            &text[..wp_start],
            &text[wp_end..mm_end],
            &text[wp_start..wp_end],
            &text[mm_end..]
        );
        let cfg = roundtrip(swapped.as_bytes());
        assert_eq!(cfg.network.markers().len(), 1);
    }

    #[test]
    fn no_mapmarker_loads_without_markers() {
        let cfg = roundtrip(&config("\n", false, ""));
        assert!(cfg.network.markers().is_empty());
    }

    #[test]
    fn count_mismatch_names_element() {
        let text = String::from_utf8(config("\n", false, "")).expect("utf8");
        let bad = text.replace("<x>1.000,2.500</x>", "<x>1.000</x>");
        match load(bad.as_bytes()) {
            Err(ConfigError::CountMismatch { element, .. }) => assert_eq!(element, "x"),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn non_finite_coordinate_is_rejected() {
        let text = String::from_utf8(config(
            "
", false, "",
        ))
        .expect("utf8");
        let bad = text.replace("<x>1.000,2.500</x>", "<x>NaN,2.500</x>");
        match load(bad.as_bytes()) {
            Err(ConfigError::InvalidValue { element, value }) => {
                assert_eq!((element.as_str(), value.as_str()), ("x", "NaN"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn bad_number_names_element_and_value() {
        let text = String::from_utf8(config("\n", false, "")).expect("utf8");
        let bad = text.replace("<y>0.000,0.000</y>", "<y>abc,0.000</y>");
        match load(bad.as_bytes()) {
            Err(ConfigError::InvalidValue { element, value }) => {
                assert_eq!((element.as_str(), value.as_str()), ("y", "abc"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn wrong_root_is_rejected() {
        match load(b"<Other><waypoints></waypoints></Other>") {
            Err(e @ ConfigError::UnexpectedRoot(_)) => {
                assert!(e.to_string().contains("unexpected root element"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    fn three_node_config(extra: &str) -> Vec<u8> {
        format!(
            "<AutoDrive>\n    <waypoints>\n        <id>1,2,3</id>\n        <x>1.000,2.000,3.000</x>\n        <y>0.000,0.000,0.000</y>\n        <z>0.000,0.000,0.000</z>\n        <out>2,3;3;-1</out>\n        <incoming>-1;1;2,1</incoming>\n        <flags>0,0,0</flags>\n    </waypoints>\n{extra}</AutoDrive>\n"
        )
        .into_bytes()
    }

    fn history_apply(cfg: &mut LoadedConfig, commands: Vec<ad_core::command::Command>) {
        let mut history = ad_core::history::History::default();
        history
            .apply(
                &mut cfg.network,
                ad_core::command::Transaction::new(commands),
            )
            .expect("applies");
    }

    fn saved_text(cfg: &LoadedConfig) -> String {
        String::from_utf8(save(cfg)).expect("utf8")
    }

    #[test]
    fn deleting_a_middle_node_renumbers() {
        use ad_core::command::{Command, DeleteNodes};
        let mm = marker_block("\n", "Shop").replace("2.000000", "3.000000");
        let mut cfg = load(&three_node_config(&mm)).expect("loads");
        let order = cfg.network.order().to_vec();
        history_apply(
            &mut cfg,
            vec![Command::DeleteNodes(DeleteNodes::new(vec![order[1]]))],
        );
        let text = saved_text(&cfg);
        assert!(text.contains("<id>1,2</id>"), "{text}");
        assert!(text.contains("<out>2;-1</out>"), "{text}");
        assert!(text.contains("<incoming>-1;1</incoming>"), "{text}");
        assert!(text.contains("<id>2.000000</id>"), "{text}");
        let reloaded = load(text.as_bytes()).expect("reloads");
        assert_eq!(reloaded.network.len(), 2);
        assert_eq!(reloaded.network.markers().len(), 1);
    }

    #[test]
    fn missing_flags_are_generated_as_zeros() {
        let text = String::from_utf8(config("\n", false, ""))
            .expect("utf8")
            .replace("        <flags>0,1</flags>\n", "");
        let cfg = load(text.as_bytes()).expect("loads");
        let out = saved_text(&cfg);
        assert!(
            out.contains("        <flags>0,0</flags>\n    </waypoints>"),
            "{out}"
        );
        let reloaded = load(out.as_bytes()).expect("reloads");
        for id in reloaded.network.order() {
            assert_eq!(
                reloaded.network.node(*id).map(|n| n.flag),
                Some(NodeFlag(0))
            );
        }
    }

    #[test]
    fn missing_mapmarker_is_inserted_before_root_end() {
        use ad_core::command::{Command, EditMarker};
        for nl in ["\n", "\r\n"] {
            let mut cfg = load(&config(nl, false, "")).expect("loads");
            let node = cfg.network.order()[1];
            history_apply(
                &mut cfg,
                vec![Command::EditMarker(EditMarker::new(
                    node,
                    Some(("Shop".to_owned(), "All".to_owned())),
                ))],
            );
            let out = saved_text(&cfg);
            let expected = config(nl, false, &marker_block(nl, "Shop"));
            assert_eq!(out, String::from_utf8(expected).expect("utf8"));
            let reloaded = load(out.as_bytes()).expect("reloads");
            assert_eq!(reloaded.network.markers().len(), 1);
        }
    }

    #[test]
    fn removing_all_markers_writes_an_empty_block() {
        use ad_core::command::{Command, EditMarker};
        let mut cfg = load(&config("\n", false, &marker_block("\n", "Shop"))).expect("loads");
        let node = cfg.network.order()[1];
        history_apply(
            &mut cfg,
            vec![Command::EditMarker(EditMarker::new(node, None))],
        );
        let out = saved_text(&cfg);
        assert!(out.contains("    <mapmarker>\n    </mapmarker>\n"), "{out}");
        let reloaded = load(out.as_bytes()).expect("reloads");
        assert!(reloaded.network.markers().is_empty());
    }

    #[test]
    fn dangling_incoming_reference_is_skipped() {
        use ad_core::command::{Command, DeleteNodes};
        // Node 2 lists `incoming` from node 1, but node 1 has no matching `out`.
        let text = String::from_utf8(config("\n", false, ""))
            .expect("utf8")
            .replace("<out>2;-1</out>", "<out>-1;-1</out>");
        let mut cfg = load(text.as_bytes()).expect("loads");
        let first = cfg.network.order()[0];
        history_apply(
            &mut cfg,
            vec![Command::DeleteNodes(DeleteNodes::new(vec![first]))],
        );
        let out = saved_text(&cfg);
        assert!(out.contains("<incoming>-1</incoming>"), "{out}");
        assert!(out.contains("<out>-1</out>"), "{out}");
        assert_eq!(load(out.as_bytes()).expect("reloads").network.len(), 1);
    }

    #[test]
    fn save_id_map_follows_order() {
        let cfg = load(&config("\n", false, "")).expect("loads");
        let ids = SaveIdMap::from_network(&cfg.network);
        let order = cfg.network.order();
        assert_eq!((ids.get(order[0]), ids.get(order[1])), (Some(1), Some(2)));
    }

    fn scratch_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ad_io_{tag}_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    #[test]
    fn save_to_replaces_target_and_leaves_no_temp_file() {
        let dir = scratch_dir("ok");
        let target = dir.join("AutoDrive_config.xml");
        std::fs::write(&target, b"old").expect("seed");
        let bytes = config("\n", false, "");
        let cfg = load(&bytes).expect("loads");
        save_to(&cfg, &target).expect("saves");
        assert_eq!(std::fs::read(&target).expect("read"), bytes);
        assert_eq!(std::fs::read_dir(&dir).expect("ls").count(), 1);
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn save_to_missing_directory_fails_with_io_error() {
        let dir = scratch_dir("fail");
        let target = dir.join("missing").join("AutoDrive_config.xml");
        let cfg = load(&config("\n", false, "")).expect("loads");
        match save_to(&cfg, &target) {
            Err(e @ ConfigError::Io { .. }) => {
                assert!(e.to_string().contains("AutoDrive_config.xml"));
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(!dir.join("missing").exists());
        assert_eq!(std::fs::read_dir(&dir).expect("ls").count(), 0);
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}
