//! Hit tests and the info box of the hovered node as pure functions (Java
//! `MapPanel.getNodeAtScreenPosition`, `MapPanel` 399-569). Nothing here paints.

use eframe::egui::{Color32, Pos2, Rect, Vec2};

use ad_core::geometry::MapGeometry;
use ad_core::network::{Network, Node, NodeId};

use crate::lod::EDGE_HIT_PX;
use crate::look;
use crate::overlay;
use crate::view::ViewTransform;

type Pt = (f64, f64);

/// The node under `cursor` (screen px): Java's screen-square rule, strict on both axes,
/// with the closest node winning and the lowest id on a tie. `buf` is a reused query buffer.
pub fn node_at(
    net: &Network,
    geometry: MapGeometry,
    t: &ViewTransform,
    ns: i32,
    cursor: Pt,
    buf: &mut Vec<NodeId>,
) -> Option<NodeId> {
    // Nodes are not drawn below 2 px, so they cannot be hit either.
    if ns < 2 {
        return None;
    }
    let half = f64::from(ns) / 2.0;
    // One px of slack: the strict test below decides, the query only has to include everything.
    let reach = half + 1.0;
    let world = |dx: f64, dy: f64| {
        let (mx, my) = t.screen_to_map(cursor.0 + dx, cursor.1 + dy);
        geometry.to_world(mx, my)
    };
    let (x0, z0) = world(-reach, -reach);
    let (x1, z1) = world(reach, reach);
    net.nodes_in_rect_into(x0, z0, x1, z1, buf);
    let mut best: Option<(f64, NodeId)> = None;
    for &id in buf.iter() {
        let Some(n) = net.node(id) else {
            continue;
        };
        let (mx, my) = geometry.to_map(n.x, n.z);
        let (sx, sy) = t.map_to_screen(mx, my);
        let (dx, dy) = (sx - cursor.0, sy - cursor.1);
        if dx.abs() >= half || dy.abs() >= half {
            continue;
        }
        let d2 = dx * dx + dy * dy;
        if best.is_none_or(|(b, bid)| d2 < b || (d2 == b && id < bid)) {
            best = Some((d2, id));
        }
    }
    best.map(|(_, id)| id)
}

/// The connection nearest to `cursor` (screen px) within `EDGE_HIT_PX` of its drawn part
/// ([`drawn_segment`]); ties go to the lowest `(from, to)`. A node under the cursor wins:
/// with `hovered` set (the result of [`node_at`]) no connection is hit.
pub fn edge_at(
    net: &Network,
    geometry: MapGeometry,
    t: &ViewTransform,
    ns: i32,
    hovered: Option<NodeId>,
    cursor: Pt,
) -> Option<(NodeId, NodeId)> {
    if hovered.is_some() {
        return None;
    }
    let (mx, my) = t.screen_to_map(cursor.0, cursor.1);
    let (wx, wz) = geometry.to_world(mx, my);
    // Screen px -> map px -> world units. The full segment contains the drawn part, so the
    // query finds every candidate; the screen test below decides.
    let reach = EDGE_HIT_PX / t.zoom * geometry.scale;
    let mut best: Option<(f64, (NodeId, NodeId))> = None;
    // `edges_near` is sorted, so the first of equal distances is the lowest pair.
    for pair in net.edges_near(wx, wz, reach) {
        let Some((a, b)) = drawn_segment(net, geometry, t, ns, pair) else {
            continue;
        };
        let d = segment_distance(cursor, a, b);
        if d <= EDGE_HIT_PX && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, pair));
        }
    }
    best.map(|(_, pair)| pair)
}

/// The screen segment the view draws for the connection `(from, to)`: shortened by `ns / 2`
/// at each end, `None` when it is not drawn (shorter than `MIN_EDGE_PX` on screen).
pub fn drawn_segment(
    net: &Network,
    geometry: MapGeometry,
    t: &ViewTransform,
    ns: i32,
    (from, to): (NodeId, NodeId),
) -> Option<(Pt, Pt)> {
    let screen = |n: &Node| {
        let (mx, my) = geometry.to_map(n.x, n.z);
        t.map_to_screen(mx, my)
    };
    look::line_ends(screen(net.node(from)?), screen(net.node(to)?), ns)
}

fn segment_distance(p: Pt, a: Pt, b: Pt) -> f64 {
    let (vx, vz) = (b.0 - a.0, b.1 - a.1);
    let len2 = vx * vx + vz * vz;
    let s = if len2 == 0.0 {
        0.0
    } else {
        (((p.0 - a.0) * vx + (p.1 - a.1) * vz) / len2).clamp(0.0, 1.0)
    };
    (p.0 - (a.0 + s * vx)).hypot(p.1 - (a.1 + s * vz))
}

/// One line of the info box.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub text: String,
    pub text_colour: Color32,
    pub background: Color32,
    pub centred: bool,
}

/// The info box rows of a hovered node, in order. `file_id` is its 1-based id in the file.
pub fn info_rows(node: &Node, file_id: usize, show_id: bool) -> Vec<Row> {
    let mut rows = Vec::new();
    if show_id {
        rows.push(Row {
            text: format!("Node ID {file_id}"),
            text_colour: Color32::BLACK,
            background: Color32::WHITE,
            centred: true,
        });
    }
    if overlay::has_height_warning(node.y) {
        rows.push(Row {
            text: "Node Y is invalid ( -1 )".to_owned(),
            text_colour: Color32::BLACK,
            background: Color32::RED,
            centred: false,
        });
    }
    rows
}

/// The box of `size` with its top-left at `anchor`, moved to lie inside `panel`.
pub fn box_rect(anchor: Pos2, size: Vec2, panel: Rect) -> Rect {
    let x = anchor.x.min(panel.max.x - size.x).max(panel.min.x);
    let y = anchor.y.min(panel.max.y - size.y).max(panel.min.y);
    Rect::from_min_size(Pos2::new(x, y), size)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use ad_core::network::{ConnectionShape, NodeFlag};

    /// Panel centred at (500, 400): world (0, 0) is at screen (500, 400).
    fn view(zoom: f64) -> ViewTransform {
        let mut t = ViewTransform::new((500.0, 400.0), (1000.0, 800.0));
        t.zoom = zoom;
        t
    }

    fn hit(net: &Network, zoom: f64, cursor: Pt) -> Option<NodeId> {
        let ns = crate::look::node_px(2.0, zoom);
        node_at(
            net,
            MapGeometry::new(1.0),
            &view(zoom),
            ns,
            cursor,
            &mut Vec::new(),
        )
    }

    #[test]
    fn node_hit_is_strict() {
        let mut net = Network::new();
        let a = net.add_node(0.0, 0.0, 0.0, NodeFlag::REGULAR);
        // Zoom 10 gives ns 11: half 5.5.
        assert_eq!(hit(&net, 10.0, (505.4, 400.0)), Some(a));
        assert_eq!(hit(&net, 10.0, (505.5, 400.0)), None);
        assert_eq!(hit(&net, 10.0, (500.0, 394.6)), Some(a));
        assert_eq!(hit(&net, 10.0, (500.0, 394.5)), None);
    }

    #[test]
    fn overlap_picks_closest_then_lowest_id() {
        let mut net = Network::new();
        // 2 px apart at zoom 10: 0.2 world units.
        let a = net.add_node(0.0, 0.0, 0.0, NodeFlag::REGULAR);
        let b = net.add_node(0.2, 0.0, 0.0, NodeFlag::REGULAR);
        assert_eq!(hit(&net, 10.0, (500.5, 400.0)), Some(a));
        assert_eq!(hit(&net, 10.0, (501.5, 400.0)), Some(b));
        // Equal distance: the lower id.
        assert_eq!(hit(&net, 10.0, (501.0, 400.0)), Some(a));
    }

    #[test]
    fn small_nodes_are_not_hit() {
        let mut net = Network::new();
        net.add_node(0.0, 0.0, 0.0, NodeFlag::REGULAR);
        // Zoom 1.5 gives ns 1.
        assert_eq!(hit(&net, 1.5, (500.0, 400.0)), None);
    }

    #[test]
    fn hit_set_follows_node_size() {
        let mut net = Network::new();
        let a = net.add_node(0.0, 0.0, 0.0, NodeFlag::REGULAR);
        // 4 px from the node: ns 3 (zoom 3), 5 (zoom 4) and 11 (zoom 10).
        for (zoom, expected) in [(3.0, None), (4.0, None), (10.0, Some(a))] {
            assert_eq!(hit(&net, zoom, (504.0, 400.0)), expected, "zoom {zoom}");
        }
        // 2.4 px: inside ns 5 (half 2.5) but not inside ns 3 (half 1.5).
        assert_eq!(hit(&net, 3.0, (502.4, 400.0)), None);
        assert_eq!(hit(&net, 4.0, (502.4, 400.0)), Some(a));
    }

    #[test]
    fn edge_hit_within_radius() {
        let mut net = Network::new();
        let a = net.add_node(-10.0, 0.0, 0.0, NodeFlag::REGULAR);
        let b = net.add_node(10.0, 0.0, 0.0, NodeFlag::REGULAR);
        net.connect(a, b, ConnectionShape::OneWay).expect("ok");
        let g = MapGeometry::new(1.0);
        let at = |c: Pt| edge_at(&net, g, &view(10.0), 11, None, c);
        assert_eq!(at((500.0, 403.0)), Some((a, b)));
        assert_eq!(at((500.0, 405.0)), None);
        // Past the end of the segment.
        assert_eq!(at((500.0 + 110.0, 400.0)), None);
    }

    #[test]
    fn edge_hit_picks_nearest() {
        let mut net = Network::new();
        let a = net.add_node(-10.0, 0.0, -0.3, NodeFlag::REGULAR);
        let b = net.add_node(10.0, 0.0, -0.3, NodeFlag::REGULAR);
        let c = net.add_node(-10.0, 0.0, 0.2, NodeFlag::REGULAR);
        let d = net.add_node(10.0, 0.0, 0.2, NodeFlag::REGULAR);
        net.connect(a, b, ConnectionShape::OneWay).expect("ok");
        net.connect(c, d, ConnectionShape::OneWay).expect("ok");
        // 3 px above and 2 px below the cursor.
        let got = edge_at(
            &net,
            MapGeometry::new(1.0),
            &view(10.0),
            11,
            None,
            (500.0, 400.0),
        );
        assert_eq!(got, Some((c, d)));
    }

    /// The node or, failing that, the connection under `cursor`, as the view resolves them.
    fn hit_any(net: &Network, zoom: f64, cursor: Pt) -> (Option<NodeId>, Option<(NodeId, NodeId)>) {
        let ns = crate::look::node_px(2.0, zoom);
        let node = hit(net, zoom, cursor);
        let edge = edge_at(net, MapGeometry::new(1.0), &view(zoom), ns, node, cursor);
        (node, edge)
    }

    #[test]
    fn a_node_under_the_cursor_wins_over_a_connection() {
        let mut net = Network::new();
        let a = net.add_node(-10.0, 0.0, 0.0, NodeFlag::REGULAR);
        let b = net.add_node(10.0, 0.0, 0.0, NodeFlag::REGULAR);
        // A node on the a-b connection, not linked to it.
        let c = net.add_node(0.0, 0.0, 0.0, NodeFlag::REGULAR);
        net.connect(a, b, ConnectionShape::OneWay).expect("ok");
        assert_eq!(hit_any(&net, 10.0, (500.0, 400.0)), (Some(c), None));
        // Beside the node, the connection is hit.
        assert_eq!(hit_any(&net, 10.0, (520.0, 400.0)), (None, Some((a, b))));
    }

    #[test]
    fn the_end_covered_by_a_node_hits_the_node() {
        let mut net = Network::new();
        let a = net.add_node(-10.0, 0.0, 0.0, NodeFlag::REGULAR);
        let b = net.add_node(10.0, 0.0, 0.0, NodeFlag::REGULAR);
        net.connect(a, b, ConnectionShape::OneWay).expect("ok");
        // Zoom 10 gives ns 11: a is at screen x 400 and its square ends at 405.5.
        assert_eq!(hit_any(&net, 10.0, (403.0, 400.0)), (Some(a), None));
        // The drawn part starts at 405.5.
        assert_eq!(hit_any(&net, 10.0, (406.0, 400.0)), (None, Some((a, b))));
    }

    #[test]
    fn only_the_drawn_part_of_a_connection_is_hit() {
        let mut net = Network::new();
        let a = net.add_node(-10.0, 0.0, 0.0, NodeFlag::REGULAR);
        let b = net.add_node(10.0, 0.0, 0.0, NodeFlag::REGULAR);
        net.connect(a, b, ConnectionShape::OneWay).expect("ok");
        let (g, t) = (MapGeometry::new(1.0), view(10.0));
        // With ns 21 and no node hit, the drawn part starts 10.5 px from a's centre at
        // x 400: beyond the 4 px reach there, within it at 407.
        assert_eq!(edge_at(&net, g, &t, 21, None, (400.0, 400.0)), None);
        assert_eq!(edge_at(&net, g, &t, 21, None, (407.0, 400.0)), Some((a, b)));
    }

    #[test]
    fn a_connection_too_short_to_draw_is_not_hit() {
        let mut net = Network::new();
        // 0.5 world units apart: 0.5 screen px at zoom 1, under MIN_EDGE_PX.
        let a = net.add_node(0.0, 0.0, 0.0, NodeFlag::REGULAR);
        let b = net.add_node(0.5, 0.0, 0.0, NodeFlag::REGULAR);
        net.connect(a, b, ConnectionShape::OneWay).expect("ok");
        let g = MapGeometry::new(1.0);
        assert_eq!(edge_at(&net, g, &view(1.0), 1, None, (500.2, 400.0)), None);
        // Zoomed in, the same connection is drawn and hit.
        assert_eq!(
            edge_at(&net, g, &view(10.0), 1, None, (502.5, 400.0)),
            Some((a, b))
        );
    }

    #[test]
    fn info_rows_follow_toggle_and_height() {
        let mut net = Network::new();
        let ok = net.add_node(0.0, 5.0, 0.0, NodeFlag::REGULAR);
        let bad = net.add_node(0.0, -1.0, 0.0, NodeFlag::REGULAR);
        let node = |id| net.node(id).expect("node");
        assert!(info_rows(node(ok), 1, false).is_empty());
        let rows = info_rows(node(ok), 7, true);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].text, "Node ID 7");
        assert_eq!(rows[0].background, Color32::WHITE);
        assert!(rows[0].centred);
        let rows = info_rows(node(bad), 2, true);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].text, "Node Y is invalid ( -1 )");
        assert_eq!(rows[1].background, Color32::RED);
        let rows = info_rows(node(bad), 2, false);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].background, Color32::RED);
    }

    #[test]
    fn box_stays_inside_panel() {
        let panel = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(300.0, 200.0));
        let size = Vec2::new(80.0, 30.0);
        let inside = box_rect(Pos2::new(50.0, 60.0), size, panel);
        assert_eq!(inside.min, Pos2::new(50.0, 60.0));
        let right = box_rect(Pos2::new(305.0, 60.0), size, panel);
        assert_eq!(right.max.x, 310.0);
        let bottom = box_rect(Pos2::new(50.0, 215.0), size, panel);
        assert_eq!(bottom.max.y, 220.0);
        let before = box_rect(Pos2::new(-5.0, 0.0), size, panel);
        assert_eq!(before.min, panel.min);
        for r in [inside, right, bottom, before] {
            assert!(panel.contains_rect(r));
        }
    }
}
