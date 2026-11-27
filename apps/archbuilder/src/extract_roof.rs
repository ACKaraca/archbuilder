//! What the viewport draws for a roof: the triangles of its slab ([`archbuilder_model::roof`]),
//! shaded in a tile red. Nothing is stored: the surface derives from the record, its type and the
//! walls its edges follow, so a change of a wall, a storey, a level or a roof type (its eave cut)
//! extracts every roof again, and a change of a roof extracts the walls attached to roofs again,
//! whose tops follow it ([`dependents`]).

use arc_base::math::Point3;
use arc_base::{Cancel, Color, Limits, ObjectId, Result, Tolerance};
use arc_doc::events::Changes;
use arc_doc::{Object, Snapshot};
use arc_gpu::{Extraction, MeshInstance, MeshItem, RenderItem};
use arc_mesh::TriMesh;
use archbuilder_model::roof::Derived;
use archbuilder_model::spatial;
use archbuilder_model::types::{
    FLOOR, LEVEL, ROOF, ROOF_TYPE, SHAFT, STOREY, VERTICAL_OPENING, WALL,
};

/// The color of a roof (linear RGB): a tile red.
pub const COLOR: Color = Color {
    r: 0.62,
    g: 0.28,
    b: 0.20,
    a: 1.0,
};

/// The extraction of a roof: its slab as one mesh. A roof without a surface (refused by the
/// model's rules, so only in a damaged file) draws nothing.
///
/// # Errors
/// The solid kernel's errors for a surface it cannot make a solid of.
pub fn extract_roof(snapshot: &Snapshot, object: &Object, _cancel: &Cancel) -> Result<Extraction> {
    let Ok(derived) = Derived::of(snapshot, object) else {
        return Ok(Extraction::default());
    };
    let (solid, _) = derived.solid_cut()?;
    let first = derived.surface.outline.first();
    let origin = Point3::new(
        first.map_or(0.0, |p| p[0]),
        first.map_or(0.0, |p| p[1]),
        derived.surface.base,
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

/// The objects to extract again after a commit: every roof when a wall, a floor, a storey or a
/// level changed or went, and every wall attached to a roof or a floor when a roof or a floor
/// changed or went.
#[must_use]
pub fn dependents(snapshot: &Snapshot, changes: &Changes) -> Vec<ObjectId> {
    let Changes::Listed(by_type) = changes else {
        return Vec::new();
    };
    let mut roofs = false;
    let mut walls = false;
    for (key, group) in by_type {
        let any = !(group.created.is_empty()
            && group.modified.is_empty()
            && group.recreated.is_empty()
            && group.deleted.is_empty());
        match key.as_str() {
            WALL | STOREY | LEVEL | SHAFT | VERTICAL_OPENING | ROOF_TYPE => roofs |= any,
            ROOF | FLOOR => walls |= any,
            _ => {}
        }
    }
    let mut out = Vec::new();
    if roofs {
        out.extend(spatial::objects(snapshot, ROOF).iter().map(|o| o.id));
    }
    if walls || roofs {
        out.extend(
            spatial::objects(snapshot, WALL)
                .iter()
                .filter(|w| archbuilder_model::wall::attachment(&w.values).is_some())
                .map(|o| o.id),
        );
    }
    out
}
