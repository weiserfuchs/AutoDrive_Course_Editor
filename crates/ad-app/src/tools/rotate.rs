//! The rotate tool (Java `RotationButton`, `RotationUtils`): for a non-empty selection it
//! shows the centre of the selection's bounding box and a handle straight above it. Dragging
//! the handle turns the selection about that centre, in whole degrees or, while grid snap
//! applies, in 5° steps. The drag is a preview; the release commits one `MoveNodes` (AD-4,
//! AD-10).

use std::cell::Cell;
use std::collections::BTreeSet;

use eframe::egui;

use ad_core::command::{Command, MoveNodes, Position, Transaction};
use ad_core::geometry::MapGeometry;
use ad_core::map_context::MapContext;
use ad_core::network::{Network, NodeId};

use super::{Tool, ToolContext};
use crate::look;
use crate::view::{Preview, ViewTransform};

pub const NAME: &str = "Rotate";

/// The rotation step while grid snap applies (Java's default `rotationStep`).
pub const SNAP_STEP_DEG: f64 = 5.0;
/// A press this close to the handle, in screen px, grabs it.
const HANDLE_HIT_PX: f64 = 8.0;
/// The smallest circle radius in screen px, so the handle's hit area never covers the centre.
const MIN_RADIUS_PX: f64 = 24.0;
/// The radius of the centre dot, in screen px (Java draws it 6 px wide).
const CENTRE_DOT_PX: f32 = 3.0;
/// The smallest radius the handle is drawn with, in screen px.
const HANDLE_MIN_PX: f32 = 4.0;

/// The centre of the bounding box of the selected nodes' (x, z); `None` when no selected
/// node is in the network.
pub fn centre(net: &Network, selection: &BTreeSet<NodeId>) -> Option<(f64, f64)> {
    let mut nodes = selection.iter().filter_map(|&id| net.node(id));
    let first = nodes.next()?;
    let (mut x0, mut x1, mut z0, mut z1) = (first.x, first.x, first.z, first.z);
    for n in nodes {
        (x0, x1, z0, z1) = (x0.min(n.x), x1.max(n.x), z0.min(n.z), z1.max(n.z));
    }
    Some(((x0 + x1) / 2.0, (z0 + z1) / 2.0))
}

/// `(x, z)` turned by `degrees` about `centre`. Positive angles turn clockwise on screen
/// (world z points down), as Java's `AffineTransform.rotate`.
pub fn turned((x, z): (f64, f64), (cx, cz): (f64, f64), degrees: f64) -> (f64, f64) {
    let (sin, cos) = degrees.to_radians().sin_cos();
    let (dx, dz) = (x - cx, z - cz);
    (cx + dx * cos - dz * sin, cz + dx * sin + dz * cos)
}

/// `degrees` rounded to whole degrees, or to [`SNAP_STEP_DEG`] steps with `snap`.
pub fn snap_angle(degrees: f64, snap: bool) -> f64 {
    let step = if snap { SNAP_STEP_DEG } else { 1.0 };
    (degrees / step).round() * step
}

/// The pointer's angle about `centre` in degrees, clockwise from straight above (where the
/// handle starts), in -180..=180.
fn pointer_angle((x, z): (f64, f64), (cx, cz): (f64, f64)) -> f64 {
    (x - cx).atan2(cz - z).to_degrees()
}

/// What the tool needs of the last painted view: world units per screen px and half the
/// node size in screen px.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ViewScale {
    world_per_px: f64,
    half_node_px: f64,
}

/// The rotation circle: its centre and radius in world units. The handle starts straight
/// above the centre, on the circle.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Circle {
    centre: (f64, f64),
    radius: f64,
}

impl Circle {
    /// The circle for `selection`: through the furthest selected node plus half a node
    /// size on screen (Java `getSelectionRadius`), and at least [`MIN_RADIUS_PX`].
    fn of(net: &Network, selection: &BTreeSet<NodeId>, scale: ViewScale) -> Option<Self> {
        Some(Self::new(reach(net, selection)?, scale))
    }

    /// The circle about `centre` through a node `reach` world units from it.
    fn new((centre, reach): Reach, scale: ViewScale) -> Self {
        let radius_px = (reach / scale.world_per_px + scale.half_node_px).max(MIN_RADIUS_PX);
        Self {
            centre,
            radius: radius_px * scale.world_per_px,
        }
    }

    /// The handle turned by `degrees`.
    fn handle(&self, degrees: f64) -> (f64, f64) {
        let start = (self.centre.0, self.centre.1 - self.radius);
        turned(start, self.centre, degrees)
    }
}

/// The selection's centre and the world distance of its furthest node from it.
fn reach(net: &Network, selection: &BTreeSet<NodeId>) -> Option<Reach> {
    let centre = centre(net, selection)?;
    let reach = selection
        .iter()
        .filter_map(|&id| net.node(id))
        .map(|n| (n.x - centre.0).hypot(n.z - centre.1))
        .fold(0.0, f64::max);
    Some((centre, reach))
}

/// A selection's centre and the world distance of its furthest node from it.
type Reach = ((f64, f64), f64);

/// When the cached centre and reach were computed: the document revision, the selection's
/// and the network's length.
type CacheKey = (u64, usize, usize);

/// A running rotation.
#[derive(Debug, Clone, PartialEq)]
struct Drag {
    circle: Circle,
    /// The pointer's angle about the centre at the press: the turn is measured from it, so
    /// a press anywhere on the handle starts at 0°.
    press_angle: f64,
    /// The previewed angle in degrees, already rounded.
    angle: f64,
    /// Every selected node with its stored position when the drag started.
    moved: Vec<(NodeId, Position)>,
    /// The previewed world (x, z) of every selected node.
    preview: Preview,
}

#[derive(Default)]
pub struct RotateTool {
    drag: Option<Drag>,
    /// The view the handle was last painted in; a press is tested against it.
    view: Cell<Option<ViewScale>>,
    /// The idle handle's centre and reach, so a large selection is not walked every frame.
    reach: Cell<Option<(CacheKey, Option<Reach>)>>,
}

impl RotateTool {
    /// Remembers the view the handle is drawn in; [`Tool::paint_selection`] calls it every
    /// frame. A drag can start only after it.
    pub fn see(&self, t: &ViewTransform, geometry: MapGeometry, node_size: f64) {
        self.view.set(Some(ViewScale {
            world_per_px: geometry.scale / t.zoom,
            half_node_px: f64::from(look::node_px(node_size, t.zoom)) / 2.0,
        }));
    }

    /// The handle's world (x, z) for `selection` in the last seen view, before any turn.
    #[cfg(test)]
    pub fn handle_at(&self, net: &Network, selection: &BTreeSet<NodeId>) -> Option<(f64, f64)> {
        Circle::of(net, selection, self.view.get()?).map(|c| c.handle(0.0))
    }
}

impl Tool for RotateTool {
    fn name(&self) -> &'static str {
        NAME
    }

    fn on_click(&mut self, _ctx: &ToolContext) -> Option<Transaction> {
        None
    }

    /// Claims only a press within [`HANDLE_HIT_PX`] of the handle; any other press pans.
    fn on_drag_start(&mut self, ctx: &ToolContext, _hovered: Option<NodeId>) -> bool {
        let Some(scale) = self.view.get() else {
            return false;
        };
        let Some(circle) = Circle::of(ctx.net, ctx.selection, scale) else {
            return false;
        };
        let (hx, hz) = circle.handle(0.0);
        if (ctx.x - hx).hypot(ctx.z - hz) > HANDLE_HIT_PX * scale.world_per_px {
            return false;
        }
        let moved: Vec<(NodeId, Position)> = ctx
            .selection
            .iter()
            .filter_map(|&id| ctx.net.node(id).map(|n| (id, (n.x, n.y, n.z))))
            .collect();
        self.drag = Some(Drag {
            circle,
            press_angle: pointer_angle((ctx.x, ctx.z), circle.centre),
            angle: 0.0,
            preview: moved.iter().map(|&(id, (x, _, z))| (id, (x, z))).collect(),
            moved,
        });
        true
    }

    fn on_drag(&mut self, ctx: &ToolContext) {
        let Some(d) = &mut self.drag else {
            return;
        };
        let turn = pointer_angle((ctx.x, ctx.z), d.circle.centre) - d.press_angle;
        // Into -180..180, so a turn across straight below stays small.
        let angle = snap_angle((turn + 180.0).rem_euclid(360.0) - 180.0, ctx.snap);
        if angle == d.angle {
            return;
        }
        d.angle = angle;
        for &(id, (x, _, z)) in &d.moved {
            d.preview.insert(id, turned((x, z), d.circle.centre, angle));
        }
    }

    fn on_drag_end(&mut self, ctx: &ToolContext) -> Option<Transaction> {
        self.on_drag(ctx);
        let d = self.drag.take()?;
        if d.angle == 0.0 {
            return None;
        }
        // The nodes keep their height, as with the move tool.
        let moves: Vec<_> = d
            .moved
            .iter()
            .map(|&(id, before)| {
                let (x, z) = d.preview.get(&id).copied().unwrap_or((before.0, before.2));
                (id, before, (x, before.1, z))
            })
            .collect();
        // A turn of nodes all at the centre (one selected node) moves nothing.
        moves
            .iter()
            .any(|(_, before, after)| before != after)
            .then(|| Transaction::new(vec![Command::MoveNodes(MoveNodes { moves })]))
    }

    fn preview(&self) -> Option<&Preview> {
        self.drag.as_ref().map(|d| &d.preview)
    }

    /// Draws the centre dot, the circle and the handle (white, as Java's control node).
    /// Without a drag the selection's centre and reach are cached per `revision`.
    fn paint_selection(
        &self,
        painter: &egui::Painter,
        t: &ViewTransform,
        map: &MapContext,
        net: &Network,
        selection: &BTreeSet<NodeId>,
        revision: u64,
    ) {
        self.see(t, map.geometry, map.node_size);
        let Some(scale) = self.view.get() else {
            return;
        };
        let (circle, angle) = match &self.drag {
            Some(d) => (d.circle, d.angle),
            None => {
                let key = (revision, selection.len(), net.len());
                let found = match self.reach.get() {
                    Some((k, found)) if k == key => found,
                    _ => {
                        let found = reach(net, selection);
                        self.reach.set(Some((key, found)));
                        found
                    }
                };
                match found {
                    Some(r) => (Circle::new(r, scale), 0.0),
                    None => return,
                }
            }
        };
        let screen = |(x, z): (f64, f64)| {
            let (mx, my) = map.geometry.to_map(x, z);
            let (sx, sy) = t.map_to_screen(mx, my);
            egui::pos2(sx as f32, sy as f32)
        };
        let white = egui::Color32::WHITE;
        let centre = screen(circle.centre);
        painter.circle_filled(centre, CENTRE_DOT_PX, white);
        let radius = (circle.radius / scale.world_per_px) as f32;
        painter.circle_stroke(
            centre,
            radius,
            egui::Stroke::new(1.0, white.gamma_multiply(0.5)),
        );
        let handle_px = (scale.half_node_px as f32).max(HANDLE_MIN_PX);
        painter.circle_filled(screen(circle.handle(angle)), handle_px, white);
    }

    /// Everything is drawn in [`Tool::paint_selection`]; the view draws the turned nodes.
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

    use ad_core::network::NodeFlag;

    use super::*;

    const EPS: f64 = 1e-9;

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < EPS && (a.1 - b.1).abs() < EPS
    }

    /// Nodes a (0, 1, 0), b (10, 2, 0) and c (10, 3, 20), plus d (100, 4, 100) outside the
    /// selection {a, b, c}, on a scale-1 map. The bounding-box centre is (5, 10).
    fn setup() -> (Network, [NodeId; 4], BTreeSet<NodeId>, MapContext) {
        let mut net = Network::new();
        let a = net.add_node(0.0, 1.0, 0.0, NodeFlag::REGULAR);
        let b = net.add_node(10.0, 2.0, 0.0, NodeFlag::REGULAR);
        let c = net.add_node(10.0, 3.0, 20.0, NodeFlag::REGULAR);
        let d = net.add_node(100.0, 4.0, 100.0, NodeFlag::REGULAR);
        let map = MapContext::new(None, 1, 2.0, (2.0, 2.0));
        (net, [a, b, c, d], BTreeSet::from([a, b, c]), map)
    }

    /// A tool that has seen a view at zoom 4 on a scale-1 map: 0.25 world units per px.
    fn tool() -> RotateTool {
        let tool = RotateTool::default();
        let mut t = ViewTransform::new((512.0, 512.0), (1024.0, 1024.0));
        t.zoom = 4.0;
        tool.see(&t, MapGeometry::new(1.0), 2.0);
        tool
    }

    fn ctx<'a>(
        net: &'a Network,
        map: &'a MapContext,
        selection: &'a BTreeSet<NodeId>,
        (x, z): (f64, f64),
        snap: bool,
    ) -> ToolContext<'a> {
        ToolContext {
            x,
            z,
            map,
            net,
            selection,
            snap,
            hovered_node: None,
            hovered_edge: None,
        }
    }

    /// The pointer at `degrees` about (5, 10), 30 world units out.
    fn pointer(degrees: f64) -> (f64, f64) {
        turned((5.0, -20.0), (5.0, 10.0), degrees)
    }

    /// Grabs the handle, drags to `degrees` and releases there; returns the commit.
    fn rotate(degrees: f64, snap: bool) -> Option<Vec<(NodeId, Position, Position)>> {
        let (net, _, selection, map) = setup();
        let mut tool = tool();
        let handle = tool.handle_at(&net, &selection).expect("handle");
        assert!(tool.on_drag_start(&ctx(&net, &map, &selection, handle, snap), None));
        let at = ctx(&net, &map, &selection, pointer(degrees), snap);
        tool.on_drag(&at);
        let tx = tool.on_drag_end(&at);
        assert_eq!(tool.preview(), None, "the release drops the preview");
        let tx = tx?;
        let [Command::MoveNodes(m)] = tx.0.as_slice() else {
            panic!("one MoveNodes");
        };
        let mut moves = m.moves.clone();
        moves.sort_by_key(|m| m.0);
        Some(moves)
    }

    #[test]
    fn helpers_find_the_centre_turn_and_round() {
        let (net, [a, _, _, d], selection, _) = setup();
        assert_eq!(centre(&net, &selection), Some((5.0, 10.0)));
        assert_eq!(centre(&net, &BTreeSet::from([a, d])), Some((50.0, 50.0)));
        assert_eq!(centre(&net, &BTreeSet::new()), None);
        // Clockwise on screen: above the centre goes to its right.
        assert!(close(turned((5.0, 0.0), (5.0, 10.0), 90.0), (15.0, 10.0)));
        assert!(close(turned((5.0, 0.0), (5.0, 10.0), -90.0), (-5.0, 10.0)));
        assert_eq!(snap_angle(37.4, false), 37.0);
        assert_eq!(snap_angle(37.4, true), 35.0);
        assert_eq!(snap_angle(-37.6, true), -40.0);
        assert!(close(
            (pointer_angle(pointer(37.4), (5.0, 10.0)), 0.0),
            (37.4, 0.0)
        ));
    }

    #[test]
    fn the_handle_sits_above_the_centre_beyond_the_furthest_node() {
        let (net, _, selection, _) = setup();
        // Furthest node: hypot(5, 10); half a node at zoom 4 is look::node_px(2, 4) / 2 px.
        let half = f64::from(look::node_px(2.0, 4.0)) / 2.0;
        let radius = 5.0f64.hypot(10.0) + half * 0.25;
        let (hx, hz) = tool().handle_at(&net, &selection).expect("handle");
        assert!(close((hx, hz), (5.0, 10.0 - radius)));
    }

    #[test]
    fn a_quarter_turn_moves_every_selected_node_in_one_move_nodes() {
        let (_, [a, b, c, _], _, _) = setup();
        let moves = rotate(90.0, false).expect("moves");
        let ids: Vec<_> = moves.iter().map(|m| m.0).collect();
        assert_eq!(ids, [a, b, c], "d is not selected");
        let want = [
            (a, (0.0, 1.0, 0.0), (15.0, 1.0, 5.0)),
            (b, (10.0, 2.0, 0.0), (15.0, 2.0, 15.0)),
            (c, (10.0, 3.0, 20.0), (-5.0, 3.0, 15.0)),
        ];
        for ((id, before, after), (wid, wbefore, wafter)) in moves.iter().zip(want) {
            assert_eq!((*id, *before), (wid, wbefore));
            assert_eq!(after.1, wafter.1, "y kept");
            assert!(close((after.0, after.2), (wafter.0, wafter.2)), "{after:?}");
        }
    }

    #[test]
    fn the_angle_is_rounded_to_whole_degrees_or_snap_steps() {
        let (net, [a, ..], _, _) = setup();
        let n = net.node(a).expect("a");
        for (snap, want) in [(false, 37.0), (true, 35.0)] {
            let moves = rotate(37.4, snap).expect("moves");
            let after = moves[0].2;
            let expected = turned((n.x, n.z), (5.0, 10.0), want);
            assert!(close((after.0, after.2), expected), "snap {snap}");
        }
    }

    #[test]
    fn the_preview_and_the_handle_follow_the_rounded_angle() {
        let (net, [a, ..], selection, map) = setup();
        let mut tool = tool();
        let handle = tool.handle_at(&net, &selection).expect("handle");
        assert!(tool.on_drag_start(&ctx(&net, &map, &selection, handle, false), None));
        assert_eq!(tool.preview().expect("preview")[&a], (0.0, 0.0));
        tool.on_drag(&ctx(&net, &map, &selection, pointer(37.4), false));
        let preview = tool.preview().expect("preview");
        assert_eq!(preview.len(), 3);
        assert!(close(preview[&a], turned((0.0, 0.0), (5.0, 10.0), 37.0)));
        let d = tool.drag.as_ref().expect("drag");
        assert!(close(
            d.circle.handle(d.angle),
            turned(handle, (5.0, 10.0), 37.0)
        ));
    }

    #[test]
    fn a_release_at_no_turn_commits_nothing() {
        assert!(rotate(0.0, false).is_none());
        // 0.4° rounds to 0°; with snap 2° does too.
        assert!(rotate(0.4, false).is_none());
        assert!(rotate(2.0, true).is_none());
    }

    #[test]
    fn undoing_a_rotation_restores_the_exact_start_positions() {
        let (mut net, ids, _, _) = setup();
        let start: Vec<_> = ids
            .iter()
            .map(|&id| net.node(id).map(|n| (n.x, n.y, n.z)))
            .collect();
        let moves = rotate(37.4, false).expect("moves");
        let mut tx = Transaction::new(vec![Command::MoveNodes(MoveNodes { moves })]);
        tx.apply(&mut net).expect("applies");
        assert_ne!(net.node(ids[0]).map(|n| (n.x, n.y, n.z)), start[0]);
        tx.revert(&mut net).expect("reverts");
        let back: Vec<_> = ids
            .iter()
            .map(|&id| net.node(id).map(|n| (n.x, n.y, n.z)))
            .collect();
        assert_eq!(back, start);
    }

    #[test]
    fn an_empty_selection_has_no_handle_and_claims_no_drag() {
        let (net, _, _, map) = setup();
        let none = BTreeSet::new();
        let mut tool = tool();
        assert_eq!(tool.handle_at(&net, &none), None);
        assert!(!tool.on_drag_start(&ctx(&net, &map, &none, (5.0, 0.0), false), None));
        assert_eq!(tool.preview(), None);
    }

    #[test]
    fn a_press_off_the_handle_is_not_claimed() {
        let (net, [a, ..], selection, map) = setup();
        let mut tool = tool();
        let (hx, hz) = tool.handle_at(&net, &selection).expect("handle");
        // 8 px is 2 world units at zoom 4: just outside, and on a selected node.
        let off = ctx(&net, &map, &selection, (hx + 2.1, hz), false);
        assert!(!tool.on_drag_start(&off, Some(a)));
        tool.on_drag(&ctx(&net, &map, &selection, pointer(90.0), false));
        assert!(
            tool.on_drag_end(&ctx(&net, &map, &selection, pointer(90.0), false))
                .is_none()
        );
        // Just inside it is.
        let near = ctx(&net, &map, &selection, (hx + 1.9, hz), false);
        assert!(tool.on_drag_start(&near, None));
    }

    #[test]
    fn a_single_node_gets_a_handle_off_its_centre_and_its_turn_commits_nothing() {
        let (net, [a, ..], _, map) = setup();
        let one = BTreeSet::from([a]);
        let mut tool = tool();
        // 24 px at zoom 4 is 6 world units, more than the 2-unit (8 px) hit area.
        let handle = tool.handle_at(&net, &one).expect("handle");
        assert!(close(handle, (0.0, -6.0)));
        assert!(!tool.on_drag_start(&ctx(&net, &map, &one, (0.0, -1.0), false), Some(a)));
        assert!(tool.on_drag_start(&ctx(&net, &map, &one, handle, false), Some(a)));
        let end = ctx(&net, &map, &one, (6.0, 0.0), false);
        tool.on_drag(&end);
        assert_eq!(tool.drag.as_ref().map(|d| d.angle), Some(90.0));
        assert!(tool.on_drag_end(&end).is_none());
    }

    #[test]
    fn a_press_beside_the_handle_starts_at_no_turn() {
        let (net, [a, ..], selection, map) = setup();
        let mut tool = tool();
        let (hx, hz) = tool.handle_at(&net, &selection).expect("handle");
        // 1.5 world units (6 px) right of the handle: about 5° off straight above.
        let press = (hx + 1.5, hz);
        assert!(tool.on_drag_start(&ctx(&net, &map, &selection, press, false), None));
        tool.on_drag(&ctx(&net, &map, &selection, press, false));
        assert_eq!(tool.preview().expect("preview")[&a], (0.0, 0.0));
        let at = turned(press, (5.0, 10.0), 37.4);
        tool.on_drag(&ctx(&net, &map, &selection, at, false));
        let want = turned((0.0, 0.0), (5.0, 10.0), 37.0);
        assert!(close(tool.preview().expect("preview")[&a], want));
        // Past straight below the turn wraps to the other side, not past ±180.
        let at = turned(press, (5.0, 10.0), 200.0);
        tool.on_drag(&ctx(&net, &map, &selection, at, false));
        assert_eq!(tool.drag.as_ref().map(|d| d.angle), Some(-160.0));
    }

    #[test]
    fn the_idle_handle_is_recomputed_only_when_the_revision_or_a_length_changes() {
        let (mut net, [a, b, ..], selection, map) = setup();
        let tool = RotateTool::default();
        let ctx = egui::Context::default();
        let paint = |net: &Network, selection: &BTreeSet<NodeId>, revision: u64| {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                let t = ViewTransform::new((512.0, 512.0), (1024.0, 1024.0));
                tool.paint_selection(ui.painter(), &t, &map, net, selection, revision);
            });
            out.textures_delta.clear();
            tool.reach.get().and_then(|(_, r)| r).map(|r| r.0)
        };
        assert_eq!(paint(&net, &selection, 0), Some((5.0, 10.0)));
        let moved = MoveNodes {
            moves: vec![(a, (0.0, 1.0, 0.0), (-10.0, 1.0, 0.0))],
        };
        Transaction::new(vec![Command::MoveNodes(moved)])
            .apply(&mut net)
            .expect("applies");
        assert_eq!(paint(&net, &selection, 0), Some((5.0, 10.0)), "cached");
        assert_eq!(paint(&net, &selection, 1), Some((0.0, 10.0)));
        assert_eq!(paint(&net, &BTreeSet::from([b]), 1), Some((10.0, 0.0)));
    }

    #[test]
    fn before_any_view_was_seen_no_drag_is_claimed() {
        let (net, _, selection, map) = setup();
        let mut tool = RotateTool::default();
        assert!(!tool.on_drag_start(&ctx(&net, &map, &selection, (5.0, -1.2), false), None));
    }

    #[test]
    fn cancel_drops_the_preview_and_commits_nothing() {
        let (net, _, selection, map) = setup();
        let mut tool = tool();
        let handle = tool.handle_at(&net, &selection).expect("handle");
        assert!(tool.on_drag_start(&ctx(&net, &map, &selection, handle, false), None));
        tool.on_drag(&ctx(&net, &map, &selection, pointer(45.0), false));
        tool.cancel();
        assert_eq!(tool.preview(), None);
        let end = ctx(&net, &map, &selection, pointer(45.0), false);
        assert!(tool.on_drag_end(&end).is_none());
    }
}
