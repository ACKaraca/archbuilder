//! What the viewport draws for a section or elevation: its drawing, built by `arc-view` in the
//! view's frame ([`archbuilder_model::plan::build_vertical`]) and drawn through the shared drafting
//! extraction of `arc-gpu`, then stood up in the vertical plane of the view's line: view x runs
//! along the line, view y is the elevation. The drawing thus lies where the cut is, and the camera
//! that shows it faces that plane ([`crate::interaction`]).
//!
//! Exact curves of the drafting extraction are flat in their item's plane, so they are flattened
//! into polylines here before they are turned. A 3D view draws nothing of its own: it is the model
//! seen through its camera. Only a view that a tab shows is drawn, as for plan views.

use arc_base::math::{Point3, Vector3};
use arc_base::{Cancel, Result};
use arc_doc::{Object, Snapshot};
use arc_gpu::curve::CurveShape;
use arc_gpu::draft::{DraftOptions, PAINT_BACKGROUND, Widths, extract_primitives};
use arc_gpu::{Extraction, LineItem, LinePath, RenderItem};
use archbuilder_model::model_view::ModelView;
use archbuilder_model::{plan, spatial};

/// Pixels per millimeter of lineweight, as in plan views.
const PX_PER_MM: f32 = 6.0;

/// The world frame of a view: the origin and the world directions of view x, y and z.
struct Frame {
    origin: Point3,
    axes: [Vector3; 3],
}

impl Frame {
    fn vector(&self, v: Vector3) -> Vector3 {
        self.axes[0] * v.x + self.axes[1] * v.y + self.axes[2] * v.z
    }

    fn point(&self, p: Point3) -> Point3 {
        self.origin + self.vector(p.coords)
    }

    fn offset(&self, p: [f32; 3]) -> [f32; 3] {
        let v = self.vector(Vector3::new(
            f64::from(p[0]),
            f64::from(p[1]),
            f64::from(p[2]),
        ));
        narrow(v)
    }
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "offsets from an item origin are uploaded in single precision"
)]
fn narrow(v: Vector3) -> [f32; 3] {
    [v.x as f32, v.y as f32, v.z as f32]
}

/// The world frame of `settings`, a section or elevation with a line.
fn frame_of(settings: &ModelView) -> Option<Frame> {
    let arc_view::ViewDirection::Frame { origin, right, up } = settings.direction().ok()? else {
        return None;
    };
    Some(Frame {
        origin,
        axes: [right, up, right.cross(&up)],
    })
}

/// Turns `item`, made in view coordinates, into the world.
fn stand_up(item: RenderItem, frame: &Frame) -> Option<RenderItem> {
    Some(match item {
        RenderItem::Mesh(mut m) => {
            m.origin = frame.point(m.origin);
            for instance in &mut m.instances {
                let t = instance.transform;
                let column = |c: usize| {
                    Vector3::new(f64::from(t[0][c]), f64::from(t[1][c]), f64::from(t[2][c]))
                };
                let mut rows = [[0.0_f32; 4]; 3];
                for c in 0..4 {
                    let turned = narrow(frame.vector(column(c)));
                    for (row, v) in rows.iter_mut().zip(turned) {
                        row[c] = v;
                    }
                }
                instance.transform = rows;
            }
            RenderItem::Mesh(m)
        }
        RenderItem::Lines(mut l) => {
            l.origin = frame.point(l.origin);
            for path in &mut l.paths {
                for p in &mut path.points {
                    *p = frame.offset(*p);
                }
            }
            RenderItem::Lines(l)
        }
        RenderItem::Curves(c) => {
            let paths: Vec<LinePath> = c
                .paths
                .iter()
                .filter_map(|path| {
                    let points: Vec<[f32; 3]> = match &path.shape {
                        CurveShape::Point(p) => vec![[p.x, p.y]],
                        CurveShape::Curve(curve) => plan::flatten(curve),
                    }
                    .into_iter()
                    .map(|[x, y]| narrow(frame.vector(Vector3::new(x, y, 0.0))))
                    .collect();
                    (!points.is_empty()).then_some(LinePath {
                        points,
                        closed: false,
                    })
                })
                .collect();
            if paths.is_empty() {
                return None;
            }
            RenderItem::Lines(LineItem {
                origin: frame.point(c.origin),
                paths,
                style: c.style,
                color: c.color,
            })
        }
    })
}

/// The extraction of a model view: the drawing of a section or elevation that a tab shows, stood
/// up in its plane; nothing for a 3D view or a view no tab shows.
///
/// # Errors
/// The errors of the drafting extraction for a list it cannot draw.
pub fn extract(snapshot: &Snapshot, object: &Object, cancel: &Cancel) -> Result<Extraction> {
    let Some(settings) = ModelView::from_values(&object.values).filter(ModelView::is_drawing)
    else {
        return Ok(Extraction::default());
    };
    if !crate::plan_state::open().contains(&object.id) {
        return Ok(Extraction::default());
    }
    let (Some(shared), Some(frame)) = (spatial::get(snapshot, object.id), frame_of(&settings))
    else {
        return Ok(Extraction::default());
    };
    let filter = crate::plan_state::filter(object.id);
    let Some(drawing) = plan::build_vertical(snapshot, &shared, &filter, cancel) else {
        return Ok(Extraction::default());
    };
    let options = DraftOptions {
        lineweight_px_per_mm: PX_PER_MM,
        ..DraftOptions::default()
    };
    let mut extraction = extract_primitives(
        snapshot,
        &drawing.list.primitives,
        Widths::Screen,
        PAINT_BACKGROUND,
        None,
        &options,
    )?
    .extraction;
    extraction.items = std::mem::take(&mut extraction.items)
        .into_iter()
        .filter_map(|item| stand_up(item, &frame))
        .collect();
    Ok(extraction)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_stands_view_coordinates_up_along_the_line() {
        let mut view = ModelView::new("s", archbuilder_model::model_view::ViewKind::Section);
        view.line = Some(([1.0, 2.0], [1.0, 7.0]));
        let frame = frame_of(&view).unwrap_or(Frame {
            origin: Point3::origin(),
            axes: [Vector3::zeros(); 3],
        });
        // Three meters along the line, two up: the line runs north from (1, 2).
        let p = frame.point(Point3::new(3.0, 2.0, 0.0));
        assert!((p - Point3::new(1.0, 5.0, 2.0)).norm() < 1e-12, "{p:?}");
    }
}
