//! The connect tools (regular, dual, reverse): a click on a start node, then on an end
//! node, creates the tool's link between them, removes it if it is already exactly that,
//! or reshapes any other link into it, in one transaction (AD-3, AD-4, AD-10).

use eframe::egui;

use ad_core::command::{Command, Connect, Disconnect, Transaction};
use ad_core::geometry::MapGeometry;
use ad_core::network::{ConnectionShape, Network, NodeId};

use super::{Tool, ToolContext};
use crate::look;
use crate::view::ViewTransform;

pub const REGULAR_NAME: &str = "Connect";
pub const DUAL_NAME: &str = "Dual";
pub const REVERSE_NAME: &str = "Reverse";

/// Radius of the ring on the start node, in screen px.
const RING_RADIUS_PX: f32 = 8.0;
/// Stroke width of the pending link and the ring, in screen px.
const STROKE_PX: f32 = 2.0;

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
}

impl ConnectTool {
    pub fn new(shape: ConnectionShape, continuous: bool) -> Self {
        Self {
            shape,
            start: None,
            continuous,
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
pub fn cycled(active: Option<&dyn Tool>, continuous: bool) -> ConnectTool {
    let shape = match active.map(Tool::name) {
        Some(REGULAR_NAME) => ConnectionShape::Dual,
        Some(DUAL_NAME) => ConnectionShape::Reverse,
        _ => ConnectionShape::OneWay,
    };
    ConnectTool::new(shape, continuous)
}

/// Whether the links between `a` and `b` are exactly `shape` from `a` to `b`.
fn is_exactly(net: &Network, a: NodeId, b: NodeId, shape: ConnectionShape) -> bool {
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

    fn on_click(&mut self, ctx: &ToolContext) -> Option<Transaction> {
        let id = ctx.hovered_node?;
        let n = ctx.net.node(id)?;
        let picked = Start { id, x: n.x, z: n.z };
        let Some(start) = self.start else {
            self.start = Some(picked);
            return None;
        };
        if start.id == id {
            self.start = None;
            return None;
        }
        let tx = plan(ctx.net, start.id, id, self.shape);
        self.start = self.continuous.then_some(picked);
        tx
    }

    fn set_continuous(&mut self, on: bool) {
        self.continuous = on;
    }

    /// Draws the pending link from the start node to the cursor, and a ring on the start.
    fn paint(
        &self,
        painter: &egui::Painter,
        t: &ViewTransform,
        geometry: MapGeometry,
        cursor: Option<egui::Pos2>,
        _snap_grid: Option<(f64, f64)>,
        _hovered_edge: Option<(egui::Pos2, egui::Pos2)>,
    ) {
        let Some(start) = self.start else {
            return;
        };
        let (mx, my) = geometry.to_map(start.x, start.z);
        let (sx, sy) = t.map_to_screen(mx, my);
        let from = egui::pos2(sx as f32, sy as f32);
        let colour = match self.shape {
            ConnectionShape::OneWay => look::REGULAR,
            ConnectionShape::Dual => look::DUAL,
            ConnectionShape::Reverse => look::REVERSE,
        };
        let stroke = egui::Stroke::new(STROKE_PX, colour);
        painter.circle_stroke(from, RING_RADIUS_PX, stroke);
        if let Some(to) = cursor {
            painter.line_segment([from, to], stroke);
        }
    }

    fn cancel(&mut self) {
        self.start = None;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use ad_core::map_context::MapContext;
    use ad_core::network::{ConnectionKind, NodeFlag};

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

    /// Clicks with `hovered` under the cursor and applies what the tool returns.
    fn click(tool: &mut ConnectTool, net: &mut Network, hovered: Option<NodeId>) -> bool {
        let map = MapContext::new(None, 1, 2.0, (2.0, 2.0));
        let tx = tool.on_click(&ToolContext {
            x: 0.0,
            z: 0.0,
            map: &map,
            net,
            snap: false,
            hovered_node: hovered,
            hovered_edge: None,
        });
        match tx {
            Some(mut tx) => {
                tx.apply(net).expect("applies");
                true
            }
            None => false,
        }
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
        let mut tool = ConnectTool::new(ConnectionShape::OneWay, false);
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
        let mut tool = ConnectTool::new(ConnectionShape::Dual, false);
        assert!(!click(&mut tool, &mut net, None));
        assert_eq!(tool.start(), None);
        click(&mut tool, &mut net, Some(a));
        assert!(!click(&mut tool, &mut net, None));
        assert_eq!(tool.start(), Some(a));
    }

    #[test]
    fn continuous_makes_the_end_node_the_next_start() {
        let (mut net, [a, b, c]) = setup();
        let mut tool = ConnectTool::new(ConnectionShape::OneWay, false);
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
        let mut tool = ConnectTool::new(ConnectionShape::Reverse, false);
        click(&mut tool, &mut net, Some(a));
        tool.cancel();
        assert_eq!(tool.start(), None);
    }

    #[test]
    fn the_shortcut_cycles_regular_dual_reverse() {
        let first = cycled(None, true);
        assert_eq!(first.shape, ConnectionShape::OneWay);
        assert!(first.continuous);
        let second = cycled(Some(&first), false);
        assert_eq!(second.shape, ConnectionShape::Dual);
        let third = cycled(Some(&second), false);
        assert_eq!(third.shape, ConnectionShape::Reverse);
        assert_eq!(cycled(Some(&third), false).shape, ConnectionShape::OneWay);
    }
}
