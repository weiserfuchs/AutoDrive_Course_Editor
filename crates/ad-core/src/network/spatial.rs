//! R-tree indexes and the spatial queries over them.

use rstar::AABB;
use rstar::primitives::{GeomWithData, Line};

use super::{ConnectionKind, Network, Node, NodeFlag, NodeId, look_of};

/// One indexed edge: its node ids and its endpoints as (x, z).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeSegment {
    pub from: NodeId,
    pub to: NodeId,
    pub a: [f64; 2],
    pub b: [f64; 2],
    pub look: LinkLook,
}

/// What drawing needs to know about a link: its kind and the flags of both ends. Cached in
/// the edge index so the view does no node lookups per edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkLook {
    pub kind: ConnectionKind,
    pub from_flag: NodeFlag,
    pub to_flag: NodeFlag,
}

/// Payload of an edge-index entry. Equality is by ids only, so an entry is found and
/// removed without knowing its current look.
#[derive(Debug, Clone, Copy)]
pub(super) struct EdgeData {
    pub from: NodeId,
    pub to: NodeId,
    pub look: LinkLook,
}

impl PartialEq for EdgeData {
    fn eq(&self, other: &Self) -> bool {
        self.from == other.from && self.to == other.to
    }
}

pub(super) type NodeEntry = GeomWithData<[f64; 2], NodeId>;
pub(super) type EdgeEntry = GeomWithData<Line<[f64; 2]>, EdgeData>;

pub(super) fn edge_entry(
    from: &Node,
    to: &Node,
    ids: (NodeId, NodeId),
    look: LinkLook,
) -> EdgeEntry {
    GeomWithData::new(
        Line::new([from.x, from.z], [to.x, to.z]),
        EdgeData {
            from: ids.0,
            to: ids.1,
            look,
        },
    )
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
            // The look does not take part in equality.
            let look = LinkLook {
                kind: ConnectionKind::None,
                from_flag: from.flag,
                to_flag: to.flag,
            };
            let entry = edge_entry(from, to, (a, b), look);
            self.edge_index.remove(&entry);
        }
    }

    /// Inserts an edge-index entry for `a -> b` when both nodes exist.
    pub(super) fn put_edge(&mut self, a: NodeId, b: NodeId) {
        if let (Some(from), Some(to), Some(look)) =
            (self.nodes.get(&a), self.nodes.get(&b), self.link_look(a, b))
        {
            self.edge_index.insert(edge_entry(from, to, (a, b), look));
        }
    }

    /// Recomputes the cached look of every edge entry that touches `id`. Call after any
    /// change to the node's flag or lists, once the edge entries themselves are in place.
    pub(super) fn refresh_looks(&mut self, id: NodeId) {
        let Some(node) = self.nodes.get(&id) else {
            return;
        };
        let point = [node.x, node.z];
        for e in self
            .edge_index
            .locate_in_envelope_intersecting_mut(AABB::from_point(point))
        {
            if (e.data.from == id || e.data.to == id)
                && let Some(look) = look_of(&self.nodes, e.data.from, e.data.to)
            {
                e.data.look = look;
            }
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

    /// Fills `out` with the nodes inside the rectangle, bounds inclusive. Unsorted;
    /// `out` is cleared first and its capacity reused.
    pub fn nodes_in_rect_into(&self, x0: f64, z0: f64, x1: f64, z1: f64, out: &mut Vec<NodeId>) {
        out.clear();
        let aabb = AABB::from_corners([x0, z0], [x1, z1]);
        out.extend(self.node_index.locate_in_envelope(aabb).map(|e| e.data));
    }

    /// Fills `out` with the edges whose segment's bounding box intersects the rectangle,
    /// so an edge crossing it with both ends outside is included. Unsorted; `out` is
    /// cleared first and its capacity reused.
    pub fn edges_in_rect_into(
        &self,
        x0: f64,
        z0: f64,
        x1: f64,
        z1: f64,
        out: &mut Vec<EdgeSegment>,
    ) {
        out.clear();
        let aabb = AABB::from_corners([x0, z0], [x1, z1]);
        // The index entries carry the endpoints, so drawing needs no node lookups.
        out.extend(
            self.edge_index
                .locate_in_envelope_intersecting(aabb)
                .map(|e| EdgeSegment {
                    from: e.data.from,
                    to: e.data.to,
                    a: e.geom().from,
                    b: e.geom().to,
                    look: e.data.look,
                }),
        );
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
            .map(|e| (e.data.from, e.data.to))
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
    fn rect_into_queries_match_brute_force() {
        let mut n = Network::new();
        let a = n.add_node(-50.0, 0.0, 5.0, NodeFlag::REGULAR);
        let b = n.add_node(50.0, 0.0, 5.0, NodeFlag::REGULAR);
        let c = n.add_node(5.0, 0.0, 5.0, NodeFlag::REGULAR);
        let d = n.add_node(100.0, 0.0, 100.0, NodeFlag::REGULAR);
        n.connect(a, b, ConnectionShape::OneWay).expect("ok");
        n.connect(c, d, ConnectionShape::OneWay).expect("ok");
        let regular = LinkLook {
            kind: ConnectionKind::Regular,
            from_flag: NodeFlag::REGULAR,
            to_flag: NodeFlag::REGULAR,
        };
        let stale = EdgeSegment {
            from: d,
            to: d,
            a: [0.0; 2],
            b: [0.0; 2],
            look: LinkLook {
                kind: ConnectionKind::None,
                from_flag: NodeFlag::REGULAR,
                to_flag: NodeFlag::REGULAR,
            },
        };
        let (mut nodes, mut edges) = (vec![d], vec![stale]);
        n.nodes_in_rect_into(0.0, 0.0, 10.0, 10.0, &mut nodes);
        n.edges_in_rect_into(0.0, 0.0, 10.0, 10.0, &mut edges);
        assert_eq!(nodes, vec![c]);
        edges.sort_by_key(|e| (e.from, e.to));
        assert_eq!(
            edges,
            vec![
                EdgeSegment {
                    from: a,
                    to: b,
                    a: [-50.0, 5.0],
                    b: [50.0, 5.0],
                    look: regular,
                },
                EdgeSegment {
                    from: c,
                    to: d,
                    a: [5.0, 5.0],
                    b: [100.0, 100.0],
                    look: regular,
                },
            ]
        );
        n.edges_in_rect_into(200.0, 200.0, 210.0, 210.0, &mut edges);
        assert!(edges.is_empty());

        let mut rng = Lcg(7);
        let mut big = Network::new();
        let ids: Vec<NodeId> = (0..300)
            .map(|_| big.add_node(rng.coord(), 0.0, rng.coord(), NodeFlag::REGULAR))
            .collect();
        for _ in 0..400 {
            let (p, q) = (ids[rng.below(300) as usize], ids[rng.below(300) as usize]);
            let _ = big.connect(p, q, ConnectionShape::OneWay);
        }
        let pos = |id: NodeId| big.node(id).map(|x| (x.x, x.z)).unwrap_or((0.0, 0.0));
        for _ in 0..30 {
            let (x0, z0) = (rng.coord(), rng.coord());
            let (x1, z1) = (x0 + rng.below(60) as f64, z0 + rng.below(60) as f64);
            let mut got = Vec::new();
            big.nodes_in_rect_into(x0, z0, x1, z1, &mut got);
            got.sort();
            assert_eq!(got, big.in_rect(x0, z0, x1, z1));
            let mut want = Vec::new();
            for id in big.order() {
                for t in big
                    .node(*id)
                    .map(|x| x.outgoing().to_vec())
                    .unwrap_or_default()
                {
                    let ((ax, az), (bx, bz)) = (pos(*id), pos(t));
                    if ax.min(bx) <= x1 && ax.max(bx) >= x0 && az.min(bz) <= z1 && az.max(bz) >= z0
                    {
                        want.push((*id, t));
                    }
                }
            }
            want.sort();
            let mut segments = Vec::new();
            big.edges_in_rect_into(x0, z0, x1, z1, &mut segments);
            let mut got: Vec<(NodeId, NodeId)> = segments.iter().map(|e| (e.from, e.to)).collect();
            got.sort();
            assert_eq!(got, want);
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

    /// Every indexed edge carries `link_look` of its ends, and the entries are exactly the
    /// adjacency.
    fn check_looks(n: &Network) {
        let mut want = Vec::new();
        for id in n.order() {
            for t in n
                .node(*id)
                .map(|x| x.outgoing().to_vec())
                .unwrap_or_default()
            {
                want.push((*id, t));
            }
        }
        want.sort();
        let mut got = Vec::new();
        for e in n.edge_index.iter() {
            assert_eq!(Some(e.data.look), n.link_look(e.data.from, e.data.to));
            got.push((e.data.from, e.data.to));
        }
        got.sort();
        assert_eq!(got, want);
    }

    #[test]
    fn cached_looks_follow_every_mutation() {
        let mut rng = Lcg(99);
        let mut n = Network::new();
        let ids: Vec<NodeId> = (0..40)
            .map(|_| n.add_node(rng.coord(), 0.0, rng.coord(), NodeFlag(rng.below(3) as u8)))
            .collect();
        let pick = |rng: &mut Lcg| ids[rng.below(ids.len() as u64) as usize];
        let shape = |rng: &mut Lcg| match rng.below(3) {
            0 => ConnectionShape::OneWay,
            1 => ConnectionShape::Dual,
            _ => ConnectionShape::Reverse,
        };
        for _ in 0..1500 {
            let (a, b) = (pick(&mut rng), pick(&mut rng));
            match rng.below(8) {
                0 => {
                    let sh = shape(&mut rng);
                    let _ = n.connect(a, b, sh);
                }
                1 => {
                    let _ = n.disconnect(a, b);
                }
                2 => {
                    let sh = shape(&mut rng);
                    let _ = n.set_shape(a, b, sh);
                }
                3 => {
                    let _ = n.set_flag(a, NodeFlag(rng.below(3) as u8));
                }
                4 => {
                    let _ = n.move_node(a, rng.coord(), 0.0, rng.coord());
                }
                5 => {
                    if let Ok(removed) = n.remove_node(a) {
                        check_looks(&n);
                        if rng.below(2) == 0 {
                            n.reinsert(removed);
                        }
                    }
                }
                6 => {
                    let sh = shape(&mut rng);
                    if let Ok(delta) = n.set_shape(a, b, sh) {
                        n.revert_links(&delta);
                    }
                }
                _ => {
                    let _ = n.connect(a, b, ConnectionShape::OneWay);
                }
            }
            check_looks(&n);
        }
    }

    /// The no-height set equals a full scan after random mutations.
    #[test]
    fn no_height_set_follows_every_mutation() {
        // The reference mirrors the network's no-height rule: exactly `y == -1.0`.
        let check = |n: &Network| {
            let mut want: Vec<NodeId> = n
                .order()
                .iter()
                .copied()
                .filter(|id| n.node(*id).is_some_and(|x| x.y == -1.0))
                .collect();
            want.sort();
            let got: Vec<NodeId> = n.nodes_without_height().collect();
            assert_eq!(got, want);
        };
        let mut rng = Lcg(7);
        let height = |rng: &mut Lcg| if rng.below(2) == 0 { -1.0 } else { 3.0 };
        let mut n = Network::new();
        let mut ids: Vec<NodeId> = (0..30)
            .map(|_| {
                let y = height(&mut rng);
                n.add_node(rng.coord(), y, rng.coord(), NodeFlag(0))
            })
            .collect();
        check(&n);
        for _ in 0..1000 {
            let a = ids[rng.below(ids.len() as u64) as usize];
            match rng.below(4) {
                0 => {
                    let y = height(&mut rng);
                    ids.push(n.add_node(rng.coord(), y, rng.coord(), NodeFlag(0)));
                }
                1 | 2 => {
                    let y = height(&mut rng);
                    let _ = n.move_node(a, rng.coord(), y, rng.coord());
                }
                _ => {
                    if let Ok(removed) = n.remove_node(a) {
                        check(&n);
                        if rng.below(2) == 0 {
                            n.reinsert(removed);
                        } else {
                            // Left out for good.
                            ids.retain(|i| *i != a);
                        }
                    }
                }
            }
            check(&n);
        }
        let loaded: Vec<LoadedNode> = n
            .order()
            .iter()
            .filter_map(|id| n.node(*id))
            .map(|x| LoadedNode {
                x: x.x,
                y: x.y,
                z: x.z,
                flag: x.flag,
                outgoing: Vec::new(),
                incoming: Vec::new(),
            })
            .collect();
        check(&Network::from_loaded(loaded).expect("ok"));
    }
}
