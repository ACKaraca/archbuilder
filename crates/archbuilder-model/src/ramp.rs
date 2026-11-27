//! The ramp: authored data read from a record, and everything else derived from it.
//!
//! A ramp record holds the foot (the start of the run), the direction up the ramp, the width, the
//! horizontal length of the sloped run, a flat landing at each end, the thickness, the form (a
//! slab of constant thickness or a solid down to the floor construction), optionally a rise, and
//! the level it arrives at. Without a rise the ramp rises from its storey's level to the level
//! above (or the named one), and the floors of that level get a shaft opening over the run. The
//! slope is the rise over the length; the type's largest slope is checked ([`Layout::exceeds`]), a
//! warning and not a refusal.
//!
//! More runs follow the first ([`Run`]): each after the landing that ends the one before, turned
//! to the left (positive) or right by the turn at the middle of that landing, with its own
//! length, landing and, optionally, its own slope. A run without a slope of its own takes the
//! slope that would carry the rise over all the runs, so setting the slope of one run changes the
//! elevations of every run after it and nothing before it; the top then differs from the level
//! it arrives at, which is a warning ([`Layout::top_off`]).
//!
//! A ramp by sketch has a boundary of straight edges and a run line (the foot, the direction, the
//! lengths of the landings and of the slope): the slope rises along the run line over the whole
//! boundary, between the two landings.
//!
//! Coordinates: `a` runs up the ramp from the foot, `c` across it to the left, `z` up from the
//! storey's level.

use arc_base::{ObjectId, Result, Tolerance};
use arc_doc::{Object, Snapshot, Value};
use arc_geom::{Curve2, FaceTag, Loop2, Polyhedron, Region, RegionPart, Segment2};

use crate::spatial;
use crate::terrain::{self, Bottom, Piece, Plane};
use crate::types::{self, level, ramp, ramp_type};

/// The default width, in meters.
pub const DEFAULT_WIDTH: f64 = 1.2;
/// The default length of the sloped run, in meters.
pub const DEFAULT_LENGTH: f64 = 6.0;
/// The default thickness, in meters.
pub const DEFAULT_THICKNESS: f64 = 0.2;
/// The default largest slope: 1 in 12.
pub const DEFAULT_MAX_SLOPE: f64 = 1.0 / 12.0;
/// The range of a width, in meters.
pub const WIDTH_RANGE: (f64, f64) = (0.5, 20.0);
/// The range of the length of the run, in meters.
pub const LENGTH_RANGE: (f64, f64) = (0.1, 500.0);
/// The range of a landing's length, in meters.
pub const LANDING_RANGE: (f64, f64) = (0.0, 50.0);
/// The range of the thickness, in meters.
pub const THICKNESS_RANGE: (f64, f64) = (0.02, 3.0);
/// The range of a rise, in meters.
pub const RISE_LIMITS: (f64, f64) = (0.01, 100.0);
/// The range of the largest slope of a type (rise over run).
pub const SLOPE_RANGE: (f64, f64) = (0.01, 1.0);
/// The clearance the shaft opening keeps around the run, except at the arrival edge, in meters.
pub const OPENING_CLEARANCE: f64 = 0.05;
/// How far a hosted railing stands in from the edge, in meters.
pub const RAIL_INSET: f64 = 0.05;
/// The most runs after the first one.
pub const MAX_RUNS: usize = 16;
/// The most points of the boundary of a ramp by sketch.
pub const MAX_BOUNDARY: usize = 200;
/// The widest turn between two runs, in radians (135 degrees): a miter wider than that is longer
/// than the landing it turns in.
pub const TURN_LIMIT: f64 = 3.0 * std::f64::consts::FRAC_PI_4;
/// How far the top of a ramp that arrives at a level may be off it before it is a warning, in
/// meters.
pub const TOP_TOLERANCE: f64 = 0.001;

coded! {
    /// The form of the ramp's body.
    Form {
        /// A slab of constant vertical thickness.
        #[default]
        Slab = 0, "slab";
        /// A solid down to the depth of the floor construction below the level.
        Solid = 1, "solid";
    }
}

/// A run after the first one, with the landing that ends it. The turn is the turn of the heading
/// at the middle of the landing that comes before it (positive to the left); the slope, when
/// given, is this run's own (rise over run).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Run {
    /// The turn at the landing before the run, in radians.
    pub turn: f64,
    /// The horizontal length of the sloped part.
    pub length: f64,
    /// The flat landing after the run: the next turn's landing, or the top landing of the last
    /// run.
    pub landing: f64,
    /// The slope of this run; the shared slope without it.
    pub slope: Option<f64>,
}

/// The authored data of a ramp, lengths in meters.
#[derive(Debug, Clone, PartialEq)]
pub struct Ramp {
    /// The foot of the run.
    pub start: [f64; 2],
    /// The direction up the ramp, in radians from the x axis.
    pub direction: f64,
    /// The width.
    pub width: f64,
    /// The horizontal length of the sloped run (the first one).
    pub length: f64,
    /// The flat landing at the foot.
    pub landing_start: f64,
    /// The flat landing after the first run: the top landing of a ramp of one run.
    pub landing_end: f64,
    /// The thickness.
    pub thickness: f64,
    /// The form.
    pub form: Form,
    /// The rise, when it is not the height to the level above.
    pub rise: Option<f64>,
    /// The level the ramp arrives at; the level above its storey when `None`.
    pub top_level: Option<ObjectId>,
    /// The ramp type.
    pub ramp_type: Option<ObjectId>,
    /// The slope of the first run, when it is not the shared one.
    pub slope: Option<f64>,
    /// The runs after the first.
    pub runs: Vec<Run>,
    /// The boundary of a ramp by sketch; empty for a ramp of runs.
    pub boundary: Vec<[f64; 2]>,
}

fn quantity(fields: &[Option<Value>], at: usize) -> Option<f64> {
    match fields.get(at) {
        Some(Some(Value::Quantity(q))) => Some(q.value()),
        _ => None,
    }
}

fn real(values: &[Option<Value>], at: usize) -> Option<f64> {
    match values.get(at) {
        Some(Some(Value::F64(v))) => Some(*v),
        _ => None,
    }
}

impl Ramp {
    /// The ramp of a record's field values; `None` when a field is missing or malformed.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Option<Self> {
        let m = |at| types::meters(values, at);
        let direction = quantity(values, ramp::DIRECTION)?;
        let reference = |at: usize| match values.get(at) {
            Some(Some(Value::Reference(id))) => Some(*id),
            _ => None,
        };
        let mut runs = Vec::new();
        if let Some(Some(Value::List(items))) = values.get(ramp::RUNS) {
            for item in items.iter().take(MAX_RUNS + 1) {
                let Value::Record(f) = item else {
                    return None;
                };
                runs.push(Run {
                    turn: quantity(f, ramp::run::TURN)?,
                    length: quantity(f, ramp::run::LENGTH)?,
                    landing: quantity(f, ramp::run::LANDING)?,
                    slope: real(f, ramp::run::SLOPE),
                });
            }
        }
        let mut boundary = Vec::new();
        if let Some(Some(Value::List(items))) = values.get(ramp::BOUNDARY) {
            for item in items.iter().take(MAX_BOUNDARY + 1) {
                let Value::Record(f) = item else {
                    return None;
                };
                boundary.push([quantity(f, 0)?, quantity(f, 1)?]);
            }
        }
        Some(Self {
            start: [m(ramp::X)?, m(ramp::Y)?],
            direction,
            width: m(ramp::WIDTH)?,
            length: m(ramp::LENGTH)?,
            landing_start: m(ramp::LANDING_START)?,
            landing_end: m(ramp::LANDING_END)?,
            thickness: m(ramp::THICKNESS)?,
            form: match values.get(ramp::FORM) {
                Some(Some(Value::Enum(c))) => Form::from_code(*c),
                _ => Form::Slab,
            },
            rise: m(ramp::RISE),
            top_level: reference(ramp::TOP_LEVEL),
            ramp_type: reference(ramp::RAMP_TYPE),
            slope: real(values, ramp::SLOPE),
            runs,
            boundary,
        })
    }

    /// The value of the list of runs, for the record.
    #[must_use]
    pub fn runs_value(runs: &[Run]) -> Value {
        Value::List(
            runs.iter()
                .map(|r| {
                    Value::Record(vec![
                        Some(crate::stair::angle(r.turn)),
                        Some(types::length(r.length)),
                        Some(types::length(r.landing)),
                        r.slope.map(Value::F64),
                    ])
                })
                .collect(),
        )
    }

    /// The value of the boundary, for the record.
    #[must_use]
    pub fn boundary_value(boundary: &[[f64; 2]]) -> Value {
        Value::List(
            boundary
                .iter()
                .map(|p| Value::Record(vec![Some(types::length(p[0])), Some(types::length(p[1]))]))
                .collect(),
        )
    }

    /// The field values of a record of this ramp named `name`.
    #[must_use]
    pub fn values(&self, name: &str) -> Vec<Option<Value>> {
        let mut values: Vec<Option<Value>> = vec![None; ramp::BOUNDARY + 1];
        values[ramp::NAME] = Some(Value::String(name.to_owned()));
        values[ramp::X] = Some(types::length(self.start[0]));
        values[ramp::Y] = Some(types::length(self.start[1]));
        values[ramp::DIRECTION] = Some(crate::stair::angle(self.direction));
        values[ramp::WIDTH] = Some(types::length(self.width));
        values[ramp::LENGTH] = Some(types::length(self.length));
        values[ramp::LANDING_START] = Some(types::length(self.landing_start));
        values[ramp::LANDING_END] = Some(types::length(self.landing_end));
        values[ramp::THICKNESS] = Some(types::length(self.thickness));
        values[ramp::FORM] = Some(Value::Enum(self.form.code()));
        values[ramp::RISE] = self.rise.map(types::length);
        values[ramp::TOP_LEVEL] = self.top_level.map(Value::Reference);
        values[ramp::RAMP_TYPE] = self.ramp_type.map(Value::Reference);
        values[ramp::SLOPE] = self.slope.map(Value::F64);
        values[ramp::RUNS] = (!self.runs.is_empty()).then(|| Self::runs_value(&self.runs));
        values[ramp::BOUNDARY] =
            (!self.boundary.is_empty()).then(|| Self::boundary_value(&self.boundary));
        values
    }

    /// The ramp under the plane motion `xf`: the foot moved and the direction turned, the boundary
    /// moved, the turns of the runs reversed under a reflection.
    #[must_use]
    pub fn moved(&self, xf: &crate::transform::Xform) -> Self {
        let (s, c) = crate::transform::sin_cos(self.direction);
        let start = xf.point(self.start);
        let ahead = xf.point([self.start[0] + c, self.start[1] + s]);
        let flip = xf.reflects();
        Self {
            start,
            direction: (ahead[1] - start[1]).atan2(ahead[0] - start[0]),
            runs: self
                .runs
                .iter()
                .map(|r| Run {
                    turn: if flip { -r.turn } else { r.turn },
                    ..*r
                })
                .collect(),
            boundary: self.boundary.iter().map(|p| xf.point(*p)).collect(),
            ..self.clone()
        }
    }

    /// The ramp with its foot (`end` false) or the end of its first sloped run (`end` true) moved
    /// to `to`. The end keeps the foot and the landing in front of the slope and turns and
    /// lengthens the run; the foot keeps the end of the run where it is, so the direction and the
    /// length change. `None` when the run would not be as long as [`LENGTH_RANGE`] allows.
    #[must_use]
    pub fn with_end_moved(&self, end: bool, to: [f64; 2]) -> Option<Self> {
        if !to.iter().all(|v| v.is_finite()) {
            return None;
        }
        let (s, c) = crate::transform::sin_cos(self.direction);
        let run_start = [
            self.start[0] + c * self.landing_start,
            self.start[1] + s * self.landing_start,
        ];
        let run_end = [
            run_start[0] + c * self.length,
            run_start[1] + s * self.length,
        ];
        let fits = |length: f64| (LENGTH_RANGE.0..=LENGTH_RANGE.1).contains(&length);
        if end {
            let d = [to[0] - run_start[0], to[1] - run_start[1]];
            let length = d[0].hypot(d[1]);
            return fits(length).then(|| Self {
                direction: d[1].atan2(d[0]),
                length,
                ..self.clone()
            });
        }
        let d = [run_end[0] - to[0], run_end[1] - to[1]];
        let length = d[0].hypot(d[1]) - self.landing_start;
        fits(length).then(|| Self {
            start: to,
            direction: d[1].atan2(d[0]),
            length,
            ..self.clone()
        })
    }

    /// The horizontal lengths of the runs, the first one included.
    fn run_lengths(&self) -> Vec<f64> {
        std::iter::once(self.length)
            .chain(self.runs.iter().map(|r| r.length))
            .collect()
    }
}

/// Why a ramp has no layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// No level above the base and no rise: the ramp has no height.
    NoTop,
    /// A length is not finite or not positive, a turn does not fit its landing, or the sketch is
    /// not a simple polygon.
    Malformed,
}

/// A point of the center line of the path of runs and landings, with its elevation above the
/// base.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Node {
    p: [f64; 2],
    z: f64,
}

/// The center line being laid.
struct Path {
    nodes: Vec<Node>,
    stretches: Vec<Option<usize>>,
}

impl Path {
    /// Adds a stretch of `length` along `heading` that rises by `rise`; none for no length.
    fn push(&mut self, length: f64, rise: f64, heading: f64, run: Option<usize>) {
        if length <= 0.0 {
            return;
        }
        let Some(last) = self.nodes.last().copied() else {
            return;
        };
        let (s, c) = crate::transform::sin_cos(heading);
        self.nodes.push(Node {
            p: [last.p[0] + c * length, last.p[1] + s * length],
            z: last.z + rise,
        });
        self.stretches.push(run);
    }
}

/// What a ramp derives from its data and its height.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// The authored data.
    pub ramp: Ramp,
    /// The rise: the elevation of the top above the base.
    pub height: f64,
    /// The largest slope allowed.
    pub max_slope: f64,
    /// Whether the ramp arrives at the level above (and so cuts the floors of that level).
    pub arrives: bool,
    /// The rise that carries the ramp to its level (or the rise it was given).
    nominal: f64,
    /// The slope of each run.
    slopes: Vec<f64>,
    /// The center line: the foot, then the end of every landing and every run (a landing with a
    /// turn has a point at its middle); empty for a ramp by sketch.
    nodes: Vec<Node>,
    /// For each stretch between two nodes: the run it belongs to, `None` for a landing.
    stretches: Vec<Option<usize>>,
    /// The number of the node where the last run ends.
    arrival: usize,
}

type P2 = [f64; 2];

fn plan_region(points: &[P2]) -> Result<Region> {
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

fn unit(a: P2, b: P2) -> P2 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l = dx.hypot(dy).max(f64::MIN_POSITIVE);
    [dx / l, dy / l]
}

/// The left and the right corners of a band of half the width `half` along the points `pts`,
/// mitered at the inner points (their turn is at most [`TURN_LIMIT`]); square at the ends.
fn ribbon(pts: &[P2], half: f64) -> (Vec<P2>, Vec<P2>) {
    let normals: Vec<P2> = pts
        .windows(2)
        .map(|w| {
            let d = unit(w[0], w[1]);
            [-d[1], d[0]]
        })
        .collect();
    let mut left = Vec::with_capacity(pts.len());
    let mut right = Vec::with_capacity(pts.len());
    for (k, p) in pts.iter().enumerate() {
        let n = match (
            k.checked_sub(1).and_then(|j| normals.get(j)),
            normals.get(k),
        ) {
            (Some(a), Some(b)) => {
                let c = 1.0 + a[0] * b[0] + a[1] * b[1];
                [(a[0] + b[0]) / c.max(0.1), (a[1] + b[1]) / c.max(0.1)]
            }
            (Some(a), None) | (None, Some(a)) => *a,
            (None, None) => [0.0, 0.0],
        };
        left.push([p[0] + n[0] * half, p[1] + n[1] * half]);
        right.push([p[0] - n[0] * half, p[1] - n[1] * half]);
    }
    (left, right)
}

/// The polygon of a ribbon (counterclockwise): the right side forward, the left side back.
fn outline_of(left: &[P2], right: &[P2]) -> Vec<P2> {
    simplify(
        right
            .iter()
            .chain(left.iter().rev())
            .copied()
            .collect::<Vec<_>>(),
        true,
    )
}

/// `points` without the ones that lie on the line between their neighbors; a closed polygon is
/// cut at no point, an open line keeps its ends.
fn simplify(points: Vec<P2>, closed: bool) -> Vec<P2> {
    let n = points.len();
    if n < 3 {
        return points;
    }
    let straight = |a: P2, b: P2, c: P2| {
        let (u, v) = ([b[0] - a[0], b[1] - a[1]], [c[0] - b[0], c[1] - b[1]]);
        let scale = u[0].hypot(u[1]) * v[0].hypot(v[1]);
        (u[0] * v[1] - u[1] * v[0]).abs() <= 1.0e-9 * scale.max(1.0e-12)
            && u[0] * v[0] + u[1] * v[1] > 0.0
    };
    (0..n)
        .filter(|k| {
            let at_end = !closed && (*k == 0 || *k + 1 == n);
            at_end || !straight(points[(k + n - 1) % n], points[*k], points[(k + 1) % n])
        })
        .map(|k| points[k])
        .collect()
}

/// The centroid of a polygon.
fn centroid(points: &[P2]) -> P2 {
    let n = points.len();
    let (mut a, mut x, mut y) = (0.0, 0.0, 0.0);
    for k in 0..n {
        let (p, q) = (points[k], points[(k + 1) % n]);
        let w = p[0] * q[1] - q[0] * p[1];
        a += w;
        x += (p[0] + q[0]) * w;
        y += (p[1] + q[1]) * w;
    }
    if a.abs() < 1.0e-12 {
        return points.first().copied().unwrap_or([0.0, 0.0]);
    }
    [x / (3.0 * a), y / (3.0 * a)]
}

/// The part of the polygon `points` on the side of the line through `at` with the normal `n`
/// where `(p - at) . n <= 0`.
fn clip_half(points: &[P2], at: P2, n: P2) -> Vec<P2> {
    let side = |p: P2| (p[0] - at[0]) * n[0] + (p[1] - at[1]) * n[1];
    let m = points.len();
    let mut out = Vec::with_capacity(m + 2);
    for k in 0..m {
        let (p, q) = (points[k], points[(k + 1) % m]);
        let (sp, sq) = (side(p), side(q));
        if sp <= 0.0 {
            out.push(p);
        }
        if (sp <= 0.0) != (sq <= 0.0) {
            let t = sp / (sp - sq);
            out.push([p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]);
        }
    }
    out
}

impl Layout {
    /// The layout of `ramp` for the level height `level_height` (used without a rise) and the
    /// type's largest slope.
    ///
    /// # Errors
    /// [`Refusal`].
    #[allow(
        clippy::too_many_lines,
        reason = "the checks, then the path, in one pass"
    )]
    pub fn new(
        ramp: &Ramp,
        level_height: Option<f64>,
        max_slope: f64,
    ) -> std::result::Result<Self, Refusal> {
        let finite = [
            ramp.width,
            ramp.length,
            ramp.landing_start,
            ramp.landing_end,
            ramp.thickness,
            ramp.direction,
            ramp.start[0],
            ramp.start[1],
            max_slope,
        ]
        .iter()
        .chain(ramp.slope.iter())
        .chain(ramp.boundary.iter().flatten())
        .chain(ramp.runs.iter().flat_map(|r| {
            [&r.turn, &r.length, &r.landing]
                .into_iter()
                .chain(r.slope.iter())
        }))
        .all(|v| v.is_finite());
        let slope_ok = |s: &Option<f64>| s.is_none_or(|s| (0.0..=SLOPE_RANGE.1).contains(&s));
        if !finite
            || ramp.width <= 0.0
            || ramp.length <= 0.0
            || ramp.landing_start < 0.0
            || ramp.landing_end < 0.0
            || ramp.thickness <= 0.0
            || ramp.runs.len() > MAX_RUNS
            || ramp.boundary.len() > MAX_BOUNDARY
            || !slope_ok(&ramp.slope)
            || ramp.runs.iter().any(|r| {
                r.length <= 0.0
                    || r.landing < 0.0
                    || r.turn.abs() > TURN_LIMIT
                    || !slope_ok(&r.slope)
            })
        {
            return Err(Refusal::Malformed);
        }
        let sketch = !ramp.boundary.is_empty();
        if sketch
            && (ramp.boundary.len() < 3
                || !ramp.runs.is_empty()
                || plan_region(&ramp.boundary).is_err())
        {
            return Err(Refusal::Malformed);
        }
        let nominal = ramp.rise.or(level_height).unwrap_or(0.0);
        if !nominal.is_finite() || nominal < types::HEIGHT_RANGE.0 {
            return Err(Refusal::NoTop);
        }
        let lengths = ramp.run_lengths();
        let total: f64 = lengths.iter().sum();
        let own: Vec<Option<f64>> = std::iter::once(ramp.slope)
            .chain(ramp.runs.iter().map(|r| r.slope))
            .collect();
        let overridden = own.iter().any(Option::is_some);
        let shared = nominal / total;
        let slopes: Vec<f64> = own.iter().map(|s| s.unwrap_or(shared)).collect();
        let height = if overridden {
            slopes.iter().zip(&lengths).map(|(s, l)| s * l).sum()
        } else {
            nominal
        };
        let mut layout = Self {
            ramp: ramp.clone(),
            height,
            max_slope,
            arrives: ramp.rise.is_none(),
            nominal,
            slopes,
            nodes: Vec::new(),
            stretches: Vec::new(),
            arrival: 0,
        };
        if !sketch {
            layout.path()?;
        }
        Ok(layout)
    }

    /// Lays the center line: the landing at the foot, each run and the landing after it, turned at
    /// the middle of the landing before the next run.
    fn path(&mut self) -> std::result::Result<(), Refusal> {
        let ramp = &self.ramp;
        let mut heading = ramp.direction;
        let mut path = Path {
            nodes: vec![Node {
                p: ramp.start,
                z: 0.0,
            }],
            stretches: Vec::new(),
        };
        path.push(ramp.landing_start, 0.0, heading, None);
        let landings: Vec<f64> = std::iter::once(ramp.landing_end)
            .chain(ramp.runs.iter().map(|r| r.landing))
            .collect();
        let lengths = ramp.run_lengths();
        let total: f64 = lengths.iter().sum();
        let mut arrival = 0;
        for k in 0..lengths.len() {
            let rise = if self.slopes_overridden() {
                self.slopes[k] * lengths[k]
            } else {
                self.nominal * lengths[k] / total
            };
            path.push(lengths[k], rise, heading, Some(k));
            arrival = path.nodes.len() - 1;
            let landing = landings[k];
            let turn = ramp.runs.get(k).map_or(0.0, |r| r.turn);
            if turn != 0.0 {
                // The miter of the turn must fit in half the landing.
                if landing / 2.0 + 1.0e-9 < ramp.width / 2.0 * (turn.abs() / 2.0).tan() {
                    return Err(Refusal::Malformed);
                }
                path.push(landing / 2.0, 0.0, heading, None);
                heading += turn;
                path.push(landing / 2.0, 0.0, heading, None);
            } else {
                path.push(landing, 0.0, heading, None);
            }
        }
        let (nodes, stretches) = (path.nodes, path.stretches);
        self.nodes = nodes;
        self.stretches = stretches;
        self.arrival = arrival;
        Ok(())
    }

    fn slopes_overridden(&self) -> bool {
        self.ramp.slope.is_some() || self.ramp.runs.iter().any(|r| r.slope.is_some())
    }

    fn sketch(&self) -> bool {
        !self.ramp.boundary.is_empty()
    }

    /// The slope of the steepest run: the rise over the length.
    #[must_use]
    pub fn slope(&self) -> f64 {
        self.slopes.iter().copied().fold(0.0, f64::max)
    }

    /// The slope of each run.
    #[must_use]
    pub fn slopes(&self) -> &[f64] {
        &self.slopes
    }

    /// Whether the slope of any run passes the largest one of the type.
    #[must_use]
    pub fn exceeds(&self) -> bool {
        self.slopes.iter().any(|s| *s > self.max_slope + 1.0e-9)
    }

    /// How far the top is above the elevation the ramp was to reach (the level it arrives at, or
    /// its rise): not zero when a run has a slope of its own.
    #[must_use]
    pub fn top_off(&self) -> f64 {
        self.height - self.nominal
    }

    /// The elevations above the base at the start and at the end of each run: the elevation of
    /// every run after one whose slope changed moves with it, those before it do not.
    #[must_use]
    pub fn elevations(&self) -> Vec<(f64, f64)> {
        if self.sketch() {
            return vec![(0.0, self.height)];
        }
        (0..self.slopes.len())
            .map(|k| {
                let at = self.stretches.iter().position(|s| *s == Some(k));
                at.map_or((0.0, 0.0), |i| (self.nodes[i].z, self.nodes[i + 1].z))
            })
            .collect()
    }

    /// The length of the run and the landings, along the center line.
    #[must_use]
    pub fn total_length(&self) -> f64 {
        if self.sketch() {
            return self.ramp.landing_start + self.ramp.length + self.ramp.landing_end;
        }
        self.nodes
            .windows(2)
            .map(|w| (w[1].p[0] - w[0].p[0]).hypot(w[1].p[1] - w[0].p[1]))
            .sum()
    }

    fn at(&self, a: f64, c: f64) -> [f64; 2] {
        let (s, co) = crate::transform::sin_cos(self.ramp.direction);
        [
            self.ramp.start[0] + co * a - s * c,
            self.ramp.start[1] + s * a + co * c,
        ]
    }

    /// The position along the run line and the one across it, of a point.
    fn along(&self, p: P2) -> (f64, f64) {
        let (s, c) = crate::transform::sin_cos(self.ramp.direction);
        let (dx, dy) = (p[0] - self.ramp.start[0], p[1] - self.ramp.start[1]);
        (dx * c + dy * s, -dx * s + dy * c)
    }

    /// The elevation above the base at the position `a` along the run line of a ramp by sketch.
    fn elevation(&self, a: f64) -> f64 {
        let r = &self.ramp;
        let from = r.landing_start;
        (self.slopes[0] * (a - from).clamp(0.0, r.length)).min(self.height)
    }

    fn centers(&self) -> Vec<P2> {
        self.nodes.iter().map(|n| n.p).collect()
    }

    /// The plan footprint, counterclockwise: the band along the center line, mitered where the
    /// path turns; the boundary of a ramp by sketch.
    #[must_use]
    pub fn footprint(&self) -> Vec<[f64; 2]> {
        if self.sketch() {
            let mut b = self.ramp.boundary.clone();
            if crate::floor::signed_area(&b) < 0.0 {
                b.reverse();
            }
            return b;
        }
        let (left, right) = ribbon(&self.centers(), self.ramp.width / 2.0);
        outline_of(&left, &right)
    }

    /// The shaft opening the floors of the level it arrives at get: the runs and the landing at
    /// the foot with [`OPENING_CLEARANCE`] around them, up to the arrival edge. Empty for a ramp
    /// with a rise of its own. For a ramp by sketch: the boundary up to the arrival edge.
    #[must_use]
    pub fn opening(&self) -> Vec<[f64; 2]> {
        if !self.arrives {
            return Vec::new();
        }
        if self.sketch() {
            let (s, c) = crate::transform::sin_cos(self.ramp.direction);
            let at = self.at(self.ramp.landing_start + self.ramp.length, 0.0);
            let out = clip_half(&self.footprint(), at, [c, s]);
            return if out.len() >= 3 { out } else { Vec::new() };
        }
        let (h, k) = (self.ramp.width / 2.0 + OPENING_CLEARANCE, OPENING_CLEARANCE);
        let mut pts: Vec<P2> = self.centers().into_iter().take(self.arrival + 1).collect();
        if pts.len() < 2 {
            return Vec::new();
        }
        let d = unit(pts[0], pts[1]);
        pts[0] = [pts[0][0] - d[0] * k, pts[0][1] - d[1] * k];
        let (left, right) = ribbon(&pts, h);
        outline_of(&left, &right)
    }

    /// The convex pieces of the top surface, each with its plane (heights above the base).
    fn pieces(&self) -> Vec<Piece> {
        if self.sketch() {
            return self.sketch_pieces();
        }
        let pts = self.centers();
        let (left, right) = ribbon(&pts, self.ramp.width / 2.0);
        (0..pts.len().saturating_sub(1))
            .map(|i| {
                let (a, b) = (self.nodes[i], self.nodes[i + 1]);
                let d = unit(a.p, b.p);
                let length = (b.p[0] - a.p[0]).hypot(b.p[1] - a.p[1]);
                let rise = (b.z - a.z) / length.max(f64::MIN_POSITIVE);
                Piece {
                    points: vec![right[i], right[i + 1], left[i + 1], left[i]],
                    plane: Plane {
                        at: a.p,
                        z: a.z,
                        grad: [d[0] * rise, d[1] * rise],
                    },
                    upper: None,
                }
            })
            .collect()
    }

    /// The pieces of a ramp by sketch: the triangles of the boundary cut at the two ends of the
    /// slope, so that each piece has one plane.
    fn sketch_pieces(&self) -> Vec<Piece> {
        let Ok(region) = plan_region(&self.ramp.boundary) else {
            return Vec::new();
        };
        let Ok(mesh) = arc_geom::triangulate(&region, 0.01, std::f64::consts::PI, 4 * MAX_BOUNDARY)
        else {
            return Vec::new();
        };
        let r = &self.ramp;
        let (s, c) = crate::transform::sin_cos(r.direction);
        let reach = self
            .footprint()
            .iter()
            .map(|p| (p[0] - r.start[0]).hypot(p[1] - r.start[1]))
            .fold(0.0, f64::max)
            + 1.0;
        let (a0, a1) = (r.landing_start, r.landing_start + r.length);
        // The bands along the run line: before the slope, on it, after it.
        let bands = [
            (-reach, a0, Plane::level(0.0)),
            (
                a0,
                a1,
                Plane {
                    at: self.at(a0, 0.0),
                    z: 0.0,
                    grad: [self.slopes[0] * c, self.slopes[0] * s],
                },
            ),
            (a1, reach, Plane::level(self.height)),
        ];
        let mut out = Vec::new();
        for t in mesh.triangles() {
            let mut tri: Vec<P2> = t
                .iter()
                .filter_map(|i| {
                    mesh.vertices()
                        .get(usize::try_from(*i).ok()?)
                        .map(|p| [p.x, p.y])
                })
                .collect();
            if terrain::double_area(&tri) < 0.0 {
                tri.reverse();
            }
            for (from, to, plane) in &bands {
                let rect = [
                    self.at(*from, -reach),
                    self.at(*to, -reach),
                    self.at(*to, reach),
                    self.at(*from, reach),
                ];
                let points = terrain::clip_convex(&tri, &rect);
                if points.len() >= 3 && terrain::double_area(&points) > 1.0e-9 {
                    out.push(Piece {
                        points,
                        plane: *plane,
                        upper: None,
                    });
                }
            }
        }
        out
    }

    /// The volume of the body, in cubic meters.
    #[must_use]
    pub fn volume(&self) -> f64 {
        let t = self.ramp.thickness;
        self.pieces()
            .iter()
            .map(|piece| {
                let area = terrain::double_area(&piece.points) / 2.0;
                match self.ramp.form {
                    Form::Slab => area * t,
                    Form::Solid => area * (piece.plane.z(centroid(&piece.points)) + t),
                }
            })
            .sum()
    }

    /// The solid of the body, its level at the elevation `base`: a slab whose top is the surface
    /// and whose underside is `thickness` below it, or a solid whose underside is level, a
    /// thickness below the level.
    ///
    /// # Errors
    /// The solid kernel's errors.
    pub fn solid(&self, base: f64) -> Result<Polyhedron> {
        let t = self.ramp.thickness;
        let bottom = match self.ramp.form {
            Form::Slab => Bottom::Pieces(base - t),
            Form::Solid => Bottom::Level(base - t),
        };
        terrain::solid(&self.pieces(), base, bottom, &|_, _| FaceTag::Untagged)
    }

    /// The path of the walking surface along the left or the right edge, [`RAIL_INSET`] in from
    /// it, as `[x, y, z]` with `z` above the level; the path a hosted railing follows. For a ramp
    /// by sketch the edges are the extremes of the boundary across the run line.
    #[must_use]
    pub fn side_path(&self, left: bool) -> Vec<[f64; 3]> {
        if self.sketch() {
            let points = self.footprint();
            let (lo, hi) = points.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
                let (a, _) = self.along(*p);
                (lo.min(a), hi.max(a))
            });
            let (clo, chi) = points.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
                let (_, c) = self.along(*p);
                (lo.min(c), hi.max(c))
            });
            let c = if left {
                chi - RAIL_INSET
            } else {
                clo + RAIL_INSET
            };
            let (a0, a1) = (
                self.ramp.landing_start,
                self.ramp.landing_start + self.ramp.length,
            );
            let mut stops = vec![lo, a0, a1, hi];
            stops.retain(|a| *a >= lo - 1.0e-9 && *a <= hi + 1.0e-9);
            stops.dedup_by(|a, b| (*a - *b).abs() < 1.0e-9);
            return stops
                .into_iter()
                .map(|a| {
                    let p = self.at(a, c);
                    [p[0], p[1], self.elevation(a)]
                })
                .collect();
        }
        let (l, r) = ribbon(&self.centers(), self.ramp.width / 2.0 - RAIL_INSET);
        let side = if left { l } else { r };
        side.iter()
            .zip(&self.nodes)
            .map(|(p, n)| [p[0], p[1], n.z])
            .collect()
    }

    /// The plan drawing.
    #[must_use]
    pub fn plan(&self) -> Plan {
        let h = self.ramp.width / 2.0;
        let (walk, ticks) = if self.sketch() {
            let (a0, a1) = (
                self.ramp.landing_start,
                self.ramp.landing_start + self.ramp.length,
            );
            let (lo, hi) = self
                .footprint()
                .iter()
                .fold((f64::MAX, f64::MIN), |(lo, hi), p| {
                    let (_, c) = self.along(*p);
                    (lo.min(c), hi.max(c))
                });
            (
                vec![self.at(0.0, 0.0), self.at(self.total_length(), 0.0)],
                vec![
                    [self.at(a0, lo), self.at(a0, hi)],
                    [self.at(a1, lo), self.at(a1, hi)],
                ],
            )
        } else {
            let mut ticks: Vec<[P2; 2]> = Vec::new();
            for (i, run) in self.stretches.iter().enumerate() {
                if run.is_none() {
                    continue;
                }
                let d = unit(self.nodes[i].p, self.nodes[i + 1].p);
                for node in [self.nodes[i], self.nodes[i + 1]] {
                    let tick = [
                        [node.p[0] + d[1] * h, node.p[1] - d[0] * h],
                        [node.p[0] - d[1] * h, node.p[1] + d[0] * h],
                    ];
                    let known = ticks.iter().any(|t| {
                        (t[0][0] - tick[0][0]).hypot(t[0][1] - tick[0][1]) < 1.0e-6
                            && (t[1][0] - tick[1][0]).hypot(t[1][1] - tick[1][1]) < 1.0e-6
                    });
                    if !known {
                        ticks.push(tick);
                    }
                }
            }
            (simplify(self.centers(), false), ticks)
        };
        let end = walk.last().copied().unwrap_or([0.0, 0.0]);
        let before = walk
            .len()
            .checked_sub(2)
            .and_then(|k| walk.get(k))
            .copied()
            .unwrap_or(end);
        let d = [end[0] - before[0], end[1] - before[1]];
        let len = d[0].hypot(d[1]).max(1.0e-9);
        let t = [d[0] / len, d[1] / len];
        let size = 0.25_f64.min(len / 2.0);
        let (s, c) = (25.0_f64.to_radians().sin(), 25.0_f64.to_radians().cos());
        let back = |sign: f64| {
            [
                end[0] - size * (t[0] * c - sign * t[1] * s),
                end[1] - size * (t[1] * c + sign * t[0] * s),
            ]
        };
        Plan {
            outline: self.footprint(),
            ticks,
            arrow: [back(1.0), end, back(-1.0)],
            walk,
        }
    }
}

/// The plan drawing of a ramp: lines in plan, the drawing layer chooses the style.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// The outline.
    pub outline: Vec<[f64; 2]>,
    /// The lines across the ramp where the slope starts and ends (one at each end of each run).
    pub ticks: Vec<[[f64; 2]; 2]>,
    /// The arrow line up the ramp, along its middle.
    pub walk: Vec<[f64; 2]>,
    /// The arrow head at the top end.
    pub arrow: [[f64; 2]; 3],
}

/// A ramp as the document places it.
#[derive(Debug, Clone)]
pub struct Placed {
    /// The record's identifier.
    pub id: ObjectId,
    /// The authored data.
    pub ramp: Ramp,
    /// The elevation of the base level.
    pub base: f64,
    /// The level the ramp arrives at, when there is one.
    pub top_level: Option<ObjectId>,
    /// The layout, or why there is none.
    pub layout: std::result::Result<Layout, Refusal>,
}

impl Placed {
    /// Reads the ramp `object` of `snapshot`; `None` when the record is unreadable.
    #[must_use]
    pub fn of(snapshot: &Snapshot, object: &Object) -> Option<Self> {
        let ramp = Ramp::from_values(&object.values)?;
        let storey = object
            .header
            .owner
            .and_then(|id| spatial::get(snapshot, id));
        let base = storey
            .as_ref()
            .map_or(0.0, |s| spatial::storey_elevation(snapshot, s));
        let top = match ramp.top_level {
            Some(id) => spatial::get(snapshot, id),
            None => storey
                .as_ref()
                .and_then(|s| spatial::level_above(snapshot, s)),
        };
        let height = top
            .as_ref()
            .and_then(|l| types::meters(&l.values, level::ELEVATION))
            .map(|e| e - base);
        let max_slope = ramp
            .ramp_type
            .and_then(|t| spatial::get(snapshot, t))
            .and_then(|t| match t.values.get(ramp_type::MAX_SLOPE) {
                Some(Some(Value::F64(v))) => Some(*v),
                _ => None,
            })
            .unwrap_or(DEFAULT_MAX_SLOPE);
        let layout = Layout::new(&ramp, height, max_slope);
        Some(Self {
            id: object.id,
            ramp,
            base,
            top_level: top.map(|l| l.id),
            layout,
        })
    }

    /// The plan outline: the footprint, or a small square at the foot for a ramp without a
    /// layout, so that it can still be picked and deleted.
    #[must_use]
    pub fn outline(&self) -> Vec<[f64; 2]> {
        match &self.layout {
            Ok(layout) => layout.footprint(),
            Err(_) => {
                let (p, r) = (self.ramp.start, 0.3);
                vec![
                    [p[0] - r, p[1] - r],
                    [p[0] + r, p[1] - r],
                    [p[0] + r, p[1] + r],
                    [p[0] - r, p[1] + r],
                ]
            }
        }
    }
}

/// The shaft openings a floor gets from the ramps that arrive at the level of its storey.
#[must_use]
pub fn openings_of_floor(snapshot: &Snapshot, floor: &Object) -> Vec<Vec<[f64; 2]>> {
    let Some(level) = floor
        .header
        .owner
        .and_then(|s| spatial::get(snapshot, s))
        .and_then(|s| spatial::level_of(snapshot, &s))
    else {
        return Vec::new();
    };
    spatial::objects(snapshot, types::RAMP)
        .iter()
        .take(crate::room::MAX_ROOMS)
        .filter_map(|o| Placed::of(snapshot, o))
        .filter(|p| p.top_level == Some(level.id))
        .filter_map(|p| p.layout.ok())
        .map(|l| l.opening())
        .filter(|o| !o.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp() -> Ramp {
        Ramp {
            start: [0.0, 0.0],
            direction: 0.0,
            width: 1.2,
            length: 6.0,
            landing_start: 1.0,
            landing_end: 1.5,
            thickness: 0.2,
            form: Form::Slab,
            rise: Some(0.5),
            top_level: None,
            ramp_type: None,
            slope: None,
            runs: Vec::new(),
            boundary: Vec::new(),
        }
    }

    #[test]
    fn slope_is_rise_over_run_and_the_type_limit_is_a_warning() {
        let ok = Layout::new(&ramp(), None, DEFAULT_MAX_SLOPE).unwrap();
        assert!((ok.slope() - 0.5 / 6.0).abs() < 1.0e-12);
        assert!(!ok.exceeds());
        let steep = Layout::new(&ramp(), None, 0.05).unwrap();
        assert!(steep.exceeds());
        assert_eq!(
            Layout::new(
                &Ramp {
                    rise: None,
                    ..ramp()
                },
                None,
                0.1
            )
            .map(|l| l.height),
            Err(Refusal::NoTop)
        );
    }

    #[test]
    fn the_body_matches_its_profile_and_has_no_opening_with_a_rise() {
        for form in [Form::Slab, Form::Solid] {
            let l = Layout::new(&Ramp { form, ..ramp() }, None, 0.1).unwrap();
            let solid = l.solid(0.0).unwrap();
            assert!((solid.volume() - l.volume()).abs() < 1.0e-6, "{form:?}");
        }
        let l = Layout::new(&ramp(), None, 0.1).unwrap();
        assert!(l.opening().is_empty());
        let arriving = Layout::new(
            &Ramp {
                rise: None,
                ..ramp()
            },
            Some(0.5),
            0.1,
        )
        .unwrap();
        assert_eq!(arriving.opening().len(), 4);
        let path = arriving.side_path(true);
        assert_eq!(path.len(), 4);
        assert!((path[3][2] - 0.5).abs() < 1.0e-12 && path[0][2].abs() < 1.0e-12);
    }

    fn switchback() -> Ramp {
        Ramp {
            landing_start: 0.0,
            landing_end: 2.0,
            rise: Some(1.0),
            runs: vec![Run {
                turn: std::f64::consts::FRAC_PI_2,
                length: 6.0,
                landing: 1.0,
                slope: None,
            }],
            ..ramp()
        }
    }

    #[test]
    fn the_foot_and_the_end_of_the_first_run_move_one_at_a_time() {
        let r = Ramp {
            landing_start: 1.0,
            ..ramp()
        };
        // The run starts 1 m on and ends 7 m on. Moving the end keeps the foot and its landing.
        let moved = r.with_end_moved(true, [1.0, 4.0]).unwrap();
        assert_eq!(moved.start, [0.0, 0.0]);
        assert!((moved.length - 4.0).abs() < 1.0e-9);
        assert!((moved.direction - std::f64::consts::FRAC_PI_2).abs() < 1.0e-9);
        // Moving the foot keeps the end of the run, 7 m on: the run is 7 - (-3) - 1 = 9 m.
        let moved = r.with_end_moved(false, [-3.0, 0.0]).unwrap();
        assert_eq!(moved.start, [-3.0, 0.0]);
        assert!((moved.length - 9.0).abs() < 1.0e-9 && moved.direction.abs() < 1.0e-9);
        // A run shorter than the least, or a point that is not a number, is not made.
        assert!(r.with_end_moved(true, [1.0, 0.05]).is_none());
        assert!(r.with_end_moved(false, [6.95, 0.0]).is_none());
        assert!(r.with_end_moved(true, [f64::NAN, 0.0]).is_none());
    }

    #[test]
    fn two_runs_turn_at_the_middle_of_their_landing_and_share_the_rise() {
        let l = Layout::new(&switchback(), None, DEFAULT_MAX_SLOPE).unwrap();
        // Both runs have the slope that carries 1 m over 12 m; the top is the rise.
        assert!(l.slopes().iter().all(|s| (s - 1.0 / 12.0).abs() < 1.0e-12));
        assert!((l.height - 1.0).abs() < 1.0e-12 && l.top_off().abs() < 1.0e-12);
        let e = l.elevations();
        assert!((e[0].0).abs() < 1.0e-12 && (e[0].1 - 0.5).abs() < 1.0e-12);
        assert!((e[1].0 - 0.5).abs() < 1.0e-12 && (e[1].1 - 1.0).abs() < 1.0e-12);
        assert!(!l.exceeds());
        // The center line: 6 m east, 1 m more to the middle of the landing, 1 m north there (the
        // heading turned left), 6 m north, 1 m more.
        let c = l.centers();
        assert_eq!(c.len(), 6);
        assert!((c[2][0] - 7.0).abs() < 1.0e-9 && c[2][1].abs() < 1.0e-9);
        assert!((c[4][1] - 7.0).abs() < 1.0e-9);
        // The outline is an L with its corner mitered: six corners, and the body is closed with
        // the volume of its pieces.
        assert_eq!(l.footprint().len(), 6);
        let solid = l.solid(0.0).unwrap();
        assert!(
            (solid.volume() - l.volume()).abs() < 1.0e-6,
            "{}",
            solid.volume()
        );
        // The landing before the next run, the arrival at its end: the opening stops there.
        let arriving = Layout::new(
            &Ramp {
                rise: None,
                ..switchback()
            },
            Some(1.0),
            0.1,
        )
        .unwrap();
        assert!(arriving.opening().len() >= 4);
        // Reading the record back gives the same ramp.
        assert_eq!(
            Ramp::from_values(&switchback().values("R")),
            Some(switchback())
        );
    }

    #[test]
    fn the_slope_of_one_run_shifts_the_elevations_after_it_and_not_before() {
        let steeper = Ramp {
            runs: vec![Run {
                slope: Some(0.1),
                ..switchback().runs[0]
            }],
            ..switchback()
        };
        let l = Layout::new(&steeper, None, DEFAULT_MAX_SLOPE).unwrap();
        let before = Layout::new(&switchback(), None, DEFAULT_MAX_SLOPE).unwrap();
        // The first run is as it was; the second starts where the first ends and rises 0.6 m.
        assert_eq!(l.elevations()[0], before.elevations()[0]);
        assert!((l.elevations()[1].1 - 1.1).abs() < 1.0e-12);
        assert!((l.height - 1.1).abs() < 1.0e-12);
        // The top is no longer the rise it was to reach; the slope of that run passes the type's.
        assert!((l.top_off() - 0.1).abs() < 1.0e-12);
        assert!(l.exceeds() && (l.slope() - 0.1).abs() < 1.0e-12);
        // A slope of the first run moves the second run's elevations with it.
        let first = Ramp {
            slope: Some(0.1),
            ..switchback()
        };
        let m = Layout::new(&first, None, 0.2).unwrap();
        assert!((m.elevations()[0].1 - 0.6).abs() < 1.0e-12);
        assert!((m.elevations()[1].0 - 0.6).abs() < 1.0e-12);
        assert!((m.elevations()[1].1 - 0.6 - 0.5).abs() < 1.0e-12);
    }

    #[test]
    fn a_turn_that_does_not_fit_its_landing_or_is_too_wide_is_refused() {
        let tight = Ramp {
            landing_end: 0.5,
            ..switchback()
        };
        assert_eq!(
            Layout::new(&tight, None, 0.1).map(|_| ()),
            Err(Refusal::Malformed)
        );
        let none = Ramp {
            landing_end: 0.0,
            ..switchback()
        };
        assert_eq!(
            Layout::new(&none, None, 0.1).map(|_| ()),
            Err(Refusal::Malformed)
        );
        let mut wide = switchback();
        wide.runs[0].turn = 3.0;
        assert_eq!(
            Layout::new(&wide, None, 0.1).map(|_| ()),
            Err(Refusal::Malformed)
        );
        let mut many = switchback();
        many.runs = vec![many.runs[0]; MAX_RUNS + 1];
        assert_eq!(
            Layout::new(&many, None, 0.1).map(|_| ()),
            Err(Refusal::Malformed)
        );
    }

    fn sketch(boundary: Vec<[f64; 2]>) -> Ramp {
        Ramp {
            landing_start: 1.0,
            landing_end: 1.0,
            length: 6.0,
            rise: Some(0.5),
            boundary,
            ..ramp()
        }
    }

    #[test]
    fn a_ramp_by_sketch_rises_along_its_run_line_over_any_simple_boundary() {
        let rectangle = vec![[0.0, -1.0], [8.0, -1.0], [8.0, 1.0], [0.0, 1.0]];
        let l = Layout::new(&sketch(rectangle), None, 0.1).unwrap();
        assert_eq!(l.footprint().len(), 4);
        assert!((l.volume() - 16.0 * 0.2).abs() < 1.0e-9);
        let solid = l.solid(0.0).unwrap();
        assert!((solid.volume() - l.volume()).abs() < 1.0e-6);
        // The top is at the rise past the slope, and the surface follows the run line.
        let zs: Vec<f64> = solid.vertices().iter().map(|v| v.z).collect();
        assert!(
            zs.iter().any(|z| (z - 0.5).abs() < 1.0e-9) && zs.iter().all(|z| *z <= 0.5 + 1.0e-9)
        );
        // A boundary that is not a rectangle, or concave, has the same: the body of every piece.
        let bent = vec![
            [0.0, -1.0],
            [8.0, -1.0],
            [8.0, 3.0],
            [5.0, 3.0],
            [5.0, 1.0],
            [0.0, 1.0],
        ];
        let l = Layout::new(&sketch(bent), None, 0.1).unwrap();
        let solid = l.solid(0.0).unwrap();
        assert!(
            (solid.volume() - l.volume()).abs() < 1.0e-6,
            "{}",
            solid.volume()
        );
        assert!((l.volume() - (16.0 + 6.0) * 0.2).abs() < 1.0e-9);
        // A boundary that crosses itself, or has fewer than three points, is refused.
        let bow = vec![[0.0, 0.0], [4.0, 4.0], [4.0, 0.0], [0.0, 4.0]];
        assert_eq!(
            Layout::new(&sketch(bow), None, 0.1).map(|_| ()),
            Err(Refusal::Malformed)
        );
        assert_eq!(
            Layout::new(&sketch(vec![[0.0, 0.0], [1.0, 0.0]]), None, 0.1).map(|_| ()),
            Err(Refusal::Malformed)
        );
        // It arrives with an opening up to the arrival edge: x = 7.
        let arriving = Layout::new(
            &Ramp {
                rise: None,
                ..sketch(vec![[0.0, -1.0], [8.0, -1.0], [8.0, 1.0], [0.0, 1.0]])
            },
            Some(0.5),
            0.1,
        )
        .unwrap();
        let open = arriving.opening();
        assert!(open.len() >= 4 && open.iter().all(|p| p[0] <= 7.0 + 1.0e-9));
        let path = arriving.side_path(true);
        assert!((path.last().unwrap()[2] - 0.5).abs() < 1.0e-9 && path[0][2].abs() < 1.0e-9);
    }
}
