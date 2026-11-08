//! The Bézier curve tool (Java `BezierCurveButton`, `BezierCurve`, `CurvePanel`): a click on
//! a start node, then on an end node, previews a curve between them. The curve panel sets
//! it up and commits it as one `PasteNodes` (AD-4, AD-7, AD-10); dragging a control point's
//! handle bends the preview. One curve at a time; nothing commits implicitly.

use std::cell::Cell;
use std::collections::BTreeSet;

use eframe::egui;

use ad_core::command::Transaction;
use ad_core::geometry::MapGeometry;
use ad_core::map_context::MapContext;
use ad_core::network::{ConnectionKind, ConnectionShape, Network, NodeFlag, NodeId};

use super::connect::plan_line;
use super::{Tool, ToolContext};
use crate::look;
use crate::view::ViewTransform;

pub const NAME: &str = "Bézier";

/// Interpolation points a curve may have, and the default (Java `curvePanel` limits).
pub const MAX_POINTS: usize = 50;
pub const DEFAULT_POINTS: usize = 6;
/// Control points a curve may use (quadratic to quintic), and the default.
pub const MAX_CONTROLS: usize = 4;
pub const DEFAULT_CONTROLS: usize = 1;
/// A handle drag moves its control point this many times as far (Java's default
/// `controlPointMoveScaler`).
pub const MOVE_SCALER: f64 = 3.0;

/// A press this close to a handle, in screen px, grabs it.
const HANDLE_HIT_PX: f64 = 8.0;
/// The smallest radius a handle is drawn with, in screen px.
const HANDLE_MIN_PX: f32 = 4.0;
/// Stroke width of the pending line and the preview segments, in screen px.
const STROKE_PX: f32 = 2.0;
/// Radius of the dot on each previewed point, in screen px.
const POINT_RADIUS_PX: f32 = 3.0;

/// The point at `t` of the Bézier curve over `points` (start, control points, end), from
/// the Bernstein polynomials of degree `points.len() - 1`. Java writes out degree 2 to 5.
pub fn bernstein(points: &[(f64, f64)], t: f64) -> (f64, f64) {
    let n = points.len().saturating_sub(1);
    let mut binomial = 1.0;
    let (mut x, mut z) = (0.0, 0.0);
    for (i, &(px, pz)) in points.iter().enumerate() {
        let b = binomial * t.powi(i as i32) * (1.0 - t).powi((n - i) as i32);
        x += b * px;
        z += b * pz;
        binomial = binomial * (n - i) as f64 / (i + 1) as f64;
    }
    (x, z)
}

/// The `n` points of the curve from `start` over `controls` to `end`, each as (x, z, t)
/// at `t = i / (n + 1)` for `i = 1..=n`; the end points are left out (Java
/// `generateInterpolationPoints`).
pub fn curve_points(
    start: (f64, f64),
    controls: &[(f64, f64)],
    end: (f64, f64),
    n: usize,
) -> Vec<(f64, f64, f64)> {
    let polygon: Vec<(f64, f64)> = std::iter::once(start)
        .chain(controls.iter().copied())
        .chain(std::iter::once(end))
        .collect();
    (1..=n)
        .map(|i| {
            let t = i as f64 / (n + 1) as f64;
            let (x, z) = bernstein(&polygon, t);
            (x, z, t)
        })
        .collect()
}

/// A previewed curve and its panel settings. All four control points exist; the first
/// `controls` are used and shown.
#[derive(Debug, Clone, PartialEq)]
pub struct Curve {
    pub start: NodeId,
    pub end: NodeId,
    /// The handles the user drags (Java's control nodes), world (x, z).
    handles: [(f64, f64); MAX_CONTROLS],
    /// The control points the curve is computed from (Java's virtual control points).
    controls_at: [(f64, f64); MAX_CONTROLS],
    /// Interpolation points, 1..=[`MAX_POINTS`].
    pub points: usize,
    /// Control points used, 1..=[`MAX_CONTROLS`].
    pub controls: usize,
    /// The new nodes' flag (the panel's priority).
    pub flag: NodeFlag,
    /// The links' shape (the panel's connection type).
    pub shape: ConnectionShape,
}

impl Curve {
    /// A curve from `start` at `s` to `end` at `e` (world x, z) with the default settings;
    /// every handle and control point sits at the midpoint (Java `initControlPoints`).
    pub fn new(start: NodeId, s: (f64, f64), end: NodeId, e: (f64, f64)) -> Self {
        let mid = ((s.0 + e.0) / 2.0, (s.1 + e.1) / 2.0);
        Self {
            start,
            end,
            handles: [mid; MAX_CONTROLS],
            controls_at: [mid; MAX_CONTROLS],
            points: DEFAULT_POINTS,
            controls: DEFAULT_CONTROLS,
            flag: NodeFlag::REGULAR,
            shape: ConnectionShape::OneWay,
        }
    }

    fn used(&self) -> usize {
        self.controls.clamp(1, MAX_CONTROLS)
    }

    /// The used handles, world (x, z).
    pub fn handles(&self) -> &[(f64, f64)] {
        &self.handles[..self.used()]
    }

    /// The used control points, world (x, z).
    pub fn control_points(&self) -> &[(f64, f64)] {
        &self.controls_at[..self.used()]
    }

    /// Moves handle `i` by `(dx, dz)` and its control point by [`MOVE_SCALER`] times that
    /// (Java `moveControlNodeBy` with scaling).
    pub fn move_handle(&mut self, i: usize, (dx, dz): (f64, f64)) {
        let (Some(h), Some(c)) = (self.handles.get_mut(i), self.controls_at.get_mut(i)) else {
            return;
        };
        *h = (h.0 + dx, h.1 + dz);
        *c = (c.0 + dx * MOVE_SCALER, c.1 + dz * MOVE_SCALER);
    }

    /// Swaps start and end and reverses the used handles and control points, so the shape
    /// stays (Java `swapCurveDirection`).
    pub fn swap(&mut self) {
        std::mem::swap(&mut self.start, &mut self.end);
        let used = self.used();
        self.handles[..used].reverse();
        self.controls_at[..used].reverse();
    }

    /// The curve's points between the start at `s` and the end at `e` (world x, z).
    pub fn points_between(&self, s: (f64, f64), e: (f64, f64)) -> Vec<(f64, f64, f64)> {
        let n = self.points.clamp(1, MAX_POINTS);
        curve_points(s, self.control_points(), e, n)
    }
}

/// The transaction committing `curve`: its points as new nodes with the curve's flag,
/// linked start → nodes → end with its shape. Heights are sampled, or without a heightmap
/// run linearly by `t` from the start's y to the end's y (AD-7). `None` for a missing node.
pub fn plan(net: &Network, map: &MapContext, curve: &Curve) -> Option<Transaction> {
    let (s, e) = (net.node(curve.start)?, net.node(curve.end)?);
    let points = curve.points_between((s.x, s.z), (e.x, e.z));
    plan_line(
        net,
        map,
        curve.start,
        &points,
        Some(curve.end),
        curve.shape,
        curve.flag,
    )
}

/// The start node of a pending curve and its world (x, z) when it was picked.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Start {
    id: NodeId,
    x: f64,
    z: f64,
}

/// A running handle drag: the handle and the pointer's last world (x, z).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Drag {
    handle: usize,
    last: (f64, f64),
}

#[derive(Default)]
pub struct BezierTool {
    start: Option<Start>,
    curve: Option<Curve>,
    drag: Option<Drag>,
    /// World units per screen px of the last painted view; a press is tested against it.
    world_per_px: Cell<Option<f64>>,
}

impl BezierTool {
    /// Remembers the view the handles are drawn in; [`Tool::paint_selection`] calls it every
    /// frame. A handle drag can start only after it.
    pub fn see(&self, t: &ViewTransform, geometry: MapGeometry) {
        self.world_per_px.set(Some(geometry.scale / t.zoom));
    }

    /// The previewed curve, if any.
    #[cfg(test)]
    pub fn curve(&self) -> Option<&Curve> {
        self.curve.as_ref()
    }

    /// The start node of the pending curve, if any.
    #[cfg(test)]
    pub fn start(&self) -> Option<NodeId> {
        self.start.map(|s| s.id)
    }
}

/// The connection kind drawn for a curve link of `shape`.
fn kind(shape: ConnectionShape) -> ConnectionKind {
    match shape {
        ConnectionShape::OneWay => ConnectionKind::Regular,
        ConnectionShape::Dual => ConnectionKind::Dual,
        ConnectionShape::Reverse => ConnectionKind::Reverse,
    }
}

impl Tool for BezierTool {
    fn name(&self) -> &'static str {
        NAME
    }

    /// Without a curve: picks the start node, then on another node creates the curve; a
    /// click on the start again cancels it. While a curve is previewed a click does nothing.
    fn on_click(&mut self, ctx: &ToolContext) -> Option<Transaction> {
        if self.curve.is_some() {
            return None;
        }
        let id = ctx.hovered_node?;
        let n = ctx.net.node(id)?;
        match self.start {
            None => self.start = Some(Start { id, x: n.x, z: n.z }),
            Some(start) if start.id == id => self.start = None,
            Some(start) => {
                self.curve = Some(Curve::new(start.id, (start.x, start.z), id, (n.x, n.z)));
                self.start = None;
            }
        }
        None
    }

    /// A right click cancels the pending start; a previewed curve stays.
    fn on_secondary_click(&mut self) {
        self.start = None;
    }

    /// Claims only a press within [`HANDLE_HIT_PX`] of a used handle (the nearest one);
    /// any other press pans.
    fn on_drag_start(&mut self, ctx: &ToolContext, _hovered: Option<NodeId>) -> bool {
        let (Some(curve), Some(world_per_px)) = (&self.curve, self.world_per_px.get()) else {
            return false;
        };
        let reach = HANDLE_HIT_PX * world_per_px;
        let nearest = curve
            .handles()
            .iter()
            .map(|&(hx, hz)| (ctx.x - hx).hypot(ctx.z - hz))
            .enumerate()
            .filter(|&(_, d)| d <= reach)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((handle, _)) = nearest else {
            return false;
        };
        self.drag = Some(Drag {
            handle,
            last: (ctx.x, ctx.z),
        });
        true
    }

    /// Moves the grabbed handle by the pointer's movement; only the preview changes.
    fn on_drag(&mut self, ctx: &ToolContext) {
        let (Some(d), Some(curve)) = (&mut self.drag, &mut self.curve) else {
            return;
        };
        curve.move_handle(d.handle, (ctx.x - d.last.0, ctx.z - d.last.1));
        d.last = (ctx.x, ctx.z);
    }

    /// A handle drag commits nothing: it changes only the preview.
    fn on_drag_end(&mut self, ctx: &ToolContext) -> Option<Transaction> {
        self.on_drag(ctx);
        self.drag = None;
        None
    }

    /// A lost pointer ends the handle drag only; the curve keeps its shape so far.
    fn on_drag_cancel(&mut self) {
        self.drag = None;
    }

    fn curve_mut(&mut self) -> Option<&mut Curve> {
        self.curve.as_mut()
    }

    /// Draws the previewed curve: its segments in the shape's colour, a dot on each point in
    /// the priority's node colour, and the used handles in white.
    fn paint_selection(
        &self,
        painter: &egui::Painter,
        t: &ViewTransform,
        map: &MapContext,
        net: &Network,
        _selection: &BTreeSet<NodeId>,
        _hovered: Option<NodeId>,
        _revision: u64,
    ) {
        self.see(t, map.geometry);
        let Some(curve) = &self.curve else {
            return;
        };
        let (Some(s), Some(e)) = (net.node(curve.start), net.node(curve.end)) else {
            return;
        };
        let screen = |(x, z): (f64, f64)| {
            let (mx, my) = map.geometry.to_map(x, z);
            let (sx, sy) = t.map_to_screen(mx, my);
            egui::pos2(sx as f32, sy as f32)
        };
        let colour = look::connection_colour(kind(curve.shape), curve.flag.0, curve.flag.0);
        let stroke = egui::Stroke::new(STROKE_PX, colour);
        let dot = look::node_colour(curve.flag.0);
        let points = curve.points_between((s.x, s.z), (e.x, e.z));
        let mut prev = screen((s.x, s.z));
        for &(x, z, _) in &points {
            let p = screen((x, z));
            painter.line_segment([prev, p], stroke);
            painter.circle_filled(p, POINT_RADIUS_PX, dot);
            prev = p;
        }
        painter.line_segment([prev, screen((e.x, e.z))], stroke);
        let half_node = look::node_px(map.node_size, t.zoom) as f32 / 2.0;
        for &h in curve.handles() {
            painter.circle_filled(
                screen(h),
                half_node.max(HANDLE_MIN_PX),
                egui::Color32::WHITE,
            );
        }
    }

    /// Draws the pending line from the start node to the hovered node or the cursor (white,
    /// as Java).
    fn paint(
        &self,
        painter: &egui::Painter,
        t: &ViewTransform,
        geometry: MapGeometry,
        cursor: Option<egui::Pos2>,
        _snap_grid: Option<(f64, f64)>,
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
        let end = match (hovered_node, cursor) {
            (Some(node), _) => screen(node),
            (None, Some(c)) => c,
            (None, None) => return,
        };
        let stroke = egui::Stroke::new(STROKE_PX, egui::Color32::WHITE);
        painter.line_segment([screen((start.x, start.z)), end], stroke);
    }

    fn cancel(&mut self) {
        self.start = None;
        self.curve = None;
        self.drag = None;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::sync::Arc;

    use ad_core::command::{Command, LinkEnd};
    use ad_core::heightmap::Heightmap;

    use super::*;

    const EPS: f64 = 1e-9;

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < EPS && (a.1 - b.1).abs() < EPS
    }

    fn no_heightmap() -> MapContext {
        MapContext::new(None, 1, 2.0, (2.0, 2.0))
    }

    /// Start a (0, 10, 0) and end b (100, 20, 0), plus c (50, 0, 50) elsewhere.
    fn setup() -> (Network, [NodeId; 3]) {
        let mut net = Network::new();
        let a = net.add_node(0.0, 10.0, 0.0, NodeFlag::REGULAR);
        let b = net.add_node(100.0, 20.0, 0.0, NodeFlag::REGULAR);
        let c = net.add_node(50.0, 0.0, 50.0, NodeFlag::REGULAR);
        (net, [a, b, c])
    }

    fn ctx<'a>(
        net: &'a Network,
        map: &'a MapContext,
        (x, z): (f64, f64),
        hovered: Option<NodeId>,
    ) -> ToolContext<'a> {
        static EMPTY: BTreeSet<NodeId> = BTreeSet::new();
        ToolContext {
            x,
            z,
            map,
            net,
            selection: &EMPTY,
            snap: false,
            hovered_node: hovered,
            hovered_edge: None,
        }
    }

    /// A tool that has seen a view at zoom 4 on a scale-1 map (0.25 world units per px)
    /// and previews a curve from a to b.
    fn previewing(net: &Network, map: &MapContext, [a, b, _]: [NodeId; 3]) -> BezierTool {
        let mut tool = BezierTool::default();
        let mut t = ViewTransform::new((512.0, 512.0), (1024.0, 1024.0));
        t.zoom = 4.0;
        tool.see(&t, MapGeometry::new(1.0));
        assert!(tool.on_click(&ctx(net, map, (0.0, 0.0), Some(a))).is_none());
        assert!(
            tool.on_click(&ctx(net, map, (100.0, 0.0), Some(b)))
                .is_none()
        );
        assert!(tool.curve().is_some());
        tool
    }

    /// Drags handle-grabbing from `from` by `by`, in two moves; asserts nothing commits.
    fn drag(
        tool: &mut BezierTool,
        net: &Network,
        map: &MapContext,
        from: (f64, f64),
        by: (f64, f64),
    ) {
        assert!(tool.on_drag_start(&ctx(net, map, from, None), None));
        let half = (from.0 + by.0 / 2.0, from.1 + by.1 / 2.0);
        tool.on_drag(&ctx(net, map, half, None));
        let end = (from.0 + by.0, from.1 + by.1);
        assert!(tool.on_drag_end(&ctx(net, map, end, None)).is_none());
    }

    /// The new nodes of a committed transaction as (x, y, z, flag), and its links.
    fn committed(tx: &Transaction) -> (Vec<NewNode>, Vec<Link>) {
        let [Command::PasteNodes(p)] = tx.0.as_slice() else {
            panic!("one PasteNodes");
        };
        (p.nodes.clone(), p.links.clone())
    }

    /// A pasted node as (x, y, z, flag) and a pasted link as (from, to, shape).
    type NewNode = (f64, f64, f64, NodeFlag);
    type Link = (LinkEnd, LinkEnd, ConnectionShape);

    #[test]
    fn bernstein_matches_javas_quadratic_to_quintic_formulas() {
        let (s, e) = ((0.0, 0.0), (100.0, 10.0));
        let c = [(20.0, 60.0), (40.0, -30.0), (70.0, 80.0), (90.0, -50.0)];
        for t in [0.0, 0.1, 0.25, 0.5, 0.8, 1.0] {
            let u = 1.0 - t;
            let java = |w: [f64; 6], k: usize| {
                let mut p = (w[0] * s.0, w[0] * s.1);
                for i in 0..k {
                    p = (p.0 + w[i + 1] * c[i].0, p.1 + w[i + 1] * c[i].1);
                }
                (p.0 + w[k + 1] * e.0, p.1 + w[k + 1] * e.1)
            };
            let cases = [
                java([u * u, 2.0 * u * t, t * t, 0.0, 0.0, 0.0], 1),
                java(
                    [
                        u.powi(3),
                        3.0 * u * u * t,
                        3.0 * u * t * t,
                        t.powi(3),
                        0.0,
                        0.0,
                    ],
                    2,
                ),
                java(
                    [
                        u.powi(4),
                        4.0 * u.powi(3) * t,
                        6.0 * u * u * t * t,
                        4.0 * u * t.powi(3),
                        t.powi(4),
                        0.0,
                    ],
                    3,
                ),
                java(
                    [
                        u.powi(5),
                        5.0 * u.powi(4) * t,
                        10.0 * u.powi(3) * t * t,
                        10.0 * u * u * t.powi(3),
                        5.0 * u * t.powi(4),
                        t.powi(5),
                    ],
                    4,
                ),
            ];
            for (k, want) in cases.into_iter().enumerate() {
                let polygon: Vec<_> = std::iter::once(s)
                    .chain(c[..=k].iter().copied())
                    .chain([e])
                    .collect();
                let got = bernstein(&polygon, t);
                assert!(
                    (got.0 - want.0).abs() < 1e-9 && (got.1 - want.1).abs() < 1e-9,
                    "k {} t {t}",
                    k + 1
                );
            }
        }
    }

    #[test]
    fn curve_points_sit_at_i_over_n_plus_one_without_the_ends() {
        let points = curve_points((0.0, 0.0), &[(50.0, 50.0)], (100.0, 0.0), 3);
        let ts: Vec<f64> = points.iter().map(|p| p.2).collect();
        assert_eq!(ts, [0.25, 0.5, 0.75]);
        // Quadratic at t 0.5: 0.25 s + 0.5 c + 0.25 e.
        assert!(close((points[1].0, points[1].1), (50.0, 25.0)));
    }

    #[test]
    fn two_clicks_preview_six_points_on_the_straight_line() {
        let (net, ids) = setup();
        let map = no_heightmap();
        let tool = previewing(&net, &map, ids);
        let curve = tool.curve().expect("curve");
        assert_eq!((curve.start, curve.end), (ids[0], ids[1]));
        assert_eq!(curve.handles(), [(50.0, 0.0)]);
        let points = curve.points_between((0.0, 0.0), (100.0, 0.0));
        assert_eq!(points.len(), 6);
        for (i, &(x, z, t)) in points.iter().enumerate() {
            let want = (i + 1) as f64 / 7.0;
            assert!((t - want).abs() < EPS && (x - 100.0 * want).abs() < EPS && z.abs() < EPS);
        }
        assert_eq!(tool.start(), None);
    }

    #[test]
    fn a_click_on_the_start_or_a_right_click_cancels_the_pending_start() {
        let (net, [a, b, c]) = setup();
        let map = no_heightmap();
        let mut tool = BezierTool::default();
        // An empty click picks nothing.
        tool.on_click(&ctx(&net, &map, (0.0, 0.0), None));
        assert_eq!(tool.start(), None);
        tool.on_click(&ctx(&net, &map, (0.0, 0.0), Some(a)));
        assert_eq!(tool.start(), Some(a));
        tool.on_click(&ctx(&net, &map, (0.0, 0.0), Some(a)));
        assert_eq!(tool.start(), None);
        tool.on_click(&ctx(&net, &map, (0.0, 0.0), Some(a)));
        tool.on_secondary_click();
        assert_eq!(tool.start(), None);
        assert!(tool.curve().is_none());
        // While a curve is previewed a click changes nothing.
        let mut tool = previewing(&net, &map, [a, b, c]);
        let before = tool.curve().cloned();
        tool.on_click(&ctx(&net, &map, (50.0, 50.0), Some(c)));
        tool.on_click(&ctx(&net, &map, (5.0, 5.0), None));
        assert_eq!(tool.curve().cloned(), before);
        assert_eq!(tool.start(), None);
    }

    #[test]
    fn dragging_a_handle_moves_it_and_its_control_point_three_times_as_far() {
        let (net, ids) = setup();
        let map = no_heightmap();
        let mut tool = previewing(&net, &map, ids);
        drag(&mut tool, &net, &map, (50.0, 0.0), (10.0, 0.0));
        let curve = tool.curve().expect("curve");
        assert!(close(curve.handles()[0], (60.0, 0.0)));
        assert!(close(curve.control_points()[0], (80.0, 0.0)));
        // A drag downwards bends the preview: the middle point leaves the line.
        drag(&mut tool, &net, &map, (60.0, 0.0), (0.0, 10.0));
        let curve = tool.curve().expect("curve");
        assert!(close(curve.control_points()[0], (80.0, 30.0)));
        let points = curve.points_between((0.0, 0.0), (100.0, 0.0));
        assert!(points.iter().all(|p| p.1 > 0.0));
        // A lost pointer ends the drag but keeps the curve and its shape.
        assert!(tool.on_drag_start(&ctx(&net, &map, (60.0, 10.0), None), None));
        tool.on_drag_cancel();
        let curve = tool.curve().expect("curve kept");
        assert!(close(curve.control_points()[0], (80.0, 30.0)));
        tool.on_drag(&ctx(&net, &map, (90.0, 90.0), None));
        assert!(close(
            tool.curve().expect("curve").handles()[0],
            (60.0, 10.0)
        ));
    }

    #[test]
    fn a_press_off_the_handles_is_not_claimed() {
        let (net, ids) = setup();
        let map = no_heightmap();
        let mut tool = previewing(&net, &map, ids);
        // 8 px is 2 world units at zoom 4; on a node too.
        assert!(!tool.on_drag_start(&ctx(&net, &map, (52.1, 0.0), None), Some(ids[0])));
        tool.on_drag(&ctx(&net, &map, (90.0, 0.0), None));
        assert_eq!(tool.curve().expect("curve").handles(), [(50.0, 0.0)]);
        assert!(tool.on_drag_start(&ctx(&net, &map, (51.9, 0.0), None), None));
        // Without a curve nothing is claimed.
        let mut idle = BezierTool::default();
        idle.see(
            &ViewTransform::new((0.0, 0.0), (1.0, 1.0)),
            MapGeometry::new(1.0),
        );
        assert!(!idle.on_drag_start(&ctx(&net, &map, (50.0, 0.0), None), None));
    }

    #[test]
    fn hidden_handles_keep_their_place_and_are_not_used() {
        let (net, ids) = setup();
        let map = no_heightmap();
        let mut tool = previewing(&net, &map, ids);
        tool.curve_mut().expect("curve").controls = 3;
        // All handles start at the midpoint: the nearest-first press grabs handle 1, so
        // move handles 1 and 2 away first, then drag handle 3.
        drag(&mut tool, &net, &map, (50.0, 0.0), (0.0, -20.0));
        drag(&mut tool, &net, &map, (50.0, 0.0), (0.0, 20.0));
        drag(&mut tool, &net, &map, (50.0, 0.0), (10.0, 0.0));
        let curve = tool.curve_mut().expect("curve");
        assert_eq!(curve.handles().len(), 3);
        assert!(close(curve.handles()[2], (60.0, 0.0)));
        curve.controls = 1;
        assert_eq!(curve.handles().len(), 1);
        assert!(close(curve.handles()[0], (50.0, -20.0)));
        // Only control point 1 shapes the curve now.
        let one = curve.points_between((0.0, 0.0), (100.0, 0.0));
        let want = curve_points((0.0, 0.0), &[(50.0, -60.0)], (100.0, 0.0), 6);
        assert_eq!(one, want);
        curve.controls = 3;
        assert!(close(curve.handles()[2], (60.0, 0.0)));
        assert!(close(curve.control_points()[2], (80.0, 0.0)));
    }

    #[test]
    fn the_panels_settings_shape_the_commit() {
        let (net, ids) = setup();
        let map = no_heightmap();
        let mut tool = previewing(&net, &map, ids);
        let curve = tool.curve_mut().expect("curve");
        curve.points = 10;
        curve.controls = 2;
        curve.flag = NodeFlag::SUB_PRIO;
        curve.shape = ConnectionShape::Dual;
        let tx = plan(&net, &map, curve).expect("tx");
        let (nodes, links) = committed(&tx);
        assert_eq!(nodes.len(), 10);
        assert!(nodes.iter().all(|n| n.3 == NodeFlag::SUB_PRIO));
        let ends: Vec<LinkEnd> = std::iter::once(LinkEnd::Existing(ids[0]))
            .chain((0..10).map(LinkEnd::New))
            .chain([LinkEnd::Existing(ids[1])])
            .collect();
        let want: Vec<_> = ends
            .windows(2)
            .map(|w| (w[0], w[1], ConnectionShape::Dual))
            .collect();
        assert_eq!(links, want);
    }

    #[test]
    fn one_point_sits_half_way_at_the_mean_height() {
        let (net, ids) = setup();
        let map = no_heightmap();
        let mut tool = previewing(&net, &map, ids);
        let curve = tool.curve_mut().expect("curve");
        curve.points = 1;
        let (nodes, links) = committed(&plan(&net, &map, curve).expect("tx"));
        assert_eq!(nodes, [(50.0, 15.0, 0.0, NodeFlag::REGULAR)]);
        assert_eq!(links.len(), 2);
    }

    #[test]
    fn heights_are_sampled_or_run_linearly_by_t() {
        let (net, ids) = setup();
        let mut map = no_heightmap();
        let mut tool = previewing(&net, &map, ids);
        let curve = tool.curve_mut().expect("curve");
        curve.points = 4;
        let (nodes, _) = committed(&plan(&net, &map, curve).expect("tx"));
        let ys: Vec<f64> = nodes.iter().map(|n| n.1).collect();
        for (y, want) in ys.iter().zip([12.0, 14.0, 16.0, 18.0]) {
            assert!((y - want).abs() < EPS, "{ys:?}");
        }
        // Every sample is 10 m (raw 2560) at height map scale 2: 5.0.
        map.heightmap = Some(Arc::new(Heightmap::new(2, 2, vec![2560; 4]).expect("size")));
        map.height_map_scale = 2.0;
        let (nodes, _) = committed(&plan(&net, &map, curve).expect("tx"));
        assert!(nodes.iter().all(|n| n.1 == 5.0));
    }

    #[test]
    fn swap_reverses_the_links_and_keeps_the_shape() {
        let (net, ids) = setup();
        let map = no_heightmap();
        let mut tool = previewing(&net, &map, ids);
        let curve = tool.curve_mut().expect("curve");
        curve.controls = 2;
        curve.move_handle(0, (-10.0, 10.0));
        curve.move_handle(1, (20.0, -5.0));
        let (before, _) = committed(&plan(&net, &map, curve).expect("tx"));
        curve.swap();
        assert_eq!((curve.start, curve.end), (ids[1], ids[0]));
        let tx = plan(&net, &map, curve).expect("tx");
        let (after, links) = committed(&tx);
        assert_eq!(links[0].0, LinkEnd::Existing(ids[1]));
        assert_eq!(links[links.len() - 1].1, LinkEnd::Existing(ids[0]));
        assert!(links.iter().all(|l| l.2 == ConnectionShape::OneWay));
        assert_eq!(after.len(), before.len());
        for (a, b) in after.iter().zip(before.iter().rev()) {
            assert!(close((a.0, a.2), (b.0, b.2)), "{a:?} {b:?}");
            assert!((a.1 - b.1).abs() < EPS);
        }
        // Hidden handles are not part of the swap.
        curve.controls = 4;
        assert!(close(curve.handles()[2], (50.0, 0.0)));
    }

    #[test]
    fn cancel_drops_the_curve_and_the_pending_start() {
        let (net, ids) = setup();
        let map = no_heightmap();
        let mut tool = previewing(&net, &map, ids);
        tool.cancel();
        assert!(tool.curve().is_none());
        tool.on_click(&ctx(&net, &map, (0.0, 0.0), Some(ids[0])));
        tool.cancel();
        assert_eq!(tool.start(), None);
        // The next curve starts with the defaults.
        let tool = previewing(&net, &map, ids);
        let curve = tool.curve().expect("curve");
        assert_eq!(
            (curve.points, curve.controls, curve.flag, curve.shape),
            (6, 1, NodeFlag::REGULAR, ConnectionShape::OneWay)
        );
    }
}
