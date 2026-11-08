//! The arc tool (Java `ArcSplineButton`, `ArcSpline`, `ArcSplineWidget`): a click on the map
//! places a half circle there; its handles move it, turn its ends and set its radius. The
//! curve panel sets it up and commits it as one standalone `PasteNodes` (AD-4, AD-7,
//! AD-10). One arc at a time; nothing commits implicitly.

use std::cell::Cell;
use std::collections::BTreeSet;
use std::f64::consts::{FRAC_PI_2, TAU};

use eframe::egui;

use ad_core::command::{Command, LinkEnd, PasteNodes, Transaction};
use ad_core::geometry::MapGeometry;
use ad_core::heightmap::sample_height;
use ad_core::map_context::MapContext;
use ad_core::network::{Network, NodeId};

use super::bezier::{self, CurveSettings, MAX_POINTS};
use super::{Tool, ToolContext};
use crate::look;
use crate::view::ViewTransform;

pub const NAME: &str = "Arc";

/// A new arc's radius and angles (Java `initialRadius`, a half circle from -90° to 90°).
pub const DEFAULT_RADIUS: f64 = 8.0;
pub const DEFAULT_START_DEG: f64 = -90.0;
pub const DEFAULT_END_DEG: f64 = 90.0;
/// The smallest radius the radius handle sets (Java `MIN_RADIUS`).
pub const MIN_RADIUS: f64 = 2.0;
/// The height of a committed node without a heightmap (the node tool's "no height").
const NO_HEIGHT: f64 = -1.0;

/// A press this close to a handle, in screen px, grabs it.
const HANDLE_HIT_PX: f64 = 8.0;
/// The smallest radius a handle is drawn with, in screen px.
const HANDLE_MIN_PX: f32 = 4.0;
/// Stroke width of the preview segments, in screen px.
const STROKE_PX: f32 = 2.0;
/// Radius of the dot on each previewed point, in screen px.
const POINT_RADIUS_PX: f32 = 3.0;

/// The arc's angles in degrees as Java normalises them: the start in [0, 360), the end in
/// [start, start + 360) (an end below the start gets +360).
pub fn span(start_deg: f64, end_deg: f64) -> (f64, f64) {
    let start = start_deg.rem_euclid(360.0);
    let end = end_deg.rem_euclid(360.0);
    (start, if end < start { end + 360.0 } else { end })
}

/// The point at `deg` on the circle of `radius` round `centre`: angle 0 lies at -z,
/// angles turn towards +x (Java's `angle - 90°`).
pub fn on_circle((cx, cz): (f64, f64), radius: f64, deg: f64) -> (f64, f64) {
    let a = (deg - 90.0).to_radians();
    (cx + radius * a.cos(), cz + radius * a.sin())
}

/// `n + 2` points from the start angle to the end angle, evenly in angle, both ends
/// included (Java `generateArcPoints`).
pub fn arc_points(
    centre: (f64, f64),
    radius: f64,
    start_deg: f64,
    end_deg: f64,
    n: usize,
) -> Vec<(f64, f64)> {
    let (start, end) = span(start_deg, end_deg);
    let last = (n + 1) as f64;
    (0..n + 2)
        .map(|i| on_circle(centre, radius, start + (end - start) * i as f64 / last))
        .collect()
}

/// The angle of `p` about `centre` in whole degrees in [0, 360] (Java `rotateStartNode`
/// without rotation snap).
pub fn angle_to((cx, cz): (f64, f64), (px, pz): (f64, f64)) -> f64 {
    let theta = (cz - pz).atan2(cx - px) - FRAC_PI_2;
    let theta = if theta < 0.0 { theta + TAU } else { theta };
    theta.to_degrees().round()
}

/// A handle of the arc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    /// Moves the whole arc.
    Centre,
    /// Turns the start angle.
    Start,
    /// Turns the end angle.
    End,
    /// Sets the radius; sits midway between the centre and the start.
    Radius,
}

const HANDLES: [Handle; 4] = [Handle::Centre, Handle::Start, Handle::End, Handle::Radius];

/// A previewed arc and its panel settings.
#[derive(Debug, Clone, PartialEq)]
pub struct Arc {
    /// World (x, z).
    pub centre: (f64, f64),
    pub radius: f64,
    /// Whole degrees, as set; [`span`] normalises them.
    pub start_deg: f64,
    pub end_deg: f64,
    /// Whether the links run from the end back to the start ("Swap direction").
    pub reversed: bool,
    /// Interpolation points, priority and connection type.
    pub settings: CurveSettings,
}

impl Arc {
    /// A half circle round `centre` with the default radius, angles and settings.
    pub fn new(centre: (f64, f64)) -> Self {
        Self {
            centre,
            radius: DEFAULT_RADIUS,
            start_deg: DEFAULT_START_DEG,
            end_deg: DEFAULT_END_DEG,
            reversed: false,
            settings: CurveSettings::default(),
        }
    }

    /// The arc's points, start to end.
    pub fn points(&self) -> Vec<(f64, f64)> {
        let n = self.settings.points.clamp(1, MAX_POINTS);
        arc_points(self.centre, self.radius, self.start_deg, self.end_deg, n)
    }

    /// Where `handle` sits, world (x, z).
    pub fn handle(&self, handle: Handle) -> (f64, f64) {
        match handle {
            Handle::Centre => self.centre,
            Handle::Start => on_circle(self.centre, self.radius, self.start_deg),
            Handle::End => on_circle(self.centre, self.radius, self.end_deg),
            Handle::Radius => on_circle(self.centre, self.radius / 2.0, self.start_deg),
        }
    }

    /// Moves the whole arc by `(dx, dz)` (Java `moveControlNodeBy` on the centre).
    pub fn move_by(&mut self, (dx, dz): (f64, f64)) {
        self.centre = (self.centre.0 + dx, self.centre.1 + dz);
    }

    /// Moves the radius handle by `(dx, dz)`, kept on the line from the centre to the start
    /// and no farther out than the start; the radius becomes twice its distance from the
    /// centre, at least [`MIN_RADIUS`] (Java `clampNodeToLine`,
    /// `updateArcRadiusFromAdjustNode`).
    pub fn move_radius_handle(&mut self, (dx, dz): (f64, f64)) {
        let (hx, hz) = self.handle(Handle::Radius);
        let (ux, uz) = on_circle((0.0, 0.0), 1.0, self.start_deg);
        let along = (hx + dx - self.centre.0) * ux + (hz + dz - self.centre.1) * uz;
        let half = along.clamp(0.0, self.radius).max(MIN_RADIUS / 2.0);
        self.radius = (half * 2.0).max(MIN_RADIUS);
    }

    /// Reverses the links' direction; the points stay (Java `swapCurveDirection`).
    pub fn swap(&mut self) {
        self.reversed = !self.reversed;
    }
}

/// The transaction committing `arc`: its points as new nodes with the arc's flag, linked in
/// order (last to first when reversed) with its shape, not linked to any existing node.
/// Heights are sampled, or [`NO_HEIGHT`] without a heightmap (AD-7).
pub fn plan(map: &MapContext, arc: &Arc) -> Transaction {
    let flag = arc.settings.flag;
    let nodes: Vec<_> = arc
        .points()
        .into_iter()
        .map(|(x, z)| (x, sample_height(map, x, z).unwrap_or(NO_HEIGHT), z, flag))
        .collect();
    let shape = arc.settings.shape;
    let links = if arc.reversed {
        (1..nodes.len())
            .rev()
            .map(|i| (LinkEnd::New(i), LinkEnd::New(i - 1), shape))
            .collect()
    } else {
        (1..nodes.len())
            .map(|i| (LinkEnd::New(i - 1), LinkEnd::New(i), shape))
            .collect()
    };
    Transaction::new(vec![Command::PasteNodes(PasteNodes::new(
        nodes,
        Vec::new(),
        links,
    ))])
}

/// A running handle drag: the handle and the pointer's last world (x, z).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Drag {
    handle: Handle,
    last: (f64, f64),
}

#[derive(Default)]
pub struct ArcTool {
    arc: Option<Arc>,
    drag: Option<Drag>,
    /// World units per screen px of the last painted view; a press is tested against it.
    world_per_px: Cell<Option<f64>>,
}

impl ArcTool {
    /// Remembers the view the handles are drawn in; [`Tool::paint_selection`] calls it every
    /// frame. A handle drag can start only after it.
    pub fn see(&self, t: &ViewTransform, geometry: MapGeometry) {
        self.world_per_px.set(Some(geometry.scale / t.zoom));
    }

    /// The previewed arc, if any.
    #[cfg(test)]
    pub fn arc(&self) -> Option<&Arc> {
        self.arc.as_ref()
    }
}

impl Tool for ArcTool {
    fn name(&self) -> &'static str {
        NAME
    }

    /// Without an arc: places one centred at the pointer. While an arc is previewed a
    /// click does nothing.
    fn on_click(&mut self, ctx: &ToolContext) -> Option<Transaction> {
        if self.arc.is_none() {
            self.arc = Some(Arc::new((ctx.x, ctx.z)));
        }
        None
    }

    /// Claims only a press within [`HANDLE_HIT_PX`] of a handle (the nearest one); any
    /// other press pans.
    fn on_drag_start(&mut self, ctx: &ToolContext, _hovered: Option<NodeId>) -> bool {
        let (Some(arc), Some(world_per_px)) = (&self.arc, self.world_per_px.get()) else {
            return false;
        };
        let reach = HANDLE_HIT_PX * world_per_px;
        let nearest = HANDLES
            .iter()
            .map(|&h| {
                let (hx, hz) = arc.handle(h);
                (h, (ctx.x - hx).hypot(ctx.z - hz))
            })
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

    /// Applies the pointer's move to the grabbed handle; only the preview changes.
    fn on_drag(&mut self, ctx: &ToolContext) {
        let (Some(d), Some(arc)) = (&mut self.drag, &mut self.arc) else {
            return;
        };
        let to = (ctx.x, ctx.z);
        let by = (to.0 - d.last.0, to.1 - d.last.1);
        match d.handle {
            Handle::Centre => arc.move_by(by),
            Handle::Start => arc.start_deg = angle_to(arc.centre, to),
            Handle::End => arc.end_deg = angle_to(arc.centre, to),
            Handle::Radius => arc.move_radius_handle(by),
        }
        d.last = to;
    }

    /// A handle drag commits nothing: it changes only the preview.
    fn on_drag_end(&mut self, ctx: &ToolContext) -> Option<Transaction> {
        self.on_drag(ctx);
        self.drag = None;
        None
    }

    /// A lost pointer ends the handle drag only; the arc keeps its shape so far.
    fn on_drag_cancel(&mut self) {
        self.drag = None;
    }

    fn arc_mut(&mut self) -> Option<&mut Arc> {
        self.arc.as_mut()
    }

    /// Draws the previewed arc: its segments in the shape's colour, a dot on each point in
    /// the priority's node colour, and the handles in white.
    fn paint_selection(
        &self,
        painter: &egui::Painter,
        t: &ViewTransform,
        map: &MapContext,
        _net: &Network,
        _selection: &BTreeSet<NodeId>,
        _hovered: Option<NodeId>,
        _revision: u64,
    ) {
        self.see(t, map.geometry);
        let Some(arc) = &self.arc else {
            return;
        };
        let screen = |(x, z): (f64, f64)| {
            let (mx, my) = map.geometry.to_map(x, z);
            let (sx, sy) = t.map_to_screen(mx, my);
            egui::pos2(sx as f32, sy as f32)
        };
        let flag = arc.settings.flag.0;
        let colour = look::connection_colour(bezier::kind(arc.settings.shape), flag, flag);
        let stroke = egui::Stroke::new(STROKE_PX, colour);
        let dot = look::node_colour(flag);
        let mut points: Vec<egui::Pos2> = arc.points().into_iter().map(screen).collect();
        // In link order, so the arrowheads show the direction "Swap direction" sets.
        if arc.reversed {
            points.reverse();
        }
        bezier::paint_chain(painter, &points, stroke, arc.settings.shape, map, t.zoom);
        for &p in &points {
            painter.circle_filled(p, POINT_RADIUS_PX, dot);
        }
        let half_node = look::node_px(map.node_size, t.zoom) as f32 / 2.0;
        for h in HANDLES {
            painter.circle_filled(
                screen(arc.handle(h)),
                half_node.max(HANDLE_MIN_PX),
                egui::Color32::WHITE,
            );
        }
    }

    /// Everything is drawn in [`Tool::paint_selection`].
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

    fn in_progress(&self) -> bool {
        self.arc.is_some() || self.drag.is_some()
    }

    fn cancel(&mut self) {
        self.arc = None;
        self.drag = None;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use ad_core::heightmap::Heightmap;
    use ad_core::network::{ConnectionShape, NodeFlag};

    use super::*;

    const EPS: f64 = 1e-9;

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < EPS && (a.1 - b.1).abs() < EPS
    }

    fn no_heightmap() -> MapContext {
        MapContext::new(None, 1, 2.0, (2.0, 2.0))
    }

    fn ctx<'a>(net: &'a Network, map: &'a MapContext, (x, z): (f64, f64)) -> ToolContext<'a> {
        static EMPTY: BTreeSet<NodeId> = BTreeSet::new();
        ToolContext {
            x,
            z,
            map,
            net,
            selection: &EMPTY,
            snap: false,
            hovered_node: None,
            hovered_edge: None,
        }
    }

    /// A tool that has seen a view at zoom 4 on a scale-1 map (0.25 world units per px)
    /// and previews an arc placed at (100, 50).
    fn placed(net: &Network, map: &MapContext) -> ArcTool {
        let mut tool = ArcTool::default();
        let mut t = ViewTransform::new((512.0, 512.0), (1024.0, 1024.0));
        t.zoom = 4.0;
        tool.see(&t, MapGeometry::new(1.0));
        assert!(tool.on_click(&ctx(net, map, (100.0, 50.0))).is_none());
        tool
    }

    /// Drags from `from` to `to` in two moves; asserts the press is claimed and nothing
    /// commits.
    fn drag(tool: &mut ArcTool, net: &Network, map: &MapContext, from: (f64, f64), to: (f64, f64)) {
        assert!(tool.on_drag_start(&ctx(net, map, from), None));
        let half = ((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0);
        tool.on_drag(&ctx(net, map, half));
        assert!(tool.on_drag_end(&ctx(net, map, to)).is_none());
    }

    fn committed(tx: &Transaction) -> &PasteNodes {
        let [Command::PasteNodes(p)] = tx.0.as_slice() else {
            panic!("one PasteNodes");
        };
        p
    }

    #[test]
    fn a_click_places_a_half_circle_of_eight_points() {
        let net = Network::new();
        let map = no_heightmap();
        let mut tool = placed(&net, &map);
        let arc = tool.arc().expect("arc").clone();
        assert_eq!(arc.centre, (100.0, 50.0));
        assert_eq!(arc.radius, 8.0);
        assert_eq!(span(arc.start_deg, arc.end_deg), (270.0, 450.0));
        let points = arc.points();
        assert_eq!(points.len(), 8);
        assert!(close(points[0], (92.0, 50.0)), "{points:?}");
        assert!(close(points[7], (108.0, 50.0)), "{points:?}");
        // Through angle 0 at -z.
        assert!(points.iter().all(|p| p.1 <= 50.0 + EPS));
        assert!(close(arc.handle(Handle::Start), (92.0, 50.0)));
        assert!(close(arc.handle(Handle::End), (108.0, 50.0)));
        assert!(close(arc.handle(Handle::Radius), (96.0, 50.0)));
        // A further click keeps the arc.
        tool.on_click(&ctx(&net, &map, (0.0, 0.0)));
        assert_eq!(tool.arc(), Some(&arc));
    }

    #[test]
    fn points_run_evenly_in_angle_with_both_ends() {
        let points = arc_points((0.0, 0.0), 10.0, 0.0, 180.0, 2);
        let want = [0.0, 60.0, 120.0, 180.0].map(|deg| on_circle((0.0, 0.0), 10.0, deg));
        assert_eq!(points.len(), 4);
        for (p, w) in points.iter().zip(want) {
            assert!(close(*p, w), "{points:?}");
        }
        assert!(close(points[0], (0.0, -10.0)));
        assert!(close(points[3], (0.0, 10.0)));
    }

    #[test]
    fn an_end_below_the_start_wraps_through_zero() {
        assert_eq!(span(270.0, 90.0), (270.0, 450.0));
        assert_eq!(span(-90.0, 450.0), (270.0, 450.0));
        assert_eq!(span(360.0, 10.0), (0.0, 10.0));
        let points = arc_points((0.0, 0.0), 10.0, 270.0, 90.0, 1);
        // Over 180° through angle 0 at -z.
        assert!(close(points[1], (0.0, -10.0)), "{points:?}");
        assert!(close(points[0], (-10.0, 0.0)) && close(points[2], (10.0, 0.0)));
    }

    #[test]
    fn the_centre_handle_moves_the_whole_arc() {
        let net = Network::new();
        let map = no_heightmap();
        let mut tool = placed(&net, &map);
        let before = tool.arc().expect("arc").clone();
        drag(&mut tool, &net, &map, (100.0, 50.0), (110.0, 55.0));
        let arc = tool.arc().expect("arc");
        for (a, b) in arc.points().iter().zip(before.points()) {
            assert!(close(*a, (b.0 + 10.0, b.1 + 5.0)));
        }
        for h in HANDLES {
            let (bx, bz) = before.handle(h);
            assert!(close(arc.handle(h), (bx + 10.0, bz + 5.0)), "{h:?}");
        }
    }

    #[test]
    fn an_angle_handle_turns_to_the_pointers_whole_degree() {
        let net = Network::new();
        let map = no_heightmap();
        let mut tool = placed(&net, &map);
        let to = on_circle((100.0, 50.0), 20.0, 30.4);
        assert_eq!(angle_to((100.0, 50.0), to), 30.0);
        drag(&mut tool, &net, &map, (92.0, 50.0), to);
        let arc = tool.arc().expect("arc");
        assert_eq!(arc.start_deg, 30.0);
        assert_eq!(arc.radius, 8.0);
        let points = arc.points();
        assert!(close(points[0], on_circle((100.0, 50.0), 8.0, 30.0)));
        assert!(close(points[7], (108.0, 50.0)));
        // The end handle follows the pointer too; 0.6° rounds up.
        drag(
            &mut tool,
            &net,
            &map,
            (108.0, 50.0),
            on_circle((100.0, 50.0), 3.0, 200.6),
        );
        assert_eq!(tool.arc().expect("arc").end_deg, 201.0);
        // A lost pointer ends the drag only.
        let start = tool.arc().expect("arc").handle(Handle::Start);
        assert!(tool.on_drag_start(&ctx(&net, &map, start), None));
        tool.on_drag_cancel();
        tool.on_drag(&ctx(&net, &map, (0.0, 0.0)));
        assert_eq!(tool.arc().expect("arc").start_deg, 30.0);
    }

    #[test]
    fn the_radius_handle_sets_twice_its_distance_at_least_two() {
        let net = Network::new();
        let map = no_heightmap();
        let mut tool = placed(&net, &map);
        // The handle at (96, 50) lies 4 from the centre; to 6 from it, off the line too.
        drag(&mut tool, &net, &map, (96.0, 50.0), (94.0, 51.0));
        let arc = tool.arc().expect("arc");
        assert!((arc.radius - 12.0).abs() < EPS);
        assert!(close(arc.handle(Handle::Radius), (94.0, 50.0)));
        assert!(close(arc.points()[0], (88.0, 50.0)));
        // To 0.5 from the centre, and past it.
        drag(&mut tool, &net, &map, (94.0, 50.0), (99.5, 50.0));
        assert!((tool.arc().expect("arc").radius - 2.0).abs() < EPS);
        let mut arc = Arc::new((0.0, 0.0));
        arc.move_radius_handle((10.0, 0.0));
        assert_eq!(arc.radius, 2.0);
        // No farther out than the start in one move.
        let mut arc = Arc::new((0.0, 0.0));
        arc.move_radius_handle((-20.0, 0.0));
        assert!((arc.radius - 16.0).abs() < EPS);
    }

    #[test]
    fn a_press_off_the_handles_is_not_claimed() {
        let net = Network::new();
        let map = no_heightmap();
        let mut tool = placed(&net, &map);
        // 8 px is 2 world units at zoom 4.
        assert!(!tool.on_drag_start(&ctx(&net, &map, (100.0, 52.1)), None));
        tool.on_drag(&ctx(&net, &map, (0.0, 0.0)));
        assert_eq!(tool.arc().expect("arc").centre, (100.0, 50.0));
        let mut idle = ArcTool::default();
        idle.see(
            &ViewTransform::new((0.0, 0.0), (1.0, 1.0)),
            MapGeometry::new(1.0),
        );
        assert!(!idle.on_drag_start(&ctx(&net, &map, (100.0, 50.0)), None));
    }

    #[test]
    fn the_commit_links_the_points_in_order_or_reversed_after_swap() {
        let mut arc = Arc::new((0.0, 0.0));
        arc.settings.points = 2;
        arc.settings.flag = NodeFlag::SUB_PRIO;
        let tx = plan(&no_heightmap(), &arc);
        let p = committed(&tx);
        assert_eq!(p.nodes.len(), 4);
        assert!(p.nodes.iter().all(|n| n.3 == NodeFlag::SUB_PRIO));
        assert!(p.markers.is_empty());
        let new = LinkEnd::New;
        let one = ConnectionShape::OneWay;
        assert_eq!(
            p.links,
            [
                (new(0), new(1), one),
                (new(1), new(2), one),
                (new(2), new(3), one)
            ]
        );
        arc.swap();
        let tx = plan(&no_heightmap(), &arc);
        let swapped = committed(&tx);
        assert_eq!(swapped.nodes, p.nodes, "the points stay");
        assert_eq!(
            swapped.links,
            [
                (new(3), new(2), one),
                (new(2), new(1), one),
                (new(1), new(0), one)
            ]
        );
        arc.settings.shape = ConnectionShape::Dual;
        let tx = plan(&no_heightmap(), &arc);
        assert!(
            committed(&tx)
                .links
                .iter()
                .all(|l| l.2 == ConnectionShape::Dual)
        );
    }

    #[test]
    fn heights_are_sampled_or_the_no_height_sentinel() {
        let arc = Arc::new((10.0, 10.0));
        let mut map = no_heightmap();
        let tx = plan(&map, &arc);
        assert!(committed(&tx).nodes.iter().all(|n| n.1 == -1.0));
        // Every sample is 10 m (raw 2560) at height map scale 2: 5.0.
        map.heightmap = Some(std::sync::Arc::new(
            Heightmap::new(2, 2, vec![2560; 4]).expect("size"),
        ));
        map.height_map_scale = 2.0;
        let tx = plan(&map, &arc);
        let nodes = &committed(&tx).nodes;
        assert_eq!(nodes.len(), 8);
        assert!(nodes.iter().all(|n| n.1 == 5.0));
    }

    #[test]
    fn cancel_drops_the_arc_and_the_next_starts_with_the_defaults() {
        let net = Network::new();
        let map = no_heightmap();
        let mut tool = placed(&net, &map);
        let arc = tool.arc_mut().expect("arc");
        arc.settings.points = 20;
        arc.swap();
        drag(&mut tool, &net, &map, (100.0, 50.0), (0.0, 0.0));
        assert!(tool.in_progress());
        tool.cancel();
        assert!(tool.arc().is_none() && !tool.in_progress());
        tool.on_click(&ctx(&net, &map, (5.0, 5.0)));
        assert_eq!(tool.arc(), Some(&Arc::new((5.0, 5.0))));
        assert_eq!(tool.arc().expect("arc").settings, CurveSettings::default());
    }
}
