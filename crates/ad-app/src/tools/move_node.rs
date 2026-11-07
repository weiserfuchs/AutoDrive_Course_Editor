//! The move tool: a primary drag that starts on a node moves it (x/z only, as Java). The
//! drag is a preview; the release commits one `MoveNodes` (AD-4, AD-10).

use eframe::egui;

use ad_core::command::{Command, MoveNodes, Position, Transaction};
use ad_core::geometry::{MAP_PIXELS, MapGeometry};
use ad_core::network::NodeId;

use super::{Tool, ToolContext, snap_to_grid};
use crate::view::ViewTransform;

pub const NAME: &str = "Move";

/// One node being dragged.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Drag {
    id: NodeId,
    /// The stored position when the drag started.
    before: Position,
    /// Node minus pointer at the start, in world units: the node stays where it was grabbed.
    grab_offset: (f64, f64),
    /// The previewed world (x, z).
    target: (f64, f64),
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
        let Some((id, n)) = hovered.and_then(|id| ctx.net.node(id).map(|n| (id, n))) else {
            return false;
        };
        self.drag = Some(Drag {
            id,
            before: (n.x, n.y, n.z),
            grab_offset: (n.x - ctx.x, n.z - ctx.z),
            target: (n.x, n.z),
        });
        true
    }

    fn on_drag(&mut self, ctx: &ToolContext) {
        if let Some(d) = &mut self.drag {
            d.target = drag_target(
                (ctx.x, ctx.z),
                d.grab_offset,
                d.target,
                half_extent(ctx.map.geometry),
                ctx.snap.then_some(ctx.map.grid),
            );
        }
    }

    fn on_drag_end(&mut self, ctx: &ToolContext) -> Option<Transaction> {
        self.on_drag(ctx);
        let d = self.drag.take()?;
        // The node keeps its height (Java moves x/z only).
        let after = (d.target.0, d.before.1, d.target.1);
        (after != d.before).then(|| {
            Transaction::new(vec![Command::MoveNodes(MoveNodes {
                moves: vec![(d.id, d.before, after)],
            })])
        })
    }

    fn preview(&self) -> Option<(NodeId, f64, f64)> {
        self.drag.map(|d| (d.id, d.target.0, d.target.1))
    }

    /// The view draws the moved node itself, from [`Tool::preview`].
    fn paint(
        &self,
        _painter: &egui::Painter,
        _t: &ViewTransform,
        _geometry: MapGeometry,
        _cursor: Option<egui::Pos2>,
        _snap_grid: Option<(f64, f64)>,
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

    use ad_core::map_context::MapContext;
    use ad_core::network::{Network, NodeFlag};

    use super::*;

    const HALF: f64 = 1024.0;

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
        assert_eq!(tool.preview(), Some((id, 10.3, -4.6)));
        tool.on_drag(&ctx(&net, &map, 20.0, -4.2, false));
        let (_, x, z) = tool.preview().expect("preview");
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
}
