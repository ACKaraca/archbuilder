//! What a view from above draws for the roofs of the active storey, whose slabs the viewport then
//! leaves out so the walls the plan cuts stay in view and pickable: a faint tint, the outline with the
//! overhangs, the ridges, hips and valleys as lighter lines, and on every sloped edge an arrow
//! down the slope with the slope as text, in the project's slope unit. The arrow and its text
//! stand inside the edge's own plane, at the middle of the edge or, where a room tag or another
//! slope text would lie under them, farther along it; where no place is free, the edge has none. The drawing derives each frame from the records
//! ([`archbuilder_model::roof::Derived`]); the slab's solid comes from the extractor.

use arc_base::math::Point3;
use arc_doc::Snapshot;
use arc_ui::{OverlayColor, ProductOverlay};
use archbuilder_model::roof::{ArcKind, Derived};
use archbuilder_model::spatial;
use archbuilder_model::types::{ROOF, ROOM};
use archbuilder_model::units::{decimal_separator, slope_text};

use crate::room_view::{anchor_for_center, label_size};

/// The outline: a warm light line over the dark plan.
const OUTLINE: OverlayColor = OverlayColor::Rgba([235, 130, 95, 255]);
/// The faint tint of a roof's area.
const TINT: OverlayColor = OverlayColor::Rgba([170, 80, 60, 38]);
/// The ridges, hips and valleys: the thin, lighter lines.
const LINES: OverlayColor = OverlayColor::Rgba([235, 130, 95, 150]);
/// The slope arrows.
const ARROW: OverlayColor = OverlayColor::Rgba([20, 90, 200, 255]);
/// The length of a slope arrow, in pixels.
const ARROW_PX: f64 = 34.0;
/// How far inside its edge an arrow starts, in pixels.
const INSET_PX: f64 = 14.0;
/// The size assumed for a room tag that the slope texts keep clear of, in pixels.
const TAG_PX: [f64; 2] = [90.0, 34.0];

fn path(points: &[[f64; 2]], z: f64, color: OverlayColor, dashed: bool) -> ProductOverlay {
    ProductOverlay::Path {
        points: points.iter().map(|p| Point3::new(p[0], p[1], z)).collect(),
        dashed,
    }
    .colored(color)
}

fn overlap(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
}

/// The plan symbols of the roofs on the active storey, in a view from above.
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "outline, lines, then the arrows of each edge"
)]
pub fn overlays(snapshot: &Snapshot, language: &str) -> Vec<ProductOverlay> {
    let view = crate::room_view::shared();
    let (Some(storey), Some(frame)) = (view.active(), view.view()) else {
        return Vec::new();
    };
    let unit = spatial::unit_settings(snapshot);
    let separator = decimal_separator(language);
    // The room tags stand about their placement points.
    let tags: Vec<[f64; 4]> = spatial::children(snapshot, ROOM, storey)
        .iter()
        .filter_map(|r| archbuilder_model::room::Room::from_values(&r.values))
        .map(|r| {
            let c = frame.to_pixel(r.point);
            [
                c[0] - TAG_PX[0] / 2.0,
                c[1] - TAG_PX[1] / 2.0,
                c[0] + TAG_PX[0] / 2.0,
                c[1] + TAG_PX[1] / 2.0,
            ]
        })
        .collect();
    let mut out = Vec::new();
    // The slope texts placed so far, which the next ones keep clear of too.
    let mut placed: Vec<[f64; 4]> = Vec::new();
    for object in spatial::children(snapshot, ROOF, storey).iter().take(500) {
        if view.is_hidden(object.id) {
            continue;
        }
        let Ok(derived) = Derived::of(snapshot, object) else {
            continue;
        };
        let surface = &derived.surface;
        let z = surface.top() + 0.01;
        let outline = &surface.outline;
        let closed: Vec<[f64; 2]> = outline.iter().chain(outline.first()).copied().collect();
        // A faint tint: the roof lies above the plan cut and must not hide what it cuts.
        out.push(
            ProductOverlay::Fill {
                rings: vec![outline.iter().map(|p| Point3::new(p[0], p[1], z)).collect()],
            }
            .colored(TINT),
        );
        out.push(path(&closed, z, OUTLINE, false));
        for line in surface.lines() {
            out.push(path(
                &[line.from, line.to],
                z,
                LINES,
                line.kind == ArcKind::Valley,
            ));
        }
        let n = outline.len();
        let ccw = archbuilder_model::floor::signed_area(outline) > 0.0;
        for (k, edge) in derived.roof.edges.iter().enumerate() {
            let (Some(slope), Some(a), Some(b)) =
                (edge.slope, outline.get(k), outline.get((k + 1) % n))
            else {
                continue;
            };
            let d = [b[0] - a[0], b[1] - a[1]];
            let length = d[0].hypot(d[1]);
            if length * frame.ppu < ARROW_PX * 1.5 {
                continue;
            }
            let u = [d[0] / length, d[1] / length];
            let inward = if ccw { [-u[1], u[0]] } else { [u[1], -u[0]] };
            let text = slope_text(slope, unit, separator);
            let size = label_size(&text);
            let place = [0.5, 0.3, 0.7, 0.2, 0.8].iter().find_map(|t| {
                let on = [a[0] + d[0] * t, a[1] + d[1] * t];
                let top = [
                    on[0] + inward[0] * (INSET_PX + ARROW_PX) / frame.ppu,
                    on[1] + inward[1] * (INSET_PX + ARROW_PX) / frame.ppu,
                ];
                let foot = [
                    on[0] + inward[0] * INSET_PX / frame.ppu,
                    on[1] + inward[1] * INSET_PX / frame.ppu,
                ];
                // The text stands just beyond the arrow's upper end, farther up the slope.
                let p = frame.to_pixel(top);
                let (sx, sy) = (
                    inward[0] * frame.right[0] + inward[1] * frame.right[1],
                    inward[0] * frame.up[0] + inward[1] * frame.up[1],
                );
                let half = (sx.abs() * size[0] + sy.abs() * size[1]) / 2.0 + 3.0;
                let center = [p[0] + sx * half, p[1] - sy * half];
                let rect = [
                    center[0] - size[0] / 2.0,
                    center[1] - size[1] / 2.0,
                    center[0] + size[0] / 2.0,
                    center[1] + size[1] / 2.0,
                ];
                let free = !tags.iter().chain(&placed).any(|t| overlap(*t, rect));
                free.then_some((top, foot, center, rect))
            });
            let Some((top, foot, center, rect)) = place else {
                continue;
            };
            placed.push(rect);
            // The arrow points down the slope, toward the eave.
            let head = 7.0 / frame.ppu;
            let back = [foot[0] + inward[0] * head, foot[1] + inward[1] * head];
            let side = [u[0] * head * 0.6, u[1] * head * 0.6];
            out.push(path(&[top, foot], z, ARROW, false));
            out.push(path(
                &[
                    [back[0] + side[0], back[1] + side[1]],
                    foot,
                    [back[0] - side[0], back[1] - side[1]],
                ],
                z,
                ARROW,
                false,
            ));
            let anchor = anchor_for_center(&frame, center, size);
            out.push(ProductOverlay::Label {
                at: Point3::new(anchor[0], anchor[1], z),
                text,
            });
        }
    }
    out
}
