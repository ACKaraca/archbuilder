//! The roof commands: `archbuilder.roof.create`, `archbuilder.roof.set_edge`,
//! `archbuilder.roof.move_vertex`, `archbuilder.roof.move_edge`, and the wall top attachment
//! `archbuilder.wall.attach_top` and `archbuilder.wall.detach_top`.
//!
//! A roof is made from `points` (a sketched footprint), `walls` (a closed chain of walls in
//! order) or `picks` (points that pick walls, or one point inside a closed loop of walls), as a
//! floor is. Picked walls give edges on their exterior face by default, so the footprint follows
//! the walls. Every edge slopes at `slope` (15° by default) except the edges `gables` names,
//! which are vertical gables. The roof's `overhang` is 0.5 m and its base offset (the underside
//! at the eaves above the storey's level) is the top of the highest picked wall, else 0.
//!
//! Attaching a wall's top to a roof (or a floor) makes the top follow the underside; the top
//! level and height stay stored and come back on detach. Nothing derived is stored, so editing
//! the roof moves the tops of its walls in the same transaction. One call is one transaction.

use std::collections::BTreeSet;
use std::str::FromStr;
use std::sync::Arc;

use arc_base::{Dimension, Error, ObjectId, Result};
use arc_command::prompt::{
    Accept, KeywordOption, PointDims, Prompt, PromptInput, PromptMachine, PromptState, Step,
};
use arc_command::{CommandSpec, Context, IdParam, QuantityParam, Registry};
use arc_doc::{FieldPath, Snapshot, Value};
use archbuilder_model::element_type::TypeKind;
use archbuilder_model::floor::{self, Face, Vertex, WallsRefusal};
use archbuilder_model::roof::{self as model, DEFAULT_OVERHANG, DEFAULT_SLOPE, Edge, Roof};
use archbuilder_model::types::{self, FLOOR, ROOF, WALL};
use archbuilder_model::units::{decimal_separator, length_text, slope_text, slope_unit};
use archbuilder_model::{defaults, names, spatial};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json};

use crate::error::codes;
use crate::floor::FaceParam;
use crate::structure::{LANGUAGE_SETTING, create, ensure_valid, expect, language, named};

/// The identifier of the create-roof command.
pub const ROOF_CREATE: &str = "archbuilder.roof.create";
/// The identifier of the command that edits roof edges.
pub const ROOF_SET_EDGE: &str = "archbuilder.roof.set_edge";
/// The identifier of the command that moves a footprint vertex.
pub const ROOF_MOVE_VERTEX: &str = "archbuilder.roof.move_vertex";
/// The identifier of the command that moves a footprint edge across itself.
pub const ROOF_MOVE_EDGE: &str = "archbuilder.roof.move_edge";
/// The identifier of the command that attaches wall tops.
pub const WALL_ATTACH_TOP: &str = "archbuilder.wall.attach_top";
/// The identifier of the command that detaches wall tops.
pub const WALL_DETACH_TOP: &str = "archbuilder.wall.detach_top";

/// Parameters of `archbuilder.roof.create`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateRoof {
    /// The storey; else the storey of the selection, else the lowest storey.
    #[serde(default)]
    pub storey: Option<IdParam>,
    /// A sketched footprint: points `[x, y]` in meters, in order, closed implicitly.
    #[serde(default)]
    pub points: Option<Vec<[f64; 2]>>,
    /// Walls of the storey in order around the area, each meeting the next.
    #[serde(default)]
    pub walls: Option<Vec<IdParam>>,
    /// Points that pick walls: each on a wall picks it; one point inside a closed loop of walls
    /// picks the smallest loop around it.
    #[serde(default)]
    pub picks: Option<Vec<[f64; 2]>>,
    /// The face the edges of picked walls follow; the exterior face when absent.
    #[serde(default)]
    pub face: Option<FaceParam>,
    /// The offset of picked edges from the wall face, toward the left of the wall's direction.
    #[serde(default)]
    pub edge_offset: Option<QuantityParam>,
    /// The slope of the sloped edges, an angle; 15° when absent.
    #[serde(default)]
    pub slope: Option<QuantityParam>,
    /// The numbers (from 0) of the edges that define no slope: gables.
    #[serde(default)]
    pub gables: Option<Vec<usize>>,
    /// The overhang of every edge; 0.5 m when absent.
    #[serde(default)]
    pub overhang: Option<QuantityParam>,
    /// The height of the underside at the eaves above the storey's level; else the top of the
    /// highest picked wall, else 0.
    #[serde(default)]
    pub base_offset: Option<QuantityParam>,
    /// The roof type: its identifier or name; else the document's default roof type.
    #[serde(default, rename = "type")]
    #[schemars(length(max = 256))]
    pub roof_type: Option<String>,
    /// The name; else the first free `Roof <n>` of the storey.
    #[serde(default)]
    pub name: Option<String>,
    /// A prompt aid: the mode the prompts start in (`walls` or `sketch`). The command ignores it.
    #[serde(default)]
    pub mode: Option<String>,
}

/// What `archbuilder.roof.create` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct RoofCreated {
    /// The new roof.
    pub roof: IdParam,
    /// Its storey.
    pub storey: IdParam,
    /// The number of edges.
    pub edges: usize,
    /// How many edges follow walls.
    pub walls: usize,
    /// The plan area of the outline, in square meters.
    pub area: f64,
    /// The elevation of the highest point of the top, in meters.
    pub top: f64,
}

/// Parameters of `archbuilder.roof.set_edge`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetEdge {
    /// The roof; else the selected roof.
    #[serde(default)]
    pub roof: Option<IdParam>,
    /// The numbers (from 0) of the edges; every edge when absent.
    #[serde(default)]
    pub edges: Option<Vec<usize>>,
    /// Whether the edges define a slope (false makes them gables).
    #[serde(default)]
    pub sloped: Option<bool>,
    /// The slope, an angle, of the sloped edges among them; an edge without a slope keeps it for
    /// when it slopes again.
    #[serde(default)]
    pub slope: Option<QuantityParam>,
    /// The edges' own overhang.
    #[serde(default)]
    pub overhang: Option<QuantityParam>,
    /// True gives the edges the roof's overhang again.
    #[serde(default)]
    pub roof_overhang: Option<bool>,
}

/// What the roof edit commands return.
#[derive(Debug, Serialize, JsonSchema)]
pub struct RoofChanged {
    /// The roof.
    pub roof: IdParam,
    /// The elevation of the highest point of the top, in meters.
    pub top: f64,
    /// The walls attached to the roof, whose tops follow it.
    pub attached_walls: usize,
}

/// Parameters of `archbuilder.roof.move_vertex`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MoveVertex {
    /// The roof; else the selected roof.
    #[serde(default)]
    pub roof: Option<IdParam>,
    /// The number (from 0) of the vertex: the start of the edge of that number.
    pub vertex: usize,
    /// Where the vertex goes, `[x, y]` in meters. The two edges at it stop following walls.
    pub to: [f64; 2],
}

/// Parameters of `archbuilder.roof.move_edge`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MoveEdge {
    /// The roof; else the selected roof.
    #[serde(default)]
    pub roof: Option<IdParam>,
    /// The number (from 0) of the edge.
    pub edge: usize,
    /// How far the edge moves outward (negative: inward), in meters. An edge that follows a wall
    /// keeps following it at a new offset.
    pub distance: QuantityParam,
}

/// Parameters of `archbuilder.wall.attach_top`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttachTop {
    /// The walls; else the selected walls.
    #[serde(default)]
    pub walls: Option<Vec<IdParam>>,
    /// The roof or floor; else, for each wall, the lowest roof whose outline covers its middle
    /// and whose underside there is above its base, else the floor of the next storey up.
    #[serde(default)]
    pub target: Option<IdParam>,
}

/// Parameters of `archbuilder.wall.detach_top`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DetachTop {
    /// The walls; else the selected walls.
    #[serde(default)]
    pub walls: Option<Vec<IdParam>>,
}

/// What the attach and detach commands return.
#[derive(Debug, Serialize, JsonSchema)]
pub struct TopsChanged {
    /// The walls whose top changed.
    pub walls: usize,
    /// The walls attached but unable to follow the underside (they keep their own top).
    pub not_following: usize,
}

fn too_big() -> Error {
    Error::new(codes::FLOOR_TOO_BIG)
        .with_arg("loops", "1")
        .with_arg("points", model::MAX_EDGES.to_string())
}

pub(crate) fn next_roof_name(snapshot: &Snapshot, storey: ObjectId, language: &str) -> String {
    let used = spatial::children(snapshot, ROOF, storey);
    let taken = |name: &str| {
        used.iter()
            .any(|f| types::text(&f.values, 0).is_some_and(|n| names::same(n, name)))
    };
    (used.len() + 1..)
        .map(|n| defaults::roof(language, n))
        .find(|name| !taken(name))
        .unwrap_or_default()
}

/// The refusal of a slope outside the range, the range written in the project's slope unit.
pub(crate) fn slope_invalid(snapshot: &Snapshot) -> Error {
    let unit = spatial::project(snapshot).map_or_else(Default::default, |p| slope_unit(&p.values));
    let (low, high) = model::SLOPE_RANGE;
    Error::new(codes::SLOPE_INVALID)
        .with_arg("low", slope_text(low, unit, '.'))
        .with_arg("high", slope_text(high, unit, '.'))
}

/// A typed slope in radians: an angle, a percentage or a rise over a run (`1:2`), a bare
/// number in the project's slope unit; refused outside the range.
///
/// # Errors
/// `ab.cmd.slope_invalid`, naming the range.
pub fn parse_slope(snapshot: &Snapshot, text: &str) -> Result<f64> {
    let unit = spatial::project(snapshot).map_or_else(Default::default, |p| slope_unit(&p.values));
    let (low, high) = model::SLOPE_RANGE;
    archbuilder_model::units::parse_slope(text, unit)
        .filter(|a| (low..=high).contains(a))
        .ok_or_else(|| slope_invalid(snapshot))
}

fn angle(ctx: &Context<'_>, param: &QuantityParam) -> Result<f64> {
    let value = match param {
        QuantityParam::Text(t) => return parse_slope(ctx.snapshot(), t),
        QuantityParam::Si(_) => ctx.quantity(param, Dimension::ANGLE)?.value(),
    };
    let (low, high) = model::SLOPE_RANGE;
    if (low..=high).contains(&value) {
        Ok(value)
    } else {
        Err(slope_invalid(ctx.snapshot()))
    }
}

fn length(ctx: &Context<'_>, param: &QuantityParam) -> Result<f64> {
    Ok(ctx.quantity(param, Dimension::LENGTH)?.value())
}

/// The walls of a chain or of picks, as the floor command finds them.
fn picked(
    ctx: &Context<'_>,
    storey: ObjectId,
    walls: Option<&[IdParam]>,
    picks: Option<&[[f64; 2]]>,
) -> Result<Option<Vec<(ObjectId, archbuilder_model::wall::Wall)>>> {
    let all = floor::storey_walls(ctx.snapshot(), storey);
    let no_loop = || Error::new(codes::FLOOR_NO_LOOP);
    let by_id = |id: ObjectId| {
        all.iter()
            .find(|(w, _)| *w == id)
            .copied()
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH).with_arg("expected", WALL))
    };
    let ids: Vec<ObjectId> = if let Some(list) = walls {
        list.iter().map(|IdParam(id)| *id).collect()
    } else if let Some(picks) = picks {
        if picks.len() > model::MAX_EDGES {
            return Err(too_big());
        }
        let hits: Vec<Option<ObjectId>> = picks.iter().map(|q| floor::wall_at(&all, *q)).collect();
        match (picks, hits.as_slice()) {
            ([q], [None]) => floor::loop_around(&all, *q).ok_or_else(no_loop)?,
            _ => hits
                .into_iter()
                .collect::<Option<Vec<_>>>()
                .ok_or_else(no_loop)?,
        }
    } else {
        return Ok(None);
    };
    if ids.len() > model::MAX_EDGES {
        return Err(too_big());
    }
    if ids
        .iter()
        .enumerate()
        .any(|(k, a)| ids.iter().take(k).any(|b| b == a))
    {
        return Err(no_loop());
    }
    ids.into_iter()
        .map(by_id)
        .collect::<Result<Vec<_>>>()
        .map(Some)
}

#[allow(
    clippy::too_many_lines,
    reason = "the three ways of giving a footprint, then the record"
)]
fn create_roof(ctx: &mut Context<'_>, p: CreateRoof) -> Result<RoofCreated> {
    let storey = crate::wall::storey_for(ctx, p.storey)?.id;
    crate::element_type::ensure_roof_types(ctx)?;
    let roof_type = crate::wall::choose_type(ctx, TypeKind::Roof, p.roof_type.as_deref())?;
    let slope = p
        .slope
        .as_ref()
        .map(|s| angle(ctx, s))
        .transpose()?
        .unwrap_or(DEFAULT_SLOPE);
    let overhang = p
        .overhang
        .as_ref()
        .map(|o| length(ctx, o))
        .transpose()?
        .unwrap_or(DEFAULT_OVERHANG);
    let edge_offset = p
        .edge_offset
        .as_ref()
        .map(|o| length(ctx, o))
        .transpose()?
        .unwrap_or(0.0);
    let face = p.face.map_or(Face::Exterior, Into::into);
    let walls = picked(ctx, storey, p.walls.as_deref(), p.picks.as_deref())?;
    let vertices: Vec<Vertex> = match (&walls, &p.points) {
        (Some(walls), None) => {
            floor::vertices_of_walls(walls, face, edge_offset).map_err(|r| match r {
                WallsRefusal::NoLoop => Error::new(codes::FLOOR_NO_LOOP),
                WallsRefusal::Parallel => Error::new(codes::FLOOR_WALLS_PARALLEL),
            })?
        }
        (None, Some(points)) => {
            if points.len() > model::MAX_EDGES {
                return Err(too_big());
            }
            points
                .iter()
                .map(|p| Vertex {
                    loop_index: 0,
                    point: *p,
                    edge: None,
                    bulge: 0.0,
                })
                .collect()
        }
        _ => return Err(Error::new(arc_command::error::codes::PARAMS_INVALID)),
    };
    let gables: BTreeSet<usize> = p.gables.unwrap_or_default().into_iter().collect();
    if gables.iter().any(|g| *g >= vertices.len()) {
        return Err(Error::new(codes::ROOF_EDGE_INDEX));
    }
    let edges: Vec<Edge> = vertices
        .iter()
        .enumerate()
        .map(|(k, v)| Edge {
            vertex: *v,
            slope: (!gables.contains(&k)).then_some(slope),
            overhang: None,
        })
        .collect();
    // The eaves sit on the highest picked wall.
    let base_offset = match &p.base_offset {
        Some(param) => length(ctx, param)?,
        None => walls
            .as_ref()
            .and_then(|w| {
                let snapshot = ctx.snapshot();
                w.iter()
                    .filter_map(|(id, _)| {
                        let object = spatial::get(snapshot, *id)?;
                        archbuilder_model::wall::Wall::of(snapshot, &object)
                            .map(|w| w.base_offset + w.height)
                    })
                    .reduce(f64::max)
            })
            .unwrap_or(0.0),
    };
    let language = language(ctx);
    let name = match p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => name.to_owned(),
        None => next_roof_name(ctx.snapshot(), storey, &language),
    };
    let kept = vec![slope; edges.len()];
    let id = create(
        ctx,
        ROOF,
        Some(storey),
        vec![
            Some(named(&name)),
            Some(types::length(model::DEFAULT_THICKNESS)),
            Some(types::length(base_offset)),
            Some(Roof::edge_values(&edges, &kept)),
            Some(types::length(overhang)),
            None,
        ],
    )?;
    crate::element_type::assign(ctx, id, TypeKind::Roof, roof_type)?;
    ensure_valid(ctx)?;
    let (area, top) = derived_numbers(ctx, id)?;
    ctx.set_selection_after(vec![id]);
    Ok(RoofCreated {
        roof: IdParam(id),
        storey: IdParam(storey),
        edges: edges.len(),
        walls: walls.as_ref().map_or(0, Vec::len),
        area,
        top,
    })
}

fn derived_numbers(ctx: &mut Context<'_>, id: ObjectId) -> Result<(f64, f64)> {
    let txn = ctx.txn()?;
    let object =
        spatial::get(txn.snapshot(), id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let derived = model::Derived::of(txn.snapshot(), &object)
        .map_err(|r| model::refusal_error(r, types::text(&object.values, 0).unwrap_or_default()))?;
    Ok((derived.surface.plan_area(), derived.surface.top()))
}

/// The roof a command works on: the given one, else the first selected roof.
fn roof_of(ctx: &Context<'_>, given: Option<IdParam>) -> Result<(ObjectId, Roof, Vec<f64>)> {
    let snapshot = ctx.snapshot();
    let id = match given {
        Some(IdParam(id)) => id,
        None => ctx
            .selection()
            .iter()
            .copied()
            .find(|id| spatial::type_of(snapshot, *id).as_deref() == Some(ROOF))
            .ok_or_else(|| Error::new(codes::NO_ROOF_SELECTED))?,
    };
    let object = expect(snapshot, id, ROOF)?;
    let roof = Roof::from_values(&object.values).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    Ok((id, roof, Roof::kept_slopes(&object.values)))
}

fn write_edges(
    ctx: &mut Context<'_>,
    id: ObjectId,
    edges: &[Edge],
    kept: &[f64],
) -> Result<RoofChanged> {
    ctx.txn()?.modify(
        id,
        FieldPath::new("edges"),
        Some(Roof::edge_values(edges, kept)),
    )?;
    ensure_valid(ctx)?;
    finished(ctx, id)
}

fn finished(ctx: &mut Context<'_>, id: ObjectId) -> Result<RoofChanged> {
    let (_, top) = derived_numbers(ctx, id)?;
    let attached_walls = attached_to(ctx.txn()?.snapshot(), id).len();
    Ok(RoofChanged {
        roof: IdParam(id),
        top,
        attached_walls,
    })
}

/// The walls whose top is attached to `target`.
#[must_use]
pub fn attached_to(snapshot: &Snapshot, target: ObjectId) -> Vec<ObjectId> {
    spatial::objects(snapshot, WALL)
        .into_iter()
        .filter(|w| archbuilder_model::wall::attachment(&w.values) == Some(target))
        .map(|w| w.id)
        .collect()
}

/// The grid's roof rows for `archbuilder.property.set`: `roof_slope` (every sloped edge),
/// `roof_edge_sloped_<k>`, `roof_edge_slope_<k>` and `roof_edge_overhang_<k>` (blank: the roof's).
/// A slope is text in the project's slope unit, or an angle, percentage or rise over run.
/// Returns false for a field that is none of these.
///
/// # Errors
/// `ab.cmd.value_invalid` for a value it cannot read; the errors of the edit.
pub(crate) fn set_property(
    ctx: &mut Context<'_>,
    roof: ObjectId,
    field: &str,
    value: &Json,
) -> Result<bool> {
    let invalid = || Error::new(codes::VALUE_INVALID);
    let slope = |v: &Json| -> Result<QuantityParam> {
        let radians = match v {
            Json::String(t) => parse_slope(ctx.snapshot(), t)?,
            Json::Number(n) => n.as_f64().ok_or_else(invalid)?,
            _ => return Err(invalid()),
        };
        Ok(QuantityParam::Si(radians))
    };
    let edge = |prefix: &str| {
        field
            .strip_prefix(prefix)
            .and_then(|k| k.parse::<usize>().ok())
    };
    let mut p = SetEdge {
        roof: Some(IdParam(roof)),
        edges: None,
        sloped: None,
        slope: None,
        overhang: None,
        roof_overhang: None,
    };
    if field == "roof_slope" {
        p.slope = Some(slope(value)?);
    } else if let Some(k) = edge("roof_edge_sloped_") {
        p.edges = Some(vec![k]);
        p.sloped = Some(value.as_bool().ok_or_else(invalid)?);
    } else if let Some(k) = edge("roof_edge_slope_") {
        p.edges = Some(vec![k]);
        p.slope = Some(slope(value)?);
    } else if let Some(k) = edge("roof_edge_overhang_") {
        p.edges = Some(vec![k]);
        if value.is_null() || value.as_str().is_some_and(|t| t.trim().is_empty()) {
            p.roof_overhang = Some(true);
        } else {
            p.overhang = Some(serde_json::from_value(value.clone()).map_err(|_| invalid())?);
        }
    } else {
        return Ok(false);
    }
    set_edge(ctx, p).map(|_| true)
}

fn set_edge(ctx: &mut Context<'_>, p: SetEdge) -> Result<RoofChanged> {
    let (id, roof, mut kept) = roof_of(ctx, p.roof)?;
    let chosen: Vec<usize> = match p.edges {
        Some(list) => list,
        None => (0..roof.edges.len()).collect(),
    };
    if chosen.iter().any(|k| *k >= roof.edges.len()) {
        return Err(Error::new(codes::ROOF_EDGE_INDEX));
    }
    let slope = p.slope.as_ref().map(|s| angle(ctx, s)).transpose()?;
    let overhang = p.overhang.as_ref().map(|o| length(ctx, o)).transpose()?;
    let mut edges = roof.edges;
    for k in chosen {
        let (Some(edge), Some(keep)) = (edges.get_mut(k), kept.get_mut(k)) else {
            continue;
        };
        // A new slope reaches the sloped edges; a gable keeps it for when it slopes again.
        if let Some(a) = slope {
            if edge.slope.is_some() {
                edge.slope = Some(a);
            }
            *keep = a;
        }
        match p.sloped {
            Some(false) => {
                if let Some(a) = edge.slope {
                    *keep = a;
                }
                edge.slope = None;
            }
            Some(true) if edge.slope.is_none() => edge.slope = Some(*keep),
            _ => {}
        }
        if let Some(o) = overhang {
            edge.overhang = Some(o);
        }
        if p.roof_overhang == Some(true) {
            edge.overhang = None;
        }
    }
    write_edges(ctx, id, &edges, &kept)
}

/// The edges with every edge that follows a wall in `free` fixed at its derived position and
/// no longer following it.
fn frozen_at(roof: &Roof, footprint: &[[f64; 2]], free: &[usize]) -> Vec<Edge> {
    let n = roof.edges.len();
    let mut edges = roof.edges.clone();
    for &k in free {
        let follows = edges.get(k).is_some_and(|e| e.vertex.edge.is_some());
        if !follows {
            continue;
        }
        for v in [k, (k + 1) % n] {
            if let (Some(e), Some(p)) = (edges.get_mut(v), footprint.get(v)) {
                e.vertex.point = *p;
            }
        }
        if let Some(e) = edges.get_mut(k) {
            e.vertex.edge = None;
        }
    }
    edges
}

fn move_vertex(ctx: &mut Context<'_>, p: MoveVertex) -> Result<RoofChanged> {
    let (id, roof, kept) = roof_of(ctx, p.roof)?;
    let n = roof.edges.len();
    if p.vertex >= n {
        return Err(Error::new(codes::ROOF_EDGE_INDEX));
    }
    let footprint = roof
        .footprint_in(ctx.snapshot())
        .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let mut edges = frozen_at(&roof, &footprint, &[(p.vertex + n - 1) % n, p.vertex]);
    if let Some(e) = edges.get_mut(p.vertex) {
        e.vertex.point = p.to;
    }
    write_edges(ctx, id, &edges, &kept)
}

fn move_edge(ctx: &mut Context<'_>, p: MoveEdge) -> Result<RoofChanged> {
    let (id, roof, kept) = roof_of(ctx, p.roof)?;
    let n = roof.edges.len();
    let distance = length(ctx, &p.distance)?;
    let snapshot = ctx.snapshot().clone();
    let footprint = roof
        .footprint_in(&snapshot)
        .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let (Some(&a), Some(&b)) = (footprint.get(p.edge), footprint.get((p.edge + 1) % n)) else {
        return Err(Error::new(codes::ROOF_EDGE_INDEX));
    };
    let d = [b[0] - a[0], b[1] - a[1]];
    let len = d[0].hypot(d[1]);
    if len <= 0.0 {
        return Err(Error::new(codes::VALUE_INVALID));
    }
    let ccw = floor::signed_area(&footprint) > 0.0;
    let out = if ccw {
        [d[1] / len, -d[0] / len]
    } else {
        [-d[1] / len, d[0] / len]
    };
    let mut edges = roof.edges.clone();
    let association = edges.get(p.edge).and_then(|e| e.vertex.edge);
    match association.and_then(|a| {
        spatial::get(&snapshot, a.wall)
            .and_then(|o| archbuilder_model::wall::Wall::from_values(&o.values))
            .map(|w| (a, w))
    }) {
        Some((a, wall)) => {
            // The offset is measured toward the left of the wall's direction.
            let wd = [wall.end[0] - wall.start[0], wall.end[1] - wall.start[1]];
            let wl = wd[0].hypot(wd[1]).max(f64::MIN_POSITIVE);
            let left = [-wd[1] / wl, wd[0] / wl];
            let sign = if out[0] * left[0] + out[1] * left[1] >= 0.0 {
                1.0
            } else {
                -1.0
            };
            if let Some(e) = edges.get_mut(p.edge) {
                e.vertex.edge = Some(floor::Association {
                    offset: a.offset + sign * distance,
                    ..a
                });
            }
        }
        None => {
            // A free edge: both ends move; neighbors that follow walls stay on them.
            for v in [p.edge, (p.edge + 1) % n] {
                if let (Some(e), Some(q)) = (edges.get_mut(v), footprint.get(v)) {
                    e.vertex.point = [q[0] + out[0] * distance, q[1] + out[1] * distance];
                }
            }
        }
    }
    write_edges(ctx, id, &edges, &kept)
}

/// The walls a command works on: the given ones, else the selected walls.
fn walls_of(ctx: &Context<'_>, given: Option<Vec<IdParam>>) -> Result<Vec<ObjectId>> {
    let snapshot = ctx.snapshot();
    let ids: Vec<ObjectId> = match given {
        Some(list) => list.into_iter().map(|IdParam(id)| id).collect(),
        None => ctx
            .selection()
            .iter()
            .copied()
            .filter(|id| spatial::type_of(snapshot, *id).as_deref() == Some(WALL))
            .collect(),
    };
    if ids.is_empty() {
        return Err(Error::new(codes::NO_WALL_SELECTED));
    }
    for id in &ids {
        expect(snapshot, *id, WALL)?;
    }
    Ok(ids)
}

/// The roof or floor above a wall: the roof (of the wall's building) with the lowest underside
/// over the wall's middle that is above the wall's base, else the floor of the next storey up.
fn target_above(snapshot: &Snapshot, wall: ObjectId) -> Option<ObjectId> {
    let object = spatial::get(snapshot, wall)?;
    let data = archbuilder_model::wall::Wall::from_values(&object.values)?;
    let storey = object
        .header
        .owner
        .and_then(|s| spatial::get(snapshot, s))?;
    let base = spatial::storey_elevation(snapshot, &storey) + data.base_offset;
    let middle = [
        (data.start[0] + data.end[0]) / 2.0,
        (data.start[1] + data.end[1]) / 2.0,
    ];
    let roof = spatial::objects(snapshot, ROOF)
        .into_iter()
        .filter_map(|r| {
            let d = model::Derived::of(snapshot, &r).ok()?;
            let z = d.surface.underside(middle)?;
            (z > base + types::HEIGHT_RANGE.0).then_some((z, r.id))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id);
    roof.or_else(|| {
        let above = spatial::level_above(snapshot, &storey)?;
        let upper = spatial::objects(snapshot, types::STOREY)
            .into_iter()
            .find(|s| spatial::level_of(snapshot, s).is_some_and(|l| l.id == above.id))?;
        spatial::children(snapshot, FLOOR, upper.id)
            .into_iter()
            .find(|f| {
                archbuilder_model::floor::Derived::of(snapshot, f)
                    .is_some_and(|d| d.boundary.contains(middle))
            })
            .map(|f| f.id)
    })
}

fn not_following(ctx: &mut Context<'_>, walls: &[ObjectId]) -> Result<usize> {
    let snapshot = ctx.txn()?.snapshot().clone();
    Ok(walls
        .iter()
        .filter_map(|id| spatial::get(&snapshot, *id))
        .filter(|w| archbuilder_model::validate::attach_problem(&snapshot, w).is_some())
        .count())
}

fn attach_top(ctx: &mut Context<'_>, p: AttachTop) -> Result<TopsChanged> {
    let walls = walls_of(ctx, p.walls)?;
    let snapshot = ctx.snapshot().clone();
    if let Some(IdParam(target)) = p.target
        && !matches!(
            spatial::type_of(&snapshot, target).as_deref(),
            Some(ROOF | FLOOR)
        )
    {
        return Err(Error::new(codes::ATTACH_TARGET_KIND));
    }
    let mut plan = Vec::with_capacity(walls.len());
    for wall in &walls {
        let target = match p.target {
            Some(IdParam(t)) => t,
            None => target_above(&snapshot, *wall).ok_or_else(|| {
                Error::new(codes::NO_ATTACH_TARGET).with_arg(
                    "name",
                    spatial::get(&snapshot, *wall)
                        .and_then(|o| types::text(&o.values, 0).map(str::to_owned))
                        .unwrap_or_default(),
                )
            })?,
        };
        plan.push((*wall, target));
    }
    let txn = ctx.txn()?;
    for (wall, target) in &plan {
        txn.modify(
            *wall,
            FieldPath::new("top_attach"),
            Some(Value::Reference(*target)),
        )?;
    }
    ensure_valid(ctx)?;
    let not = not_following(ctx, &walls)?;
    Ok(TopsChanged {
        walls: walls.len(),
        not_following: not,
    })
}

fn detach_top(ctx: &mut Context<'_>, p: DetachTop) -> Result<TopsChanged> {
    let walls = walls_of(ctx, p.walls)?;
    let txn = ctx.txn()?;
    for wall in &walls {
        txn.modify(*wall, FieldPath::new("top_attach"), None)?;
    }
    ensure_valid(ctx)?;
    Ok(TopsChanged {
        walls: walls.len(),
        not_following: 0,
    })
}

/// Detaches the walls attached to any of `deleted` (roofs or floors about to go), before they
/// go, so the delete can say how many. Returns how many walls were detached.
///
/// # Errors
/// The transaction's errors.
pub fn detach_from(ctx: &mut Context<'_>, deleted: &BTreeSet<ObjectId>) -> Result<usize> {
    let snapshot = ctx.txn()?.snapshot().clone();
    let walls: Vec<ObjectId> = spatial::objects(&snapshot, WALL)
        .into_iter()
        .filter(|w| {
            archbuilder_model::wall::attachment(&w.values).is_some_and(|t| deleted.contains(&t))
        })
        .map(|w| w.id)
        .collect();
    let txn = ctx.txn()?;
    for wall in &walls {
        txn.modify(*wall, FieldPath::new("top_attach"), None)?;
    }
    Ok(walls.len())
}

/// Freezes the roof edges that follow the walls `deleted`, before they go: each keeps its derived
/// position and loses its wall. Returns how many edges were frozen.
///
/// # Errors
/// The transaction's errors.
pub fn freeze_edges(ctx: &mut Context<'_>, deleted: &BTreeSet<ObjectId>) -> Result<usize> {
    let snapshot = ctx.txn()?.snapshot().clone();
    let mut frozen = 0;
    let mut changes = Vec::new();
    for object in spatial::objects(&snapshot, ROOF) {
        let Some(roof) = Roof::from_values(&object.values) else {
            continue;
        };
        if !roof.walls().iter().any(|w| deleted.contains(w)) {
            continue;
        }
        let Some(footprint) = roof.footprint_in(&snapshot) else {
            continue;
        };
        if let Some(edges) = roof.frozen(&footprint, deleted) {
            frozen += roof
                .edges
                .iter()
                .filter(|e| e.vertex.edge.is_some_and(|a| deleted.contains(&a.wall)))
                .count();
            changes.push((object.id, edges, Roof::kept_slopes(&object.values)));
        }
    }
    let txn = ctx.txn()?;
    for (id, edges, kept) in changes {
        txn.modify(
            id,
            FieldPath::new("edges"),
            Some(Roof::edge_values(&edges, &kept)),
        )?;
    }
    Ok(frozen)
}

/// The edges of the roof `object` under `xf`, with the stored slopes and the number of edges that
/// lost their wall (an edge keeps a wall when `keep` gives one: the same wall when it moves too,
/// the copy of it for a copy).
pub(crate) fn moved_edges(
    snapshot: &Snapshot,
    object: &arc_doc::Object,
    xf: &archbuilder_model::transform::Xform,
    keep: &dyn Fn(ObjectId) -> Option<ObjectId>,
) -> Result<(Vec<Edge>, Vec<f64>, usize)> {
    let roof = Roof::from_values(&object.values).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let as_floor = roof.as_floor();
    let boundary = as_floor.boundary_in(snapshot);
    let (vertices, lost) = crate::modify::floor_vertices(&as_floor, boundary.as_ref(), xf, keep);
    let edges = roof
        .edges
        .iter()
        .zip(vertices)
        .map(|(e, vertex)| Edge { vertex, ..*e })
        .collect();
    Ok((edges, Roof::kept_slopes(&object.values), lost))
}

/// Applies `xf` to the roofs `roofs` in place; returns how many edges lost their wall (an edge
/// keeps its wall when the wall is in `walls`, which move too).
pub(crate) fn move_roofs(
    ctx: &mut Context<'_>,
    snapshot: &Snapshot,
    roofs: &[ObjectId],
    xf: &archbuilder_model::transform::Xform,
    walls: &BTreeSet<ObjectId>,
) -> Result<usize> {
    let mut dissociated = 0;
    for id in roofs {
        let object = spatial::get(snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let (edges, kept, lost) =
            moved_edges(snapshot, &object, xf, &|w| walls.contains(&w).then_some(w))?;
        dissociated += lost;
        ctx.txn()?.modify(
            *id,
            FieldPath::new("edges"),
            Some(Roof::edge_values(&edges, &kept)),
        )?;
    }
    Ok(dissociated)
}

/// Copies the roofs `roofs` under `xf` to `target` (else their own storey) with the next free
/// names; returns the copies and how many edges lost their wall (`keep` gives the copy of a
/// copied wall).
pub(crate) fn copy_roofs(
    ctx: &mut Context<'_>,
    snapshot: &Snapshot,
    roofs: &[ObjectId],
    xf: &archbuilder_model::transform::Xform,
    keep: &dyn Fn(ObjectId) -> Option<ObjectId>,
    target: Option<ObjectId>,
) -> Result<(Vec<ObjectId>, usize)> {
    let tongue = language(ctx);
    let mut created = Vec::new();
    let mut dissociated = 0;
    for id in roofs {
        let object = spatial::get(snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let (edges, kept, lost) = moved_edges(snapshot, &object, xf, keep)?;
        dissociated += lost;
        let Some(storey) = target.or(object.header.owner) else {
            return Err(Error::new(codes::KIND_MISMATCH));
        };
        let seen = ctx.txn()?.snapshot().clone();
        let mut values = object.values.clone();
        if let Some(slot) = values.get_mut(types::roof::NAME) {
            *slot = Some(Value::String(next_roof_name(&seen, storey, &tongue)));
        }
        if let Some(slot) = values.get_mut(types::roof::EDGES) {
            *slot = Some(Roof::edge_values(&edges, &kept));
        }
        created.push(create(ctx, ROOF, Some(storey), values)?);
    }
    Ok((created, dissociated))
}

/// A message of the catalog, named by identifier.
struct Msg {
    message_id: &'static str,
}

const M_AB_CMD_ROOF_CREATE_DESCRIPTION: Msg = Msg {
    message_id: "ab-cmd-roof-create-description",
};
const M_AB_CMD_ROOF_CREATE_LABEL: Msg = Msg {
    message_id: "ab-cmd-roof-create-label",
};
const M_AB_CMD_ROOF_MOVE_EDGE_DESCRIPTION: Msg = Msg {
    message_id: "ab-cmd-roof-move-edge-description",
};
const M_AB_CMD_ROOF_MOVE_EDGE_LABEL: Msg = Msg {
    message_id: "ab-cmd-roof-move-edge-label",
};
const M_AB_CMD_ROOF_MOVE_VERTEX_DESCRIPTION: Msg = Msg {
    message_id: "ab-cmd-roof-move-vertex-description",
};
const M_AB_CMD_ROOF_MOVE_VERTEX_LABEL: Msg = Msg {
    message_id: "ab-cmd-roof-move-vertex-label",
};
const M_AB_CMD_ROOF_SET_EDGE_DESCRIPTION: Msg = Msg {
    message_id: "ab-cmd-roof-set-edge-description",
};
const M_AB_CMD_ROOF_SET_EDGE_LABEL: Msg = Msg {
    message_id: "ab-cmd-roof-set-edge-label",
};
const M_AB_CMD_WALL_ATTACH_TOP_DESCRIPTION: Msg = Msg {
    message_id: "ab-cmd-wall-attach-top-description",
};
const M_AB_CMD_WALL_ATTACH_TOP_LABEL: Msg = Msg {
    message_id: "ab-cmd-wall-attach-top-label",
};
const M_AB_CMD_WALL_DETACH_TOP_DESCRIPTION: Msg = Msg {
    message_id: "ab-cmd-wall-detach-top-description",
};
const M_AB_CMD_WALL_DETACH_TOP_LABEL: Msg = Msg {
    message_id: "ab-cmd-wall-detach-top-label",
};
const M_AB_KEYWORD_FLOOR_CLOSE: Msg = Msg {
    message_id: "ab-keyword-floor-close",
};
const M_AB_KEYWORD_FLOOR_DONE: Msg = Msg {
    message_id: "ab-keyword-floor-done",
};
const M_AB_KEYWORD_FLOOR_SKETCH: Msg = Msg {
    message_id: "ab-keyword-floor-sketch",
};
const M_AB_KEYWORD_FLOOR_UNDO: Msg = Msg {
    message_id: "ab-keyword-floor-undo",
};
const M_AB_KEYWORD_FLOOR_WALLS: Msg = Msg {
    message_id: "ab-keyword-floor-walls",
};
const M_AB_KEYWORD_ROOF_BASE: Msg = Msg {
    message_id: "ab-keyword-roof-base",
};
const M_AB_KEYWORD_ROOF_OVERHANG: Msg = Msg {
    message_id: "ab-keyword-roof-overhang",
};
const M_AB_KEYWORD_ROOF_SLOPE: Msg = Msg {
    message_id: "ab-keyword-roof-slope",
};
const PROMPT_PICK: Msg = Msg {
    message_id: "ab-prompt-roof-pick",
};
const PROMPT_START: Msg = Msg {
    message_id: "ab-prompt-roof-start",
};
const PROMPT_NEXT: Msg = Msg {
    message_id: "ab-prompt-roof-next",
};
const PROMPT_SLOPE: Msg = Msg {
    message_id: "ab-prompt-roof-slope",
};
const PROMPT_OVERHANG: Msg = Msg {
    message_id: "ab-prompt-roof-overhang",
};
const PROMPT_BASE: Msg = Msg {
    message_id: "ab-prompt-roof-base",
};

/// The prompts of `archbuilder.roof.create`: picks of walls (or a sketch) and the keywords
/// `slope`, `overhang`, `base`, `sketch`/`walls`, `undo` and `done`/`close`.
#[derive(Debug)]
pub struct RoofPrompts;

struct Scan {
    sketch: bool,
    slope: f64,
    overhang: f64,
    base: Option<f64>,
    points: Vec<[f64; 2]>,
}

impl RoofPrompts {
    #[allow(
        clippy::too_many_lines,
        reason = "one pass over the answers of the prompt machine"
    )]
    fn scan(state: &PromptState<'_>) -> (Scan, Step) {
        let preset = state.preset;
        let language = state
            .setting(LANGUAGE_SETTING)
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        let turkish = language.starts_with("tr");
        let separator = decimal_separator(&language);
        let unit = spatial::length_unit(state.snapshot);
        let slopes = spatial::project(state.snapshot)
            .map_or_else(Default::default, |p| slope_unit(&p.values));
        let number = |key: &str| preset.get(key).and_then(Json::as_f64);
        let mut scan = Scan {
            sketch: preset.get("mode").and_then(Json::as_str) == Some("sketch"),
            slope: number("slope").unwrap_or(DEFAULT_SLOPE),
            overhang: number("overhang").unwrap_or(DEFAULT_OVERHANG),
            base: number("base_offset"),
            points: Vec::new(),
        };
        let storey_name = preset
            .get("storey")
            .and_then(Json::as_str)
            .and_then(|s| ObjectId::from_str(s).ok())
            .and_then(|id| spatial::get(state.snapshot, id))
            .or_else(|| spatial::lowest_storey(state.snapshot))
            .and_then(|s| types::text(&s.values, 0).map(str::to_owned))
            .unwrap_or_default();
        let key = |turkish: bool, en: char, tr: char| if turkish { tr } else { en };
        let mut at = 0;
        loop {
            let count = scan.points.len();
            let ask = |scan: &Scan, id: String| {
                let message = match (scan.sketch, count) {
                    (false, _) => &PROMPT_PICK,
                    (true, 0) => &PROMPT_START,
                    (true, _) => &PROMPT_NEXT,
                };
                let mut options = vec![
                    KeywordOption::new("slope", M_AB_KEYWORD_ROOF_SLOPE.message_id)
                        .with_shortcut(key(turkish, 's', 'e')),
                    KeywordOption::new("overhang", M_AB_KEYWORD_ROOF_OVERHANG.message_id)
                        .with_shortcut(key(turkish, 'o', 't')),
                    KeywordOption::new("base", M_AB_KEYWORD_ROOF_BASE.message_id)
                        .with_shortcut(key(turkish, 'b', 'k')),
                ];
                if scan.sketch {
                    options.push(
                        KeywordOption::new("close", M_AB_KEYWORD_FLOOR_CLOSE.message_id)
                            .with_shortcut(key(turkish, 'c', 'p')),
                    );
                    options.push(
                        KeywordOption::new("walls", M_AB_KEYWORD_FLOOR_WALLS.message_id)
                            .with_shortcut(key(turkish, 'w', 'd')),
                    );
                } else {
                    options.push(
                        KeywordOption::new("sketch", M_AB_KEYWORD_FLOOR_SKETCH.message_id)
                            .with_shortcut(key(turkish, 'k', 'ç')),
                    );
                    options.push(
                        KeywordOption::new("done", M_AB_KEYWORD_FLOOR_DONE.message_id)
                            .with_shortcut(key(turkish, 'd', 'b')),
                    );
                }
                if count > 0 {
                    options.push(
                        KeywordOption::new("undo", M_AB_KEYWORD_FLOOR_UNDO.message_id)
                            .with_shortcut(key(turkish, 'u', 'g')),
                    );
                }
                Prompt::new(
                    id,
                    message.message_id,
                    Accept::Point {
                        dims: PointDims::Two,
                    },
                )
                .or(Accept::keywords(options))
                .with_arg("storey", storey_name.as_str())
                .with_arg("slope", slope_text(scan.slope, slopes, separator))
                .with_arg("overhang", length_text(scan.overhang, unit, separator))
                .with_arg(
                    "base",
                    scan.base
                        .map_or_else(String::new, |b| length_text(b, unit, separator)),
                )
                .with_arg("count", count.to_string())
            };
            let Some(answer) = state.answers.get(at) else {
                let prompt = ask(&scan, format!("point-{at}"));
                return (scan, Step::Ask(prompt));
            };
            at += 1;
            match &answer.input {
                PromptInput::Point(p) => scan.points.push([p.x, p.y]),
                PromptInput::Keyword(k) => match k.as_str() {
                    "sketch" => {
                        scan.sketch = true;
                        scan.points.clear();
                    }
                    "walls" => {
                        scan.sketch = false;
                        scan.points.clear();
                    }
                    "undo" => {
                        scan.points.pop();
                    }
                    "close" if scan.sketch && scan.points.len() >= 3 => break,
                    "done" if !scan.sketch && !scan.points.is_empty() => break,
                    "slope" | "overhang" | "base" => {
                        let (message, accept, now) = match k.as_str() {
                            "slope" => (
                                &PROMPT_SLOPE,
                                Accept::Text {
                                    min_chars: 1,
                                    max_chars: Some(32),
                                },
                                PromptInput::Text(slope_text(scan.slope, slopes, '.')),
                            ),
                            "overhang" => (
                                &PROMPT_OVERHANG,
                                Accept::Distance {
                                    min: Some(model::OVERHANG_RANGE.0),
                                    max: Some(model::OVERHANG_RANGE.1),
                                },
                                PromptInput::Distance(scan.overhang),
                            ),
                            _ => (
                                &PROMPT_BASE,
                                Accept::Distance {
                                    min: Some(types::BASE_OFFSET_RANGE.0),
                                    max: Some(types::BASE_OFFSET_RANGE.1),
                                },
                                PromptInput::Distance(scan.base.unwrap_or(0.0)),
                            ),
                        };
                        let prompt = Prompt::new(format!("value-{at}"), message.message_id, accept)
                            .with_default(now)
                            .with_arg("slope", slope_text(scan.slope, slopes, separator))
                            .with_arg("overhang", length_text(scan.overhang, unit, separator));
                        let Some(value) = state.answers.get(at) else {
                            return (scan, Step::Ask(prompt));
                        };
                        at += 1;
                        match (k.as_str(), &value.input) {
                            ("slope", PromptInput::Text(t)) => {
                                if let Ok(a) = parse_slope(state.snapshot, t) {
                                    scan.slope = a;
                                }
                            }
                            ("overhang", PromptInput::Distance(d)) => scan.overhang = *d,
                            ("base", PromptInput::Distance(d)) => scan.base = Some(*d),
                            _ => {}
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        let mut params = Map::new();
        let list = Json::from(scan.points.iter().map(|p| p.to_vec()).collect::<Vec<_>>());
        params.insert(if scan.sketch { "points" } else { "picks" }.into(), list);
        params.insert("slope".into(), Json::from(scan.slope));
        params.insert("overhang".into(), Json::from(scan.overhang));
        if let Some(base) = scan.base {
            params.insert("base_offset".into(), Json::from(base));
        }
        (scan, Step::Done(params))
    }
}

impl PromptMachine for RoofPrompts {
    fn next(&self, state: &PromptState<'_>) -> Result<Step> {
        Ok(Self::scan(state).1)
    }

    fn check(&self, state: &PromptState<'_>, prompt: &Prompt, input: &PromptInput) -> Result<()> {
        // A slope that cannot be read or is out of range is refused at the prompt, naming it.
        if let PromptInput::Text(t) = input
            && prompt.message.id == PROMPT_SLOPE.message_id
        {
            parse_slope(state.snapshot, t)?;
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
            ROOF_CREATE,
            M_AB_CMD_ROOF_CREATE_LABEL.message_id,
            M_AB_CMD_ROOF_CREATE_DESCRIPTION.message_id,
            |ctx: &mut Context<'_>, p: CreateRoof| create_roof(ctx, p),
        )
        .with_prompt(Arc::new(RoofPrompts)),
    )?;
    registry.register(CommandSpec::new(
        ROOF_SET_EDGE,
        M_AB_CMD_ROOF_SET_EDGE_LABEL.message_id,
        M_AB_CMD_ROOF_SET_EDGE_DESCRIPTION.message_id,
        |ctx: &mut Context<'_>, p: SetEdge| set_edge(ctx, p),
    ))?;
    registry.register(CommandSpec::new(
        ROOF_MOVE_VERTEX,
        M_AB_CMD_ROOF_MOVE_VERTEX_LABEL.message_id,
        M_AB_CMD_ROOF_MOVE_VERTEX_DESCRIPTION.message_id,
        |ctx: &mut Context<'_>, p: MoveVertex| move_vertex(ctx, p),
    ))?;
    registry.register(CommandSpec::new(
        ROOF_MOVE_EDGE,
        M_AB_CMD_ROOF_MOVE_EDGE_LABEL.message_id,
        M_AB_CMD_ROOF_MOVE_EDGE_DESCRIPTION.message_id,
        |ctx: &mut Context<'_>, p: MoveEdge| move_edge(ctx, p),
    ))?;
    registry.register(CommandSpec::new(
        WALL_ATTACH_TOP,
        M_AB_CMD_WALL_ATTACH_TOP_LABEL.message_id,
        M_AB_CMD_WALL_ATTACH_TOP_DESCRIPTION.message_id,
        |ctx: &mut Context<'_>, p: AttachTop| attach_top(ctx, p),
    ))?;
    registry.register(CommandSpec::new(
        WALL_DETACH_TOP,
        M_AB_CMD_WALL_DETACH_TOP_LABEL.message_id,
        M_AB_CMD_WALL_DETACH_TOP_DESCRIPTION.message_id,
        |ctx: &mut Context<'_>, p: DetachTop| detach_top(ctx, p),
    ))
}
