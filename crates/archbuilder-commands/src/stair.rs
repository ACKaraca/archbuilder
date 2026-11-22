//! `archbuilder.stair.create`: a stair on a storey, and the prompts that ask for it.
//!
//! A stair is its authored data ([`archbuilder_model::stair::Stair`]): the start (the middle of
//! the first riser) and the direction of the first run, the shape (straight, an L turn or a U
//! turn, each turn to the left or to the right), the width, the tread depth, the highest riser
//! and the waist. Everything else is derived from the storey's level and the level above. The
//! direction is given by `to` (a point the first run heads toward) or `direction` (an angle);
//! without either the stair runs along the x axis. One call is one transaction, one undo step;
//! a stair the levels cannot carry (no level above, too few risers) is refused by the model's
//! rules.
//!
//! The prompt machine ([`StairPrompts`]) asks for the start point and then the direction point;
//! the keywords `straight`, `l_turn` and `u_turn` change the shape (a turn then asks for the
//! side), `width` and `tread` ask for a length. Every prompt shows the riser count and height the
//! storey gives.

use std::str::FromStr;
use std::sync::Arc;

use arc_base::{Dimension, Error, ObjectId, Result};
use arc_command::prompt::{
    Accept, KeywordOption, PointDims, Prompt, PromptInput, PromptMachine, PromptState, Step,
};
use arc_command::{CommandSpec, Context, IdParam, QuantityParam, Registry};
use arc_doc::Snapshot;
use archbuilder_model::stair::{
    DEFAULT_MAX_RISER, DEFAULT_TREAD, DEFAULT_WAIST, DEFAULT_WIDTH, Layout, Placed, Shape, Stair,
    Turn,
};
use archbuilder_model::types::{self, LEVEL, STAIR, STAIR_WIDTH_RANGE, TREAD_RANGE};
use archbuilder_model::units::{decimal_separator, length_text};
use archbuilder_model::{defaults, names, spatial};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json};

use crate::error::codes;
use crate::structure::{LANGUAGE_SETTING, create, ensure_valid, expect, language};

/// The identifier of the create-stair command.
pub const STAIR_CREATE: &str = "archbuilder.stair.create";

/// A stair shape as a command parameter.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ShapeParam {
    /// One run.
    Straight,
    /// Two runs at a right angle with a landing.
    LTurn,
    /// Two runs side by side in opposite directions with a landing.
    UTurn,
}

impl From<ShapeParam> for Shape {
    fn from(shape: ShapeParam) -> Self {
        match shape {
            ShapeParam::Straight => Self::Straight,
            ShapeParam::LTurn => Self::LTurn,
            ShapeParam::UTurn => Self::UTurn,
        }
    }
}

/// The side of a turn as a command parameter.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TurnParam {
    /// To the left, seen walking up.
    Left,
    /// To the right.
    Right,
}

impl From<TurnParam> for Turn {
    fn from(turn: TurnParam) -> Self {
        match turn {
            TurnParam::Left => Self::Left,
            TurnParam::Right => Self::Right,
        }
    }
}

/// Parameters of `archbuilder.stair.create`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateStair {
    /// The storey; else the storey of the selection, else the lowest storey.
    #[serde(default)]
    pub storey: Option<IdParam>,
    /// The middle of the first riser, `[x, y]` in meters.
    pub start: [f64; 2],
    /// A point the first run heads toward.
    #[serde(default)]
    pub to: Option<[f64; 2]>,
    /// The direction of the first run as an angle, used without `to`, read as rotate reads its
    /// angle: a JSON number is radians, a text number such as `"90"` is degrees, and a text with
    /// a unit says its own.
    #[serde(default)]
    pub direction: Option<QuantityParam>,
    /// The shape; straight when absent.
    #[serde(default)]
    pub shape: Option<ShapeParam>,
    /// The side an L or U stair turns to; left when absent.
    #[serde(default)]
    pub turn: Option<TurnParam>,
    /// The width; 1 m when absent.
    #[serde(default)]
    pub width: Option<QuantityParam>,
    /// The tread depth; 0.28 m when absent.
    #[serde(default)]
    pub tread: Option<QuantityParam>,
    /// The highest riser; 0.18 m when absent.
    #[serde(default)]
    pub max_riser: Option<QuantityParam>,
    /// The waist thickness; 0.15 m when absent.
    #[serde(default)]
    pub waist: Option<QuantityParam>,
    /// The level the stair arrives at; else the level above the storey.
    #[serde(default)]
    pub top_level: Option<IdParam>,
    /// The name; else the first free `Stair <n>` of the storey.
    #[serde(default)]
    pub name: Option<String>,
}

/// What `archbuilder.stair.create` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct StairCreated {
    /// The new stair.
    pub stair: IdParam,
    /// Its storey.
    pub storey: IdParam,
    /// The riser count.
    pub risers: usize,
    /// The riser height in meters.
    pub riser: f64,
    /// The horizontal length of each run in meters.
    pub runs: Vec<f64>,
    /// The stride `2R + T` in meters.
    pub stride: f64,
    /// Whether the stride lies in the comfortable range.
    pub stride_ok: bool,
    /// The volume of the concrete in cubic meters.
    pub volume: f64,
}

pub(crate) fn next_stair_name(snapshot: &Snapshot, storey: ObjectId, language: &str) -> String {
    let used = spatial::children(snapshot, STAIR, storey);
    let taken = |name: &str| {
        used.iter()
            .any(|f| types::text(&f.values, 0).is_some_and(|n| names::same(n, name)))
    };
    (used.len() + 1..)
        .map(|n| defaults::stair(language, n))
        .find(|name| !taken(name))
        .unwrap_or_default()
}

fn invalid() -> Error {
    Error::new(arc_command::error::codes::PARAMS_INVALID)
}

fn create_stair(ctx: &mut Context<'_>, p: CreateStair) -> Result<StairCreated> {
    let storey = crate::wall::storey_for(ctx, p.storey)?.id;
    let meters = |ctx: &Context<'_>, param: &Option<QuantityParam>, default: f64| match param {
        Some(param) => ctx.quantity(param, Dimension::LENGTH).map(|q| q.value()),
        None => Ok(default),
    };
    if !p.start.iter().all(|v| v.is_finite()) {
        return Err(invalid());
    }
    let direction = match (&p.to, &p.direction) {
        (Some(to), _) => {
            let d = [to[0] - p.start[0], to[1] - p.start[1]];
            if !d.iter().all(|v| v.is_finite()) || d[0].hypot(d[1]) < 1.0e-6 {
                return Err(invalid());
            }
            d[1].atan2(d[0])
        }
        (None, Some(QuantityParam::Text(t))) if t.trim().parse::<f64>().is_ok() => {
            t.trim().parse::<f64>().unwrap_or(0.0).to_radians()
        }
        (None, Some(param)) => ctx.quantity(param, Dimension::ANGLE)?.value(),
        (None, None) => 0.0,
    };
    if !direction.is_finite() {
        return Err(invalid());
    }
    let top_level = match p.top_level {
        Some(IdParam(id)) => Some(expect(ctx.snapshot(), id, LEVEL)?.id),
        None => None,
    };
    let stair = Stair {
        shape: p.shape.map_or(Shape::Straight, Into::into),
        turn: p.turn.map_or(Turn::Left, Into::into),
        start: p.start,
        direction,
        width: meters(ctx, &p.width, DEFAULT_WIDTH)?,
        tread: meters(ctx, &p.tread, DEFAULT_TREAD)?,
        max_riser: meters(ctx, &p.max_riser, DEFAULT_MAX_RISER)?,
        waist: meters(ctx, &p.waist, DEFAULT_WAIST)?,
        railings: [true, true],
        top_level,
    };
    let tongue = language(ctx);
    let name = match p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => name.to_owned(),
        None => next_stair_name(ctx.snapshot(), storey, &tongue),
    };
    let id = create(ctx, STAIR, Some(storey), stair.values(&name))?;
    ensure_valid(ctx)?;
    let placed = {
        let txn = ctx.txn()?;
        spatial::get(txn.snapshot(), id).and_then(|o| Placed::of(txn.snapshot(), &o))
    };
    let layout = placed
        .and_then(|p| p.layout.ok())
        .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    ctx.set_selection_after(vec![id]);
    Ok(StairCreated {
        stair: IdParam(id),
        storey: IdParam(storey),
        risers: layout.risers,
        riser: layout.riser,
        runs: layout.runs.iter().map(|r| r.length).collect(),
        stride: layout.stride(),
        stride_ok: layout.stride_ok(),
        volume: layout.volume(),
    })
}

/// A message of the catalog, named by identifier.
struct Msg {
    message_id: &'static str,
}

const PROMPT_START: Msg = Msg {
    message_id: "ab-prompt-stair-start",
};
const PROMPT_DIRECTION: Msg = Msg {
    message_id: "ab-prompt-stair-direction",
};
const PROMPT_TURN: Msg = Msg {
    message_id: "ab-prompt-stair-turn",
};
const PROMPT_TURN_AGAIN: Msg = Msg {
    message_id: "ab-prompt-stair-turn-again",
};
const PROMPT_WIDTH: Msg = Msg {
    message_id: "ab-prompt-stair-width",
};
const PROMPT_TREAD: Msg = Msg {
    message_id: "ab-prompt-stair-tread",
};
const KEY_STRAIGHT: Msg = Msg {
    message_id: "ab-keyword-stair-straight",
};
const KEY_L: Msg = Msg {
    message_id: "ab-keyword-stair-l-turn",
};
const KEY_U: Msg = Msg {
    message_id: "ab-keyword-stair-u-turn",
};
const KEY_LEFT: Msg = Msg {
    message_id: "ab-keyword-stair-left",
};
const KEY_RIGHT: Msg = Msg {
    message_id: "ab-keyword-stair-right",
};
const KEY_WIDTH: Msg = Msg {
    message_id: "ab-keyword-stair-width",
};
const KEY_TREAD: Msg = Msg {
    message_id: "ab-keyword-stair-tread",
};

/// The prompts of `archbuilder.stair.create`.
#[derive(Debug)]
pub struct StairPrompts;

/// What the prompts have read so far.
#[derive(Debug, Clone)]
struct Scan {
    shape: ShapeParam,
    turn: TurnParam,
    width: f64,
    tread: f64,
    start: Option<[f64; 2]>,
}

fn pick(turkish: bool, english: char, tr: char) -> char {
    if turkish { tr } else { english }
}

impl Scan {
    fn params(
        &self,
        start: [f64; 2],
        to: [f64; 2],
        preset: &Map<String, Json>,
    ) -> Map<String, Json> {
        let mut params = Map::new();
        params.insert("start".into(), Json::from(start.to_vec()));
        params.insert("to".into(), Json::from(to.to_vec()));
        params.insert(
            "shape".into(),
            serde_json::to_value(self.shape).unwrap_or(Json::Null),
        );
        params.insert(
            "turn".into(),
            serde_json::to_value(self.turn).unwrap_or(Json::Null),
        );
        params.insert("width".into(), Json::from(self.width));
        params.insert("tread".into(), Json::from(self.tread));
        if let Some(storey) = preset.get("storey") {
            params.insert("storey".into(), storey.clone());
        }
        params
    }

    /// The layout the storey gives this stair, when it has one.
    fn layout(&self, snapshot: &Snapshot, storey: Option<&arc_doc::Object>) -> Option<Layout> {
        let storey = storey?;
        let base = spatial::storey_elevation(snapshot, storey);
        let top = spatial::level_above(snapshot, storey)
            .and_then(|l| types::meters(&l.values, types::level::ELEVATION))?;
        let stair = Stair {
            shape: self.shape.into(),
            turn: self.turn.into(),
            start: [0.0, 0.0],
            direction: 0.0,
            width: self.width,
            tread: self.tread,
            max_riser: DEFAULT_MAX_RISER,
            waist: DEFAULT_WAIST,
            railings: [true, true],
            top_level: None,
        };
        Layout::new(&stair, top - base).ok()
    }
}

impl StairPrompts {
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
        let unit = spatial::length_unit(state.snapshot);
        let text = |m: f64| length_text(m, unit, decimal_separator(&language));
        let number = |key: &str| preset.get(key).and_then(Json::as_f64);
        let mut scan = Scan {
            shape: preset
                .get("shape")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or(ShapeParam::Straight),
            turn: preset
                .get("turn")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or(TurnParam::Left),
            width: number("width").unwrap_or(DEFAULT_WIDTH),
            tread: number("tread").unwrap_or(DEFAULT_TREAD),
            start: None,
        };
        let storey = preset
            .get("storey")
            .and_then(Json::as_str)
            .and_then(|s| ObjectId::from_str(s).ok())
            .and_then(|id| spatial::get(state.snapshot, id))
            .or_else(|| spatial::lowest_storey(state.snapshot));
        let storey_name = storey
            .as_ref()
            .and_then(|s| types::text(&s.values, 0).map(str::to_owned))
            .unwrap_or_default();
        let ask = |scan: &Scan, key: String| {
            let layout = scan.layout(state.snapshot, storey.as_deref());
            let message = if scan.start.is_some() {
                &PROMPT_DIRECTION
            } else {
                &PROMPT_START
            };
            let options = vec![
                KeywordOption::new("straight", KEY_STRAIGHT.message_id)
                    .with_shortcut(pick(turkish, 's', 'z')),
                KeywordOption::new("l_turn", KEY_L.message_id).with_shortcut('l'),
                KeywordOption::new("u_turn", KEY_U.message_id).with_shortcut('u'),
                KeywordOption::new("width", KEY_WIDTH.message_id)
                    .with_shortcut(pick(turkish, 'w', 'g')),
                KeywordOption::new("tread", KEY_TREAD.message_id)
                    .with_shortcut(pick(turkish, 't', 'd')),
            ];
            Prompt::new(
                key,
                message.message_id,
                Accept::Point {
                    dims: PointDims::Two,
                },
            )
            .or(Accept::keywords(options))
            .with_arg("storey", storey_name.as_str())
            .with_arg(
                "shape",
                match scan.shape {
                    ShapeParam::Straight => "straight",
                    ShapeParam::LTurn => "l_turn",
                    ShapeParam::UTurn => "u_turn",
                },
            )
            .with_arg(
                "turn",
                match scan.turn {
                    TurnParam::Left => "left",
                    TurnParam::Right => "right",
                },
            )
            .with_arg(
                "risers",
                layout
                    .as_ref()
                    .map_or_else(|| "?".to_owned(), |l| l.risers.to_string()),
            )
            .with_arg(
                "riser",
                layout
                    .as_ref()
                    .map_or_else(|| "?".to_owned(), |l| text(l.riser)),
            )
            .with_arg("width", text(scan.width))
            .with_arg("tread", text(scan.tread))
        };
        let mut at = 0;
        loop {
            let Some(answer) = state.answers.get(at) else {
                let prompt = ask(&scan, format!("point-{at}"));
                return (scan, Step::Ask(prompt));
            };
            at += 1;
            match &answer.input {
                PromptInput::Point(p) => match scan.start {
                    None => scan.start = Some([p.x, p.y]),
                    Some(start) => {
                        let params = scan.params(start, [p.x, p.y], preset);
                        return (scan, Step::Done(params));
                    }
                },
                PromptInput::Keyword(k) => match k.as_str() {
                    "straight" => scan.shape = ShapeParam::Straight,
                    "l_turn" | "u_turn" => {
                        scan.shape = if k == "l_turn" {
                            ShapeParam::LTurn
                        } else {
                            ShapeParam::UTurn
                        };
                        // Any other text is taken and the side is asked again, the prompt then
                        // naming the answer it could not use and the valid ones.
                        let mut stray: Option<String> = None;
                        loop {
                            let message = if stray.is_some() {
                                &PROMPT_TURN_AGAIN
                            } else {
                                &PROMPT_TURN
                            };
                            let prompt = Prompt::new(
                                format!("turn-{at}"),
                                message.message_id,
                                Accept::keywords([
                                    KeywordOption::new("left", KEY_LEFT.message_id)
                                        .with_shortcut(pick(turkish, 'l', 's')),
                                    KeywordOption::new("right", KEY_RIGHT.message_id)
                                        .with_shortcut(pick(turkish, 'r', 'a')),
                                ]),
                            )
                            .or(Accept::text())
                            .with_default(PromptInput::Keyword(
                                match scan.turn {
                                    TurnParam::Left => "left",
                                    TurnParam::Right => "right",
                                }
                                .to_owned(),
                            ))
                            .with_arg("typed", stray.clone().unwrap_or_default());
                            let Some(side) = state.answers.get(at) else {
                                return (scan, Step::Ask(prompt));
                            };
                            at += 1;
                            match &side.input {
                                PromptInput::Keyword(s) => {
                                    scan.turn = if s == "right" {
                                        TurnParam::Right
                                    } else {
                                        TurnParam::Left
                                    };
                                    break;
                                }
                                PromptInput::Text(t) => stray = Some(t.clone()),
                                _ => break,
                            }
                        }
                    }
                    "width" | "tread" => {
                        let (message, range, now) = if k == "width" {
                            (&PROMPT_WIDTH, STAIR_WIDTH_RANGE, scan.width)
                        } else {
                            (&PROMPT_TREAD, TREAD_RANGE, scan.tread)
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
                        .with_arg("width", text(scan.width))
                        .with_arg("tread", text(scan.tread));
                        let Some(value) = state.answers.get(at) else {
                            return (scan, Step::Ask(prompt));
                        };
                        at += 1;
                        if let PromptInput::Distance(d) = value.input {
                            if k == "width" {
                                scan.width = d;
                            } else {
                                scan.tread = d;
                            }
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
}

impl PromptMachine for StairPrompts {
    fn next(&self, state: &PromptState<'_>) -> Result<Step> {
        Ok(Self::scan(state).1)
    }

    fn check(&self, state: &PromptState<'_>, _: &Prompt, input: &PromptInput) -> Result<()> {
        // The direction point must lie away from the start.
        if let (PromptInput::Point(p), (Scan { start: Some(s), .. }, _)) =
            (input, Self::scan(state))
            && (p.x - s[0]).hypot(p.y - s[1]) < 1.0e-6
        {
            return Err(invalid());
        }
        Ok(())
    }

    /// The parameters the command would get with the pointer as the direction point: the host
    /// draws the stair they make.
    fn preview(&self, state: &PromptState<'_>, pointer: Option<&PromptInput>) -> Option<Json> {
        let (scan, _) = Self::scan(state);
        let (Some(start), Some(PromptInput::Point(p))) = (scan.start, pointer) else {
            return None;
        };
        if (p.x - start[0]).hypot(p.y - start[1]) < 1.0e-6 {
            return None;
        }
        Some(Json::Object(scan.params(start, [p.x, p.y], state.preset)))
    }
}

/// The stair the parameters of a preview make, when they make one: for drawing it before it is
/// created.
#[must_use]
pub fn previewed(params: &Json) -> Option<Stair> {
    let start = params.get("start")?.as_array()?;
    let to = params.get("to")?.as_array()?;
    let at = |a: &Vec<Json>, k: usize| a.get(k).and_then(Json::as_f64);
    let (sx, sy, tx, ty) = (at(start, 0)?, at(start, 1)?, at(to, 0)?, at(to, 1)?);
    let shape: ShapeParam = serde_json::from_value(params.get("shape")?.clone()).ok()?;
    let turn: TurnParam = serde_json::from_value(params.get("turn")?.clone()).ok()?;
    Some(Stair {
        shape: shape.into(),
        turn: turn.into(),
        start: [sx, sy],
        direction: (ty - sy).atan2(tx - sx),
        width: params.get("width")?.as_f64()?,
        tread: params.get("tread")?.as_f64()?,
        max_riser: DEFAULT_MAX_RISER,
        waist: DEFAULT_WAIST,
        railings: [true, true],
        top_level: None,
    })
}

/// Registers the commands of this module.
///
/// # Errors
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    const LABEL: Msg = Msg {
        message_id: "ab-cmd-stair-create-label",
    };
    const DESCRIPTION: Msg = Msg {
        message_id: "ab-cmd-stair-create-description",
    };
    registry.register(
        CommandSpec::new(
            STAIR_CREATE,
            LABEL.message_id,
            DESCRIPTION.message_id,
            |ctx: &mut Context<'_>, p: CreateStair| create_stair(ctx, p),
        )
        .with_prompt(Arc::new(StairPrompts)),
    )
}
