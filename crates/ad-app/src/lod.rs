//! Level-of-detail limits of the map view.

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
/// Half-angle of the arrowhead, in degrees (Java `arrowLeftAngle`).
pub const ARROW_ANGLE_DEG: f64 = 20.0;
/// Arrowhead length per `node_size * zoom` (Java 0.7).
pub const ARROW_LEN_FACTOR: f64 = 0.7;
/// Divisor of the shrink applied to arrowheads on lines shorter than the nodes (Java 1.15).
pub const ARROW_SHRINK_DIVISOR: f64 = 1.15;
