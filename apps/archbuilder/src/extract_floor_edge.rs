//! What the viewport draws for a floor edge: the triangles of the profile swept along its edges
//! ([`archbuilder_model::floor_edge`]), in a light concrete gray. Nothing is stored: the sweep
//! derives from the record, its floor, the walls the floor's edges follow and the level, so a
//! change of any of them extracts it again ([`dependents`]).

use arc_base::math::Point3;
use arc_base::{Cancel, Color, Limits, ObjectId, Result, Tolerance};
use arc_doc::events::Changes;
use arc_doc::{Object, Snapshot};
use arc_gpu::{Extraction, MeshInstance, MeshItem, RenderItem};
use arc_mesh::TriMesh;
use archbuilder_model::spatial;
use archbuilder_model::types::{FLOOR, FLOOR_EDGE, LEVEL, SHAFT, STOREY, VERTICAL_OPENING, WALL};

/// The color of a floor edge (linear RGB): a light concrete gray.
pub const COLOR: Color = Color {
    r: 0.62,
    g: 0.62,
    b: 0.60,
    a: 1.0,
};

/// The extraction of a floor edge: its swept body as one mesh. One that cannot be built draws
/// nothing (the rules refuse it, so only in a damaged file).
///
/// # Errors
/// The solid kernel's errors for a body it cannot make a mesh of.
pub fn extract_floor_edge(
    snapshot: &Snapshot,
    object: &Object,
    _cancel: &Cancel,
) -> Result<Extraction> {
    let Some(derived) = archbuilder_model::floor_edge::Derived::of(snapshot, object) else {
        return Ok(Extraction::default());
    };
    let Some(solid) = derived.solid()? else {
        return Ok(Extraction::default());
    };
    let first = derived.chains.first().and_then(|c| c.points.first());
    let origin = Point3::new(
        first.map_or(0.0, |p| p[0]),
        first.map_or(0.0, |p| p[1]),
        derived.z_range().map_or(0.0, |z| z.0),
    );
    let mesh = TriMesh::from_polyhedron(&solid, &Tolerance::DEFAULT, &Limits::DEFAULT)?;
    let mut extraction = Extraction::default();
    let geometry = extraction.add_geometry(mesh, origin);
    extraction.items.push(RenderItem::Mesh(MeshItem {
        geometry,
        origin,
        instances: vec![MeshInstance::new(COLOR)],
    }));
    Ok(extraction)
}

/// The floor edges to extract again after a commit: all of them when a floor, a wall, an opening,
/// a storey or a level changed (an edge follows its floor, which follows the walls), none else
/// (a floor edge that changed itself is extracted by the shell).
#[must_use]
pub fn dependents(snapshot: &Snapshot, changes: &Changes) -> Vec<ObjectId> {
    let Changes::Listed(by_type) = changes else {
        return Vec::new();
    };
    let any = by_type.iter().any(|(key, group)| {
        matches!(
            key.as_str(),
            WALL | FLOOR | STOREY | LEVEL | SHAFT | VERTICAL_OPENING
        ) && (!group.created.is_empty()
            || !group.modified.is_empty()
            || !group.recreated.is_empty()
            || !group.deleted.is_empty())
    });
    if !any {
        return Vec::new();
    }
    spatial::objects(snapshot, FLOOR_EDGE)
        .into_iter()
        .map(|e| e.id)
        .collect()
}
