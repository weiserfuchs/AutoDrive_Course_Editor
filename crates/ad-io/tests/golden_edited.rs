use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use ad_core::command::{
    AddNodes, Command, Connect, DeleteNodes, Disconnect, EditMarker, LinkEnd, MoveNodes,
    PasteNodes, Position, SetFlag, Transaction,
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
    // The move tool on a selection: three nodes no other edit touches move in one
    // MoveNodes, each to a value the writer's three decimals hold exactly, y kept.
    let busy = [&busy[..], &[flip_a, flip_b]].concat();
    let group: Vec<(NodeId, Position, Position)> = cfg
        .network
        .order()
        .iter()
        .copied()
        .filter(|id| !busy.contains(id))
        .take(3)
        .zip([(300.125, -150.5), (305.625, -148.25), (310.0, -152.75)])
        .filter_map(|(id, (x, z))| {
            let n = cfg.network.node(id)?;
            Some((id, (n.x, n.y, n.z), (x, n.y, z)))
        })
        .collect();
    assert_eq!(group.len(), 3);
    history.apply(
        &mut cfg.network,
        Transaction::new(vec![Command::MoveNodes(MoveNodes {
            moves: group.clone(),
        })]),
    )?;
    // The rotate tool on a selection: three more untouched nodes turn 37° about the centre
    // of their bounding box in one MoveNodes, y kept, at full precision. The turn is
    // ad-app's `tools::rotate::turned`; the writer rounds the results to three decimals.
    let busy = [
        &busy[..],
        &group.iter().map(|g| g.0).collect::<Vec<_>>()[..],
    ]
    .concat();
    let turning: Vec<(NodeId, Position)> = cfg
        .network
        .order()
        .iter()
        .copied()
        .filter(|id| !busy.contains(id))
        .take(3)
        .filter_map(|id| cfg.network.node(id).map(|n| (id, (n.x, n.y, n.z))))
        .collect();
    assert_eq!(turning.len(), 3);
    let (x0, x1, z0, z1) = turning.iter().fold(
        (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ),
        |(x0, x1, z0, z1), &(_, (x, _, z))| (x0.min(x), x1.max(x), z0.min(z), z1.max(z)),
    );
    let (cx, cz) = ((x0 + x1) / 2.0, (z0 + z1) / 2.0);
    let (sin, cos) = 37f64.to_radians().sin_cos();
    let rotated: Vec<(NodeId, Position, Position)> = turning
        .iter()
        .map(|&(id, (x, y, z))| {
            let (dx, dz) = (x - cx, z - cz);
            let after = (cx + dx * cos - dz * sin, y, cz + dx * sin + dz * cos);
            (id, (x, y, z), after)
        })
        .collect();
    assert!(rotated.iter().all(|(_, before, after)| before != after));
    history.apply(
        &mut cfg.network,
        Transaction::new(vec![Command::MoveNodes(MoveNodes {
            moves: rotated.clone(),
        })]),
    )?;
    // The align tool (Shift+X) on a selection: three more untouched nodes take the z of a
    // fourth, the reference, in one MoveNodes; x and y kept, the reference stays put.
    let busy = [
        &busy[..],
        &rotated.iter().map(|g| g.0).collect::<Vec<_>>()[..],
    ]
    .concat();
    let free: Vec<NodeId> = cfg
        .network
        .order()
        .iter()
        .copied()
        .filter(|id| !busy.contains(id))
        .take(4)
        .collect();
    let [reference, ref aligning @ ..] = free[..] else {
        return Err("fixture has too few free nodes".into());
    };
    let ref_z = cfg.network.node(reference).ok_or("reference")?.z;
    let aligned: Vec<(NodeId, Position, Position)> = aligning
        .iter()
        .filter_map(|&id| {
            let n = cfg.network.node(id)?;
            Some((id, (n.x, n.y, n.z), (n.x, n.y, ref_z)))
        })
        .collect();
    assert_eq!(aligned.len(), 3);
    assert!(aligned.iter().all(|(_, before, after)| before != after));
    history.apply(
        &mut cfg.network,
        Transaction::new(vec![Command::MoveNodes(MoveNodes {
            moves: aligned.clone(),
        })]),
    )?;
    // Edit location: one more untouched node is set to typed x, y and z, already rounded to
    // three decimals by the dialog, in one MoveNodes with absolute positions.
    let busy = [&busy[..], &free[..]].concat();
    let located = cfg
        .network
        .order()
        .iter()
        .copied()
        .find(|id| !busy.contains(id))
        .ok_or("fixture has no free node to edit")?;
    let located_before = cfg
        .network
        .node(located)
        .map(|n| (n.x, n.y, n.z))
        .ok_or("located node")?;
    let located_after = (123.457, 45.25, -678.901);
    assert_ne!(located_before, located_after);
    history.apply(
        &mut cfg.network,
        Transaction::new(vec![Command::MoveNodes(MoveNodes {
            moves: vec![(located, located_before, located_after)],
        })]),
    )?;
    // The delete tool or key on a selection: three unmarked nodes no other edit touches go
    // in one DeleteNodes, in id order.
    let busy = [&busy[..], &[located]].concat();
    let mut gone: Vec<NodeId> = cfg
        .network
        .order()
        .iter()
        .copied()
        .filter(|id| *id != added && !busy.contains(id) && !markers.contains(id))
        .take(3)
        .collect();
    gone.sort();
    assert_eq!(gone.len(), 3);
    history.apply(
        &mut cfg.network,
        Transaction::new(vec![Command::DeleteNodes(DeleteNodes::new(gone.clone()))]),
    )?;
    assert!(gone.iter().all(|id| cfg.network.node(*id).is_none()));
    let shaped_kind = cfg.network.connection_kind(shape_a, shape_b);
    assert_ne!(shaped_kind, old_kind);
    let index_of = |id: NodeId| cfg.network.order().iter().position(|n| *n == id);
    let flagged_index = index_of(flagged).ok_or("flag node")?;
    let flipped = (
        index_of(flip_a).ok_or("flip node")?,
        index_of(flip_b).ok_or("flip node")?,
    );
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
    let group_moved: Vec<(usize, (f64, f64, f64))> = group
        .iter()
        .filter_map(|&(id, _, after)| Some((index_of(id)?, after)))
        .collect();
    assert_eq!(group_moved.len(), 3);
    let rotated_moved: Vec<(usize, (f64, f64, f64))> = rotated
        .iter()
        .filter_map(|&(id, _, after)| Some((index_of(id)?, after)))
        .collect();
    assert_eq!(rotated_moved.len(), 3);
    let aligned_moved: Vec<(usize, (f64, f64, f64))> = aligned
        .iter()
        .filter_map(|&(id, _, after)| Some((index_of(id)?, after)))
        .collect();
    assert_eq!(aligned_moved.len(), 3);
    let ref_index = index_of(reference).ok_or("reference")?;
    let located_index = index_of(located).ok_or("located node")?;
    assert_eq!(cfg.network.len(), order.len() - 4);

    let dir = std::env::temp_dir().join(format!("ad_golden_edited_{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let out = dir.join("AutoDrive_config.xml");
    save_to(&cfg, &out)?;
    let written = std::fs::read(&out)?;
    std::fs::remove_dir_all(&dir)?;

    let reloaded = load(&written)?;
    let (mut want, got) = (model(&cfg.network), model(&reloaded.network));
    // The rotated nodes reload within the writer's rounding; y exactly.
    for &(i, (x, y, z)) in &rotated_moved {
        let w = want.0.get_mut(i).ok_or("rotated node")?;
        let g = got.0.get(i).ok_or("rotated node")?;
        assert!(
            (g.0 - x).abs() <= 0.0005 && (g.2 - z).abs() <= 0.0005,
            "{g:?}"
        );
        assert_eq!(g.1, y);
        (w.0, w.2) = (g.0, g.2);
    }
    assert!(want == got, "reloaded model differs from the edited model");
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
    for (i, after) in group_moved {
        let m = net.node(net.order()[i]).ok_or("group node")?;
        assert_eq!((m.x, m.y, m.z), after);
    }
    // The aligned group reloads on the reference's z exactly.
    let ref_z = net.node(net.order()[ref_index]).ok_or("reference")?.z;
    for (i, after) in aligned_moved {
        let m = net.node(net.order()[i]).ok_or("aligned node")?;
        assert_eq!((m.x, m.y, m.z), after);
        assert_eq!(m.z, ref_z);
    }
    // The located node reloads on the typed values exactly.
    let m = net.node(net.order()[located_index]).ok_or("located node")?;
    assert_eq!((m.x, m.y, m.z), located_after);
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

#[test]
fn pasted_group_saves_and_reloads() -> Result<(), Error> {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../AutoDrive_config.xml");
    let original = std::fs::read(&fixture)?;
    let mut cfg = load(&original)?;
    let len = cfg.network.len();
    let marker_count = cfg.network.markers().len();
    // A group as ad-app's paste builds it: rounded x and z, inner links by index, a marker
    // with a free name.
    let paste = PasteNodes::new(
        vec![
            (10.125, 50.0, -20.5, NodeFlag::REGULAR),
            (14.0, 51.25, -20.5, NodeFlag::SUB_PRIO),
            (14.0, 52.5, -16.75, NodeFlag::REGULAR),
        ],
        vec![(1, "Pasted".to_owned(), "All".to_owned())],
        vec![
            (LinkEnd::New(0), LinkEnd::New(1), ConnectionShape::OneWay),
            (LinkEnd::New(1), LinkEnd::New(2), ConnectionShape::Dual),
            (LinkEnd::New(2), LinkEnd::New(0), ConnectionShape::Reverse),
        ],
    );
    let mut history = History::default();
    history.apply(
        &mut cfg.network,
        Transaction::new(vec![Command::PasteNodes(paste)]),
    )?;

    let dir = std::env::temp_dir().join(format!("ad_golden_pasted_{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let out = dir.join("AutoDrive_config.xml");
    save_to(&cfg, &out)?;
    let written = std::fs::read(&out)?;
    std::fs::remove_dir_all(&dir)?;

    let reloaded = load(&written)?;
    assert!(model(&cfg.network) == model(&reloaded.network));
    assert!(untouched(&original)? == untouched(&written)?);
    let net = &reloaded.network;
    assert_eq!(net.len(), len + 3);
    assert_eq!(net.markers().len(), marker_count + 1);
    let new = &net.order()[len..];
    let (a, b, c) = (new[0], new[1], new[2]);
    let m = net.markers().last().ok_or("marker")?;
    assert_eq!((m.node, &*m.name, &*m.group), (b, "Pasted", "All"));
    let n = net.node(a).ok_or("a")?;
    assert_eq!(
        (n.x, n.y, n.z, n.flag),
        (10.125, 50.0, -20.5, NodeFlag::REGULAR)
    );
    assert_eq!(net.connection_kind(a, b), ConnectionKind::Regular);
    assert_eq!(net.connection_kind(b, c), ConnectionKind::Dual);
    assert_eq!(net.connection_kind(c, a), ConnectionKind::Reverse);
    assert_eq!(net.connection_kind(a, c), ConnectionKind::None);
    Ok(())
}

#[test]
fn pasted_chain_linked_to_existing_nodes_saves_and_reloads() -> Result<(), Error> {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../AutoDrive_config.xml");
    let original = std::fs::read(&fixture)?;
    let mut cfg = load(&original)?;
    let len = cfg.network.len();
    // A line or curve commit: new nodes between two existing ones.
    let (start, end) = (cfg.network.order()[0], cfg.network.order()[1]);
    let old = |net: &Network, id: NodeId| {
        net.node(id)
            .map(|n| (n.outgoing().to_vec(), n.incoming().to_vec()))
    };
    let (start_links, end_links) = (old(&cfg.network, start), old(&cfg.network, end));
    let chain = PasteNodes::new(
        vec![
            (11.5, 40.0, 12.25, NodeFlag::REGULAR),
            (13.0, 41.0, 14.0, NodeFlag::REGULAR),
        ],
        Vec::new(),
        vec![
            (
                LinkEnd::Existing(start),
                LinkEnd::New(0),
                ConnectionShape::OneWay,
            ),
            (LinkEnd::New(0), LinkEnd::New(1), ConnectionShape::OneWay),
            (
                LinkEnd::New(1),
                LinkEnd::Existing(end),
                ConnectionShape::Dual,
            ),
        ],
    );
    let mut history = History::default();
    history.apply(
        &mut cfg.network,
        Transaction::new(vec![Command::PasteNodes(chain)]),
    )?;

    let dir = std::env::temp_dir().join(format!("ad_golden_chain_{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let out = dir.join("AutoDrive_config.xml");
    save_to(&cfg, &out)?;
    let written = std::fs::read(&out)?;
    std::fs::remove_dir_all(&dir)?;

    let reloaded = load(&written)?;
    assert!(model(&cfg.network) == model(&reloaded.network));
    assert!(untouched(&original)? == untouched(&written)?);
    let net = &reloaded.network;
    assert_eq!(net.len(), len + 2);
    let new = &net.order()[len..];
    assert_eq!(net.connection_kind(start, new[0]), ConnectionKind::Regular);
    assert_eq!(net.connection_kind(new[0], new[1]), ConnectionKind::Regular);
    assert_eq!(net.connection_kind(new[1], end), ConnectionKind::Dual);
    // Undo restores the existing ends' links exactly.
    history.undo(&mut cfg.network);
    assert_eq!(old(&cfg.network, start), start_links);
    assert_eq!(old(&cfg.network, end), end_links);
    Ok(())
}

#[test]
fn committed_line_saves_and_reloads() -> Result<(), Error> {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../AutoDrive_config.xml");
    let original = std::fs::read(&fixture)?;
    let mut cfg = load(&original)?;
    let len = cfg.network.len();
    // What the dual connect tool commits for a line with 4 segments: three regular nodes
    // at 1/4, 2/4, 3/4 of the way, heights linear, every segment dual. Positions are kept
    // to the 3 decimals the file holds, so the reloaded model compares equal.
    let (start, end) = (cfg.network.order()[2], cfg.network.order()[len - 1]);
    let (s, e) = match (cfg.network.node(start), cfg.network.node(end)) {
        (Some(s), Some(e)) => ((s.x, s.y, s.z), (e.x, e.y, e.z)),
        _ => return Err("fixture nodes".into()),
    };
    let at = |t: f64| {
        let lerp = |a: f64, b: f64| ((a + t * (b - a)) * 1000.0).round() / 1000.0;
        (
            lerp(s.0, e.0),
            lerp(s.1, e.1),
            lerp(s.2, e.2),
            NodeFlag::REGULAR,
        )
    };
    let ends = [
        LinkEnd::Existing(start),
        LinkEnd::New(0),
        LinkEnd::New(1),
        LinkEnd::New(2),
        LinkEnd::Existing(end),
    ];
    let line = PasteNodes::new(
        vec![at(0.25), at(0.5), at(0.75)],
        Vec::new(),
        ends.windows(2)
            .map(|w| (w[0], w[1], ConnectionShape::Dual))
            .collect(),
    );
    let before = model(&cfg.network);
    let mut history = History::default();
    history.apply(
        &mut cfg.network,
        Transaction::new(vec![Command::PasteNodes(line)]),
    )?;

    let dir = std::env::temp_dir().join(format!("ad_golden_line_{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let out = dir.join("AutoDrive_config.xml");
    save_to(&cfg, &out)?;
    let written = std::fs::read(&out)?;
    std::fs::remove_dir_all(&dir)?;

    let reloaded = load(&written)?;
    assert!(model(&cfg.network) == model(&reloaded.network));
    assert!(untouched(&original)? == untouched(&written)?);
    let net = &reloaded.network;
    assert_eq!(net.len(), len + 3);
    let chain = [
        start,
        net.order()[len],
        net.order()[len + 1],
        net.order()[len + 2],
        end,
    ];
    for pair in chain.windows(2) {
        assert_eq!(net.connection_kind(pair[0], pair[1]), ConnectionKind::Dual);
    }
    // One undo removes the whole line.
    history.undo(&mut cfg.network);
    assert!(model(&cfg.network) == before);
    Ok(())
}

#[test]
fn committed_curve_saves_and_reloads() -> Result<(), Error> {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../AutoDrive_config.xml");
    let original = std::fs::read(&fixture)?;
    let mut cfg = load(&original)?;
    let len = cfg.network.len();
    // What the Bézier tool commits for a quadratic curve with 5 interpolation points, the
    // control point pushed 30 units off the midpoint: sub-priority nodes at t = i / 6,
    // heights linear by t, every segment dual. Positions are kept to the 3 decimals the
    // file holds, so the reloaded model compares equal.
    let (start, end) = (cfg.network.order()[2], cfg.network.order()[len - 1]);
    let (s, e) = match (cfg.network.node(start), cfg.network.node(end)) {
        (Some(s), Some(e)) => ((s.x, s.y, s.z), (e.x, e.y, e.z)),
        _ => return Err("fixture nodes".into()),
    };
    let control = ((s.0 + e.0) / 2.0 + 30.0, (s.2 + e.2) / 2.0);
    let round = |v: f64| (v * 1000.0).round() / 1000.0;
    let at = |t: f64| {
        let u = 1.0 - t;
        let bezier = |a: f64, c: f64, b: f64| u * u * a + 2.0 * u * t * c + t * t * b;
        (
            round(bezier(s.0, control.0, e.0)),
            round(s.1 + t * (e.1 - s.1)),
            round(bezier(s.2, control.1, e.2)),
            NodeFlag::SUB_PRIO,
        )
    };
    let nodes: Vec<_> = (1..=5).map(|i| at(f64::from(i) / 6.0)).collect();
    let ends: Vec<LinkEnd> = std::iter::once(LinkEnd::Existing(start))
        .chain((0..5).map(LinkEnd::New))
        .chain([LinkEnd::Existing(end)])
        .collect();
    let curve = PasteNodes::new(
        nodes,
        Vec::new(),
        ends.windows(2)
            .map(|w| (w[0], w[1], ConnectionShape::Dual))
            .collect(),
    );
    let before = model(&cfg.network);
    let mut history = History::default();
    history.apply(
        &mut cfg.network,
        Transaction::new(vec![Command::PasteNodes(curve)]),
    )?;

    let dir = std::env::temp_dir().join(format!("ad_golden_curve_{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let out = dir.join("AutoDrive_config.xml");
    save_to(&cfg, &out)?;
    let written = std::fs::read(&out)?;
    std::fs::remove_dir_all(&dir)?;

    let reloaded = load(&written)?;
    assert!(model(&cfg.network) == model(&reloaded.network));
    assert!(untouched(&original)? == untouched(&written)?);
    let net = &reloaded.network;
    assert_eq!(net.len(), len + 5);
    let chain: Vec<NodeId> = std::iter::once(start)
        .chain(net.order()[len..].iter().copied())
        .chain([end])
        .collect();
    for pair in chain.windows(2) {
        assert_eq!(net.connection_kind(pair[0], pair[1]), ConnectionKind::Dual);
    }
    assert!(
        net.order()[len..]
            .iter()
            .all(|id| net.node(*id).is_some_and(|n| n.flag == NodeFlag::SUB_PRIO))
    );
    // One undo removes the whole curve.
    history.undo(&mut cfg.network);
    assert!(model(&cfg.network) == before);
    Ok(())
}

#[test]
fn committed_arc_saves_and_reloads() -> Result<(), Error> {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../AutoDrive_config.xml");
    let original = std::fs::read(&fixture)?;
    let mut cfg = load(&original)?;
    let len = cfg.network.len();
    // What the arc tool commits for its default half circle (radius 8, -90° to 90°, 6
    // interpolation points) round (100, 50), swapped, without a heightmap: 8 standalone
    // nodes at y -1, linked one-way from the last back to the first. Positions are kept
    // to the 3 decimals the file holds, so the reloaded model compares equal.
    let round = |v: f64| (v * 1000.0).round() / 1000.0;
    let nodes: Vec<_> = (0..8)
        .map(|i| {
            let a = (270.0 + 180.0 * f64::from(i) / 7.0 - 90.0_f64).to_radians();
            let (x, z) = (100.0 + 8.0 * a.cos(), 50.0 + 8.0 * a.sin());
            (round(x), -1.0, round(z), NodeFlag::REGULAR)
        })
        .collect();
    let links = (1..8)
        .rev()
        .map(|i| {
            (
                LinkEnd::New(i),
                LinkEnd::New(i - 1),
                ConnectionShape::OneWay,
            )
        })
        .collect();
    let arc = PasteNodes::new(nodes, Vec::new(), links);
    let before = model(&cfg.network);
    let mut history = History::default();
    history.apply(
        &mut cfg.network,
        Transaction::new(vec![Command::PasteNodes(arc)]),
    )?;

    let dir = std::env::temp_dir().join(format!("ad_golden_arc_{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let out = dir.join("AutoDrive_config.xml");
    save_to(&cfg, &out)?;
    let written = std::fs::read(&out)?;
    std::fs::remove_dir_all(&dir)?;

    let reloaded = load(&written)?;
    assert!(model(&cfg.network) == model(&reloaded.network));
    assert!(untouched(&original)? == untouched(&written)?);
    let net = &reloaded.network;
    assert_eq!(net.len(), len + 8);
    let arc = &net.order()[len..];
    for pair in arc.windows(2) {
        assert_eq!(
            net.connection_kind(pair[1], pair[0]),
            ConnectionKind::Regular
        );
    }
    // Standalone: the arc's nodes link only to each other.
    for id in arc {
        let n = net.node(*id).ok_or("arc node")?;
        assert_eq!(n.y, -1.0);
        assert!(
            n.outgoing()
                .iter()
                .chain(n.incoming())
                .all(|t| arc.contains(t))
        );
    }
    let (first, last) = (
        net.node(arc[0]).ok_or("first")?,
        net.node(arc[7]).ok_or("last")?,
    );
    assert_eq!(
        (first.x, first.z, last.x, last.z),
        (92.0, 50.0, 108.0, 50.0)
    );
    // One undo removes the whole arc.
    history.undo(&mut cfg.network);
    assert!(model(&cfg.network) == before);
    Ok(())
}

#[test]
fn added_marker_in_a_new_group_saves_and_reloads() -> Result<(), Error> {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../AutoDrive_config.xml");
    let original = std::fs::read(&fixture)?;
    let mut cfg = load(&original)?;
    let marker_count = cfg.network.markers().len();
    // As the add-marker dialog commits it: one EditMarker on an existing unmarked node.
    let node = *cfg
        .network
        .order()
        .iter()
        .find(|id| !cfg.network.markers().iter().any(|m| m.node == **id))
        .ok_or("no unmarked node")?;
    let mut history = History::default();
    history.apply(
        &mut cfg.network,
        Transaction::new(vec![Command::EditMarker(EditMarker::new(
            node,
            Some(("Hof".to_owned(), "Neu".to_owned())),
        ))]),
    )?;

    let dir = std::env::temp_dir().join(format!("ad_golden_marker_{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let out = dir.join("AutoDrive_config.xml");
    save_to(&cfg, &out)?;
    let written = std::fs::read(&out)?;
    std::fs::remove_dir_all(&dir)?;

    let reloaded = load(&written)?;
    assert!(model(&cfg.network) == model(&reloaded.network));
    assert!(untouched(&original)? == untouched(&written)?);
    let net = &reloaded.network;
    assert_eq!(net.markers().len(), marker_count + 1);
    let m = net.markers().last().ok_or("marker")?;
    assert_eq!((m.node, &*m.name, &*m.group), (node, "Hof", "Neu"));
    Ok(())
}

#[test]
fn edited_and_deleted_markers_save_reload_and_undo() -> Result<(), Error> {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../AutoDrive_config.xml");
    let original = std::fs::read(&fixture)?;
    let mut cfg = load(&original)?;
    let before = cfg.network.markers().to_vec();
    let [edited, deleted, ..] = &before[..] else {
        return Err("fixture has fewer than two markers".into());
    };
    let (edited, deleted) = (edited.node, deleted.node);
    // A vehicle parked at the deleted marker, as a vehicles.xml load gives it (story 6.1).
    assert!(cfg.network.park_vehicle(1, "7".to_owned()));
    let parked = cfg.network.markers().to_vec();
    let name = format!("{} edited", before[0].name);
    // As the edit-marker dialog and the delete-marker tool commit them: one EditMarker each.
    let mut history = History::default();
    history.apply(
        &mut cfg.network,
        Transaction::new(vec![Command::EditMarker(EditMarker::new(
            edited,
            Some((name.clone(), "Neu".to_owned())),
        ))]),
    )?;
    history.apply(
        &mut cfg.network,
        Transaction::new(vec![Command::EditMarker(EditMarker::new(deleted, None))]),
    )?;

    let dir = std::env::temp_dir().join(format!("ad_golden_edit_marker_{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let out = dir.join("AutoDrive_config.xml");
    save_to(&cfg, &out)?;
    let written = std::fs::read(&out)?;
    std::fs::remove_dir_all(&dir)?;

    let reloaded = load(&written)?;
    assert!(model(&cfg.network) == model(&reloaded.network));
    assert!(untouched(&original)? == untouched(&written)?);
    let net = &reloaded.network;
    assert_eq!(net.markers().len(), before.len() - 1);
    let m = &net.markers()[0];
    assert_eq!((m.node, &*m.name, &*m.group), (edited, &*name, "Neu"));
    assert!(net.markers().iter().all(|m| m.node != deleted));
    // Later markers move up, in order.
    let rest: Vec<NodeId> = net.markers()[1..].iter().map(|m| m.node).collect();
    let want: Vec<NodeId> = before[2..].iter().map(|m| m.node).collect();
    assert_eq!(rest, want);
    // Each undo takes back one step; the deleted marker returns with its vehicle.
    history.undo(&mut cfg.network);
    assert_eq!(cfg.network.markers()[1], parked[1]);
    assert_eq!(cfg.network.markers()[0].name, name);
    history.undo(&mut cfg.network);
    assert_eq!(cfg.network.markers(), parked);
    Ok(())
}
