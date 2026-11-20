//! What the viewport draws for a door or a window: the triangles of the solids of its frame, its
//! leaf or glass pane and, for a door, its handle ([`archbuilder_model::insert::pieces`]), each
//! flat shaded in the color of its role. Derived from the record, the host wall and the elevation of
//! the wall's storey; nothing is stored. The wall's own mesh has the opening cut
//! ([`crate::extract_wall`]); the insert's solids sit in that opening, with the leaf closed.
//! The meshes hold project coordinates and are uploaded relative to the insert's base.

use arc_base::math::Point3;
use arc_base::{Cancel, Color, Limits, Result, Tolerance};
use arc_doc::{Object, Snapshot};
use arc_gpu::{Extraction, MeshInstance, MeshItem, RenderItem};
use arc_mesh::TriMesh;
use archbuilder_model::insert::{Insert, Role, pieces};
use archbuilder_model::spatial;
use archbuilder_model::wall::Wall;

/// The color of a frame bar (linear RGB).
const FRAME: Color = Color {
    r: 0.80,
    g: 0.78,
    b: 0.72,
    a: 1.0,
};
/// The color of a door leaf.
const PANEL: Color = Color {
    r: 0.45,
    g: 0.27,
    b: 0.12,
    a: 1.0,
};
/// The color of a glass pane: a light blue, apart from the frame and the wall.
const GLASS: Color = Color {
    r: 0.45,
    g: 0.72,
    b: 0.92,
    a: 1.0,
};
/// The color of a door handle.
const HANDLE: Color = Color {
    r: 0.85,
    g: 0.70,
    b: 0.20,
    a: 1.0,
};

/// The color of a part by its role.
#[must_use]
pub fn color(role: Role) -> Color {
    match role {
        Role::Frame => FRAME,
        Role::Panel => PANEL,
        Role::Glass => GLASS,
        Role::Handle => HANDLE,
    }
}

/// The extraction of a door or a window: one mesh per part.
///
/// # Errors
/// The solid kernel's errors for a record it cannot make solids of; the shell shows the insert as
/// failed to extract.
pub fn extract_insert(
    snapshot: &Snapshot,
    object: &Object,
    _cancel: &Cancel,
) -> Result<Extraction> {
    let Some(host) = object
        .header
        .owner
        .and_then(|id| spatial::get(snapshot, id))
    else {
        return Ok(Extraction::default());
    };
    crate::extract_wall::remember_host(object.id, host.id);
    let (Some(item), Some(wall)) = (Insert::of(snapshot, object), Wall::of(snapshot, &host)) else {
        return Ok(Extraction::default());
    };
    let elevation = host
        .header
        .owner
        .and_then(|id| spatial::get(snapshot, id))
        .map_or(0.0, |s| spatial::storey_elevation(snapshot, &s));
    let origin = Point3::new(wall.start[0], wall.start[1], elevation + wall.base_offset);
    let mut extraction = Extraction::default();
    for (role, solid) in pieces(&wall, elevation, &item)? {
        let mesh = TriMesh::from_polyhedron(&solid, &Tolerance::DEFAULT, &Limits::DEFAULT)?;
        let geometry = extraction.add_geometry(mesh, origin);
        extraction.items.push(RenderItem::Mesh(MeshItem {
            geometry,
            origin,
            instances: vec![MeshInstance::new(color(role))],
        }));
    }
    Ok(extraction)
}
