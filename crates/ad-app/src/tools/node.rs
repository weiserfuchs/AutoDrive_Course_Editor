//! The node tool: a primary click adds one unconnected node.

use eframe::egui;

use ad_core::command::{AddNodes, Command, Transaction};
use ad_core::geometry::MapGeometry;
use ad_core::heightmap::sample_height;
use ad_core::network::NodeFlag;

use super::{Tool, ToolContext};
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

impl Tool for NodeTool {
    fn name(&self) -> &'static str {
        if self.flag == NodeFlag::SUB_PRIO {
            SUB_PRIO_NAME
        } else {
            REGULAR_NAME
        }
    }

    fn on_click(&mut self, ctx: &ToolContext) -> Option<Transaction> {
        // The file sentinel for "no height" (AD-7).
        let y = sample_height(ctx.map, ctx.x, ctx.z).unwrap_or(-1.0);
        Some(Transaction::new(vec![Command::AddNodes(AddNodes::new(
            vec![(ctx.x, y, ctx.z, self.flag)],
        ))]))
    }

    fn paint(
        &self,
        painter: &egui::Painter,
        _t: &ViewTransform,
        _geometry: MapGeometry,
        cursor: Option<egui::Pos2>,
    ) {
        if let Some(p) = cursor {
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
        let mut tx = tool
            .on_click(&ToolContext { x, z, map })
            .expect("a transaction");
        let mut net = Network::new();
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
    fn the_shortcut_cycles_regular_and_sub_prio() {
        let first = cycled(None);
        assert_eq!(first.flag, NodeFlag::REGULAR);
        let second = cycled(Some(&first));
        assert_eq!(second.flag, NodeFlag::SUB_PRIO);
        assert_eq!(cycled(Some(&second)).flag, NodeFlag::REGULAR);
    }
}
