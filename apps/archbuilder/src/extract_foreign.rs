//! What the viewport draws for a foreign element of an import: its mesh, flat shaded in one
//! neutral color. The mesh is stored in project coordinates, so the extraction only decodes it.

use arc_base::math::Point3;
use arc_base::{Cancel, Color, Limits, Result};
use arc_doc::{Object, Snapshot, Value};
use arc_gpu::{Extraction, MeshInstance, MeshItem, RenderItem};
use arc_mesh::TriMesh;
use archbuilder_model::types::foreign_element;

/// The color of a foreign element (linear RGB): a muted grey-blue, so that what the import could
/// not model reads as a reference.
const COLOR: Color = Color {
    r: 0.45,
    g: 0.5,
    b: 0.58,
    a: 1.0,
};

/// The extraction of a foreign element: one mesh; nothing for an element without one.
///
/// # Errors
/// The decoding errors of the mesh.
pub fn extract_foreign(
    _snapshot: &Snapshot,
    object: &Object,
    _cancel: &Cancel,
) -> Result<Extraction> {
    let Some(Some(Value::Geometry(bytes))) = object.values.get(foreign_element::MESH) else {
        return Ok(Extraction::default());
    };
    let mesh = TriMesh::decode(&bytes.0, &Limits::DEFAULT)?;
    // Positions are measured from a point of the mesh, which keeps far coordinates exact.
    let origin = mesh.positions().first().map_or_else(
        || Point3::new(0.0, 0.0, 0.0),
        |p| Point3::new(p.x, p.y, p.z),
    );
    let mut extraction = Extraction::default();
    let geometry = extraction.add_geometry(mesh, origin);
    extraction.items.push(RenderItem::Mesh(MeshItem {
        geometry,
        origin,
        instances: vec![MeshInstance::new(COLOR)],
    }));
    Ok(extraction)
}
