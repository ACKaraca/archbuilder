//! `archbuilder.dormer.create` and `archbuilder.roof_edge.create`: dormers and roof edge elements
//! (fascia, gutter), which the roof that hosts them owns and which go with it.
//!
//! A dormer is placed by the middle of its opening in plan (`center`) and faces `to` (a point), or
//! `direction` (an angle), or, without either, down the slope of the roof at the middle. Its
//! width, depth, pitch, style (`gable` or `shed`), wall height and thickness have defaults. A
//! dormer whose eave would lie below the host roof at the back, or whose opening leaves the roof,
//! is refused. A roof edge element sweeps a profile (a board for a fascia, a channel for a gutter
//! by default) along `edges` (their numbers in the roof's edge list) or `all` (every edge that
//! defines a slope).

use arc_base::{Dimension, Error, ObjectId, Result};
use arc_command::{CommandSpec, Context, IdParam, QuantityParam, Registry};
use archbuilder_model::dormer::{
    DEFAULT_DEPTH, DEFAULT_PITCH, DEFAULT_THICKNESS, DEFAULT_WALL_HEIGHT, DEFAULT_WIDTH, Dormer,
    Style,
};
use archbuilder_model::floor_edge::FloorEdge;
use archbuilder_model::roof::Roof;
use archbuilder_model::roof_edge::{Derived as EdgeDerived, Role, RoofEdge};
use archbuilder_model::types::{DORMER, ROOF, ROOF_EDGE};
use archbuilder_model::{defaults, names, spatial, types};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::codes;
use crate::structure::{create, ensure_valid, expect, language};

/// The identifier of the create-dormer command.
pub const DORMER_CREATE: &str = "archbuilder.dormer.create";
/// The identifier of the create-roof-edge command.
pub const ROOF_EDGE_CREATE: &str = "archbuilder.roof_edge.create";

/// The style of a dormer as a command parameter.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StyleParam {
    /// Two roof planes meeting at a ridge.
    Gable,
    /// One roof plane.
    Shed,
}

/// The role of a roof edge element as a command parameter.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoleParam {
    /// A fascia board.
    Fascia,
    /// A gutter.
    Gutter,
}

/// Parameters of `archbuilder.dormer.create`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateDormer {
    /// The host roof; else the selected roof.
    #[serde(default)]
    pub roof: Option<IdParam>,
    /// The middle of the opening in plan, `[x, y]` in meters.
    pub center: [f64; 2],
    /// A point the front faces.
    #[serde(default)]
    pub to: Option<[f64; 2]>,
    /// The direction the front faces as an angle; else down the slope.
    #[serde(default)]
    pub direction: Option<QuantityParam>,
    /// The width across the front; 1.5 m when absent.
    #[serde(default)]
    pub width: Option<QuantityParam>,
    /// The depth into the roof; 2 m when absent.
    #[serde(default)]
    pub depth: Option<QuantityParam>,
    /// The pitch of the dormer's roof; 30 degrees when absent.
    #[serde(default)]
    pub pitch: Option<QuantityParam>,
    /// The style; gable when absent.
    #[serde(default)]
    pub style: Option<StyleParam>,
    /// The height of the front wall's eave above the roof; 1.2 m when absent.
    #[serde(default)]
    pub wall_height: Option<QuantityParam>,
    /// The thickness of the walls and the roof; 0.15 m when absent.
    #[serde(default)]
    pub thickness: Option<QuantityParam>,
    /// The name; else the first free `Dormer <n>` of the roof.
    #[serde(default)]
    pub name: Option<String>,
}

/// Parameters of `archbuilder.roof_edge.create`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateRoofEdge {
    /// The roof; else the selected roof.
    #[serde(default)]
    pub roof: Option<IdParam>,
    /// Fascia or gutter; fascia when absent.
    #[serde(default)]
    pub role: Option<RoleParam>,
    /// The edges, by their numbers in the roof's edge list.
    #[serde(default)]
    pub edges: Option<Vec<usize>>,
    /// Every edge that defines a slope (the default without `edges`).
    #[serde(default)]
    pub all: Option<bool>,
    /// The profile: points `[u, v]` in meters (`u` outward from the eave, `v` above the underside).
    #[serde(default)]
    pub profile: Option<Vec<[f64; 2]>>,
    /// The name; else the first free `Fascia <n>` or `Gutter <n>` of the roof.
    #[serde(default)]
    pub name: Option<String>,
}

/// What the dormer command returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct DormerMade {
    /// The dormer.
    pub dormer: IdParam,
    /// Its roof.
    pub roof: IdParam,
    /// The area of the opening in the roof, in square meters.
    pub opening_area: f64,
    /// The volume of the walls and the small roof, in cubic meters.
    pub volume: f64,
}

/// What the roof edge command returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct RoofEdgeMade {
    /// The element.
    pub roof_edge: IdParam,
    /// Its roof.
    pub roof: IdParam,
    /// The length swept along the plan, in meters.
    pub length: f64,
    /// The volume of the solid, in cubic meters.
    pub volume: f64,
}

fn invalid() -> Error {
    Error::new(arc_command::error::codes::PARAMS_INVALID)
}

fn selected_roof(ctx: &Context<'_>, given: Option<IdParam>) -> Result<ObjectId> {
    match given {
        Some(IdParam(id)) => Ok(expect(ctx.snapshot(), id, ROOF)?.id),
        None => {
            let snapshot = ctx.snapshot();
            ctx.selection()
                .iter()
                .copied()
                .find(|id| spatial::type_of(snapshot, *id).as_deref() == Some(ROOF))
                .ok_or_else(|| Error::new(codes::NO_ROOF_SELECTED))
        }
    }
}

fn meters(ctx: &Context<'_>, param: &Option<QuantityParam>, default: f64) -> Result<f64> {
    match param {
        Some(p) => crate::length(ctx, p).map(|q| q.value()),
        None => Ok(default),
    }
}

fn next<F: Fn(usize) -> String>(
    snapshot: &arc_doc::Snapshot,
    key: &str,
    roof: ObjectId,
    name: F,
) -> String {
    let used = spatial::children(snapshot, key, roof);
    let taken = |n: &str| {
        used.iter()
            .any(|f| types::text(&f.values, 0).is_some_and(|t| names::same(t, n)))
    };
    (used.len() + 1..)
        .map(&name)
        .find(|n| !taken(n))
        .unwrap_or_default()
}

pub(crate) fn next_dormer_name(
    snapshot: &arc_doc::Snapshot,
    roof: ObjectId,
    tongue: &str,
) -> String {
    next(snapshot, DORMER, roof, |n| defaults::dormer(tongue, n))
}

pub(crate) fn next_roof_edge_name(
    snapshot: &arc_doc::Snapshot,
    roof: ObjectId,
    gutter: bool,
    tongue: &str,
) -> String {
    next(snapshot, ROOF_EDGE, roof, |n| {
        defaults::roof_edge(tongue, gutter, n)
    })
}

fn create_dormer(ctx: &mut Context<'_>, p: CreateDormer) -> Result<DormerMade> {
    let roof = selected_roof(ctx, p.roof)?;
    let direction = match (&p.to, &p.direction) {
        (Some(to), _) => {
            let d = [to[0] - p.center[0], to[1] - p.center[1]];
            if !d.iter().all(|v| v.is_finite()) || d[0].hypot(d[1]) < 1.0e-6 {
                return Err(invalid());
            }
            d[1].atan2(d[0])
        }
        (None, Some(QuantityParam::Text(t))) if t.trim().parse::<f64>().is_ok() => {
            t.trim().parse::<f64>().unwrap_or(0.0).to_radians()
        }
        (None, Some(param)) => ctx.quantity(param, Dimension::ANGLE)?.value(),
        (None, None) => downslope(ctx, roof, p.center),
    };
    let pitch = match &p.pitch {
        Some(QuantityParam::Text(t)) if t.trim().parse::<f64>().is_ok() => {
            t.trim().parse::<f64>().unwrap_or(0.0).to_radians()
        }
        Some(param) => ctx.quantity(param, Dimension::ANGLE)?.value(),
        None => DEFAULT_PITCH,
    };
    let dormer = Dormer {
        center: p.center,
        direction,
        width: meters(ctx, &p.width, DEFAULT_WIDTH)?,
        depth: meters(ctx, &p.depth, DEFAULT_DEPTH)?,
        pitch,
        style: match p.style {
            Some(StyleParam::Shed) => Style::Shed,
            _ => Style::Gable,
        },
        wall_height: meters(ctx, &p.wall_height, DEFAULT_WALL_HEIGHT)?,
        thickness: meters(ctx, &p.thickness, DEFAULT_THICKNESS)?,
    };
    if !dormer.sizes_ok() {
        return Err(invalid());
    }
    let tongue = language(ctx);
    let name = match p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => name.to_owned(),
        None => next(ctx.snapshot(), DORMER, roof, |n| {
            defaults::dormer(&tongue, n)
        }),
    };
    let id = create(ctx, DORMER, Some(roof), dormer.values(&name))?;
    ensure_valid(ctx)?;
    let built = {
        let txn = ctx.txn()?;
        let object = spatial::get(txn.snapshot(), id).ok_or_else(invalid)?;
        archbuilder_model::dormer::Derived::of(txn.snapshot(), &object)
    };
    // An opening off the roof, or a wall too low for the depth, leaves nothing to build.
    let built = built.map_err(|_| {
        Error::new(archbuilder_model::error::codes::DORMER_INVALID).with_arg("name", name.clone())
    })?;
    ctx.set_selection_after(vec![id]);
    Ok(DormerMade {
        dormer: IdParam(id),
        roof: IdParam(roof),
        opening_area: archbuilder_model::floor::signed_area(&dormer.opening()).abs(),
        volume: built.volume(),
    })
}

/// The direction down the slope of the roof at `at`; along the x axis on a level roof.
fn downslope(ctx: &Context<'_>, roof: ObjectId, at: [f64; 2]) -> f64 {
    let Some(surface) = spatial::get(ctx.snapshot(), roof)
        .and_then(|o| archbuilder_model::roof::Derived::of(ctx.snapshot(), &o).ok())
        .map(|d| d.surface)
    else {
        return 0.0;
    };
    let h = 0.05;
    let z = |p: [f64; 2]| surface.top_at(p);
    let (Some(x0), Some(x1), Some(y0), Some(y1)) = (
        z([at[0] - h, at[1]]),
        z([at[0] + h, at[1]]),
        z([at[0], at[1] - h]),
        z([at[0], at[1] + h]),
    ) else {
        return 0.0;
    };
    let g = [x1 - x0, y1 - y0];
    if g[0].hypot(g[1]) < 1.0e-9 {
        0.0
    } else {
        (-g[1]).atan2(-g[0])
    }
}

fn create_roof_edge(ctx: &mut Context<'_>, p: CreateRoofEdge) -> Result<RoofEdgeMade> {
    let roof = selected_roof(ctx, p.roof)?;
    let object = expect(ctx.snapshot(), roof, ROOF)?;
    let data = Roof::from_values(&object.values).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let sloped: Vec<usize> = data
        .edges
        .iter()
        .enumerate()
        .filter(|(_, e)| e.slope.is_some())
        .map(|(k, _)| k)
        .collect();
    let edges = match p.edges {
        Some(mut e) => {
            e.sort_unstable();
            e.dedup();
            // Only an edge that defines a slope has an eave.
            if e.is_empty() || e.iter().any(|k| !sloped.contains(k)) {
                return Err(invalid());
            }
            e
        }
        None if p.all.unwrap_or(true) => sloped,
        None => return Err(invalid()),
    };
    let role = match p.role {
        Some(RoleParam::Gutter) => Role::Gutter,
        _ => Role::Fascia,
    };
    let profile = p
        .profile
        .unwrap_or_else(|| RoofEdge::default_profile(role, data.thickness));
    if edges.is_empty() || !FloorEdge::profile_ok(&profile) {
        return Err(invalid());
    }
    let tongue = language(ctx);
    let gutter = role == Role::Gutter;
    let name = match p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => name.to_owned(),
        None => next(ctx.snapshot(), ROOF_EDGE, roof, |n| {
            defaults::roof_edge(&tongue, gutter, n)
        }),
    };
    let element = RoofEdge {
        role,
        profile,
        edges,
    };
    let id = create(ctx, ROOF_EDGE, Some(roof), element.values(&name))?;
    ensure_valid(ctx)?;
    let derived = {
        let txn = ctx.txn()?;
        spatial::get(txn.snapshot(), id).and_then(|o| EdgeDerived::of(txn.snapshot(), &o))
    }
    .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    ctx.set_selection_after(vec![id]);
    Ok(RoofEdgeMade {
        roof_edge: IdParam(id),
        roof: IdParam(roof),
        length: derived.length(),
        volume: derived.volume(),
    })
}

/// Copies the dormers and roof edge elements of the roofs `hosts` maps (original to copy) onto the
/// copies, under fresh names; returns the new identifiers.
pub(crate) fn copy_for(
    ctx: &mut Context<'_>,
    snapshot: &arc_doc::Snapshot,
    hosts: &std::collections::BTreeMap<ObjectId, ObjectId>,
    xf: &archbuilder_model::transform::Xform,
) -> Result<Vec<ObjectId>> {
    let tongue = language(ctx);
    let mut out = Vec::new();
    for (old, new) in hosts {
        for dormer in spatial::children(snapshot, DORMER, *old) {
            let mut values = dormer.values.clone();
            if let Some(d) = Dormer::from_values(&values) {
                let name = next(&ctx.txn()?.snapshot().clone(), DORMER, *new, |n| {
                    defaults::dormer(&tongue, n)
                });
                values = d.moved(xf).values(&name);
            }
            out.push(create(ctx, DORMER, Some(*new), values)?);
        }
        for edge in spatial::children(snapshot, ROOF_EDGE, *old) {
            let mut values = edge.values.clone();
            let gutter = RoofEdge::from_values(&values).is_some_and(|e| e.role == Role::Gutter);
            let name = next(&ctx.txn()?.snapshot().clone(), ROOF_EDGE, *new, |n| {
                defaults::roof_edge(&tongue, gutter, n)
            });
            if let Some(slot) = values.get_mut(types::roof_edge::NAME) {
                *slot = Some(arc_doc::Value::String(name));
            }
            out.push(create(ctx, ROOF_EDGE, Some(*new), values)?);
        }
    }
    Ok(out)
}

/// Moves the elevators and dormers under `xf`.
pub(crate) fn move_parts(
    ctx: &mut Context<'_>,
    snapshot: &arc_doc::Snapshot,
    elevators: &[ObjectId],
    dormers: &[ObjectId],
    xf: &archbuilder_model::transform::Xform,
) -> Result<()> {
    for id in elevators {
        let object = spatial::get(snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let moved = archbuilder_model::elevator::Elevator::from_values(&object.values)
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?
            .moved(xf);
        let name = types::text(&object.values, 0)
            .unwrap_or_default()
            .to_owned();
        crate::circulation::write_changes(ctx, types::ELEVATOR, &object, moved.values(&name))?;
    }
    for id in dormers {
        let object = spatial::get(snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let moved = Dormer::from_values(&object.values)
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?
            .moved(xf);
        let name = types::text(&object.values, 0)
            .unwrap_or_default()
            .to_owned();
        crate::circulation::write_changes(ctx, DORMER, &object, moved.values(&name))?;
    }
    Ok(())
}

/// Copies the elevators `elevators` (onto `target`, else their own storey) and the dormers
/// `dormers` whose roof is not among the copied ones (onto their own roof), moved by `xf`.
pub(crate) fn copy_parts(
    ctx: &mut Context<'_>,
    snapshot: &arc_doc::Snapshot,
    elevators: &[ObjectId],
    dormers: &[ObjectId],
    xf: &archbuilder_model::transform::Xform,
    target: Option<ObjectId>,
) -> Result<Vec<ObjectId>> {
    let tongue = language(ctx);
    let mut out = Vec::new();
    for id in elevators {
        let object = spatial::get(snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let storey = target
            .or(object.header.owner)
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let mut moved = archbuilder_model::elevator::Elevator::from_values(&object.values)
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?
            .moved(xf);
        if target.is_some_and(|t| Some(t) != object.header.owner) {
            // In another storey the levels are that storey's own.
            moved.top_level = None;
            moved.stops.clear();
        }
        let name = crate::elevator::next_name(&ctx.txn()?.snapshot().clone(), storey, &tongue);
        out.push(create(
            ctx,
            types::ELEVATOR,
            Some(storey),
            moved.values(&name),
        )?);
    }
    for id in dormers {
        let object = spatial::get(snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let roof = object
            .header
            .owner
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let moved = Dormer::from_values(&object.values)
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?
            .moved(xf);
        let name = next(&ctx.txn()?.snapshot().clone(), DORMER, roof, |n| {
            defaults::dormer(&tongue, n)
        });
        out.push(create(ctx, DORMER, Some(roof), moved.values(&name))?);
    }
    Ok(out)
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
        DORMER_CREATE,
        Msg {
            message_id: "ab-cmd-dormer-create-label",
        }
        .message_id,
        Msg {
            message_id: "ab-cmd-dormer-create-description",
        }
        .message_id,
        |ctx: &mut Context<'_>, p: CreateDormer| create_dormer(ctx, p),
    ))?;
    registry.register(CommandSpec::new(
        ROOF_EDGE_CREATE,
        Msg {
            message_id: "ab-cmd-roof-edge-create-label",
        }
        .message_id,
        Msg {
            message_id: "ab-cmd-roof-edge-create-description",
        }
        .message_id,
        |ctx: &mut Context<'_>, p: CreateRoofEdge| create_roof_edge(ctx, p),
    ))
}
