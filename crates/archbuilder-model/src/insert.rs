//! Doors and windows: the authored data of an insert read from its record, the rules that keep it
//! inside its host wall, the opening it cuts and the solids of its own frame and panel.
//!
//! An insert is a built-in parametric kind, not a family: a record with a host wall (its header
//! owner), the distance along the wall's baseline from the baseline's start to the insert's center,
//! a width, a height, a sill height, the frame, an operation, a hand and a facing. Nothing of its
//! geometry is stored. The hand is the side of the hinge seen from the side the leaf swings
//! toward, and the facing is that side: the exterior side of the wall (see [`crate::wall`]) or the
//! interior one.
//!
//! The insert lies in the clear span of its wall: the stretch of the baseline where the joined
//! solid has both faces ([`clear_span`]). An end that is mitered at a corner has a shorter span, so
//! an insert that would reach into the corner is refused instead of being cut through the miter.
//! Inserts of one wall keep [`MIN_GAP`] between their openings, whatever their heights.

use std::sync::Arc;

use arc_base::math::{Point2, Point3, Vector3};
use arc_base::{Error, Limits, Result, Tolerance};
use arc_doc::{Object, Snapshot, Value};
use arc_geom::{
    Curve2, Loop2, PlaneFrame, Polyhedron, Polyline2, PolylineSegment, PolylineVertex, PrismTool,
    Region, RegionPart, Segment2, prism,
};

use crate::error::codes;
use crate::spatial;
use crate::types::{self, DOOR, WALL, WALL_OPENING, WINDOW, insert};
use crate::wall::Wall;

/// The least distance between an opening and the end of the clear span, in meters.
pub const SIDE_CLEARANCE: f64 = 0.01;
/// The least distance between the head of an opening and the wall's top, in meters.
pub const HEAD_CLEARANCE: f64 = 0.01;
/// The least distance between the openings of two inserts of one wall, in meters.
pub const MIN_GAP: f64 = 0.01;
/// The least clear size left inside the frame bars, in meters.
pub const MIN_CLEAR: f64 = 0.02;
/// How far the cutting prism reaches beyond the faces of the wall, in meters.
const OVERSHOOT: f64 = 0.5;
/// The thickness of a door leaf and of a glass pane, in meters.
const LEAF: f64 = 0.04;
const PANE: f64 = 0.01;
/// The gap between the two leaves of a double door, in meters.
const GAP: f64 = 0.005;
/// How far the two panels of a sliding door or window overlap, in meters.
const OVERLAP: f64 = 0.05;
/// The chord of the prisms; every outline is straight.
const CHORD: f64 = 0.01;
/// The most inserts one wall can hold.
pub const MAX_PER_WALL: usize = 1_000;

/// Whether an insert is a door or a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A door: the sill is the wall's base.
    Door,
    /// A window.
    Window,
    /// A bare opening, without a leaf or a frame: a rectangle, or an arch on top.
    Opening,
}

impl Kind {
    /// The kind of a type key.
    #[must_use]
    pub fn of_key(key: &str) -> Option<Self> {
        match key {
            DOOR => Some(Self::Door),
            WINDOW => Some(Self::Window),
            WALL_OPENING => Some(Self::Opening),
            _ => None,
        }
    }

    /// The type key of the kind.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Door => DOOR,
            Self::Window => WINDOW,
            Self::Opening => WALL_OPENING,
        }
    }
}

coded! {
    /// How an insert operates.
    Operation {
        /// A door with one leaf on hinges.
        #[default]
        SingleSwing = 0, "single_swing";
        /// A window that does not open.
        Fixed = 1, "fixed";
        /// A window with a leaf on hinges.
        Casement = 2, "casement";
        /// A door with two leaves on hinges at both jambs.
        DoubleSwing = 3, "double_swing";
        /// A door or window whose leaf slides along the wall.
        Sliding = 4, "sliding";
    }
}

impl Operation {
    /// Whether an insert of `kind` can operate this way: a door swings with one or two leaves or
    /// slides; a window is fixed, casement or sliding.
    #[must_use]
    pub const fn allowed(self, kind: Kind) -> bool {
        match kind {
            Kind::Door => matches!(self, Self::SingleSwing | Self::DoubleSwing | Self::Sliding),
            Kind::Window => matches!(self, Self::Fixed | Self::Casement | Self::Sliding),
            Kind::Opening => true,
        }
    }
}

coded! {
    /// The side of the hinge, seen from the side the leaf swings toward.
    Hand {
        /// The hinge is on the left.
        #[default]
        Left = 0, "left";
        /// The hinge is on the right.
        Right = 1, "right";
    }
}

coded! {
    /// The side of the wall the leaf swings toward.
    Facing {
        /// The exterior side: the left of the baseline's direction.
        #[default]
        Exterior = 0, "exterior";
        /// The interior side.
        Interior = 1, "interior";
    }
}

impl Hand {
    /// The other hand.
    #[must_use]
    pub const fn flipped(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

impl Facing {
    /// The other side.
    #[must_use]
    pub const fn flipped(self) -> Self {
        match self {
            Self::Exterior => Self::Interior,
            Self::Interior => Self::Exterior,
        }
    }
}

/// The authored data of a door or a window, lengths in meters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Insert {
    /// Door or window.
    pub kind: Kind,
    /// The distance from the baseline's start to the center of the opening.
    pub distance: f64,
    /// The width of the opening.
    pub width: f64,
    /// The height of the opening.
    pub height: f64,
    /// The height of the opening's bottom above the wall's base.
    pub sill: f64,
    /// The width of the frame bars.
    pub frame_width: f64,
    /// The depth of the frame across the wall.
    pub frame_depth: f64,
    /// The operation.
    pub operation: Operation,
    /// The hand.
    pub hand: Hand,
    /// The facing.
    pub facing: Facing,
    /// The rise of the arched top of a bare opening, which counts in `height`; 0 for a rectangle.
    pub rise: f64,
}

impl Insert {
    /// The insert of a record's field values; `None` when a field is missing.
    #[must_use]
    pub fn from_values(kind: Kind, values: &[Option<Value>]) -> Option<Self> {
        let m = |at| types::meters(values, at);
        if kind == Kind::Opening {
            use types::wall_opening as o;
            return Some(Self {
                kind,
                distance: m(o::DISTANCE)?,
                width: m(o::WIDTH)?,
                height: m(o::HEIGHT)?,
                sill: m(o::SILL)?,
                frame_width: 0.0,
                frame_depth: 0.0,
                operation: Operation::default(),
                hand: Hand::default(),
                facing: Facing::default(),
                // The arch cannot rise above the opening.
                rise: m(o::RISE).unwrap_or(0.0).clamp(0.0, m(o::HEIGHT)?),
            });
        }
        Some(Self {
            kind,
            distance: m(insert::DISTANCE)?,
            width: m(insert::WIDTH)?,
            height: m(insert::HEIGHT)?,
            sill: m(insert::SILL)?,
            frame_width: m(insert::FRAME_WIDTH)?,
            frame_depth: m(insert::FRAME_DEPTH)?,
            operation: Operation::of_field(values, insert::OPERATION),
            hand: Hand::of_field(values, insert::HAND),
            facing: Facing::of_field(values, insert::FACING),
            rise: 0.0,
        })
    }

    /// The insert of a record of the document, by its type key.
    #[must_use]
    pub fn of(snapshot: &Snapshot, object: &Object) -> Option<Self> {
        let key = spatial::type_of(snapshot, object.id)?;
        Self::from_values(Kind::of_key(&key)?, &object.values)
    }

    /// The stretch of the baseline the opening covers: `(from, to)` distances from the start.
    #[must_use]
    pub fn extent(&self) -> (f64, f64) {
        (
            self.distance - self.width / 2.0,
            self.distance + self.width / 2.0,
        )
    }

    /// The height of the opening's top above the wall's base.
    #[must_use]
    pub fn top(&self) -> f64 {
        self.sill + self.height
    }

    /// The hinge's direction along the baseline: `1.0` toward the end, `-1.0` toward the start.
    /// Seen from the exterior side the baseline's start is on the right.
    #[must_use]
    pub fn hinge_direction(&self) -> f64 {
        let hand = if self.hand == Hand::Left { 1.0 } else { -1.0 };
        let side = if self.facing == Facing::Exterior {
            1.0
        } else {
            -1.0
        };
        hand * side
    }
}

/// The inserts hosted by the wall `wall`, with their records, by distance and then identifier.
#[must_use]
pub fn hosted(snapshot: &Snapshot, wall: arc_base::ObjectId) -> Vec<(Arc<Object>, Insert)> {
    let mut found: Vec<(Arc<Object>, Insert)> = [DOOR, WINDOW, WALL_OPENING]
        .into_iter()
        .flat_map(|key| spatial::children(snapshot, key, wall))
        .filter_map(|o| Insert::of(snapshot, &o).map(|i| (o, i)))
        .collect();
    found.sort_by(|a, b| {
        a.1.distance
            .total_cmp(&b.1.distance)
            .then_with(|| a.0.id.cmp(&b.0.id))
    });
    found
}

/// The openings of the wall `id`: those of its inserts and those curtain walls embedded in it cut.
#[must_use]
pub fn openings_of(snapshot: &Snapshot, id: arc_base::ObjectId) -> Vec<Insert> {
    let mut out: Vec<Insert> = hosted(snapshot, id).into_iter().map(|(_, i)| i).collect();
    if let Some(host) = spatial::get(snapshot, id) {
        out.extend(
            crate::curtain::embedded(snapshot, &host)
                .into_iter()
                .map(|(_, i)| i),
        );
    }
    out
}

/// The stretch of the baseline, as distances from its start, where the joined solid with the
/// corners `footprint` has both faces: between the inner corners of the two ends.
#[must_use]
pub fn clear_span(wall: &Wall, footprint: [[f64; 2]; 4]) -> (f64, f64) {
    let length = wall.length();
    if length <= 0.0 {
        return (0.0, 0.0);
    }
    let (ux, uy) = (
        (wall.end[0] - wall.start[0]) / length,
        (wall.end[1] - wall.start[1]) / length,
    );
    let along = |p: [f64; 2]| (p[0] - wall.start[0]) * ux + (p[1] - wall.start[1]) * uy;
    (
        along(footprint[0]).max(along(footprint[3])),
        along(footprint[1]).min(along(footprint[2])),
    )
}

/// The clear span of a wall record in its document.
#[must_use]
pub fn clear_span_of(snapshot: &Snapshot, object: &Object) -> Option<(f64, f64)> {
    let wall = Wall::of(snapshot, object)?;
    let footprint = crate::join::footprint_of(snapshot, object)?;
    Some(clear_span(&wall, footprint))
}

/// The error of an opening that would open into the body of another wall of the storey, one that
/// meets the insert's wall in a T or crosses it.
fn meets_wall(
    snapshot: &Snapshot,
    wall_object: &Object,
    wall: &Wall,
    item: &Insert,
    name: &str,
) -> Option<Error> {
    let storey = wall_object.header.owner?;
    let axes = Axes::new(wall, 0.0).ok()?;
    let (from, to) = item.extent();
    for other in spatial::walls(snapshot, storey) {
        if other.id == wall_object.id {
            continue;
        }
        let Some(feet) = crate::join::footprint_of(snapshot, &other) else {
            continue;
        };
        let quad = feet.map(|p| {
            let (dx, dy) = (p[0] - axes.origin[0], p[1] - axes.origin[1]);
            [
                dx * axes.u[0] + dy * axes.u[1],
                dx * axes.n[0] + dy * axes.n[1],
            ]
        });
        let rect = [
            [from, axes.low - TOUCH],
            [to, axes.low - TOUCH],
            [to, axes.high + TOUCH],
            [from, axes.high + TOUCH],
        ];
        if convex_overlap(&quad, &rect) {
            return Some(
                Error::new(codes::INSERT_MEETS_WALL)
                    .with_arg("name", name)
                    .with_arg(
                        "other",
                        types::text(&other.values, 0).unwrap_or_default().to_owned(),
                    )
                    .with_detail("other_object", other.id.to_string()),
            );
        }
    }
    None
}

/// The rules of one insert in its document: the errors it breaks, naming the insert (and the
/// wall or the other insert involved). A record without a host wall has none here; the
/// containment rule reports it.
#[must_use]
pub fn problems(snapshot: &Snapshot, object: &Object) -> Vec<Error> {
    let mut out = Vec::new();
    let Some(wall_object) = object
        .header
        .owner
        .filter(|id| spatial::type_of(snapshot, *id).as_deref() == Some(WALL))
        .and_then(|id| spatial::get(snapshot, id))
    else {
        return out;
    };
    let (Some(item), Some(wall)) = (
        Insert::of(snapshot, object),
        Wall::of(snapshot, &wall_object),
    ) else {
        return out;
    };
    let name = types::text(&object.values, 0)
        .unwrap_or_default()
        .to_owned();
    if crate::curtain::is_curtain(snapshot, &wall_object) {
        out.push(Error::new(codes::CURTAIN_INSERT).with_arg("name", name));
        return out;
    }
    let host = types::text(&wall_object.values, 0)
        .unwrap_or_default()
        .to_owned();
    if !item.operation.allowed(item.kind) {
        out.push(
            Error::new(codes::INSERT_OPERATION)
                .with_arg("name", name.clone())
                .with_arg("operation", item.operation.name()),
        );
    }
    let bar = 2.0 * item.frame_width + MIN_CLEAR;
    if item.width < bar || item.height < bar {
        out.push(Error::new(codes::INSERT_FRAME).with_arg("name", name.clone()));
    }
    if let Some((low, high)) = clear_span_of(snapshot, &wall_object) {
        let (from, to) = item.extent();
        if from < low + SIDE_CLEARANCE || to > high - SIDE_CLEARANCE {
            out.push(
                Error::new(codes::INSERT_OUTSIDE)
                    .with_arg("name", name.clone())
                    .with_arg("wall", host.clone())
                    .with_arg("from", format!("{:.2}", low + SIDE_CLEARANCE))
                    .with_arg("to", format!("{:.2}", high - SIDE_CLEARANCE)),
            );
        }
    }
    if item.top() > wall.height - HEAD_CLEARANCE {
        out.push(
            Error::new(codes::INSERT_TOO_TALL)
                .with_arg("name", name.clone())
                .with_arg("wall", host)
                .with_arg("top", format!("{:.2}", wall.height - HEAD_CLEARANCE)),
        );
    }
    out.extend(
        meets_wall(snapshot, &wall_object, &wall, &item, &name)
            .map(|e| e.with_detail("object", object.id.to_string())),
    );
    for (other, theirs) in hosted(snapshot, wall_object.id) {
        let (a, b) = (item.extent(), theirs.extent());
        if other.id != object.id && a.0 < b.1 + MIN_GAP && b.0 < a.1 + MIN_GAP {
            out.push(
                Error::new(codes::INSERT_OVERLAP)
                    .with_arg("name", name.clone())
                    .with_arg(
                        "other",
                        types::text(&other.values, 0).unwrap_or_default().to_owned(),
                    ),
            );
            break;
        }
    }
    out
}

/// How far a neighbor's face may be from the wall's face and still count as touching it, in meters.
const TOUCH: f64 = 0.002;

/// Whether two convex polygons overlap by more than a sliver (separating-axis test).
fn convex_overlap(a: &[[f64; 2]], b: &[[f64; 2]]) -> bool {
    for poly in [a, b] {
        for k in 0..poly.len() {
            let (p, q) = (poly[k], poly[(k + 1) % poly.len()]);
            let axis = [p[1] - q[1], q[0] - p[0]];
            let length = axis[0].hypot(axis[1]);
            if length < 1e-12 {
                continue;
            }
            let span = |s: &[[f64; 2]]| {
                s.iter()
                    .map(|v| (v[0] * axis[0] + v[1] * axis[1]) / length)
                    .fold((f64::MAX, f64::MIN), |(lo, hi), d| (lo.min(d), hi.max(d)))
            };
            let (sa, sb) = (span(a), span(b));
            if sa.1 <= sb.0 + 1e-6 || sb.1 <= sa.0 + 1e-6 {
                return false;
            }
        }
    }
    true
}

/// The wall's axes: where the baseline starts, the direction along it, the left normal, the
/// extent of the body across the baseline and the height of the base.
pub(crate) struct Axes {
    pub(crate) origin: [f64; 2],
    pub(crate) u: [f64; 2],
    pub(crate) n: [f64; 2],
    pub(crate) low: f64,
    pub(crate) high: f64,
    pub(crate) base: f64,
}

impl Axes {
    pub(crate) fn new(wall: &Wall, level: f64) -> Result<Self> {
        let length = wall.length();
        if !(length >= types::MIN_WALL_LENGTH && length.is_finite()) {
            return Err(Error::new(codes::WALL_TOO_SHORT).with_arg("name", ""));
        }
        let u = [
            (wall.end[0] - wall.start[0]) / length,
            (wall.end[1] - wall.start[1]) / length,
        ];
        let (low, high) = wall.extent();
        Ok(Self {
            origin: wall.start,
            u,
            n: [-u[1], u[0]],
            low,
            high,
            base: level + wall.base_offset,
        })
    }

    /// The plan point `x` along the baseline and `y` to the left of it.
    pub(crate) fn at(&self, x: f64, y: f64) -> Point2 {
        Point2::new(
            self.origin[0] + self.u[0] * x + self.n[0] * y,
            self.origin[1] + self.u[1] * x + self.n[1] * y,
        )
    }
}

fn region(points: [Point2; 4], tolerance: &Tolerance) -> Result<Region> {
    let mut curves = Vec::with_capacity(4);
    for k in 0..4 {
        curves.push(Curve2::Segment(Segment2::new(
            points[k],
            points[(k + 1) % 4],
        )?));
    }
    Region::new(vec![RegionPart::new(
        Loop2::new(curves, tolerance)?,
        Vec::new(),
    )])
}

/// The outline of an opening across its wall, in the wall's frame (x along the baseline, y up):
/// a rectangle, or, for an opening with a rise, a rectangle with a circular arch on top.
fn profile(item: &Insert, from: f64, to: f64, tolerance: &Tolerance) -> Result<Region> {
    let rise = if item.rise > tolerance.linear && item.rise < item.height {
        item.rise
    } else {
        0.0
    };
    if rise == 0.0 {
        return region(
            [
                Point2::new(from, item.sill),
                Point2::new(to, item.sill),
                Point2::new(to, item.top()),
                Point2::new(from, item.top()),
            ],
            tolerance,
        );
    }
    let spring = item.top() - rise;
    // The sagitta over half the chord is the bulge.
    let bulge = 2.0 * rise / (to - from);
    let line = Polyline2::new(
        vec![
            PolylineVertex::new(Point2::new(from, item.sill), 0.0),
            PolylineVertex::new(Point2::new(to, item.sill), 0.0),
            PolylineVertex::new(Point2::new(to, spring), bulge),
            PolylineVertex::new(Point2::new(from, spring), 0.0),
        ],
        true,
    )?;
    let curves = line
        .segments()
        .map(|segment| match segment {
            PolylineSegment::Line(s) => Curve2::Segment(s),
            PolylineSegment::Arc(a) => Curve2::Arc(a),
        })
        .collect();
    Region::new(vec![RegionPart::new(
        Loop2::new(curves, tolerance)?,
        Vec::new(),
    )])
}

/// The wall solid `solid` (of `wall` standing on a level at `level`) with the openings of
/// `inserts` cut through its thickness, one opening each.
///
/// # Errors
/// The errors of the solid kernel: an opening outside the supported case (see
/// [`arc_geom::PrismTool`]) is refused, never cut wrongly.
pub fn cut(wall: &Wall, level: f64, solid: Polyhedron, inserts: &[Insert]) -> Result<Polyhedron> {
    if inserts.is_empty() {
        return Ok(solid);
    }
    let axes = Axes::new(wall, level)?;
    let tolerance = Tolerance::DEFAULT;
    // The prism runs along the frame's normal, which points to the right of the baseline.
    let frame = PlaneFrame::new(
        Point3::new(axes.origin[0], axes.origin[1], axes.base),
        Vector3::new(axes.u[0], axes.u[1], 0.0),
        Vector3::z(),
        &tolerance,
    )?;
    let mut current = solid;
    for (k, item) in inserts.iter().enumerate() {
        let (from, to) = item.extent();
        let profile = profile(item, from, to, &tolerance)?;
        let tool = PrismTool {
            profile: &profile,
            frame,
            start: -axes.high - OVERSHOOT,
            end: -axes.low + OVERSHOOT,
            chord: CHORD,
            opening: u32::try_from(k).unwrap_or(u32::MAX),
            allow_split: false,
        };
        current = current.subtract_prism(&tool, &tolerance, &Limits::DEFAULT)?;
    }
    Ok(current)
}

/// The solid of a wall record with the openings of its inserts cut: the joined solid minus its
/// openings. `None` when the record is not a readable wall.
#[must_use]
pub fn wall_solid(snapshot: &Snapshot, object: &Object) -> Option<Result<Polyhedron>> {
    let wall = Wall::of(snapshot, object)?;
    let level = object
        .header
        .owner
        .and_then(|id| spatial::get(snapshot, id))
        .map_or(0.0, |s| spatial::storey_elevation(snapshot, &s));
    let footprint =
        crate::join::footprint_of(snapshot, object).unwrap_or_else(|| wall.plain_footprint());
    let inserts = openings_of(snapshot, object.id);
    Some(
        gross_solid(snapshot, object, &wall, footprint, level)
            .and_then(|solid| cut(&wall, level, solid, &inserts)),
    )
}

/// The roof surface a wall record's top is attached to, when it is attached to a roof whose
/// surface can be derived.
#[must_use]
pub fn attached_roof(
    snapshot: &Snapshot,
    object: &Object,
) -> Option<(Arc<Object>, crate::roof::Surface)> {
    let target =
        crate::wall::attachment(&object.values).and_then(|id| spatial::get(snapshot, id))?;
    if spatial::type_of(snapshot, target.id).as_deref() != Some(crate::types::ROOF) {
        return None;
    }
    let surface = crate::roof::Derived::of(snapshot, &target).ok()?.surface;
    Some((target, surface))
}

/// The solid of a wall record without its openings: under the roof it is attached to when it can
/// follow it, else the prism of its own top.
///
/// # Errors
/// The solid kernel's errors.
pub fn gross_solid(
    snapshot: &Snapshot,
    object: &Object,
    wall: &Wall,
    footprint: [[f64; 2]; 4],
    level: f64,
) -> Result<Polyhedron> {
    if let Some((_, surface)) = attached_roof(snapshot, object)
        && let Ok(solid) = crate::wall::under_roof(wall, footprint, level, &surface)
    {
        return Ok(solid);
    }
    wall.solid_in(footprint, level)
}

/// The part of an insert a solid belongs to, which decides its color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// One of the four frame bars.
    Frame,
    /// The door leaf, closed.
    Panel,
    /// The glass pane of a window.
    Glass,
    /// The handle of a door leaf.
    Handle,
}

/// The solids of an insert standing in `wall` on a level at `level`: the four bars of the frame,
/// the leaf (door) or the pane (window) and, for a door, a handle on the side opposite the hinge
/// on the facing side of the leaf. The leaf and the pane are closed.
///
/// # Errors
/// The errors of the solid kernel for numbers it cannot make solids of.
pub fn pieces(wall: &Wall, level: f64, item: &Insert) -> Result<Vec<(Role, Polyhedron)>> {
    if item.kind == Kind::Opening {
        return Ok(Vec::new());
    }
    let axes = Axes::new(wall, level)?;
    let tolerance = Tolerance::DEFAULT;
    let boxed = |x: (f64, f64), y: (f64, f64), z: (f64, f64)| -> Result<Polyhedron> {
        let plan = region(
            [
                axes.at(x.0, y.0),
                axes.at(x.1, y.0),
                axes.at(x.1, y.1),
                axes.at(x.0, y.1),
            ],
            &tolerance,
        )?;
        prism(
            &plan,
            axes.base + z.0,
            z.1 - z.0,
            CHORD,
            &tolerance,
            &Limits::DEFAULT,
        )
    };
    let (x0, x1) = item.extent();
    let (z0, z1) = (item.sill, item.top());
    let fw = item.frame_width;
    let middle = (axes.low + axes.high) / 2.0;
    let half = item.frame_depth.min(wall.thickness) / 2.0;
    let depth = (middle - half, middle + half);
    let mut out = Vec::with_capacity(6);
    for (x, z) in [
        ((x0, x0 + fw), (z0, z1)),
        ((x1 - fw, x1), (z0, z1)),
        ((x0 + fw, x1 - fw), (z1 - fw, z1)),
        ((x0 + fw, x1 - fw), (z0, z0 + fw)),
    ] {
        out.push((Role::Frame, boxed(x, depth, z)?));
    }
    let clear = ((x0 + fw, x1 - fw), (z0 + fw, z1 - fw));
    let (role, thick) = match item.kind {
        Kind::Window => (Role::Glass, PANE),
        Kind::Door | Kind::Opening => (Role::Panel, LEAF),
    };
    let across = |offset: f64| (middle + offset - thick / 2.0, middle + offset + thick / 2.0);
    let (c0, c1) = clear.0;
    let mid = (c0 + c1) / 2.0;
    match item.operation {
        // Two leaves meeting in the middle.
        Operation::DoubleSwing => {
            out.push((role, boxed((c0, mid - GAP / 2.0), across(0.0), clear.1)?));
            out.push((role, boxed((mid + GAP / 2.0, c1), across(0.0), clear.1)?));
        }
        // Two panels side by side in two tracks, overlapping a little in the middle.
        Operation::Sliding => {
            let track = (half - thick / 2.0).clamp(0.0, thick);
            out.push((
                role,
                boxed((c0, mid + OVERLAP / 2.0), across(-track), clear.1)?,
            ));
            out.push((
                role,
                boxed((mid - OVERLAP / 2.0, c1), across(track), clear.1)?,
            ));
        }
        _ => out.push((role, boxed(clear.0, across(0.0), clear.1)?)),
    }
    if item.kind == Kind::Door
        && item.operation == Operation::SingleSwing
        && item.height >= 1.3
        && item.width >= 0.4
    {
        // On the facing side of the leaf, near the edge opposite the hinge.
        let at = item.distance - item.hinge_direction() * (item.width / 2.0 - fw - 0.1);
        let face = if item.facing == Facing::Exterior {
            middle + thick / 2.0
        } else {
            middle - thick / 2.0
        };
        let toward = if item.facing == Facing::Exterior {
            0.04
        } else {
            -0.04
        };
        let (y0, y1) = if toward > 0.0 {
            (face, face + toward)
        } else {
            (face + toward, face)
        };
        out.push((
            Role::Handle,
            boxed((at - 0.06, at + 0.06), (y0, y1), (z0 + 0.95, z0 + 1.05))?,
        ));
    }
    Ok(out)
}

/// The plan symbol of an insert, as polylines in project coordinates at a cut 1 m above the wall's
/// base: the opening's outline across the wall; for a door the leaf open at 90 degrees on the
/// facing side, hinged at the jamb the hand names, with the quarter-circle of its swing; for a
/// window the frame outline and the glass line in the wall's center.
///
/// # Errors
/// `ab.model.wall_too_short` for a wall without a length.
pub fn symbol(wall: &Wall, level: f64, item: &Insert) -> Result<Vec<Vec<[f64; 3]>>> {
    let axes = Axes::new(wall, level)?;
    let z = axes.base + 1.0;
    let at = |x: f64, y: f64| {
        let p = axes.at(x, y);
        [p.x, p.y, z]
    };
    let (x0, x1) = item.extent();
    let mut lines = vec![vec![
        at(x0, axes.low),
        at(x1, axes.low),
        at(x1, axes.high),
        at(x0, axes.high),
        at(x0, axes.low),
    ]];
    let fw = item.frame_width;
    let middle = (axes.low + axes.high) / 2.0;
    match item.kind {
        Kind::Opening => {}
        Kind::Window => {
            let half = item.frame_depth.min(wall.thickness) / 2.0;
            lines.push(vec![
                at(x0, middle - half),
                at(x1, middle - half),
                at(x1, middle + half),
                at(x0, middle + half),
                at(x0, middle - half),
            ]);
            if item.operation == Operation::Sliding {
                let mid = item.distance;
                lines.push(vec![
                    at(x0 + fw, middle - 0.015),
                    at(mid + 0.025, middle - 0.015),
                ]);
                lines.push(vec![
                    at(mid - 0.025, middle + 0.015),
                    at(x1 - fw, middle + 0.015),
                ]);
            } else {
                lines.push(vec![at(x0 + fw, middle), at(x1 - fw, middle)]);
            }
        }
        Kind::Door if item.operation == Operation::Sliding => {
            // The panel in front of the opening, slid half open toward the hinge side.
            let hinge = item.hinge_direction();
            let (face, out) = if item.facing == Facing::Exterior {
                (axes.high, 1.0)
            } else {
                (axes.low, -1.0)
            };
            let clear = item.width - 2.0 * fw;
            let from = item.distance - hinge * (clear / 2.0);
            let to = item.distance + hinge * clear;
            lines.push(vec![at(from, face + out * 0.03), at(to, face + out * 0.03)]);
            lines.push(vec![at(x0 + fw, middle), at(x1 - fw, middle)]);
        }
        Kind::Door if item.operation == Operation::DoubleSwing => {
            let (face, out) = if item.facing == Facing::Exterior {
                (axes.high, 1.0)
            } else {
                (axes.low, -1.0)
            };
            let leaf = (item.width - 2.0 * fw) / 2.0;
            for (hx, toward) in [(x0 + fw, 1.0), (x1 - fw, -1.0)] {
                lines.push(vec![at(hx, face), at(hx, face + out * leaf)]);
                let arc: Vec<[f64; 3]> = (0..=12)
                    .map(|k| {
                        let t = f64::from(k) / 12.0 * std::f64::consts::FRAC_PI_2;
                        at(hx + toward * leaf * t.cos(), face + out * leaf * t.sin())
                    })
                    .collect();
                lines.push(arc);
            }
        }
        Kind::Door => {
            let hinge = item.hinge_direction();
            let (hx, clear) = (
                item.distance + hinge * (item.width / 2.0 - fw),
                item.width - 2.0 * fw,
            );
            // The leaf starts on the facing face of the wall and runs out across it.
            let (face, out) = if item.facing == Facing::Exterior {
                (axes.high, 1.0)
            } else {
                (axes.low, -1.0)
            };
            lines.push(vec![at(hx, face), at(hx, face + out * clear)]);
            let arc: Vec<[f64; 3]> = (0..=12)
                .map(|k| {
                    let t = f64::from(k) / 12.0 * std::f64::consts::FRAC_PI_2;
                    at(hx - hinge * clear * t.cos(), face + out * clear * t.sin())
                })
                .collect();
            lines.push(arc);
        }
    }
    Ok(lines)
}

/// The polylines that mark a selected insert: its opening outline on the top of the wall and in
/// each face of the wall, in project coordinates.
///
/// # Errors
/// `ab.model.wall_too_short` for a wall without a length.
pub fn outline(wall: &Wall, level: f64, item: &Insert) -> Result<Vec<Vec<[f64; 3]>>> {
    let axes = Axes::new(wall, level)?;
    let (x0, x1) = item.extent();
    let at = |x: f64, y: f64, z: f64| {
        let p = axes.at(x, y);
        [p.x, p.y, z]
    };
    let top = axes.base + wall.height;
    let mut lines = vec![vec![
        at(x0, axes.low, top),
        at(x1, axes.low, top),
        at(x1, axes.high, top),
        at(x0, axes.high, top),
        at(x0, axes.low, top),
    ]];
    // Again a little higher, so that the outline stays over a highlight drawn on the wall's top.
    let raised: Vec<[f64; 3]> = lines[0].iter().map(|p| [p[0], p[1], p[2] + 0.02]).collect();
    lines.push(raised);
    for y in [axes.low, axes.high] {
        let (z0, z1) = (axes.base + item.sill, axes.base + item.top());
        lines.push(vec![
            at(x0, y, z0),
            at(x1, y, z0),
            at(x1, y, z1),
            at(x0, y, z1),
            at(x0, y, z0),
        ]);
    }
    Ok(lines)
}

/// The insert whose opening a view ray enters first, with the wall boxes of other walls competing:
/// the ray is `origin + t direction` in project coordinates. An insert beats its own wall (the
/// opening lies inside the wall's body); a wall in front of the insert beats it. The opening's
/// box spans the whole thickness of the wall, from the sill to the head.
#[must_use]
pub fn pick(snapshot: &Snapshot, ray: ([f64; 3], [f64; 3])) -> Option<arc_base::ObjectId> {
    let (o, d) = ray;
    let local = |wall: &Wall, level: f64| -> Option<([f64; 3], [f64; 3], Axes)> {
        let axes = Axes::new(wall, level).ok()?;
        let to = |v: [f64; 3], origin: bool| {
            let r = if origin {
                [
                    v[0] - axes.origin[0],
                    v[1] - axes.origin[1],
                    v[2] - axes.base,
                ]
            } else {
                v
            };
            [
                r[0] * axes.u[0] + r[1] * axes.u[1],
                r[0] * axes.n[0] + r[1] * axes.n[1],
                r[2],
            ]
        };
        Some((to(o, true), to(d, false), axes))
    };
    let mut walls = Vec::new();
    for object in spatial::objects(snapshot, WALL) {
        let Some(wall) = Wall::of(snapshot, &object) else {
            continue;
        };
        let level = object
            .header
            .owner
            .and_then(|id| spatial::get(snapshot, id))
            .map_or(0.0, |s| spatial::storey_elevation(snapshot, &s));
        if let Some((lo, ld, axes)) = local(&wall, level) {
            walls.push((object, wall, lo, ld, axes));
        }
    }
    let mut best: Option<(f64, arc_base::ObjectId, arc_base::ObjectId)> = None;
    for key in [DOOR, WINDOW, WALL_OPENING] {
        for object in spatial::objects(snapshot, key) {
            let (Some(item), Some((host, _, lo, ld, axes))) = (
                Insert::of(snapshot, &object),
                walls.iter().find(|w| Some(w.0.id) == object.header.owner),
            ) else {
                continue;
            };
            let (x0, x1) = item.extent();
            if let Some(t) = enters(
                *lo,
                *ld,
                [x0, axes.low, item.sill],
                [x1, axes.high, item.top()],
            ) && best.is_none_or(|(b, _, _)| t < b)
            {
                best = Some((t, object.id, host.id));
            }
        }
    }
    let (t, id, host) = best?;
    // A wall in front of the opening, other than its own, hides it.
    let hidden = walls.iter().any(|(object, wall, lo, ld, axes)| {
        object.id != host
            && enters(
                *lo,
                *ld,
                [0.0, axes.low, 0.0],
                [wall.length(), axes.high, wall.height],
            )
            .is_some_and(|w| w < t - 1e-9)
    });
    (!hidden).then_some(id)
}

/// Where the ray `o + t d` enters the box `min..max` for `t >= 0`; `None` when it misses.
#[must_use]
pub fn enters(o: [f64; 3], d: [f64; 3], min: [f64; 3], max: [f64; 3]) -> Option<f64> {
    let (mut near, mut far) = (0.0_f64, f64::INFINITY);
    for k in 0..3 {
        if d[k].abs() < 1e-12 {
            if o[k] < min[k] || o[k] > max[k] {
                return None;
            }
            continue;
        }
        let (a, b) = ((min[k] - o[k]) / d[k], (max[k] - o[k]) / d[k]);
        near = near.max(a.min(b));
        far = far.min(a.max(b));
    }
    (near <= far).then_some(near)
}
