//! AutoDrive route-manager files: a route (`routeExport`), loaded and saved as the mod's
//! `readGraphFromXml` and `writeGraphToXml` do, and the routes list (`routes.xml`, root
//! `autoDriveRoutesManager`), read as `RoutesManager` does.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use ad_core::network::{LoadedNode, Marker, Network, NetworkError, Node, NodeFlag, NodeId};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use thiserror::Error;

const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
const ROUTE_ROOT: &str = "routeExport";
const LIST_ROOT: &str = "autoDriveRoutesManager";

#[derive(Debug, Error)]
pub enum RouteError {
    #[error("XML error: {0}")]
    Xml(String),
    #[error("unexpected root element <{found}>, expected <{expected}>")]
    UnexpectedRoot { found: String, expected: String },
    #[error("missing root element")]
    MissingRoot,
    #[error("missing {0}")]
    Missing(String),
    #[error("invalid value {value:?} in {element}")]
    InvalidValue { element: String, value: String },
    #[error("{element} has {found} values, expected {expected}")]
    CountMismatch {
        element: String,
        found: usize,
        expected: usize,
    },
    #[error("{element} refers to unknown node {value:?}")]
    UnknownNode { element: String, value: String },
    #[error(transparent)]
    Network(#[from] NetworkError),
}

fn xml_err(e: impl std::fmt::Display) -> RouteError {
    RouteError::Xml(e.to_string())
}

fn invalid(element: &str, value: &str) -> RouteError {
    RouteError::InvalidValue {
        element: element.to_owned(),
        value: value.to_owned(),
    }
}

/// A parsed route: the network with its markers, and the group list as read (name, id).
#[derive(Debug, Clone)]
pub struct LoadedRoute {
    pub network: Network,
    pub groups: Vec<(String, u32)>,
    /// True when a flag 2 or 4 was reset to 0, so a save writes different flags.
    pub flags_reset: bool,
}

/// One `route` of `routes.xml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteEntry {
    pub name: String,
    pub file_name: String,
    pub map: String,
    pub revision: i64,
    pub date: String,
    pub server_id: String,
}

/// Minimal element tree: attributes, text and children.
struct Elem {
    name: String,
    attrs: Vec<(String, String)>,
    text: String,
    children: Vec<Elem>,
}

impl Elem {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    fn child(&self, name: &str) -> Option<&Elem> {
        self.children.iter().find(|c| c.name == name)
    }
}

fn start_elem(e: &BytesStart) -> Result<Elem, RouteError> {
    let name = e.name().as_ref().to_owned();
    let mut attrs = Vec::new();
    for attr in e.attributes() {
        let attr = attr.map_err(xml_err)?;
        let key = attr.key.as_ref().to_owned();
        let raw: &str = &attr.value;
        let value = quick_xml::escape::unescape(raw).map_err(xml_err)?;
        attrs.push((key, value.into_owned()));
    }
    Ok(Elem {
        name,
        attrs,
        text: String::new(),
        children: Vec::new(),
    })
}

/// Reads the children and text of `elem`, whose start tag was just consumed.
fn read_children(reader: &mut Reader<&[u8]>, mut elem: Elem) -> Result<Elem, RouteError> {
    loop {
        match reader.read_event().map_err(xml_err)? {
            Event::Text(t) => elem
                .text
                .push_str(&t.xml_content(quick_xml::XmlVersion::Implicit1_0)),
            Event::GeneralRef(r) => match r.resolve_char_ref().map_err(xml_err)? {
                Some(c) => elem.text.push(c),
                None => {
                    let n = r.into_inner();
                    let s = quick_xml::escape::resolve_predefined_entity(&n)
                        .ok_or_else(|| invalid(&elem.name, &format!("&{n};")))?;
                    elem.text.push_str(s);
                }
            },
            Event::Start(e) => {
                let child = start_elem(&e)?;
                elem.children.push(read_children(reader, child)?);
            }
            Event::Empty(e) => elem.children.push(start_elem(&e)?),
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

/// Parses the whole document; the root must be `expected`.
fn parse_tree(bytes: &[u8], expected: &str) -> Result<Elem, RouteError> {
    let bytes = bytes.strip_prefix(BOM).unwrap_or(bytes);
    let mut reader = Reader::from_reader(bytes);
    loop {
        let (elem, empty) = match reader.read_event().map_err(xml_err)? {
            Event::Start(e) => (start_elem(&e)?, false),
            Event::Empty(e) => (start_elem(&e)?, true),
            Event::Eof => return Err(RouteError::MissingRoot),
            _ => continue,
        };
        if elem.name != expected {
            return Err(RouteError::UnexpectedRoot {
                found: elem.name,
                expected: expected.to_owned(),
            });
        }
        return if empty {
            Ok(elem)
        } else {
            read_children(&mut reader, elem)
        };
    }
}

/// The name of the root element, or `None` when there is none or the XML is broken before it.
fn root_name(bytes: &[u8]) -> Option<String> {
    let bytes = bytes.strip_prefix(BOM).unwrap_or(bytes);
    let mut reader = Reader::from_reader(bytes);
    loop {
        match reader.read_event().ok()? {
            Event::Start(e) | Event::Empty(e) => {
                return Some(e.name().as_ref().to_owned());
            }
            Event::Eof => return None,
            _ => {}
        }
    }
}

/// Whether the root element is `routeExport`; anything else is left to the config loader.
pub fn is_route(bytes: &[u8]) -> bool {
    root_name(bytes).as_deref() == Some(ROUTE_ROOT)
}

/// The `;`-separated values of the `waypoints` child `element`, exactly `count` of them.
fn values<'a>(
    waypoints: &'a Elem,
    element: &str,
    count: usize,
) -> Result<Vec<&'a str>, RouteError> {
    let text = waypoints
        .child(element)
        .ok_or_else(|| RouteError::Missing(format!("<{element}>")))?
        .text
        .trim();
    let parts: Vec<&str> = if text.is_empty() {
        Vec::new()
    } else {
        text.split(';').map(str::trim).collect()
    };
    if parts.len() != count {
        return Err(RouteError::CountMismatch {
            element: format!("<{element}>"),
            found: parts.len(),
            expected: count,
        });
    }
    Ok(parts)
}

fn coords(waypoints: &Elem, element: &str, count: usize) -> Result<Vec<f64>, RouteError> {
    values(waypoints, element, count)?
        .into_iter()
        .map(|s| {
            s.parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or_else(|| invalid(&format!("<{element}>"), s))
        })
        .collect()
}

/// A 1-based node id in `1..=count`, as a 0-based index.
fn node_index(element: &str, text: &str, count: usize) -> Result<usize, RouteError> {
    let id: i64 = text
        .trim()
        .parse()
        .map_err(|_| invalid(element, text.trim()))?;
    usize::try_from(id)
        .ok()
        .filter(|id| (1..=count).contains(id))
        .map(|id| id - 1)
        .ok_or_else(|| RouteError::UnknownNode {
            element: element.to_owned(),
            value: text.trim().to_owned(),
        })
}

/// One `,`-list of 1-based ids per node; `-1` is none.
fn adjacency(waypoints: &Elem, element: &str, count: usize) -> Result<Vec<Vec<usize>>, RouteError> {
    let name = format!("<{element}>");
    values(waypoints, element, count)?
        .into_iter()
        .map(|group| {
            if group == "-1" {
                return Ok(Vec::new());
            }
            group
                .split(',')
                .map(|part| node_index(&name, part, count))
                .collect()
        })
        .collect()
}

fn escape_attr(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
}

/// The groups a route save writes: `groups` as read (names and ids kept, also when no
/// marker uses them), then each marker group not in it, in marker order, with fresh ids
/// after the highest one.
pub fn groups_to_write(network: &Network, groups: &[(String, u32)]) -> Vec<(String, u32)> {
    let mut out = groups.to_vec();
    let mut next = groups.iter().map(|(_, id)| *id).max().unwrap_or(0);
    for m in network.markers() {
        if !out.iter().any(|(name, _)| *name == m.group) {
            next += 1;
            out.push((m.group.clone(), next));
        }
    }
    out
}

/// Writes `network` as a `routeExport` file the way the mod's `writeGraphToXml` does:
/// `waypoints#c`; `x`, `y`, `z` with 2 decimals, `out` and `in` as comma lists (`-1` for
/// none) and `flags`, each joined by `;`; markers with the 1-based node id, name and
/// group; the groups of `groups_to_write`. Node ids follow `Network::order` (AD-2).
pub fn save(network: &Network, groups: &[(String, u32)]) -> Vec<u8> {
    let ids = crate::config_xml::SaveIdMap::from_network(network);
    let nodes: Vec<_> = network
        .order()
        .iter()
        .filter_map(|id| network.node(*id))
        .collect();
    let join =
        |f: &dyn Fn(&Node) -> String| nodes.iter().map(|n| f(n)).collect::<Vec<_>>().join(";");
    let links = |list: &[NodeId]| {
        let list: Vec<String> = list
            .iter()
            .filter_map(|t| ids.get(*t).map(|i| i.to_string()))
            .collect();
        if list.is_empty() {
            "-1".to_owned()
        } else {
            list.join(",")
        }
    };
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"no\"?>\n<routeExport>\n",
    );
    let _ = writeln!(out, "    <waypoints c=\"{}\">", nodes.len());
    let _ = writeln!(out, "        <x>{}</x>", join(&|n| format!("{:.2}", n.x)));
    let _ = writeln!(out, "        <y>{}</y>", join(&|n| format!("{:.2}", n.y)));
    let _ = writeln!(out, "        <z>{}</z>", join(&|n| format!("{:.2}", n.z)));
    let _ = writeln!(out, "        <out>{}</out>", join(&|n| links(n.outgoing())));
    let _ = writeln!(out, "        <in>{}</in>", join(&|n| links(n.incoming())));
    let _ = writeln!(
        out,
        "        <flags>{}</flags>",
        join(&|n| n.flag.0.to_string())
    );
    out.push_str("    </waypoints>\n    <markers>\n");
    for m in network.markers() {
        if let Some(i) = ids.get(m.node) {
            let _ = write!(out, "        <m i=\"{i}\" n=\"");
            escape_attr(&m.name, &mut out);
            out.push_str("\" g=\"");
            escape_attr(&m.group, &mut out);
            out.push_str("\"/>\n");
        }
    }
    out.push_str("    </markers>\n    <groups>\n");
    for (name, id) in groups_to_write(network, groups) {
        out.push_str("        <g n=\"");
        escape_attr(&name, &mut out);
        let _ = writeln!(out, "\" i=\"{id}\"/>");
    }
    out.push_str("    </groups>\n</routeExport>\n");
    out.into_bytes()
}

/// Parses a `routeExport` file (the mod's `readGraphFromXml`). Flags 2 and 4 are reset to 0
/// as for FS22/FS25 configs (AD-11); a missing or empty `<flags>` means all 0.
pub fn load(bytes: &[u8]) -> Result<LoadedRoute, RouteError> {
    let root = parse_tree(bytes, ROUTE_ROOT)?;
    let waypoints = root
        .child("waypoints")
        .ok_or_else(|| RouteError::Missing("<waypoints>".to_owned()))?;
    let c = waypoints
        .attr("c")
        .ok_or_else(|| RouteError::Missing("<waypoints c>".to_owned()))?;
    let count: usize = c.trim().parse().map_err(|_| invalid("<waypoints c>", c))?;
    let (xs, ys, zs) = (
        coords(waypoints, "x", count)?,
        coords(waypoints, "y", count)?,
        coords(waypoints, "z", count)?,
    );
    let outs = adjacency(waypoints, "out", count)?;
    let ins = adjacency(waypoints, "in", count)?;
    let flags: Vec<u8> = match waypoints.child("flags") {
        Some(f) if !f.text.trim().is_empty() => values(waypoints, "flags", count)?
            .into_iter()
            .map(|s| s.parse::<u8>().map_err(|_| invalid("<flags>", s)))
            .collect::<Result<_, _>>()?,
        _ => vec![0; count],
    };
    let flags_reset = flags.iter().any(|f| matches!(f, 2 | 4));
    let loaded = outs
        .into_iter()
        .zip(ins)
        .enumerate()
        .map(|(i, (outgoing, incoming))| LoadedNode {
            x: xs[i],
            y: ys[i],
            z: zs[i],
            flag: NodeFlag(if matches!(flags[i], 2 | 4) {
                0
            } else {
                flags[i]
            }),
            outgoing,
            incoming,
        })
        .collect();
    let mut network = Network::from_loaded(loaded)?;

    let attr = |e: &Elem, name: &str| {
        e.attr(name)
            .map(str::to_owned)
            .ok_or_else(|| RouteError::Missing(format!("<{} {name}>", e.name)))
    };
    let markers = root.child("markers").map_or(&[][..], |m| &m.children);
    for m in markers.iter().filter(|m| m.name == "m") {
        let index = node_index("<m i>", &attr(m, "i")?, count)?;
        network.push_marker(Marker {
            node: network.order()[index],
            name: attr(m, "n")?,
            group: attr(m, "g")?,
            parked_vehicles: Vec::new(),
        });
    }
    let groups = root.child("groups").map_or(&[][..], |g| &g.children);
    let groups = groups
        .iter()
        .filter(|g| g.name == "g")
        .enumerate()
        .map(|(i, g)| {
            let id = match g.attr("i") {
                Some(text) => text
                    .trim()
                    .parse::<u32>()
                    .map_err(|_| invalid("<g i>", text))?,
                // The mod's `Utils.getNoNil(..., i + 1)`.
                None => i as u32 + 1,
            };
            Ok((attr(g, "n")?, id))
        })
        .collect::<Result<_, RouteError>>()?;
    Ok(LoadedRoute {
        network,
        groups,
        flags_reset,
    })
}

/// Parses `routes.xml`: each `routes/route` with its attributes and `serverId` child
/// (missing means empty).
pub fn load_list(bytes: &[u8]) -> Result<Vec<RouteEntry>, RouteError> {
    let root = parse_tree(bytes, LIST_ROOT)?;
    let routes = root.child("routes").map_or(&[][..], |r| &r.children);
    routes
        .iter()
        .filter(|r| r.name == "route")
        .map(|r| {
            let attr = |name: &str| {
                r.attr(name)
                    .map(str::to_owned)
                    .ok_or_else(|| RouteError::Missing(format!("<route {name}>")))
            };
            let revision = attr("revision")?;
            Ok(RouteEntry {
                name: attr("name")?,
                file_name: attr("fileName")?,
                map: attr("map")?,
                revision: revision
                    .trim()
                    .parse()
                    .map_err(|_| invalid("<route revision>", &revision))?,
                date: attr("date")?,
                server_id: r
                    .child("serverId")
                    .map(|s| s.text.trim().to_owned())
                    .unwrap_or_default(),
            })
        })
        .collect()
}

/// `routes.xml` two folders up from a route file (`routesManager/routes/<file>.xml`).
pub fn list_path(route_file: &Path) -> Option<PathBuf> {
    Some(route_file.parent()?.parent()?.join("routes.xml"))
}

/// The file name `routes.xml` lists for `file_name`: an `_autosave_<n>` or `_mergeBackup`
/// suffix is replaced by `.xml` (Java `loadRouteManagerXML`).
pub fn listed_file_name(file_name: &str) -> String {
    for suffix in ["_autosave_", "_mergeBackup"] {
        if let Some(at) = file_name.find(suffix) {
            return format!("{}.xml", &file_name[..at]);
        }
    }
    file_name.to_owned()
}

/// Why a route has no map name.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NoMapName {
    #[error("routes.xml not found")]
    NoList,
    #[error("routes.xml could not be read: {0}")]
    Unreadable(String),
    #[error("not listed in routes.xml")]
    NotListed,
}

/// The `map` of the `routes.xml` entry whose `fileName` is the route's listed file name.
pub fn map_name_for(route_file: &Path) -> Result<String, NoMapName> {
    let list = list_path(route_file)
        .filter(|p| p.is_file())
        .ok_or(NoMapName::NoList)?;
    let bytes = std::fs::read(&list).map_err(|e| NoMapName::Unreadable(e.to_string()))?;
    let entries = load_list(&bytes).map_err(|e| NoMapName::Unreadable(e.to_string()))?;
    let file_name = route_file
        .file_name()
        .map(|n| listed_file_name(&n.to_string_lossy()))
        .ok_or(NoMapName::NotListed)?;
    entries
        .into_iter()
        .find(|e| e.file_name == file_name)
        .map(|e| e.map.trim().to_owned())
        .filter(|m| !m.is_empty())
        .ok_or(NoMapName::NotListed)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn route(waypoints: &str, rest: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"no\"?>\n<routeExport>\n    \
             {waypoints}\n{rest}</routeExport>\n"
        )
    }

    fn three(flags: &str) -> String {
        route(
            &format!(
                "<waypoints c=\"3\"><x>1.00;2.50;-3.25</x><y>0.00;1.00;2.00</y>\
                 <z>4.00;5.00;6.00</z><out>2;1,3;-1</out><in>2;1;2</in>{flags}</waypoints>"
            ),
            "    <markers><m i=\"2\" n=\"Hof &amp; Feld\" g=\"Felder\"/></markers>\n    \
             <groups><g n=\"All\" i=\"1\"/><g n=\"Felder\" i=\"2\"/></groups>\n",
        )
    }

    #[test]
    fn parse_reads_nodes_links_flags_markers_and_groups() {
        let r = load(three("<flags>0;1;0</flags>").as_bytes()).expect("loads");
        let net = &r.network;
        let ids = net.order().to_vec();
        assert_eq!(ids.len(), 3);
        let n = |i: usize| net.node(ids[i]).expect("node");
        assert_eq!((n(2).x, n(2).y, n(2).z), (-3.25, 2.0, 6.0));
        assert_eq!(n(0).outgoing(), &[ids[1]]);
        assert_eq!(n(1).outgoing(), &[ids[0], ids[2]]);
        assert!(n(2).outgoing().is_empty());
        assert_eq!(n(2).incoming(), &[ids[1]]);
        let flags: Vec<u8> = (0..3).map(|i| n(i).flag.0).collect();
        assert_eq!(flags, [0, 1, 0]);
        let markers = net.markers();
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].node, ids[1]);
        assert_eq!(markers[0].name, "Hof & Feld");
        assert_eq!(markers[0].group, "Felder");
        assert!(markers[0].parked_vehicles.is_empty());
        assert_eq!(r.groups, [("All".to_owned(), 1), ("Felder".to_owned(), 2)]);
        assert!(!r.flags_reset);
    }

    /// Nodes (position, flag, links by order index) and markers of `net`, for comparing.
    type Shape = (
        Vec<(f64, f64, f64, u8, Vec<usize>, Vec<usize>)>,
        Vec<(usize, String, String)>,
    );

    fn shape(net: &Network) -> Shape {
        let order = net.order();
        let at = |id: &NodeId| order.iter().position(|o| o == id).expect("in order");
        let nodes = order
            .iter()
            .map(|id| {
                let n = net.node(*id).expect("node");
                let links = |l: &[NodeId]| l.iter().map(at).collect::<Vec<_>>();
                (
                    n.x,
                    n.y,
                    n.z,
                    n.flag.0,
                    links(n.outgoing()),
                    links(n.incoming()),
                )
            })
            .collect();
        let markers = net
            .markers()
            .iter()
            .map(|m| (at(&m.node), m.name.clone(), m.group.clone()))
            .collect();
        (nodes, markers)
    }

    #[test]
    fn save_then_load_equals_the_edited_model() {
        let mut r = load(three("<flags>0;1;0</flags>").as_bytes()).expect("loads");
        let ids = r.network.order().to_vec();
        r.network
            .move_node(ids[0], 10.004, -1.5, 7.0)
            .expect("move");
        r.network.remove_node(ids[2]).expect("remove");
        let added = r.network.add_node(-4.0, 3.0, 2.0, NodeFlag(1));
        r.network
            .connect(ids[1], added, ad_core::network::ConnectionShape::Dual)
            .expect("connect");
        r.network
            .set_marker(added, "Say \"<hi>\"".to_owned(), "Höfe & Co".to_owned())
            .expect("marker");
        let text = save(&r.network, &r.groups);
        let back = load(&text).expect("loads back");
        let (mut want_nodes, want_markers) = shape(&r.network);
        want_nodes[0].0 = 10.0;
        assert_eq!(shape(&back.network), (want_nodes, want_markers));
        assert_eq!(
            back.groups,
            [
                ("All".to_owned(), 1),
                ("Felder".to_owned(), 2),
                ("Höfe & Co".to_owned(), 3)
            ]
        );
        let text = String::from_utf8(text).expect("utf-8");
        assert!(text.contains("<x>10.00;2.50;-4.00</x>"), "{text}");
        assert!(text.contains("<out>2;1,3;2</out>"), "{text}");
        assert!(text.contains("<flags>0;1;1</flags>"), "{text}");
        assert!(text.contains("n=\"Say &quot;&lt;hi&gt;&quot;\""), "{text}");
    }

    #[test]
    fn groups_without_markers_are_kept_and_new_ones_get_fresh_ids() {
        let mut r = load(three("").as_bytes()).expect("loads");
        r.groups = vec![
            ("Felder".to_owned(), 2),
            ("Leer".to_owned(), 7),
            ("All".to_owned(), 1),
        ];
        let ids = r.network.order().to_vec();
        r.network
            .set_marker(ids[0], "A".to_owned(), "Neu".to_owned())
            .expect("marker");
        r.network
            .set_marker(ids[2], "B".to_owned(), "Neu".to_owned())
            .expect("marker");
        let back = load(&save(&r.network, &r.groups)).expect("loads back");
        assert_eq!(
            back.groups,
            [
                ("Felder".to_owned(), 2),
                ("Leer".to_owned(), 7),
                ("All".to_owned(), 1),
                ("Neu".to_owned(), 8)
            ]
        );
        let empty = save(&Network::default(), &[]);
        let back = load(&empty).expect("an empty route loads");
        assert!(back.network.is_empty() && back.groups.is_empty());
    }

    #[test]
    fn missing_or_empty_flags_are_all_zero() {
        for flags in ["", "<flags></flags>", "<flags/>"] {
            let r = load(three(flags).as_bytes()).expect("loads");
            let net = &r.network;
            assert!(
                net.order()
                    .iter()
                    .all(|id| net.node(*id).expect("node").flag.0 == 0),
                "{flags}"
            );
        }
    }

    #[test]
    fn spline_flags_2_and_4_are_reset() {
        let r = load(three("<flags>2;4;1</flags>").as_bytes()).expect("loads");
        let net = &r.network;
        let flags: Vec<u8> = net
            .order()
            .iter()
            .map(|id| net.node(*id).expect("node").flag.0)
            .collect();
        assert_eq!(flags, [0, 0, 1]);
        assert!(r.flags_reset);
    }

    #[test]
    fn fewer_values_than_c_names_the_element() {
        let text = three("").replace("<x>1.00;2.50;-3.25</x>", "<x>1.00;2.50</x>");
        let e = load(text.as_bytes()).expect_err("short");
        assert!(
            matches!(&e, RouteError::CountMismatch { element, found: 2, expected: 3 } if element == "<x>")
        );
        assert!(e.to_string().contains("<x>"));
    }

    #[test]
    fn an_id_out_of_range_or_not_a_number_names_the_element() {
        let text = three("").replace("<out>2;1,3;-1</out>", "<out>2;1,7;-1</out>");
        let e = load(text.as_bytes()).expect_err("bad id");
        assert!(
            matches!(&e, RouteError::UnknownNode { element, value } if element == "<out>" && value == "7")
        );
        let text = three("").replace("<in>2;1;2</in>", "<in>2;x;2</in>");
        let e = load(text.as_bytes()).expect_err("not a number");
        assert!(e.to_string().contains("<in>"), "{e}");
        let text = three("").replace("m i=\"2\"", "m i=\"4\"");
        let e = load(text.as_bytes()).expect_err("marker id");
        assert!(e.to_string().contains("<m i>"), "{e}");
    }

    #[test]
    fn a_config_is_not_a_route() {
        assert!(is_route(three("").as_bytes()));
        assert!(!is_route(b"<AutoDrive><version>3</version></AutoDrive>"));
        assert!(!is_route(b"not xml <"));
        assert!(matches!(
            load(b"<AutoDrive/>"),
            Err(RouteError::UnexpectedRoot { .. })
        ));
    }

    fn list(entries: &[(&str, &str)]) -> String {
        let routes: String = entries
            .iter()
            .map(|(file, map)| {
                format!(
                    "        <route name=\"R\" fileName=\"{file}\" map=\"{map}\" revision=\"1\" \
                     date=\"2026/11/08\">\n            <serverId/>\n        </route>\n"
                )
            })
            .collect();
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"no\"?>\n\
             <autoDriveRoutesManager>\n    <routes>\n{routes}    </routes>\n</autoDriveRoutesManager>\n"
        )
    }

    #[test]
    fn routes_list_is_read() {
        let entries = load_list(list(&[("A.xml", "Map A")]).as_bytes()).expect("loads");
        assert_eq!(
            entries,
            [RouteEntry {
                name: "R".to_owned(),
                file_name: "A.xml".to_owned(),
                map: "Map A".to_owned(),
                revision: 1,
                date: "2026/11/08".to_owned(),
                server_id: String::new(),
            }]
        );
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ad_io_routes_{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("routesManager").join("routes")).expect("dir");
        dir.join("routesManager")
    }

    #[test]
    fn map_name_comes_from_routes_xml_two_folders_up() {
        let manager = scratch("map");
        std::fs::write(
            manager.join("routes.xml"),
            list(&[("Other.xml", "X"), ("ABC.xml", "NF_Marsch_4fach")]),
        )
        .expect("write");
        let file = manager.join("routes").join("ABC.xml");
        assert_eq!(map_name_for(&file), Ok("NF_Marsch_4fach".to_owned()));
        let unlisted = manager.join("routes").join("Nope.xml");
        assert_eq!(map_name_for(&unlisted), Err(NoMapName::NotListed));
        let elsewhere = manager.join("ABC.xml");
        assert_eq!(map_name_for(&elsewhere), Err(NoMapName::NoList));
        std::fs::write(manager.join("routes.xml"), "<broken").expect("write");
        assert!(matches!(map_name_for(&file), Err(NoMapName::Unreadable(_))));
        let _ = std::fs::remove_dir_all(manager.parent().expect("dir"));
    }

    #[test]
    fn autosave_and_merge_backup_suffixes_match_the_listed_file() {
        assert_eq!(listed_file_name("ABC_autosave_2.xml"), "ABC.xml");
        assert_eq!(listed_file_name("ABC_mergeBackup.xml"), "ABC.xml");
        assert_eq!(listed_file_name("ABC.xml"), "ABC.xml");
        let manager = scratch("suffix");
        std::fs::write(manager.join("routes.xml"), list(&[("ABC.xml", "M")])).expect("write");
        let file = manager.join("routes").join("ABC_autosave_2.xml");
        assert_eq!(map_name_for(&file), Ok("M".to_owned()));
        let _ = std::fs::remove_dir_all(manager.parent().expect("dir"));
    }
}
