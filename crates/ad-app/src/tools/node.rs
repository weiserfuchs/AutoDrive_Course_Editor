//! The node tool: a primary click adds one unconnected node.

use eframe::egui;

use ad_core::command::{AddNodes, Command, Transaction};
use ad_core::geometry::MapGeometry;
use ad_core::heightmap::sample_height;
use ad_core::network::NodeFlag;

use super::{Tool, ToolContext, snap_to_grid};
use crate::look;
use crate::view::ViewTransform;

/// Radius of the preview dot at the cursor, in screen px.
const PREVIEW_RADIUS_PX: f32 = 3.0;

pub const REGULAR_NAME: &str = "Node";
pub const SUB_PRIO_NAME: &str = "Sub-prio node";

/// Adds nodes with `flag`: regular or sub-priority.
pub struct NodeTool {
    pub flag: NodeFlag,
}

impl NodeTool {
    pub fn new(flag: NodeFlag) -> Self {
        Self { flag }
    }
}

/// The node tool the node shortcut selects (Java cycles the add-node group): sub-priority
/// after the regular node tool, the regular one otherwise.
pub fn cycled(active: Option<&dyn Tool>) -> NodeTool {
    if active.map(Tool::name) == Some(REGULAR_NAME) {
        NodeTool::new(NodeFlag::SUB_PRIO)
    } else {
        NodeTool::new(NodeFlag::REGULAR)
    }
}

/// Screen position of the preview dot for the pointer at `cursor`: the grid point a click
/// there would use when `snap_grid` is set, the pointer itself otherwise.
fn preview_at(
    cursor: egui::Pos2,
    t: &ViewTransform,
    geometry: MapGeometry,
    snap_grid: Option<(f64, f64)>,
) -> egui::Pos2 {
    let Some(grid) = snap_grid else {
        return cursor;
    };
    let (mx, my) = t.screen_to_map(f64::from(cursor.x), f64::from(cursor.y));
    let (x, z) = snap_to_grid(geometry.to_world(mx, my), grid);
    let (mx, my) = geometry.to_map(x, z);
    let (sx, sy) = t.map_to_screen(mx, my);
    egui::pos2(sx as f32, sy as f32)
}

impl Tool for NodeTool {
    fn name(&self) -> &'static str {
        if self.flag == NodeFlag::SUB_PRIO {
            SUB_PRIO_NAME
        } else {
            REGULAR_NAME
        }
    }

    fn on_click(&mut self, ctx: &ToolContext) -> Option<Transaction> {
        let (x, z) = if ctx.snap {
            snap_to_grid((ctx.x, ctx.z), ctx.map.grid)
        } else {
            (ctx.x, ctx.z)
        };
        // The file sentinel for "no height" (AD-7).
        let y = sample_height(ctx.map, x, z).unwrap_or(-1.0);
        Some(Transaction::new(vec![Command::AddNodes(AddNodes::new(
            vec![(x, y, z, self.flag)],
        ))]))
    }

    fn paint(
        &self,
        painter: &egui::Painter,
        t: &ViewTransform,
        geometry: MapGeometry,
        cursor: Option<egui::Pos2>,
        snap_grid: Option<(f64, f64)>,
    ) {
        if let Some(p) = cursor {
            let p = preview_at(p, t, geometry, snap_grid);
            painter.circle_filled(p, PREVIEW_RADIUS_PX, look::node_colour(self.flag.0));
        }
    }

    fn cancel(&mut self) {}
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::sync::Arc;

    use ad_core::heightmap::Heightmap;
    use ad_core::map_context::MapContext;
    use ad_core::network::Network;

    use super::*;

    /// Applies the tool's transaction for a click at (x, z) and returns the new node.
    fn click(tool: &mut NodeTool, map: &MapContext, x: f64, z: f64) -> (f64, f64, f64, NodeFlag) {
        click_snapped(tool, map, x, z, false)
    }

    fn click_snapped(
        tool: &mut NodeTool,
        map: &MapContext,
        x: f64,
        z: f64,
        snap: bool,
    ) -> (f64, f64, f64, NodeFlag) {
        let mut net = Network::new();
        let mut tx = tool
            .on_click(&ToolContext {
                x,
                z,
                map,
                net: &net,
                snap,
            })
            .expect("a transaction");
        tx.apply(&mut net).expect("applies");
        assert_eq!(net.len(), 1);
        let n = net.node(net.order()[0]).expect("node");
        (n.x, n.y, n.z, n.flag)
    }

    #[test]
    fn add_with_heightmap_samples_the_height() {
        let mut map = MapContext::new(None, 1, 2.0, (2.0, 2.0));
        // 2x2 heightmap: every sample is 10 m (raw 2560).
        map.heightmap = Some(Arc::new(Heightmap::new(2, 2, vec![2560; 4]).expect("size")));
        map.height_map_scale = 2.0;
        let mut tool = NodeTool::new(NodeFlag::REGULAR);
        assert_eq!(
            click(&mut tool, &map, 12.5, -40.125),
            (12.5, 5.0, -40.125, NodeFlag::REGULAR)
        );
    }

    #[test]
    fn add_without_heightmap_writes_minus_one() {
        let map = MapContext::new(None, 1, 2.0, (2.0, 2.0));
        let mut tool = NodeTool::new(NodeFlag::REGULAR);
        assert_eq!(
            click(&mut tool, &map, 3.0, 4.0),
            (3.0, -1.0, 4.0, NodeFlag::REGULAR)
        );
    }

    #[test]
    fn sub_priority_tool_adds_sub_prio_nodes() {
        let map = MapContext::new(None, 1, 2.0, (2.0, 2.0));
        let mut tool = NodeTool::new(NodeFlag::SUB_PRIO);
        assert_eq!(tool.name(), SUB_PRIO_NAME);
        assert_eq!(click(&mut tool, &map, 0.0, 0.0).3, NodeFlag::SUB_PRIO);
    }

    #[test]
    fn snap_puts_the_node_on_the_nearest_grid_point() {
        let map = MapContext::new(None, 1, 2.0, (2.0, 5.0));
        let mut tool = NodeTool::new(NodeFlag::REGULAR);
        assert_eq!(
            click_snapped(&mut tool, &map, 3.1, -7.4, true),
            (4.0, -1.0, -5.0, NodeFlag::REGULAR)
        );
        assert_eq!(click_snapped(&mut tool, &map, 3.1, -7.4, false).0, 3.1);
    }

    #[test]
    fn the_snapped_preview_sits_where_the_click_lands() {
        // Zoom ~0.49 about map px (1024, 1024) on a scale-1 map.
        let t = ViewTransform::new((500.0, 400.0), (1000.0, 800.0));
        let g = MapGeometry::new(1.0);
        let cursor = egui::pos2(503.0, 398.0);
        assert_eq!(preview_at(cursor, &t, g, None), cursor);
        let (mx, my) = t.screen_to_map(503.0, 398.0);
        let (x, z) = snap_to_grid(g.to_world(mx, my), (2.0, 5.0));
        let (sx, sy) = t.map_to_screen(g.to_map(x, z).0, g.to_map(x, z).1);
        assert_eq!(
            preview_at(cursor, &t, g, Some((2.0, 5.0))),
            egui::pos2(sx as f32, sy as f32)
        );
        assert_ne!(preview_at(cursor, &t, g, Some((2.0, 5.0))), cursor);
    }

    #[test]
    fn the_shortcut_cycles_regular_and_sub_prio() {
        let first = cycled(None);
        assert_eq!(first.flag, NodeFlag::REGULAR);
        let second = cycled(Some(&first));
        assert_eq!(second.flag, NodeFlag::SUB_PRIO);
        assert_eq!(cycled(Some(&second)).flag, NodeFlag::REGULAR);
    }
}
