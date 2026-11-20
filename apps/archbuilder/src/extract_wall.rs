//! What the viewport draws for a wall: the triangles of its derived solid, flat shaded. The solid
//! is derived here from the wall's authored data and the elevation of its storey's level
//! ([`archbuilder_model::wall::Wall::solid`]); nothing of it is stored. The mesh holds project
//! coordinates; the scene uploads it relative to the wall's start point at its base, so
//! single-precision display coordinates stay exact far from the project origin.
//!
//! A change of the wall re-extracts it, and so does a change of its storey or of the level of its
//! storey ([`dependents`]): the solid stands at the level's elevation.

use arc_base::math::Point3;
use std::collections::BTreeSet;

use arc_base::{Cancel, Color, Limits, ObjectId, Result, Tolerance};
use arc_doc::events::Changes;
use arc_doc::{Object, Snapshot};
use arc_gpu::{Extraction, MeshInstance, MeshItem, RenderItem};
use arc_mesh::TriMesh;
use archbuilder_model::spatial;
use archbuilder_model::types::{LEVEL, STOREY, WALL};
use archbuilder_model::wall::Wall;

/// The color of a wall (linear RGB).
pub const COLOR: Color = Color {
    r: 0.62,
    g: 0.58,
    b: 0.50,
    a: 1.0,
};

/// The extraction of a wall: its solid as one mesh.
///
/// # Errors
/// The solid kernel's errors for a wall whose data it cannot make a solid of (a record read
/// from a damaged file); the shell shows the wall as failed to extract.
pub fn extract_wall(snapshot: &Snapshot, object: &Object, _cancel: &Cancel) -> Result<Extraction> {
    let Some(wall) = Wall::of(snapshot, object) else {
        return Ok(Extraction::default());
    };
    let storey = object
        .header
        .owner
        .and_then(|id| spatial::get(snapshot, id));
    let elevation = storey
        .as_ref()
        .map_or(0.0, |s| spatial::storey_elevation(snapshot, s));
    let solid = wall.solid(elevation)?;
    let origin = Point3::new(wall.start[0], wall.start[1], elevation + wall.base_offset);
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

/// The walls to extract again after a commit: those of a storey that the commit changed or whose
/// level it changed (the elevation of the level places the solid).
#[must_use]
pub fn dependents(snapshot: &Snapshot, changes: &Changes) -> Vec<ObjectId> {
    let Changes::Listed(by_type) = changes else {
        return Vec::new();
    };
    let touched: BTreeSet<ObjectId> = by_type
        .iter()
        .filter(|(key, _)| [LEVEL, STOREY].contains(&key.as_str()))
        .flat_map(|(_, g)| g.created.iter().chain(&g.modified).chain(&g.recreated))
        .copied()
        .collect();
    if touched.is_empty() {
        return Vec::new();
    }
    let on_storey = spatial::objects(snapshot, STOREY)
        .into_iter()
        .filter(|s| {
            touched.contains(&s.id)
                || spatial::level_of(snapshot, s).is_some_and(|l| touched.contains(&l.id))
        })
        .flat_map(|s| spatial::walls(snapshot, s.id));
    // A wall whose top follows a changed level.
    let topped = spatial::objects(snapshot, WALL)
        .into_iter()
        .filter(|w| Wall::top_level(&w.values).is_some_and(|level| touched.contains(&level)));
    let found: BTreeSet<ObjectId> = on_storey.chain(topped).map(|w| w.id).collect();
    found.into_iter().collect()
}
