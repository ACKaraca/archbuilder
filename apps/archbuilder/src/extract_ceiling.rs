//! What the viewport draws for a ceiling: the triangles of its body ([`archbuilder_model::ceiling`])
//! in a pale warm white, a prism of its boundary hanging up from its underside (a sloped
//! ceiling's body follows the slope); and for a shaft its outline at the base and at the top with
//! the vertical edges between, in orange lines, since the cut it makes shows in the floors and
//! roofs it passes through. Nothing is stored: a ceiling derives from the record, its type, the
//! walls its edges follow, its storey and level and the openings that cut it, so a change of any
//! of them extracts it again ([`dependents`]).

use std::collections::BTreeSet;

use arc_base::math::Point3;
use arc_base::{Cancel, Color, Limits, ObjectId, Result, Tolerance};
use arc_doc::events::Changes;
use arc_doc::{Object, Snapshot};
use arc_gpu::{
    Extraction, LineItem, LinePath, LineStyle, LineWidth, MeshInstance, MeshItem, RenderItem,
};
use arc_mesh::TriMesh;
use archbuilder_model::spatial;
use archbuilder_model::types::{CEILING, LEVEL, SHAFT, STOREY, VERTICAL_OPENING, WALL};

/// The color of a ceiling (linear RGB): a pale warm white.
pub const COLOR: Color = Color {
    r: 0.86,
    g: 0.82,
    b: 0.72,
    a: 1.0,
};

/// The extraction of a ceiling: its body as one mesh. A ceiling that cannot be built draws
/// nothing (the model's rules refuse it, so only in a damaged file).
///
/// # Errors
/// The solid kernel's errors for a body it cannot make a mesh of.
pub fn extract_ceiling(
    snapshot: &Snapshot,
    object: &Object,
    _cancel: &Cancel,
) -> Result<Extraction> {
    let Some(shared) = spatial::get(snapshot, object.id) else {
        return Ok(Extraction::default());
    };
    let Some(derived) = archbuilder_model::ceiling::derived(snapshot, &shared) else {
        return Ok(Extraction::default());
    };
    let solid = derived.solid()?;
    let first = derived.raw.loops.first().and_then(|l| l.first());
    let origin = Point3::new(
        first.map_or(0.0, |p| p[0]),
        first.map_or(0.0, |p| p[1]),
        derived.z_range().0,
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

/// The extraction of a shaft: the outline at its base and at its top and the vertical edges.
///
/// # Errors
/// None for this extraction; the extractor interface returns a result.
pub fn extract_shaft(snapshot: &Snapshot, object: &Object, _cancel: &Cancel) -> Result<Extraction> {
    let Some(span) = archbuilder_model::opening::shaft_span(snapshot, object) else {
        return Ok(Extraction::default());
    };
    let Some(first) = span.outline.first() else {
        return Ok(Extraction::default());
    };
    let origin = Point3::new(first[0], first[1], span.z.0);
    #[expect(clippy::cast_possible_truncation, reason = "display precision")]
    let at = |p: &[f64; 2], z: f64| {
        [
            (p[0] - first[0]) as f32,
            (p[1] - first[1]) as f32,
            (z - span.z.0) as f32,
        ]
    };
    let mut paths = Vec::new();
    for z in [span.z.0, span.z.1] {
        paths.push(LinePath {
            points: span.outline.iter().map(|p| at(p, z)).collect(),
            closed: true,
        });
    }
    // The vertical edges of the corners (an arc contributes its chord ends).
    for p in span
        .outline
        .iter()
        .step_by(span.outline.len().div_ceil(32).max(1))
    {
        paths.push(LinePath {
            points: vec![at(p, span.z.0), at(p, span.z.1)],
            closed: false,
        });
    }
    Ok(Extraction {
        items: vec![RenderItem::Lines(LineItem {
            origin,
            paths,
            style: LineStyle::round(LineWidth::Screen(1.5)),
            color: Color::new(0.95, 0.55, 0.15, 1.0),
        })],
        ..Extraction::default()
    })
}

/// The objects to extract again after a commit: the ceilings (and shafts) whose walls, storey,
/// level, type or openings changed. A shaft depends on its levels and storey.
#[must_use]
pub fn dependents(snapshot: &Snapshot, changes: &Changes) -> Vec<ObjectId> {
    let Changes::Listed(by_type) = changes else {
        return Vec::new();
    };
    let mut storeys: BTreeSet<ObjectId> = BTreeSet::new();
    let (mut all, mut shafts) = (false, false);
    for (key, group) in by_type {
        let live = group
            .created
            .iter()
            .chain(&group.modified)
            .chain(&group.recreated);
        let any = live.clone().next().is_some() || !group.deleted.is_empty();
        match key.as_str() {
            WALL => {
                storeys.extend(live.filter_map(|w| spatial::get(snapshot, *w)?.header.owner));
                all |= !group.deleted.is_empty();
            }
            STOREY => storeys.extend(live),
            LEVEL => {
                all |= any;
                shafts |= any;
            }
            SHAFT | VERTICAL_OPENING => all |= any,
            _ => {}
        }
    }
    let mut out: Vec<ObjectId> = spatial::objects(snapshot, CEILING)
        .into_iter()
        .filter(|c| all || c.header.owner.is_some_and(|s| storeys.contains(&s)))
        .map(|c| c.id)
        .collect();
    if shafts || !storeys.is_empty() {
        out.extend(spatial::objects(snapshot, SHAFT).into_iter().map(|s| s.id));
    }
    out
}
