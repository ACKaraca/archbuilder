//! The commands that make and change model views other than plans: `archbuilder.view.create_3d`,
//! `archbuilder.view.create_section`, `archbuilder.view.create_elevations` (the four elevations of
//! a building), `archbuilder.view.set_camera` (the camera of a 3D view) and
//! `archbuilder.view.set_section_box`. Renaming, duplicating, deleting, hiding elements in and
//! setting one field of a model view are the plan view commands of [`crate::view`].
//!
//! Each is one transaction, so one undo step. Coordinates are project coordinates in meters.

use arc_base::{Error, ObjectId, Result};
use arc_command::{CommandSpec, Context, IdParam, Registry};
use archbuilder_model::model_view::{
    self as mv, Camera, DEFAULT_FOV, DEPTH_RANGE, FOV_RANGE, ModelView, ViewKind,
};
use archbuilder_model::types::{BUILDING, COORDINATE_RANGE, MODEL_VIEW};
use archbuilder_model::{names, spatial, types};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::codes;
use crate::structure::{create, ensure_valid, expect, language};
use crate::view::ViewMade;

/// The identifier of the command that makes a 3D view.
pub const VIEW_CREATE_3D: &str = "archbuilder.view.create_3d";
/// The identifier of the command that makes a section.
pub const VIEW_CREATE_SECTION: &str = "archbuilder.view.create_section";
/// The identifier of the command that makes the four elevations of a building.
pub const VIEW_CREATE_ELEVATIONS: &str = "archbuilder.view.create_elevations";
/// The identifier of the command that sets the camera of a 3D view.
pub const VIEW_SET_CAMERA: &str = "archbuilder.view.set_camera";
/// The identifier of the command that sets or clears the section box of a 3D view.
pub const VIEW_SET_SECTION_BOX: &str = "archbuilder.view.set_section_box";

fn coordinate(v: f64) -> Result<f64> {
    if v.is_finite() && v >= COORDINATE_RANGE.0 && v <= COORDINATE_RANGE.1 {
        Ok(v)
    } else {
        Err(Error::new(codes::VALUE_INVALID))
    }
}

fn point<const N: usize>(p: [f64; N]) -> Result<[f64; N]> {
    for v in p {
        coordinate(v)?;
    }
    Ok(p)
}

/// The first free name `make(n)` for `n` from `from`, among `used`.
fn free_name(
    used: &[std::sync::Arc<arc_doc::Object>],
    from: usize,
    make: impl Fn(usize) -> String,
) -> String {
    let taken = |name: &str| {
        used.iter()
            .any(|v| types::text(&v.values, 0).is_some_and(|n| names::same(n, name)))
    };
    (from..)
        .take(100_000)
        .map(make)
        .find(|n| !taken(n))
        .unwrap_or_default()
}

/// The building `id` names, else the first one.
fn building_of(ctx: &Context<'_>, id: Option<IdParam>) -> Result<ObjectId> {
    match id {
        Some(IdParam(id)) => Ok(expect(ctx.snapshot(), id, BUILDING)?.id),
        None => spatial::buildings(ctx.snapshot())
            .first()
            .map(|b| b.id)
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH)),
    }
}

fn name_or(given: Option<String>, default: impl FnOnce() -> String) -> String {
    given
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(default)
}

/// Parameters of `archbuilder.view.create_3d`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Create3d {
    /// The name; else the first free `3D View <n>` in the interface language.
    #[serde(default)]
    #[schemars(length(min = 1, max = 256))]
    pub name: Option<String>,
    /// The eye of the camera; with `target`, the view keeps this camera.
    #[serde(default)]
    pub eye: Option<[f64; 3]>,
    /// The point the camera looks at.
    #[serde(default)]
    pub target: Option<[f64; 3]>,
    /// Perspective (the default), else orthographic.
    #[serde(default)]
    pub perspective: Option<bool>,
    /// The vertical field of view in radians.
    #[serde(default)]
    pub fov: Option<f64>,
}

fn camera_of(
    eye: [f64; 3],
    target: [f64; 3],
    perspective: Option<bool>,
    fov: Option<f64>,
) -> Result<Camera> {
    let (eye, target) = (point(eye)?, point(target)?);
    let fov = fov.unwrap_or(DEFAULT_FOV);
    let apart = eye.iter().zip(&target).any(|(a, b)| (a - b).abs() > 1e-6);
    if !(apart && fov.is_finite() && fov >= FOV_RANGE.0 && fov <= FOV_RANGE.1) {
        return Err(Error::new(codes::VALUE_INVALID));
    }
    Ok(Camera {
        eye,
        target,
        perspective: perspective.unwrap_or(true),
        fov,
    })
}

fn create_3d(ctx: &mut Context<'_>, p: Create3d) -> Result<ViewMade> {
    let project =
        spatial::project(ctx.snapshot()).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let camera = match (p.eye, p.target) {
        (Some(eye), Some(target)) => Some(camera_of(eye, target, p.perspective, p.fov)?),
        (None, None) => None,
        _ => return Err(Error::new(arc_command::error::codes::PARAMS_INVALID)),
    };
    let tongue = language(ctx);
    let used = spatial::children(ctx.snapshot(), MODEL_VIEW, project.id);
    let name = name_or(p.name, || {
        free_name(&used, 1, |n| {
            archbuilder_model::defaults::view_3d(&tongue, n)
        })
    });
    let mut view = ModelView::new(&name, ViewKind::ThreeD);
    view.camera = camera;
    made(ctx, project.id, &view)
}

fn made(ctx: &mut Context<'_>, owner: ObjectId, view: &ModelView) -> Result<ViewMade> {
    let id = create(ctx, MODEL_VIEW, Some(owner), view.values())?;
    ensure_valid(ctx)?;
    ctx.set_selection_after(vec![id]);
    Ok(ViewMade { view: IdParam(id) })
}

/// Parameters of `archbuilder.view.create_section`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateSection {
    /// The start of the section line in plan; the section looks to the left of start to end.
    pub start: [f64; 2],
    /// The end of the section line.
    pub end: [f64; 2],
    /// The far clip distance behind the cut, in meters; 20 m by default.
    #[serde(default)]
    pub depth: Option<f64>,
    /// The name; else the first free `Section <n>` in the interface language.
    #[serde(default)]
    #[schemars(length(min = 1, max = 256))]
    pub name: Option<String>,
    /// The building the section belongs to; else the first.
    #[serde(default)]
    pub building: Option<IdParam>,
}

fn create_section(ctx: &mut Context<'_>, p: CreateSection) -> Result<ViewMade> {
    let building = building_of(ctx, p.building)?;
    let depth = p.depth.unwrap_or(mv::SECTION_DEPTH);
    if !(depth.is_finite() && depth >= DEPTH_RANGE.0 && depth <= DEPTH_RANGE.1) {
        return Err(Error::new(codes::VALUE_INVALID));
    }
    let tongue = language(ctx);
    let used = spatial::children(ctx.snapshot(), MODEL_VIEW, building);
    let name = name_or(p.name, || {
        free_name(&used, 1, |n| {
            archbuilder_model::defaults::section(&tongue, n)
        })
    });
    let mut view = ModelView::new(&name, ViewKind::Section);
    view.line = Some((point(p.start)?, point(p.end)?));
    view.depth = depth;
    made(ctx, building, &view)
}

/// Parameters of `archbuilder.view.create_elevations`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateElevations {
    /// The building; else the first.
    #[serde(default)]
    pub building: Option<IdParam>,
}

/// What `archbuilder.view.create_elevations` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ElevationsMade {
    /// The new views: north, east, south and west.
    pub views: Vec<IdParam>,
}

fn create_elevations(ctx: &mut Context<'_>, p: CreateElevations) -> Result<ElevationsMade> {
    let building = building_of(ctx, p.building)?;
    // An empty building gets elevations around a 10 m square at the origin.
    let (lo, hi) =
        mv::bounds(ctx.snapshot(), Some(building)).unwrap_or(([0.0, 0.0, 0.0], [10.0, 10.0, 3.0]));
    let tongue = language(ctx);
    let used = spatial::children(ctx.snapshot(), MODEL_VIEW, building);
    let mut views = Vec::new();
    for (side, start, end) in mv::elevation_lines([lo[0], lo[1]], [hi[0], hi[1]]) {
        let base = archbuilder_model::defaults::elevation(&tongue, side);
        let name = free_name(&used, 1, |n| {
            if n == 1 {
                base.clone()
            } else {
                format!("{base} {n}")
            }
        });
        let mut view = ModelView::new(&name, ViewKind::Elevation);
        view.line = Some((point(start)?, point(end)?));
        // The far clip reaches past the far side of the building.
        view.depth = ((hi[0] - lo[0]).max(hi[1] - lo[1]) + 2.0 * mv::ELEVATION_OFFSET)
            .clamp(DEPTH_RANGE.0, DEPTH_RANGE.1);
        let id = create(ctx, MODEL_VIEW, Some(building), view.values())?;
        views.push(IdParam(id));
    }
    ensure_valid(ctx)?;
    Ok(ElevationsMade { views })
}

/// Parameters of `archbuilder.view.set_camera`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetCamera {
    /// The 3D view.
    pub view: IdParam,
    /// The eye.
    pub eye: [f64; 3],
    /// The point looked at.
    pub target: [f64; 3],
    /// Perspective, else orthographic; perspective by default.
    #[serde(default)]
    pub perspective: Option<bool>,
    /// The vertical field of view in radians.
    #[serde(default)]
    pub fov: Option<f64>,
}

fn three_d(ctx: &Context<'_>, id: ObjectId) -> Result<ModelView> {
    let object = expect(ctx.snapshot(), id, MODEL_VIEW)?;
    ModelView::from_values(&object.values)
        .filter(|v| v.kind == ViewKind::ThreeD)
        .ok_or_else(|| Error::new(codes::KIND_MISMATCH))
}

/// Writes the fields of `view` that differ from the record `id`'s.
fn replace(ctx: &mut Context<'_>, id: ObjectId, view: &ModelView) -> Result<()> {
    let before = expect(ctx.snapshot(), id, MODEL_VIEW)?.values.clone();
    let names: Vec<String> = ctx
        .snapshot()
        .registry()
        .get(MODEL_VIEW)
        .map(|d| d.schema.fields.iter().map(|f| f.name.clone()).collect())
        .unwrap_or_default();
    let txn = ctx.txn()?;
    for ((value, name), old) in view
        .values()
        .into_iter()
        .zip(&names)
        .zip(before.iter().map(Some).chain(std::iter::repeat(None)))
    {
        if old.cloned().flatten() != value {
            txn.modify(id, arc_doc::FieldPath::new(name.as_str()), value)?;
        }
    }
    ensure_valid(ctx)
}

fn set_camera(ctx: &mut Context<'_>, p: SetCamera) -> Result<()> {
    let mut view = three_d(ctx, p.view.0)?;
    view.camera = Some(camera_of(p.eye, p.target, p.perspective, p.fov)?);
    replace(ctx, p.view.0, &view)
}

/// Parameters of `archbuilder.view.set_section_box`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetSectionBox {
    /// The 3D view.
    pub view: IdParam,
    /// Whether the box crops the view.
    pub on: bool,
    /// The lowest corner; with `max`. Without both, a box switched on takes the model's extent.
    #[serde(default)]
    pub min: Option<[f64; 3]>,
    /// The highest corner.
    #[serde(default)]
    pub max: Option<[f64; 3]>,
}

fn set_section_box(ctx: &mut Context<'_>, p: SetSectionBox) -> Result<()> {
    let mut view = three_d(ctx, p.view.0)?;
    view.section_box = p.on;
    match (p.min, p.max) {
        (Some(lo), Some(hi)) => {
            let (lo, hi) = (point(lo)?, point(hi)?);
            if lo.iter().zip(&hi).any(|(l, h)| l >= h) {
                return Err(Error::new(codes::VALUE_INVALID));
            }
            view.corners = Some((lo, hi));
        }
        (None, None) => {
            if p.on && view.corners.is_none() {
                view.corners = mv::bounds(ctx.snapshot(), None);
            }
        }
        _ => return Err(Error::new(arc_command::error::codes::PARAMS_INVALID)),
    }
    replace(ctx, p.view.0, &view)
}

/// A message of the catalog, named by identifier.
struct Message {
    message_id: &'static str,
}

/// Registers the commands of this module.
///
/// # Errors
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    for s in [
        CommandSpec::new(
            VIEW_CREATE_3D,
            Message {
                message_id: "ab-cmd-view-create-3d-label",
            }
            .message_id,
            Message {
                message_id: "ab-cmd-view-create-3d-description",
            }
            .message_id,
            create_3d,
        ),
        CommandSpec::new(
            VIEW_CREATE_SECTION,
            Message {
                message_id: "ab-cmd-view-create-section-label",
            }
            .message_id,
            Message {
                message_id: "ab-cmd-view-create-section-description",
            }
            .message_id,
            create_section,
        ),
        CommandSpec::new(
            VIEW_CREATE_ELEVATIONS,
            Message {
                message_id: "ab-cmd-view-create-elevations-label",
            }
            .message_id,
            Message {
                message_id: "ab-cmd-view-create-elevations-description",
            }
            .message_id,
            create_elevations,
        ),
        CommandSpec::new(
            VIEW_SET_CAMERA,
            Message {
                message_id: "ab-cmd-view-set-camera-label",
            }
            .message_id,
            Message {
                message_id: "ab-cmd-view-set-camera-description",
            }
            .message_id,
            set_camera,
        ),
        CommandSpec::new(
            VIEW_SET_SECTION_BOX,
            Message {
                message_id: "ab-cmd-view-set-section-box-label",
            }
            .message_id,
            Message {
                message_id: "ab-cmd-view-set-section-box-description",
            }
            .message_id,
            set_section_box,
        ),
    ] {
        registry.register(s)?;
    }
    Ok(())
}
