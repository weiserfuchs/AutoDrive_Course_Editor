//! The move tool: a primary drag that starts on a node moves it (x/z only, as Java), and
//! with it the whole selection if the node is selected. The drag is a preview; the release
//! commits one `MoveNodes` (AD-4, AD-10).

use eframe::egui;

use ad_core::command::{Command, MoveNodes, Position, Transaction};
use ad_core::geometry::{MAP_PIXELS, MapGeometry};
use ad_core::network::NodeId;

use super::{Tool, ToolContext, snap_to_grid};
use crate::view::{Preview, ViewTransform};

pub const NAME: &str = "Move";

/// The nodes being dragged: the grabbed one, and the rest of the selection if it is selected.
#[derive(Debug, Clone, PartialEq)]
struct Drag {
    /// The node under the pointer at the start; snap applies to it.
    grabbed: NodeId,
    /// The grabbed node's stored (x, z) when the drag started.
    origin: (f64, f64),
    /// Grabbed node minus pointer at the start, in world units: it stays where it was grabbed.
    grab_offset: (f64, f64),
    /// The grabbed node's previewed world (x, z).
    target: (f64, f64),
    /// The grabbed node's allowed target range (x0, x1, z0, z1): ±half narrowed by the
    /// group's extent, so no moved node leaves the map and the group keeps its shape. It
    /// always holds the start, so a node already outside never makes the group move on its
    /// own; it can only move back toward the map.
    range: (f64, f64, f64, f64),
    /// Every moved node with its stored position when the drag started.
    moved: Vec<(NodeId, Position)>,
    /// The previewed world (x, z) of every moved node.
    preview: Preview,
}

#[derive(Default)]
pub struct MoveTool {
    drag: Option<Drag>,
}

/// Half the side of the world square the map covers: world x and z lie in ±this.
pub fn half_extent(geometry: MapGeometry) -> f64 {
    MAP_PIXELS / 2.0 * geometry.scale
}

/// The node's target for the pointer at world `cursor`, per axis: the pointer plus the grab
/// offset, kept inside ±`half`, then snapped to `grid` if given. An axis whose pointer lies
/// beyond ±`half` keeps `last`, so movement stops there (Java `MoveWidget.calcMoveOffset`).
pub fn drag_target(
    cursor: (f64, f64),
    grab_offset: (f64, f64),
    last: (f64, f64),
    half: f64,
    grid: Option<(f64, f64)>,
) -> (f64, f64) {
    let raw = (
        (cursor.0 + grab_offset.0).clamp(-half, half),
        (cursor.1 + grab_offset.1).clamp(-half, half),
    );
    let (x, z) = match grid {
        Some(g) => {
            // The nearest grid multiple inside the bounds, so a snapped target stays on grid.
            let inner = |v: f64, g: f64| v.clamp((-half / g).ceil() * g, (half / g).floor() * g);
            let (x, z) = snap_to_grid(raw, g);
            (inner(x, g.0), inner(z, g.1))
        }
        None => raw,
    };
    let inside = |c: f64| (-half..=half).contains(&c);
    (
        if inside(cursor.0) { x } else { last.0 },
        if inside(cursor.1) { z } else { last.1 },
    )
}

impl Tool for MoveTool {
    fn name(&self) -> &'static str {
        NAME
    }

    fn on_click(&mut self, _ctx: &ToolContext) -> Option<Transaction> {
        None
    }

    fn on_drag_start(&mut self, ctx: &ToolContext, hovered: Option<NodeId>) -> bool {
        let Some((grabbed, n)) = hovered.and_then(|id| ctx.net.node(id).map(|n| (id, n))) else {
            return false;
        };
        // A drag on a selected node moves the selection; on any other node that node alone.
        let moved: Vec<(NodeId, Position)> = if ctx.selection.contains(&grabbed) {
            ctx.selection
                .iter()
                .filter_map(|&id| ctx.net.node(id).map(|m| (id, (m.x, m.y, m.z))))
                .collect()
        } else {
            vec![(grabbed, (n.x, n.y, n.z))]
        };
        let (mut x0, mut x1, mut z0, mut z1) = (n.x, n.x, n.z, n.z);
        for &(_, (x, _, z)) in &moved {
            (x0, x1, z0, z1) = (x0.min(x), x1.max(x), z0.min(z), z1.max(z));
        }
        let half = half_extent(ctx.map.geometry);
        self.drag = Some(Drag {
            grabbed,
            origin: (n.x, n.z),
            grab_offset: (n.x - ctx.x, n.z - ctx.z),
            target: (n.x, n.z),
            range: (
                n.x.min(-half + (n.x - x0)),
                n.x.max(half - (x1 - n.x)),
                n.z.min(-half + (n.z - z0)),
                n.z.max(half - (z1 - n.z)),
            ),
            preview: {
                let mut p = Preview::default();
                p.place_all(moved.iter().map(|&(id, (x, _, z))| (id, (x, z), (x, z))));
                p
            },
            moved,
        });
        true
    }

    fn on_drag(&mut self, ctx: &ToolContext) {
        let Some(d) = &mut self.drag else {
            return;
        };
        let (x, z) = drag_target(
            (ctx.x, ctx.z),
            d.grab_offset,
            d.target,
            half_extent(ctx.map.geometry),
            ctx.snap.then_some(ctx.map.grid),
        );
        let (x0, x1, z0, z1) = d.range;
        d.target = (x.max(x0).min(x1), z.max(z0).min(z1));
        let (dx, dz) = (d.target.0 - d.origin.0, d.target.1 - d.origin.1);
        let (grabbed, target) = (d.grabbed, d.target);
        d.preview.place_all(d.moved.iter().map(|&(id, (x, _, z))| {
            // The grabbed node takes the target exactly, so a snapped node lands on the grid.
            let at = if id == grabbed {
                target
            } else {
                (x + dx, z + dz)
            };
            (id, (x, z), at)
        }));
    }

    fn on_drag_end(&mut self, ctx: &ToolContext) -> Option<Transaction> {
        self.on_drag(ctx);
        let d = self.drag.take()?;
        // The nodes keep their height (Java moves x/z only).
        let moves: Vec<_> = d
            .moved
            .iter()
            .map(|&(id, before)| {
                let (x, z) = d.preview.get(&id).copied().unwrap_or((before.0, before.2));
                (id, before, (x, before.1, z))
            })
            .collect();
        moves
            .iter()
            .any(|(_, before, after)| before != after)
            .then(|| Transaction::new(vec![Command::MoveNodes(MoveNodes { moves })]))
    }

    fn preview(&self) -> Option<&Preview> {
        self.drag.as_ref().map(|d| &d.preview)
    }

    /// The view draws the moved nodes itself, from [`Tool::preview`].
    fn paint(
        &self,
        _painter: &egui::Painter,
        _t: &ViewTransform,
        _geometry: MapGeometry,
        _cursor: Option<egui::Pos2>,
        _snap_grid: Option<(f64, f64)>,
        _hovered_node: Option<(f64, f64)>,
        _hovered_edge: Option<(egui::Pos2, egui::Pos2)>,
    ) {
    }

    fn cancel(&mut self) {
        self.drag = None;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::collections::BTreeSet;

    use ad_core::map_context::MapContext;
    use ad_core::network::{Network, NodeFlag};

    use super::*;

    const HALF: f64 = 1024.0;
    static NONE: BTreeSet<NodeId> = BTreeSet::new();

    /// A fixture of one node at (10.3, 7, -4.6) on a scale-1 map with a 2 x 4 grid.
    fn setup() -> (Network, NodeId, MapContext) {
        let mut net = Network::new();
        let id = net.add_node(10.3, 7.0, -4.6, NodeFlag::REGULAR);
        (net, id, MapContext::new(None, 1, 2.0, (2.0, 4.0)))
    }

    fn ctx<'a>(
        net: &'a Network,
        map: &'a MapContext,
        x: f64,
        z: f64,
        snap: bool,
    ) -> ToolContext<'a> {
        ToolContext {
            x,
            z,
            map,
            net,
            selection: &NONE,
            snap,
            hovered_node: None,
            hovered_edge: None,
        }
    }

    /// Grabs the node 0.3 left and 0.4 up of its centre, drags to the given pointer
    /// positions and releases at the last; returns the committed transaction.
    fn drag(path: &[(f64, f64)], snap: bool) -> (Network, NodeId, Option<Transaction>) {
        let (net, id, map) = setup();
        let mut tool = MoveTool::default();
        assert!(tool.on_drag_start(&ctx(&net, &map, 10.0, -4.2, snap), Some(id)));
        for &(x, z) in &path[..path.len() - 1] {
            tool.on_drag(&ctx(&net, &map, x, z, snap));
        }
        let (x, z) = path[path.len() - 1];
        let tx = tool.on_drag_end(&ctx(&net, &map, x, z, snap));
        assert_eq!(tool.preview(), None, "the release drops the preview");
        (net, id, tx)
    }

    fn moves(tx: &Transaction, net: &mut Network) -> Position {
        tx.clone().apply(net).expect("applies");
        let n = net.node(net.order()[0]).expect("node");
        (n.x, n.y, n.z)
    }

    #[test]
    fn plain_drag_keeps_the_grab_offset_and_the_height() {
        let (mut net, id, tx) = drag(&[(50.0, 50.0), (100.0, -20.2)], false);
        let tx = tx.expect("a transaction");
        let [Command::MoveNodes(m)] = tx.0.as_slice() else {
            panic!("a MoveNodes");
        };
        let [(moved, before, _)] = m.moves.as_slice() else {
            panic!("one move");
        };
        assert_eq!((*moved, *before), (id, (10.3, 7.0, -4.6)));
        let (x, y, z) = moves(&tx, &mut net);
        assert!((x - 100.3).abs() < 1e-9 && (z - -20.6).abs() < 1e-9);
        assert_eq!(y, 7.0);
    }

    #[test]
    fn snapped_drag_lands_on_the_nearest_grid_point() {
        let (mut net, _, tx) = drag(&[(100.0, -20.2)], true);
        // Target (100.3, -20.6) on a 2 x 4 grid.
        assert_eq!(moves(&tx.expect("tx"), &mut net), (100.0, 7.0, -20.0));
    }

    #[test]
    fn the_preview_follows_the_pointer() {
        let (net, id, map) = setup();
        let mut tool = MoveTool::default();
        assert!(tool.on_drag_start(&ctx(&net, &map, 10.0, -4.2, false), Some(id)));
        assert_eq!(tool.preview().and_then(|p| p.get(&id)), Some(&(10.3, -4.6)));
        tool.on_drag(&ctx(&net, &map, 20.0, -4.2, false));
        let (x, z) = tool.preview().expect("preview")[&id];
        assert!((x - 20.3).abs() < 1e-9 && (z - -4.6).abs() < 1e-9);
    }

    #[test]
    fn a_drag_starting_off_a_node_is_not_claimed() {
        let (net, _, map) = setup();
        let mut tool = MoveTool::default();
        assert!(!tool.on_drag_start(&ctx(&net, &map, 0.0, 0.0, false), None));
        tool.on_drag(&ctx(&net, &map, 5.0, 5.0, false));
        assert!(
            tool.on_drag_end(&ctx(&net, &map, 5.0, 5.0, false))
                .is_none()
        );
    }

    #[test]
    fn a_drag_back_to_the_start_commits_nothing() {
        let (_, _, tx) = drag(&[(40.0, 40.0), (10.0, -4.2)], false);
        assert!(tx.is_none());
    }

    #[test]
    fn cancel_drops_the_preview_and_commits_nothing() {
        let (net, id, map) = setup();
        let mut tool = MoveTool::default();
        assert!(tool.on_drag_start(&ctx(&net, &map, 10.0, -4.2, false), Some(id)));
        tool.on_drag(&ctx(&net, &map, 50.0, 50.0, false));
        tool.cancel();
        assert_eq!(tool.preview(), None);
        assert!(
            tool.on_drag_end(&ctx(&net, &map, 50.0, 50.0, false))
                .is_none()
        );
    }

    #[test]
    fn out_of_bounds_stops_at_the_last_in_bounds_target() {
        let last = (1000.0, 5.0);
        // The pointer leaves on x: x keeps its last value, z still follows.
        assert_eq!(
            drag_target((1030.0, 20.0), (0.0, 0.0), last, HALF, None),
            (1000.0, 20.0)
        );
        assert_eq!(
            drag_target((20.0, -1024.5), (0.0, 0.0), last, HALF, None),
            (20.0, 5.0)
        );
        // Inside, the grab offset cannot push the node past the edge.
        assert_eq!(
            drag_target((1020.0, 0.0), (10.0, 0.0), last, HALF, None),
            (1024.0, 0.0)
        );
        // The bounds scale with the map.
        let half = half_extent(MapGeometry::new(2.0));
        assert_eq!(half, 2048.0);
        assert_eq!(
            drag_target((1030.0, 0.0), (0.0, 0.0), last, half, None),
            (1030.0, 0.0)
        );
        // A snapped target stays inside and on the grid: 1025 is the nearest multiple of 5,
        // 1020 the nearest one inside.
        assert_eq!(
            drag_target((1023.9, -1023.9), (0.0, 0.0), last, HALF, Some((5.0, 5.0))),
            (1020.0, -1020.0)
        );
    }

    #[test]
    fn a_drag_through_out_of_bounds_commits_the_last_in_bounds_target() {
        let (mut net, _, tx) = drag(&[(1000.0, -4.2), (1500.0, -4.2)], false);
        let (x, _, z) = moves(&tx.expect("tx"), &mut net);
        assert!((x - 1000.3).abs() < 1e-9 && (z - -4.6).abs() < 1e-9);
    }

    /// Nodes a (0, 1, 0), b (10, 2, 5), c (-20, 3, 8) and d (50, 4, 50) on a scale-1 map
    /// with a 5 x 5 grid.
    fn group() -> (Network, [NodeId; 4], MapContext) {
        let mut net = Network::new();
        let a = net.add_node(0.0, 1.0, 0.0, NodeFlag::REGULAR);
        let b = net.add_node(10.0, 2.0, 5.0, NodeFlag::REGULAR);
        let c = net.add_node(-20.0, 3.0, 8.0, NodeFlag::REGULAR);
        let d = net.add_node(50.0, 4.0, 50.0, NodeFlag::REGULAR);
        (net, [a, b, c, d], MapContext::new(None, 1, 2.0, (5.0, 5.0)))
    }

    /// Grabs `start` at its centre with `selection`, drags along `path` and releases at its
    /// last point; returns the committed moves, sorted by id.
    fn group_drag(
        net: &Network,
        map: &MapContext,
        selection: &BTreeSet<NodeId>,
        start: NodeId,
        path: &[(f64, f64)],
        snap: bool,
    ) -> Option<Vec<(NodeId, Position, Position)>> {
        let at = |x, z| ToolContext {
            selection,
            ..ctx(net, map, x, z, snap)
        };
        let n = net.node(start).expect("node");
        let mut tool = MoveTool::default();
        assert!(tool.on_drag_start(&at(n.x, n.z), Some(start)));
        for &(x, z) in &path[..path.len() - 1] {
            tool.on_drag(&at(x, z));
        }
        let (x, z) = path[path.len() - 1];
        let tx = tool.on_drag_end(&at(x, z))?;
        let [Command::MoveNodes(m)] = tx.0.as_slice() else {
            panic!("one MoveNodes");
        };
        let mut moves = m.moves.clone();
        moves.sort_by_key(|m| m.0);
        Some(moves)
    }

    #[test]
    fn a_drag_on_a_selected_node_moves_the_selection_by_one_offset() {
        let (net, [a, b, c, _], map) = group();
        let selection = BTreeSet::from([a, b, c]);
        let mut tool = MoveTool::default();
        let at = |x, z| ToolContext {
            selection: &selection,
            ..ctx(&net, &map, x, z, false)
        };
        assert!(tool.on_drag_start(&at(10.0, 5.0), Some(b)));
        tool.on_drag(&at(13.0, 1.0));
        // Every selected node is previewed 3 right and 4 up; d is not.
        let preview = tool.preview().expect("preview");
        assert_eq!(preview.len(), 3);
        assert_eq!(
            (preview[&a], preview[&b], preview[&c]),
            ((3.0, -4.0), (13.0, 1.0), (-17.0, 4.0))
        );
        let moves = group_drag(&net, &map, &selection, b, &[(13.0, 1.0)], false);
        assert_eq!(
            moves.expect("moves"),
            [
                (a, (0.0, 1.0, 0.0), (3.0, 1.0, -4.0)),
                (b, (10.0, 2.0, 5.0), (13.0, 2.0, 1.0)),
                (c, (-20.0, 3.0, 8.0), (-17.0, 3.0, 4.0)),
            ]
        );
    }

    #[test]
    fn a_snapped_group_drag_puts_the_grabbed_node_on_the_grid() {
        let (net, [a, b, c, _], map) = group();
        let selection = BTreeSet::from([a, b, c]);
        // b targets (13.4, 1.1), snapped to (15, 0): an offset of (5, -5) for all.
        let moves = group_drag(&net, &map, &selection, b, &[(13.4, 1.1)], true);
        let after: Vec<_> = moves.expect("moves").iter().map(|m| m.2).collect();
        assert_eq!(
            after,
            [(5.0, 1.0, -5.0), (15.0, 2.0, 0.0), (-15.0, 3.0, 3.0)]
        );
    }

    #[test]
    fn the_group_stops_where_its_outermost_node_meets_the_map_edge() {
        let (net, [a, b, c, _], map) = group();
        let selection = BTreeSet::from([a, b, c]);
        // Rightwards with a grabbed, b (10 right of a) reaches 1024 first: a stops at 1014.
        let moves = group_drag(&net, &map, &selection, a, &[(1020.0, 0.0)], false).expect("moves");
        let after: Vec<_> = moves.iter().map(|m| (m.2.0, m.2.2)).collect();
        assert_eq!(after, [(1014.0, 0.0), (1024.0, 5.0), (994.0, 8.0)]);
        // Leftwards, c (30 left of b) reaches -1024 first: b stops at -994.
        let moves = group_drag(&net, &map, &selection, b, &[(-1010.0, 5.0)], false).expect("moves");
        let after: Vec<_> = moves.iter().map(|m| (m.2.0, m.2.2)).collect();
        assert_eq!(after, [(-1004.0, 0.0), (-994.0, 5.0), (-1024.0, 8.0)]);
        // Upwards on z, a (5 above b) reaches -1024 first; snapped, the edge still wins.
        let moves = group_drag(&net, &map, &selection, b, &[(10.0, -1023.0)], true).expect("moves");
        let after: Vec<_> = moves.iter().map(|m| (m.2.0, m.2.2)).collect();
        assert_eq!(after, [(0.0, -1024.0), (10.0, -1019.0), (-20.0, -1016.0)]);
    }

    #[test]
    fn a_node_stored_past_the_edge_never_moves_the_group_on_its_own() {
        let (mut net, [a, b, _, _], map) = group();
        let e = net.add_node(1100.0, 6.0, 0.0, NodeFlag::REGULAR);
        let selection = BTreeSet::from([a, b, e]);
        // Released where it started: nothing commits.
        assert!(group_drag(&net, &map, &selection, b, &[(10.0, 5.0)], false).is_none());
        // Outwards on x the group cannot go; z still follows.
        assert!(group_drag(&net, &map, &selection, b, &[(30.0, 5.0)], false).is_none());
        let moves = group_drag(&net, &map, &selection, b, &[(30.0, 20.0)], false).expect("moves");
        let after: Vec<_> = moves.iter().map(|m| (m.2.0, m.2.2)).collect();
        assert_eq!(after, [(0.0, 15.0), (10.0, 20.0), (1100.0, 15.0)]);
        // Inwards it moves, with e still outside the map.
        let moves = group_drag(&net, &map, &selection, b, &[(-50.0, 5.0)], false).expect("moves");
        let after: Vec<_> = moves.iter().map(|m| (m.2.0, m.2.2)).collect();
        assert_eq!(after, [(-60.0, 0.0), (-50.0, 5.0), (1040.0, 0.0)]);
    }

    #[test]
    fn a_drag_on_an_unselected_node_moves_it_alone() {
        let (net, [a, b, c, d], map) = group();
        let selection = BTreeSet::from([a, b, c]);
        let moves = group_drag(&net, &map, &selection, d, &[(60.0, 40.0)], false);
        assert_eq!(
            moves.expect("moves"),
            [(d, (50.0, 4.0, 50.0), (60.0, 4.0, 40.0))]
        );
    }

    #[test]
    fn a_group_drag_back_to_the_start_commits_nothing() {
        let (net, [a, b, c, _], map) = group();
        let selection = BTreeSet::from([a, b, c]);
        let path = [(40.0, 40.0), (10.0, 5.0)];
        assert!(group_drag(&net, &map, &selection, b, &path, false).is_none());
    }

    #[test]
    fn undoing_a_group_move_restores_every_start_position() {
        let (mut net, [a, b, c, _], map) = group();
        let selection = BTreeSet::from([a, b, c]);
        let start: Vec<_> = [a, b, c]
            .iter()
            .map(|&id| net.node(id).map(|n| (n.x, n.y, n.z)))
            .collect();
        let moves = group_drag(&net, &map, &selection, b, &[(13.37, -0.1)], false);
        let mut tx = Transaction::new(vec![Command::MoveNodes(MoveNodes {
            moves: moves.expect("moves"),
        })]);
        tx.apply(&mut net).expect("applies");
        tx.revert(&mut net).expect("reverts");
        let back: Vec<_> = [a, b, c]
            .iter()
            .map(|&id| net.node(id).map(|n| (n.x, n.y, n.z)))
            .collect();
        assert_eq!(back, start);
    }
}
