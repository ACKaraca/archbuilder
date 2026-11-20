//! What the viewport draws for a floor: the triangles of the slab, a prism of its derived boundary
//! ([`archbuilder_model::floor::Derived`]) hanging down from the top, flat shaded in a light tint
//! so that in plan it reads as a filled area below the walls. Nothing is stored: the boundary
//! derives from the record and from the walls its edges follow, so a change of such a wall, of
//! the storey or of its level extracts the floor again ([`dependents`]).

use std::collections::BTreeSet;

use arc_base::math::Point3;
use arc_base::{Cancel, Color, Limits, ObjectId, Result, Tolerance};
use arc_doc::events::Changes;
use arc_doc::{Object, Snapshot};
use arc_gpu::{Extraction, MeshInstance, MeshItem, RenderItem};
use arc_mesh::TriMesh;
use archbuilder_model::floor::Derived;
use archbuilder_model::spatial;
use archbuilder_model::types::{FLOOR, LEVEL, STOREY, WALL};

/// The color of a floor (linear RGB): a light blue-gray.
pub const COLOR: Color = Color {
    r: 0.60,
    g: 0.70,
    b: 0.80,
    a: 1.0,
};

/// The extraction of a floor: its slab as one mesh.
///
/// # Errors
/// The solid kernel's errors for a record it cannot make a solid of (a floor read from a damaged
/// file); the shell shows the floor as failed to extract.
pub fn extract_floor(snapshot: &Snapshot, object: &Object, _cancel: &Cancel) -> Result<Extraction> {
    let Some(floor) = Derived::of(snapshot, object) else {
        return Ok(Extraction::default());
    };
    let solid = floor.solid()?;
    let first = floor.boundary.loops.first().and_then(|l| l.first());
    let origin = Point3::new(
        first.map_or(0.0, |p| p[0]),
        first.map_or(0.0, |p| p[1]),
        floor.top - floor.floor.thickness,
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

/// The floors to extract again after a commit: those on a storey that has a changed or deleted
/// wall (an associated edge follows it), a changed storey or a changed level.
#[must_use]
pub fn dependents(snapshot: &Snapshot, changes: &Changes) -> Vec<ObjectId> {
    let Changes::Listed(by_type) = changes else {
        return Vec::new();
    };
    let mut storeys: BTreeSet<ObjectId> = BTreeSet::new();
    let mut levels: BTreeSet<ObjectId> = BTreeSet::new();
    let mut deleted_wall = false;
    for (key, group) in by_type {
        let live = group
            .created
            .iter()
            .chain(&group.modified)
            .chain(&group.recreated);
        match key.as_str() {
            WALL => {
                storeys.extend(live.filter_map(|w| spatial::get(snapshot, *w)?.header.owner));
                deleted_wall |= !group.deleted.is_empty();
            }
            STOREY => storeys.extend(live),
            LEVEL => levels.extend(live),
            _ => {}
        }
    }
    spatial::objects(snapshot, FLOOR)
        .into_iter()
        .filter(|f| {
            // A deleted wall is gone from the snapshot: its floors changed with it, but a floor
            // that lost nothing still extracts again, which is cheap.
            deleted_wall
                || f.header.owner.is_some_and(|s| {
                    storeys.contains(&s)
                        || spatial::get(snapshot, s)
                            .and_then(|o| spatial::level_of(snapshot, &o))
                            .is_some_and(|l| levels.contains(&l.id))
                })
        })
        .map(|f| f.id)
        .collect()
}
