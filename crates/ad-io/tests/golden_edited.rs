use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use ad_core::command::{AddNodes, Command, Connect, DeleteNodes, EditMarker, Transaction};
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
        .take(2)
        .collect();

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
        ]),
    )?;
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
    let kinds: Vec<_> = net
        .node(new_last)
        .ok_or("added")?
        .outgoing()
        .iter()
        .map(|t| net.connection_kind(new_last, *t))
        .collect();
    assert_eq!(kinds, [ConnectionKind::Dual, ConnectionKind::Regular]);
    Ok(())
}
