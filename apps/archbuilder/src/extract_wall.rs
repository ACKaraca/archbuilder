//! What the viewport draws for a wall: the triangles of its derived solid, flat shaded. The solid
//! is derived here from the wall's authored data and the elevation of its storey's level
//! ([`archbuilder_model::wall::Wall::solid`]); nothing of it is stored. The mesh holds project
//! coordinates; the scene uploads it relative to the wall's start point at its base, so
//! single-precision display coordinates stay exact far from the project origin.
//!
//! The solid is that of the wall's joined footprint ([`archbuilder_model::join`]), derived from the
//! walls of its storey, with the openings of the wall's doors and windows cut
//! ([`archbuilder_model::insert::wall_solid`]). A change of the wall re-extracts it, and so does a change of its storey or
//! of the level of its storey, or of any wall on its storey ([`dependents`]): the solid stands at
//! the level's elevation and its ends follow the walls it meets.

use arc_base::math::Point3;
use std::collections::{BTreeMap, BTreeSet};

use arc_base::{Cancel, Color, Limits, ObjectId, Result, Tolerance};
use arc_doc::events::Changes;
use arc_doc::{Object, Snapshot};
use arc_gpu::{Extraction, MeshInstance, MeshItem, RenderItem};
use arc_mesh::TriMesh;
use archbuilder_model::spatial;
use archbuilder_model::types::{DOOR, LEVEL, STOREY, WALL, WINDOW};
use archbuilder_model::wall::Wall;

/// The color of a wall (linear RGB).
pub const COLOR: Color = Color {
    r: 0.62,
    g: 0.58,
    b: 0.50,
    a: 1.0,
};

/// The storey of each wall extracted, so that the deletion of a wall, which the snapshot no longer
/// holds, still names the storey whose other walls join differently now.
static OWNERS: std::sync::Mutex<BTreeMap<ObjectId, ObjectId>> =
    std::sync::Mutex::new(BTreeMap::new());

/// The wall of each door and window extracted, so that the deletion of an insert, which the
/// snapshot no longer holds, still names the wall whose opening closes.
static HOSTS: std::sync::Mutex<BTreeMap<ObjectId, ObjectId>> =
    std::sync::Mutex::new(BTreeMap::new());

/// Remembers the wall that hosts the insert `insert`.
pub fn remember_host(insert: ObjectId, wall: ObjectId) {
    if let Ok(mut hosts) = HOSTS.lock() {
        if hosts.len() >= MAX_OWNERS {
            hosts.clear();
        }
        hosts.insert(insert, wall);
    }
}

/// The most walls whose storey is remembered.
const MAX_OWNERS: usize = 1_000_000;

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
    if let (Some(owner), Ok(mut owners)) = (object.header.owner, OWNERS.lock()) {
        if owners.len() >= MAX_OWNERS {
            owners.clear();
        }
        owners.insert(object.id, owner);
    }
    let Some(solid) = archbuilder_model::insert::wall_solid(snapshot, object) else {
        return Ok(Extraction::default());
    };
    let solid = solid?;
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
/// level it changed (the elevation of the level places the solid), those of the storeys of walls
/// the commit created, changed or deleted (the ends of walls that meet follow each other), and
/// the walls whose top follows a changed level. A deleted wall is gone from the snapshot, so its
/// storey is the one remembered when it was extracted; a deleted wall never extracted extracts
/// the walls of every storey again.
#[must_use]
pub fn dependents(snapshot: &Snapshot, changes: &Changes) -> Vec<ObjectId> {
    let Changes::Listed(by_type) = changes else {
        return Vec::new();
    };
    let live = |key: &str| -> BTreeSet<ObjectId> {
        by_type
            .iter()
            .filter(|(k, _)| k.as_str() == key)
            .flat_map(|(_, g)| g.created.iter().chain(&g.modified).chain(&g.recreated))
            .copied()
            .collect()
    };
    let touched: BTreeSet<ObjectId> = [LEVEL, STOREY].into_iter().flat_map(live).collect();
    let changed_walls = live(WALL);
    let changed_inserts: BTreeSet<ObjectId> = [DOOR, WINDOW].into_iter().flat_map(live).collect();
    let deleted_of = |keys: &[&str]| -> Vec<ObjectId> {
        by_type
            .iter()
            .filter(|(k, _)| keys.contains(&k.as_str()))
            .flat_map(|(_, g)| g.deleted.iter().copied())
            .collect()
    };
    let deleted = deleted_of(&[WALL]);
    let deleted_inserts = deleted_of(&[DOOR, WINDOW]);
    if touched.is_empty()
        && changed_walls.is_empty()
        && deleted.is_empty()
        && changed_inserts.is_empty()
        && deleted_inserts.is_empty()
    {
        return Vec::new();
    }
    let mut owners: BTreeSet<ObjectId> = changed_walls
        .iter()
        .filter_map(|w| spatial::get(snapshot, *w).and_then(|o| o.header.owner))
        .collect();
    let mut every_storey = false;
    if let Ok(known) = OWNERS.lock() {
        for id in &deleted {
            match known.get(id) {
                Some(owner) => {
                    owners.insert(*owner);
                }
                None => every_storey = true,
            }
        }
    } else {
        every_storey = !deleted.is_empty();
    }
    let on_storey = spatial::objects(snapshot, STOREY)
        .into_iter()
        .filter(|s| {
            every_storey
                || touched.contains(&s.id)
                || owners.contains(&s.id)
                || spatial::level_of(snapshot, s).is_some_and(|l| touched.contains(&l.id))
        })
        .flat_map(|s| spatial::walls(snapshot, s.id));
    // A wall whose top follows a changed level.
    let topped = spatial::objects(snapshot, WALL)
        .into_iter()
        .filter(|w| Wall::top_level(&w.values).is_some_and(|level| touched.contains(&level)));
    // The wall of an insert that changed or went: its opening is cut or closed.
    let hosts: Vec<ObjectId> = changed_inserts
        .iter()
        .filter_map(|i| spatial::get(snapshot, *i).and_then(|o| o.header.owner))
        .chain(
            HOSTS
                .lock()
                .map(|known| {
                    deleted_inserts
                        .iter()
                        .filter_map(|i| known.get(i).copied())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
        )
        .filter(|w| spatial::get(snapshot, *w).is_some())
        .collect();
    let walls: BTreeSet<ObjectId> = on_storey.chain(topped).map(|w| w.id).chain(hosts).collect();
    // The inserts follow their wall: its ends, its base and its height place them.
    let inserts: Vec<ObjectId> = walls
        .iter()
        .flat_map(|w| {
            [DOOR, WINDOW]
                .into_iter()
                .flat_map(|key| spatial::children(snapshot, key, *w))
        })
        .map(|i| i.id)
        .collect();
    walls.into_iter().chain(inserts).collect()
}
