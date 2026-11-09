//! Map-level data of an open document (AD-14). No I/O.

use std::sync::Arc;

use crate::geometry::MapGeometry;
use crate::heightmap::Heightmap;

/// Smallest node size; Java steps the size by 0.1.
pub const MIN_NODE_SIZE: f64 = 0.1;

/// Smallest grid spacing in world units.
pub const MIN_GRID_SPACING: f64 = 0.1;

/// Most subdivisions of a grid cell (Java 32).
pub const MAX_SUBDIVISIONS: u32 = 32;

/// What the map is called and how world coordinates sit on it.
#[derive(Debug, Clone, PartialEq)]
pub struct MapContext {
    pub name: Option<String>,
    pub geometry: MapGeometry,
    /// Node size in map-view units (Java `nodeSize`), at least 1.
    pub node_size: f64,
    /// Grid spacing in world units along x and z, at least 0.1.
    pub grid: (f64, f64),
    /// Subdivisions of each grid cell, 1 to [`MAX_SUBDIVISIONS`] (Java `GridSubDivisions`).
    pub subdivisions: u32,
    /// Whether grid snap uses the subdivisions instead of the grid lines (Java
    /// `SnapSubDivision`, D); the app sets it from the settings.
    pub snap_subdivisions: bool,
    /// Terrain heights, if the map has a heightmap.
    pub heightmap: Option<Arc<Heightmap>>,
    /// Divisor for sampled heights (Java `heightMapScale`), at least 1.
    pub height_map_scale: f64,
}

impl MapContext {
    /// Builds the context; a scale below 1 counts as 1, a node size or grid spacing below
    /// 0.1 as 0.1.
    pub fn new(name: Option<String>, scale: u32, node_size: f64, grid: (f64, f64)) -> Self {
        Self {
            name,
            geometry: MapGeometry::new(f64::from(scale.max(1))),
            node_size: node_size.max(MIN_NODE_SIZE),
            grid: (grid.0.max(MIN_GRID_SPACING), grid.1.max(MIN_GRID_SPACING)),
            subdivisions: 1,
            snap_subdivisions: false,
            heightmap: None,
            height_map_scale: 1.0,
        }
    }

    /// The spacing grid snap rounds to: the grid, or its subdivisions while they snap.
    pub fn snap_grid(&self) -> (f64, f64) {
        if self.snap_subdivisions {
            let n = f64::from(self.subdivisions.clamp(1, MAX_SUBDIVISIONS));
            (self.grid.0 / n, self.grid.1 / n)
        } else {
            self.grid
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_grid_uses_the_subdivisions_only_while_they_snap() {
        let mut c = MapContext::new(None, 1, 2.0, (4.0, 2.0));
        c.subdivisions = 4;
        assert_eq!(c.snap_grid(), (4.0, 2.0));
        c.snap_subdivisions = true;
        assert_eq!(c.snap_grid(), (1.0, 0.5));
        c.subdivisions = 0;
        assert_eq!(c.snap_grid(), (4.0, 2.0));
    }

    #[test]
    fn scale_has_a_minimum_of_one() {
        assert_eq!(
            MapContext::new(None, 0, 2.0, (2.0, 2.0)).geometry.scale,
            1.0
        );
        let c = MapContext::new(Some("Marsch".to_owned()), 2, 3.5, (5.0, 0.0));
        assert_eq!(c.geometry.scale, 2.0);
        assert_eq!(c.node_size, 3.5);
        assert_eq!(
            MapContext::new(None, 1, 0.0, (2.0, 2.0)).node_size,
            MIN_NODE_SIZE
        );
        assert_eq!(c.grid, (5.0, MIN_GRID_SPACING));
        assert_eq!(c.name.as_deref(), Some("Marsch"));
    }
}
