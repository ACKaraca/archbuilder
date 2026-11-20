//! `archbuilder.door.place` and `archbuilder.window.place`: one door or window in a wall, the
//! prompts that ask for it, and `archbuilder.insert.flip_hand` and `archbuilder.insert.flip_facing`.
//!
//! The place commands take the host wall (else the selected wall, else the wall of the selected
//! insert), the distance of the insert's center from the baseline's start (else the middle of the
//! wall's clear span), and optionally the width, height, sill height (windows only: a door's sill is
//! the wall's base), the hand and the facing. Without options a door is 0.9 m by 2.1 m and a window
//! 1.2 m by 1.2 m on a 0.9 m sill, left hand, facing the exterior side. One call is one transaction,
//! so one undo step; a chain of inserts placed in a row is one undo step each.
//!
//! The prompt machine ([`InsertPrompts`]) asks for a distance along the wall; the answers `pick`
//! (a wall the host's pointer is on) and the keywords `width`, `height`, `sill`, `flip hand` and
//! `flip facing` come first and return to the distance. Typed distances read in the session's
//! context unit.

use std::collections::BTreeSet;
use std::str::FromStr;
use std::sync::Arc;

use arc_base::{Dimension, Error, ObjectId, Result};
use arc_command::prompt::{
    Accept, KeywordOption, Prompt, PromptInput, PromptMachine, PromptState, Step,
};
use arc_command::{CommandSpec, Context, IdParam, QuantityParam, Registry};
use arc_doc::{FieldPath, Snapshot, Value};
use archbuilder_model::insert::{self, Facing, HEAD_CLEARANCE, Hand, Insert, Kind, Operation};
use archbuilder_model::spatial;
use archbuilder_model::types::{
    self, DOOR, INSERT_HEIGHT_RANGE, INSERT_WIDTH_RANGE, SILL_RANGE, WALL, WINDOW,
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
/// The identifier of the command that flips the hand of inserts.
pub const INSERT_FLIP_HAND: &str = "archbuilder.insert.flip_hand";
/// The identifier of the command that flips the facing of inserts.
pub const INSERT_FLIP_FACING: &str = "archbuilder.insert.flip_facing";

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
    /// The width of the opening; 0.9 m for a door and 1.2 m for a window when absent.
    #[serde(default)]
    pub width: Option<QuantityParam>,
    /// The height of the opening; 2.1 m for a door and 1.2 m for a window when absent, never
    /// above the wall's top.
    #[serde(default)]
    pub height: Option<QuantityParam>,
    /// The sill height of a window; 0.9 m when absent. A door's sill is the wall's base, and the
    /// value is ignored.
    #[serde(default)]
    pub sill: Option<QuantityParam>,
    /// Whether the hand is flipped from the default (left): the hinge is on the right.
    #[serde(default)]
    pub flip_hand: Option<bool>,
    /// Whether the facing is flipped from the default (exterior): the leaf swings to the interior.
    #[serde(default)]
    pub flip_facing: Option<bool>,
    /// The name; else the first free `Door <n>` or `Window <n>`.
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

/// The insert kinds the selection holds, in selection order.
fn selected_inserts(ctx: &Context<'_>) -> Vec<ObjectId> {
    let snapshot = ctx.snapshot();
    ctx.selection()
        .iter()
        .copied()
        .filter(|id| is_insert(snapshot, *id))
        .collect()
}

/// Whether `id` is a door or a window.
pub(crate) fn is_insert(snapshot: &Snapshot, id: ObjectId) -> bool {
    matches!(
        spatial::type_of(snapshot, id).as_deref(),
        Some(DOOR | WINDOW)
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
            Some(DOOR | WINDOW) => spatial::get(snapshot, *id)
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

fn next_name(snapshot: &Snapshot, kind: Kind, wall: ObjectId, language: &str) -> String {
    let used = spatial::children(snapshot, kind.key(), wall);
    let taken = |name: &str| {
        used.iter().any(|o| {
            types::text(&o.values, types::insert::NAME).is_some_and(|n| names::same(n, name))
        })
    };
    let window = kind == Kind::Window;
    let total = spatial::objects(snapshot, kind.key()).len();
    (total + 1..)
        .map(|n| defaults::insert(language, window, n))
        .find(|name| !taken(name))
        .unwrap_or_default()
}

fn place(ctx: &mut Context<'_>, kind: Kind, p: PlaceInsert) -> Result<InsertPlaced> {
    let wall_object = wall_for(ctx, p.wall)?;
    let wall =
        Wall::of(ctx.snapshot(), &wall_object).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let meters = |ctx: &Context<'_>, param: &Option<QuantityParam>| match param {
        Some(param) => ctx
            .quantity(param, Dimension::LENGTH)
            .map(|q| Some(q.value())),
        None => Ok(None),
    };
    let sill = match kind {
        Kind::Door => 0.0,
        Kind::Window => meters(ctx, &p.sill)?.unwrap_or(WINDOW_SILL),
    };
    let (default_width, default_height) = match kind {
        Kind::Door => (DOOR_WIDTH, DOOR_HEIGHT),
        Kind::Window => (WINDOW_WIDTH, WINDOW_HEIGHT),
    };
    let width = meters(ctx, &p.width)?.unwrap_or(default_width);
    // A default height gives way to a low wall; a typed one is checked as it is.
    let height = match meters(ctx, &p.height)? {
        Some(height) => height,
        None => default_height
            .min(wall.height - sill - HEAD_CLEARANCE)
            .max(INSERT_HEIGHT_RANGE.0),
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
    let operation = match kind {
        Kind::Door => Operation::SingleSwing,
        Kind::Window => Operation::Fixed,
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
            Some(types::length(0.05)),
            Some(types::length(0.1_f64.min(wall.thickness))),
            Some(Value::Enum(operation.code())),
            Some(Value::Enum(hand.code())),
            Some(Value::Enum(facing.code())),
        ],
    )?;
    ensure_valid(ctx)?;
    Ok(InsertPlaced {
        insert: IdParam(id),
        wall: IdParam(wall_object.id),
        distance,
        width,
        height,
        sill,
        flip_hand: hand == Hand::Right,
        flip_facing: facing == Facing::Interior,
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
        if !is_insert(ctx.snapshot(), *id) {
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
    flip_hand: bool,
    flip_facing: bool,
    turkish: bool,
    separator: char,
    unit: archbuilder_model::units::LengthUnit,
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
        let (width, height) = match kind {
            Kind::Door => (DOOR_WIDTH, DOOR_HEIGHT),
            Kind::Window => (WINDOW_WIDTH, WINDOW_HEIGHT),
        };
        Self {
            wall: wall
                .and_then(|id| spatial::get(snapshot, id))
                .and_then(|w| types::text(&w.values, 0).map(str::to_owned))
                .unwrap_or_else(|| "-".to_owned()),
            width: number("width").unwrap_or(width),
            height: number("height").unwrap_or(height),
            sill: if kind == Kind::Window {
                number("sill").unwrap_or(WINDOW_SILL)
            } else {
                0.0
            },
            flip_hand: flag("flip_hand"),
            flip_facing: flag("flip_facing"),
            turkish: language.starts_with("tr"),
            separator: decimal_separator(&language),
            unit: spatial::length_unit(snapshot),
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
        };
        let mut options = vec![
            KeywordOption::new("width", KEY_WIDTH.message_id)
                .with_shortcut(shown.shortcut('w', 'g')),
            KeywordOption::new("height", KEY_HEIGHT.message_id)
                .with_shortcut(shown.shortcut('h', 'y')),
        ];
        if self.kind == Kind::Window {
            options.push(
                KeywordOption::new("sill", KEY_SILL.message_id)
                    .with_shortcut(shown.shortcut('s', 'd')),
            );
        }
        options.push(
            KeywordOption::new("flip_hand", KEY_HAND.message_id)
                .with_shortcut(shown.shortcut('f', 'k')),
        );
        options.push(
            KeywordOption::new("flip_facing", KEY_FACING.message_id)
                .with_shortcut(shown.shortcut('a', 'a')),
        );
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
    ))
}
