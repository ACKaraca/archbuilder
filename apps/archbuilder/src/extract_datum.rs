//! What the viewport draws for the datums in 3D: a grid as the vertical outline of its plane
//! from its bottom to its top level (the line itself at both, the ends joining them), a reference
//! plane the same over its two points, and the displayed work plane as a rectangle on its plane.
//! The heads and labels are overlays ([`crate::datum_view`]). Nothing is stored: the lines derive
//! from the records and the levels, so a change of a level or a datum extracts them again
//! ([`dependents`]).

use arc_base::math::Point3;
use arc_base::{Cancel, Color, ObjectId, Result};
use arc_doc::events::Changes;
use arc_doc::{Object, Snapshot};
use arc_gpu::{Extraction, LineItem, LinePath, LineStyle, LineWidth, RenderItem};
use archbuilder_model::datum::{self, Grid, RefPlane};
use archbuilder_model::spatial;
use archbuilder_model::types::{GRID, LEVEL, REFERENCE_PLANE, STOREY, WORK_PLANE, work_plane};

/// The color of a grid line.
pub const GRID_COLOR: Color = Color {
    r: 0.85,
    g: 0.35,
    b: 0.65,
    a: 1.0,
};
/// The color of a reference plane.
pub const PLANE_COLOR: Color = Color {
    r: 0.3,
    g: 0.75,
    b: 0.45,
    a: 1.0,
};
/// The color of the displayed work plane.
pub const WORK_COLOR: Color = Color {
    r: 0.95,
    g: 0.75,
    b: 0.2,
    a: 1.0,
};

#[expect(clippy::cast_possible_truncation, reason = "display precision")]
fn point(p: [f64; 3]) -> [f32; 3] {
    [p[0] as f32, p[1] as f32, p[2] as f32]
}

fn item(paths: Vec<Vec<[f64; 3]>>, closed: bool, color: Color, width: f32) -> Extraction {
    Extraction {
        items: vec![RenderItem::Lines(LineItem {
            origin: Point3::new(0.0, 0.0, 0.0),
            paths: paths
                .into_iter()
                .map(|p| LinePath {
                    points: p.into_iter().map(point).collect(),
                    closed,
                })
                .collect(),
            style: LineStyle::round(LineWidth::Screen(width)),
            color,
        })],
        ..Extraction::default()
    }
}

/// The outline of a vertical plane over `line` between `low` and `high`: the line at both
/// heights and an edge at each end.
fn wall_of(line: &[[f64; 2]], low: f64, high: f64) -> Vec<Vec<[f64; 3]>> {
    let at = |z: f64| -> Vec<[f64; 3]> { line.iter().map(|p| [p[0], p[1], z]).collect() };
    let mut out = vec![at(low), at(high)];
    for p in [line.first(), line.last()].into_iter().flatten() {
        out.push(vec![[p[0], p[1], low], [p[0], p[1], high]]);
    }
    out
}

/// The extraction of a grid.
///
/// # Errors
/// None; the extractor interface returns a result.
pub fn extract_grid(snapshot: &Snapshot, object: &Object, _cancel: &Cancel) -> Result<Extraction> {
    let Some(g) = Grid::from_values(&object.values) else {
        return Ok(Extraction::default());
    };
    let (low, high) = datum::grid_extent(snapshot, object);
    Ok(item(
        wall_of(&g.polyline(), low, high),
        false,
        GRID_COLOR,
        2.0,
    ))
}

/// The extraction of a reference plane: bounded over its points, as a plane that runs on cannot
/// be drawn whole.
///
/// # Errors
/// None; the extractor interface returns a result.
pub fn extract_reference_plane(
    snapshot: &Snapshot,
    object: &Object,
    _cancel: &Cancel,
) -> Result<Extraction> {
    let Some(r) = RefPlane::from_values(&object.values) else {
        return Ok(Extraction::default());
    };
    let (low, high) = datum::extent(snapshot, object.header.owner, None, None);
    Ok(item(
        wall_of(&[r.start, r.end], low, high),
        false,
        PLANE_COLOR,
        1.5,
    ))
}

/// The extraction of a work plane: a rectangle on the plane while it is active and shown.
///
/// # Errors
/// None; the extractor interface returns a result.
pub fn extract_work_plane(
    snapshot: &Snapshot,
    object: &Object,
    _cancel: &Cancel,
) -> Result<Extraction> {
    let flag = |at: usize| {
        matches!(
            object.values.get(at),
            Some(Some(arc_doc::Value::Bool(true)))
        )
    };
    if !(flag(work_plane::ACTIVE) && flag(work_plane::SHOWN)) {
        return Ok(Extraction::default());
    }
    let Some(plane) = datum::resolve(snapshot, object) else {
        return Ok(Extraction::default());
    };
    let (n, u) = (plane.normal, plane.u);
    let v = [
        n[1] * u[2] - n[2] * u[1],
        n[2] * u[0] - n[0] * u[2],
        n[0] * u[1] - n[1] * u[0],
    ];
    // A horizontal plane spreads around its origin, a vertical one stands on it.
    let ((s0, s1), (t0, t1)) = if n[2].abs() > 0.99 {
        ((-10.0, 10.0), (-10.0, 10.0))
    } else {
        ((-2.0, 12.0), (0.0, 9.0))
    };
    let corner = |s: f64, t: f64| -> [f64; 3] {
        [
            plane.origin[0] + u[0] * s + v[0] * t,
            plane.origin[1] + u[1] * s + v[1] * t,
            plane.origin[2] + u[2] * s + v[2] * t,
        ]
    };
    let outline = vec![
        corner(s0, t0),
        corner(s1, t0),
        corner(s1, t1),
        corner(s0, t1),
    ];
    Ok(item(vec![outline], true, WORK_COLOR, 2.0))
}

/// The datums to extract again after a change: grids and reference planes when a level or storey
/// changed (their extent follows it), work planes when a level or datum changed (they follow it).
#[must_use]
pub fn dependents(snapshot: &Snapshot, changes: &Changes) -> Vec<ObjectId> {
    let Changes::Listed(by_type) = changes else {
        return Vec::new();
    };
    let (mut levels, mut datums) = (false, false);
    for (key, group) in by_type {
        let any = !(group.created.is_empty()
            && group.modified.is_empty()
            && group.recreated.is_empty()
            && group.deleted.is_empty());
        match key.as_str() {
            LEVEL | STOREY => levels |= any,
            GRID | REFERENCE_PLANE => datums |= any,
            _ => {}
        }
    }
    let mut out = Vec::new();
    if levels {
        for key in [GRID, REFERENCE_PLANE] {
            out.extend(spatial::objects(snapshot, key).iter().map(|o| o.id));
        }
    }
    if levels || datums {
        out.extend(spatial::objects(snapshot, WORK_PLANE).iter().map(|o| o.id));
    }
    out
}
