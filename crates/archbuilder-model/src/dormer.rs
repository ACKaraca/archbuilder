//! Dormers: a gable or shed dormer on one roof by footprint.
//!
//! The record holds the authored data only: the middle of the opening in plan, the direction the
//! front faces, the width across the front, the depth from the front into the roof, the pitch of
//! the dormer's own roof, the style, the height of the front wall's eave above the host roof and
//! the thickness of the walls and the small roof. The roof owns the record and the dormer goes
//! with it.
//!
//! Derived each time: the opening in the host roof (the rectangle width x depth, which the host
//! reads in [`crate::opening::cuts`]), and the parts of the assembly: the front wall (a pentagon
//! under a gable, a rectangle under a shed), the two cheek walls, which stand on the host roof and
//! reach the eave line (the shed's follow its roof), and the dormer roof (two planes for a gable,
//! one for a shed). The assembly ends at the depth of the opening; where the dormer meets the
//! host further back is not modeled.

use arc_base::math::Point3;
use arc_base::{Limits, Result, Tolerance};
use arc_doc::{Object, Snapshot, Value};
use arc_geom::{PolyFace, Polyhedron};

use crate::types::{self, dormer};

/// The range of the width, the depth and the wall height, in meters.
pub const SIZE_RANGE: (f64, f64) = (0.3, 20.0);
/// The range of the thickness of the walls and the roof, in meters.
pub const THICKNESS_RANGE: (f64, f64) = (0.02, 1.0);
/// The range of the pitch, in radians (1 to 80 degrees).
pub const PITCH_RANGE: (f64, f64) = (0.017_453_292_519_943_295, 1.396_263_401_595_463_7);
/// The width by default, in meters.
pub const DEFAULT_WIDTH: f64 = 1.5;
/// The depth by default, in meters.
pub const DEFAULT_DEPTH: f64 = 2.0;
/// The pitch by default, 30 degrees.
pub const DEFAULT_PITCH: f64 = std::f64::consts::FRAC_PI_6;
/// The wall height by default, in meters.
pub const DEFAULT_WALL_HEIGHT: f64 = 1.2;
/// The thickness by default, in meters.
pub const DEFAULT_THICKNESS: f64 = 0.15;

coded! {
    /// The style of a dormer.
    Style {
        /// Two roof planes meeting at a ridge across the middle.
        #[default]
        Gable = 0, "gable";
        /// One roof plane rising from the front toward the host roof.
        Shed = 1, "shed";
    }
}

/// The authored data of a dormer, lengths in meters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dormer {
    /// The middle of the opening in plan.
    pub center: [f64; 2],
    /// The direction the front faces, in radians from the x axis.
    pub direction: f64,
    /// The width across the front.
    pub width: f64,
    /// The depth from the front into the roof.
    pub depth: f64,
    /// The pitch of the dormer's roof.
    pub pitch: f64,
    /// The style.
    pub style: Style,
    /// The height of the front wall's eave above the host roof.
    pub wall_height: f64,
    /// The thickness of the walls and the roof.
    pub thickness: f64,
}

/// What a dormer part is, for the export and the drawings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartKind {
    /// The wall at the front.
    Front,
    /// A wall at a side.
    Cheek,
    /// A plane of the dormer's roof.
    Roof,
}

/// Why a dormer has no parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The record or its sizes are unreadable, or the host roof is.
    Malformed,
    /// The host roof does not cover the dormer's footprint.
    OffRoof,
    /// The eave lies below the host roof at the back: the wall height is too small for the depth.
    TooLow,
}

impl Dormer {
    /// The dormer of a record's field values; `None` when a field is missing or malformed.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Option<Self> {
        let m = |at| types::meters(values, at);
        let angle = |at| match values.get(at) {
            Some(Some(Value::Quantity(q))) => Some(q.value()),
            _ => None,
        };
        Some(Self {
            center: [m(dormer::X)?, m(dormer::Y)?],
            direction: angle(dormer::DIRECTION)?,
            width: m(dormer::WIDTH)?,
            depth: m(dormer::DEPTH)?,
            pitch: angle(dormer::PITCH)?,
            style: Style::of_field(values, dormer::STYLE),
            wall_height: m(dormer::WALL_HEIGHT)?,
            thickness: m(dormer::THICKNESS)?,
        })
    }

    /// The field values of a record of this dormer named `name`.
    #[must_use]
    pub fn values(&self, name: &str) -> Vec<Option<Value>> {
        let mut v: Vec<Option<Value>> = vec![None; dormer::THICKNESS + 1];
        v[dormer::NAME] = Some(Value::String(name.to_owned()));
        v[dormer::X] = Some(types::length(self.center[0]));
        v[dormer::Y] = Some(types::length(self.center[1]));
        v[dormer::DIRECTION] = Some(crate::stair::angle(self.direction));
        v[dormer::WIDTH] = Some(types::length(self.width));
        v[dormer::DEPTH] = Some(types::length(self.depth));
        v[dormer::PITCH] = Some(crate::stair::angle(self.pitch));
        v[dormer::STYLE] = Some(Value::Enum(self.style.code()));
        v[dormer::WALL_HEIGHT] = Some(types::length(self.wall_height));
        v[dormer::THICKNESS] = Some(types::length(self.thickness));
        v
    }

    /// Whether every size is finite and in its range.
    #[must_use]
    pub fn sizes_ok(&self) -> bool {
        let within = |v: f64, (lo, hi): (f64, f64)| v.is_finite() && v >= lo && v <= hi;
        self.center.iter().all(|v| v.is_finite())
            && self.direction.is_finite()
            && within(self.width, SIZE_RANGE)
            && within(self.depth, SIZE_RANGE)
            && within(self.wall_height, SIZE_RANGE)
            && within(self.pitch, PITCH_RANGE)
            && within(self.thickness, THICKNESS_RANGE)
            && self.thickness * 2.0 < self.width
            && self.thickness * 2.0 < self.depth
    }

    /// The point `along` the front direction and `across` to its left from the middle.
    fn at(&self, along: f64, across: f64) -> [f64; 2] {
        let (s, c) = crate::transform::sin_cos(self.direction);
        [
            self.center[0] + c * along - s * across,
            self.center[1] + s * along + c * across,
        ]
    }

    /// The opening in the host roof, counterclockwise.
    #[must_use]
    pub fn opening(&self) -> Vec<[f64; 2]> {
        let (d, w) = (self.depth / 2.0, self.width / 2.0);
        vec![
            self.at(-d, -w),
            self.at(d, -w),
            self.at(d, w),
            self.at(-d, w),
        ]
    }

    /// The dormer under the plane motion `xf`.
    #[must_use]
    pub fn moved(&self, xf: &crate::transform::Xform) -> Self {
        let (s, c) = crate::transform::sin_cos(self.direction);
        let center = xf.point(self.center);
        let ahead = xf.point([self.center[0] + c, self.center[1] + s]);
        Self {
            center,
            direction: (ahead[1] - center[1]).atan2(ahead[0] - center[0]),
            ..*self
        }
    }
}

/// A prism: the planar polygon `poly` swept by `offset`, closed; `None` when the kernel refuses.
fn thick(poly: &[[f64; 3]], offset: [f64; 3]) -> Result<Polyhedron> {
    let n = poly.len();
    let mut vertices: Vec<Point3> = poly.iter().map(|p| Point3::new(p[0], p[1], p[2])).collect();
    vertices.extend(
        poly.iter()
            .map(|p| Point3::new(p[0] + offset[0], p[1] + offset[1], p[2] + offset[2])),
    );
    let at = |k: usize| u32::try_from(k).unwrap_or(u32::MAX);
    let mut faces = vec![
        PolyFace::new((0..n).rev().map(at).collect()),
        PolyFace::new((n..2 * n).map(at).collect()),
    ];
    for k in 0..n {
        let j = (k + 1) % n;
        faces.push(PolyFace::new(vec![at(k), at(j), at(n + j), at(n + k)]));
    }
    let made = Polyhedron::new(
        vertices.clone(),
        faces.clone(),
        &Tolerance::DEFAULT,
        &Limits::DEFAULT,
    );
    if made.as_ref().is_ok_and(|s| s.volume() > 0.0) {
        return made;
    }
    let flipped = faces
        .into_iter()
        .map(|f| PolyFace::new(f.outer.into_iter().rev().collect()))
        .collect();
    Polyhedron::new(vertices, flipped, &Tolerance::DEFAULT, &Limits::DEFAULT)
}

/// What a dormer reads from a snapshot.
#[derive(Debug, Clone)]
pub struct Derived {
    /// The authored data.
    pub dormer: Dormer,
    /// The roof that owns it.
    pub roof: arc_base::ObjectId,
    /// The parts of the assembly.
    pub parts: Vec<(PartKind, Polyhedron)>,
}

impl Derived {
    /// Reads the dormer `object` of `snapshot`.
    ///
    /// # Errors
    /// The [`Refusal`] that names what is wrong.
    pub fn of(snapshot: &Snapshot, object: &Object) -> std::result::Result<Self, Refusal> {
        let dormer = Dormer::from_values(&object.values).ok_or(Refusal::Malformed)?;
        if !dormer.sizes_ok() {
            return Err(Refusal::Malformed);
        }
        let host = object
            .header
            .owner
            .and_then(|r| crate::spatial::get(snapshot, r))
            .ok_or(Refusal::Malformed)?;
        let roof = crate::roof::Derived::of(snapshot, &host).map_err(|_| Refusal::Malformed)?;
        let parts = build(&dormer, &roof.surface)?;
        Ok(Self {
            dormer,
            roof: host.id,
            parts,
        })
    }

    /// The volume of every part, in cubic meters.
    #[must_use]
    pub fn volume(&self) -> f64 {
        self.parts.iter().map(|(_, s)| s.volume().abs()).sum()
    }
}

fn build(
    d: &Dormer,
    surface: &crate::roof::Surface,
) -> std::result::Result<Vec<(PartKind, Polyhedron)>, Refusal> {
    let (a, w, t) = (d.depth / 2.0, d.width / 2.0, d.thickness);
    let top = |along: f64, across: f64| {
        let p = d.at(along, across);
        surface.top_at(p).ok_or(Refusal::OffRoof)
    };
    let p3 = |along: f64, across: f64, z: f64| {
        let p = d.at(along, across);
        [p[0], p[1], z]
    };
    let (zf_l, zf_r) = (top(a, -w)?, top(a, w)?);
    let (zb_l, zb_r) = (top(-a, -w)?, top(-a, w)?);
    let eave = zf_l.max(zf_r) + d.wall_height;
    let rise = 2.0 * a * d.pitch.tan();
    let gable = d.style == Style::Gable;
    let ridge = eave + w * d.pitch.tan();
    // A cheek must have height at its back end, where the host roof is highest.
    let back_top = if gable { eave } else { eave + rise };
    if back_top < zb_l.max(zb_r) + 0.05 {
        return Err(Refusal::TooLow);
    }
    let (s, c) = crate::transform::sin_cos(d.direction);
    let mut out = Vec::new();
    let mut add = |kind, poly: Vec<[f64; 3]>, off: [f64; 3]| {
        thick(&poly, off)
            .map(|solid| out.push((kind, solid)))
            .map_err(|_| Refusal::Malformed)
    };
    let mut front = vec![p3(a, -w, zf_l), p3(a, w, zf_r), p3(a, w, eave)];
    if gable {
        front.push(p3(a, 0.0, ridge));
    }
    front.push(p3(a, -w, eave));
    add(PartKind::Front, front, [-c * t, -s * t, 0.0])?;
    // The left cheek (across < 0) is offset inward, to the left; the right one to the right.
    for (side, zf, zb) in [(-1.0, zf_l, zb_l), (1.0, zf_r, zb_r)] {
        let poly = vec![
            p3(a, side * w, zf),
            p3(-a, side * w, zb),
            p3(-a, side * w, back_top),
            p3(a, side * w, eave),
        ];
        add(PartKind::Cheek, poly, [s * side * t, -c * side * t, 0.0])?;
    }
    let up = [0.0, 0.0, t];
    if gable {
        for side in [-1.0, 1.0] {
            let poly = vec![
                p3(a, side * w, eave),
                p3(a, 0.0, ridge),
                p3(-a, 0.0, ridge),
                p3(-a, side * w, eave),
            ];
            add(PartKind::Roof, poly, up)?;
        }
    } else {
        let poly = vec![
            p3(a, -w, eave),
            p3(a, w, eave),
            p3(-a, w, eave + rise),
            p3(-a, -w, eave + rise),
        ];
        add(PartKind::Roof, poly, up)?;
    }
    Ok(out)
}
