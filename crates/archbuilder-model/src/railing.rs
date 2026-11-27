//! The railing by path and its type: authored data read from records, and everything else derived.
//!
//! A railing record holds a sketched path (points with a height above the storey's level and the
//! bulge of the arc to the next point, as the boundary of a floor does) or, instead, a host (a
//! stair or a ramp) and the side of it; the path of a hosted railing is derived from the host each
//! time, so it follows the host. The record also holds the type, the extension at each end of the
//! rail, and the railing whose start the top rail continues to.
//!
//! A railing type ([`RailingType`]) holds the height, the profile of the top rail, an optional
//! handrail height, the profile and the largest spacing of the balusters, the size of the posts
//! and how the top rail turns at a corner (a mitered join or a fillet).
//!
//! Derived ([`Derived`]): the walking-surface polyline (arcs flattened), the posts (at both ends
//! and at every corner), the balusters (spaced evenly between posts, never farther apart than the
//! type says, measured along the path in 3D), the top rail with its extensions, its fillets and
//! its continuation, the handrail, the plan line and the solids. Heights are above the storey's
//! level.

use arc_base::math::{Point3, Vector3};
use arc_base::{Limits, ObjectId, Result, Tolerance};
use arc_doc::{Object, Snapshot, Value};
use arc_geom::{Curve2, Loop2, PlaneFrame, Polyhedron, Region, RegionPart, Segment2, extrude};

use crate::spatial;
use crate::types::{self, RAILING_TYPE, railing, railing_type};

/// The most points of a railing's path.
pub const MAX_POINTS: usize = 500;
/// The default horizontal length of an extension, in meters.
pub const DEFAULT_EXTENSION: f64 = 0.3;
/// The range of an extension's length, in meters.
pub const EXTENSION_RANGE: (f64, f64) = (0.0, 5.0);
/// The default height of the top rail, in meters.
pub const DEFAULT_HEIGHT: f64 = crate::stair::RAIL_HEIGHT;
/// The range of a rail height, in meters.
pub const HEIGHT_LIMITS: (f64, f64) = (0.3, 3.0);
/// The range of the size of a rail, baluster or post, in meters.
pub const SECTION_RANGE: (f64, f64) = (0.01, 0.5);
/// The range of the baluster spacing, in meters.
pub const SPACING_RANGE: (f64, f64) = (0.05, 2.0);
/// The default width of the top rail, in meters.
pub const DEFAULT_RAIL_WIDTH: f64 = 0.05;
/// The default depth of the top rail, in meters.
pub const DEFAULT_RAIL_DEPTH: f64 = 0.04;
/// The default size of a baluster, in meters.
pub const DEFAULT_BALUSTER_SIZE: f64 = 0.02;
/// The default baluster spacing, in meters.
pub const DEFAULT_BALUSTER_SPACING: f64 = 0.12;
/// The default size of a post, in meters.
pub const DEFAULT_POST_SIZE: f64 = 0.05;
/// The widest opening between two balusters (or a baluster and a post) that does not get a
/// warning, in meters: a sphere of 100 mm must not pass.
pub const CLEAR_GAP_LIMIT: f64 = 0.10;
/// The length of the return of a rail that returns to a wall, in meters.
pub const RETURN_LENGTH: f64 = 0.10;
/// The tolerance at which arcs are flattened, in meters.
const FLAT: f64 = 0.01;
/// The side of a corner, in radians, below which two directions are one.
const STRAIGHT: f64 = 0.087;
/// The chord of the solid kernel; rails are straight.
const CHORD: f64 = 0.01;

coded! {
    /// The section of a rail or a baluster.
    Profile {
        /// A rectangle.
        #[default]
        Square = 0, "square";
        /// A round section, drawn as a twelve-sided polygon.
        Round = 1, "round";
    }
}

coded! {
    /// How the top rail turns at a corner of the path.
    Corner {
        /// Two bars meeting at the corner, overlapped to close the gap.
        #[default]
        Miter = 0, "miter";
        /// The corner rounded by an arc of the type's fillet radius.
        Fillet = 1, "fillet";
    }
}

coded! {
    /// What the rail does past its end.
    Extension {
        /// Nothing: the rail ends at the last point.
        #[default]
        None = 0, "none";
        /// The rail continues level for the extension length.
        Horizontal = 1, "horizontal";
        /// The rail continues level, then turns toward the wall.
        ReturnWall = 2, "return_wall";
        /// The rail continues level, then drops to the floor.
        ReturnFloor = 3, "return_floor";
    }
}

coded! {
    /// A side of a path or of a host, seen walking along it.
    Side {
        /// The left side.
        #[default]
        Left = 0, "left";
        /// The right side.
        Right = 1, "right";
    }
}

/// A point of a sketched path: the position, the height of the walking surface above the
/// storey's level and the bulge of the arc to the next point (0 for a line; positive is
/// counterclockwise, `tan(sweep / 4)`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    /// `[x, y, z]` in meters.
    pub point: [f64; 3],
    /// The bulge of the edge that starts here.
    pub bulge: f64,
}

/// The authored data of a railing type, lengths in meters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RailingType {
    /// The height of the top rail's axis above the walking surface.
    pub height: f64,
    /// The profile of the top rail.
    pub rail_profile: Profile,
    /// The width of the top rail.
    pub rail_width: f64,
    /// The depth (height) of the top rail's section.
    pub rail_depth: f64,
    /// The height of the handrail; 0 for none.
    pub handrail_height: f64,
    /// The profile of the balusters.
    pub baluster_profile: Profile,
    /// The size of a baluster.
    pub baluster_size: f64,
    /// The largest distance between two balusters along the path.
    pub baluster_spacing: f64,
    /// The size of a post.
    pub post_size: f64,
    /// How the top rail turns at a corner.
    pub corner: Corner,
    /// The radius of a filleted corner.
    pub fillet_radius: f64,
}

impl Default for RailingType {
    fn default() -> Self {
        Self {
            height: DEFAULT_HEIGHT,
            rail_profile: Profile::Square,
            rail_width: DEFAULT_RAIL_WIDTH,
            rail_depth: DEFAULT_RAIL_DEPTH,
            handrail_height: 0.0,
            baluster_profile: Profile::Square,
            baluster_size: DEFAULT_BALUSTER_SIZE,
            baluster_spacing: DEFAULT_BALUSTER_SPACING,
            post_size: DEFAULT_POST_SIZE,
            corner: Corner::Miter,
            fillet_radius: 0.15,
        }
    }
}

fn code(values: &[Option<Value>], at: usize) -> u32 {
    match values.get(at) {
        Some(Some(Value::Enum(c))) => *c,
        _ => 0,
    }
}

impl RailingType {
    /// The type of a record's field values; `None` when a field is missing or malformed.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Option<Self> {
        let m = |at| types::meters(values, at);
        Some(Self {
            height: m(railing_type::HEIGHT)?,
            rail_profile: Profile::from_code(code(values, railing_type::RAIL_PROFILE)),
            rail_width: m(railing_type::RAIL_WIDTH)?,
            rail_depth: m(railing_type::RAIL_DEPTH)?,
            handrail_height: m(railing_type::HANDRAIL_HEIGHT)?,
            baluster_profile: Profile::from_code(code(values, railing_type::BALUSTER_PROFILE)),
            baluster_size: m(railing_type::BALUSTER_SIZE)?,
            baluster_spacing: m(railing_type::BALUSTER_SPACING)?,
            post_size: m(railing_type::POST_SIZE)?,
            corner: Corner::from_code(code(values, railing_type::CORNER)),
            fillet_radius: m(railing_type::FILLET_RADIUS)?,
        })
    }

    /// The field values of a record of this type named `name`.
    #[must_use]
    pub fn values(&self, name: &str) -> Vec<Option<Value>> {
        let mut values: Vec<Option<Value>> = vec![None; railing_type::FILLET_RADIUS + 1];
        values[railing_type::NAME] = Some(Value::String(name.to_owned()));
        values[railing_type::HEIGHT] = Some(types::length(self.height));
        values[railing_type::RAIL_PROFILE] = Some(Value::Enum(self.rail_profile.code()));
        values[railing_type::RAIL_WIDTH] = Some(types::length(self.rail_width));
        values[railing_type::RAIL_DEPTH] = Some(types::length(self.rail_depth));
        values[railing_type::HANDRAIL_HEIGHT] = Some(types::length(self.handrail_height));
        values[railing_type::BALUSTER_PROFILE] = Some(Value::Enum(self.baluster_profile.code()));
        values[railing_type::BALUSTER_SIZE] = Some(types::length(self.baluster_size));
        values[railing_type::BALUSTER_SPACING] = Some(types::length(self.baluster_spacing));
        values[railing_type::POST_SIZE] = Some(types::length(self.post_size));
        values[railing_type::CORNER] = Some(Value::Enum(self.corner.code()));
        values[railing_type::FILLET_RADIUS] = Some(types::length(self.fillet_radius));
        values
    }
}

/// The authored data of a railing.
#[derive(Debug, Clone, PartialEq)]
pub struct Railing {
    /// The sketched path; empty for a hosted railing.
    pub path: Vec<Vertex>,
    /// The railing type; the default type when absent.
    pub railing_type: Option<ObjectId>,
    /// The stair or ramp the railing is hosted on.
    pub host: Option<ObjectId>,
    /// The side of the host; the side of the path the wall is on, for a return to a wall.
    pub side: Side,
    /// The extension at the start of the rail.
    pub start: Extension,
    /// The extension at the end of the rail.
    pub end: Extension,
    /// The horizontal length of an extension.
    pub extension: f64,
    /// The railing whose start the top rail continues to.
    pub continues_to: Option<ObjectId>,
    /// On a floor: the edge the railing stands on (the number of the vertex it starts at); on a
    /// floor edge element: the number of its chain. The whole outer boundary of a floor, or the
    /// first chain of a floor edge, without it.
    pub host_edge: Option<usize>,
}

fn reference(values: &[Option<Value>], at: usize) -> Option<ObjectId> {
    match values.get(at) {
        Some(Some(Value::Reference(id))) => Some(*id),
        _ => None,
    }
}

impl Railing {
    /// The railing of a record's field values; `None` when a field is missing or malformed.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Option<Self> {
        let mut path = Vec::new();
        if let Some(Some(Value::List(items))) = values.get(railing::PATH) {
            for item in items {
                let Value::Record(fields) = item else {
                    return None;
                };
                let m = |at: usize| match fields.get(at) {
                    Some(Some(Value::Quantity(q))) => Some(q.value()),
                    _ => None,
                };
                let bulge = match fields.get(3) {
                    Some(Some(Value::F64(b))) => *b,
                    _ => 0.0,
                };
                path.push(Vertex {
                    point: [m(0)?, m(1)?, m(2)?],
                    bulge,
                });
            }
        }
        Some(Self {
            path,
            railing_type: reference(values, railing::RAILING_TYPE),
            host: reference(values, railing::HOST),
            side: Side::from_code(code(values, railing::SIDE)),
            start: Extension::from_code(code(values, railing::START_EXTENSION)),
            end: Extension::from_code(code(values, railing::END_EXTENSION)),
            extension: types::meters(values, railing::EXTENSION_LENGTH)?,
            continues_to: reference(values, railing::CONTINUES_TO),
            host_edge: match values.get(railing::HOST_EDGE) {
                Some(Some(Value::I64(n))) => usize::try_from(*n).ok(),
                _ => None,
            },
        })
    }

    /// The value of a path, for the record.
    #[must_use]
    pub fn path_value(path: &[Vertex]) -> Value {
        Value::List(
            path.iter()
                .map(|v| {
                    Value::Record(vec![
                        Some(types::length(v.point[0])),
                        Some(types::length(v.point[1])),
                        Some(types::length(v.point[2])),
                        Some(Value::F64(v.bulge)),
                    ])
                })
                .collect(),
        )
    }

    /// The field values of a record of this railing named `name`.
    #[must_use]
    pub fn values(&self, name: &str) -> Vec<Option<Value>> {
        let mut values: Vec<Option<Value>> = vec![None; railing::HOST_EDGE + 1];
        values[railing::NAME] = Some(Value::String(name.to_owned()));
        values[railing::PATH] = Some(Self::path_value(&self.path));
        values[railing::RAILING_TYPE] = self.railing_type.map(Value::Reference);
        values[railing::HOST] = self.host.map(Value::Reference);
        values[railing::SIDE] = Some(Value::Enum(self.side.code()));
        values[railing::START_EXTENSION] = Some(Value::Enum(self.start.code()));
        values[railing::END_EXTENSION] = Some(Value::Enum(self.end.code()));
        values[railing::EXTENSION_LENGTH] = Some(types::length(self.extension));
        values[railing::CONTINUES_TO] = self.continues_to.map(Value::Reference);
        values[railing::HOST_EDGE] = self
            .host_edge
            .and_then(|n| i64::try_from(n).ok())
            .map(Value::I64);
        values
    }

    /// The railing under the plane motion `xf`: the path moved (an arc keeps its sweep, so under
    /// a reflection its bulge changes sign) and the side swapped under a reflection.
    #[must_use]
    pub fn moved(&self, xf: &crate::transform::Xform) -> Self {
        let flip = xf.reflects();
        let path = self
            .path
            .iter()
            .map(|v| {
                let p = xf.point([v.point[0], v.point[1]]);
                Vertex {
                    point: [p[0], p[1], v.point[2]],
                    bulge: if flip { -v.bulge } else { v.bulge },
                }
            })
            .collect();
        Self {
            path,
            side: if flip {
                match self.side {
                    Side::Left => Side::Right,
                    Side::Right => Side::Left,
                }
            } else {
                self.side
            },
            ..self.clone()
        }
    }
}

/// Why a railing has no derived form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// A railing without a host has fewer than two points.
    NoPath,
    /// A coordinate is not finite, two consecutive points coincide, the host is gone or has no
    /// layout, or the railing continues to itself.
    Malformed,
}

/// One point of a flattened path: the position and whether it is a node (an authored point, or a
/// corner of the host) rather than a point of an arc.
type Node = ([f64; 3], bool);

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn lerp(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// The points of the edge from `a` to `b` bulging `bulge`, without `a` and including `b`: one
/// point for a line, chords within [`FLAT`] for an arc.
fn edge(a: [f64; 3], b: [f64; 3], bulge: f64) -> Vec<Node> {
    let chord = (b[0] - a[0]).hypot(b[1] - a[1]);
    if bulge.abs() < crate::floor::MIN_BULGE || chord <= 0.0 {
        return vec![(b, true)];
    }
    let sweep = 4.0 * bulge.atan();
    let s = bulge.abs() * chord / 2.0;
    let r = (chord * chord / 4.0 + s * s) / (2.0 * s);
    let n = [-(b[1] - a[1]) / chord, (b[0] - a[0]) / chord];
    let lift = bulge.signum() * (r - s);
    let center = [
        (a[0] + b[0]) / 2.0 + n[0] * lift,
        (a[1] + b[1]) / 2.0 + n[1] * lift,
    ];
    let start = (a[1] - center[1]).atan2(a[0] - center[0]);
    let step = 2.0 * (1.0 - (FLAT / r).min(1.0)).acos();
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a small count, clamped"
    )]
    let count = ((sweep.abs() / step.max(1.0e-3)).ceil() as usize).clamp(2, 256);
    let mut out = Vec::with_capacity(count);
    for k in 1..count {
        let t = k as f64 / count as f64;
        let phi = start + sweep * t;
        out.push((
            [
                center[0] + r * phi.cos(),
                center[1] + r * phi.sin(),
                a[2] + (b[2] - a[2]) * t,
            ],
            false,
        ));
    }
    out.push((b, true));
    out
}

/// The flattened polyline of a sketched path.
fn flatten(path: &[Vertex]) -> Vec<Node> {
    let mut out: Vec<Node> = path.first().map(|v| (v.point, true)).into_iter().collect();
    for pair in path.windows(2) {
        out.extend(edge(pair[0].point, pair[1].point, pair[0].bulge));
    }
    out
}

/// The unit direction from `a` to `b`, and the length.
fn direction(a: [f64; 3], b: [f64; 3]) -> Option<([f64; 3], f64)> {
    let l = distance(a, b);
    (l > 1.0e-9).then(|| ([(b[0] - a[0]) / l, (b[1] - a[1]) / l, (b[2] - a[2]) / l], l))
}

/// The angle between the directions of two consecutive segments.
fn turn(p: [f64; 3], v: [f64; 3], n: [f64; 3]) -> f64 {
    match (direction(p, v), direction(v, n)) {
        (Some((a, _)), Some((b, _))) => (a[0] * b[0] + a[1] * b[1] + a[2] * b[2])
            .clamp(-1.0, 1.0)
            .acos(),
        _ => 0.0,
    }
}

/// The polyline with the corners (nodes) rounded by arcs of `radius`: each corner is replaced by
/// a quadratic curve that starts and ends at most `radius` from it (and never past the middle of
/// a leg).
fn fillet(points: &[Node], radius: f64) -> Vec<Node> {
    let mut out: Vec<Node> = Vec::with_capacity(points.len());
    for (k, (v, node)) in points.iter().enumerate() {
        let (Some(prev), Some(next)) = (
            k.checked_sub(1).and_then(|j| points.get(j)),
            points.get(k + 1),
        ) else {
            out.push((*v, *node));
            continue;
        };
        if !*node || turn(prev.0, *v, next.0) < STRAIGHT {
            out.push((*v, *node));
            continue;
        }
        let (l1, l2) = (distance(prev.0, *v), distance(next.0, *v));
        let d = radius.min(l1 / 2.0).min(l2 / 2.0);
        let (a, b) = (lerp(*v, prev.0, d / l1), lerp(*v, next.0, d / l2));
        for i in 0..=4 {
            let t = f64::from(i) / 4.0;
            let (p, q) = (lerp(a, *v, t), lerp(*v, b, t));
            out.push((lerp(p, q, t), i == 0 || i == 4));
        }
    }
    out
}

/// A section of a bar as a loop of `(across, up)` corners.
fn section(profile: Profile, width: f64, depth: f64) -> Vec<[f64; 2]> {
    let (hw, hd) = (width / 2.0, depth / 2.0);
    match profile {
        Profile::Square => vec![[-hw, -hd], [hw, -hd], [hw, hd], [-hw, hd]],
        Profile::Round => (0..12)
            .map(|k| {
                let a = f64::from(k) * std::f64::consts::TAU / 12.0;
                [hw * a.cos(), hd * a.sin()]
            })
            .collect(),
    }
}

fn region(points: &[[f64; 2]]) -> Result<Region> {
    let mut curves = Vec::with_capacity(points.len());
    for k in 0..points.len() {
        let (a, b) = (points[k], points[(k + 1) % points.len()]);
        curves.push(Curve2::Segment(Segment2::new(
            arc_base::math::Point2::new(a[0], a[1]),
            arc_base::math::Point2::new(b[0], b[1]),
        )?));
    }
    Region::new(vec![RegionPart::new(
        Loop2::new(curves, &Tolerance::DEFAULT)?,
        Vec::new(),
    )])
}

/// A bar of `section` from `p` to `q`; vertical bars are across `x` and `y`.
fn bar(p: [f64; 3], q: [f64; 3], section: &[[f64; 2]]) -> Result<Polyhedron> {
    let tolerance = Tolerance::DEFAULT;
    let d = Vector3::new(q[0] - p[0], q[1] - p[1], q[2] - p[2]);
    let length = d.norm();
    let d = d / length;
    let side = Vector3::z().cross(&d);
    let u = if side.norm() > 1.0e-9 {
        side.normalize()
    } else {
        Vector3::x()
    };
    let v = d.cross(&u);
    let frame = PlaneFrame::new(Point3::new(p[0], p[1], p[2]), u, v, &tolerance)?;
    extrude(
        &region(section)?,
        &frame,
        length,
        CHORD,
        &tolerance,
        &Limits::DEFAULT,
    )
}

/// The bars of a rail along `points`. Where two bars meet at a corner each is lengthened by half
/// the width times the tangent of half the turn (at most one width), so that they overlap and the
/// outer side of the corner is closed.
///
/// shortcut: an overlap, not a true miter plane; exact miters need a sweep in the solid kernel.
fn rail_bars(points: &[[f64; 3]], section: &[[f64; 2]], width: f64) -> Result<Vec<Polyhedron>> {
    let mut out = Vec::new();
    let last = points.len().saturating_sub(1);
    for (k, pair) in points.windows(2).enumerate() {
        let Some((dir, length)) = direction(pair[0], pair[1]) else {
            continue;
        };
        let more = |at: usize, sign: f64, from: [f64; 3]| {
            let (before, after) = (at.checked_sub(1), points.get(at + 1));
            let Some(((prev, next), true)) = before
                .and_then(|j| points.get(j))
                .zip(after)
                .map(|pn| (pn, at > 0 && at < last))
            else {
                return from;
            };
            let angle = turn(*prev, points[at], *next);
            let e = (width / 2.0 * (angle / 2.0).tan()).clamp(0.0, width);
            [
                from[0] + dir[0] * sign * e,
                from[1] + dir[1] * sign * e,
                from[2] + dir[2] * sign * e,
            ]
        };
        let a = more(k, -1.0, pair[0]);
        let b = more(k + 1, 1.0, pair[1]);
        if length > 1.0e-6 {
            out.push(bar(a, b, section)?);
        }
    }
    Ok(out)
}

/// A post or baluster: its foot and its top, heights above the level.
pub type Post = ([f64; 3], [f64; 3]);

/// The railing as the document places it: the path, the type and everything derived.
#[derive(Debug, Clone)]
pub struct Derived {
    /// The record's identifier.
    pub id: ObjectId,
    /// The authored data.
    pub railing: Railing,
    /// The type, or the default one.
    pub kind: RailingType,
    /// The elevation of the storey's level.
    pub base: f64,
    /// The walking-surface polyline, heights above the level.
    pub surface: Vec<([f64; 3], bool)>,
    /// The top rail: the surface raised by the height, with fillets, extensions and the
    /// continuation.
    pub rail: Vec<[f64; 3]>,
    /// The handrail, empty when the type has none.
    pub handrail: Vec<[f64; 3]>,
}

/// The path along the chain of a floor edge: the slab's top at the edge, [`ramp::RAIL_INSET`] in
/// from it toward the slab, heights above `base`. A corner is offset along its miter.
fn along_chain(chain: &crate::floor_edge::Chain, base: f64) -> Vec<[f64; 3]> {
    let n = chain.normals.len();
    let normal_at = |k: usize| -> [f64; 2] {
        let (before, after) = if k == 0 {
            (
                if chain.closed {
                    chain.normals.get(n - 1)
                } else {
                    None
                },
                chain.normals.first(),
            )
        } else {
            (chain.normals.get(k - 1), chain.normals.get(k))
        };
        match (before, after) {
            (Some(a), Some(b)) => {
                let c = 1.0 + a[0] * b[0] + a[1] * b[1];
                if c > 0.25 {
                    [(a[0] + b[0]) / c, (a[1] + b[1]) / c]
                } else {
                    *a
                }
            }
            (Some(a), None) | (None, Some(a)) => *a,
            (None, None) => [0.0, 0.0],
        }
    };
    chain
        .points
        .iter()
        .enumerate()
        .map(|(k, p)| {
            let m = normal_at(if chain.closed && k == n { 0 } else { k });
            [
                p[0] - m[0] * crate::ramp::RAIL_INSET,
                p[1] - m[1] * crate::ramp::RAIL_INSET,
                p[2] - base,
            ]
        })
        .collect()
}

/// The surface polyline of a hosted railing: the host's side, lowered to the walking surface.
fn hosted(
    snapshot: &Snapshot,
    host: ObjectId,
    side: Side,
    edge: Option<usize>,
    base: f64,
) -> Option<Vec<Node>> {
    let object = spatial::get(snapshot, host)?;
    let left = side == Side::Left;
    let path: Vec<[f64; 3]> = match spatial::type_of(snapshot, host).as_deref() {
        Some(types::STAIR) => {
            let mut layout = crate::stair::Placed::of(snapshot, &object)?.layout.ok()?;
            layout.set_railings([true, true]);
            let rails = layout.rails();
            let path = if left { &rails[0] } else { &rails[1] };
            path.iter()
                .map(|p| [p[0], p[1], p[2] - crate::stair::RAIL_HEIGHT])
                .collect()
        }
        Some(types::RAMP) => {
            let layout = crate::ramp::Placed::of(snapshot, &object)?.layout.ok()?;
            layout.side_path(left)
        }
        Some(types::FLOOR) => along_chain(
            &crate::floor_edge::boundary_chain(snapshot, &object, edge)?,
            base,
        ),
        Some(types::FLOOR_EDGE) => {
            let derived = crate::floor_edge::Derived::of(snapshot, &object)?;
            along_chain(derived.chains.get(edge.unwrap_or(0))?, base)
        }
        _ => return None,
    };
    Some(path.into_iter().map(|p| (p, true)).collect())
}

impl Derived {
    /// Reads the railing `object` of `snapshot`.
    ///
    /// # Errors
    /// [`Refusal`].
    pub fn of(snapshot: &Snapshot, object: &Object) -> std::result::Result<Self, Refusal> {
        Self::build(snapshot, object, true)
    }

    fn build(
        snapshot: &Snapshot,
        object: &Object,
        follow: bool,
    ) -> std::result::Result<Self, Refusal> {
        let railing = Railing::from_values(&object.values).ok_or(Refusal::Malformed)?;
        let base = object
            .header
            .owner
            .and_then(|s| spatial::get(snapshot, s))
            .map_or(0.0, |s| spatial::storey_elevation(snapshot, &s));
        let surface: Vec<Node> = match railing.host {
            Some(host) => hosted(snapshot, host, railing.side, railing.host_edge, base)
                .ok_or(Refusal::Malformed)?,
            None => {
                if railing.path.len() < 2 {
                    return Err(Refusal::NoPath);
                }
                if railing
                    .path
                    .iter()
                    .any(|v| !v.point.iter().chain([&v.bulge]).all(|c| c.is_finite()))
                    || railing
                        .path
                        .windows(2)
                        .any(|w| distance(w[0].point, w[1].point) < 1.0e-3)
                {
                    return Err(Refusal::Malformed);
                }
                flatten(&railing.path)
            }
        };
        if surface.len() < 2 || railing.continues_to == Some(object.id) {
            return Err(Refusal::Malformed);
        }
        let kind = railing
            .railing_type
            .and_then(|t| spatial::get(snapshot, t))
            .and_then(|t| RailingType::from_values(&t.values))
            .unwrap_or_default();
        let next = if follow {
            railing
                .continues_to
                .and_then(|t| spatial::get(snapshot, t))
                .and_then(|t| Self::build(snapshot, &t, false).ok())
                .and_then(|d| d.rail.first().map(|p| [p[0], p[1], p[2] + d.base - base]))
        } else {
            None
        };
        let line = |height: f64| -> Vec<[f64; 3]> {
            let raised: Vec<Node> = surface
                .iter()
                .map(|(p, n)| ([p[0], p[1], p[2] + height], *n))
                .collect();
            let rounded = if kind.corner == Corner::Fillet {
                fillet(&raised, kind.fillet_radius)
            } else {
                raised
            };
            let mut points: Vec<[f64; 3]> = rounded.into_iter().map(|(p, _)| p).collect();
            extend(&mut points, &railing, &surface, next.is_some());
            if let Some(n) = next {
                points.push([n[0], n[1], n[2] - kind.height + height]);
            }
            points
        };
        let rail = line(kind.height);
        let handrail = if kind.handrail_height > 0.0 {
            line(kind.handrail_height)
        } else {
            Vec::new()
        };
        Ok(Self {
            id: object.id,
            railing,
            kind,
            base,
            surface,
            rail,
            handrail,
        })
    }

    /// The length of the path along the walking surface, in meters, arcs and slopes included.
    #[must_use]
    pub fn length(&self) -> f64 {
        self.surface
            .windows(2)
            .map(|w| distance(w[0].0, w[1].0))
            .sum()
    }

    /// The numbers of the surface points that carry a post: both ends and every corner.
    fn post_points(&self) -> Vec<usize> {
        let last = self.surface.len().saturating_sub(1);
        self.surface
            .iter()
            .enumerate()
            .filter(|(k, (p, node))| {
                *node
                    && (*k == 0
                        || *k == last
                        || self.surface.get(k + 1).is_some_and(|n| {
                            k.checked_sub(1)
                                .and_then(|j| self.surface.get(j))
                                .is_some_and(|b| turn(b.0, *p, n.0) >= STRAIGHT)
                        }))
            })
            .map(|(k, _)| k)
            .collect()
    }

    /// The posts: one at each end and at every corner of the path, each as its foot and its top
    /// (heights above the level).
    #[must_use]
    pub fn posts(&self) -> Vec<Post> {
        let top = self.kind.height - self.kind.rail_depth / 2.0;
        self.post_points()
            .into_iter()
            .filter_map(|k| self.surface.get(k))
            .map(|(p, _)| (*p, [p[0], p[1], p[2] + top]))
            .collect()
    }

    /// The balusters: between two posts as many as keep the distance along the path within the
    /// type's spacing, evenly spread. Returns the feet and the tops, and the largest actual
    /// distance between the axes of two neighbors (a baluster and a post included).
    #[must_use]
    pub fn balusters(&self) -> (Vec<Post>, f64) {
        let top = self.kind.height - self.kind.rail_depth / 2.0;
        let mut cumulative = vec![0.0];
        for w in self.surface.windows(2) {
            cumulative.push(cumulative.last().copied().unwrap_or(0.0) + distance(w[0].0, w[1].0));
        }
        let at = |s: f64| -> [f64; 3] {
            let k = cumulative
                .partition_point(|c| *c <= s)
                .clamp(1, self.surface.len() - 1);
            let (a, b) = (self.surface[k - 1].0, self.surface[k].0);
            let span = cumulative[k] - cumulative[k - 1];
            lerp(
                a,
                b,
                if span > 0.0 {
                    (s - cumulative[k - 1]) / span
                } else {
                    0.0
                },
            )
        };
        // The posts once, by point number: a long path has many points (arcs are flattened).
        let mut stops: Vec<f64> = self
            .post_points()
            .into_iter()
            .filter_map(|k| cumulative.get(k).copied())
            .collect();
        stops.dedup_by(|a, b| (*a - *b).abs() < 1.0e-9);
        let mut out = Vec::new();
        let mut widest = 0.0_f64;
        for pair in stops.windows(2) {
            let span = pair[1] - pair[0];
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a small count, clamped"
            )]
            let parts = ((span / self.kind.baluster_spacing).ceil() as usize).clamp(1, 10_000);
            widest = widest.max(span / parts as f64);
            for i in 1..parts {
                let foot = at(pair[0] + span * i as f64 / parts as f64);
                out.push((foot, [foot[0], foot[1], foot[2] + top]));
            }
        }
        (out, widest)
    }

    /// The widest clear opening between two neighbors (balusters and posts), in meters.
    #[must_use]
    pub fn widest_gap(&self) -> f64 {
        let (_, widest) = self.balusters();
        (widest - self.kind.baluster_size.max(self.kind.post_size)).max(0.0)
    }

    /// The plan line of the rail: the rail with its extensions, as points `[x, y]`.
    #[must_use]
    pub fn plan_line(&self) -> Vec<[f64; 2]> {
        self.rail.iter().map(|p| [p[0], p[1]]).collect()
    }

    /// The solids: the bars of the top rail and the handrail, the posts and the balusters, at
    /// their elevation.
    ///
    /// # Errors
    /// The solid kernel's errors.
    pub fn solids(&self) -> Result<Vec<Polyhedron>> {
        let lift = |p: [f64; 3]| [p[0], p[1], p[2] + self.base];
        let k = &self.kind;
        let rail = section(k.rail_profile, k.rail_width, k.rail_depth);
        let mut out = Vec::new();
        for line in [&self.rail, &self.handrail] {
            let points: Vec<[f64; 3]> = line.iter().map(|p| lift(*p)).collect();
            out.extend(rail_bars(&points, &rail, k.rail_width)?);
        }
        let post = section(Profile::Square, k.post_size, k.post_size);
        for (foot, top) in self.posts() {
            out.push(bar(lift(foot), lift(top), &post)?);
        }
        let baluster = section(k.baluster_profile, k.baluster_size, k.baluster_size);
        for (foot, top) in self.balusters().0 {
            out.push(bar(lift(foot), lift(top), &baluster)?);
        }
        Ok(out)
    }
}

/// Adds the extensions to the ends of the rail `points`: level for the extension length, then a
/// return to the wall (toward the railing's side, [`RETURN_LENGTH`]) or down to the floor. The end
/// extension is left out when the rail continues to another railing.
fn extend(points: &mut Vec<[f64; 3]>, railing: &Railing, surface: &[Node], joined: bool) {
    let n = points.len();
    if n < 2 {
        return;
    }
    let forward = |from: [f64; 3], to: [f64; 3]| -> Option<[f64; 2]> {
        let (d, _) = direction(from, to)?;
        let h = d[0].hypot(d[1]);
        (h > 1.0e-6).then(|| [d[0] / h, d[1] / h])
    };
    let tail = |extension: Extension, end: [f64; 3], t: [f64; 2], floor: f64| -> Vec<[f64; 3]> {
        match extension {
            Extension::None | Extension::Horizontal => vec![end],
            Extension::ReturnWall => {
                let s = if railing.side == Side::Left {
                    1.0
                } else {
                    -1.0
                };
                let w = [-t[1] * s * RETURN_LENGTH, t[0] * s * RETURN_LENGTH];
                vec![end, [end[0] + w[0], end[1] + w[1], end[2]]]
            }
            Extension::ReturnFloor => vec![end, [end[0], end[1], floor]],
        }
    };
    let len = railing.extension;
    if railing.end != Extension::None
        && !joined
        && let Some(t) = forward(points[n - 2], points[n - 1])
    {
        let last = points[n - 1];
        let end = [last[0] + t[0] * len, last[1] + t[1] * len, last[2]];
        let floor = surface.last().map_or(0.0, |s| s.0[2]);
        points.extend(tail(railing.end, end, t, floor));
    }
    if railing.start != Extension::None
        && let Some(t) = forward(points[0], points[1])
    {
        let first = points[0];
        let start = [first[0] - t[0] * len, first[1] - t[1] * len, first[2]];
        let floor = surface.first().map_or(0.0, |s| s.0[2]);
        let mut head = tail(railing.start, start, t, floor);
        head.reverse();
        head.append(points);
        *points = head;
    }
}

/// Whether `object` is a railing type record.
#[must_use]
pub fn is_type(snapshot: &Snapshot, id: ObjectId) -> bool {
    spatial::type_of(snapshot, id).as_deref() == Some(RAILING_TYPE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arc_base::id::Identifier;

    fn derived(points: &[[f64; 3]], kind: RailingType) -> Derived {
        let surface: Vec<Node> = points.iter().map(|p| (*p, true)).collect();
        Derived {
            id: ObjectId::from_u128(1),
            railing: Railing {
                path: Vec::new(),
                railing_type: None,
                host: None,
                side: Side::Left,
                start: Extension::None,
                end: Extension::None,
                extension: 0.3,
                continues_to: None,
                host_edge: None,
            },
            rail: surface
                .iter()
                .map(|(p, _)| [p[0], p[1], p[2] + kind.height])
                .collect(),
            handrail: Vec::new(),
            kind,
            base: 0.0,
            surface,
        }
    }

    #[test]
    fn balusters_of_a_path_with_many_points_take_linear_time() {
        // The flattened arcs of a long path: 500 points, each arc up to 256 pieces. The stops
        // used to look up every point among all posts, recomputed per point: hours, not
        // milliseconds.
        let points: Vec<[f64; 3]> = (0..100_000)
            .map(|k| [f64::from(k) * 0.01, f64::from(k % 2) * 0.01, 0.0])
            .collect();
        let d = derived(&points, RailingType::default());
        let started = std::time::Instant::now();
        let (balusters, _) = d.balusters();
        assert!(started.elapsed().as_secs() < 20, "{:?}", started.elapsed());
        assert_eq!(d.posts().len(), 100_000);
        assert!(balusters.is_empty());
    }

    #[test]
    fn balusters_are_spaced_evenly_along_the_path_within_the_type_spacing() {
        let d = derived(&[[0.0, 0.0, 0.0], [3.0, 0.0, 0.0]], RailingType::default());
        assert_eq!(d.posts().len(), 2);
        let (balusters, widest) = d.balusters();
        assert_eq!(balusters.len(), 24);
        assert!((widest - 0.12).abs() < 1.0e-9);
        assert!(d.widest_gap() < CLEAR_GAP_LIMIT);
        let wide = RailingType {
            baluster_spacing: 0.4,
            ..RailingType::default()
        };
        assert!(derived(&[[0.0, 0.0, 0.0], [3.0, 0.0, 0.0]], wide).widest_gap() > CLEAR_GAP_LIMIT);
    }

    #[test]
    fn spacing_is_measured_along_a_slope_and_corners_get_posts() {
        // 4 m along x and 3 m up: 5 m of path, then a turn.
        let d = derived(
            &[[0.0, 0.0, 0.0], [4.0, 0.0, 3.0], [4.0, 2.0, 3.0]],
            RailingType::default(),
        );
        assert_eq!(d.posts().len(), 3);
        assert!((d.length() - 7.0).abs() < 1.0e-9);
        let (balusters, _) = d.balusters();
        // 5 m / 0.12 -> 42 parts (41 balusters) and 2 m / 0.12 -> 17 parts (16).
        assert_eq!(balusters.len(), 41 + 16);
        assert!(d.solids().is_ok_and(|s| s.len() > 60));
    }

    #[test]
    fn an_arc_is_flattened_to_its_length_and_a_fillet_rounds_the_corner() {
        let quarter = (std::f64::consts::PI / 8.0).tan();
        let a = [0.0, 0.0, 0.0];
        let b = [1.0, 1.0, 0.0];
        let mut points = vec![(a, true)];
        points.extend(edge(a, b, quarter));
        let length: f64 = points.windows(2).map(|w| distance(w[0].0, w[1].0)).sum();
        // The radius of the arc through a and b with a quarter sweep is 1: length pi / 2.
        assert!(
            (length - std::f64::consts::FRAC_PI_2).abs() < 0.01,
            "{length}"
        );
        assert!(points.iter().filter(|(_, n)| *n).count() == 2);
        let corner = fillet(
            &[
                ([0.0, 0.0, 0.0], true),
                ([1.0, 0.0, 0.0], true),
                ([1.0, 1.0, 0.0], true),
            ],
            0.2,
        );
        assert!(corner.len() > 3);
        assert!(
            corner
                .iter()
                .all(|(p, _)| p[0] <= 1.0 + 1.0e-9 && p[1] >= -1.0e-9)
        );
    }

    #[test]
    fn extensions_continue_level_and_return() {
        let mut r = derived(&[[0.0, 0.0, 0.0], [2.0, 0.0, 1.0]], RailingType::default()).railing;
        let mut points = vec![[0.0, 0.0, 0.9], [2.0, 0.0, 1.9]];
        let surface: Vec<Node> = vec![([0.0, 0.0, 0.0], true), ([2.0, 0.0, 1.0], true)];
        r.start = Extension::Horizontal;
        r.end = Extension::ReturnWall;
        extend(&mut points, &r, &surface, false);
        assert_eq!(points.first(), Some(&[-0.3, 0.0, 0.9]));
        assert_eq!(points[points.len() - 2], [2.3, 0.0, 1.9]);
        // Left side: the return turns to +y by 0.1 m at the same height.
        assert_eq!(points[points.len() - 1], [2.3, RETURN_LENGTH, 1.9]);
        r.end = Extension::ReturnFloor;
        let mut p2 = vec![[0.0, 0.0, 0.9], [2.0, 0.0, 1.9]];
        extend(&mut p2, &r, &surface, false);
        assert_eq!(p2[p2.len() - 1], [2.3, 0.0, 1.0]);
        // A rail that continues to another one gets no end extension.
        let mut p3 = vec![[0.0, 0.0, 0.9], [2.0, 0.0, 1.9]];
        extend(&mut p3, &r, &surface, true);
        assert_eq!(p3.last(), Some(&[2.0, 0.0, 1.9]));
    }

    #[test]
    fn the_record_round_trips() {
        let r = Railing {
            path: vec![
                Vertex {
                    point: [0.0, 0.0, 0.0],
                    bulge: 0.4,
                },
                Vertex {
                    point: [1.0, 0.0, 0.5],
                    bulge: 0.0,
                },
            ],
            railing_type: None,
            host: None,
            side: Side::Right,
            start: Extension::ReturnWall,
            end: Extension::Horizontal,
            extension: 0.4,
            continues_to: None,
            host_edge: Some(3),
        };
        assert_eq!(Railing::from_values(&r.values("R")), Some(r.clone()));
        let t = RailingType {
            handrail_height: 0.7,
            corner: Corner::Fillet,
            ..RailingType::default()
        };
        assert_eq!(RailingType::from_values(&t.values("T")), Some(t));
    }

    #[test]
    fn field_positions_match_the_declarations() {
        let mut registry = arc_doc::TypeRegistry::new();
        types::register(&mut registry).unwrap();
        let names = |key: &str| -> Vec<String> {
            registry
                .get(key)
                .unwrap()
                .schema
                .fields
                .iter()
                .map(|f| f.name.to_string())
                .collect()
        };
        let at = |key: &str, n: usize| names(key)[n].clone();
        assert_eq!(at(types::RAILING, railing::PATH), "path");
        assert_eq!(at(types::RAILING, railing::CONTINUES_TO), "continues_to");
        assert_eq!(at(types::RAILING, railing::PINNED), "pinned");
        assert_eq!(at(types::RAILING, railing::HOST_EDGE), "host_edge");
        assert_eq!(
            at(types::RAILING_TYPE, railing_type::FILLET_RADIUS),
            "fillet_radius"
        );
        assert_eq!(at(types::RAMP, crate::types::ramp::RISE), "rise");
        assert_eq!(at(types::RAMP, crate::types::ramp::PINNED), "pinned");
        assert_eq!(
            at(types::RAMP_TYPE, crate::types::ramp_type::THICKNESS),
            "thickness"
        );
    }
}
