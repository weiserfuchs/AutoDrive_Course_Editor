//! The Edit location tool (Java `EditLocationButton`): a primary click on a node asks the
//! app to open a dialog with its x, y and z. OK sets the typed position, rounded half-up
//! to three decimals, in one `MoveNodes` with absolute positions, so it undoes (AD-13).
//! The selection is kept.

use std::fmt;

use eframe::egui;

use ad_core::command::{Command, MoveNodes, Position, Transaction};
use ad_core::geometry::MapGeometry;
use ad_core::network::{Network, NodeId};

use super::{Tool, ToolContext, ToolRequest};
use crate::view::ViewTransform;

/// The labels of the three fields, in field order.
pub const AXES: [&str; 3] = ["x", "y", "z"];

/// Half the map's side in world units at scale 1 (Java's `1024 * mapScale` bound).
const HALF_MAP: f64 = 1024.0;

/// Why a field's text is refused.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Note {
    /// Empty, not a number, or not finite.
    NotANumber,
    /// A number outside `min..=max`.
    OutOfRange { min: f64, max: f64 },
}

impl fmt::Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Note::NotANumber => write!(f, "* Not a number"),
            Note::OutOfRange { min, max } => write!(f, "* Must be between {min} and {max}"),
        }
    }
}

/// The valid range of x, y and z on a map of `scale`: x and z within the map, y from 0.
pub fn ranges(scale: f64) -> [(f64, f64); 3] {
    let half = HALF_MAP * scale;
    [(-half, half), (0.0, half), (-half, half)]
}

/// `v` rounded half-up (away from zero on a tie) to three decimals, as decimal text: the
/// shortest text that reads back as `v` is rounded, so a typed `0.0005` rounds up even
/// though its nearest f64 lies just below it. `v` is finite and within the map.
pub(crate) fn round3(v: f64) -> f64 {
    // Display never uses an exponent and prints the shortest round-trip digits.
    let text = format!("{}", v.abs());
    let (int, frac) = text.split_once('.').unwrap_or((&text, ""));
    let digit = |i: usize| i64::from(frac.as_bytes().get(i).map_or(0, |b| b - b'0'));
    let int: i64 = int.parse().unwrap_or(0);
    let mut milli = int * 1000 + digit(0) * 100 + digit(1) * 10 + digit(2);
    if digit(3) >= 5 {
        milli += 1;
    }
    if milli == 0 {
        // No -0.000 for a tiny negative value.
        return 0.0;
    }
    // Both operands are exact, so the quotient is the f64 nearest to the decimal.
    v.signum() * milli as f64 / 1000.0
}

/// The value of a field: trimmed, a comma read as the decimal point, finite and within
/// `min..=max`, rounded half-up to three decimals.
pub fn parse(text: &str, min: f64, max: f64) -> Result<f64, Note> {
    let v: f64 = text
        .trim()
        .replace(',', ".")
        .parse()
        .map_err(|_| Note::NotANumber)?;
    if !v.is_finite() {
        return Err(Note::NotANumber);
    }
    if !(min..=max).contains(&v) {
        return Err(Note::OutOfRange { min, max });
    }
    Ok(round3(v))
}

/// The move that puts `node` at `to`; `None` when it is already there or does not exist.
pub fn plan(net: &Network, node: NodeId, to: Position) -> Option<Transaction> {
    let n = net.node(node)?;
    let before = (n.x, n.y, n.z);
    (before != to).then(|| {
        Transaction::new(vec![Command::MoveNodes(MoveNodes {
            moves: vec![(node, before, to)],
        })])
    })
}

/// The open dialog: the node it edits and the text of the x, y and z fields.
#[derive(Debug, Clone, PartialEq)]
pub struct LocationDialog {
    pub node: NodeId,
    pub text: [String; 3],
    /// The node's coordinates when the dialog opened, and their prefilled text.
    old: [f64; 3],
    prefill: [String; 3],
}

impl LocationDialog {
    /// The dialog for `node`, each field prefilled with its coordinate at three decimals.
    pub fn open(net: &Network, node: NodeId) -> Option<Self> {
        let n = net.node(node)?;
        let old = [n.x, n.y, n.z];
        let text = old.map(|v| format!("{v:.3}"));
        Some(Self {
            node,
            prefill: text.clone(),
            text,
            old,
        })
    }

    /// Each field parsed against its range on a map of `scale`. A field still showing its
    /// prefilled text keeps the node's exact coordinate, so OK without an edit neither
    /// rounds the node to three decimals nor refuses a value already out of range.
    pub fn values(&self, scale: f64) -> [Result<f64, Note>; 3] {
        let ranges = ranges(scale);
        std::array::from_fn(|i| {
            if self.text[i] == self.prefill[i] {
                Ok(self.old[i])
            } else {
                parse(&self.text[i], ranges[i].0, ranges[i].1)
            }
        })
    }
}

/// The tool: a click on a node leaves its id for the app to open the dialog with.
#[derive(Default)]
pub struct EditLocationTool {
    requested: Option<NodeId>,
}

impl Tool for EditLocationTool {
    fn name(&self) -> &'static str {
        "Edit location"
    }

    /// A click on a node asks for the dialog; nothing commits here.
    fn on_click(&mut self, ctx: &ToolContext) -> Option<Transaction> {
        self.requested = ctx.hovered_node;
        None
    }

    fn take_request(&mut self) -> Option<ToolRequest> {
        self.requested.take().map(ToolRequest::EditLocation)
    }

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

    fn cancel(&mut self) {
        self.requested = None;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::collections::BTreeSet;

    use ad_core::map_context::MapContext;
    use ad_core::network::NodeFlag;

    use super::*;

    const R: (f64, f64) = (-1024.0, 1024.0);

    fn setup() -> (Network, NodeId) {
        let mut net = Network::new();
        let id = net.add_node(1.5, 2.0, -3.25, NodeFlag::REGULAR);
        (net, id)
    }

    #[test]
    fn values_round_half_up_to_three_decimals() {
        let p = |t: &str| parse(t, R.0, R.1);
        assert_eq!(p("12.3456"), Ok(12.346));
        assert_eq!(p("12.3454"), Ok(12.345));
        // Ties go away from zero, also where the nearest f64 lies below the typed text.
        assert_eq!(p("0.0005"), Ok(0.001));
        assert_eq!(p("1.0005"), Ok(1.001));
        assert_eq!(p("-2.0005"), Ok(-2.001));
        assert_eq!(p("-0.0004"), Ok(0.0));
        assert!(p("-0.0004").expect("valid").is_sign_positive());
        assert_eq!(p("7"), Ok(7.0));
        assert_eq!(p("  -1023.9995 "), Ok(-1024.0));
        assert_eq!(p("1e2"), Ok(100.0));
    }

    #[test]
    fn a_comma_is_read_as_the_decimal_point() {
        assert_eq!(parse("12,5", R.0, R.1), Ok(12.5));
        assert_eq!(parse(" -0,1236 ", R.0, R.1), Ok(-0.124));
    }

    #[test]
    fn invalid_text_is_refused() {
        for text in [
            "abc", "", "   ", "1e999", "-1e999", "NaN", "inf", "1.2.3", "1,2,3",
        ] {
            assert_eq!(parse(text, R.0, R.1), Err(Note::NotANumber), "{text:?}");
        }
    }

    #[test]
    fn values_out_of_range_are_refused() {
        let [x, y, z] = ranges(2.0);
        assert_eq!(
            (x, y, z),
            ((-2048.0, 2048.0), (0.0, 2048.0), (-2048.0, 2048.0))
        );
        let out = |r: (f64, f64)| Err(Note::OutOfRange { min: r.0, max: r.1 });
        assert_eq!(parse("2048.001", x.0, x.1), out(x));
        assert_eq!(parse("-2048.001", z.0, z.1), out(z));
        assert_eq!(parse("-0.001", y.0, y.1), out(y));
        assert_eq!(parse("2048", x.0, x.1), Ok(2048.0));
        assert_eq!(parse("-2048", z.0, z.1), Ok(-2048.0));
        assert_eq!(parse("0", y.0, y.1), Ok(0.0));
        assert_eq!(parse("-1024.5", R.0, R.1), out(R));
    }

    #[test]
    fn the_dialog_prefills_three_decimals_and_parses_each_field_against_its_range() {
        let (net, id) = setup();
        let mut dialog = LocationDialog::open(&net, id).expect("node");
        assert_eq!(dialog.text, ["1.500", "2.000", "-3.250"].map(String::from));
        assert_eq!(dialog.values(1.0), [Ok(1.5), Ok(2.0), Ok(-3.25)]);
        dialog.text[1] = "-1".to_owned();
        assert_eq!(
            dialog.values(1.0)[1],
            Err(Note::OutOfRange {
                min: 0.0,
                max: 1024.0
            })
        );
        assert!(LocationDialog::open(&Network::new(), id).is_none());
    }

    #[test]
    fn untouched_fields_keep_the_exact_coordinate() {
        let mut net = Network::new();
        let id = net.add_node(1.234567, -5.0, 9.8765, NodeFlag::REGULAR);
        let mut dialog = LocationDialog::open(&net, id).expect("node");
        assert_eq!(dialog.text, ["1.235", "-5.000", "9.877"].map(String::from));
        // y is below 0, but untouched, so OK is not blocked.
        assert_eq!(dialog.values(1.0), [Ok(1.234567), Ok(-5.0), Ok(9.8765)]);
        let [Ok(x), Ok(y), Ok(z)] = dialog.values(1.0) else {
            panic!("valid");
        };
        assert!(plan(&net, id, (x, y, z)).is_none(), "OK without edits");
        dialog.text[0] = "2".to_owned();
        assert_eq!(dialog.values(1.0), [Ok(2.0), Ok(-5.0), Ok(9.8765)]);
    }

    #[test]
    fn an_edit_moves_the_node_in_one_move_nodes_and_reverts() {
        let (mut net, id) = setup();
        let x = parse("12.3456", R.0, R.1).expect("valid");
        let mut tx = plan(&net, id, (x, 2.0, -3.25)).expect("moves");
        let [Command::MoveNodes(m)] = tx.0.as_slice() else {
            panic!("one MoveNodes");
        };
        assert_eq!(m.moves, [(id, (1.5, 2.0, -3.25), (12.346, 2.0, -3.25))]);
        tx.apply(&mut net).expect("apply");
        let n = net.node(id).expect("node");
        assert_eq!((n.x, n.y, n.z), (12.346, 2.0, -3.25));
        tx.revert(&mut net).expect("revert");
        let n = net.node(id).expect("node");
        assert_eq!((n.x, n.y, n.z), (1.5, 2.0, -3.25));
    }

    #[test]
    fn an_unchanged_position_or_a_missing_node_commits_nothing() {
        let (net, id) = setup();
        assert!(plan(&net, id, (1.5, 2.0, -3.25)).is_none());
        assert!(plan(&Network::new(), id, (0.0, 0.0, 0.0)).is_none());
    }

    #[test]
    fn a_click_asks_for_the_dialog_only_on_a_node() {
        let (net, id) = setup();
        let map = MapContext::new(None, 1, 2.0, (2.0, 2.0));
        let selection = BTreeSet::new();
        let ctx = |hovered_node| ToolContext {
            x: 0.0,
            z: 0.0,
            map: &map,
            net: &net,
            selection: &selection,
            snap: false,
            hovered_node,
            hovered_edge: None,
        };
        let mut tool = EditLocationTool::default();
        assert!(tool.on_click(&ctx(Some(id))).is_none());
        assert_eq!(tool.take_request(), Some(ToolRequest::EditLocation(id)));
        assert_eq!(tool.take_request(), None, "taken once");
        assert!(tool.on_click(&ctx(None)).is_none());
        assert_eq!(tool.take_request(), None);
    }
}
