//! Level-of-detail limits of the map view.

/// Node size in map-view units; the on-screen diameter is `NODE_SIZE * zoom * 0.5` (Java).
pub const NODE_SIZE: f64 = 2.0;
/// Nodes whose on-screen size is below this many pixels are not drawn.
pub const MIN_NODE_PX: f64 = 2.0;
/// Connections shorter than this many pixels on screen are not drawn.
pub const MIN_EDGE_PX: f64 = 1.0;
/// Zoom factor of one wheel step.
pub const ZOOM_STEP: f64 = 1.6;
/// Largest zoom, in screen pixels per map pixel.
pub const MAX_ZOOM: f64 = 30.0;
/// Scroll points (trackpads) that count as one wheel step.
pub const WHEEL_POINTS_PER_STEP: f32 = 50.0;
