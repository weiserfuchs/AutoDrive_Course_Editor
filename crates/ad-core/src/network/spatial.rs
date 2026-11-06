//! R-tree indexes and the spatial queries over them.

use rstar::AABB;
use rstar::primitives::{GeomWithData, Line};

use super::{Network, Node, NodeId};

pub(super) type NodeEntry = GeomWithData<[f64; 2], NodeId>;
pub(super) type EdgeEntry = GeomWithData<Line<[f64; 2]>, (NodeId, NodeId)>;

pub(super) fn edge_entry(from: &Node, to: &Node, ids: (NodeId, NodeId)) -> EdgeEntry {
    GeomWithData::new(Line::new([from.x, from.z], [to.x, to.z]), ids)
}

/// Even-odd ray casting.
fn point_in_polygon(x: f64, z: f64, polygon: &[(f64, f64)]) -> bool {
    let mut inside = false;
    let mut j = polygon.len().wrapping_sub(1);
    for (i, &(xi, zi)) in polygon.iter().enumerate() {
        let (xj, zj) = polygon[j];
        if (zi > z) != (zj > z) && x < (xj - xi) * (z - zi) / (zj - zi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

impl Network {
    /// Removes one edge-index entry for `a -> b` when both nodes exist.
    pub(super) fn drop_edge(&mut self, a: NodeId, b: NodeId) {
        if let (Some(from), Some(to)) = (self.nodes.get(&a), self.nodes.get(&b)) {
            let entry = edge_entry(from, to, (a, b));
            self.edge_index.remove(&entry);
        }
    }

    /// Inserts an edge-index entry for `a -> b` when both nodes exist.
    pub(super) fn put_edge(&mut self, a: NodeId, b: NodeId) {
        if let (Some(from), Some(to)) = (self.nodes.get(&a), self.nodes.get(&b)) {
            self.edge_index.insert(edge_entry(from, to, (a, b)));
        }
    }

    /// Edge-index entries whose envelope contains `point`.
    pub(super) fn edges_at(&self, point: [f64; 2]) -> impl Iterator<Item = &EdgeEntry> {
        self.edge_index
            .locate_in_envelope_intersecting(AABB::from_point(point))
    }

    /// The node closest to (x, z).
    pub fn nearest(&self, x: f64, z: f64) -> Option<NodeId> {
        self.node_index.nearest_neighbor([x, z]).map(|e| e.data)
    }

    /// Nodes inside the rectangle, bounds inclusive. Sorted by id.
    pub fn in_rect(&self, x0: f64, z0: f64, x1: f64, z1: f64) -> Vec<NodeId> {
        let aabb = AABB::from_corners([x0, z0], [x1, z1]);
        let mut ids: Vec<NodeId> = self
            .node_index
            .locate_in_envelope(aabb)
            .map(|e| e.data)
            .collect();
        ids.sort();
        ids
    }

    /// Nodes inside the polygon (even-odd rule). Sorted by id.
    pub fn in_polygon(&self, polygon: &[(f64, f64)]) -> Vec<NodeId> {
        if polygon.len() < 3 {
            return Vec::new();
        }
        let aabb = AABB::from_points(
            polygon
                .iter()
                .map(|&(x, z)| [x, z])
                .collect::<Vec<_>>()
                .iter(),
        );
        let mut ids: Vec<NodeId> = self
            .node_index
            .locate_in_envelope(aabb)
            .filter(|e| point_in_polygon(e.geom()[0], e.geom()[1], polygon))
            .map(|e| e.data)
            .collect();
        ids.sort();
        ids
    }

    /// Nodes within `distance` of (x, z), inclusive. Sorted by id.
    pub fn within(&self, x: f64, z: f64, distance: f64) -> Vec<NodeId> {
        let mut ids: Vec<NodeId> = self
            .node_index
            .locate_within_distance([x, z], distance * distance)
            .map(|e| e.data)
            .collect();
        ids.sort();
        ids
    }

    /// `(from, to)` of every edge whose segment lies within `distance` of (x, z), inclusive.
    /// Sorted.
    pub fn edges_near(&self, x: f64, z: f64, distance: f64) -> Vec<(NodeId, NodeId)> {
        let mut edges: Vec<(NodeId, NodeId)> = self
            .edge_index
            .locate_within_distance([x, z], distance * distance)
            .map(|e| e.data)
            .collect();
        edges.sort();
        edges
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::super::tests::HUGE;
    use super::super::{ConnectionShape, LoadedNode, NodeFlag};
    use super::*;

    /// Small deterministic generator, so the tests need no extra dependency.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 33
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
        fn coord(&mut self) -> f64 {
            self.below(200) as f64
        }
    }

    #[test]
    fn polygon_literal_shapes() {
        let mut n = Network::new();
        let mut add = |x: f64, z: f64| n.add_node(x, 0.0, z, NodeFlag::REGULAR);
        let inside_sq = add(5.0, 5.0);
        let outside_sq = add(15.0, 5.0);
        let left_arm = add(1.0, 5.0);
        let right_arm = add(9.0, 5.0);
        let notch = add(5.0, 8.0);
        let bottom = add(5.0, 1.0);
        let square = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        assert_eq!(
            n.in_polygon(&square),
            vec![inside_sq, left_arm, right_arm, notch, bottom]
        );
        // U shape opening towards +z: arms x 0..3 and 7..10, bottom z 0..3, notch x 3..7, z 3..10.
        let u = [
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 10.0),
            (7.0, 10.0),
            (7.0, 3.0),
            (3.0, 3.0),
            (3.0, 10.0),
            (0.0, 10.0),
        ];
        assert_eq!(n.in_polygon(&u), vec![left_arm, right_arm, bottom]);
        assert!(!n.in_polygon(&u).contains(&notch));
        assert!(!n.in_polygon(&u).contains(&outside_sq));
    }

    #[test]
    fn distance_bounds_are_inclusive() {
        let mut n = Network::new();
        let a = n.add_node(3.0, 0.0, 4.0, NodeFlag::REGULAR);
        let b = n.add_node(30.0, 0.0, 4.0, NodeFlag::REGULAR);
        let c = n.add_node(30.0, 0.0, 40.0, NodeFlag::REGULAR);
        n.connect(b, c, ConnectionShape::OneWay).expect("ok");
        assert_eq!(n.within(0.0, 0.0, 5.0), vec![a]);
        assert!(n.within(0.0, 0.0, 4.999).is_empty());
        // The segment b-c (x = 30) is exactly 25 away from (5, 20).
        assert_eq!(n.edges_near(5.0, 20.0, 25.0), vec![(b, c)]);
        assert!(n.edges_near(5.0, 20.0, 24.999).is_empty());
    }

    fn seg_dist2(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
        let (dx, dz) = (b.0 - a.0, b.1 - a.1);
        let len2 = dx * dx + dz * dz;
        let t = if len2 == 0.0 {
            0.0
        } else {
            (((p.0 - a.0) * dx + (p.1 - a.1) * dz) / len2).clamp(0.0, 1.0)
        };
        let (cx, cz) = (a.0 + t * dx, a.1 + t * dz);
        (p.0 - cx).powi(2) + (p.1 - cz).powi(2)
    }

    fn check_queries(n: &Network, rng: &mut Lcg) {
        let pos = |id: NodeId| n.node(id).map(|x| (x.x, x.z)).unwrap_or((0.0, 0.0));
        let mut all_edges = Vec::new();
        for id in n.order() {
            for t in n
                .node(*id)
                .map(|x| x.outgoing().to_vec())
                .unwrap_or_default()
            {
                all_edges.push((*id, t));
            }
        }
        all_edges.sort();
        assert_eq!(n.edges_near(0.0, 0.0, HUGE), all_edges);

        for _ in 0..40 {
            let (px, pz) = (rng.coord() + 0.37, rng.coord() + 0.61);
            let want = n
                .order()
                .iter()
                .map(|id| {
                    let (x, z) = pos(*id);
                    (x - px).powi(2) + (z - pz).powi(2)
                })
                .fold(f64::INFINITY, f64::min);
            let got = n.nearest(px, pz).map(|id| {
                let (x, z) = pos(id);
                (x - px).powi(2) + (z - pz).powi(2)
            });
            assert_eq!(got, Some(want));

            let (x0, z0, x1, z1) = (rng.coord(), rng.coord(), rng.coord(), rng.coord());
            let (lo_x, hi_x, lo_z, hi_z) = (x0.min(x1), x0.max(x1), z0.min(z1), z0.max(z1));
            let want: Vec<NodeId> = n
                .order()
                .iter()
                .copied()
                .filter(|id| {
                    let (x, z) = pos(*id);
                    x >= lo_x && x <= hi_x && z >= lo_z && z <= hi_z
                })
                .collect();
            let mut want = want;
            want.sort();
            assert_eq!(n.in_rect(x0, z0, x1, z1), want);

            let poly: Vec<(f64, f64)> = (0..5)
                .map(|_| (rng.coord() + 0.5, rng.coord() + 0.5))
                .collect();
            let mut want: Vec<NodeId> = n
                .order()
                .iter()
                .copied()
                .filter(|id| {
                    let (x, z) = pos(*id);
                    point_in_polygon(x, z, &poly)
                })
                .collect();
            want.sort();
            assert_eq!(n.in_polygon(&poly), want);

            let d = 7.3 + rng.below(20) as f64;
            let mut want: Vec<NodeId> = n
                .order()
                .iter()
                .copied()
                .filter(|id| {
                    let (x, z) = pos(*id);
                    (x - px).powi(2) + (z - pz).powi(2) <= d * d
                })
                .collect();
            want.sort();
            assert_eq!(n.within(px, pz, d), want);

            let want: Vec<(NodeId, NodeId)> = all_edges
                .iter()
                .copied()
                .filter(|(a, b)| seg_dist2((px, pz), pos(*a), pos(*b)) <= d * d)
                .collect();
            assert_eq!(n.edges_near(px, pz, d), want);
        }
    }

    #[test]
    fn queries_match_brute_force_after_connects_and_moves() {
        let mut rng = Lcg(42);
        let mut n = Network::new();
        let ids: Vec<NodeId> = (0..500)
            .map(|_| n.add_node(rng.coord(), 0.0, rng.coord(), NodeFlag::REGULAR))
            .collect();
        let pick = |rng: &mut Lcg| ids[rng.below(ids.len() as u64) as usize];
        for _ in 0..800 {
            let (a, b) = (pick(&mut rng), pick(&mut rng));
            let shape = match rng.below(3) {
                0 => ConnectionShape::OneWay,
                1 => ConnectionShape::Dual,
                _ => ConnectionShape::Reverse,
            };
            let _ = n.connect(a, b, shape);
        }
        check_queries(&n, &mut rng);
        for _ in 0..200 {
            let id = pick(&mut rng);
            let _ = n.move_node(id, rng.coord(), 1.0, rng.coord());
        }
        for _ in 0..100 {
            let (a, b) = (pick(&mut rng), pick(&mut rng));
            let _ = n.disconnect(a, b);
            let (a, b) = (pick(&mut rng), pick(&mut rng));
            let _ = n.set_shape(a, b, ConnectionShape::OneWay);
        }
        check_queries(&n, &mut rng);

        let loaded: Vec<LoadedNode> = n
            .order()
            .iter()
            .filter_map(|id| n.node(*id))
            .map(|x| LoadedNode {
                x: x.x,
                y: x.y,
                z: x.z,
                flag: x.flag,
                outgoing: x.outgoing().iter().map(|t| t.get() as usize).collect(),
                incoming: x.incoming().iter().map(|t| t.get() as usize).collect(),
            })
            .collect();
        let reloaded = Network::from_loaded(loaded).expect("ok");
        check_queries(&reloaded, &mut rng);
    }
}
