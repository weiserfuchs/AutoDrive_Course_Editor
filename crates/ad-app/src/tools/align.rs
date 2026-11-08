//! The align tools (Java `AlignBaseButton` and its horizontal, vertical and depth
//! buttons): with a selection, a primary click on a reference node, selected or not, sets
//! one coordinate of every selected node to the reference's value in one `MoveNodes` with
//! absolute positions, so aligning to 0 undoes (AD-13). The selection is kept.

use std::collections::BTreeSet;

use eframe::egui;

use ad_core::command::{Command, MoveNodes, Position, Transaction};
use ad_core::geometry::MapGeometry;
use ad_core::map_context::MapContext;
use ad_core::network::{Network, NodeId};

use super::{Tool, ToolContext};
use crate::view::ViewTransform;

/// The coordinate an align tool copies from the reference node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// z: the nodes line up on a horizontal screen line.
    Horizontal,
    /// x: the nodes line up on a vertical screen line.
    Vertical,
    /// y: the nodes get the reference's height.
    Depth,
}

impl Axis {
    /// `p` with this axis's coordinate set to `to`'s.
    fn aligned(self, (x, y, z): Position, (rx, ry, rz): Position) -> Position {
        match self {
            Axis::Horizontal => (x, y, rz),
            Axis::Vertical => (rx, y, z),
            Axis::Depth => (x, ry, z),
        }
    }
}

/// Stroke width of the guide line, in screen px (Java's 3 px dash).
const GUIDE_WIDTH_PX: f32 = 3.0;
/// Dash and gap length of the guide line, in screen px.
const GUIDE_DASH_PX: f32 = 8.0;
const GUIDE_GAP_PX: f32 = 4.0;

/// The move that aligns every selected node to `reference` along `axis`. Nodes already at
/// the value are left out; `None` when no node changes or the reference does not exist.
pub fn plan(
    net: &Network,
    selection: &BTreeSet<NodeId>,
    reference: NodeId,
    axis: Axis,
) -> Option<Transaction> {
    let r = net.node(reference)?;
    let to = (r.x, r.y, r.z);
    let moves: Vec<(NodeId, Position, Position)> = selection
        .iter()
        .filter_map(|&id| net.node(id).map(|n| (id, (n.x, n.y, n.z))))
        .map(|(id, before)| (id, before, axis.aligned(before, to)))
        .filter(|(_, before, after)| before != after)
        .collect();
    (!moves.is_empty()).then(|| Transaction::new(vec![Command::MoveNodes(MoveNodes { moves })]))
}

pub struct AlignTool {
    pub axis: Axis,
}

impl AlignTool {
    pub fn new(axis: Axis) -> Self {
        Self { axis }
    }
}

impl Tool for AlignTool {
    fn name(&self) -> &'static str {
        match self.axis {
            Axis::Horizontal => "Align H",
            Axis::Vertical => "Align V",
            Axis::Depth => "Align depth",
        }
    }

    /// A click on a node with a non-empty selection aligns the selection to it.
    fn on_click(&mut self, ctx: &ToolContext) -> Option<Transaction> {
        if ctx.selection.is_empty() {
            return None;
        }
        plan(ctx.net, ctx.selection, ctx.hovered_node?, self.axis)
    }

    /// Draws a dashed guide through the hovered reference along the aligned axis (Java's
    /// `drawLine`); depth draws none, as in Java.
    fn paint_selection(
        &self,
        painter: &egui::Painter,
        t: &ViewTransform,
        map: &MapContext,
        net: &Network,
        selection: &BTreeSet<NodeId>,
        hovered: Option<NodeId>,
        _revision: u64,
    ) {
        if selection.is_empty() {
            return;
        }
        let Some(r) = hovered.and_then(|id| net.node(id)) else {
            return;
        };
        let (mx, my) = map.geometry.to_map(r.x, r.z);
        let (sx, sy) = t.map_to_screen(mx, my);
        let (sx, sy) = (sx as f32, sy as f32);
        let clip = painter.clip_rect();
        let ends = match self.axis {
            Axis::Horizontal => [egui::pos2(clip.left(), sy), egui::pos2(clip.right(), sy)],
            Axis::Vertical => [egui::pos2(sx, clip.top()), egui::pos2(sx, clip.bottom())],
            Axis::Depth => return,
        };
        let stroke = egui::Stroke::new(GUIDE_WIDTH_PX, egui::Color32::WHITE.gamma_multiply(0.5));
        painter.extend(egui::Shape::dashed_line(
            &ends,
            stroke,
            GUIDE_DASH_PX,
            GUIDE_GAP_PX,
        ));
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

    fn cancel(&mut self) {}
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use ad_core::network::NodeFlag;

    use super::*;

    /// a, b, c at distinct x, y, z; d outside at (7, 8, 9).
    fn setup() -> (Network, [NodeId; 4]) {
        let mut net = Network::new();
        let a = net.add_node(1.0, 2.0, 3.0, NodeFlag::REGULAR);
        let b = net.add_node(-4.0, 5.0, -6.0, NodeFlag::REGULAR);
        let c = net.add_node(10.0, -1.0, 20.0, NodeFlag::REGULAR);
        let d = net.add_node(7.0, 8.0, 9.0, NodeFlag::REGULAR);
        (net, [a, b, c, d])
    }

    fn click(
        net: &Network,
        selection: &BTreeSet<NodeId>,
        node: Option<NodeId>,
        axis: Axis,
    ) -> Option<Transaction> {
        let map = MapContext::new(None, 1, 2.0, (2.0, 2.0));
        let ctx = ToolContext {
            x: 0.0,
            z: 0.0,
            map: &map,
            net,
            selection,
            snap: false,
            hovered_node: node,
            hovered_edge: None,
        };
        AlignTool::new(axis).on_click(&ctx)
    }

    fn positions(net: &Network, ids: &[NodeId]) -> Vec<Position> {
        ids.iter()
            .map(|id| net.node(*id).map(|n| (n.x, n.y, n.z)).expect("node"))
            .collect()
    }

    fn moves(tx: &Transaction) -> &[(NodeId, Position, Position)] {
        let [Command::MoveNodes(m)] = tx.0.as_slice() else {
            panic!("one MoveNodes");
        };
        &m.moves
    }

    #[test]
    fn each_axis_copies_one_coordinate_of_the_reference_in_one_move_nodes() {
        let cases = [
            (
                Axis::Horizontal,
                [(1.0, 2.0, 9.0), (-4.0, 5.0, 9.0), (10.0, -1.0, 9.0)],
            ),
            (
                Axis::Vertical,
                [(7.0, 2.0, 3.0), (7.0, 5.0, -6.0), (7.0, -1.0, 20.0)],
            ),
            (
                Axis::Depth,
                [(1.0, 8.0, 3.0), (-4.0, 8.0, -6.0), (10.0, 8.0, 20.0)],
            ),
        ];
        for (axis, want) in cases {
            let (mut net, [a, b, c, d]) = setup();
            let selection = BTreeSet::from([a, b, c]);
            let start = positions(&net, &[a, b, c, d]);
            let mut tx = click(&net, &selection, Some(d), axis).expect("align");
            assert_eq!(moves(&tx).len(), 3, "{axis:?}");
            tx.apply(&mut net).expect("apply");
            assert_eq!(positions(&net, &[a, b, c]), want, "{axis:?}");
            assert_eq!(positions(&net, &[d]), [start[3]], "{axis:?}");
            tx.revert(&mut net).expect("revert");
            assert_eq!(positions(&net, &[a, b, c, d]), start, "{axis:?}");
        }
    }

    #[test]
    fn aligning_to_zero_undoes_to_the_old_values() {
        let (mut net, [a, b, c, _]) = setup();
        let zero = net.add_node(0.0, 0.0, 0.0, NodeFlag::REGULAR);
        let selection = BTreeSet::from([a, b, c]);
        let start = positions(&net, &[a, b, c]);
        for axis in [Axis::Horizontal, Axis::Vertical, Axis::Depth] {
            let mut tx = click(&net, &selection, Some(zero), axis).expect("align");
            tx.apply(&mut net).expect("apply");
            tx.revert(&mut net).expect("revert");
            assert_eq!(positions(&net, &[a, b, c]), start, "{axis:?}");
        }
    }

    #[test]
    fn a_selected_reference_stays_put_and_the_others_align() {
        let (mut net, [a, b, c, _]) = setup();
        let selection = BTreeSet::from([a, b, c]);
        let mut tx = click(&net, &selection, Some(b), Axis::Horizontal).expect("align");
        assert_eq!(
            moves(&tx).iter().map(|m| m.0).collect::<Vec<_>>(),
            [a, c],
            "the reference is left out"
        );
        tx.apply(&mut net).expect("apply");
        assert_eq!(
            positions(&net, &[a, b, c]),
            [(1.0, 2.0, -6.0), (-4.0, 5.0, -6.0), (10.0, -1.0, -6.0)]
        );
    }

    #[test]
    fn nodes_already_aligned_are_left_out_and_none_changing_commits_nothing() {
        let (mut net, [a, b, c, d]) = setup();
        net.move_node(b, -4.0, 5.0, 9.0).expect("move");
        let tx = click(&net, &BTreeSet::from([a, b, c]), Some(d), Axis::Horizontal);
        let ids: Vec<NodeId> = moves(&tx.expect("align")).iter().map(|m| m.0).collect();
        assert_eq!(ids, [a, c]);
        net.move_node(a, 1.0, 2.0, 9.0).expect("move");
        net.move_node(c, 10.0, -1.0, 9.0).expect("move");
        assert!(click(&net, &BTreeSet::from([a, b, c]), Some(d), Axis::Horizontal).is_none());
    }

    #[test]
    fn the_guide_shows_only_for_a_hovered_reference_with_a_selection() {
        let (net, [a, _, _, d]) = setup();
        let map = MapContext::new(None, 1, 2.0, (2.0, 2.0));
        let ctx = egui::Context::default();
        let shapes = |axis: Axis, selection: &BTreeSet<NodeId>, hovered: Option<NodeId>| {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                let t = ViewTransform::new((512.0, 512.0), (1024.0, 1024.0));
                AlignTool::new(axis).paint_selection(
                    ui.painter(),
                    &t,
                    &map,
                    &net,
                    selection,
                    hovered,
                    0,
                );
            });
            out.textures_delta.clear();
            out.shapes.len()
        };
        let selection = BTreeSet::from([a]);
        let blank = shapes(Axis::Horizontal, &BTreeSet::new(), None);
        assert!(shapes(Axis::Horizontal, &selection, Some(d)) > blank);
        assert!(shapes(Axis::Vertical, &selection, Some(d)) > blank);
        assert_eq!(shapes(Axis::Depth, &selection, Some(d)), blank);
        assert_eq!(shapes(Axis::Horizontal, &selection, None), blank);
        assert_eq!(shapes(Axis::Horizontal, &BTreeSet::new(), Some(d)), blank);
    }

    #[test]
    fn an_empty_selection_or_a_click_off_a_node_does_nothing() {
        let (net, [a, b, _, d]) = setup();
        for axis in [Axis::Horizontal, Axis::Vertical, Axis::Depth] {
            assert!(click(&net, &BTreeSet::new(), Some(d), axis).is_none());
            assert!(click(&net, &BTreeSet::from([a, b]), None, axis).is_none());
        }
    }
}
