//! The editing commands that work on picked lines and walls, or on the selection: align, offset,
//! trim and extend, split and merge, linear and radial arrays, and pin and unpin. Each is one
//! transaction, so one undo step; the model's rules apply as for every command (a pinned element
//! refuses them all, [`archbuilder_model::validate::pin_refusal`]).
//!
//! Picks are points. A point picks the nearest line within `radius` (the faces and the center line
//! of a wall, the edges of a floor) or the wall whose body is within `radius`, on the storey
//! `storey` when it is given, else on any storey.
//!
//! - **Align** (`archbuilder.modify.align`): the element of the target line turns about the picked
//!   point until the line is parallel to the reference, then moves across onto it. Walls keep their
//!   joins and their doors and windows follow, as a move does.
//! - **Offset** (`archbuilder.modify.offset`): the selected walls and floors move, or are copied,
//!   across by a distance toward the side of the point `side` (the distance is the point's own
//!   when none is given). Selected walls joined end to end move as a chain: their corners stay
//!   joined. A floor's boundary grows when the point is outside it and shrinks when inside.
//! - **Trim** (`archbuilder.modify.trim`): two walls trim or extend to the corner of their lines;
//!   the picked part of each stays. **Trim to** (`trim_to`) trims or extends each target wall to a
//!   boundary line, keeping the picked part.
//! - **Split** (`archbuilder.wall.split`): a wall becomes two at a point (or at a distance from the
//!   end nearest it), with an optional gap. Doors and windows go to the piece that holds them; an
//!   opening across the cut refuses the split. **Merge** (`archbuilder.wall.merge`) joins two
//!   collinear walls that touch end to end, of the same type and height, into the first.
//! - **Array** (`archbuilder.modify.array`, `array_radial`): independent copies of the selection
//!   in a row (spacing or distance to the last) or around a center. A door or window arrays only
//!   along its own wall.
//! - **Pin** and **unpin** (`archbuilder.element.pin`, `unpin`) set the pinned flag.

use std::sync::Arc;

use arc_base::{Dimension, Error, ObjectId, Result, Tolerance};
use arc_command::prompt::{
    Accept, KeywordOption, Point, PointDims, Prompt, PromptInput, PromptMachine, PromptState, Step,
};
use arc_command::{CommandSpec, Context, IdParam, QuantityParam, Registry};
use arc_doc::{FieldPath, Snapshot, Value};
use archbuilder_model::floor::{Floor, Vertex, signed_area};
use archbuilder_model::insert;
use archbuilder_model::spatial;
use archbuilder_model::transform::Xform;
use archbuilder_model::types::{self, FLOOR, STOREY, WALL};
use archbuilder_model::wall::{JoinStyle, Wall};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json};

use crate::error::codes;
use crate::modify::{
    MAX_OBJECTS, Modified, Set, copy_insert, copy_set, ensure_valid_lenient, gather, invalid,
    set_length, shifted_inserts, start_shift, transform_in_place,
};
use crate::structure::{create, language};

/// The identifier of the align command.
pub const MODIFY_ALIGN: &str = "archbuilder.modify.align";
/// The identifier of the offset command.
pub const MODIFY_OFFSET: &str = "archbuilder.modify.offset";
/// The identifier of the command that trims or extends two walls to a corner.
pub const MODIFY_TRIM: &str = "archbuilder.modify.trim";
/// The identifier of the command that trims or extends one wall at a time to a boundary.
pub const MODIFY_TRIM_TO: &str = "archbuilder.modify.trim_to";
/// The identifier of the command that trims or extends several walls to a boundary at once.
pub const MODIFY_TRIM_MANY: &str = "archbuilder.modify.trim_many";
/// The identifier of the split command.
pub const WALL_SPLIT: &str = "archbuilder.wall.split";
/// The identifier of the merge command.
pub const WALL_MERGE: &str = "archbuilder.wall.merge";
/// The identifier of the linear array command.
pub const MODIFY_ARRAY: &str = "archbuilder.modify.array";
/// The identifier of the radial array command.
pub const MODIFY_ARRAY_RADIAL: &str = "archbuilder.modify.array_radial";
/// The identifier of the pin command.
pub const ELEMENT_PIN: &str = "archbuilder.element.pin";
/// The identifier of the unpin command.
pub const ELEMENT_UNPIN: &str = "archbuilder.element.unpin";

/// The commands that act on picks rather than on the selection.
pub const PICK_COMMANDS: [&str; 6] = [
    MODIFY_ALIGN,
    MODIFY_TRIM,
    MODIFY_TRIM_TO,
    MODIFY_TRIM_MANY,
    WALL_SPLIT,
    WALL_MERGE,
];

/// The most items an array makes, the original included.
pub const MAX_COUNT: u32 = 1000;
/// The most targets one trim takes.
pub const MAX_TARGETS: usize = 500;
/// The pick radius when none is given, in meters.
const DEFAULT_RADIUS: f64 = 0.3;

type P = [f64; 2];

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1]]
}
fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1]]
}
fn scale(a: P, k: f64) -> P {
    [a[0] * k, a[1] * k]
}
fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn cross(a: P, b: P) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
fn unit(a: P) -> Option<P> {
    let n = a[0].hypot(a[1]);
    (n > Tolerance::DEFAULT.linear).then(|| [a[0] / n, a[1] / n])
}
fn left(u: P) -> P {
    [-u[1], u[0]]
}
/// The distance from `p` to the segment `a`-`b`.
fn segment_distance(p: P, a: P, b: P) -> f64 {
    let d = sub(b, a);
    let len2 = dot(d, d);
    let t = if len2 > 0.0 {
        (dot(sub(p, a), d) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let q = add(a, scale(d, t));
    (p[0] - q[0]).hypot(p[1] - q[1])
}
/// Where the line through `a` along `u` meets the line through `b` along `v`; `None` when they
/// are parallel.
fn meet(a: P, u: P, b: P, v: P) -> Option<P> {
    let c = cross(u, v);
    if c.abs() <= Tolerance::DEFAULT.angular {
        return None;
    }
    Some(add(a, scale(u, cross(sub(b, a), v) / c)))
}

/// The direction of a wall's baseline as a unit vector.
fn direction(w: &Wall) -> Option<P> {
    unit(sub(w.end, w.start))
}

/// The distance from `p` to the body of `w` (zero inside it).
fn wall_distance(w: &Wall, p: P) -> f64 {
    let Some(u) = direction(w) else {
        return (p[0] - w.start[0]).hypot(p[1] - w.start[1]);
    };
    let (s, o) = (dot(sub(p, w.start), u), dot(sub(p, w.start), left(u)));
    let (low, high) = w.extent();
    let along = (-s).max(s - w.length()).max(0.0);
    let across = (low - o).max(o - high).max(0.0);
    along.hypot(across)
}

/// The storeys picks look in: `storey`, else all of them.
fn storeys(snapshot: &Snapshot, storey: Option<ObjectId>) -> Vec<ObjectId> {
    match storey {
        Some(s) => vec![s],
        None => spatial::objects(snapshot, STOREY)
            .iter()
            .map(|o| o.id)
            .collect(),
    }
}

/// The wall whose body is nearest `p`, within `radius`.
fn wall_at(
    snapshot: &Snapshot,
    storey: Option<ObjectId>,
    p: P,
    radius: f64,
) -> Result<(ObjectId, Wall)> {
    storeys(snapshot, storey)
        .into_iter()
        .flat_map(|s| spatial::walls(snapshot, s))
        .take(MAX_OBJECTS)
        .filter_map(|o| Wall::from_values(&o.values).map(|w| (o.id, w)))
        .map(|(id, w)| (wall_distance(&w, p), id, w))
        .filter(|(d, _, _)| *d <= radius)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .ok_or_else(|| Error::new(codes::NO_WALL_AT))
        .and_then(|(_, id, w)| {
            if w.is_arc() {
                Err(Error::new(codes::ARC_WALL_UNSUPPORTED).with_arg("name", name_of(snapshot, id)))
            } else {
                Ok((id, w))
            }
        })
}

/// A straight line an element offers to align or trim to: a face or the center line of a wall,
/// an edge of a floor.
#[derive(Debug, Clone, Copy)]
struct Line {
    owner: ObjectId,
    a: P,
    b: P,
}

/// The lines of the walls and floors of `storey`.
fn lines(snapshot: &Snapshot, storey: ObjectId) -> Vec<Line> {
    let mut out = Vec::new();
    for object in spatial::walls(snapshot, storey).iter().take(MAX_OBJECTS) {
        let Some(w) = Wall::from_values(&object.values).filter(|w| !w.is_arc()) else {
            continue;
        };
        let f = w.plain_footprint();
        let mid = |a: P, b: P| scale(add(a, b), 0.5);
        for (a, b) in [
            (f[0], f[1]),
            (f[3], f[2]),
            (mid(f[0], f[3]), mid(f[1], f[2])),
        ] {
            out.push(Line {
                owner: object.id,
                a,
                b,
            });
        }
    }
    for object in spatial::children(snapshot, FLOOR, storey)
        .iter()
        .take(MAX_OBJECTS)
    {
        let Some(boundary) =
            Floor::from_values(&object.values).and_then(|f| f.boundary_in(snapshot))
        else {
            continue;
        };
        for points in &boundary.loops {
            for (k, a) in points.iter().enumerate() {
                if let Some(b) = points.get((k + 1) % points.len()) {
                    out.push(Line {
                        owner: object.id,
                        a: *a,
                        b: *b,
                    });
                }
            }
        }
    }
    out
}

/// The line nearest `p` within `radius`, not of the element `skip`.
fn line_at(
    snapshot: &Snapshot,
    storey: Option<ObjectId>,
    p: P,
    radius: f64,
    skip: Option<ObjectId>,
) -> Result<Line> {
    storeys(snapshot, storey)
        .into_iter()
        .flat_map(|s| lines(snapshot, s))
        .filter(|l| Some(l.owner) != skip && unit(sub(l.b, l.a)).is_some())
        .map(|l| (segment_distance(p, l.a, l.b), l))
        .filter(|(d, _)| *d <= radius)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, l)| l)
        .ok_or_else(|| Error::new(codes::NO_LINE_AT))
}

/// The pick radius of a command: the given one, bounded, else the default.
fn radius_of(radius: Option<f64>) -> Result<f64> {
    match radius {
        None => Ok(DEFAULT_RADIUS),
        Some(r) if r.is_finite() && r > 0.0 && r <= 10.0 => Ok(r),
        Some(_) => Err(invalid()),
    }
}

fn finite(points: &[P]) -> Result<()> {
    if points.iter().flatten().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err(invalid())
    }
}

/// The name of an object as written.
fn name_of(snapshot: &Snapshot, id: ObjectId) -> String {
    spatial::get(snapshot, id)
        .and_then(|o| types::text(&o.values, 0).map(str::to_owned))
        .unwrap_or_default()
}

/// Writes the baseline and the join styles of wall `id` as `new`; its doors and windows keep their
/// place in the plan (a door the wall no longer reaches is refused by the model's rules).
fn reshape(ctx: &mut Context<'_>, id: ObjectId, new: &Wall) -> Result<()> {
    let snapshot = ctx.txn()?.snapshot().clone();
    let old = spatial::get(&snapshot, id)
        .and_then(|o| Wall::from_values(&o.values))
        .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let hosted = shifted_inserts(&snapshot, id, new, start_shift(&old, new), None)?;
    for (field, value) in [
        ("start_x", new.start[0]),
        ("start_y", new.start[1]),
        ("end_x", new.end[0]),
        ("end_y", new.end[1]),
    ] {
        set_length(ctx, id, field, value)?;
    }
    for (field, before, after) in [
        ("start_join", old.start_join, new.start_join),
        ("end_join", old.end_join, new.end_join),
    ] {
        if before != after {
            ctx.txn()?
                .modify(id, FieldPath::new(field), Some(Value::Enum(after.code())))?;
        }
    }
    for (item, distance, _) in hosted {
        set_length(ctx, item, "distance", distance)?;
    }
    Ok(())
}

/// `w` trimmed or extended to the point `x` of its line, keeping the part on the side of `pick`.
fn keep_side(w: &Wall, x: P, pick: P) -> Wall {
    let u = direction(w).unwrap_or([1.0, 0.0]);
    if dot(sub(pick, w.start), u) > dot(sub(x, w.start), u) {
        Wall { start: x, ..*w }
    } else {
        Wall { end: x, ..*w }
    }
}

/// What the editing commands return: what a modify command returns, and what the next run of a
/// repeating tool keeps.
#[derive(Debug, Default, Serialize, JsonSchema)]
pub struct Edited {
    /// What changed.
    #[serde(flatten)]
    pub modified: Modified,
    /// The parameters the next run of the tool keeps: the reference of align, the boundary of
    /// trim to, the gap of split.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep: Option<Json>,
}

fn kept(modified: Modified, key: &str, value: Json) -> Edited {
    let mut keep = Map::new();
    keep.insert(key.to_owned(), value);
    Edited {
        modified,
        keep: Some(Json::Object(keep)),
    }
}

/// Parameters of `archbuilder.modify.align`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Align {
    /// A point on the reference line.
    pub reference: [f64; 2],
    /// A point on the line of the element to align.
    pub target: [f64; 2],
    /// The storey the picks look in; any storey when absent.
    #[serde(default)]
    pub storey: Option<IdParam>,
    /// The pick radius in meters; 0.3 when absent.
    #[serde(default)]
    pub radius: Option<f64>,
}

fn align(ctx: &mut Context<'_>, p: Align) -> Result<Edited> {
    finite(&[p.reference, p.target])?;
    let radius = radius_of(p.radius)?;
    let storey = p.storey.map(|IdParam(id)| id);
    let snapshot = ctx.snapshot().clone();
    let reference = line_at(&snapshot, storey, p.reference, radius, None)?;
    let target = line_at(&snapshot, storey, p.target, radius, Some(reference.owner))?;
    let (Some(ur), Some(ut)) = (
        unit(sub(reference.b, reference.a)),
        unit(sub(target.b, target.a)),
    ) else {
        return Err(Error::new(codes::NO_LINE_AT));
    };
    // The smallest turn that makes the target parallel, either way along the reference.
    let mut angle = cross(ut, ur).atan2(dot(ut, ur));
    if angle > std::f64::consts::FRAC_PI_2 {
        angle -= std::f64::consts::PI;
    } else if angle <= -std::f64::consts::FRAC_PI_2 {
        angle += std::f64::consts::PI;
    }
    let pivot = add(target.a, scale(ut, dot(sub(p.target, target.a), ut)));
    let mut out = Modified::default();
    let objects = [target.owner];
    if angle.abs() > Tolerance::DEFAULT.angular {
        out = turn_or_move(ctx, &objects, &Xform::Rotate { base: pivot, angle })?;
    }
    let n = left(ur);
    let across = dot(sub(reference.a, pivot), n);
    if across.abs() > Tolerance::DEFAULT.linear {
        let moved = turn_or_move(ctx, &objects, &Xform::Translate(scale(n, across)))?;
        out.objects = out.objects.max(moved.objects);
        out.unjoined.extend(moved.unjoined);
        out.meets = moved.meets;
        out.reconnected = moved.reconnected.or(out.reconnected);
        out.dissociated_edges = moved.dissociated_edges.or(out.dissociated_edges);
    }
    Ok(kept(out, "reference", Json::from(p.reference.to_vec())))
}

fn turn_or_move(ctx: &mut Context<'_>, objects: &[ObjectId], xf: &Xform) -> Result<Modified> {
    let set = gather(ctx, Some(objects.iter().copied().map(IdParam).collect()))?;
    transform_in_place(ctx, &set, xf)
}

/// Parameters of `archbuilder.modify.offset`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Offset {
    /// The walls and floors; else the selection.
    #[serde(default)]
    pub objects: Option<Vec<IdParam>>,
    /// A point on the side to offset toward; also the distance when `distance` is absent.
    pub side: [f64; 2],
    /// The distance in meters; the distance of `side` from the nearest wall or floor edge when
    /// absent.
    #[serde(default)]
    pub distance: Option<f64>,
    /// Whether the originals stay and offset copies are made; true when absent.
    #[serde(default)]
    pub copy: Option<bool>,
    /// A prompt aid: the distance of the last offset, which Enter takes. The command ignores it.
    #[serde(default)]
    pub last_distance: Option<f64>,
}

/// The walls of `walls` joined end to end, as chains in order: each wall with whether it runs
/// along the chain, and whether the chain closes. A wall at a node of three or more is a chain of
/// its own.
fn chains(walls: &[(ObjectId, Wall)]) -> Vec<(Vec<(usize, bool)>, bool)> {
    let tol = archbuilder_model::join::JOIN_TOLERANCE;
    let near = |a: P, b: P| (a[0] - b[0]).hypot(a[1] - b[1]) <= tol;
    let end = |k: usize, e: usize| {
        walls
            .get(k)
            .map_or([0.0, 0.0], |(_, w)| if e == 0 { w.start } else { w.end })
    };
    // The other walls whose end meets end `e` of wall `k`.
    let meets = |k: usize, e: usize| -> Vec<(usize, usize)> {
        (0..walls.len())
            .filter(|j| *j != k)
            .flat_map(|j| [(j, 0), (j, 1)])
            .filter(|(j, f)| near(end(k, e), end(*j, *f)))
            .collect()
    };
    let mut used = vec![false; walls.len()];
    let mut out = Vec::new();
    for first in 0..walls.len() {
        if used[first] {
            continue;
        }
        used[first] = true;
        let mut chain = vec![(first, true)];
        let mut closed = false;
        // Forward from the end of the first wall, then backward from its start.
        for direction in [1_usize, 0] {
            let (mut k, mut e) = (first, direction);
            loop {
                let next = meets(k, e);
                let [(j, f)] = next.as_slice() else {
                    break;
                };
                if *j == first {
                    closed = true;
                    break;
                }
                if used[*j] || meets(*j, *f).len() != 1 {
                    break;
                }
                used[*j] = true;
                // Entering wall j at end f: it runs along the chain when entered at its start.
                let along = (*f == 0) == (direction == 1);
                if direction == 1 {
                    chain.push((*j, along));
                } else {
                    chain.insert(0, (*j, along));
                }
                k = *j;
                e = 1 - *f;
            }
            if closed {
                break;
            }
        }
        out.push((chain, closed));
    }
    out
}

/// A chain of an offset: its walls in order with their direction along it, whether it closes, its
/// signed distance and the new baselines.
type PlannedChain = (Vec<(usize, bool)>, bool, f64, Vec<Wall>);

/// What an offset of walls and floors would make: the walls and floors read, the distance, each
/// chain with its signed distance and the new baselines, and each floor's new vertices.
struct OffsetPlan {
    walls: Vec<(ObjectId, Wall)>,
    floors: Vec<(ObjectId, Floor, Vec<Vec<P>>)>,
    distance: f64,
    chains: Vec<PlannedChain>,
}

/// The plan of an offset of `wall_ids` and `floor_ids` toward `side` by `given` (else by the
/// distance of `side` from the nearest wall line or floor edge).
fn plan_offset(
    snapshot: &Snapshot,
    wall_ids: &[ObjectId],
    floor_ids: &[ObjectId],
    side: P,
    given: Option<f64>,
) -> Result<OffsetPlan> {
    let walls: Vec<(ObjectId, Wall)> = wall_ids
        .iter()
        .take(MAX_OBJECTS)
        .filter_map(|id| {
            spatial::get(snapshot, *id)
                .and_then(|o| Wall::from_values(&o.values))
                .map(|w| (*id, w))
        })
        .collect();
    if let Some((id, _)) = walls.iter().find(|(_, w)| w.is_arc()) {
        return Err(
            Error::new(codes::ARC_WALL_UNSUPPORTED).with_arg("name", name_of(snapshot, *id))
        );
    }
    let floors: Vec<(ObjectId, Floor, Vec<Vec<P>>)> = floor_ids
        .iter()
        .take(MAX_OBJECTS)
        .filter_map(|id| {
            let floor = spatial::get(snapshot, *id).and_then(|o| Floor::from_values(&o.values))?;
            let loops = floor.boundary_in(snapshot)?.loops;
            Some((*id, floor, loops))
        })
        .collect();
    let distance = match given {
        Some(d) if d.is_finite() && d > 0.0 && d <= 1000.0 => d,
        Some(_) => return Err(invalid()),
        None => {
            let to_walls = walls
                .iter()
                .filter_map(|(_, w)| direction(w).map(|u| dot(sub(side, w.start), left(u)).abs()));
            let to_floors = floors.iter().flat_map(|(_, _, loops)| {
                loops.iter().flat_map(|l| {
                    (0..l.len()).map(|k| segment_distance(side, l[k], l[(k + 1) % l.len()]))
                })
            });
            to_walls
                .chain(to_floors)
                .min_by(f64::total_cmp)
                .filter(|d| *d > Tolerance::DEFAULT.linear)
                .ok_or_else(|| Error::new(codes::NOTHING_TO_OFFSET))?
        }
    };
    let mut planned = Vec::new();
    for (chain, closed) in chains(&walls) {
        // The side: the left or the right of the chain at the wall nearest the point.
        let nearest = chain
            .iter()
            .filter_map(|(k, along)| walls.get(*k).map(|(_, w)| (w, *along)))
            .min_by(|a, b| wall_distance(a.0, side).total_cmp(&wall_distance(b.0, side)));
        let Some((w, along)) = nearest else {
            continue;
        };
        let u = direction(w).unwrap_or([1.0, 0.0]);
        let u = if along { u } else { scale(u, -1.0) };
        let d = if cross(u, sub(side, w.start)) >= 0.0 {
            distance
        } else {
            -distance
        };
        let moved = offset_chain(&walls, &chain, closed, d);
        planned.push((chain, closed, d, moved));
    }
    Ok(OffsetPlan {
        walls,
        floors,
        distance,
        chains: planned,
    })
}

/// Refuses to change the outline of the floor `id` (data `floor`) when it has arc edges: the
/// offset and the split of a wall under it cannot keep them.
fn refuse_arcs(snapshot: &arc_doc::Snapshot, id: ObjectId, floor: &Floor) -> Result<()> {
    if floor.vertices.iter().any(|v| v.bulge != 0.0) {
        let name = spatial::get(snapshot, id)
            .and_then(|o| types::text(&o.values, 0).map(str::to_owned))
            .unwrap_or_default();
        return Err(Error::new(codes::FLOOR_ARCS_UNSUPPORTED).with_arg("name", name));
    }
    Ok(())
}

/// The new floor vertices of an offset toward `side` by `distance`: the boundary grows when
/// `side` is outside it and shrinks inside.
fn offset_floor(loops: &[Vec<P>], side: P, distance: f64) -> Vec<Vertex> {
    let inside = archbuilder_model::floor::Boundary {
        loops: loops.to_vec(),
        ..archbuilder_model::floor::Boundary::default()
    }
    .contains(side);
    offset_loops(loops, if inside { -distance } else { distance })
}

/// The outlines an offset of `objects` toward `side` would leave, for a host's preview: the
/// footprints of the offset walls and the loops of the offset floors. Empty when nothing can be
/// offset.
#[must_use]
pub fn offset_outlines(
    snapshot: &Snapshot,
    objects: &[ObjectId],
    side: [f64; 2],
    distance: Option<f64>,
) -> Vec<Vec<[f64; 2]>> {
    let of = |key: &str| -> Vec<ObjectId> {
        objects
            .iter()
            .copied()
            .filter(|id| spatial::type_of(snapshot, *id).as_deref() == Some(key))
            .collect()
    };
    let Ok(plan) = plan_offset(snapshot, &of(WALL), &of(FLOOR), side, distance) else {
        return Vec::new();
    };
    let mut out: Vec<Vec<[f64; 2]>> = plan
        .chains
        .iter()
        .flat_map(|(_, _, _, moved)| moved.iter().map(|w| w.plain_footprint().to_vec()))
        .collect();
    for (_, _, loops) in &plan.floors {
        let vertices = offset_floor(loops, side, plan.distance);
        for k in 0..loops.len() {
            out.push(
                vertices
                    .iter()
                    .filter(|v| v.loop_index as usize == k)
                    .map(|v| v.point)
                    .collect(),
            );
        }
    }
    out
}

/// The baselines of a chain of walls offset by `d` toward the left of the chain's direction: the
/// lines move across and meet again at the corners.
fn offset_chain(
    walls: &[(ObjectId, Wall)],
    chain: &[(usize, bool)],
    closed: bool,
    d: f64,
) -> Vec<Wall> {
    // Each wall's line along the chain: its start, its end and its direction.
    let runs: Vec<(P, P, P, Wall)> = chain
        .iter()
        .filter_map(|(k, along)| {
            let (_, w) = walls.get(*k)?;
            let (a, b) = if *along {
                (w.start, w.end)
            } else {
                (w.end, w.start)
            };
            let u = unit(sub(b, a))?;
            let shift = scale(left(u), d);
            Some((add(a, shift), add(b, shift), u, *w))
        })
        .collect();
    let n = runs.len();
    let mut out = Vec::with_capacity(n);
    for (i, (a, b, u, w)) in runs.iter().enumerate() {
        let before = (i > 0 || closed).then(|| runs[(i + n - 1) % n]);
        let after = (i + 1 < n || closed).then(|| runs[(i + 1) % n]);
        let start = before
            .and_then(|(pa, _, pu, _)| meet(pa, pu, *a, *u))
            .unwrap_or(*a);
        let end = after
            .and_then(|(na, _, nu, _)| meet(*a, *u, na, nu))
            .unwrap_or(*b);
        let along = chain.get(i).is_none_or(|(_, along)| *along);
        out.push(if along {
            Wall { start, end, ..*w }
        } else {
            Wall {
                start: end,
                end: start,
                ..*w
            }
        });
    }
    out
}

/// The boundary of `floor` offset by `grow` (negative shrinks): each edge moves away from the
/// slab's material, and consecutive edges meet again.
fn offset_loops(loops: &[Vec<P>], grow: f64) -> Vec<Vertex> {
    let mut out = Vec::new();
    for (k, points) in loops.iter().enumerate() {
        let n = points.len();
        let area = signed_area(points);
        // The right of a counterclockwise outer loop is outside; a hole's material is outside it.
        let sign = if k == 0 {
            area.signum()
        } else {
            -area.signum()
        };
        let edge = |i: usize| -> Option<(P, P)> {
            let a = *points.get(i % n)?;
            let b = *points.get((i + 1) % n)?;
            let u = unit(sub(b, a))?;
            let normal = scale([u[1], -u[0]], sign * grow);
            Some((add(a, normal), u))
        };
        for (i, own) in points.iter().enumerate() {
            let at = match (edge(i + n - 1), edge(i)) {
                (Some((pa, pu)), Some((a, u))) => meet(pa, pu, a, u).unwrap_or(a),
                (_, Some((a, _))) => a,
                _ => *own,
            };
            out.push(Vertex {
                loop_index: u32::try_from(k).unwrap_or(u32::MAX),
                point: at,
                edge: None,
                bulge: 0.0,
            });
        }
    }
    out
}

#[allow(
    clippy::too_many_lines,
    reason = "walls, then floors, each moved or copied"
)]
fn offset(ctx: &mut Context<'_>, p: Offset) -> Result<Edited> {
    finite(&[p.side])?;
    let copy = p.copy.unwrap_or(true);
    let set = gather(ctx, p.objects)?;
    if set.walls.is_empty() && set.floors.is_empty() {
        return Err(Error::new(codes::NOTHING_TO_OFFSET));
    }
    let snapshot = ctx.snapshot().clone();
    let plan = plan_offset(&snapshot, &set.walls, &set.floors, p.side, p.distance)?;
    let (walls, floors, distance) = (&plan.walls, &plan.floors, plan.distance);
    let tongue = language(ctx);
    let mut out = Modified::default();
    let mut created = Vec::new();
    for (chain, closed, _, moved) in &plan.chains {
        let closed = *closed;
        out.objects += moved.len();
        for ((k, _), new) in chain.iter().zip(moved) {
            let Some((id, old)) = walls.get(*k) else {
                continue;
            };
            if copy {
                let object =
                    spatial::get(&snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
                let storey = object.header.owner;
                let name = match storey {
                    Some(s) => {
                        let seen = ctx.txn()?.snapshot().clone();
                        crate::wall::next_wall_name(&seen, s, &tongue)
                    }
                    None => return Err(Error::new(codes::KIND_MISMATCH)),
                };
                let mut values = object.values.clone();
                for (at, value) in [
                    (types::wall::NAME, Value::String(name)),
                    (types::wall::START_X, types::length(new.start[0])),
                    (types::wall::START_Y, types::length(new.start[1])),
                    (types::wall::END_X, types::length(new.end[0])),
                    (types::wall::END_Y, types::length(new.end[1])),
                ] {
                    if let Some(slot) = values.get_mut(at) {
                        *slot = Some(value);
                    }
                }
                let wall = create(ctx, WALL, storey, values)?;
                created.push(wall);
                // The copy's doors and windows stand where the originals do, moved across.
                // Only the shift along the wall counts; the move across does not.
                let shift = start_shift(old, new);
                for (host, item) in insert::hosted(&snapshot, *id) {
                    created.push(copy_insert(
                        ctx,
                        &host,
                        wall,
                        item.distance - shift,
                        None,
                        &tongue,
                    )?);
                }
            } else if chain.len() == 1 && !closed {
                // A wall alone moves as a move does: its neighbors keep their joins.
                let moved = transform_in_place(
                    ctx,
                    &Set {
                        walls: vec![*id],
                        ..Set::default()
                    },
                    &Xform::Translate(sub(new.start, old.start)),
                )?;
                out.unjoined.extend(moved.unjoined);
                out.reconnected = moved.reconnected.or(out.reconnected);
            } else {
                reshape(ctx, *id, new)?;
            }
        }
    }
    for (id, floor, loops) in floors {
        refuse_arcs(&snapshot, *id, floor)?;
        let vertices = offset_floor(loops, p.side, distance);
        out.objects += 1;
        let lost = floor.vertices.iter().filter(|v| v.edge.is_some()).count();
        if copy {
            let object =
                spatial::get(&snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
            let storey = object.header.owner;
            let name = match storey {
                Some(s) => {
                    let seen = ctx.txn()?.snapshot().clone();
                    crate::floor::next_floor_name(&seen, s, &tongue)
                }
                None => return Err(Error::new(codes::KIND_MISMATCH)),
            };
            let mut values = object.values.clone();
            if let Some(slot) = values.get_mut(types::floor::NAME) {
                *slot = Some(Value::String(name));
            }
            if let Some(slot) = values.get_mut(types::floor::VERTICES) {
                *slot = Some(Floor::vertex_values(&vertices));
            }
            if let Some(slot) = values.get_mut(types::floor::ARCS) {
                *slot = Floor::arc_values(&vertices);
            }
            created.push(create(ctx, FLOOR, storey, values)?);
        } else {
            out.dissociated_edges =
                Some(out.dissociated_edges.unwrap_or(0) + lost).filter(|n| *n > 0);
            crate::floor::set_vertices(ctx, *id, &vertices)?;
        }
    }
    out.meets = ensure_valid_lenient(ctx)?;
    if !created.is_empty() {
        ctx.set_selection_after(created.clone());
    }
    out.created = created.into_iter().map(IdParam).collect();
    let mut edited = kept(out, "copy", Json::from(copy));
    if let Some(Json::Object(keep)) = &mut edited.keep {
        keep.insert("last_distance".to_owned(), Json::from(distance));
    }
    Ok(edited)
}

/// Parameters of `archbuilder.modify.trim`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrimCorner {
    /// A point on the part of the first wall that stays.
    pub first: [f64; 2],
    /// A point on the part of the second wall that stays.
    pub other: [f64; 2],
    /// The storey the picks look in; any storey when absent.
    #[serde(default)]
    pub storey: Option<IdParam>,
    /// The pick radius in meters; 0.3 when absent.
    #[serde(default)]
    pub radius: Option<f64>,
}

fn trim_corner(ctx: &mut Context<'_>, p: TrimCorner) -> Result<Edited> {
    finite(&[p.first, p.other])?;
    let radius = radius_of(p.radius)?;
    let storey = p.storey.map(|IdParam(id)| id);
    let snapshot = ctx.snapshot().clone();
    let (a, wa) = wall_at(&snapshot, storey, p.first, radius)?;
    let (b, wb) = wall_at(&snapshot, storey, p.other, radius)?;
    if a == b {
        return Err(Error::new(codes::SAME_WALL));
    }
    let (Some(ua), Some(ub)) = (direction(&wa), direction(&wb)) else {
        return Err(Error::new(codes::NO_WALL_AT));
    };
    let corner = meet(wa.start, ua, wb.start, ub).ok_or_else(|| Error::new(codes::PARALLEL))?;
    reshape(ctx, a, &keep_side(&wa, corner, p.first))?;
    reshape(ctx, b, &keep_side(&wb, corner, p.other))?;
    let meets = ensure_valid_lenient(ctx)?;
    Ok(Edited {
        modified: Modified {
            objects: 2,
            meets,
            ..Modified::default()
        },
        keep: None,
    })
}

/// Parameters of `archbuilder.modify.trim_to` and `archbuilder.modify.trim_many`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrimTo {
    /// A point on the boundary line.
    pub boundary: [f64; 2],
    /// A point on the part of each wall that stays.
    pub targets: Vec<[f64; 2]>,
    /// The storey the picks look in; any storey when absent.
    #[serde(default)]
    pub storey: Option<IdParam>,
    /// The pick radius in meters; 0.3 when absent.
    #[serde(default)]
    pub radius: Option<f64>,
}

fn trim_to(ctx: &mut Context<'_>, p: TrimTo) -> Result<Edited> {
    if p.targets.is_empty() || p.targets.len() > MAX_TARGETS {
        return Err(invalid());
    }
    finite(&[p.boundary])?;
    finite(&p.targets)?;
    let radius = radius_of(p.radius)?;
    let storey = p.storey.map(|IdParam(id)| id);
    let snapshot = ctx.snapshot().clone();
    let line = line_at(&snapshot, storey, p.boundary, radius, None)?;
    let v = unit(sub(line.b, line.a)).ok_or_else(|| Error::new(codes::NO_LINE_AT))?;
    let mut count = 0;
    for target in &p.targets {
        // Each pick reads the walls as the earlier picks left them.
        let seen = ctx.txn()?.snapshot().clone();
        let (id, w) = wall_at(&seen, storey, *target, radius)?;
        if id == line.owner {
            return Err(Error::new(codes::SAME_WALL));
        }
        let u = direction(&w).ok_or_else(|| Error::new(codes::NO_WALL_AT))?;
        let x = meet(w.start, u, line.a, v).ok_or_else(|| Error::new(codes::PARALLEL))?;
        reshape(ctx, id, &keep_side(&w, x, *target))?;
        count += 1;
    }
    let meets = ensure_valid_lenient(ctx)?;
    Ok(kept(
        Modified {
            objects: count,
            meets,
            ..Modified::default()
        },
        "boundary",
        Json::from(p.boundary.to_vec()),
    ))
}

/// Parameters of `archbuilder.wall.split`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Split {
    /// A point on the wall: where it splits, or the end the distance is measured from.
    pub at: [f64; 2],
    /// The distance of the split from the wall's end nearest `at`, in meters; the point itself
    /// when absent.
    #[serde(default)]
    pub distance: Option<f64>,
    /// The width of the gap between the two pieces, in meters; none when absent.
    #[serde(default)]
    pub gap: Option<f64>,
    /// The storey the pick looks in; any storey when absent.
    #[serde(default)]
    pub storey: Option<IdParam>,
    /// The pick radius in meters; 0.3 when absent.
    #[serde(default)]
    pub radius: Option<f64>,
}

#[allow(
    clippy::too_many_lines,
    reason = "the checks, the pieces, the openings and the floor edges"
)]
fn split(ctx: &mut Context<'_>, p: Split) -> Result<Edited> {
    finite(&[p.at])?;
    let radius = radius_of(p.radius)?;
    let storey = p.storey.map(|IdParam(id)| id);
    let gap = match p.gap {
        None => 0.0,
        Some(g) if g.is_finite() && (0.0..=1000.0).contains(&g) => g,
        Some(_) => return Err(invalid()),
    };
    let snapshot = ctx.snapshot().clone();
    let (id, w) = wall_at(&snapshot, storey, p.at, radius)?;
    let u = direction(&w).ok_or_else(|| Error::new(codes::NO_WALL_AT))?;
    let length = w.length();
    let picked = dot(sub(p.at, w.start), u);
    let at = match p.distance {
        Some(d) if d.is_finite() && d >= 0.0 => {
            if picked <= length / 2.0 {
                d
            } else {
                length - d
            }
        }
        Some(_) => return Err(invalid()),
        None => picked.clamp(0.0, length),
    };
    let (cut_low, cut_high) = (at - gap / 2.0, at + gap / 2.0);
    if cut_low < types::MIN_WALL_LENGTH || length - cut_high < types::MIN_WALL_LENGTH {
        return Err(Error::new(codes::SPLIT_END).with_arg("name", name_of(&snapshot, id)));
    }
    let hosted = insert::hosted(&snapshot, id);
    // An opening across the cut cannot go to either piece.
    // The same room an opening needs at a wall's end (the fit rule's side clearance).
    let clear = insert::SIDE_CLEARANCE;
    if let Some((object, _)) = hosted.iter().find(|(_, item)| {
        let (low, high) = item.extent();
        high + clear > cut_low && low - clear < cut_high
    }) {
        return Err(Error::new(codes::SPLIT_INSERT)
            .with_arg(
                "name",
                types::text(&object.values, 0)
                    .unwrap_or_default()
                    .to_owned(),
            )
            .with_arg("wall", name_of(&snapshot, id)));
    }
    let point = |s: f64| add(w.start, scale(u, s));
    let first = Wall {
        end: point(cut_low),
        end_join: JoinStyle::Auto,
        ..w
    };
    let object = spatial::get(&snapshot, id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let storey_id = object.header.owner;
    let tongue = language(ctx);
    let name = match storey_id {
        Some(s) => crate::wall::next_wall_name(&snapshot, s, &tongue),
        None => return Err(Error::new(codes::KIND_MISMATCH)),
    };
    let second_start = point(cut_high);
    let mut values = object.values.clone();
    for (at, value) in [
        (types::wall::NAME, Value::String(name)),
        (types::wall::START_X, types::length(second_start[0])),
        (types::wall::START_Y, types::length(second_start[1])),
        (types::wall::START_JOIN, Value::Enum(JoinStyle::Auto.code())),
    ] {
        if let Some(slot) = values.get_mut(at) {
            *slot = Some(value);
        }
    }
    // The openings past the cut move to the new piece first, so the first piece holds only its own.
    let second = create(ctx, WALL, storey_id, values)?;
    for (object, item) in &hosted {
        if item.distance > at {
            let txn = ctx.txn()?;
            txn.relink(object.id, Some(second))?;
            set_length(ctx, object.id, "distance", item.distance - cut_high)?;
        }
    }
    reshape(ctx, id, &first)?;
    // A curtain wall's grid edits and panel overrides go to the piece they belong to.
    let mut unkept = Vec::new();
    if let Some(([one, two], lost)) = crate::curtain::split_carry(&snapshot, id, cut_low, cut_high)
    {
        crate::curtain::write_carried(ctx, id, &one)?;
        crate::curtain::write_carried(ctx, second, &two)?;
        unkept = lost;
    }
    // Floor edges that follow the wall follow the piece they lie along; an edge across the cut
    // is cut there too, so the outline stays where it is.
    for floor in spatial::children(&snapshot, FLOOR, storey_id.unwrap_or(id)) {
        let Some(data) = Floor::from_values(&floor.values) else {
            continue;
        };
        let Some(boundary) = data.boundary_in(&snapshot) else {
            continue;
        };
        let cut = SplitCut {
            wall: id,
            second,
            start: w.start,
            u,
            low: cut_low,
            high: cut_high,
        };
        if let Some(vertices) = cut.floor_vertices(&data, &boundary) {
            refuse_arcs(&snapshot, floor.id, &data)?;
            crate::floor::set_vertices(ctx, floor.id, &vertices)?;
        }
    }
    let meets = ensure_valid_lenient(ctx)?;
    ctx.set_selection_after(vec![id, second]);
    Ok(kept(
        Modified {
            objects: 2,
            created: vec![IdParam(second)],
            meets,
            unkept,
            ..Modified::default()
        },
        "gap",
        Json::from(gap),
    ))
}

/// Where a split cuts a wall: the wall that keeps the first piece, the new second piece, the
/// baseline's start and direction, and the stretch the cut takes out of it.
struct SplitCut {
    wall: ObjectId,
    second: ObjectId,
    start: P,
    u: P,
    low: f64,
    high: f64,
}

impl SplitCut {
    /// The vertices of `floor` (whose derived boundary is `boundary`) with every edge that followed
    /// the split wall following the piece it lies along; an edge across the cut gets a vertex at
    /// the cut (two with a gap between, the edge across the gap following no wall). `None` when no
    /// edge followed the wall.
    fn floor_vertices(
        &self,
        floor: &Floor,
        boundary: &archbuilder_model::floor::Boundary,
    ) -> Option<Vec<Vertex>> {
        use archbuilder_model::floor::Association;
        let along = |p: P| dot(sub(p, self.start), self.u);
        let at = |s: f64| add(self.start, scale(self.u, s));
        let mid = (self.low + self.high) / 2.0;
        let mut out = Vec::with_capacity(floor.vertices.len() + 2);
        let (mut current, mut k, mut touched) = (u32::MAX, 0_usize, false);
        for v in &floor.vertices {
            if v.loop_index != current {
                current = v.loop_index;
                k = 0;
            }
            let ring = boundary.loops.get(v.loop_index as usize);
            let ends = ring.and_then(|l| Some((*l.get(k)?, *l.get((k + 1) % l.len())?)));
            k += 1;
            let (Some(e), Some((p, q))) = (v.edge.filter(|e| e.wall == self.wall), ends) else {
                out.push(*v);
                continue;
            };
            touched = true;
            let (sp, sq) = (along(p), along(q));
            let piece = |s: f64| if s < mid { self.wall } else { self.second };
            let follow = |wall: ObjectId| Some(Association { wall, ..e });
            if (sp < mid) == (sq < mid) {
                out.push(Vertex {
                    edge: follow(piece((sp + sq) / 2.0)),
                    ..*v
                });
                continue;
            }
            // Across the cut: the part before it, the gap (when there is one), the part after.
            let (near, far) = if sp < sq {
                (self.low, self.high)
            } else {
                (self.high, self.low)
            };
            out.push(Vertex {
                edge: follow(piece(sp)),
                bulge: 0.0,
                ..*v
            });
            let gap = (self.high - self.low) > MIN_FLOOR_GAP;
            if gap {
                out.push(Vertex {
                    loop_index: v.loop_index,
                    point: at(near),
                    edge: None,
                    bulge: 0.0,
                });
            }
            out.push(Vertex {
                loop_index: v.loop_index,
                point: at(if gap { far } else { mid }),
                edge: follow(piece(sq)),
                bulge: 0.0,
            });
        }
        touched.then_some(out)
    }
}

/// The narrowest split gap a floor edge across it is cut twice for; a narrower one keeps a single
/// vertex at the middle of the gap.
const MIN_FLOOR_GAP: f64 = 0.01;

/// `vertices` without a vertex between two edges that follow the same wall face the same way (what
/// a merge leaves where a split had cut an edge); a loop keeps at least three vertices.
fn without_repeats(vertices: &[Vertex]) -> Vec<Vertex> {
    let mut out = Vec::with_capacity(vertices.len());
    let mut start = 0;
    while start < vertices.len() {
        let index = vertices[start].loop_index;
        let end = vertices[start..]
            .iter()
            .position(|v| v.loop_index != index)
            .map_or(vertices.len(), |n| start + n);
        let ring = &vertices[start..end];
        let n = ring.len();
        let mut kept: Vec<Vertex> = ring
            .iter()
            .enumerate()
            .filter(|(k, v)| {
                let before = ring[(k + n - 1) % n].edge;
                v.edge.is_none() || before != v.edge
            })
            .map(|(_, v)| *v)
            .collect();
        if kept.len() < 3 {
            kept = ring.to_vec();
        }
        out.extend(kept);
        start = end;
    }
    out
}

/// Parameters of `archbuilder.wall.merge`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Merge {
    /// A point on the wall that stays.
    pub first: [f64; 2],
    /// A point on the wall merged into it.
    pub other: [f64; 2],
    /// The storey the picks look in; any storey when absent.
    #[serde(default)]
    pub storey: Option<IdParam>,
    /// The pick radius in meters; 0.3 when absent.
    #[serde(default)]
    pub radius: Option<f64>,
}

/// Whether the walls `a` and `b` (records) can merge: the same storey, type, height, base offset,
/// top and location line.
fn mergeable(a: &arc_doc::Object, b: &arc_doc::Object) -> bool {
    use types::wall::{BASE_OFFSET, HEIGHT, LOCATION_LINE, THICKNESS, TOP_LEVEL, WALL_TYPE};
    a.header.owner == b.header.owner
        && [
            WALL_TYPE,
            HEIGHT,
            BASE_OFFSET,
            TOP_LEVEL,
            LOCATION_LINE,
            THICKNESS,
        ]
        .iter()
        .all(|k| a.values.get(*k) == b.values.get(*k))
}

#[allow(
    clippy::too_many_lines,
    reason = "the checks, then the wall, its openings and floors"
)]
fn merge(ctx: &mut Context<'_>, p: Merge) -> Result<Edited> {
    finite(&[p.first, p.other])?;
    let radius = radius_of(p.radius)?;
    let storey = p.storey.map(|IdParam(id)| id);
    let snapshot = ctx.snapshot().clone();
    let (ia, a) = wall_at(&snapshot, storey, p.first, radius)?;
    let (ib, b) = wall_at(&snapshot, storey, p.other, radius)?;
    if ia == ib {
        return Err(Error::new(codes::SAME_WALL));
    }
    let refused = || {
        Error::new(codes::MERGE_REFUSED)
            .with_arg("name", name_of(&snapshot, ia))
            .with_arg("other", name_of(&snapshot, ib))
    };
    let (oa, ob) = (
        spatial::get(&snapshot, ia).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?,
        spatial::get(&snapshot, ib).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?,
    );
    let (Some(ua), Some(ub)) = (direction(&a), direction(&b)) else {
        return Err(refused());
    };
    let tol = archbuilder_model::join::JOIN_TOLERANCE;
    let on_line = |q: P| dot(sub(q, a.start), left(ua)).abs() <= tol;
    if !mergeable(&oa, &ob) || !on_line(b.start) || !on_line(b.end) {
        return Err(refused());
    }
    let same = dot(ua, ub) > 0.0;
    // A wall drawn the other way must have its body on the same side to become one.
    let (low, high) = a.extent();
    let (blow, bhigh) = if same {
        b.extent()
    } else {
        let (l, h) = b.extent();
        (-h, -l)
    };
    if (low - blow).abs() > tol || (high - bhigh).abs() > tol {
        return Err(refused());
    }
    let near = |p: P, q: P| (p[0] - q[0]).hypot(p[1] - q[1]) <= tol;
    // The shared ends: `ea` of a and `eb` of b.
    let shared = [(1, 0), (1, 1), (0, 0), (0, 1)]
        .into_iter()
        .find(|(ea, eb)| {
            let pa = if *ea == 0 { a.start } else { a.end };
            let pb = if *eb == 0 { b.start } else { b.end };
            near(pa, pb)
        });
    let Some((ea, eb)) = shared else {
        return Err(refused());
    };
    let (far, far_join) = if eb == 0 {
        (b.end, b.end_join)
    } else {
        (b.start, b.start_join)
    };
    let (la, lb) = (a.length(), b.length());
    let merged = if ea == 1 {
        Wall {
            end: far,
            end_join: far_join,
            ..a
        }
    } else {
        Wall {
            start: far,
            start_join: far_join,
            ..a
        }
    };
    // The openings of b, by their place along the merged wall.
    let place = |d: f64| -> f64 {
        let from_shared = if eb == 0 { d } else { lb - d };
        if ea == 1 {
            la + from_shared
        } else {
            lb - from_shared
        }
    };
    let moved: Vec<(ObjectId, f64, Option<u32>)> = insert::hosted(&snapshot, ib)
        .into_iter()
        .map(|(o, item)| {
            let facing = (!same).then(|| item.facing.flipped().code());
            (o.id, place(item.distance), facing)
        })
        .collect();
    for (item, _, _) in &moved {
        ctx.txn()?.relink(*item, Some(ia))?;
    }
    // A's own openings keep their place: its start moves only when b comes before it.
    reshape(ctx, ia, &merged)?;
    // The grid edits and panel overrides of curtain walls go to the merged wall.
    let mut unkept = Vec::new();
    if let Some((carried, lost)) =
        crate::curtain::merge_carry(&snapshot, ia, ib, (u8::from(ea == 1), u8::from(eb == 1)))
    {
        crate::curtain::write_carried(ctx, ia, &carried)?;
        unkept = lost;
    }
    for (item, distance, facing) in moved {
        set_length(ctx, item, "distance", distance)?;
        if let Some(code) = facing {
            ctx.txn()?
                .modify(item, FieldPath::new("facing"), Some(Value::Enum(code)))?;
        }
    }
    // Floor edges that followed b follow the merged wall.
    for floor in spatial::children(&snapshot, FLOOR, oa.header.owner.unwrap_or(ia)) {
        let Some(data) = Floor::from_values(&floor.values) else {
            continue;
        };
        if !data
            .vertices
            .iter()
            .any(|v| v.edge.is_some_and(|e| e.wall == ib))
        {
            continue;
        }
        let vertices: Vec<Vertex> = data
            .vertices
            .iter()
            .map(|v| Vertex {
                edge: v.edge.map(|e| {
                    if e.wall == ib {
                        archbuilder_model::floor::Association {
                            wall: ia,
                            offset: if same { e.offset } else { -e.offset },
                            ..e
                        }
                    } else {
                        e
                    }
                }),
                ..*v
            })
            .collect();
        crate::floor::set_vertices(ctx, floor.id, &without_repeats(&vertices))?;
    }
    let limits = *ctx.limits();
    let txn = ctx.txn()?;
    txn.delete(ib)?;
    txn.apply_delete_rules(&limits)?;
    let meets = ensure_valid_lenient(ctx)?;
    ctx.set_selection_after(vec![ia]);
    Ok(Edited {
        modified: Modified {
            objects: 2,
            meets,
            unkept,
            ..Modified::default()
        },
        keep: None,
    })
}

/// Parameters of `archbuilder.modify.array`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LinearArray {
    /// The objects; else the selection.
    #[serde(default)]
    pub objects: Option<Vec<IdParam>>,
    /// The number of items, the original included: 2 to 1000.
    pub count: u32,
    /// The displacement `[dx, dy]` in meters to the second item, or to the last with `last`.
    pub displacement: [f64; 2],
    /// Whether the displacement reaches the last item (the spacing is the displacement divided by
    /// `count - 1`); false when absent.
    #[serde(default)]
    pub last: Option<bool>,
    /// A prompt aid: the base point. The command ignores it.
    #[serde(default)]
    pub base: Option<[f64; 2]>,
}

/// Parameters of `archbuilder.modify.array_radial`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RadialArray {
    /// The objects; else the selection.
    #[serde(default)]
    pub objects: Option<Vec<IdParam>>,
    /// The number of items, the original included: 2 to 1000.
    pub count: u32,
    /// The center `[x, y]` in meters.
    pub center: [f64; 2],
    /// The angle the items cover, counterclockwise: a full turn spaces them evenly around the
    /// center, a smaller angle puts the last item at it. A JSON number is radians, a text with a
    /// unit says its own, a bare text number is degrees; a full turn when absent.
    #[serde(default)]
    pub angle: Option<QuantityParam>,
}

/// The copies of `set` under each of `steps`, all in one transaction, the copies selected after.
fn array_copies(ctx: &mut Context<'_>, set: &Set, steps: &[Xform]) -> Result<Edited> {
    let per =
        set.walls.len() + set.floors.len() + set.rooms.len() + set.stairs.len() + set.inserts.len();
    if per.saturating_mul(steps.len()) > MAX_OBJECTS {
        return Err(invalid());
    }
    let mut out = Modified::default();
    for xf in steps {
        if !xf.is_finite() {
            return Err(invalid());
        }
        let made = copy_set(ctx, set, xf, None)?;
        out.objects += made.objects;
        out.created.extend(made.created);
        out.meets.extend(made.meets);
    }
    ctx.set_selection_after(out.created.iter().map(|IdParam(id)| *id).collect());
    Ok(Edited {
        modified: out,
        keep: None,
    })
}

fn count_of(count: u32) -> Result<u32> {
    if (2..=MAX_COUNT).contains(&count) {
        Ok(count)
    } else {
        Err(invalid())
    }
}

fn linear_array(ctx: &mut Context<'_>, p: LinearArray) -> Result<Edited> {
    let count = count_of(p.count)?;
    finite(&[p.displacement])?;
    let step = if p.last.unwrap_or(false) {
        scale(p.displacement, 1.0 / f64::from(count - 1))
    } else {
        p.displacement
    };
    let set = gather(ctx, p.objects)?;
    // A door or window alone stays in its wall: the step is projected onto the wall (as a copy
    // of it slides), and a step across the wall makes no row.
    let snapshot = ctx.snapshot().clone();
    for id in &set.inserts {
        let host = spatial::get(&snapshot, *id)
            .and_then(|o| o.header.owner)
            .and_then(|w| spatial::get(&snapshot, w))
            .and_then(|w| Wall::from_values(&w.values))
            .and_then(|w| direction(&w));
        let along = host.map_or(0.0, |u| dot(u, step).abs());
        if along <= types::MIN_WALL_LENGTH {
            return Err(Error::new(codes::ARRAY_INSERT).with_arg("name", name_of(&snapshot, *id)));
        }
    }
    let steps: Vec<Xform> = (1..count)
        .map(|k| Xform::Translate(scale(step, f64::from(k))))
        .collect();
    array_copies(ctx, &set, &steps)
}

fn radial_array(ctx: &mut Context<'_>, p: RadialArray) -> Result<Edited> {
    let count = count_of(p.count)?;
    finite(&[p.center])?;
    let full = std::f64::consts::TAU;
    let angle = match &p.angle {
        None => full,
        Some(other) => crate::angle(ctx, other)?,
    };
    if !angle.is_finite() || angle == 0.0 || angle.abs() > full + Tolerance::DEFAULT.angular {
        return Err(invalid());
    }
    let set = gather(ctx, p.objects)?;
    if let Some(id) = set.inserts.first() {
        let snapshot = ctx.snapshot().clone();
        return Err(Error::new(codes::ARRAY_INSERT).with_arg("name", name_of(&snapshot, *id)));
    }
    // A full turn spaces the items evenly; a smaller angle ends at the last item.
    let step = if (angle.abs() - full).abs() <= 1e-9 {
        angle / f64::from(count)
    } else {
        angle / f64::from(count - 1)
    };
    let steps: Vec<Xform> = (1..count)
        .map(|k| Xform::Rotate {
            base: p.center,
            angle: step * f64::from(k),
        })
        .collect();
    array_copies(ctx, &set, &steps)
}

/// Parameters of `archbuilder.element.pin` and `archbuilder.element.unpin`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Pin {
    /// The elements; else the selection.
    #[serde(default)]
    pub objects: Option<Vec<IdParam>>,
}

/// What pin and unpin return.
#[derive(Debug, Default, Serialize, JsonSchema)]
pub struct Pinned {
    /// The elements whose flag changed.
    pub changed: usize,
}

fn set_pins(ctx: &mut Context<'_>, p: Pin, on: bool) -> Result<Pinned> {
    let ids: Vec<ObjectId> = match p.objects {
        Some(list) if list.len() > MAX_OBJECTS => return Err(invalid()),
        Some(list) => list.into_iter().map(|IdParam(id)| id).collect(),
        None => ctx.selection().iter().take(MAX_OBJECTS).copied().collect(),
    };
    let snapshot = ctx.snapshot().clone();
    let elements: Vec<(ObjectId, String)> = ids
        .into_iter()
        .filter_map(|id| {
            let key = spatial::type_of(&snapshot, id)?;
            types::pinned_slot(&key).map(|_| (id, key))
        })
        .collect();
    if elements.is_empty() {
        return Err(Error::new(codes::NOTHING_SELECTED));
    }
    let mut changed = 0;
    for (id, key) in elements {
        let Some(object) = spatial::get(&snapshot, id) else {
            continue;
        };
        if types::is_pinned(&key, &object.values) != on {
            ctx.txn()?.modify(
                id,
                FieldPath::new("pinned"),
                on.then_some(Value::Bool(true)),
            )?;
            changed += 1;
        }
    }
    crate::structure::ensure_valid(ctx)?;
    Ok(Pinned { changed })
}

/// Which editing command a prompt machine asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    Align,
    Offset,
    Trim,
    TrimTo,
    TrimMany,
    Split,
    Merge,
    Array,
    Radial,
}

/// The prompts of the editing commands.
#[derive(Debug)]
pub struct EditPrompts {
    which: Which,
}

fn point_accept() -> Accept {
    Accept::Point {
        dims: PointDims::Two,
    }
}

fn ask_point(key: String, message: &str) -> Step {
    Step::Ask(Prompt::new(key, message, point_accept()))
}

fn xy(p: &Point) -> Json {
    Json::from(vec![p.x, p.y])
}

fn points(state: &PromptState<'_>) -> Vec<Point> {
    state
        .answers
        .iter()
        .filter_map(|a| match &a.input {
            PromptInput::Point(p) => Some(*p),
            _ => None,
        })
        .collect()
}

fn done(pairs: Vec<(&str, Json)>) -> Step {
    Step::Done(pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}

impl EditPrompts {
    /// A reference (align) or a boundary (trim to, trim many), kept from the last run when preset,
    /// then the targets.
    fn picked_line(&self, state: &PromptState<'_>) -> Step {
        let (name, first_message) = if self.which == Which::Align {
            ("reference", PROMPT_ALIGN_REFERENCE.message_id)
        } else {
            ("boundary", PROMPT_TRIM_BOUNDARY.message_id)
        };
        let mut picks = points(state);
        let line = match state.preset.get(name) {
            Some(v) => v.clone(),
            None => {
                if picks.is_empty() {
                    return ask_point(format!("{name}-0"), first_message);
                }
                xy(&picks.remove(0))
            }
        };
        let at = state.answers.len();
        let target_message = match self.which {
            Which::Align => PROMPT_ALIGN_TARGET.message_id,
            Which::TrimTo => PROMPT_TRIM_TARGET.message_id,
            _ => PROMPT_TRIM_TARGETS.message_id,
        };
        match self.which {
            Which::Align => match picks.first() {
                Some(p) => done(vec![(name, line), ("target", xy(p))]),
                None => ask_point(format!("target-{at}"), target_message),
            },
            Which::TrimTo => match picks.first() {
                Some(p) => done(vec![(name, line), ("targets", Json::from(vec![xy(p)]))]),
                None => ask_point(format!("target-{at}"), target_message),
            },
            _ => {
                let finished = matches!(
                    state.answers.last().map(|a| &a.input),
                    Some(PromptInput::Keyword(_))
                );
                if finished && !picks.is_empty() {
                    return done(vec![
                        (name, line),
                        (
                            "targets",
                            Json::from(picks.iter().map(xy).collect::<Vec<_>>()),
                        ),
                    ]);
                }
                let prompt = Prompt::new(format!("target-{at}"), target_message, point_accept());
                // Enter ends the picks once there is one.
                Step::Ask(if picks.is_empty() {
                    prompt
                } else {
                    prompt
                        .or(Accept::keywords([KeywordOption::new(
                            "done",
                            KEYWORD_DONE.message_id,
                        )
                        .with_shortcut('d')]))
                        .with_default(PromptInput::Keyword("done".to_owned()))
                })
            }
        }
    }

    fn two_walls(&self, state: &PromptState<'_>) -> Step {
        let (first, second) = if self.which == Which::Trim {
            (PROMPT_TRIM_FIRST.message_id, PROMPT_TRIM_OTHER.message_id)
        } else {
            (PROMPT_MERGE_FIRST.message_id, PROMPT_MERGE_OTHER.message_id)
        };
        match points(state).as_slice() {
            [] => ask_point("first-0".to_owned(), first),
            [_] => ask_point("other-1".to_owned(), second),
            [a, b, ..] => done(vec![("first", xy(a)), ("other", xy(b))]),
        }
    }

    fn offset(state: &PromptState<'_>) -> Step {
        let mut copy = state
            .preset
            .get("copy")
            .and_then(Json::as_bool)
            .unwrap_or(true);
        let mut distance = None;
        for answer in state.answers {
            match &answer.input {
                PromptInput::Keyword(k) => copy = k == "copy",
                PromptInput::Distance(d) => distance = Some(*d),
                PromptInput::Point(p) => {
                    let mut out = vec![("side", xy(p)), ("copy", Json::from(copy))];
                    if let Some(d) = distance {
                        out.push(("distance", Json::from(d)));
                    }
                    return done(out);
                }
                _ => {}
            }
        }
        let at = state.answers.len();
        if distance.is_some() {
            return ask_point(format!("side-{at}"), PROMPT_OFFSET_SIDE.message_id);
        }
        let toggle = if copy {
            KeywordOption::new("move", KEYWORD_OFFSET_MOVE.message_id).with_shortcut('m')
        } else {
            KeywordOption::new("copy", KEYWORD_OFFSET_COPY.message_id).with_shortcut('c')
        };
        let prompt = Prompt::new(
            format!("distance-{at}"),
            PROMPT_OFFSET_DISTANCE.message_id,
            Accept::Distance {
                min: Some(0.001),
                max: Some(1000.0),
            },
        )
        .or(point_accept())
        .or(Accept::keywords([toggle]));
        // The next offset of a run offers the last distance: Enter takes it.
        Step::Ask(
            match state
                .preset
                .get("last_distance")
                .and_then(Json::as_f64)
                .filter(|d| d.is_finite() && *d > 0.0)
            {
                Some(d) => prompt.with_default(PromptInput::Distance(d)),
                None => prompt,
            },
        )
    }

    fn split(state: &PromptState<'_>) -> Step {
        let mut gap = state
            .preset
            .get("gap")
            .and_then(Json::as_f64)
            .unwrap_or(0.0);
        let mut asking_gap = false;
        let mut distance = None;
        for answer in state.answers {
            match &answer.input {
                PromptInput::Keyword(_) => asking_gap = true,
                PromptInput::Distance(d) if asking_gap => {
                    gap = *d;
                    asking_gap = false;
                }
                PromptInput::Distance(d) => distance = Some(*d),
                PromptInput::Point(p) => {
                    let mut out = vec![("at", xy(p)), ("gap", Json::from(gap))];
                    if let Some(d) = distance {
                        out.push(("distance", Json::from(d)));
                    }
                    return done(out);
                }
                _ => {}
            }
        }
        let at = state.answers.len();
        if asking_gap {
            return Step::Ask(
                Prompt::new(
                    format!("gap-{at}"),
                    PROMPT_SPLIT_GAP.message_id,
                    Accept::Distance {
                        min: Some(0.0),
                        max: Some(1000.0),
                    },
                )
                .with_default(PromptInput::Distance(gap)),
            );
        }
        if distance.is_some() {
            return ask_point(format!("end-{at}"), PROMPT_SPLIT_END.message_id);
        }
        Step::Ask(
            Prompt::new(
                format!("split-{at}"),
                PROMPT_SPLIT_POINT.message_id,
                point_accept(),
            )
            .or(Accept::Distance {
                min: Some(0.0),
                max: Some(100_000.0),
            })
            .or(Accept::keywords([KeywordOption::new(
                "gap",
                KEYWORD_SPLIT_GAP.message_id,
            )
            .with_shortcut('g')])),
        )
    }

    fn count(state: &PromptState<'_>) -> Option<u32> {
        state.answers.first().and_then(|a| match &a.input {
            PromptInput::Number(q) => {
                let v = q.value().round();
                (2..=MAX_COUNT).find(|n| f64::from(*n) == v)
            }
            _ => None,
        })
    }

    fn count_prompt() -> Step {
        Step::Ask(Prompt::new(
            "count-0",
            PROMPT_ARRAY_COUNT.message_id,
            Accept::Number {
                dimension: Dimension::NONE,
                min: Some(2.0),
                max: Some(f64::from(MAX_COUNT)),
                integer: true,
            },
        ))
    }

    fn array(state: &PromptState<'_>) -> Step {
        let Some(count) = Self::count(state) else {
            return Self::count_prompt();
        };
        let last = state
            .answers
            .iter()
            .filter(|a| matches!(a.input, PromptInput::Keyword(_)))
            .count()
            % 2
            == 1;
        let picks = points(state);
        let at = state.answers.len();
        match picks.as_slice() {
            [] => ask_point(format!("base-{at}"), PROMPT_ARRAY_BASE.message_id),
            [_] => {
                let (message, toggle) = if last {
                    (
                        PROMPT_ARRAY_LAST.message_id,
                        KeywordOption::new("spacing", KEYWORD_ARRAY_SPACING.message_id)
                            .with_shortcut('s'),
                    )
                } else {
                    (
                        PROMPT_ARRAY_SECOND.message_id,
                        KeywordOption::new("last", KEYWORD_ARRAY_LAST.message_id)
                            .with_shortcut('l'),
                    )
                };
                Step::Ask(
                    Prompt::new(format!("second-{at}"), message, point_accept())
                        .or(Accept::keywords([toggle])),
                )
            }
            [a, b, ..] => done(vec![
                ("count", Json::from(count)),
                ("displacement", Json::from(vec![b.x - a.x, b.y - a.y])),
                ("last", Json::from(last)),
            ]),
        }
    }

    fn radial(state: &PromptState<'_>) -> Step {
        let Some(count) = Self::count(state) else {
            return Self::count_prompt();
        };
        let Some(center) = points(state).first().copied() else {
            return ask_point("center-1".to_owned(), PROMPT_ARRAY_CENTER.message_id);
        };
        let angle = state.answers.get(2).and_then(|a| match &a.input {
            PromptInput::Angle(v) => Some(*v),
            PromptInput::Point(p) => Some((p.y - center.y).atan2(p.x - center.x)),
            _ => None,
        });
        match angle {
            Some(v) => done(vec![
                ("count", Json::from(count)),
                ("center", xy(&center)),
                ("angle", Json::from(v)),
            ]),
            None => Step::Ask(
                Prompt::new("sweep-2", PROMPT_ARRAY_ANGLE.message_id, Accept::angle())
                    .or(point_accept())
                    .with_default(PromptInput::Angle(std::f64::consts::TAU)),
            ),
        }
    }
}

impl PromptMachine for EditPrompts {
    /// A pick is refused at once when nothing lies under it: a line for a reference, a boundary
    /// or an align target, a wall for the other picks.
    fn check(&self, state: &PromptState<'_>, prompt: &Prompt, input: &PromptInput) -> Result<()> {
        let PromptInput::Point(p) = input else {
            return Ok(());
        };
        let storey = state
            .preset
            .get("storey")
            .and_then(Json::as_str)
            .and_then(|t| t.parse::<ObjectId>().ok());
        let radius = radius_of(state.preset.get("radius").and_then(Json::as_f64))?;
        let at = [p.x, p.y];
        match prompt.key.split('-').next().unwrap_or_default() {
            "reference" | "boundary" => {
                line_at(state.snapshot, storey, at, radius, None).map(|_| ())
            }
            "target" if self.which == Which::Align => {
                line_at(state.snapshot, storey, at, radius, None).map(|_| ())
            }
            "target" | "first" | "other" | "split" | "end" => {
                wall_at(state.snapshot, storey, at, radius).map(|_| ())
            }
            _ => Ok(()),
        }
    }

    fn next(&self, state: &PromptState<'_>) -> Result<Step> {
        Ok(match self.which {
            Which::Align | Which::TrimTo | Which::TrimMany => self.picked_line(state),
            Which::Trim | Which::Merge => self.two_walls(state),
            Which::Offset => Self::offset(state),
            Which::Split => Self::split(state),
            Which::Array => Self::array(state),
            Which::Radial => Self::radial(state),
        })
    }
}

fn prompts(which: Which) -> Arc<EditPrompts> {
    Arc::new(EditPrompts { which })
}

/// A message of the catalog, named by identifier.
struct Msg {
    message_id: &'static str,
}

const CMD_ELEMENT_PIN_DESCRIPTION: Msg = Msg {
    message_id: "ab-cmd-element-pin-description",
};
const CMD_ELEMENT_PIN_LABEL: Msg = Msg {
    message_id: "ab-cmd-element-pin-label",
};
const CMD_ELEMENT_UNPIN_DESCRIPTION: Msg = Msg {
    message_id: "ab-cmd-element-unpin-description",
};
const CMD_ELEMENT_UNPIN_LABEL: Msg = Msg {
    message_id: "ab-cmd-element-unpin-label",
};
const CMD_MODIFY_ALIGN_DESCRIPTION: Msg = Msg {
    message_id: "ab-cmd-modify-align-description",
};
const CMD_MODIFY_ALIGN_LABEL: Msg = Msg {
    message_id: "ab-cmd-modify-align-label",
};
const CMD_MODIFY_ARRAY_DESCRIPTION: Msg = Msg {
    message_id: "ab-cmd-modify-array-description",
};
const CMD_MODIFY_ARRAY_LABEL: Msg = Msg {
    message_id: "ab-cmd-modify-array-label",
};
const CMD_MODIFY_ARRAY_RADIAL_DESCRIPTION: Msg = Msg {
    message_id: "ab-cmd-modify-array-radial-description",
};
const CMD_MODIFY_ARRAY_RADIAL_LABEL: Msg = Msg {
    message_id: "ab-cmd-modify-array-radial-label",
};
const CMD_MODIFY_OFFSET_DESCRIPTION: Msg = Msg {
    message_id: "ab-cmd-modify-offset-description",
};
const CMD_MODIFY_OFFSET_LABEL: Msg = Msg {
    message_id: "ab-cmd-modify-offset-label",
};
const CMD_MODIFY_TRIM_DESCRIPTION: Msg = Msg {
    message_id: "ab-cmd-modify-trim-description",
};
const CMD_MODIFY_TRIM_LABEL: Msg = Msg {
    message_id: "ab-cmd-modify-trim-label",
};
const CMD_MODIFY_TRIM_MANY_DESCRIPTION: Msg = Msg {
    message_id: "ab-cmd-modify-trim-many-description",
};
const CMD_MODIFY_TRIM_MANY_LABEL: Msg = Msg {
    message_id: "ab-cmd-modify-trim-many-label",
};
const CMD_MODIFY_TRIM_TO_DESCRIPTION: Msg = Msg {
    message_id: "ab-cmd-modify-trim-to-description",
};
const CMD_MODIFY_TRIM_TO_LABEL: Msg = Msg {
    message_id: "ab-cmd-modify-trim-to-label",
};
const CMD_WALL_MERGE_DESCRIPTION: Msg = Msg {
    message_id: "ab-cmd-wall-merge-description",
};
const CMD_WALL_MERGE_LABEL: Msg = Msg {
    message_id: "ab-cmd-wall-merge-label",
};
const CMD_WALL_SPLIT_DESCRIPTION: Msg = Msg {
    message_id: "ab-cmd-wall-split-description",
};
const CMD_WALL_SPLIT_LABEL: Msg = Msg {
    message_id: "ab-cmd-wall-split-label",
};
const KEYWORD_ARRAY_LAST: Msg = Msg {
    message_id: "ab-keyword-array-last",
};
const KEYWORD_ARRAY_SPACING: Msg = Msg {
    message_id: "ab-keyword-array-spacing",
};
const KEYWORD_DONE: Msg = Msg {
    message_id: "ab-keyword-done",
};
const KEYWORD_OFFSET_COPY: Msg = Msg {
    message_id: "ab-keyword-offset-copy",
};
const KEYWORD_OFFSET_MOVE: Msg = Msg {
    message_id: "ab-keyword-offset-move",
};
const KEYWORD_SPLIT_GAP: Msg = Msg {
    message_id: "ab-keyword-split-gap",
};
const PROMPT_ALIGN_REFERENCE: Msg = Msg {
    message_id: "ab-prompt-align-reference",
};
const PROMPT_ALIGN_TARGET: Msg = Msg {
    message_id: "ab-prompt-align-target",
};
const PROMPT_ARRAY_ANGLE: Msg = Msg {
    message_id: "ab-prompt-array-angle",
};
const PROMPT_ARRAY_BASE: Msg = Msg {
    message_id: "ab-prompt-array-base",
};
const PROMPT_ARRAY_CENTER: Msg = Msg {
    message_id: "ab-prompt-array-center",
};
const PROMPT_ARRAY_COUNT: Msg = Msg {
    message_id: "ab-prompt-array-count",
};
const PROMPT_ARRAY_LAST: Msg = Msg {
    message_id: "ab-prompt-array-last",
};
const PROMPT_ARRAY_SECOND: Msg = Msg {
    message_id: "ab-prompt-array-second",
};
const PROMPT_MERGE_FIRST: Msg = Msg {
    message_id: "ab-prompt-merge-first",
};
const PROMPT_MERGE_OTHER: Msg = Msg {
    message_id: "ab-prompt-merge-other",
};
const PROMPT_OFFSET_DISTANCE: Msg = Msg {
    message_id: "ab-prompt-offset-distance",
};
const PROMPT_OFFSET_SIDE: Msg = Msg {
    message_id: "ab-prompt-offset-side",
};
const PROMPT_SPLIT_END: Msg = Msg {
    message_id: "ab-prompt-split-end",
};
const PROMPT_SPLIT_GAP: Msg = Msg {
    message_id: "ab-prompt-split-gap",
};
const PROMPT_SPLIT_POINT: Msg = Msg {
    message_id: "ab-prompt-split-point",
};
const PROMPT_TRIM_BOUNDARY: Msg = Msg {
    message_id: "ab-prompt-trim-boundary",
};
const PROMPT_TRIM_FIRST: Msg = Msg {
    message_id: "ab-prompt-trim-first",
};
const PROMPT_TRIM_OTHER: Msg = Msg {
    message_id: "ab-prompt-trim-other",
};
const PROMPT_TRIM_TARGET: Msg = Msg {
    message_id: "ab-prompt-trim-target",
};
const PROMPT_TRIM_TARGETS: Msg = Msg {
    message_id: "ab-prompt-trim-targets",
};

/// Registers the commands of this module and their typed names.
///
/// # Errors
/// `cmd.duplicate` when an identifier or a name is already registered.
#[allow(clippy::too_many_lines, reason = "one registration per command")]
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(
        CommandSpec::new(
            MODIFY_ALIGN,
            CMD_MODIFY_ALIGN_LABEL.message_id,
            CMD_MODIFY_ALIGN_DESCRIPTION.message_id,
            align,
        )
        .with_prompt(prompts(Which::Align)),
    )?;
    registry.register(
        CommandSpec::new(
            MODIFY_OFFSET,
            CMD_MODIFY_OFFSET_LABEL.message_id,
            CMD_MODIFY_OFFSET_DESCRIPTION.message_id,
            offset,
        )
        .with_prompt(prompts(Which::Offset)),
    )?;
    registry.register(
        CommandSpec::new(
            MODIFY_TRIM,
            CMD_MODIFY_TRIM_LABEL.message_id,
            CMD_MODIFY_TRIM_DESCRIPTION.message_id,
            trim_corner,
        )
        .with_prompt(prompts(Which::Trim)),
    )?;
    registry.register(
        CommandSpec::new(
            MODIFY_TRIM_TO,
            CMD_MODIFY_TRIM_TO_LABEL.message_id,
            CMD_MODIFY_TRIM_TO_DESCRIPTION.message_id,
            trim_to,
        )
        .with_prompt(prompts(Which::TrimTo)),
    )?;
    registry.register(
        CommandSpec::new(
            MODIFY_TRIM_MANY,
            CMD_MODIFY_TRIM_MANY_LABEL.message_id,
            CMD_MODIFY_TRIM_MANY_DESCRIPTION.message_id,
            trim_to,
        )
        .with_prompt(prompts(Which::TrimMany)),
    )?;
    registry.register(
        CommandSpec::new(
            WALL_SPLIT,
            CMD_WALL_SPLIT_LABEL.message_id,
            CMD_WALL_SPLIT_DESCRIPTION.message_id,
            split,
        )
        .with_prompt(prompts(Which::Split)),
    )?;
    registry.register(
        CommandSpec::new(
            WALL_MERGE,
            CMD_WALL_MERGE_LABEL.message_id,
            CMD_WALL_MERGE_DESCRIPTION.message_id,
            merge,
        )
        .with_prompt(prompts(Which::Merge)),
    )?;
    registry.register(
        CommandSpec::new(
            MODIFY_ARRAY,
            CMD_MODIFY_ARRAY_LABEL.message_id,
            CMD_MODIFY_ARRAY_DESCRIPTION.message_id,
            linear_array,
        )
        .with_prompt(prompts(Which::Array)),
    )?;
    registry.register(
        CommandSpec::new(
            MODIFY_ARRAY_RADIAL,
            CMD_MODIFY_ARRAY_RADIAL_LABEL.message_id,
            CMD_MODIFY_ARRAY_RADIAL_DESCRIPTION.message_id,
            radial_array,
        )
        .with_prompt(prompts(Which::Radial)),
    )?;
    registry.register(CommandSpec::new(
        ELEMENT_PIN,
        CMD_ELEMENT_PIN_LABEL.message_id,
        CMD_ELEMENT_PIN_DESCRIPTION.message_id,
        |ctx: &mut Context<'_>, p: Pin| set_pins(ctx, p, true),
    ))?;
    registry.register(CommandSpec::new(
        ELEMENT_UNPIN,
        CMD_ELEMENT_UNPIN_LABEL.message_id,
        CMD_ELEMENT_UNPIN_DESCRIPTION.message_id,
        |ctx: &mut Context<'_>, p: Pin| set_pins(ctx, p, false),
    ))?;
    for (alias, id) in [
        ("al", MODIFY_ALIGN),
        ("align", MODIFY_ALIGN),
        ("of", MODIFY_OFFSET),
        ("offset", MODIFY_OFFSET),
        ("tr", MODIFY_TRIM),
        ("trim", MODIFY_TRIM),
        ("split", WALL_SPLIT),
        ("merge", WALL_MERGE),
        ("ar", MODIFY_ARRAY),
        ("array", MODIFY_ARRAY),
        ("pin", ELEMENT_PIN),
        ("unpin", ELEMENT_UNPIN),
    ] {
        registry.add_user_alias(alias, id)?;
    }
    Ok(())
}
