//! `archbuilder.roof_extrusion.create` and `archbuilder.roof_extrusion.edit`: a roof made by
//! extruding an open profile sketched in a vertical plane.
//!
//! The plane is given by `plane`, two points `[x, y]` of the storey: it passes through both, its
//! horizontal axis runs from the first to the second. The `profile` is points `[u, z]` (`u` along
//! that axis from the first point, `z` above the storey's level) and is the roof's top surface;
//! the roof is thickened downward, normal to the profile, by the thickness of its type (layers
//! offset normal to the profile). `start` and `end` are the distances from the plane, along its
//! normal (the axis turned a quarter counterclockwise), that the profile is extruded between.
//! One call is one transaction, one undo step. A roof by extrusion is deleted with
//! `archbuilder.object.delete`.

use arc_base::{Dimension, Error, ObjectId, Result};
use arc_command::{CommandSpec, Context, IdParam, QuantityParam, Registry};
use arc_doc::{FieldPath, Snapshot};
use archbuilder_model::element_type::TypeKind;
use archbuilder_model::roof_extrusion::{Derived, MAX_PROFILE};
use archbuilder_model::types::{self, ROOF_EXTRUSION};
use archbuilder_model::{defaults, names, spatial};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::codes;
use crate::structure::{create, ensure_valid, expect, language, named};

/// The identifier of the create command.
pub const ROOF_EXTRUSION_CREATE: &str = "archbuilder.roof_extrusion.create";
/// The identifier of the command that changes a roof by extrusion.
pub const ROOF_EXTRUSION_EDIT: &str = "archbuilder.roof_extrusion.edit";

/// Parameters of `archbuilder.roof_extrusion.create`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateRoofExtrusion {
    /// The storey; else the storey of the selection, else the lowest storey.
    #[serde(default)]
    pub storey: Option<IdParam>,
    /// Two points `[x, y]` of the plane, in meters: its origin and a point along its axis.
    pub plane: [[f64; 2]; 2],
    /// The open profile: points `[u, z]` in meters, in order.
    pub profile: Vec<[f64; 2]>,
    /// The start of the extrusion: its distance from the plane along the plane's normal.
    #[serde(default)]
    pub start: Option<QuantityParam>,
    /// The end of the extrusion, in the same distance.
    pub end: QuantityParam,
    /// The roof type: its identifier or name; else the document's default roof type.
    #[serde(default, rename = "type")]
    #[schemars(length(max = 256))]
    pub roof_type: Option<String>,
    /// The name; else the first free `Extruded roof <n>` of the storey.
    #[serde(default)]
    pub name: Option<String>,
}

/// Parameters of `archbuilder.roof_extrusion.edit`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditRoofExtrusion {
    /// The roof; else the selected roof by extrusion.
    #[serde(default)]
    pub roof: Option<IdParam>,
    /// The new plane, as in `archbuilder.roof_extrusion.create`.
    #[serde(default)]
    pub plane: Option<[[f64; 2]; 2]>,
    /// The new profile.
    #[serde(default)]
    pub profile: Option<Vec<[f64; 2]>>,
    /// The new start distance.
    #[serde(default)]
    pub start: Option<QuantityParam>,
    /// The new end distance.
    #[serde(default)]
    pub end: Option<QuantityParam>,
}

/// What the commands return.
#[derive(Debug, Serialize, JsonSchema)]
pub struct RoofExtrusionMade {
    /// The roof.
    pub roof: IdParam,
    /// Its storey.
    pub storey: IdParam,
    /// The thickness normal to the profile, in meters.
    pub thickness: f64,
    /// The area of the top surface in square meters.
    pub area: f64,
    /// The volume of the body in cubic meters.
    pub volume: f64,
}

fn length(ctx: &Context<'_>, param: &QuantityParam) -> Result<f64> {
    Ok(ctx.quantity(param, Dimension::LENGTH)?.value())
}

fn direction(plane: &[[f64; 2]; 2]) -> Result<f64> {
    let (dx, dy) = (plane[1][0] - plane[0][0], plane[1][1] - plane[0][1]);
    if !plane.iter().flatten().all(|v| v.is_finite()) || dx.hypot(dy) < types::MIN_WALL_LENGTH {
        return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
    }
    Ok(dy.atan2(dx))
}

fn profile_ok(profile: &[[f64; 2]]) -> Result<()> {
    if profile.len() < 2 || profile.len() > MAX_PROFILE {
        return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
    }
    Ok(())
}

pub(crate) fn next_name(snapshot: &Snapshot, storey: ObjectId, tongue: &str) -> String {
    let used = spatial::children(snapshot, ROOF_EXTRUSION, storey);
    let taken = |name: &str| {
        used.iter()
            .any(|f| types::text(&f.values, 0).is_some_and(|n| names::same(n, name)))
    };
    (used.len() + 1..)
        .map(|n| defaults::roof_extrusion(tongue, n))
        .find(|name| !taken(name))
        .unwrap_or_default()
}

fn made(ctx: &mut Context<'_>, id: ObjectId) -> Result<RoofExtrusionMade> {
    let txn = ctx.txn()?;
    let object =
        spatial::get(txn.snapshot(), id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let d = Derived::of(txn.snapshot(), &object).map_err(|_| Error::new(codes::KIND_MISMATCH))?;
    Ok(RoofExtrusionMade {
        roof: IdParam(id),
        storey: IdParam(object.header.owner.unwrap_or(id)),
        thickness: d.thickness,
        area: d.surface_area(),
        volume: d.volume(),
    })
}

fn create_roof(ctx: &mut Context<'_>, p: CreateRoofExtrusion) -> Result<RoofExtrusionMade> {
    let storey = crate::wall::storey_for(ctx, p.storey)?.id;
    crate::element_type::ensure_roof_types(ctx)?;
    let roof_type = crate::wall::choose_type(ctx, TypeKind::Roof, p.roof_type.as_deref())?;
    let angle = direction(&p.plane)?;
    profile_ok(&p.profile)?;
    let start = p
        .start
        .as_ref()
        .map(|s| length(ctx, s))
        .transpose()?
        .unwrap_or(0.0);
    let end = length(ctx, &p.end)?;
    let name = match p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => name.to_owned(),
        None => next_name(ctx.snapshot(), storey, &language(ctx)),
    };
    let id = create(
        ctx,
        ROOF_EXTRUSION,
        Some(storey),
        vec![
            Some(named(&name)),
            Some(types::length(archbuilder_model::roof::DEFAULT_THICKNESS)),
            Some(types::length(start)),
            Some(types::length(end)),
            Some(types::angle(angle)),
            None,
            None,
            Some(types::length(p.plane[0][0])),
            Some(types::length(p.plane[0][1])),
            Some(archbuilder_model::roof_extrusion::RoofExtrusion::profile_value(&p.profile)),
        ],
    )?;
    crate::element_type::assign(ctx, id, TypeKind::Roof, roof_type)?;
    ensure_valid(ctx)?;
    ctx.set_selection_after(vec![id]);
    made(ctx, id)
}

fn edit_roof(ctx: &mut Context<'_>, p: EditRoofExtrusion) -> Result<RoofExtrusionMade> {
    let id = match p.roof {
        Some(IdParam(id)) => id,
        None => {
            let snapshot = ctx.snapshot();
            ctx.selection()
                .iter()
                .copied()
                .find(|id| spatial::type_of(snapshot, *id).as_deref() == Some(ROOF_EXTRUSION))
                .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?
        }
    };
    expect(ctx.snapshot(), id, ROOF_EXTRUSION)?;
    let start = p.start.as_ref().map(|s| length(ctx, s)).transpose()?;
    let end = p.end.as_ref().map(|s| length(ctx, s)).transpose()?;
    if let Some(profile) = &p.profile {
        profile_ok(profile)?;
    }
    let plane = p
        .plane
        .as_ref()
        .map(|pl| direction(pl).map(|a| (pl[0], a)))
        .transpose()?;
    let txn = ctx.txn()?;
    let mut put = |field: &str, value| txn.modify(id, FieldPath::new(field), Some(value));
    if let Some(v) = start {
        put("start", types::length(v))?;
    }
    if let Some(v) = end {
        put("end", types::length(v))?;
    }
    if let Some((origin, angle)) = plane {
        put("x", types::length(origin[0]))?;
        put("y", types::length(origin[1]))?;
        put("direction", types::angle(angle))?;
    }
    if let Some(profile) = p.profile {
        put(
            "profile",
            archbuilder_model::roof_extrusion::RoofExtrusion::profile_value(&profile),
        )?;
    }
    ensure_valid(ctx)?;
    ctx.set_selection_after(vec![id]);
    made(ctx, id)
}

/// A message of the catalog, named by identifier.
struct Msg {
    message_id: &'static str,
}

/// Registers the commands of this module.
///
/// # Errors
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(CommandSpec::new(
        ROOF_EXTRUSION_CREATE,
        Msg {
            message_id: "ab-cmd-roof-extrusion-create-label",
        }
        .message_id,
        Msg {
            message_id: "ab-cmd-roof-extrusion-create-description",
        }
        .message_id,
        |ctx: &mut Context<'_>, p: CreateRoofExtrusion| create_roof(ctx, p),
    ))?;
    registry.register(CommandSpec::new(
        ROOF_EXTRUSION_EDIT,
        Msg {
            message_id: "ab-cmd-roof-extrusion-edit-label",
        }
        .message_id,
        Msg {
            message_id: "ab-cmd-roof-extrusion-edit-description",
        }
        .message_id,
        |ctx: &mut Context<'_>, p: EditRoofExtrusion| edit_roof(ctx, p),
    ))
}
