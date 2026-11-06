//! Anchors and grid lines of the icons, marker names and grid as pure functions
//! (Java `NodeDrawThread` 110-139, `MapPanel.drawGrid`). Nothing here paints.

/// The grid shows only above this zoom (Java).
pub const GRID_MIN_ZOOM: f64 = 2.0;
/// Opaque grid line colour (Java `Color(25, 25, 25)`).
pub const GRID_RGB: [u8; 3] = [25, 25, 25];
/// The marker icon sits this many px above its anchor.
pub const MARKER_ICON_LIFT: f32 = 20.0;
/// The name's vertical centre sits this many px above the anchor.
pub const NAME_LIFT: f32 = 3.0;

/// Only a height of exactly -1 means "no height" (Java compares with `==`).
pub fn has_height_warning(y: f64) -> bool {
    y == -1.0
}

/// World position a marker is drawn at: one world unit above the node.
pub fn marker_anchor(x: f64, z: f64) -> (f64, f64) {
    (x, z - 1.0)
}

/// Screen centre of the marker icon for the anchor's screen position.
pub fn marker_icon_centre(anchor: (f64, f64)) -> (f32, f32) {
    (anchor.0 as f32, anchor.1 as f32 - MARKER_ICON_LIFT)
}

/// Screen centre of the marker name for the anchor's screen position.
pub fn name_centre(anchor: (f64, f64)) -> (f32, f32) {
    (anchor.0 as f32, anchor.1 as f32 - NAME_LIFT)
}

/// Whether the grid is drawn.
pub fn grid_visible(on: bool, zoom: f64) -> bool {
    on && zoom > GRID_MIN_ZOOM
}

/// Whole multiples of `spacing` in `min..=max`.
pub fn grid_lines(spacing: f64, min: f64, max: f64) -> impl Iterator<Item = f64> {
    let first = (min / spacing).ceil() as i64;
    let last = (max / spacing).floor() as i64;
    (first..=last).map(move |k| k as f64 * spacing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_warning_only_for_exactly_minus_one() {
        assert!(has_height_warning(-1.0));
        assert!(!has_height_warning(-1.0005));
        assert!(!has_height_warning(0.0));
    }

    #[test]
    fn marker_placement() {
        let anchor = marker_anchor(100.0, 200.0);
        assert_eq!(anchor, (100.0, 199.0));
        // Identity transform at scale 1: the anchor's screen position is the anchor.
        assert_eq!(marker_icon_centre(anchor), (100.0, 179.0));
        assert_eq!(name_centre(anchor), (100.0, 196.0));
    }

    #[test]
    fn grid_needs_toggle_and_zoom() {
        assert!(grid_visible(true, 2.5));
        assert!(!grid_visible(true, 2.0));
        assert!(!grid_visible(false, 5.0));
    }

    #[test]
    fn grid_lines_at_whole_multiples() {
        assert_eq!(
            grid_lines(2.0, 3.0, 9.0).collect::<Vec<_>>(),
            [4.0, 6.0, 8.0]
        );
        assert_eq!(
            grid_lines(2.0, -3.0, 3.0).collect::<Vec<_>>(),
            [-2.0, 0.0, 2.0]
        );
        assert_eq!(grid_lines(0.5, 0.1, 0.4).count(), 0);
    }
}
