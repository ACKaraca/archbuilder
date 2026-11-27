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
//! Coordinates: `a` runs up the ramp from the foot, `c` across it to the left, `z` up from the
//! storey's level.

use arc_base::math::{Point3, Vector3};
use arc_base::{Limits, ObjectId, Result, Tolerance};
use arc_doc::{Object, Snapshot, Value};
use arc_geom::{Curve2, Loop2, PlaneFrame, Polyhedron, Region, RegionPart, Segment2, extrude};

use crate::spatial;
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
/// The chord of the solid kernel; ramps have no curves.
const CHORD: f64 = 0.01;

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

/// The authored data of a ramp, lengths in meters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ramp {
    /// The foot of the run.
    pub start: [f64; 2],
    /// The direction up the ramp, in radians from the x axis.
    pub direction: f64,
    /// The width.
    pub width: f64,
    /// The horizontal length of the sloped run.
    pub length: f64,
    /// The flat landing at the foot.
    pub landing_start: f64,
    /// The flat landing at the top.
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
}

impl Ramp {
    /// The ramp of a record's field values; `None` when a field is missing or malformed.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Option<Self> {
        let m = |at| types::meters(values, at);
        let direction = match values.get(ramp::DIRECTION)? {
            Some(Value::Quantity(q)) => q.value(),
            _ => return None,
        };
        let reference = |at: usize| match values.get(at) {
            Some(Some(Value::Reference(id))) => Some(*id),
            _ => None,
        };
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
        })
    }

    /// The field values of a record of this ramp named `name`.
    #[must_use]
    pub fn values(&self, name: &str) -> Vec<Option<Value>> {
        let mut values: Vec<Option<Value>> = vec![None; ramp::PINNED + 1];
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
        values
    }

    /// The ramp under the plane motion `xf`: the foot moved and the direction turned.
    #[must_use]
    pub fn moved(&self, xf: &crate::transform::Xform) -> Self {
        let (s, c) = crate::transform::sin_cos(self.direction);
        let start = xf.point(self.start);
        let ahead = xf.point([self.start[0] + c, self.start[1] + s]);
        Self {
            start,
            direction: (ahead[1] - start[1]).atan2(ahead[0] - start[0]),
            ..*self
        }
    }
}

/// Why a ramp has no layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// No level above the base and no rise: the ramp has no height.
    NoTop,
    /// A length is not finite or not positive.
    Malformed,
}

/// What a ramp derives from its data and its height.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// The authored data.
    pub ramp: Ramp,
    /// The rise.
    pub height: f64,
    /// The largest slope allowed.
    pub max_slope: f64,
    /// Whether the ramp arrives at the level above (and so cuts the floors of that level).
    pub arrives: bool,
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

/// The plan drawing of a ramp: lines in plan, the drawing layer chooses the style.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// The outline.
    pub outline: Vec<[f64; 2]>,
    /// The lines across the ramp where the slope starts and ends (one at each end of the run).
    pub ticks: Vec<[[f64; 2]; 2]>,
    /// The arrow line up the ramp, along its middle.
    pub walk: Vec<[f64; 2]>,
    /// The arrow head at the top end.
    pub arrow: [[f64; 2]; 3],
}

impl Layout {
    /// The layout of `ramp` for the level height `level_height` (used without a rise) and the
    /// type's largest slope.
    ///
    /// # Errors
    /// [`Refusal`].
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
        .all(|v| v.is_finite());
        if !finite
            || ramp.width <= 0.0
            || ramp.length <= 0.0
            || ramp.landing_start < 0.0
            || ramp.landing_end < 0.0
            || ramp.thickness <= 0.0
        {
            return Err(Refusal::Malformed);
        }
        let height = ramp.rise.or(level_height).unwrap_or(0.0);
        if !height.is_finite() || height < types::HEIGHT_RANGE.0 {
            return Err(Refusal::NoTop);
        }
        Ok(Self {
            ramp: *ramp,
            height,
            max_slope,
            arrives: ramp.rise.is_none(),
        })
    }

    /// The slope of the run: the rise over the length.
    #[must_use]
    pub fn slope(&self) -> f64 {
        self.height / self.ramp.length
    }

    /// Whether the slope passes the largest one of the type.
    #[must_use]
    pub fn exceeds(&self) -> bool {
        self.slope() > self.max_slope + 1.0e-9
    }

    /// The length of the run and both landings.
    #[must_use]
    pub fn total_length(&self) -> f64 {
        self.ramp.landing_start + self.ramp.length + self.ramp.landing_end
    }

    fn at(&self, a: f64, c: f64) -> [f64; 2] {
        let (s, co) = crate::transform::sin_cos(self.ramp.direction);
        [
            self.ramp.start[0] + co * a - s * c,
            self.ramp.start[1] + s * a + co * c,
        ]
    }

    /// The plan footprint, counterclockwise.
    #[must_use]
    pub fn footprint(&self) -> Vec<[f64; 2]> {
        let h = self.ramp.width / 2.0;
        let t = self.total_length();
        vec![
            self.at(0.0, -h),
            self.at(t, -h),
            self.at(t, h),
            self.at(0.0, h),
        ]
    }

    /// The shaft opening the floors of the level it arrives at get: the run and the landing at the
    /// foot with [`OPENING_CLEARANCE`] around them, up to the arrival edge. Empty for a ramp with
    /// a rise of its own.
    #[must_use]
    pub fn opening(&self) -> Vec<[f64; 2]> {
        if !self.arrives {
            return Vec::new();
        }
        let (h, k) = (self.ramp.width / 2.0 + OPENING_CLEARANCE, OPENING_CLEARANCE);
        let end = self.ramp.landing_start + self.ramp.length;
        vec![
            self.at(-k, -h),
            self.at(end, -h),
            self.at(end, h),
            self.at(-k, h),
        ]
    }

    /// The top surface along the run as `(a, z)` points: the foot landing, the slope, the top
    /// landing; points that coincide are one.
    fn surface(&self) -> Vec<[f64; 2]> {
        let (ls, l, le, h) = (
            self.ramp.landing_start,
            self.ramp.length,
            self.ramp.landing_end,
            self.height,
        );
        let mut out = vec![[0.0, 0.0]];
        for p in [[ls, 0.0], [ls + l, h], [ls + l + le, h]] {
            if out
                .last()
                .is_some_and(|q| (q[0] - p[0]).abs() + (q[1] - p[1]).abs() > 1.0e-9)
            {
                out.push(p);
            }
        }
        out
    }

    /// The side profile in `(along, up)`, counterclockwise.
    fn profile(&self) -> Vec<[f64; 2]> {
        let top = self.surface();
        let t = self.ramp.thickness;
        let bottom: Vec<[f64; 2]> = match self.ramp.form {
            Form::Slab => top.iter().rev().map(|p| [p[0], p[1] - t]).collect(),
            Form::Solid => {
                let (first, last) = (top[0][0], top[top.len() - 1][0]);
                vec![[last, -t], [first, -t]]
            }
        };
        let mut out = top;
        out.extend(bottom);
        out.dedup_by(|a, b| (a[0] - b[0]).abs() + (a[1] - b[1]).abs() < 1.0e-9);
        if crate::floor::signed_area(&out) < 0.0 {
            out.reverse();
        }
        out
    }

    /// The volume of the body, in cubic meters.
    #[must_use]
    pub fn volume(&self) -> f64 {
        crate::floor::signed_area(&self.profile()).abs() * self.ramp.width
    }

    /// The solid of the body, its level at the elevation `base`.
    ///
    /// # Errors
    /// The solid kernel's errors.
    pub fn solid(&self, base: f64) -> Result<Polyhedron> {
        let tolerance = Tolerance::DEFAULT;
        let h = self.ramp.width / 2.0;
        let left = self.at(0.0, h);
        let (s, c) = crate::transform::sin_cos(self.ramp.direction);
        let frame = PlaneFrame::new(
            Point3::new(left[0], left[1], base),
            Vector3::new(c, s, 0.0),
            Vector3::z(),
            &tolerance,
        )?;
        extrude(
            &region(&self.profile())?,
            &frame,
            self.ramp.width,
            CHORD,
            &tolerance,
            &Limits::DEFAULT,
        )
    }

    /// The path of the walking surface along the left or the right edge, [`RAIL_INSET`] in from
    /// it, as `[x, y, z]` with `z` above the level; the path a hosted railing follows.
    #[must_use]
    pub fn side_path(&self, left: bool) -> Vec<[f64; 3]> {
        let c = (self.ramp.width / 2.0 - RAIL_INSET) * if left { 1.0 } else { -1.0 };
        self.surface()
            .into_iter()
            .map(|[a, z]| {
                let p = self.at(a, c);
                [p[0], p[1], z]
            })
            .collect()
    }

    /// The plan drawing.
    #[must_use]
    pub fn plan(&self) -> Plan {
        let h = self.ramp.width / 2.0;
        let (a0, a1) = (
            self.ramp.landing_start,
            self.ramp.landing_start + self.ramp.length,
        );
        let walk = vec![self.at(0.0, 0.0), self.at(self.total_length(), 0.0)];
        let (end, before) = (walk[1], walk[0]);
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
            ticks: vec![
                [self.at(a0, -h), self.at(a0, h)],
                [self.at(a1, -h), self.at(a1, h)],
            ],
            arrow: [back(1.0), end, back(-1.0)],
            walk,
        }
    }
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
        Some(Self {
            id: object.id,
            ramp,
            base,
            top_level: top.map(|l| l.id),
            layout: Layout::new(&ramp, height, max_slope),
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
            ),
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
}
