//! What the viewport draws for roof edge elements, dormers and elevators: the swept profile of a
//! fascia or gutter, the walls and the small roof of a dormer, and the car of an elevator
//! ([`archbuilder_model::roof_edge`], [`archbuilder_model::dormer`],
//! [`archbuilder_model::elevator`]). Nothing is stored: each derives from its record, its roof or
//! storey and the levels, so a change of any of them extracts it again ([`dependents`]).

use arc_base::math::Point3;
use arc_base::{Cancel, Color, Limits, ObjectId, Result, Tolerance};
use arc_doc::events::Changes;
use arc_doc::{Object, Snapshot};
use arc_geom::Polyhedron;
use arc_gpu::{Extraction, MeshInstance, MeshItem, RenderItem};
use arc_mesh::TriMesh;
use archbuilder_model::dormer::PartKind;
use archbuilder_model::spatial;
use archbuilder_model::types::{DORMER, ELEVATOR, LEVEL, ROOF, ROOF_EDGE, STOREY, WALL};

const FASCIA: Color = Color {
    r: 0.80,
    g: 0.78,
    b: 0.72,
    a: 1.0,
};
const GUTTER: Color = Color {
    r: 0.45,
    g: 0.48,
    b: 0.52,
    a: 1.0,
};
const DORMER_WALL: Color = Color {
    r: 0.78,
    g: 0.74,
    b: 0.68,
    a: 1.0,
};
const DORMER_ROOF: Color = Color {
    r: 0.55,
    g: 0.30,
    b: 0.25,
    a: 1.0,
};
const CAR: Color = Color {
    r: 0.55,
    g: 0.58,
    b: 0.62,
    a: 1.0,
};

/// The height of the car drawn in the viewport, in meters.
const CAR_HEIGHT: f64 = 2.4;

fn add(extraction: &mut Extraction, solid: &Polyhedron, color: Color) -> Result<()> {
    let b = solid.bounding_box();
    let origin = Point3::new(b.min().x, b.min().y, b.min().z);
    let mesh = TriMesh::from_polyhedron(solid, &Tolerance::DEFAULT, &Limits::DEFAULT)?;
    let geometry = extraction.add_geometry(mesh, origin);
    extraction.items.push(RenderItem::Mesh(MeshItem {
        geometry,
        origin,
        instances: vec![MeshInstance::new(color)],
    }));
    Ok(())
}

/// The swept body of a roof edge element.
///
/// # Errors
/// The solid kernel's errors for a body it cannot make a mesh of.
pub fn extract_roof_edge(
    snapshot: &Snapshot,
    object: &Object,
    _cancel: &Cancel,
) -> Result<Extraction> {
    let mut extraction = Extraction::default();
    let Some(derived) = archbuilder_model::roof_edge::Derived::of(snapshot, object) else {
        return Ok(extraction);
    };
    if let Some(solid) = derived.solid()? {
        let color = match derived.element.role {
            archbuilder_model::roof_edge::Role::Fascia => FASCIA,
            archbuilder_model::roof_edge::Role::Gutter => GUTTER,
        };
        add(&mut extraction, &solid, color)?;
    }
    Ok(extraction)
}

/// The walls and the small roof of a dormer.
///
/// # Errors
/// The solid kernel's errors for a body it cannot make a mesh of.
pub fn extract_dormer(
    snapshot: &Snapshot,
    object: &Object,
    _cancel: &Cancel,
) -> Result<Extraction> {
    let mut extraction = Extraction::default();
    let Ok(derived) = archbuilder_model::dormer::Derived::of(snapshot, object) else {
        return Ok(extraction);
    };
    for (kind, solid) in &derived.parts {
        let color = if *kind == PartKind::Roof {
            DORMER_ROOF
        } else {
            DORMER_WALL
        };
        add(&mut extraction, solid, color)?;
    }
    Ok(extraction)
}

/// The car of an elevator at its base level.
///
/// # Errors
/// The solid kernel's errors for a body it cannot make a mesh of.
pub fn extract_elevator(
    snapshot: &Snapshot,
    object: &Object,
    _cancel: &Cancel,
) -> Result<Extraction> {
    let mut extraction = Extraction::default();
    let Some(derived) = archbuilder_model::elevator::derived(snapshot, object) else {
        return Ok(extraction);
    };
    let region = archbuilder_model::floor::Boundary::polygons(vec![derived.elevator.car_outline()])
        .region()?;
    let solid = arc_geom::prism(
        &region,
        derived.base,
        CAR_HEIGHT,
        0.01,
        &Tolerance::DEFAULT,
        &Limits::DEFAULT,
    )?;
    add(&mut extraction, &solid, CAR)?;
    Ok(extraction)
}

/// The elements to extract again after a commit: all of them when a roof, a wall, a storey, a
/// level or one of their own kind changed (a dormer follows its roof's surface, an elevator the
/// levels), none else.
#[must_use]
pub fn dependents(snapshot: &Snapshot, changes: &Changes) -> Vec<ObjectId> {
    let Changes::Listed(by_type) = changes else {
        return Vec::new();
    };
    let any = by_type.iter().any(|(key, group)| {
        matches!(key.as_str(), WALL | ROOF | STOREY | LEVEL)
            && (!group.created.is_empty()
                || !group.modified.is_empty()
                || !group.recreated.is_empty()
                || !group.deleted.is_empty())
    });
    if !any {
        return Vec::new();
    }
    [ROOF_EDGE, DORMER, ELEVATOR]
        .into_iter()
        .flat_map(|key| spatial::objects(snapshot, key))
        .map(|e| e.id)
        .collect()
}
