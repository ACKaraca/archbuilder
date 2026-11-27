//! The heads and labels of grids and the names of reference planes, drawn as overlays. In a plan
//! view a grid shows on the storeys its extent reaches, as a dash-dot line with a head (a ring
//! with the label) at each end that shows one, and a reference plane as a dashed line with its
//! name; in 3D the heads stand at the top of the grid. The drawing derives each frame from the
//! records ([`archbuilder_model::datum`]); hidden objects are left out.

use arc_base::math::Point3;
use arc_doc::Snapshot;
use arc_ui::{OverlayColor, ProductOverlay};
use archbuilder_model::datum::{self, Grid, RefPlane};
use archbuilder_model::spatial;
use archbuilder_model::types::{GRID, REFERENCE_PLANE};

/// The radius of a head, in meters.
pub const HEAD: f64 = 0.45;
const GRID_RGBA: OverlayColor = OverlayColor::Rgba([217, 89, 166, 255]);
const PLANE_RGBA: OverlayColor = OverlayColor::Rgba([77, 191, 115, 255]);
const SEGMENTS: usize = 24;
const MAX_DATUMS: usize = 2_000;

fn path(points: Vec<Point3>, dashed: bool, color: OverlayColor) -> ProductOverlay {
    ProductOverlay::Path { points, dashed }.colored(color)
}

/// A ring of radius [`HEAD`] around `center` at height `z`.
fn ring(center: [f64; 2], z: f64) -> Vec<Point3> {
    (0..=SEGMENTS)
        .map(|k| {
            #[expect(clippy::cast_precision_loss, reason = "a small count")]
            let a = std::f64::consts::TAU * k as f64 / SEGMENTS as f64;
            Point3::new(center[0] + HEAD * a.cos(), center[1] + HEAD * a.sin(), z)
        })
        .collect()
}

/// The overlays of the datums of `snapshot`. `plan` is the view's storey and whether it is a plan
/// view; `hidden` says what is hidden.
#[must_use]
pub fn overlays(
    snapshot: &Snapshot,
    plan: Option<f64>,
    hidden: &dyn Fn(arc_base::ObjectId) -> bool,
) -> Vec<ProductOverlay> {
    let mut out = Vec::new();
    for object in spatial::objects(snapshot, GRID).iter().take(MAX_DATUMS) {
        let Some(g) = Grid::from_values(&object.values) else {
            continue;
        };
        if hidden(object.id) || plan.is_some_and(|level| !datum::shows_at(snapshot, object, level))
        {
            continue;
        }
        let (_, top) = datum::grid_extent(snapshot, object);
        let z = plan.map_or(top, |level| level + 3.05);
        if plan.is_some() {
            out.push(path(
                g.polyline()
                    .iter()
                    .map(|p| Point3::new(p[0], p[1], z))
                    .collect(),
                true,
                GRID_RGBA,
            ));
        }
        let (d0, d1) = g.directions();
        for (on, end, dir) in [
            (g.head_start, g.start, [-d0[0], -d0[1]]),
            (g.head_end, g.end, d1),
        ] {
            if !on {
                continue;
            }
            let center = [end[0] + dir[0] * HEAD, end[1] + dir[1] * HEAD];
            out.push(path(ring(center, z), false, GRID_RGBA));
            out.push(
                ProductOverlay::Label {
                    at: Point3::new(center[0], center[1], z),
                    text: g.label.clone(),
                }
                .colored(GRID_RGBA),
            );
        }
    }
    if let Some(level) = plan {
        for object in spatial::objects(snapshot, REFERENCE_PLANE)
            .iter()
            .take(MAX_DATUMS)
        {
            let Some(r) = RefPlane::from_values(&object.values) else {
                continue;
            };
            if hidden(object.id) {
                continue;
            }
            let z = level + 3.05;
            let line = r.line();
            out.push(path(
                line.iter().map(|p| Point3::new(p[0], p[1], z)).collect(),
                true,
                PLANE_RGBA,
            ));
            out.push(
                ProductOverlay::Label {
                    at: Point3::new(r.start[0], r.start[1], z),
                    text: r.name,
                }
                .colored(PLANE_RGBA),
            );
        }
    }
    out
}
