//! Terrain heightmap and the one height lookup (AD-7). No I/O.

use crate::geometry::{MAP_PIXELS, MapGeometry};
use crate::map_context::MapContext;
use crate::network::Network;

/// Raw 16-bit samples of a heightmap, row-major; one raw step is 1/256 m before scaling.
#[derive(Debug, Clone, PartialEq)]
pub struct Heightmap {
    width: u32,
    height: u32,
    raw: Vec<u16>,
}

impl Heightmap {
    /// `None` when a side is zero or `raw` does not hold `width * height` samples.
    pub fn new(width: u32, height: u32, raw: Vec<u16>) -> Option<Self> {
        let expected = u64::from(width) * u64::from(height);
        (width > 0 && height > 0 && raw.len() as u64 == expected).then_some(Self {
            width,
            height,
            raw,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// The raw sample at pixel (px, py), clamped to the image.
    pub fn sample_raw(&self, px: i64, py: i64) -> u16 {
        let x = px.clamp(0, i64::from(self.width) - 1) as usize;
        let y = py.clamp(0, i64::from(self.height) - 1) as usize;
        self.raw[y * self.width as usize + x]
    }

    /// Pixel of world (x, z) (Java `getYValueFromHeightMap`), clamped to the image.
    fn pixel(&self, geometry: MapGeometry, x: f64, z: f64) -> (i64, i64) {
        let axis = |world: f64, size: u32| {
            let centre = f64::from(size / 2);
            let step = MAP_PIXELS / f64::from(size);
            (centre + ((world / geometry.scale) / step).floor()).clamp(0.0, f64::from(size - 1))
                as i64
        };
        (axis(x, self.width), axis(z, self.height))
    }

    /// Height in metres at world (x, z) for a height scale: `raw / 256`, rounded to three
    /// decimals half-up, divided by `height_map_scale`.
    fn height_at(&self, geometry: MapGeometry, height_map_scale: f64, x: f64, z: f64) -> f64 {
        let (px, py) = self.pixel(geometry, x, z);
        let metres = f64::from(self.sample_raw(px, py)) / 256.0;
        (metres * 1000.0 + 0.5).floor() / 1000.0 / height_map_scale
    }
}

/// Java `calculateHeightMapScaling`: `round(mean((h - y) / y))` over nodes with `y > 0`,
/// at least 1; 1 when no node qualifies.
pub fn calibrate(heightmap: &Heightmap, network: &Network, geometry: MapGeometry) -> f64 {
    let (mut sum, mut count) = (0.0, 0_u32);
    for node in network.order().iter().filter_map(|&id| network.node(id)) {
        if node.y > 0.0 {
            let h = heightmap.height_at(geometry, 1.0, node.x, node.z);
            sum += (h - node.y) / node.y;
            count += 1;
        }
    }
    if count == 0 {
        return 1.0;
    }
    (sum / f64::from(count) + 0.5).floor().max(1.0)
}

/// Terrain height at world (x, z), or `None` without a heightmap. The only height function.
pub fn sample_height(map: &MapContext, x: f64, z: f64) -> Option<f64> {
    let heightmap = map.heightmap.as_ref()?;
    Some(heightmap.height_at(map.geometry, map.height_map_scale, x, z))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::sync::Arc;

    use super::*;
    use crate::network::NodeFlag;

    fn map_with(w: u32, h: u32, raw: Vec<u16>, scale: u32) -> MapContext {
        let mut map = MapContext::new(None, scale, 2.0, (2.0, 2.0));
        map.heightmap = Some(Arc::new(Heightmap::new(w, h, raw).expect("size")));
        map
    }

    #[test]
    fn new_checks_the_length() {
        assert!(Heightmap::new(2, 2, vec![0; 3]).is_none());
        assert!(Heightmap::new(0, 2, vec![]).is_none());
        assert!(Heightmap::new(2, 2, vec![0; 4]).is_some());
    }

    #[test]
    fn pixel_mapping_clamps() {
        let w = 2049_u32;
        let hm = Heightmap::new(w, w, vec![0; (w * w) as usize]).expect("size");
        let g = MapGeometry::new(2.0);
        assert_eq!(hm.pixel(g, 0.0, 0.0), (1024, 1024));
        assert_eq!(hm.pixel(g, -4096.0, -4096.0), (0, 0));
        assert_eq!(hm.pixel(g, 4095.9, 4095.9), (2048, 2048));
        assert_eq!(hm.pixel(g, f64::NAN, 1e30), (0, 2048));
    }

    #[test]
    fn rounding_and_height_scale() {
        let map = map_with(2, 2, vec![12345; 4], 1);
        assert_eq!(sample_height(&map, 0.0, 0.0), Some(48.223));
        let mut hm = map_with(4, 4, vec![25600; 16], 1);
        assert_eq!(sample_height(&hm, 0.0, 0.0), Some(100.0));
        hm.height_map_scale = 2.0;
        assert_eq!(sample_height(&hm, 0.0, 0.0), Some(50.0));
    }

    #[test]
    fn no_heightmap_gives_none() {
        let map = MapContext::new(None, 1, 2.0, (2.0, 2.0));
        assert_eq!(sample_height(&map, 0.0, 0.0), None);
    }

    fn network(ys: &[f64]) -> Network {
        let mut n = Network::new();
        for &y in ys {
            n.add_node(0.0, y, 0.0, NodeFlag::REGULAR);
        }
        n
    }

    #[test]
    fn calibration_rows() {
        let hm = Heightmap::new(2, 2, vec![25600; 4]).expect("size");
        let g = MapGeometry::new(1.0);
        assert_eq!(calibrate(&hm, &network(&[100.0, 100.0]), g), 1.0);
        assert_eq!(calibrate(&hm, &network(&[50.0, 50.0]), g), 1.0);
        assert_eq!(calibrate(&hm, &network(&[0.0, -1.0]), g), 1.0);
        assert_eq!(calibrate(&hm, &Network::new(), g), 1.0);
        // Samples are three times the heights: mean ratio 2.
        assert_eq!(calibrate(&hm, &network(&[100.0 / 3.0]), g), 2.0);
        assert_eq!(calibrate(&hm, &network(&[100.0 / 3.0, -1.0, 0.0]), g), 2.0);
    }
}
