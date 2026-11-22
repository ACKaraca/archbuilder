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
use arc_geom::{Curve2, FaceTag, Loop2, Polyhedron, Region, RegionPart, Segment2, prism};

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
        })
    }

    /// The wall of a record with its top constraint applied: when the record refers to a level
    /// for its top, the height is that level's elevation minus the base (the storey's level
    /// plus the base offset); else, or when that would not be a valid height, the stored height.
    #[must_use]
    pub fn of(snapshot: &arc_doc::Snapshot, object: &arc_doc::Object) -> Option<Self> {
        let wall = Self::from_values(&object.values)?;
        let top = Self::top_level(&object.values)
            .and_then(|id| crate::spatial::get(snapshot, id))
            .and_then(|level| types::meters(&level.values, types::level::ELEVATION));
        let storey = object
            .header
            .owner
            .and_then(|id| crate::spatial::get(snapshot, id))
            .map_or(0.0, |s| crate::spatial::storey_elevation(snapshot, &s));
        Some(wall.constrained(storey, top))
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

    /// The length of the baseline.
    #[must_use]
    pub fn length(&self) -> f64 {
        (self.end[0] - self.start[0]).hypot(self.end[1] - self.start[1])
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
        let shift = low - new_low;
        let (nx, ny) = (
            -(self.end[1] - self.start[1]) / length,
            (self.end[0] - self.start[0]) / length,
        );
        let at = |p: [f64; 2]| [p[0] + nx * shift, p[1] + ny * shift];
        Self {
            start: at(self.start),
            end: at(self.end),
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
        let (ux, uy) = if length > 0.0 {
            (
                (self.end[0] - self.start[0]) / length,
                (self.end[1] - self.start[1]) / length,
            )
        } else {
            (1.0, 0.0)
        };
        let (low, high) = self.extent();
        // The left normal of the direction.
        let (nx, ny) = (-uy, ux);
        let corner = |at: [f64; 2], side: f64| [at[0] + nx * side, at[1] + ny * side];
        [
            corner(self.start, low),
            corner(self.end, low),
            corner(self.end, high),
            corner(self.start, high),
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
        let mut curves = Vec::with_capacity(4);
        for k in 0..4 {
            let (a, b) = (points[k], points[(k + 1) % 4]);
            curves.push(Curve2::Segment(Segment2::new(a, b)?));
        }
        let region = Region::new(vec![RegionPart::new(
            Loop2::new(curves, &tolerance)?,
            Vec::new(),
        )])?;
        prism(
            &region,
            level_elevation + self.base_offset,
            self.height,
            CHORD,
            &tolerance,
            &Limits::DEFAULT,
        )
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

    #[test]
    fn a_zero_length_baseline_has_no_solid() {
        let mut w = wall(LocationLine::Center);
        w.end = w.start;
        assert!(w.solid(0.0).is_err());
    }
}
