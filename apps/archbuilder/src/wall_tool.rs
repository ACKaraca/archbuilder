//! The wall tool: the chain of `archbuilder.wall.create` prompts behind the viewport and the
//! command line, without any windowing. [`Tool`] owns a command line and the state of the chain;
//! its methods take the pieces of the outside world (document, session, selection) in [`Env`] and
//! run the commands through the runner the caller gives, so the window and the tests drive the
//! same code.
//!
//! A chain is a run of walls from one start point: each wall is one `archbuilder.wall.create`
//! (one undo step per wall), and when a wall is placed the next one starts at its end with the
//! same storey, thickness, height and location line. The chain ends on Enter or Escape (the walls
//! already placed stay; the segment in progress is dropped), and when a wall ends at the start of
//! the chain after at least two walls (the `close` keyword, a typed or a clicked point).

use std::collections::BTreeMap;

use arc_base::{Error, ObjectId, Result};
use arc_command::prompt::{Point, PromptInput};
use arc_command::{
    CommandLine, Invocation, LineStep, Outcome, Registry, Session, Submitted, Target,
};
use arc_doc::{Document, Snapshot};
use archbuilder_commands::wall::WALL_CREATE;
use archbuilder_model::spatial;
use archbuilder_model::types::{DEFAULT_THICKNESS, MIN_WALL_LENGTH, STOREY, WALL};
use serde_json::{Map, Value};

/// What the tool reads from the outside: the commands, the document, the session (with the
/// context units for typed lengths), the selection and the seed of the next invocation.
pub struct Env<'a> {
    /// The product's commands, for the prompts.
    pub registry: &'a Registry,
    /// The document.
    pub document: &'a Document,
    /// The session the command line runs in.
    pub session: &'a Session,
    /// The selection.
    pub selection: &'a [ObjectId],
    /// The seed of the invocation.
    pub seed: u64,
}

/// Runs an invocation of a document command.
pub type Runner<'r> = &'r mut dyn FnMut(Invocation) -> Result<Outcome>;

/// The options and position of the chain being drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct Chain {
    /// The storey the walls stand on.
    pub storey: ObjectId,
    /// The elevation of the storey's level, where clicks land.
    pub elevation: f64,
    /// The start of the first wall, once placed.
    pub start: Option<[f64; 2]>,
    /// The end of the last wall placed: where the next one starts.
    pub end: Option<[f64; 2]>,
    /// The walls placed so far.
    pub walls: usize,
    /// The thickness of the next wall, in meters.
    pub thickness: f64,
    /// The height of the next wall, once a wall carried it (unconnected walls only).
    pub height: Option<f64>,
    /// The level the next wall's top is constrained to, once a wall carried it.
    pub top_level: Option<String>,
    /// The location line of the next wall, once a wall carried it.
    pub location: Option<String>,
    /// Whether the next wall is flipped, once a wall carried it.
    pub flip: bool,
}

/// The state of the wall tool of one tab.
#[derive(Debug, Default)]
pub struct Tool {
    line: CommandLine,
    chain: Option<Chain>,
    error: Option<Error>,
}

fn target<'a>(env: &'a Env<'_>, localized: &'a BTreeMap<String, String>) -> Target<'a> {
    Target {
        registry: env.registry,
        document: env.document,
        session: env.session,
        selection: env.selection,
        localized,
        seed: env.seed,
    }
}

/// The storey the selection names (a selected storey, a selected level's storey or a selected
/// wall's storey), else the lowest storey, with the elevation of its level: where walls are drawn.
#[must_use]
pub fn storey_for(snapshot: &Snapshot, selection: &[ObjectId]) -> Option<(ObjectId, f64)> {
    let from_selection = selection.iter().find_map(|id| {
        let object = spatial::get(snapshot, *id)?;
        match spatial::type_of(snapshot, *id)?.as_str() {
            STOREY => Some(object),
            WALL => spatial::get(snapshot, object.header.owner?),
            _ => spatial::objects(snapshot, STOREY)
                .into_iter()
                .find(|s| spatial::level_of(snapshot, s).is_some_and(|l| l.id == *id)),
        }
    });
    let storey = from_selection.or_else(|| spatial::lowest_storey(snapshot))?;
    let elevation = spatial::storey_elevation(snapshot, &storey);
    Some((storey.id, elevation))
}

fn near(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).hypot(a[1] - b[1]) < MIN_WALL_LENGTH
}

fn point_of(value: &Value) -> Option<[f64; 2]> {
    let items = value.as_array()?;
    match items.as_slice() {
        [x, y] => Some([x.as_f64()?, y.as_f64()?]),
        _ => None,
    }
}

impl Tool {
    /// An idle tool.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The command line, for the prompt and the options it shows.
    #[must_use]
    pub fn line(&self) -> &CommandLine {
        &self.line
    }

    /// The chain being drawn.
    #[must_use]
    pub fn chain(&self) -> Option<&Chain> {
        self.chain.as_ref()
    }

    /// The error a click or a rejected answer left, taken once.
    pub fn take_error(&mut self) -> Option<Error> {
        self.error.take()
    }

    /// True while a command of the line is asking.
    #[must_use]
    pub fn active(&self) -> bool {
        self.line.active().is_some()
    }

    /// Starts a chain: the start-point prompt of the first wall, on the storey of the selection
    /// or the lowest storey.
    ///
    /// # Errors
    /// `ab.cmd.no_building` without a storey; the errors of the command line.
    pub fn begin(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        self.line.cancel();
        self.chain = None;
        let (storey, elevation) = storey_for(&env.document.snapshot(), env.selection)
            .ok_or_else(|| Error::new(archbuilder_commands::error::codes::NO_BUILDING))?;
        self.line.set_last_point(None)?;
        self.chain = Some(Chain {
            storey,
            elevation,
            start: None,
            end: None,
            walls: 0,
            thickness: DEFAULT_THICKNESS,
            height: None,
            top_level: None,
            location: None,
            flip: false,
        });
        self.next(env, run)
    }

    /// Starts the prompts of the next wall with the options the chain carries.
    fn next(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        let Some(chain) = &self.chain else {
            return Ok(());
        };
        let mut preset = Map::new();
        preset.insert("storey".into(), Value::from(chain.storey.to_string()));
        if let Some(end) = chain.end {
            preset.insert("start".into(), Value::from(end.to_vec()));
        }
        if chain.walls > 0 {
            preset.insert("thickness".into(), Value::from(chain.thickness));
        }
        if let Some(level) = &chain.top_level {
            preset.insert("top_level".into(), Value::from(level.as_str()));
        } else if let Some(height) = chain.height {
            preset.insert("height".into(), Value::from(height));
        }
        if chain.flip {
            preset.insert("flip".into(), Value::from(true));
        }
        if let Some(location) = &chain.location {
            preset.insert("location_line".into(), Value::from(location.as_str()));
        }
        if let (Some(start), true) = (chain.start, chain.walls >= 2) {
            preset.insert("close_to".into(), Value::from(start.to_vec()));
        }
        let localized = BTreeMap::new();
        let step = LineStep::Start {
            command: WALL_CREATE,
            preset,
        };
        let result = self.line.drive(&target(env, &localized), step, run);
        self.follow(env, run, result)
    }

    /// Takes what a line step led to: a wall that was placed continues the chain, a rejected
    /// answer is kept as the pending error, a failed run ends the chain.
    fn follow(&mut self, env: &Env<'_>, run: Runner<'_>, result: Result<Submitted>) -> Result<()> {
        match result {
            Err(error) => {
                self.chain = None;
                Err(error)
            }
            Ok(Submitted::Rejected { error, .. }) => {
                self.error = Some(error);
                Ok(())
            }
            Ok(Submitted::Ran(outcome)) if outcome.command == WALL_CREATE => {
                self.placed(&outcome.value);
                self.next(env, run)
            }
            Ok(Submitted::Cancelled | Submitted::Idle) => {
                self.chain = None;
                Ok(())
            }
            Ok(_) => Ok(()),
        }
    }

    /// A wall was placed: the chain carries its options and ends when it closed.
    fn placed(&mut self, wall: &Value) {
        let Some(chain) = &mut self.chain else {
            return;
        };
        let start = wall.get("start").and_then(point_of);
        let end = wall.get("end").and_then(point_of);
        chain.start = chain.start.or(start);
        chain.end = end;
        chain.walls += 1;
        chain.thickness = wall
            .get("thickness")
            .and_then(Value::as_f64)
            .unwrap_or(chain.thickness);
        chain.top_level = wall
            .get("top_level")
            .and_then(Value::as_str)
            .map(str::to_owned);
        chain.height = wall
            .get("height")
            .and_then(Value::as_f64)
            .filter(|_| chain.top_level.is_none());
        chain.flip = wall.get("flip").and_then(Value::as_bool).unwrap_or(false);
        chain.location = wall
            .get("location_line")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if let (Some(first), Some(end), true) = (chain.start, end, chain.walls >= 2)
            && near(first, end)
        {
            self.chain = None;
        }
    }

    /// A click at `at` (project coordinates, on the storey's level plane).
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn click(&mut self, env: &Env<'_>, run: Runner<'_>, at: [f64; 2]) -> Result<()> {
        let Some(prompt) = self.line.active().and_then(|s| s.prompt()) else {
            return Ok(());
        };
        if !prompt
            .accepts
            .iter()
            .any(|a| matches!(a, arc_command::prompt::Accept::Point { .. }))
        {
            return Ok(());
        }
        let input = PromptInput::Point(Point {
            x: at[0],
            y: at[1],
            z: None,
        });
        let localized = BTreeMap::new();
        let result = self
            .line
            .drive(&target(env, &localized), LineStep::Answer(input), run);
        self.follow(env, run, result)
    }

    /// A line the user typed. While the tool asks, empty text (Enter) ends the chain; otherwise
    /// the text answers the prompt (a point, a distance or a keyword). With nothing active, a
    /// line naming the wall command starts a chain and any other line goes to the command line.
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn text(&mut self, env: &Env<'_>, run: Runner<'_>, text: &str) -> Result<()> {
        let text = text.trim();
        if self.active() {
            if text.is_empty() {
                // Enter at a value prompt keeps the value in effect; at a point prompt it ends
                // the chain.
                if self
                    .line
                    .active()
                    .and_then(|s| s.prompt())
                    .is_some_and(|p| {
                        !p.accepts
                            .iter()
                            .any(|a| matches!(a, arc_command::prompt::Accept::Point { .. }))
                    })
                {
                    let localized = BTreeMap::new();
                    let result =
                        self.line
                            .drive(&target(env, &localized), LineStep::Submit(""), run);
                    return self.follow(env, run, result);
                }
                self.stop();
                return Ok(());
            }
            let localized = BTreeMap::new();
            let result = self
                .line
                .drive(&target(env, &localized), LineStep::Submit(text), run);
            return self.follow(env, run, result);
        }
        if is_wall_word(text) {
            return self.begin(env, run);
        }
        let localized = BTreeMap::new();
        let result = self
            .line
            .drive(&target(env, &localized), LineStep::Submit(text), run);
        self.follow(env, run, result)
    }

    /// Steps back one answer of the active command.
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn back(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        if !self.active() {
            return Ok(());
        }
        let localized = BTreeMap::new();
        let result = self
            .line
            .drive(&target(env, &localized), LineStep::Back, run);
        self.follow(env, run, result)
    }

    /// Points the direct distance of a typed length along the line from the segment's start to
    /// `cursor`.
    pub fn aim(&mut self, cursor: [f64; 2]) {
        let direction = self.line.base_point().and_then(|from| {
            let (dx, dy) = (cursor[0] - from.x, cursor[1] - from.y);
            (dx.hypot(dy) > MIN_WALL_LENGTH).then_some([dx, dy])
        });
        // A direction that is not finite is dropped; the line keeps the one it had.
        let _ = self.line.set_pointer_direction(direction);
    }

    /// Escape: the first one drops the segment in progress and waits for a new start point (the
    /// walls placed stay); with no segment in progress it ends the tool.
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn escape(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        let mid_chain = self.chain.as_ref().is_some_and(|c| c.end.is_some());
        if !mid_chain {
            self.stop();
            return Ok(());
        }
        let options = self.chain.clone();
        self.line.cancel();
        self.chain = options.map(|c| Chain {
            start: None,
            end: None,
            walls: 0,
            ..c
        });
        self.line.set_last_point(None)?;
        self.next(env, run)
    }

    /// Enter or Escape: drops the segment in progress and ends the chain. The walls already
    /// placed stay.
    pub fn stop(&mut self) {
        self.line.cancel();
        self.chain = None;
    }
}

/// Whether `text` names the wall command: its identifier or the word `wall` or `duvar`.
fn is_wall_word(text: &str) -> bool {
    [WALL_CREATE, "wall", "duvar"]
        .iter()
        .any(|w| text.eq_ignore_ascii_case(w))
}
