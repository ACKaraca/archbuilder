//! What a view from above draws for the ceilings of the active storey. A ceiling hangs above the
//! plan's cut, so it is only a symbol: its outline dashed, the lines of its grid pattern clipped
//! to its boundary (thin and faint, so the walls the plan cuts stay the strongest lines), and, for a
//! sloped ceiling, the slope arrow with its start line and the slope as text in the project's
//! slope unit. Nothing is filled and nothing here is pickable: the viewport picks what the plan
//! cuts first, and a ceiling only where nothing else is ([`archbuilder_model::pick::stack`]).
//! The drawing derives each frame from the records ([`archbuilder_model::ceiling::derived`]).

use arc_base::math::Point3;
use arc_doc::Snapshot;
use arc_ui::{OverlayColor, ProductOverlay};
use archbuilder_model::spatial;
use archbuilder_model::types::CEILING;
use archbuilder_model::units::{decimal_separator, slope_text, slope_unit};

use crate::room_view::{anchor_for_center, label_size};

/// The dashed outline: a quiet blue-gray, so it reads as above the cut.
const OUTLINE: OverlayColor = OverlayColor::Rgba([120, 150, 190, 230]);
/// The grid pattern: thinner in effect, fainter.
const GRID: OverlayColor = OverlayColor::Rgba([120, 150, 190, 110]);
/// The slope arrow.
const ARROW: OverlayColor = OverlayColor::Rgba([20, 90, 200, 255]);
/// The most ceilings drawn at once and the most grid segments of each.
const MAX_CEILINGS: usize = 500;
const MAX_SEGMENTS: usize = 4_000;

fn path(points: &[[f64; 2]], z: f64, color: OverlayColor, dashed: bool) -> ProductOverlay {
    ProductOverlay::Path {
        points: points.iter().map(|p| Point3::new(p[0], p[1], z)).collect(),
        dashed,
    }
    .colored(color)
}

/// The plan symbols of the ceilings on the active storey, in a view from above.
#[must_use]
pub fn overlays(snapshot: &Snapshot, language: &str) -> Vec<ProductOverlay> {
    let view = crate::room_view::shared();
    let (Some(storey), Some(frame)) = (view.active(), view.view()) else {
        return Vec::new();
    };
    let unit = spatial::project(snapshot).map_or_else(Default::default, |p| slope_unit(&p.values));
    let separator = decimal_separator(language);
    let mut out = Vec::new();
    for object in spatial::children(snapshot, CEILING, storey)
        .iter()
        .take(MAX_CEILINGS)
    {
        if view.is_hidden(object.id) {
            continue;
        }
        let Some(derived) = archbuilder_model::ceiling::derived(snapshot, object) else {
            continue;
        };
        let z = derived.z_range().1 + 0.01;
        for l in derived
            .boundary
            .flattened(archbuilder_model::floor::FLAT_CHORD)
        {
            let closed: Vec<[f64; 2]> = l.iter().chain(l.first()).copied().collect();
            out.push(path(&closed, z, OUTLINE, true));
        }
        for [a, b] in derived.grid_lines().into_iter().take(MAX_SEGMENTS) {
            out.push(path(&[a, b], z, GRID, false));
        }
        let Some(arrow) = derived.ceiling.arrow.filter(|_| derived.ceiling.sloped()) else {
            continue;
        };
        let (start, end) = (arrow.start, arrow.end());
        let (c, s) = (arrow.direction.cos(), arrow.direction.sin());
        // The start line, square to the arrow, a little beyond the arrow on both sides.
        let half = 0.5;
        out.push(path(
            &[
                [start[0] - s * half, start[1] + c * half],
                [start[0] + s * half, start[1] - c * half],
            ],
            z,
            ARROW,
            false,
        ));
        out.push(path(&[start, end], z, ARROW, false));
        let head = 8.0 / frame.ppu;
        let side = head * 0.5;
        let back = [end[0] - c * head, end[1] - s * head];
        out.push(path(
            &[
                [back[0] - s * side, back[1] + c * side],
                end,
                [back[0] + s * side, back[1] - c * side],
            ],
            z,
            ARROW,
            false,
        ));
        let text = slope_text(
            arrow.gradient(derived.ceiling.height()).atan(),
            unit,
            separator,
        );
        let size = label_size(&text);
        let anchor = anchor_for_center(
            &frame,
            frame.to_pixel([
                (start[0] + end[0]) / 2.0 - s * 0.25,
                (start[1] + end[1]) / 2.0 + c * 0.25,
            ]),
            size,
        );
        out.push(ProductOverlay::Label {
            at: Point3::new(anchor[0], anchor[1], z),
            text,
        });
    }
    out
}
