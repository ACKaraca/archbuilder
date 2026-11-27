//! `archbuilder.door.place` and `archbuilder.window.place`: one door or window in a wall, the
//! prompts that ask for it, and `archbuilder.insert.flip_hand` and `archbuilder.insert.flip_facing`.
//!
//! The place commands take the host wall (else the selected wall, else the wall of the selected
//! insert), the distance of the insert's center from the baseline's start (else the middle of the
//! wall's clear span), and optionally the type, the width, height, sill height, the hand and the
//! facing. Without options an insert takes the size, frame, operation and sill of the document's
//! first type of its kind, left hand, facing the exterior side. One call is one transaction,
//! so one undo step; a chain of inserts placed in a row is one undo step each.
//!
//! The prompt machine ([`InsertPrompts`]) asks for a distance along the wall; the answers `pick`
//! (a wall the host's pointer is on) and the keywords `width`, `height`, `sill`, `flip hand` and
//! `flip facing` come first and return to the distance. Typed distances read in the session's
//! context unit.

use std::collections::BTreeSet;
use std::str::FromStr;
use std::sync::Arc;

use arc_base::{Error, ObjectId, Result};
use arc_command::prompt::{
    Accept, KeywordOption, Prompt, PromptInput, PromptMachine, PromptState, Step,
};
use arc_command::{CommandSpec, Context, IdParam, QuantityParam, Registry};
use arc_doc::{FieldPath, Snapshot, Value};
use archbuilder_model::element_type::{InsertType, TypeKind};
use archbuilder_model::insert::{self, Facing, HEAD_CLEARANCE, Hand, Insert, Kind};
use archbuilder_model::spatial;
use archbuilder_model::types::{
    self, DOOR, INSERT_HEIGHT_RANGE, INSERT_WIDTH_RANGE, RISE_RANGE, SILL_RANGE, WALL,
    WALL_OPENING, WINDOW,
};
use archbuilder_model::units::{decimal_separator, length_text};
use archbuilder_model::wall::Wall;
use archbuilder_model::{defaults, names};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json};

use crate::error::codes;
use crate::structure::{LANGUAGE_SETTING, create, ensure_valid, expect, language, named};

/// The identifier of the place-door command.
pub const DOOR_PLACE: &str = "archbuilder.door.place";
/// The identifier of the place-window command.
pub const WINDOW_PLACE: &str = "archbuilder.window.place";
/// The identifier of the place-wall-opening command.
pub const OPENING_PLACE: &str = "archbuilder.opening.place";
/// The default width of a wall opening, in meters.
pub const OPENING_WIDTH: f64 = 1.0;
/// The default height of a wall opening, in meters.
pub const OPENING_HEIGHT: f64 = 2.1;
/// The identifier of the command that flips the hand of inserts.
pub const INSERT_FLIP_HAND: &str = "archbuilder.insert.flip_hand";
/// The identifier of the command that flips the facing of inserts.
pub const INSERT_FLIP_FACING: &str = "archbuilder.insert.flip_facing";

/// The identifier of the command that moves inserts to another wall.
pub const INSERT_REHOST: &str = "archbuilder.insert.rehost";

/// The identifier of the command that reports the rooms of doors and windows.
pub const INSERT_ROOMS: &str = "archbuilder.insert.rooms";

/// The most inserts one flip names.
const MAX_FLIPPED: usize = 100_000;
/// The default width of a door, in meters.
pub const DOOR_WIDTH: f64 = 0.9;
/// The default height of a door, in meters.
pub const DOOR_HEIGHT: f64 = 2.1;
/// The default width of a window, in meters.
pub const WINDOW_WIDTH: f64 = 1.2;
/// The default height of a window, in meters.
pub const WINDOW_HEIGHT: f64 = 1.2;
/// The default sill height of a window, in meters.
pub const WINDOW_SILL: f64 = 0.9;

/// Parameters of `archbuilder.door.place` and `archbuilder.window.place`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceInsert {
    /// The host wall; else the selected wall, else the wall of the selected insert.
    #[serde(default)]
    pub wall: Option<IdParam>,
    /// The distance from the wall's baseline start to the center of the opening, a number in
    /// meters or a text with a unit; the middle of the wall's clear span when absent.
    #[serde(default)]
    pub distance: Option<QuantityParam>,
    /// The door or window type: its identifier or name; else the document's first type of the
    /// kind.
    #[serde(default, rename = "type")]
    #[schemars(length(max = 256))]
    pub insert_type: Option<String>,
    /// The width of the opening; the type's when absent. A width or height other than the type's
    /// gives the insert a type of that size with the type's frame and operation (an existing
    /// one, else a new one).
    #[serde(default)]
    pub width: Option<QuantityParam>,
    /// The height of the opening; the type's when absent, lowered to fit under the wall's top.
    #[serde(default)]
    pub height: Option<QuantityParam>,
    /// The sill height above the wall's base; the type's when absent.
    #[serde(default)]
    pub sill: Option<QuantityParam>,
    /// The operation recipe (single swing, double swing, sliding, folding, fixed, casement,
    /// awning, hopper or pivot); the type's when absent. Another operation than the type's gives
    /// the insert a type of that operation with the type's size and frame (an existing one, else
    /// a new one).
    #[serde(default)]
    pub operation: Option<crate::element_type::OperationParam>,
    /// Whether the hand is flipped from the default (left): the hinge is on the right.
    #[serde(default)]
    pub flip_hand: Option<bool>,
    /// Whether the facing is flipped from the default (exterior): the leaf swings to the interior.
    #[serde(default)]
    pub flip_facing: Option<bool>,
    /// A wall opening only: the rise of its arched top, in meters; 0 (a rectangle) when absent.
    #[serde(default)]
    pub rise: Option<QuantityParam>,
    /// A wall opening only: its outline sketched in the wall's elevation, three to 32 points in
    /// meters, x from the baseline's start and y above the wall's base. The distance, width,
    /// height and sill come from the outline's bounds, so none of those may be given with it.
    #[serde(default)]
    #[schemars(length(min = 3, max = 32))]
    pub profile: Option<Vec<[f64; 2]>>,
    /// The name; else the first free `Door <n>`, `Window <n>` or `Opening <n>`.
    #[serde(default)]
    pub name: Option<String>,
}

/// What a place command returns: the insert and the values it was made with, which a host that
/// places inserts in a row carries to the next one.
#[derive(Debug, Serialize, JsonSchema)]
pub struct InsertPlaced {
    /// The new insert.
    pub insert: IdParam,
    /// Its host wall.
    pub wall: IdParam,
    /// The distance of its center from the baseline's start, in meters.
    pub distance: f64,
    /// The width in meters.
    pub width: f64,
    /// The height in meters.
    pub height: f64,
    /// The sill height in meters.
    pub sill: f64,
    /// The rise of the arch of a wall opening, in meters; 0 for the others.
    pub rise: f64,
    /// The type.
    #[serde(rename = "type")]
    pub insert_type: IdParam,
    /// Whether the hand is the flipped one (right).
    pub flip_hand: bool,
    /// Whether the facing is the flipped one (interior).
    pub flip_facing: bool,
}

/// Parameters of `archbuilder.insert.flip_hand` and `archbuilder.insert.flip_facing`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FlipInserts {
    /// The doors and windows; else those of the selection.
    #[serde(default)]
    pub inserts: Option<Vec<IdParam>>,
}

/// Parameters of `archbuilder.insert.rehost`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RehostInserts {
    /// The doors, windows and wall openings to move; else those of the selection.
    #[serde(default)]
    pub inserts: Option<Vec<IdParam>>,
    /// The wall that hosts them from now on; else the selected wall.
    #[serde(default)]
    pub wall: Option<IdParam>,
    /// The distance of the center from the new wall's baseline start, for one insert; else each
    /// keeps its place in the plan, projected onto the new baseline.
    #[serde(default)]
    pub distance: Option<QuantityParam>,
}

/// What `archbuilder.insert.rehost` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct InsertsRehosted {
    /// The inserts moved.
    pub inserts: Vec<IdParam>,
    /// The new host.
    pub wall: IdParam,
}

/// Parameters of `archbuilder.insert.rooms`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InsertRooms {
    /// The doors and windows; else those of the selection, else every one of the document.
    #[serde(default)]
    pub inserts: Option<Vec<IdParam>>,
}

/// The rooms of one door or window.
#[derive(Debug, Serialize, JsonSchema)]
pub struct InsertRelation {
    /// The door or window.
    pub insert: IdParam,
    /// Its name.
    pub name: String,
    /// `door` or `window`.
    pub kind: String,
    /// The room the leaf swings away from (a door), or on the facing side (a window).
    pub from_room: Option<IdParam>,
    /// Its name.
    pub from_name: Option<String>,
    /// The room the leaf swings toward (a door), or the interior room opposite the facing (a
    /// window).
    pub to_room: Option<IdParam>,
    /// Its name.
    pub to_name: Option<String>,
}

/// What `archbuilder.insert.rooms` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct RoomsOfInserts {
    /// One entry for each door or window, in identifier order.
    pub inserts: Vec<InsertRelation>,
}

/// The insert kinds the selection holds, in selection order.
fn selected_inserts(ctx: &Context<'_>) -> Vec<ObjectId> {
    let snapshot = ctx.snapshot();
    ctx.selection()
        .iter()
        .copied()
        .filter(|id| {
            matches!(
                spatial::type_of(snapshot, *id).as_deref(),
                Some(DOOR | WINDOW)
            )
        })
        .collect()
}

/// Whether `id` is a door or a window.
pub(crate) fn is_insert(snapshot: &Snapshot, id: ObjectId) -> bool {
    matches!(
        spatial::type_of(snapshot, id).as_deref(),
        Some(DOOR | WINDOW | WALL_OPENING)
    )
}

/// The wall an insert goes in: the given one, the selected wall, else the wall of a selected insert.
fn wall_for(ctx: &Context<'_>, given: Option<IdParam>) -> Result<arc_doc::Object> {
    let snapshot = ctx.snapshot();
    if let Some(IdParam(id)) = given {
        return expect(snapshot, id, WALL);
    }
    for id in ctx.selection() {
        let found = match spatial::type_of(snapshot, *id).as_deref() {
            Some(WALL) => spatial::get(snapshot, *id),
            Some(DOOR | WINDOW | WALL_OPENING) => spatial::get(snapshot, *id)
                .and_then(|i| i.header.owner)
                .and_then(|owner| spatial::get(snapshot, owner)),
            _ => None,
        };
        if let Some(found) = found {
            return Ok((*found).clone());
        }
    }
    Err(Error::new(codes::NO_WALL_SELECTED))
}

pub(crate) fn next_name(snapshot: &Snapshot, kind: Kind, wall: ObjectId, language: &str) -> String {
    let used = spatial::children(snapshot, kind.key(), wall);
    let taken = |name: &str| {
        used.iter().any(|o| {
            types::text(&o.values, types::insert::NAME).is_some_and(|n| names::same(n, name))
        })
    };
    let window = kind == Kind::Window;
    let total = spatial::objects(snapshot, kind.key()).len();
    (total + 1..)
        .map(|n| {
            if kind == Kind::Opening {
                defaults::wall_opening(language, n)
            } else {
                defaults::insert(language, window, n)
            }
        })
        .find(|name| !taken(name))
        .unwrap_or_default()
}

#[allow(
    clippy::too_many_lines,
    reason = "the options of one placement, in order"
)]
fn place(ctx: &mut Context<'_>, kind: Kind, p: PlaceInsert) -> Result<InsertPlaced> {
    if kind == Kind::Opening {
        return place_opening(ctx, p);
    }
    let wall_object = wall_for(ctx, p.wall)?;
    let wall =
        Wall::of(ctx.snapshot(), &wall_object).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let meters = |ctx: &Context<'_>, param: &Option<QuantityParam>| match param {
        Some(param) => crate::length(ctx, param).map(|q| Some(q.value())),
        None => Ok(None),
    };
    let type_kind = if kind == Kind::Door {
        TypeKind::Door
    } else {
        TypeKind::Window
    };
    let chosen = crate::wall::choose_type(ctx, type_kind, p.insert_type.as_deref())?;
    let base = spatial::get(ctx.txn()?.snapshot(), chosen)
        .and_then(|t| InsertType::from_values(&t.values))
        .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let sill = meters(ctx, &p.sill)?.unwrap_or(base.sill);
    let width = meters(ctx, &p.width)?.unwrap_or(base.width);
    // The type's height gives way to a low wall; a typed one is checked as it is.
    let height = match meters(ctx, &p.height)? {
        Some(height) => height,
        None => base
            .height
            .min(wall.height - sill - HEAD_CLEARANCE)
            .max(INSERT_HEIGHT_RANGE.0),
    };
    let same = |a: f64, b: f64| (a - b).abs() <= 1e-9;
    let operation = p.operation.map_or(base.operation, insert::Operation::from);
    let insert_type =
        if same(width, base.width) && same(height, base.height) && operation == base.operation {
            chosen
        } else {
            crate::element_type::ensure(
                ctx,
                type_kind,
                &[
                    (types::insert::WIDTH, types::length(width)),
                    (types::insert::HEIGHT, types::length(height)),
                    (types::insert::FRAME_WIDTH, types::length(base.frame_width)),
                    (types::insert::FRAME_DEPTH, types::length(base.frame_depth)),
                    (types::insert::OPERATION, Value::Enum(operation.code())),
                ],
            )?
        };
    let distance = match meters(ctx, &p.distance)? {
        Some(d) => d,
        None => insert::clear_span_of(ctx.snapshot(), &wall_object)
            .map_or(wall.length() / 2.0, |(low, high)| (low + high) / 2.0),
    };
    let hand = if p.flip_hand.unwrap_or(false) {
        Hand::Right
    } else {
        Hand::Left
    };
    let facing = if p.flip_facing.unwrap_or(false) {
        Facing::Interior
    } else {
        Facing::Exterior
    };
    if spatial::children(ctx.snapshot(), kind.key(), wall_object.id).len()
        + spatial::children(
            ctx.snapshot(),
            if kind == Kind::Door { WINDOW } else { DOOR },
            wall_object.id,
        )
        .len()
        >= insert::MAX_PER_WALL
    {
        return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
    }
    let name = match p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => name.to_owned(),
        None => next_name(ctx.snapshot(), kind, wall_object.id, &language(ctx)),
    };
    let id = create(
        ctx,
        kind.key(),
        Some(wall_object.id),
        vec![
            Some(named(&name)),
            Some(types::length(distance)),
            Some(types::length(width)),
            Some(types::length(height)),
            Some(types::length(sill)),
            Some(types::length(base.frame_width)),
            Some(types::length(base.frame_depth)),
            Some(Value::Enum(operation.code())),
            Some(Value::Enum(hand.code())),
            Some(Value::Enum(facing.code())),
            None,
        ],
    )?;
    crate::element_type::assign(ctx, id, type_kind, insert_type)?;
    // The sill is the instance's own: the given one, else the type's default.
    ctx.txn()?.modify(
        id,
        arc_doc::FieldPath::new("sill"),
        Some(types::length(sill)),
    )?;
    ensure_valid(ctx)?;
    Ok(InsertPlaced {
        insert: IdParam(id),
        wall: IdParam(wall_object.id),
        distance,
        width,
        height,
        sill,
        rise: 0.0,
        insert_type: IdParam(insert_type),
        flip_hand: hand == Hand::Right,
        flip_facing: facing == Facing::Interior,
    })
}

/// Places a bare opening: a rectangle or an arch in the wall, sized by width, height, sill and
/// rise. It has no type; it moves with its wall like a door.
#[allow(
    clippy::too_many_lines,
    reason = "the options of one opening, in order"
)]
fn place_opening(ctx: &mut Context<'_>, p: PlaceInsert) -> Result<InsertPlaced> {
    let wall_object = wall_for(ctx, p.wall)?;
    let wall =
        Wall::of(ctx.snapshot(), &wall_object).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let meters = |ctx: &Context<'_>, param: &Option<QuantityParam>| match param {
        Some(param) => crate::length(ctx, param).map(|q| Some(q.value())),
        None => Ok(None),
    };
    let mut sketch: Option<Vec<[f64; 2]>> = None;
    let (mut sill, mut width, mut height, mut rise, mut distance) = (
        meters(ctx, &p.sill)?,
        meters(ctx, &p.width)?,
        meters(ctx, &p.height)?,
        meters(ctx, &p.rise)?,
        meters(ctx, &p.distance)?,
    );
    if let Some(points) = &p.profile {
        let given = [sill, width, height, rise, distance];
        if given.iter().any(Option::is_some)
            || points.len() < 3
            || points.len() > insert::MAX_PROFILE
            || points.iter().flatten().any(|v| !v.is_finite())
        {
            return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
        }
        let bounds = points
            .iter()
            .fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), q| {
                (
                    [lo[0].min(q[0]), lo[1].min(q[1])],
                    [hi[0].max(q[0]), hi[1].max(q[1])],
                )
            });
        (distance, width) = (
            Some((bounds.0[0] + bounds.1[0]) / 2.0),
            Some(bounds.1[0] - bounds.0[0]),
        );
        (sill, height, rise) = (
            Some(bounds.0[1]),
            Some(bounds.1[1] - bounds.0[1]),
            Some(0.0),
        );
        sketch = Some(
            points
                .iter()
                .map(|q| [q[0] - bounds.0[0], q[1] - bounds.0[1]])
                .collect(),
        );
    }
    let sill = sill.unwrap_or(0.0);
    let width = width.unwrap_or(OPENING_WIDTH);
    let height = match height {
        Some(height) => height,
        None => OPENING_HEIGHT
            .min(wall.height - sill - HEAD_CLEARANCE)
            .max(INSERT_HEIGHT_RANGE.0),
    };
    let rise = rise.unwrap_or(0.0);
    if rise > height || !(RISE_RANGE.0..=RISE_RANGE.1).contains(&rise) {
        return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
    }
    let distance = match distance {
        Some(d) => d,
        None => insert::clear_span_of(ctx.snapshot(), &wall_object)
            .map_or(wall.length() / 2.0, |(low, high)| (low + high) / 2.0),
    };
    if insert::hosted(ctx.snapshot(), wall_object.id).len() >= insert::MAX_PER_WALL {
        return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
    }
    let name = match p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => name.to_owned(),
        None => next_name(
            ctx.snapshot(),
            Kind::Opening,
            wall_object.id,
            &language(ctx),
        ),
    };
    let id = create(
        ctx,
        WALL_OPENING,
        Some(wall_object.id),
        vec![
            Some(named(&name)),
            Some(types::length(distance)),
            Some(types::length(width)),
            Some(types::length(height)),
            Some(types::length(sill)),
            Some(types::length(rise)),
            None,
            sketch.as_deref().map(insert::profile_values),
        ],
    )?;
    ensure_valid(ctx)?;
    ctx.set_selection_after(vec![id]);
    Ok(InsertPlaced {
        insert: IdParam(id),
        wall: IdParam(wall_object.id),
        distance,
        width,
        height,
        sill,
        rise,
        // A bare opening has no type; the wall stands for it.
        insert_type: IdParam(wall_object.id),
        flip_hand: false,
        flip_facing: false,
    })
}

/// Flips one field of the inserts, in one transaction.
fn flip(ctx: &mut Context<'_>, given: Option<Vec<IdParam>>, hand: bool) -> Result<()> {
    let ids: BTreeSet<ObjectId> = match given {
        Some(list) if list.len() > MAX_FLIPPED => {
            return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
        }
        Some(list) => list.into_iter().map(|IdParam(id)| id).collect(),
        None => selected_inserts(ctx).into_iter().collect(),
    };
    if ids.is_empty() {
        return Err(Error::new(codes::NO_INSERT_SELECTED));
    }
    for id in &ids {
        if !matches!(
            spatial::type_of(ctx.snapshot(), *id).as_deref(),
            Some(DOOR | WINDOW)
        ) {
            return Err(Error::new(codes::KIND_MISMATCH));
        }
    }
    for id in ids {
        let Some(item) =
            spatial::get(ctx.snapshot(), id).and_then(|o| Insert::of(ctx.snapshot(), &o))
        else {
            continue;
        };
        // The hand is read from the side the leaf swings toward, so flipping the facing flips the
        // hand with it: the hinge stays on the same jamb.
        let mut fields = vec![if hand {
            ("hand", item.hand.flipped().code())
        } else {
            ("facing", item.facing.flipped().code())
        }];
        if !hand {
            fields.push(("hand", item.hand.flipped().code()));
        }
        for (field, code) in fields {
            ctx.txn()?
                .modify(id, FieldPath::new(field), Some(Value::Enum(code)))?;
        }
    }
    ensure_valid(ctx)
}

/// Moves inserts to another wall in one transaction. An insert keeps its sill, size and kind; its
/// position is the given distance or its plan position projected onto the new baseline. The
/// model's rules refuse a result outside the wall, in another insert's way or too tall, naming the
/// insert and the wall or insert involved, and nothing changes.
fn rehost(ctx: &mut Context<'_>, p: RehostInserts) -> Result<InsertsRehosted> {
    let target = match p.wall {
        Some(IdParam(id)) => id,
        None => ctx
            .selection()
            .iter()
            .copied()
            .find(|id| spatial::type_of(ctx.snapshot(), *id).as_deref() == Some(WALL))
            .ok_or_else(|| Error::new(codes::NO_WALL_SELECTED))?,
    };
    let target_object = expect(ctx.snapshot(), target, WALL)?;
    let target_wall =
        Wall::of(ctx.snapshot(), &target_object).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let ids: Vec<ObjectId> = match p.inserts {
        Some(list) if list.len() > MAX_FLIPPED => {
            return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
        }
        Some(list) => list
            .into_iter()
            .map(|IdParam(id)| id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        None => ctx
            .selection()
            .iter()
            .copied()
            .filter(|id| is_insert(ctx.snapshot(), *id))
            .collect(),
    };
    if ids.is_empty() {
        return Err(Error::new(codes::NO_INSERT_SELECTED));
    }
    let given = match &p.distance {
        Some(param) if ids.len() == 1 => Some(crate::length(ctx, param)?.value()),
        Some(_) => return Err(Error::new(arc_command::error::codes::PARAMS_INVALID)),
        None => None,
    };
    let length = target_wall.length();
    let dir = [
        (target_wall.end[0] - target_wall.start[0]) / length,
        (target_wall.end[1] - target_wall.start[1]) / length,
    ];
    for id in &ids {
        let snapshot = ctx.snapshot();
        if !is_insert(snapshot, *id) {
            return Err(Error::new(codes::KIND_MISMATCH));
        }
        let object = spatial::get(snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let distance = match given {
            Some(d) => d,
            None => {
                let (old, item) = object
                    .header
                    .owner
                    .and_then(|h| spatial::get(snapshot, h))
                    .and_then(|h| Wall::of(snapshot, &h))
                    .zip(Insert::of(snapshot, &object))
                    .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
                let old_length = old.length();
                let at = [
                    old.start[0] + (old.end[0] - old.start[0]) / old_length * item.distance,
                    old.start[1] + (old.end[1] - old.start[1]) / old_length * item.distance,
                ];
                (at[0] - target_wall.start[0]) * dir[0] + (at[1] - target_wall.start[1]) * dir[1]
            }
        };
        if spatial::children(snapshot, DOOR, target).len()
            + spatial::children(snapshot, WINDOW, target).len()
            + spatial::children(snapshot, WALL_OPENING, target).len()
            >= insert::MAX_PER_WALL
        {
            return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
        }
        let txn = ctx.txn()?;
        txn.relink(*id, Some(target))?;
        txn.modify(
            *id,
            FieldPath::new("distance"),
            Some(types::length(distance)),
        )?;
    }
    ensure_valid(ctx)?;
    ctx.set_selection_after(ids.clone());
    Ok(InsertsRehosted {
        inserts: ids.into_iter().map(IdParam).collect(),
        wall: IdParam(target),
    })
}

/// The rooms of doors and windows, derived at the call (read only).
fn rooms(ctx: &Context<'_>, p: InsertRooms) -> Result<RoomsOfInserts> {
    let snapshot = ctx.snapshot();
    let mut ids: Vec<ObjectId> = match p.inserts {
        Some(list) if list.len() > MAX_FLIPPED => {
            return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
        }
        Some(list) => list.into_iter().map(|IdParam(id)| id).collect(),
        None => selected_inserts(ctx),
    };
    if ids.is_empty() {
        ids = [DOOR, WINDOW]
            .iter()
            .flat_map(|key| spatial::objects(snapshot, key))
            .map(|o| o.id)
            .take(MAX_FLIPPED)
            .collect();
    }
    ids.sort();
    ids.dedup();
    let name_of = |id: Option<ObjectId>| {
        id.and_then(|id| spatial::get(snapshot, id))
            .and_then(|o| types::text(&o.values, types::room::NAME).map(str::to_owned))
    };
    let mut inserts = Vec::new();
    for id in ids {
        let object = spatial::get(snapshot, id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let kind = match spatial::type_of(snapshot, id).as_deref() {
            Some(DOOR) => "door",
            Some(WINDOW) => "window",
            _ => return Err(Error::new(codes::KIND_MISMATCH)),
        };
        let relation = insert::relation(snapshot, &object).unwrap_or_default();
        inserts.push(InsertRelation {
            insert: IdParam(id),
            name: types::text(&object.values, types::insert::NAME)
                .unwrap_or_default()
                .to_owned(),
            kind: kind.to_owned(),
            from_room: relation.from.map(IdParam),
            from_name: name_of(relation.from),
            to_room: relation.to.map(IdParam),
            to_name: name_of(relation.to),
        });
    }
    Ok(RoomsOfInserts { inserts })
}

/// A message of the catalog, named by identifier.
struct Msg {
    message_id: &'static str,
}

const PROMPT_DOOR: Msg = Msg {
    message_id: "ab-prompt-door-place",
};
const PROMPT_WINDOW: Msg = Msg {
    message_id: "ab-prompt-window-place",
};
const PROMPT_OPENING: Msg = Msg {
    message_id: "ab-prompt-opening-place",
};
const PROMPT_RISE: Msg = Msg {
    message_id: "ab-prompt-opening-rise",
};
const KEY_RISE: Msg = Msg {
    message_id: "ab-keyword-opening-rise",
};
const PROMPT_WIDTH: Msg = Msg {
    message_id: "ab-prompt-insert-width",
};
const PROMPT_HEIGHT: Msg = Msg {
    message_id: "ab-prompt-insert-height",
};
const PROMPT_SILL: Msg = Msg {
    message_id: "ab-prompt-insert-sill",
};
const KEY_WIDTH: Msg = Msg {
    message_id: "ab-keyword-insert-width",
};
const KEY_HEIGHT: Msg = Msg {
    message_id: "ab-keyword-insert-height",
};
const KEY_SILL: Msg = Msg {
    message_id: "ab-keyword-insert-sill",
};
const KEY_HAND: Msg = Msg {
    message_id: "ab-keyword-insert-hand",
};
const KEY_FACING: Msg = Msg {
    message_id: "ab-keyword-insert-facing",
};

/// The prompts of `archbuilder.door.place` and `archbuilder.window.place`; see the module
/// documentation.
#[derive(Debug)]
pub struct InsertPrompts {
    kind: Kind,
}

/// What the prompts show: the wall and the options in effect.
struct Shown {
    wall: String,
    width: f64,
    height: f64,
    sill: f64,
    rise: f64,
    flip_hand: bool,
    flip_facing: bool,
    turkish: bool,
    separator: char,
    unit: archbuilder_model::units::UnitSettings,
}

impl Shown {
    fn of(
        state: &PromptState<'_>,
        over: &Map<String, Json>,
        kind: Kind,
        wall: Option<ObjectId>,
    ) -> Self {
        let snapshot = state.snapshot;
        let number = |key: &str| {
            over.get(key)
                .or_else(|| state.preset.get(key))
                .and_then(Json::as_f64)
        };
        let flag = |key: &str| {
            over.get(key)
                .or_else(|| state.preset.get(key))
                .and_then(Json::as_bool)
                .unwrap_or(false)
        };
        let language = state
            .setting(LANGUAGE_SETTING)
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        // The type's size and sill, unless an answer or the host gave others.
        let type_kind = match kind {
            Kind::Door | Kind::Opening => TypeKind::Door,
            Kind::Window => TypeKind::Window,
        };
        let chosen = over
            .get("type")
            .or_else(|| state.preset.get("type"))
            .and_then(Json::as_str)
            .and_then(|t| crate::element_type::resolve(snapshot, type_kind, t).ok())
            .or_else(|| crate::element_type::default_of(snapshot, type_kind))
            .and_then(|id| spatial::get(snapshot, id))
            .and_then(|t| InsertType::from_values(&t.values));
        let (width, height, sill) = chosen.map_or(
            match kind {
                Kind::Door => (DOOR_WIDTH, DOOR_HEIGHT, 0.0),
                Kind::Opening => (OPENING_WIDTH, OPENING_HEIGHT, 0.0),
                Kind::Window => (WINDOW_WIDTH, WINDOW_HEIGHT, WINDOW_SILL),
            },
            |t| match kind {
                Kind::Opening => (OPENING_WIDTH, OPENING_HEIGHT, 0.0),
                _ => (t.width, t.height, t.sill),
            },
        );
        Self {
            wall: wall
                .and_then(|id| spatial::get(snapshot, id))
                .and_then(|w| types::text(&w.values, 0).map(str::to_owned))
                .unwrap_or_else(|| "-".to_owned()),
            width: number("width").unwrap_or(width),
            height: number("height").unwrap_or(height),
            sill: number("sill").unwrap_or(sill),
            rise: number("rise").unwrap_or(0.0),
            flip_hand: flag("flip_hand"),
            flip_facing: flag("flip_facing"),
            turkish: language.starts_with("tr"),
            separator: decimal_separator(&language),
            unit: spatial::unit_settings(snapshot),
        }
    }

    fn text(&self, meters: f64) -> String {
        length_text(meters, self.unit, self.separator)
    }

    fn arg(&self, prompt: Prompt) -> Prompt {
        prompt
            .with_arg("wall", self.wall.as_str())
            .with_arg("width", self.text(self.width))
            .with_arg("height", self.text(self.height))
            .with_arg("sill", self.text(self.sill))
            .with_arg("hand", if self.flip_hand { "right" } else { "left" })
            .with_arg(
                "facing",
                if self.flip_facing {
                    "interior"
                } else {
                    "exterior"
                },
            )
            // The values in effect as plain numbers, for a host that draws a preview.
            .with_arg("width_m", self.width.to_string())
            .with_arg("height_m", self.height.to_string())
            .with_arg("sill_m", self.sill.to_string())
            .with_arg("rise_m", self.rise.to_string())
    }

    fn shortcut(&self, english: char, turkish: char) -> char {
        if self.turkish { turkish } else { english }
    }
}

fn distance(range: (f64, f64)) -> Accept {
    Accept::Distance {
        min: Some(range.0),
        max: Some(range.1),
    }
}

fn id_of(value: Option<&Json>) -> Option<ObjectId> {
    ObjectId::from_str(value?.as_str()?).ok()
}

impl InsertPrompts {
    /// Reads the answers: the wall (preset or picked) and the options set on the way. Returns the
    /// wall known so far and the next step.
    fn scan(&self, state: &PromptState<'_>) -> (Option<ObjectId>, Step) {
        let mut over = Map::new();
        let mut wall = id_of(state.preset.get("wall"));
        let mut at = 0;
        loop {
            let shown = Shown::of(state, &over, self.kind, wall);
            let Some(answer) = state.answers.get(at) else {
                return (
                    wall,
                    Step::Ask(shown.arg(self.point_prompt(format!("point-{at}"), &shown))),
                );
            };
            at += 1;
            match &answer.input {
                PromptInput::Pick(id) => wall = Some(*id),
                PromptInput::Distance(d) => {
                    let mut params = over;
                    // What the chain carried reaches the command with the answer.
                    for key in ["width", "height", "sill", "rise"] {
                        if let Some(v) = state.preset.get(key)
                            && !params.contains_key(key)
                        {
                            params.insert(key.to_owned(), v.clone());
                        }
                    }
                    params.insert("distance".to_owned(), Json::from(*d));
                    if let Some(wall) = wall {
                        params.insert("wall".to_owned(), Json::from(wall.to_string()));
                    }
                    return (wall, Step::Done(params));
                }
                PromptInput::Keyword(k) if k == "flip_hand" || k == "flip_facing" => {
                    let key = if k == "flip_hand" {
                        "flip_hand"
                    } else {
                        "flip_facing"
                    };
                    let now = over
                        .get(key)
                        .or_else(|| state.preset.get(key))
                        .and_then(Json::as_bool)
                        .unwrap_or(false);
                    over.insert(key.to_owned(), Json::from(!now));
                }
                PromptInput::Keyword(k) => {
                    let (message, range, current) = match k.as_str() {
                        "width" => (&PROMPT_WIDTH, INSERT_WIDTH_RANGE, shown.width),
                        "height" => (&PROMPT_HEIGHT, INSERT_HEIGHT_RANGE, shown.height),
                        "rise" => (&PROMPT_RISE, RISE_RANGE, 0.0),
                        _ => (&PROMPT_SILL, SILL_RANGE, shown.sill),
                    };
                    let ask =
                        Prompt::new(format!("value-{at}"), message.message_id, distance(range))
                            .with_default(PromptInput::Distance(current));
                    let Some(value) = state.answers.get(at) else {
                        return (wall, Step::Ask(shown.arg(ask)));
                    };
                    at += 1;
                    if let PromptInput::Distance(d) = &value.input {
                        over.insert(k.clone(), Json::from(*d));
                    }
                }
                _ => {}
            }
        }
    }

    fn point_prompt(&self, key: String, shown: &Shown) -> Prompt {
        let message = match self.kind {
            Kind::Door => &PROMPT_DOOR,
            Kind::Window => &PROMPT_WINDOW,
            Kind::Opening => &PROMPT_OPENING,
        };
        let mut options = vec![
            KeywordOption::new("width", KEY_WIDTH.message_id)
                .with_shortcut(shown.shortcut('w', 'g')),
            KeywordOption::new("height", KEY_HEIGHT.message_id)
                .with_shortcut(shown.shortcut('h', 'y')),
        ];
        if self.kind != Kind::Door {
            options.push(
                KeywordOption::new("sill", KEY_SILL.message_id)
                    .with_shortcut(shown.shortcut('s', 'd')),
            );
        }
        if self.kind == Kind::Opening {
            options.push(
                KeywordOption::new("rise", KEY_RISE.message_id)
                    .with_shortcut(shown.shortcut('r', 'y')),
            );
        } else {
            options.push(
                KeywordOption::new("flip_hand", KEY_HAND.message_id)
                    .with_shortcut(shown.shortcut('f', 'k')),
            );
            options.push(
                KeywordOption::new("flip_facing", KEY_FACING.message_id)
                    .with_shortcut(shown.shortcut('a', 'a')),
            );
        }
        Prompt::new(
            key,
            message.message_id,
            Accept::Distance {
                min: Some(0.0),
                max: Some(types::DISTANCE_RANGE.1),
            },
        )
        .or(Accept::Pick {
            types: vec![WALL.to_owned()],
        })
        .or(Accept::keywords(options))
    }
}

impl PromptMachine for InsertPrompts {
    fn next(&self, state: &PromptState<'_>) -> Result<Step> {
        Ok(self.scan(state).1)
    }

    fn check(&self, state: &PromptState<'_>, prompt: &Prompt, input: &PromptInput) -> Result<()> {
        // A distance along a wall needs a wall: the preset one or one picked before. The value
        // of an option (width, height, sill) needs none.
        if matches!(input, PromptInput::Distance(_))
            && prompt.key.starts_with("point")
            && self.scan(state).0.is_none()
        {
            return Err(Error::new(codes::NO_WALL_SELECTED));
        }
        Ok(())
    }
}

fn spec(id: &str, label: Msg, description: Msg, kind: Kind) -> CommandSpec {
    CommandSpec::new(
        id,
        label.message_id,
        description.message_id,
        move |ctx: &mut Context<'_>, p: PlaceInsert| place(ctx, kind, p),
    )
    .with_prompt(Arc::new(InsertPrompts { kind }))
}

/// Registers the commands of this module.
///
/// # Errors
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(spec(
        DOOR_PLACE,
        Msg {
            message_id: "ab-cmd-door-place-label",
        },
        Msg {
            message_id: "ab-cmd-door-place-description",
        },
        Kind::Door,
    ))?;
    registry.register(spec(
        OPENING_PLACE,
        Msg {
            message_id: "ab-cmd-opening-place-label",
        },
        Msg {
            message_id: "ab-cmd-opening-place-description",
        },
        Kind::Opening,
    ))?;
    registry.register(spec(
        WINDOW_PLACE,
        Msg {
            message_id: "ab-cmd-window-place-label",
        },
        Msg {
            message_id: "ab-cmd-window-place-description",
        },
        Kind::Window,
    ))?;
    const FLIP_HAND_LABEL: Msg = Msg {
        message_id: "ab-cmd-insert-flip-hand-label",
    };
    const FLIP_HAND_DESCRIPTION: Msg = Msg {
        message_id: "ab-cmd-insert-flip-hand-description",
    };
    const FLIP_FACING_LABEL: Msg = Msg {
        message_id: "ab-cmd-insert-flip-facing-label",
    };
    const FLIP_FACING_DESCRIPTION: Msg = Msg {
        message_id: "ab-cmd-insert-flip-facing-description",
    };
    registry.register(CommandSpec::new(
        INSERT_FLIP_HAND,
        FLIP_HAND_LABEL.message_id,
        FLIP_HAND_DESCRIPTION.message_id,
        |ctx: &mut Context<'_>, p: FlipInserts| flip(ctx, p.inserts, true),
    ))?;
    registry.register(CommandSpec::new(
        INSERT_FLIP_FACING,
        FLIP_FACING_LABEL.message_id,
        FLIP_FACING_DESCRIPTION.message_id,
        |ctx: &mut Context<'_>, p: FlipInserts| flip(ctx, p.inserts, false),
    ))?;
    registry.register(CommandSpec::new(
        INSERT_REHOST,
        Msg {
            message_id: "ab-cmd-insert-rehost-label",
        }
        .message_id,
        Msg {
            message_id: "ab-cmd-insert-rehost-description",
        }
        .message_id,
        |ctx: &mut Context<'_>, p: RehostInserts| rehost(ctx, p),
    ))?;
    registry.register(
        CommandSpec::new(
            INSERT_ROOMS,
            Msg {
                message_id: "ab-cmd-insert-rooms-label",
            }
            .message_id,
            Msg {
                message_id: "ab-cmd-insert-rooms-description",
            }
            .message_id,
            |ctx: &mut Context<'_>, p: InsertRooms| rooms(ctx, p),
        )
        .with_undo(arc_command::UndoBehavior::NotUndoable)
        .with_permissions([arc_command::Permission::Read]),
    )
}
