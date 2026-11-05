//! Lossless load and save of `AutoDrive_config.xml` by byte splicing (AD-6).
//!
//! quick-xml only parses the modelled elements and locates the byte ranges of
//! `<waypoints>` and `<mapmarker>`. Save regenerates those two blocks and
//! splices them into the original bytes.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::ops::Range;

use ad_core::network::{Marker, Network, Node, NodeFlag, NodeId};
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
    by_file_id: &HashMap<i64, NodeId>,
) -> Result<Vec<Vec<NodeId>>, ConfigError> {
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
        s.parse::<f64>().ok()
    })?;
    let ys = parse_list("y", require(waypoints, "y")?, Some(n), |s| {
        s.parse::<f64>().ok()
    })?;
    let zs = parse_list("z", require(waypoints, "z")?, Some(n), |s| {
        s.parse::<f64>().ok()
    })?;
    let flags = match require(waypoints, "flags") {
        Ok(t) => parse_list("flags", t, Some(n), |s| s.parse::<u8>().ok())?,
        Err(_) => vec![0; n],
    };

    let mut network = Network::new();
    let mut by_file_id = HashMap::with_capacity(n);
    let mut order = Vec::with_capacity(n);
    for i in 0..n {
        let id = network.push_node(Node {
            x: xs[i],
            y: ys[i],
            z: zs[i],
            flag: NodeFlag(flags[i]),
            outgoing: Vec::new(),
            incoming: Vec::new(),
        });
        by_file_id.insert(ids[i], id);
        order.push(id);
    }
    let outs = parse_adjacency("out", require(waypoints, "out")?, n, &by_file_id)?;
    let ins = parse_adjacency("incoming", require(waypoints, "incoming")?, n, &by_file_id)?;
    for ((id, out), inc) in order.iter().zip(outs).zip(ins) {
        if let Some(node) = network.node_mut(*id) {
            node.outgoing = out;
            node.incoming = inc;
        }
    }

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

fn join_nodes(
    network: &Network,
    file_ids: &HashMap<NodeId, usize>,
    pick: impl Fn(&Node) -> &Vec<NodeId>,
) -> String {
    let mut out = String::new();
    for (i, id) in network.order().iter().enumerate() {
        if i > 0 {
            out.push(';');
        }
        let list = network.node(*id).map(&pick).filter(|l| !l.is_empty());
        match list {
            None => out.push_str("-1"),
            Some(list) => {
                for (j, target) in list.iter().enumerate() {
                    if j > 0 {
                        out.push(',');
                    }
                    let _ = write!(out, "{}", file_ids.get(target).copied().unwrap_or(0));
                }
            }
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

fn waypoints_block(cfg: &LoadedConfig, file_ids: &HashMap<NodeId, usize>) -> String {
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
            "out" => join_nodes(net, file_ids, |n| &n.outgoing),
            "incoming" => join_nodes(net, file_ids, |n| &n.incoming),
            "flags" => join_nodes_values(net, |n| n.flag.0.to_string()),
            _ => continue,
        };
        let _ = write!(out, "{child_indent}<{name}>{value}</{name}>{nl}");
    }
    out.push_str(&cfg.base_indent);
    out.push_str("</waypoints>");
    out
}

fn mapmarker_block(cfg: &LoadedConfig, file_ids: &HashMap<NodeId, usize>) -> String {
    let nl = &cfg.newline;
    let i1 = format!("{}{}", cfg.base_indent, cfg.unit);
    let i2 = format!("{i1}{}", cfg.unit);
    let mut out = String::from("<mapmarker>");
    out.push_str(nl);
    for (i, m) in cfg.network.markers().iter().enumerate() {
        let n = i + 1;
        let file_id = file_ids.get(&m.node).copied().unwrap_or(0);
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
    let file_ids: HashMap<NodeId, usize> = cfg
        .network
        .order()
        .iter()
        .enumerate()
        .map(|(i, id)| (*id, i + 1))
        .collect();
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
        assert!(cfg.network.node(first).expect("node").outgoing.is_empty());
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
}
