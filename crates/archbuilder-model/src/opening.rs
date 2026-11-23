//! Shafts and vertical openings: cuts through floors, roofs and ceilings.
//!
//! A shaft has one footprint (the vertex and arc records of a floor boundary, so it can be
//! sketched with arcs and follow wall faces), a base and a top, each a level with an offset. It
//! cuts every floor, roof and ceiling whose body it passes through: the shaft's span must overlap
//! the element's body (the lowest to the highest elevation) by more than a tolerance, and an
//! element that only touches the span at a face is not cut. A vertical opening is a sketched cut
//! in one floor, roof or ceiling, which hosts it (the header owner). Wall openings are in
//! [`crate::insert`].
//!
//! The readers here never change a document; the elements read them ([`cuts`]) and list the
//! shafts among the objects their derived results depend on ([`shaft_deps`]).

use arc_base::math::{Point3, Vector3};
use arc_base::{Limits, ObjectId, Result, Tolerance};
use arc_doc::{Object, Snapshot, Value};
use arc_geom::{PlaneFrame, Polyhedron, PrismTool};

use crate::cleanup::Deps;
use crate::floor::{Boundary, FLAT_CHORD, Floor, SHAFT_SLOTS, Slots, VERTICAL_SLOTS};
use crate::spatial;
use crate::types::{self, SHAFT, VERTICAL_OPENING};

/// The distance a cut prism reaches beyond the element it cuts, in meters.
const OVERSHOOT: f64 = 0.1;
/// The overlap a shaft must have with an element's body to cut it, in meters.
const TOUCH: f64 = 1.0e-6;
/// The most openings one element is cut by.
pub const MAX_CUTS: usize = 1_000;

/// A shaft read from the document: its footprint and its span.
#[derive(Debug, Clone)]
pub struct ShaftSpan {
    /// The shaft.
    pub id: ObjectId,
    /// The storey it starts on.
    pub storey: Option<ObjectId>,
    /// The outer loop of the footprint, arcs flattened.
    pub outline: Vec<[f64; 2]>,
    /// The footprint with its arcs, for drawing and area.
    pub boundary: Boundary,
    /// The elevations of the base and the top, in meters.
    pub z: (f64, f64),
}

fn elevation(snapshot: &Snapshot, level: Option<ObjectId>) -> Option<f64> {
    types::meters(
        &spatial::get(snapshot, level?)?.values,
        types::level::ELEVATION,
    )
}

/// The shaft `object`; `None` when its record or footprint cannot be read.
#[must_use]
pub fn shaft_span(snapshot: &Snapshot, object: &Object) -> Option<ShaftSpan> {
    let footprint = Floor::read(&object.values, SHAFT_SLOTS)?;
    let boundary = footprint.boundary_in(snapshot)?;
    let outline = boundary.flattened(FLAT_CHORD).into_iter().next()?;
    let storey = object.header.owner.and_then(|s| spatial::get(snapshot, s));
    let level = |at: usize| match object.values.get(at) {
        Some(Some(Value::Reference(id))) => Some(*id),
        _ => None,
    };
    let own = storey
        .as_ref()
        .map_or(0.0, |s| spatial::storey_elevation(snapshot, s));
    let base = elevation(snapshot, level(types::shaft::BASE_LEVEL)).unwrap_or(own)
        + types::meters(&object.values, types::shaft::BASE_OFFSET).unwrap_or(0.0);
    let above = storey
        .as_ref()
        .and_then(|s| spatial::level_above(snapshot, s))
        .and_then(|l| types::meters(&l.values, types::level::ELEVATION));
    let top = elevation(snapshot, level(types::shaft::TOP_LEVEL))
        .or(above)
        .unwrap_or(own + crate::defaults::STOREY_HEIGHT)
        + types::meters(&object.values, types::shaft::TOP_OFFSET).unwrap_or(0.0);
    Some(ShaftSpan {
        id: object.id,
        storey: storey.map(|s| s.id),
        outline,
        boundary,
        z: (base, top),
    })
}

/// Every readable shaft of the document.
#[must_use]
pub fn shafts(snapshot: &Snapshot) -> Vec<ShaftSpan> {
    spatial::objects(snapshot, SHAFT)
        .iter()
        .take(MAX_CUTS)
        .filter_map(|o| shaft_span(snapshot, o))
        .collect()
}

/// Adds to `deps` what [`cuts`] reads besides the element and its own children: the shafts and,
/// when there are any, the levels and storeys that decide their spans.
pub(crate) fn shaft_deps(snapshot: &Snapshot, deps: &mut Deps) {
    let found = spatial::objects(snapshot, SHAFT);
    if found.is_empty() {
        return;
    }
    deps.extend(found);
    deps.extend(spatial::objects(snapshot, types::LEVEL));
    deps.extend(spatial::objects(snapshot, types::STOREY));
}

/// The outer loop of the boundary of `object`, whose fields are at `slots`.
#[must_use]
pub fn outline_of(snapshot: &Snapshot, object: &Object, slots: Slots) -> Option<Vec<[f64; 2]>> {
    Floor::read(&object.values, slots)?
        .boundary_in(snapshot)?
        .flattened(FLAT_CHORD)
        .into_iter()
        .next()
}

/// The outline of an element that can host a vertical opening: a floor or ceiling boundary, or a
/// roof's outline with its overhangs.
#[must_use]
pub fn host_outline(snapshot: &Snapshot, host: &Object) -> Option<Vec<[f64; 2]>> {
    match spatial::type_of(snapshot, host.id).as_deref() {
        Some(types::ROOF) => crate::roof::Derived::of(snapshot, host)
            .ok()
            .map(|d| d.surface.outline),
        _ => outline_of(snapshot, host, crate::floor::FLOOR_SLOTS),
    }
}

/// The footprints of the shafts and vertical openings that cut the element `host`, whose body
/// spans the elevations `z`.
#[must_use]
pub fn cuts(snapshot: &Snapshot, host: &Object, z: (f64, f64)) -> Vec<Vec<[f64; 2]>> {
    // Only a shaft whose footprint meets the host in plan cuts it. The outline is read without
    // the roof's surface (which reads the cuts).
    let area = match spatial::type_of(snapshot, host.id).as_deref() {
        Some(types::ROOF) => {
            crate::roof::Roof::from_values(&host.values).and_then(|r| r.footprint_in(snapshot))
        }
        _ => outline_of(snapshot, host, crate::floor::FLOOR_SLOTS),
    };
    let mut out: Vec<Vec<[f64; 2]>> = shafts(snapshot)
        .into_iter()
        .filter(|s| s.z.1 > z.0 + TOUCH && s.z.0 < z.1 - TOUCH)
        .filter(|s| {
            area.as_ref()
                .is_none_or(|a| crate::stair::outlines_overlap(a, &s.outline))
        })
        .map(|s| s.outline)
        .collect();
    out.extend(
        spatial::children(snapshot, VERTICAL_OPENING, host.id)
            .iter()
            .take(MAX_CUTS)
            .filter_map(|o| outline_of(snapshot, o, VERTICAL_SLOTS)),
    );
    out.truncate(MAX_CUTS);
    out
}

/// The solid `solid` with a vertical prism through the elevations `z` cut for each of `outlines`.
///
/// # Errors
/// The solid kernel's errors: a cut outside the supported case is refused, never cut wrongly.
pub fn cut_vertical(
    solid: Polyhedron,
    outlines: &[Vec<[f64; 2]>],
    z: (f64, f64),
) -> Result<Polyhedron> {
    if outlines.is_empty() {
        return Ok(solid);
    }
    let tolerance = Tolerance::DEFAULT;
    let frame = PlaneFrame::new(Point3::origin(), Vector3::x(), Vector3::y(), &tolerance)?;
    let mut current = solid;
    // A footprint that misses the solid in plan cuts nothing.
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for v in current.vertices() {
        lo = [lo[0].min(v.x), lo[1].min(v.y)];
        hi = [hi[0].max(v.x), hi[1].max(v.y)];
    }
    for (k, outline) in outlines.iter().enumerate() {
        let misses = outline.iter().all(|p| p[0] < lo[0])
            || outline.iter().all(|p| p[0] > hi[0])
            || outline.iter().all(|p| p[1] < lo[1])
            || outline.iter().all(|p| p[1] > hi[1]);
        if misses {
            continue;
        }
        let profile = Boundary::polygons(vec![outline.clone()]).region()?;
        let tool = PrismTool {
            profile: &profile,
            frame,
            start: z.0 - OVERSHOOT,
            end: z.1 + OVERSHOOT,
            chord: FLAT_CHORD,
            opening: u32::try_from(k).unwrap_or(u32::MAX),
            allow_split: true,
        };
        current = current.subtract_prism(&tool, &tolerance, &Limits::DEFAULT)?;
    }
    Ok(current)
}

/// Whether the shaft span `z` passes through the storey whose level is at `level`: the level lies
/// within the span, ends included.
#[must_use]
pub fn passes_level(z: (f64, f64), level: f64) -> bool {
    level >= z.0 - TOUCH && level <= z.1 + TOUCH
}
