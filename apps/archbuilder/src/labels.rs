//! The level labels of the viewport: the name and the elevation of each level as text that faces
//! the screen, anchored at the end of the level's outline. The elevation is written in the
//! project's length unit with the interface language's decimal separator. The labels come from the
//! document at each frame, so they follow a rename, a new elevation and a change of unit or
//! language at once. The viewport keeps labels that would cover each other apart (it steps the
//! later one down), so the labels are given from the highest level to the lowest: seen from above,
//! where every level projects to the same place, they stack in the order of the storeys. The plan
//! symbols of doors and windows ([`symbols`]) are drawn by the interaction, in views from above.

use arc_app::application::{Application, TabId};
use arc_base::math::Point3;
use arc_doc::{Object, Snapshot};
use arc_ui::ProductOverlay;
use archbuilder_model::spatial;
use archbuilder_model::types::{self, level};
use archbuilder_model::units::UnitSettings;

use crate::extract_level::HALF;
use crate::room_view::{PixelRect, ViewRect, label_size};

const LINE_PX: f64 = 17.0;

pub use archbuilder_model::units::decimal_separator;

/// `meters` in `unit` with the decimal separator `separator`.
#[must_use]
pub fn elevation_text(meters: f64, unit: impl Into<UnitSettings>, separator: char) -> String {
    archbuilder_model::units::length_text(meters, unit, separator)
}

/// The text of a level's label: its name, then its elevation.
#[must_use]
pub fn label(level_object: &Object, unit: impl Into<UnitSettings>, language: &str) -> String {
    let name = types::text(&level_object.values, level::NAME).unwrap_or_default();
    let meters = types::meters(&level_object.values, level::ELEVATION).unwrap_or(0.0);
    format!(
        "{name}  {}",
        elevation_text(meters, unit, decimal_separator(language))
    )
}

/// One level label: where it is anchored (project coordinates), its text, and the pixels it
/// takes in a view from above.
struct Placed {
    at: Point3,
    text: String,
    rect: Option<PixelRect>,
}

/// The view cube's width and the gap beside it: labels in the top strip keep clear of it, in
/// pixels.
const CUBE_PX: f64 = 112.0;
const EDGE_PX: f64 = 4.0;

/// The labels of every level, highest first. In a view from above the shared anchor (the corner
/// of the level outlines) moves in so that the labels stay inside the image and out of the view
/// cube's corner; they stack downward from there, as the viewport steps overlapping labels down.
fn placed(snapshot: &Snapshot, language: &str, view: Option<&ViewRect>) -> Vec<Placed> {
    let unit = spatial::unit_settings(snapshot);
    let mut levels = spatial::objects(snapshot, types::LEVEL);
    let elevation = |o: &Object| types::meters(&o.values, level::ELEVATION).unwrap_or(0.0);
    levels.sort_by(|a, b| elevation(b).total_cmp(&elevation(a)));
    let texts: Vec<String> = levels.iter().map(|l| label(l, unit, language)).collect();
    let anchor = HALF;
    let mut shift = [0.0_f64, 0.0_f64];
    let mut origin = [0.0_f64, 0.0_f64];
    if let Some(v) = view {
        let ap = v.to_pixel(anchor);
        let widest = texts
            .iter()
            .map(|t| label_size(t)[0])
            .fold(0.0_f64, f64::max);
        let h = texts.first().map_or(LINE_PX, |t| label_size(t)[1]);
        // The first label's rectangle, as the viewport places it beside its anchor.
        let (left, top) = (ap[0] + 8.0, ap[1] - 8.0 - h);
        let right_limit = if top < CUBE_PX {
            v.size[0] - CUBE_PX
        } else {
            v.size[0] - EDGE_PX
        };
        shift[0] = (right_limit - (left + widest)).min(0.0);
        shift[0] += (EDGE_PX - (left + shift[0])).max(0.0);
        shift[1] = (EDGE_PX - top).max(0.0);
        origin = [left + shift[0], top + shift[1]];
    }
    levels
        .iter()
        .zip(texts)
        .enumerate()
        .map(|(k, (l, text))| {
            let at = view.map_or([anchor[0], anchor[1]], |v| {
                let ap = v.to_pixel(anchor);
                v.to_model([ap[0] + shift[0], ap[1] + shift[1]])
            });
            let size = label_size(&text);
            let rect = view.map(|_| {
                let top = origin[1] + k as f64 * (size[1] + 2.0);
                [origin[0], top, origin[0] + size[0], top + size[1]]
            });
            Placed {
                at: Point3::new(at[0], at[1], elevation(l)),
                text,
                rect,
            }
        })
        .collect()
}

/// The pixels the level labels take in a view from above; none for other views.
#[must_use]
pub fn level_rects(snapshot: &Snapshot, language: &str, view: Option<ViewRect>) -> Vec<PixelRect> {
    view.map_or_else(Vec::new, |v| {
        placed(snapshot, language, Some(&v))
            .into_iter()
            .filter_map(|p| p.rect)
            .collect()
    })
}

/// The labels of every level of `snapshot`, highest level first.
#[must_use]
pub fn of_snapshot(snapshot: &Snapshot, language: &str) -> Vec<ProductOverlay> {
    of_snapshot_in(snapshot, language, None)
}

/// The labels of every level, kept inside the visible area of a view from above.
#[must_use]
pub fn of_snapshot_in(
    snapshot: &Snapshot,
    language: &str,
    view: Option<&ViewRect>,
) -> Vec<ProductOverlay> {
    placed(snapshot, language, view)
        .into_iter()
        .map(|p| ProductOverlay::Label {
            at: p.at,
            text: p.text,
        })
        .collect()
}

/// The most plan symbols drawn.
const MAX_SYMBOLS: usize = 5_000;

/// The plan symbol of each door and window: the opening across its wall, a door's open leaf and
/// swing, a window's frame and glass line, at a cut 1 m above the wall's base, so that an insert
/// shows in a view from above, where the wall's top hides it; and the lines between the layers of
/// each wall at the same cut.
#[must_use]
pub fn symbols(
    snapshot: &Snapshot,
    shown: &dyn Fn(arc_base::ObjectId) -> bool,
) -> Vec<ProductOverlay> {
    let mut out = Vec::new();
    for key in [types::DOOR, types::WINDOW, types::WALL_OPENING] {
        for object in spatial::objects(snapshot, key).iter().take(MAX_SYMBOLS) {
            if !shown(object.id) {
                continue;
            }
            let Some(host) = object.header.owner.and_then(|w| spatial::get(snapshot, w)) else {
                continue;
            };
            let (Some(item), Some(wall)) = (
                archbuilder_model::insert::Insert::of(snapshot, object),
                archbuilder_model::wall::Wall::of(snapshot, &host),
            ) else {
                continue;
            };
            let level = host
                .header
                .owner
                .and_then(|s| spatial::get(snapshot, s))
                .map_or(0.0, |s| spatial::storey_elevation(snapshot, &s));
            for line in archbuilder_model::insert::symbol(&wall, level, &item).unwrap_or_default() {
                out.push(ProductOverlay::Path {
                    points: line.iter().map(|p| Point3::new(p[0], p[1], p[2])).collect(),
                    dashed: false,
                });
            }
        }
    }
    out.extend(layer_lines(snapshot, shown));
    out
}

/// The lines between the layers of each wall at the plan cut, 1 m above its base: thin dashed
/// lines, the boundaries of the core solid. Joins are cleaned by layer function, the finish of
/// a wrapped end or opening turns into it, and the openings the cut passes through are left out.
fn layer_lines(
    snapshot: &Snapshot,
    shown: &dyn Fn(arc_base::ObjectId) -> bool,
) -> Vec<ProductOverlay> {
    let mut out = Vec::new();
    let mut bases: std::collections::HashMap<arc_base::ObjectId, f64> =
        std::collections::HashMap::new();
    for storey in spatial::objects(snapshot, types::STOREY) {
        let level = spatial::storey_elevation(snapshot, &storey);
        for (id, line, core) in archbuilder_model::cleanup::plan_lines(snapshot, storey.id, CUT)
            .iter()
            .take(MAX_SYMBOLS * 8)
            .copied()
        {
            if !shown(id) {
                continue;
            }
            let base = *bases.entry(id).or_insert_with(|| {
                spatial::get(snapshot, id)
                    .and_then(|o| archbuilder_model::wall::Wall::from_values(&o.values))
                    .map_or(0.0, |w| w.base_offset)
            });
            let z = level + base + CUT;
            out.push(ProductOverlay::Path {
                points: line.iter().map(|p| Point3::new(p[0], p[1], z)).collect(),
                dashed: !core,
            });
        }
    }
    out
}

/// The height of the plan cut above a wall's base, in meters.
const CUT: f64 = 1.0;

/// The overlay probe of the shell: the labels of the active tab's levels.
#[must_use]
pub fn overlays(app: &Application, tab: TabId, language: &str) -> Vec<ProductOverlay> {
    app.tab(tab).map_or_else(
        |_| Vec::new(),
        |t| {
            of_snapshot_in(
                &t.document().snapshot(),
                language,
                crate::room_view::shared().view().as_ref(),
            )
        },
    )
}
