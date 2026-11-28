//! The datums of a section or elevation, drawn as overlays in the plane of its drawing: each level
//! of its building as a line across the view with its name and elevation at the end, and each
//! grid that the view's line crosses as an upright line with its head (a ring with the label)
//! above the highest level. They derive each frame from the records, so they follow a level or a
//! grid as it changes. A level's line runs the width of the view (its extent in elevations is not
//! adjustable per view yet).

use arc_base::math::Point3;
use arc_doc::Snapshot;
use arc_ui::{OverlayColor, ProductOverlay};
use archbuilder_model::datum::Grid;
use archbuilder_model::model_view::ModelView;
use archbuilder_model::spatial;
use archbuilder_model::types::{self, GRID, level};

use crate::datum_view::HEAD;

const LEVEL_RGBA: OverlayColor = OverlayColor::Rgba([90, 150, 230, 255]);
const GRID_RGBA: OverlayColor = OverlayColor::Rgba([217, 89, 166, 255]);
/// How far a level line reaches past the view's sides, in meters.
const REACH: f64 = 1.0;
const SEGMENTS: u32 = 24;
const MAX_DATUMS: usize = 2_000;

/// Where along the line `a`–`b` (as a length from `a`) the segment `p`–`q` crosses it; `None`
/// when it does not cross within both.
fn crossing(a: [f64; 2], b: [f64; 2], p: [f64; 2], q: [f64; 2]) -> Option<f64> {
    let d = [b[0] - a[0], b[1] - a[1]];
    let e = [q[0] - p[0], q[1] - p[1]];
    let den = d[0] * e[1] - d[1] * e[0];
    if den.abs() < 1e-12 {
        return None;
    }
    let w = [p[0] - a[0], p[1] - a[1]];
    let t = (w[0] * e[1] - w[1] * e[0]) / den;
    let u = (w[0] * d[1] - w[1] * d[0]) / den;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then(|| t * d[0].hypot(d[1]))
}

/// The overlays of the section or elevation `view` (its settings `settings`), in `language`.
#[must_use]
pub fn overlays(
    snapshot: &Snapshot,
    building: Option<arc_base::ObjectId>,
    settings: &ModelView,
    language: &str,
) -> Vec<ProductOverlay> {
    let Some((a, b)) = settings.line else {
        return Vec::new();
    };
    let width = settings.width();
    if width <= 0.0 {
        return Vec::new();
    }
    let along = [(b[0] - a[0]) / width, (b[1] - a[1]) / width];
    let at = |x: f64, z: f64| Point3::new(a[0] + along[0] * x, a[1] + along[1] * x, z);
    let unit = spatial::length_unit(snapshot);
    let levels: Vec<_> = building
        .map(|b| spatial::levels(snapshot, b))
        .unwrap_or_default()
        .into_iter()
        .take(MAX_DATUMS)
        .collect();
    let elevation = |l: &arc_doc::Object| types::meters(&l.values, level::ELEVATION).unwrap_or(0.0);
    let mut out = Vec::new();
    for l in &levels {
        let z = elevation(l);
        out.push(
            ProductOverlay::Path {
                points: vec![at(-REACH, z), at(width + REACH, z)],
                dashed: true,
            }
            .colored(LEVEL_RGBA),
        );
        // At the start of the view, so the label stays in the framed drawing.
        out.push(ProductOverlay::Label {
            at: at(0.0, z),
            text: crate::labels::label(l, unit, language),
        });
    }
    let (low, high) = levels
        .iter()
        .map(|l| elevation(l))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), z| {
            (lo.min(z), hi.max(z))
        });
    let (low, high) = if low.is_finite() {
        (low - REACH, high + 2.0 * REACH)
    } else {
        (-REACH, 3.0)
    };
    for object in spatial::objects(snapshot, GRID).iter().take(MAX_DATUMS) {
        let Some(grid) = Grid::from_values(&object.values) else {
            continue;
        };
        let line = grid.polyline();
        let Some(x) = line
            .windows(2)
            .find_map(|w| crossing(a, b, *w.first()?, *w.get(1)?))
        else {
            continue;
        };
        out.push(
            ProductOverlay::Path {
                points: vec![at(x, low), at(x, high)],
                dashed: true,
            }
            .colored(GRID_RGBA),
        );
        let center = high + HEAD;
        let ring: Vec<Point3> = (0..=SEGMENTS)
            .map(|k| {
                let t = std::f64::consts::TAU * f64::from(k) / f64::from(SEGMENTS);
                at(x + HEAD * t.cos(), center + HEAD * t.sin())
            })
            .collect();
        out.push(
            ProductOverlay::Path {
                points: ring,
                dashed: false,
            }
            .colored(GRID_RGBA),
        );
        out.push(ProductOverlay::Label {
            at: at(x, center),
            text: grid.label.clone(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_grid_is_found_where_it_crosses_the_view_line() {
        // A line along x from 0 to 10 is crossed by a north-south grid at x = 4.
        let x = crossing([0.0, 0.0], [10.0, 0.0], [4.0, -5.0], [4.0, 5.0]);
        assert!(x.is_some_and(|x| (x - 4.0).abs() < 1e-12));
        assert!(crossing([0.0, 0.0], [10.0, 0.0], [12.0, -5.0], [12.0, 5.0]).is_none());
        assert!(crossing([0.0, 0.0], [10.0, 0.0], [0.0, 1.0], [10.0, 1.0]).is_none());
    }
}
