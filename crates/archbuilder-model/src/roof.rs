//! The roof by footprint: authored data read from a record, the outline and the sloped surface
//! derived from it, the slab solid and the lines a plan draws.
//!
//! A roof record holds a closed footprint as a list of edges. Each edge starts at a vertex with
//! an authored point and may follow a wall face, exactly as a floor edge does
//! ([`crate::floor::Association`]), so the footprint follows the walls it was picked from. Each
//! edge either defines a slope (an angle) or does not (a gable: a vertical plane), and has an
//! overhang, its own or the roof's.
//!
//! Derived each time, never stored ([`Surface::compute`]): the outline is the footprint with
//! every edge moved outward by its overhang. Each sloped edge's plane passes through its
//! footprint edge at the base height (the storey's level plus the base offset) and rises inward
//! at the edge's slope; an overhang drops below the base. A gable edge's plane is vertical. The
//! underside is the weighted straight skeleton of these planes (`arc_geom::weighted_skeleton`),
//! clipped to the outline. The slab lies between the underside and the top, whose planes are the
//! underside's moved square to the slope by the type's thickness ([`crate::terrain::solid`]).
//! Ridges, hips and valleys are the skeleton's lines inside the outline.

use arc_base::math::Point2;
use arc_base::{ObjectId, Result, Tolerance};
use arc_doc::{Snapshot, Value};
pub use arc_geom::ArcKind;
use arc_geom::{FaceTag, Polyhedron, Skeleton, weighted_skeleton};

use crate::floor::{self, Association, Boundary, Face, Floor, Vertex};
use crate::terrain::{self, Bottom, Piece, Plane};
use crate::types::{self, roof};
use crate::wall::Wall;

/// The most edges of a footprint.
pub const MAX_EDGES: usize = arc_geom::MAX_SKELETON_EDGES;
/// The range of an edge's slope, in radians (1° to 85°).
pub const SLOPE_RANGE: (f64, f64) = (0.017_453_292_519_943_295, 1.483_529_864_195_180_3);
/// The slope of a new edge, in radians (15°).
pub const DEFAULT_SLOPE: f64 = 0.261_799_387_799_149_4;
/// The range of an overhang, in meters.
pub const OVERHANG_RANGE: (f64, f64) = (0.0, 10.0);
/// The overhang of a new roof, in meters.
pub const DEFAULT_OVERHANG: f64 = 0.5;
/// The thickness of a roof without a type, in meters.
pub const DEFAULT_THICKNESS: f64 = 0.3;

/// One edge of the footprint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edge {
    /// The vertex the edge starts at, with the wall face it follows.
    pub vertex: Vertex,
    /// The slope in radians; `None` for an edge that defines no slope (a gable).
    pub slope: Option<f64>,
    /// The edge's own overhang; `None` takes the roof's.
    pub overhang: Option<f64>,
}

/// The authored data of a roof, lengths in meters.
#[derive(Debug, Clone, PartialEq)]
pub struct Roof {
    /// The thickness of the slab: the total of the type's layers.
    pub thickness: f64,
    /// The height of the underside at the eaves above the storey's level.
    pub base_offset: f64,
    /// The overhang of the edges that have none of their own.
    pub overhang: f64,
    /// The edges of the footprint, in order.
    pub edges: Vec<Edge>,
}

/// Why a roof has no surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Refusal {
    /// The record cannot be read, or a count or number is out of its bounds.
    Malformed,
    /// The footprint is not a valid simple loop.
    Footprint(floor::Refusal),
    /// The outline (the footprint with the overhangs) crosses or touches itself.
    Outline,
    /// No edge defines a slope.
    NoSlope,
    /// Two neighboring edges lie on one line with different slopes, at this point.
    CollinearSlopes([f64; 2]),
    /// No continuous surface covers the outline near this point.
    Unclosed([f64; 2]),
    /// The surface could not be computed reliably near this point.
    Degenerate([f64; 2]),
}

impl Roof {
    /// The roof of a record's field values; `None` when a field is missing or malformed.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Option<Self> {
        let m = |at| types::meters(values, at);
        let Some(Some(Value::List(items))) = values.get(roof::EDGES) else {
            return None;
        };
        if items.len() > MAX_EDGES {
            return None;
        }
        let mut edges = Vec::with_capacity(items.len());
        for item in items {
            let Value::Record(fields) = item else {
                return None;
            };
            let e = |at| types::meters(fields, at);
            use roof::edge;
            let association = match fields.get(edge::WALL) {
                Some(Some(Value::Reference(wall))) => Some(Association {
                    wall: *wall,
                    face: match fields.get(edge::FACE) {
                        Some(Some(Value::Enum(code))) => Face::from_code(*code),
                        _ => Face::default(),
                    },
                    offset: e(edge::OFFSET).unwrap_or(0.0),
                }),
                _ => None,
            };
            let sloped = !matches!(fields.get(edge::SLOPED), Some(Some(Value::Bool(false))));
            let slope = match fields.get(edge::SLOPE) {
                Some(Some(Value::Quantity(q))) => q.value(),
                _ => DEFAULT_SLOPE,
            };
            edges.push(Edge {
                vertex: Vertex {
                    loop_index: 0,
                    point: [e(edge::X)?, e(edge::Y)?],
                    edge: association,
                    bulge: 0.0,
                },
                slope: sloped.then_some(slope),
                overhang: e(edge::OVERHANG),
            });
        }
        Some(Self {
            thickness: m(roof::THICKNESS)?,
            base_offset: m(roof::BASE_OFFSET)?,
            overhang: m(roof::OVERHANG).unwrap_or(DEFAULT_OVERHANG),
            edges,
        })
    }

    /// The edge records of `edges`, for the record's list field. An edge without a slope keeps
    /// the slope it had (`kept`, the same length as `edges`, or the default) for when it slopes
    /// again.
    #[must_use]
    pub fn edge_values(edges: &[Edge], kept: &[f64]) -> Value {
        Value::List(
            edges
                .iter()
                .enumerate()
                .map(|(k, e)| {
                    let slope = e
                        .slope
                        .or_else(|| kept.get(k).copied())
                        .unwrap_or(DEFAULT_SLOPE);
                    let v = e.vertex;
                    Value::Record(vec![
                        Some(types::length(v.point[0])),
                        Some(types::length(v.point[1])),
                        v.edge.map(|a| Value::Reference(a.wall)),
                        Some(Value::Enum(v.edge.map_or(0, |a| a.face.code()))),
                        Some(types::length(v.edge.map_or(0.0, |a| a.offset))),
                        Some(Value::Bool(e.slope.is_some())),
                        Some(types::angle(slope)),
                        e.overhang.map(types::length),
                    ])
                })
                .collect(),
        )
    }

    /// The stored slope of every edge, sloped or not.
    #[must_use]
    pub fn kept_slopes(values: &[Option<Value>]) -> Vec<f64> {
        let Some(Some(Value::List(items))) = values.get(roof::EDGES) else {
            return Vec::new();
        };
        items
            .iter()
            .map(|item| match item {
                Value::Record(f) => match f.get(roof::edge::SLOPE) {
                    Some(Some(Value::Quantity(q))) => q.value(),
                    _ => DEFAULT_SLOPE,
                },
                _ => DEFAULT_SLOPE,
            })
            .collect()
    }

    /// The footprint as a floor's vertex list, for the boundary rules of floors.
    #[must_use]
    pub fn as_floor(&self) -> Floor {
        Floor {
            thickness: self.thickness,
            top_offset: self.base_offset,
            vertices: self.edges.iter().map(|e| e.vertex).collect(),
        }
    }

    /// The walls the edges follow, each once.
    #[must_use]
    pub fn walls(&self) -> Vec<ObjectId> {
        self.as_floor().walls()
    }

    /// The overhang of edge `k`.
    #[must_use]
    pub fn overhang_of(&self, k: usize) -> f64 {
        self.edges
            .get(k)
            .and_then(|e| e.overhang)
            .unwrap_or(self.overhang)
    }

    /// The footprint with the edges that follow walls derived from the walls `wall_of` finds.
    #[must_use]
    pub fn footprint(&self, wall_of: &dyn Fn(ObjectId) -> Option<Wall>) -> Option<Vec<[f64; 2]>> {
        self.as_floor()
            .boundary(wall_of)
            .and_then(|b| b.loops.into_iter().next())
    }

    /// The footprint read from the walls of `snapshot`.
    #[must_use]
    pub fn footprint_in(&self, snapshot: &Snapshot) -> Option<Vec<[f64; 2]>> {
        self.as_floor()
            .boundary_in(snapshot)
            .and_then(|b| b.loops.into_iter().next())
    }

    /// The edges after the walls `deleted` are gone: the edges that followed one keep their
    /// position and lose the association (see [`Floor::frozen`]).
    #[must_use]
    pub fn frozen(
        &self,
        footprint: &[[f64; 2]],
        deleted: &std::collections::BTreeSet<ObjectId>,
    ) -> Option<Vec<Edge>> {
        let boundary = Boundary {
            loops: vec![footprint.to_vec()],
            ..Boundary::default()
        };
        let vertices = self.as_floor().frozen(&boundary, deleted)?;
        Some(
            self.edges
                .iter()
                .zip(vertices)
                .map(|(e, vertex)| Edge { vertex, ..*e })
                .collect(),
        )
    }
}

/// The outline: every edge of `footprint` moved outward by its overhang (the vertices meet
/// where the moved lines do; parallel neighbors keep the moved vertex).
fn outline(footprint: &[[f64; 2]], overhang: &dyn Fn(usize) -> f64) -> Vec<[f64; 2]> {
    outline_by(footprint, overhang)
}

/// The footprint with every edge `k` moved outward by `by(k)`.
fn outline_by(footprint: &[[f64; 2]], overhang: &dyn Fn(usize) -> f64) -> Vec<[f64; 2]> {
    let n = footprint.len();
    let ccw = floor::signed_area(footprint) > 0.0;
    let line = |k: usize| -> Option<([f64; 2], [f64; 2])> {
        let a = *footprint.get(k)?;
        let b = *footprint.get((k + 1) % n)?;
        let d = [b[0] - a[0], b[1] - a[1]];
        let length = d[0].hypot(d[1]);
        if length <= 0.0 {
            return None;
        }
        let u = [d[0] / length, d[1] / length];
        // The outward normal: right of the direction for a counterclockwise loop.
        let out = if ccw { [u[1], -u[0]] } else { [-u[1], u[0]] };
        let o = overhang(k);
        Some(([a[0] + out[0] * o, a[1] + out[1] * o], u))
    };
    (0..n)
        .filter_map(|k| {
            let (p0, u0) = line((k + n - 1) % n)?;
            let (p1, u1) = line(k)?;
            let det = u0[0] * u1[1] - u0[1] * u1[0];
            if det.abs() < Tolerance::DEFAULT.angular {
                return Some(p1);
            }
            let w = [p1[0] - p0[0], p1[1] - p0[1]];
            let t = (w[0] * u1[1] - w[1] * u1[0]) / det;
            Some([p0[0] + u0[0] * t, p0[1] + u0[1] * t])
        })
        .collect()
}

/// The derived surface of a roof.
#[derive(Debug, Clone)]
pub struct Surface {
    /// The footprint.
    pub footprint: Vec<[f64; 2]>,
    /// The outline: the footprint with the overhangs.
    pub outline: Vec<[f64; 2]>,
    /// The skeleton of the underside, over the extended outline; heights from `base - drop`.
    pub skeleton: Skeleton,
    /// The skeleton of the top, over the extended outline moved out by the thickness.
    pub over: Skeleton,
    /// The elevation of the underside at the footprint line.
    pub base: f64,
    /// How far below the base the skeletons start.
    pub drop: f64,
    /// The thickness, measured square to the slope.
    pub thickness: f64,
    /// The outline cut into triangles, which the pieces are clipped to.
    triangles: Vec<Vec<[f64; 2]>>,
}

/// A line a plan draws: its ends and what it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanLine {
    /// The lower end.
    pub from: [f64; 2],
    /// The upper end.
    pub to: [f64; 2],
    /// A ridge, hip or valley.
    pub kind: ArcKind,
}

/// Whether a simple-loop check passes for `points`, mapped to the refusal of an outline.
fn check_outline(points: &[[f64; 2]]) -> std::result::Result<(), Refusal> {
    Boundary {
        loops: vec![points.to_vec()],
        ..Boundary::default()
    }
    .check()
    .map_err(|r| match r {
        floor::Refusal::Malformed => Refusal::Malformed,
        _ => Refusal::Outline,
    })
}

fn skeleton_of(points: &[[f64; 2]], speeds: &[f64]) -> std::result::Result<Skeleton, Refusal> {
    let points: Vec<Point2> = points.iter().map(|p| Point2::new(p[0], p[1])).collect();
    weighted_skeleton(&points, speeds, &Tolerance::DEFAULT).map_err(|e| {
        let at = |key: &str| match e.arguments().get(key) {
            Some(arc_base::error::Value::Text(t)) => t.parse().unwrap_or(0.0),
            _ => 0.0,
        };
        let p = [at("x"), at("y")];
        match e.code() {
            "geom.skeleton.no_slope" => Refusal::NoSlope,
            "geom.skeleton.collinear_speeds" => Refusal::CollinearSlopes(p),
            "geom.skeleton.unclosed" => Refusal::Unclosed(p),
            "geom.skeleton.polygon_invalid" => Refusal::Outline,
            _ => Refusal::Degenerate(p),
        }
    })
}

/// The plane of `edge` of `skeleton` as a piece's plane, its heights lifted by `lift`.
fn plane_of(skeleton: &Skeleton, edge: usize, at: [f64; 2], lift: f64) -> Option<Plane> {
    let p = Point2::new(at[0], at[1]);
    let z = skeleton.edge_height(edge, &p)?;
    let zx = skeleton.edge_height(edge, &Point2::new(p.x + 1.0, p.y))?;
    let zy = skeleton.edge_height(edge, &Point2::new(p.x, p.y + 1.0))?;
    Some(Plane {
        at,
        z: lift + z,
        grad: [zx - z, zy - z],
    })
}

/// The sloped cells of `skeleton` with their planes, lifted by `lift`.
fn cells(skeleton: &Skeleton, lift: f64) -> Vec<(Vec<[f64; 2]>, Plane)> {
    skeleton
        .cells
        .iter()
        .filter_map(|cell| {
            let first = cell.points.first()?;
            let plane = plane_of(skeleton, cell.edge, [first.x, first.y], lift)?;
            Some((cell.points.iter().map(|p| [p.x, p.y]).collect(), plane))
        })
        .collect()
}

/// The part of the segment `a`-`b` inside the convex counterclockwise polygon `poly`.
fn clip_segment(a: [f64; 2], b: [f64; 2], poly: &[[f64; 2]]) -> Option<([f64; 2], [f64; 2])> {
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    let d = [b[0] - a[0], b[1] - a[1]];
    let n = poly.len();
    for k in 0..n {
        let (p, q) = (*poly.get(k)?, *poly.get((k + 1) % n)?);
        let e = [q[0] - p[0], q[1] - p[1]];
        // Inside is the left of each edge: cross(e, x - p) >= 0.
        let start = e[0] * (a[1] - p[1]) - e[1] * (a[0] - p[0]);
        let rate = e[0] * d[1] - e[1] * d[0];
        if rate.abs() < f64::EPSILON {
            if start < 0.0 {
                return None;
            }
            continue;
        }
        let t = -start / rate;
        if rate > 0.0 {
            t0 = t0.max(t);
        } else {
            t1 = t1.min(t);
        }
    }
    (t1 - t0 > 1e-12).then(|| {
        (
            [a[0] + d[0] * t0, a[1] + d[1] * t0],
            [a[0] + d[0] * t1, a[1] + d[1] * t1],
        )
    })
}

impl Surface {
    /// The surface of `roof` over `footprint` with the underside at `base` along the footprint
    /// line (the wall line).
    ///
    /// Every plane passes through its footprint edge at `base`; an overhang drops below it. To
    /// solve planes that start at different eave heights with one skeleton, each sloped edge is
    /// first moved out to where its plane is `drop` below the base (the deepest eave), the
    /// skeleton is solved there and clipped to the outline. The top's planes are the underside's
    /// moved square to the slope by the thickness, solved the same way.
    ///
    /// # Errors
    /// The [`Refusal`] that names what is wrong.
    pub fn compute(
        roof: &Roof,
        footprint: Vec<[f64; 2]>,
        base: f64,
    ) -> std::result::Result<Self, Refusal> {
        if footprint.len() != roof.edges.len() || footprint.len() > MAX_EDGES {
            return Err(Refusal::Malformed);
        }
        Boundary {
            loops: vec![footprint.clone()],
            ..Boundary::default()
        }
        .check()
        .map_err(Refusal::Footprint)?;
        let tans: Vec<Option<f64>> = roof.edges.iter().map(|e| e.slope.map(f64::tan)).collect();
        if tans.iter().flatten().any(|t| !t.is_finite() || *t <= 0.0) {
            return Err(Refusal::Malformed);
        }
        let speeds: Vec<f64> = tans.iter().map(|t| t.map_or(0.0, |t| 1.0 / t)).collect();
        let outline = outline(&footprint, &|k| roof.overhang_of(k));
        if outline.len() != footprint.len() {
            return Err(Refusal::Outline);
        }
        check_outline(&outline)?;
        let drop = tans
            .iter()
            .enumerate()
            .filter_map(|(k, t)| t.map(|t| roof.overhang_of(k) * t))
            .fold(0.0_f64, f64::max);
        let reach = |k: usize, thick: bool| match tans.get(k).copied().flatten() {
            Some(t) => {
                let extra = if thick {
                    roof.thickness * (1.0 + t * t).sqrt() / t
                } else {
                    0.0
                };
                drop / t + extra
            }
            None => roof.overhang_of(k),
        };
        let under_outline = outline_by(&footprint, &|k| reach(k, false));
        let over_outline = outline_by(&footprint, &|k| reach(k, true));
        check_outline(&under_outline)?;
        check_outline(&over_outline)?;
        let skeleton = skeleton_of(&under_outline, &speeds)?;
        let over = skeleton_of(&over_outline, &speeds)?;
        let region = Boundary {
            loops: vec![outline.clone()],
            ..Boundary::default()
        }
        .region()
        .map_err(|_| Refusal::Outline)?;
        let mesh = arc_geom::triangulate(&region, CHORD, std::f64::consts::PI, 4 * MAX_EDGES)
            .map_err(|_| Refusal::Outline)?;
        let triangles = mesh
            .triangles()
            .iter()
            .filter_map(|t| {
                let v: Option<Vec<[f64; 2]>> = t
                    .iter()
                    .map(|i| {
                        mesh.vertices()
                            .get(usize::try_from(*i).ok()?)
                            .map(|p| [p.x, p.y])
                    })
                    .collect();
                let mut v = v?;
                if terrain::double_area(&v) < 0.0 {
                    v.reverse();
                }
                Some(v)
            })
            .collect();
        Ok(Self {
            footprint,
            outline,
            skeleton,
            over,
            base,
            drop,
            thickness: roof.thickness,
            triangles,
        })
    }

    /// The outline cut into counterclockwise triangles.
    #[must_use]
    pub fn outline_triangles(&self) -> &[Vec<[f64; 2]>] {
        &self.triangles
    }

    /// The elevation of the underside over `p`; `None` outside the outline.
    #[must_use]
    pub fn underside(&self, p: [f64; 2]) -> Option<f64> {
        let inside = Boundary {
            loops: vec![self.outline.clone()],
            ..Boundary::default()
        }
        .contains(p);
        if !inside {
            return None;
        }
        self.skeleton
            .height_at(&Point2::new(p[0], p[1]), &Tolerance::DEFAULT)
            .map(|z| self.base - self.drop + z)
    }

    /// The underside over the outline as convex pieces with their planes.
    #[must_use]
    pub fn pieces(&self) -> Vec<Piece> {
        let lift = self.base - self.drop;
        let mut out = Vec::new();
        for (cell, plane) in cells(&self.skeleton, lift) {
            for triangle in &self.triangles {
                let part = terrain::clip_convex(triangle, &cell);
                if part.len() >= 3 && terrain::double_area(&part) > AREA {
                    out.push(Piece {
                        points: part,
                        plane,
                        upper: None,
                    });
                }
            }
        }
        out
    }

    /// The slab over the outline as convex pieces, each with its underside and top planes.
    fn slab_pieces(&self) -> Vec<Piece> {
        let lift = self.base - self.drop;
        let tops = cells(&self.over, lift);
        let mut out = Vec::new();
        for piece in self.pieces() {
            for (cell, upper) in &tops {
                let part = terrain::clip_convex(&piece.points, cell);
                if part.len() >= 3 && terrain::double_area(&part) > AREA {
                    out.push(Piece {
                        points: part,
                        plane: piece.plane,
                        upper: Some(*upper),
                    });
                }
            }
        }
        out
    }

    /// The slab between the underside and the top, the thickness square to the slope.
    ///
    /// # Errors
    /// The solid kernel's errors.
    pub fn solid(&self) -> Result<Polyhedron> {
        terrain::solid(&self.slab_pieces(), 0.0, Bottom::Pieces(0.0), &|_, _| {
            FaceTag::Untagged
        })
    }

    /// The ridges, hips and valleys in plan, inside the outline.
    #[must_use]
    pub fn lines(&self) -> Vec<PlanLine> {
        let mut out = Vec::new();
        for a in &self.skeleton.arcs {
            let (from, to) = ([a.start.x, a.start.y], [a.end.x, a.end.y]);
            let d = [to[0] - from[0], to[1] - from[1]];
            let length2 = d[0] * d[0] + d[1] * d[1];
            if length2 <= 0.0 {
                continue;
            }
            let along = |p: [f64; 2]| ((p[0] - from[0]) * d[0] + (p[1] - from[1]) * d[1]) / length2;
            // The stretches inside the triangles, joined where they meet.
            let mut spans: Vec<(f64, f64)> = self
                .triangles
                .iter()
                .filter_map(|t| clip_segment(from, to, t))
                .map(|(p, q)| (along(p), along(q)))
                .collect();
            spans.sort_by(|x, y| x.0.total_cmp(&y.0));
            let mut merged: Vec<(f64, f64)> = Vec::new();
            let gap = Tolerance::DEFAULT.linear / length2.sqrt();
            for (t0, t1) in spans {
                match merged.last_mut() {
                    Some(last) if t0 <= last.1 + gap => last.1 = last.1.max(t1),
                    _ => merged.push((t0, t1)),
                }
            }
            for (t0, t1) in merged {
                out.push(PlanLine {
                    from: [from[0] + d[0] * t0, from[1] + d[1] * t0],
                    to: [from[0] + d[0] * t1, from[1] + d[1] * t1],
                    kind: a.kind,
                });
            }
        }
        out
    }

    /// The area of the outline in plan, in square meters.
    #[must_use]
    pub fn plan_area(&self) -> f64 {
        floor::signed_area(&self.outline).abs()
    }

    /// The elevation of the highest point of the top.
    #[must_use]
    pub fn top(&self) -> f64 {
        self.base - self.drop + self.over.height
    }
}

/// The roofs derived from the last snapshot asked about.
type Cache = Option<(
    Snapshot,
    std::collections::HashMap<ObjectId, std::result::Result<Derived, Refusal>>,
)>;
static CACHE: std::sync::Mutex<Cache> = std::sync::Mutex::new(None);
/// The most roofs the cache holds for one snapshot.
const CACHE_ENTRIES: usize = 4096;

/// The chord of the outline's triangulation; the outline has no curves.
const CHORD: f64 = 0.01;
/// The smallest twice-area of a piece kept, in square meters.
const AREA: f64 = 1e-12;

/// What a roof reads from a snapshot: its data and its surface.
#[derive(Debug, Clone)]
pub struct Derived {
    /// The authored data.
    pub roof: Roof,
    /// The surface.
    pub surface: Surface,
}

impl Derived {
    /// Reads the roof `object` of `snapshot`.
    ///
    /// # Errors
    /// The [`Refusal`] that names what is wrong.
    pub fn of(snapshot: &Snapshot, object: &arc_doc::Object) -> std::result::Result<Self, Refusal> {
        // Derived once per snapshot: the plan, the viewport, picking and the walls under the
        // roof all ask for the same surface.
        if let Ok(guard) = CACHE.lock()
            && let Some((cached, found)) = guard.as_ref()
            && cached.ptr_eq(snapshot)
            && let Some(hit) = found.get(&object.id)
        {
            return hit.clone();
        }
        let result = Self::derive(snapshot, object);
        if let Ok(mut guard) = CACHE.lock() {
            let fresh = guard.as_ref().is_none_or(|(cached, found)| {
                !cached.ptr_eq(snapshot) || found.len() >= CACHE_ENTRIES
            });
            if fresh {
                *guard = Some((snapshot.clone(), std::collections::HashMap::new()));
            }
            if let Some((_, found)) = guard.as_mut() {
                found.insert(object.id, result.clone());
            }
        }
        result
    }

    fn derive(snapshot: &Snapshot, object: &arc_doc::Object) -> std::result::Result<Self, Refusal> {
        let roof = Roof::from_values(&object.values).ok_or(Refusal::Malformed)?;
        let level = object
            .header
            .owner
            .and_then(|id| crate::spatial::get(snapshot, id))
            .map_or(0.0, |s| crate::spatial::storey_elevation(snapshot, &s));
        let footprint = roof.footprint_in(snapshot).ok_or(Refusal::Malformed)?;
        let surface = Surface::compute(&roof, footprint, level + roof.base_offset)?;
        Ok(Self { roof, surface })
    }
}

/// The error of a refusal, naming the roof `name`.
#[must_use]
pub fn refusal_error(refusal: Refusal, name: &str) -> arc_base::Error {
    use crate::error::codes;
    let located = |code, p: [f64; 2]| {
        arc_base::Error::new(code)
            .with_arg("name", name.to_owned())
            .with_arg("x", format!("{:.3}", p[0]))
            .with_arg("y", format!("{:.3}", p[1]))
    };
    match refusal {
        Refusal::Malformed => {
            arc_base::Error::new(codes::ROOF_MALFORMED).with_arg("name", name.to_owned())
        }
        Refusal::Footprint(_) => {
            arc_base::Error::new(codes::ROOF_FOOTPRINT).with_arg("name", name.to_owned())
        }
        Refusal::Outline => {
            arc_base::Error::new(codes::ROOF_OUTLINE).with_arg("name", name.to_owned())
        }
        Refusal::NoSlope => {
            arc_base::Error::new(codes::ROOF_NO_SLOPE).with_arg("name", name.to_owned())
        }
        Refusal::CollinearSlopes(p) => located(codes::ROOF_COLLINEAR_SLOPES, p),
        Refusal::Unclosed(p) => located(codes::ROOF_UNCLOSED, p),
        Refusal::Degenerate(p) => located(codes::ROOF_DEGENERATE, p),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roof(points: &[[f64; 2]], slopes: &[Option<f64>], overhang: f64) -> Roof {
        Roof {
            thickness: 0.3,
            base_offset: 0.0,
            overhang,
            edges: points
                .iter()
                .zip(slopes)
                .map(|(p, s)| Edge {
                    vertex: Vertex {
                        loop_index: 0,
                        point: *p,
                        edge: None,
                        bulge: 0.0,
                    },
                    slope: *s,
                    overhang: None,
                })
                .collect(),
        }
    }

    const RECT: [[f64; 2]; 4] = [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]];

    #[test]
    fn hip_roof_ridge_and_heights() {
        let a = std::f64::consts::FRAC_PI_4;
        let r = roof(&RECT, &[Some(a); 4], 0.0);
        let s = Surface::compute(&r, RECT.to_vec(), 3.0).unwrap();
        let ridge: Vec<_> = s
            .lines()
            .into_iter()
            .filter(|l| l.kind == ArcKind::Ridge)
            .collect();
        assert_eq!(ridge.len(), 1);
        let length = (ridge[0].to[0] - ridge[0].from[0]).hypot(ridge[0].to[1] - ridge[0].from[1]);
        assert!((length - 4.0).abs() < 1e-9);
        // The thickness is square to the 45° slope: 0.3 / cos 45° vertically.
        let vertical = 0.3 * 2.0_f64.sqrt();
        assert!((s.top() - (3.0 + 3.0 + vertical)).abs() < 1e-9);
        assert!((s.underside([5.0, 1.0]).unwrap() - 4.0).abs() < 1e-9);
        let solid = s.solid().unwrap();
        assert!((solid.volume() - 60.0 * vertical).abs() < 1e-6);
    }

    #[test]
    fn the_base_is_at_the_wall_line_and_overhangs_drop_below_it() {
        let a = std::f64::consts::FRAC_PI_4;
        let mut r = roof(&RECT, &[Some(a); 4], 0.5);
        // A deeper overhang on one edge and a steeper slope on another: eaves at different
        // heights, every plane through its wall line at the base.
        r.edges[0].overhang = Some(1.5);
        r.edges[1].slope = Some(1.0);
        let s = Surface::compute(&r, RECT.to_vec(), 3.0).unwrap();
        assert!((s.underside([5.0, 0.0]).unwrap() - 3.0).abs() < 1e-9);
        assert!((s.underside([5.0, -1.4]).unwrap() - 1.6).abs() < 1e-9);
        assert!((s.underside([10.0, 3.0]).unwrap() - 3.0).abs() < 1e-9);
        assert!((s.plan_area() - 11.0 * 8.0).abs() < 1e-9);
        let solid = s.solid().unwrap();
        assert!(solid.volume() > 0.0);
        // The pieces cover the outline once.
        let covered: f64 = s
            .pieces()
            .iter()
            .map(|p| terrain::double_area(&p.points) / 2.0)
            .sum();
        assert!((covered - s.plan_area()).abs() < 1e-6);
    }

    #[test]
    fn a_wall_partly_outside_follows_the_roof_only_under_it() {
        let a = std::f64::consts::FRAC_PI_4;
        let r = roof(&RECT, &[Some(a); 4], 0.0);
        let s = Surface::compute(&r, RECT.to_vec(), 3.0).unwrap();
        let wall = Wall {
            start: [8.0, 3.0],
            end: [14.0, 3.0],
            thickness: 0.2,
            core: [0.0, 0.0],
            height: 3.0,
            base_offset: 0.0,
            location: crate::wall::LocationLine::Center,
            start_join: crate::wall::JoinStyle::Auto,
            end_join: crate::wall::JoinStyle::Auto,
        };
        let solid = crate::wall::under_roof(&wall, wall.plain_footprint(), 0.0, &s).unwrap();
        // Under the hip from x = 8 to 10 the top rises to 3 + (10 - x); beyond the outline the
        // wall keeps its 3 m.
        assert!(
            (solid.volume() - 0.2 * (8.0 + 12.0)).abs() < 1e-6,
            "{}",
            solid.volume()
        );
        assert!((solid.bounding_box().max().z - 5.0).abs() < 1e-9);
    }

    #[test]
    fn refusals() {
        let r = roof(&RECT, &[None; 4], 0.0);
        assert_eq!(
            Surface::compute(&r, RECT.to_vec(), 0.0).unwrap_err(),
            Refusal::NoSlope
        );
        let l = [
            [0.0, 0.0],
            [10.0, 0.0],
            [10.0, 4.0],
            [4.0, 4.0],
            [4.0, 10.0],
            [0.0, 10.0],
        ];
        let slopes = [None, None, None, Some(0.5), None, None];
        let r = roof(&l, &slopes, 0.0);
        assert!(matches!(
            Surface::compute(&r, l.to_vec(), 0.0),
            Err(Refusal::Unclosed(_))
        ));
    }
}
