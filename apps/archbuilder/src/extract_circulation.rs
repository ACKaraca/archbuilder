//! What the viewport draws for a ramp (its body in the concrete gray of a stair) and for a
//! railing (rails, posts and balusters in the railing tone), all derived from the records, the
//! levels and, for a hosted railing, its host. A change of a level or a storey extracts every ramp
//! and railing again ([`dependents`]); a change of a ramp or a stair extracts the railings (they
//! may be hosted on it) and, for a ramp, the floors (the opening it cuts); a change of a railing
//! type extracts the railings.

use arc_base::math::Point3;
use arc_base::{Cancel, Limits, ObjectId, Result, Tolerance};
use arc_doc::events::Changes;
use arc_doc::{Object, Snapshot};
use arc_gpu::{Extraction, MeshInstance, MeshItem, RenderItem};
use arc_mesh::TriMesh;
use archbuilder_model::spatial;
use archbuilder_model::types::{FLOOR, LEVEL, RAILING, RAILING_TYPE, RAMP, STAIR, STOREY};

use crate::extract_stair::{COLOR, RAILING as RAILING_COLOR};

fn add(
    extraction: &mut Extraction,
    solids: &[arc_geom::Polyhedron],
    color: arc_base::Color,
    base: f64,
    cancel: &Cancel,
) -> Result<()> {
    for solid in solids {
        cancel.check()?;
        let mesh = TriMesh::from_polyhedron(solid, &Tolerance::DEFAULT, &Limits::DEFAULT)?;
        let origin = solid
            .vertices()
            .first()
            .copied()
            .unwrap_or_else(|| Point3::new(0.0, 0.0, base));
        let geometry = extraction.add_geometry(mesh, origin);
        extraction.items.push(RenderItem::Mesh(MeshItem {
            geometry,
            origin,
            instances: vec![MeshInstance::new(color)],
        }));
    }
    Ok(())
}

/// The extraction of a ramp: its body. A ramp without a layout draws nothing.
///
/// # Errors
/// The solid kernel's errors.
pub fn extract_ramp(snapshot: &Snapshot, object: &Object, cancel: &Cancel) -> Result<Extraction> {
    let mut extraction = Extraction::default();
    if let Some(placed) = archbuilder_model::ramp::Placed::of(snapshot, object)
        && let Ok(layout) = &placed.layout
    {
        add(
            &mut extraction,
            &[layout.solid(placed.base)?],
            COLOR,
            placed.base,
            cancel,
        )?;
    }
    Ok(extraction)
}

/// The extraction of a railing: the bars of its rails, its posts and its balusters.
///
/// # Errors
/// The solid kernel's errors.
pub fn extract_railing(
    snapshot: &Snapshot,
    object: &Object,
    cancel: &Cancel,
) -> Result<Extraction> {
    let mut extraction = Extraction::default();
    if let Ok(derived) = archbuilder_model::railing::Derived::of(snapshot, object) {
        add(
            &mut extraction,
            &derived.solids()?,
            RAILING_COLOR,
            derived.base,
            cancel,
        )?;
    }
    Ok(extraction)
}

/// The objects to extract again after a commit.
#[must_use]
pub fn dependents(snapshot: &Snapshot, changes: &Changes) -> Vec<ObjectId> {
    let Changes::Listed(by_type) = changes else {
        return Vec::new();
    };
    let (mut levels, mut ramps, mut hosts, mut types) = (false, false, false, false);
    for (key, group) in by_type {
        let any = !(group.created.is_empty()
            && group.modified.is_empty()
            && group.recreated.is_empty()
            && group.deleted.is_empty());
        match key.as_str() {
            LEVEL | STOREY => levels |= any,
            RAMP => {
                ramps |= any;
                hosts |= any;
            }
            STAIR | RAILING => hosts |= any,
            RAILING_TYPE => types |= any,
            _ => {}
        }
    }
    let mut out = Vec::new();
    if levels {
        out.extend(spatial::objects(snapshot, RAMP).iter().map(|o| o.id));
    }
    if levels || hosts || types {
        out.extend(spatial::objects(snapshot, RAILING).iter().map(|o| o.id));
    }
    if ramps || levels {
        out.extend(spatial::objects(snapshot, FLOOR).iter().map(|o| o.id));
    }
    out
}
