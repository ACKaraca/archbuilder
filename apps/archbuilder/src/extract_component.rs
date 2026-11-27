//! What the viewport draws for a family instance: the triangles of the solids its definition
//! shows in 3D at high detail, one mesh per material slot, flat shaded. Derived from the
//! definition, the type's and instance's values, the document variables and the storey's level;
//! nothing is stored. A change of a loaded family, a family type, a variable or a level extracts
//! the components again ([`dependents`]).

use arc_base::math::Point3;
use arc_base::{Cancel, Color, Limits, ObjectId, Result, Tolerance};
use arc_doc::events::Changes;
use arc_doc::{Object, Snapshot};
use arc_gpu::{Extraction, MeshInstance, MeshItem, RenderItem};
use arc_mesh::TriMesh;
use archbuilder_model::family::def::{Detail, ViewKind};
use archbuilder_model::family::loaded::{Component, place};
use archbuilder_model::spatial;
use archbuilder_model::types::{COMPONENT, FAMILY, FAMILY_TYPE, LEVEL, STOREY, VARIABLE};

/// The color of a material slot (linear RGB): a muted tone chosen from the slot's name, so that
/// the slots of one family differ and a slot has the same color everywhere.
#[must_use]
pub fn color(slot: &str) -> Color {
    let h = slot
        .bytes()
        .fold(7_u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
    let tone =
        |shift: u32| 0.45 + f32::from(u8::try_from((h >> shift) & 0x3f).unwrap_or(0)) / 255.0;
    Color {
        r: tone(0),
        g: tone(6),
        b: tone(12) * 0.9,
        a: 1.0,
    }
}

/// The extraction of a component: one mesh per solid shown in 3D. A component that does not
/// evaluate is an error the shell shows as failed to extract.
///
/// # Errors
/// The errors of [`place`] and of the mesh.
pub fn extract_component(
    snapshot: &Snapshot,
    object: &Object,
    _cancel: &Cancel,
) -> Result<Extraction> {
    let Some(component) = Component::of(object) else {
        return Ok(Extraction::default());
    };
    let placed = place(snapshot, &component)?;
    let elevation = component
        .storey
        .and_then(|s| spatial::get(snapshot, s))
        .map_or(0.0, |s| spatial::storey_elevation(snapshot, &s));
    let origin = Point3::new(
        component.origin[0],
        component.origin[1],
        elevation + component.offset,
    );
    let mut extraction = Extraction::default();
    for solid in placed
        .solids
        .iter()
        .filter(|s| s.shown.at(Detail::High, ViewKind::ThreeD))
    {
        let mesh = TriMesh::from_polyhedron(&solid.solid, &Tolerance::DEFAULT, &Limits::DEFAULT)?;
        let geometry = extraction.add_geometry(mesh, origin);
        extraction.items.push(RenderItem::Mesh(MeshItem {
            geometry,
            origin,
            instances: vec![MeshInstance::new(color(&solid.material))],
        }));
    }
    Ok(extraction)
}

/// The components to extract again after a commit: all of them when a loaded family, a family
/// type, a variable, a storey or a level changed.
#[must_use]
pub fn dependents(snapshot: &Snapshot, changes: &Changes) -> Vec<ObjectId> {
    let Changes::Listed(by_type) = changes else {
        return Vec::new();
    };
    let any = by_type.iter().any(|(key, group)| {
        matches!(
            key.as_str(),
            FAMILY | FAMILY_TYPE | VARIABLE | STOREY | LEVEL
        ) && (!group.created.is_empty()
            || !group.modified.is_empty()
            || !group.recreated.is_empty()
            || !group.deleted.is_empty())
    });
    if !any {
        return Vec::new();
    }
    spatial::objects(snapshot, COMPONENT)
        .into_iter()
        .map(|c| c.id)
        .collect()
}
