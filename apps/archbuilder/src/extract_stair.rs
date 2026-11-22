//! What the viewport draws for a stair: the solids of its runs and its landing, shaded in a
//! concrete gray, and the bars of its railings in a darker tone, all derived from the record and
//! the levels ([`archbuilder_model::stair::Placed`]). A change of a level or a storey extracts
//! every stair again ([`dependents`]); a change of a stair extracts the floors again, as their
//! shaft openings follow it.

use arc_base::math::Point3;
use arc_base::{Cancel, Color, Limits, ObjectId, Result, Tolerance};
use arc_doc::events::Changes;
use arc_doc::{Object, Snapshot};
use arc_gpu::{Extraction, MeshInstance, MeshItem, RenderItem};
use arc_mesh::TriMesh;
use archbuilder_model::spatial;
use archbuilder_model::stair::Placed;
use archbuilder_model::types::{FLOOR, LEVEL, STAIR, STOREY};

/// The color of a stair's concrete (linear RGB): a warm light gray.
pub const COLOR: Color = Color {
    r: 0.72,
    g: 0.70,
    b: 0.66,
    a: 1.0,
};

/// The color of the railings: a dark steel gray.
pub const RAILING: Color = Color {
    r: 0.25,
    g: 0.27,
    b: 0.30,
    a: 1.0,
};

/// The extraction of a stair: one mesh per run, landing, rail bar and post.
///
/// # Errors
/// The solid kernel's errors for a record it cannot make a solid of; the shell shows the stair as
/// failed to extract. A stair without a layout (no level above) draws nothing.
pub fn extract_stair(snapshot: &Snapshot, object: &Object, cancel: &Cancel) -> Result<Extraction> {
    let Some(placed) = Placed::of(snapshot, object) else {
        return Ok(Extraction::default());
    };
    let Ok(layout) = &placed.layout else {
        return Ok(Extraction::default());
    };
    let mut extraction = Extraction::default();
    let solids = layout.solids(placed.base)?;
    let rails = layout.railing_solids(placed.base)?;
    for (solid, color) in solids
        .iter()
        .map(|s| (s, COLOR))
        .chain(rails.iter().map(|s| (s, RAILING)))
    {
        cancel.check()?;
        let mesh = TriMesh::from_polyhedron(solid, &Tolerance::DEFAULT, &Limits::DEFAULT)?;
        let origin = solid
            .vertices()
            .first()
            .copied()
            .unwrap_or_else(|| Point3::new(0.0, 0.0, placed.base));
        let geometry = extraction.add_geometry(mesh, origin);
        extraction.items.push(RenderItem::Mesh(MeshItem {
            geometry,
            origin,
            instances: vec![MeshInstance::new(color)],
        }));
    }
    Ok(extraction)
}

/// The objects to extract again after a commit: every stair when a level or a storey changed
/// (the height follows them), and every floor when a stair changed or went (its shaft opening).
#[must_use]
pub fn dependents(snapshot: &Snapshot, changes: &Changes) -> Vec<ObjectId> {
    let Changes::Listed(by_type) = changes else {
        return Vec::new();
    };
    let mut levels = false;
    let mut stairs = false;
    for (key, group) in by_type {
        let any = !(group.created.is_empty()
            && group.modified.is_empty()
            && group.recreated.is_empty()
            && group.deleted.is_empty());
        match key.as_str() {
            LEVEL | STOREY => levels |= any,
            STAIR => stairs |= any,
            _ => {}
        }
    }
    let mut out = Vec::new();
    if levels {
        out.extend(spatial::objects(snapshot, STAIR).iter().map(|o| o.id));
    }
    if stairs || levels {
        out.extend(spatial::objects(snapshot, FLOOR).iter().map(|o| o.id));
    }
    out
}
