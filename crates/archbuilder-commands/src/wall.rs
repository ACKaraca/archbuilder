//! `archbuilder.wall.create`: one wall on a storey, and the prompts that ask for it.
//!
//! The command takes the baseline's two points (meters, project coordinates) and optionally the
//! thickness, height, base offset, location line and storey. Without a storey it uses the storey of
//! the selection (a selected storey, its level, or a wall's storey), else the lowest storey. The
//! height defaults to the distance to the next storey's level, else three meters. One call is one
//! transaction, so one undo step: a chain of walls drawn in a row is one undo step per wall.
//!
//! The prompt machine ([`WallPrompts`]) asks for the start point, then the end point; at either
//! prompt the keywords `thickness`, `height` and `location` ask for a value first and return to
//! the point. `close` (only when the host presets `close_to`, the start of its chain) ends the wall
//! at that point. Typed points are read by the foundation's point entry: `x,y`, relative `@dx,dy`
//! and polar `@length<angle`, in the session's context unit.

use std::collections::BTreeSet;
use std::str::FromStr;
use std::sync::Arc;

use arc_base::{Error, ObjectId, Result};
use arc_command::prompt::{
    Accept, KeywordOption, Point, PointDims, Prompt, PromptInput, PromptMachine, PromptState, Step,
};
use arc_command::{CommandSpec, Context, IdParam, QuantityParam, Registry};
use arc_doc::{Object, Snapshot, Value};
use archbuilder_model::element_type::TypeKind;
use archbuilder_model::spatial;
use archbuilder_model::types::{
    self, DEFAULT_THICKNESS, FLOOR, HEIGHT_RANGE, LEVEL, MIN_WALL_LENGTH, STOREY, THICKNESS_RANGE,
    WALL, storey,
};
use archbuilder_model::units::{decimal_separator, length_text};
use archbuilder_model::wall::{JoinStyle, LocationLine};
use archbuilder_model::{defaults, names};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json};

use crate::error::codes;
use crate::structure::{LANGUAGE_SETTING, create, ensure_valid, expect, language, named};

/// The identifier of the create-wall command.
pub const WALL_CREATE: &str = "archbuilder.wall.create";
/// The identifier of the delete-wall command.
pub const WALL_DELETE: &str = "archbuilder.wall.delete";
/// The identifier of the command that flips walls to the other side of their location line.
pub const WALL_FLIP: &str = "archbuilder.wall.flip";
/// The identifier of the command that moves one end of a wall.
pub const WALL_MOVE_END: &str = "archbuilder.wall.move_end";
/// The identifier of the command that sets the join style of wall ends.
pub const WALL_JOIN_STYLE: &str = "archbuilder.wall.join_style";
/// The identifier of the command that takes wall ends out of joins.
pub const WALL_DISALLOW_JOIN: &str = "archbuilder.wall.disallow_join";
/// The identifier of the command that puts wall ends back into joins.
pub const WALL_ALLOW_JOIN: &str = "archbuilder.wall.allow_join";
/// The identifier of the command that deletes the selection: its walls, else its storey.
pub const OBJECT_DELETE: &str = "archbuilder.object.delete";

/// The most walls one delete names.
const MAX_DELETED: usize = 100_000;

/// A location line as a command parameter.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LocationLineParam {
    /// The middle of the thickness.
    Center,
    /// The exterior face: the body lies on the right of the baseline's direction.
    Exterior,
    /// The interior face: the body lies on the left of the baseline's direction.
    Interior,
    /// The middle of the core of the wall's type.
    CoreCenter,
    /// The exterior face of the core.
    CoreExterior,
    /// The interior face of the core.
    CoreInterior,
}

impl From<LocationLineParam> for LocationLine {
    fn from(line: LocationLineParam) -> Self {
        match line {
            LocationLineParam::Center => Self::Center,
            LocationLineParam::Exterior => Self::Exterior,
            LocationLineParam::Interior => Self::Interior,
            LocationLineParam::CoreCenter => Self::CoreCenter,
            LocationLineParam::CoreExterior => Self::CoreExterior,
            LocationLineParam::CoreInterior => Self::CoreInterior,
        }
    }
}

impl From<LocationLine> for LocationLineParam {
    fn from(line: LocationLine) -> Self {
        match line {
            LocationLine::Center => Self::Center,
            LocationLine::Exterior => Self::Exterior,
            LocationLine::Interior => Self::Interior,
            LocationLine::CoreCenter => Self::CoreCenter,
            LocationLine::CoreExterior => Self::CoreExterior,
            LocationLine::CoreInterior => Self::CoreInterior,
        }
    }
}

/// Parameters of `archbuilder.wall.create`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateWall {
    /// The baseline's start point `[x, y]` in meters, project coordinates.
    pub start: [f64; 2],
    /// The baseline's end point `[x, y]` in meters.
    pub end: [f64; 2],
    /// The storey; else the storey of the selection, else the lowest storey.
    #[serde(default)]
    pub storey: Option<IdParam>,
    /// The wall type: its identifier or name; else the document's first wall type.
    #[serde(default, rename = "type")]
    #[schemars(length(max = 256))]
    pub wall_type: Option<String>,
    /// The thickness, a number in meters or a text with a unit. A thickness other than the
    /// type's gives the wall a type of one layer of that thickness (an existing one, else a new
    /// one).
    #[serde(default)]
    pub thickness: Option<QuantityParam>,
    /// The height above the base, which leaves the wall unconnected at the top; else the wall's
    /// top is constrained to the level above (`top_level`), and without one the height is 3 m.
    #[serde(default)]
    pub height: Option<QuantityParam>,
    /// The level the wall's top is constrained to; else, when no height is given, the level of
    /// the next higher storey. The height then follows the level.
    #[serde(default)]
    pub top_level: Option<IdParam>,
    /// The base's distance above the storey's level; 0 when absent.
    #[serde(default)]
    pub base_offset: Option<QuantityParam>,
    /// Which line of the wall the baseline is; the center when absent.
    #[serde(default)]
    pub location_line: Option<LocationLineParam>,
    /// The wall name; else the first free `Wall <n>` of the storey.
    #[serde(default)]
    pub name: Option<String>,
    /// Whether the wall is flipped: its baseline is stored reversed, so the exterior side and, for
    /// an exterior or interior face on the line, the body swap sides.
    #[serde(default)]
    pub flip: Option<bool>,
    /// A prompt aid: the point the keyword `close` ends the wall at. The command ignores it.
    #[serde(default)]
    pub close_to: Option<[f64; 2]>,
}

/// What `archbuilder.wall.create` returns: the wall and the values it was made with, which a
/// host that chains walls carries to the next one.
#[derive(Debug, Serialize, JsonSchema)]
pub struct WallCreated {
    /// The new wall.
    pub wall: IdParam,
    /// Its storey.
    pub storey: IdParam,
    /// The start point `[x, y]`, in meters.
    pub start: [f64; 2],
    /// The end point `[x, y]`, in meters.
    pub end: [f64; 2],
    /// The thickness in meters.
    pub thickness: f64,
    /// The wall type.
    #[serde(rename = "type")]
    pub wall_type: IdParam,
    /// The height in meters.
    pub height: f64,
    /// The level the top is constrained to, when it is.
    pub top_level: Option<IdParam>,
    /// Whether the wall was flipped; `start` and `end` stay the points as given.
    pub flip: bool,
    /// The base offset in meters.
    pub base_offset: f64,
    /// The location line.
    pub location_line: LocationLineParam,
}

/// Parameters of `archbuilder.wall.delete`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteWalls {
    /// The walls; else the walls of the selection.
    #[serde(default)]
    pub walls: Option<Vec<IdParam>>,
}

/// Parameters of `archbuilder.wall.flip`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FlipWalls {
    /// The walls; else the walls of the selection.
    #[serde(default)]
    pub walls: Option<Vec<IdParam>>,
}

/// Which end of a wall a join command changes.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WallEnd {
    /// The baseline's start.
    Start,
    /// The baseline's end.
    End,
    /// Both ends.
    #[default]
    Both,
}

/// A join style as a command parameter.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum JoinStyleParam {
    /// Miter at a corner, stop at the face at a T.
    Auto,
    /// This end stops at the other wall's face, which runs through.
    Butt,
    /// Like butt, with the end cut square.
    Square,
    /// The end is not joined.
    Disallowed,
}

impl From<JoinStyleParam> for JoinStyle {
    fn from(style: JoinStyleParam) -> Self {
        match style {
            JoinStyleParam::Auto => Self::Auto,
            JoinStyleParam::Butt => Self::Butt,
            JoinStyleParam::Square => Self::Square,
            JoinStyleParam::Disallowed => Self::Disallowed,
        }
    }
}

/// Parameters of `archbuilder.wall.join_style`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetJoinStyle {
    /// The walls; else the walls of the selection.
    #[serde(default)]
    pub walls: Option<Vec<IdParam>>,
    /// The end; both when absent.
    #[serde(default)]
    pub end: Option<WallEnd>,
    /// The style of that end.
    pub style: JoinStyleParam,
}

/// Parameters of `archbuilder.wall.disallow_join` and `archbuilder.wall.allow_join`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JoinEnds {
    /// The walls; else the walls of the selection.
    #[serde(default)]
    pub walls: Option<Vec<IdParam>>,
    /// The end; both when absent.
    #[serde(default)]
    pub end: Option<WallEnd>,
}

/// Parameters of `archbuilder.object.delete`: none; the selection decides.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteSelection {}

/// The walls of the selection.
fn selected_walls(ctx: &Context<'_>) -> Vec<ObjectId> {
    let snapshot = ctx.snapshot();
    ctx.selection()
        .iter()
        .copied()
        .filter(|id| spatial::type_of(snapshot, *id).as_deref() == Some(WALL))
        .collect()
}

/// What a delete returns: the walls deleted, and the doors and windows deleted with them or by
/// themselves.
#[derive(Debug, Default, Serialize, JsonSchema)]
pub struct Deleted {
    /// The walls deleted.
    pub walls: usize,
    /// The doors and windows deleted: those of the deleted walls (they go with their host in the
    /// same transaction) and those named themselves.
    pub inserts: usize,
    /// The floors deleted by themselves.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub floors: Option<usize>,
    /// The rooms deleted by themselves.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rooms: Option<usize>,
    /// The stairs deleted by themselves.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stairs: Option<usize>,
    /// The roofs deleted by themselves.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roofs: Option<usize>,
    /// The floor and roof edges that followed a deleted wall and now stay where they were.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frozen_edges: Option<usize>,
    /// The walls whose top was attached to a deleted roof or floor; they keep their own top.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detached_walls: Option<usize>,
}

/// Deletes walls and inserts in one transaction, so one undo step brings them all back. The
/// inserts of a deleted wall go with it by the deletion rules of their host.
fn delete_set(
    ctx: &mut Context<'_>,
    walls: BTreeSet<ObjectId>,
    inserts: BTreeSet<ObjectId>,
    floors: BTreeSet<ObjectId>,
) -> Result<Deleted> {
    if walls.is_empty() && inserts.is_empty() && floors.is_empty() {
        return Err(Error::new(codes::NOTHING_SELECTED));
    }
    for id in &walls {
        expect(ctx.snapshot(), *id, WALL)?;
    }
    // The third set holds floors, rooms and stairs, which nothing else depends on.
    let mut room_count = 0;
    let mut stair_count = 0;
    let mut roof_count = 0;
    for id in &floors {
        let kind = spatial::type_of(ctx.snapshot(), *id);
        if kind.as_deref() == Some(archbuilder_model::types::ROOM) {
            room_count += 1;
        } else if kind.as_deref() == Some(archbuilder_model::types::STAIR) {
            stair_count += 1;
        } else if kind.as_deref() == Some(archbuilder_model::types::ROOF) {
            roof_count += 1;
        } else if !kind.as_deref().is_some_and(|k| {
            archbuilder_model::types::is_storey_element(k)
                || archbuilder_model::types::is_datum(k)
                || is_hosted_cut(k)
        }) {
            expect(ctx.snapshot(), *id, FLOOR)?;
        }
    }
    // Floor edges that follow a deleted wall keep their last position.
    let frozen_edges =
        crate::floor::freeze_edges(ctx, &walls)? + crate::roof::freeze_edges(ctx, &walls)?;
    // Walls attached to a deleted roof or floor go back to their own top.
    let detached = crate::roof::detach_from(ctx, &floors)?;
    // The inserts of a deleted wall go with it; the others named are deleted by themselves.
    let mut cascaded = 0;
    for wall in &walls {
        cascaded += archbuilder_model::insert::hosted(ctx.snapshot(), *wall).len();
    }
    let alone: Vec<ObjectId> = inserts
        .into_iter()
        .filter(|id| {
            !spatial::get(ctx.snapshot(), *id)
                .is_some_and(|o| o.header.owner.is_some_and(|w| walls.contains(&w)))
        })
        .collect();
    let count = cascaded + alone.len();
    let limits = *ctx.limits();
    let txn = ctx.txn()?;
    for id in walls.iter().chain(&alone).chain(&floors) {
        // A vertical opening or floor edge goes with its host when both are named.
        let hosted_in_set = spatial::get(txn.snapshot(), *id)
            .filter(|o| {
                spatial::type_of(txn.snapshot(), o.id)
                    .as_deref()
                    .is_some_and(is_hosted_cut)
            })
            .and_then(|o| o.header.owner)
            .is_some_and(|h| floors.contains(&h));
        if !hosted_in_set {
            txn.delete(*id)?;
        }
    }
    txn.apply_delete_rules(&limits)?;
    ensure_valid(ctx)?;
    Ok(Deleted {
        walls: walls.len(),
        inserts: count,
        floors: Some(floors.len() - room_count - stair_count - roof_count).filter(|n| *n > 0),
        rooms: Some(room_count).filter(|n| *n > 0),
        stairs: Some(stair_count).filter(|n| *n > 0),
        roofs: Some(roof_count).filter(|n| *n > 0),
        frozen_edges: Some(frozen_edges).filter(|n| *n > 0),
        detached_walls: Some(detached).filter(|n| *n > 0),
    })
}

/// Deletes walls (with their inserts) in one transaction.
fn delete_walls(ctx: &mut Context<'_>, p: DeleteWalls) -> Result<Deleted> {
    let ids: BTreeSet<ObjectId> = match p.walls {
        Some(list) if list.len() > MAX_DELETED => {
            return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
        }
        Some(list) => list.into_iter().map(|IdParam(id)| id).collect(),
        None => selected_walls(ctx).into_iter().collect(),
    };
    delete_set(ctx, ids, BTreeSet::new(), BTreeSet::new())
}

/// The end of a baseline.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BaselineEnd {
    /// The baseline's start point.
    Start,
    /// The baseline's end point.
    End,
}

/// Parameters of `archbuilder.wall.move_end`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MoveEnd {
    /// The wall.
    pub wall: IdParam,
    /// The end that moves; the other end stays.
    pub end: BaselineEnd,
    /// The new position `[x, y]` of the end, in meters.
    pub to: [f64; 2],
}

/// Moves one end of a wall to a point and keeps the joins: neighbors whose ends met this end
/// (auto, butt or square joins) take the same corner point, and a stem standing on the wall
/// follows its new line; a free end or a disallowed join moves alone. The other end of the wall
/// stays, and so do its doors and windows (their distance from that end), and the openings of the
/// neighbors keep their place. The command is refused, naming the opening and telling the range of
/// the wall as it would stand, when an opening would no longer fit.
fn move_end(ctx: &mut Context<'_>, p: MoveEnd) -> Result<crate::modify::Modified> {
    let IdParam(id) = p.wall;
    expect(ctx.snapshot(), id, WALL)?;
    if !p.to.iter().all(|v| v.is_finite()) {
        return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
    }
    let end = match p.end {
        BaselineEnd::Start => 0,
        BaselineEnd::End => 1,
    };
    let (followed, meets) = crate::modify::move_end_of(ctx, id, end, p.to)?;
    Ok(crate::modify::Modified {
        objects: 1 + followed,
        meets,
        ..crate::modify::Modified::default()
    })
}

/// Flips walls: the baseline is reversed (start and end swap) and the location line stays. The
/// exterior side swaps; the body of a wall on its center line stays where it is, and the body of
/// a wall on a face moves to the other side of the line, the face on the line keeping its kind.
fn flip_walls(ctx: &mut Context<'_>, p: FlipWalls) -> Result<()> {
    let ids: BTreeSet<ObjectId> = match p.walls {
        Some(list) if list.len() > MAX_DELETED => {
            return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
        }
        Some(list) => list.into_iter().map(|IdParam(id)| id).collect(),
        None => selected_walls(ctx).into_iter().collect(),
    };
    if ids.is_empty() {
        return Err(Error::new(codes::NOTHING_SELECTED));
    }
    for id in &ids {
        expect(ctx.snapshot(), *id, WALL)?;
    }
    // Floors keep their edges where they are: the left of a reversed baseline is the other side.
    let flips: Vec<(
        ObjectId,
        archbuilder_model::wall::Wall,
        archbuilder_model::wall::Wall,
    )> = ids
        .iter()
        .filter_map(|id| {
            let w = spatial::get(ctx.snapshot(), *id)
                .and_then(|o| archbuilder_model::wall::Wall::from_values(&o.values))?;
            let new = archbuilder_model::wall::Wall {
                start: w.end,
                end: w.start,
                start_join: w.end_join,
                end_join: w.start_join,
                ..w
            };
            Some((*id, w, new))
        })
        .collect();
    let mut retargeted = Vec::new();
    for object in spatial::objects(ctx.snapshot(), FLOOR) {
        if let Some(vertices) = archbuilder_model::floor::Floor::from_values(&object.values)
            .and_then(|f| f.flipped(&flips))
        {
            retargeted.push((
                object.id,
                archbuilder_model::floor::Floor::vertex_values(&vertices),
            ));
        }
    }
    for (floor, vertices) in retargeted {
        ctx.txn()?
            .modify(floor, arc_doc::FieldPath::new("vertices"), Some(vertices))?;
    }
    for id in ids {
        let Some(wall) = spatial::get(ctx.snapshot(), id)
            .and_then(|w| archbuilder_model::wall::Wall::from_values(&w.values))
        else {
            continue;
        };
        // The inserts keep their place and swing: distance from the new start, the other facing.
        let hosted = archbuilder_model::insert::hosted(ctx.snapshot(), id);
        let txn = ctx.txn()?;
        for (object, item) in hosted {
            txn.modify(
                object.id,
                arc_doc::FieldPath::new("distance"),
                Some(types::length((wall.length() - item.distance).max(0.0))),
            )?;
            txn.modify(
                object.id,
                arc_doc::FieldPath::new("facing"),
                Some(Value::Enum(item.facing.flipped().code())),
            )?;
        }
        // The ends swap with the baseline, and so do their join styles.
        for (field, style) in [("start_join", wall.end_join), ("end_join", wall.start_join)] {
            txn.modify(
                id,
                arc_doc::FieldPath::new(field),
                Some(Value::Enum(style.code())),
            )?;
        }
        for (field, value) in [
            ("start_x", wall.end[0]),
            ("start_y", wall.end[1]),
            ("end_x", wall.start[0]),
            ("end_y", wall.start[1]),
        ] {
            txn.modify(
                id,
                arc_doc::FieldPath::new(field),
                Some(types::length(value)),
            )?;
        }
    }
    ensure_valid(ctx)
}

/// Sets the join style of the chosen ends of walls, in one transaction.
fn set_join(
    ctx: &mut Context<'_>,
    walls: Option<Vec<IdParam>>,
    end: Option<WallEnd>,
    style: JoinStyle,
) -> Result<()> {
    let ids: BTreeSet<ObjectId> = match walls {
        Some(list) if list.len() > MAX_DELETED => {
            return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
        }
        Some(list) => list.into_iter().map(|IdParam(id)| id).collect(),
        None => selected_walls(ctx).into_iter().collect(),
    };
    if ids.is_empty() {
        return Err(Error::new(codes::NOTHING_SELECTED));
    }
    for id in &ids {
        expect(ctx.snapshot(), *id, WALL)?;
    }
    let fields: &[&str] = match end.unwrap_or_default() {
        WallEnd::Start => &["start_join"],
        WallEnd::End => &["end_join"],
        WallEnd::Both => &["start_join", "end_join"],
    };
    let txn = ctx.txn()?;
    for id in ids {
        for field in fields {
            txn.modify(
                id,
                arc_doc::FieldPath::new(*field),
                Some(Value::Enum(style.code())),
            )?;
        }
    }
    ensure_valid(ctx)
}

/// Deletes the selected walls and inserts; with neither selected, the selected storey.
/// The elements a floor, roof or ceiling hosts that are deleted by themselves: vertical openings
/// floor edges, dormers and roof edge elements.
fn is_hosted_cut(key: &str) -> bool {
    use archbuilder_model::types as t;
    [t::VERTICAL_OPENING, t::FLOOR_EDGE, t::DORMER, t::ROOF_EDGE].contains(&key)
}

fn delete_selection(ctx: &mut Context<'_>, _: DeleteSelection) -> Result<Deleted> {
    let walls: BTreeSet<ObjectId> = selected_walls(ctx).into_iter().collect();
    let snapshot = ctx.snapshot();
    let inserts: BTreeSet<ObjectId> = ctx
        .selection()
        .iter()
        .copied()
        .filter(|id| crate::insert::is_insert(snapshot, *id))
        .collect();
    let floors: BTreeSet<ObjectId> = ctx
        .selection()
        .iter()
        .copied()
        .filter(|id| {
            // The storey's own elements other than walls, which nothing else depends on.
            spatial::type_of(ctx.snapshot(), *id).is_some_and(|k| {
                (k != WALL && archbuilder_model::types::is_storey_element(&k))
                    || archbuilder_model::types::is_datum(&k)
                    || is_hosted_cut(&k)
            })
        })
        .collect();
    if walls.is_empty() && inserts.is_empty() && floors.is_empty() {
        crate::structure::delete_storey(ctx, crate::structure::DeleteStorey::default())
            .map(|()| Deleted::default())
    } else {
        delete_set(ctx, walls, inserts, floors)
    }
}

/// The storey a wall goes on: the given one, the selection's, else the lowest.
pub(crate) fn storey_for(ctx: &Context<'_>, given: Option<IdParam>) -> Result<Object> {
    let snapshot = ctx.snapshot();
    if let Some(IdParam(id)) = given {
        return expect(snapshot, id, STOREY);
    }
    for id in ctx.selection() {
        let found = match spatial::type_of(snapshot, *id).as_deref() {
            Some(STOREY) => spatial::get(snapshot, *id),
            Some(k) if archbuilder_model::types::is_storey_element(k) => {
                spatial::get(snapshot, *id)
                    .and_then(|w| w.header.owner)
                    .and_then(|owner| spatial::get(snapshot, owner))
            }
            Some(LEVEL) => spatial::objects(snapshot, STOREY)
                .into_iter()
                .find(|s| spatial::level_of(snapshot, s).is_some_and(|l| l.id == *id)),
            _ => None,
        };
        if let Some(found) = found {
            return Ok((*found).clone());
        }
    }
    spatial::lowest_storey(snapshot)
        .map(|s| (*s).clone())
        .ok_or_else(|| Error::new(codes::NO_BUILDING))
}

/// The type of `kind` a new instance gets: the one `given` names, else the document's default,
/// else a new generic one.
pub(crate) fn choose_type(
    ctx: &mut Context<'_>,
    kind: TypeKind,
    given: Option<&str>,
) -> Result<ObjectId> {
    let snapshot = ctx.txn()?.snapshot().clone();
    if let Some(text) = given.filter(|t| !t.trim().is_empty()) {
        return crate::element_type::resolve(&snapshot, kind, text);
    }
    if let Some(id) = crate::element_type::default_of(&snapshot, kind) {
        return Ok(id);
    }
    crate::element_type::ensure(ctx, kind, &crate::element_type::generic(kind))
}

pub(crate) fn next_wall_name(snapshot: &Snapshot, storey: ObjectId, language: &str) -> String {
    let used = spatial::walls(snapshot, storey);
    let taken = |name: &str| {
        used.iter().any(|w| {
            types::text(&w.values, types::wall::NAME).is_some_and(|n| names::same(n, name))
        })
    };
    (used.len() + 1..)
        .map(|n| defaults::wall(language, n))
        .find(|name| !taken(name))
        .unwrap_or_default()
}

pub(crate) fn create_wall(ctx: &mut Context<'_>, p: CreateWall) -> Result<WallCreated> {
    let storey = storey_for(ctx, p.storey)?;
    let meters = |ctx: &Context<'_>, param: &Option<QuantityParam>, default: f64| match param {
        Some(param) => crate::length(ctx, param).map(|q| q.value()),
        None => Ok(default),
    };
    let wall_type = crate::element_type::layered_for(
        ctx,
        TypeKind::Wall,
        p.wall_type.as_deref(),
        p.thickness.as_ref(),
    )?;
    let base_offset = meters(ctx, &p.base_offset, 0.0)?;
    // A given height leaves the top unconnected; else the top follows the level above.
    let top_level = match (&p.height, p.top_level) {
        (Some(_), None) => None,
        (_, Some(IdParam(id))) => Some(expect(ctx.snapshot(), id, LEVEL)?.id),
        (None, None) => spatial::level_above(ctx.snapshot(), &storey)
            // A base at or above the level above leaves the wall unconnected.
            .filter(|l| {
                types::meters(&l.values, types::level::ELEVATION).is_some_and(|e| {
                    e - (spatial::storey_elevation(ctx.snapshot(), &storey) + base_offset)
                        >= types::HEIGHT_RANGE.0
                })
            })
            .map(|l| l.id),
    };
    let default_height = spatial::default_wall_height(ctx.snapshot(), &storey);
    let own = spatial::storey_elevation(ctx.snapshot(), &storey) + base_offset;
    let height = match (&p.height, top_level) {
        (Some(_), _) => meters(ctx, &p.height, default_height)?,
        (None, Some(level)) => spatial::get(ctx.snapshot(), level)
            .and_then(|l| types::meters(&l.values, types::level::ELEVATION))
            .map_or(default_height, |e| e - own),
        (None, None) => default_height,
    };
    let location = p.location_line.map_or(LocationLine::Center, Into::into);
    let name = match p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => name.to_owned(),
        None => next_wall_name(ctx.snapshot(), storey.id, &language(ctx)),
    };
    // A flipped wall stores its baseline reversed.
    let flip = p.flip.unwrap_or(false);
    let baseline = if flip {
        [p.end, p.start]
    } else {
        [p.start, p.end]
    };
    let id = create(
        ctx,
        WALL,
        Some(storey.id),
        vec![
            Some(named(&name)),
            Some(types::length(baseline[0][0])),
            Some(types::length(baseline[0][1])),
            Some(types::length(baseline[1][0])),
            Some(types::length(baseline[1][1])),
            Some(types::length(DEFAULT_THICKNESS)),
            Some(types::length(height)),
            Some(types::length(base_offset)),
            Some(Value::Enum(location.code())),
            top_level.map(Value::Reference),
            Some(Value::Enum(JoinStyle::Auto.code())),
            Some(Value::Enum(JoinStyle::Auto.code())),
            None,
            Some(types::length(0.0)),
            Some(types::length(0.0)),
        ],
    )?;
    crate::element_type::assign(ctx, id, TypeKind::Wall, wall_type)?;
    ensure_valid(ctx)?;
    let thickness = spatial::get(ctx.txn()?.snapshot(), id)
        .and_then(|w| types::meters(&w.values, types::wall::THICKNESS))
        .unwrap_or(DEFAULT_THICKNESS);
    Ok(WallCreated {
        wall: IdParam(id),
        storey: IdParam(storey.id),
        start: p.start,
        end: p.end,
        thickness,
        wall_type: IdParam(wall_type),
        height,
        top_level: top_level.map(IdParam),
        flip,
        base_offset,
        location_line: location.into(),
    })
}

/// A message of the catalog, named by identifier.
struct Msg {
    message_id: &'static str,
}

const PROMPT_START: Msg = Msg {
    message_id: "ab-prompt-wall-start",
};
const PROMPT_END: Msg = Msg {
    message_id: "ab-prompt-wall-end",
};
const PROMPT_THICKNESS: Msg = Msg {
    message_id: "ab-prompt-wall-thickness",
};
const PROMPT_HEIGHT: Msg = Msg {
    message_id: "ab-prompt-wall-height",
};
const PROMPT_LOCATION: Msg = Msg {
    message_id: "ab-prompt-wall-location",
};
const KEY_THICKNESS: Msg = Msg {
    message_id: "ab-keyword-wall-thickness",
};
const KEY_HEIGHT: Msg = Msg {
    message_id: "ab-keyword-wall-height",
};
const KEY_LOCATION: Msg = Msg {
    message_id: "ab-keyword-wall-location",
};
const KEY_FLIP: Msg = Msg {
    message_id: "ab-keyword-wall-flip",
};
const KEY_CLOSE: Msg = Msg {
    message_id: "ab-keyword-wall-close",
};
const KEY_CORE_CENTER: Msg = Msg {
    message_id: "ab-keyword-wall-core-center",
};
const KEY_CORE_EXTERIOR: Msg = Msg {
    message_id: "ab-keyword-wall-core-exterior",
};
const KEY_CORE_INTERIOR: Msg = Msg {
    message_id: "ab-keyword-wall-core-interior",
};
const KEY_CENTER: Msg = Msg {
    message_id: "ab-keyword-wall-center",
};
const KEY_EXTERIOR: Msg = Msg {
    message_id: "ab-keyword-wall-exterior",
};
const KEY_INTERIOR: Msg = Msg {
    message_id: "ab-keyword-wall-interior",
};

/// The prompts of `archbuilder.wall.create`; see the module documentation.
#[derive(Debug)]
pub struct WallPrompts;

/// The point of a JSON `[x, y]`.
fn point_of(value: Option<&Json>) -> Option<[f64; 2]> {
    let items = value?.as_array()?;
    match items.as_slice() {
        [x, y] => Some([x.as_f64()?, y.as_f64()?]),
        _ => None,
    }
}

/// What the prompts show: the storey and the options in effect.
struct Shown {
    storey: String,
    thickness: String,
    height: String,
    location: &'static str,
    /// The thickness and the height in meters, the defaults of their prompts.
    meters: (f64, f64),
    /// Whether the interface language is Turkish: its keywords have their own shortcut letters.
    turkish: bool,
    /// Whether the wall being placed is flipped.
    flip: bool,
}

impl Shown {
    fn of(state: &PromptState<'_>, over: &Map<String, Json>) -> Self {
        let snapshot = state.snapshot;
        let storey = state
            .preset
            .get("storey")
            .and_then(Json::as_str)
            .and_then(|s| ObjectId::from_str(s).ok())
            .and_then(|id| spatial::get(snapshot, id))
            .or_else(|| spatial::lowest_storey(snapshot));
        let number = |key: &str| {
            over.get(key)
                .or_else(|| state.preset.get(key))
                .and_then(Json::as_f64)
        };
        let unit = spatial::unit_settings(snapshot);
        let language = state
            .setting(LANGUAGE_SETTING)
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        let text = |meters| length_text(meters, unit, decimal_separator(&language));
        let height = number("height").or_else(|| {
            storey
                .as_ref()
                .map(|s| spatial::default_wall_height(snapshot, s))
        });
        let location = match over
            .get("location_line")
            .or_else(|| state.preset.get("location_line"))
            .and_then(Json::as_str)
        {
            Some(name) => LocationLine::ALL
                .into_iter()
                .find(|l| l.name() == name)
                .map_or("center", LocationLine::name),
            None => "center",
        };
        Self {
            storey: storey
                .and_then(|s| types::text(&s.values, storey::NAME).map(str::to_owned))
                .unwrap_or_default(),
            thickness: text(number("thickness").unwrap_or(DEFAULT_THICKNESS)),
            height: text(height.unwrap_or(defaults::STOREY_HEIGHT)),
            location,
            meters: (
                number("thickness").unwrap_or(DEFAULT_THICKNESS),
                height.unwrap_or(defaults::STOREY_HEIGHT),
            ),
            turkish: language.starts_with("tr"),
            flip: over
                .get("flip")
                .or_else(|| state.preset.get("flip"))
                .and_then(Json::as_bool)
                .unwrap_or(false),
        }
    }

    fn arg(&self, prompt: Prompt) -> Prompt {
        prompt
            .with_arg("storey", self.storey.as_str())
            .with_arg("thickness", self.thickness.as_str())
            .with_arg("height", self.height.as_str())
            .with_arg("location", self.location)
            // The values in effect as plain numbers and words, for a host that draws a preview.
            .with_arg("thickness_m", self.meters.0.to_string())
            .with_arg("flip", self.flip.to_string())
    }
}

fn distance(min_max: (f64, f64)) -> Accept {
    Accept::Distance {
        min: Some(min_max.0),
        max: Some(min_max.1),
    }
}

/// The shortcut letter of a keyword: the English one, or the letter the Turkish label gives.
fn shortcut(shown: &Shown, english: char, turkish: char) -> char {
    if shown.turkish { turkish } else { english }
}

fn point_prompt(key: String, message: &Msg, close: bool, shown: &Shown) -> Prompt {
    let mut options = vec![
        KeywordOption::new("thickness", KEY_THICKNESS.message_id)
            .with_shortcut(shortcut(shown, 't', 'k')),
        KeywordOption::new("height", KEY_HEIGHT.message_id)
            .with_shortcut(shortcut(shown, 'h', 'y')),
        KeywordOption::new("location", KEY_LOCATION.message_id)
            .with_shortcut(shortcut(shown, 'l', 'o')),
        KeywordOption::new("flip", KEY_FLIP.message_id).with_shortcut(shortcut(shown, 'f', 'ç')),
    ];
    if close {
        options.push(
            KeywordOption::new("close", KEY_CLOSE.message_id)
                .with_shortcut(shortcut(shown, 'c', 'p')),
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
}

fn location_prompt(key: String, shown: &Shown) -> Prompt {
    Prompt::new(
        key,
        PROMPT_LOCATION.message_id,
        Accept::keywords([
            KeywordOption::new("center", KEY_CENTER.message_id)
                .with_shortcut(shortcut(shown, 'c', 'e')),
            KeywordOption::new("exterior", KEY_EXTERIOR.message_id)
                .with_shortcut(shortcut(shown, 'e', 'd')),
            KeywordOption::new("interior", KEY_INTERIOR.message_id)
                .with_shortcut(shortcut(shown, 'i', 'i')),
            KeywordOption::new("core_center", KEY_CORE_CENTER.message_id)
                .with_shortcut(shortcut(shown, 'o', 'k')),
            KeywordOption::new("core_exterior", KEY_CORE_EXTERIOR.message_id)
                .with_shortcut(shortcut(shown, 'x', 'r')),
            KeywordOption::new("core_interior", KEY_CORE_INTERIOR.message_id)
                .with_shortcut(shortcut(shown, 'n', 'n')),
        ]),
    )
    .with_default(PromptInput::Keyword(shown.location.to_owned()))
}

impl WallPrompts {
    /// Reads the answers: the start point (when not preset) and the end point, with the options
    /// set on the way. Returns the start point and the next step.
    fn scan(state: &PromptState<'_>) -> (Option<[f64; 2]>, Step) {
        let close_to = point_of(state.preset.get("close_to"));
        let mut over = Map::new();
        let mut start = point_of(state.preset.get("start"));
        let mut end = None;
        let mut at = 0;
        for stage in [0, 1] {
            if stage == 0 && start.is_some() {
                continue;
            }
            loop {
                let shown = Shown::of(state, &over);
                let message = if stage == 0 {
                    &PROMPT_START
                } else {
                    &PROMPT_END
                };
                let Some(answer) = state.answers.get(at) else {
                    let prompt = point_prompt(
                        format!("point-{at}"),
                        message,
                        stage == 1 && close_to.is_some(),
                        &shown,
                    );
                    return (start, Step::Ask(shown.arg(prompt)));
                };
                at += 1;
                match &answer.input {
                    PromptInput::Point(p) => {
                        if stage == 0 {
                            start = Some([p.x, p.y]);
                        } else {
                            end = Some([p.x, p.y]);
                        }
                        break;
                    }
                    PromptInput::Keyword(k) if k == "close" && stage == 1 && close_to.is_some() => {
                        end = close_to;
                        break;
                    }
                    PromptInput::Keyword(k) if k == "flip" => {
                        // The other side of the location line.
                        over.insert("flip".to_owned(), Json::from(!shown.flip));
                    }
                    PromptInput::Keyword(k) => {
                        let ask = match k.as_str() {
                            "thickness" => Prompt::new(
                                format!("value-{at}"),
                                PROMPT_THICKNESS.message_id,
                                distance(THICKNESS_RANGE),
                            )
                            .with_default(PromptInput::Distance(shown.meters.0)),
                            "height" => Prompt::new(
                                format!("value-{at}"),
                                PROMPT_HEIGHT.message_id,
                                distance(HEIGHT_RANGE),
                            )
                            .with_default(PromptInput::Distance(shown.meters.1)),
                            _ => location_prompt(format!("value-{at}"), &shown),
                        };
                        let Some(value) = state.answers.get(at) else {
                            return (start, Step::Ask(shown.arg(ask)));
                        };
                        at += 1;
                        match (k.as_str(), &value.input) {
                            ("thickness" | "height", PromptInput::Distance(d)) => {
                                over.insert(k.clone(), Json::from(*d));
                            }
                            ("location", PromptInput::Keyword(l)) => {
                                over.insert("location_line".to_owned(), Json::from(l.as_str()));
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut params = over;
        if params.contains_key("height") {
            // A typed height leaves the top unconnected.
            params.insert("top_level".to_owned(), Json::Null);
        }
        for (key, point) in [("start", start), ("end", end)] {
            params.insert(
                key.to_owned(),
                point.map_or(Json::Null, |p| Json::from(p.to_vec())),
            );
        }
        (start, Step::Done(params))
    }
}

impl PromptMachine for WallPrompts {
    fn next(&self, state: &PromptState<'_>) -> Result<Step> {
        Ok(Self::scan(state).1)
    }

    fn check(&self, state: &PromptState<'_>, prompt: &Prompt, input: &PromptInput) -> Result<()> {
        // Only the end point can repeat the start point; it is the second point prompt of the path.
        if let (PromptInput::Point(Point { x, y, .. }), (Some(start), true)) = (
            input,
            (
                Self::scan(state).0,
                prompt.message.id == PROMPT_END.message_id,
            ),
        ) && (x - start[0]).hypot(y - start[1]) < MIN_WALL_LENGTH
        {
            return Err(Error::new(codes::WALL_END_SAME));
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
        message_id: "ab-cmd-wall-create-label",
    };
    const DESCRIPTION: Msg = Msg {
        message_id: "ab-cmd-wall-create-description",
    };
    registry.register(
        CommandSpec::new(
            WALL_CREATE,
            LABEL.message_id,
            DESCRIPTION.message_id,
            |ctx: &mut Context<'_>, p: CreateWall| create_wall(ctx, p),
        )
        .with_prompt(Arc::new(WallPrompts)),
    )?;
    const WALL_DELETE_LABEL: Msg = Msg {
        message_id: "ab-cmd-wall-delete-label",
    };
    const WALL_DELETE_DESCRIPTION: Msg = Msg {
        message_id: "ab-cmd-wall-delete-description",
    };
    const OBJECT_DELETE_LABEL: Msg = Msg {
        message_id: "ab-cmd-object-delete-label",
    };
    const OBJECT_DELETE_DESCRIPTION: Msg = Msg {
        message_id: "ab-cmd-object-delete-description",
    };
    registry.register(CommandSpec::new(
        WALL_DELETE,
        WALL_DELETE_LABEL.message_id,
        WALL_DELETE_DESCRIPTION.message_id,
        delete_walls,
    ))?;
    const WALL_FLIP_LABEL: Msg = Msg {
        message_id: "ab-cmd-wall-flip-label",
    };
    const WALL_FLIP_DESCRIPTION: Msg = Msg {
        message_id: "ab-cmd-wall-flip-description",
    };
    registry.register(CommandSpec::new(
        WALL_FLIP,
        WALL_FLIP_LABEL.message_id,
        WALL_FLIP_DESCRIPTION.message_id,
        flip_walls,
    ))?;
    const MOVE_END_LABEL: Msg = Msg {
        message_id: "ab-cmd-wall-move-end-label",
    };
    const MOVE_END_DESCRIPTION: Msg = Msg {
        message_id: "ab-cmd-wall-move-end-description",
    };
    registry.register(CommandSpec::new(
        WALL_MOVE_END,
        MOVE_END_LABEL.message_id,
        MOVE_END_DESCRIPTION.message_id,
        move_end,
    ))?;
    const JOIN_STYLE_LABEL: Msg = Msg {
        message_id: "ab-cmd-wall-join-style-label",
    };
    const JOIN_STYLE_DESCRIPTION: Msg = Msg {
        message_id: "ab-cmd-wall-join-style-description",
    };
    const DISALLOW_LABEL: Msg = Msg {
        message_id: "ab-cmd-wall-disallow-join-label",
    };
    const DISALLOW_DESCRIPTION: Msg = Msg {
        message_id: "ab-cmd-wall-disallow-join-description",
    };
    const ALLOW_LABEL: Msg = Msg {
        message_id: "ab-cmd-wall-allow-join-label",
    };
    const ALLOW_DESCRIPTION: Msg = Msg {
        message_id: "ab-cmd-wall-allow-join-description",
    };
    registry.register(CommandSpec::new(
        WALL_JOIN_STYLE,
        JOIN_STYLE_LABEL.message_id,
        JOIN_STYLE_DESCRIPTION.message_id,
        |ctx: &mut Context<'_>, p: SetJoinStyle| set_join(ctx, p.walls, p.end, p.style.into()),
    ))?;
    registry.register(CommandSpec::new(
        WALL_DISALLOW_JOIN,
        DISALLOW_LABEL.message_id,
        DISALLOW_DESCRIPTION.message_id,
        |ctx: &mut Context<'_>, p: JoinEnds| set_join(ctx, p.walls, p.end, JoinStyle::Disallowed),
    ))?;
    registry.register(CommandSpec::new(
        WALL_ALLOW_JOIN,
        ALLOW_LABEL.message_id,
        ALLOW_DESCRIPTION.message_id,
        |ctx: &mut Context<'_>, p: JoinEnds| set_join(ctx, p.walls, p.end, JoinStyle::Auto),
    ))?;
    registry.register(CommandSpec::new(
        OBJECT_DELETE,
        OBJECT_DELETE_LABEL.message_id,
        OBJECT_DELETE_DESCRIPTION.message_id,
        delete_selection,
    ))
}
