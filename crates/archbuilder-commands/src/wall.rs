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

use arc_base::{Dimension, Error, ObjectId, Result};
use arc_command::prompt::{
    Accept, KeywordOption, Point, PointDims, Prompt, PromptInput, PromptMachine, PromptState, Step,
};
use arc_command::{CommandSpec, Context, IdParam, QuantityParam, Registry};
use arc_doc::{Object, Snapshot, Value};
use archbuilder_model::spatial;
use archbuilder_model::types::{
    self, DEFAULT_THICKNESS, HEIGHT_RANGE, LEVEL, MIN_WALL_LENGTH, STOREY, THICKNESS_RANGE, WALL,
    storey,
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
}

impl From<LocationLineParam> for LocationLine {
    fn from(line: LocationLineParam) -> Self {
        match line {
            LocationLineParam::Center => Self::Center,
            LocationLineParam::Exterior => Self::Exterior,
            LocationLineParam::Interior => Self::Interior,
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
    /// The thickness, a number in meters or a text with a unit; 0.2 m when absent.
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
}

/// Deletes walls and inserts in one transaction, so one undo step brings them all back. The
/// inserts of a deleted wall go with it by the deletion rules of their host.
fn delete_set(
    ctx: &mut Context<'_>,
    walls: BTreeSet<ObjectId>,
    inserts: BTreeSet<ObjectId>,
) -> Result<Deleted> {
    if walls.is_empty() && inserts.is_empty() {
        return Err(Error::new(codes::NOTHING_SELECTED));
    }
    for id in &walls {
        expect(ctx.snapshot(), *id, WALL)?;
    }
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
    for id in walls.iter().chain(&alone) {
        txn.delete(*id)?;
    }
    txn.apply_delete_rules(&limits)?;
    ensure_valid(ctx)?;
    Ok(Deleted {
        walls: walls.len(),
        inserts: count,
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
    delete_set(ctx, ids, BTreeSet::new())
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
fn delete_selection(ctx: &mut Context<'_>, _: DeleteSelection) -> Result<Deleted> {
    let walls: BTreeSet<ObjectId> = selected_walls(ctx).into_iter().collect();
    let snapshot = ctx.snapshot();
    let inserts: BTreeSet<ObjectId> = ctx
        .selection()
        .iter()
        .copied()
        .filter(|id| crate::insert::is_insert(snapshot, *id))
        .collect();
    if walls.is_empty() && inserts.is_empty() {
        crate::structure::delete_storey(ctx, crate::structure::DeleteStorey::default())
            .map(|()| Deleted::default())
    } else {
        delete_set(ctx, walls, inserts)
    }
}

/// The storey a wall goes on: the given one, the selection's, else the lowest.
fn storey_for(ctx: &Context<'_>, given: Option<IdParam>) -> Result<Object> {
    let snapshot = ctx.snapshot();
    if let Some(IdParam(id)) = given {
        return expect(snapshot, id, STOREY);
    }
    for id in ctx.selection() {
        let found = match spatial::type_of(snapshot, *id).as_deref() {
            Some(STOREY) => spatial::get(snapshot, *id),
            Some(WALL) => spatial::get(snapshot, *id)
                .and_then(|w| w.header.owner)
                .and_then(|owner| spatial::get(snapshot, owner)),
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

fn next_wall_name(snapshot: &Snapshot, storey: ObjectId, language: &str) -> String {
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

fn create_wall(ctx: &mut Context<'_>, p: CreateWall) -> Result<WallCreated> {
    let storey = storey_for(ctx, p.storey)?;
    let meters = |ctx: &Context<'_>, param: &Option<QuantityParam>, default: f64| match param {
        Some(param) => ctx.quantity(param, Dimension::LENGTH).map(|q| q.value()),
        None => Ok(default),
    };
    let thickness = meters(ctx, &p.thickness, DEFAULT_THICKNESS)?;
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
            Some(types::length(thickness)),
            Some(types::length(height)),
            Some(types::length(base_offset)),
            Some(Value::Enum(location.code())),
            top_level.map(Value::Reference),
            Some(Value::Enum(JoinStyle::Auto.code())),
            Some(Value::Enum(JoinStyle::Auto.code())),
        ],
    )?;
    ensure_valid(ctx)?;
    Ok(WallCreated {
        wall: IdParam(id),
        storey: IdParam(storey.id),
        start: p.start,
        end: p.end,
        thickness,
        height,
        top_level: top_level.map(IdParam),
        flip,
        base_offset,
        location_line: match location {
            LocationLine::Center => LocationLineParam::Center,
            LocationLine::Exterior => LocationLineParam::Exterior,
            LocationLine::Interior => LocationLineParam::Interior,
        },
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
        let unit = spatial::length_unit(snapshot);
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
            Some("exterior") => "exterior",
            Some("interior") => "interior",
            _ => "center",
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
