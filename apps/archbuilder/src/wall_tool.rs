//! The wall tool: the chain of `archbuilder.wall.create` prompts behind the viewport and the
//! command line, without any windowing. [`Tool`] owns a command line and the state of the chain;
//! its methods take the pieces of the outside world (document, session, selection) in [`Env`] and
//! run the commands through the runner the caller gives, so the window and the tests drive the
//! same code.
//!
//! A chain is a run of walls from one start point: each wall is one `archbuilder.wall.create`
//! (one undo step per wall), and when a wall is placed the next one starts at its end with the
//! same storey, type, height and location line. The type is the one picked in the type selector
//! ([`crate::tool_type`]); a thickness typed at the prompt gives the wall a type of that thickness. The chain ends on Enter or Escape (the walls
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
use archbuilder_model::element_type::TypeKind;
use archbuilder_model::spatial;
use archbuilder_model::types::{DEFAULT_THICKNESS, DOOR, MIN_WALL_LENGTH, STOREY, WINDOW};
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
    /// The thickness of the next wall, in meters: its type's.
    pub thickness: f64,
    /// The thickness of the layers outside the core of the next wall's type, on the exterior
    /// and the interior side.
    pub core: [f64; 2],
    /// The type of the next wall: the type picked in the type selector, or the type the last
    /// wall got (a typed thickness gives a wall a type of that thickness).
    pub wall_type: Option<String>,
    /// The type picked in the type selector when the chain last looked; a new pick replaces
    /// `wall_type`.
    pub picked: Option<String>,
    /// The height of the next wall, once a wall carried it (unconnected walls only).
    pub height: Option<f64>,
    /// The level the next wall's top is constrained to, once a wall carried it.
    pub top_level: Option<String>,
    /// The location line of the next wall, once a wall carried it.
    pub location: Option<String>,
    /// Whether the next wall is flipped, once a wall carried it.
    pub flip: bool,
    /// Whether the walls are arcs: the prompts ask for a point on each.
    pub arc: bool,
    /// The start of each wall placed, in order: where the chain goes back to on undo.
    pub trail: Vec<[f64; 2]>,
}

/// The state of the wall tool of one tab.
#[derive(Debug, Default)]
pub struct Tool {
    line: CommandLine,
    chain: Option<Chain>,
    error: Option<Error>,
    /// The last placed wall was taken back and the host must undo its transaction.
    undo_wanted: bool,
}

pub(crate) fn target<'a>(env: &'a Env<'_>, localized: &'a BTreeMap<String, String>) -> Target<'a> {
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
            k if archbuilder_model::types::is_storey_element(k) => {
                spatial::get(snapshot, object.header.owner?)
            }
            DOOR | WINDOW => spatial::get(
                snapshot,
                spatial::get(snapshot, object.header.owner?)?.header.owner?,
            ),
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

    /// Starts a chain of curtain walls: the first curtain wall type of the document (one is made
    /// when there is none) is the type of the walls drawn.
    ///
    /// # Errors
    /// The errors of the type command and of [`Self::begin`].
    pub fn begin_curtain(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        let snapshot = env.document.snapshot();
        let id = match archbuilder_commands::curtain::first_type(&snapshot) {
            Some(id) => id,
            None => {
                let outcome = run(Invocation::new(
                    archbuilder_commands::curtain::CURTAIN_TYPE,
                    serde_json::json!({}),
                ))?;
                outcome
                    .value
                    .get("type")
                    .and_then(Value::as_str)
                    .and_then(|t| t.parse().ok())
                    .ok_or_else(|| Error::new(archbuilder_commands::error::codes::NO_BUILDING))?
            }
        };
        crate::tool_type::choose(TypeKind::Wall, id);
        self.begin(env, run)
    }

    /// Starts a chain: the start-point prompt of the first wall, on the storey of the selection
    /// or the lowest storey.
    ///
    /// # Errors
    /// `ab.cmd.no_building` without a storey; the errors of the command line.
    pub fn begin(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        self.begin_with(env, run, false)
    }

    /// Starts a chain of arc walls: like [`Self::begin`], with a point on each arc asked for.
    ///
    /// # Errors
    /// As [`Self::begin`].
    pub fn begin_arc(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        self.begin_with(env, run, true)
    }

    fn begin_with(&mut self, env: &Env<'_>, run: Runner<'_>, arc: bool) -> Result<()> {
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
            core: [0.0, 0.0],
            wall_type: None,
            picked: None,
            height: None,
            top_level: None,
            location: None,
            flip: false,
            arc,
            trail: Vec::new(),
        });
        self.next(env, run)
    }

    /// Starts the prompts of the next wall with the options the chain carries.
    fn next(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        let snapshot = env.document.snapshot();
        let Some(chain) = &mut self.chain else {
            return Ok(());
        };
        let picked = crate::tool_type::current_param(&snapshot, TypeKind::Wall);
        if picked != chain.picked || chain.wall_type.is_none() {
            chain.wall_type.clone_from(&picked);
            chain.picked = picked;
        }
        // The thickness and core of the type, for the prompt and the preview.
        if let Some(t) = chain
            .wall_type
            .as_deref()
            .and_then(|id| id.parse().ok())
            .and_then(|id| spatial::get(&snapshot, id))
            .and_then(|t| archbuilder_model::element_type::Layers::from_values(&t.values))
        {
            chain.thickness = t.total();
            chain.core = t.core_offsets();
        }
        let chain = &*chain;
        let mut preset = Map::new();
        preset.insert("storey".into(), Value::from(chain.storey.to_string()));
        if let Some(end) = chain.end {
            preset.insert("start".into(), Value::from(end.to_vec()));
        }
        if let Some(wall_type) = &chain.wall_type {
            preset.insert("type".into(), Value::from(wall_type.as_str()));
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
        if chain.arc {
            preset.insert("arc".into(), Value::from(true));
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
        chain.trail.extend(start);
        chain.thickness = wall
            .get("thickness")
            .and_then(Value::as_f64)
            .unwrap_or(chain.thickness);
        chain.wall_type = wall.get("type").and_then(Value::as_str).map(str::to_owned);
        chain.top_level = wall
            .get("top_level")
            .and_then(Value::as_str)
            .map(str::to_owned);
        chain.height = wall
            .get("height")
            .and_then(Value::as_f64)
            .filter(|_| chain.top_level.is_none());
        chain.flip = wall.get("flip").and_then(Value::as_bool).unwrap_or(false);
        // The chain goes on with arcs when the last wall was one (the keyword turns it off).
        chain.arc = wall.get("arc").and_then(Value::as_bool).unwrap_or(false);
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
        // Nothing answered in the segment in progress: take back the last wall placed. The host
        // undoes its transaction ([`Self::take_undo`]) and the chain goes on from that wall's start.
        let answered = self.line.active().is_some_and(|s| !s.answers().is_empty());
        if !answered && let Some(chain) = self.chain.as_mut().filter(|c| c.walls > 0) {
            let from = chain.trail.pop();
            chain.walls -= 1;
            chain.end = from;
            if chain.walls == 0 {
                chain.start = None;
                chain.end = None;
            }
            let last = chain.end.map(|p| Point {
                x: p[0],
                y: p[1],
                z: None,
            });
            self.undo_wanted = true;
            self.line.cancel();
            self.line.set_last_point(last)?;
            return self.next(env, run);
        }
        let localized = BTreeMap::new();
        let result = self
            .line
            .drive(&target(env, &localized), LineStep::Back, run);
        self.follow(env, run, result)
    }

    /// Whether a wall was taken back since the last call, so the host must undo its transaction;
    /// taken once.
    pub fn take_undo(&mut self) -> bool {
        std::mem::take(&mut self.undo_wanted)
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

    /// Escape: with a start point clicked and no wall placed after it, drops that segment and
    /// waits for a new start point; otherwise it ends the tool at once (the walls placed stay).
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn escape(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        let started = self.chain.as_ref().is_some_and(|c| c.end.is_none())
            && self.line.active().is_some_and(|s| {
                s.answers()
                    .iter()
                    .any(|a| matches!(a.input, PromptInput::Point(_)))
            });
        if !started {
            self.stop();
            return Ok(());
        }
        self.line.cancel();
        self.line.set_last_point(None)?;
        self.next(env, run)
    }

    /// A type was picked in the type selector: the segment in progress takes it at once when no
    /// point of it was answered yet; otherwise the next wall does.
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn retype(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        let answered = self.line.active().is_some_and(|s| !s.answers().is_empty());
        if self.chain.is_none() || answered {
            return Ok(());
        }
        self.line.cancel();
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
