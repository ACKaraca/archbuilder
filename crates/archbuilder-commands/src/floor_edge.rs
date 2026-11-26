//! `archbuilder.floor_edge.create` and `archbuilder.floor_edge.edit`: a profile swept along
//! selected edges of a floor (slab edges, curbs, nosings).
//!
//! The profile is `profile` (points `[u, v]`: `u` outward from the slab, `v` up from the top of the
//! slab at the edge) or a rectangle of `width` outward and `height` down from the top; by default
//! a board the thickness of the slab, 50 mm wide. The edges are `edges` (the numbers of the
//! vertices they start at) or `all` (every edge of the outer loop). A floor edge is deleted with
//! `archbuilder.object.delete`, and goes with its floor.

use arc_base::{Dimension, Error, ObjectId, Result};
use arc_command::{CommandSpec, Context, IdParam, QuantityParam, Registry};
use arc_doc::{FieldPath, Value};
use archbuilder_model::floor::Floor;
use archbuilder_model::floor_edge::{Derived, FloorEdge};
use archbuilder_model::spatial;
use archbuilder_model::types::{self, FLOOR, FLOOR_EDGE};
use archbuilder_model::{defaults, names};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::codes;
use crate::structure::{create, ensure_valid, expect, language, named};

/// The identifier of the create-floor-edge command.
pub const FLOOR_EDGE_CREATE: &str = "archbuilder.floor_edge.create";
/// The identifier of the command that changes the profile or the edges of a floor edge.
pub const FLOOR_EDGE_EDIT: &str = "archbuilder.floor_edge.edit";

/// The width of the board a floor edge is by default, in meters.
pub const DEFAULT_WIDTH: f64 = 0.05;

/// Parameters of `archbuilder.floor_edge.create`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateFloorEdge {
    /// The floor; else the selected floor.
    #[serde(default)]
    pub floor: Option<IdParam>,
    /// The edges, by the number of the vertex each starts at (the vertices of all loops in order).
    #[serde(default)]
    pub edges: Option<Vec<usize>>,
    /// Every edge of the floor's outer loop.
    #[serde(default)]
    pub all: Option<bool>,
    /// The profile: points `[u, v]` in meters, in order, closed implicitly.
    #[serde(default)]
    pub profile: Option<Vec<[f64; 2]>>,
    /// The width of the rectangular profile, outward from the edge.
    #[serde(default)]
    pub width: Option<QuantityParam>,
    /// The height of the rectangular profile, down from the top of the slab; negative goes up.
    #[serde(default)]
    pub height: Option<QuantityParam>,
    /// The name; else the first free `Slab edge <n>` of the floor.
    #[serde(default)]
    pub name: Option<String>,
}

/// Parameters of `archbuilder.floor_edge.edit`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditFloorEdge {
    /// The floor edge; else the selected one.
    #[serde(default)]
    pub floor_edge: Option<IdParam>,
    /// The new edges.
    #[serde(default)]
    pub edges: Option<Vec<usize>>,
    /// Every edge of the floor's outer loop.
    #[serde(default)]
    pub all: Option<bool>,
    /// The new profile.
    #[serde(default)]
    pub profile: Option<Vec<[f64; 2]>>,
}

/// What the floor edge commands return.
#[derive(Debug, Serialize, JsonSchema)]
pub struct FloorEdgeMade {
    /// The floor edge.
    pub floor_edge: IdParam,
    /// Its floor.
    pub floor: IdParam,
    /// The length swept along the plan, in meters.
    pub length: f64,
    /// The volume of the solid, in cubic meters.
    pub volume: f64,
}

fn meters(ctx: &Context<'_>, param: &Option<QuantityParam>) -> Result<Option<f64>> {
    param
        .as_ref()
        .map(|p| ctx.quantity(p, Dimension::LENGTH).map(|q| q.value()))
        .transpose()
}

fn outer_edges(ctx: &Context<'_>, floor: ObjectId) -> Result<Vec<usize>> {
    let object = expect(ctx.snapshot(), floor, FLOOR)?;
    let data =
        Floor::from_values(&object.values).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    Ok((0..data
        .vertices
        .iter()
        .take_while(|v| v.loop_index == 0)
        .count())
        .collect())
}

fn chosen_edges(
    ctx: &Context<'_>,
    floor: ObjectId,
    edges: Option<Vec<usize>>,
    all: Option<bool>,
) -> Result<Option<Vec<usize>>> {
    let count = expect(ctx.snapshot(), floor, FLOOR)
        .ok()
        .and_then(|o| Floor::from_values(&o.values))
        .map_or(0, |f| f.vertices.len());
    match (edges, all.unwrap_or(false)) {
        (Some(mut e), _) => {
            e.sort_unstable();
            e.dedup();
            if e.is_empty() || e.iter().any(|n| *n >= count) {
                return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
            }
            Ok(Some(e))
        }
        (None, true) => outer_edges(ctx, floor).map(Some),
        (None, false) => Ok(None),
    }
}

fn made(ctx: &mut Context<'_>, id: ObjectId) -> Result<FloorEdgeMade> {
    let txn = ctx.txn()?;
    let object =
        spatial::get(txn.snapshot(), id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let d = Derived::of(txn.snapshot(), &object).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    Ok(FloorEdgeMade {
        floor_edge: IdParam(id),
        floor: IdParam(d.floor),
        length: d.length(),
        volume: d.volume(),
    })
}

fn create_edge(ctx: &mut Context<'_>, p: CreateFloorEdge) -> Result<FloorEdgeMade> {
    let floor = match p.floor {
        Some(IdParam(id)) => id,
        None => {
            let snapshot = ctx.snapshot();
            ctx.selection()
                .iter()
                .copied()
                .find(|id| spatial::type_of(snapshot, *id).as_deref() == Some(FLOOR))
                .ok_or_else(|| Error::new(codes::NO_FLOOR_SELECTED))?
        }
    };
    let thickness = expect(ctx.snapshot(), floor, FLOOR)
        .ok()
        .and_then(|o| types::meters(&o.values, types::floor::THICKNESS));
    let edges = chosen_edges(ctx, floor, p.edges, p.all.or(Some(true)))?
        .ok_or_else(|| Error::new(arc_command::error::codes::PARAMS_INVALID))?;
    let profile = match p.profile {
        Some(profile) => profile,
        None => {
            let width = meters(ctx, &p.width)?.unwrap_or(DEFAULT_WIDTH);
            let height = meters(ctx, &p.height)?.unwrap_or(thickness.unwrap_or(0.2));
            vec![[0.0, 0.0], [width, 0.0], [width, -height], [0.0, -height]]
        }
    };
    if !FloorEdge::profile_ok(&profile) {
        return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
    }
    let name = match p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => name.to_owned(),
        None => next_name(ctx.snapshot(), floor, &language(ctx)),
    };
    let id = create(
        ctx,
        FLOOR_EDGE,
        Some(floor),
        vec![
            Some(named(&name)),
            Some(FloorEdge::profile_value(&profile)),
            Some(FloorEdge::edges_value(&edges)),
        ],
    )?;
    ensure_valid(ctx)?;
    ctx.set_selection_after(vec![id]);
    made(ctx, id)
}

fn edit_edge(ctx: &mut Context<'_>, p: EditFloorEdge) -> Result<FloorEdgeMade> {
    let id = match p.floor_edge {
        Some(IdParam(id)) => id,
        None => {
            let snapshot = ctx.snapshot();
            ctx.selection()
                .iter()
                .copied()
                .find(|id| spatial::type_of(snapshot, *id).as_deref() == Some(FLOOR_EDGE))
                .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?
        }
    };
    let object = expect(ctx.snapshot(), id, FLOOR_EDGE)?;
    let floor = object
        .header
        .owner
        .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let edges = chosen_edges(ctx, floor, p.edges, p.all)?;
    if let Some(profile) = &p.profile
        && !FloorEdge::profile_ok(profile)
    {
        return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
    }
    let txn = ctx.txn()?;
    if let Some(profile) = p.profile {
        txn.modify(
            id,
            FieldPath::new("profile"),
            Some(FloorEdge::profile_value(&profile)),
        )?;
    }
    if let Some(edges) = edges {
        txn.modify(
            id,
            FieldPath::new("edges"),
            Some(FloorEdge::edges_value(&edges)),
        )?;
    }
    ensure_valid(ctx)?;
    ctx.set_selection_after(vec![id]);
    made(ctx, id)
}

pub(crate) fn next_name(snapshot: &arc_doc::Snapshot, floor: ObjectId, tongue: &str) -> String {
    let used = spatial::children(snapshot, FLOOR_EDGE, floor);
    let taken = |name: &str| {
        used.iter()
            .any(|f| types::text(&f.values, 0).is_some_and(|n| names::same(n, name)))
    };
    (used.len() + 1..)
        .map(|n| defaults::floor_edge(tongue, n))
        .find(|name| !taken(name))
        .unwrap_or_default()
}

/// Copies the floor edges of the floors `hosts` maps (original to copy) onto the copies, under
/// fresh names; returns the new identifiers.
pub(crate) fn copy_for(
    ctx: &mut Context<'_>,
    snapshot: &arc_doc::Snapshot,
    hosts: &std::collections::BTreeMap<ObjectId, ObjectId>,
) -> Result<Vec<ObjectId>> {
    let tongue = language(ctx);
    let mut out = Vec::new();
    for (old, new) in hosts {
        for edge in spatial::children(snapshot, FLOOR_EDGE, *old) {
            let mut values = edge.values.clone();
            let seen = ctx.txn()?.snapshot().clone();
            if let Some(slot) = values.get_mut(types::floor_edge::NAME) {
                *slot = Some(Value::String(next_name(&seen, *new, &tongue)));
            }
            out.push(create(ctx, FLOOR_EDGE, Some(*new), values)?);
        }
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
        FLOOR_EDGE_CREATE,
        Msg {
            message_id: "ab-cmd-floor-edge-create-label",
        }
        .message_id,
        Msg {
            message_id: "ab-cmd-floor-edge-create-description",
        }
        .message_id,
        |ctx: &mut Context<'_>, p: CreateFloorEdge| create_edge(ctx, p),
    ))?;
    registry.register(CommandSpec::new(
        FLOOR_EDGE_EDIT,
        Msg {
            message_id: "ab-cmd-floor-edge-edit-label",
        }
        .message_id,
        Msg {
            message_id: "ab-cmd-floor-edge-edit-description",
        }
        .message_id,
        |ctx: &mut Context<'_>, p: EditFloorEdge| edit_edge(ctx, p),
    ))
}
