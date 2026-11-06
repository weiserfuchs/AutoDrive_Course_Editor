//! The Java editor's look as pure functions: colours, node size, line and arrow geometry.
//! Nothing here paints (Java `ConnectionDrawThread`, `MapPanel.updateNodeScaling`).

use eframe::egui::Color32;

use ad_core::network::ConnectionKind;

use crate::lod::{ARROW_ANGLE_DEG, ARROW_LEN_FACTOR, ARROW_SHRINK_DIVISOR, MIN_EDGE_PX};

pub const REGULAR: Color32 = Color32::from_rgb(0, 255, 0);
pub const SUBPRIO: Color32 = Color32::from_rgb(255, 200, 0);
pub const DUAL: Color32 = Color32::from_rgb(0, 0, 255);
pub const DUAL_SUBPRIO: Color32 = Color32::from_rgb(150, 100, 50);
pub const REVERSE: Color32 = Color32::from_rgb(0, 255, 255);
pub const NODE_REGULAR: Color32 = Color32::from_rgb(255, 0, 0);
pub const NODE_SUBPRIO: Color32 = Color32::from_rgb(255, 200, 0);

/// Colour of the link `a -> b` from its kind and the flags of `a` and `b`.
/// Crossed regular and unknown links draw like regular ones: nothing is left undrawn.
pub fn connection_colour(kind: ConnectionKind, flag_a: u8, flag_b: u8) -> Color32 {
    match kind {
        ConnectionKind::Dual => {
            if flag_a == 0 && flag_b == 0 {
                DUAL
            } else {
                DUAL_SUBPRIO
            }
        }
        ConnectionKind::Reverse | ConnectionKind::CrossedReverse => REVERSE,
        ConnectionKind::Regular
        | ConnectionKind::SubPrio
        | ConnectionKind::CrossedRegular
        | ConnectionKind::Unknown
        | ConnectionKind::None => {
            if flag_a == 1 {
                SUBPRIO
            } else {
                REGULAR
            }
        }
    }
}

pub fn node_colour(flag: u8) -> Color32 {
    if flag == 0 {
        NODE_REGULAR
    } else {
        NODE_SUBPRIO
    }
}

/// On-screen node diameter in px: `floor(node_size * zoom * 0.5)`, made odd (Java).
pub fn node_px(node_size: f64, zoom: f64) -> i32 {
    let ns = (node_size * zoom * 0.5).floor() as i32;
    if ns % 2 == 0 { ns + 1 } else { ns }
}

type Pt = (f64, f64);

/// The line from `start` to `end`, each end pulled in by `ns / 2`. `None` when the full
/// length is under `MIN_EDGE_PX`.
pub fn line_ends(start: Pt, end: Pt, ns: i32) -> Option<(Pt, Pt)> {
    let dist = (end.0 - start.0).hypot(end.1 - start.1);
    if dist < MIN_EDGE_PX {
        return None;
    }
    let half = f64::from(ns) / 2.0;
    let (ux, uy) = ((end.0 - start.0) / dist, (end.1 - start.1) / dist);
    Some((
        (start.0 + ux * half, start.1 + uy * half),
        (end.0 - ux * half, end.1 - uy * half),
    ))
}

/// The filled arrowhead at `end`: the tip, then the two base points. `None` when the
/// nodes are too small or the line shorter than a node.
pub fn arrow(
    start: Pt,
    end: Pt,
    ns: i32,
    node_size: f64,
    zoom: f64,
    dual: bool,
) -> Option<[Pt; 3]> {
    let dist = (end.0 - start.0).hypot(end.1 - start.1);
    if ns < 2 || dist < MIN_EDGE_PX || dist < f64::from(ns) {
        return None;
    }
    let nsf = f64::from(ns);
    let line_length = dist - nsf;
    let diff = if dual {
        if line_length <= 2.0 * nsf {
            (2.0 * nsf - line_length) / 2.0
        } else {
            0.0
        }
    } else if line_length <= nsf {
        nsf - line_length
    } else {
        0.0
    };
    let len = node_size * zoom * ARROW_LEN_FACTOR - diff / ARROW_SHRINK_DIVISOR;
    // Angle pointing from the end back to the start.
    let theta = (start.1 - end.1).atan2(start.0 - end.0);
    let spread = ARROW_ANGLE_DEG.to_radians();
    let base = |a: f64| (end.0 + len * a.cos(), end.1 + len * a.sin());
    let half = nsf / 2.0;
    Some([
        (end.0 + half * theta.cos(), end.1 + half * theta.sin()),
        base(theta - spread),
        base(theta + spread),
    ])
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use ConnectionKind::{
        CrossedRegular, CrossedReverse, Dual, Regular, Reverse, SubPrio, Unknown,
    };

    #[test]
    fn colours_per_shape_and_flags() {
        let flags = [(0, 0), (1, 0), (0, 1), (1, 1), (2, 0)];
        for (a, b) in flags {
            let regular = if a == 1 { SUBPRIO } else { REGULAR };
            for kind in [Regular, SubPrio, CrossedRegular, Unknown] {
                assert_eq!(connection_colour(kind, a, b), regular, "{kind:?} {a}/{b}");
            }
            let dual = if (a, b) == (0, 0) { DUAL } else { DUAL_SUBPRIO };
            assert_eq!(connection_colour(Dual, a, b), dual, "dual {a}/{b}");
            for kind in [Reverse, CrossedReverse] {
                assert_eq!(connection_colour(kind, a, b), REVERSE, "{kind:?} {a}/{b}");
            }
        }
    }

    #[test]
    fn node_colours() {
        assert_eq!(node_colour(0), NODE_REGULAR);
        assert_eq!(node_colour(1), NODE_SUBPRIO);
        assert_eq!(node_colour(2), NODE_SUBPRIO);
    }

    #[test]
    fn node_size_is_odd() {
        // node_size 2: node_size * zoom * 0.5 = zoom.
        for (zoom, want) in [(1.9, 1), (2.0, 3), (3.0, 3), (4.0, 5), (4.6, 5)] {
            assert_eq!(node_px(2.0, zoom), want, "zoom {zoom}");
        }
    }

    #[test]
    fn regular_arrow_geometry() {
        // Edge of 100 px, zoom 10, node_size 2: ns 11, len 14.
        let (start, end) = ((0.0, 0.0), (100.0, 0.0));
        let ns = node_px(2.0, 10.0);
        assert_eq!(ns, 11);
        for dual in [false, true] {
            let [tip, l, r] = arrow(start, end, ns, 2.0, 10.0, dual).expect("arrow");
            assert!((tip.0 - 94.5).abs() < 1e-9 && tip.1.abs() < 1e-9);
            for p in [l, r] {
                let (dx, dy) = (p.0 - end.0, p.1 - end.1);
                assert!((dx.hypot(dy) - 14.0).abs() < 1e-9);
                // Pointing back towards the start, 20 degrees off the line.
                assert!((dy.atan2(dx).abs() - (180.0 - 20.0_f64).to_radians()).abs() < 1e-9);
            }
            assert!(l.1 * r.1 < 0.0);
        }
    }

    #[test]
    fn short_lines() {
        assert_eq!(line_ends((0.0, 0.0), (0.5, 0.0), 3), None);
        assert_eq!(arrow((0.0, 0.0), (0.5, 0.0), 3, 2.0, 2.0, false), None);
        // Shorter than the node: a line, no arrowhead.
        assert!(line_ends((0.0, 0.0), (8.0, 0.0), 11).is_some());
        assert_eq!(arrow((0.0, 0.0), (8.0, 0.0), 11, 2.0, 10.0, false), None);
        // Nodes under 2 px: no arrowhead.
        assert_eq!(arrow((0.0, 0.0), (80.0, 0.0), 1, 2.0, 1.0, false), None);
        let (s, e) = line_ends((0.0, 0.0), (100.0, 0.0), 11).expect("line");
        assert!((s.0 - 5.5).abs() < 1e-9 && (e.0 - 94.5).abs() < 1e-9);
    }

    #[test]
    fn arrow_shrinks_on_short_lines() {
        // dist 15, ns 11: line_length 4 <= ns, diff 7, len = 14 - 7/1.15.
        let [_, l, _] = arrow((0.0, 0.0), (15.0, 0.0), 11, 2.0, 10.0, false).expect("arrow");
        let len = (l.0 - 15.0).hypot(l.1);
        assert!((len - (14.0 - 7.0 / 1.15)).abs() < 1e-9);
    }
}
