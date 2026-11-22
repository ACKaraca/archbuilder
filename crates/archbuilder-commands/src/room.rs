//! `archbuilder.room.place` and `archbuilder.room.place_all`: rooms on a storey.
//!
//! A room is its placement point: the boundary, area, perimeter and volume are derived from the
//! walls around it ([`archbuilder_model::room`]). `place` takes `points`; each must lie inside a
//! closed loop of walls that has no room yet, and gets a room named `Room` (`Oda`) with the next
//! number of the storey (101, 102, ...) unless `name` or `number` say otherwise. `place_all` puts a
//! room in every enclosed region of the storey that has none. A region with a net area below
//! [`model::MIN_ROOM_AREA`] is a sliver: `place_all` skips it and reports how many it skipped, and
//! `place` refuses a point in one. One call is one transaction. The
//! prompt machine asks for one point per room; the tool chains them until Escape.

use std::str::FromStr;
use std::sync::Arc;

use arc_base::{Error, ObjectId, Result};
use arc_command::prompt::{
    Accept, PointDims, Prompt, PromptInput, PromptMachine, PromptState, Step,
};
use arc_command::{CommandSpec, Context, IdParam, Registry};
use arc_doc::Value;
use archbuilder_model::floor::{self, Boundary};
use archbuilder_model::room::{self as model, State};
use archbuilder_model::types::{self, ROOM};
use archbuilder_model::{defaults, spatial};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json};

use crate::error::codes;
use crate::structure::{create, ensure_valid, language, named};

/// The identifier of the place-room command.
pub const ROOM_PLACE: &str = "archbuilder.room.place";
/// The identifier of the command that places a room in every region that has none.
pub const ROOM_PLACE_ALL: &str = "archbuilder.room.place_all";

/// A message of the catalog, named by identifier.
struct Msg {
    message_id: &'static str,
}

const PROMPT_PLACE: Msg = Msg {
    message_id: "ab-prompt-room-place",
};
const PLACE_LABEL: Msg = Msg {
    message_id: "ab-cmd-room-place-label",
};
const PLACE_DESCRIPTION: Msg = Msg {
    message_id: "ab-cmd-room-place-description",
};
const PLACE_ALL_LABEL: Msg = Msg {
    message_id: "ab-cmd-room-place-all-label",
};
const PLACE_ALL_DESCRIPTION: Msg = Msg {
    message_id: "ab-cmd-room-place-all-description",
};

/// The most rooms one call places.
const MAX_PLACED: usize = 1000;

/// Parameters of `archbuilder.room.place`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceRoom {
    /// The storey; else the storey of the selection, else the lowest storey.
    #[serde(default)]
    pub storey: Option<IdParam>,
    /// The points `[x, y]` in meters, each inside a region closed by walls.
    pub points: Vec<[f64; 2]>,
    /// The name; `Room` (`Oda`) when absent.
    #[serde(default)]
    pub name: Option<String>,
    /// The number; the next free whole number of the storey from 101 when absent.
    #[serde(default)]
    pub number: Option<String>,
    /// The department.
    #[serde(default)]
    pub department: Option<String>,
}

/// Parameters of `archbuilder.room.place_all`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceAll {
    /// The storey; else the storey of the selection, else the lowest storey.
    #[serde(default)]
    pub storey: Option<IdParam>,
}

/// What the commands return.
#[derive(Debug, Serialize, JsonSchema)]
pub struct RoomsPlaced {
    /// The new rooms, in order.
    pub rooms: Vec<IdParam>,
    /// Their numbers.
    pub numbers: Vec<String>,
    /// The storey.
    pub storey: IdParam,
    /// The area of each new room in square meters.
    pub areas: Vec<f64>,
    /// Enclosed regions whose boundary could not be resolved and got no room (`place_all`).
    pub unresolved: usize,
    /// Enclosed regions smaller than the least room area that got no room (`place_all`).
    pub slivers: usize,
}

fn add(
    ctx: &mut Context<'_>,
    storey: ObjectId,
    point: [f64; 2],
    name: Option<&str>,
    number: Option<&str>,
    department: Option<&str>,
) -> Result<ObjectId> {
    let tongue = language(ctx);
    let seen = ctx.txn()?.snapshot().clone();
    let name = name
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| defaults::room(&tongue))
        .to_owned();
    let number = match number.map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) => n.to_owned(),
        None => {
            let rooms = spatial::children(&seen, ROOM, storey);
            model::next_number(
                rooms
                    .iter()
                    .filter_map(|r| types::text(&r.values, types::room::NUMBER)),
            )
        }
    };
    let text = |t: Option<&str>| {
        t.map(str::trim)
            .filter(|t| !t.is_empty())
            .map(|t| Value::String(t.to_owned()))
    };
    let mut values: Vec<Option<Value>> = vec![None; types::room::BASE_OFFSET + 1];
    values[types::room::NAME] = Some(named(&name));
    values[types::room::NUMBER] = Some(Value::String(number));
    values[types::room::X] = Some(types::length(point[0]));
    values[types::room::Y] = Some(types::length(point[1]));
    values[types::room::DEPARTMENT] = text(department);
    values[types::room::UPPER_OFFSET] = Some(types::length(0.0));
    values[types::room::BASE_OFFSET] = Some(types::length(0.0));
    create(ctx, ROOM, Some(storey), values)
}

fn finish(ctx: &mut Context<'_>, storey: ObjectId, ids: Vec<ObjectId>) -> Result<RoomsPlaced> {
    ensure_valid(ctx)?;
    let txn = ctx.txn()?;
    let snapshot = txn.snapshot().clone();
    let entries = model::of_storey(&snapshot, storey);
    let pick = |id: &ObjectId| entries.iter().find(|e| e.id == *id);
    let numbers = ids
        .iter()
        .map(|id| {
            pick(id)
                .and_then(|e| e.room.number.clone())
                .unwrap_or_default()
        })
        .collect();
    let areas = ids
        .iter()
        .map(|id| pick(id).and_then(model::Entry::area).unwrap_or(0.0))
        .collect();
    ctx.set_selection_after(ids.clone());
    Ok(RoomsPlaced {
        rooms: ids.into_iter().map(IdParam).collect(),
        numbers,
        storey: IdParam(storey),
        areas,
        unresolved: 0,
        slivers: 0,
    })
}

fn place(ctx: &mut Context<'_>, p: PlaceRoom) -> Result<RoomsPlaced> {
    let storey = crate::wall::storey_for(ctx, p.storey)?.id;
    if p.points.is_empty() || p.points.len() > MAX_PLACED {
        return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
    }
    let walls = floor::storey_walls(ctx.snapshot(), storey);
    let existing: Vec<[f64; 2]> = model::of_storey(ctx.snapshot(), storey)
        .iter()
        .map(|e| e.room.point)
        .collect();
    let mut all = existing.clone();
    all.extend(p.points.iter().copied());
    let prints = model::footprints(&walls, &all);
    let mut ids = Vec::new();
    for (k, point) in p.points.iter().enumerate() {
        let print = &prints[existing.len() + k];
        if print.state == State::Unenclosed {
            return Err(Error::new(codes::ROOM_NO_REGION));
        }
        if print.state == State::Redundant {
            return Err(Error::new(codes::ROOM_REGION_TAKEN));
        }
        if print.area().is_some_and(|a| a < model::MIN_ROOM_AREA) {
            return Err(Error::new(codes::ROOM_REGION_SLIVER)
                .with_arg("area", format!("{:.2}", print.area().unwrap_or(0.0)))
                .with_arg("least", format!("{:.2}", model::MIN_ROOM_AREA)));
        }
        ids.push(add(
            ctx,
            storey,
            *point,
            p.name.as_deref(),
            p.number.as_deref().filter(|_| p.points.len() == 1),
            p.department.as_deref(),
        )?);
    }
    finish(ctx, storey, ids)
}

fn place_all(ctx: &mut Context<'_>, p: PlaceAll) -> Result<RoomsPlaced> {
    let storey = crate::wall::storey_for(ctx, p.storey)?.id;
    let walls = floor::storey_walls(ctx.snapshot(), storey);
    let existing: Vec<[f64; 2]> = model::of_storey(ctx.snapshot(), storey)
        .iter()
        .map(|e| e.room.point)
        .collect();
    let model::Enclosed {
        regions,
        unresolved,
        slivers,
    } = model::enclosed_regions(&walls);
    let missing: Vec<[f64; 2]> = regions
        .into_iter()
        .filter(|(_, loops)| {
            let b = Boundary {
                loops: loops.clone(),
            };
            !existing.iter().any(|e| b.contains(*e))
        })
        .map(|(point, _)| point)
        .take(MAX_PLACED)
        .collect();
    if missing.is_empty() {
        return Err(if unresolved > 0 {
            Error::new(codes::ROOM_REGION_UNRESOLVED).with_arg("count", unresolved.to_string())
        } else if slivers > 0 {
            Error::new(codes::ROOM_ONLY_SLIVERS)
                .with_arg("count", slivers.to_string())
                .with_arg("least", format!("{:.2}", model::MIN_ROOM_AREA))
        } else {
            Error::new(codes::ROOM_NONE_MISSING)
        });
    }
    let mut ids = Vec::new();
    for point in missing {
        ids.push(add(ctx, storey, point, None, None, None)?);
    }
    let mut placed = finish(ctx, storey, ids)?;
    placed.unresolved = unresolved;
    placed.slivers = slivers;
    Ok(placed)
}

/// The prompt of `archbuilder.room.place`: one point per room.
#[derive(Debug)]
pub struct RoomPrompts;

impl PromptMachine for RoomPrompts {
    fn next(&self, state: &PromptState<'_>) -> Result<Step> {
        let name_of = |key: &str| {
            state
                .preset
                .get(key)
                .and_then(Json::as_str)
                .and_then(|s| ObjectId::from_str(s).ok())
                .and_then(|id| spatial::get(state.snapshot, id))
                .or_else(|| spatial::lowest_storey(state.snapshot))
                .and_then(|s| types::text(&s.values, 0).map(str::to_owned))
                .unwrap_or_default()
        };
        match state.answers.first().map(|a| &a.input) {
            Some(PromptInput::Point(p)) => {
                let mut params = Map::new();
                params.insert("points".into(), Json::from(vec![vec![p.x, p.y]]));
                Ok(Step::Done(params))
            }
            _ => Ok(Step::Ask(
                Prompt::new(
                    "point-0",
                    PROMPT_PLACE.message_id,
                    Accept::Point {
                        dims: PointDims::Two,
                    },
                )
                .with_arg("storey", name_of("storey")),
            )),
        }
    }
}

/// Registers the commands of this module.
///
/// # Errors
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(
        CommandSpec::new(
            ROOM_PLACE,
            PLACE_LABEL.message_id,
            PLACE_DESCRIPTION.message_id,
            |ctx: &mut Context<'_>, p: PlaceRoom| place(ctx, p),
        )
        .with_prompt(Arc::new(RoomPrompts)),
    )?;
    registry.register(CommandSpec::new(
        ROOM_PLACE_ALL,
        PLACE_ALL_LABEL.message_id,
        PLACE_ALL_DESCRIPTION.message_id,
        |ctx: &mut Context<'_>, p: PlaceAll| place_all(ctx, p),
    ))
}
