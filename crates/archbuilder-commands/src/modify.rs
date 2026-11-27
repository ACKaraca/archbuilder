//! The modify commands: `archbuilder.modify.move`, `copy`, `rotate` and `mirror`, each one
//! transaction and so one undo step, on the objects named or else on the selection.
//!
//! What moves: walls (their baselines), floors (their sketched points and holes), rooms (their
//! points), stairs (their start and direction; a mirrored stair turns the other way) and, with
//! their host wall, doors and windows. Storeys and levels are not movable and are skipped in a selection;
//! a command that names only them is refused (`ab.cmd.not_movable`).
//!
//! - A wall keeps its identifier; its joins are derived, so they re-derive from the new baselines
//!   (the walls that met it stay where they are and no longer meet it). A door or window follows
//!   its host: the distance along the baseline does not change, so it keeps its place in the wall.
//! - A reflection reverses handedness. A mirrored wall is stored with its baseline reversed (and
//!   its two join styles swapped), so its exterior face stays the exterior face; its doors and
//!   windows keep their facing, take the distance from the other end and swap hand.
//! - A floor follows the walls its edges are associated with. When a floor moves, an edge stays
//!   associated only if its wall moves with it (or, in a copy, is copied with it, the edge then
//!   following the new wall); every other edge is dissociated and stays where the motion puts
//!   it. The result reports how many edges were dissociated. Refusing instead would make moving
//!   a slab on its own impossible.
//! - A door or window moved alone slides along its host wall: the displacement is projected onto
//!   the baseline, and the move is refused when the opening would leave the clear span or meet
//!   another opening. Rotating or mirroring an insert alone is refused (`ab.cmd.insert_alone`):
//!   it is turned by turning its wall.
//! - A copy creates new walls, floors and inserts with new identifiers and the next free names;
//!   copying a wall copies its doors and windows. Copied inserts of a wall that is not copied
//!   (an insert alone) go to the same wall at the projected distance.
//!
//! - Paste (`archbuilder.clipboard.paste`, `archbuilder.clipboard.paste_to_storey`) is a copy of the
//!   named objects (the host's clipboard) by the displacement from the clipboard's base point to the
//!   insertion point, or, with `storey`, into another storey at the same plan position. Elevations
//!   belong to the storey, so they follow it. A door or window alone cannot go to another storey
//!   (`ab.cmd.not_movable`).
//!
//! The prompt machine ([`ModifyPrompts`]) asks for the base point and the second point (move and
//! copy; the keyword `displacement` takes the displacement alone), the base point and the angle
//! (rotate, with the keyword `reference`), the two points of the mirror line and whether to erase
//! the originals (mirror). Objects are not asked for here: the host gathers them first and
//! passes them as the preset `objects`, else the selection is used.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arc_base::{Dimension, Error, ObjectId, Result};
use arc_command::prompt::{
    Accept, KeywordOption, Point, PointDims, Prompt, PromptInput, PromptMachine, PromptState, Step,
};
use arc_command::{CommandSpec, Context, IdParam, QuantityParam, Registry};
use arc_doc::{FieldPath, Snapshot, Value};
use archbuilder_model::floor::{Association, Boundary, Floor, Vertex};
use archbuilder_model::insert::{self, Insert, Kind};
use archbuilder_model::spatial;
use archbuilder_model::stair::Stair;
use archbuilder_model::transform::Xform;
use archbuilder_model::types::{self, DOOR, FLOOR, WALL, WINDOW};
use archbuilder_model::wall::{JoinStyle, Wall};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json};

use crate::error::codes;
use crate::structure::{create, language};

/// The identifier of the move command.
pub const MODIFY_MOVE: &str = "archbuilder.modify.move";
/// The identifier of the copy command.
pub const MODIFY_COPY: &str = "archbuilder.modify.copy";
/// The identifier of the rotate command.
pub const MODIFY_ROTATE: &str = "archbuilder.modify.rotate";
/// The identifier of the mirror command.
pub const MODIFY_MIRROR: &str = "archbuilder.modify.mirror";

/// The identifier of the paste command: a copy of the clipboard objects by a displacement.
pub const CLIPBOARD_PASTE: &str = "archbuilder.clipboard.paste";
/// The identifier of the command that pastes the clipboard objects into another storey at the
/// same plan position.
pub const CLIPBOARD_PASTE_TO_STOREY: &str = "archbuilder.clipboard.paste_to_storey";

/// The most objects one modify command takes.
pub const MAX_OBJECTS: usize = 5_000;

/// Parameters of `archbuilder.modify.move` and `archbuilder.modify.copy`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Displace {
    /// The objects; else the selection.
    #[serde(default)]
    pub objects: Option<Vec<IdParam>>,
    /// The displacement `[dx, dy]` in meters.
    pub displacement: [f64; 2],
    /// A prompt aid: the base point the copy prompts measure from. The command ignores it.
    #[serde(default)]
    pub base: Option<[f64; 2]>,
}

/// Parameters of `archbuilder.clipboard.paste` and `archbuilder.clipboard.paste_to_storey`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Paste {
    /// The objects to paste; else the selection.
    #[serde(default)]
    pub objects: Option<Vec<IdParam>>,
    /// The displacement `[dx, dy]` in meters; `[0, 0]` (paste in place) when absent.
    #[serde(default)]
    pub displacement: Option<[f64; 2]>,
    /// A prompt aid: the base point of the clipboard. The command ignores it.
    #[serde(default)]
    pub base: Option<[f64; 2]>,
    /// The storey to paste into, at the same plan position; required by `paste_to_storey`.
    #[serde(default)]
    pub storey: Option<IdParam>,
    /// Clipboard content made by [`crate::clip::capture`]: when given, new objects are created from
    /// it (in `storey`, which is then required) and `objects` is not read.
    #[serde(default)]
    pub content: Option<Json>,
}

/// Parameters of `archbuilder.modify.rotate`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Rotate {
    /// The objects; else the selection.
    #[serde(default)]
    pub objects: Option<Vec<IdParam>>,
    /// The center `[x, y]` in meters.
    pub base: [f64; 2],
    /// The counterclockwise angle: a JSON number is radians, a text with a unit (`"90 deg"`) says
    /// its own, and a text that is a bare number (`"90"`) is degrees.
    pub angle: QuantityParam,
}

/// Parameters of `archbuilder.modify.mirror`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Mirror {
    /// The objects; else the selection.
    #[serde(default)]
    pub objects: Option<Vec<IdParam>>,
    /// The two points `[x, y]` of the mirror line, in meters.
    pub axis: [[f64; 2]; 2],
    /// Whether the originals stay (a mirrored copy); true when absent.
    #[serde(default)]
    pub keep: Option<bool>,
}

/// What a modify command returns.
#[derive(Debug, Default, Serialize, JsonSchema)]
pub struct Modified {
    /// The walls, floors and doors and windows the command worked on, hosted inserts included.
    pub objects: usize,
    /// The new objects of a copy, in creation order; the selection after the command.
    pub created: Vec<IdParam>,
    /// The floor edges that lost their wall association.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dissociated_edges: Option<usize>,
    /// The wall ends of walls outside the selection that were moved to keep a join, and the ends
    /// of moved walls that slid along a wall they stood on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reconnected: Option<usize>,
    /// The walls whose join with a moved wall could not be kept and is now open.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unjoined: Vec<String>,
    /// Doors and windows that now open into another wall, as `[door or window, wall]`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub meets: Vec<[String; 2]>,
}

/// The movable objects a command works on.
#[derive(Debug, Default)]
pub(crate) struct Set {
    /// The walls.
    pub(crate) walls: Vec<ObjectId>,
    /// The floors.
    pub(crate) floors: Vec<ObjectId>,
    /// The rooms (their placement points move).
    pub(crate) rooms: Vec<ObjectId>,
    /// The stairs (their start and direction move).
    pub(crate) stairs: Vec<ObjectId>,
    /// The roofs (their sketched footprint points move).
    pub(crate) roofs: Vec<ObjectId>,
    /// The doors, windows and wall openings whose wall is not in the set.
    pub(crate) inserts: Vec<ObjectId>,
    /// The ceilings (their boundary, slope arrow and grid origin move).
    pub(crate) ceilings: Vec<ObjectId>,
    /// The shafts (their footprint moves).
    pub(crate) shafts: Vec<ObjectId>,
    /// The vertical openings, and those of the floors, roofs and ceilings in the set.
    pub(crate) cuts: Vec<ObjectId>,
}

impl Set {
    fn len(&self) -> usize {
        self.walls.len()
            + self.floors.len()
            + self.rooms.len()
            + self.stairs.len()
            + self.roofs.len()
            + self.inserts.len()
            + self.ceilings.len()
            + self.shafts.len()
            + self.cuts.len()
    }
}

pub(crate) fn invalid() -> Error {
    Error::new(arc_command::error::codes::PARAMS_INVALID)
}

/// The objects a command works on: `given`, else the selection; each once, sorted into kinds.
pub(crate) fn gather(ctx: &Context<'_>, given: Option<Vec<IdParam>>) -> Result<Set> {
    let explicit = given.is_some();
    let ids: Vec<ObjectId> = match given {
        Some(list) if list.len() > MAX_OBJECTS => return Err(invalid()),
        Some(list) => list.into_iter().map(|IdParam(id)| id).collect(),
        None => ctx
            .selection()
            .iter()
            .take(MAX_OBJECTS + 1)
            .copied()
            .collect(),
    };
    if ids.len() > MAX_OBJECTS {
        return Err(invalid());
    }
    let snapshot = ctx.snapshot();
    let mut set = Set::default();
    let mut skipped = false;
    let mut seen = BTreeSet::new();
    let mut inserts = Vec::new();
    for id in ids {
        if !seen.insert(id) {
            continue;
        }
        match spatial::type_of(snapshot, id).as_deref() {
            Some(WALL) => set.walls.push(id),
            Some(FLOOR) => set.floors.push(id),
            Some(types::ROOM) => set.rooms.push(id),
            Some(types::STAIR) => set.stairs.push(id),
            Some(types::ROOF) => set.roofs.push(id),
            Some(types::CEILING) => set.ceilings.push(id),
            Some(types::SHAFT) => set.shafts.push(id),
            Some(types::VERTICAL_OPENING) => set.cuts.push(id),
            Some(DOOR | WINDOW | types::WALL_OPENING) => inserts.push(id),
            // Storeys and levels are skipped in a selection; naming one is an error.
            Some(_) if !explicit => skipped = true,
            _ => return Err(Error::new(codes::NOT_MOVABLE)),
        }
    }
    for id in inserts {
        let host = spatial::get(snapshot, id).and_then(|o| o.header.owner);
        if !host.is_some_and(|w| set.walls.contains(&w)) {
            set.inserts.push(id);
        }
    }
    // The vertical openings of a floor, roof or ceiling go where it goes.
    for host in set.floors.iter().chain(&set.roofs).chain(&set.ceilings) {
        for cut in spatial::children(snapshot, types::VERTICAL_OPENING, *host) {
            if !set.cuts.contains(&cut.id) {
                set.cuts.push(cut.id);
            }
        }
    }
    if set.len() == 0 {
        return Err(Error::new(if skipped {
            codes::NOT_MOVABLE
        } else {
            codes::NOTHING_SELECTED
        }));
    }
    Ok(set)
}

/// The motion of a translation, for an insert alone: the displacement projected onto its wall.
fn slide(snapshot: &Snapshot, id: ObjectId, xf: &Xform) -> Result<(Insert, f64)> {
    let object = spatial::get(snapshot, id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let item = Insert::of(snapshot, &object).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let Xform::Translate(d) = *xf else {
        return Err(Error::new(codes::INSERT_ALONE));
    };
    let wall = object
        .header
        .owner
        .and_then(|w| spatial::get(snapshot, w))
        .and_then(|w| Wall::from_values(&w.values))
        .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let length = wall.length();
    let along = if length > 0.0 {
        ((wall.end[0] - wall.start[0]) * d[0] + (wall.end[1] - wall.start[1]) * d[1]) / length
    } else {
        0.0
    };
    Ok((item, item.distance + along))
}

/// The vertices of `floor` under `xf`. The authored points are replaced by the boundary as
/// derived (`boundary`), so a vertex of an edge that follows a wall keeps where it is. An edge
/// stays associated only to the wall `keep` returns for its wall; the others are dissociated.
/// Returns the vertices and the number of edges dissociated.
pub(crate) fn floor_vertices(
    floor: &Floor,
    boundary: Option<&Boundary>,
    xf: &Xform,
    keep: &dyn Fn(ObjectId) -> Option<ObjectId>,
) -> (Vec<Vertex>, usize) {
    let mut lost = 0;
    let mut out = Vec::with_capacity(floor.vertices.len());
    let (mut current, mut k) = (0_u32, 0_usize);
    for v in &floor.vertices {
        if v.loop_index != current {
            current = v.loop_index;
            k = 0;
        }
        let at = boundary
            .and_then(|b| b.loops.get(v.loop_index as usize))
            .and_then(|l| l.get(k))
            .copied()
            .unwrap_or(v.point);
        k += 1;
        let edge = v.edge.and_then(|a| {
            let kept = keep(a.wall);
            if kept.is_none() {
                lost += 1;
            }
            kept.map(|wall| Association { wall, ..a })
        });
        out.push(Vertex {
            loop_index: v.loop_index,
            point: xf.point(at),
            edge,
            // A reflection turns an arc the other way round.
            bulge: if xf.reflects() { -v.bulge } else { v.bulge },
        });
    }
    (out, lost)
}

/// The wall `wall` under `xf`: its baseline moved, reversed under a reflection (the join styles
/// swap with the ends).
pub(crate) fn moved_wall(wall: &Wall, xf: &Xform) -> Wall {
    let (a, b) = (xf.point(wall.start), xf.point(wall.end));
    if xf.reflects() {
        Wall {
            start: b,
            end: a,
            start_join: wall.end_join,
            end_join: wall.start_join,
            ..*wall
        }
    } else {
        Wall {
            start: a,
            end: b,
            ..*wall
        }
    }
}

/// The distance and hand of an insert of a wall under `xf`.
fn moved_insert(item: &Insert, length: f64, xf: &Xform) -> (f64, u32) {
    if xf.reflects() {
        (
            (length - item.distance).max(0.0),
            item.hand.flipped().code(),
        )
    } else {
        (item.distance, item.hand.code())
    }
}

pub(crate) fn set_length(
    ctx: &mut Context<'_>,
    id: ObjectId,
    field: &str,
    value: f64,
) -> Result<()> {
    ctx.txn()?
        .modify(id, FieldPath::new(field), Some(types::length(value)))
}

/// How far past its own length a neighbor may be stretched to meet a moved wall, as a multiple
/// of the neighbor's length; a corner farther away is not made.
const MAX_STRETCH: f64 = 100.0;

/// One end of a baseline: 0 the start, 1 the end.
fn end_of(wall: &Wall, end: usize) -> [f64; 2] {
    if end == 0 { wall.start } else { wall.end }
}

fn with_end(wall: &Wall, end: usize, to: [f64; 2]) -> Wall {
    if end == 0 {
        Wall { start: to, ..*wall }
    } else {
        Wall { end: to, ..*wall }
    }
}

fn gap(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// The distance along the baseline of `wall` from `old.start` to `new.start`, for a start that
/// moved along the line.
pub(crate) fn start_shift(old: &Wall, new: &Wall) -> f64 {
    let length = old.length();
    if length <= 0.0 {
        return 0.0;
    }
    ((new.start[0] - old.start[0]) * (old.end[0] - old.start[0])
        + (new.start[1] - old.start[1]) * (old.end[1] - old.start[1]))
        / length
}

/// What keeping the joins of the moved walls does: the new baselines of the walls of the set
/// (a wall whose end sat on another wall slides along it), the neighbors that follow, and the
/// names of the walls that could not be reconnected.
struct Reconnected {
    moved: Vec<(ObjectId, Wall, Wall)>,
    neighbors: Vec<(ObjectId, Wall, Wall)>,
    ends: usize,
    unjoined: Vec<String>,
}

/// Joins that a moved wall had with walls outside the set are kept: a neighbor whose end met the
/// moved wall (at its end, or on its baseline) stretches or shrinks along its own line to the
/// moved wall's new line; a moved wall whose end sat on a neighbor's baseline slides along that
/// neighbor. A join that cannot be kept (parallel lines, a corner farther than
/// [`MAX_STRETCH`] lengths, a wall left with no length) is left open and named in the result.
#[allow(
    clippy::too_many_lines,
    reason = "the two join cases read the same lists"
)]
fn reconnect(snapshot: &Snapshot, set: &Set, xf: &Xform) -> Result<Reconnected> {
    let in_set: BTreeSet<ObjectId> = set.walls.iter().copied().collect();
    let tol = archbuilder_model::join::JOIN_TOLERANCE;
    let name_of = |id: ObjectId| {
        spatial::get(snapshot, id)
            .and_then(|o| types::text(&o.values, 0).map(str::to_owned))
            .unwrap_or_default()
    };
    let mut moved = Vec::new();
    let mut storeys = BTreeSet::new();
    for id in &set.walls {
        let object = spatial::get(snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let old =
            Wall::from_values(&object.values).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        storeys.extend(object.header.owner);
        moved.push((*id, old, moved_wall(&old, xf)));
    }
    let mut neighbors: Vec<(ObjectId, Wall, Wall)> = Vec::new();
    for storey in storeys {
        for object in spatial::walls(snapshot, storey) {
            if !in_set.contains(&object.id)
                && let Some(w) = Wall::from_values(&object.values)
            {
                neighbors.push((object.id, w, w));
            }
        }
    }
    let mut out = Reconnected {
        moved: Vec::new(),
        neighbors: Vec::new(),
        ends: 0,
        unjoined: Vec::new(),
    };
    let mut claimed: BTreeSet<(ObjectId, usize)> = BTreeSet::new();
    let mut moved_claimed: BTreeSet<(ObjectId, usize)> = BTreeSet::new();
    let note = |out: &mut Reconnected, id: ObjectId| {
        let name = name_of(id);
        if !out.unjoined.contains(&name) {
            out.unjoined.push(name);
        }
    };
    for (id, old, new) in &mut moved {
        let dir = [new.end[0] - new.start[0], new.end[1] - new.start[1]];
        // A neighbor whose end met the old wall follows the new line.
        for (nid, nold, nnew) in &mut neighbors {
            for end in 0..2 {
                let point = end_of(nold, end);
                let style = if end == 0 {
                    nold.start_join
                } else {
                    nold.end_join
                };
                if claimed.contains(&(*nid, end))
                    || style == JoinStyle::Disallowed
                    || archbuilder_model::join::baseline_distance(old, point) > tol
                {
                    continue;
                }
                let hit = archbuilder_model::join::line_hit(nnew, new.start, dir);
                let length = nold.length();
                let kept = hit
                    .filter(|(q, _)| gap(*q, point) <= MAX_STRETCH * length)
                    .and_then(|(q, _)| {
                        let candidate = with_end(nnew, end, q);
                        let ahead = (candidate.end[0] - candidate.start[0])
                            * (nold.end[0] - nold.start[0])
                            + (candidate.end[1] - candidate.start[1])
                                * (nold.end[1] - nold.start[1]);
                        (candidate.length() >= types::MIN_WALL_LENGTH && ahead > 0.0)
                            .then_some(candidate)
                    });
                match kept {
                    Some(candidate) => {
                        let corner = end_of(&candidate, end);
                        *nnew = candidate;
                        claimed.insert((*nid, end));
                        out.ends += 1;
                        // A corner closes at both ends: the moved wall's own end meets the
                        // neighbor at the same point.
                        let met = (0..2).find(|e| gap(point, end_of(old, *e)) <= tol);
                        if let Some(e) = met {
                            let target = if xf.reflects() { 1 - e } else { e };
                            if moved_claimed.insert((*id, target)) {
                                *new = with_end(new, target, corner);
                            }
                        }
                    }
                    None => note(&mut out, *nid),
                }
            }
        }
        // An end of the old wall on a neighbor's baseline slides along it.
        for end in 0..2 {
            let point = end_of(old, end);
            let target = if xf.reflects() { 1 - end } else { end };
            if moved_claimed.contains(&(*id, target)) {
                continue;
            }
            for (_, nold, _) in &neighbors {
                let on = archbuilder_model::join::baseline_distance(nold, point) <= tol
                    && gap(point, nold.start) > tol
                    && gap(point, nold.end) > tol;
                if !on {
                    continue;
                }
                let length = new.length();
                let hit = archbuilder_model::join::line_hit(nold, new.start, dir);
                let kept = hit
                    .filter(|(q, _)| gap(*q, end_of(new, target)) <= MAX_STRETCH * length)
                    .map(|(q, _)| with_end(new, target, q))
                    .filter(|c| c.length() >= types::MIN_WALL_LENGTH);
                match kept {
                    Some(candidate) => {
                        *new = candidate;
                        out.ends += 1;
                    }
                    None => note(&mut out, *id),
                }
                break;
            }
        }
    }
    out.moved = moved;
    out.neighbors = neighbors
        .into_iter()
        .filter(|(_, old, new)| old != new)
        .collect();
    Ok(out)
}

/// The walls a move, rotate or mirror of `objects` would change, as they would stand: the moved
/// walls and the neighbors that stretch to keep their joins. For a host's preview; empty when
/// the set is too large or the objects hold no wall.
#[must_use]
pub fn preview(snapshot: &Snapshot, objects: &[ObjectId], xf: &Xform) -> Vec<Wall> {
    let walls: Vec<ObjectId> = objects
        .iter()
        .copied()
        .filter(|id| spatial::type_of(snapshot, *id).as_deref() == Some(WALL))
        .take(200)
        .collect();
    let set = Set {
        walls,
        ..Set::default()
    };
    match reconnect(snapshot, &set, xf) {
        Ok(r) => r
            .moved
            .into_iter()
            .map(|(_, _, new)| new)
            .chain(r.neighbors.into_iter().map(|(_, _, new)| new))
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// What moving one end of a wall changes: the wall as it would stand and the walls that follow.
struct EndMove {
    old: Wall,
    new: Wall,
    neighbors: Vec<(ObjectId, Wall, Wall)>,
}

/// Moves end `end` (0 the start, 1 the end) of wall `id` to `to` and keeps its joins: a neighbor
/// whose end met this end with an auto, butt or square join takes the same corner point, and a
/// neighbor whose end stood on the wall's baseline (a T) stretches or shrinks along its own line to
/// the wall's new line. A wall end with a disallowed join, on either side, moves alone, as does a
/// free end.
fn end_plan(snapshot: &Snapshot, id: ObjectId, end: usize, to: [f64; 2]) -> Result<EndMove> {
    let object = spatial::get(snapshot, id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let old = Wall::from_values(&object.values).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let new = with_end(&old, end, to);
    let tol = archbuilder_model::join::JOIN_TOLERANCE;
    let at = end_of(&old, end);
    let own = if end == 0 {
        old.start_join
    } else {
        old.end_join
    };
    let dir = [new.end[0] - new.start[0], new.end[1] - new.start[1]];
    let mut neighbors: Vec<(ObjectId, Wall, Wall)> = object
        .header
        .owner
        .map(|storey| spatial::walls(snapshot, storey))
        .unwrap_or_default()
        .iter()
        .take(MAX_OBJECTS)
        .filter(|o| o.id != id)
        .filter_map(|o| Wall::from_values(&o.values).map(|w| (o.id, w, w)))
        .collect();
    for (_, nold, nnew) in &mut neighbors {
        for k in 0..2 {
            let point = end_of(nold, k);
            let style = if k == 0 {
                nold.start_join
            } else {
                nold.end_join
            };
            if style == JoinStyle::Disallowed {
                continue;
            }
            if gap(point, at) <= tol {
                if own != JoinStyle::Disallowed {
                    *nnew = with_end(nnew, k, to);
                }
                continue;
            }
            let on_body = archbuilder_model::join::baseline_distance(&old, point) <= tol
                && gap(point, old.start) > tol
                && gap(point, old.end) > tol;
            if !on_body {
                continue;
            }
            let reach = MAX_STRETCH * nold.length();
            let stretched = archbuilder_model::join::line_hit(nnew, new.start, dir)
                .filter(|(q, _)| gap(*q, point) <= reach)
                .map(|(q, _)| with_end(nnew, k, q))
                .filter(|c| c.length() >= types::MIN_WALL_LENGTH);
            if let Some(candidate) = stretched {
                *nnew = candidate;
            }
        }
    }
    neighbors.retain(|(_, o, n)| o != n);
    Ok(EndMove {
        old,
        new,
        neighbors,
    })
}

/// The walls moving one end of wall `id` to `to` would leave: the wall and the neighbors that
/// follow, for a host's preview. Empty when the wall is unknown.
#[must_use]
pub fn preview_end(snapshot: &Snapshot, id: ObjectId, end: usize, to: [f64; 2]) -> Vec<Wall> {
    match end_plan(snapshot, id, end, to) {
        Ok(m) => std::iter::once(m.new)
            .chain(m.neighbors.into_iter().map(|(_, _, n)| n))
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Moves one end of wall `id` (see [`end_plan`]); the openings of the wall keep their distance
/// from the end that stays and those of the neighbors keep their place along them. Returns the
/// number of neighbors that followed and the doors and windows that now open into a wall.
pub(crate) fn move_end_of(
    ctx: &mut Context<'_>,
    id: ObjectId,
    end: usize,
    to: [f64; 2],
) -> Result<(usize, Vec<[String; 2]>)> {
    let snapshot = ctx.snapshot().clone();
    let plan = end_plan(&snapshot, id, end, to)?;
    // The range a refused opening is told is the wall as it would stand after the move.
    let shift = if end == 0 {
        plan.old.length() - plan.new.length()
    } else {
        0.0
    };
    let own = shifted_inserts(&snapshot, id, &plan.new, shift, None)?;
    let mut follows = Vec::new();
    for (nid, nold, nnew) in &plan.neighbors {
        follows.push((
            *nid,
            *nnew,
            shifted_inserts(&snapshot, *nid, nnew, start_shift(nold, nnew), None)?,
        ));
    }
    let count = follows.len();
    for (wid, wall, hosted) in std::iter::once((id, plan.new, own)).chain(follows) {
        for (field, value) in [
            ("start_x", wall.start[0]),
            ("start_y", wall.start[1]),
            ("end_x", wall.end[0]),
            ("end_y", wall.end[1]),
        ] {
            set_length(ctx, wid, field, value)?;
        }
        for (insert, distance, _) in hosted {
            set_length(ctx, insert, "distance", distance)?;
        }
    }
    // A door that now opens into the wall is said, not refused, as a trim says it.
    let meets = ensure_valid_lenient(ctx)?;
    Ok((count, meets))
}

/// The doors and windows of `wall` with their distance after the wall's start moved by `shift`
/// along its line (and the extra change a reflection makes), keeping them where they are.
pub(crate) fn shifted_inserts(
    snapshot: &Snapshot,
    id: ObjectId,
    wall: &Wall,
    shift: f64,
    reflect: Option<&Xform>,
) -> Result<Vec<(ObjectId, f64, u32)>> {
    let mut out = Vec::new();
    for (object, item) in insert::hosted(snapshot, id) {
        let (distance, hand) = match reflect {
            Some(xf) => moved_insert(&item, wall.length(), xf),
            None => (item.distance, item.hand.code()),
        };
        let distance = distance - shift;
        if distance < 0.0 {
            let host = spatial::get(snapshot, id)
                .and_then(|o| types::text(&o.values, 0).map(str::to_owned))
                .unwrap_or_default();
            return Err(Error::new(archbuilder_model::error::codes::INSERT_OUTSIDE)
                .with_arg(
                    "name",
                    types::text(&object.values, 0)
                        .unwrap_or_default()
                        .to_owned(),
                )
                .with_arg("wall", host)
                .with_arg("from", "0.00")
                .with_arg("to", format!("{:.2}", wall.length())));
        }
        out.push((object.id, distance, hand));
    }
    Ok(out)
}

/// Runs the model's rules on the open transaction; what is only a warning (a door or window that
/// now opens into another wall) is returned as `[name, other]` pairs instead of refusing.
pub(crate) fn ensure_valid_lenient(ctx: &mut Context<'_>) -> Result<Vec<[String; 2]>> {
    let txn = ctx.txn()?;
    if let Some(error) =
        archbuilder_model::validate::pin_refusal(txn.base(), txn.snapshot(), txn.touched())
    {
        return Err(error);
    }
    let mut meets = Vec::new();
    for error in archbuilder_model::validate::check(txn.snapshot(), txn.touched()) {
        if !archbuilder_model::validate::is_warning(&error)
            || crate::structure::refuses_always(txn, &error)
        {
            return Err(error);
        }
        // A warning this command did not cause is the register's, not this command's.
        if !crate::structure::caused(txn, &error) {
            continue;
        }
        let arg = |k: &str| {
            error
                .arguments()
                .get(k)
                .map(|v| match v {
                    arc_base::error::Value::Text(t) => t.clone(),
                    other => format!("{other:?}"),
                })
                .unwrap_or_default()
        };
        meets.push([arg("name"), arg("other")]);
    }
    Ok(meets)
}

/// Writes the fields of the stairs `stairs` that `xf` changes: the start, the direction and, under
/// a reflection, the turn and the railing sides.
fn move_stairs(
    ctx: &mut Context<'_>,
    snapshot: &Snapshot,
    stairs: &[ObjectId],
    xf: &Xform,
) -> Result<()> {
    for id in stairs {
        let object = spatial::get(snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let moved = Stair::from_values(&object.values)
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?
            .moved(xf);
        let name = types::text(&object.values, 0)
            .unwrap_or_default()
            .to_owned();
        let txn = ctx.txn()?;
        for (k, value) in moved.values(&name).into_iter().enumerate().skip(1) {
            if let Some(decl) = snapshot
                .registry()
                .get(types::STAIR)
                .and_then(|d| d.schema.fields.get(k))
                && object.values.get(k) != Some(&value)
            {
                txn.modify(*id, FieldPath::new(decl.name.as_str()), value)?;
            }
        }
    }
    Ok(())
}

/// Applies `xf` to the objects of `set` in place.
#[allow(
    clippy::too_many_lines,
    reason = "one block for each kind of element moved"
)]
pub(crate) fn transform_in_place(ctx: &mut Context<'_>, set: &Set, xf: &Xform) -> Result<Modified> {
    // The transaction's view: a command may move walls after it changed others.
    let snapshot = ctx.txn()?.snapshot().clone();
    let walls: BTreeSet<ObjectId> = set.walls.iter().copied().collect();
    let mut count = set.len();
    // Everything is read before the first write.
    let joined = reconnect(&snapshot, set, xf)?;
    let mut wall_changes = Vec::new();
    for (id, old, new) in &joined.moved {
        // A start that slid along a neighbor moves the doors of the wall with it.
        let reflect = xf.reflects().then_some(xf);
        let rigid = moved_wall(old, xf);
        let shift = start_shift(&rigid, new);
        let hosted = shifted_inserts(&snapshot, *id, old, shift, reflect)?;
        count += hosted.len();
        wall_changes.push((*id, *new, hosted));
    }
    let mut follows = Vec::new();
    for (id, old, new) in &joined.neighbors {
        follows.push((
            *id,
            *new,
            shifted_inserts(&snapshot, *id, old, start_shift(old, new), None)?,
        ));
    }
    let mut floor_changes = Vec::new();
    let mut dissociated = 0;
    for id in &set.floors {
        let object =
            spatial::get(&snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let floor =
            Floor::from_values(&object.values).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let boundary = floor.boundary_in(&snapshot);
        let (vertices, lost) = floor_vertices(&floor, boundary.as_ref(), xf, &|w| {
            walls.contains(&w).then_some(w)
        });
        dissociated += lost;
        let mut moved = object.values.clone();
        crate::ceiling::moved_floor_slope(&mut moved, xf);
        floor_changes.push((*id, vertices, moved, object.values.clone()));
    }
    let mut slides = Vec::new();
    for id in &set.inserts {
        let (_, distance) = slide(&snapshot, *id, xf)?;
        slides.push((*id, distance));
    }
    for (id, wall, hosted) in wall_changes.into_iter().chain(follows) {
        for (field, value) in [
            ("start_x", wall.start[0]),
            ("start_y", wall.start[1]),
            ("end_x", wall.end[0]),
            ("end_y", wall.end[1]),
        ] {
            set_length(ctx, id, field, value)?;
        }
        if xf.reflects() && walls.contains(&id) {
            let txn = ctx.txn()?;
            for (field, style) in [("start_join", wall.start_join), ("end_join", wall.end_join)] {
                txn.modify(id, FieldPath::new(field), Some(Value::Enum(style.code())))?;
            }
        }
        for (insert, distance, hand) in hosted {
            set_length(ctx, insert, "distance", distance)?;
            // A bare opening has no hand.
            if xf.reflects()
                && walls.contains(&id)
                && spatial::type_of(&snapshot, insert).as_deref() != Some(types::WALL_OPENING)
            {
                ctx.txn()?
                    .modify(insert, FieldPath::new("hand"), Some(Value::Enum(hand)))?;
            }
        }
    }
    for (id, vertices, moved, old) in floor_changes {
        crate::floor::set_vertices(ctx, id, &vertices)?;
        let txn = ctx.txn()?;
        for (field, at) in [
            ("slope_x", types::floor::SLOPE_X),
            ("slope_y", types::floor::SLOPE_Y),
            ("slope_direction", types::floor::SLOPE_DIRECTION),
        ] {
            if moved.get(at) != old.get(at) {
                txn.modify(id, FieldPath::new(field), moved.get(at).cloned().flatten())?;
            }
        }
    }
    for (id, distance) in slides {
        set_length(ctx, id, "distance", distance)?;
    }
    for id in &set.rooms {
        let room = spatial::get(&snapshot, *id)
            .and_then(|o| archbuilder_model::room::Room::from_values(&o.values))
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let at = xf.point(room.point);
        set_length(ctx, *id, "x", at[0])?;
        set_length(ctx, *id, "y", at[1])?;
    }
    move_stairs(ctx, &snapshot, &set.stairs, xf)?;
    dissociated += crate::roof::move_roofs(ctx, &snapshot, &set.roofs, xf, &walls)?;
    dissociated += crate::ceiling::move_ceilings(ctx, &snapshot, &set.ceilings, xf, &walls)?;
    crate::opening::move_cuts(
        ctx,
        &snapshot,
        &set.shafts,
        archbuilder_model::floor::SHAFT_SLOTS,
        xf,
    )?;
    crate::opening::move_cuts(
        ctx,
        &snapshot,
        &set.cuts,
        archbuilder_model::floor::VERTICAL_SLOTS,
        xf,
    )?;
    let meets = ensure_valid_lenient(ctx)?;
    Ok(Modified {
        objects: count,
        created: Vec::new(),
        dissociated_edges: Some(dissociated).filter(|n| *n > 0),
        reconnected: Some(joined.ends).filter(|n| *n > 0),
        unjoined: joined.unjoined,
        meets,
    })
}

/// Moves the walls `walls` by `d` and keeps their joins as a move does: neighbors whose ends met
/// them stretch along their own lines to the moved walls, and the moved walls' ends go to the same
/// corner points.
///
/// # Errors
/// The errors of a move.
pub(crate) fn translate_walls(
    ctx: &mut Context<'_>,
    walls: &[ObjectId],
    d: [f64; 2],
) -> Result<Modified> {
    let set = Set {
        walls: walls.to_vec(),
        ..Set::default()
    };
    transform_in_place(ctx, &set, &Xform::Translate(d))
}

/// The storey an object belongs to.
fn storey_of_source(object: &arc_doc::Object) -> Option<ObjectId> {
    object.header.owner
}

/// The field values of the copy of the wall `object` under `xf`, named `name`. When the copy goes
/// to another storey (`elsewhere`) a top constrained to a level keeps its height and becomes
/// unconnected: the level may be the new storey's own.
fn copied_wall_values(
    snapshot: &Snapshot,
    object: &arc_doc::Object,
    wall: &Wall,
    xf: &Xform,
    name: String,
    elsewhere: bool,
) -> Vec<Option<Value>> {
    let moved = moved_wall(wall, xf);
    let mut values = object.values.clone();
    let mut put = |at: usize, value: Value| {
        if let Some(slot) = values.get_mut(at) {
            *slot = Some(value);
        }
    };
    put(types::wall::NAME, Value::String(name));
    if elsewhere {
        let height = Wall::of(snapshot, object).map_or(wall.height, |w| w.height);
        put(types::wall::HEIGHT, types::length(height));
    }
    for (at, v) in [
        (types::wall::START_X, moved.start[0]),
        (types::wall::START_Y, moved.start[1]),
        (types::wall::END_X, moved.end[0]),
        (types::wall::END_Y, moved.end[1]),
    ] {
        put(at, types::length(v));
    }
    put(
        types::wall::START_JOIN,
        Value::Enum(moved.start_join.code()),
    );
    put(types::wall::END_JOIN, Value::Enum(moved.end_join.code()));
    if elsewhere && let Some(slot) = values.get_mut(types::wall::TOP_LEVEL) {
        *slot = None;
    }
    values
}

/// Creates copies of the objects of `set` under `xf`, and selects them after the command.
#[allow(
    clippy::too_many_lines,
    reason = "one block per kind of element copied"
)]
pub(crate) fn copy_set(
    ctx: &mut Context<'_>,
    set: &Set,
    xf: &Xform,
    target: Option<ObjectId>,
) -> Result<Modified> {
    let snapshot = ctx.snapshot().clone();
    // A door or window alone has no wall in another storey.
    if target.is_some() && !set.inserts.is_empty() {
        return Err(Error::new(codes::NOT_MOVABLE));
    }
    let tongue = language(ctx);
    let mut created: Vec<ObjectId> = Vec::new();
    let mut map: BTreeMap<ObjectId, ObjectId> = BTreeMap::new();
    let mut count = set.len();
    for id in &set.walls {
        let object =
            spatial::get(&snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let wall =
            Wall::from_values(&object.values).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let storey = target.or(object.header.owner);
        let name = match storey {
            Some(s) => {
                let seen = ctx.txn()?.snapshot().clone();
                crate::wall::next_wall_name(&seen, s, &tongue)
            }
            None => return Err(Error::new(codes::KIND_MISMATCH)),
        };
        let elsewhere = target.is_some_and(|t| Some(t) != storey_of_source(&object));
        let values = copied_wall_values(&snapshot, &object, &wall, xf, name, elsewhere);
        let new = create(ctx, WALL, storey, values)?;
        map.insert(*id, new);
        created.push(new);
        // Copying a wall copies its doors and windows.
        for (host, item) in insert::hosted(&snapshot, *id) {
            let (distance, hand) = moved_insert(&item, wall.length(), xf);
            count += 1;
            created.push(copy_insert(ctx, &host, new, distance, Some(hand), &tongue)?);
        }
    }
    for id in &set.inserts {
        let (item, distance) = slide(&snapshot, *id, xf)?;
        let object =
            spatial::get(&snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let host = object
            .header
            .owner
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let _ = item;
        created.push(copy_insert(ctx, &object, host, distance, None, &tongue)?);
    }
    let mut dissociated = 0;
    // The copies of floors, roofs and ceilings, by original: their vertical openings follow.
    let mut hosts: BTreeMap<ObjectId, ObjectId> = BTreeMap::new();
    for id in &set.floors {
        let object =
            spatial::get(&snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let floor =
            Floor::from_values(&object.values).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let boundary = floor.boundary_in(&snapshot);
        let (vertices, lost) =
            floor_vertices(&floor, boundary.as_ref(), xf, &|w| map.get(&w).copied());
        dissociated += lost;
        let storey = target.or(object.header.owner);
        let name = match storey {
            Some(s) => {
                let seen = ctx.txn()?.snapshot().clone();
                crate::floor::next_floor_name(&seen, s, &tongue)
            }
            None => return Err(Error::new(codes::KIND_MISMATCH)),
        };
        let mut values = object.values.clone();
        crate::ceiling::moved_floor_slope(&mut values, xf);
        if let Some(slot) = values.get_mut(types::floor::NAME) {
            *slot = Some(Value::String(name));
        }
        if let Some(slot) = values.get_mut(types::floor::VERTICES) {
            *slot = Some(Floor::vertex_values(&vertices));
        }
        if let Some(slot) = values.get_mut(types::floor::ARCS) {
            *slot = Floor::arc_values(&vertices);
        }
        let new = create(ctx, FLOOR, storey, values)?;
        hosts.insert(*id, new);
        created.push(new);
    }
    for id in &set.rooms {
        created.push(copy_room(ctx, &snapshot, *id, xf, target)?);
    }
    for id in &set.stairs {
        let object =
            spatial::get(&snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let mut moved = Stair::from_values(&object.values)
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?
            .moved(xf);
        let storey = target.or(object.header.owner);
        let Some(s) = storey else {
            return Err(Error::new(codes::KIND_MISMATCH));
        };
        // In another storey the top is that storey's level above.
        if target.is_some_and(|t| Some(t) != object.header.owner) {
            moved.top_level = None;
        }
        let seen = ctx.txn()?.snapshot().clone();
        let name = crate::stair::next_stair_name(&seen, s, &tongue);
        created.push(create(ctx, types::STAIR, storey, moved.values(&name))?);
    }
    let (roofs, lost) = crate::roof::copy_roofs(
        ctx,
        &snapshot,
        &set.roofs,
        xf,
        &|w| map.get(&w).copied(),
        target,
    )?;
    hosts.extend(set.roofs.iter().copied().zip(roofs.iter().copied()));
    created.extend(roofs);
    dissociated += lost;
    let (ceilings, lost) = crate::ceiling::copy_ceilings(
        ctx,
        &snapshot,
        &set.ceilings,
        xf,
        &|w| map.get(&w).copied(),
        target,
    )?;
    hosts.extend(ceilings.iter().copied());
    created.extend(ceilings.into_iter().map(|(_, new)| new));
    dissociated += lost;
    // The edge elements of the floors copied go with them: they follow the new floor's edges.
    created.extend(crate::floor_edge::copy_for(ctx, &snapshot, &hosts)?);
    created.extend(crate::opening::copy_cuts(
        ctx,
        &snapshot,
        &set.shafts,
        xf,
        &|_| None,
        target,
    )?);
    created.extend(crate::opening::copy_cuts(
        ctx,
        &snapshot,
        &set.cuts,
        xf,
        &|host| hosts.get(&host).copied(),
        target,
    )?);
    let meets = ensure_valid_lenient(ctx)?;
    ctx.set_selection_after(created.clone());
    Ok(Modified {
        objects: count,
        created: created.into_iter().map(IdParam).collect(),
        dissociated_edges: Some(dissociated).filter(|n| *n > 0),
        meets,
        ..Modified::default()
    })
}

/// A copy of the room `id` with its point under `xf` and the next free number of its storey.
fn copy_room(
    ctx: &mut Context<'_>,
    snapshot: &Snapshot,
    id: ObjectId,
    xf: &Xform,
    target: Option<ObjectId>,
) -> Result<ObjectId> {
    let object = spatial::get(snapshot, id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let room = archbuilder_model::room::Room::from_values(&object.values)
        .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let storey = target.or(object.header.owner);
    let at = xf.point(room.point);
    let mut values = object.values.clone();
    let number = {
        let seen = ctx.txn()?.snapshot().clone();
        let rooms = storey.map(|s| spatial::children(&seen, types::ROOM, s));
        archbuilder_model::room::next_number(
            rooms
                .iter()
                .flatten()
                .filter_map(|r| types::text(&r.values, types::room::NUMBER)),
        )
    };
    for (slot, value) in [
        (types::room::NUMBER, Value::String(number)),
        (types::room::X, types::length(at[0])),
        (types::room::Y, types::length(at[1])),
    ] {
        if let Some(v) = values.get_mut(slot) {
            *v = Some(value);
        }
    }
    create(ctx, types::ROOM, storey, values)
}

/// A copy of the insert `source` on the wall `host` at `distance`, with the next free name and,
/// when given, the hand `hand`.
pub(crate) fn copy_insert(
    ctx: &mut Context<'_>,
    source: &arc_doc::Object,
    host: ObjectId,
    distance: f64,
    hand: Option<u32>,
    tongue: &str,
) -> Result<ObjectId> {
    let key = spatial::type_of(ctx.snapshot(), source.id)
        .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let kind = Kind::of_key(&key).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let seen = ctx.txn()?.snapshot().clone();
    let name = crate::insert::next_name(&seen, kind, host, tongue);
    let mut values = source.values.clone();
    let mut put = |at: usize, value: Value| {
        if let Some(slot) = values.get_mut(at) {
            *slot = Some(value);
        }
    };
    put(types::insert::NAME, Value::String(name));
    put(types::insert::DISTANCE, types::length(distance));
    if let Some(code) = hand {
        put(types::insert::HAND, Value::Enum(code));
    }
    create(ctx, &key, Some(host), values)
}

fn run(
    ctx: &mut Context<'_>,
    objects: Option<Vec<IdParam>>,
    xf: Xform,
    copy: bool,
) -> Result<Modified> {
    if !xf.is_finite() {
        return Err(invalid());
    }
    let set = gather(ctx, objects)?;
    if copy {
        copy_set(ctx, &set, &xf, None)
    } else {
        transform_in_place(ctx, &set, &xf)
    }
}

fn move_objects(ctx: &mut Context<'_>, p: Displace) -> Result<Modified> {
    run(ctx, p.objects, Xform::Translate(p.displacement), false)
}

fn copy_objects(ctx: &mut Context<'_>, p: Displace) -> Result<Modified> {
    run(ctx, p.objects, Xform::Translate(p.displacement), true)
}

/// Pastes `p.objects` as a copy by the displacement, into the storey `p.storey` when named.
fn paste_objects(ctx: &mut Context<'_>, p: Paste, into_storey: bool) -> Result<Modified> {
    if let Some(content) = &p.content {
        let IdParam(storey) = p.storey.ok_or_else(invalid)?;
        if spatial::type_of(ctx.snapshot(), storey).as_deref() != Some(types::STOREY) {
            return Err(invalid());
        }
        return crate::clip::paste_content(
            ctx,
            content,
            p.displacement.unwrap_or([0.0, 0.0]),
            storey,
        );
    }
    let storey = match p.storey {
        Some(IdParam(id)) => {
            if spatial::type_of(ctx.snapshot(), id).as_deref() != Some(types::STOREY) {
                return Err(invalid());
            }
            Some(id)
        }
        None if into_storey => return Err(invalid()),
        None => None,
    };
    let displacement = p.displacement.unwrap_or([0.0, 0.0]);
    let xf = Xform::Translate(displacement);
    if !xf.is_finite() {
        return Err(invalid());
    }
    let set = gather(ctx, p.objects)?;
    copy_set(ctx, &set, &xf, storey)
}

fn rotate_objects(ctx: &mut Context<'_>, p: Rotate) -> Result<Modified> {
    // A bare text number is degrees; a JSON number is radians; a text with a unit says its own.
    let angle = match &p.angle {
        QuantityParam::Text(t) if t.trim().parse::<f64>().is_ok() => {
            t.trim().parse::<f64>().unwrap_or(0.0).to_radians()
        }
        other => ctx.quantity(other, Dimension::ANGLE)?.value(),
    };
    run(
        ctx,
        p.objects,
        Xform::Rotate {
            base: p.base,
            angle,
        },
        false,
    )
}

fn mirror_objects(ctx: &mut Context<'_>, p: Mirror) -> Result<Modified> {
    let xf = Xform::mirror(p.axis[0], p.axis[1]).ok_or_else(|| Error::new(codes::MIRROR_AXIS))?;
    run(ctx, p.objects, xf, p.keep.unwrap_or(true))
}

/// A message of the catalog, named by identifier.
struct Msg {
    message_id: &'static str,
}

const PROMPT_BASE: Msg = Msg {
    message_id: "ab-prompt-modify-base",
};
const PROMPT_SECOND_MOVE: Msg = Msg {
    message_id: "ab-prompt-modify-second-move",
};
const PROMPT_SECOND_COPY: Msg = Msg {
    message_id: "ab-prompt-modify-second-copy",
};
const PROMPT_PASTE_POINT: Msg = Msg {
    message_id: "ab-prompt-paste-point",
};
const PROMPT_DISPLACEMENT: Msg = Msg {
    message_id: "ab-prompt-modify-displacement",
};
const PROMPT_ROTATE_BASE: Msg = Msg {
    message_id: "ab-prompt-rotate-base",
};
const PROMPT_ROTATE_ANGLE: Msg = Msg {
    message_id: "ab-prompt-rotate-angle",
};
const PROMPT_ROTATE_REFERENCE: Msg = Msg {
    message_id: "ab-prompt-rotate-reference",
};
const PROMPT_ROTATE_NEW: Msg = Msg {
    message_id: "ab-prompt-rotate-new",
};
const PROMPT_MIRROR_FIRST: Msg = Msg {
    message_id: "ab-prompt-mirror-first",
};
const PROMPT_MIRROR_SECOND: Msg = Msg {
    message_id: "ab-prompt-mirror-second",
};
const PROMPT_MIRROR_ERASE: Msg = Msg {
    message_id: "ab-prompt-mirror-erase",
};
const PROMPT_SELECT: Msg = Msg {
    message_id: "ab-prompt-modify-select",
};
const KEY_DISPLACEMENT: Msg = Msg {
    message_id: "ab-keyword-modify-displacement",
};
const KEY_REFERENCE: Msg = Msg {
    message_id: "ab-keyword-rotate-reference",
};
const KEY_YES: Msg = Msg {
    message_id: "ab-keyword-yes",
};
const KEY_NO: Msg = Msg {
    message_id: "ab-keyword-no",
};

/// Which command a prompt machine asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    Move,
    Copy,
    Rotate,
    Mirror,
    Paste,
}

/// The prompts of the modify commands; see the module documentation.
#[derive(Debug)]
pub struct ModifyPrompts {
    which: Which,
}

fn point_of(value: Option<&Json>) -> Option<[f64; 2]> {
    match value?.as_array()?.as_slice() {
        [x, y] => Some([x.as_f64()?, y.as_f64()?]),
        _ => None,
    }
}

fn point_prompt(key: String, message: &Msg, keywords: Vec<KeywordOption>) -> Prompt {
    let prompt = Prompt::new(
        key,
        message.message_id,
        Accept::Point {
            dims: PointDims::Two,
        },
    );
    if keywords.is_empty() {
        prompt
    } else {
        prompt.or(Accept::keywords(keywords))
    }
}

fn angle_prompt(key: String, message: &Msg, keywords: Vec<KeywordOption>) -> Prompt {
    // The angle first, so a typed number is an angle; a point gives the direction from the base.
    let prompt = Prompt::new(key, message.message_id, Accept::angle()).or(Accept::Point {
        dims: PointDims::Two,
    });
    if keywords.is_empty() {
        prompt
    } else {
        prompt.or(Accept::keywords(keywords))
    }
}

/// The direction of the point `p` seen from `base`, in radians.
fn direction(base: [f64; 2], p: &Point) -> f64 {
    (p.y - base[1]).atan2(p.x - base[0])
}

impl ModifyPrompts {
    fn displace(&self, state: &PromptState<'_>) -> Step {
        let mut base = point_of(state.preset.get("base"));
        let mut at = 0;
        if base.is_none() {
            let Some(answer) = state.answers.first() else {
                let key = "base-0".to_owned();
                return Step::Ask(point_prompt(
                    key,
                    &PROMPT_BASE,
                    vec![
                        KeywordOption::new("displacement", KEY_DISPLACEMENT.message_id)
                            .with_shortcut('d'),
                    ],
                ));
            };
            at = 1;
            match &answer.input {
                PromptInput::Point(p) => base = Some([p.x, p.y]),
                PromptInput::Keyword(_) => {
                    return match state.answers.get(1).map(|a| &a.input) {
                        Some(PromptInput::Point(p)) => Step::Done(displacement([p.x, p.y])),
                        _ => Step::Ask(point_prompt(
                            "displacement-1".to_owned(),
                            &PROMPT_DISPLACEMENT,
                            Vec::new(),
                        )),
                    };
                }
                _ => {}
            }
        }
        let Some(base) = base else {
            return Step::Ask(point_prompt("base-0".to_owned(), &PROMPT_BASE, Vec::new()));
        };
        let message = match self.which {
            Which::Copy => &PROMPT_SECOND_COPY,
            Which::Paste => &PROMPT_PASTE_POINT,
            _ => &PROMPT_SECOND_MOVE,
        };
        match state.answers.get(at).map(|a| &a.input) {
            Some(PromptInput::Point(p)) => Step::Done(displacement([p.x - base[0], p.y - base[1]])),
            _ => Step::Ask(point_prompt(format!("second-{at}"), message, Vec::new())),
        }
    }

    fn rotate(&self, state: &PromptState<'_>) -> Step {
        let mut at = 0;
        let mut base = point_of(state.preset.get("base"));
        if base.is_none() {
            match state.answers.first().map(|a| &a.input) {
                Some(PromptInput::Point(p)) => {
                    base = Some([p.x, p.y]);
                    at = 1;
                }
                _ => {
                    return Step::Ask(point_prompt(
                        "base-0".to_owned(),
                        &PROMPT_ROTATE_BASE,
                        Vec::new(),
                    ));
                }
            }
        }
        let Some(base) = base else {
            return Step::Ask(point_prompt(
                "base-0".to_owned(),
                &PROMPT_ROTATE_BASE,
                Vec::new(),
            ));
        };
        let angle_of = |input: &PromptInput| match input {
            PromptInput::Angle(a) => Some(*a),
            PromptInput::Point(p) => Some(direction(base, p)),
            _ => None,
        };
        let done = |angle: f64| {
            let mut out = Map::new();
            out.insert("base".to_owned(), Json::from(base.to_vec()));
            out.insert("angle".to_owned(), Json::from(angle));
            Step::Done(out)
        };
        match state.answers.get(at).map(|a| &a.input) {
            Some(PromptInput::Keyword(_)) => {
                // Reference: the angle now, then the angle it should become.
                let Some(reference) = state.answers.get(at + 1).and_then(|a| angle_of(&a.input))
                else {
                    return Step::Ask(angle_prompt(
                        format!("reference-{at}"),
                        &PROMPT_ROTATE_REFERENCE,
                        Vec::new(),
                    ));
                };
                match state.answers.get(at + 2).and_then(|a| angle_of(&a.input)) {
                    Some(new) => done(new - reference),
                    None => Step::Ask(angle_prompt(
                        format!("new-{at}"),
                        &PROMPT_ROTATE_NEW,
                        Vec::new(),
                    )),
                }
            }
            Some(input) => match angle_of(input) {
                Some(angle) => done(angle),
                None => Step::Ask(angle_prompt(
                    format!("angle-{at}"),
                    &PROMPT_ROTATE_ANGLE,
                    Vec::new(),
                )),
            },
            None => Step::Ask(angle_prompt(
                format!("angle-{at}"),
                &PROMPT_ROTATE_ANGLE,
                vec![KeywordOption::new("reference", KEY_REFERENCE.message_id).with_shortcut('r')],
            )),
        }
    }

    fn mirror(state: &PromptState<'_>) -> Step {
        let points: Vec<[f64; 2]> = state
            .answers
            .iter()
            .filter_map(|a| match &a.input {
                PromptInput::Point(p) => Some([p.x, p.y]),
                _ => None,
            })
            .collect();
        match points.as_slice() {
            [] => Step::Ask(point_prompt(
                "first-0".to_owned(),
                &PROMPT_MIRROR_FIRST,
                Vec::new(),
            )),
            [_] => Step::Ask(point_prompt(
                "second-1".to_owned(),
                &PROMPT_MIRROR_SECOND,
                Vec::new(),
            )),
            [a, b, ..] => match state.answers.get(2).map(|x| &x.input) {
                Some(PromptInput::Keyword(k)) => {
                    let mut out = Map::new();
                    out.insert("axis".to_owned(), Json::from(vec![a.to_vec(), b.to_vec()]));
                    out.insert("keep".to_owned(), Json::from(k != "yes"));
                    Step::Done(out)
                }
                _ => Step::Ask(
                    Prompt::new(
                        "erase-2",
                        PROMPT_MIRROR_ERASE.message_id,
                        Accept::keywords([
                            KeywordOption::new("yes", KEY_YES.message_id).with_shortcut('y'),
                            KeywordOption::new("no", KEY_NO.message_id).with_shortcut('n'),
                        ]),
                    )
                    .with_default(PromptInput::Keyword("no".to_owned())),
                ),
            },
        }
    }
}

fn displacement(d: [f64; 2]) -> Map<String, Json> {
    let mut out = Map::new();
    out.insert("displacement".to_owned(), Json::from(d.to_vec()));
    out
}

impl PromptMachine for ModifyPrompts {
    fn next(&self, state: &PromptState<'_>) -> Result<Step> {
        Ok(match self.which {
            Which::Move | Which::Copy | Which::Paste => self.displace(state),
            Which::Rotate => self.rotate(state),
            Which::Mirror => Self::mirror(state),
        })
    }

    fn check(&self, state: &PromptState<'_>, prompt: &Prompt, input: &PromptInput) -> Result<()> {
        // The second point of the mirror line must differ from the first.
        if let (Which::Mirror, PromptInput::Point(p), Some(PromptInput::Point(first))) =
            (self.which, input, state.answers.first().map(|a| &a.input))
            && prompt.key.starts_with("second")
            && (p.x - first.x).hypot(p.y - first.y) < 1.0e-9
        {
            return Err(Error::new(codes::MIRROR_AXIS));
        }
        Ok(())
    }
}

fn spec(id: &str, label: Msg, description: Msg, which: Which) -> CommandSpec {
    match which {
        Which::Move | Which::Copy => {
            let copy = which == Which::Copy;
            CommandSpec::new(
                id,
                label.message_id,
                description.message_id,
                move |ctx: &mut Context<'_>, p: Displace| {
                    if copy {
                        copy_objects(ctx, p)
                    } else {
                        move_objects(ctx, p)
                    }
                },
            )
            .with_prompt(Arc::new(ModifyPrompts { which }))
        }
        Which::Rotate => {
            CommandSpec::new(id, label.message_id, description.message_id, rotate_objects)
                .with_prompt(Arc::new(ModifyPrompts { which }))
        }
        Which::Mirror => {
            CommandSpec::new(id, label.message_id, description.message_id, mirror_objects)
                .with_prompt(Arc::new(ModifyPrompts { which }))
        }
        Which::Paste => CommandSpec::new(
            id,
            label.message_id,
            description.message_id,
            |ctx: &mut Context<'_>, p: Paste| paste_objects(ctx, p, false),
        )
        .with_prompt(Arc::new(ModifyPrompts { which })),
    }
}

/// The prompt a host shows while it waits for the objects of a modify command to be selected.
#[must_use]
pub fn select_prompt() -> Prompt {
    Prompt::new(
        "select-0",
        PROMPT_SELECT.message_id,
        Accept::Selection {
            min: 1,
            max: None,
            types: Vec::new(),
        },
    )
}

/// Registers the commands of this module and their typed names: `m`, `co`, `ro`, `mi`.
///
/// # Errors
/// `cmd.duplicate` when an identifier or a name is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(spec(
        MODIFY_MOVE,
        Msg {
            message_id: "ab-cmd-modify-move-label",
        },
        Msg {
            message_id: "ab-cmd-modify-move-description",
        },
        Which::Move,
    ))?;
    registry.register(spec(
        MODIFY_COPY,
        Msg {
            message_id: "ab-cmd-modify-copy-label",
        },
        Msg {
            message_id: "ab-cmd-modify-copy-description",
        },
        Which::Copy,
    ))?;
    registry.register(spec(
        MODIFY_ROTATE,
        Msg {
            message_id: "ab-cmd-modify-rotate-label",
        },
        Msg {
            message_id: "ab-cmd-modify-rotate-description",
        },
        Which::Rotate,
    ))?;
    registry.register(spec(
        MODIFY_MIRROR,
        Msg {
            message_id: "ab-cmd-modify-mirror-label",
        },
        Msg {
            message_id: "ab-cmd-modify-mirror-description",
        },
        Which::Mirror,
    ))?;
    registry.register(spec(
        CLIPBOARD_PASTE,
        Msg {
            message_id: "ab-cmd-clipboard-paste-label",
        },
        Msg {
            message_id: "ab-cmd-clipboard-paste-description",
        },
        Which::Paste,
    ))?;
    let (storey_label, storey_description) = (
        Msg {
            message_id: "ab-cmd-clipboard-paste-storey-label",
        },
        Msg {
            message_id: "ab-cmd-clipboard-paste-storey-description",
        },
    );
    registry.register(CommandSpec::new(
        CLIPBOARD_PASTE_TO_STOREY,
        storey_label.message_id,
        storey_description.message_id,
        |ctx: &mut Context<'_>, p: Paste| paste_objects(ctx, p, true),
    ))?;
    for (alias, id) in [
        ("m", MODIFY_MOVE),
        ("move", MODIFY_MOVE),
        ("co", MODIFY_COPY),
        ("copy", MODIFY_COPY),
        ("ro", MODIFY_ROTATE),
        ("rotate", MODIFY_ROTATE),
        ("mi", MODIFY_MIRROR),
        ("mirror", MODIFY_MIRROR),
    ] {
        registry.add_user_alias(alias, id)?;
    }
    Ok(())
}
