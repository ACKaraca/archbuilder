//! The grids of a building as one `IfcGrid`: every grid line is an `IfcGridAxis` tagged with its
//! label, a straight line as a polyline of two points and an arc as a trimmed circle. The grid is
//! contained in the lowest storey, its axes in the storey's plane.
//!
//! `IfcGrid` needs both a U and a V family of axes. The lines are split by direction: the first
//! straight line starts the U family, a straight line within 45 degrees of it (as lines, not as
//! rays) belongs to it, the others and every arc are V axes; a grid of one family only has no
//! valid `IfcGrid` and is reported, never padded.

use arc_base::id::Identifier;
use arc_base::math::Point2;
use arc_io_ifc::WriteModel;
use arc_io_ifc::write::{AxisCurve, Grid, GridAxis, derived_id, role};
use archbuilder_model::datum;
use archbuilder_model::types::GRID;

use super::Mapper;
use crate::error::reasons;

/// The role of the grid of a building in [`derived_id`].
const BUILDING_GRID: u32 = role::CALLER_FIRST + 4;

/// Two straight lines closer than this in direction, in radians, are parallel.
const PARALLEL: f64 = 1.0e-6;

/// The axis curve of a grid line; `None` for a line without length.
fn curve(grid: &datum::Grid) -> Option<(AxisCurve, bool)> {
    let (s, e) = (grid.start, grid.end);
    let chord = grid.length();
    if chord < 1.0e-6 {
        return None;
    }
    if grid.bulge.abs() < 1.0e-9 {
        let line = AxisCurve::Line {
            start: Point2::new(s[0], s[1]),
            end: Point2::new(e[0], e[1]),
        };
        return Some((line, true));
    }
    // The center lies on the left of the chord by `chord / 4 * (1 / b - b)` (a positive bulge is
    // counterclockwise); the sweep is `4 atan b`.
    let b = grid.bulge;
    let (dx, dy) = ((e[0] - s[0]) / chord, (e[1] - s[1]) / chord);
    let offset = chord / 4.0 * (1.0 / b - b);
    let center = [
        f64::midpoint(s[0], e[0]) - dy * offset,
        f64::midpoint(s[1], e[1]) + dx * offset,
    ];
    let radius = chord / 4.0 * (1.0 / b + b).abs();
    let sweep = 4.0 * b.abs().atan();
    // A clockwise arc is the counterclockwise one from its end to its start, against the axis.
    let from = if b > 0.0 { s } else { e };
    let start_angle = (from[1] - center[1]).atan2(from[0] - center[0]);
    let arc = AxisCurve::Arc {
        center: Point2::new(center[0], center[1]),
        radius,
        start_angle,
        end_angle: start_angle + sweep,
    };
    Some((arc, b > 0.0))
}

impl Mapper<'_> {
    /// The grids of `building` as one `IfcGrid` of `model`; the grids of the other buildings are
    /// not exported with their buildings.
    pub(super) fn grids(&mut self, model: &mut WriteModel, building: arc_base::ObjectId) {
        let Some(storey) = model
            .storeys
            .iter()
            .min_by(|a, b| a.elevation.total_cmp(&b.elevation))
            .map(|s| s.id)
        else {
            return;
        };
        let mut straight: Vec<(f64, GridAxis)> = Vec::new();
        let mut arcs: Vec<GridAxis> = Vec::new();
        let mut kept = Vec::new();
        for object in archbuilder_model::spatial::objects(self.snapshot, GRID) {
            if object.header.owner != Some(building) {
                self.lose(reasons::NOT_EXPORTED, "IFCGRID", &object);
                continue;
            }
            let parsed = datum::Grid::from_values(&object.values).and_then(|g| {
                let (curve, same_sense) = curve(&g)?;
                Some((g, curve, same_sense))
            });
            let Some((grid, curve, same_sense)) = parsed else {
                self.lose(reasons::NOT_EXPORTED, "IFCGRID", &object);
                continue;
            };
            let mut axis = GridAxis::new(&grid.label, curve);
            axis.same_sense = same_sense;
            if matches!(curve, AxisCurve::Line { .. }) {
                let angle = (grid.end[1] - grid.start[1]).atan2(grid.end[0] - grid.start[0]);
                straight.push((angle, axis));
            } else {
                arcs.push(axis);
            }
            kept.push(object);
        }
        let reference = straight.first().map_or(0.0, |(angle, _)| *angle);
        let (mut u, mut v): (Vec<GridAxis>, Vec<GridAxis>) = (Vec::new(), arcs);
        let mut parallel = true;
        let mut square = true;
        for (angle, axis) in straight {
            // The angle between two lines, in [0, pi/2].
            let d = (angle - reference).rem_euclid(std::f64::consts::PI);
            let d = d.min(std::f64::consts::PI - d);
            if d <= std::f64::consts::FRAC_PI_4 {
                parallel &= d < PARALLEL;
                u.push(axis);
            } else {
                square &= (d - std::f64::consts::FRAC_PI_2).abs() < PARALLEL;
                v.push(axis);
            }
        }
        if u.is_empty() && !v.is_empty() {
            u.push(v.remove(0));
        }
        if u.is_empty() || v.is_empty() {
            for object in &kept {
                self.lose(reasons::GRID_ONE_DIRECTION, "IFCGRID", object);
            }
            return;
        }
        let rectangular =
            parallel && square && !v.iter().any(|a| !matches!(a.curve, AxisCurve::Line { .. }));
        let mut grid = Grid::new(
            derived_id(building.as_u128(), BUILDING_GRID, 0),
            "Grid",
            storey,
            u,
            v,
        );
        grid.predefined_type = Some(
            if rectangular {
                "RECTANGULAR"
            } else {
                "IRREGULAR"
            }
            .to_owned(),
        );
        model.grids.push(grid);
    }
}
