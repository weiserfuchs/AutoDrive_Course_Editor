//! The map view: map pixel <-> screen transform, culling, painting and input.

use std::borrow::Cow;

use eframe::egui;

use ad_core::geometry::{MAP_PIXELS, MapGeometry};
use ad_core::network::{ConnectionKind, EdgeSegment, Network, NodeId};

use crate::icons::{self, Icons};
use crate::lod::{HOVER_RING_FACTOR, MAX_ZOOM, MIN_NODE_PX, WHEEL_POINTS_PER_STEP, ZOOM_STEP};
use crate::{hover, look, overlay};

/// Screen px an icon or name reaches from its anchor; the cull rectangle grows by this.
const OVERLAY_PAD_PX: f64 = 60.0;
/// Marker name size in egui points.
const NAME_FONT_SIZE: f32 = 12.0;
/// Padding around the text of an info box row, in px.
const BOX_PAD_PX: f32 = 2.0;

/// The only map-pixel <-> screen conversion. Zoom is screen px per map px; the
/// centre is in map px. The panel is the screen rectangle the map is shown in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewTransform {
    pub zoom: f64,
    pub center: (f64, f64),
    panel_center: (f64, f64),
    panel_size: (f64, f64),
}

impl ViewTransform {
    /// A view of the map's centre at the minimum zoom of the panel.
    pub fn new(panel_center: (f64, f64), panel_size: (f64, f64)) -> Self {
        let mut v = Self {
            zoom: 0.0,
            center: (MAP_PIXELS / 2.0, MAP_PIXELS / 2.0),
            panel_center,
            panel_size,
        };
        v.clamp();
        v
    }

    /// Java minimum: the map always covers the panel.
    fn min_zoom(&self) -> f64 {
        (self.panel_size.0 / MAP_PIXELS).max(self.panel_size.1 / MAP_PIXELS)
    }

    /// Moves/resizes the panel, keeping zoom and centre valid.
    pub fn set_panel(&mut self, panel_center: (f64, f64), panel_size: (f64, f64)) {
        self.panel_center = panel_center;
        self.panel_size = panel_size;
        self.clamp();
    }

    fn clamp(&mut self) {
        let min = self.min_zoom();
        self.zoom = self.zoom.clamp(min, MAX_ZOOM.max(min));
        let axis = |c: f64, size: f64| {
            let half = size / 2.0 / self.zoom;
            if half * 2.0 >= MAP_PIXELS {
                MAP_PIXELS / 2.0
            } else {
                c.clamp(half, MAP_PIXELS - half)
            }
        };
        self.center = (
            axis(self.center.0, self.panel_size.0),
            axis(self.center.1, self.panel_size.1),
        );
    }

    pub fn map_to_screen(&self, mx: f64, my: f64) -> (f64, f64) {
        (
            self.panel_center.0 + (mx - self.center.0) * self.zoom,
            self.panel_center.1 + (my - self.center.1) * self.zoom,
        )
    }

    pub fn screen_to_map(&self, sx: f64, sy: f64) -> (f64, f64) {
        (
            self.center.0 + (sx - self.panel_center.0) / self.zoom,
            self.center.1 + (sy - self.panel_center.1) / self.zoom,
        )
    }

    /// Zooms by `ZOOM_STEP^steps` keeping the map point under `cursor` there.
    pub fn zoom_at(&mut self, cursor: (f64, f64), steps: f64) {
        let (mx, my) = self.screen_to_map(cursor.0, cursor.1);
        self.zoom = (self.zoom * ZOOM_STEP.powf(steps)).clamp(self.min_zoom(), MAX_ZOOM);
        self.center = (
            mx - (cursor.0 - self.panel_center.0) / self.zoom,
            my - (cursor.1 - self.panel_center.1) / self.zoom,
        );
        self.clamp();
    }

    /// Drags the content by (dx, dy) screen px.
    pub fn pan(&mut self, dx: f64, dy: f64) {
        self.center = (
            self.center.0 - dx / self.zoom,
            self.center.1 - dy / self.zoom,
        );
        self.clamp();
    }

    /// Map-pixel rectangle `(x0, y0, x1, y1)` the panel shows.
    pub fn visible_map_rect(&self) -> (f64, f64, f64, f64) {
        let hx = self.panel_size.0 / 2.0 / self.zoom;
        let hy = self.panel_size.1 / 2.0 / self.zoom;
        (
            self.center.0 - hx,
            self.center.1 - hy,
            self.center.0 + hx,
            self.center.1 + hy,
        )
    }
}

/// Screen rectangle of map pixels 0..2048.
fn image_rect(t: &ViewTransform) -> egui::Rect {
    let (x0, y0) = t.map_to_screen(0.0, 0.0);
    let (x1, y1) = t.map_to_screen(MAP_PIXELS, MAP_PIXELS);
    egui::Rect::from_min_max(
        egui::pos2(x0 as f32, y0 as f32),
        egui::pos2(x1 as f32, y1 as f32),
    )
}

/// Draws the network on the map. Owns the query buffers reused across frames.
pub struct MapView {
    transform: Option<ViewTransform>,
    nodes: Vec<NodeId>,
    edges: Vec<EdgeSegment>,
    /// Loaded on the first frame, once per app run.
    icons: Option<Icons>,
    pub show_grid: bool,
    pub show_marker_icons: bool,
    pub show_marker_names: bool,
    /// Session toggle for the "Node ID" row of the info box.
    pub show_node_id: bool,
    /// The node under the pointer, recomputed every frame.
    pub hovered_node: Option<NodeId>,
    /// The connection under the pointer, recomputed every frame; the tools epic reads it.
    #[allow(dead_code)]
    pub hovered_edge: Option<(NodeId, NodeId)>,
    /// Grid spacing in world units, from the map's settings.
    pub grid: (f64, f64),
}

impl Default for MapView {
    fn default() -> Self {
        Self {
            transform: None,
            nodes: Vec::new(),
            edges: Vec::new(),
            icons: None,
            show_grid: false,
            show_marker_icons: true,
            show_marker_names: true,
            show_node_id: false,
            hovered_node: None,
            hovered_edge: None,
            grid: (2.0, 2.0),
        }
    }
}

impl MapView {
    /// Forgets the view; the next frame starts at the minimum zoom, centred.
    pub fn reset(&mut self) {
        self.transform = None;
        self.clear_hover();
    }

    /// Nothing is hovered, e.g. while there is no document.
    pub fn clear_hover(&mut self) {
        self.hovered_node = None;
        self.hovered_edge = None;
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        net: &Network,
        geometry: MapGeometry,
        node_size: f64,
        map_texture: Option<egui::TextureId>,
        overlay_hook: impl FnOnce(&egui::Painter, &ViewTransform, MapGeometry),
    ) {
        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let rect = response.rect;
        if rect.width() < 1.0 || rect.height() < 1.0 {
            return;
        }
        let panel_center = (f64::from(rect.center().x), f64::from(rect.center().y));
        let panel_size = (f64::from(rect.width()), f64::from(rect.height()));
        let transform = self
            .transform
            .get_or_insert_with(|| ViewTransform::new(panel_center, panel_size));
        transform.set_panel(panel_center, panel_size);

        if response.dragged_by(egui::PointerButton::Primary)
            || response.dragged_by(egui::PointerButton::Middle)
        {
            let d = response.drag_delta();
            transform.pan(f64::from(d.x), f64::from(d.y));
        }
        if response.hovered() {
            let steps: f32 = ui.input(|i| {
                i.events
                    .iter()
                    .map(|e| match e {
                        egui::Event::MouseWheel { unit, delta, .. } => match unit {
                            egui::MouseWheelUnit::Point => delta.y / WHEEL_POINTS_PER_STEP,
                            _ => delta.y,
                        },
                        _ => 0.0,
                    })
                    .sum()
            });
            if let (true, Some(p)) = (steps != 0.0, response.hover_pos()) {
                transform.zoom_at((f64::from(p.x), f64::from(p.y)), f64::from(steps));
            }
        }
        let t = *transform;
        if self.icons.is_none() {
            self.icons = Some(Icons::load(ui.ctx()));
        }
        let ns = look::node_px(node_size, t.zoom);
        let cursor = response
            .hover_pos()
            .map(|p| (f64::from(p.x), f64::from(p.y)));
        self.hovered_node =
            cursor.and_then(|c| hover::node_at(net, geometry, &t, ns, c, &mut self.nodes));
        self.hovered_edge = cursor.and_then(|c| hover::edge_at(net, geometry, &t, c));
        self.paint(&painter, net, geometry, node_size, map_texture, &t);
        if let Some(id) = self.hovered_node {
            self.paint_info_box(&painter, net, geometry, &t, ns, id, rect);
        }
        // Last, so tools draw on top of everything.
        overlay_hook(&painter, &t, geometry);
    }

    /// The info box of the hovered node, beside it and inside the panel.
    #[allow(clippy::too_many_arguments)]
    fn paint_info_box(
        &self,
        painter: &egui::Painter,
        net: &Network,
        geometry: MapGeometry,
        t: &ViewTransform,
        ns: i32,
        id: NodeId,
        panel: egui::Rect,
    ) {
        let Some(node) = net.node(id) else {
            return;
        };
        // One lookup per frame, and only while the ID row is shown.
        let file_id = if self.show_node_id {
            net.order()
                .iter()
                .position(|&n| n == id)
                .map_or(0, |i| i + 1)
        } else {
            0
        };
        let rows = hover::info_rows(node, file_id, self.show_node_id);
        if rows.is_empty() {
            return;
        }
        let font = egui::FontId::proportional(NAME_FONT_SIZE);
        let galleys: Vec<_> = rows
            .iter()
            .map(|r| painter.layout_no_wrap(r.text.clone(), font.clone(), r.text_colour))
            .collect();
        let width = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max) + 2.0 * BOX_PAD_PX;
        let row_heights: Vec<f32> = galleys
            .iter()
            .map(|g| g.size().y + 2.0 * BOX_PAD_PX)
            .collect();
        let size = egui::vec2(width, row_heights.iter().sum());
        let (mx, my) = geometry.to_map(node.x, node.z);
        let (sx, sy) = t.map_to_screen(mx, my);
        let anchor = egui::pos2(sx as f32 + ns as f32 / 2.0, sy as f32);
        let boxed = hover::box_rect(anchor, size, panel);
        let mut y = boxed.min.y;
        for ((row, galley), height) in rows.iter().zip(galleys).zip(row_heights) {
            let rect =
                egui::Rect::from_min_size(egui::pos2(boxed.min.x, y), egui::vec2(width, height));
            painter.rect_filled(rect, 0.0, row.background);
            let text_x = if row.centred {
                rect.center().x - galley.size().x / 2.0
            } else {
                rect.min.x + BOX_PAD_PX
            };
            painter.galley(
                egui::pos2(text_x, rect.min.y + BOX_PAD_PX),
                galley,
                row.text_colour,
            );
            y += height;
        }
    }

    fn paint(
        &mut self,
        painter: &egui::Painter,
        net: &Network,
        geometry: MapGeometry,
        node_size: f64,
        map_texture: Option<egui::TextureId>,
        t: &ViewTransform,
    ) {
        if let Some(id) = map_texture {
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            painter.image(id, image_rect(t), uv, egui::Color32::WHITE);
        }
        let (mx0, my0, mx1, my1) = t.visible_map_rect();
        let (x0, z0) = geometry.to_world(mx0, my0);
        let (x1, z1) = geometry.to_world(mx1, my1);
        let to_screen = |x: f64, z: f64| {
            let (mx, my) = geometry.to_map(x, z);
            t.map_to_screen(mx, my)
        };

        if overlay::grid_visible(self.show_grid, t.zoom) {
            let [r, g, b] = overlay::GRID_RGB;
            let colour = egui::Color32::from_rgb(r, g, b);
            let mut mesh = egui::Mesh::default();
            let (top, bottom) = (to_screen(x0, z0).1 as f32, to_screen(x0, z1).1 as f32);
            let (left, right) = (to_screen(x0, z0).0 as f32, to_screen(x1, z0).0 as f32);
            let (sx_ok, sz_ok) = (
                overlay::grid_spacing_visible(self.grid.0, t.zoom, geometry.scale),
                overlay::grid_spacing_visible(self.grid.1, t.zoom, geometry.scale),
            );
            for x in
                overlay::grid_lines(self.grid.0, x0, x1).take(if sx_ok { usize::MAX } else { 0 })
            {
                let sx = to_screen(x, z0).0 as f32;
                let (a, b) = (egui::pos2(sx - 0.5, top), egui::pos2(sx + 0.5, bottom));
                mesh.add_colored_rect(egui::Rect::from_min_max(a, b), colour);
            }
            for z in
                overlay::grid_lines(self.grid.1, z0, z1).take(if sz_ok { usize::MAX } else { 0 })
            {
                let sy = to_screen(x0, z).1 as f32;
                let (a, b) = (egui::pos2(left, sy - 0.5), egui::pos2(right, sy + 0.5));
                mesh.add_colored_rect(egui::Rect::from_min_max(a, b), colour);
            }
            painter.add(egui::Shape::mesh(mesh));
        }

        let ns = look::node_px(node_size, t.zoom);
        let hovered = self.hovered_node;

        // All connections and arrowheads go into one mesh with per-vertex colour: one shape
        // instead of tens of thousands is what keeps the tessellation inside the frame budget.
        let half_width = 0.5_f32;
        let mut mesh = egui::Mesh::default();
        net.edges_in_rect_into(x0, z0, x1, z1, &mut self.edges);
        mesh.reserve_triangles(self.edges.len() * 3);
        mesh.reserve_vertices(self.edges.len() * 7);
        for e in &self.edges {
            let start = to_screen(e.a[0], e.a[1]);
            let end = to_screen(e.b[0], e.b[1]);
            let Some((from, to)) = look::line_ends(start, end, ns) else {
                continue;
            };
            let colour = look::connection_colour(e.look.kind, e.look.from_flag.0, e.look.to_flag.0);
            // The normal comes from the full line; shortening can cross the ends on short lines.
            let len = (end.0 - start.0).hypot(end.1 - start.1);
            let nx = (-(end.1 - start.1) / len) as f32 * half_width;
            let ny = ((end.0 - start.0) / len) as f32 * half_width;
            let (a, b) = (
                egui::pos2(from.0 as f32, from.1 as f32),
                egui::pos2(to.0 as f32, to.1 as f32),
            );
            let i = mesh.vertices.len() as u32;
            for p in [
                egui::pos2(a.x + nx, a.y + ny),
                egui::pos2(a.x - nx, a.y - ny),
                egui::pos2(b.x - nx, b.y - ny),
                egui::pos2(b.x + nx, b.y + ny),
            ] {
                mesh.colored_vertex(p, colour);
            }
            mesh.add_triangle(i, i + 1, i + 2);
            mesh.add_triangle(i, i + 2, i + 3);
            let dual = e.look.kind == ConnectionKind::Dual;
            if let Some(tri) = look::arrow(start, end, ns, node_size, t.zoom, dual) {
                let i = mesh.vertices.len() as u32;
                for p in tri {
                    mesh.colored_vertex(egui::pos2(p.0 as f32, p.1 as f32), colour);
                }
                mesh.add_triangle(i, i + 1, i + 2);
            }
        }
        painter.add(egui::Shape::mesh(mesh));

        // Widen by the largest overlay extent (dots, icons) so those just outside the
        // view still show.
        let radius = ns as f32 / 2.0;
        let pad_px = f64::from(radius).max(OVERLAY_PAD_PX);
        let pad = pad_px / t.zoom * geometry.scale;
        net.nodes_in_rect_into(x0 - pad, z0 - pad, x1 + pad, z1 + pad, &mut self.nodes);
        if f64::from(ns) >= MIN_NODE_PX {
            for &id in &self.nodes {
                if Some(id) == hovered {
                    continue;
                }
                if let Some(n) = net.node(id) {
                    let (sx, sy) = to_screen(n.x, n.z);
                    painter.circle_filled(
                        egui::pos2(sx as f32, sy as f32),
                        radius,
                        look::node_colour(n.flag.0),
                    );
                }
            }
            // On top of all other nodes, with a white ring.
            if let Some(n) = hovered.and_then(|id| net.node(id)) {
                let (sx, sy) = to_screen(n.x, n.z);
                let centre = egui::pos2(sx as f32, sy as f32);
                painter.circle_filled(centre, radius, look::node_colour(n.flag.0));
                painter.circle_stroke(
                    centre,
                    radius,
                    egui::Stroke::new(HOVER_RING_FACTOR * ns as f32, egui::Color32::WHITE),
                );
            }
        }

        // One mesh per icon texture: hundreds of single-image shapes cost far more to tessellate.
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        let add_icon = |mesh: &mut egui::Mesh, centre: (f32, f32), px: u32| {
            let rect = egui::Rect::from_center_size(
                egui::pos2(centre.0, centre.1),
                egui::Vec2::splat(px as f32),
            );
            mesh.add_rect_with_uv(rect, uv, egui::Color32::WHITE);
        };
        if let Some(tex) = self.icons.as_ref().and_then(|i| i.warning_y.as_ref()) {
            let mut mesh = egui::Mesh::with_texture(tex.id());
            for &id in &self.nodes {
                if let Some(n) = net.node(id)
                    && overlay::has_height_warning(n.y)
                {
                    let (sx, sy) = to_screen(n.x, n.z);
                    add_icon(&mut mesh, (sx as f32, sy as f32), icons::SMALL_PX);
                }
            }
            if !mesh.is_empty() {
                painter.add(egui::Shape::mesh(mesh));
            }
        }
        if self.show_marker_icons || self.show_marker_names || hovered.is_some() {
            let marker_tex = self
                .icons
                .as_ref()
                .and_then(|i| i.marker.as_ref())
                .filter(|_| self.show_marker_icons);
            let mut icon_mesh = marker_tex.map(|t| egui::Mesh::with_texture(t.id()));
            let mut names = Vec::new();
            let pad = OVERLAY_PAD_PX / t.zoom * geometry.scale;
            let (wx0, wz0, wx1, wz1) = (x0 - pad, z0 - pad, x1 + pad, z1 + pad);
            for m in net.markers() {
                let Some(n) = net.node(m.node) else {
                    continue;
                };
                if n.x < wx0 || n.x > wx1 || n.z < wz0 || n.z > wz1 {
                    continue;
                }
                let (ax, az) = overlay::marker_anchor(n.x, n.z);
                let anchor = to_screen(ax, az);
                if let Some(mesh) = &mut icon_mesh {
                    add_icon(mesh, overlay::marker_icon_centre(anchor), icons::MARKER_PX);
                }
                if Some(m.node) == hovered {
                    // A hovered marker always shows its name, with its group.
                    let text = Cow::Owned(format!("{} ( {} )", m.name, m.group));
                    names.push((text, overlay::name_centre(anchor)));
                } else if self.show_marker_names {
                    names.push((Cow::Borrowed(m.name.as_str()), overlay::name_centre(anchor)));
                }
            }
            if let Some(mesh) = icon_mesh.filter(|m| !m.is_empty()) {
                painter.add(egui::Shape::mesh(mesh));
            }
            let font = egui::FontId::proportional(NAME_FONT_SIZE);
            for (name, (cx, cy)) in names {
                painter.text(
                    egui::pos2(cx, cy),
                    egui::Align2::CENTER_CENTER,
                    name,
                    font.clone(),
                    egui::Color32::WHITE,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use ad_core::network::{ConnectionShape, NodeFlag};

    fn view() -> ViewTransform {
        ViewTransform::new((500.0, 400.0), (1000.0, 800.0))
    }

    #[test]
    fn starts_at_min_zoom_centred() {
        let v = view();
        assert!((v.zoom - 1000.0 / 2048.0).abs() < 1e-12);
        assert_eq!(v.center, (1024.0, 1024.0));
    }

    #[test]
    fn zoom_keeps_point_under_cursor() {
        let mut v = view();
        for (cursor, steps) in [
            ((500.0, 400.0), 1.0),
            ((120.0, 700.0), 2.0),
            ((900.0, 50.0), 3.0),
            ((300.0, 300.0), -1.0),
        ] {
            let before = v.screen_to_map(cursor.0, cursor.1);
            v.zoom_at(cursor, steps);
            let after = v.screen_to_map(cursor.0, cursor.1);
            assert!((before.0 - after.0).abs() < 1e-6 && (before.1 - after.1).abs() < 1e-6);
        }
    }

    #[test]
    fn zoom_is_clamped() {
        let mut v = view();
        v.zoom_at((500.0, 400.0), 100.0);
        assert_eq!(v.zoom, MAX_ZOOM);
        v.zoom_at((500.0, 400.0), -100.0);
        assert!((v.zoom - 1000.0 / 2048.0).abs() < 1e-12);
    }

    #[test]
    fn pan_moves_centre_and_clamps() {
        let mut v = view();
        v.zoom_at((500.0, 400.0), 4.0);
        let c = v.center;
        v.pan(10.0, -20.0);
        assert!((v.center.0 - (c.0 - 10.0 / v.zoom)).abs() < 1e-9);
        assert!((v.center.1 - (c.1 + 20.0 / v.zoom)).abs() < 1e-9);
        v.pan(1e9, 1e9);
        let (x0, y0, _, _) = v.visible_map_rect();
        assert!(x0.abs() < 1e-9 && y0.abs() < 1e-9);
        v.pan(-1e9, -1e9);
        let (_, _, x1, y1) = v.visible_map_rect();
        assert!((x1 - 2048.0).abs() < 1e-9 && (y1 - 2048.0).abs() < 1e-9);
    }

    #[test]
    fn screen_map_round_trip() {
        let mut v = view();
        v.zoom_at((200.0, 200.0), 3.0);
        let (sx, sy) = v.map_to_screen(900.0, 1100.0);
        let (mx, my) = v.screen_to_map(sx, sy);
        assert!((mx - 900.0).abs() < 1e-9 && (my - 1100.0).abs() < 1e-9);
    }

    /// Runs one frame of `MapView` and returns what it painted.
    fn frame(
        ctx: &egui::Context,
        view: &mut MapView,
        net: &Network,
        geometry: MapGeometry,
        size: egui::Vec2,
        texture: Option<egui::TextureId>,
    ) -> egui::FullOutput {
        frame_sized(ctx, view, net, geometry, 2.0, size, texture)
    }

    fn frame_sized(
        ctx: &egui::Context,
        view: &mut MapView,
        net: &Network,
        geometry: MapGeometry,
        node_size: f64,
        size: egui::Vec2,
        texture: Option<egui::TextureId>,
    ) -> egui::FullOutput {
        frame_hover(ctx, view, net, geometry, node_size, size, texture, None)
    }

    /// One frame with the pointer at `pointer`. egui sees a widget as hovered one frame
    /// after the pointer arrives, so hover tests run two frames.
    #[allow(clippy::too_many_arguments)]
    fn frame_hover(
        ctx: &egui::Context,
        view: &mut MapView,
        net: &Network,
        geometry: MapGeometry,
        node_size: f64,
        size: egui::Vec2,
        texture: Option<egui::TextureId>,
        pointer: Option<egui::Pos2>,
    ) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events: pointer.map(egui::Event::PointerMoved).into_iter().collect(),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            view.show(ui, net, geometry, node_size, texture, |_, _, _| {});
        });
        // No renderer here, so there is nothing to upload.
        out.textures_delta.clear();
        out
    }

    /// Two frames with the pointer at `pointer`; returns the second.
    fn hover_frame(
        ctx: &egui::Context,
        view: &mut MapView,
        net: &Network,
        pointer: Option<egui::Pos2>,
    ) -> egui::FullOutput {
        let size = egui::vec2(1024.0, 1024.0);
        let g = MapGeometry::new(1.0);
        let _ = frame_hover(ctx, view, net, g, 2.0, size, None, pointer);
        frame_hover(ctx, view, net, g, 2.0, size, None, pointer)
    }

    /// Counts painted (connection quads, circles). Arrowheads are triangles in the mesh.
    fn count(out: &egui::FullOutput) -> (usize, usize) {
        let (lines, _, circles) = count3(out);
        (lines, circles)
    }

    /// Counts painted (connection quads, arrowheads, circles).
    fn count3(out: &egui::FullOutput) -> (usize, usize, usize) {
        let (mut verts, mut tris, mut circles) = (0, 0, 0);
        for s in &out.shapes {
            let mut stack = vec![&s.shape];
            while let Some(shape) = stack.pop() {
                match shape {
                    egui::Shape::Mesh(m) => {
                        verts += m.vertices.len();
                        tris += m.indices.len() / 3;
                    }
                    egui::Shape::Circle(_) => circles += 1,
                    egui::Shape::Vec(v) => stack.extend(v.iter()),
                    _ => {}
                }
            }
        }
        // Quad = 4 vertices and 2 triangles, arrowhead = 3 and 1.
        let lines = (3 * tris - verts) / 2;
        (lines, tris - 2 * lines, circles)
    }

    #[test]
    fn image_rect_matches_map_pixels() {
        let mut v = view();
        v.zoom_at((200.0, 300.0), 3.0);
        let r = image_rect(&v);
        let (x0, y0) = v.map_to_screen(0.0, 0.0);
        let (x1, y1) = v.map_to_screen(2048.0, 2048.0);
        assert_eq!((r.min.x, r.min.y), (x0 as f32, y0 as f32));
        assert_eq!((r.max.x, r.max.y), (x1 as f32, y1 as f32));
        // World (0,0) at scale 2 is map px (1024,1024): the middle of the image rect.
        let (mx, my) = MapGeometry::new(2.0).to_map(0.0, 0.0);
        let (sx, sy) = v.map_to_screen(mx, my);
        assert!((r.center().x - sx as f32).abs() < 1e-3);
        assert!((r.center().y - sy as f32).abs() < 1e-3);
    }

    #[test]
    fn zero_size_panel_does_not_panic() {
        let ctx = egui::Context::default();
        let mut view = MapView::default();
        let net = Network::new();
        let out = frame(
            &ctx,
            &mut view,
            &net,
            MapGeometry::new(1.0),
            egui::Vec2::ZERO,
            None,
        );
        assert_eq!(count(&out), (0, 0));
    }

    #[test]
    fn culling_and_lod() {
        let mut net = Network::new();
        let mut add = |x: f64, z: f64| net.add_node(x, 0.0, z, NodeFlag::REGULAR);
        // Scale 1: map px = world + 1024.
        let near = add(-1000.0, -1000.0);
        let near2 = add(-990.0, -1000.0);
        let far_a = add(1000.0, 1000.0);
        let far_b = add(1000.0, 990.0);
        let cross_a = add(-1000.0, 500.0);
        let cross_b = add(1000.0, 500.0);
        for (a, b) in [(near, near2), (far_a, far_b), (cross_a, cross_b)] {
            net.connect(a, b, ConnectionShape::OneWay).expect("ok");
        }
        let ctx = egui::Context::default();
        let g = MapGeometry::new(1.0);
        let mut view = MapView::default();
        let size = egui::vec2(1024.0, 1024.0);

        // Minimum zoom 0.5: nodes are 0.5 px (no dots), all three edges are visible.
        let out = frame(&ctx, &mut view, &net, g, size, None);
        assert_eq!(count(&out), (3, 0));

        // Zoomed into the top-left corner: only the near edge and its two nodes.
        view.transform
            .as_mut()
            .expect("set")
            .zoom_at((0.0, 0.0), 8.0);
        let out = frame(&ctx, &mut view, &net, g, size, None);
        assert_eq!(count(&out), (1, 2));
        // Node size 2 at zoom 30 gives 31 px nodes; the one-way edge has one arrowhead.
        assert_eq!(count3(&out), (1, 1, 2));

        // A view of the crossing edge's middle, with both ends outside it.
        view.transform.as_mut().expect("set").center = (1024.0, 1524.0);
        let out = frame(&ctx, &mut view, &net, g, size, None);
        assert_eq!(count(&out), (1, 0));

        // An edge shorter than 1 px on screen is skipped (1 map px at zoom 0.5).
        let mut tiny = Network::new();
        let a = tiny.add_node(0.0, 0.0, 0.0, NodeFlag::REGULAR);
        let b = tiny.add_node(1.0, 0.0, 0.0, NodeFlag::REGULAR);
        tiny.connect(a, b, ConnectionShape::OneWay).expect("ok");
        let mut view = MapView::default();
        let out = frame(&ctx, &mut view, &tiny, g, size, None);
        assert_eq!(count(&out), (0, 0));
    }

    #[test]
    fn colours_arrows_and_node_size() {
        let mut net = Network::new();
        let a = net.add_node(-1000.0, -1000.0, -1000.0, NodeFlag::REGULAR);
        let b = net.add_node(-900.0, 0.0, -1000.0, NodeFlag::SUB_PRIO);
        let c = net.add_node(-1000.0, 0.0, -900.0, NodeFlag::REGULAR);
        net.connect(a, b, ConnectionShape::Dual).expect("ok");
        net.connect(a, c, ConnectionShape::OneWay).expect("ok");
        let ctx = egui::Context::default();
        let g = MapGeometry::new(1.0);
        let size = egui::vec2(1024.0, 1024.0);
        let mut view = MapView::default();
        let _ = frame(&ctx, &mut view, &net, g, size, None);
        view.transform
            .as_mut()
            .expect("set")
            .zoom_at((0.0, 0.0), 4.0);
        let out = frame(&ctx, &mut view, &net, g, size, None);
        // Dual: two entries with an arrowhead each; one-way: one.
        assert_eq!(count3(&out), (3, 3, 3));
        let colours: Vec<egui::Color32> = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Mesh(m) => {
                    Some(m.vertices.iter().map(|v| v.color).collect::<Vec<_>>())
                }
                _ => None,
            })
            .flatten()
            .collect();
        // Dual with a subprio end is brown; the one-way edge from a regular node is green.
        assert!(colours.contains(&look::DUAL_SUBPRIO) && colours.contains(&look::REGULAR));
        // Nodes: red for flag 0, orange for the subprio node.
        let mut node_colours: Vec<_> = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Circle(c) => Some(c.fill),
                _ => None,
            })
            .collect();
        node_colours.sort_by_key(|c| c.to_array());
        assert_eq!(node_colours.len(), 3);
        assert!(node_colours.contains(&look::NODE_REGULAR));
        assert!(node_colours.contains(&look::NODE_SUBPRIO));

        // At zoom 3.28 node size 2 gives 3 px nodes (circles and arrowheads); size 1 gives
        // 1 px (neither), so the size from the settings is what decides.
        let counts = |node_size: f64| {
            let mut v = MapView::default();
            let _ = frame_sized(&ctx, &mut v, &net, g, node_size, size, None);
            let t = v.transform.as_mut().expect("set");
            t.zoom = 3.28;
            t.center = (160.0, 160.0);
            let (_, arrows, circles) =
                count3(&frame_sized(&ctx, &mut v, &net, g, node_size, size, None));
            (arrows, circles)
        };
        assert_eq!(counts(2.0), (3, 3));
        assert_eq!(counts(1.0), (0, 0));
    }

    /// Painted (icon rects, name texts with their centres), in draw order.
    fn overlay_shapes(out: &egui::FullOutput) -> (Vec<egui::Rect>, Vec<(String, egui::Pos2)>) {
        let (mut icons, mut texts) = (Vec::new(), Vec::new());
        for s in &out.shapes {
            match &s.shape {
                egui::Shape::Mesh(m) if m.texture_id != egui::TextureId::default() => {
                    for quad in m.vertices.chunks(4) {
                        let mut rect = egui::Rect::NOTHING;
                        for v in quad {
                            rect.extend_with(v.pos);
                        }
                        icons.push(rect);
                    }
                }
                egui::Shape::Text(t) => {
                    texts.push((t.galley.text().to_owned(), t.pos + t.galley.size() / 2.0));
                }
                _ => {}
            }
        }
        (icons, texts)
    }

    /// A view at zoom `zoom`, centred on the map; the first frame creates the transform.
    fn view_at(ctx: &egui::Context, net: &Network, g: MapGeometry, zoom: f64) -> MapView {
        let mut view = MapView::default();
        let _ = frame(ctx, &mut view, net, g, egui::vec2(1024.0, 1024.0), None);
        view.transform.as_mut().expect("set").zoom = zoom;
        view
    }

    #[test]
    fn height_warning_icon() {
        let ctx = egui::Context::default();
        let g = MapGeometry::new(1.0);
        let size = egui::vec2(1024.0, 1024.0);
        for (y, shown) in [(-1.0, 1), (-1.0005, 0), (0.0, 0)] {
            let mut net = Network::new();
            net.add_node(10.0, y, 20.0, NodeFlag::REGULAR);
            let mut view = view_at(&ctx, &net, g, 1.0);
            let out = frame(&ctx, &mut view, &net, g, size, None);
            let (icons, texts) = overlay_shapes(&out);
            assert_eq!((icons.len(), texts.len()), (shown, 0), "y = {y}");
            if shown == 1 {
                let t = view.transform.expect("set");
                let (mx, my) = g.to_map(10.0, 20.0);
                let (sx, sy) = t.map_to_screen(mx, my);
                let c = icons[0].center();
                assert!((c.x - sx as f32).abs() < 1e-3 && (c.y - sy as f32).abs() < 1e-3);
                assert_eq!(icons[0].size(), egui::vec2(20.0, 20.0));
            }
        }
    }

    #[test]
    fn marker_icon_and_name_placement_and_toggles() {
        let ctx = egui::Context::default();
        let g = MapGeometry::new(1.0);
        let size = egui::vec2(1024.0, 1024.0);
        let mut net = Network::new();
        let id = net.add_node(100.0, 5.0, 200.0, NodeFlag::REGULAR);
        net.set_marker(id, "Hof".to_owned(), "All".to_owned())
            .expect("marker");
        let far = net.add_node(-1000.0, 5.0, -1000.0, NodeFlag::REGULAR);
        net.set_marker(far, "Weit".to_owned(), "All".to_owned())
            .expect("marker");
        let mut view = view_at(&ctx, &net, g, 1.0);
        view.transform.as_mut().expect("set").center = (1124.0, 1224.0);
        let t = view.transform.expect("set");
        let (mx, my) = g.to_map(100.0, 199.0);
        let (ax, ay) = t.map_to_screen(mx, my);
        let (ax, ay) = (ax as f32, ay as f32);

        let out = frame(&ctx, &mut view, &net, g, size, None);
        let (icons, texts) = overlay_shapes(&out);
        // The far marker is outside the view.
        assert_eq!((icons.len(), texts.len()), (1, 1));
        assert_eq!(icons[0].size(), egui::vec2(25.0, 25.0));
        assert!((icons[0].center().x - ax).abs() < 1e-3);
        assert!((icons[0].center().y - (ay - 20.0)).abs() < 1e-3);
        assert_eq!(texts[0].0, "Hof");
        assert!((texts[0].1.x - ax).abs() < 1.0 && (texts[0].1.y - (ay - 3.0)).abs() < 1.0);

        view.show_marker_names = false;
        let (icons, texts) = overlay_shapes(&frame(&ctx, &mut view, &net, g, size, None));
        assert_eq!((icons.len(), texts.len()), (1, 0));
        view.show_marker_names = true;
        view.show_marker_icons = false;
        let (icons, texts) = overlay_shapes(&frame(&ctx, &mut view, &net, g, size, None));
        assert_eq!((icons.len(), texts.len()), (0, 1));
    }

    /// Two nodes on a view at zoom 10 (ns 11): `a` at the panel centre (512, 512), `b` 200 px right.
    fn hover_net() -> (Network, NodeId, NodeId) {
        let mut net = Network::new();
        let a = net.add_node(0.0, -1.0, 0.0, NodeFlag::REGULAR);
        let b = net.add_node(20.0, 5.0, 0.0, NodeFlag::SUB_PRIO);
        (net, a, b)
    }

    #[test]
    fn hovered_node_is_drawn_last_with_ring() {
        let ctx = egui::Context::default();
        let (net, a, _) = hover_net();
        let mut view = view_at(&ctx, &net, MapGeometry::new(1.0), 10.0);
        let out = hover_frame(&ctx, &mut view, &net, Some(egui::pos2(513.0, 512.0)));
        assert_eq!(view.hovered_node, Some(a));
        let circles: Vec<_> = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Circle(c) => Some(*c),
                _ => None,
            })
            .collect();
        // The other node, then the hovered node on top, then its ring.
        assert_eq!(circles.len(), 3);
        assert_eq!(circles[0].fill, look::NODE_SUBPRIO);
        assert_eq!(circles[1].fill, look::NODE_REGULAR);
        assert_eq!(circles[2].stroke.color, egui::Color32::WHITE);
        assert!((circles[2].stroke.width - 1.1).abs() < 1e-5);
        assert_eq!(circles[2].radius, 5.5);
    }

    #[test]
    fn no_pointer_means_nothing_hovered() {
        let ctx = egui::Context::default();
        let (net, _, _) = hover_net();
        let mut view = view_at(&ctx, &net, MapGeometry::new(1.0), 10.0);
        view.show_node_id = true;
        for pointer in [None, Some(egui::pos2(5000.0, 5000.0))] {
            let out = hover_frame(&ctx, &mut view, &net, pointer);
            assert_eq!(view.hovered_node, None);
            assert_eq!(view.hovered_edge, None);
            assert_eq!(count(&out).1, 2, "no ring");
            assert!(overlay_shapes(&out).1.is_empty(), "no box");
        }
        let empty = Network::new();
        let _ = hover_frame(&ctx, &mut view, &empty, Some(egui::pos2(512.0, 512.0)));
        assert_eq!(view.hovered_node, None);
        assert_eq!(view.hovered_edge, None);
        // Cleared state: a hovered node and edge are forgotten without a frame.
        let (net, a, b) = hover_net();
        let mut net = net;
        net.connect(a, b, ConnectionShape::OneWay).expect("ok");
        let _ = hover_frame(&ctx, &mut view, &net, Some(egui::pos2(512.0, 512.0)));
        assert_eq!(view.hovered_node, Some(a));
        view.clear_hover();
        assert_eq!((view.hovered_node, view.hovered_edge), (None, None));
    }

    #[test]
    fn info_box_rows() {
        let ctx = egui::Context::default();
        let (net, _, b) = hover_net();
        let mut view = view_at(&ctx, &net, MapGeometry::new(1.0), 10.0);
        let on_a = Some(egui::pos2(512.0, 512.0));
        // y = -1 with the toggle off: only the red row.
        let texts = overlay_shapes(&hover_frame(&ctx, &mut view, &net, on_a)).1;
        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0].0, "Node Y is invalid ( -1 )");
        view.show_node_id = true;
        let texts = overlay_shapes(&hover_frame(&ctx, &mut view, &net, on_a)).1;
        let strings: Vec<_> = texts.iter().map(|t| t.0.as_str()).collect();
        assert_eq!(strings, ["Node ID 1", "Node Y is invalid ( -1 )"]);
        // The box starts ns/2 right of the node.
        assert!(texts[0].1.x > 512.0 + 5.5);
        // y = 5, toggle off: no box at all.
        view.show_node_id = false;
        let on_b = Some(egui::pos2(712.0, 512.0));
        let out = hover_frame(&ctx, &mut view, &net, on_b);
        assert_eq!(view.hovered_node, Some(b));
        assert!(overlay_shapes(&out).1.is_empty());
        // The second node in the file is "Node ID 2".
        view.show_node_id = true;
        let texts = overlay_shapes(&hover_frame(&ctx, &mut view, &net, on_b)).1;
        assert_eq!(texts[0].0, "Node ID 2");
    }

    #[test]
    fn hovered_marker_shows_name_and_group_even_with_names_off() {
        let ctx = egui::Context::default();
        let (mut net, a, _) = hover_net();
        net.set_marker(a, "Hof".to_owned(), "Farm".to_owned())
            .expect("marker");
        let mut view = view_at(&ctx, &net, MapGeometry::new(1.0), 10.0);
        view.show_marker_names = false;
        view.show_marker_icons = false;
        let out = hover_frame(&ctx, &mut view, &net, None);
        assert!(overlay_shapes(&out).1.is_empty());
        let out = hover_frame(&ctx, &mut view, &net, Some(egui::pos2(512.0, 512.0)));
        let texts = overlay_shapes(&out).1;
        assert!(texts.iter().any(|t| t.0 == "Hof ( Farm )"), "{texts:?}");
        // With names on, the hovered marker still has the group; no plain name beside it.
        view.show_marker_names = true;
        let texts = overlay_shapes(&hover_frame(
            &ctx,
            &mut view,
            &net,
            Some(egui::pos2(512.0, 512.0)),
        ))
        .1;
        assert!(texts.iter().all(|t| t.0 != "Hof"));
        assert!(texts.iter().any(|t| t.0 == "Hof ( Farm )"));
    }

    #[test]
    fn overlay_hook_paints_last() {
        let ctx = egui::Context::default();
        let (net, _, _) = hover_net();
        let mut view = view_at(&ctx, &net, MapGeometry::new(1.0), 10.0);
        let marker = egui::Rect::from_min_size(egui::pos2(1.0, 2.0), egui::vec2(3.0, 4.0));
        let mut seen = None;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1024.0, 1024.0),
            )),
            events: vec![egui::Event::PointerMoved(egui::pos2(512.0, 512.0))],
            ..Default::default()
        };
        let g = MapGeometry::new(1.0);
        let mut out = ctx.run_ui(input, |ui| {
            view.show(ui, &net, g, 2.0, None, |painter, t, geometry| {
                seen = Some((t.zoom, geometry));
                painter.rect_filled(marker, 0.0, egui::Color32::YELLOW);
            });
        });
        out.textures_delta.clear();
        assert_eq!(seen, Some((10.0, g)));
        let last = out.shapes.last().expect("shapes");
        assert!(matches!(&last.shape, egui::Shape::Rect(r) if r.rect == marker));
    }

    /// Vertices of the meshes painted in the grid colour.
    fn grid_vertices(out: &egui::FullOutput) -> usize {
        let [r, g, b] = overlay::GRID_RGB;
        let grid = egui::Color32::from_rgb(r, g, b);
        out.shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Mesh(m) if m.vertices.iter().all(|v| v.color == grid) => {
                    Some(m.vertices.len())
                }
                _ => None,
            })
            .sum()
    }

    #[test]
    fn grid_needs_toggle_and_zoom() {
        let ctx = egui::Context::default();
        let g = MapGeometry::new(1.0);
        let size = egui::vec2(1024.0, 1024.0);
        let net = Network::new();
        let mut view = view_at(&ctx, &net, g, 3.0);
        assert_eq!(
            grid_vertices(&frame(&ctx, &mut view, &net, g, size, None)),
            0
        );
        view.show_grid = true;
        // 1024 px at zoom 3 show about 341 world units: 170 or 171 lines per axis.
        let n = grid_vertices(&frame(&ctx, &mut view, &net, g, size, None));
        assert!((4 * 2 * 170..=4 * 2 * 172).contains(&n), "{n}");
        view.grid = (4.0, 4.0);
        let n4 = grid_vertices(&frame(&ctx, &mut view, &net, g, size, None));
        assert!(n4 * 2 <= n + 16 && n4 * 2 + 16 >= n, "{n4} vs {n}");
        view.transform.as_mut().expect("set").zoom = 2.0;
        assert_eq!(
            grid_vertices(&frame(&ctx, &mut view, &net, g, size, None)),
            0
        );
    }

    /// Release timing of the view path on the fixture. Run by hand:
    /// `cargo test -p ad-app --release -- --ignored frame_time --nocapture`
    #[test]
    #[ignore = "timing; run by hand in release"]
    fn frame_time() {
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../AutoDrive_config.xml"
        ))
        .expect("fixture");
        let cfg = ad_io::config_xml::load(&bytes).expect("loads");
        let net = &cfg.network;
        let geometry = MapGeometry::new(2.0);
        let size = egui::vec2(1920.0, 1080.0);
        let ctx = egui::Context::default();
        let texture = ctx
            .load_texture(
                "map_image",
                egui::ColorImage::filled([2048, 2048], egui::Color32::GRAY),
                egui::TextureOptions::LINEAR,
            )
            .id();

        // The densest 256x256 world-unit cell of a coarse grid over the map.
        let mut dense_world = (0.0, 0.0);
        let mut best = 0;
        let mut ids = Vec::new();
        for gx in 0..16 {
            for gz in 0..16 {
                let cx = -2048.0 + 256.0 * (f64::from(gx) + 0.5);
                let cz = -2048.0 + 256.0 * (f64::from(gz) + 0.5);
                net.nodes_in_rect_into(cx - 128.0, cz - 128.0, cx + 128.0, cz + 128.0, &mut ids);
                if ids.len() > best {
                    best = ids.len();
                    dense_world = (cx, cz);
                }
            }
        }
        let dense = geometry.to_map(dense_world.0, dense_world.1);
        println!("{} nodes; densest cell {best} nodes", net.order().len());

        let run = |name: &str, steps: f64, at: (f64, f64), grid: bool, hover: bool| {
            let mut view = MapView {
                show_grid: grid,
                ..MapView::default()
            };
            // The first frame creates the transform at minimum zoom.
            let _ = frame(&ctx, &mut view, net, geometry, size, Some(texture));
            let t = view.transform.as_mut().expect("set");
            t.center = at;
            t.zoom_at((960.0, 540.0), steps);
            // With `hover`, the pointer rests on the lowest-id node within 300 px of the panel centre.
            let pointer = hover.then(|| {
                let t = *view.transform.as_ref().expect("set");
                let ns = look::node_px(2.0, t.zoom);
                let mut ids = Vec::new();
                let (cx, cz) = {
                    let (mx, my) = t.screen_to_map(960.0, 540.0);
                    geometry.to_world(mx, my)
                };
                let reach = 300.0 / t.zoom * geometry.scale;
                net.nodes_in_rect_into(cx - reach, cz - reach, cx + reach, cz + reach, &mut ids);
                ids.sort();
                let id = ids.first().expect("a node near the centre");
                let n = net.node(*id).expect("node");
                let (mx, my) = geometry.to_map(n.x, n.z);
                let (sx, sy) = t.map_to_screen(mx, my);
                assert!(ns >= 2);
                egui::pos2(sx as f32, sy as f32)
            });
            let mut times = Vec::new();
            for i in 0..40 {
                let start = std::time::Instant::now();
                let out = frame_hover(
                    &ctx,
                    &mut view,
                    net,
                    geometry,
                    2.0,
                    size,
                    Some(texture),
                    pointer,
                );
                if hover && i >= 2 {
                    assert!(view.hovered_node.is_some());
                }
                let _prims = ctx.tessellate(out.shapes, out.pixels_per_point);
                if i >= 10 {
                    times.push(start.elapsed().as_secs_f64() * 1000.0);
                }
            }
            times.sort_by(|a, b| a.total_cmp(b));
            println!(
                "{name}: median {:.3} ms, max {:.3} ms",
                times[times.len() / 2],
                times[times.len() - 1]
            );
        };
        run("min zoom, centre", -100.0, (1024.0, 1024.0), false, false);
        run("mid zoom, dense", 6.0, dense, false, false);
        run("max zoom, centre", 100.0, (1024.0, 1024.0), false, false);
        run("max zoom, dense", 100.0, dense, false, false);
        run("max zoom, dense, grid", 100.0, dense, true, false);
        run("mid zoom, dense, grid", 6.0, dense, true, false);
        run("max zoom, dense, hovered node", 100.0, dense, false, true);
    }
}
