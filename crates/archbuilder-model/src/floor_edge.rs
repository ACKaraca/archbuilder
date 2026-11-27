//! Floor edges: a profile swept along selected edges of one floor (slab edges, curbs, nosings).
//!
//! The record holds the authored data only: a profile, a closed loop of points `(u, v)` in the
//! plane square to the edge (`u` outward from the slab, away from its material; `v` up from the top
//! of the slab at the edge, down when negative), and the edges, each by the number of the vertex
//! it starts at in the floor's vertex list. The floor owns the record (the header owner).
//!
//! The sweep is derived each time ([`Derived::of`]): the floor's boundary as derived (edges that
//! follow walls move with them), a sloped floor's top included. Consecutive edges of one loop make
//! one chain, mitered at the corners between them (a closed loop of selected edges has no ends); an
//! arc edge is followed in its flattened points. An edge number that names no edge of the floor
//! any more is an orphan: it is reported and never rebound to another edge.

use arc_base::math::Point3;
use arc_base::{Limits, ObjectId, Result, Tolerance};
use arc_doc::{Object, Snapshot, Value};
use arc_geom::{PolyFace, Polyhedron};

use crate::floor::{Boundary, FLAT_CHORD, Floor, flatten_edge};
use crate::spatial;
use crate::types::{self, floor_edge};

/// The most points of a profile.
pub const MAX_PROFILE: usize = 64;
/// The range of a profile coordinate, in meters.
pub const PROFILE_RANGE: (f64, f64) = (-5.0, 5.0);
/// The most corner points of all chains of one element.
const MAX_POINTS: usize = 20_000;
/// The longest miter, as a multiple of the profile offset: sharper corners are cut square.
const MAX_MITER: f64 = 8.0;

/// The authored data of a floor edge element, lengths in meters.
#[derive(Debug, Clone, PartialEq)]
pub struct FloorEdge {
    /// The profile `(u, v)`, in order.
    pub profile: Vec<[f64; 2]>,
    /// The edges swept, by the number of the vertex each starts at.
    pub edges: Vec<usize>,
}

impl FloorEdge {
    /// The floor edge of a record's field values; `None` when a field is missing or malformed.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Option<Self> {
        let Some(Some(Value::List(points))) = values.get(floor_edge::PROFILE) else {
            return None;
        };
        let Some(Some(Value::List(edges))) = values.get(floor_edge::EDGES) else {
            return None;
        };
        if points.len() > MAX_PROFILE || edges.len() > crate::floor::MAX_VERTICES {
            return None;
        }
        let profile = points
            .iter()
            .map(|p| match p {
                Value::Record(f) => Some([types::meters(f, 0)?, types::meters(f, 1)?]),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        let edges = edges
            .iter()
            .map(|e| match e {
                Value::I64(n) => usize::try_from(*n).ok(),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self { profile, edges })
    }

    /// The record's value of a profile.
    #[must_use]
    pub fn profile_value(profile: &[[f64; 2]]) -> Value {
        Value::List(
            profile
                .iter()
                .map(|p| Value::Record(vec![Some(types::length(p[0])), Some(types::length(p[1]))]))
                .collect(),
        )
    }

    /// The record's value of a list of edges.
    #[must_use]
    pub fn edges_value(edges: &[usize]) -> Value {
        Value::List(
            edges
                .iter()
                .map(|e| Value::I64(i64::try_from(*e).unwrap_or(i64::MAX)))
                .collect(),
        )
    }

    /// Whether the profile can be swept: at least three points and a simple loop with an area.
    #[must_use]
    pub fn profile_ok(profile: &[[f64; 2]]) -> bool {
        profile.len() >= 3
            && profile.len() <= MAX_PROFILE
            && profile
                .iter()
                .flatten()
                .all(|v| v.is_finite() && v.abs() <= PROFILE_RANGE.1)
            && Boundary::polygons(vec![profile.to_vec()]).check().is_ok()
    }
}

/// What a floor edge element reads from a snapshot.
#[derive(Debug, Clone)]
pub struct Derived {
    /// The authored data.
    pub edge: FloorEdge,
    /// The floor that owns it.
    pub floor: ObjectId,
    /// The edge numbers that name no edge of the floor.
    pub orphans: Vec<usize>,
    /// The chains of corner points the profile is swept along, each with whether it closes.
    pub chains: Vec<Chain>,
}

/// One run of consecutive edges: its corner points with the elevation of the slab's top there,
/// and the outward unit normal of each segment.
#[derive(Debug, Clone, PartialEq)]
pub struct Chain {
    /// The corner points `(x, y, z)`, the last one repeating the first for a closed chain.
    pub points: Vec<[f64; 3]>,
    /// Whether the chain is a whole loop: no ends.
    pub closed: bool,
    /// The outward normal of the segment that starts at each point but the last.
    pub normals: Vec<[f64; 2]>,
    /// The plane of the slab's top, which the profile's `v` is measured from, also outward of the
    /// edge (a sheared profile keeps every face planar).
    pub top: crate::terrain::Plane,
}

/// The floor that owns `object`, read.
fn host_of(
    snapshot: &Snapshot,
    object: &Object,
) -> Option<(ObjectId, Floor, crate::floor::Derived)> {
    let host = spatial::get(snapshot, object.header.owner?)?;
    let derived = crate::floor::Derived::of(snapshot, &host)?;
    Some((host.id, derived.floor.clone(), derived))
}

/// The start index in the vertex list of every loop, and the number of its vertices.
fn loop_spans(floor: &Floor) -> Vec<(usize, usize)> {
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut current = None;
    for (k, v) in floor.vertices.iter().enumerate() {
        match (spans.last_mut(), current) {
            (Some((_, n)), Some(l)) if l == v.loop_index => *n += 1,
            _ => {
                current = Some(v.loop_index);
                spans.push((k, 1));
            }
        }
    }
    spans
}

fn unit(a: [f64; 2], b: [f64; 2]) -> Option<[f64; 2]> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l = dx.hypot(dy);
    (l > 1.0e-9).then(|| [dx / l, dy / l])
}

/// The chains of the selected edges (`selected` by vertex number) of the loops `spans` of
/// `boundary`, at the top plane `top` of the slab.
fn chains_of(
    boundary: &Boundary,
    spans: &[(usize, usize)],
    selected: &[bool],
    top: crate::terrain::Plane,
) -> Vec<Chain> {
    let mut chains = Vec::new();
    let mut budget = MAX_POINTS;
    for (l, (first, n)) in spans.iter().enumerate() {
        let Some(points) = boundary.loops.get(l) else {
            continue;
        };
        if points.len() != *n || *n < 2 {
            continue;
        }
        // Away from the slab's material: the right of a counterclockwise outer loop, the left
        // of a counterclockwise hole.
        let ccw = crate::floor::signed_area(points) > 0.0;
        let right = ccw == (l == 0);
        let on = |k: usize| selected.get(first + k).copied().unwrap_or(false);
        let all = (0..*n).all(on);
        // A chain starts at a selected edge whose predecessor is not selected.
        let starts: Vec<usize> = if all {
            vec![0]
        } else {
            (0..*n).filter(|k| on(*k) && !on((k + n - 1) % n)).collect()
        };
        for start in starts {
            let mut chain = Chain {
                points: Vec::new(),
                closed: all,
                normals: Vec::new(),
                top,
            };
            let mut k = start;
            loop {
                let (a, b) = (points[k], points[(k + 1) % n]);
                let bulge = boundary.bulge(l, k);
                let mut run = flatten_edge(a, b, bulge, FLAT_CHORD);
                run.push(b);
                for w in run.windows(2) {
                    let Some(d) = unit(w[0], w[1]) else {
                        continue;
                    };
                    let normal = if right { [d[1], -d[0]] } else { [-d[1], d[0]] };
                    if chain.points.is_empty() {
                        chain.points.push(at_plane(&top, w[0]));
                    }
                    chain.points.push(at_plane(&top, w[1]));
                    chain.normals.push(normal);
                }
                k = (k + 1) % n;
                if k == start || !on(k) {
                    break;
                }
            }
            budget = budget.saturating_sub(chain.points.len());
            if chain.normals.is_empty() {
                continue;
            }
            chains.push(chain);
            if budget == 0 {
                break;
            }
        }
    }
    chains
}

/// The point `p` of the plan at the elevation of the slab's top, `plane`.
fn at_plane(plane: &crate::terrain::Plane, p: [f64; 2]) -> [f64; 3] {
    [p[0], p[1], plane.z(p)]
}

/// The chain of the boundary edge `edge` (the one that starts at that vertex number) of the floor
/// `floor`, or, without a number, of the whole outer loop, at the top of the slab with arcs
/// flattened; `None` when the floor is unreadable or has no such edge. Heights are absolute.
#[must_use]
pub fn boundary_chain(snapshot: &Snapshot, floor: &Object, edge: Option<usize>) -> Option<Chain> {
    let data = Floor::from_values(&floor.values)?;
    let top = crate::floor::Derived::of(snapshot, floor)?.plane;
    let boundary = data.boundary_in(snapshot)?;
    let spans = loop_spans(&data);
    let mut selected = vec![false; data.vertices.len()];
    match (edge, spans.first()) {
        (Some(e), _) => *selected.get_mut(e)? = true,
        (None, Some((first, n))) => {
            selected
                .get_mut(*first..first + n)?
                .iter_mut()
                .for_each(|s| *s = true);
        }
        (None, None) => return None,
    }
    chains_of(&boundary, &spans, &selected, top)
        .into_iter()
        .next()
}

impl Derived {
    /// Reads the floor edge `object` of `snapshot`; `None` when it or its floor is unreadable.
    #[must_use]
    pub fn of(snapshot: &Snapshot, object: &Object) -> Option<Self> {
        let edge = FloorEdge::from_values(&object.values)?;
        let (floor_id, floor, top) = host_of(snapshot, object)?;
        let boundary = floor.boundary_in(snapshot)?;
        let spans = loop_spans(&floor);
        let mut selected = vec![false; floor.vertices.len()];
        let mut orphans = Vec::new();
        for e in &edge.edges {
            match selected.get_mut(*e) {
                Some(s) => *s = true,
                None => orphans.push(*e),
            }
        }
        let chains = chains_of(&boundary, &spans, &selected, top.plane);
        Some(Self {
            edge,
            floor: floor_id,
            orphans,
            chains,
        })
    }

    /// The solid: the profile swept along each chain, as one solid with a shell for each chain.
    ///
    /// # Errors
    /// The solid kernel's errors when a shell is not closed (a profile that turns inside out at a
    /// sharp corner).
    pub fn solid(&self) -> Result<Option<Polyhedron>> {
        let mut vertices: Vec<Point3> = Vec::new();
        let mut faces: Vec<PolyFace> = Vec::new();
        for chain in &self.chains {
            let first = vertices.len();
            let sections = sections(chain, &self.edge.profile);
            let count = sections.len();
            let m = self.edge.profile.len();
            for s in &sections {
                vertices.extend(s.iter().map(|p| Point3::new(p[0], p[1], p[2])));
            }
            let at = |s: usize, j: usize| u32::try_from(first + s * m + j % m).unwrap_or(u32::MAX);
            let mut shell: Vec<Vec<u32>> = Vec::new();
            let rings = if chain.closed { count } else { count - 1 };
            for s in 0..rings {
                for j in 0..m {
                    shell.push(vec![
                        at(s, j),
                        at(s, j + 1),
                        at((s + 1) % count, j + 1),
                        at((s + 1) % count, j),
                    ]);
                }
            }
            if !chain.closed {
                shell.push((0..m).rev().map(|j| at(0, j)).collect());
                shell.push((0..m).map(|j| at(count - 1, j)).collect());
            }
            faces.extend(shell.into_iter().map(PolyFace::new));
        }
        if faces.is_empty() {
            return Ok(None);
        }
        let mut solid = Polyhedron::new(
            vertices.clone(),
            faces.clone(),
            &Tolerance::DEFAULT,
            &Limits::DEFAULT,
        );
        // The profile may run either way round: the same shells turned inside out are the solid.
        if solid.as_ref().is_ok_and(|s| s.volume() < 0.0) || solid.is_err() {
            let flipped = faces
                .into_iter()
                .map(|f| PolyFace::new(f.outer.into_iter().rev().collect()))
                .collect();
            solid = Polyhedron::new(vertices, flipped, &Tolerance::DEFAULT, &Limits::DEFAULT);
        }
        solid.map(Some)
    }

    /// The length of the edges swept, in meters, along the slab's boundary in plan.
    #[must_use]
    pub fn length(&self) -> f64 {
        self.chains
            .iter()
            .flat_map(|c| c.points.windows(2))
            .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
            .sum()
    }

    /// The area of the profile, in square meters.
    #[must_use]
    pub fn profile_area(&self) -> f64 {
        crate::floor::signed_area(&self.edge.profile).abs()
    }

    /// The volume of the swept solid, in cubic meters.
    #[must_use]
    pub fn volume(&self) -> f64 {
        self.solid()
            .ok()
            .flatten()
            .map_or(0.0, |s| s.volume().abs())
    }

    /// The lowest and the highest elevation of the solid.
    #[must_use]
    pub fn z_range(&self) -> Option<(f64, f64)> {
        let sections: Vec<_> = self
            .chains
            .iter()
            .flat_map(|c| sections(c, &self.edge.profile))
            .flatten()
            .collect();
        let lo = sections.iter().map(|p| p[2]).fold(f64::MAX, f64::min);
        let hi = sections.iter().map(|p| p[2]).fold(f64::MIN, f64::max);
        (lo <= hi).then_some((lo, hi))
    }
}

/// The profile placed at each corner point of `chain`: mitered between two segments, square at an
/// end. Each section lists the profile's points in order.
fn sections(chain: &Chain, profile: &[[f64; 2]]) -> Vec<Vec<[f64; 3]>> {
    let n = chain.normals.len();
    let corners = if chain.closed { n } else { n + 1 };
    (0..corners)
        .map(|k| {
            let into = if chain.closed {
                chain.normals[(k + n - 1) % n]
            } else if k == 0 {
                chain.normals[0]
            } else {
                chain.normals[k - 1]
            };
            let out = if !chain.closed && k == n {
                chain.normals[n - 1]
            } else {
                chain.normals[k % n]
            };
            let c = 1.0 + into[0] * out[0] + into[1] * out[1];
            // The miter vector: the offset that is `1` square to both segments.
            let m = if c > 1.0 / (MAX_MITER * MAX_MITER) * 2.0 {
                [(into[0] + out[0]) / c, (into[1] + out[1]) / c]
            } else {
                into
            };
            let p = chain.points[k];
            profile
                .iter()
                .map(|q| {
                    let at = [p[0] + m[0] * q[0], p[1] + m[1] * q[0]];
                    [at[0], at[1], chain.top.z(at) + q[1]]
                })
                .collect()
        })
        .collect()
}

/// The edge numbers of `host` the elements of `object`'s floor do not find: orphans, for the
/// warnings register.
#[must_use]
pub fn orphans_of(snapshot: &Snapshot, object: &Object) -> Vec<usize> {
    let (Some(edge), Some(host)) = (
        FloorEdge::from_values(&object.values),
        object.header.owner.and_then(|h| spatial::get(snapshot, h)),
    ) else {
        return Vec::new();
    };
    let count = Floor::from_values(&host.values).map_or(0, |f| f.vertices.len());
    edge.edges.into_iter().filter(|e| *e >= count).collect()
}
