//! `archbuilder.shaft.create` and `archbuilder.vertical_opening.create`: cuts through floors,
//! roofs and ceilings. (A wall opening is placed by `archbuilder.opening.place`, see
//! [`crate::insert`].)
//!
//! A shaft is a sketched footprint (`points`) with a base and a top, each a level (its identifier
//! or name) and an offset; without levels it runs from the level of its storey to the level above.
//! It cuts every floor, roof and ceiling whose body it passes through, now and later. A vertical
//! opening is a sketched cut in one floor, roof or ceiling (`host`, else the selected one), which
//! hosts it: the cut goes where the host goes.

use std::sync::Arc;

use arc_base::{Error, ObjectId, Result};
use arc_command::{CommandSpec, Context, IdParam, QuantityParam, Registry};
use arc_doc::{Snapshot, Value};
use archbuilder_model::floor::{Floor, VERTICAL_SLOTS, Vertex};
use archbuilder_model::types::{self, SHAFT, VERTICAL_HOSTS, VERTICAL_OPENING};
use archbuilder_model::{defaults, names, spatial};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::codes;
use crate::floor::{FloorPrompts, Subject};
use crate::structure::{create, ensure_valid, language, named};

/// The identifier of the create-shaft command.
pub const SHAFT_CREATE: &str = "archbuilder.shaft.create";
/// The identifier of the create-vertical-opening command.
pub const VERTICAL_OPENING_CREATE: &str = "archbuilder.vertical_opening.create";

/// Parameters of `archbuilder.shaft.create`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateShaft {
    /// The storey the shaft starts on; else the storey of the selection, else the lowest.
    #[serde(default)]
    pub storey: Option<IdParam>,
    /// The footprint: points `[x, y]` in meters, in order, closed implicitly.
    pub points: Vec<[f64; 2]>,
    /// Arc edges of the sketch: pairs of the number of the vertex the edge starts at and its bulge.
    #[serde(default)]
    pub arcs: Option<Vec<[f64; 2]>>,
    /// The level of the base: its identifier or name; the storey's level when absent.
    #[serde(default)]
    #[schemars(length(max = 256))]
    pub base_level: Option<String>,
    /// The base's offset from its level; 0 when absent.
    #[serde(default)]
    pub base_offset: Option<QuantityParam>,
    /// The level of the top: its identifier or name; the level above the storey when absent.
    #[serde(default)]
    #[schemars(length(max = 256))]
    pub top_level: Option<String>,
    /// The top's offset from its level; 0 when absent.
    #[serde(default)]
    pub top_offset: Option<QuantityParam>,
    /// The name; else the first free `Shaft <n>`.
    #[serde(default)]
    pub name: Option<String>,
    /// A prompt aid: the mode the prompts start in. The command ignores it.
    #[serde(default)]
    pub mode: Option<String>,
}

/// Parameters of `archbuilder.vertical_opening.create`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateCut {
    /// The floor, roof or ceiling that hosts the opening; else the selected one.
    #[serde(default)]
    pub host: Option<IdParam>,
    /// A prompt aid: the storey the prompts ran on. The command ignores it.
    #[serde(default)]
    pub storey: Option<IdParam>,
    /// The footprint: points `[x, y]` in meters, in order, closed implicitly.
    pub points: Vec<[f64; 2]>,
    /// Arc edges of the sketch: pairs of the number of the vertex the edge starts at and its bulge.
    #[serde(default)]
    pub arcs: Option<Vec<[f64; 2]>>,
    /// The name; else the first free `Cut <n>` of the host.
    #[serde(default)]
    pub name: Option<String>,
    /// A prompt aid: the mode the prompts start in. The command ignores it.
    #[serde(default)]
    pub mode: Option<String>,
}

/// What the opening commands return.
#[derive(Debug, Serialize, JsonSchema)]
pub struct OpeningCreated {
    /// The new shaft or vertical opening.
    pub opening: IdParam,
    /// The area of the footprint in square meters.
    pub area: f64,
    /// A shaft: the elevations of its base and top, in meters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<[f64; 2]>,
}

/// `vertices` with the bulges of `arcs` set (pairs of vertex number and bulge).
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a vertex number of a sketch, checked to be one"
)]
pub(crate) fn with_arcs(mut vertices: Vec<Vertex>, arcs: Option<&[[f64; 2]]>) -> Vec<Vertex> {
    for [at, bulge] in arcs.unwrap_or_default().iter().take(vertices.len()) {
        if at.is_finite()
            && *at >= 0.0
            && let Some(v) = vertices.get_mut(*at as usize)
            && bulge.is_finite()
            && bulge.abs() <= archbuilder_model::floor::MAX_BULGE
        {
            v.bulge = *bulge;
        }
    }
    vertices
}

fn free_loop(points: &[[f64; 2]]) -> Result<Vec<Vertex>> {
    if points.len() < 3 || points.len() > archbuilder_model::floor::MAX_VERTICES {
        return Err(Error::new(codes::FLOOR_TOO_BIG)
            .with_arg("loops", archbuilder_model::floor::MAX_LOOPS.to_string())
            .with_arg("points", archbuilder_model::floor::MAX_VERTICES.to_string()));
    }
    Ok(points
        .iter()
        .map(|p| Vertex {
            loop_index: 0,
            point: *p,
            edge: None,
            bulge: 0.0,
        })
        .collect())
}

/// The level `text` names (an identifier or a name) among the levels of the building of `storey`.
fn level_named(snapshot: &Snapshot, storey: ObjectId, text: &str) -> Result<ObjectId> {
    let building = spatial::get(snapshot, storey).and_then(|s| s.header.owner);
    let levels = building
        .map(|b| spatial::levels(snapshot, b))
        .unwrap_or_default();
    levels
        .iter()
        .find(|l| {
            l.id.to_string() == text.trim()
                || types::text(&l.values, types::level::NAME).is_some_and(|n| names::same(n, text))
        })
        .map(|l| l.id)
        .ok_or_else(|| Error::new(codes::VALUE_INVALID))
}

pub(crate) fn next_name(
    snapshot: &Snapshot,
    key: &str,
    owner: Option<ObjectId>,
    tongue: &str,
) -> String {
    let used: Vec<_> = match owner {
        Some(o) => spatial::children(snapshot, key, o),
        None => spatial::objects(snapshot, key),
    };
    let taken = |name: &str| {
        used.iter()
            .any(|f| types::text(&f.values, 0).is_some_and(|n| names::same(n, name)))
    };
    (used.len() + 1..)
        .map(|n| {
            if key == SHAFT {
                defaults::shaft(tongue, n)
            } else {
                defaults::vertical_opening(tongue, n)
            }
        })
        .find(|name| !taken(name))
        .unwrap_or_default()
}

fn create_shaft(ctx: &mut Context<'_>, p: CreateShaft) -> Result<OpeningCreated> {
    let storey = crate::wall::storey_for(ctx, p.storey)?.id;
    let meters = |ctx: &Context<'_>, param: &Option<QuantityParam>| match param {
        Some(param) => crate::length(ctx, param).map(|q| q.value()),
        None => Ok(0.0),
    };
    let (base_offset, top_offset) = (meters(ctx, &p.base_offset)?, meters(ctx, &p.top_offset)?);
    let vertices = with_arcs(free_loop(&p.points)?, p.arcs.as_deref());
    let level = |ctx: &Context<'_>, text: &Option<String>| match text {
        Some(t) if !t.trim().is_empty() => level_named(ctx.snapshot(), storey, t).map(Some),
        _ => Ok(None),
    };
    let (base, top) = (level(ctx, &p.base_level)?, level(ctx, &p.top_level)?);
    let name = match p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => name.to_owned(),
        None => next_name(ctx.snapshot(), SHAFT, None, &language(ctx)),
    };
    let reference = |id: Option<ObjectId>| id.map(Value::Reference);
    let id = create(
        ctx,
        SHAFT,
        Some(storey),
        vec![
            Some(named(&name)),
            Some(types::length(base_offset)),
            Some(types::length(top_offset)),
            Some(Floor::vertex_values(&vertices)),
            None,
            None,
            reference(base),
            reference(top),
        ],
    )?;
    ensure_valid(ctx)?;
    let txn = ctx.txn()?;
    let span = spatial::get(txn.snapshot(), id)
        .and_then(|o| archbuilder_model::opening::shaft_span(txn.snapshot(), &o));
    ctx.set_selection_after(vec![id]);
    Ok(OpeningCreated {
        opening: IdParam(id),
        area: span.as_ref().map_or(0.0, |s| s.boundary.area()),
        span: span.map(|s| [s.z.0, s.z.1]),
    })
}

fn create_cut(ctx: &mut Context<'_>, p: CreateCut) -> Result<OpeningCreated> {
    let snapshot = ctx.snapshot();
    let is_host = |id: ObjectId| {
        spatial::type_of(snapshot, id).is_some_and(|k| VERTICAL_HOSTS.contains(&k.as_str()))
    };
    let host = match p.host {
        Some(IdParam(id)) if is_host(id) => id,
        Some(_) => return Err(Error::new(codes::KIND_MISMATCH)),
        None => ctx
            .selection()
            .iter()
            .copied()
            .find(|id| is_host(*id))
            .ok_or_else(|| Error::new(codes::NO_HOST_SELECTED))?,
    };
    let vertices = with_arcs(free_loop(&p.points)?, p.arcs.as_deref());
    let name = match p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => name.to_owned(),
        None => next_name(ctx.snapshot(), VERTICAL_OPENING, Some(host), &language(ctx)),
    };
    let id = create(
        ctx,
        VERTICAL_OPENING,
        Some(host),
        vec![Some(named(&name)), Some(Floor::vertex_values(&vertices))],
    )?;
    ensure_valid(ctx)?;
    let txn = ctx.txn()?;
    let area = spatial::get(txn.snapshot(), id)
        .and_then(|o| Floor::read(&o.values, VERTICAL_SLOTS)?.boundary_in(txn.snapshot()))
        .map_or(0.0, |b| b.area());
    ctx.set_selection_after(vec![id]);
    Ok(OpeningCreated {
        opening: IdParam(id),
        area,
        span: None,
    })
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
    registry.register(
        CommandSpec::new(
            SHAFT_CREATE,
            Msg {
                message_id: "ab-cmd-shaft-create-label",
            }
            .message_id,
            Msg {
                message_id: "ab-cmd-shaft-create-description",
            }
            .message_id,
            |ctx: &mut Context<'_>, p: CreateShaft| create_shaft(ctx, p),
        )
        .with_prompt(Arc::new(FloorPrompts::of(Subject::Shaft))),
    )?;
    registry.register(
        CommandSpec::new(
            VERTICAL_OPENING_CREATE,
            Msg {
                message_id: "ab-cmd-vertical-opening-create-label",
            }
            .message_id,
            Msg {
                message_id: "ab-cmd-vertical-opening-create-description",
            }
            .message_id,
            |ctx: &mut Context<'_>, p: CreateCut| create_cut(ctx, p),
        )
        .with_prompt(Arc::new(FloorPrompts::of(Subject::Cut))),
    )
}

/// Moves the shafts or vertical openings `ids`, whose boundary is at `slots`, by `xf`.
pub(crate) fn move_cuts(
    ctx: &mut Context<'_>,
    snapshot: &Snapshot,
    ids: &[ObjectId],
    slots: archbuilder_model::floor::Slots,
    xf: &archbuilder_model::transform::Xform,
) -> Result<()> {
    for id in ids {
        let object = spatial::get(snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let f =
            Floor::read(&object.values, slots).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let boundary = f.boundary_in(snapshot);
        // A cut is not tied to the walls it was drawn on once it moves.
        let (vertices, _) = crate::modify::floor_vertices(&f, boundary.as_ref(), xf, &|_| None);
        crate::floor::set_vertices(ctx, *id, &vertices)?;
    }
    Ok(())
}

/// Copies the shafts or vertical openings `ids` moved by `xf`; a vertical opening goes to the
/// host `host_of` names for its own (the copy of its host), else stays in its host.
pub(crate) fn copy_cuts(
    ctx: &mut Context<'_>,
    snapshot: &Snapshot,
    ids: &[ObjectId],
    xf: &archbuilder_model::transform::Xform,
    host_of: &dyn Fn(ObjectId) -> Option<ObjectId>,
    target: Option<ObjectId>,
) -> Result<Vec<ObjectId>> {
    let tongue = language(ctx);
    let mut out = Vec::new();
    for id in ids {
        let object = spatial::get(snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let key =
            spatial::type_of(snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let slots = archbuilder_model::floor::slots_of(&key)
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let f =
            Floor::read(&object.values, slots).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let boundary = f.boundary_in(snapshot);
        let (vertices, _) = crate::modify::floor_vertices(&f, boundary.as_ref(), xf, &|_| None);
        let owner = if key == VERTICAL_OPENING {
            object.header.owner.map(|h| host_of(h).unwrap_or(h))
        } else {
            target.or(object.header.owner)
        };
        let seen = ctx.txn()?.snapshot().clone();
        let mut values = object.values.clone();
        let (name_at, vertices_at, arcs_at) = if key == SHAFT {
            (
                types::shaft::NAME,
                types::shaft::VERTICES,
                types::shaft::ARCS,
            )
        } else {
            (
                types::vertical_opening::NAME,
                types::vertical_opening::VERTICES,
                types::vertical_opening::ARCS,
            )
        };
        if let Some(slot) = values.get_mut(name_at) {
            *slot = Some(Value::String(next_name(
                &seen,
                &key,
                if key == SHAFT { None } else { owner },
                &tongue,
            )));
        }
        if let Some(slot) = values.get_mut(vertices_at) {
            *slot = Some(Floor::vertex_values(&vertices));
        }
        if let Some(slot) = values.get_mut(arcs_at) {
            *slot = Floor::arc_values(&vertices);
        }
        // A shaft copied to another storey keeps its levels only when it has none of its own.
        if key == SHAFT && target.is_some_and(|t| Some(t) != object.header.owner) {
            for at in [types::shaft::BASE_LEVEL, types::shaft::TOP_LEVEL] {
                if let Some(slot) = values.get_mut(at) {
                    *slot = None;
                }
            }
        }
        out.push(create(ctx, &key, owner, values)?);
    }
    Ok(out)
}
