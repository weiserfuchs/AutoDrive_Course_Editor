use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use ad_core::command::{
    AddNodes, Command, Connect, DeleteNodes, Disconnect, EditMarker, MoveNodes, SetFlag,
    Transaction,
};
use ad_core::heightmap::{Heightmap, sample_height};
use ad_core::history::History;
use ad_core::map_context::MapContext;
use ad_core::network::{ConnectionKind, ConnectionShape, Network, NodeFlag, NodeId};
use ad_io::config_xml::{GameVersion, load, save_to};

type Error = Box<dyn std::error::Error>;

/// A node as `(x, y, z, flag, outgoing, incoming)` with adjacency as order indexes.
type NodeModel = (f64, f64, f64, u8, Vec<usize>, Vec<usize>);

/// The model compared by order index: nodes, adjacency in order, markers in order.
fn model(net: &Network) -> (Vec<NodeModel>, Vec<(usize, String, String)>) {
    let index: HashMap<NodeId, usize> = net
        .order()
        .iter()
        .enumerate()
        .map(|(i, id)| (*id, i))
        .collect();
    let list = |ids: &[NodeId]| ids.iter().filter_map(|t| index.get(t).copied()).collect();
    let nodes = net
        .order()
        .iter()
        .filter_map(|id| net.node(*id))
        .map(|n| {
            (
                n.x,
                n.y,
                n.z,
                n.flag.0,
                list(n.outgoing()),
                list(n.incoming()),
            )
        })
        .collect();
    let markers = net
        .markers()
        .iter()
        .filter_map(|m| Some((*index.get(&m.node)?, m.name.clone(), m.group.clone())))
        .collect();
    (nodes, markers)
}

fn find(haystack: &[u8], needle: &str) -> Result<usize, Error> {
    haystack
        .windows(needle.len())
        .position(|w| w == needle.as_bytes())
        .ok_or_else(|| format!("{needle} not found").into())
}

/// Returns (prefix, middle, suffix): the bytes outside the two modelled blocks.
type Outside<'a> = (&'a [u8], &'a [u8], &'a [u8]);

fn untouched(bytes: &[u8]) -> Result<Outside<'_>, Error> {
    let wp_start = find(bytes, "<waypoints>")?;
    let wp_end = find(bytes, "</waypoints>")? + "</waypoints>".len();
    let mm_start = find(bytes, "<mapmarker>")?;
    let mm_end = find(bytes, "</mapmarker>")? + "</mapmarker>".len();
    assert!(wp_end <= mm_start, "waypoints must come first");
    Ok((
        &bytes[..wp_start],
        &bytes[wp_end..mm_start],
        &bytes[mm_end..],
    ))
}

#[test]
fn edited_fixture_saves_and_reloads_to_the_same_model() -> Result<(), Error> {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../AutoDrive_config.xml");
    let original = std::fs::read(&fixture)?;
    let mut cfg = load(&original)?;
    assert_eq!(cfg.game_version(), GameVersion::FS25);
    assert!(cfg.read_only().is_none());
    let order: Vec<NodeId> = cfg.network.order().to_vec();

    // A node that some other node lists in `out` without listing it back in `incoming`.
    let reverse_target = order
        .iter()
        .filter_map(|a| cfg.network.node(*a).map(|n| (*a, n)))
        .flat_map(|(a, n)| n.outgoing().iter().map(move |t| (a, *t)))
        .find(|(a, t)| {
            cfg.network
                .node(*t)
                .is_some_and(|n| !n.incoming().contains(a))
        })
        .map(|(_, t)| t)
        .ok_or("fixture has no reverse link")?;
    let markers: Vec<NodeId> = cfg.network.markers().iter().map(|m| m.node).collect();
    let marked = *markers
        .iter()
        .find(|m| **m != reverse_target)
        .ok_or("no marker")?;
    let renamed = *markers
        .iter()
        .find(|m| **m != reverse_target && **m != marked)
        .ok_or("no second marker")?;
    let keep: Vec<NodeId> = order
        .iter()
        .copied()
        .filter(|id| ![reverse_target, marked, renamed].contains(id))
        .take(3)
        .collect();
    let moved = keep[2];
    let before = cfg
        .network
        .node(moved)
        .map(|n| (n.x, n.y, n.z))
        .ok_or("node")?;
    // Values the writer's three decimals hold exactly; y stays (the move tool keeps it).
    let after = (100.25, before.1, -200.5);
    // An existing link between two nodes no other edit touches, removed by the delete tool.
    let touched = [reverse_target, marked, renamed, keep[0], keep[1], keep[2]];
    let (cut_a, cut_b) = order
        .iter()
        .filter(|id| !touched.contains(id))
        .filter_map(|a| cfg.network.node(*a).map(|n| (*a, n)))
        .flat_map(|(a, n)| n.outgoing().iter().map(move |t| (a, *t)))
        .find(|(_, t)| !touched.contains(t))
        .ok_or("fixture has no untouched link")?;
    // The connect tools: a reshaped existing link and a new reverse link, both on nodes no
    // other edit touches.
    let touched = [&touched[..], &[cut_a, cut_b]].concat();
    let (shape_a, shape_b) = order
        .iter()
        .filter(|id| !touched.contains(id))
        .filter_map(|a| cfg.network.node(*a).map(|n| (*a, n)))
        .flat_map(|(a, n)| n.outgoing().iter().map(move |t| (a, *t)))
        .find(|(_, t)| !touched.contains(t))
        .ok_or("fixture has no second untouched link")?;
    let old_kind = cfg.network.connection_kind(shape_a, shape_b);
    let reshaped = if old_kind == ConnectionKind::Dual {
        ConnectionShape::OneWay
    } else {
        ConnectionShape::Dual
    };
    let touched = [&touched[..], &[shape_a, shape_b]].concat();
    let linked = |a: NodeId, b: NodeId| {
        cfg.network
            .node(a)
            .is_some_and(|n| n.outgoing().contains(&b) || n.incoming().contains(&b))
            || cfg
                .network
                .node(b)
                .is_some_and(|n| n.outgoing().contains(&a) || n.incoming().contains(&a))
    };
    let free: Vec<NodeId> = order
        .iter()
        .copied()
        .filter(|id| !touched.contains(id))
        .collect();
    let (rev_a, rev_b) = free
        .iter()
        .flat_map(|a| free.iter().map(move |b| (*a, *b)))
        .find(|(a, b)| a != b && !linked(*a, *b))
        .ok_or("fixture has no unlinked untouched pair")?;

    // The node tool's height: sampled from a small generated heightmap (AD-7).
    let mut map = MapContext::new(None, 1, 2.0, (2.0, 2.0));
    let raw: Vec<u16> = (0..16 * 16).map(|i| 1000 + 37 * i).collect();
    map.heightmap = Some(Arc::new(Heightmap::new(16, 16, raw).ok_or("heightmap")?));
    let (x, z) = (12.5, -40.125);
    let y = sample_height(&map, x, z).ok_or("no height")?;
    assert!(y > 0.0);

    let mut history = History::default();
    // The add runs in its own transaction because `Connect` needs the new node's id.
    history.apply(
        &mut cfg.network,
        Transaction::new(vec![Command::AddNodes(AddNodes::new(vec![(
            x,
            y,
            z,
            NodeFlag::REGULAR,
        )]))]),
    )?;
    let added = *cfg.network.order().last().ok_or("empty")?;

    let (old_name, old_group) = {
        let m = cfg
            .network
            .markers()
            .iter()
            .find(|m| m.node == renamed)
            .ok_or("marker")?;
        (m.name.clone(), m.group.clone())
    };
    history.apply(
        &mut cfg.network,
        Transaction::new(vec![
            Command::DeleteNodes(DeleteNodes::new(vec![reverse_target])),
            Command::DeleteNodes(DeleteNodes::new(vec![marked])),
            Command::Connect(Connect::new(added, keep[0], ConnectionShape::Dual)),
            Command::Connect(Connect::new(added, keep[1], ConnectionShape::OneWay)),
            Command::EditMarker(EditMarker::new(
                renamed,
                Some((format!("{old_name} renamed"), old_group)),
            )),
            Command::EditMarker(EditMarker::new(
                added,
                Some(("New place".to_owned(), "All".to_owned())),
            )),
            Command::MoveNodes(MoveNodes {
                moves: vec![(moved, before, after)],
            }),
            Command::Disconnect(Disconnect::new(cut_a, cut_b)),
        ]),
    )?;
    assert_eq!(
        cfg.network.connection_kind(cut_a, cut_b),
        ConnectionKind::None
    );
    // One transaction per connect-tool click.
    history.apply(
        &mut cfg.network,
        Transaction::new(vec![
            Command::Disconnect(Disconnect::new(shape_a, shape_b)),
            Command::Connect(Connect::new(shape_a, shape_b, reshaped)),
        ]),
    )?;
    history.apply(
        &mut cfg.network,
        Transaction::new(vec![Command::Connect(Connect::new(
            rev_a,
            rev_b,
            ConnectionShape::Reverse,
        ))]),
    )?;
    assert_eq!(
        cfg.network.connection_kind(rev_a, rev_b),
        ConnectionKind::Reverse
    );
    // The priority tool: one node on no other edit swaps regular and sub-priority.
    let flagged = *free
        .iter()
        .find(|id| ![rev_a, rev_b].contains(*id))
        .ok_or("fixture has no free node for the flag")?;
    let old_flag = cfg.network.node(flagged).ok_or("flag node")?.flag;
    let new_flag = if old_flag == NodeFlag::SUB_PRIO {
        NodeFlag::REGULAR
    } else {
        NodeFlag::SUB_PRIO
    };
    history.apply(
        &mut cfg.network,
        Transaction::new(vec![Command::SetFlag(SetFlag::new(flagged, new_flag))]),
    )?;
    let flagged_index = cfg
        .network
        .order()
        .iter()
        .position(|id| *id == flagged)
        .ok_or("flag node")?;
    // The flip tool: a one-way link between nodes no other edit touches turns around.
    let busy = [&touched[..], &[rev_a, rev_b, flagged]].concat();
    let (flip_a, flip_b) = cfg
        .network
        .order()
        .iter()
        .filter(|id| !busy.contains(id))
        .filter_map(|a| cfg.network.node(*a).map(|n| (*a, n)))
        .flat_map(|(a, n)| n.outgoing().iter().map(move |b| (a, *b)))
        .find(|(a, b)| {
            !busy.contains(b)
                && cfg
                    .network
                    .node(*b)
                    .is_some_and(|nb| nb.incoming().contains(a) && !nb.outgoing().contains(a))
                && cfg
                    .network
                    .node(*a)
                    .is_some_and(|na| !na.incoming().contains(b))
        })
        .ok_or("fixture has no free one-way link")?;
    assert_ne!(
        cfg.network.connection_kind(flip_a, flip_b),
        ConnectionKind::None
    );
    history.apply(
        &mut cfg.network,
        Transaction::new(vec![
            Command::Disconnect(Disconnect::new(flip_a, flip_b)),
            Command::Connect(Connect::new(flip_b, flip_a, ConnectionShape::OneWay)),
        ]),
    )?;
    let flipped = (
        cfg.network
            .order()
            .iter()
            .position(|id| *id == flip_a)
            .ok_or("flip node")?,
        cfg.network
            .order()
            .iter()
            .position(|id| *id == flip_b)
            .ok_or("flip node")?,
    );
    let shaped_kind = cfg.network.connection_kind(shape_a, shape_b);
    assert_ne!(shaped_kind, old_kind);
    let index_of = |id: NodeId| cfg.network.order().iter().position(|n| *n == id);
    let cut = (
        index_of(cut_a).ok_or("cut node")?,
        index_of(cut_b).ok_or("cut node")?,
    );
    let shaped = (
        index_of(shape_a).ok_or("reshaped node")?,
        index_of(shape_b).ok_or("reshaped node")?,
    );
    let reverse = (
        index_of(rev_a).ok_or("reverse node")?,
        index_of(rev_b).ok_or("reverse node")?,
    );
    let moved_index = cfg
        .network
        .order()
        .iter()
        .position(|id| *id == moved)
        .ok_or("moved node")?;
    assert_eq!(cfg.network.len(), order.len() - 1);

    let dir = std::env::temp_dir().join(format!("ad_golden_edited_{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let out = dir.join("AutoDrive_config.xml");
    save_to(&cfg, &out)?;
    let written = std::fs::read(&out)?;
    std::fs::remove_dir_all(&dir)?;

    let reloaded = load(&written)?;
    assert!(
        model(&cfg.network) == model(&reloaded.network),
        "reloaded model differs from the edited model"
    );
    assert!(untouched(&original)? == untouched(&written)?);
    assert!(written != original);

    let net = &reloaded.network;
    let new_last = *net.order().last().ok_or("empty")?;
    let named = |name: &str| net.markers().iter().filter(|m| m.name == name).count();
    assert_eq!(net.markers().len(), markers.len());
    assert_eq!(named("New place"), 1);
    assert_eq!(named(&format!("{old_name} renamed")), 1);
    assert_eq!(named(&old_name), 0);
    assert!(net.markers().iter().any(|m| m.node == new_last));
    assert_eq!(net.node(new_last).ok_or("added")?.y, y);
    let m = net.node(net.order()[moved_index]).ok_or("moved")?;
    assert_eq!((m.x, m.y, m.z), after);
    let kinds: Vec<_> = net
        .node(new_last)
        .ok_or("added")?
        .outgoing()
        .iter()
        .map(|t| net.connection_kind(new_last, *t))
        .collect();
    assert_eq!(kinds, [ConnectionKind::Dual, ConnectionKind::Regular]);
    let (ra, rb) = (net.order()[cut.0], net.order()[cut.1]);
    assert_eq!(net.connection_kind(ra, rb), ConnectionKind::None);
    assert_eq!(net.connection_kind(rb, ra), ConnectionKind::None);
    let (sa, sb) = (net.order()[shaped.0], net.order()[shaped.1]);
    assert_eq!(net.connection_kind(sa, sb), shaped_kind);
    let (va, vb) = (net.order()[reverse.0], net.order()[reverse.1]);
    assert_eq!(net.connection_kind(va, vb), ConnectionKind::Reverse);
    assert_eq!(net.connection_kind(vb, va), ConnectionKind::None);
    assert_eq!(
        net.node(net.order()[flagged_index])
            .ok_or("flag node")?
            .flag,
        new_flag
    );
    let (fa, fb) = (net.order()[flipped.0], net.order()[flipped.1]);
    assert_eq!(net.connection_kind(fa, fb), ConnectionKind::None);
    let back = net.node(fb).ok_or("flip node")?;
    assert!(back.outgoing().contains(&fa));
    assert!(net.node(fa).ok_or("flip node")?.incoming().contains(&fb));
    assert!(!back.incoming().contains(&fa));
    Ok(())
}
