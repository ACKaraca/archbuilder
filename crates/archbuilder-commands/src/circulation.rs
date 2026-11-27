//! Railings by path, railing types, rail extensions and continuous rails, and ramps.
//!
//! - `archbuilder.railing.create`: a railing from a sketched path (`points` as `[x, y]` or
//!   `[x, y, z]` with the height of the walking surface above the level, `arcs` as pairs of vertex
//!   number and bulge) or hosted on a stair, a ramp, a floor (an edge of its boundary, `host_edge`)
//!   or a floor edge element (`host` and `side`; the path follows the host).
//!   Extensions at both ends (`start_extension`, `end_extension`: `none`, `horizontal`,
//!   `return_wall`, `return_floor`) and `continues_to`, the railing whose start the top rail
//!   continues to, are authored on it.
//! - `archbuilder.railing_type.define` and `archbuilder.ramp_type.define`: create a type, or edit
//!   the one named by `type`.
//! - `archbuilder.ramp.create`: a ramp by run with landings, a width, a thickness and a form (or
//!   by sketch: a `boundary` of points with the run line given by `start`, `to` and the
//!   landings); the slope follows from the rise (the level above, else `rise`) and the length, and
//!   a slope over the type's largest is a warning. More `runs` follow the first, each turned at
//!   the middle of the landing before it and with a slope of its own if it is given one.
//!   Railings are created on the sides named in `railings` (both by default), hosted on the ramp.
//! - `archbuilder.ramp.set_run` and `archbuilder.ramp.add_run`: change one run of a ramp (its
//!   length, landing, turn or slope; remove it) and add a run. A change of one run's slope moves
//!   the elevations of the runs after it.
//!
//! The prompts ([`RailingPrompts`], [`RampPrompts`]) ask for points: a railing takes points until
//! Enter, a ramp takes the foot and the end of the run.

use std::sync::Arc;

use arc_base::{Dimension, Error, ObjectId, Result};
use arc_command::prompt::{
    Accept, KeywordOption, PointDims, Prompt, PromptInput, PromptMachine, PromptState, Step,
};
use arc_command::{CommandSpec, Context, IdParam, QuantityParam, Registry};
use arc_doc::{FieldPath, Snapshot, Value};
use archbuilder_model::railing::{
    Corner, Derived, Extension, Profile, Railing, RailingType, Side, Vertex,
};
use archbuilder_model::ramp::{self, Form, Placed, Ramp};
use archbuilder_model::types::{
    self, FLOOR, FLOOR_EDGE, RAILING, RAILING_TYPE, RAMP, RAMP_TYPE, STAIR,
};
use archbuilder_model::{defaults, names, spatial};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json};

use crate::structure::{create, ensure_valid, expect, language};

/// The identifier of the create-railing command.
pub const RAILING_CREATE: &str = "archbuilder.railing.create";
/// The identifier of the define-railing-type command.
pub const RAILING_TYPE_DEFINE: &str = "archbuilder.railing_type.define";
/// The identifier of the create-ramp command.
pub const RAMP_CREATE: &str = "archbuilder.ramp.create";
/// The identifier of the define-ramp-type command.
pub const RAMP_TYPE_DEFINE: &str = "archbuilder.ramp_type.define";
/// The identifier of the command that edits one run of a ramp.
pub const RAMP_SET_RUN: &str = "archbuilder.ramp.set_run";
/// The identifier of the command that adds a run to a ramp.
pub const RAMP_ADD_RUN: &str = "archbuilder.ramp.add_run";

fn invalid() -> Error {
    Error::new(arc_command::error::codes::PARAMS_INVALID)
}

/// The variant of `all` named `text`; `default` without a text.
fn coded<T: Copy>(
    text: Option<&str>,
    all: &[T],
    name: impl Fn(T) -> &'static str,
    default: T,
) -> Result<T> {
    match text {
        None => Ok(default),
        Some(t) => all
            .iter()
            .copied()
            .find(|v| name(*v) == t)
            .ok_or_else(invalid),
    }
}

fn meters(ctx: &Context<'_>, param: Option<&QuantityParam>) -> Result<Option<f64>> {
    param
        .map(|p| ctx.quantity(p, Dimension::LENGTH).map(|q| q.value()))
        .transpose()
}

/// The object `text` names (an identifier, else a name) among the records of kind `key`.
fn named(snapshot: &Snapshot, key: &str, text: &str) -> Result<ObjectId> {
    let found = spatial::objects(snapshot, key).into_iter().find(|o| {
        o.id.to_string() == text.trim()
            || types::text(&o.values, 0).is_some_and(|n| names::same(n, text))
    });
    found.map(|o| o.id).ok_or_else(invalid)
}

/// Parameters of the type commands that define a railing type.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DefineRailingType {
    /// The type to edit, by identifier or name; a new type is made without it.
    #[serde(default, rename = "type")]
    pub type_ref: Option<String>,
    /// The name; `Railing 900` (or its translation) for a new type without one.
    #[serde(default)]
    pub name: Option<String>,
    /// The height of the top rail above the walking surface; 0.9 m.
    #[serde(default)]
    pub height: Option<QuantityParam>,
    /// `square` or `round`.
    #[serde(default)]
    pub rail_profile: Option<String>,
    /// The width of the top rail.
    #[serde(default)]
    pub rail_width: Option<QuantityParam>,
    /// The depth of the top rail's section.
    #[serde(default)]
    pub rail_depth: Option<QuantityParam>,
    /// The height of a handrail; 0 for none.
    #[serde(default)]
    pub handrail_height: Option<QuantityParam>,
    /// `square` or `round`.
    #[serde(default)]
    pub baluster_profile: Option<String>,
    /// The size of a baluster.
    #[serde(default)]
    pub baluster_size: Option<QuantityParam>,
    /// The largest distance between the axes of two balusters along the path.
    #[serde(default)]
    pub baluster_spacing: Option<QuantityParam>,
    /// The size of a post.
    #[serde(default)]
    pub post_size: Option<QuantityParam>,
    /// `miter` or `fillet`.
    #[serde(default)]
    pub corner: Option<String>,
    /// The radius of a filleted corner.
    #[serde(default)]
    pub fillet_radius: Option<QuantityParam>,
}

/// What the type commands return.
#[derive(Debug, Serialize, JsonSchema)]
pub struct TypeDefined {
    /// The type.
    #[serde(rename = "type")]
    pub type_id: IdParam,
}

fn define_railing_type(ctx: &mut Context<'_>, p: DefineRailingType) -> Result<TypeDefined> {
    let existing = p
        .type_ref
        .as_deref()
        .map(|t| named(ctx.snapshot(), RAILING_TYPE, t))
        .transpose()?;
    let mut kind = match existing {
        Some(id) => spatial::get(ctx.snapshot(), id)
            .and_then(|o| RailingType::from_values(&o.values))
            .ok_or_else(invalid)?,
        None => RailingType::default(),
    };
    let set = |ctx: &Context<'_>, field: &mut f64, p: &Option<QuantityParam>| -> Result<()> {
        if let Some(v) = meters(ctx, p.as_ref())? {
            *field = v;
        }
        Ok(())
    };
    set(ctx, &mut kind.height, &p.height)?;
    set(ctx, &mut kind.rail_width, &p.rail_width)?;
    set(ctx, &mut kind.rail_depth, &p.rail_depth)?;
    set(ctx, &mut kind.handrail_height, &p.handrail_height)?;
    set(ctx, &mut kind.baluster_size, &p.baluster_size)?;
    set(ctx, &mut kind.baluster_spacing, &p.baluster_spacing)?;
    set(ctx, &mut kind.post_size, &p.post_size)?;
    set(ctx, &mut kind.fillet_radius, &p.fillet_radius)?;
    kind.rail_profile = coded(
        p.rail_profile.as_deref(),
        &Profile::ALL,
        Profile::name,
        kind.rail_profile,
    )?;
    kind.baluster_profile = coded(
        p.baluster_profile.as_deref(),
        &Profile::ALL,
        Profile::name,
        kind.baluster_profile,
    )?;
    kind.corner = coded(p.corner.as_deref(), &Corner::ALL, Corner::name, kind.corner)?;
    let id = match existing {
        Some(id) => {
            let name = p
                .name
                .clone()
                .or_else(|| {
                    spatial::get(ctx.snapshot(), id)
                        .and_then(|o| types::text(&o.values, 0).map(str::to_owned))
                })
                .unwrap_or_default();
            let object = spatial::get(ctx.snapshot(), id).ok_or_else(invalid)?;
            let txn = ctx.txn()?;
            for (k, value) in kind.values(&name).into_iter().enumerate() {
                if let Some(decl) = txn
                    .snapshot()
                    .registry()
                    .get(RAILING_TYPE)
                    .and_then(|d| d.schema.fields.get(k))
                    && object.values.get(k) != Some(&value)
                {
                    txn.modify(id, FieldPath::new(decl.name.as_str()), value)?;
                }
            }
            id
        }
        None => {
            let tongue = language(ctx);
            let name = p
                .name
                .clone()
                .unwrap_or_else(|| defaults::railing_type(&tongue).to_owned());
            let name = free_name(ctx.snapshot(), RAILING_TYPE, &name);
            let owner = crate::element_type::project(ctx.snapshot())?;
            create(ctx, RAILING_TYPE, Some(owner), kind.values(&name))?
        }
    };
    ensure_valid(ctx)?;
    Ok(TypeDefined {
        type_id: IdParam(id),
    })
}

/// `name`, or the first `name (n)` that no record of kind `key` has.
fn free_name(snapshot: &Snapshot, key: &str, name: &str) -> String {
    let taken = |candidate: &str| {
        spatial::objects(snapshot, key)
            .iter()
            .any(|o| types::text(&o.values, 0).is_some_and(|n| names::same(n, candidate)))
    };
    if !taken(name) {
        return name.to_owned();
    }
    (2..)
        .map(|n| format!("{name} ({n})"))
        .find(|c| !taken(c))
        .unwrap_or_default()
}

/// Parameters of `archbuilder.ramp_type.define`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DefineRampType {
    /// The type to edit, by identifier or name; a new type is made without it.
    #[serde(default, rename = "type")]
    pub type_ref: Option<String>,
    /// The name; `Ramp 1:12` (or its translation) for a new type without one.
    #[serde(default)]
    pub name: Option<String>,
    /// The largest slope of a run as rise over run, for example `0.0833` for 1 in 12.
    #[serde(default)]
    pub max_slope: Option<f64>,
    /// The thickness of a ramp made with the type.
    #[serde(default)]
    pub thickness: Option<QuantityParam>,
}

fn ramp_type_values(name: &str, max_slope: f64, thickness: f64) -> Vec<Option<Value>> {
    let mut values: Vec<Option<Value>> = vec![None; types::ramp_type::THICKNESS + 1];
    values[types::ramp_type::NAME] = Some(Value::String(name.to_owned()));
    values[types::ramp_type::MAX_SLOPE] = Some(Value::F64(max_slope));
    values[types::ramp_type::THICKNESS] = Some(types::length(thickness));
    values
}

fn define_ramp_type(ctx: &mut Context<'_>, p: DefineRampType) -> Result<TypeDefined> {
    let existing = p
        .type_ref
        .as_deref()
        .map(|t| named(ctx.snapshot(), RAMP_TYPE, t))
        .transpose()?;
    let current = existing.and_then(|id| spatial::get(ctx.snapshot(), id));
    let read = |at: usize| match current.as_ref().and_then(|o| o.values.get(at)) {
        Some(Some(Value::F64(v))) => Some(*v),
        Some(Some(Value::Quantity(q))) => Some(q.value()),
        _ => None,
    };
    let max_slope = p
        .max_slope
        .or_else(|| read(types::ramp_type::MAX_SLOPE))
        .unwrap_or(ramp::DEFAULT_MAX_SLOPE);
    if !max_slope.is_finite() || max_slope <= 0.0 {
        return Err(invalid());
    }
    let thickness = meters(ctx, p.thickness.as_ref())?
        .or_else(|| read(types::ramp_type::THICKNESS))
        .unwrap_or(ramp::DEFAULT_THICKNESS);
    let id = if let Some(id) = existing {
        let name = p
            .name
            .clone()
            .or_else(|| {
                current
                    .as_ref()
                    .and_then(|o| types::text(&o.values, 0).map(str::to_owned))
            })
            .unwrap_or_default();
        let txn = ctx.txn()?;
        for (field, value) in [
            ("name", Value::String(name)),
            ("max_slope", Value::F64(max_slope)),
            ("thickness", types::length(thickness)),
        ] {
            txn.modify(id, FieldPath::new(field), Some(value))?;
        }
        id
    } else {
        let tongue = language(ctx);
        let name = p
            .name
            .clone()
            .unwrap_or_else(|| defaults::ramp_type(&tongue).to_owned());
        let name = free_name(ctx.snapshot(), RAMP_TYPE, &name);
        let owner = crate::element_type::project(ctx.snapshot())?;
        create(
            ctx,
            RAMP_TYPE,
            Some(owner),
            ramp_type_values(&name, max_slope, thickness),
        )?
    };
    ensure_valid(ctx)?;
    Ok(TypeDefined {
        type_id: IdParam(id),
    })
}

/// The key of the instances and the name of their reference field for a railing or ramp type
/// record `id`; `None` for any other record.
pub fn instance_of_type(snapshot: &Snapshot, id: ObjectId) -> Option<(&'static str, &'static str)> {
    match spatial::type_of(snapshot, id).as_deref() {
        Some(RAILING_TYPE) => Some((RAILING, "railing_type")),
        Some(RAMP_TYPE) => Some((RAMP, "ramp_type")),
        _ => None,
    }
}

/// The railings or ramps that have the type `id`.
pub fn type_users(snapshot: &Snapshot, id: ObjectId) -> Vec<ObjectId> {
    let Some((key, _)) = instance_of_type(snapshot, id) else {
        return Vec::new();
    };
    let at = if key == RAILING {
        types::railing::RAILING_TYPE
    } else {
        types::ramp::RAMP_TYPE
    };
    spatial::objects(snapshot, key)
        .iter()
        .filter(|o| matches!(o.values.get(at), Some(Some(Value::Reference(t))) if *t == id))
        .map(|o| o.id)
        .collect()
}

/// A copy of the railing type or ramp type `id`, named `name` or, without one, like it with the
/// next free number.
pub(crate) fn duplicate_type(
    ctx: &mut Context<'_>,
    id: ObjectId,
    name: Option<&str>,
) -> Result<ObjectId> {
    let (key, _) = instance_of_type(ctx.snapshot(), id).ok_or_else(invalid)?;
    let object = spatial::get(ctx.snapshot(), id).ok_or_else(invalid)?;
    let base = name
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .or_else(|| types::text(&object.values, 0))
        .unwrap_or_default()
        .to_owned();
    let kind = if key == RAILING {
        RAILING_TYPE
    } else {
        RAMP_TYPE
    };
    let name = free_name(ctx.snapshot(), kind, &base);
    let mut values = object.values.clone();
    if let Some(slot) = values.get_mut(0) {
        *slot = Some(Value::String(name));
    }
    let owner = crate::element_type::project(ctx.snapshot())?;
    create(ctx, kind, Some(owner), values)
}

/// Gives each of `objects` that is a railing (for a railing type) or a ramp (for a ramp type) the
/// type `type_id`; returns how many. Objects of another kind are left alone.
pub(crate) fn retype(
    ctx: &mut Context<'_>,
    type_id: ObjectId,
    objects: &[ObjectId],
) -> Result<usize> {
    let (key, field) = instance_of_type(ctx.snapshot(), type_id).ok_or_else(invalid)?;
    let mut count = 0;
    for id in objects {
        if spatial::type_of(ctx.snapshot(), *id).as_deref() == Some(key) {
            ctx.txn()?
                .modify(*id, FieldPath::new(field), Some(Value::Reference(type_id)))?;
            count += 1;
        }
    }
    ensure_valid(ctx)?;
    Ok(count)
}

/// The railing type `given` names, else the default one (made when the document has none).
fn railing_type_for(ctx: &mut Context<'_>, given: Option<&str>) -> Result<ObjectId> {
    if let Some(text) = given {
        return named(ctx.snapshot(), RAILING_TYPE, text);
    }
    let tongue = language(ctx);
    let wanted = defaults::railing_type(&tongue);
    if let Ok(id) = named(ctx.snapshot(), RAILING_TYPE, wanted) {
        return Ok(id);
    }
    if let Some(first) = spatial::objects(ctx.snapshot(), RAILING_TYPE).first() {
        return Ok(first.id);
    }
    Ok(define_railing_type(
        ctx,
        DefineRailingType {
            type_ref: None,
            name: None,
            height: None,
            rail_profile: None,
            rail_width: None,
            rail_depth: None,
            handrail_height: None,
            baluster_profile: None,
            baluster_size: None,
            baluster_spacing: None,
            post_size: None,
            corner: None,
            fillet_radius: None,
        },
    )?
    .type_id
    .0)
}

fn ramp_type_for(ctx: &mut Context<'_>, given: Option<&str>) -> Result<ObjectId> {
    if let Some(text) = given {
        return named(ctx.snapshot(), RAMP_TYPE, text);
    }
    let tongue = language(ctx);
    if let Ok(id) = named(ctx.snapshot(), RAMP_TYPE, defaults::ramp_type(&tongue)) {
        return Ok(id);
    }
    if let Some(first) = spatial::objects(ctx.snapshot(), RAMP_TYPE).first() {
        return Ok(first.id);
    }
    Ok(define_ramp_type(
        ctx,
        DefineRampType {
            type_ref: None,
            name: None,
            max_slope: None,
            thickness: None,
        },
    )?
    .type_id
    .0)
}

pub(crate) fn next_name(
    snapshot: &Snapshot,
    key: &str,
    storey: ObjectId,
    language: &str,
) -> String {
    let used = spatial::children(snapshot, key, storey);
    let make = |n: usize| {
        if key == RAMP {
            defaults::ramp(language, n)
        } else {
            defaults::railing(language, n)
        }
    };
    let taken = |name: &str| {
        spatial::objects(snapshot, key)
            .iter()
            .any(|f| types::text(&f.values, 0).is_some_and(|n| names::same(n, name)))
    };
    (used.len() + 1..)
        .map(make)
        .find(|name| !taken(name))
        .unwrap_or_default()
}

/// Parameters of `archbuilder.railing.create`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateRailing {
    /// The storey; else the host's, else the storey of the selection, else the lowest.
    #[serde(default)]
    pub storey: Option<IdParam>,
    /// The sketched path, `[x, y]` or `[x, y, z]` in meters (`z` above the level, 0 without it).
    #[serde(default)]
    pub points: Option<Vec<Vec<f64>>>,
    /// Arcs of the path: pairs of the vertex number and the bulge of the edge that starts there.
    #[serde(default)]
    pub arcs: Option<Vec<[f64; 2]>>,
    /// The stair, ramp, floor or floor edge the railing is hosted on; the path follows it.
    #[serde(default)]
    pub host: Option<IdParam>,
    /// `left` or `right`: the side of the host, or the side the wall is on for a return.
    #[serde(default)]
    pub side: Option<String>,
    /// The railing type, by identifier or name; the default type without one.
    #[serde(default)]
    pub railing_type: Option<String>,
    /// `none`, `horizontal`, `return_wall` or `return_floor`.
    #[serde(default)]
    pub start_extension: Option<String>,
    /// As `start_extension`, at the end of the rail.
    #[serde(default)]
    pub end_extension: Option<String>,
    /// The horizontal length of an extension; 0.3 m.
    #[serde(default)]
    pub extension_length: Option<QuantityParam>,
    /// The railing whose start the top rail continues to.
    #[serde(default)]
    pub continues_to: Option<IdParam>,
    /// With a floor as `host`: the edge the railing stands on, by the number of the vertex it
    /// starts at (the whole outer boundary without it); with a floor edge: its chain.
    #[serde(default)]
    pub host_edge: Option<u32>,
    /// The name; else the first free `Railing <n>`.
    #[serde(default)]
    pub name: Option<String>,
}

/// What `archbuilder.railing.create` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct RailingCreated {
    /// The new railing.
    pub railing: IdParam,
    /// Its storey.
    pub storey: IdParam,
    /// The length along the walking surface in meters.
    pub length: f64,
    /// The number of posts.
    pub posts: usize,
    /// The number of balusters.
    pub balusters: usize,
    /// The widest clear opening between neighbors, in meters.
    pub widest_gap: f64,
}

fn path_of(p: &CreateRailing) -> Result<Vec<Vertex>> {
    let mut path = Vec::new();
    for point in p.points.as_deref().unwrap_or_default() {
        let (x, y, z) = match point.as_slice() {
            [x, y] => (*x, *y, 0.0),
            [x, y, z] => (*x, *y, *z),
            _ => return Err(invalid()),
        };
        if ![x, y, z].iter().all(|v| v.is_finite()) {
            return Err(invalid());
        }
        path.push(Vertex {
            point: [x, y, z],
            bulge: 0.0,
        });
    }
    if path.len() > archbuilder_model::railing::MAX_POINTS {
        return Err(invalid());
    }
    for [at, bulge] in p.arcs.as_deref().unwrap_or_default() {
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a vertex number, checked to be one"
        )]
        let vertex = (*at >= 0.0 && at.is_finite()).then_some(*at as usize);
        match vertex.and_then(|k| path.get_mut(k)) {
            Some(v) if bulge.is_finite() && bulge.abs() <= archbuilder_model::floor::MAX_BULGE => {
                v.bulge = *bulge;
            }
            _ => return Err(invalid()),
        }
    }
    Ok(path)
}

fn create_railing(ctx: &mut Context<'_>, p: CreateRailing) -> Result<RailingCreated> {
    let host = match &p.host {
        Some(IdParam(id)) => match spatial::type_of(ctx.snapshot(), *id).as_deref() {
            Some(STAIR | RAMP | FLOOR | FLOOR_EDGE) => Some(*id),
            _ => return Err(Error::new(crate::error::codes::KIND_MISMATCH)),
        },
        None => None,
    };
    let storey = match host {
        // A floor edge is owned by its floor; the railing is in the floor's storey.
        Some(h) if spatial::type_of(ctx.snapshot(), h).as_deref() == Some(FLOOR_EDGE) => {
            spatial::get(ctx.snapshot(), h)
                .and_then(|o| o.header.owner)
                .and_then(|f| spatial::get(ctx.snapshot(), f))
                .and_then(|f| f.header.owner)
                .ok_or_else(invalid)?
        }
        Some(h) => spatial::get(ctx.snapshot(), h)
            .and_then(|o| o.header.owner)
            .ok_or_else(invalid)?,
        None => crate::wall::storey_for(ctx, p.storey)?.id,
    };
    let path = if host.is_some() {
        Vec::new()
    } else {
        path_of(&p)?
    };
    if host.is_none() && path.len() < 2 {
        return Err(invalid());
    }
    let continues_to = match p.continues_to {
        Some(IdParam(id)) => Some(expect(ctx.snapshot(), id, RAILING)?.id),
        None => None,
    };
    let railing = Railing {
        path,
        railing_type: Some(railing_type_for(ctx, p.railing_type.as_deref())?),
        host,
        side: coded(p.side.as_deref(), &Side::ALL, Side::name, Side::Left)?,
        start: coded(
            p.start_extension.as_deref(),
            &Extension::ALL,
            Extension::name,
            Extension::None,
        )?,
        end: coded(
            p.end_extension.as_deref(),
            &Extension::ALL,
            Extension::name,
            Extension::None,
        )?,
        extension: meters(ctx, p.extension_length.as_ref())?
            .unwrap_or(archbuilder_model::railing::DEFAULT_EXTENSION),
        continues_to,
        host_edge: host.and(p.host_edge).and_then(|n| usize::try_from(n).ok()),
    };
    let name = match p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => name.to_owned(),
        None => next_name(ctx.snapshot(), RAILING, storey, &language(ctx)),
    };
    let id = create(ctx, RAILING, Some(storey), railing.values(&name))?;
    ensure_valid(ctx)?;
    let derived = {
        let txn = ctx.txn()?;
        spatial::get(txn.snapshot(), id).and_then(|o| Derived::of(txn.snapshot(), &o).ok())
    }
    .ok_or_else(|| Error::new(crate::error::codes::KIND_MISMATCH))?;
    ctx.set_selection_after(vec![id]);
    Ok(RailingCreated {
        railing: IdParam(id),
        storey: IdParam(storey),
        length: derived.length(),
        posts: derived.posts().len(),
        balusters: derived.balusters().0.len(),
        widest_gap: derived.widest_gap(),
    })
}

/// Parameters of `archbuilder.ramp.create`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateRamp {
    /// The storey; else the storey of the selection, else the lowest.
    #[serde(default)]
    pub storey: Option<IdParam>,
    /// The foot of the sloped run, `[x, y]` in meters (after the landing at the foot).
    pub start: [f64; 2],
    /// The end of the sloped run: gives the direction and, without `length`, the length.
    #[serde(default)]
    pub to: Option<[f64; 2]>,
    /// The direction as an angle, used without `to`: a JSON number is radians, a text number
    /// such as `"90"` is degrees.
    #[serde(default)]
    pub direction: Option<QuantityParam>,
    /// The horizontal length of the sloped run; 6 m without `to` and `length`.
    #[serde(default)]
    pub length: Option<QuantityParam>,
    /// The width; 1.2 m.
    #[serde(default)]
    pub width: Option<QuantityParam>,
    /// The flat landing at the foot; none.
    #[serde(default)]
    pub landing_start: Option<QuantityParam>,
    /// The flat landing at the top; none.
    #[serde(default)]
    pub landing_end: Option<QuantityParam>,
    /// The thickness; the type's.
    #[serde(default)]
    pub thickness: Option<QuantityParam>,
    /// `slab` or `solid`.
    #[serde(default)]
    pub form: Option<String>,
    /// The rise, when it is not the height to the level above.
    #[serde(default)]
    pub rise: Option<QuantityParam>,
    /// The level the ramp arrives at; else the level above the storey.
    #[serde(default)]
    pub top_level: Option<IdParam>,
    /// The slope (rise over run) of the first run, when it is not the one that carries the rise
    /// over all the runs.
    #[serde(default)]
    pub slope: Option<f64>,
    /// The runs after the first, each with the turn at the landing before it, its length, its
    /// landing and, if it is to have its own, its slope.
    #[serde(default)]
    pub runs: Option<Vec<RunParam>>,
    /// The boundary of a ramp by sketch, `[x, y]` points of a simple polygon; the foot, the
    /// direction and the lengths then describe the run line the slope rises along.
    #[serde(default)]
    pub boundary: Option<Vec<[f64; 2]>>,
    /// The ramp type, by identifier or name; the default type without one.
    #[serde(default)]
    pub ramp_type: Option<String>,
    /// The sides that get a railing, `left` and `right`; both without it, none with `[]`.
    #[serde(default)]
    pub railings: Option<Vec<String>>,
    /// The railing type of those railings.
    #[serde(default)]
    pub railing_type: Option<String>,
    /// The name; else the first free `Ramp <n>`.
    #[serde(default)]
    pub name: Option<String>,
}

/// A run after the first one of a ramp.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunParam {
    /// The turn at the landing before the run, positive to the left; a JSON number is radians, a
    /// text number such as `"90"` is degrees. None without it.
    #[serde(default)]
    pub turn: Option<QuantityParam>,
    /// The horizontal length of the sloped part.
    pub length: QuantityParam,
    /// The flat landing after the run; none.
    #[serde(default)]
    pub landing: Option<QuantityParam>,
    /// The slope of this run; the shared slope without it.
    #[serde(default)]
    pub slope: Option<f64>,
}

/// An angle as a command parameter: a JSON number is radians, a text number is degrees.
fn angle(ctx: &Context<'_>, param: &QuantityParam) -> Result<f64> {
    match param {
        QuantityParam::Text(t) if t.trim().parse::<f64>().is_ok() => {
            Ok(t.trim().parse::<f64>().unwrap_or(0.0).to_radians())
        }
        other => Ok(ctx.quantity(other, Dimension::ANGLE)?.value()),
    }
}

fn run_of(ctx: &Context<'_>, p: &RunParam) -> Result<archbuilder_model::ramp::Run> {
    Ok(archbuilder_model::ramp::Run {
        turn: p
            .turn
            .as_ref()
            .map(|t| angle(ctx, t))
            .transpose()?
            .unwrap_or(0.0),
        length: meters(ctx, Some(&p.length))?.ok_or_else(invalid)?,
        landing: meters(ctx, p.landing.as_ref())?.unwrap_or(0.0),
        slope: p.slope,
    })
}

/// What `archbuilder.ramp.create` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct RampCreated {
    /// The new ramp.
    pub ramp: IdParam,
    /// Its storey.
    pub storey: IdParam,
    /// The rise in meters.
    pub rise: f64,
    /// The slope of the steepest run (rise over run).
    pub slope: f64,
    /// Whether the slope of any run passes the type's largest.
    pub slope_exceeded: bool,
    /// The volume of the body in cubic meters.
    pub volume: f64,
    /// The railings made on its sides.
    pub railings: Vec<IdParam>,
}

fn create_ramp(ctx: &mut Context<'_>, p: CreateRamp) -> Result<RampCreated> {
    let storey = crate::wall::storey_for(ctx, p.storey)?.id;
    if !p.start.iter().all(|v| v.is_finite()) {
        return Err(invalid());
    }
    let direction = match (&p.to, &p.direction) {
        (Some(to), _) => {
            let d = [to[0] - p.start[0], to[1] - p.start[1]];
            if !d.iter().all(|v| v.is_finite()) || d[0].hypot(d[1]) < 1.0e-6 {
                return Err(invalid());
            }
            d[1].atan2(d[0])
        }
        (None, Some(param)) => angle(ctx, param)?,
        (None, None) => 0.0,
    };
    let length = match (meters(ctx, p.length.as_ref())?, p.to) {
        (Some(l), _) => l,
        (None, Some(to)) => (to[0] - p.start[0]).hypot(to[1] - p.start[1]),
        (None, None) => ramp::DEFAULT_LENGTH,
    };
    let type_id = ramp_type_for(ctx, p.ramp_type.as_deref())?;
    let type_thickness = spatial::get(ctx.snapshot(), type_id)
        .and_then(|t| types::meters(&t.values, types::ramp_type::THICKNESS))
        .unwrap_or(ramp::DEFAULT_THICKNESS);
    let top_level = match p.top_level {
        Some(IdParam(id)) => Some(expect(ctx.snapshot(), id, types::LEVEL)?.id),
        None => None,
    };
    let body = Ramp {
        start: p.start,
        direction,
        width: meters(ctx, p.width.as_ref())?.unwrap_or(ramp::DEFAULT_WIDTH),
        length,
        landing_start: meters(ctx, p.landing_start.as_ref())?.unwrap_or(0.0),
        landing_end: meters(ctx, p.landing_end.as_ref())?.unwrap_or(0.0),
        thickness: meters(ctx, p.thickness.as_ref())?.unwrap_or(type_thickness),
        form: coded(p.form.as_deref(), &Form::ALL, Form::name, Form::Slab)?,
        rise: meters(ctx, p.rise.as_ref())?,
        top_level,
        ramp_type: Some(type_id),
        slope: p.slope,
        runs: p
            .runs
            .iter()
            .flatten()
            .map(|r| run_of(ctx, r))
            .collect::<Result<Vec<_>>>()?,
        boundary: p.boundary.clone().unwrap_or_default(),
    };
    let sides = match &p.railings {
        None => vec![Side::Left, Side::Right],
        Some(list) => list
            .iter()
            .map(|s| coded(Some(s), &Side::ALL, Side::name, Side::Left))
            .collect::<Result<Vec<_>>>()?,
    };
    let name = match p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => name.to_owned(),
        None => next_name(ctx.snapshot(), RAMP, storey, &language(ctx)),
    };
    let id = create(ctx, RAMP, Some(storey), body.values(&name))?;
    ensure_valid(ctx)?;
    let mut railings = Vec::new();
    for side in sides {
        let made = create_railing(
            ctx,
            CreateRailing {
                storey: None,
                points: None,
                arcs: None,
                host: Some(IdParam(id)),
                side: Some(side.name().to_owned()),
                railing_type: p.railing_type.clone(),
                start_extension: None,
                end_extension: None,
                extension_length: None,
                continues_to: None,
                host_edge: None,
                name: None,
            },
        )?;
        railings.push(made.railing);
    }
    let layout = {
        let txn = ctx.txn()?;
        spatial::get(txn.snapshot(), id).and_then(|o| Placed::of(txn.snapshot(), &o))
    }
    .and_then(|p| p.layout.ok())
    .ok_or_else(|| Error::new(crate::error::codes::KIND_MISMATCH))?;
    ctx.set_selection_after(vec![id]);
    Ok(RampCreated {
        ramp: IdParam(id),
        storey: IdParam(storey),
        rise: layout.height,
        slope: layout.slope(),
        slope_exceeded: layout.exceeds(),
        volume: layout.volume(),
        railings,
    })
}

/// Parameters of `archbuilder.ramp.set_run`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetRampRun {
    /// The ramp.
    pub ramp: IdParam,
    /// The run: 0 is the first, 1 the one after its landing, and so on.
    pub run: usize,
    /// The new horizontal length of the sloped part.
    #[serde(default)]
    pub length: Option<QuantityParam>,
    /// The new landing after the run.
    #[serde(default)]
    pub landing: Option<QuantityParam>,
    /// The new turn at the landing before the run (not for the first run); a JSON number is
    /// radians, a text number degrees.
    #[serde(default)]
    pub turn: Option<QuantityParam>,
    /// The slope of the run (rise over run).
    #[serde(default)]
    pub slope: Option<f64>,
    /// Gives the run the shared slope again.
    #[serde(default)]
    pub clear_slope: bool,
    /// Removes the run (not the first).
    #[serde(default)]
    pub remove: bool,
}

/// Parameters of `archbuilder.ramp.add_run`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddRampRun {
    /// The ramp.
    pub ramp: IdParam,
    /// The turn at the landing before the run, positive to the left; a JSON number is radians, a
    /// text number such as `"90"` is degrees. None without it.
    #[serde(default)]
    pub turn: Option<QuantityParam>,
    /// The horizontal length of the sloped part.
    pub length: QuantityParam,
    /// The flat landing after the run; none.
    #[serde(default)]
    pub landing: Option<QuantityParam>,
    /// The slope of this run; the shared slope without it.
    #[serde(default)]
    pub slope: Option<f64>,
}

/// What the commands that edit the runs of a ramp return.
#[derive(Debug, Serialize, JsonSchema)]
pub struct RampEdited {
    /// The ramp.
    pub ramp: IdParam,
    /// The rise: the elevation of the top above the base, in meters.
    pub rise: f64,
    /// How far the top is above the elevation the ramp was to reach, in meters.
    pub top_off: f64,
    /// The slope of each run.
    pub slopes: Vec<f64>,
    /// The elevation at the start and at the end of each run, in meters.
    pub elevations: Vec<[f64; 2]>,
}

/// Changes the ramp `id` as `change` says and reports what its runs are now.
fn edit_ramp(
    ctx: &mut Context<'_>,
    id: ObjectId,
    change: impl FnOnce(&Context<'_>, &mut Ramp) -> Result<()>,
) -> Result<RampEdited> {
    let object = expect(ctx.snapshot(), id, RAMP)?;
    let name = types::text(&object.values, 0)
        .unwrap_or_default()
        .to_owned();
    let mut body = Ramp::from_values(&object.values).ok_or_else(invalid)?;
    change(ctx, &mut body)?;
    write_changes(ctx, RAMP, &object, body.values(&name))?;
    ensure_valid(ctx)?;
    let layout = {
        let txn = ctx.txn()?;
        spatial::get(txn.snapshot(), id).and_then(|o| Placed::of(txn.snapshot(), &o))
    }
    .and_then(|p| p.layout.ok())
    .ok_or_else(|| Error::new(crate::error::codes::KIND_MISMATCH))?;
    Ok(RampEdited {
        ramp: IdParam(id),
        rise: layout.height,
        top_off: layout.top_off(),
        slopes: layout.slopes().to_vec(),
        elevations: layout.elevations().iter().map(|e| [e.0, e.1]).collect(),
    })
}

fn set_ramp_run(ctx: &mut Context<'_>, p: SetRampRun) -> Result<RampEdited> {
    edit_ramp(ctx, p.ramp.0, |ctx, body| {
        let length = meters(ctx, p.length.as_ref())?;
        let landing = meters(ctx, p.landing.as_ref())?;
        let turn = p.turn.as_ref().map(|t| angle(ctx, t)).transpose()?;
        let slope = if p.clear_slope {
            Some(None)
        } else {
            p.slope.map(Some)
        };
        if p.run == 0 {
            if turn.is_some() || p.remove {
                return Err(invalid());
            }
            body.length = length.unwrap_or(body.length);
            body.landing_end = landing.unwrap_or(body.landing_end);
            body.slope = slope.unwrap_or(body.slope);
            return Ok(());
        }
        let at = p.run - 1;
        if at >= body.runs.len() {
            return Err(invalid());
        }
        if p.remove {
            body.runs.remove(at);
            return Ok(());
        }
        let run = &mut body.runs[at];
        run.length = length.unwrap_or(run.length);
        run.landing = landing.unwrap_or(run.landing);
        run.turn = turn.unwrap_or(run.turn);
        run.slope = slope.unwrap_or(run.slope);
        Ok(())
    })
}

fn add_ramp_run(ctx: &mut Context<'_>, p: AddRampRun) -> Result<RampEdited> {
    edit_ramp(ctx, p.ramp.0, |ctx, body| {
        body.runs.push(run_of(
            ctx,
            &RunParam {
                turn: p.turn,
                length: p.length,
                landing: p.landing,
                slope: p.slope,
            },
        )?);
        Ok(())
    })
}

/// Writes the fields of `new` that differ from the record `object` of kind `key`.
fn write_changes(
    ctx: &mut Context<'_>,
    key: &str,
    object: &arc_doc::Object,
    new: Vec<Option<Value>>,
) -> Result<()> {
    let txn = ctx.txn()?;
    for (k, value) in new.into_iter().enumerate().skip(1) {
        if let Some(decl) = txn
            .snapshot()
            .registry()
            .get(key)
            .and_then(|d| d.schema.fields.get(k))
            && object.values.get(k) != Some(&value)
        {
            txn.modify(object.id, FieldPath::new(decl.name.as_str()), value)?;
        }
    }
    Ok(())
}

/// Moves the ramps and the railings by path under `xf`: the foot and direction of a ramp, the
/// points of a path (a hosted railing follows its host).
pub(crate) fn move_circulation(
    ctx: &mut Context<'_>,
    snapshot: &Snapshot,
    ramps: &[ObjectId],
    railings: &[ObjectId],
    xf: &archbuilder_model::transform::Xform,
) -> Result<()> {
    let kind = || Error::new(crate::error::codes::KIND_MISMATCH);
    for id in ramps {
        let object = spatial::get(snapshot, *id).ok_or_else(kind)?;
        let name = types::text(&object.values, 0)
            .unwrap_or_default()
            .to_owned();
        let moved = Ramp::from_values(&object.values)
            .ok_or_else(kind)?
            .moved(xf);
        write_changes(ctx, RAMP, &object, moved.values(&name))?;
    }
    for id in railings {
        let object = spatial::get(snapshot, *id).ok_or_else(kind)?;
        let name = types::text(&object.values, 0)
            .unwrap_or_default()
            .to_owned();
        let moved = Railing::from_values(&object.values)
            .ok_or_else(kind)?
            .moved(xf);
        write_changes(ctx, RAILING, &object, moved.values(&name))?;
    }
    Ok(())
}

/// Copies the ramps (with the railings hosted on them) and the railings by path of `set`,
/// moved by `xf`, into `target` (else their own storey). Returns the new objects.
/// The railings hosted on the stair or ramp `host`.
pub(crate) fn hosted_railings(snapshot: &Snapshot, host: ObjectId) -> Vec<Railing> {
    spatial::objects(snapshot, RAILING)
        .iter()
        .filter_map(|o| Railing::from_values(&o.values))
        .filter(|r| r.host == Some(host))
        .collect()
}

/// Creates copies of the hosted railings `railings` on `host` in `storey`; a reflection swaps
/// their sides. A railing type the document does not have becomes the default type.
pub(crate) fn copy_hosted_railings(
    ctx: &mut Context<'_>,
    railings: Vec<Railing>,
    host: ObjectId,
    storey: ObjectId,
    reflects: bool,
    tongue: &str,
) -> Result<Vec<ObjectId>> {
    let mut created = Vec::new();
    for r in railings {
        let side = if reflects {
            match r.side {
                Side::Left => Side::Right,
                Side::Right => Side::Left,
            }
        } else {
            r.side
        };
        let name = next_name(ctx.txn()?.snapshot(), RAILING, storey, tongue);
        let copy = Railing {
            host: Some(host),
            side,
            continues_to: None,
            railing_type: own(ctx.snapshot(), r.railing_type, RAILING_TYPE),
            ..r
        };
        created.push(create(ctx, RAILING, Some(storey), copy.values(&name))?);
    }
    Ok(created)
}

/// `id` when the document has it as an object of the kind `key`.
pub(crate) fn own(snapshot: &Snapshot, id: Option<ObjectId>, key: &str) -> Option<ObjectId> {
    id.filter(|t| spatial::type_of(snapshot, *t).as_deref() == Some(key))
}

pub(crate) fn copy_circulation(
    ctx: &mut Context<'_>,
    snapshot: &Snapshot,
    set: (&[ObjectId], &[ObjectId]),
    xf: &archbuilder_model::transform::Xform,
    target: Option<ObjectId>,
    tongue: &str,
) -> Result<Vec<ObjectId>> {
    let kind = || Error::new(crate::error::codes::KIND_MISMATCH);
    let mut created = Vec::new();
    for id in set.0 {
        let object = spatial::get(snapshot, *id).ok_or_else(kind)?;
        let mut moved = Ramp::from_values(&object.values)
            .ok_or_else(kind)?
            .moved(xf);
        let storey = target.or(object.header.owner).ok_or_else(kind)?;
        // In another storey the top is that storey's level above.
        if target.is_some_and(|t| Some(t) != object.header.owner) {
            moved.top_level = None;
        }
        let name = next_name(ctx.txn()?.snapshot(), RAMP, storey, tongue);
        let new = create(ctx, RAMP, Some(storey), moved.values(&name))?;
        created.push(new);
        created.extend(copy_hosted_railings(
            ctx,
            hosted_railings(snapshot, *id),
            new,
            storey,
            xf.reflects(),
            tongue,
        )?);
    }
    for id in set.1 {
        let object = spatial::get(snapshot, *id).ok_or_else(kind)?;
        let moved = Railing::from_values(&object.values)
            .ok_or_else(kind)?
            .moved(xf);
        let storey = target.or(object.header.owner).ok_or_else(kind)?;
        let name = next_name(ctx.txn()?.snapshot(), RAILING, storey, tongue);
        let copy = Railing {
            continues_to: None,
            ..moved
        };
        created.push(create(ctx, RAILING, Some(storey), copy.values(&name))?);
    }
    Ok(created)
}

/// A message of the catalog, named by identifier.
struct Msg {
    message_id: &'static str,
}

const PROMPT_RAILING_FIRST: Msg = Msg {
    message_id: "ab-prompt-railing-first",
};
const PROMPT_RAILING_NEXT: Msg = Msg {
    message_id: "ab-prompt-railing-next",
};
const KEY_DONE: Msg = Msg {
    message_id: "ab-keyword-railing-done",
};
const PROMPT_RAMP_START: Msg = Msg {
    message_id: "ab-prompt-ramp-start",
};
const PROMPT_RAMP_END: Msg = Msg {
    message_id: "ab-prompt-ramp-end",
};
const PROMPT_RAMP_BOUNDARY: Msg = Msg {
    message_id: "ab-prompt-ramp-boundary",
};
const KEY_SKETCH: Msg = Msg {
    message_id: "ab-keyword-ramp-sketch",
};

/// The prompts of `archbuilder.railing.create`: points until Enter.
#[derive(Debug)]
pub struct RailingPrompts;

impl PromptMachine for RailingPrompts {
    fn next(&self, state: &PromptState<'_>) -> Result<Step> {
        let mut points: Vec<Json> = Vec::new();
        for answer in state.answers {
            match &answer.input {
                PromptInput::Point(p) => points.push(Json::from(vec![p.x, p.y])),
                PromptInput::Keyword(k) if k == "done" && points.len() >= 2 => {
                    let mut params = Map::new();
                    params.insert("points".into(), Json::Array(points));
                    for key in ["storey", "railing_type"] {
                        if let Some(v) = state.preset.get(key) {
                            params.insert(key.into(), v.clone());
                        }
                    }
                    return Ok(Step::Done(params));
                }
                _ => {}
            }
        }
        let count = points.len();
        let message = if count == 0 {
            PROMPT_RAILING_FIRST.message_id
        } else {
            PROMPT_RAILING_NEXT.message_id
        };
        let mut prompt = Prompt::new(
            format!("point-{}", state.answers.len()),
            message,
            Accept::Point {
                dims: PointDims::Two,
            },
        )
        .with_arg("count", count.to_string());
        if count >= 2 {
            prompt = prompt
                .or(Accept::keywords([KeywordOption::new(
                    "done",
                    KEY_DONE.message_id,
                )
                .with_shortcut('d')]))
                .with_default(PromptInput::Keyword("done".to_owned()));
        }
        Ok(Step::Ask(prompt))
    }

    fn check(&self, state: &PromptState<'_>, _: &Prompt, input: &PromptInput) -> Result<()> {
        // A point on the previous one makes no edge.
        if let (PromptInput::Point(p), Some(PromptInput::Point(last))) = (
            input,
            state
                .answers
                .iter()
                .rev()
                .map(|a| &a.input)
                .find(|i| matches!(i, PromptInput::Point(_))),
        ) && (p.x - last.x).hypot(p.y - last.y) < 1.0e-3
        {
            return Err(invalid());
        }
        Ok(())
    }
}

/// The prompts of `archbuilder.ramp.create`: the foot, then the end of the run; with the keyword
/// `sketch` at the first prompt, the run line is followed by the points of the boundary, ended
/// with Enter.
#[derive(Debug)]
pub struct RampPrompts;

impl PromptMachine for RampPrompts {
    fn next(&self, state: &PromptState<'_>) -> Result<Step> {
        let points: Vec<[f64; 2]> = state
            .answers
            .iter()
            .filter_map(|a| match &a.input {
                PromptInput::Point(p) => Some([p.x, p.y]),
                _ => None,
            })
            .collect();
        let sketch = state
            .answers
            .iter()
            .any(|a| matches!(&a.input, PromptInput::Keyword(k) if k == "sketch"));
        let done = state
            .answers
            .iter()
            .any(|a| matches!(&a.input, PromptInput::Keyword(k) if k == "done"));
        let finished = if sketch {
            done && points.len() >= 5
        } else {
            points.len() >= 2
        };
        if finished {
            let mut params = Map::new();
            params.insert("start".into(), Json::from(points[0].to_vec()));
            params.insert("to".into(), Json::from(points[1].to_vec()));
            if sketch {
                let boundary: Vec<Json> =
                    points[2..].iter().map(|p| Json::from(p.to_vec())).collect();
                params.insert("boundary".into(), Json::Array(boundary));
            }
            for key in ["storey", "ramp_type", "railing_type"] {
                if let Some(v) = state.preset.get(key) {
                    params.insert(key.into(), v.clone());
                }
            }
            return Ok(Step::Done(params));
        }
        let point = Accept::Point {
            dims: PointDims::Two,
        };
        let key = format!("point-{}", state.answers.len());
        Ok(Step::Ask(match (sketch, points.len()) {
            (false, 0) => {
                Prompt::new(key, PROMPT_RAMP_START.message_id, point).or(Accept::keywords([
                    KeywordOption::new("sketch", KEY_SKETCH.message_id).with_shortcut('s'),
                ]))
            }
            (_, 0) => Prompt::new(key, PROMPT_RAMP_START.message_id, point),
            (_, 1) => Prompt::new(key, PROMPT_RAMP_END.message_id, point),
            (_, count) => {
                let prompt = Prompt::new(key, PROMPT_RAMP_BOUNDARY.message_id, point)
                    .with_arg("count", (count - 2).to_string());
                if count >= 5 {
                    prompt
                        .or(Accept::keywords([KeywordOption::new(
                            "done",
                            KEY_DONE.message_id,
                        )
                        .with_shortcut('d')]))
                        .with_default(PromptInput::Keyword("done".to_owned()))
                } else {
                    prompt
                }
            }
        }))
    }

    fn check(&self, state: &PromptState<'_>, _: &Prompt, input: &PromptInput) -> Result<()> {
        // The end of the run is not on the foot; a boundary point is not on the one before it.
        let points: Vec<&arc_command::prompt::Point> = state
            .answers
            .iter()
            .filter_map(|a| match &a.input {
                PromptInput::Point(p) => Some(p),
                _ => None,
            })
            .collect();
        if let (PromptInput::Point(p), Some(last)) = (input, points.last())
            && (p.x - last.x).hypot(p.y - last.y) < 1.0e-6
        {
            return Err(invalid());
        }
        Ok(())
    }
}

/// Registers the commands of this module.
///
/// # Errors
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    const RC_LABEL: Msg = Msg {
        message_id: "ab-cmd-railing-create-label",
    };
    const RC_DESCRIPTION: Msg = Msg {
        message_id: "ab-cmd-railing-create-description",
    };
    let (label, description) = (RC_LABEL.message_id, RC_DESCRIPTION.message_id);
    registry.register(
        CommandSpec::new(
            RAILING_CREATE,
            label,
            description,
            |ctx: &mut Context<'_>, p: CreateRailing| create_railing(ctx, p),
        )
        .with_prompt(Arc::new(RailingPrompts)),
    )?;
    const RTD_LABEL: Msg = Msg {
        message_id: "ab-cmd-railing-type-define-label",
    };
    const RTD_DESCRIPTION: Msg = Msg {
        message_id: "ab-cmd-railing-type-define-description",
    };
    let (label, description) = (RTD_LABEL.message_id, RTD_DESCRIPTION.message_id);
    registry.register(CommandSpec::new(
        RAILING_TYPE_DEFINE,
        label,
        description,
        |ctx: &mut Context<'_>, p: DefineRailingType| define_railing_type(ctx, p),
    ))?;
    const RMC_LABEL: Msg = Msg {
        message_id: "ab-cmd-ramp-create-label",
    };
    const RMC_DESCRIPTION: Msg = Msg {
        message_id: "ab-cmd-ramp-create-description",
    };
    let (label, description) = (RMC_LABEL.message_id, RMC_DESCRIPTION.message_id);
    registry.register(
        CommandSpec::new(
            RAMP_CREATE,
            label,
            description,
            |ctx: &mut Context<'_>, p: CreateRamp| create_ramp(ctx, p),
        )
        .with_prompt(Arc::new(RampPrompts)),
    )?;
    const RMTD_LABEL: Msg = Msg {
        message_id: "ab-cmd-ramp-type-define-label",
    };
    const RMTD_DESCRIPTION: Msg = Msg {
        message_id: "ab-cmd-ramp-type-define-description",
    };
    let (label, description) = (RMTD_LABEL.message_id, RMTD_DESCRIPTION.message_id);
    registry.register(CommandSpec::new(
        RAMP_TYPE_DEFINE,
        label,
        description,
        |ctx: &mut Context<'_>, p: DefineRampType| define_ramp_type(ctx, p),
    ))?;
    const RSR_LABEL: Msg = Msg {
        message_id: "ab-cmd-ramp-set-run-label",
    };
    const RSR_DESCRIPTION: Msg = Msg {
        message_id: "ab-cmd-ramp-set-run-description",
    };
    let (label, description) = (RSR_LABEL.message_id, RSR_DESCRIPTION.message_id);
    registry.register(CommandSpec::new(
        RAMP_SET_RUN,
        label,
        description,
        |ctx: &mut Context<'_>, p: SetRampRun| set_ramp_run(ctx, p),
    ))?;
    const RAR_LABEL: Msg = Msg {
        message_id: "ab-cmd-ramp-add-run-label",
    };
    const RAR_DESCRIPTION: Msg = Msg {
        message_id: "ab-cmd-ramp-add-run-description",
    };
    let (label, description) = (RAR_LABEL.message_id, RAR_DESCRIPTION.message_id);
    registry.register(CommandSpec::new(
        RAMP_ADD_RUN,
        label,
        description,
        |ctx: &mut Context<'_>, p: AddRampRun| add_ramp_run(ctx, p),
    ))
}
