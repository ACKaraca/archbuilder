//! What a view from above draws for shafts and vertical openings. A shaft shows its outline on
//! every storey whose level lies within its span, and a cross corner to corner on the storeys it
//! cuts (a solid outline on the storey it starts on, dashed elsewhere). A vertical opening shows
//! its outline dashed on the storey of its host. A wall opening has its own symbol across its
//! wall ([`crate::labels::symbols`]). The drawing derives each frame from the records
//! ([`archbuilder_model::opening`]).

use arc_base::math::Point3;
use arc_doc::Snapshot;
use arc_ui::{OverlayColor, ProductOverlay};
use archbuilder_model::floor::{FLAT_CHORD, VERTICAL_SLOTS};
use archbuilder_model::opening::{outline_of, passes_level, shafts};
use archbuilder_model::spatial;
use archbuilder_model::types::VERTICAL_OPENING;

const SHAFT: OverlayColor = OverlayColor::Rgba([200, 110, 30, 255]);
const CUT: OverlayColor = OverlayColor::Rgba([200, 110, 30, 200]);
const MAX_OPENINGS: usize = 1_000;

fn path(points: &[[f64; 2]], z: f64, color: OverlayColor, dashed: bool) -> ProductOverlay {
    ProductOverlay::Path {
        points: points.iter().map(|p| Point3::new(p[0], p[1], z)).collect(),
        dashed,
    }
    .colored(color)
}

fn closed(l: &[[f64; 2]]) -> Vec<[f64; 2]> {
    l.iter().chain(l.first()).copied().collect()
}

/// The plan symbols of the shafts and vertical openings on the active storey.
#[must_use]
pub fn overlays(snapshot: &Snapshot) -> Vec<ProductOverlay> {
    let view = crate::room_view::shared();
    let Some(storey) = view.active().and_then(|s| spatial::get(snapshot, s)) else {
        return Vec::new();
    };
    let level = spatial::storey_elevation(snapshot, &storey);
    let z = level + 3.05;
    let mut out = Vec::new();
    for shaft in shafts(snapshot).into_iter().take(MAX_OPENINGS) {
        if view.is_hidden(shaft.id) || !passes_level(shaft.z, level) {
            continue;
        }
        let own = shaft.storey == Some(storey.id);
        for l in shaft.boundary.flattened(FLAT_CHORD).iter().take(1) {
            out.push(path(&closed(l), z, SHAFT, !own));
        }
        // The cross runs corner to corner of the footprint's bounding box.
        let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
        for p in &shaft.outline {
            lo = [lo[0].min(p[0]), lo[1].min(p[1])];
            hi = [hi[0].max(p[0]), hi[1].max(p[1])];
        }
        if lo[0] < hi[0] && lo[1] < hi[1] {
            out.push(path(&[lo, hi], z, SHAFT, false));
            out.push(path(&[[lo[0], hi[1]], [hi[0], lo[1]]], z, SHAFT, false));
        }
    }
    for cut in spatial::objects(snapshot, VERTICAL_OPENING)
        .iter()
        .take(MAX_OPENINGS)
    {
        let on_storey = cut
            .header
            .owner
            .and_then(|h| spatial::get(snapshot, h))
            .is_some_and(|h| h.header.owner == Some(storey.id));
        if !on_storey || view.is_hidden(cut.id) {
            continue;
        }
        if let Some(outline) = outline_of(snapshot, cut, VERTICAL_SLOTS) {
            out.push(path(&closed(&outline), z, CUT, true));
        }
    }
    out
}
