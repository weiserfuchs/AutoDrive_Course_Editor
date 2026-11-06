//! Map-level data of an open document (AD-14). No I/O.

use crate::geometry::MapGeometry;

/// Smallest node size; Java steps the size by 0.1.
pub const MIN_NODE_SIZE: f64 = 0.1;

/// Smallest grid spacing in world units.
pub const MIN_GRID_SPACING: f64 = 0.1;

/// What the map is called and how world coordinates sit on it.
#[derive(Debug, Clone, PartialEq)]
pub struct MapContext {
    pub name: Option<String>,
    pub geometry: MapGeometry,
    /// Node size in map-view units (Java `nodeSize`), at least 1.
    pub node_size: f64,
    /// Grid spacing in world units along x and z, at least 0.1.
    pub grid: (f64, f64),
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
