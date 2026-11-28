//! The wall: authored data read from a record, and the solid derived from it.
//!
//! A wall record holds a baseline (two points in project coordinates), a thickness, a height, a
//! base offset and a location line, and belongs to a storey. No geometry is stored: [`Wall::solid`]
//! derives it each time it is needed, from the elevation of the storey's level.
//!
//! The side of a wall is named from the baseline's direction: the left of the direction from the
//! start point to the end point is the exterior face, the right is the interior face. The
//! location line says which line of the wall the baseline is (see [`LocationLine`]), so changing
//! it moves the baseline: the physical position stays (see [`Wall::with_location`]).
//!
//! Joins are derived and never stored ([`crate::join`]): the ends of walls that meet are closed
//! against each other when the solids are made, from the walls of the storey. The only authored
//! part is the choice of each end, [`JoinStyle`]. [`Wall::solid`] is the solid of the wall alone
//! (square ends); [`Wall::solid_in`] takes the joined footprint that [`crate::join::footprints`]
//! derives.

use arc_base::math::Point2;
use arc_base::{Limits, Result, Tolerance};
use arc_doc::Value;
use arc_geom::{
    Curve2, FaceTag, Loop2, Polyhedron, Polyline2, PolylineSegment, PolylineVertex, Region,
    RegionPart, Segment2, prism,
};

use crate::types::{self, wall};

/// Which line of the wall the baseline is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LocationLine {
    /// The middle of the thickness.
    #[default]
    Center,
    /// The exterior face: the wall's body lies on the right of the baseline's direction.
    Exterior,
    /// The interior face: the wall's body lies on the left of the baseline's direction.
    Interior,
    /// The middle of the core layers of the wall's type.
    CoreCenter,
    /// The exterior face of the core.
    CoreExterior,
    /// The interior face of the core.
    CoreInterior,
}

impl LocationLine {
    /// Every location line, in code order.
    pub const ALL: [Self; 6] = [
        Self::Center,
        Self::Exterior,
        Self::Interior,
        Self::CoreCenter,
        Self::CoreExterior,
        Self::CoreInterior,
    ];

    /// The name of the variant in the wall record.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Center => "center",
            Self::Exterior => "exterior",
            Self::Interior => "interior",
            Self::CoreCenter => "core_center",
            Self::CoreExterior => "core_exterior",
            Self::CoreInterior => "core_interior",
        }
    }

    /// The code stored in the wall record.
    #[must_use]
    pub const fn code(self) -> u32 {
        match self {
            Self::Center => 0,
            Self::Exterior => 1,
            Self::Interior => 2,
            Self::CoreCenter => 3,
            Self::CoreExterior => 4,
            Self::CoreInterior => 5,
        }
    }

    /// The location line of a stored code; the default for a code that names none.
    #[must_use]
    pub fn from_code(code: u32) -> Self {
        Self::ALL
            .into_iter()
            .find(|l| l.code() == code)
            .unwrap_or_default()
    }
}

/// How one end of a wall takes part in a join; the only authored part of a join.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JoinStyle {
    /// The default: walls meeting at their ends close the corner (a miter for two walls), and a
    /// wall ending on another wall stops at its face.
    #[default]
    Auto,
    /// This end stops at the face of the other wall, which runs through to the far face of this
    /// one. Where two walls meet and both ends say butt, the corner is mitered.
    Butt,
    /// Like [`Self::Butt`], with this end cut square across: it stops where its nearer corner
    /// touches the other wall.
    Square,
    /// The end is not joined: it stays square at the baseline's end and may overlap other walls.
    Disallowed,
}

impl JoinStyle {
    /// Every style, in code order.
    pub const ALL: [Self; 4] = [Self::Auto, Self::Butt, Self::Square, Self::Disallowed];

    /// The name of the variant in the wall record.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Butt => "butt",
            Self::Square => "square",
            Self::Disallowed => "disallowed",
        }
    }

    /// The code stored in the wall record.
    #[must_use]
    pub const fn code(self) -> u32 {
        match self {
            Self::Auto => 0,
            Self::Butt => 1,
            Self::Square => 2,
            Self::Disallowed => 3,
        }
    }

    /// The style of a stored code; the default for a code that names none.
    #[must_use]
    pub fn from_code(code: u32) -> Self {
        Self::ALL
            .into_iter()
            .find(|s| s.code() == code)
            .unwrap_or_default()
    }

    /// The style of a record's field; the default when the field is absent.
    fn of_field(values: &[Option<Value>], at: usize) -> Self {
        match values.get(at) {
            Some(Some(Value::Enum(code))) => Self::from_code(*code),
            _ => Self::default(),
        }
    }
}

/// The curve of the footprint loop that is the exterior face of the solid: the side on the left of
/// the baseline's direction. The footprint loop runs start right, end right, end left, start left.
pub const EXTERIOR_CURVE: u32 = 2;
/// The curve of the footprint loop that is the interior face of the solid.
pub const INTERIOR_CURVE: u32 = 0;
/// The footprint is not tessellated; the chord only has to be a positive number.
const CHORD: f64 = 0.01;

/// The tag of the exterior face of a wall solid.
pub const EXTERIOR_FACE: FaceTag = FaceTag::Side {
    loop_index: 0,
    curve: EXTERIOR_CURVE,
    facet: 0,
};
/// The distance of a wall record's top above its top level; 0 when absent.
#[must_use]
pub fn top_offset(values: &[Option<Value>]) -> f64 {
    types::meters(values, wall::TOP_OFFSET).unwrap_or(0.0)
}

/// The roof or floor a wall record's top is attached to.
#[must_use]
pub fn attachment(values: &[Option<Value>]) -> Option<arc_base::ObjectId> {
    match values.get(wall::TOP_ATTACH)? {
        Some(Value::Reference(id)) => Some(*id),
        _ => None,
    }
}

/// The solid of `wall` standing on a level at `level` over `footprint` (the joined corners, as
/// [`Wall::solid_in`] takes them) with its top on the underside of the roof `surface`: the
/// footprint is cut by the roof's cells and each piece rises to its cell's plane, so a gable
/// end gets its triangle and a wall under a hip its sloping top, across the thickness too; a
/// part outside the roof's outline keeps the wall's own top, with a vertical step where they
/// meet. The exterior and interior faces keep their tags.
///
/// # Errors
/// `ab.model.wall_attach_failed` (with the argument `name` left empty) when no part of the
/// footprint is under the roof or the underside comes down to the wall's base, and
/// the solid kernel's errors.
pub fn under_roof(
    wall: &Wall,
    footprint: [[f64; 2]; 4],
    level: f64,
    surface: &crate::roof::Surface,
) -> Result<Polyhedron> {
    // The side faces take the tag of the footprint edge they lie on.
    let side = |a: [f64; 2], b: [f64; 2]| {
        let middle = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
        let distance = |k: usize| {
            let (Some(p), Some(q)) = (footprint.get(k), footprint.get((k + 1) % 4)) else {
                return f64::INFINITY;
            };
            let d = [q[0] - p[0], q[1] - p[1]];
            let length = d[0].hypot(d[1]).max(f64::MIN_POSITIVE);
            ((middle[0] - p[0]) * d[1] - (middle[1] - p[1]) * d[0]).abs() / length
        };
        let curve = (0..4_u32)
            .min_by(|x, y| distance(*x as usize).total_cmp(&distance(*y as usize)))
            .unwrap_or(0);
        FaceTag::Side {
            loop_index: 0,
            curve,
            facet: 0,
        }
    };
    let triangles: Vec<Vec<[f64; 2]>> = crate::terrain::split_quad(footprint)
        .iter()
        .map(|t| t.to_vec())
        .collect();
    under_roof_in(wall, &triangles, level, surface, &side)
}

/// [`under_roof`] over a footprint given as convex pieces (triangles) in plan, and with the face
/// tags of the sides given by `side`; the layer solids of a wall use it.
///
/// # Errors
/// As [`under_roof`].
pub fn under_roof_in(
    wall: &Wall,
    footprint: &[Vec<[f64; 2]>],
    level: f64,
    surface: &crate::roof::Surface,
    side: &dyn Fn([f64; 2], [f64; 2]) -> FaceTag,
) -> Result<Polyhedron> {
    use crate::terrain::{self, Bottom, Piece, Plane, clip_convex, double_area};
    let tolerance = Tolerance::DEFAULT;
    let failed = || {
        arc_base::Error::new(crate::error::codes::WALL_ATTACH_FAILED)
            .with_arg("name", "")
            .with_arg("target", "")
    };
    let base = level + wall.base_offset;
    let cells = surface.pieces();
    let mut pieces = Vec::new();
    let mut covered = 0.0;
    for triangle in footprint {
        for cell in &cells {
            let part = clip_convex(triangle, &cell.points);
            let area = double_area(&part) / 2.0;
            if part.len() >= 3 && area > tolerance.linear * tolerance.linear {
                covered += area;
                pieces.push(Piece {
                    points: part,
                    plane: cell.plane,
                    upper: None,
                });
            }
        }
    }
    if covered <= tolerance.linear * tolerance.linear {
        return Err(failed());
    }
    let low = base + types::HEIGHT_RANGE.0;
    if pieces
        .iter()
        .any(|p| p.points.iter().any(|q| p.plane.z(*q) < low))
    {
        return Err(failed());
    }
    // The part outside the roof's outline keeps the wall's own top.
    let mut outside: Vec<Vec<[f64; 2]>> = footprint.to_vec();
    for cut in surface.outline_triangles() {
        outside = terrain::subtract_convex(outside, cut, tolerance.linear);
    }
    let own = Plane::level(base + wall.height);
    pieces.extend(outside.into_iter().map(|points| Piece {
        points,
        plane: own,
        upper: None,
    }));
    terrain::solid(&pieces, 0.0, Bottom::Level(base), side)
}

/// The tag of the interior face of a wall solid.
pub const INTERIOR_FACE: FaceTag = FaceTag::Side {
    loop_index: 0,
    curve: INTERIOR_CURVE,
    facet: 0,
};

/// The authored data of a wall, lengths in meters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wall {
    /// The baseline's start point.
    pub start: [f64; 2],
    /// The baseline's end point.
    pub end: [f64; 2],
    /// The thickness: the total of the type's layers.
    pub thickness: f64,
    /// The thickness of the type's layers outside the core: on the exterior side, then on the
    /// interior side. Zero for a wall that is all core.
    pub core: [f64; 2],
    /// The height above the base.
    pub height: f64,
    /// The base's distance above the storey's level.
    pub base_offset: f64,
    /// Which line of the wall the baseline is.
    pub location: LocationLine,
    /// How the start end joins.
    pub start_join: JoinStyle,
    /// How the end end joins.
    pub end_join: JoinStyle,
    /// The bulge of the baseline: the tangent of a quarter of the arc's sweep from the start to
    /// the end, positive counter-clockwise; 0 for a straight wall.
    pub bulge: f64,
    /// The lean of the exterior and the interior face from the vertical, in radians, positive
    /// when the top moves toward the left (exterior) side; `[0.0, 0.0]` for a vertical wall.
    pub lean: [f64; 2],
}

/// The largest lean of a wall face, in radians (60 degrees).
pub const MAX_LEAN: f64 = 1.047_197_551_196_597_6;
/// The most points of a wall's sketched profile.
pub const MAX_PROFILE_POINTS: usize = 256;
/// The most bands of one wall.
pub const MAX_BANDS: usize = 32;
/// The most facets an arc wall is divided into.
pub const MAX_FACETS: usize = 256;
/// The chord tolerance at which an arc wall is faceted, in meters.
pub const FACET_CHORD: f64 = 0.002;
/// The smallest radius a face of an arc wall may have, in meters.
pub const MIN_RADIUS: f64 = 0.05;
/// The thinnest a leaning wall may get at its top, in meters.
pub const MIN_TOP_THICKNESS: f64 = 0.01;

/// The circle of an arc wall's baseline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArcBase {
    /// The center of the circle.
    pub center: [f64; 2],
    /// The radius of the baseline.
    pub radius: f64,
    /// The angle of the start point about the center.
    pub first: f64,
    /// The signed sweep from the start to the end; positive is counter-clockwise.
    pub sweep: f64,
}

impl Wall {
    /// The wall of a record's field values; `None` when a field is missing.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Option<Self> {
        let m = |at| types::meters(values, at);
        let location = match values.get(wall::LOCATION_LINE)? {
            Some(Value::Enum(code)) => LocationLine::from_code(*code),
            _ => return None,
        };
        Some(Self {
            start: [m(wall::START_X)?, m(wall::START_Y)?],
            end: [m(wall::END_X)?, m(wall::END_Y)?],
            thickness: m(wall::THICKNESS)?,
            core: [
                m(wall::CORE_EXTERIOR).unwrap_or(0.0),
                m(wall::CORE_INTERIOR).unwrap_or(0.0),
            ],
            height: m(wall::HEIGHT)?,
            base_offset: m(wall::BASE_OFFSET)?,
            location,
            start_join: JoinStyle::of_field(values, wall::START_JOIN),
            end_join: JoinStyle::of_field(values, wall::END_JOIN),
            bulge: match values.get(wall::BULGE) {
                Some(Some(Value::F64(b))) if b.is_finite() => *b,
                _ => 0.0,
            },
            lean: [
                lean_of(values, wall::LEAN_EXTERIOR),
                lean_of(values, wall::LEAN_INTERIOR),
            ],
        })
    }

    /// The wall of a record with its top constraint applied: when the record refers to a level
    /// for its top, the height is that level's elevation minus the base (the storey's level
    /// plus the base offset); else, or when that would not be a valid height, the stored height.
    #[must_use]
    pub fn of(snapshot: &arc_doc::Snapshot, object: &arc_doc::Object) -> Option<Self> {
        let wall = Self::from_values(&object.values)?;
        let offset = top_offset(&object.values);
        let top = Self::top_level(&object.values)
            .and_then(|id| crate::spatial::get(snapshot, id))
            .and_then(|level| types::meters(&level.values, types::level::ELEVATION))
            .map(|e| e + offset);
        let storey = object
            .header
            .owner
            .and_then(|id| crate::spatial::get(snapshot, id))
            .map_or(0.0, |s| crate::spatial::storey_elevation(snapshot, &s));
        // A top attached to a floor stops at the floor's underside.
        let floor_top = attachment(&object.values)
            .and_then(|id| crate::spatial::get(snapshot, id))
            .filter(|o| crate::spatial::type_of(snapshot, o.id).as_deref() == Some(types::FLOOR))
            .and_then(|o| crate::floor::Derived::of(snapshot, &o))
            .map(|d| d.z_range().0);
        Some(wall.constrained(storey, floor_top.or(top)))
    }

    /// The wall with its top constrained to a level at `top_elevation` (when there is one): the
    /// height becomes that elevation minus the base, `storey_elevation` plus the base offset; a
    /// height that would not be valid leaves the stored one. This is the one derivation of a
    /// constrained height; [`Self::of`] and the command-line dump use it.
    #[must_use]
    pub fn constrained(mut self, storey_elevation: f64, top_elevation: Option<f64>) -> Self {
        if let Some(top) = top_elevation {
            let height = top - (storey_elevation + self.base_offset);
            if height.is_finite() && height >= types::HEIGHT_RANGE.0 {
                self.height = height;
            }
        }
        self
    }

    /// The levels a wall's top can follow: the levels of its building above its base, from the
    /// lowest up. The choice of the property grid numbers them from 1 (0 is unconnected).
    #[must_use]
    pub fn levels_above(
        snapshot: &arc_doc::Snapshot,
        object: &arc_doc::Object,
    ) -> Vec<std::sync::Arc<arc_doc::Object>> {
        let storey = object
            .header
            .owner
            .and_then(|id| crate::spatial::get(snapshot, id));
        let base = storey
            .as_ref()
            .map_or(0.0, |s| crate::spatial::storey_elevation(snapshot, s))
            + types::meters(&object.values, wall::BASE_OFFSET).unwrap_or(0.0);
        storey
            .and_then(|s| s.header.owner)
            .map(|building| crate::spatial::levels(snapshot, building))
            .unwrap_or_default()
            .into_iter()
            .filter(|l| {
                types::meters(&l.values, types::level::ELEVATION)
                    .is_some_and(|e| e - base >= types::HEIGHT_RANGE.0)
            })
            .collect()
    }

    /// The level a record's top is constrained to.
    #[must_use]
    pub fn top_level(values: &[Option<Value>]) -> Option<arc_base::ObjectId> {
        match values.get(wall::TOP_LEVEL)? {
            Some(Value::Reference(id)) => Some(*id),
            _ => None,
        }
    }

    /// The length of the baseline: along the arc for an arc wall.
    #[must_use]
    pub fn length(&self) -> f64 {
        self.arc()
            .map_or_else(|| self.chord(), |a| a.radius * a.sweep.abs())
    }

    /// The distance from the start point to the end point.
    #[must_use]
    pub fn chord(&self) -> f64 {
        (self.end[0] - self.start[0]).hypot(self.end[1] - self.start[1])
    }

    /// The circle of the baseline; `None` for a straight wall.
    #[must_use]
    pub fn arc(&self) -> Option<ArcBase> {
        let (center, sweep, radius) = crate::floor::arc_geometry(self.start, self.end, self.bulge)?;
        Some(ArcBase {
            center,
            radius,
            first: (self.start[1] - center[1]).atan2(self.start[0] - center[0]),
            sweep,
        })
    }

    /// Whether the baseline is an arc.
    #[must_use]
    pub fn is_arc(&self) -> bool {
        self.arc().is_some()
    }

    /// Whether a face leans from the vertical.
    #[must_use]
    pub fn leans(&self) -> bool {
        self.lean.iter().any(|a| a.abs() > 1e-12)
    }

    /// The point of the baseline at `distance` from the start, along the arc for an arc wall;
    /// the distance is not clamped.
    #[must_use]
    pub fn point_at(&self, distance: f64) -> [f64; 2] {
        match self.arc() {
            Some(a) => {
                let t = a.first + a.sweep * distance / (a.radius * a.sweep.abs());
                [
                    a.center[0] + a.radius * t.cos(),
                    a.center[1] + a.radius * t.sin(),
                ]
            }
            None => {
                let l = self.length().max(f64::MIN_POSITIVE);
                [
                    self.start[0] + (self.end[0] - self.start[0]) * distance / l,
                    self.start[1] + (self.end[1] - self.start[1]) * distance / l,
                ]
            }
        }
    }

    /// The unit direction of the baseline at `distance` from the start.
    #[must_use]
    pub fn tangent_at(&self, distance: f64) -> [f64; 2] {
        match self.arc() {
            Some(a) => {
                let t = a.first + a.sweep * distance / (a.radius * a.sweep.abs());
                let s = a.sweep.signum();
                [-t.sin() * s, t.cos() * s]
            }
            None => {
                let l = self.chord();
                if l > 0.0 {
                    [
                        (self.end[0] - self.start[0]) / l,
                        (self.end[1] - self.start[1]) / l,
                    ]
                } else {
                    [1.0, 0.0]
                }
            }
        }
    }

    /// The distance along the baseline of the point of it nearest to `p`, clamped to the wall.
    #[must_use]
    pub fn distance_of(&self, p: [f64; 2]) -> f64 {
        let length = self.length();
        match self.arc() {
            Some(a) => {
                let t = (p[1] - a.center[1]).atan2(p[0] - a.center[0]);
                // The angle from the start, taken in the direction of the sweep and wrapped.
                let mut d = (t - a.first) * a.sweep.signum();
                let turn = std::f64::consts::TAU;
                d = d.rem_euclid(turn);
                let along = d / a.sweep.abs();
                // A point past the end is nearer the end or the start, by angle.
                let beyond = if along > 1.0 {
                    let past = d - a.sweep.abs();
                    if past < turn - d { 1.0 } else { 0.0 }
                } else {
                    along
                };
                beyond * length
            }
            None => {
                let l = self.chord();
                if l <= 0.0 {
                    return 0.0;
                }
                let d = ((p[0] - self.start[0]) * (self.end[0] - self.start[0])
                    + (p[1] - self.start[1]) * (self.end[1] - self.start[1]))
                    / l;
                d.clamp(0.0, l)
            }
        }
    }

    /// The radius of the face at `side` across the baseline (positive toward the left) of an
    /// arc wall; `None` for a straight wall.
    #[must_use]
    pub fn radius_at(&self, side: f64) -> Option<f64> {
        let a = self.arc()?;
        Some(a.radius - side * a.sweep.signum())
    }

    /// Whether the faces of an arc wall have a usable radius; true for a straight wall.
    #[must_use]
    pub fn arc_fits(&self) -> bool {
        let (low, high) = self.extent();
        !self.bulge.is_finite()
            || self.arc().is_none()
            || [low, high]
                .iter()
                .all(|s| self.radius_at(*s).is_some_and(|r| r >= MIN_RADIUS))
    }

    /// The thickness of the wall at the height `z` above its base: it changes with the lean.
    #[must_use]
    pub fn thickness_at(&self, z: f64) -> f64 {
        self.thickness + z * (self.lean[0].tan() - self.lean[1].tan())
    }

    /// The sideways shift of a point at `across` (measured like [`Self::extent`]) at the height
    /// `z` above the wall's base, toward the left: the faces lean, the layers between follow in
    /// proportion.
    #[must_use]
    pub fn lean_shift(&self, across: f64, z: f64) -> f64 {
        let (low, high) = self.extent();
        let t = if high > low {
            ((across - low) / (high - low)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (ae, ai) = (self.lean[0].tan(), self.lean[1].tan());
        z * (ai + t * (ae - ai))
    }

    /// The pieces an arc wall is divided into: straight walls on the chords of its baseline, in
    /// order from the start, which join each other by miters; the ends keep the wall's join
    /// styles. A straight wall is its own only piece. The layers, joins, openings and plan
    /// drawing of an arc wall use these.
    #[must_use]
    pub fn facets(&self) -> Vec<Wall> {
        let Some(a) = self.arc() else {
            return vec![*self];
        };
        let (low, high) = self.extent();
        let reach = [low, high]
            .iter()
            .filter_map(|s| self.radius_at(*s))
            .fold(a.radius, f64::max)
            .max(MIN_RADIUS);
        let step = if FACET_CHORD >= reach {
            std::f64::consts::FRAC_PI_2
        } else {
            2.0 * (1.0 - FACET_CHORD / reach).acos()
        };
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped to 2 and the facet limit"
        )]
        let n = (a.sweep.abs() / step.max(1.0e-3)).ceil().clamp(
            2.0,
            f64::from(u16::try_from(MAX_FACETS).unwrap_or(u16::MAX)),
        ) as usize;
        let length = self.length();
        let at = |k: usize| -> [f64; 2] {
            if k == 0 {
                self.start
            } else if k == n {
                self.end
            } else {
                self.point_at(length * k as f64 / n as f64)
            }
        };
        (0..n)
            .map(|k| Wall {
                start: at(k),
                end: at(k + 1),
                bulge: 0.0,
                start_join: if k == 0 {
                    self.start_join
                } else {
                    JoinStyle::Auto
                },
                end_join: if k + 1 == n {
                    self.end_join
                } else {
                    JoinStyle::Auto
                },
                ..*self
            })
            .collect()
    }

    /// The distances along the baseline where the facets of [`Self::facets`] start, then the
    /// length; `[0, length]` for a straight wall.
    #[must_use]
    pub fn facet_marks(&self) -> Vec<f64> {
        let n = self.facets().len();
        let length = self.length();
        (0..=n).map(|k| length * k as f64 / n as f64).collect()
    }

    /// The outline of the wall alone in plan as a closed polygon (the corners of
    /// [`Self::plain_footprint`] for a straight wall; for an arc wall the faces faceted within
    /// [`FACET_CHORD`]), starting on the right face at the start.
    #[must_use]
    pub fn plain_outline(&self) -> Vec<[f64; 2]> {
        if !self.is_arc() {
            return self.plain_footprint().to_vec();
        }
        let (low, high) = self.extent();
        let n = self.facets().len();
        let length = self.length();
        let face = |side: f64, k: usize| -> [f64; 2] {
            let d = length * k as f64 / n as f64;
            let p = self.point_at(d);
            let t = self.tangent_at(d);
            [p[0] - t[1] * side, p[1] + t[0] * side]
        };
        let mut out: Vec<[f64; 2]> = (0..=n).map(|k| face(low, k)).collect();
        out.extend((0..=n).rev().map(|k| face(high, k)));
        out
    }

    /// The extent of the wall's body across the baseline, measured toward the left of the
    /// baseline's direction: `(low, high)`, with `high - low` the thickness.
    #[must_use]
    pub fn extent(&self) -> (f64, f64) {
        let t = self.thickness;
        // Where the line lies, measured from the interior face toward the exterior one.
        let [exterior, interior] = self.core;
        let line = match self.location {
            LocationLine::Center => t / 2.0,
            LocationLine::Exterior => t,
            LocationLine::Interior => 0.0,
            LocationLine::CoreCenter => (interior + t - exterior) / 2.0,
            LocationLine::CoreExterior => t - exterior,
            LocationLine::CoreInterior => interior,
        };
        (-line, t - line)
    }

    /// The extent of the core across the baseline, like [`Self::extent`].
    #[must_use]
    pub fn core_extent(&self) -> (f64, f64) {
        let (low, high) = self.extent();
        (low + self.core[1], high - self.core[0])
    }

    /// The same wall with another location line, in the same physical position: the baseline moves
    /// across by the difference of the two offsets, so the body does not move.
    #[must_use]
    pub fn with_location(&self, location: LocationLine) -> Self {
        let (low, _) = self.extent();
        let moved = Self { location, ..*self };
        let (new_low, _) = moved.extent();
        let length = self.length();
        if length <= 0.0 {
            return moved;
        }
        // The left normal of the direction; the body's low edge stays at `low` from the old line.
        // An arc's end points move along their own normals: the new baseline is concentric.
        let shift = low - new_low;
        let at = |p: [f64; 2], d: f64| {
            let t = self.tangent_at(d);
            [p[0] - t[1] * shift, p[1] + t[0] * shift]
        };
        Self {
            start: at(self.start, 0.0),
            end: at(self.end, length),
            ..moved
        }
    }

    /// The four corners of the wall alone, square at both ends, in the order of the footprint
    /// loop: start right, end right, end left, start left (see [`EXTERIOR_CURVE`]). A zero-length
    /// baseline gets the direction of the x axis and falls through to the kernel's
    /// degenerate-loop error in [`Self::solid_in`].
    #[must_use]
    pub fn plain_footprint(&self) -> [[f64; 2]; 4] {
        let length = self.length();
        // The directions at the two ends: the tangents of an arc wall.
        let at = |d: f64| {
            if length > 0.0 {
                self.tangent_at(d)
            } else {
                [1.0, 0.0]
            }
        };
        let (us, ue) = (at(0.0), at(length));
        let (low, high) = self.extent();
        // The left normal of the direction.
        let corner = |p: [f64; 2], u: [f64; 2], side: f64| [p[0] - u[1] * side, p[1] + u[0] * side];
        [
            corner(self.start, us, low),
            corner(self.end, ue, low),
            corner(self.end, ue, high),
            corner(self.start, us, high),
        ]
    }

    /// The solid of the wall alone standing on a level at `level_elevation`: a prism over the
    /// footprint rectangle (see [`Self::solid_in`]).
    ///
    /// # Errors
    /// As [`Self::solid_in`].
    pub fn solid(&self, level_elevation: f64) -> Result<Polyhedron> {
        self.solid_in(self.plain_footprint(), level_elevation)
    }

    /// The solid of the wall standing on a level at `level_elevation`: a prism over `footprint`
    /// (the corners in the order of [`Self::plain_footprint`], joined by
    /// [`crate::join::footprints`]) from `level_elevation + base_offset` up by the height. The
    /// exterior face carries [`EXTERIOR_FACE`] and the interior face [`INTERIOR_FACE`]; the caps
    /// are [`FaceTag::Top`] and [`FaceTag::Bottom`], and the two ends the other side tags.
    ///
    /// # Errors
    /// `geom.region.loop_zero_area` and the other errors of the solid kernel for a wall whose
    /// baseline has no length or whose numbers are not finite.
    pub fn solid_in(&self, footprint: [[f64; 2]; 4], level_elevation: f64) -> Result<Polyhedron> {
        let tolerance = Tolerance::DEFAULT;
        let points = footprint.map(|p| Point2::new(p[0], p[1]));
        if !self.shape_fits() {
            return Err(arc_base::Error::new(
                crate::error::codes::WALL_SHAPE_INVALID,
            ));
        }
        if self.leans() {
            return self.leaning_solid(footprint, level_elevation);
        }
        let mut curves = Vec::with_capacity(4);
        if self.is_arc() {
            // The two faces are arcs of the baseline's sweep (the second runs backwards).
            let line = Polyline2::new(
                vec![
                    PolylineVertex::new(points[0], self.bulge),
                    PolylineVertex::new(points[1], 0.0),
                    PolylineVertex::new(points[2], -self.bulge),
                    PolylineVertex::new(points[3], 0.0),
                ],
                true,
            )?;
            for segment in line.segments() {
                curves.push(match segment {
                    PolylineSegment::Line(s) => Curve2::Segment(s),
                    PolylineSegment::Arc(a) => Curve2::Arc(a),
                });
            }
        } else {
            for k in 0..4 {
                let (a, b) = (points[k], points[(k + 1) % 4]);
                curves.push(Curve2::Segment(Segment2::new(a, b)?));
            }
        }
        let region = Region::new(vec![RegionPart::new(
            Loop2::new(curves, &tolerance)?,
            Vec::new(),
        )])?;
        prism(
            &region,
            level_elevation + self.base_offset,
            self.height,
            if self.is_arc() { FACET_CHORD } else { CHORD },
            &tolerance,
            &Limits::DEFAULT,
        )
    }

    /// Whether the shape numbers are usable: the faces of an arc wall have a radius, a leaning
    /// wall keeps a thickness at its top, and an arc wall does not lean.
    #[must_use]
    pub fn shape_fits(&self) -> bool {
        self.arc_fits()
            && self
                .lean
                .iter()
                .all(|a| a.is_finite() && a.abs() <= MAX_LEAN)
            && !(self.is_arc() && self.leans())
            && (!self.leans() || self.thickness_at(self.height) >= MIN_TOP_THICKNESS)
    }

    /// The solid of a leaning (slanted or tapered) wall: the joined footprint at the base and
    /// the same footprint with every corner moved sideways by [`Self::lean_shift`] at the top
    /// (see [`crate::wall_shape::leaning_prism`]).
    fn leaning_solid(&self, footprint: [[f64; 2]; 4], level_elevation: f64) -> Result<Polyhedron> {
        let base = level_elevation + self.base_offset;
        crate::wall_shape::leaning_prism(self, &footprint, base, base + self.height, base)
    }
}

impl Wall {
    /// The lines between the layers of the wall's type `layers` in plan, inside the joined
    /// footprint `footprint` (corners as [`Self::plain_footprint`] orders them), each with whether
    /// it is a boundary of the core. The faces of the wall are not among them, layers without
    /// thickness add no second line, and the stretches `gaps` along the baseline (distances from
    /// its start: the openings) are left out.
    #[must_use]
    pub fn layer_lines(
        &self,
        footprint: [[f64; 2]; 4],
        layers: &crate::element_type::Layers,
        gaps: &[(f64, f64)],
    ) -> Vec<([[f64; 2]; 2], bool)> {
        let (low, high) = self.extent();
        let length = self.length();
        if high - low <= 0.0 || length <= 0.0 {
            return Vec::new();
        }
        let u = [
            (self.end[0] - self.start[0]) / length,
            (self.end[1] - self.start[1]) / length,
        ];
        let along = |p: [f64; 2]| (p[0] - self.start[0]) * u[0] + (p[1] - self.start[1]) * u[1];
        let lerp =
            |a: [f64; 2], b: [f64; 2], t: f64| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
        // Each boundary's distance from the exterior face, once, with whether the core starts or
        // ends there.
        let mut offsets: Vec<(f64, bool)> = Vec::new();
        let mut at = 0.0;
        for (k, layer) in layers
            .layers
            .iter()
            .enumerate()
            .take(layers.layers.len().saturating_sub(1))
        {
            at += layer.thickness;
            let core = k + 1 == layers.core.0 || k + 1 == layers.core.1;
            match offsets.last_mut() {
                Some(last) if (last.0 - at).abs() < 1e-9 => last.1 |= core,
                _ => offsets.push((at, core)),
            }
        }
        let mut out = Vec::new();
        for (offset, core) in offsets {
            if offset <= 1e-9 || offset >= high - low - 1e-9 {
                continue;
            }
            let t = offset / (high - low);
            let from = lerp(footprint[3], footprint[0], t);
            let to = lerp(footprint[2], footprint[1], t);
            let (a, b) = (along(from), along(to));
            if b - a <= 1e-9 {
                continue;
            }
            let point = |d: f64| lerp(from, to, (d - a) / (b - a));
            let mut start = a;
            let mut cuts: Vec<(f64, f64)> = gaps
                .iter()
                .copied()
                .filter(|g| g.1 > a && g.0 < b)
                .collect();
            cuts.sort_by(|x, y| x.0.total_cmp(&y.0));
            for (g0, g1) in cuts {
                if g0 > start {
                    out.push(([point(start), point(g0)], core));
                }
                start = start.max(g1);
            }
            if b > start {
                out.push(([point(start), point(b)], core));
            }
        }
        out
    }
}

/// A lean angle of a record, in radians; 0 when absent.
fn lean_of(values: &[Option<Value>], at: usize) -> f64 {
    match values.get(at) {
        Some(Some(Value::Quantity(q))) if q.value().is_finite() => q.value(),
        _ => 0.0,
    }
}

/// The sketched elevation profile of a wall: loops in the vertical plane of the baseline, a point
/// being the distance along the baseline from its start and the height above the wall's base.
/// The first loop is the outline; the others are openings in it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Profile {
    /// The outline, then the holes.
    pub loops: Vec<Vec<[f64; 2]>>,
}

/// The declaration of the profile field of a wall record.
#[must_use]
pub fn profile_field() -> arc_doc::FieldDecl {
    use arc_doc::{Bound, FieldDecl, ValueType};
    let point = |name: &str| {
        crate::types::length_field(
            name,
            Some(crate::types::real(crate::types::COORDINATE_RANGE)),
        )
    };
    FieldDecl::new(
        "profile",
        ValueType::List(Box::new(ValueType::Record(vec![
            point("x"),
            point("z"),
            FieldDecl::new("loop", ValueType::I64).with_default(Value::I64(0)),
        ]))),
    )
    .with_bound(Bound::Length {
        max: MAX_PROFILE_POINTS as u64,
    })
    .optional()
    .in_aspect("geometry")
    .since_minor(6)
}

/// The record of one profile point: its distance along the baseline, its height and its loop.
#[must_use]
pub fn profile_point(x: f64, z: f64, loop_index: usize) -> Value {
    Value::Record(vec![
        Some(types::length(x)),
        Some(types::length(z)),
        Some(Value::I64(i64::try_from(loop_index).unwrap_or(0))),
    ])
}

impl Profile {
    /// The profile of a wall record; `None` when it has none (or an unreadable one).
    #[must_use]
    pub fn of(values: &[Option<Value>]) -> Option<Self> {
        let Some(Some(Value::List(items))) = values.get(wall::PROFILE) else {
            return None;
        };
        let mut loops: Vec<Vec<[f64; 2]>> = Vec::new();
        for item in items.iter().take(MAX_PROFILE_POINTS) {
            let Value::Record(fields) = item else {
                return None;
            };
            let x = types::meters(fields, 0)?;
            let z = types::meters(fields, 1)?;
            let at = match fields.get(2) {
                Some(Some(Value::I64(k))) if *k >= 0 => usize::try_from(*k).ok()?,
                _ => return None,
            };
            if at > loops.len() || at > MAX_PROFILE_POINTS {
                return None;
            }
            if at == loops.len() {
                loops.push(Vec::new());
            }
            loops.get_mut(at)?.push([x, z]);
        }
        (!loops.is_empty()).then_some(Self { loops })
    }

    /// The records of the profile, for the wall record's field.
    #[must_use]
    pub fn records(&self) -> Vec<Value> {
        self.loops
            .iter()
            .enumerate()
            .flat_map(|(k, l)| l.iter().map(move |p| profile_point(p[0], p[1], k)))
            .collect()
    }

    /// Whether the profile is usable for `wall` of `length` along the baseline: every loop has
    /// three points or more and is a simple polygon, the outline lies inside the rectangle of the
    /// wall, and the holes lie inside the outline and apart from one another.
    #[must_use]
    pub fn fits(&self, length: f64, height: f64) -> bool {
        let tol = 1e-9;
        let total: usize = self.loops.iter().map(Vec::len).sum();
        if total > MAX_PROFILE_POINTS || self.loops.is_empty() {
            return false;
        }
        if self.loops.iter().any(|l| {
            l.len() < 3
                || l.iter().any(|p| !p[0].is_finite() || !p[1].is_finite())
                || !simple(l)
                || crate::floor::signed_area(l).abs() < 1e-9
        }) {
            return false;
        }
        let outline = &self.loops[0];
        if outline
            .iter()
            .any(|p| p[0] < -tol || p[0] > length + tol || p[1] < -tol || p[1] > height + tol)
        {
            return false;
        }
        for (k, hole) in self.loops.iter().enumerate().skip(1) {
            if hole
                .iter()
                .any(|p| !inside_polygon(outline, *p) && !on_boundary(outline, *p))
            {
                return false;
            }
            for other in self.loops.iter().skip(1).take(k - 1) {
                if loops_cross(hole, other) {
                    return false;
                }
            }
            if loops_cross(hole, outline) {
                return false;
            }
        }
        true
    }

    /// Whether the rectangle `[x0, x1] x [z0, z1]` of the elevation lies in the material of the
    /// profile: inside the outline and clear of its holes.
    #[must_use]
    pub fn holds(&self, x: (f64, f64), z: (f64, f64)) -> bool {
        let rect = [[x.0, z.0], [x.1, z.0], [x.1, z.1], [x.0, z.1]];
        let Some(outline) = self.loops.first() else {
            return false;
        };
        let material = |p: [f64; 2]| {
            (inside_polygon(outline, p) || on_boundary(outline, p))
                && self
                    .loops
                    .iter()
                    .skip(1)
                    .all(|h| !inside_polygon(h, p) || on_boundary(h, p))
        };
        if !rect.iter().all(|p| material(*p)) {
            return false;
        }
        // No edge of the profile may cross into the rectangle.
        let center = [(x.0 + x.1) / 2.0, (z.0 + z.1) / 2.0];
        material(center)
            && self.loops.iter().all(|l| {
                !loops_cross(&rect, l) && l.iter().all(|p| !strictly_inside_rect(*p, x, z))
            })
    }

    /// The solid of `wall` standing on a level at `level_elevation` with this profile: the
    /// profile extruded across the thickness from `from` to `to` (measured across the baseline
    /// like [`Wall::extent`]; the whole wall is `extent()`), ends square.
    ///
    /// # Errors
    /// The solid kernel's errors for a profile it cannot make a solid of.
    pub fn solid(
        &self,
        wall: &Wall,
        level_elevation: f64,
        (from, to): (f64, f64),
    ) -> Result<Polyhedron> {
        use arc_base::math::{Point3, Vector3};
        let tolerance = Tolerance::DEFAULT;
        let length = wall.length().max(f64::MIN_POSITIVE);
        let (ux, uy) = (
            (wall.end[0] - wall.start[0]) / length,
            (wall.end[1] - wall.start[1]) / length,
        );
        // The plane of the profile at the high side; its normal points to the low side.
        let origin = Point3::new(
            wall.start[0] - uy * to,
            wall.start[1] + ux * to,
            level_elevation + wall.base_offset,
        );
        let frame = arc_geom::PlaneFrame::new(
            origin,
            Vector3::new(ux, uy, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            &tolerance,
        )?;
        let mut parts = Vec::new();
        for l in &self.loops {
            let mut curves = Vec::with_capacity(l.len());
            for k in 0..l.len() {
                let (a, b) = (l[k], l[(k + 1) % l.len()]);
                curves.push(Curve2::Segment(Segment2::new(
                    Point2::new(a[0], a[1]),
                    Point2::new(b[0], b[1]),
                )?));
            }
            parts.push(Loop2::new(curves, &tolerance)?);
        }
        let mut loops = parts.into_iter();
        let outer = loops
            .next()
            .ok_or_else(|| arc_base::Error::new(arc_geom::error::codes::REGION_EMPTY))?;
        let region = Region::new(vec![RegionPart::new(outer, loops.collect())])?;
        arc_geom::extrude(
            &region,
            &frame,
            to - from,
            CHORD,
            &tolerance,
            &Limits::DEFAULT,
        )
    }
}

fn orient(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

/// Whether the closed polygon `l` has no crossing or touching edges that are not neighbors.
fn simple(l: &[[f64; 2]]) -> bool {
    let n = l.len();
    for i in 0..n {
        for j in i + 1..n {
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            if segments_meet(l[i], l[(i + 1) % n], l[j], l[(j + 1) % n]) {
                return false;
            }
        }
    }
    true
}

fn segments_meet(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let (o1, o2, o3, o4) = (
        orient(a, b, c),
        orient(a, b, d),
        orient(c, d, a),
        orient(c, d, b),
    );
    let eps = 1e-12;
    if ((o1 > eps && o2 < -eps) || (o1 < -eps && o2 > eps))
        && ((o3 > eps && o4 < -eps) || (o3 < -eps && o4 > eps))
    {
        return true;
    }
    let on = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| {
        orient(p, q, r).abs() <= eps
            && r[0] >= p[0].min(q[0]) - eps
            && r[0] <= p[0].max(q[0]) + eps
            && r[1] >= p[1].min(q[1]) - eps
            && r[1] <= p[1].max(q[1]) + eps
    };
    on(a, b, c) || on(a, b, d) || on(c, d, a) || on(c, d, b)
}

/// Whether the edges of `a` and `b` properly cross (touching at a point is not crossing).
fn loops_cross(a: &[[f64; 2]], b: &[[f64; 2]]) -> bool {
    let eps = 1e-12;
    (0..a.len()).any(|i| {
        let (p, q) = (a[i], a[(i + 1) % a.len()]);
        (0..b.len()).any(|j| {
            let (r, t) = (b[j], b[(j + 1) % b.len()]);
            let (o1, o2, o3, o4) = (
                orient(p, q, r),
                orient(p, q, t),
                orient(r, t, p),
                orient(r, t, q),
            );
            ((o1 > eps && o2 < -eps) || (o1 < -eps && o2 > eps))
                && ((o3 > eps && o4 < -eps) || (o3 < -eps && o4 > eps))
        })
    })
}

fn on_boundary(l: &[[f64; 2]], p: [f64; 2]) -> bool {
    (0..l.len()).any(|i| {
        let (a, b) = (l[i], l[(i + 1) % l.len()]);
        orient(a, b, p).abs() <= 1e-9
            && p[0] >= a[0].min(b[0]) - 1e-9
            && p[0] <= a[0].max(b[0]) + 1e-9
            && p[1] >= a[1].min(b[1]) - 1e-9
            && p[1] <= a[1].max(b[1]) + 1e-9
    })
}

/// Whether `p` is inside the closed polygon `l`.
#[must_use]
pub fn inside_outline(l: &[[f64; 2]], p: [f64; 2]) -> bool {
    inside_polygon(l, p)
}

fn inside_polygon(l: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut inside = false;
    for i in 0..l.len() {
        let (a, b) = (l[i], l[(i + 1) % l.len()]);
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
    }
    inside
}

fn strictly_inside_rect(p: [f64; 2], x: (f64, f64), z: (f64, f64)) -> bool {
    let e = 1e-9;
    p[0] > x.0 + e && p[0] < x.1 - e && p[1] > z.0 + e && p[1] < z.1 - e
}

/// The side of a wall a band is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandFace {
    /// The exterior face, on the left of the baseline.
    Exterior,
    /// The interior face, on the right of the baseline.
    Interior,
}

/// A band on a wall face: a strip that runs the length of the wall (horizontal) or the height of
/// it (vertical), projecting from the face or recessed into it. Measured from the base or the
/// top of the wall (the start or the end of the baseline for a vertical band).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Band {
    /// The face the band is on.
    pub face: BandFace,
    /// Whether the band is vertical.
    pub vertical: bool,
    /// Whether the position is measured from the top (the end for a vertical band).
    pub from_top: bool,
    /// The distance of the band's near edge from the base (start), in meters.
    pub position: f64,
    /// The extent of the band in the direction of the measurement, in meters.
    pub width: f64,
    /// How far the band stands out of the face; negative for a recess, in meters.
    pub depth: f64,
}

/// The largest depth of a band, in meters.
pub const MAX_BAND_DEPTH: f64 = 2.0;

/// The declaration of the bands field of a wall record.
#[must_use]
pub fn bands_field() -> arc_doc::FieldDecl {
    use arc_doc::{Bound, FieldDecl, ValueType};
    let m = |name: &str, range: (f64, f64)| {
        crate::types::length_field(name, Some(crate::types::real(range)))
    };
    FieldDecl::new(
        "bands",
        ValueType::List(Box::new(ValueType::Record(vec![
            FieldDecl::new("face", ValueType::I64).with_default(Value::I64(0)),
            FieldDecl::new("vertical", ValueType::Bool).with_default(Value::Bool(false)),
            FieldDecl::new("from_top", ValueType::Bool).with_default(Value::Bool(false)),
            m("position", (0.0, 100_000.0)),
            m("width", (0.0, 100_000.0)),
            m("depth", (-MAX_BAND_DEPTH, MAX_BAND_DEPTH)),
        ]))),
    )
    .with_bound(Bound::Length {
        max: MAX_BANDS as u64,
    })
    .optional()
    .in_aspect("geometry")
    .since_minor(6)
}

impl Band {
    /// The record of the band for the wall record's field.
    #[must_use]
    pub fn record(&self) -> Value {
        Value::Record(vec![
            Some(Value::I64(i64::from(self.face == BandFace::Interior))),
            Some(Value::Bool(self.vertical)),
            Some(Value::Bool(self.from_top)),
            Some(types::length(self.position)),
            Some(types::length(self.width)),
            Some(types::length(self.depth)),
        ])
    }

    /// The bands of a wall record; empty when it has none.
    #[must_use]
    pub fn of(values: &[Option<Value>]) -> Vec<Self> {
        let Some(Some(Value::List(items))) = values.get(wall::BANDS) else {
            return Vec::new();
        };
        items
            .iter()
            .take(MAX_BANDS)
            .filter_map(|item| {
                let Value::Record(f) = item else { return None };
                Some(Self {
                    face: match f.first() {
                        Some(Some(Value::I64(1))) => BandFace::Interior,
                        _ => BandFace::Exterior,
                    },
                    vertical: matches!(f.get(1), Some(Some(Value::Bool(true)))),
                    from_top: matches!(f.get(2), Some(Some(Value::Bool(true)))),
                    position: types::meters(f, 3)?,
                    width: types::meters(f, 4)?,
                    depth: types::meters(f, 5)?,
                })
            })
            .collect()
    }

    /// The extent of the band along the measured direction, from its low to its high end, for a
    /// wall with `span` (the height, or the length for a vertical band).
    #[must_use]
    pub fn range(&self, span: f64) -> (f64, f64) {
        if self.from_top {
            (span - self.position - self.width, span - self.position)
        } else {
            (self.position, self.position + self.width)
        }
    }

    /// Whether the band is usable on a wall whose measured span is `span`.
    #[must_use]
    pub fn fits(&self, span: f64) -> bool {
        let (a, b) = self.range(span);
        self.width > 1e-3
            && self.depth.abs() > 1e-3
            && self.depth.abs() <= MAX_BAND_DEPTH
            && a >= -1e-9
            && b <= span + 1e-9
            && [self.position, self.width, self.depth]
                .iter()
                .all(|v| v.is_finite())
    }
}

/// The index of the face of `solid` that carries `tag`.
#[must_use]
pub fn face_index(solid: &Polyhedron, tag: FaceTag) -> Option<usize> {
    (0..solid.faces().len()).find(|&i| solid.face_tag(i) == Some(tag))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_location_line_keeps_the_body_where_it_is() {
        let wall = Wall {
            start: [0.0, 0.0],
            end: [4.0, 0.0],
            thickness: 0.2,
            core: [0.0, 0.0],
            height: 3.0,
            base_offset: 0.0,
            location: LocationLine::Exterior,
            start_join: JoinStyle::Auto,
            end_join: JoinStyle::Auto,
            bulge: 0.0,
            lean: [0.0; 2],
        };
        // The body lies at y in [-0.2, 0]; the baseline follows its new face or middle.
        let interior = wall.with_location(LocationLine::Interior);
        assert_eq!((interior.start, interior.end), ([0.0, -0.2], [4.0, -0.2]));
        let center = wall.with_location(LocationLine::Center);
        assert!((center.start[1] + 0.1).abs() < 1e-12);
        assert_eq!(center.location, LocationLine::Center);
        // Back again: the exact starting wall.
        let back = center.with_location(LocationLine::Exterior);
        assert!((back.start[1]).abs() < 1e-12 && (back.end[0] - 4.0).abs() < 1e-12);
    }

    fn wall(location: LocationLine) -> Wall {
        Wall {
            start: [1.0, 2.0],
            end: [4.0, 6.0],
            thickness: 0.3,
            core: [0.05, 0.02],
            height: 2.5,
            base_offset: 0.1,
            location,
            start_join: JoinStyle::Auto,
            end_join: JoinStyle::Auto,
            bulge: 0.0,
            lean: [0.0; 2],
        }
    }

    #[test]
    fn the_volume_is_length_times_thickness_times_height_for_each_location_line() {
        for location in LocationLine::ALL {
            let w = wall(location);
            let solid = w.solid(3.0).unwrap();
            assert!(
                (solid.volume() - 5.0 * 0.3 * 2.5).abs() < 1e-9,
                "{location:?}"
            );
            let bounds = solid.bounding_box();
            assert!((bounds.min().z - 3.1).abs() < 1e-9);
            assert!((bounds.max().z - 5.6).abs() < 1e-9);
        }
    }

    #[test]
    fn the_exterior_face_is_on_the_left_of_the_baseline_direction() {
        for location in LocationLine::ALL {
            let w = wall(location);
            let solid = w.solid(0.0).unwrap();
            let face = face_index(&solid, EXTERIOR_FACE).unwrap();
            let normal = solid.face_normal(face).unwrap();
            // Direction (3, 4) / 5; its left normal is (-0.8, 0.6).
            assert!((normal.x + 0.8).abs() < 1e-9 && (normal.y - 0.6).abs() < 1e-9);
            let interior = face_index(&solid, INTERIOR_FACE).unwrap();
            let opposite = solid.face_normal(interior).unwrap();
            assert!((opposite.x - 0.8).abs() < 1e-9);
            // Where the faces lie across the baseline: signed distance to the baseline.
            let side = |f: usize| {
                let p = solid.vertices()[solid.faces()[f].outer[0] as usize];
                (p.x - 1.0) * -0.8 + (p.y - 2.0) * 0.6
            };
            let (low, high) = w.extent();
            assert!((side(face) - high).abs() < 1e-9, "{location:?}");
            assert!((side(interior) - low).abs() < 1e-9, "{location:?}");
        }
    }

    #[test]
    fn layer_lines_lie_between_the_layers_and_skip_openings() {
        use crate::element_type::{Function, Layer, Layers};
        let wall = Wall {
            thickness: 0.3,
            core: [0.1, 0.05],
            ..wall(LocationLine::Exterior)
        };
        let wall = Wall {
            start: [0.0, 0.0],
            end: [4.0, 0.0],
            ..wall
        };
        let layers = Layers {
            layers: vec![
                Layer::new(Function::Finish, "a", 0.1),
                Layer::new(Function::Membrane, "m", 0.0),
                Layer::new(Function::Structure, "b", 0.15),
                Layer::new(Function::Finish, "c", 0.05),
            ],
            core: (2, 3),
        };
        let lines = wall.layer_lines(wall.plain_footprint(), &layers, &[(1.0, 2.0)]);
        // Two boundaries (the membrane adds none), both of the core, each cut by the opening.
        assert_eq!(lines.len(), 4);
        assert!(lines.iter().all(|(_, core)| *core));
        // The exterior face is the baseline; the first boundary is 0.1 m inside it.
        assert!((lines[0].0[0][1] + 0.1).abs() < 1e-9);
        assert!((lines[0].0[1][0] - 1.0).abs() < 1e-9);
        assert!((lines[2].0[0][1] + 0.25).abs() < 1e-9);
    }

    fn quarter_arc() -> Wall {
        Wall {
            start: [5.0, 0.0],
            end: [0.0, 5.0],
            bulge: (std::f64::consts::FRAC_PI_2 / 4.0).tan(),
            ..wall(LocationLine::Center)
        }
    }

    #[test]
    fn an_arc_walks_its_baseline_by_distance_and_finds_distances_back() {
        let w = quarter_arc();
        let a = w.arc().unwrap();
        assert!((a.radius - 5.0).abs() < 1e-12 && a.center[0].abs() < 1e-12);
        assert!((w.length() - 5.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        for k in 0..=10 {
            let d = w.length() * f64::from(k) / 10.0;
            let p = w.point_at(d);
            assert!((p[0].hypot(p[1]) - 5.0).abs() < 1e-12);
            assert!((w.distance_of(p) - d).abs() < 1e-9, "{d}");
            let t = w.tangent_at(d);
            assert!(
                (t[0].hypot(t[1]) - 1.0).abs() < 1e-12 && (t[0] * p[0] + t[1] * p[1]).abs() < 1e-9
            );
        }
        // Points beyond the ends clamp to the nearer end.
        assert!(w.distance_of([5.0, -1.0]).abs() < 1e-9);
        assert!((w.distance_of([-1.0, 5.0]) - w.length()).abs() < 1e-9);
    }

    #[test]
    fn the_facets_of_an_arc_join_end_to_end_within_the_chord_tolerance() {
        let w = quarter_arc();
        let facets = w.facets();
        assert!(facets.len() >= 8);
        assert_eq!(facets[0].start, w.start);
        assert_eq!(facets[facets.len() - 1].end, w.end);
        for pair in facets.windows(2) {
            assert_eq!(pair[0].end, pair[1].start);
        }
        // Every facet midpoint lies within the tolerance of the arc.
        for f in &facets {
            let m = [(f.start[0] + f.end[0]) / 2.0, (f.start[1] + f.end[1]) / 2.0];
            assert!((5.0 - m[0].hypot(m[1])) < FACET_CHORD + 1e-12);
        }
        assert_eq!(w.facets().len(), w.facet_marks().len() - 1);
        // A straight wall is its own facet.
        assert_eq!(wall(LocationLine::Center).facets().len(), 1);
    }

    #[test]
    fn a_new_location_line_keeps_an_arc_concentric() {
        let w = quarter_arc();
        let moved = w.with_location(LocationLine::Interior);
        // The interior face is on the right of a counter-clockwise arc, away from the center: the
        // baseline moves onto it, concentric with the old one, and the body stays.
        assert!((moved.arc().unwrap().radius - (5.0 + w.thickness / 2.0)).abs() < 1e-12);
        assert!((moved.arc().unwrap().center[0]).abs() < 1e-9);
        assert!((moved.bulge - w.bulge).abs() < 1e-12);
        assert!((moved.arc().unwrap().sweep - w.arc().unwrap().sweep).abs() < 1e-12);
    }

    #[test]
    fn faces_of_an_arc_need_a_radius() {
        let mut w = quarter_arc();
        assert!(w.arc_fits() && w.shape_fits());
        // A wall as thick as the arc is wide has no inner face.
        w.thickness = 12.0;
        assert!(!w.arc_fits());
        assert!(w.solid(0.0).is_err());
    }

    #[test]
    fn a_lean_moves_the_faces_and_the_layers_follow_in_proportion() {
        let mut w = wall(LocationLine::Center);
        w.lean = [0.2, 0.1];
        let (low, high) = w.extent();
        let z = 2.0;
        assert!((w.lean_shift(high, z) - z * 0.2_f64.tan()).abs() < 1e-12);
        assert!((w.lean_shift(low, z) - z * 0.1_f64.tan()).abs() < 1e-12);
        let middle = w.lean_shift((low + high) / 2.0, z);
        assert!((middle - z * (0.2_f64.tan() + 0.1_f64.tan()) / 2.0).abs() < 1e-12);
        assert!(
            (w.thickness_at(z) - (w.thickness + z * (0.2_f64.tan() - 0.1_f64.tan()))).abs() < 1e-12
        );
        // Too much taper leaves nothing at the top.
        w.lean = [-0.9, 0.9];
        assert!(!w.shape_fits());
    }

    #[test]
    fn a_profile_is_checked_for_shape_and_for_what_it_holds() {
        let outline = vec![[0.0, 0.0], [6.0, 0.0], [6.0, 2.0], [3.0, 3.0], [0.0, 2.0]];
        let hole = vec![[1.0, 0.5], [2.0, 0.5], [2.0, 1.5], [1.0, 1.5]];
        let profile = Profile {
            loops: vec![outline.clone(), hole],
        };
        assert!(profile.fits(6.0, 3.0));
        // Material: right of the hole, up to the eaves; not the hole, not above the roof line.
        assert!(profile.holds((4.0, 5.0), (0.0, 2.0)));
        assert!(!profile.holds((1.2, 1.8), (0.6, 1.4)));
        assert!(!profile.holds((0.0, 1.0), (1.5, 2.8)));
        // A profile past the wall, a bow tie and a hole outside the outline do not fit.
        assert!(
            !Profile {
                loops: vec![outline.clone()]
            }
            .fits(5.0, 3.0)
        );
        let bow = vec![[0.0, 0.0], [2.0, 2.0], [2.0, 0.0], [0.0, 2.0]];
        assert!(!Profile { loops: vec![bow] }.fits(6.0, 3.0));
        let outside = vec![[7.0, 0.5], [8.0, 0.5], [8.0, 1.0]];
        assert!(
            !Profile {
                loops: vec![outline, outside]
            }
            .fits(6.0, 3.0)
        );
        // The records round-trip through the field values.
        let values: Vec<Option<Value>> = std::iter::repeat_n(None, wall::PROFILE)
            .chain([Some(Value::List(profile.records()))])
            .collect();
        assert_eq!(Profile::of(&values), Some(profile));
    }

    #[test]
    fn bands_measure_from_the_base_or_the_top() {
        let band = Band {
            face: BandFace::Exterior,
            vertical: false,
            from_top: true,
            position: 0.5,
            width: 0.3,
            depth: 0.05,
        };
        let (a, b) = band.range(3.0);
        assert!((a - 2.2).abs() < 1e-12 && (b - 2.5).abs() < 1e-12);
        assert!(
            band.fits(3.0)
                && !Band {
                    position: 2.8,
                    ..band
                }
                .fits(3.0)
        );
        assert!(!Band { depth: 0.0, ..band }.fits(3.0));
        let values: Vec<Option<Value>> = std::iter::repeat_n(None, wall::BANDS)
            .chain([Some(Value::List(vec![band.record()]))])
            .collect();
        assert_eq!(Band::of(&values), vec![band]);
    }

    #[test]
    fn a_zero_length_baseline_has_no_solid() {
        let mut w = wall(LocationLine::Center);
        w.end = w.start;
        assert!(w.solid(0.0).is_err());
    }
}
