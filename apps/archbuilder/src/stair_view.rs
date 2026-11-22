//! What a view from above draws for stairs. On the storey a stair stands on: the outline, a line
//! for each riser, the break line where the plan cut ([`CUT_HEIGHT`] above the base) crosses the
//! stair, the risers above the cut lighter and dashed, and the walking line with its arrow and the
//! up text at the foot. On the storey a stair arrives at: the risers above that cut drawn normal
//! and the rest lighter, the break line, the shaft opening's outline, and the walking line from the
//! arrival down to the foot with the down text at the arrival. The texts stand just outside the
//! stair, clear of its lines and of the other stairs (to a side, else not drawn). Other views draw no symbol; the solids come from the extractor. The
//! drawing is derived each frame from the records ([`archbuilder_model::stair::Layout::plan`]).

use arc_base::math::Point3;
use arc_doc::Snapshot;
use arc_ui::{OverlayColor, ProductOverlay};
use archbuilder_model::spatial;
use archbuilder_model::stair::{CUT_HEIGHT, Placed};
use archbuilder_model::types::STAIR;

use crate::room_view::{ViewRect, anchor_for_center, label_size};

/// A message of the application, named by identifier.
struct Msg {
    message_id: &'static str,
}

/// The up text, in the interface language.
const UP: Msg = Msg {
    message_id: "ab-stair-up",
};
/// The down text, in the interface language.
const DOWN: Msg = Msg {
    message_id: "ab-stair-down",
};

/// The normal lines: dark, as they lie on the stair's light concrete.
const NORMAL: OverlayColor = OverlayColor::Rgba([25, 25, 30, 255]);
/// The lighter lines (overhead on the lower storey, below the cut on the upper one).
const LIGHT: OverlayColor = OverlayColor::Rgba([25, 25, 30, 110]);
/// The walking line and its arrow.
const WALK: OverlayColor = OverlayColor::Rgba([20, 90, 200, 255]);
/// The gap between a stair edge and its text, in pixels.
const TEXT_GAP_PX: f64 = 6.0;

fn line(points: &[[f64; 2]], z: f64, color: OverlayColor, dashed: bool) -> ProductOverlay {
    ProductOverlay::Path {
        points: points.iter().map(|p| Point3::new(p[0], p[1], z)).collect(),
        dashed,
    }
    .colored(color)
}

/// The label of `text` that stands just outside the stair at `edge`, beyond it in the plan
/// direction `out`, clear of the riser lines: its anchor and its corners in plan.
fn text_place(
    view: &ViewRect,
    edge: [f64; 2],
    out: [f64; 2],
    text: &str,
) -> ([f64; 2], Vec<[f64; 2]>) {
    let size = label_size(text);
    // The half extent of the label along `out`, in pixels.
    let sx = out[0] * view.right[0] + out[1] * view.right[1];
    let sy = out[0] * view.up[0] + out[1] * view.up[1];
    let half = (sx.abs() * size[0] + sy.abs() * size[1]) / 2.0 + TEXT_GAP_PX;
    let p = view.to_pixel(edge);
    let c = [p[0] + sx * half, p[1] - sy * half];
    let (w, h) = (size[0] / 2.0, size[1] / 2.0);
    let corners = [[-w, -h], [w, -h], [w, h], [-w, h]]
        .iter()
        .map(|d| view.to_model([c[0] + d[0], c[1] + d[1]]))
        .collect();
    (anchor_for_center(view, c, size), corners)
}

/// A text to place: the stair, the elevation, the text, the edge it stands at and the way out.
type Text<'a> = (arc_base::ObjectId, f64, &'a String, [f64; 2], [f64; 2]);

/// The plan symbols of the stairs on and arriving at the active storey, in a view from above.
#[must_use]
pub fn overlays(snapshot: &Snapshot, language: &str) -> Vec<ProductOverlay> {
    let view = crate::room_view::shared();
    let (Some(storey), Some(frame)) = (view.active(), view.view()) else {
        return Vec::new();
    };
    let localizer = crate::localizer(&[language.to_owned()]).ok();
    let say = |m: &Msg, fallback: &str| {
        localizer
            .as_ref()
            .map_or_else(|| fallback.to_owned(), |l| l.format(m.message_id, &[]).text)
    };
    let (up, down) = (say(&UP, "UP"), say(&DOWN, "DN"));
    let level = spatial::get(snapshot, storey)
        .and_then(|s| spatial::level_of(snapshot, &s))
        .map(|l| l.id);
    let mut out = Vec::new();
    // The outlines drawn, which the texts keep clear of, and the texts to place.
    let mut outlines: Vec<(arc_base::ObjectId, Vec<[f64; 2]>)> = Vec::new();
    let mut texts: Vec<Text<'_>> = Vec::new();
    for object in spatial::objects(snapshot, STAIR).iter().take(500) {
        if view.is_hidden(object.id) {
            continue;
        }
        let Some(placed) = Placed::of(snapshot, object) else {
            continue;
        };
        let Ok(layout) = &placed.layout else {
            continue;
        };
        let here = object.header.owner == Some(storey);
        let arriving = level.is_some() && placed.top_level == level;
        if !here && !arriving {
            continue;
        }
        let (plan, z, text, at, toward) = if here {
            let foot = layout.runs[0];
            (
                layout.plan(CUT_HEIGHT),
                placed.base + 0.01,
                &up,
                foot.start,
                [-foot.dir[0], -foot.dir[1]],
            )
        } else {
            let (arrival, dir) = layout.arrival();
            (
                layout.plan_above(CUT_HEIGHT),
                placed.base + layout.height + 0.01,
                &down,
                arrival,
                dir,
            )
        };
        for ring in &plan.outline {
            let closed: Vec<[f64; 2]> = ring.iter().chain(ring.first()).copied().collect();
            out.push(line(&closed, z, NORMAL, false));
            outlines.push((object.id, ring.clone()));
        }
        for (pair, light) in &plan.treads {
            out.push(line(pair, z, if *light { LIGHT } else { NORMAL }, *light));
        }
        if let Some(cut) = &plan.cut {
            out.push(line(cut, z, NORMAL, false));
        }
        out.push(line(&plan.walk, z, WALK, false));
        out.push(line(&plan.arrow, z, WALK, false));
        texts.push((object.id, z, text, at, toward));
    }
    // A text goes beyond its edge, else to either side; one that would lie over another stair
    // in every place is not drawn.
    for (id, z, text, at, toward) in texts {
        let sides = [toward, [-toward[1], toward[0]], [toward[1], -toward[0]]];
        let clear = sides.iter().find_map(|dir| {
            let (anchor, corners) = text_place(&frame, at, *dir, text);
            let blocked = outlines.iter().any(|(other, ring)| {
                *other != id && archbuilder_model::stair::outlines_overlap(&corners, ring)
            });
            (!blocked).then_some(anchor)
        });
        // The label keeps the viewport's own text color on its plate.
        if let Some(anchor) = clear {
            out.push(ProductOverlay::Label {
                at: Point3::new(anchor[0], anchor[1], z),
                text: text.clone(),
            });
        }
    }
    out
}
