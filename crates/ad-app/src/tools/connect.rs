//! The connect tools (regular, dual, reverse): a click on a start node, then on an end
//! node, creates the tool's link between them, removes it if it is already exactly that,
//! or reshapes any other link into it, in one transaction (AD-3, AD-4, AD-10). A long
//! line gets interpolated nodes, and optionally a click on empty map ends it at a new
//! node (Java `LinearLine`).

use eframe::egui;

use ad_core::command::{Command, Connect, Disconnect, LinkEnd, PasteNodes, Transaction};
use ad_core::geometry::MapGeometry;
use ad_core::heightmap::sample_height;
use ad_core::map_context::MapContext;
use ad_core::network::{ConnectionShape, Network, NodeFlag, NodeId};
use ad_io::settings::GlobalSettings;

use super::{Tool, ToolContext, snap_to_grid};
use crate::look;
use crate::view::ViewTransform;

pub const REGULAR_NAME: &str = "Connect";
pub const DUAL_NAME: &str = "Dual";
pub const REVERSE_NAME: &str = "Reverse";

/// Radius of the ring on the start node, in screen px.
const RING_RADIUS_PX: f32 = 8.0;
/// Stroke width of the pending link and the ring, in screen px.
const STROKE_PX: f32 = 2.0;
/// Radius of the dot on each point the line would add, in screen px.
const POINT_RADIUS_PX: f32 = 3.0;

/// The line options of the settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineOptions {
    /// Spacing of the interpolated nodes in world units (Java `LinearLineNodeDistance`).
    pub node_distance: f64,
    /// Whether a click on empty map ends the line at a new node (Java
    /// `CreateLinearLineEndNode`).
    pub end_node: bool,
}

impl LineOptions {
    pub fn from_settings(global: &GlobalSettings) -> Self {
        Self {
            node_distance: f64::from(global.line_node_distance.max(1)),
            end_node: global.create_line_end_node,
        }
    }
}

/// The start node of a pending link and its world (x, z) when it was picked.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Start {
    id: NodeId,
    x: f64,
    z: f64,
}

/// Links two nodes with `shape`; with `continuous`, the end node starts the next link.
pub struct ConnectTool {
    pub shape: ConnectionShape,
    start: Option<Start>,
    pub continuous: bool,
    pub line: LineOptions,
    /// The last click returned a line to a new end node, which starts the next line once
    /// it is committed.
    from_new_end: bool,
}

impl ConnectTool {
    pub fn new(shape: ConnectionShape, continuous: bool, line: LineOptions) -> Self {
        Self {
            shape,
            start: None,
            continuous,
            line,
            from_new_end: false,
        }
    }

    /// The start node of the pending link, if any.
    #[cfg(test)]
    pub fn start(&self) -> Option<NodeId> {
        self.start.map(|s| s.id)
    }
}

/// The connect tool the connect shortcut selects (Java cycles the line group): dual after
/// regular, reverse after dual, regular otherwise.
pub fn cycled(active: Option<&dyn Tool>, continuous: bool, line: LineOptions) -> ConnectTool {
    let shape = match active.map(Tool::name) {
        Some(REGULAR_NAME) => ConnectionShape::Dual,
        Some(DUAL_NAME) => ConnectionShape::Reverse,
        _ => ConnectionShape::OneWay,
    };
    ConnectTool::new(shape, continuous, line)
}

/// The points a line from `start` to `end` (world x, z) adds, each as (x, z, fraction of
/// the way). With `segments = round(distance / node_distance)` above 1 they are the
/// points at `i / segments` for `i = 1 .. segments - 1`; either way `end` follows unless
/// `end_is_node` (Java `generatePreviewInterpolationPoints`).
pub fn line_points(
    start: (f64, f64),
    end: (f64, f64),
    node_distance: f64,
    end_is_node: bool,
) -> Vec<(f64, f64, f64)> {
    let (dx, dz) = (end.0 - start.0, end.1 - start.1);
    let segments = ((dx.hypot(dz) / node_distance).round() as usize).max(1);
    let last = if end_is_node { segments - 1 } else { segments };
    (1..=last)
        .map(|i| {
            if i == segments {
                return (end.0, end.1, 1.0);
            }
            let t = i as f64 / segments as f64;
            (start.0 + t * dx, start.1 + t * dz, t)
        })
        .collect()
}

/// One `PasteNodes` of `nodes` (world x, y, z) with `flag`, linked `start` → nodes →
/// `end` (when given) with `shape` on every segment.
pub(crate) fn chain(
    start: NodeId,
    nodes: Vec<(f64, f64, f64)>,
    end: Option<NodeId>,
    shape: ConnectionShape,
    flag: NodeFlag,
) -> Transaction {
    let ends: Vec<LinkEnd> = std::iter::once(LinkEnd::Existing(start))
        .chain((0..nodes.len()).map(LinkEnd::New))
        .chain(end.map(LinkEnd::Existing))
        .collect();
    let links = ends.windows(2).map(|w| (w[0], w[1], shape)).collect();
    let nodes = nodes.into_iter().map(|(x, y, z)| (x, y, z, flag)).collect();
    Transaction::new(vec![Command::PasteNodes(PasteNodes::new(
        nodes,
        Vec::new(),
        links,
    ))])
}

/// The transaction of a line from `start` through `points` (from [`line_points`]) to the
/// node `end`, or ending at the last point when `end` is `None`. Each new node's height
/// is sampled, or without a heightmap runs linearly by fraction from the start's y to the
/// end node's y (the start's y for a new end point) (AD-7). The new nodes get `flag`.
/// `None` for a missing node.
pub fn plan_line(
    net: &Network,
    map: &MapContext,
    start: NodeId,
    points: &[(f64, f64, f64)],
    end: Option<NodeId>,
    shape: ConnectionShape,
    flag: NodeFlag,
) -> Option<Transaction> {
    let y0 = net.node(start)?.y;
    let y1 = match end {
        Some(id) => net.node(id)?.y,
        None => y0,
    };
    let nodes = points
        .iter()
        .map(|&(x, z, t)| {
            let y = sample_height(map, x, z).unwrap_or(y0 + (y1 - y0) * t);
            (x, y, z)
        })
        .collect();
    Some(chain(start, nodes, end, shape, flag))
}

/// Whether the links between `a` and `b` are exactly `shape` from `a` to `b`.
pub(crate) fn is_exactly(net: &Network, a: NodeId, b: NodeId, shape: ConnectionShape) -> bool {
    let (Some(na), Some(nb)) = (net.node(a), net.node(b)) else {
        return false;
    };
    let a_out = na.outgoing().contains(&b);
    let a_in = na.incoming().contains(&b);
    let b_out = nb.outgoing().contains(&a);
    let b_in = nb.incoming().contains(&a);
    match shape {
        ConnectionShape::OneWay => a_out && b_in && !b_out && !a_in,
        ConnectionShape::Dual => a_out && b_in && b_out && a_in,
        ConnectionShape::Reverse => a_out && !b_in && !b_out && !a_in,
    }
}

/// The transaction an end click on `b` with start `a` commits: `Connect` when the two are
/// unlinked, `Disconnect` when the links are exactly `shape` from `a` to `b`, otherwise
/// `Disconnect` then `Connect`. `None` for a missing node or `a == b`.
pub fn plan(net: &Network, a: NodeId, b: NodeId, shape: ConnectionShape) -> Option<Transaction> {
    let (na, nb) = (net.node(a)?, net.node(b)?);
    if a == b {
        return None;
    }
    let linked = na.outgoing().contains(&b)
        || na.incoming().contains(&b)
        || nb.outgoing().contains(&a)
        || nb.incoming().contains(&a);
    let disconnect = Command::Disconnect(Disconnect::new(a, b));
    let connect = Command::Connect(Connect::new(a, b, shape));
    let commands = if !linked {
        vec![connect]
    } else if is_exactly(net, a, b, shape) {
        vec![disconnect]
    } else {
        vec![disconnect, connect]
    };
    Some(Transaction::new(commands))
}

impl Tool for ConnectTool {
    fn name(&self) -> &'static str {
        match self.shape {
            ConnectionShape::OneWay => REGULAR_NAME,
            ConnectionShape::Dual => DUAL_NAME,
            ConnectionShape::Reverse => REVERSE_NAME,
        }
    }

    /// On a node: a line with interpolated nodes, or today's link for a short one. On
    /// empty map, with the end node option: a line ending at a new node at the (snapped)
    /// pointer.
    fn on_click(&mut self, ctx: &ToolContext) -> Option<Transaction> {
        self.from_new_end = false;
        let Some(start) = self.start else {
            let id = ctx.hovered_node?;
            let n = ctx.net.node(id)?;
            self.start = Some(Start { id, x: n.x, z: n.z });
            return None;
        };
        let from = (start.x, start.z);
        if let Some(id) = ctx.hovered_node {
            let n = ctx.net.node(id)?;
            if start.id == id {
                self.start = None;
                return None;
            }
            let points = line_points(from, (n.x, n.z), self.line.node_distance, true);
            let tx = if points.is_empty() {
                plan(ctx.net, start.id, id, self.shape)
            } else {
                plan_line(
                    ctx.net,
                    ctx.map,
                    start.id,
                    &points,
                    Some(id),
                    self.shape,
                    NodeFlag::REGULAR,
                )
            };
            self.start = self.continuous.then_some(Start { id, x: n.x, z: n.z });
            return tx;
        }
        if !self.line.end_node {
            return None;
        }
        let end = if ctx.snap {
            snap_to_grid((ctx.x, ctx.z), ctx.map.grid)
        } else {
            (ctx.x, ctx.z)
        };
        let points = line_points(from, end, self.line.node_distance, false);
        let tx = plan_line(
            ctx.net,
            ctx.map,
            start.id,
            &points,
            None,
            self.shape,
            NodeFlag::REGULAR,
        );
        self.start = None;
        self.from_new_end = self.continuous && tx.is_some();
        tx
    }

    /// After a line to a new end node, with continuous on, that node starts the next line.
    fn committed(&mut self, tx: &Transaction) {
        if !std::mem::take(&mut self.from_new_end) {
            return;
        }
        if let Some(Command::PasteNodes(paste)) = tx.0.first()
            && let (Some(&id), Some(&(x, _, z, _))) = (paste.ids().last(), paste.nodes.last())
        {
            self.start = Some(Start { id, x, z });
        }
    }

    fn set_continuous(&mut self, on: bool) {
        self.continuous = on;
    }

    /// Draws a ring on the start node and the pending line to the hovered node, or else to
    /// the (snapped) cursor, with a dot on each point the line would add.
    fn paint(
        &self,
        painter: &egui::Painter,
        t: &ViewTransform,
        geometry: MapGeometry,
        cursor: Option<egui::Pos2>,
        snap_grid: Option<(f64, f64)>,
        hovered_node: Option<(f64, f64)>,
        _hovered_edge: Option<(egui::Pos2, egui::Pos2)>,
    ) {
        let Some(start) = self.start else {
            return;
        };
        let screen = |(x, z): (f64, f64)| {
            let (mx, my) = geometry.to_map(x, z);
            let (sx, sy) = t.map_to_screen(mx, my);
            egui::pos2(sx as f32, sy as f32)
        };
        let from = screen((start.x, start.z));
        let colour = match self.shape {
            ConnectionShape::OneWay => look::REGULAR,
            ConnectionShape::Dual => look::DUAL,
            ConnectionShape::Reverse => look::REVERSE,
        };
        let stroke = egui::Stroke::new(STROKE_PX, colour);
        painter.circle_stroke(from, RING_RADIUS_PX, stroke);
        let end = match (hovered_node, cursor) {
            (Some(node), _) => node,
            (None, Some(c)) => {
                let (mx, my) = t.screen_to_map(f64::from(c.x), f64::from(c.y));
                let w = geometry.to_world(mx, my);
                snap_grid.map_or(w, |grid| snap_to_grid(w, grid))
            }
            (None, None) => return,
        };
        let points = line_points(
            (start.x, start.z),
            end,
            self.line.node_distance,
            hovered_node.is_some(),
        );
        let mut prev = from;
        for &(x, z, _) in &points {
            let p = screen((x, z));
            painter.line_segment([prev, p], stroke);
            painter.circle_filled(p, POINT_RADIUS_PX, colour);
            prev = p;
        }
        if hovered_node.is_some() {
            painter.line_segment([prev, screen(end)], stroke);
        }
    }

    fn in_progress(&self) -> bool {
        self.start.is_some()
    }

    fn cancel(&mut self) {
        self.start = None;
        self.from_new_end = false;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::collections::BTreeSet;
    use std::sync::Arc;

    use ad_core::heightmap::Heightmap;
    use ad_core::network::ConnectionKind;

    use super::*;

    fn setup() -> (Network, [NodeId; 3]) {
        let mut net = Network::new();
        let a = net.add_node(0.0, 0.0, 0.0, NodeFlag::REGULAR);
        let b = net.add_node(10.0, 0.0, 0.0, NodeFlag::REGULAR);
        let c = net.add_node(20.0, 0.0, 0.0, NodeFlag::REGULAR);
        (net, [a, b, c])
    }

    /// The links between `a` and `b` as (a.out∋b, a.in∋b, b.out∋a, b.in∋a).
    fn links(net: &Network, a: NodeId, b: NodeId) -> (bool, bool, bool, bool) {
        let (na, nb) = (net.node(a).expect("a"), net.node(b).expect("b"));
        (
            na.outgoing().contains(&b),
            na.incoming().contains(&b),
            nb.outgoing().contains(&a),
            nb.incoming().contains(&a),
        )
    }

    const LINE: LineOptions = LineOptions {
        node_distance: 12.0,
        end_node: false,
    };
    const END_NODE: LineOptions = LineOptions {
        node_distance: 12.0,
        end_node: true,
    };

    fn no_heightmap() -> MapContext {
        MapContext::new(None, 1, 2.0, (2.0, 2.0))
    }

    /// Clicks with `hovered` under the cursor and applies what the tool returns.
    fn click(tool: &mut ConnectTool, net: &mut Network, hovered: Option<NodeId>) -> bool {
        click_at(tool, net, &no_heightmap(), hovered, (0.0, 0.0), false).is_some()
    }

    /// Clicks at world `(x, z)` with `hovered` under the cursor, applies what the tool
    /// returns, tells the tool and returns the applied transaction.
    fn click_at(
        tool: &mut ConnectTool,
        net: &mut Network,
        map: &MapContext,
        hovered: Option<NodeId>,
        (x, z): (f64, f64),
        snap: bool,
    ) -> Option<Transaction> {
        let mut tx = tool.on_click(&ToolContext {
            x,
            z,
            map,
            net,
            selection: &BTreeSet::new(),
            snap,
            hovered_node: hovered,
            hovered_edge: None,
        })?;
        tx.apply(net).expect("applies");
        tool.committed(&tx);
        Some(tx)
    }

    /// Start (0, 0) at y 10 and end (60, 0) at y 20.
    fn long() -> (Network, NodeId, NodeId) {
        let mut net = Network::new();
        let a = net.add_node(0.0, 10.0, 0.0, NodeFlag::REGULAR);
        let b = net.add_node(60.0, 20.0, 0.0, NodeFlag::REGULAR);
        (net, a, b)
    }

    /// The nodes after `skip` in order, as (x, y, z).
    fn added(net: &Network, skip: usize) -> Vec<(f64, f64, f64)> {
        net.order()[skip..]
            .iter()
            .map(|id| {
                let n = net.node(*id).expect("node");
                (n.x, n.y, n.z)
            })
            .collect()
    }

    #[test]
    fn line_points_follow_java() {
        // 60 / 12: 5 segments, 4 inner points, then the end unless it is a node.
        let inner = [(12.0, 0.2), (24.0, 0.4), (36.0, 0.6), (48.0, 0.8)];
        let points = line_points((0.0, 0.0), (60.0, 0.0), 12.0, true);
        assert_eq!(points.len(), 4);
        for (&(x, z, t), (ex, et)) in points.iter().zip(inner) {
            assert!((x - ex).abs() < 1e-9 && z == 0.0 && (t - et).abs() < 1e-12);
        }
        let points = line_points((0.0, 0.0), (60.0, 0.0), 12.0, false);
        assert_eq!(points.len(), 5);
        assert_eq!(points[4], (60.0, 0.0, 1.0));
        // round(15 / 12) = 1, round(5 / 12) = 0: no inner points.
        assert!(line_points((0.0, 0.0), (15.0, 0.0), 12.0, true).is_empty());
        assert_eq!(
            line_points((0.0, 0.0), (3.0, 4.0), 12.0, false),
            [(3.0, 4.0, 1.0)]
        );
        // round(17 / 12) = 1, round(19 / 12) = 2.
        assert!(line_points((0.0, 0.0), (17.0, 0.0), 12.0, true).is_empty());
        assert_eq!(line_points((0.0, 0.0), (19.0, 0.0), 12.0, true).len(), 1);
    }

    #[test]
    fn a_long_line_gets_nodes_linked_with_the_tools_shape_in_one_transaction() {
        let cases = [
            (ConnectionShape::OneWay, (true, false, false, true)),
            (ConnectionShape::Dual, (true, true, true, true)),
            (ConnectionShape::Reverse, (true, false, false, false)),
        ];
        let map = no_heightmap();
        for (shape, expected) in cases {
            let (mut net, a, b) = long();
            let before = format!("{:?}", added(&net, 0));
            let mut tool = ConnectTool::new(shape, false, LINE);
            assert!(click_at(&mut tool, &mut net, &map, Some(a), (0.0, 0.0), false).is_none());
            let mut tx =
                click_at(&mut tool, &mut net, &map, Some(b), (60.0, 0.0), false).expect("a line");
            assert!(matches!(tx.0[..], [Command::PasteNodes(_)]), "{shape:?}");
            assert_eq!(net.len(), 6, "{shape:?}");
            // Without a heightmap y runs from 10 to 20 by point index.
            let nodes = added(&net, 2);
            for (&(x, y, z), (ex, ey)) in
                nodes
                    .iter()
                    .zip([(12.0, 12.0), (24.0, 14.0), (36.0, 16.0), (48.0, 18.0)])
            {
                assert!((x - ex).abs() < 1e-9 && (y - ey).abs() < 1e-9 && z == 0.0);
            }
            let chain: Vec<NodeId> = std::iter::once(a)
                .chain(net.order()[2..].iter().copied())
                .chain([b])
                .collect();
            for pair in chain.windows(2) {
                assert_eq!(links(&net, pair[0], pair[1]), expected, "{shape:?}");
            }
            assert!(
                net.order()[2..]
                    .iter()
                    .all(|id| { net.node(*id).expect("node").flag == NodeFlag::REGULAR })
            );
            tx.revert(&mut net).expect("reverts");
            assert_eq!(format!("{:?}", added(&net, 0)), before, "{shape:?}");
            assert_eq!(links(&net, a, b), (false, false, false, false));
        }
    }

    #[test]
    fn a_short_line_links_as_before() {
        let mut net = Network::new();
        let a = net.add_node(0.0, 0.0, 0.0, NodeFlag::REGULAR);
        let b = net.add_node(15.0, 0.0, 0.0, NodeFlag::REGULAR);
        let map = no_heightmap();
        let mut tool = ConnectTool::new(ConnectionShape::OneWay, false, LINE);
        click_at(&mut tool, &mut net, &map, Some(a), (0.0, 0.0), false);
        let tx = click_at(&mut tool, &mut net, &map, Some(b), (15.0, 0.0), false).expect("tx");
        assert!(matches!(tx.0[..], [Command::Connect(_)]));
        assert_eq!(net.len(), 2);
        assert_eq!(net.connection_kind(a, b), ConnectionKind::Regular);
        // The same click again removes the link, as today.
        click_at(&mut tool, &mut net, &map, Some(a), (0.0, 0.0), false);
        click_at(&mut tool, &mut net, &map, Some(b), (15.0, 0.0), false);
        assert_eq!(net.connection_kind(a, b), ConnectionKind::None);
    }

    #[test]
    fn with_a_heightmap_each_new_node_is_sampled() {
        let mut map = no_heightmap();
        // Every sample is 10 m (raw 2560) at height map scale 2: 5.0.
        map.heightmap = Some(Arc::new(Heightmap::new(2, 2, vec![2560; 4]).expect("size")));
        map.height_map_scale = 2.0;
        let (mut net, a, b) = long();
        let mut tool = ConnectTool::new(ConnectionShape::OneWay, false, LINE);
        click_at(&mut tool, &mut net, &map, Some(a), (0.0, 0.0), false);
        click_at(&mut tool, &mut net, &map, Some(b), (60.0, 0.0), false);
        let ys: Vec<f64> = added(&net, 2).iter().map(|n| n.1).collect();
        assert_eq!(ys, [5.0; 4]);
    }

    #[test]
    fn with_the_end_node_option_an_empty_click_ends_the_line_at_a_new_node() {
        let map = no_heightmap();
        // 60 away, snapped from (59.3, 0.6) to (60, 0): 4 inner nodes and the end.
        let (mut net, a, _) = long();
        let mut tool = ConnectTool::new(ConnectionShape::Dual, false, END_NODE);
        click_at(&mut tool, &mut net, &map, Some(a), (0.0, 0.0), false);
        let tx = click_at(&mut tool, &mut net, &map, None, (59.3, 0.6), true).expect("a line");
        assert_eq!(tx.0.len(), 1);
        let nodes = added(&net, 2);
        assert_eq!(nodes.len(), 5);
        assert_eq!(
            nodes[4],
            (60.0, 10.0, 0.0),
            "at the snapped cursor, start y"
        );
        assert!(nodes.iter().all(|n| n.1 == 10.0));
        let chain: Vec<NodeId> = std::iter::once(a)
            .chain(net.order()[2..].iter().copied())
            .collect();
        for pair in chain.windows(2) {
            assert_eq!(net.connection_kind(pair[0], pair[1]), ConnectionKind::Dual);
        }
        assert_eq!(tool.start(), None, "not continuous");
        // 5 away: one new node at the cursor, linked from the start.
        let (mut net, a, _) = long();
        click_at(&mut tool, &mut net, &map, Some(a), (0.0, 0.0), false);
        click_at(&mut tool, &mut net, &map, None, (3.0, 4.0), false).expect("a node");
        let id = net.order()[2];
        assert_eq!(added(&net, 2), [(3.0, 10.0, 4.0)]);
        assert_eq!(net.connection_kind(a, id), ConnectionKind::Dual);
    }

    #[test]
    fn without_the_end_node_option_an_empty_click_does_nothing() {
        let (mut net, a, _) = long();
        let map = no_heightmap();
        let mut tool = ConnectTool::new(ConnectionShape::OneWay, true, LINE);
        click_at(&mut tool, &mut net, &map, Some(a), (0.0, 0.0), false);
        assert!(click_at(&mut tool, &mut net, &map, None, (60.0, 0.0), false).is_none());
        assert_eq!(net.len(), 2);
        assert_eq!(tool.start(), Some(a));
    }

    #[test]
    fn a_node_under_an_interpolated_point_is_not_reused() {
        let (mut net, a, b) = long();
        let c = net.add_node(24.0, 0.0, 0.0, NodeFlag::REGULAR);
        let d = net.add_node(24.0, 0.0, 30.0, NodeFlag::REGULAR);
        net.connect(c, d, ConnectionShape::OneWay).expect("ok");
        let map = no_heightmap();
        let mut tool = ConnectTool::new(ConnectionShape::OneWay, false, LINE);
        click_at(&mut tool, &mut net, &map, Some(a), (0.0, 0.0), false);
        click_at(&mut tool, &mut net, &map, Some(b), (60.0, 0.0), false);
        assert_eq!(net.len(), 8, "4 new nodes besides c and d");
        let nc = net.node(c).expect("c");
        assert_eq!((nc.outgoing(), nc.incoming()), (&[d][..], &[][..]));
        assert_eq!(net.node(d).expect("d").incoming(), [c]);
    }

    #[test]
    fn continuous_starts_the_next_line_at_its_end() {
        let map = no_heightmap();
        // At an end node.
        let (mut net, a, b) = long();
        let mut tool = ConnectTool::new(ConnectionShape::OneWay, true, END_NODE);
        click_at(&mut tool, &mut net, &map, Some(a), (0.0, 0.0), false);
        click_at(&mut tool, &mut net, &map, Some(b), (60.0, 0.0), false);
        assert_eq!((tool.start(), net.len()), (Some(b), 6));
        // At a new end node, once the line is committed: 36 units, 3 new nodes each.
        click_at(&mut tool, &mut net, &map, None, (60.0, 36.0), false);
        assert_eq!(net.len(), 9);
        let end = net.order()[8];
        assert_eq!(tool.start(), Some(end));
        click_at(&mut tool, &mut net, &map, None, (96.0, 36.0), false);
        assert_eq!(net.len(), 12);
        assert_eq!(
            net.connection_kind(end, net.order()[9]),
            ConnectionKind::Regular
        );
    }

    #[test]
    fn cancel_drops_a_line_waiting_for_its_commit() {
        let (mut net, a, _) = long();
        let mut tool = ConnectTool::new(ConnectionShape::OneWay, true, END_NODE);
        let map = no_heightmap();
        click_at(&mut tool, &mut net, &map, Some(a), (0.0, 0.0), false);
        let mut tx = tool
            .on_click(&ToolContext {
                x: 60.0,
                z: 0.0,
                map: &map,
                net: &net,
                selection: &BTreeSet::new(),
                snap: false,
                hovered_node: None,
                hovered_edge: None,
            })
            .expect("a line");
        tool.cancel();
        tx.apply(&mut net).expect("applies");
        tool.committed(&tx);
        assert_eq!(tool.start(), None);
    }

    fn apply(net: &mut Network, a: NodeId, b: NodeId, shape: ConnectionShape) -> Transaction {
        let mut tx = plan(net, a, b, shape).expect("a transaction");
        tx.apply(net).expect("applies");
        tx
    }

    #[test]
    fn unlinked_nodes_get_the_tools_shape() {
        let cases = [
            (ConnectionShape::OneWay, (true, false, false, true)),
            (ConnectionShape::Dual, (true, true, true, true)),
            (ConnectionShape::Reverse, (true, false, false, false)),
        ];
        for (shape, expected) in cases {
            let (mut net, [a, b, _]) = setup();
            let tx = apply(&mut net, a, b, shape);
            assert_eq!(tx.0.len(), 1, "{shape:?}");
            assert_eq!(links(&net, a, b), expected, "{shape:?}");
        }
    }

    #[test]
    fn exactly_the_tools_shape_is_removed() {
        for shape in [
            ConnectionShape::OneWay,
            ConnectionShape::Dual,
            ConnectionShape::Reverse,
        ] {
            let (mut net, [a, b, _]) = setup();
            net.connect(a, b, shape).expect("ok");
            let tx = apply(&mut net, a, b, shape);
            assert!(matches!(tx.0[..], [Command::Disconnect(_)]), "{shape:?}");
            assert_eq!(links(&net, a, b), (false, false, false, false), "{shape:?}");
        }
    }

    #[test]
    fn the_same_shape_the_other_way_is_reshaped_not_removed() {
        // b -> a one-way, regular tool from a to b: a -> b afterwards.
        let (mut net, [a, b, _]) = setup();
        net.connect(b, a, ConnectionShape::OneWay).expect("ok");
        let tx = apply(&mut net, a, b, ConnectionShape::OneWay);
        assert_eq!(tx.0.len(), 2);
        assert_eq!(links(&net, a, b), (true, false, false, true));
    }

    #[test]
    fn other_links_are_reshaped_in_one_transaction_and_one_revert_restores_them() {
        let (mut net, [a, b, _]) = setup();
        net.connect(a, b, ConnectionShape::OneWay).expect("ok");
        let mut tx = apply(&mut net, a, b, ConnectionShape::Dual);
        assert!(matches!(
            tx.0[..],
            [Command::Disconnect(_), Command::Connect(_)]
        ));
        assert_eq!(net.connection_kind(a, b), ConnectionKind::Dual);
        tx.revert(&mut net).expect("reverts");
        assert_eq!(links(&net, a, b), (true, false, false, true));
        // A dual link with the reverse tool: a -> b reverse only.
        net.connect(b, a, ConnectionShape::OneWay).expect("ok");
        apply(&mut net, a, b, ConnectionShape::Reverse);
        assert_eq!(links(&net, a, b), (true, false, false, false));
    }

    #[test]
    fn plan_needs_two_existing_distinct_nodes() {
        let (mut net, [a, b, _]) = setup();
        assert!(plan(&net, a, a, ConnectionShape::OneWay).is_none());
        net.remove_node(b).expect("removes");
        assert!(plan(&net, a, b, ConnectionShape::OneWay).is_none());
    }

    #[test]
    fn two_clicks_connect_and_a_second_click_on_the_start_cancels() {
        let (mut net, [a, b, _]) = setup();
        let mut tool = ConnectTool::new(ConnectionShape::OneWay, false, LINE);
        assert!(!click(&mut tool, &mut net, Some(a)));
        assert_eq!(tool.start(), Some(a));
        assert!(
            !click(&mut tool, &mut net, Some(a)),
            "cancel commits nothing"
        );
        assert_eq!(tool.start(), None);
        assert!(!click(&mut tool, &mut net, Some(a)));
        assert!(click(&mut tool, &mut net, Some(b)));
        assert_eq!(net.connection_kind(a, b), ConnectionKind::Regular);
        assert_eq!(tool.start(), None, "not continuous");
    }

    #[test]
    fn an_empty_click_keeps_the_pending_start() {
        let (mut net, [a, _, _]) = setup();
        let mut tool = ConnectTool::new(ConnectionShape::Dual, false, LINE);
        assert!(!click(&mut tool, &mut net, None));
        assert_eq!(tool.start(), None);
        click(&mut tool, &mut net, Some(a));
        assert!(!click(&mut tool, &mut net, None));
        assert_eq!(tool.start(), Some(a));
    }

    #[test]
    fn continuous_makes_the_end_node_the_next_start() {
        let (mut net, [a, b, c]) = setup();
        let mut tool = ConnectTool::new(ConnectionShape::OneWay, false, LINE);
        tool.set_continuous(true);
        click(&mut tool, &mut net, Some(a));
        assert!(click(&mut tool, &mut net, Some(b)));
        assert_eq!(tool.start(), Some(b));
        assert!(click(&mut tool, &mut net, Some(c)));
        assert_eq!(net.connection_kind(a, b), ConnectionKind::Regular);
        assert_eq!(net.connection_kind(b, c), ConnectionKind::Regular);
        assert_eq!(tool.start(), Some(c));
    }

    #[test]
    fn cancel_clears_the_start() {
        let (mut net, [a, _, _]) = setup();
        let mut tool = ConnectTool::new(ConnectionShape::Reverse, false, LINE);
        click(&mut tool, &mut net, Some(a));
        tool.cancel();
        assert_eq!(tool.start(), None);
    }

    #[test]
    fn the_shortcut_cycles_regular_dual_reverse() {
        let first = cycled(None, true, LINE);
        assert_eq!(first.shape, ConnectionShape::OneWay);
        assert!(first.continuous);
        let second = cycled(Some(&first), false, LINE);
        assert_eq!(second.shape, ConnectionShape::Dual);
        let third = cycled(Some(&second), false, LINE);
        assert_eq!(third.shape, ConnectionShape::Reverse);
        assert_eq!(
            cycled(Some(&third), false, LINE).shape,
            ConnectionShape::OneWay
        );
    }
}
