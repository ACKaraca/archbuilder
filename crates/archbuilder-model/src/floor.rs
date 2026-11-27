//! The floor (slab): authored data read from a record, the boundary derived from it, and the solid.
//!
//! A floor record holds a thickness, a top offset from the level of its storey (the slab hangs
//! down from its top) and the boundary as a list of vertices: the outer loop first, then the
//! holes, each loop in order, straight edges only. The edge that starts at a vertex runs to the
//! next vertex of its loop. Every vertex keeps its authored point; an edge may also be associated
//! with a wall face ([`Association`]).
//!
//! Associated edges are derived each time ([`Floor::boundary`]): the edge lies on the line of the
//! chosen face of the wall (the interior face, the exterior face or the core center line), moved across by the offset, and a vertex that touches
//! an associated edge is the intersection of the two lines of its edges. So the floor follows when
//! the wall moves or changes thickness. An edge that is not associated lies on the line through
//! its two authored points. [`Floor::frozen`] gives the vertices after a wall is deleted: the
//! edges that followed it keep their last derived position and lose the association.
//!
//! Nothing derived is stored: the area and the solid come from the boundary each time. The
//! boundary a floor shows ([`Derived`]) also has the shaft openings of the stairs that arrive at
//! its level taken out ([`Boundary::with_openings`]).

use std::collections::BTreeSet;

use arc_base::math::Point2;
use arc_base::{Limits, ObjectId, Result, Tolerance};
use arc_doc::{Snapshot, Value};
use arc_geom::{
    Curve2, Loop2, Polyhedron, Polyline2, PolylineSegment, PolylineVertex, Region, RegionPart,
    Segment2, prism,
};

use crate::types::{self, COORDINATE_RANGE, MIN_WALL_LENGTH, floor};
use crate::wall::Wall;

/// The most vertices in all loops of a floor.
pub const MAX_VERTICES: usize = 1000;
/// The most loops of a floor, the outer one included.
pub const MAX_LOOPS: usize = 64;
/// The least area of a floor, in square meters, and the largest.
pub const AREA_RANGE: (f64, f64) = (1.0e-4, 1.0e9);
/// The shortest edge of a floor, in meters: the model tolerance of a boundary.
pub const MIN_EDGE: f64 = MIN_WALL_LENGTH;
/// The most walls a click inside a closed loop of walls may pick, and the most pieces it searches.
const MAX_PICKED: usize = 1000;
/// How far from a wall's body a click still picks it, in meters.
const PICK_MARGIN: f64 = 0.05;
/// The slab's chord for the solid kernel; the boundary has no curves.
const CHORD: f64 = 0.01;

/// Which line of a wall an associated edge lies on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Face {
    /// The interior face of the wall (the right of its baseline's direction).
    #[default]
    Interior,
    /// The exterior face (the left of the baseline's direction).
    Exterior,
    /// The middle of the core of the wall's type.
    Center,
    /// The face of the core on the right of the baseline's direction (the interior side).
    CoreInterior,
    /// The face of the core on the left of the baseline's direction (the exterior side).
    CoreExterior,
}

impl Face {
    /// Every face, in code order.
    pub const ALL: [Self; 5] = [
        Self::Interior,
        Self::Exterior,
        Self::Center,
        Self::CoreInterior,
        Self::CoreExterior,
    ];

    /// The name in the record.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Interior => "interior",
            Self::Exterior => "exterior",
            Self::Center => "center",
            Self::CoreInterior => "core_interior",
            Self::CoreExterior => "core_exterior",
        }
    }

    /// The code stored in the record.
    #[must_use]
    pub const fn code(self) -> u32 {
        match self {
            Self::Interior => 0,
            Self::Exterior => 1,
            Self::Center => 2,
            Self::CoreInterior => 3,
            Self::CoreExterior => 4,
        }
    }

    /// The face of a stored code; the default for a code that names none.
    #[must_use]
    pub fn from_code(code: u32) -> Self {
        Self::ALL
            .into_iter()
            .find(|f| f.code() == code)
            .unwrap_or_default()
    }

    /// The offset of this face across the baseline of `wall`, toward the left of its direction.
    #[must_use]
    pub fn across(self, wall: &Wall) -> f64 {
        let (low, high) = wall.extent();
        match self {
            Self::Interior => low,
            Self::Exterior => high,
            Self::Center => {
                let (low, high) = wall.core_extent();
                (low + high) / 2.0
            }
            Self::CoreInterior => wall.core_extent().0,
            Self::CoreExterior => wall.core_extent().1,
        }
    }
}

/// The association of an edge with a wall.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Association {
    /// The wall.
    pub wall: ObjectId,
    /// The face of the wall the edge lies on.
    pub face: Face,
    /// The distance the edge is moved from that face toward the left of the wall's baseline
    /// direction, in meters.
    pub offset: f64,
}

/// One vertex of the boundary: its authored point and the association of the edge that starts
/// there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    /// The loop: 0 for the outer loop, 1 and up for the holes.
    pub loop_index: u32,
    /// The authored point, in meters.
    pub point: [f64; 2],
    /// The wall the edge from this vertex follows, when it does.
    pub edge: Option<Association>,
    /// The bulge of the edge that starts here: the tangent of a quarter of its included angle,
    /// positive for an arc that bends counterclockwise, 0 for a straight edge. An arc edge
    /// follows no wall.
    pub bulge: f64,
}

/// The authored data of a floor, lengths in meters.
#[derive(Debug, Clone, PartialEq)]
pub struct Floor {
    /// The thickness of the slab.
    pub thickness: f64,
    /// The top's distance above the level of the storey.
    pub top_offset: f64,
    /// The vertices of all loops, loop by loop.
    pub vertices: Vec<Vertex>,
}

/// The derived boundary: the points of each loop, the outer loop first, and the arcs between them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Boundary {
    /// The loops; each lists its vertices once, without repeating the first. An arc edge is
    /// given by its chord here and by its bulge in [`Self::bulges`].
    pub loops: Vec<Vec<[f64; 2]>>,
    /// The bulge of the edge that starts at each vertex, like `loops` (see [`Vertex::bulge`]);
    /// empty, or short, for straight edges.
    pub bulges: Vec<Vec<f64>>,
}

/// The least bulge that makes an edge an arc.
pub const MIN_BULGE: f64 = 1.0e-9;
/// The largest bulge of an arc edge: a sweep of about 337 degrees.
pub const MAX_BULGE: f64 = 10.0;
/// The chord tolerance, in meters, at which arcs are flattened to test, draw and export them.
pub const FLAT_CHORD: f64 = 0.002;

/// Why a boundary is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// A loop has fewer than three vertices, the record is malformed, or a count or number is out
    /// of its bounds.
    Malformed,
    /// An edge is shorter than the model tolerance.
    EdgeTooShort,
    /// A loop crosses or touches itself.
    SelfIntersecting,
    /// The outer loop has no area, or too much.
    Area,
    /// A hole is not inside the outer loop.
    HoleOutside,
    /// Two holes overlap, or a hole lies inside another.
    HolesOverlap,
}

/// A line: a point and a unit direction.
#[derive(Debug, Clone, Copy)]
struct Line {
    at: [f64; 2],
    dir: [f64; 2],
}

fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

fn cross(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

impl Line {
    fn through(a: [f64; 2], b: [f64; 2]) -> Option<Self> {
        let d = sub(b, a);
        let length = d[0].hypot(d[1]);
        (length.is_finite() && length > 0.0).then(|| Self {
            at: a,
            dir: [d[0] / length, d[1] / length],
        })
    }

    /// The line of a wall's face, moved by `offset` toward the left of its direction.
    fn of_face(wall: &Wall, face: Face, offset: f64) -> Option<Self> {
        let base = Self::through(wall.start, wall.end)?;
        let side = face.across(wall) + offset;
        Some(Self {
            at: [
                base.at[0] - base.dir[1] * side,
                base.at[1] + base.dir[0] * side,
            ],
            dir: base.dir,
        })
    }

    /// The point where two lines meet; `None` for lines that are (nearly) parallel.
    fn meet(&self, other: &Self) -> Option<[f64; 2]> {
        let denominator = cross(self.dir, other.dir);
        if denominator.abs() < 1.0e-9 {
            return None;
        }
        let t = cross(sub(other.at, self.at), other.dir) / denominator;
        Some([self.at[0] + self.dir[0] * t, self.at[1] + self.dir[1] * t])
    }

    /// The foot of the perpendicular from `p`.
    fn project(&self, p: [f64; 2]) -> [f64; 2] {
        let t = dot(sub(p, self.at), self.dir);
        [self.at[0] + self.dir[0] * t, self.at[1] + self.dir[1] * t]
    }
}

/// The index ranges of the loops of `vertices`, when the loop numbers run 0, 1, 2, ... in order.
fn loop_ranges(vertices: &[Vertex]) -> Option<Vec<std::ops::Range<usize>>> {
    let mut ranges: Vec<std::ops::Range<usize>> = Vec::new();
    for (at, v) in vertices.iter().enumerate() {
        match ranges.len() {
            n if u32::try_from(n).ok() == Some(v.loop_index) => ranges.push(at..at + 1),
            n if u32::try_from(n.checked_sub(1)?).ok() == Some(v.loop_index) => {
                ranges.last_mut()?.end = at + 1;
            }
            _ => return None,
        }
    }
    (!ranges.is_empty()).then_some(ranges)
}

/// Where a record keeps what a boundary reads: floors, ceilings, shafts and vertical openings
/// share the vertex and arc records and differ in the positions of the fields.
#[derive(Debug, Clone, Copy)]
pub struct Slots {
    /// The thickness, when the record has one.
    pub thickness: Option<usize>,
    /// The top offset, when the record has one.
    pub top: Option<usize>,
    /// The vertex list.
    pub vertices: usize,
    /// The arc list.
    pub arcs: usize,
}

/// The slots of a floor; a ceiling has the same.
pub const FLOOR_SLOTS: Slots = Slots {
    thickness: Some(floor::THICKNESS),
    top: Some(floor::TOP_OFFSET),
    vertices: floor::VERTICES,
    arcs: floor::ARCS,
};

/// The slots of a shaft.
pub const SHAFT_SLOTS: Slots = Slots {
    thickness: None,
    top: None,
    vertices: types::shaft::VERTICES,
    arcs: types::shaft::ARCS,
};

/// The slots of a vertical opening.
pub const VERTICAL_SLOTS: Slots = Slots {
    thickness: None,
    top: None,
    vertices: types::vertical_opening::VERTICES,
    arcs: types::vertical_opening::ARCS,
};

/// The slots of the element type `key`, for the elements whose boundary is a sketch: floors,
/// ceilings, shafts and vertical openings.
#[must_use]
pub fn slots_of(key: &str) -> Option<Slots> {
    match key {
        types::FLOOR | types::CEILING => Some(FLOOR_SLOTS),
        types::SHAFT => Some(SHAFT_SLOTS),
        types::VERTICAL_OPENING => Some(VERTICAL_SLOTS),
        _ => None,
    }
}

impl Floor {
    /// The floor of a record's field values; `None` when a field is missing or malformed.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Option<Self> {
        Self::read(values, FLOOR_SLOTS)
    }

    /// The boundary data of a record whose fields are at `slots`; the thickness and the top
    /// offset are 0 for a record without them.
    #[must_use]
    pub fn read(values: &[Option<Value>], slots: Slots) -> Option<Self> {
        let thickness = match slots.thickness {
            Some(at) => types::meters(values, at)?,
            None => 0.0,
        };
        let top_offset = match slots.top {
            Some(at) => types::meters(values, at)?,
            None => 0.0,
        };
        let Some(Some(Value::List(items))) = values.get(slots.vertices) else {
            return None;
        };
        let mut vertices = Vec::with_capacity(items.len().min(MAX_VERTICES));
        for item in items.iter().take(MAX_VERTICES + 1) {
            let Value::Record(fields) = item else {
                return None;
            };
            let m = |at| types::meters(fields, at);
            let loop_index = match fields.get(floor::vertex::LOOP)? {
                Some(Value::I64(n)) => u32::try_from(*n).ok()?,
                _ => return None,
            };
            let edge = match fields.get(floor::vertex::WALL) {
                Some(Some(Value::Reference(wall))) => Some(Association {
                    wall: *wall,
                    face: match fields.get(floor::vertex::FACE) {
                        Some(Some(Value::Enum(code))) => Face::from_code(*code),
                        _ => Face::default(),
                    },
                    offset: m(floor::vertex::OFFSET).unwrap_or(0.0),
                }),
                _ => None,
            };
            vertices.push(Vertex {
                loop_index,
                point: [m(floor::vertex::X)?, m(floor::vertex::Y)?],
                edge,
                bulge: 0.0,
            });
        }
        // The arcs are a list of their own, so that files from before them load as they were.
        if let Some(Some(Value::List(arcs))) = values.get(slots.arcs) {
            for arc in arcs {
                if let Value::Record(fields) = arc
                    && let (Some(Some(Value::I64(at))), Some(Some(Value::F64(bulge)))) =
                        (fields.first(), fields.get(1))
                    && let Some(v) = usize::try_from(*at)
                        .ok()
                        .and_then(|at| vertices.get_mut(at))
                {
                    v.bulge = *bulge;
                }
            }
        }
        Some(Self {
            thickness,
            top_offset,
            vertices,
        })
    }

    /// The vertex records of `vertices`, for the record's list field.
    #[must_use]
    pub fn vertex_values(vertices: &[Vertex]) -> Value {
        Value::List(
            vertices
                .iter()
                .map(|v| {
                    Value::Record(vec![
                        Some(Value::I64(i64::from(v.loop_index))),
                        Some(types::length(v.point[0])),
                        Some(types::length(v.point[1])),
                        v.edge.map(|e| Value::Reference(e.wall)),
                        Some(Value::Enum(v.edge.map_or(0, |e| e.face.code()))),
                        Some(types::length(v.edge.map_or(0.0, |e| e.offset))),
                    ])
                })
                .collect(),
        )
    }

    /// The value of the record's arc list for `vertices`; `None` when no edge is an arc.
    #[must_use]
    pub fn arc_values(vertices: &[Vertex]) -> Option<Value> {
        let arcs: Vec<Value> = vertices
            .iter()
            .enumerate()
            .filter(|(_, v)| v.bulge.abs() >= MIN_BULGE)
            .map(|(at, v)| {
                Value::Record(vec![
                    Some(Value::I64(i64::try_from(at).unwrap_or(i64::MAX))),
                    Some(Value::F64(v.bulge)),
                ])
            })
            .collect();
        (!arcs.is_empty()).then_some(Value::List(arcs))
    }

    /// The walls the edges follow, each once, in the order of the vertices.
    #[must_use]
    pub fn walls(&self) -> Vec<ObjectId> {
        let mut seen = Vec::new();
        for id in self.vertices.iter().filter_map(|v| v.edge.map(|e| e.wall)) {
            if !seen.contains(&id) {
                seen.push(id);
            }
        }
        seen
    }

    /// The line of the edge that starts at vertex `at` of the loop `range`.
    fn edge_line(
        &self,
        range: &std::ops::Range<usize>,
        at: usize,
        wall_of: &dyn Fn(ObjectId) -> Option<Wall>,
    ) -> Option<Line> {
        let v = self.vertices.get(range.start + at)?;
        if v.bulge.abs() < MIN_BULGE
            && let Some(a) = v.edge
            && let Some(wall) = wall_of(a.wall)
        {
            return Line::of_face(&wall, a.face, a.offset);
        }
        let next = self.vertices.get(range.start + (at + 1) % range.len())?;
        Line::through(v.point, next.point)
    }

    /// Whether the edge that starts at vertex `at` follows a wall that `wall_of` finds.
    fn follows(
        &self,
        range: &std::ops::Range<usize>,
        at: usize,
        wall_of: &dyn Fn(ObjectId) -> Option<Wall>,
    ) -> bool {
        self.vertices
            .get(range.start + at)
            .filter(|v| v.bulge.abs() < MIN_BULGE)
            .and_then(|v| v.edge)
            .is_some_and(|a| wall_of(a.wall).is_some())
    }

    /// The boundary: the authored points, with the vertices of associated edges derived from the
    /// walls `wall_of` finds (an association whose wall is missing counts as none). `None` for a
    /// malformed record or an edge that has no line.
    #[must_use]
    pub fn boundary(&self, wall_of: &dyn Fn(ObjectId) -> Option<Wall>) -> Option<Boundary> {
        let ranges = loop_ranges(&self.vertices)?;
        let mut loops = Vec::with_capacity(ranges.len());
        let mut bulges = Vec::with_capacity(ranges.len());
        for range in &ranges {
            let n = range.len();
            let mut points = Vec::with_capacity(n);
            let mut arcs = Vec::with_capacity(n);
            for at in 0..n {
                let own = self.vertices.get(range.start + at)?.point;
                let before = (at + n - 1) % n;
                let (a, b) = (
                    self.follows(range, before, wall_of),
                    self.follows(range, at, wall_of),
                );
                let arc_before = self
                    .vertices
                    .get(range.start + before)
                    .is_some_and(|v| v.bulge.abs() >= MIN_BULGE);
                let arc_at = self
                    .vertices
                    .get(range.start + at)
                    .is_some_and(|v| v.bulge.abs() >= MIN_BULGE);
                let point = if a || b {
                    let l0 = self.edge_line(range, before, wall_of)?;
                    let l1 = self.edge_line(range, at, wall_of)?;
                    // Two lines meet where they cross: a wall's edge and the free edge beside it
                    // (at any angle) too. An arc has no line, so the vertex stays on the line of
                    // the wall's edge, at the point nearest to where it was drawn; parallel
                    // edges stay on the wall's line.
                    match (a, b) {
                        (true, true) => l0.meet(&l1).unwrap_or_else(|| l1.project(own)),
                        (true, false) if arc_at => l0.project(own),
                        (false, true) if arc_before => l1.project(own),
                        (true, false) => l0.meet(&l1).unwrap_or_else(|| l0.project(own)),
                        (false, _) => l0.meet(&l1).unwrap_or_else(|| l1.project(own)),
                    }
                } else {
                    own
                };
                points.push(point);
                arcs.push(self.vertices.get(range.start + at)?.bulge);
            }
            loops.push(points);
            bulges.push(arcs);
        }
        Some(Boundary { loops, bulges })
    }

    /// The boundary read from the record's own wall data in `snapshot`.
    #[must_use]
    pub fn boundary_in(&self, snapshot: &Snapshot) -> Option<Boundary> {
        self.boundary(&|id| {
            crate::spatial::get(snapshot, id).and_then(|o| Wall::from_values(&o.values))
        })
    }

    /// The vertices after the walls `deleted` are gone: each edge that followed one keeps the
    /// position it has now (`boundary`, derived while the wall still exists) and loses the
    /// association. `None` when the boundary cannot be derived.
    #[must_use]
    pub fn frozen(&self, boundary: &Boundary, deleted: &BTreeSet<ObjectId>) -> Option<Vec<Vertex>> {
        let ranges = loop_ranges(&self.vertices)?;
        let mut out = self.vertices.clone();
        for (range, points) in ranges.iter().zip(&boundary.loops) {
            let n = range.len();
            for at in 0..n {
                let lost = self.vertices.get(range.start + at)?.edge;
                if lost.is_some_and(|a| deleted.contains(&a.wall)) {
                    // The line through both derived ends is the edge's last position.
                    for k in [at, (at + 1) % n] {
                        out.get_mut(range.start + k)?.point = *points.get(k)?;
                    }
                    out.get_mut(range.start + at)?.edge = None;
                }
            }
        }
        Some(out)
    }
}

impl Floor {
    /// The walls whose associated edge no longer lies along them: the derived edge runs past
    /// either end of the wall by more than the wall's thickness plus the edge's offset. A floor
    /// whose boundary cannot be read has none.
    #[must_use]
    pub fn unsupported_edges(
        &self,
        boundary: &Boundary,
        wall_of: &dyn Fn(ObjectId) -> Option<Wall>,
    ) -> Vec<ObjectId> {
        let mut out = Vec::new();
        let Some(ranges) = loop_ranges(&self.vertices) else {
            return out;
        };
        for (range, points) in ranges.iter().zip(&boundary.loops) {
            let n = range.len();
            for at in 0..n {
                let Some(a) = self.vertices.get(range.start + at).and_then(|v| v.edge) else {
                    continue;
                };
                let (Some(wall), Some(p), Some(q)) =
                    (wall_of(a.wall), points.get(at), points.get((at + 1) % n))
                else {
                    continue;
                };
                let length = wall.length();
                if length <= 0.0 {
                    continue;
                }
                let u = [
                    (wall.end[0] - wall.start[0]) / length,
                    (wall.end[1] - wall.start[1]) / length,
                ];
                let s =
                    |pt: &[f64; 2]| (pt[0] - wall.start[0]) * u[0] + (pt[1] - wall.start[1]) * u[1];
                let margin = wall.thickness + a.offset.abs() + 0.05;
                let (lo, hi) = (s(p).min(s(q)), s(p).max(s(q)));
                if (lo < -margin || hi > length + margin) && !out.contains(&a.wall) {
                    out.push(a.wall);
                }
            }
        }
        out
    }
}

impl Floor {
    /// The vertices after the walls in `flips` (each with its wall before and after) were flipped:
    /// a flip reverses a baseline, so the left of its direction changes. Each association that
    /// follows such a wall takes the face and offset that keep its line where it is, preferring
    /// the swapped face (interior and exterior swap sides). `None` when nothing follows them.
    #[must_use]
    pub fn flipped(&self, flips: &[(ObjectId, Wall, Wall)]) -> Option<Vec<Vertex>> {
        let mut out = self.vertices.clone();
        let mut changed = false;
        for v in &mut out {
            let Some(a) = v.edge else { continue };
            let Some((_, old, new)) = flips.iter().find(|(id, _, _)| *id == a.wall) else {
                continue;
            };
            let target = -(a.face.across(old) + a.offset);
            let swapped = match a.face {
                Face::Interior => Face::Exterior,
                Face::Exterior => Face::Interior,
                Face::Center => Face::Center,
                Face::CoreInterior => Face::CoreExterior,
                Face::CoreExterior => Face::CoreInterior,
            };
            let mut best: Option<(f64, Face)> = None;
            for f in [swapped, a.face].into_iter().chain(Face::ALL) {
                let off = target - f.across(new);
                if best.is_none_or(|(b, _)| off.abs() + 1.0e-12 < b.abs()) {
                    best = Some((off, f));
                }
            }
            if let Some((offset, face)) = best {
                v.edge = Some(Association { face, offset, ..a });
                changed = true;
            }
        }
        changed.then_some(out)
    }
}

fn segments_touch(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let eps = 1.0e-9;
    let o = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| {
        let v = cross(sub(q, p), sub(r, p));
        if v.abs() <= eps {
            0
        } else if v > 0.0 {
            1
        } else {
            -1
        }
    };
    let on = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| {
        r[0] >= p[0].min(q[0]) - eps
            && r[0] <= p[0].max(q[0]) + eps
            && r[1] >= p[1].min(q[1]) - eps
            && r[1] <= p[1].max(q[1]) + eps
    };
    let (o1, o2, o3, o4) = (o(a, b, c), o(a, b, d), o(c, d, a), o(c, d, b));
    (o1 != o2 && o3 != o4)
        || (o1 == 0 && on(a, b, c))
        || (o2 == 0 && on(a, b, d))
        || (o3 == 0 && on(c, d, a))
        || (o4 == 0 && on(c, d, b))
}

/// The signed area of a loop (positive counterclockwise).
#[must_use]
pub fn signed_area(points: &[[f64; 2]]) -> f64 {
    let n = points.len();
    (0..n)
        .map(|k| cross(points[k], points[(k + 1) % n]))
        .sum::<f64>()
        / 2.0
}

fn inside(points: &[[f64; 2]], p: [f64; 2]) -> bool {
    let n = points.len();
    let mut result = false;
    for k in 0..n {
        let (a, b) = (points[k], points[(k + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            result = !result;
        }
    }
    result
}

/// Whether two loops cross or touch each other.
fn loops_touch(a: &[[f64; 2]], b: &[[f64; 2]]) -> bool {
    (0..a.len()).any(|i| {
        (0..b.len()).any(|j| segments_touch(a[i], a[(i + 1) % a.len()], b[j], b[(j + 1) % b.len()]))
    })
}

/// The center, the signed sweep and the radius of the arc from `a` to `b` with `bulge`; `None`
/// for a straight edge or points that coincide.
pub(crate) fn arc_geometry(a: [f64; 2], b: [f64; 2], bulge: f64) -> Option<([f64; 2], f64, f64)> {
    arc_of(a, b, bulge)
}

fn arc_of(a: [f64; 2], b: [f64; 2], bulge: f64) -> Option<([f64; 2], f64, f64)> {
    let d = sub(b, a);
    let chord = d[0].hypot(d[1]);
    if bulge.abs() < MIN_BULGE || !bulge.is_finite() || chord <= 0.0 || !chord.is_finite() {
        return None;
    }
    let k = 0.25 * (1.0 / bulge - bulge);
    let center = [
        (a[0] + b[0]) / 2.0 - d[1] * k,
        (a[1] + b[1]) / 2.0 + d[0] * k,
    ];
    let radius = 0.25 * chord * (1.0 / bulge.abs() + bulge.abs());
    Some((center, 4.0 * bulge.atan(), radius))
}

/// The points of the arc from `a` to `b` with `bulge`, from `a` up to but not including `b`, so
/// that no point of the arc is farther than `chord` from the polyline; `[a]` for a straight edge.
#[must_use]
pub fn flatten_edge(a: [f64; 2], b: [f64; 2], bulge: f64, chord: f64) -> Vec<[f64; 2]> {
    let Some((center, sweep, radius)) = arc_of(a, b, bulge) else {
        return vec![a];
    };
    let step = if chord >= radius {
        std::f64::consts::FRAC_PI_2
    } else {
        2.0 * (1.0 - chord / radius).acos()
    };
    let n = (sweep.abs() / step.max(1.0e-3)).ceil().clamp(2.0, 512.0);
    let first = (a[1] - center[1]).atan2(a[0] - center[0]);
    let mut out = Vec::new();
    let mut k = 0.0;
    while k < n {
        let t = first + sweep * k / n;
        out.push(if k == 0.0 {
            a
        } else {
            [center[0] + radius * t.cos(), center[1] + radius * t.sin()]
        });
        k += 1.0;
    }
    out
}

/// The area between the arc and its chord, in square meters.
fn segment_area(a: [f64; 2], b: [f64; 2], bulge: f64) -> f64 {
    arc_of(a, b, bulge).map_or(0.0, |(_, sweep, r)| {
        0.5 * r * r * (sweep.abs() - sweep.abs().sin())
    })
}

impl Boundary {
    /// A boundary of straight edges.
    #[must_use]
    pub fn polygons(loops: Vec<Vec<[f64; 2]>>) -> Self {
        Self {
            loops,
            bulges: Vec::new(),
        }
    }

    /// The bulge of the edge that starts at vertex `at` of loop `l`.
    #[must_use]
    pub fn bulge(&self, l: usize, at: usize) -> f64 {
        self.bulges
            .get(l)
            .and_then(|b| b.get(at))
            .copied()
            .unwrap_or(0.0)
    }

    /// Whether some edge is an arc.
    #[must_use]
    pub fn has_arcs(&self) -> bool {
        self.bulges.iter().flatten().any(|b| b.abs() >= MIN_BULGE)
    }

    /// The loops with every arc replaced by the chords of [`flatten_edge`] within `chord`.
    #[must_use]
    pub fn flattened(&self, chord: f64) -> Vec<Vec<[f64; 2]>> {
        self.loops
            .iter()
            .enumerate()
            .map(|(l, points)| {
                let n = points.len();
                (0..n)
                    .flat_map(|k| {
                        flatten_edge(points[k], points[(k + 1) % n], self.bulge(l, k), chord)
                    })
                    .collect()
            })
            .collect()
    }

    /// The boundary of the flattened loops, which has straight edges only.
    #[must_use]
    pub fn faceted(&self, chord: f64) -> Self {
        Self::polygons(self.flattened(chord))
    }

    /// The area: the outer loop minus the holes, in square meters.
    #[must_use]
    pub fn area(&self) -> f64 {
        self.loops
            .iter()
            .enumerate()
            .map(|(l, points)| {
                let n = points.len();
                let a = (signed_area(points)
                    + (0..n)
                        .map(|k| {
                            let b = self.bulge(l, k);
                            // An arc that bends the way the loop runs adds its segment.
                            segment_area(points[k], points[(k + 1) % n], b) * b.signum()
                        })
                        .sum::<f64>())
                .abs();
                if l == 0 { a } else { -a }
            })
            .sum()
    }

    /// Whether `p` is inside the outer loop and outside every hole.
    #[must_use]
    pub fn contains(&self, p: [f64; 2]) -> bool {
        let flat;
        let loops = if self.has_arcs() {
            flat = self.flattened(FLAT_CHORD / 4.0);
            &flat
        } else {
            &self.loops
        };
        loops
            .iter()
            .enumerate()
            .all(|(k, l)| inside(l, p) == (k == 0))
    }

    /// The perimeter of all loops, in meters.
    #[must_use]
    pub fn perimeter(&self) -> f64 {
        self.loops
            .iter()
            .enumerate()
            .flat_map(|(l, points)| (0..points.len()).map(move |k| (l, k, points)))
            .map(|(l, k, points)| {
                let (a, b) = (points[k], points[(k + 1) % points.len()]);
                let d = sub(b, a);
                let chord = d[0].hypot(d[1]);
                arc_of(a, b, self.bulge(l, k)).map_or(chord, |(_, sweep, r)| r * sweep.abs())
            })
            .sum()
    }

    /// Checks the boundary: each loop simple with edges of at least [`MIN_EDGE`], an outer loop
    /// with an area in [`AREA_RANGE`], holes inside it and apart from each other. Arcs are
    /// checked as the chords of [`flatten_edge`] within [`FLAT_CHORD`].
    ///
    /// # Errors
    /// The [`Refusal`] that names what is wrong.
    pub fn check(&self) -> std::result::Result<(), Refusal> {
        let count: usize = self.loops.iter().map(Vec::len).sum();
        let (low, high) = COORDINATE_RANGE;
        let arcs = self.has_arcs();
        if self.loops.is_empty()
            || self.loops.len() > MAX_LOOPS
            || count > MAX_VERTICES
            || self.loops.iter().enumerate().any(|(l, p)| {
                p.len()
                    < if arcs && self.bulges.get(l).is_some() {
                        2
                    } else {
                        3
                    }
            })
            || self
                .loops
                .iter()
                .flatten()
                .any(|p| p.iter().any(|c| !c.is_finite() || *c < low || *c > high))
            || self
                .bulges
                .iter()
                .flatten()
                .any(|b| !b.is_finite() || b.abs() > MAX_BULGE)
        {
            return Err(Refusal::Malformed);
        }
        // The edges are measured on the authored vertices, the crossings on the flattened loops.
        for points in &self.loops {
            let n = points.len();
            for k in 0..n {
                let d = sub(points[(k + 1) % n], points[k]);
                if d[0].hypot(d[1]) < MIN_EDGE {
                    return Err(Refusal::EdgeTooShort);
                }
            }
        }
        let flat = self.flattened(FLAT_CHORD);
        for points in &flat {
            let n = points.len();
            if n < 3 {
                return Err(Refusal::Malformed);
            }
            for k in 0..n {
                let (a, b) = (points[k], points[(k + 1) % n]);
                let d = sub(b, a);
                // The next edge may only share its start; folding back on itself is a crossing.
                let e = sub(points[(k + 2) % n], b);
                if cross(d, e).abs() <= 1.0e-12 * d[0].hypot(d[1]) * e[0].hypot(e[1])
                    && dot(d, e) < 0.0
                {
                    return Err(Refusal::SelfIntersecting);
                }
                for j in k + 2..n {
                    if k == 0 && j == n - 1 {
                        continue;
                    }
                    if segments_touch(a, b, points[j], points[(j + 1) % n]) {
                        return Err(Refusal::SelfIntersecting);
                    }
                }
            }
        }
        let outer_area = self.loop_area(0).abs();
        if outer_area < AREA_RANGE.0 || outer_area > AREA_RANGE.1 {
            return Err(Refusal::Area);
        }
        for (k, hole) in flat.iter().enumerate().skip(1) {
            if loops_touch(&flat[0], hole) || !inside(&flat[0], hole[0]) {
                return Err(Refusal::HoleOutside);
            }
            for other in flat.iter().skip(1).take(k - 1) {
                if loops_touch(other, hole) || inside(other, hole[0]) || inside(hole, other[0]) {
                    return Err(Refusal::HolesOverlap);
                }
            }
        }
        Ok(())
    }

    /// The area of loop `l` alone, arcs included.
    fn loop_area(&self, l: usize) -> f64 {
        Self {
            loops: self.loops.get(l).cloned().into_iter().collect(),
            bulges: self.bulges.get(l).cloned().into_iter().collect(),
        }
        .area()
    }

    /// The solid: a prism of the boundary from `top - thickness` up to `top`, where `top` is the
    /// elevation of the level plus the top offset.
    ///
    /// # Errors
    /// The solid kernel's errors for a boundary it cannot make a solid of.
    pub fn solid(&self, top: f64, thickness: f64) -> Result<Polyhedron> {
        let tolerance = Tolerance::DEFAULT;
        prism(
            &self.region()?,
            top - thickness,
            thickness,
            CHORD,
            &tolerance,
            &Limits::DEFAULT,
        )
    }

    /// The boundary as convex pieces (a triangulation, arcs faceted), each with `plane`.
    #[must_use]
    pub fn pieces(&self, plane: crate::terrain::Plane) -> Vec<crate::terrain::Piece> {
        let Ok(region) = self.region() else {
            return Vec::new();
        };
        let Ok(mesh) = arc_geom::triangulate(&region, 0.01, std::f64::consts::PI, 4 * MAX_VERTICES)
        else {
            return Vec::new();
        };
        mesh.triangles()
            .iter()
            .filter_map(|t| {
                let mut points: Vec<[f64; 2]> = t
                    .iter()
                    .map(|i| {
                        mesh.vertices()
                            .get(usize::try_from(*i).ok()?)
                            .map(|p| [p.x, p.y])
                    })
                    .collect::<Option<_>>()?;
                if crate::terrain::double_area(&points) < 0.0 {
                    points.reverse();
                }
                Some(crate::terrain::Piece {
                    points,
                    plane,
                    upper: None,
                })
            })
            .collect()
    }

    /// The boundary as a region of the solid kernel, with exact arcs.
    ///
    /// # Errors
    /// The solid kernel's errors for loops it cannot make a region of.
    pub fn region(&self) -> Result<Region> {
        let tolerance = Tolerance::DEFAULT;
        let mut parts = Vec::with_capacity(self.loops.len());
        for (l, points) in self.loops.iter().enumerate() {
            let pts: Vec<Point2> = points.iter().map(|p| Point2::new(p[0], p[1])).collect();
            let mut curves = Vec::with_capacity(pts.len());
            for k in 0..pts.len() {
                let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
                let bulge = self.bulge(l, k);
                if bulge.abs() < MIN_BULGE {
                    curves.push(Curve2::Segment(Segment2::new(a, b)?));
                } else {
                    let line = Polyline2::new(
                        vec![PolylineVertex::new(a, bulge), PolylineVertex::new(b, 0.0)],
                        false,
                    )?;
                    for segment in line.segments() {
                        curves.push(match segment {
                            PolylineSegment::Line(s) => Curve2::Segment(s),
                            PolylineSegment::Arc(a) => Curve2::Arc(a),
                        });
                    }
                }
            }
            parts.push(Loop2::new(curves, &tolerance)?);
        }
        let mut loops = parts.into_iter();
        let outer = loops
            .next()
            .ok_or_else(|| arc_base::Error::new(arc_geom::error::codes::REGION_EMPTY))?;
        Region::new(vec![RegionPart::new(outer, loops.collect())])
    }

    /// The boundary of a region of the solid kernel: its first part, one loop per boundary with
    /// the arcs kept as bulges; `None` when it has several parts or a curve that is neither a
    /// segment nor a circular arc.
    #[must_use]
    pub fn of_region(region: &Region) -> Option<Self> {
        let [part] = region.parts() else {
            return None;
        };
        let mut loops = Vec::new();
        let mut bulges = Vec::new();
        for l in std::iter::once(part.outer()).chain(part.holes()) {
            let (points, arcs) = loop_points(l)?;
            loops.push(points);
            bulges.push(arcs);
        }
        Some(Self { loops, bulges })
    }

    /// The boundary with the shaft `openings` taken out, and how many openings that reach the
    /// boundary could not be cut. An opening inside the outer loop and clear of the holes is a new
    /// hole; one that crosses an edge is subtracted, which leaves a notch, as long as the result
    /// is one valid boundary. An opening that does not reach the boundary changes nothing.
    #[must_use]
    pub fn with_openings(&self, openings: &[Vec<[f64; 2]>]) -> (Self, usize) {
        let mut out = self.clone();
        let mut uncut = 0;
        // Openings that overlap are cut as one, so two stairs side by side leave one hole.
        let merged = merge_loops(openings);
        for opening in merged.iter().take(MAX_LOOPS) {
            let mut hole = out.clone();
            hole.loops.push(opening.clone());
            hole.bulges.resize(hole.loops.len(), Vec::new());
            if hole.check().is_ok() {
                out = hole;
                continue;
            }
            let flat = out.flattened(FLAT_CHORD);
            let outer = &flat[0];
            let reaches = opening.iter().any(|p| inside(outer, *p))
                || outer.iter().any(|p| inside(opening, *p))
                || loops_touch(outer, opening);
            if !reaches {
                continue;
            }
            match out.subtract(opening) {
                Some(cut) => out = cut,
                None => uncut += 1,
            }
        }
        (out, uncut)
    }

    /// The boundary less the polygon `cut`, when the result is one valid boundary.
    fn subtract(&self, cut: &[[f64; 2]]) -> Option<Self> {
        let tool = Self::polygons(vec![cut.to_vec()]).region().ok()?;
        let result = arc_geom::boolean(
            &self.region().ok()?,
            &tool,
            arc_geom::BooleanOp::Difference,
            &Tolerance::DEFAULT,
            &Limits::DEFAULT,
        )
        .ok()??;
        let boundary = Self::of_region(&result)?;
        boundary.check().is_ok().then_some(boundary)
    }
}

/// The start points and bulges of the curves of a loop; `None` for a curve that is neither a
/// segment nor a circular arc.
fn loop_points(l: &Loop2) -> Option<(Vec<[f64; 2]>, Vec<f64>)> {
    let mut points = Vec::new();
    let mut bulges = Vec::new();
    for curve in l.curves() {
        let p = curve.start_point()?;
        points.push([p.x, p.y]);
        bulges.push(match curve {
            Curve2::Segment(_) => 0.0,
            Curve2::Arc(arc) => (arc.sweep() / 4.0).tan(),
            _ => return None,
        });
    }
    Some((points, bulges))
}

/// The union of `loops` as the outer loops of its parts (an enclosed gap is filled); `loops`
/// as they are when the solid kernel cannot unite them.
fn merge_loops(loops: &[Vec<[f64; 2]>]) -> Vec<Vec<[f64; 2]>> {
    if loops.len() < 2 {
        return loops.to_vec();
    }
    let region = |l: &Vec<[f64; 2]>| Boundary::polygons(vec![l.clone()]).region().ok();
    let mut acc: Option<Region> = None;
    for l in loops.iter().take(MAX_LOOPS) {
        let Some(r) = region(l) else {
            return loops.to_vec();
        };
        acc = Some(match acc {
            None => r,
            Some(a) => match arc_geom::boolean(
                &a,
                &r,
                arc_geom::BooleanOp::Union,
                &Tolerance::DEFAULT,
                &Limits::DEFAULT,
            ) {
                Ok(Some(u)) => u,
                _ => return loops.to_vec(),
            },
        });
    }
    let Some(acc) = acc else {
        return loops.to_vec();
    };
    acc.parts()
        .iter()
        .map(|p| {
            p.outer()
                .curves()
                .iter()
                .filter_map(|c| c.start_point().map(|q| [q.x, q.y]))
                .collect()
        })
        .collect()
}

/// What a floor reads from a snapshot: its data, the elevation of the top and the boundary.
#[derive(Debug, Clone)]
pub struct Derived {
    /// The authored data.
    pub floor: Floor,
    /// The elevation of the slab's top: the level plus the top offset, in meters.
    pub top: f64,
    /// The boundary, derived from the walls, with the shaft openings of the stairs that arrive at
    /// the floor's level taken out.
    pub boundary: Boundary,
    /// The shaft openings that reach the floor but could not be cut.
    pub uncut: usize,
}

impl Derived {
    /// Reads the floor `object` of `snapshot`; `None` when the record or its edges are unreadable.
    #[must_use]
    pub fn of(snapshot: &Snapshot, object: &arc_doc::Object) -> Option<Self> {
        let floor = Floor::from_values(&object.values)?;
        let level = object
            .header
            .owner
            .and_then(|id| crate::spatial::get(snapshot, id))
            .map_or(0.0, |s| crate::spatial::storey_elevation(snapshot, &s));
        let mut openings = crate::stair::openings_of_floor(snapshot, object);
        openings.extend(crate::ramp::openings_of_floor(snapshot, object));
        let top = level + floor.top_offset;
        // Shafts and vertical openings that pass through the slab's body cut it like a stair.
        openings.extend(crate::opening::cuts(
            snapshot,
            object,
            (top - floor.thickness, top),
        ));
        let (boundary, uncut) = floor.boundary_in(snapshot)?.with_openings(&openings);
        Some(Self {
            top: level + floor.top_offset,
            floor,
            boundary,
            uncut,
        })
    }

    /// The area in square meters.
    #[must_use]
    pub fn area(&self) -> f64 {
        self.boundary.area()
    }

    /// The volume of the slab, in cubic meters: the area times the thickness.
    #[must_use]
    pub fn volume(&self) -> f64 {
        self.area() * self.floor.thickness
    }

    /// The solid of the slab.
    ///
    /// # Errors
    /// The solid kernel's errors.
    pub fn solid(&self) -> Result<Polyhedron> {
        self.boundary.solid(self.top, self.floor.thickness)
    }
}

/// The walls of `storey` that have a readable baseline, in identifier order.
#[must_use]
pub fn storey_walls(snapshot: &Snapshot, storey: ObjectId) -> Vec<(ObjectId, Wall)> {
    crate::spatial::walls(snapshot, storey)
        .iter()
        .filter_map(|o| Wall::from_values(&o.values).map(|w| (o.id, w)))
        .filter(|(_, w)| w.length() >= MIN_WALL_LENGTH)
        .collect()
}

/// The walls of `storey` that bound rooms: [`storey_walls`] without those whose type is not
/// room-bounding.
#[must_use]
pub fn room_walls(snapshot: &Snapshot, storey: ObjectId) -> Vec<(ObjectId, Wall)> {
    use crate::element_type::{TypeKind, WallType, type_of};
    let bounding = |id: ObjectId| {
        crate::spatial::get(snapshot, id)
            .and_then(|o| type_of(snapshot, TypeKind::Wall, &o.values))
            .and_then(|t| WallType::from_values(&t.values))
            .is_none_or(|t| t.room_bounding)
    };
    storey_walls(snapshot, storey)
        .into_iter()
        .filter(|(id, _)| bounding(*id))
        .collect()
}

/// Why walls cannot make a floor boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallsRefusal {
    /// Fewer than three walls, a wall named twice, or no closed loop around the point.
    NoLoop,
    /// Two walls that follow each other are parallel: they do not meet.
    Parallel,
}

/// Where two collinear walls that follow each other meet: the middle of their nearest ends, on
/// the line of the second; `None` when the lines are not the same line. The walls may run either
/// way (a wall drawn back over part of another).
fn junction(w0: &Wall, w1: &Wall, a: &Line, b: &Line) -> Option<[f64; 2]> {
    if cross(a.dir, b.dir).abs() > 1.0e-9 || cross(sub(b.at, a.at), a.dir).abs() > 1.0e-3 {
        return None;
    }
    let ends = |w: &Wall| [w.start, w.end];
    let mut best: Option<(f64, [f64; 2])> = None;
    for p in ends(w0) {
        for q in ends(w1) {
            let d = sub(q, p);
            let gap = d[0].hypot(d[1]);
            if best.is_none_or(|(g, _)| gap < g) {
                best = Some((gap, [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0]));
            }
        }
    }
    best.map(|(_, m)| b.project(m))
}

/// The vertices of a floor over the closed chain `walls` (in order, each meeting the next and the
/// last meeting the first; a wall that bounds the region on two separate stretches, such as a
/// through wall that a notch ends on, appears twice), edges on the chosen `face`. For [`Face::Interior`] and
/// [`Face::Exterior`] the face is the one that looks into, or away from, the area the chain
/// encloses, whichever side of its baseline that is.
///
/// # Errors
/// [`WallsRefusal`].
pub fn vertices_of_walls(
    walls: &[(ObjectId, Wall)],
    face: Face,
    offset: f64,
) -> std::result::Result<Vec<Vertex>, WallsRefusal> {
    let n = walls.len();
    if !(3..=MAX_VERTICES).contains(&n) {
        return Err(WallsRefusal::NoLoop);
    }
    let lines = |faces: &[Face]| -> std::result::Result<Vec<[f64; 2]>, WallsRefusal> {
        (0..n)
            .map(|k| {
                let before = (k + n - 1) % n;
                let a = Line::of_face(&walls[before].1, faces[before], offset);
                let b = Line::of_face(&walls[k].1, faces[k], offset);
                let (a, b) = a.zip(b).ok_or(WallsRefusal::Parallel)?;
                // Collinear neighbors (a wall split in two) meet where their ends meet.
                a.meet(&b)
                    .or_else(|| junction(&walls[before].1, &walls[k].1, &a, &b))
                    .ok_or(WallsRefusal::Parallel)
            })
            .collect()
    };
    // The center lines give the polygon whose direction says which side the area is on.
    let polygon = lines(&vec![Face::Center; n])?;
    let ccw = signed_area(&polygon) > 0.0;
    let faces: Vec<Face> = (0..n)
        .map(|k| {
            if face == Face::Center {
                return Face::Center;
            }
            // The wall runs from vertex k to vertex k+1 of the polygon; the area is on the left
            // of that run when the polygon runs counterclockwise.
            let run = sub(polygon[(k + 1) % n], polygon[k]);
            let base = sub(walls[k].1.end, walls[k].1.start);
            let same = dot(run, base) >= 0.0;
            let area_on_left_of_baseline = ccw == same;
            // The left of the baseline is the exterior face.
            let toward_area = if area_on_left_of_baseline {
                Face::Exterior
            } else {
                Face::Interior
            };
            // The face the caller asked for, looking toward the area or away from it, on the
            // wall's surface (`Interior`, `Exterior`) or on its core.
            let core = matches!(face, Face::CoreInterior | Face::CoreExterior);
            let toward = matches!(face, Face::Interior | Face::CoreInterior);
            let left = (toward_area == Face::Exterior) == toward;
            match (core, left) {
                (false, true) => Face::Exterior,
                (false, false) => Face::Interior,
                (true, true) => Face::CoreExterior,
                (true, false) => Face::CoreInterior,
            }
        })
        .collect();
    let points = lines(&faces)?;
    Ok((0..n)
        .map(|k| Vertex {
            loop_index: 0,
            // Vertex k is where wall k-1 meets wall k; the edge from it follows wall k.
            point: points[k],
            edge: Some(Association {
                wall: walls[k].0,
                face: faces[k],
                offset,
            }),
            bulge: 0.0,
        })
        .collect())
}

/// What a click picks among the walls of a storey.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick {
    /// The click is on this wall.
    Wall(ObjectId),
    /// The click is inside a closed loop of joined walls: the walls of the smallest such loop, in
    /// order around it.
    Loop(Vec<ObjectId>),
}

/// The wall whose body (with a small margin) holds `p`, the nearest one.
#[must_use]
pub fn wall_at(walls: &[(ObjectId, Wall)], p: [f64; 2]) -> Option<ObjectId> {
    walls
        .iter()
        .filter_map(|(id, w)| {
            let base = Line::through(w.start, w.end)?;
            let rel = sub(p, w.start);
            let along = dot(rel, base.dir);
            let across = cross(base.dir, rel);
            let (low, high) = w.extent();
            let gap = (low - across).max(across - high).max(0.0);
            let beyond = (-along).max(along - w.length()).max(0.0);
            let miss = gap.hypot(beyond);
            (miss <= PICK_MARGIN).then_some((miss, *id))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// The pairs of walls that lie on top of each other: collinear within the region tolerance and
/// sharing a stretch longer than it. A model-check finding; region finding counts each shared
/// stretch once, so such walls bound regions as one wall.
#[must_use]
pub fn overlapping_walls(walls: &[(ObjectId, Wall)]) -> Vec<(ObjectId, ObjectId)> {
    let tol = 1.0e-3;
    let mut out = Vec::new();
    for (i, (a_id, a)) in walls.iter().enumerate().take(MAX_PICKED) {
        let Some(base) = Line::through(a.start, a.end) else {
            continue;
        };
        let length = a.length();
        for (b_id, b) in walls.iter().skip(i + 1).take(MAX_PICKED) {
            let across = |q: [f64; 2]| cross(base.dir, sub(q, a.start)).abs();
            if across(b.start) > tol || across(b.end) > tol {
                continue;
            }
            let (s, e) = (
                dot(sub(b.start, a.start), base.dir),
                dot(sub(b.end, a.start), base.dir),
            );
            if s.max(e).min(length) - s.min(e).max(0.0) > tol {
                out.push((*a_id, *b_id));
            }
        }
    }
    out
}

/// The directed piece of a wall between two nodes.
#[derive(Debug, Clone, Copy)]
struct Piece {
    from: usize,
    to: usize,
    wall: usize,
}

/// Where the center lines of two walls cross strictly inside both; `None` for parallel walls and
/// for walls that only meet at an end (the ends are nodes already).
fn crossing(a: &Wall, b: &Wall, tol: f64) -> Option<[f64; 2]> {
    let (r, s) = (sub(a.end, a.start), sub(b.end, b.start));
    let denom = cross(r, s);
    let (la, lb) = (a.length(), b.length());
    if denom.abs() <= 1.0e-12 * la * lb {
        return None;
    }
    let q = sub(b.start, a.start);
    let t = cross(q, s) / denom;
    let u = cross(q, r) / denom;
    let (ta, tb) = (tol / la, tol / lb);
    ((ta..=1.0 - ta).contains(&t) && (tb..=1.0 - tb).contains(&u))
        .then(|| [a.start[0] + r[0] * t, a.start[1] + r[1] * t])
}

/// One closed region of a storey's walls: a face of the plan the walls make.
#[derive(Debug, Clone, PartialEq)]
pub struct WallRegion {
    /// The area inside the center lines of the walls, in square meters.
    pub area: f64,
    /// The corners of the loop on the center lines, counterclockwise.
    pub polygon: Vec<[f64; 2]>,
    /// The walls around the region in order, one entry per stretch (a through wall that another
    /// wall's ends land on appears twice); `None` when the loop has fewer than three walls.
    pub walls: Option<Vec<ObjectId>>,
    /// The closed loops of walls inside the region that touch none of its walls (a courtyard), each
    /// as its walls in order: the region's holes, bounded by the outer faces of those walls. Only
    /// the outermost loops are listed; a loop inside a hole is a region of its own.
    pub holes: Vec<Vec<ObjectId>>,
    /// The area inside the center lines of the walls with the holes' center-line areas taken out,
    /// in square meters.
    pub center_area: f64,
}

impl WallRegion {
    /// Whether `p` is inside the loop on the center lines.
    #[must_use]
    pub fn contains(&self, p: [f64; 2]) -> bool {
        inside(&self.polygon, p)
    }
}

/// The smallest of `regions` that holds `p`.
#[must_use]
pub fn smallest_around(regions: &[WallRegion], p: [f64; 2]) -> Option<usize> {
    regions
        .iter()
        .enumerate()
        .filter(|(_, r)| r.contains(p))
        .min_by(|a, b| a.1.area.total_cmp(&b.1.area))
        .map(|(k, _)| k)
}

/// The walls of the smallest closed loop of walls around `p`, in order; walls are joined where
/// their ends meet or an end lies on another wall (a T). `None` when no loop encloses the point,
/// or the search exceeds its bounds.
#[must_use]
pub fn loop_around(walls: &[(ObjectId, Wall)], p: [f64; 2]) -> Option<Vec<ObjectId>> {
    region_around(walls, p)?.walls
}

/// The smallest region around `p`: the walls of its loop and the loops of its holes. A point in
/// a courtyard is in the courtyard's own region, which is smaller.
#[must_use]
pub fn region_around(walls: &[(ObjectId, Wall)], p: [f64; 2]) -> Option<WallRegion> {
    let all = regions(walls)?;
    all.get(smallest_around(&all, p)?).cloned()
}

/// Every closed region of the plan the walls make, found once: walls are joined where their ends
/// meet or an end lies on another wall (a T). `None` when the search exceeds its bounds.
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "one search: nodes, pieces, face walk, spur removal"
)]
pub fn regions(walls: &[(ObjectId, Wall)]) -> Option<Vec<WallRegion>> {
    if walls.is_empty() || walls.len() > MAX_PICKED {
        return None;
    }
    let tol = 1.0e-3;
    let mut nodes: Vec<[f64; 2]> = Vec::new();
    let node_of = |q: [f64; 2], nodes: &mut Vec<[f64; 2]>| -> usize {
        match nodes
            .iter()
            .position(|n| (n[0] - q[0]).hypot(n[1] - q[1]) < tol)
        {
            Some(k) => k,
            None => {
                nodes.push(q);
                nodes.len() - 1
            }
        }
    };
    // Each wall's nodes along it: its ends, the ends of other walls that lie on it, and the
    // points where other walls cross it.
    let ends: Vec<[f64; 2]> = walls.iter().flat_map(|(_, w)| [w.start, w.end]).collect();
    let mut pieces: Vec<Piece> = Vec::new();
    // Walls on top of each other make the same pieces; each stretch is kept once and belongs to
    // the longest wall over it, so overlapping walls bound regions as their union instead of
    // leaving a face of no width or a chain that turns back on one line.
    let mut taken: std::collections::BTreeMap<(usize, usize), usize> =
        std::collections::BTreeMap::new();
    for (k, (_, w)) in walls.iter().enumerate() {
        let base = Line::through(w.start, w.end)?;
        let length = w.length();
        let mut along: Vec<(f64, [f64; 2])> = vec![(0.0, w.start), (length, w.end)];
        for q in &ends {
            let rel = sub(*q, w.start);
            let t = dot(rel, base.dir);
            if t > tol && t < length - tol && cross(base.dir, rel).abs() < tol {
                along.push((t, *q));
            }
        }
        // Walls that cross this one split it too, whether or not they end beyond it.
        for (j, (_, other)) in walls.iter().enumerate() {
            if j == k {
                continue;
            }
            if let Some(at) = crossing(w, other, tol) {
                along.push((dot(sub(at, w.start), base.dir), at));
            }
        }
        along.sort_by(|a, b| a.0.total_cmp(&b.0));
        let ids: Vec<usize> = along.iter().map(|(_, q)| node_of(*q, &mut nodes)).collect();
        for pair in ids.windows(2) {
            if pair[0] == pair[1] {
                continue;
            }
            let key = (pair[0].min(pair[1]), pair[0].max(pair[1]));
            if let Some(&at) = taken.get(&key) {
                if length > walls[pieces[at].wall].1.length() {
                    pieces[at].wall = k;
                    pieces[at + 1].wall = k;
                }
            } else {
                taken.insert(key, pieces.len());
                pieces.push(Piece {
                    from: pair[0],
                    to: pair[1],
                    wall: k,
                });
                pieces.push(Piece {
                    from: pair[1],
                    to: pair[0],
                    wall: k,
                });
            }
        }
    }
    // The next piece of the face on the left of a piece: at its end, the piece that turns most
    // clockwise from the way back.
    let angle = |piece: &Piece| {
        let d = sub(nodes[piece.to], nodes[piece.from]);
        d[1].atan2(d[0])
    };
    let next_of = |piece: &Piece| -> Option<usize> {
        let back = angle(piece) + std::f64::consts::PI;
        let mut best: Option<(f64, usize)> = None;
        for (k, q) in pieces.iter().enumerate() {
            if q.from != piece.to || (q.to == piece.from && q.wall == piece.wall) {
                continue;
            }
            // Counterclockwise angle from the way back to the candidate, in (0, 2 pi].
            let mut turn = (angle(q) - back).rem_euclid(std::f64::consts::TAU);
            if turn <= 1.0e-12 {
                turn = std::f64::consts::TAU;
            }
            if best.is_none_or(|(t, _)| turn > t) {
                best = Some((turn, k));
            }
        }
        // A dead end turns back along the same piece.
        best.map(|(_, k)| k).or_else(|| {
            pieces
                .iter()
                .position(|q| q.from == piece.to && q.to == piece.from && q.wall == piece.wall)
        })
    };
    let mut seen = vec![false; pieces.len()];
    let mut found: Vec<WallRegion> = Vec::new();
    for start in 0..pieces.len() {
        if seen[start] {
            continue;
        }
        let mut cycle = Vec::new();
        let mut at = start;
        while !seen[at] && cycle.len() <= pieces.len() {
            seen[at] = true;
            cycle.push(at);
            at = next_of(&pieces[at])?;
        }
        if at != start {
            continue;
        }
        // Dead-end spurs walk out and back: drop each piece followed by its reverse.
        let mut stack: Vec<usize> = Vec::new();
        for &k in &cycle {
            match stack.last() {
                Some(&top)
                    if pieces[top].from == pieces[k].to
                        && pieces[top].to == pieces[k].from
                        && pieces[top].wall == pieces[k].wall =>
                {
                    stack.pop();
                }
                _ => stack.push(k),
            }
        }
        if stack.len() < 3 {
            continue;
        }
        let polygon: Vec<[f64; 2]> = stack.iter().map(|&k| nodes[pieces[k].from]).collect();
        let area = signed_area(&polygon);
        if area <= 0.0 {
            continue;
        }
        let mut order: Vec<usize> = Vec::new();
        for &k in &stack {
            let wall = pieces[k].wall;
            if order.last() != Some(&wall) {
                order.push(wall);
            }
        }
        // The loop closes: the last wall may be the first one continued.
        if order.len() > 1 && order.first() == order.last() {
            order.pop();
        }
        found.push(WallRegion {
            area,
            polygon,
            walls: (order.len() >= 3).then(|| order.into_iter().map(|k| walls[k].0).collect()),
            holes: Vec::new(),
            center_area: area,
        });
    }
    nest(&mut found);
    Some(found)
}

/// Gives each region the regions directly inside it as holes. A region is inside another when all
/// its corners are inside the other's center-line loop; faces of one plan never overlap, so only a
/// loop that touches no wall of the other can be inside it. A region with a hole whose walls do
/// make no chain (fewer than three walls) loses its own chain, so no room or floor is made of it.
fn nest(found: &mut [WallRegion]) {
    let parents: Vec<Option<usize>> = (0..found.len())
        .map(|s| {
            (0..found.len())
                .filter(|&r| {
                    r != s
                        && found[s].area < found[r].area
                        && found[s]
                            .polygon
                            .iter()
                            .all(|q| inside(&found[r].polygon, *q))
                })
                .min_by(|&a, &b| found[a].area.total_cmp(&found[b].area))
        })
        .collect();
    for (s, parent) in parents.into_iter().enumerate() {
        let Some(r) = parent else { continue };
        match found[s].walls.clone() {
            Some(chain) => {
                let area = found[s].area;
                found[r].holes.push(chain);
                found[r].center_area -= area;
            }
            None => found[r].walls = None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wall::{JoinStyle, LocationLine};
    use arc_base::id::Identifier as _;

    fn wall(start: [f64; 2], end: [f64; 2]) -> Wall {
        Wall {
            start,
            end,
            thickness: 0.2,
            core: [0.0, 0.0],
            height: 3.0,
            base_offset: 0.0,
            location: LocationLine::Center,
            start_join: JoinStyle::Auto,
            end_join: JoinStyle::Auto,
        }
    }

    fn id(n: u64) -> ObjectId {
        ObjectId::from_u128(u128::from(n))
    }

    /// The 10 x 6 house, drawn clockwise: bottom-left, top-left, top-right, bottom-right.
    fn house(clockwise: bool) -> Vec<(ObjectId, Wall)> {
        let mut corners = [[0.0, 0.0], [0.0, 6.0], [10.0, 6.0], [10.0, 0.0]];
        if !clockwise {
            corners.reverse();
        }
        (0..4)
            .map(|k| (id(k as u64 + 1), wall(corners[k], corners[(k + 1) % 4])))
            .collect()
    }

    #[test]
    fn the_interior_face_loop_of_the_house_has_the_clear_area_either_way_round() {
        for clockwise in [true, false] {
            let walls = house(clockwise);
            let vertices = vertices_of_walls(&walls, Face::Interior, 0.0).unwrap();
            let floor = Floor {
                thickness: 0.2,
                top_offset: 0.0,
                vertices,
            };
            let lookup = |w: ObjectId| walls.iter().find(|(i, _)| *i == w).map(|(_, w)| *w);
            let boundary = floor.boundary(&lookup).unwrap();
            assert!((boundary.area() - 9.8 * 5.8).abs() < 1e-9, "{clockwise}");
            boundary.check().unwrap();
            let exterior = vertices_of_walls(&walls, Face::Exterior, 0.0).unwrap();
            let f = Floor {
                vertices: exterior,
                ..floor
            };
            assert!((f.boundary(&lookup).unwrap().area() - 10.2 * 6.2).abs() < 1e-9);
        }
    }

    #[test]
    fn a_click_inside_picks_the_loop_and_a_click_on_a_wall_picks_the_wall() {
        let mut walls = house(true);
        // A stub wall inside the room that touches one wall only does not break the loop.
        walls.push((id(9), wall([5.0, 0.0], [5.0, 2.0])));
        let picked = loop_around(&walls, [5.0, 3.0]).unwrap();
        assert_eq!(picked.len(), 4);
        assert!(loop_around(&walls, [20.0, 3.0]).is_none());
        assert_eq!(wall_at(&walls, [5.0, 1.0]), Some(id(9)));
        assert_eq!(wall_at(&walls, [0.0, 3.0]), Some(id(1)));
        assert_eq!(wall_at(&walls, [3.0, 3.0]), None);
    }

    #[test]
    fn walls_on_top_of_each_other_count_as_one_and_are_reported() {
        let mut walls = house(true);
        walls.push((id(8), wall([6.0, 6.0], [6.0, 2.0])));
        walls.push((id(9), wall([6.0, 2.0], [6.0, 6.0])));
        let found = regions(&walls).unwrap();
        assert_eq!(found.len(), 1);
        assert!((found[0].area - 60.0).abs() < 1e-9);
        assert_eq!(overlapping_walls(&walls), vec![(id(8), id(9))]);
        // An outer wall drawn twice still closes the house.
        let mut walls = house(true);
        walls.push((id(9), wall([0.0, 6.0], [0.0, 0.0])));
        let found = regions(&walls).unwrap();
        assert_eq!(found.len(), 1);
        assert!((found[0].area - 60.0).abs() < 1e-9);
        assert_eq!(overlapping_walls(&walls), vec![(id(1), id(9))]);
        // A partial overlap is reported; walls that only meet end to end are not.
        let mut walls = house(true);
        walls.push((id(8), wall([5.0, 0.0], [5.0, 4.0])));
        walls.push((id(9), wall([5.0, 3.0], [5.0, 6.0])));
        assert_eq!(overlapping_walls(&walls), vec![(id(8), id(9))]);
        walls[5].1 = wall([5.0, 4.0], [5.0, 6.0]);
        assert!(overlapping_walls(&walls).is_empty());
    }

    #[test]
    fn a_wall_that_divides_the_house_gives_the_smaller_room() {
        let mut walls = house(true);
        walls.push((id(9), wall([4.0, 0.0], [4.0, 6.0])));
        let picked = loop_around(&walls, [2.0, 3.0]).unwrap();
        assert!(picked.contains(&id(9)) && picked.contains(&id(1)));
        assert_eq!(picked.len(), 4);
    }

    #[test]
    fn overlapping_openings_are_cut_as_one_hole() {
        let rect =
            |x0: f64, y0: f64, x1: f64, y1: f64| vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
        let slab = Boundary {
            loops: vec![rect(0.0, 0.0, 10.0, 6.0)],
            ..Boundary::default()
        };
        let (cut, uncut) =
            slab.with_openings(&[rect(1.0, 1.0, 3.0, 3.0), rect(2.0, 2.0, 4.0, 4.0)]);
        assert_eq!(uncut, 0);
        assert_eq!(cut.loops.len(), 2);
        assert!((cut.area() - (60.0 - 7.0)).abs() < 1e-9, "{}", cut.area());
    }

    #[test]
    fn refusals_name_what_is_wrong() {
        let square = |s: f64, at: [f64; 2]| -> Vec<[f64; 2]> {
            vec![
                at,
                [at[0] + s, at[1]],
                [at[0] + s, at[1] + s],
                [at[0], at[1] + s],
            ]
        };
        let b = |loops: Vec<Vec<[f64; 2]>>| Boundary::polygons(loops);
        assert_eq!(b(vec![square(4.0, [0.0, 0.0])]).check(), Ok(()));
        // A bow tie.
        let bow = vec![[0.0, 0.0], [4.0, 4.0], [4.0, 0.0], [0.0, 4.0]];
        assert_eq!(b(vec![bow]).check(), Err(Refusal::SelfIntersecting));
        assert!(
            b(vec![vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]]])
                .check()
                .is_err()
        );
        assert_eq!(
            b(vec![vec![[0.0, 0.0], [1.0, 0.0]]]).check(),
            Err(Refusal::Malformed)
        );
        assert_eq!(
            b(vec![vec![
                [0.0, 0.0],
                [0.0005, 0.0],
                [0.0005, 4.0],
                [0.0, 4.0]
            ]])
            .check(),
            Err(Refusal::EdgeTooShort)
        );
        let outer = square(10.0, [0.0, 0.0]);
        assert_eq!(
            b(vec![outer.clone(), square(2.0, [3.0, 3.0])]).check(),
            Ok(())
        );
        assert_eq!(
            b(vec![outer.clone(), square(2.0, [9.0, 3.0])]).check(),
            Err(Refusal::HoleOutside)
        );
        assert_eq!(
            b(vec![outer.clone(), square(2.0, [20.0, 3.0])]).check(),
            Err(Refusal::HoleOutside)
        );
        assert_eq!(
            b(vec![
                outer.clone(),
                square(3.0, [2.0, 2.0]),
                square(3.0, [4.0, 3.0])
            ])
            .check(),
            Err(Refusal::HolesOverlap)
        );
        assert_eq!(
            b(vec![
                outer.clone(),
                square(6.0, [2.0, 2.0]),
                square(2.0, [4.0, 4.0])
            ])
            .check(),
            Err(Refusal::HolesOverlap)
        );
        let holed = b(vec![outer, square(2.0, [3.0, 3.0])]);
        assert!((holed.area() - 96.0).abs() < 1e-12);
    }

    #[test]
    fn the_solid_is_the_area_times_the_thickness() {
        let b = Boundary::polygons(vec![
            vec![[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]],
            vec![[1.0, 1.0], [2.0, 1.0], [2.0, 2.0], [1.0, 2.0]],
        ]);
        let solid = b.solid(3.0, 0.2).unwrap();
        assert!((solid.volume() - 59.0 * 0.2).abs() < 1e-9);
        let bounds = solid.bounding_box();
        assert!((bounds.min().z - 2.8).abs() < 1e-9 && (bounds.max().z - 3.0).abs() < 1e-9);
    }

    #[test]
    fn a_vertex_between_a_wall_edge_and_an_angled_free_edge_is_where_their_lines_cross() {
        use arc_base::id::Identifier;
        let id = ObjectId::from_u128(5);
        let wall = Wall {
            start: [0.0, 0.0],
            end: [10.0, 0.0],
            thickness: 0.2,
            core: [0.0, 0.0],
            height: 3.0,
            base_offset: 0.0,
            location: crate::wall::LocationLine::Center,
            start_join: crate::wall::JoinStyle::Auto,
            end_join: crate::wall::JoinStyle::Auto,
        };
        let v = |x: f64, y: f64, edge: Option<Association>| Vertex {
            loop_index: 0,
            point: [x, y],
            edge,
            bulge: 0.0,
        };
        let follows = Some(Association {
            wall: id,
            face: Face::Center,
            offset: 0.0,
        });
        // The edge from the first vertex follows the wall; the edge from (10, 1) to (9, 5) is free
        // and slanted: the vertex at (10, 1) moves to where that line meets the wall's.
        let floor = Floor {
            thickness: 0.2,
            top_offset: 0.0,
            vertices: vec![
                v(0.0, 0.0, follows),
                v(10.0, 1.0, None),
                v(9.0, 5.0, None),
                v(1.0, 5.0, None),
            ],
        };
        let b = floor.boundary(&|w| (w == id).then_some(wall)).unwrap();
        assert!(
            (b.loops[0][1][0] - 10.25).abs() < 1e-9 && b.loops[0][1][1].abs() < 1e-9,
            "{:?}",
            b.loops[0][1]
        );
        // An arc beside the wall's edge cannot meet it: the vertex stays on the wall's line.
        let mut arc = floor.clone();
        arc.vertices[1].bulge = 0.5;
        let b = arc.boundary(&|w| (w == id).then_some(wall)).unwrap();
        assert!((b.loops[0][1][0] - 10.0).abs() < 1e-9 && b.loops[0][1][1].abs() < 1e-9);
    }
}
