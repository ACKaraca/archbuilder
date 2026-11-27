//! Model views in the viewport: what a saved 3D view, a section or an elevation shows each frame,
//! how it is framed when it opens, the visual style, the camera and the section box of 3D views,
//! and the turn of a plan view oriented to true north. The settings are the view record's; only
//! the default 3D view's visual style is a setting of the session.

use arc_app::shell::{InteractionContext, ViewRequest};
use arc_base::math::{Point3, Vector3};
use arc_base::{Error, ObjectId, Result};
use arc_command::Invocation;
use arc_doc::Snapshot;
use arc_geom::view::{Projection, ViewFrame};
use arc_gpu::ShadingMode;
use arc_gpu::display::{Section, SectionBox};
use arc_ui::Intent;
use archbuilder_commands::error::codes;
use archbuilder_commands::model_view::{VIEW_CREATE_3D, VIEW_SET_CAMERA, VIEW_SET_SECTION_BOX};
use archbuilder_model::model_view::{self as mv, ModelView, VisualStyle};
use archbuilder_model::spatial;
use archbuilder_model::types::{ELEMENTS, MODEL_VIEW, STOREY};
use archbuilder_model::view::Category;

use super::WallInteraction;
use crate::shortcuts::ui;
use crate::visibility::Lists;

/// The objects that are drawings of views, which the default 3D view hides: plan views, storeys
/// (for their default views) and model views.
pub(super) fn drawings(snapshot: &Snapshot) -> Vec<ObjectId> {
    archbuilder_model::view::all(snapshot)
        .iter()
        .chain(spatial::objects(snapshot, STOREY).iter())
        .chain(spatial::objects(snapshot, MODEL_VIEW).iter())
        .map(|v| v.id)
        .collect()
}

/// The shading mode of a visual style.
pub(super) fn shading(style: VisualStyle) -> ShadingMode {
    match style {
        VisualStyle::Wireframe => ShadingMode::Wireframe,
        VisualStyle::HiddenLine => ShadingMode::HiddenLine,
        VisualStyle::Shaded => ShadingMode::Shaded,
        VisualStyle::ShadedEdges => ShadingMode::ShadedWithEdges,
        VisualStyle::ConsistentColors => ShadingMode::ConsistentColors,
    }
}

/// The camera a saved 3D view keeps, as the viewport takes it.
fn frame_of(camera: &mv::Camera) -> Option<ViewFrame> {
    let (eye, target) = (Point3::from(camera.eye), Point3::from(camera.target));
    let forward = target - eye;
    // Looking straight down or up, the screen's up is the plan's +y.
    let up = if forward.x.hypot(forward.y) < 1e-6 * forward.norm() {
        Vector3::y()
    } else {
        Vector3::z()
    };
    let projection = if camera.perspective {
        Projection::Perspective {
            vertical_fov: camera.fov,
        }
    } else {
        Projection::Orthographic {
            height: camera.height().max(0.01),
        }
    };
    let far = (forward.norm() * 100.0).max(100_000.0);
    ViewFrame::new(eye, target, up, projection, 0.05, far).ok()
}

/// The camera of the viewport, as a 3D view stores it: an orthographic camera keeps the height it
/// shows through the field of view at the target.
fn camera_of(frame: &ViewFrame) -> mv::Camera {
    let (eye, target) = (frame.eye(), frame.target());
    let d = (target - eye).norm().max(1e-6);
    let (perspective, fov) = match frame.projection() {
        Projection::Perspective { vertical_fov } => (true, vertical_fov),
        Projection::Orthographic { height } => (false, 2.0 * (height / (2.0 * d)).atan()),
    };
    mv::Camera {
        eye: [eye.x, eye.y, eye.z],
        target: [target.x, target.y, target.z],
        perspective,
        fov: fov.clamp(mv::FOV_RANGE.0, mv::FOV_RANGE.1),
    }
}

/// The vertical range a section or elevation is framed on: its own crop range, else its
/// building's levels with a margin below and room for a storey above.
fn vertical_range(snapshot: &Snapshot, object: &arc_doc::Object, view: &ModelView) -> (f64, f64) {
    let levels: Vec<f64> = object
        .header
        .owner
        .map(|b| {
            spatial::storeys(snapshot, b)
                .iter()
                .map(spatial::StoreyLevel::elevation)
                .collect()
        })
        .unwrap_or_default();
    let lo = levels.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = levels.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let (lo, hi) = if lo.is_finite() && hi.is_finite() {
        (lo - 1.0, hi + 4.0)
    } else {
        (-1.0, 10.0)
    };
    (view.bottom.unwrap_or(lo), view.top.unwrap_or(hi))
}

/// How a model view opens: a 3D view with its camera (or framed on everything without one), a
/// section or elevation facing its drawing, which fills the viewport.
pub(super) fn framing(snapshot: &Snapshot, id: ObjectId) -> ViewRequest {
    let (Some(object), Some(view)) = (spatial::get(snapshot, id), mv::settings(snapshot, id))
    else {
        return ViewRequest::Extents;
    };
    if !view.is_drawing() {
        return view
            .camera
            .as_ref()
            .and_then(frame_of)
            .map_or(ViewRequest::Extents, ViewRequest::Camera);
    }
    let Some((a, b)) = view.line else {
        return ViewRequest::Extents;
    };
    let (lo, hi) = vertical_range(snapshot, &object, &view);
    let look = view.looking();
    ViewRequest::Facing {
        center: [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, (lo + hi) / 2.0],
        forward: [look[0], look[1], 0.0],
        up: [0.0, 0.0, 1.0],
        size: [view.width(), hi - lo],
    }
}

/// How far the plan view `view` is turned from the internal axes, counterclockwise in radians: by
/// the true north angle for a view oriented to true north.
pub(super) fn plan_rotation(snapshot: &Snapshot, view: ObjectId) -> f64 {
    spatial::get(snapshot, view)
        .and_then(|o| archbuilder_model::view::settings_of(snapshot, &o))
        .map_or(0.0, |v| {
            archbuilder_model::coordinates::Coordinates::of(snapshot).view_rotation(v.north)
        })
}

/// A plan framed on the rectangle `a`–`b` at the height `z`, turned by `turn` (counterclockwise,
/// radians): the screen's up is the plan's +y turned the other way, so the drawing turns by
/// `turn` on the screen.
pub(super) fn turned_plan(a: [f64; 2], b: [f64; 2], z: f64, turn: f64) -> ViewRequest {
    let (s, c) = turn.sin_cos();
    let up = [s, c];
    let right = [c, -s];
    let corners = [a, [b[0], a[1]], b, [a[0], b[1]]];
    let span = |axis: [f64; 2]| {
        let along: Vec<f64> = corners
            .iter()
            .map(|p| p[0] * axis[0] + p[1] * axis[1])
            .collect();
        along.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            - along.iter().copied().fold(f64::INFINITY, f64::min)
    };
    ViewRequest::Facing {
        center: [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, z],
        forward: [0.0, 0.0, -1.0],
        up: [up[0], up[1], 0.0],
        size: [span(right), span(up)],
    }
}

impl WallInteraction {
    /// The frame of a model view: a section or elevation shows only its drawing (filtered by the
    /// temporary display state); a 3D view shows the model without the view drawings, the
    /// elements and categories it hides, in its visual style and cropped by its section box.
    pub(super) fn model_frame(
        &mut self,
        cx: &InteractionContext<'_>,
        snapshot: &Snapshot,
        view: ObjectId,
    ) {
        self.habits.shown_plan = Some(view);
        let Some(settings) = mv::settings(snapshot, view) else {
            return;
        };
        if settings.is_drawing() {
            let temporary = &self.habits.temporary;
            let filter = archbuilder_model::plan::Filter {
                hidden: temporary.hidden.iter().copied().collect(),
                only: temporary
                    .isolated
                    .as_ref()
                    .map(|l| l.iter().copied().collect()),
            };
            if crate::plan_state::set_filter(view, filter) {
                cx.ask_extraction(&[view]);
            }
            self.habits.lists = Lists {
                hidden: Vec::new(),
                isolated: Some(vec![view]),
            };
            return;
        }
        let mut hidden = drawings(snapshot);
        hidden.extend(settings.hidden.iter().copied());
        for key in ELEMENTS {
            if Category::of_key(key).is_some_and(|c| !settings.shows(c)) {
                hidden.extend(spatial::objects(snapshot, key).iter().map(|o| o.id));
            }
        }
        self.habits.lists.hidden.extend(hidden);
        self.habits.shading = Some(shading(settings.style));
        self.habits.section = settings
            .section_box
            .then(|| settings.corners.or_else(|| mv::bounds(snapshot, None)))
            .flatten()
            .map(|(lo, hi)| Section {
                volume: Some(SectionBox {
                    min: Point3::from(lo),
                    max: Point3::from(hi),
                }),
                cap_color: Some(arc_base::Color::new(0.55, 0.53, 0.5, 1.0)),
                ..Section::default()
            });
    }

    /// The saved 3D view that is open.
    fn open_3d(&self, cx: &InteractionContext<'_>) -> Option<ObjectId> {
        let snapshot = cx.document().ok()?.snapshot();
        self.habits
            .model
            .filter(|id| mv::settings(&snapshot, *id).is_some_and(|v| !v.is_drawing()))
    }

    fn run(
        &mut self,
        cx: &InteractionContext<'_>,
        command: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let mut invocation = Invocation::new(command, params);
        invocation.seed = self.next_seed();
        cx.invoke(invocation).map(|o| o.value)
    }

    /// Sets the visual style (the `style` parameter, a style's record name): the open saved 3D
    /// view's, as one undo step, else the default 3D view's for the session.
    pub(super) fn set_style(&mut self, cx: &InteractionContext<'_>, intent: &Intent) -> Result<()> {
        let name = intent
            .params
            .get("style")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let style = VisualStyle::ALL
            .into_iter()
            .find(|s| s.name() == name)
            .ok_or_else(|| Error::new(codes::VALUE_INVALID))?;
        match self.open_3d(cx) {
            Some(view) => self
                .run(
                    cx,
                    archbuilder_commands::view::VIEW_SET,
                    serde_json::json!({"view": view.to_string(), "field": "style", "value": name}),
                )
                .map(drop),
            None => {
                self.habits.style_3d = style;
                Ok(())
            }
        }
    }

    /// Makes a saved 3D view of the viewport's camera and opens it.
    pub(super) fn new_view_3d(&mut self, cx: &InteractionContext<'_>) -> Result<()> {
        let camera = camera_of(cx.camera());
        let made = self.run(
            cx,
            VIEW_CREATE_3D,
            serde_json::json!({
                "eye": camera.eye,
                "target": camera.target,
                "perspective": camera.perspective,
                "fov": camera.fov,
            }),
        )?;
        let id = made
            .get("view")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?
            .to_owned();
        self.open_view(cx, &Intent::new(ui::OPEN_VIEW).with("view", id))
    }

    /// Stores the viewport's camera in the open saved 3D view; from any other view, makes a new
    /// saved 3D view of it.
    pub(super) fn save_camera(&mut self, cx: &InteractionContext<'_>) -> Result<()> {
        let Some(view) = self.open_3d(cx) else {
            return self.new_view_3d(cx);
        };
        let camera = camera_of(cx.camera());
        self.run(
            cx,
            VIEW_SET_CAMERA,
            serde_json::json!({
                "view": view.to_string(),
                "eye": camera.eye,
                "target": camera.target,
                "perspective": camera.perspective,
                "fov": camera.fov,
            }),
        )
        .map(drop)
    }

    /// Switches the section box of the open saved 3D view on (around the whole model) or off;
    /// from the default 3D view, makes a saved 3D view of its camera with the box on.
    pub(super) fn toggle_section_box(&mut self, cx: &InteractionContext<'_>) -> Result<()> {
        if self.open_3d(cx).is_none() {
            self.new_view_3d(cx)?;
        }
        let view = self
            .open_3d(cx)
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let snapshot = cx.document()?.snapshot();
        let on = !mv::settings(&snapshot, view).is_some_and(|v| v.section_box);
        self.run(
            cx,
            VIEW_SET_SECTION_BOX,
            serde_json::json!({"view": view.to_string(), "on": on}),
        )
        .map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_camera_round_trips_through_a_saved_view() {
        let camera = mv::Camera {
            eye: [10.0, -8.0, 6.0],
            target: [1.0, 2.0, 0.5],
            perspective: false,
            fov: 0.6,
        };
        let frame = frame_of(&camera).unwrap_or_else(|| panic!("no frame"));
        let again = camera_of(&frame);
        assert!((again.fov - camera.fov).abs() < 1e-9 && !again.perspective);
        assert_eq!(again.eye, camera.eye);
    }

    #[test]
    fn a_plan_turned_to_true_north_looks_down_with_true_north_up() {
        let ViewRequest::Facing { forward, up, .. } =
            turned_plan([0.0, 0.0], [10.0, 5.0], 3.0, -0.3)
        else {
            panic!("not facing");
        };
        assert_eq!(forward, [0.0, 0.0, -1.0]);
        // True north, 0.3 rad counterclockwise from +y, points up.
        let north = [-(0.3_f64).sin(), 0.3_f64.cos()];
        assert!((up[0] - north[0]).abs() < 1e-12 && (up[1] - north[1]).abs() < 1e-12);
    }
}
