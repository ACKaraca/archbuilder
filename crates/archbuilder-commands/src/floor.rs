//! `archbuilder.floor.create` and `archbuilder.floor.add_hole`: floors (slabs) on a storey, and the
//! prompts that ask for them.
//!
//! A floor is made in one of three ways, by the parameter that is given: `points` (a sketched
//! outer boundary, with optional `holes`), `walls` (a closed chain of wall identifiers in order
//! around the area) or `picks` (points: each on a wall picks that wall; a single point inside a
//! closed loop of joined walls picks the smallest loop around it). Picked walls give associated
//! edges on the `face` asked for (the interior face by default: the face that looks into the
//! area), so the floor follows when a wall moves or changes thickness.
//!
//! `thickness` is 0.2 m and `offset` (the top's distance above the storey's level) is 0 by
//! default. One call is one transaction, one undo step. Deleting a wall that edges follow freezes
//! those edges at their last position ([`freeze_edges`], used by the wall delete).
//!
//! The prompt machine ([`FloorPrompts`]) asks for points. In the walls mode every point is a pick;
//! the keywords `thickness`, `offset`, `interior`, `exterior`, `sketch` or `walls` (to change the
//! mode) and `done` or `close` come first and return to the point.

use archbuilder_model::element_type::TypeKind;
use std::collections::BTreeSet;
use std::str::FromStr;
use std::sync::Arc;

use arc_base::{Error, ObjectId, Result};
use arc_command::prompt::{
    Accept, KeywordOption, Point, PointDims, Prompt, PromptInput, PromptMachine, PromptState, Step,
};
use arc_command::{CommandSpec, Context, IdParam, QuantityParam, Registry};
use arc_doc::{FieldPath, Snapshot};
use archbuilder_model::floor::{
    self, Derived, Face, Floor, MAX_LOOPS, MAX_VERTICES, Vertex, WallsRefusal,
};
use archbuilder_model::types::{
    self, BASE_OFFSET_RANGE, DEFAULT_FLOOR_THICKNESS, FLOOR, FLOOR_THICKNESS_RANGE, WALL,
};
use archbuilder_model::units::{decimal_separator, length_text};
use archbuilder_model::{defaults, names, spatial};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json};

use crate::error::codes;
use crate::structure::{LANGUAGE_SETTING, create, ensure_valid, expect, language, named};

/// The identifier of the create-floor command.
pub const FLOOR_CREATE: &str = "archbuilder.floor.create";
/// The identifier of the command that replaces the boundary of a floor.
pub const FLOOR_EDIT_BOUNDARY: &str = "archbuilder.floor.edit_boundary";
/// The identifier of the command that cuts a hole in a floor.
pub const FLOOR_ADD_HOLE: &str = "archbuilder.floor.add_hole";

/// A wall face as a command parameter.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FaceParam {
    /// The face that looks into the area the walls enclose.
    Interior,
    /// The face that looks away from it.
    Exterior,
    /// The center line of the wall (the core center while walls have no layers).
    Center,
    /// The face of the core that looks into the area the walls enclose.
    CoreInterior,
    /// The face of the core that looks away from it.
    CoreExterior,
}

impl From<FaceParam> for Face {
    fn from(face: FaceParam) -> Self {
        match face {
            FaceParam::Interior => Self::Interior,
            FaceParam::Exterior => Self::Exterior,
            FaceParam::Center => Self::Center,
            FaceParam::CoreInterior => Self::CoreInterior,
            FaceParam::CoreExterior => Self::CoreExterior,
        }
    }
}

/// Parameters of `archbuilder.floor.create`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateFloor {
    /// The storey; else the storey of the selection, else the lowest storey.
    #[serde(default)]
    pub storey: Option<IdParam>,
    /// A sketched outer boundary: points `[x, y]` in meters, in order, closed implicitly.
    #[serde(default)]
    pub points: Option<Vec<[f64; 2]>>,
    /// Holes for a sketched floor: each a list of points.
    #[serde(default)]
    pub holes: Option<Vec<Vec<[f64; 2]>>>,
    /// Walls of the storey in order around the area, each meeting the next.
    #[serde(default)]
    pub walls: Option<Vec<IdParam>>,
    /// Points that pick walls (see the module documentation).
    #[serde(default)]
    pub picks: Option<Vec<[f64; 2]>>,
    /// The face the edges of picked walls follow; the interior face when absent.
    #[serde(default)]
    pub face: Option<FaceParam>,
    /// The offset of the edges from the wall face, toward the left of the wall's direction, in
    /// meters; 0 when absent.
    #[serde(default)]
    pub edge_offset: Option<QuantityParam>,
    /// The floor type: its identifier or name; else the document's first floor type.
    #[serde(default, rename = "type")]
    #[schemars(length(max = 256))]
    pub floor_type: Option<String>,
    /// The thickness. A thickness other than the type's gives the floor a type of one layer of
    /// that thickness (an existing one, else a new one).
    #[serde(default)]
    pub thickness: Option<QuantityParam>,
    /// The top's distance above the storey's level; 0 when absent.
    #[serde(default)]
    pub offset: Option<QuantityParam>,
    /// The name; else the first free `Slab <n>` of the storey.
    #[serde(default)]
    pub name: Option<String>,
    /// A prompt aid: the mode the prompts start in (`walls` or `sketch`). The command ignores it.
    #[serde(default)]
    pub mode: Option<String>,
}

/// What `archbuilder.floor.create` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct FloorCreated {
    /// The new floor.
    pub floor: IdParam,
    /// Its storey.
    pub storey: IdParam,
    /// The area in square meters.
    pub area: f64,
    /// The volume of the slab in cubic meters.
    pub volume: f64,
    /// The thickness in meters.
    pub thickness: f64,
    /// The top offset in meters.
    pub offset: f64,
    /// How many walls the edges follow.
    pub walls: usize,
    /// The face of picked walls, when walls were picked.
    pub face: Option<FaceParam>,
}

/// One vertex of a sketched boundary, as a command parameter.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SketchVertex {
    /// The loop: 0 for the outline, 1 and up for the openings.
    pub loop_index: u32,
    /// The point `[x, y]` in meters.
    pub point: [f64; 2],
    /// The bulge of the edge that starts here: the tangent of a quarter of its included angle,
    /// positive for an arc that bends counterclockwise; absent or 0 for a straight edge.
    #[serde(default)]
    pub bulge: Option<f64>,
    /// The wall the edge that starts here follows; absent for a free edge.
    #[serde(default)]
    pub wall: Option<IdParam>,
    /// The face of that wall; the interior face when absent.
    #[serde(default)]
    pub face: Option<FaceParam>,
    /// The offset of the edge from that face, in meters; 0 when absent.
    #[serde(default)]
    pub offset: Option<f64>,
}

/// Parameters of `archbuilder.floor.edit_boundary`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditBoundary {
    /// The floor; else the selected floor.
    #[serde(default)]
    pub floor: Option<IdParam>,
    /// The new boundary: the vertices of all loops, loop by loop, as a sketch gives them.
    pub vertices: Vec<SketchVertex>,
}

/// What `archbuilder.floor.edit_boundary` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct BoundaryEdited {
    /// The floor.
    pub floor: IdParam,
    /// The area after the edit, in square meters.
    pub area: f64,
    /// How many edges that followed a wall are free now.
    pub freed: usize,
}

/// Parameters of `archbuilder.floor.add_hole`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddHole {
    /// The floor; else the selected floor.
    #[serde(default)]
    pub floor: Option<IdParam>,
    /// A prompt aid: the storey the prompts ran on. The command ignores it.
    #[serde(default)]
    pub storey: Option<IdParam>,
    /// The hole's points `[x, y]` in meters, in order, closed implicitly.
    pub points: Vec<[f64; 2]>,
}

/// What `archbuilder.floor.add_hole` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct HoleAdded {
    /// The floor.
    pub floor: IdParam,
    /// The floor's area after the hole, in square meters.
    pub area: f64,
}

fn too_big() -> Error {
    Error::new(codes::FLOOR_TOO_BIG)
        .with_arg("loops", MAX_LOOPS.to_string())
        .with_arg("points", MAX_VERTICES.to_string())
}

/// Writes the vertices of floor `id`, with their arcs, as one change.
///
/// # Errors
/// The transaction's errors.
pub(crate) fn set_vertices(ctx: &mut Context<'_>, id: ObjectId, vertices: &[Vertex]) -> Result<()> {
    let txn = ctx.txn()?;
    txn.modify(
        id,
        FieldPath::new("vertices"),
        Some(Floor::vertex_values(vertices)),
    )?;
    txn.modify(id, FieldPath::new("arcs"), Floor::arc_values(vertices))
}

/// The vertices of a free loop `points` as loop `index`.
fn free_loop(index: u32, points: &[[f64; 2]]) -> Vec<Vertex> {
    points
        .iter()
        .map(|p| Vertex {
            loop_index: index,
            point: *p,
            edge: None,
            bulge: 0.0,
        })
        .collect()
}

/// The storey a floor goes on: the given one, the selection's, else the lowest.
fn storey_for(ctx: &Context<'_>, given: Option<IdParam>) -> Result<ObjectId> {
    crate::wall::storey_for(ctx, given).map(|s| s.id)
}

pub(crate) fn next_floor_name(snapshot: &Snapshot, storey: ObjectId, language: &str) -> String {
    let used = spatial::children(snapshot, FLOOR, storey);
    let taken = |name: &str| {
        used.iter()
            .any(|f| types::text(&f.values, 0).is_some_and(|n| names::same(n, name)))
    };
    (used.len() + 1..)
        .map(|n| defaults::slab(language, n))
        .find(|name| !taken(name))
        .unwrap_or_default()
}

pub(crate) type WallChain = Vec<(ObjectId, archbuilder_model::wall::Wall)>;

/// The walls a floor over `p` follows: explicit walls, picks (a loop around a single point, with
/// the loops of the courtyards inside it as holes), or none for a sketch.
pub(crate) fn picked_walls(
    ctx: &Context<'_>,
    storey: ObjectId,
    p: &CreateFloor,
    rooms: bool,
) -> Result<Option<(WallChain, Vec<WallChain>)>> {
    // A ceiling is laid out like a room: the walls that bound rooms.
    let all = if rooms {
        floor::room_walls(ctx.snapshot(), storey)
    } else {
        floor::storey_walls(ctx.snapshot(), storey)
    };
    let by_id = |id: ObjectId| {
        all.iter()
            .find(|(w, _)| *w == id)
            .copied()
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH).with_arg("expected", WALL))
    };
    if let Some(list) = &p.walls {
        if list.len() > MAX_VERTICES {
            return Err(too_big());
        }
        // A hand-made chain names each wall once; only a derived region repeats a through wall.
        if list.iter().enumerate().any(|(k, a)| list[..k].contains(a)) {
            return Err(Error::new(codes::FLOOR_NO_LOOP));
        }
        return list
            .iter()
            .map(|IdParam(id)| by_id(*id))
            .collect::<Result<Vec<_>>>()
            .map(|chain| Some((chain, Vec::new())));
    }
    let Some(picks) = &p.picks else {
        return Ok(None);
    };
    if picks.len() > MAX_VERTICES {
        return Err(too_big());
    }
    let hits: Vec<Option<ObjectId>> = picks.iter().map(|q| floor::wall_at(&all, *q)).collect();
    let no_loop = || Error::new(codes::FLOOR_NO_LOOP);
    let (ids, holes): (Vec<ObjectId>, Vec<Vec<ObjectId>>) =
        match (picks.as_slice(), hits.as_slice()) {
            ([q], [None]) => {
                let region = floor::region_around(&all, *q).ok_or_else(no_loop)?;
                (region.walls.ok_or_else(no_loop)?, region.holes)
            }
            _ => {
                let ids = hits
                    .into_iter()
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(no_loop)?;
                if ids.iter().enumerate().any(|(k, a)| ids[..k].contains(a)) {
                    return Err(no_loop());
                }
                (ids, Vec::new())
            }
        };
    let chain = |ids: Vec<ObjectId>| ids.into_iter().map(by_id).collect::<Result<WallChain>>();
    let holes = holes.into_iter().map(chain).collect::<Result<Vec<_>>>()?;
    Ok(Some((chain(ids)?, holes)))
}

/// The vertices of the boundary `p` asks for: the loop of picked walls (with courtyards as holes)
/// at `face`, or the sketched points and holes.
///
/// # Errors
/// `ab.cmd.floor_no_loop`, `ab.cmd.floor_walls_parallel`, `ab.cmd.floor_too_big` and
/// `cmd.params_invalid` when neither or both of walls and points are given.
pub(crate) fn boundary_vertices(
    walls: &Option<(WallChain, Vec<WallChain>)>,
    p: &CreateFloor,
    face: Face,
    edge_offset: f64,
) -> Result<Vec<Vertex>> {
    let vertices: Vec<Vertex> = match (walls, &p.points) {
        (Some((walls, holes)), None) => {
            let refuse = |r| match r {
                WallsRefusal::NoLoop => Error::new(codes::FLOOR_NO_LOOP),
                WallsRefusal::Parallel => Error::new(codes::FLOOR_WALLS_PARALLEL),
            };
            let mut vertices =
                floor::vertices_of_walls(walls, face, edge_offset).map_err(refuse)?;
            // A courtyard's walls bound a hole on the side that looks away from them.
            let away = match face {
                Face::Interior => Face::Exterior,
                Face::Exterior => Face::Interior,
                Face::Center => Face::Center,
                Face::CoreInterior => Face::CoreExterior,
                Face::CoreExterior => Face::CoreInterior,
            };
            for (k, hole) in (1_u32..).zip(holes) {
                let mut loop_vertices =
                    floor::vertices_of_walls(hole, away, edge_offset).map_err(refuse)?;
                for v in &mut loop_vertices {
                    v.loop_index = k;
                }
                vertices.extend(loop_vertices);
            }
            vertices
        }
        (None, Some(points)) => {
            let holes = p.holes.as_deref().unwrap_or_default();
            let total = points.len() + holes.iter().map(Vec::len).sum::<usize>();
            if holes.len() >= MAX_LOOPS || total > MAX_VERTICES {
                return Err(too_big());
            }
            let mut vertices = free_loop(0, points);
            for (k, hole) in (1_u32..).zip(holes) {
                vertices.extend(free_loop(k, hole));
            }
            vertices
        }
        _ => return Err(Error::new(arc_command::error::codes::PARAMS_INVALID)),
    };
    Ok(vertices)
}

fn create_floor(ctx: &mut Context<'_>, p: CreateFloor) -> Result<FloorCreated> {
    let storey = storey_for(ctx, p.storey)?;
    let meters = |ctx: &Context<'_>, param: &Option<QuantityParam>, default: f64| match param {
        Some(param) => crate::length(ctx, param).map(|q| q.value()),
        None => Ok(default),
    };
    let floor_type = crate::element_type::layered_for(
        ctx,
        TypeKind::Floor,
        p.floor_type.as_deref(),
        p.thickness.as_ref(),
    )?;
    let offset = meters(ctx, &p.offset, 0.0)?;
    let edge_offset = meters(ctx, &p.edge_offset, 0.0)?;
    let face = p.face.map_or(Face::Interior, Into::into);
    let walls = picked_walls(ctx, storey, &p, false)?;
    let vertices = boundary_vertices(&walls, &p, face, edge_offset)?;
    let language = language(ctx);
    let name = match p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => name.to_owned(),
        None => next_floor_name(ctx.snapshot(), storey, &language),
    };
    let count = walls.as_ref().map_or(0, |(w, _)| w.len());
    let id = create(
        ctx,
        FLOOR,
        Some(storey),
        vec![
            Some(named(&name)),
            Some(types::length(DEFAULT_FLOOR_THICKNESS)),
            Some(types::length(offset)),
            Some(Floor::vertex_values(&vertices)),
            None,
        ],
    )?;
    crate::element_type::assign(ctx, id, TypeKind::Floor, floor_type)?;
    ensure_valid(ctx)?;
    let thickness = spatial::get(ctx.txn()?.snapshot(), id)
        .and_then(|f| types::meters(&f.values, types::floor::THICKNESS))
        .unwrap_or(DEFAULT_FLOOR_THICKNESS);
    let (area, volume) = {
        let txn = ctx.txn()?;
        spatial::get(txn.snapshot(), id)
            .and_then(|o| Derived::of(txn.snapshot(), &o))
            .map_or((0.0, 0.0), |d| (d.area(), d.volume()))
    };
    ctx.set_selection_after(vec![id]);
    Ok(FloorCreated {
        floor: IdParam(id),
        storey: IdParam(storey),
        area,
        volume,
        thickness,
        offset,
        walls: count,
        face: walls.as_ref().map(|_| match face {
            Face::Interior => FaceParam::Interior,
            Face::Exterior => FaceParam::Exterior,
            Face::Center => FaceParam::Center,
            Face::CoreInterior => FaceParam::CoreInterior,
            Face::CoreExterior => FaceParam::CoreExterior,
        }),
    })
}

/// Replaces the boundary of a floor in one change: the outline and the openings with their arcs
/// and the walls their edges follow. The new boundary is validated (closed loops that do not
/// cross, openings inside the outline, no edge shorter than the model tolerance) and a pinned
/// floor refuses, like every command; the whole edit is one undo step.
fn edit_boundary(ctx: &mut Context<'_>, p: EditBoundary) -> Result<BoundaryEdited> {
    let id = match p.floor {
        Some(IdParam(id)) => id,
        None => {
            let snapshot = ctx.snapshot();
            ctx.selection()
                .iter()
                .copied()
                .find(|id| {
                    spatial::type_of(snapshot, *id).is_some_and(|k| floor::slots_of(&k).is_some())
                })
                .ok_or_else(|| Error::new(codes::NO_FLOOR_SELECTED))?
        }
    };
    // A floor, a ceiling, a shaft or a vertical opening: each has a sketched boundary.
    let key =
        spatial::type_of(ctx.snapshot(), id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let slots = floor::slots_of(&key).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let object = expect(ctx.snapshot(), id, &key)?;
    let old = Floor::read(&object.values, slots).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let loops = p.vertices.last().map_or(0, |v| v.loop_index as usize + 1);
    if p.vertices.is_empty() || p.vertices.len() > MAX_VERTICES || loops > MAX_LOOPS {
        return Err(too_big());
    }
    let mut vertices = Vec::with_capacity(p.vertices.len());
    for v in &p.vertices {
        let edge = match v.wall {
            Some(IdParam(wall)) => {
                expect(ctx.snapshot(), wall, WALL)?;
                Some(floor::Association {
                    wall,
                    face: v.face.map_or(Face::Interior, Into::into),
                    offset: v.offset.unwrap_or(0.0),
                })
            }
            None => None,
        };
        let bulge = v.bulge.unwrap_or(0.0);
        if !bulge.is_finite() || bulge.abs() > floor::MAX_BULGE {
            return Err(Error::new(archbuilder_model::error::codes::FLOOR_MALFORMED));
        }
        vertices.push(Vertex {
            loop_index: v.loop_index,
            point: v.point,
            edge: edge.filter(|_| bulge.abs() < floor::MIN_BULGE),
            bulge,
        });
    }
    let freed = old
        .vertices
        .iter()
        .filter(|o| o.edge.is_some())
        .filter(|o| {
            !vertices
                .iter()
                .any(|n| n.edge.map(|a| a.wall) == o.edge.map(|a| a.wall))
        })
        .count();
    crate::floor::set_vertices(ctx, id, &vertices)?;
    ensure_valid(ctx)?;
    let txn = ctx.txn()?;
    let area = spatial::get(txn.snapshot(), id).map_or(0.0, |o| {
        if key == FLOOR {
            Derived::of(txn.snapshot(), &o).map_or(0.0, |d| d.area())
        } else {
            Floor::read(&o.values, slots)
                .and_then(|f| f.boundary_in(txn.snapshot()))
                .map_or(0.0, |b| b.area())
        }
    });
    ctx.set_selection_after(vec![id]);
    Ok(BoundaryEdited {
        floor: IdParam(id),
        area,
        freed,
    })
}

fn add_hole(ctx: &mut Context<'_>, p: AddHole) -> Result<HoleAdded> {
    let id = match p.floor {
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
    let object = expect(ctx.snapshot(), id, FLOOR)?;
    let floor =
        Floor::from_values(&object.values).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let loops = floor.vertices.last().map_or(0, |v| v.loop_index + 1);
    if p.points.len() + floor.vertices.len() > MAX_VERTICES || loops as usize >= MAX_LOOPS {
        return Err(too_big());
    }
    let mut vertices = floor.vertices;
    vertices.extend(free_loop(loops, &p.points));
    crate::floor::set_vertices(ctx, id, &vertices)?;
    ensure_valid(ctx)?;
    let txn = ctx.txn()?;
    let area = spatial::get(txn.snapshot(), id)
        .and_then(|o| Derived::of(txn.snapshot(), &o))
        .map_or(0.0, |d| d.area());
    Ok(HoleAdded {
        floor: IdParam(id),
        area,
    })
}

/// Freezes the edges of floors that follow the walls `deleted`, before those walls go: each such
/// edge keeps its derived position and loses its wall. Returns how many edges were frozen. The
/// caller has the transaction open on `ctx`.
///
/// # Errors
/// The transaction's errors.
pub fn freeze_edges(ctx: &mut Context<'_>, deleted: &BTreeSet<ObjectId>) -> Result<usize> {
    let snapshot = ctx.snapshot().clone();
    let mut frozen = 0;
    let mut changes: Vec<(ObjectId, Vec<Vertex>)> = Vec::new();
    // Floors, ceilings, shafts and vertical openings all have edges that can follow walls.
    for (key, slots) in [
        (FLOOR, floor::FLOOR_SLOTS),
        (types::CEILING, floor::FLOOR_SLOTS),
        (types::SHAFT, floor::SHAFT_SLOTS),
        (types::VERTICAL_OPENING, floor::VERTICAL_SLOTS),
    ] {
        for object in spatial::objects(&snapshot, key) {
            let Some(floor) = Floor::read(&object.values, slots) else {
                continue;
            };
            if !floor.walls().iter().any(|w| deleted.contains(w)) {
                continue;
            }
            let Some(boundary) = floor.boundary_in(&snapshot) else {
                continue;
            };
            if let Some(vertices) = floor.frozen(&boundary, deleted) {
                frozen += floor
                    .vertices
                    .iter()
                    .filter(|v| v.edge.is_some_and(|a| deleted.contains(&a.wall)))
                    .count();
                changes.push((object.id, vertices));
            }
        }
    }
    for (id, vertices) in changes {
        crate::floor::set_vertices(ctx, id, &vertices)?;
    }
    Ok(frozen)
}

/// A message of the catalog, named by identifier.
struct Msg {
    message_id: &'static str,
}

const PROMPT_PICK: Msg = Msg {
    message_id: "ab-prompt-floor-pick",
};
const PROMPT_START: Msg = Msg {
    message_id: "ab-prompt-floor-start",
};
const PROMPT_NEXT: Msg = Msg {
    message_id: "ab-prompt-floor-next",
};
const PROMPT_HOLE_START: Msg = Msg {
    message_id: "ab-prompt-floor-hole-start",
};
const PROMPT_HOLE_NEXT: Msg = Msg {
    message_id: "ab-prompt-floor-hole-next",
};
const PROMPT_CEILING_PICK: Msg = Msg {
    message_id: "ab-prompt-ceiling-pick",
};
const PROMPT_CEILING_START: Msg = Msg {
    message_id: "ab-prompt-ceiling-start",
};
const PROMPT_CEILING_NEXT: Msg = Msg {
    message_id: "ab-prompt-ceiling-next",
};
const PROMPT_SHAFT_START: Msg = Msg {
    message_id: "ab-prompt-shaft-start",
};
const PROMPT_SHAFT_NEXT: Msg = Msg {
    message_id: "ab-prompt-shaft-next",
};
const PROMPT_CUT_START: Msg = Msg {
    message_id: "ab-prompt-cut-start",
};
const PROMPT_CUT_NEXT: Msg = Msg {
    message_id: "ab-prompt-cut-next",
};
const PROMPT_ARC_MID: Msg = Msg {
    message_id: "ab-prompt-arc-mid",
};
const PROMPT_ARC_END: Msg = Msg {
    message_id: "ab-prompt-arc-end",
};
const KEY_ARC: Msg = Msg {
    message_id: "ab-keyword-arc",
};
const PROMPT_SHAFT_BASE: Msg = Msg {
    message_id: "ab-prompt-shaft-base",
};
const PROMPT_SHAFT_TOP: Msg = Msg {
    message_id: "ab-prompt-shaft-top",
};
const PROMPT_SHAFT_OFFSET: Msg = Msg {
    message_id: "ab-prompt-shaft-offset",
};
const KEY_SHAFT_BASE: Msg = Msg {
    message_id: "ab-keyword-shaft-base",
};
const KEY_SHAFT_TOP: Msg = Msg {
    message_id: "ab-keyword-shaft-top",
};
const KEY_SHAFT_OFFSET: Msg = Msg {
    message_id: "ab-keyword-shaft-offset",
};
const PROMPT_HEIGHT: Msg = Msg {
    message_id: "ab-prompt-ceiling-height",
};
const PROMPT_THICKNESS: Msg = Msg {
    message_id: "ab-prompt-floor-thickness",
};
const PROMPT_OFFSET: Msg = Msg {
    message_id: "ab-prompt-floor-offset",
};
const KEY_THICKNESS: Msg = Msg {
    message_id: "ab-keyword-floor-thickness",
};
const KEY_OFFSET: Msg = Msg {
    message_id: "ab-keyword-floor-offset",
};
const KEY_INTERIOR: Msg = Msg {
    message_id: "ab-keyword-floor-interior",
};
const KEY_EXTERIOR: Msg = Msg {
    message_id: "ab-keyword-floor-exterior",
};
const KEY_SKETCH: Msg = Msg {
    message_id: "ab-keyword-floor-sketch",
};
const KEY_WALLS: Msg = Msg {
    message_id: "ab-keyword-floor-walls",
};
const KEY_DONE: Msg = Msg {
    message_id: "ab-keyword-floor-done",
};
const KEY_UNDO: Msg = Msg {
    message_id: "ab-keyword-floor-undo",
};
const KEY_CLOSE: Msg = Msg {
    message_id: "ab-keyword-floor-close",
};

/// What the boundary prompts ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subject {
    /// A floor, or with `hole` a hole in one.
    Floor,
    /// A ceiling: picks inside a room or a sketch, a height instead of a top offset.
    Ceiling,
    /// A shaft: a sketched footprint only.
    Shaft,
    /// A vertical opening: a sketched footprint only, in the host of the preset `host`.
    Cut,
}

/// The prompts of `archbuilder.floor.create` (or, with `hole`, of `archbuilder.floor.add_hole`),
/// and, with another subject, of the ceiling, shaft and vertical opening commands, which ask the
/// same way.
#[derive(Debug)]
pub struct FloorPrompts {
    /// Whether the prompts ask for a hole.
    pub hole: bool,
    /// What the boundary is for.
    pub subject: Subject,
}

impl FloorPrompts {
    /// Prompts for a floor, or for a hole in one.
    #[must_use]
    pub const fn floor(hole: bool) -> Self {
        Self {
            hole,
            subject: Subject::Floor,
        }
    }

    /// Prompts for `subject`.
    #[must_use]
    pub const fn of(subject: Subject) -> Self {
        Self {
            hole: false,
            subject,
        }
    }

    /// Whether only a sketch with points is asked for: a hole, a shaft or a vertical opening.
    fn sketch_only(&self) -> bool {
        self.hole || matches!(self.subject, Subject::Shaft | Subject::Cut)
    }
}

/// What the prompts have read so far.
struct Scan {
    sketch: bool,
    face: &'static str,
    thickness: f64,
    /// Whether the thickness was answered; else the floor takes its type's.
    thickness_given: bool,
    offset: f64,
    points: Vec<[f64; 2]>,
    picks: Vec<[f64; 2]>,
    /// The arc edges of a sketch: the number of the vertex the edge starts at and its bulge.
    arcs: Vec<[f64; 2]>,
    /// A shaft's base and top level (names) and the top's offset.
    base: Option<String>,
    top: Option<String>,
    top_offset: f64,
}

fn shortcut(turkish: bool, english: char, tr: char) -> char {
    if turkish { tr } else { english }
}

impl FloorPrompts {
    #[allow(
        clippy::too_many_lines,
        reason = "one pass over the answers of the prompt machine"
    )]
    fn type_kind(&self) -> TypeKind {
        if self.subject == Subject::Ceiling {
            TypeKind::Ceiling
        } else {
            TypeKind::Floor
        }
    }

    #[allow(
        clippy::too_many_lines,
        clippy::cognitive_complexity,
        reason = "one pass over the answers of the prompt machine"
    )]
    fn scan(&self, state: &PromptState<'_>) -> (Scan, Step) {
        let preset = state.preset;
        let language = state
            .setting(LANGUAGE_SETTING)
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        let turkish = language.starts_with("tr");
        let unit = spatial::length_unit(state.snapshot);
        let text = |m| length_text(m, unit, decimal_separator(&language));
        let number = |key: &str| preset.get(key).and_then(Json::as_f64);
        let mut scan = Scan {
            sketch: self.sketch_only()
                || preset.get("mode").and_then(Json::as_str) == Some("sketch"),
            face: "interior",
            // The thickness of the floor's type (the preset one, else the default one).
            thickness: number("thickness").unwrap_or_else(|| {
                preset
                    .get("type")
                    .and_then(Json::as_str)
                    .and_then(|t| {
                        crate::element_type::resolve(state.snapshot, self.type_kind(), t).ok()
                    })
                    .or_else(|| crate::element_type::default_of(state.snapshot, self.type_kind()))
                    .and_then(|id| spatial::get(state.snapshot, id))
                    .and_then(|t| archbuilder_model::element_type::Layers::from_values(&t.values))
                    .map_or(DEFAULT_FLOOR_THICKNESS, |l| l.total())
            }),
            thickness_given: number("thickness").is_some(),
            offset: number("offset").unwrap_or(match self.subject {
                // A ceiling's height is the type's default.
                Subject::Ceiling => types::DEFAULT_CEILING_HEIGHT,
                _ => 0.0,
            }),
            points: Vec::new(),
            picks: Vec::new(),
            arcs: Vec::new(),
            base: None,
            top: None,
            top_offset: 0.0,
        };
        let storey_name = preset
            .get("storey")
            .and_then(Json::as_str)
            .and_then(|s| ObjectId::from_str(s).ok())
            .and_then(|id| spatial::get(state.snapshot, id))
            .or_else(|| spatial::lowest_storey(state.snapshot))
            .and_then(|s| types::text(&s.values, 0).map(str::to_owned))
            .unwrap_or_default();
        let floor_name = preset
            .get("floor")
            .and_then(Json::as_str)
            .and_then(|s| ObjectId::from_str(s).ok())
            .and_then(|id| spatial::get(state.snapshot, id))
            .or_else(|| {
                preset
                    .get("host")
                    .and_then(Json::as_str)
                    .and_then(|s| ObjectId::from_str(s).ok())
                    .and_then(|id| spatial::get(state.snapshot, id))
            })
            .and_then(|f| types::text(&f.values, 0).map(str::to_owned))
            .unwrap_or_default();
        let mut at = 0;
        loop {
            let ask = |scan: &Scan, key: String| {
                let count = if scan.sketch {
                    scan.points.len()
                } else {
                    scan.picks.len()
                };
                let message = match (self.hole, self.subject, scan.sketch, count) {
                    (true, _, _, 0) => &PROMPT_HOLE_START,
                    (true, _, _, _) => &PROMPT_HOLE_NEXT,
                    (_, Subject::Shaft, _, 0) => &PROMPT_SHAFT_START,
                    (_, Subject::Shaft, _, _) => &PROMPT_SHAFT_NEXT,
                    (_, Subject::Cut, _, 0) => &PROMPT_CUT_START,
                    (_, Subject::Cut, _, _) => &PROMPT_CUT_NEXT,
                    (_, Subject::Ceiling, false, _) => &PROMPT_CEILING_PICK,
                    (_, Subject::Ceiling, true, 0) => &PROMPT_CEILING_START,
                    (_, Subject::Ceiling, true, _) => &PROMPT_CEILING_NEXT,
                    (_, _, false, _) => &PROMPT_PICK,
                    (_, _, true, 0) => &PROMPT_START,
                    (_, _, true, _) => &PROMPT_NEXT,
                };
                let mut options = Vec::new();
                if !self.sketch_only() {
                    options.push(
                        KeywordOption::new("thickness", KEY_THICKNESS.message_id)
                            .with_shortcut(shortcut(turkish, 't', 'k')),
                    );
                    options.push(
                        KeywordOption::new("offset", KEY_OFFSET.message_id)
                            .with_shortcut(shortcut(turkish, 'o', 'o')),
                    );
                }
                if scan.sketch && self.subject != Subject::Floor && count > 0 {
                    options.push(
                        KeywordOption::new("arc", KEY_ARC.message_id)
                            .with_shortcut(shortcut(turkish, 'a', 'y')),
                    );
                }
                if self.subject == Subject::Shaft {
                    for (word, key, short) in [
                        ("base", &KEY_SHAFT_BASE, ('b', 'b')),
                        ("top", &KEY_SHAFT_TOP, ('t', 't')),
                        ("offset", &KEY_SHAFT_OFFSET, ('o', 'o')),
                    ] {
                        options.push(
                            KeywordOption::new(word, key.message_id)
                                .with_shortcut(shortcut(turkish, short.0, short.1)),
                        );
                    }
                }
                if scan.sketch {
                    options.push(
                        KeywordOption::new("close", KEY_CLOSE.message_id)
                            .with_shortcut(shortcut(turkish, 'c', 'p')),
                    );
                    if !self.sketch_only() {
                        options.push(
                            KeywordOption::new("walls", KEY_WALLS.message_id)
                                .with_shortcut(shortcut(turkish, 'w', 'd')),
                        );
                    }
                } else {
                    options.push(
                        KeywordOption::new("interior", KEY_INTERIOR.message_id)
                            .with_shortcut(shortcut(turkish, 'i', 'i')),
                    );
                    options.push(
                        KeywordOption::new("exterior", KEY_EXTERIOR.message_id)
                            .with_shortcut(shortcut(turkish, 'e', 'd')),
                    );
                    options.push(
                        KeywordOption::new("sketch", KEY_SKETCH.message_id)
                            .with_shortcut(shortcut(turkish, 's', 'ç')),
                    );
                    options.push(
                        KeywordOption::new("done", KEY_DONE.message_id)
                            .with_shortcut(shortcut(turkish, 'd', 'b')),
                    );
                }
                if count > 0 {
                    options.push(
                        KeywordOption::new("undo", KEY_UNDO.message_id)
                            .with_shortcut(shortcut(turkish, 'u', 'g')),
                    );
                }
                Prompt::new(
                    key,
                    message.message_id,
                    Accept::Point {
                        dims: PointDims::Two,
                    },
                )
                .or(Accept::keywords(options))
                .with_arg("storey", storey_name.as_str())
                .with_arg("floor", floor_name.as_str())
                .with_arg("thickness", text(scan.thickness))
                .with_arg("offset", text(scan.offset))
                .with_arg("face", scan.face)
                .with_arg("count", count.to_string())
                .with_arg("mode", if scan.sketch { "sketch" } else { "walls" })
            };
            let Some(answer) = state.answers.get(at) else {
                let prompt = ask(&scan, format!("point-{at}"));
                return (scan, Step::Ask(prompt));
            };
            at += 1;
            match &answer.input {
                PromptInput::Point(p) if scan.sketch => scan.points.push([p.x, p.y]),
                PromptInput::Point(p) => scan.picks.push([p.x, p.y]),
                PromptInput::Keyword(k) => match k.as_str() {
                    "interior" => scan.face = "interior",
                    "exterior" => scan.face = "exterior",
                    "sketch" => {
                        scan.sketch = true;
                        scan.picks.clear();
                    }
                    "walls" => {
                        scan.sketch = false;
                        scan.points.clear();
                    }
                    "undo" => {
                        // The last point of the sketch, or the last wall picked.
                        if scan.sketch {
                            scan.points.pop();
                            let n = scan.points.len() as f64;
                            scan.arcs.retain(|a| a[0] < n - 1.0);
                        } else {
                            scan.picks.pop();
                        }
                    }
                    "close" if scan.points.len() >= 3 => break,
                    "done" if !scan.picks.is_empty() => break,
                    "arc" if scan.sketch && !scan.points.is_empty() => {
                        // A three-point arc from the last point: a point on it, then its end.
                        let mut got = Vec::new();
                        for (n, message) in [(0, &PROMPT_ARC_MID), (1, &PROMPT_ARC_END)] {
                            let prompt = Prompt::new(
                                format!("value-{at}-{n}"),
                                message.message_id,
                                Accept::Point {
                                    dims: PointDims::Two,
                                },
                            );
                            let Some(value) = state.answers.get(at) else {
                                return (scan, Step::Ask(prompt));
                            };
                            at += 1;
                            if let PromptInput::Point(p) = &value.input {
                                got.push([p.x, p.y]);
                            }
                        }
                        if let (Some(&start), [mid, end]) = (scan.points.last(), got.as_slice())
                            && let Some(bulge) =
                                archbuilder_model::sketch::bulge_through(start, *mid, *end)
                        {
                            scan.arcs.push([(scan.points.len() - 1) as f64, bulge]);
                            scan.points.push(*end);
                        }
                    }
                    "base" | "top" => {
                        let prompt = Prompt::new(
                            format!("value-{at}"),
                            if k == "base" {
                                PROMPT_SHAFT_BASE.message_id
                            } else {
                                PROMPT_SHAFT_TOP.message_id
                            },
                            Accept::Text {
                                min_chars: 1,
                                max_chars: Some(64),
                            },
                        );
                        let Some(value) = state.answers.get(at) else {
                            return (scan, Step::Ask(prompt));
                        };
                        at += 1;
                        if let PromptInput::Text(t) = &value.input {
                            if k == "base" {
                                scan.base = Some(t.clone());
                            } else {
                                scan.top = Some(t.clone());
                            }
                        }
                    }
                    "offset" if self.subject == Subject::Shaft => {
                        let prompt = Prompt::new(
                            format!("value-{at}"),
                            PROMPT_SHAFT_OFFSET.message_id,
                            Accept::Distance {
                                min: Some(BASE_OFFSET_RANGE.0),
                                max: Some(BASE_OFFSET_RANGE.1),
                            },
                        )
                        .with_default(PromptInput::Distance(scan.top_offset));
                        let Some(value) = state.answers.get(at) else {
                            return (scan, Step::Ask(prompt));
                        };
                        at += 1;
                        if let PromptInput::Distance(d) = value.input {
                            scan.top_offset = d;
                        }
                    }
                    "thickness" | "offset" => {
                        let (message, range, now) = if k == "thickness" {
                            (&PROMPT_THICKNESS, FLOOR_THICKNESS_RANGE, scan.thickness)
                        } else if self.subject == Subject::Ceiling {
                            (&PROMPT_HEIGHT, BASE_OFFSET_RANGE, scan.offset)
                        } else {
                            (&PROMPT_OFFSET, BASE_OFFSET_RANGE, scan.offset)
                        };
                        let prompt = Prompt::new(
                            format!("value-{at}"),
                            message.message_id,
                            Accept::Distance {
                                min: Some(range.0),
                                max: Some(range.1),
                            },
                        )
                        .with_default(PromptInput::Distance(now))
                        .with_arg("thickness", text(scan.thickness))
                        .with_arg("offset", text(scan.offset));
                        let Some(value) = state.answers.get(at) else {
                            return (scan, Step::Ask(prompt));
                        };
                        at += 1;
                        if let PromptInput::Distance(d) = value.input {
                            if k == "thickness" {
                                scan.thickness = d;
                                scan.thickness_given = true;
                            } else {
                                scan.offset = d;
                            }
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        let mut params = Map::new();
        if !scan.arcs.is_empty() {
            params.insert("arcs".into(), points_json(&scan.arcs));
        }
        if self.subject == Subject::Shaft {
            if let Some(b) = &scan.base {
                params.insert("base_level".into(), Json::from(b.as_str()));
            }
            if let Some(t) = &scan.top {
                params.insert("top_level".into(), Json::from(t.as_str()));
            }
            if scan.top_offset != 0.0 {
                params.insert("top_offset".into(), Json::from(scan.top_offset));
            }
        }
        if self.sketch_only() {
            params.insert("points".into(), points_json(&scan.points));
        } else {
            if scan.thickness_given {
                params.insert("thickness".into(), Json::from(scan.thickness));
            }
            params.insert("offset".into(), Json::from(scan.offset));
            if scan.sketch {
                params.insert("points".into(), points_json(&scan.points));
            } else {
                params.insert("picks".into(), points_json(&scan.picks));
                params.insert("face".into(), Json::from(scan.face));
            }
        }
        (scan, Step::Done(params))
    }
}

fn points_json(points: &[[f64; 2]]) -> Json {
    Json::from(points.iter().map(|p| p.to_vec()).collect::<Vec<_>>())
}

impl PromptMachine for FloorPrompts {
    fn next(&self, state: &PromptState<'_>) -> Result<Step> {
        Ok(self.scan(state).1)
    }

    fn check(&self, state: &PromptState<'_>, _: &Prompt, input: &PromptInput) -> Result<()> {
        // A point that repeats the last one of a sketch makes an edge of no length.
        if let (PromptInput::Point(Point { x, y, .. }), (scan, _)) = (input, self.scan(state))
            && scan.sketch
            && scan
                .points
                .last()
                .is_some_and(|l| (x - l[0]).hypot(y - l[1]) < floor::MIN_EDGE)
        {
            return Err(
                Error::new(archbuilder_model::error::codes::FLOOR_EDGE_SHORT).with_arg("name", ""),
            );
        }
        Ok(())
    }
}

/// Registers the commands of this module.
///
/// # Errors
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    const LABEL: Msg = Msg {
        message_id: "ab-cmd-floor-create-label",
    };
    const DESCRIPTION: Msg = Msg {
        message_id: "ab-cmd-floor-create-description",
    };
    const HOLE_LABEL: Msg = Msg {
        message_id: "ab-cmd-floor-add-hole-label",
    };
    const HOLE_DESCRIPTION: Msg = Msg {
        message_id: "ab-cmd-floor-add-hole-description",
    };
    registry.register(
        CommandSpec::new(
            FLOOR_CREATE,
            LABEL.message_id,
            DESCRIPTION.message_id,
            |ctx: &mut Context<'_>, p: CreateFloor| create_floor(ctx, p),
        )
        .with_prompt(Arc::new(FloorPrompts::floor(false))),
    )?;
    const EDIT_LABEL: Msg = Msg {
        message_id: "ab-cmd-floor-edit-boundary-label",
    };
    const EDIT_DESCRIPTION: Msg = Msg {
        message_id: "ab-cmd-floor-edit-boundary-description",
    };
    registry.register(CommandSpec::new(
        FLOOR_EDIT_BOUNDARY,
        EDIT_LABEL.message_id,
        EDIT_DESCRIPTION.message_id,
        |ctx: &mut Context<'_>, p: EditBoundary| edit_boundary(ctx, p),
    ))?;
    registry.register(
        CommandSpec::new(
            FLOOR_ADD_HOLE,
            HOLE_LABEL.message_id,
            HOLE_DESCRIPTION.message_id,
            |ctx: &mut Context<'_>, p: AddHole| add_hole(ctx, p),
        )
        .with_prompt(Arc::new(FloorPrompts::floor(true))),
    )
}
