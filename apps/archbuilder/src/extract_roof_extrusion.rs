//! What the viewport draws for a roof by extrusion: the triangles of its body
//! ([`archbuilder_model::roof_extrusion`]), in the tile red of a roof. Nothing is stored: the body
//! derives from the record, its type and its storey's level, so a change of a storey, a level or a
//! roof type extracts every roof by extrusion again ([`dependents`]).

use arc_base::math::Point3;
use arc_base::{Cancel, Limits, ObjectId, Result, Tolerance};
use arc_doc::events::Changes;
use arc_doc::{Object, Snapshot};
use arc_gpu::{Extraction, MeshInstance, MeshItem, RenderItem};
use arc_mesh::TriMesh;
use archbuilder_model::roof_extrusion::Derived;
use archbuilder_model::spatial;
use archbuilder_model::types::{LEVEL, ROOF_EXTRUSION, ROOF_TYPE, STOREY};

/// The extraction of a roof by extrusion: its body as one mesh. One the rules refuse draws nothing
/// (only in a damaged file).
///
/// # Errors
/// The solid kernel's errors for a body it cannot make a mesh of.
pub fn extract_roof_extrusion(
    snapshot: &Snapshot,
    object: &Object,
    _cancel: &Cancel,
) -> Result<Extraction> {
    let Ok(derived) = Derived::of(snapshot, object) else {
        return Ok(Extraction::default());
    };
    let solid = derived.solid()?;
    let corner = derived.world(0.0, 0.0, derived.roof.start);
    let origin = Point3::new(corner.x, corner.y, derived.z_range().0);
    let mesh = TriMesh::from_polyhedron(&solid, &Tolerance::DEFAULT, &Limits::DEFAULT)?;
    let mut extraction = Extraction::default();
    let geometry = extraction.add_geometry(mesh, origin);
    extraction.items.push(RenderItem::Mesh(MeshItem {
        geometry,
        origin,
        instances: vec![MeshInstance::new(crate::extract_roof::COLOR)],
    }));
    Ok(extraction)
}

/// The roofs by extrusion to extract again after a commit: all of them when a storey, a level or
/// a roof type changed.
#[must_use]
pub fn dependents(snapshot: &Snapshot, changes: &Changes) -> Vec<ObjectId> {
    let Changes::Listed(by_type) = changes else {
        return Vec::new();
    };
    let any = by_type.iter().any(|(key, g)| {
        matches!(key.as_str(), STOREY | LEVEL | ROOF_TYPE)
            && (!g.created.is_empty()
                || !g.modified.is_empty()
                || !g.recreated.is_empty()
                || !g.deleted.is_empty())
    });
    if !any {
        return Vec::new();
    }
    spatial::objects(snapshot, ROOF_EXTRUSION)
        .into_iter()
        .map(|o| o.id)
        .collect()
}
