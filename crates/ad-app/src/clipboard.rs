//! Copy and paste of the selection (Java `CopyPasteManager`, `SnapShot`; AD-15).
//!
//! The clipboard is app-level and detached: it holds no `NodeId`, so it survives a cut, an
//! undo and opening another config.

use std::collections::{BTreeSet, HashMap, HashSet};

use ad_core::command::{Command, LinkEnd, PasteNodes, Transaction};
use ad_core::heightmap::sample_height;
use ad_core::map_context::MapContext;
use ad_core::network::{ConnectionShape, Network, NodeFlag, NodeId};

use crate::tools::edit_location::round3;

/// A copied node: position relative to the clipboard's centre, its y and flag.
#[derive(Debug, Clone, PartialEq)]
struct CopiedNode {
    dx: f64,
    dz: f64,
    y: f64,
    flag: NodeFlag,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Clipboard {
    /// Centre of the copied nodes' x/z bounding box.
    centre: (f64, f64),
    nodes: Vec<CopiedNode>,
    /// `(index, name, group)`, in the network's marker order.
    markers: Vec<(usize, String, String)>,
    /// `(from, to, shape)` by index; only links with both ends copied.
    links: Vec<(usize, usize, ConnectionShape)>,
}

impl Clipboard {
    pub fn len(&self) -> usize {
        self.nodes.len()
    }
}

/// The clipboard for `selection`; `None` when no selected node exists.
pub fn copy(net: &Network, selection: &BTreeSet<NodeId>) -> Option<Clipboard> {
    let ids: Vec<NodeId> = selection
        .iter()
        .copied()
        .filter(|id| net.node(*id).is_some())
        .collect();
    let index: HashMap<NodeId, usize> = ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let nodes: Vec<_> = ids.iter().filter_map(|id| net.node(*id)).collect();
    let (x0, x1, z0, z1) = nodes.iter().fold(
        (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ),
        |(x0, x1, z0, z1), n| (x0.min(n.x), x1.max(n.x), z0.min(n.z), z1.max(n.z)),
    );
    if nodes.is_empty() {
        return None;
    }
    let centre = ((x0 + x1) / 2.0, (z0 + z1) / 2.0);
    let markers = net
        .markers()
        .iter()
        .filter_map(|m| Some((*index.get(&m.node)?, m.name.clone(), m.group.clone())))
        .collect();
    let mut links = Vec::new();
    for (i, a) in nodes.iter().enumerate() {
        for b in a.outgoing() {
            let Some(&j) = index.get(b) else {
                continue;
            };
            let Some(nb) = net.node(*b) else {
                continue;
            };
            let a_id = ids[i];
            let b_in = nb.incoming().contains(&a_id);
            let dual = b_in && nb.outgoing().contains(&a_id) && a.incoming().contains(b);
            let shape = match (dual, b_in) {
                // A dual link is listed from both ends; it is kept once, from the lower index.
                (true, _) if i > j => continue,
                (true, _) => ConnectionShape::Dual,
                (false, true) => ConnectionShape::OneWay,
                (false, false) => ConnectionShape::Reverse,
            };
            links.push((i, j, shape));
        }
    }
    Some(Clipboard {
        centre,
        nodes: nodes
            .iter()
            .map(|n| CopiedNode {
                dx: n.x - centre.0,
                dz: n.z - centre.1,
                y: n.y,
                flag: n.flag,
            })
            .collect(),
        markers,
        links,
    })
}

/// `name`, or the first of `name_2`, `name_3`, ... not in `taken`.
fn free_name(name: &str, taken: &HashSet<String>) -> String {
    if !taken.contains(name) {
        return name.to_owned();
    }
    (2u64..)
        .map(|k| format!("{name}_{k}"))
        .find(|n| !taken.contains(n))
        .unwrap_or_else(|| name.to_owned())
}

/// The paste of `clip` centred on `centre`, or at the copied place for `None`. x and z are
/// rounded half-up to three decimals; y is sampled from the heightmap, or the copied y
/// without one. Marker names already used in `net` get the first free `_n` suffix.
pub fn paste(
    clip: &Clipboard,
    centre: Option<(f64, f64)>,
    net: &Network,
    map: &MapContext,
) -> Transaction {
    let (cx, cz) = centre.unwrap_or(clip.centre);
    let nodes = clip
        .nodes
        .iter()
        .map(|n| {
            let (x, z) = (round3(cx + n.dx), round3(cz + n.dz));
            let y = sample_height(map, x, z).unwrap_or(n.y);
            (x, y, z, n.flag)
        })
        .collect();
    let mut taken: HashSet<String> = net.markers().iter().map(|m| m.name.clone()).collect();
    let markers = clip
        .markers
        .iter()
        .map(|(i, name, group)| {
            let name = free_name(name, &taken);
            taken.insert(name.clone());
            (*i, name, group.clone())
        })
        .collect();
    Transaction::new(vec![Command::PasteNodes(PasteNodes::new(
        nodes,
        markers,
        clip.links
            .iter()
            .map(|&(a, b, shape)| (LinkEnd::New(a), LinkEnd::New(b), shape))
            .collect(),
    ))])
}

/// The nodes an applied paste created, to select after it.
pub fn pasted(tx: &Transaction) -> BTreeSet<NodeId> {
    tx.0.iter()
        .filter_map(|c| match c {
            Command::PasteNodes(p) => Some(p.ids()),
            _ => None,
        })
        .flatten()
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::sync::Arc;

    use ad_core::heightmap::Heightmap;
    use ad_core::network::{ConnectionKind, Marker};

    use super::*;

    fn map() -> MapContext {
        MapContext::new(None, 1, 2.0, (2.0, 2.0))
    }

    /// a -> b one-way, b <-> c dual, c -> d reverse, d -> e one-way to an outside node,
    /// a marker "Feld" on b. Returns the network, [a, b, c, d] and e.
    fn group() -> (Network, [NodeId; 4], NodeId) {
        let mut n = Network::new();
        let ids = [
            n.add_node(10.0, 1.0, 20.0, NodeFlag::REGULAR),
            n.add_node(14.0, 2.0, 20.0, NodeFlag::SUB_PRIO),
            n.add_node(14.0, 3.0, 26.0, NodeFlag::REGULAR),
            n.add_node(10.0, 4.0, 26.0, NodeFlag::REGULAR),
        ];
        let e = n.add_node(100.0, 0.0, 100.0, NodeFlag::REGULAR);
        let [a, b, c, d] = ids;
        n.connect(a, b, ConnectionShape::OneWay).expect("ok");
        n.connect(b, c, ConnectionShape::Dual).expect("ok");
        n.connect(c, d, ConnectionShape::Reverse).expect("ok");
        n.connect(d, e, ConnectionShape::OneWay).expect("ok");
        n.connect(e, a, ConnectionShape::OneWay).expect("ok");
        n.push_marker(Marker {
            node: b,
            name: "Feld".into(),
            group: "Felder".into(),
            parked_vehicles: Vec::new(),
        });
        (n, ids, e)
    }

    /// Pastes `clip` into `net` and returns the new ids in order.
    fn paste_into(
        net: &mut Network,
        clip: &Clipboard,
        centre: Option<(f64, f64)>,
        map: &MapContext,
    ) -> Vec<NodeId> {
        let mut tx = paste(clip, centre, net, map);
        tx.apply(net).expect("applies");
        pasted(&tx).into_iter().collect()
    }

    fn pos(net: &Network, id: NodeId) -> (f64, f64, f64) {
        let n = net.node(id).expect("node");
        (n.x, n.y, n.z)
    }

    #[test]
    fn copy_of_nothing_is_none() {
        let (n, ..) = group();
        assert!(copy(&n, &BTreeSet::new()).is_none());
    }

    #[test]
    fn paste_centres_the_group_and_keeps_flags_markers_and_inner_links_only() {
        let (mut n, ids, e) = group();
        let clip = copy(&n, &ids.into_iter().collect()).expect("clip");
        assert_eq!(clip.len(), 4);
        let new = paste_into(&mut n, &clip, Some((-50.0, 70.0)), &map());
        assert_eq!(new.len(), 4);
        // Box x 10..14, z 20..26: centre (12, 23).
        assert_eq!(pos(&n, new[0]), (-52.0, 1.0, 67.0));
        assert_eq!(pos(&n, new[2]), (-48.0, 3.0, 73.0));
        assert_eq!(n.node(new[1]).map(|x| x.flag), Some(NodeFlag::SUB_PRIO));
        assert_eq!(n.connection_kind(new[0], new[1]), ConnectionKind::Regular);
        assert_eq!(n.connection_kind(new[1], new[2]), ConnectionKind::Dual);
        assert_eq!(n.connection_kind(new[2], new[3]), ConnectionKind::Reverse);
        // The shapes are exact: nothing extra on the other direction or the in-lists.
        for (a, b) in [(0, 1), (2, 3)] {
            assert_eq!(n.connection_kind(new[b], new[a]), ConnectionKind::None);
        }
        assert!(n.node(new[3]).expect("d").incoming().is_empty());
        // No link to the outside node, in either direction.
        assert_eq!(n.node(new[3]).expect("d").outgoing(), &[] as &[NodeId]);
        assert!(
            !n.node(e)
                .expect("e")
                .outgoing()
                .iter()
                .any(|t| new.contains(t))
        );
        assert!(n.node(new[0]).expect("a").incoming().is_empty());
        let m = n.markers().last().expect("marker");
        assert_eq!((m.node, &*m.name, &*m.group), (new[1], "Feld_2", "Felder"));
    }

    #[test]
    fn paste_in_place_uses_the_copied_x_and_z() {
        let (mut n, ids, _) = group();
        let clip = copy(&n, &ids.into_iter().collect()).expect("clip");
        let new = paste_into(&mut n, &clip, None, &map());
        for (old, new) in ids.iter().zip(&new) {
            assert_eq!(pos(&n, *old), pos(&n, *new));
        }
    }

    #[test]
    fn x_and_z_are_rounded_half_up_to_three_decimals() {
        let mut n = Network::new();
        let a = n.add_node(0.0, 0.0, 0.0, NodeFlag::REGULAR);
        let clip = copy(&n, &[a].into_iter().collect()).expect("clip");
        let new = paste_into(&mut n, &clip, Some((1.2345, -1.2345)), &map());
        assert_eq!(pos(&n, new[0]), (1.235, 0.0, -1.235));
    }

    #[test]
    fn y_comes_from_the_heightmap_when_the_map_has_one() {
        let (mut n, ids, _) = group();
        let clip = copy(&n, &ids.into_iter().collect()).expect("clip");
        let mut with = map();
        let raw: Vec<u16> = (0..16 * 16).map(|i| 1000 + 37 * i).collect();
        with.heightmap = Some(Arc::new(Heightmap::new(16, 16, raw).expect("heightmap")));
        let new = paste_into(&mut n, &clip, Some((300.0, -200.0)), &with);
        for id in new {
            let (x, y, z) = pos(&n, id);
            assert_eq!(Some(y), sample_height(&with, x, z));
        }
    }

    #[test]
    fn repeated_pastes_make_independent_copies_with_free_marker_names() {
        let (mut n, ids, _) = group();
        let clip = copy(&n, &ids.into_iter().collect()).expect("clip");
        let first = paste_into(&mut n, &clip, Some((0.0, 0.0)), &map());
        let second = paste_into(&mut n, &clip, Some((0.0, 0.0)), &map());
        assert_eq!(n.len(), 13);
        assert!(first.iter().all(|id| !second.contains(id)));
        let names: Vec<&str> = n.markers().iter().map(|m| &*m.name).collect();
        assert_eq!(names, ["Feld", "Feld_2", "Feld_3"]);
        // The copies are not linked to each other.
        for a in &first {
            let node = n.node(*a).expect("node");
            assert!(node.outgoing().iter().all(|t| first.contains(t)));
        }
    }

    #[test]
    fn a_free_marker_name_is_kept_and_names_in_one_paste_do_not_clash() {
        let mut n = Network::new();
        let a = n.add_node(0.0, 0.0, 0.0, NodeFlag::REGULAR);
        let b = n.add_node(1.0, 0.0, 0.0, NodeFlag::REGULAR);
        for (node, name) in [(a, "Hof"), (b, "Hof_2")] {
            n.push_marker(Marker {
                node,
                name: name.into(),
                group: "All".into(),
                parked_vehicles: Vec::new(),
            });
        }
        let clip = copy(&n, &[a, b].into_iter().collect()).expect("clip");
        // Into a network without them: both kept.
        let mut empty = Network::new();
        paste_into(&mut empty, &clip, None, &map());
        let names: Vec<&str> = empty.markers().iter().map(|m| &*m.name).collect();
        assert_eq!(names, ["Hof", "Hof_2"]);
        // Into the same network: "Hof" takes "Hof_3" ("Hof_2" is taken), then "Hof_2" too
        // is taken and becomes "Hof_2_2".
        paste_into(&mut n, &clip, None, &map());
        let names: Vec<&str> = n.markers().iter().map(|m| &*m.name).collect();
        assert_eq!(names, ["Hof", "Hof_2", "Hof_3", "Hof_2_2"]);
    }
}
