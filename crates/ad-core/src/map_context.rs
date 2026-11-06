//! Map-level data of an open document (AD-14). No I/O.

use crate::geometry::MapGeometry;

/// What the map is called and how world coordinates sit on it.
#[derive(Debug, Clone, PartialEq)]
pub struct MapContext {
    pub name: Option<String>,
    pub geometry: MapGeometry,
}

impl MapContext {
    /// Builds the context; a scale below 1 counts as 1.
    pub fn new(name: Option<String>, scale: u32) -> Self {
        Self {
            name,
            geometry: MapGeometry::new(f64::from(scale.max(1))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_has_a_minimum_of_one() {
        assert_eq!(MapContext::new(None, 0).geometry.scale, 1.0);
        let c = MapContext::new(Some("Marsch".to_owned()), 2);
        assert_eq!(c.geometry.scale, 2.0);
        assert_eq!(c.name.as_deref(), Some("Marsch"));
    }
}
