//! Marker list: the named, grouped nodes of the config.

use super::{Network, NetworkError, NodeId};

#[derive(Debug, Clone, PartialEq)]
pub struct Marker {
    pub node: NodeId,
    pub name: String,
    pub group: String,
    /// Ids of the vehicles `vehicles.xml` parks at this marker (AD-17).
    pub parked_vehicles: Vec<String>,
}

/// What a marker edit changed.
#[derive(Debug, Clone, PartialEq)]
pub enum MarkerDelta {
    Unchanged,
    Added { index: usize },
    Replaced { index: usize, old: Marker },
    Removed { index: usize, marker: Marker },
}

impl Network {
    pub fn push_marker(&mut self, marker: Marker) {
        self.markers.push(marker);
    }

    pub fn markers(&self) -> &[Marker] {
        &self.markers
    }

    /// Adds `vehicle` to the parked vehicles of the marker at `index`, as a load does.
    /// Returns false when there is no such marker.
    pub fn park_vehicle(&mut self, index: usize, vehicle: String) -> bool {
        match self.markers.get_mut(index) {
            Some(marker) => {
                marker.parked_vehicles.push(vehicle);
                true
            }
            None => false,
        }
    }

    /// Sets the marker of `node`, replacing it in place or appending a new one. A
    /// replaced marker keeps its parked vehicles; a new one has none.
    pub fn set_marker(
        &mut self,
        node: NodeId,
        name: String,
        group: String,
    ) -> Result<MarkerDelta, NetworkError> {
        self.require(node)?;
        if let Some(index) = self.markers.iter().position(|m| m.node == node) {
            let parked_vehicles = self.markers[index].parked_vehicles.clone();
            let marker = Marker {
                node,
                name,
                group,
                parked_vehicles,
            };
            let old = std::mem::replace(&mut self.markers[index], marker);
            Ok(MarkerDelta::Replaced { index, old })
        } else {
            self.markers.push(Marker {
                node,
                name,
                group,
                parked_vehicles: Vec::new(),
            });
            Ok(MarkerDelta::Added {
                index: self.markers.len() - 1,
            })
        }
    }

    /// Removes the marker of `node`, if it has one.
    pub fn remove_marker(&mut self, node: NodeId) -> Result<MarkerDelta, NetworkError> {
        self.require(node)?;
        Ok(match self.markers.iter().position(|m| m.node == node) {
            Some(index) => MarkerDelta::Removed {
                index,
                marker: self.markers.remove(index),
            },
            None => MarkerDelta::Unchanged,
        })
    }

    pub fn revert_marker(&mut self, delta: &MarkerDelta) {
        match delta {
            MarkerDelta::Unchanged => {}
            MarkerDelta::Added { index } => {
                if *index < self.markers.len() {
                    self.markers.remove(*index);
                }
            }
            MarkerDelta::Replaced { index, old } => {
                if let Some(slot) = self.markers.get_mut(*index) {
                    *slot = old.clone();
                }
            }
            MarkerDelta::Removed { index, marker } => {
                self.markers
                    .insert((*index).min(self.markers.len()), marker.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::super::tests::two;
    use super::*;

    #[test]
    fn marker_deltas_revert() {
        let (mut n, a, b) = two();
        let d1 = n.set_marker(a, "A".into(), "g".into()).expect("ok");
        let d2 = n.set_marker(b, "B".into(), "g".into()).expect("ok");
        let d3 = n.set_marker(a, "A2".into(), "h".into()).expect("ok");
        let d4 = n.remove_marker(a).expect("ok");
        assert_eq!(n.markers().len(), 1);
        assert_eq!(n.remove_marker(a).expect("ok"), MarkerDelta::Unchanged);
        for d in [&d4, &d3] {
            n.revert_marker(d);
        }
        assert_eq!(n.markers()[0].name, "A");
        n.revert_marker(&d2);
        n.revert_marker(&d1);
        assert!(n.markers().is_empty());
        let ghost = NodeId(99);
        assert_eq!(
            n.set_marker(ghost, String::new(), String::new()),
            Err(NetworkError::UnknownNode(ghost))
        );
    }

    #[test]
    fn parked_vehicles_survive_replace_remove_and_reverts() {
        let (mut n, a, b) = two();
        n.set_marker(a, "A".into(), "g".into()).expect("ok");
        assert!(n.park_vehicle(0, "v1".into()));
        assert!(!n.park_vehicle(1, "v2".into()));
        let parked = vec!["v1".to_owned()];
        let replace = n.set_marker(a, "A2".into(), "h".into()).expect("ok");
        assert_eq!(n.markers()[0].parked_vehicles, parked);
        let remove = n.remove_marker(a).expect("ok");
        n.revert_marker(&remove);
        assert_eq!(n.markers()[0].parked_vehicles, parked);
        n.revert_marker(&replace);
        assert_eq!(
            (
                n.markers()[0].name.as_str(),
                &n.markers()[0].parked_vehicles
            ),
            ("A", &parked)
        );
        n.set_marker(b, "B".into(), "g".into()).expect("ok");
        assert!(n.markers()[1].parked_vehicles.is_empty());
    }
}
