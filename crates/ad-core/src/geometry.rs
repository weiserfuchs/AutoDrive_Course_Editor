//! World <-> map-pixel conversion (AD-7). The only place this mapping lives.

/// Side length of every map image in pixels.
pub const MAP_PIXELS: f64 = 2048.0;

/// The map spans `2048 * scale` world units, centred on 0. World z maps to map-pixel y.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapGeometry {
    pub scale: f64,
}

impl MapGeometry {
    pub fn new(scale: f64) -> Self {
        Self { scale }
    }

    /// Map pixel (x, y) of world (x, z).
    pub fn to_map(&self, x: f64, z: f64) -> (f64, f64) {
        let half = MAP_PIXELS / 2.0;
        (x / self.scale + half, z / self.scale + half)
    }

    /// World (x, z) of map pixel (px, py). Exact inverse of [`Self::to_map`].
    pub fn to_world(&self, px: f64, py: f64) -> (f64, f64) {
        let half = MAP_PIXELS / 2.0;
        ((px - half) * self.scale, (py - half) * self.scale)
    }
}

impl Default for MapGeometry {
    fn default() -> Self {
        Self { scale: 1.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_edges() {
        for s in [1.0, 2.0, 4.0] {
            let g = MapGeometry::new(s);
            assert_eq!(g.to_map(0.0, 0.0), (1024.0, 1024.0));
            assert_eq!(g.to_map(-1024.0 * s, -1024.0 * s), (0.0, 0.0));
            assert_eq!(g.to_map(1024.0 * s, 1024.0 * s), (2048.0, 2048.0));
            for (x, z) in [
                (0.0, 0.0),
                (1024.0 * s, -1024.0 * s),
                (-1024.0 * s, 1024.0 * s),
                (123.456, -987.654),
            ] {
                let (px, py) = g.to_map(x, z);
                let (wx, wz) = g.to_world(px, py);
                assert!((wx - x).abs() < 1e-9 && (wz - z).abs() < 1e-9);
            }
        }
    }
}
