//! What the viewport draws for a plan view: its display list, built by `arc-view`
//! ([`archbuilder_model::plan`]) and drawn through the shared drafting extraction of `arc-gpu`
//! (strokes as curves with screen lineweights, cut regions as filled meshes). The list is in
//! project coordinates; the extraction puts it at the elevation of the view's level.
//!
//! The overhead lines are dashed here (a drawing from `arc-view` carries only the class): each
//! stroke of the class is cut into dashes whose length is a sheet distance times the view's scale.
//! The faint storey below is a second list that is built with its own graphic override and drawn
//! first.
//!
//! Only a view that a tab shows is drawn ([`crate::plan_state::is_open_in`]): any other view
//! extracts to nothing, and the controller asks the shell to extract a view again when it opens
//! or when its temporary filter changes. A commit extracts again only the open views, and only
//! when it changed something other than plan views ([`dependents`]); the work runs on the shell's
//! extraction workers with their cancel token, and a view whose two passes and underlay are the
//! very results of the last extraction reuses that extraction.

use std::sync::{Arc, Mutex};

use arc_base::{Cancel, ObjectId, Result};
use arc_doc::events::Changes;
use arc_doc::{Object, Snapshot};
use arc_draft::display::{Primitive, Shape};
use arc_draft::linetype::{DashOptions, Dashes, Element, Pattern, dashes};
use arc_draft::types::ElementKind;
use arc_gpu::draft::{DraftOptions, PAINT_BACKGROUND, Widths, extract_primitives};
use arc_gpu::{Extraction, RenderItem};
use arc_view::{ViewResult, VisibilityClass};
use archbuilder_model::plan;
use archbuilder_model::spatial;
use archbuilder_model::types::{PLAN_VIEW, STOREY};

/// Pixels per millimeter of lineweight: a cut line of 0.5 mm is three pixels wide.
const PX_PER_MM: f32 = 6.0;
/// The length of a dash and of its gap on the sheet, in meters.
const DASH: [f64; 2] = [0.003, 0.0015];
/// The most dashes one stroke is cut into.
const MAX_DASHES: usize = 2_000;

type Memo = (ObjectId, [usize; 3], Vec<Arc<ViewResult>>, Extraction);
static MEMO: Mutex<Vec<Memo>> = Mutex::new(Vec::new());
const MEMO_VIEWS: usize = 32;

fn dashed(p: &Primitive, scale: f64) -> Primitive {
    let Shape::Stroke { curve, .. } = &p.shape else {
        return p.clone();
    };
    let element = |kind, length: f64| Element {
        kind,
        length: length * scale,
    };
    let Ok(pattern) = Pattern::new(vec![
        element(ElementKind::Dash, DASH[0]),
        element(ElementKind::Gap, DASH[1]),
    ]) else {
        return p.clone();
    };
    let options = DashOptions {
        max_dashes: MAX_DASHES,
        ..DashOptions::default()
    };
    match dashes(curve, &pattern, &options, &arc_base::Tolerance::DEFAULT) {
        Ok(Dashes::Pieces(pieces)) => {
            let drawn = pieces
                .iter()
                .filter_map(|d| arc_draft::clip::piece(curve, d.t0, d.t1).ok())
                .collect();
            let mut out = p.clone();
            out.shape = Shape::Stroke {
                curve: curve.clone(),
                drawn: Some(drawn),
            };
            out
        }
        _ => p.clone(),
    }
}

fn collect(into: &mut Vec<Primitive>, result: &ViewResult, scale: f64) {
    for (p, link) in result.list.primitives.iter().zip(&result.links) {
        into.push(if link.class == VisibilityClass::Overhead {
            dashed(p, scale)
        } else {
            p.clone()
        });
    }
}

/// The extraction of a plan view.
///
/// # Errors
/// The errors of the drafting extraction for a list it cannot draw.
pub fn extract_view(snapshot: &Snapshot, object: &Object, cancel: &Cancel) -> Result<Extraction> {
    let Some(shared) = spatial::get(snapshot, object.id) else {
        return Ok(Extraction::default());
    };
    let Some(settings) = archbuilder_model::view::settings_of(snapshot, object) else {
        return Ok(Extraction::default());
    };
    // A storey's extraction derives its rooms here, on the workers, so the interface finds them
    // ready (room tags, areas) instead of deriving them on its own thread.
    if spatial::type_of(snapshot, object.id).as_deref() == Some(STOREY) {
        warm_rooms(snapshot, object.id, cancel);
    }
    if !crate::plan_state::is_open_in(snapshot, object.id) {
        return Ok(Extraction::default());
    }
    let filter = crate::plan_state::filter(object.id);
    let Some(built) = plan::build(snapshot, &shared, &filter, cancel) else {
        return Ok(Extraction::default());
    };
    let under = settings
        .underlay
        .then(|| plan::underlay(snapshot, &shared, cancel))
        .flatten();
    let held: Vec<Arc<ViewResult>> = under
        .iter()
        .cloned()
        .chain([Arc::clone(&built.geometry), Arc::clone(&built.notes)])
        .collect();
    let key = [
        Arc::as_ptr(&built.geometry) as usize,
        Arc::as_ptr(&built.notes) as usize,
        under.as_ref().map_or(0, |u| Arc::as_ptr(u) as usize),
    ];
    if let Ok(memo) = MEMO.lock()
        && let Some((_, _, _, hit)) = memo
            .iter()
            .find(|(id, k, _, _)| *id == object.id && *k == key)
    {
        return Ok(hit.clone());
    }
    let mut primitives = Vec::new();
    for result in &held {
        collect(&mut primitives, result, built.scale);
    }
    let options = DraftOptions {
        lineweight_px_per_mm: PX_PER_MM,
        ..DraftOptions::default()
    };
    let mut extraction = extract_primitives(
        snapshot,
        &primitives,
        Widths::Screen,
        PAINT_BACKGROUND,
        None,
        &options,
    )?
    .extraction;
    for item in &mut extraction.items {
        match item {
            RenderItem::Mesh(m) => m.origin.z += built.level,
            RenderItem::Lines(l) => l.origin.z += built.level,
            RenderItem::Curves(c) => c.origin.z += built.level,
        }
    }
    if let Ok(mut memo) = MEMO.lock() {
        memo.retain(|(id, ..)| *id != object.id);
        if memo.len() >= MEMO_VIEWS {
            memo.remove(0);
        }
        memo.push((object.id, key, held, extraction.clone()));
    }
    Ok(extraction)
}

/// Derives the rooms of `storey` and their solids, which the caches keep.
fn warm_rooms(snapshot: &Snapshot, storey: ObjectId, cancel: &Cancel) {
    for entry in archbuilder_model::room::of_storey(snapshot, storey) {
        if cancel.is_cancelled() {
            return;
        }
        let _ = archbuilder_model::room::shape(snapshot, &entry);
    }
}

/// The objects to extract again after a commit that changed anything but views (a view record
/// that changed is extracted as a changed object): the open views of the document, and every
/// storey, whose extraction derives its rooms on the workers. Their caches make a view or a
/// storey that nothing it reads changed cheap.
#[must_use]
pub fn dependents(snapshot: &Snapshot, changes: &Changes) -> Vec<ObjectId> {
    let Changes::Listed(by_type) = changes else {
        return Vec::new();
    };
    let other = by_type.iter().any(|(k, g)| {
        k.as_str() != PLAN_VIEW
            && (!g.created.is_empty()
                || !g.modified.is_empty()
                || !g.deleted.is_empty()
                || !g.recreated.is_empty())
    });
    if !other {
        return Vec::new();
    }
    let mut out: Vec<ObjectId> = crate::plan_state::open()
        .into_iter()
        .filter_map(|id| crate::plan_state::resolve(snapshot, id))
        .collect();
    out.extend(spatial::objects(snapshot, STOREY).iter().map(|s| s.id));
    out
}
