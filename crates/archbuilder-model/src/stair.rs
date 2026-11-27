//! The stair: authored data read from a record, and everything else derived from it.
//!
//! A stair record holds only what was authored: the shape (straight, an L turn or a U turn, each
//! turn with one flat landing), the side the turn goes to, the start (the middle of the first
//! riser), the direction of the first run, the width, the tread depth, the highest riser allowed,
//! the waist thickness of the monolithic construction, a railing flag for each side and, when
//! given, the level it arrives at. The base is the level of its storey; the top is the named
//! level, else the level above the storey.
//!
//! Derived each time ([`Layout`]): the riser count `ceil(height / max riser)`, the riser height
//! `height / count`, the runs (an L stair gives the first run the larger half of the risers, a U
//! stair the smaller half),
//! their lengths `(risers - 1) x tread`, the landing, the plan footprint, the solids (each run an
//! extruded stepped side profile with the waist below it, the landing a slab of the waist
//! thickness), the railings (a top rail [`RAIL_HEIGHT`] above the line of the nosings and posts)
//! and the shaft opening the floors of the top level get ([`Layout::opening`]). The stride rule
//! `2R + T` in [`STRIDE_RANGE`] is a warning, not a refusal.
//!
//! A stair by sketch (shape `sketch`) takes its runs from run lines instead: each run line is the
//! walking line of one straight run, from the middle of its first riser. The risers are shared
//! among the runs in proportion to their line lengths (two at least each), each run keeps the
//! stair's width, tread and waist, and the landing between two runs is the convex hull of the
//! last riser of one and the first riser of the next. Everything else (solids, rails, plan, shaft
//! opening, headroom) follows the rules of the component stairs. The least clear height above the
//! walking surface is the stair's `headroom`, [`DEFAULT_HEADROOM`] when absent.
//!
//! Coordinates: `a` runs along the first run from the start, `c` across it to the left (to the
//! right for a right turn, so a right stair is the mirror image of a left one), `z` up from the
//! base level.

use arc_base::math::{Point2, Point3, Vector3};
use arc_base::{Limits, ObjectId, Result, Tolerance};
use arc_doc::{Object, Snapshot, Value};
use arc_geom::{Curve2, Loop2, PlaneFrame, Polyhedron, Region, RegionPart, Segment2, extrude};

use crate::spatial;
use crate::types::{self, level, stair};

/// The default width, in meters.
pub const DEFAULT_WIDTH: f64 = 1.0;
/// The default tread depth, in meters.
pub const DEFAULT_TREAD: f64 = 0.28;
/// The default highest riser, in meters.
pub const DEFAULT_MAX_RISER: f64 = 0.18;
/// The default waist thickness, in meters.
pub const DEFAULT_WAIST: f64 = 0.15;
/// The range of the stride `2R + T` that is comfortable to walk, in meters; outside it the
/// stair gets a warning.
pub const STRIDE_RANGE: (f64, f64) = (0.60, 0.66);
/// The gap between the two runs of a U stair, in meters.
pub const U_GAP: f64 = 0.10;
/// The most risers of one stair.
pub const MAX_RISERS: usize = 200;
/// The height of the plan cut above the base, in meters.
pub const CUT_HEIGHT: f64 = 1.2;
/// The clearance the shaft opening keeps around the stair, except at the arrival edge, in meters.
pub const OPENING_CLEARANCE: f64 = 0.05;
/// The height of the top rail above the line of the nosings (and above a landing), in meters.
pub const RAIL_HEIGHT: f64 = 0.9;
/// How far the railing stands in from the edge of the stair, in meters.
pub const RAIL_INSET: f64 = 0.05;
/// The largest distance between two posts of a railing, in meters.
pub const POST_SPACING: f64 = 1.2;
/// The side of the top rail's square section, in meters.
pub const RAIL_SIZE: f64 = 0.05;
/// The side of a post's square section, in meters.
pub const POST_SIZE: f64 = 0.04;
/// The most run lines of a stair by sketch.
pub const MAX_RUN_LINES: usize = 16;
/// The least clear height above treads and landings, in meters, when the stair names none.
pub const DEFAULT_HEADROOM: f64 = 2.0;

/// The chord of the solid kernel; stairs have no curves.
const CHORD: f64 = 0.01;

/// The shape of a stair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shape {
    /// One run.
    #[default]
    Straight,
    /// Two runs at a right angle with a square landing between them.
    LTurn,
    /// Two runs side by side in opposite directions with a landing across both.
    UTurn,
    /// Straight runs along sketched run lines, with a landing between each two.
    Sketch,
}

impl Shape {
    /// Every shape, in code order.
    pub const ALL: [Self; 4] = [Self::Straight, Self::LTurn, Self::UTurn, Self::Sketch];

    /// The name in the record.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Straight => "straight",
            Self::LTurn => "l_turn",
            Self::UTurn => "u_turn",
            Self::Sketch => "sketch",
        }
    }

    /// The code stored in the record.
    #[must_use]
    pub const fn code(self) -> u32 {
        match self {
            Self::Straight => 0,
            Self::LTurn => 1,
            Self::UTurn => 2,
            Self::Sketch => 3,
        }
    }

    /// The shape of a code; `None` for a code that names none.
    #[must_use]
    pub fn from_code(code: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.code() == code)
    }

    /// The least risers of the shape: two in each run.
    #[must_use]
    pub const fn least_risers(self) -> usize {
        match self {
            Self::Straight | Self::Sketch => 2,
            Self::LTurn | Self::UTurn => 4,
        }
    }
}

/// The side an L or U stair turns to, seen walking up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Turn {
    /// To the left.
    #[default]
    Left,
    /// To the right.
    Right,
}

impl Turn {
    /// Both sides, in code order.
    pub const ALL: [Self; 2] = [Self::Left, Self::Right];

    /// The name in the record.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
        }
    }

    /// The code stored in the record.
    #[must_use]
    pub const fn code(self) -> u32 {
        match self {
            Self::Left => 0,
            Self::Right => 1,
        }
    }

    /// The side of a code; `None` for a code that names none.
    #[must_use]
    pub fn from_code(code: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.code() == code)
    }

    /// The other side.
    #[must_use]
    pub const fn flipped(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

/// The authored data of a stair, lengths in meters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stair {
    /// The shape.
    pub shape: Shape,
    /// The side a turn goes to.
    pub turn: Turn,
    /// The middle of the first riser.
    pub start: [f64; 2],
    /// The direction of the first run, in radians from the x axis.
    pub direction: f64,
    /// The width of the runs and the landing.
    pub width: f64,
    /// The tread depth.
    pub tread: f64,
    /// The highest riser allowed.
    pub max_riser: f64,
    /// The waist thickness.
    pub waist: f64,
    /// Whether the left and the right side (walking up) have a railing.
    pub railings: [bool; 2],
    /// The level the stair arrives at; the level above its storey when `None`.
    pub top_level: Option<ObjectId>,
}

fn enumerated(values: &[Option<Value>], at: usize) -> Option<u32> {
    match values.get(at)? {
        Some(Value::Enum(code)) => Some(*code),
        _ => None,
    }
}

fn flag(values: &[Option<Value>], at: usize) -> bool {
    !matches!(values.get(at), Some(Some(Value::Bool(false))))
}

impl Stair {
    /// The stair of a record's field values; `None` when a field is missing or malformed.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Option<Self> {
        let m = |at| types::meters(values, at);
        let direction = match values.get(stair::DIRECTION)? {
            Some(Value::Quantity(q)) => q.value(),
            _ => return None,
        };
        Some(Self {
            shape: Shape::from_code(enumerated(values, stair::SHAPE)?)?,
            turn: Turn::from_code(enumerated(values, stair::TURN)?)?,
            start: [m(stair::X)?, m(stair::Y)?],
            direction,
            width: m(stair::WIDTH)?,
            tread: m(stair::TREAD)?,
            max_riser: m(stair::MAX_RISER)?,
            waist: m(stair::WAIST)?,
            railings: [
                flag(values, stair::RAILING_LEFT),
                flag(values, stair::RAILING_RIGHT),
            ],
            top_level: match values.get(stair::TOP_LEVEL) {
                Some(Some(Value::Reference(id))) => Some(*id),
                _ => None,
            },
        })
    }

    /// The field values of a record of this stair named `name`.
    #[must_use]
    pub fn values(&self, name: &str) -> Vec<Option<Value>> {
        let mut values: Vec<Option<Value>> = vec![None; stair::TOP_LEVEL + 1];
        values[stair::NAME] = Some(Value::String(name.to_owned()));
        values[stair::SHAPE] = Some(Value::Enum(self.shape.code()));
        values[stair::TURN] = Some(Value::Enum(self.turn.code()));
        values[stair::X] = Some(types::length(self.start[0]));
        values[stair::Y] = Some(types::length(self.start[1]));
        values[stair::DIRECTION] = Some(angle(self.direction));
        values[stair::WIDTH] = Some(types::length(self.width));
        values[stair::TREAD] = Some(types::length(self.tread));
        values[stair::MAX_RISER] = Some(types::length(self.max_riser));
        values[stair::WAIST] = Some(types::length(self.waist));
        values[stair::RAILING_LEFT] = Some(Value::Bool(self.railings[0]));
        values[stair::RAILING_RIGHT] = Some(Value::Bool(self.railings[1]));
        values[stair::TOP_LEVEL] = self.top_level.map(Value::Reference);
        values
    }

    /// The stair under the plane motion `xf`: the start moved, the direction turned, and under a
    /// reflection the turn and the railings swapped, so the mirror image is what is stored.
    #[must_use]
    pub fn moved(&self, xf: &crate::transform::Xform) -> Self {
        let (s, c) = crate::transform::sin_cos(self.direction);
        let start = xf.point(self.start);
        let ahead = xf.point([self.start[0] + c, self.start[1] + s]);
        let direction = (ahead[1] - start[1]).atan2(ahead[0] - start[0]);
        if xf.reflects() {
            Self {
                start,
                direction,
                turn: self.turn.flipped(),
                railings: [self.railings[1], self.railings[0]],
                ..*self
            }
        } else {
            Self {
                start,
                direction,
                ..*self
            }
        }
    }
}

/// One run line of a stair by sketch: the walking line of a straight run, from the middle of its
/// first riser toward the middle of its last.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunLine {
    /// The middle of the first riser.
    pub from: [f64; 2],
    /// The point the run heads toward; only the direction of the line is used.
    pub to: [f64; 2],
}

/// The run lines of a record's field values; empty when it has none.
#[must_use]
pub fn run_lines(values: &[Option<Value>]) -> Vec<RunLine> {
    let Some(Some(Value::List(items))) = values.get(stair::RUNS) else {
        return Vec::new();
    };
    items
        .iter()
        .take(MAX_RUN_LINES)
        .filter_map(|item| {
            let Value::Record(f) = item else {
                return None;
            };
            let m = |at| types::meters(f, at);
            Some(RunLine {
                from: [m(0)?, m(1)?],
                to: [m(2)?, m(3)?],
            })
        })
        .collect()
}

/// The record's value of a list of run lines.
#[must_use]
pub fn run_values(lines: &[RunLine]) -> Value {
    Value::List(
        lines
            .iter()
            .map(|l| {
                Value::Record(vec![
                    Some(types::length(l.from[0])),
                    Some(types::length(l.from[1])),
                    Some(types::length(l.to[0])),
                    Some(types::length(l.to[1])),
                ])
            })
            .collect(),
    )
}

/// The least clear height of a record: its own, else [`DEFAULT_HEADROOM`].
#[must_use]
pub fn headroom_of(values: &[Option<Value>]) -> f64 {
    types::meters(values, stair::HEADROOM)
        .filter(|h| h.is_finite() && *h > 0.0)
        .unwrap_or(DEFAULT_HEADROOM)
}

impl Stair {
    /// The field values of the stair of `values` under the plane motion `xf`, named `name`: the
    /// moved stair ([`Stair::moved`]) with its run lines moved and its headroom kept; `None` when
    /// the record is unreadable. `clear_top` drops the top level (a stair copied to another
    /// storey arrives at that storey's level above).
    #[must_use]
    pub fn moved_values(
        values: &[Option<Value>],
        name: &str,
        xf: &crate::transform::Xform,
        clear_top: bool,
    ) -> Option<Vec<Option<Value>>> {
        let mut moved = Self::from_values(values)?.moved(xf);
        if clear_top {
            moved.top_level = None;
        }
        let mut out = moved.values(name);
        out.resize(stair::HEADROOM + 1, None);
        out[stair::HEADROOM] = values.get(stair::HEADROOM).cloned().flatten();
        let lines = run_lines(values);
        if !lines.is_empty() {
            let lines: Vec<RunLine> = lines
                .iter()
                .map(|l| RunLine {
                    from: xf.point(l.from),
                    to: xf.point(l.to),
                })
                .collect();
            out[stair::RUNS] = Some(run_values(&lines));
        }
        Some(out)
    }
}

/// An angle as a field value.
#[must_use]
pub fn angle(radians: f64) -> Value {
    Value::Quantity(arc_base::Quantity::new(radians, arc_base::Dimension::ANGLE))
}

/// Why a stair has no layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// No level above the base: the stair has no height.
    NoTop,
    /// The height and the highest riser give fewer risers than the shape needs, or more than
    /// [`MAX_RISERS`].
    Risers {
        /// The riser count the height gives.
        count: usize,
        /// The least the shape needs.
        least: usize,
    },
    /// A length is not finite or not positive.
    Malformed,
}

/// One run: risers in a straight line, in plan from the middle of its first riser.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Run {
    /// The middle of the first riser, in plan.
    pub start: [f64; 2],
    /// The unit direction up the run, in plan.
    pub dir: [f64; 2],
    /// The risers of the run; the last one rises onto the landing or the top level.
    pub risers: usize,
    /// The elevation of the run's foot above the base.
    pub base: f64,
    /// The horizontal length: `(risers - 1) x tread`.
    pub length: f64,
}

/// What a stair derives from its data and its height.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// The authored data.
    pub stair: Stair,
    /// The height from the base level to the top level.
    pub height: f64,
    /// The riser count.
    pub risers: usize,
    /// The riser height.
    pub riser: f64,
    /// The runs, from the bottom up.
    pub runs: Vec<Run>,
    /// The landing's corners in plan, counterclockwise; empty for a straight stair.
    pub landing: Vec<[f64; 2]>,
    /// The elevation of the landing's top above the base.
    pub landing_top: f64,
    /// The landings of a stair by sketch, each its corners (counterclockwise) and the elevation
    /// of its top above the base; empty for the other shapes.
    pub landings: Vec<(Vec<[f64; 2]>, f64)>,
    /// The local frame: origin, along and across (to the left for a left turn).
    frame: Frame,
}

/// The plan frame of a stair: `a` along the first run, `c` across it toward the turn.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Frame {
    origin: [f64; 2],
    u: [f64; 2],
    v: [f64; 2],
    mirrored: bool,
}

impl Frame {
    fn new(stair: &Stair) -> Self {
        let (s, c) = crate::transform::sin_cos(stair.direction);
        let side =
            if stair.turn == Turn::Right && matches!(stair.shape, Shape::LTurn | Shape::UTurn) {
                -1.0
            } else {
                1.0
            };
        Self {
            origin: stair.start,
            u: [c, s],
            v: [-s * side, c * side],
            mirrored: side < 0.0,
        }
    }

    fn at(&self, a: f64, c: f64) -> [f64; 2] {
        [
            self.origin[0] + self.u[0] * a + self.v[0] * c,
            self.origin[1] + self.u[1] * a + self.v[1] * c,
        ]
    }

    fn dir(&self, a: f64, c: f64) -> [f64; 2] {
        [self.u[0] * a + self.v[0] * c, self.u[1] * a + self.v[1] * c]
    }

    /// A polygon given counterclockwise in the frame, counterclockwise in plan.
    fn polygon(&self, local: &[[f64; 2]]) -> Vec<[f64; 2]> {
        let mut out: Vec<[f64; 2]> = local.iter().map(|p| self.at(p[0], p[1])).collect();
        if self.mirrored {
            out.reverse();
        }
        out
    }
}

/// The signed area of a polygon (positive counterclockwise).
fn signed_area(points: &[[f64; 2]]) -> f64 {
    crate::floor::signed_area(points)
}

/// The convex hull of `points`, counterclockwise (Andrew's monotone chain).
fn hull(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut p: Vec<[f64; 2]> = points.to_vec();
    p.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    p.dedup_by(|a, b| (a[0] - b[0]).abs() < 1.0e-9 && (a[1] - b[1]).abs() < 1.0e-9);
    if p.len() < 3 {
        return p;
    }
    let cross = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };
    let mut out: Vec<[f64; 2]> = Vec::new();
    for pass in 0..2 {
        let start = out.len();
        let iter: Box<dyn Iterator<Item = &[f64; 2]>> = if pass == 0 {
            Box::new(p.iter())
        } else {
            Box::new(p.iter().rev())
        };
        for q in iter {
            while out.len() >= start + 2 && cross(out[out.len() - 2], out[out.len() - 1], *q) <= 0.0
            {
                out.pop();
            }
            out.push(*q);
        }
        out.pop();
    }
    out
}

/// A region of one loop of straight edges.
fn region(points: &[[f64; 2]]) -> Result<Region> {
    let tolerance = Tolerance::DEFAULT;
    let pts: Vec<Point2> = points.iter().map(|p| Point2::new(p[0], p[1])).collect();
    let mut curves = Vec::with_capacity(pts.len());
    for k in 0..pts.len() {
        curves.push(Curve2::Segment(Segment2::new(
            pts[k],
            pts[(k + 1) % pts.len()],
        )?));
    }
    Region::new(vec![RegionPart::new(
        Loop2::new(curves, &tolerance)?,
        Vec::new(),
    )])
}

/// A square bar of side `size` from `p` to `q`.
fn bar(p: [f64; 3], q: [f64; 3], size: f64) -> Result<Polyhedron> {
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
    let h = size / 2.0;
    extrude(
        &region(&[[-h, -h], [h, -h], [h, h], [-h, h]])?,
        &frame,
        length,
        CHORD,
        &tolerance,
        &Limits::DEFAULT,
    )
}

/// The side profile of a run with `n` risers in its own `(along, up)` plane, counterclockwise:
/// the steps on top up to the last tread, the waist below, cut vertically at the top end and flat
/// on the base. The last riser is the edge of what the run arrives at: the landing or the floor
/// of the top level.
fn profile(n: usize, riser: f64, tread: f64, waist: f64) -> Vec<[f64; 2]> {
    let length = (n - 1) as f64 * tread;
    let slope = riser / tread;
    // The waist is measured square to the pitch line through the roots of the steps.
    let drop = waist * tread.hypot(riser) / tread;
    let mut steps = vec![[0.0, 0.0]];
    for k in 0..n - 1 {
        let a = k as f64 * tread;
        steps.push([a, (k + 1) as f64 * riser]);
        steps.push([a + tread, (k + 1) as f64 * riser]);
    }
    let top = slope * length - drop;
    if top > 0.0 {
        steps.push([length, top]);
        steps.push([drop / slope, 0.0]);
    } else {
        steps.push([length, 0.0]);
    }
    steps.reverse();
    steps
}

impl Layout {
    /// The layout of `stair` rising `height`.
    ///
    /// # Errors
    /// [`Refusal`].
    #[allow(
        clippy::too_many_lines,
        reason = "one layout for the three component shapes"
    )]
    pub fn new(stair: &Stair, height: f64) -> std::result::Result<Self, Refusal> {
        let finite = [
            height,
            stair.width,
            stair.tread,
            stair.max_riser,
            stair.waist,
            stair.direction,
            stair.start[0],
            stair.start[1],
        ]
        .iter()
        .all(|v| v.is_finite());
        if !finite || stair.width <= 0.0 || stair.tread <= 0.0 || stair.waist <= 0.0 {
            return Err(Refusal::Malformed);
        }
        if height < types::HEIGHT_RANGE.0 {
            return Err(Refusal::NoTop);
        }
        if stair.max_riser <= 0.0 {
            return Err(Refusal::Malformed);
        }
        if stair.shape == Shape::Sketch {
            // A stair by sketch has run lines; without them it has no layout.
            return Err(Refusal::Malformed);
        }
        let least = stair.shape.least_risers();
        let ratio = height / stair.max_riser;
        // A height a whole number of highest risers needs exactly that many.
        let wanted = (ratio - 1.0e-9).ceil();
        if !(1.0..=MAX_RISERS as f64).contains(&wanted) {
            return Err(Refusal::Risers {
                count: if wanted < 1.0 { 0 } else { MAX_RISERS + 1 },
                least,
            });
        }
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a whole number between 1 and MAX_RISERS"
        )]
        let risers = wanted as usize;
        if risers < least {
            return Err(Refusal::Risers {
                count: risers,
                least,
            });
        }
        let riser = height / risers as f64;
        let frame = Frame::new(stair);
        let (t, w) = (stair.tread, stair.width);
        let h = w / 2.0;
        let first = match stair.shape {
            Shape::Straight | Shape::Sketch => risers,
            // An L stair's first run takes the larger half; a U stair's the smaller one, so its
            // upper run reaches past the foot and the opening covers the whole lower run.
            Shape::LTurn => risers.div_ceil(2),
            Shape::UTurn => risers / 2,
        };
        let second = risers - first;
        let l0 = (first - 1) as f64 * t;
        let mut runs = vec![Run {
            start: frame.at(0.0, 0.0),
            dir: frame.dir(1.0, 0.0),
            risers: first,
            base: 0.0,
            length: l0,
        }];
        let landing_top = first as f64 * riser;
        let l1 = second.saturating_sub(1) as f64 * t;
        let (landing, second_run) = match stair.shape {
            Shape::Straight | Shape::Sketch => (Vec::new(), None),
            Shape::LTurn => (
                vec![[l0, -h], [l0 + w, -h], [l0 + w, h], [l0, h]],
                Some(([l0 + h, h], [0.0, 1.0])),
            ),
            Shape::UTurn => (
                vec![
                    [l0, -h],
                    [l0 + w, -h],
                    [l0 + w, 3.0 * h + U_GAP],
                    [l0, 3.0 * h + U_GAP],
                ],
                Some(([l0, w + U_GAP], [-1.0, 0.0])),
            ),
        };
        if let Some((start, dir)) = second_run {
            runs.push(Run {
                start: frame.at(start[0], start[1]),
                dir: frame.dir(dir[0], dir[1]),
                risers: second,
                base: landing_top,
                length: l1,
            });
        }
        Ok(Self {
            stair: *stair,
            height,
            risers,
            riser,
            runs,
            landing: frame.polygon(&landing),
            landing_top,
            landings: Vec::new(),
            frame,
        })
    }

    /// The layout of `stair` rising `height`: along `lines` for a stair by sketch, else as
    /// [`Layout::new`].
    ///
    /// # Errors
    /// [`Refusal`].
    pub fn build(
        stair: &Stair,
        lines: &[RunLine],
        height: f64,
    ) -> std::result::Result<Self, Refusal> {
        if stair.shape == Shape::Sketch {
            Self::sketch(stair, lines, height)
        } else {
            Self::new(stair, height)
        }
    }

    /// The layout of a stair by sketch: one run along each line, the risers shared in proportion
    /// to the lengths of the lines, and a landing between each two runs.
    ///
    /// # Errors
    /// [`Refusal`].
    #[allow(
        clippy::too_many_lines,
        reason = "validation, riser sharing, runs and landings"
    )]
    pub fn sketch(
        stair: &Stair,
        lines: &[RunLine],
        height: f64,
    ) -> std::result::Result<Self, Refusal> {
        let finite = [
            height,
            stair.width,
            stair.tread,
            stair.max_riser,
            stair.waist,
        ]
        .iter()
        .all(|v| v.is_finite())
            && lines
                .iter()
                .flat_map(|l| l.from.iter().chain(&l.to))
                .all(|v| v.is_finite());
        if !finite
            || stair.width <= 0.0
            || stair.tread <= 0.0
            || stair.waist <= 0.0
            || stair.max_riser <= 0.0
            || lines.is_empty()
            || lines.len() > MAX_RUN_LINES
        {
            return Err(Refusal::Malformed);
        }
        if height < types::HEIGHT_RANGE.0 {
            return Err(Refusal::NoTop);
        }
        let dirs: Vec<([f64; 2], f64)> = lines
            .iter()
            .map(|l| {
                let d = [l.to[0] - l.from[0], l.to[1] - l.from[1]];
                let len = d[0].hypot(d[1]);
                ([d[0] / len, d[1] / len], len)
            })
            .collect();
        if dirs
            .iter()
            .any(|(_, len)| !len.is_finite() || *len < 1.0e-6)
        {
            return Err(Refusal::Malformed);
        }
        let least = 2 * lines.len();
        let wanted = ((height / stair.max_riser) - 1.0e-9).ceil();
        if !(1.0..=MAX_RISERS as f64).contains(&wanted) {
            return Err(Refusal::Risers {
                count: if wanted < 1.0 { 0 } else { MAX_RISERS + 1 },
                least,
            });
        }
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a whole number between 1 and MAX_RISERS"
        )]
        let risers = wanted as usize;
        if risers < least {
            return Err(Refusal::Risers {
                count: risers,
                least,
            });
        }
        // Two risers in each run, then one at a time to the run with the most line length per
        // riser (the lowest first on a tie).
        let mut counts = vec![2_usize; lines.len()];
        for _ in 0..risers - least {
            let mut best = 0;
            for (k, (_, len)) in dirs.iter().enumerate() {
                if len / counts[k] as f64 > dirs[best].1 / counts[best] as f64 + 1.0e-12 {
                    best = k;
                }
            }
            counts[best] += 1;
        }
        let riser = height / risers as f64;
        let mut runs = Vec::with_capacity(lines.len());
        let mut done = 0_usize;
        for (k, line) in lines.iter().enumerate() {
            runs.push(Run {
                start: line.from,
                dir: dirs[k].0,
                risers: counts[k],
                base: done as f64 * riser,
                length: (counts[k] - 1) as f64 * stair.tread,
            });
            done += counts[k];
        }
        let h = stair.width / 2.0;
        let mut landings = Vec::new();
        for (k, pair) in runs.windows(2).enumerate() {
            let end = |run: &Run, c: f64| {
                let n = [-run.dir[1], run.dir[0]];
                [
                    run.start[0] + run.dir[0] * run.length + n[0] * c,
                    run.start[1] + run.dir[1] * run.length + n[1] * c,
                ]
            };
            let begin = |run: &Run, c: f64| {
                let n = [-run.dir[1], run.dir[0]];
                [run.start[0] + n[0] * c, run.start[1] + n[1] * c]
            };
            let corners = hull(&[
                end(&pair[0], -h),
                end(&pair[0], h),
                begin(&pair[1], -h),
                begin(&pair[1], h),
            ]);
            if corners.len() >= 3 && signed_area(&corners) > 1.0e-4 {
                let top: usize = counts[..=k].iter().sum();
                landings.push((corners, top as f64 * riser));
            }
        }
        let first = runs[0];
        let moved = Stair {
            start: first.start,
            direction: first.dir[1].atan2(first.dir[0]),
            ..*stair
        };
        Ok(Self {
            stair: moved,
            height,
            risers,
            riser,
            runs,
            landing: Vec::new(),
            landing_top: counts[0] as f64 * riser,
            landings,
            frame: Frame::new(&moved),
        })
    }

    /// Turns the railing of each side (left, right) on or off; the railings of the hosted
    /// railing records are read with both on.
    pub fn set_railings(&mut self, on: [bool; 2]) {
        self.stair.railings = on;
    }

    /// The stride `2R + T`.
    #[must_use]
    pub fn stride(&self) -> f64 {
        2.0 * self.riser + self.stair.tread
    }

    /// Whether the stride lies in [`STRIDE_RANGE`].
    #[must_use]
    pub fn stride_ok(&self) -> bool {
        let s = self.stride();
        s >= STRIDE_RANGE.0 - 1.0e-9 && s <= STRIDE_RANGE.1 + 1.0e-9
    }

    /// The corners of a run in plan, counterclockwise: the foot of the first riser first.
    #[must_use]
    pub fn run_corners(&self, run: &Run) -> Vec<[f64; 2]> {
        let h = self.stair.width / 2.0;
        let n = [-run.dir[1], run.dir[0]];
        let at = |a: f64, c: f64| {
            [
                run.start[0] + run.dir[0] * a + n[0] * c,
                run.start[1] + run.dir[1] * a + n[1] * c,
            ]
        };
        vec![
            at(0.0, -h),
            at(run.length, -h),
            at(run.length, h),
            at(0.0, h),
        ]
    }

    /// The plan footprint, counterclockwise: the runs and the landing as one outline (a U
    /// stair's outline has the gap between its runs as a notch).
    #[must_use]
    pub fn footprint(&self) -> Vec<[f64; 2]> {
        if self.stair.shape == Shape::Sketch {
            return self.sketch_footprint();
        }
        let (w, h) = (self.stair.width, self.stair.width / 2.0);
        let l0 = self.runs[0].length;
        let l1 = self.runs.get(1).map_or(0.0, |r| r.length);
        let g = U_GAP;
        let local: Vec<[f64; 2]> = match self.stair.shape {
            Shape::Straight | Shape::Sketch => vec![[0.0, -h], [l0, -h], [l0, h], [0.0, h]],
            Shape::LTurn => vec![
                [0.0, -h],
                [l0 + w, -h],
                [l0 + w, h + l1],
                [l0, h + l1],
                [l0, h],
                [0.0, h],
            ],
            Shape::UTurn => vec![
                [0.0, -h],
                [l0 + w, -h],
                [l0 + w, 3.0 * h + g],
                [l0 - l1, 3.0 * h + g],
                [l0 - l1, h + g],
                [l0, h + g],
                [l0, h],
                [0.0, h],
            ],
        };
        self.frame.polygon(&local)
    }

    /// The shaft opening the floors of the top level get, counterclockwise: the footprint with
    /// [`OPENING_CLEARANCE`] on every side except the arrival edge, where the floor meets the
    /// last riser. A U stair's opening is the rectangle over both runs and the gap, from the
    /// last riser of the upper run (past the foot) to the landing.
    #[must_use]
    pub fn opening(&self) -> Vec<[f64; 2]> {
        if self.stair.shape == Shape::Sketch {
            return self.sketch_opening();
        }
        let (w, h, k) = (self.stair.width, self.stair.width / 2.0, OPENING_CLEARANCE);
        let l0 = self.runs[0].length;
        let l1 = self.runs.get(1).map_or(0.0, |r| r.length);
        let g = U_GAP;
        let local: Vec<[f64; 2]> = match self.stair.shape {
            Shape::Straight | Shape::Sketch => {
                vec![[-k, -h - k], [l0, -h - k], [l0, h + k], [-k, h + k]]
            }
            Shape::LTurn => vec![
                [-k, -h - k],
                [l0 + w + k, -h - k],
                [l0 + w + k, h + l1],
                [l0 - k, h + l1],
                [l0 - k, h + k],
                [-k, h + k],
            ],
            Shape::UTurn => vec![
                [l0 - l1, -h - k],
                [l0 + w + k, -h - k],
                [l0 + w + k, 3.0 * h + g + k],
                [l0 - l1, 3.0 * h + g + k],
            ],
        };
        self.frame.polygon(&local)
    }

    /// The corner loops of the runs and the landings of a stair by sketch.
    fn sketch_parts(&self) -> Vec<Vec<[f64; 2]>> {
        let mut parts: Vec<Vec<[f64; 2]>> = self.runs.iter().map(|r| self.run_corners(r)).collect();
        parts.extend(self.landings.iter().map(|(corners, _)| corners.clone()));
        parts
    }

    /// The outline of a stair by sketch: the union of its runs and landings, the convex hull when
    /// the kernel cannot unite them.
    fn sketch_footprint(&self) -> Vec<[f64; 2]> {
        let parts = self.sketch_parts();
        let merged = crate::floor::merge_loops(&parts);
        let outline = merged
            .into_iter()
            .max_by(|a, b| signed_area(a).abs().total_cmp(&signed_area(b).abs()));
        match outline {
            Some(mut o) if o.len() >= 3 => {
                if signed_area(&o) < 0.0 {
                    o.reverse();
                }
                o
            }
            _ => hull(&parts.concat()),
        }
    }

    /// The shaft opening of a stair by sketch: the convex hull of its footprint, grown by
    /// [`OPENING_CLEARANCE`] on every side.
    fn sketch_opening(&self) -> Vec<[f64; 2]> {
        let h = hull(&self.sketch_footprint());
        let n = h.len().max(1) as f64;
        let c = h
            .iter()
            .fold([0.0, 0.0], |a, p| [a[0] + p[0] / n, a[1] + p[1] / n]);
        h.iter()
            .map(|p| {
                let d = [p[0] - c[0], p[1] - c[1]];
                let l = d[0].hypot(d[1]).max(1.0e-9);
                [
                    p[0] + d[0] / l * OPENING_CLEARANCE,
                    p[1] + d[1] / l * OPENING_CLEARANCE,
                ]
            })
            .collect()
    }

    /// The side profile area of a run times the width: the concrete of the run, in cubic meters.
    fn run_volume(&self, run: &Run) -> f64 {
        signed_area(&profile(
            run.risers,
            self.riser,
            self.stair.tread,
            self.stair.waist,
        ))
        .abs()
            * self.stair.width
    }

    /// The thickness of the landing slab: the waist, and at least a riser, so that its edge is
    /// the last riser of the first run.
    #[must_use]
    pub fn landing_thickness(&self) -> f64 {
        self.stair.waist.max(self.riser)
    }

    /// The volume of the concrete: the runs and the landing slab, in cubic meters.
    #[must_use]
    pub fn volume(&self) -> f64 {
        let landing = (signed_area(&self.landing).abs()
            + self
                .landings
                .iter()
                .map(|(p, _)| signed_area(p).abs())
                .sum::<f64>())
            * self.landing_thickness();
        self.runs.iter().map(|r| self.run_volume(r)).sum::<f64>() + landing
    }

    /// The solid of a run, its base at the elevation `base` of the stair's base level.
    ///
    /// # Errors
    /// The solid kernel's errors.
    pub fn run_solid(&self, run: &Run, base: f64) -> Result<Polyhedron> {
        let tolerance = Tolerance::DEFAULT;
        let h = self.stair.width / 2.0;
        let left = [run.start[0] - run.dir[1] * h, run.start[1] + run.dir[0] * h];
        // The frame's normal (along x up) points to the right of the run: the extrusion runs
        // from the left edge across the width.
        let frame = PlaneFrame::new(
            Point3::new(left[0], left[1], base + run.base),
            Vector3::new(run.dir[0], run.dir[1], 0.0),
            Vector3::z(),
            &tolerance,
        )?;
        extrude(
            &region(&profile(
                run.risers,
                self.riser,
                self.stair.tread,
                self.stair.waist,
            ))?,
            &frame,
            self.stair.width,
            CHORD,
            &tolerance,
            &Limits::DEFAULT,
        )
    }

    /// The landing slab, its top at the landing; `None` for a straight stair.
    ///
    /// # Errors
    /// The solid kernel's errors.
    pub fn landing_solid(&self, base: f64) -> Option<Result<Polyhedron>> {
        if self.landing.is_empty() {
            return None;
        }
        Some(region(&self.landing).and_then(|r| {
            arc_geom::prism(
                &r,
                base + self.landing_top - self.landing_thickness(),
                self.landing_thickness(),
                CHORD,
                &Tolerance::DEFAULT,
                &Limits::DEFAULT,
            )
        }))
    }

    /// The solids of the stair's concrete: the runs, then the landing.
    ///
    /// # Errors
    /// The solid kernel's errors.
    pub fn solids(&self, base: f64) -> Result<Vec<Polyhedron>> {
        let mut out = Vec::new();
        for run in &self.runs {
            out.push(self.run_solid(run, base)?);
        }
        if let Some(landing) = self.landing_solid(base) {
            out.push(landing?);
        }
        for (corners, top) in &self.landings {
            let t = self.landing_thickness();
            out.push(region(corners).and_then(|r| {
                arc_geom::prism(
                    &r,
                    base + top - t,
                    t,
                    CHORD,
                    &Tolerance::DEFAULT,
                    &Limits::DEFAULT,
                )
            })?);
        }
        Ok(out)
    }

    /// The paths of the top rails `[left, right]` (seen walking up), as points `[x, y, z]` with
    /// `z` above the base; an empty path for a side without a railing.
    #[must_use]
    pub fn rails(&self) -> [Vec<[f64; 3]>; 2] {
        if self.stair.shape == Shape::Sketch {
            return self.sketch_rails();
        }
        let (w, h, e) = (self.stair.width, self.stair.width / 2.0, RAIL_INSET);
        let (r, hh) = (self.riser, RAIL_HEIGHT);
        let l0 = self.runs[0].length;
        let l1 = self.runs.get(1).map_or(0.0, |run| run.length);
        let z1 = self.landing_top;
        let top = self.height;
        let g = U_GAP;
        // In the frame: the side toward the turn (inner) and the other side (outer).
        let (inner, outer): (Vec<[f64; 3]>, Vec<[f64; 3]>) = match self.stair.shape {
            Shape::Straight | Shape::Sketch => (
                vec![[0.0, h - e, r + hh], [l0, h - e, top + hh]],
                vec![[0.0, -(h - e), r + hh], [l0, -(h - e), top + hh]],
            ),
            Shape::LTurn => (
                vec![
                    [0.0, h - e, r + hh],
                    [l0, h - e, z1 + hh],
                    [l0 + e, h - e, z1 + hh],
                    [l0 + e, h, z1 + r + hh],
                    [l0 + e, h + l1, top + hh],
                ],
                vec![
                    [0.0, -(h - e), r + hh],
                    [l0, -(h - e), z1 + hh],
                    [l0 + w - e, -(h - e), z1 + hh],
                    [l0 + w - e, h, z1 + hh],
                    [l0 + w - e, h, z1 + r + hh],
                    [l0 + w - e, h + l1, top + hh],
                ],
            ),
            Shape::UTurn => (
                vec![
                    [0.0, h - e, r + hh],
                    [l0, h - e, z1 + hh],
                    [l0 + e, h - e, z1 + hh],
                    [l0 + e, h + g + e, z1 + hh],
                    [l0, h + g + e, z1 + r + hh],
                    [l0 - l1, h + g + e, top + hh],
                ],
                vec![
                    [0.0, -(h - e), r + hh],
                    [l0, -(h - e), z1 + hh],
                    [l0 + w - e, -(h - e), z1 + hh],
                    [l0 + w - e, 3.0 * h + g - e, z1 + hh],
                    [l0, 3.0 * h + g - e, z1 + hh],
                    [l0, 3.0 * h + g - e, z1 + r + hh],
                    [l0 - l1, 3.0 * h + g - e, top + hh],
                ],
            ),
        };
        let place = |path: Vec<[f64; 3]>| -> Vec<[f64; 3]> {
            path.into_iter()
                .map(|p| {
                    let q = self.frame.at(p[0], p[1]);
                    [q[0], q[1], p[2]]
                })
                .collect()
        };
        // A right turn is the mirror image: its inner side is on the right.
        let (left, right) = if self.frame.mirrored {
            (place(outer), place(inner))
        } else {
            (place(inner), place(outer))
        };
        let pick = |on: bool, path: Vec<[f64; 3]>| if on { path } else { Vec::new() };
        [
            pick(self.stair.railings[0], left),
            pick(self.stair.railings[1], right),
        ]
    }

    /// The top rails of a stair by sketch: along each run at the inset of each side, the rail of
    /// one run continuing into the next at the landing.
    fn sketch_rails(&self) -> [Vec<[f64; 3]>; 2] {
        let (h, e, hh) = (self.stair.width / 2.0, RAIL_INSET, RAIL_HEIGHT);
        let side = |c: f64| -> Vec<[f64; 3]> {
            let mut path = Vec::new();
            for run in &self.runs {
                let n = [-run.dir[1], run.dir[0]];
                let at = |a: f64, z: f64| {
                    [
                        run.start[0] + run.dir[0] * a + n[0] * c,
                        run.start[1] + run.dir[1] * a + n[1] * c,
                        z,
                    ]
                };
                path.push(at(0.0, run.base + self.riser + hh));
                path.push(at(
                    run.length,
                    run.base + run.risers as f64 * self.riser + hh,
                ));
            }
            path
        };
        let pick = |on: bool, path: Vec<[f64; 3]>| if on { path } else { Vec::new() };
        [
            pick(self.stair.railings[0], side(h - e)),
            pick(self.stair.railings[1], side(-(h - e))),
        ]
    }

    /// The posts of a rail path: at its ends, at its corners and between them at most
    /// [`POST_SPACING`] apart, each as its foot and its top (`z` above the base); posts closer
    /// than a centimeter in plan are one.
    #[must_use]
    pub fn posts(path: &[[f64; 3]]) -> Vec<([f64; 3], [f64; 3])> {
        let mut tops: Vec<[f64; 3]> = Vec::new();
        for (k, pair) in path.windows(2).enumerate() {
            let (p, q) = (pair[0], pair[1]);
            let span = (q[0] - p[0]).hypot(q[1] - p[1]);
            if k == 0 {
                tops.push(p);
            }
            // The count is small: the path is a few meters long.
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a small whole number"
            )]
            let parts = ((span / POST_SPACING).ceil() as usize).clamp(1, 1000);
            for i in 1..=parts {
                let f = i as f64 / parts as f64;
                tops.push([
                    p[0] + (q[0] - p[0]) * f,
                    p[1] + (q[1] - p[1]) * f,
                    p[2] + (q[2] - p[2]) * f,
                ]);
            }
        }
        let mut out: Vec<([f64; 3], [f64; 3])> = Vec::new();
        for t in tops {
            if out
                .iter()
                .any(|(_, o)| (o[0] - t[0]).hypot(o[1] - t[1]) < 0.01)
            {
                continue;
            }
            out.push(([t[0], t[1], t[2] - RAIL_HEIGHT], t));
        }
        out
    }

    /// The solids of the railings: a bar along each piece of each top rail and the posts.
    ///
    /// # Errors
    /// The solid kernel's errors.
    pub fn railing_solids(&self, base: f64) -> Result<Vec<Polyhedron>> {
        let mut out = Vec::new();
        for path in self.rails() {
            out.extend(Self::rail_solids(&path, base)?);
        }
        Ok(out)
    }

    /// The solids of one rail from [`Layout::rails`]: a bar along each piece of the top rail and
    /// the posts, the base level at the elevation `base`; none for an empty path.
    ///
    /// # Errors
    /// The solid kernel's errors.
    pub fn rail_solids(path: &[[f64; 3]], base: f64) -> Result<Vec<Polyhedron>> {
        let lift = |p: [f64; 3]| [p[0], p[1], p[2] + base];
        let mut out = Vec::new();
        for pair in path.windows(2) {
            let (p, q) = (lift(pair[0]), lift(pair[1]));
            let gap =
                ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2)).sqrt();
            if gap > 1.0e-3 {
                out.push(bar(p, q, RAIL_SIZE)?);
            }
        }
        for (foot, top) in Self::posts(path) {
            out.push(bar(lift(foot), lift(top), POST_SIZE)?);
        }
        Ok(out)
    }

    /// The plan drawing of the stair, cut at `cut` above the base.
    #[must_use]
    pub fn plan(&self, cut: f64) -> Plan {
        let h = self.stair.width / 2.0;
        let mut treads = Vec::new();
        let mut cut_line = None;
        for run in &self.runs {
            let n = [-run.dir[1], run.dir[0]];
            let at = |a: f64, c: f64| {
                [
                    run.start[0] + run.dir[0] * a + n[0] * c,
                    run.start[1] + run.dir[1] * a + n[1] * c,
                ]
            };
            for i in 0..run.risers {
                let a = i as f64 * self.stair.tread;
                let above = run.base + (i + 1) as f64 * self.riser > cut + 1.0e-9;
                treads.push(([at(a, -h), at(a, h)], above));
            }
            // Where the line of the nosings reaches the cut, drawn as a break line.
            let top = run.base + run.risers as f64 * self.riser;
            if cut_line.is_none() && top > cut + 1.0e-9 {
                let a = ((cut - run.base - self.riser) * self.stair.tread / self.riser)
                    .clamp(0.0, run.length);
                let k = 0.25 * self.stair.width;
                let (p, q) = (at(a - k, -h), at(a + k, h));
                let d = [q[0] - p[0], q[1] - p[1]];
                let len = d[0].hypot(d[1]).max(1.0e-9);
                let (t, m) = (
                    [d[0] / len, d[1] / len],
                    [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0],
                );
                let nn = [-t[1], t[0]];
                let off =
                    |s: f64, o: f64| [m[0] + t[0] * s + nn[0] * o, m[1] + t[1] * s + nn[1] * o];
                cut_line = Some(vec![
                    p,
                    off(-0.06, 0.0),
                    off(-0.03, 0.08),
                    off(0.03, -0.08),
                    off(0.06, 0.0),
                    q,
                ]);
            }
        }
        let mut walk: Vec<[f64; 2]> = vec![self.runs[0].start];
        let l0 = self.runs[0].length;
        let w = self.stair.width;
        match self.stair.shape {
            Shape::Sketch => {
                walk.clear();
                for run in &self.runs {
                    walk.push(run.start);
                    walk.push([
                        run.start[0] + run.dir[0] * run.length,
                        run.start[1] + run.dir[1] * run.length,
                    ]);
                }
            }
            Shape::Straight => walk.push(self.frame.at(l0, 0.0)),
            Shape::LTurn => {
                walk.push(self.frame.at(l0 + h, 0.0));
            }
            Shape::UTurn => {
                walk.push(self.frame.at(l0 + h, 0.0));
                walk.push(self.frame.at(l0 + h, w + U_GAP));
            }
        }
        if let Some(run) = self.runs.get(1)
            && self.stair.shape != Shape::Sketch
        {
            walk.push([
                run.start[0] + run.dir[0] * run.length,
                run.start[1] + run.dir[1] * run.length,
            ]);
        }
        Plan {
            outline: std::iter::once(self.footprint()).collect(),
            treads,
            cut: cut_line,
            arrow: arrow(&walk),
            walk,
        }
    }

    /// The plan drawing on the level the stair arrives at, cut at `cut` above the stair's base
    /// (the cut of the level below): what lies above that cut (the top risers) is drawn normal and
    /// the rest lighter, the shaft opening is outlined, and the walking line runs from the
    /// arrival down to the foot with its arrow there.
    #[must_use]
    pub fn plan_above(&self, cut: f64) -> Plan {
        let mut plan = self.plan(cut);
        for (_, above) in &mut plan.treads {
            *above = !*above;
        }
        plan.outline.push(self.opening());
        plan.walk.reverse();
        plan.arrow = arrow(&plan.walk);
        plan
    }

    /// Where the walking line arrives: the end of the last run, and its direction.
    #[must_use]
    pub fn arrival(&self) -> ([f64; 2], [f64; 2]) {
        let run = self.runs[self.runs.len() - 1];
        (
            [
                run.start[0] + run.dir[0] * run.length,
                run.start[1] + run.dir[1] * run.length,
            ],
            run.dir,
        )
    }
}

/// The arrow head at the end of a walking line.
fn arrow(walk: &[[f64; 2]]) -> [[f64; 2]; 3] {
    let n = walk.len();
    let (end, before) = (walk[n - 1], walk[n.saturating_sub(2)]);
    let d = [end[0] - before[0], end[1] - before[1]];
    let len = d[0].hypot(d[1]).max(1.0e-9);
    let t = [d[0] / len, d[1] / len];
    let size = (0.25_f64).min(len / 2.0);
    let (s, c) = (25.0_f64.to_radians().sin(), 25.0_f64.to_radians().cos());
    let back = |sign: f64| {
        [
            end[0] - size * (t[0] * c - sign * t[1] * s),
            end[1] - size * (t[1] * c + sign * t[0] * s),
        ]
    };
    [back(1.0), end, back(-1.0)]
}

/// The plan drawing of a stair: lines in plan, the drawing layer chooses the style.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// The outline loops.
    pub outline: Vec<Vec<[f64; 2]>>,
    /// The tread lines (one per riser), each with whether it lies above the cut.
    pub treads: Vec<([[f64; 2]; 2], bool)>,
    /// The break line where the stair is cut, when the stair rises above the cut.
    pub cut: Option<Vec<[f64; 2]>>,
    /// The walking line from the first riser to the arrival.
    pub walk: Vec<[f64; 2]>,
    /// The arrow head at the end of the walking line.
    pub arrow: [[f64; 2]; 3],
}

/// A stair as the document places it.
#[derive(Debug, Clone)]
pub struct Placed {
    /// The record's identifier.
    pub id: ObjectId,
    /// The authored data.
    pub stair: Stair,
    /// The elevation of the base level.
    pub base: f64,
    /// The level the stair arrives at, when there is one.
    pub top_level: Option<ObjectId>,
    /// The layout, or why there is none.
    pub layout: std::result::Result<Layout, Refusal>,
}

impl Placed {
    /// Reads the stair `object` of `snapshot`; `None` when the record is unreadable.
    #[must_use]
    pub fn of(snapshot: &Snapshot, object: &Object) -> Option<Self> {
        let stair = Stair::from_values(&object.values)?;
        let storey = object
            .header
            .owner
            .and_then(|id| spatial::get(snapshot, id));
        let base = storey
            .as_ref()
            .map_or(0.0, |s| spatial::storey_elevation(snapshot, s));
        let top = match stair.top_level {
            Some(id) => spatial::get(snapshot, id),
            None => storey
                .as_ref()
                .and_then(|s| spatial::level_above(snapshot, s)),
        };
        let elevation = top
            .as_ref()
            .and_then(|l| types::meters(&l.values, level::ELEVATION));
        let layout = match elevation {
            Some(e) => Layout::build(&stair, &run_lines(&object.values), e - base),
            None => Err(Refusal::NoTop),
        };
        Some(Self {
            id: object.id,
            stair,
            base,
            top_level: top.map(|l| l.id),
            layout,
        })
    }
}

impl Placed {
    /// The plan outline: the footprint, or a small square at the start for a stair without a
    /// layout, so that it can still be picked, selected and deleted.
    #[must_use]
    pub fn outline(&self) -> Vec<[f64; 2]> {
        match &self.layout {
            Ok(layout) => layout.footprint(),
            Err(_) => {
                let (p, r) = (self.stair.start, 0.3);
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

/// Whether two plan outlines share an area of more than a square centimeter.
#[must_use]
pub fn outlines_overlap(a: &[[f64; 2]], b: &[[f64; 2]]) -> bool {
    let (Ok(ra), Ok(rb)) = (region(a), region(b)) else {
        return false;
    };
    match arc_geom::boolean(
        &ra,
        &rb,
        arc_geom::BooleanOp::Intersection,
        &Tolerance::DEFAULT,
        &Limits::DEFAULT,
    ) {
        Ok(Some(common)) => {
            common
                .parts()
                .iter()
                .map(|p| {
                    p.outer().signed_area().abs()
                        - p.holes().iter().map(|h| h.signed_area().abs()).sum::<f64>()
                })
                .sum::<f64>()
                > 1.0e-4
        }
        _ => false,
    }
}

/// The stairs of `snapshot` that arrive at the level `level`, with their layouts.
#[must_use]
pub fn arriving_at(snapshot: &Snapshot, level: ObjectId) -> Vec<Layout> {
    spatial::objects(snapshot, types::STAIR)
        .iter()
        .take(crate::room::MAX_ROOMS)
        .filter_map(|o| Placed::of(snapshot, o))
        .filter(|p| p.top_level == Some(level))
        .filter_map(|p| p.layout.ok())
        .collect()
}

/// The shaft openings a floor gets: those of the stairs arriving at the level of its storey.
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
    arriving_at(snapshot, level.id)
        .iter()
        .map(Layout::opening)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stair(shape: Shape, turn: Turn) -> Stair {
        Stair {
            shape,
            turn,
            start: [0.0, 0.0],
            direction: 0.0,
            width: 1.0,
            tread: 0.28,
            max_riser: 0.18,
            waist: 0.15,
            railings: [true, true],
            top_level: None,
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1.0e-9
    }

    #[test]
    fn three_meters_with_eighteen_centimeter_risers_take_seventeen_risers() {
        let layout = Layout::new(&stair(Shape::Straight, Turn::Left), 3.0).unwrap();
        assert_eq!(layout.risers, 17);
        assert!(close(layout.riser, 3.0 / 17.0));
        assert!((layout.riser - 0.17647).abs() < 1.0e-5);
        assert!(close(layout.runs[0].length, 16.0 * 0.28));
        // 2 x 0.17647 + 0.28 = 0.633: in the comfortable range.
        assert!(layout.stride_ok());
        // An exact multiple needs no extra riser: 2.88 m / 0.18 m = 16.
        assert_eq!(
            Layout::new(&stair(Shape::Straight, Turn::Left), 2.88)
                .unwrap()
                .risers,
            16
        );
    }

    #[test]
    fn a_stride_outside_the_range_is_flagged_and_too_few_risers_are_refused() {
        let mut steep = stair(Shape::Straight, Turn::Left);
        steep.tread = 0.20;
        let layout = Layout::new(&steep, 3.0).unwrap();
        assert!(!layout.stride_ok(), "2 x 0.176 + 0.20 = 0.553");
        assert_eq!(
            Layout::new(&stair(Shape::LTurn, Turn::Left), 0.5),
            Err(Refusal::Risers { count: 3, least: 4 })
        );
        assert_eq!(
            Layout::new(&stair(Shape::Straight, Turn::Left), 0.0),
            Err(Refusal::NoTop)
        );
    }

    #[test]
    fn l_and_u_footprints_and_run_lengths_are_exact() {
        let l = Layout::new(&stair(Shape::LTurn, Turn::Left), 3.0).unwrap();
        assert_eq!((l.runs[0].risers, l.runs[1].risers), (9, 8));
        assert!(close(l.runs[0].length, 8.0 * 0.28));
        assert!(close(l.runs[1].length, 7.0 * 0.28));
        assert!(close(l.landing_top, 9.0 * 3.0 / 17.0));
        // The first run, the 1 x 1 landing and the second run going left (+y).
        let area = signed_area(&l.footprint());
        assert!(close(area, 2.24 + 1.0 + 1.96), "{area}");
        assert_eq!(l.runs[1].start, [2.24 + 0.5, 0.5]);
        assert_eq!(l.runs[1].dir, [0.0, 1.0]);
        // A right turn is the mirror image across the first run.
        let r = Layout::new(&stair(Shape::LTurn, Turn::Right), 3.0).unwrap();
        assert!(close(signed_area(&r.footprint()), area));
        assert_eq!(r.runs[1].start, [2.74, -0.5]);
        assert_eq!(r.runs[1].dir, [0.0, -1.0]);

        let u = Layout::new(&stair(Shape::UTurn, Turn::Left), 3.0).unwrap();
        // A U stair's lower run takes the smaller half: 8 + 9 risers.
        assert!(close(u.runs[0].length, 1.96) && close(u.runs[1].length, 2.24));
        // The landing spans both runs and the gap: 1 x 2.1.
        assert!(close(signed_area(&u.landing), 2.1));
        assert!(close(u.runs[1].start[0], 1.96) && close(u.runs[1].start[1], 1.1));
        assert_eq!(u.runs[1].dir, [-1.0, 0.0]);
        let area = signed_area(&u.footprint());
        assert!(close(area, 2.24 + 2.1 + 1.96), "{area}");
    }

    #[test]
    fn a_stair_by_sketch_with_one_line_is_the_straight_stair_and_two_lines_share_the_risers() {
        let sketch = Stair {
            shape: Shape::Sketch,
            ..stair(Shape::Straight, Turn::Left)
        };
        let line = |x0, y0, x1, y1| RunLine {
            from: [x0, y0],
            to: [x1, y1],
        };
        let one = Layout::build(&sketch, &[line(0.0, 0.0, 5.0, 0.0)], 3.0).unwrap();
        let straight = Layout::new(&stair(Shape::Straight, Turn::Left), 3.0).unwrap();
        assert_eq!(one.risers, straight.risers);
        assert!(close(one.runs[0].length, straight.runs[0].length));
        assert!(close(
            signed_area(&one.footprint()),
            signed_area(&straight.footprint())
        ));
        assert!(one.landings.is_empty());
        // Two lines of 3 m and 1 m: the longer run takes more risers; the landing is the hull
        // of the end of the first run and the start of the second.
        let two = Layout::build(
            &sketch,
            &[line(0.0, 0.0, 3.0, 0.0), line(3.5, 0.5, 3.5, 1.5)],
            3.0,
        )
        .unwrap();
        assert_eq!(two.runs.iter().map(|r| r.risers).sum::<usize>(), 17);
        assert!(two.runs[0].risers > two.runs[1].risers);
        assert_eq!(two.landings.len(), 1);
        assert!(close(
            two.runs[1].base,
            two.runs[0].risers as f64 * two.riser
        ));
        let solids: f64 = two
            .solids(0.0)
            .unwrap()
            .iter()
            .map(Polyhedron::volume)
            .sum();
        assert!((solids - two.volume()).abs() < 1.0e-6);
        // A stair that cannot give each run two risers is refused.
        assert_eq!(
            Layout::build(
                &sketch,
                &[line(0.0, 0.0, 3.0, 0.0), line(3.5, 0.5, 3.5, 1.5)],
                0.5
            ),
            Err(Refusal::Risers { count: 3, least: 4 })
        );
        assert_eq!(Layout::build(&sketch, &[], 3.0), Err(Refusal::Malformed));
    }

    #[test]
    fn the_volume_is_the_profile_times_the_width_and_matches_the_solids() {
        for shape in [Shape::Straight, Shape::LTurn, Shape::UTurn] {
            let layout = Layout::new(&stair(shape, Turn::Right), 3.0).unwrap();
            let solids = layout.solids(3.0).unwrap();
            let total: f64 = solids.iter().map(Polyhedron::volume).sum();
            assert!((total - layout.volume()).abs() < 1.0e-6, "{shape:?}");
            assert!(!layout.railing_solids(3.0).unwrap().is_empty());
        }
        // A two-riser run by hand: the first step's rectangle less the triangle under the waist.
        let p = profile(2, 0.2, 0.25, 0.1);
        let drop = 0.1 * 0.25_f64.hypot(0.2) / 0.25;
        let top_end = 0.2 - drop;
        let foot = drop * 0.25 / 0.2;
        let expected = 0.25 * 0.2 - 0.5 * (0.25 - foot) * top_end;
        assert!(close(signed_area(&p), expected), "{}", signed_area(&p));
    }

    #[test]
    fn the_opening_keeps_the_arrival_edge_and_a_clearance_elsewhere() {
        let s = Layout::new(&stair(Shape::Straight, Turn::Left), 3.0).unwrap();
        let k = OPENING_CLEARANCE;
        assert!(close(
            signed_area(&s.opening()),
            (4.48 + k) * (1.0 + 2.0 * k)
        ));
        let u = Layout::new(&stair(Shape::UTurn, Turn::Left), 3.0).unwrap();
        assert!(close(
            signed_area(&u.opening()),
            (2.24 + 1.0 + k) * (2.1 + 2.0 * k)
        ));
    }

    #[test]
    fn outlines_that_share_area_overlap_and_touching_ones_do_not() {
        let sq = |x: f64| vec![[x, 0.0], [x + 1.0, 0.0], [x + 1.0, 1.0], [x, 1.0]];
        assert!(outlines_overlap(&sq(0.0), &sq(0.5)));
        assert!(!outlines_overlap(&sq(0.0), &sq(1.0)));
        assert!(!outlines_overlap(&sq(0.0), &sq(3.0)));
    }

    #[test]
    fn the_upper_plan_flips_the_cut_outlines_the_opening_and_walks_down() {
        let l = Layout::new(&stair(Shape::Straight, Turn::Left), 3.0).unwrap();
        let up = l.plan_above(CUT_HEIGHT);
        assert_eq!(up.treads.iter().filter(|(_, light)| *light).count(), 6);
        assert_eq!(up.outline.len(), 2);
        assert_eq!(up.walk.first(), Some(&[4.48, 0.0]));
        assert_eq!(up.arrow[1], [0.0, 0.0]);
        assert_eq!(l.arrival(), ([4.48, 0.0], [1.0, 0.0]));
    }

    #[test]
    fn railings_follow_the_sides_that_are_on() {
        let mut s = stair(Shape::LTurn, Turn::Left);
        s.railings = [false, true];
        let l = Layout::new(&s, 3.0).unwrap();
        let [left, right] = l.rails();
        assert!(left.is_empty());
        // The top rail starts 0.9 m above the first nosing and ends 0.9 m above the top.
        assert!(close(right[0][2], l.riser + RAIL_HEIGHT));
        assert!(close(right[right.len() - 1][2], 3.0 + RAIL_HEIGHT));
        let posts = Layout::posts(&right);
        assert!(posts.iter().all(|(f, t)| close(t[2] - f[2], RAIL_HEIGHT)));
        assert!(posts.len() >= 5);
    }

    #[test]
    fn the_plan_cuts_at_the_height_and_marks_what_lies_above() {
        let l = Layout::new(&stair(Shape::Straight, Turn::Left), 3.0).unwrap();
        let plan = l.plan(CUT_HEIGHT);
        assert_eq!(plan.treads.len(), 17);
        // Risers whose top is at most 1.2 m: 6 x 0.176 = 1.059; the seventh reaches 1.235.
        assert_eq!(plan.treads.iter().filter(|(_, above)| !above).count(), 6);
        assert!(plan.cut.is_some());
        assert_eq!(plan.walk.first(), Some(&[0.0, 0.0]));
        assert_eq!(plan.arrow[1], [4.48, 0.0]);
    }

    #[test]
    fn a_move_turns_the_direction_and_a_mirror_swaps_the_turn() {
        let s = stair(Shape::LTurn, Turn::Left);
        let turned = s.moved(&crate::transform::Xform::Rotate {
            base: [0.0, 0.0],
            angle: std::f64::consts::FRAC_PI_2,
        });
        assert!(close(turned.direction, std::f64::consts::FRAC_PI_2));
        let mirrored = s.moved(&crate::transform::Xform::mirror([0.0, 0.0], [1.0, 0.0]).unwrap());
        assert_eq!(mirrored.turn, Turn::Right);
        let a = Layout::new(&s, 3.0).unwrap().footprint();
        let b = Layout::new(&mirrored, 3.0).unwrap().footprint();
        let flipped: Vec<[f64; 2]> = a.iter().map(|p| [p[0], -p[1]]).collect();
        assert!(b.iter().all(|p| {
            flipped
                .iter()
                .any(|q| close(p[0], q[0]) && close(p[1], q[1]))
        }));
    }
}
