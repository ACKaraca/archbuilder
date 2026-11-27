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
//!
//! The prompts ([`RoofExtrusionPrompts`]) ask for the two points of the plane in plan, then for
//! the points of the profile typed as `u,z` (a plan cannot show heights), until Enter, then for
//! the end of the extrusion.

use arc_base::{Dimension, Error, ObjectId, Result};
use arc_command::prompt::{
    Accept, KeywordOption, PointDims, Prompt, PromptInput, PromptMachine, PromptState, Step,
};
use arc_command::{CommandSpec, Context, IdParam, QuantityParam, Registry};
use arc_doc::{FieldPath, Snapshot};
use archbuilder_model::element_type::TypeKind;
use archbuilder_model::roof_extrusion::{Derived, MAX_PROFILE, RoofExtrusion};
use archbuilder_model::transform::Xform;
use archbuilder_model::types::{self, ROOF_EXTRUSION};
use archbuilder_model::{defaults, names, spatial};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json};

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

/// The field values `values` of a roof by extrusion moved by `xf`: the plane's origin and axis
/// follow. A reflection turns the plane's normal around, so the start and end change sign (and
/// places); the profile, measured along the axis and up, stays as it is.
pub(crate) fn moved_values(values: &mut [Option<arc_doc::Value>], xf: &Xform) -> Result<()> {
    let roof =
        RoofExtrusion::from_values(values).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let (s, c) = roof.direction.sin_cos();
    let origin = xf.point(roof.origin);
    let ahead = xf.point([roof.origin[0] + c, roof.origin[1] + s]);
    let direction = (ahead[1] - origin[1]).atan2(ahead[0] - origin[0]);
    let (start, end) = if xf.reflects() {
        (-roof.end, -roof.start)
    } else {
        (roof.start, roof.end)
    };
    for (at, value) in [
        (types::roof_extrusion::X, types::length(origin[0])),
        (types::roof_extrusion::Y, types::length(origin[1])),
        (types::roof_extrusion::DIRECTION, types::angle(direction)),
        (types::roof_extrusion::START, types::length(start)),
        (types::roof_extrusion::END, types::length(end)),
    ] {
        if let Some(slot) = values.get_mut(at) {
            *slot = Some(value);
        }
    }
    Ok(())
}

/// Moves the roofs by extrusion `ids` by `xf` in place.
pub(crate) fn move_roofs(
    ctx: &mut Context<'_>,
    snapshot: &Snapshot,
    ids: &[ObjectId],
    xf: &Xform,
) -> Result<()> {
    for id in ids {
        let object = spatial::get(snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let mut values = object.values.clone();
        moved_values(&mut values, xf)?;
        let txn = ctx.txn()?;
        for (field, at) in [
            ("x", types::roof_extrusion::X),
            ("y", types::roof_extrusion::Y),
            ("direction", types::roof_extrusion::DIRECTION),
            ("start", types::roof_extrusion::START),
            ("end", types::roof_extrusion::END),
        ] {
            txn.modify(
                *id,
                FieldPath::new(field),
                values.get(at).cloned().flatten(),
            )?;
        }
    }
    Ok(())
}

/// Copies the roofs by extrusion `ids` moved by `xf` into their storey, or into `target`: each
/// gets the next free name there and keeps its type when the document has it, else takes the
/// default roof type.
pub(crate) fn copy_roofs(
    ctx: &mut Context<'_>,
    sources: Vec<(Option<ObjectId>, Vec<Option<arc_doc::Value>>)>,
    xf: &Xform,
    target: Option<ObjectId>,
) -> Result<Vec<ObjectId>> {
    let tongue = language(ctx);
    let mut out = Vec::new();
    for (owner, mut values) in sources {
        let storey = target
            .or(owner)
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        moved_values(&mut values, xf)?;
        let name = next_name(ctx.txn()?.snapshot(), storey, &tongue);
        if let Some(slot) = values.get_mut(types::roof_extrusion::NAME) {
            *slot = Some(named(&name));
        }
        let roof_type = match values.get(types::roof_extrusion::ROOF_TYPE) {
            Some(Some(arc_doc::Value::Reference(t))) => Some(*t),
            _ => None,
        };
        let kept = roof_type.is_some_and(|t| {
            spatial::type_of(ctx.snapshot(), t).as_deref() == Some(types::ROOF_TYPE)
        });
        if let Some(slot) = values.get_mut(types::roof_extrusion::ROOF_TYPE) {
            *slot = None;
        }
        let id = create(ctx, ROOF_EXTRUSION, Some(storey), values)?;
        let chosen = match roof_type.filter(|_| kept) {
            Some(t) => t,
            None => {
                crate::element_type::ensure_roof_types(ctx)?;
                crate::wall::choose_type(ctx, TypeKind::Roof, None)?
            }
        };
        crate::element_type::assign(ctx, id, TypeKind::Roof, chosen)?;
        out.push(id);
    }
    Ok(out)
}

/// The key prefix of the profile prompts: their points are typed `u,z` in the vertical plane, so
/// a host does not answer them with a click in plan.
pub const PROFILE_PROMPT: &str = "profile-";

const PROMPT_PLANE_START: Msg = Msg {
    message_id: "ab-prompt-roof-extrusion-plane-start",
};
const PROMPT_PLANE_END: Msg = Msg {
    message_id: "ab-prompt-roof-extrusion-plane-end",
};
const PROMPT_PROFILE: Msg = Msg {
    message_id: "ab-prompt-roof-extrusion-profile",
};
const PROMPT_END: Msg = Msg {
    message_id: "ab-prompt-roof-extrusion-end",
};
const KEY_DONE: Msg = Msg {
    message_id: "ab-keyword-roof-extrusion-done",
};

/// What the answers of the prompts give so far.
#[derive(Default)]
struct Answers {
    plane: Vec<[f64; 2]>,
    profile: Vec<[f64; 2]>,
    closed: bool,
    end: Option<f64>,
}

fn answers(state: &PromptState<'_>) -> Answers {
    let mut out = Answers::default();
    for answer in state.answers {
        match &answer.input {
            PromptInput::Point(p) if out.plane.len() < 2 => out.plane.push([p.x, p.y]),
            PromptInput::Point(p) if !out.closed => out.profile.push([p.x, p.y]),
            PromptInput::Keyword(k) if k == "done" && out.profile.len() >= 2 => out.closed = true,
            PromptInput::Distance(d) if out.closed => out.end = Some(*d),
            _ => {}
        }
    }
    out
}

fn points(list: &[[f64; 2]]) -> Json {
    Json::Array(list.iter().map(|p| Json::from(p.to_vec())).collect())
}

/// The prompts of `archbuilder.roof_extrusion.create`: the plane, the profile, the end.
#[derive(Debug)]
pub struct RoofExtrusionPrompts;

impl PromptMachine for RoofExtrusionPrompts {
    fn next(&self, state: &PromptState<'_>) -> Result<Step> {
        let got = answers(state);
        if let Some(end) = got.end {
            let mut params = Map::new();
            params.insert("plane".into(), points(&got.plane));
            params.insert("profile".into(), points(&got.profile));
            params.insert("end".into(), Json::from(end));
            return Ok(Step::Done(params));
        }
        let at = state.answers.len();
        let point = Accept::Point {
            dims: PointDims::Two,
        };
        let prompt = if got.plane.len() < 2 {
            let message = if got.plane.is_empty() {
                PROMPT_PLANE_START
            } else {
                PROMPT_PLANE_END
            };
            Prompt::new(format!("plane-{at}"), message.message_id, point)
        } else if !got.closed {
            let mut prompt = Prompt::new(
                format!("{PROFILE_PROMPT}{at}"),
                PROMPT_PROFILE.message_id,
                point,
            )
            .with_arg("count", got.profile.len().to_string());
            if got.profile.len() >= 2 {
                prompt = prompt
                    .or(Accept::keywords([KeywordOption::new(
                        "done",
                        KEY_DONE.message_id,
                    )
                    .with_shortcut('d')]))
                    .with_default(PromptInput::Keyword("done".to_owned()));
            }
            prompt
        } else {
            Prompt::new(
                format!("end-{at}"),
                PROMPT_END.message_id,
                Accept::Distance {
                    min: None,
                    max: None,
                },
            )
        };
        Ok(Step::Ask(prompt))
    }

    fn check(&self, state: &PromptState<'_>, _: &Prompt, input: &PromptInput) -> Result<()> {
        let got = answers(state);
        let refused = match input {
            PromptInput::Point(p) if got.plane.len() == 1 => {
                let [x, y] = got.plane[0];
                (p.x - x).hypot(p.y - y) < types::MIN_WALL_LENGTH
            }
            PromptInput::Point(p) if got.plane.len() == 2 && !got.closed => {
                got.profile.len() >= MAX_PROFILE
                    || got
                        .profile
                        .last()
                        .is_some_and(|[u, z]| (p.x - u).hypot(p.y - z) < 1.0e-3)
            }
            PromptInput::Distance(d) => !d.is_finite() || d.abs() < 1.0e-3,
            _ => false,
        };
        if refused {
            return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
        }
        Ok(())
    }
}

/// Registers the commands of this module.
///
/// # Errors
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(
        CommandSpec::new(
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
        )
        .with_prompt(std::sync::Arc::new(RoofExtrusionPrompts)),
    )?;
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
