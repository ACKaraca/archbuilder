//! Model views other than plans: saved 3D views, sections and elevations. Each is a record of
//! authored settings (a `archbuilder.model_view`); what it shows is derived and never stored.
//!
//! - A **3D view** keeps a camera (eye, target, perspective or orthographic, field of view), a
//!   visual style, an optional section box that crops the model to an axis-aligned box, and the
//!   per-view visibility a plan view has (category flags and the elements hidden in the view). It
//!   is owned by the project.
//! - A **section** is a vertical cut along a line in plan, from `start` to `end`, looking to the
//!   left of that direction; what lies within the far clip distance (`depth`) behind the cut is
//!   drawn beyond it, what lies in front of the cut is not drawn. With `crop` on, the drawing is
//!   cropped to the length of the line and, when they are set, between the elevations `bottom`
//!   and `top`. A section is owned by a building.
//! - An **elevation** is a section whose line lies outside the building, so nothing is cut: four
//!   are made per building, one for each side of its extent, facing it.
//!
//! Sections and elevations are drawn by `arc-view` in an orthographic frame
//! ([`ModelView::direction`]); their markers are drawn in the plan views of their building.

use std::sync::Arc;

use arc_base::math::{Point3, Vector3};
use arc_base::{ObjectId, Result};
use arc_doc::{Object, Snapshot, Value};

use crate::spatial;
use crate::types::{BUILDING, MODEL_VIEW, PROJECT, model_view as f};
use crate::view::{Category, Detail};

/// The most model views of a document that are read.
pub const MAX_VIEWS: usize = 10_000;
/// The far clip distance of a new section, in meters.
pub const SECTION_DEPTH: f64 = 20.0;
/// How far outside the building's extent an elevation's line lies, in meters.
pub const ELEVATION_OFFSET: f64 = 5.0;
/// The vertical field of view of a new perspective camera, in radians (50 degrees).
pub const DEFAULT_FOV: f64 = 0.872_664_625_997_164_8;
/// The range of a camera's vertical field of view, in radians.
pub const FOV_RANGE: (f64, f64) = (0.01, 3.0);
/// The range of a section's far clip distance, in meters.
pub const DEPTH_RANGE: (f64, f64) = (0.01, 10_000.0);

coded! {
    /// What a model view shows.
    ViewKind {
        /// The model in 3D through a camera.
        #[default]
        ThreeD = 0, "three_d";
        /// A vertical cut along a line.
        Section = 1, "section";
        /// A building side seen from outside.
        Elevation = 2, "elevation";
    }
}

coded! {
    /// How a 3D view draws faces and edges.
    VisualStyle {
        /// Edges only, all of them.
        Wireframe = 0, "wireframe";
        /// Edges, with faces hiding what lies behind them.
        HiddenLine = 1, "hidden_line";
        /// Lit faces.
        #[default]
        Shaded = 2, "shaded";
        /// Lit faces with their edges.
        ShadedEdges = 3, "shaded_with_edges";
        /// Unlit faces in their own colors.
        ConsistentColors = 4, "consistent_colors";
    }
}

/// The camera of a 3D view, in project coordinates (meters).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Where the eye is.
    pub eye: [f64; 3],
    /// The point looked at.
    pub target: [f64; 3],
    /// Perspective, else orthographic.
    pub perspective: bool,
    /// The vertical field of view in radians; an orthographic camera shows the height this angle
    /// spans at the target.
    pub fov: f64,
}

impl Camera {
    /// The height the camera shows at its target: what an orthographic camera frames.
    #[must_use]
    pub fn height(&self) -> f64 {
        let d = (Point3::from(self.eye) - Point3::from(self.target)).norm();
        2.0 * d * (self.fov / 2.0).tan()
    }
}

/// The authored settings of a model view, lengths in meters, coordinates in the project's.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelView {
    /// The name.
    pub name: String,
    /// What the view shows.
    pub kind: ViewKind,
    /// The scale denominator.
    pub scale: f64,
    /// The detail level.
    pub detail: Detail,
    /// The visual style of a 3D view.
    pub style: VisualStyle,
    /// The elements hidden in this view.
    pub hidden: Vec<ObjectId>,
    /// Which categories are drawn, in the order of [`Category::ALL`].
    pub shown: [bool; 11],
    /// The camera of a 3D view; `None` frames the whole model when it opens.
    pub camera: Option<Camera>,
    /// Whether the section box crops a 3D view.
    pub section_box: bool,
    /// The corners of the section box (lowest, highest); `None` takes the model's extent.
    pub corners: Option<([f64; 3], [f64; 3])>,
    /// The line of a section or elevation in plan, from start to end.
    pub line: Option<([f64; 2], [f64; 2])>,
    /// The far clip distance behind the cut.
    pub depth: f64,
    /// Whether the drawing is cropped to the line and the vertical range.
    pub crop: bool,
    /// The lowest elevation drawn, when cropped.
    pub bottom: Option<f64>,
    /// The highest elevation drawn, when cropped.
    pub top: Option<f64>,
}

fn flag(values: &[Option<Value>], at: usize, default: bool) -> bool {
    match values.get(at) {
        Some(Some(Value::Bool(b))) => *b,
        _ => default,
    }
}

fn triple(values: &[Option<Value>], at: usize) -> Option<[f64; 3]> {
    Some([
        crate::types::meters(values, at)?,
        crate::types::meters(values, at + 1)?,
        crate::types::meters(values, at + 2)?,
    ])
}

fn pair(values: &[Option<Value>], at: usize) -> Option<[f64; 2]> {
    Some([
        crate::types::meters(values, at)?,
        crate::types::meters(values, at + 1)?,
    ])
}

impl ModelView {
    /// A new view of `kind` named `name`: every category shown, shaded, 1:100.
    #[must_use]
    pub fn new(name: &str, kind: ViewKind) -> Self {
        Self {
            name: name.to_owned(),
            kind,
            scale: crate::view::DEFAULT_SCALE,
            detail: Detail::Medium,
            style: VisualStyle::Shaded,
            hidden: Vec::new(),
            shown: [true; 11],
            camera: None,
            section_box: false,
            corners: None,
            line: None,
            depth: SECTION_DEPTH,
            crop: true,
            bottom: None,
            top: None,
        }
    }

    /// The settings of a view record; `None` when it is not readable.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Option<Self> {
        let scale = match values.get(f::SCALE) {
            Some(Some(Value::F64(s))) if s.is_finite() && *s > 0.0 => *s,
            _ => crate::view::DEFAULT_SCALE,
        };
        let hidden = match values.get(f::HIDDEN) {
            Some(Some(Value::List(items))) => items
                .iter()
                .take(crate::types::MAX_HIDDEN)
                .filter_map(|item| match item {
                    Value::Record(r) => match r.first() {
                        Some(Some(Value::Reference(id))) => Some(*id),
                        _ => None,
                    },
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        let mut shown = [true; 11];
        for (k, slot) in shown.iter_mut().enumerate() {
            *slot = flag(values, f::CATEGORIES + k, true);
        }
        let camera = match (triple(values, f::EYE), triple(values, f::TARGET)) {
            (Some(eye), Some(target)) => Some(Camera {
                eye,
                target,
                perspective: flag(values, f::PERSPECTIVE, true),
                fov: crate::types::meters(values, f::FOV)
                    .filter(|v| v.is_finite())
                    .map_or(DEFAULT_FOV, |v| v.clamp(FOV_RANGE.0, FOV_RANGE.1)),
            }),
            _ => None,
        };
        let corners = triple(values, f::BOX_MIN).zip(triple(values, f::BOX_MAX));
        let line = pair(values, f::START).zip(pair(values, f::END));
        Some(Self {
            name: crate::types::text(values, f::NAME)?.to_owned(),
            kind: ViewKind::of_field(values, f::KIND),
            scale,
            detail: Detail::of_field(values, f::DETAIL),
            style: VisualStyle::of_field(values, f::STYLE),
            hidden,
            shown,
            camera,
            section_box: flag(values, f::SECTION_BOX, false),
            corners,
            line,
            depth: crate::types::meters(values, f::DEPTH)
                .filter(|d| d.is_finite() && *d > 0.0)
                .unwrap_or(SECTION_DEPTH),
            crop: flag(values, f::CROP, true),
            bottom: crate::types::meters(values, f::BOTTOM),
            top: crate::types::meters(values, f::TOP),
        })
    }

    /// The record's values, in field order.
    #[must_use]
    pub fn values(&self) -> Vec<Option<Value>> {
        let length = crate::types::length;
        let mut out = vec![
            Some(Value::String(self.name.clone())),
            Some(Value::Enum(self.kind.code())),
            Some(Value::F64(self.scale)),
            Some(Value::Enum(self.detail.code())),
            Some(Value::Enum(self.style.code())),
            (!self.hidden.is_empty()).then(|| crate::view::hidden_value(&self.hidden)),
        ];
        out.extend(self.shown.iter().map(|b| Some(Value::Bool(*b))));
        let camera = self.camera;
        out.push(camera.map(|c| Value::Bool(c.perspective)));
        for at in 0..3 {
            out.push(camera.and_then(|c| c.eye.get(at).copied()).map(length));
        }
        for at in 0..3 {
            out.push(camera.and_then(|c| c.target.get(at).copied()).map(length));
        }
        out.push(camera.map(|c| crate::types::angle(c.fov)));
        out.push(Some(Value::Bool(self.section_box)));
        for corner in [self.corners.map(|c| c.0), self.corners.map(|c| c.1)] {
            for at in 0..3 {
                out.push(corner.and_then(|c| c.get(at).copied()).map(length));
            }
        }
        for end in [self.line.map(|l| l.0), self.line.map(|l| l.1)] {
            for at in 0..2 {
                out.push(end.and_then(|p| p.get(at).copied()).map(length));
            }
        }
        out.push(Some(length(self.depth)));
        out.push(Some(Value::Bool(self.crop)));
        out.push(self.bottom.map(length));
        out.push(self.top.map(length));
        out
    }

    /// Whether the category is drawn.
    #[must_use]
    pub fn shows(&self, category: Category) -> bool {
        Category::ALL
            .iter()
            .position(|c| *c == category)
            .and_then(|k| self.shown.get(k))
            .copied()
            .unwrap_or(true)
    }

    /// Whether the view is drawn as a 2D drawing (a section or an elevation).
    #[must_use]
    pub fn is_drawing(&self) -> bool {
        self.kind != ViewKind::ThreeD
    }

    /// The length of the line of a section or elevation; 0 without one.
    #[must_use]
    pub fn width(&self) -> f64 {
        self.line
            .map_or(0.0, |(a, b)| (b[0] - a[0]).hypot(b[1] - a[1]))
    }

    /// The orthographic frame of a section or elevation: view x along the line from its start,
    /// view y up (world z, from elevation 0), view z toward the viewer, so the cut plane is view
    /// z = 0 and the view looks to the left of the line.
    ///
    /// # Errors
    /// `view.definition.invalid` for a view without a line or with a line of no length.
    pub fn direction(&self) -> Result<arc_view::ViewDirection> {
        let (a, b) = self.line.unwrap_or_default();
        arc_view::ViewDirection::frame(
            Point3::new(a[0], a[1], 0.0),
            Vector3::new(b[0] - a[0], b[1] - a[1], 0.0),
            Vector3::z(),
        )
    }

    /// The direction a section or elevation looks in, in plan (unit; zero without a line).
    #[must_use]
    pub fn looking(&self) -> [f64; 2] {
        let w = self.width();
        match self.line {
            Some((a, b)) if w > 0.0 => [-(b[1] - a[1]) / w, (b[0] - a[0]) / w],
            _ => [0.0, 0.0],
        }
    }
}

/// The rule of a model view: a section or elevation has a line of some length and a positive
/// far clip, and its bottom lies below its top.
pub(crate) fn problem(object: &Object) -> Option<arc_base::Error> {
    let view = ModelView::from_values(&object.values)?;
    let bad = (view.is_drawing() && view.width() < crate::types::MIN_WALL_LENGTH)
        || view.bottom.zip(view.top).is_some_and(|(b, t)| b >= t)
        || view
            .corners
            .is_some_and(|(lo, hi)| lo.iter().zip(&hi).any(|(l, h)| l >= h));
    bad.then(|| arc_base::Error::new(crate::error::codes::VIEW_RANGE).with_arg("name", view.name))
}

/// Every model view of the document: 3D views, then sections, then elevations, each by name.
#[must_use]
pub fn all(snapshot: &Snapshot) -> Vec<Arc<Object>> {
    let mut found: Vec<Arc<Object>> = spatial::objects(snapshot, MODEL_VIEW);
    found.truncate(MAX_VIEWS);
    found.sort_by(|a, b| {
        ViewKind::of_field(&a.values, f::KIND)
            .code()
            .cmp(&ViewKind::of_field(&b.values, f::KIND).code())
            .then(crate::types::text(&a.values, 0).cmp(&crate::types::text(&b.values, 0)))
            .then(a.id.cmp(&b.id))
    });
    found
}

/// Whether `id` is a model view record.
#[must_use]
pub fn is_model_view(snapshot: &Snapshot, id: ObjectId) -> bool {
    spatial::type_of(snapshot, id).as_deref() == Some(MODEL_VIEW)
}

/// The settings of the model view `id`, when it is one.
#[must_use]
pub fn settings(snapshot: &Snapshot, id: ObjectId) -> Option<ModelView> {
    if !is_model_view(snapshot, id) {
        return None;
    }
    ModelView::from_values(&spatial::get(snapshot, id)?.values)
}

/// The sections and elevations shown in the plan views of `building`'s storeys.
#[must_use]
pub fn markers(snapshot: &Snapshot, building: ObjectId) -> Vec<Arc<Object>> {
    spatial::children(snapshot, MODEL_VIEW, building)
}

/// The owner a new model view of `kind` gets: the project for a 3D view, else `building`.
#[must_use]
pub fn owner_kind(kind: ViewKind) -> &'static str {
    match kind {
        ViewKind::ThreeD => PROJECT,
        _ => BUILDING,
    }
}

/// The extent of the model of `building` (every building when `None`): the walls in plan, their
/// bottoms and tops and the levels in height, as (lowest, highest) corners. `None` when there is
/// nothing to measure.
#[must_use]
pub fn bounds(snapshot: &Snapshot, building: Option<ObjectId>) -> Option<([f64; 3], [f64; 3])> {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    let mut add = |p: [f64; 3]| {
        for k in 0..3 {
            if let (Some(l), Some(h), Some(v)) = (lo.get_mut(k), hi.get_mut(k), p.get(k)) {
                *l = l.min(*v);
                *h = h.max(*v);
            }
        }
    };
    let buildings: Vec<ObjectId> = match building {
        Some(b) => vec![b],
        None => spatial::buildings(snapshot).iter().map(|b| b.id).collect(),
    };
    let mut any_plan = false;
    let mut levels = Vec::new();
    for b in buildings {
        for entry in spatial::storeys(snapshot, b) {
            let z = entry.elevation();
            for wall in spatial::walls(snapshot, entry.storey.id) {
                if let Some(w) = crate::wall::Wall::of(snapshot, &wall) {
                    let top = z + w.base_offset + w.height;
                    for p in w.plain_footprint() {
                        add([p[0], p[1], z + w.base_offset]);
                        add([p[0], p[1], top]);
                    }
                    any_plan = true;
                }
            }
            levels.push(z);
        }
    }
    for z in levels {
        lo[2] = lo[2].min(z);
        hi[2] = hi[2].max(z);
    }
    (any_plan && lo.iter().zip(&hi).all(|(l, h)| l.is_finite() && h > l)).then_some((lo, hi))
}

/// The four elevations of a building whose plan extent is `lo`–`hi`: (side, start, end), each
/// line [`ELEVATION_OFFSET`] outside the extent and looking at the building. The sides are north,
/// east, south and west of project north.
#[must_use]
pub fn elevation_lines(lo: [f64; 2], hi: [f64; 2]) -> [(Side, [f64; 2], [f64; 2]); 4] {
    let o = ELEVATION_OFFSET;
    let (x0, y0, x1, y1) = (lo[0] - o, lo[1] - o, hi[0] + o, hi[1] + o);
    // A view looks to the left of its line.
    [
        (Side::North, [x1, y1], [x0, y1]),
        (Side::East, [x1, y0], [x1, y1]),
        (Side::South, [x0, y0], [x1, y0]),
        (Side::West, [x0, y1], [x0, y0]),
    ]
}

/// A side of a building.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Toward project north (+y).
    North,
    /// Toward +x.
    East,
    /// Toward −y.
    South,
    /// Toward −x.
    West,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_view_round_trips_through_its_values() {
        let mut view = ModelView::new("Section 1", ViewKind::Section);
        view.line = Some(([0.0, 2.0], [5.0, 2.0]));
        view.depth = 7.5;
        view.bottom = Some(-1.0);
        view.top = Some(9.0);
        view.shown[2] = false;
        view.style = VisualStyle::HiddenLine;
        assert_eq!(ModelView::from_values(&view.values()).as_ref(), Some(&view));
        let mut three = ModelView::new("3D", ViewKind::ThreeD);
        three.camera = Some(Camera {
            eye: [10.0, -10.0, 8.0],
            target: [0.0, 0.0, 1.0],
            perspective: false,
            fov: 0.5,
        });
        three.section_box = true;
        three.corners = Some(([0.0; 3], [4.0, 5.0, 3.0]));
        assert_eq!(
            ModelView::from_values(&three.values()).as_ref(),
            Some(&three)
        );
    }

    #[test]
    fn a_section_looks_left_of_its_line_and_elevations_face_the_building() {
        let mut view = ModelView::new("s", ViewKind::Section);
        view.line = Some(([0.0, 2.0], [5.0, 2.0]));
        assert_eq!(view.looking(), [0.0, 1.0]);
        let frame = view.direction().unwrap_or_default();
        // A point behind the cut (north of it) is at negative view z, at its height in view y.
        let p = frame.to_view(Point3::new(1.0, 3.0, 2.5));
        assert!((p.x - 1.0).abs() < 1e-12 && (p.y - 2.5).abs() < 1e-12 && p.z < 0.0);
        for (_, a, b) in elevation_lines([0.0, 0.0], [10.0, 6.0]) {
            let mut e = ModelView::new("e", ViewKind::Elevation);
            e.line = Some((a, b));
            // Looking from the line toward the middle of the building.
            let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
            let to = [5.0 - mid[0], 3.0 - mid[1]];
            let look = e.looking();
            assert!(look[0] * to[0] + look[1] * to[1] > 0.0, "{a:?} {b:?}");
        }
    }
}
