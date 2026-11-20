//! The floor tool: the prompts of `archbuilder.floor.create` (or of `archbuilder.floor.add_hole`)
//! behind the viewport and the command line, without any windowing. [`FloorTool`] owns a command
//! line; its methods take the pieces of the outside world in [`Env`] and run the commands
//! through the runner the caller gives, so the window and the tests drive the same code.
//!
//! In the walls mode (the start) a click on a wall picks it and Enter makes the floor; a click
//! inside a closed loop of joined walls, with nothing picked yet, picks the whole loop and makes
//! the floor at once. The keyword `sketch` switches to the sketch mode, where points are clicked
//! or typed and Enter (or `close`) closes the boundary; `walls` switches back. After a floor is
//! made the tool asks for the next one, in the same mode. Escape steps out one level at a time:
//! the first one cancels only the segment that follows the pointer and keeps the points placed
//! (they stay editable, `undo` still removes the last), the second discards the sketch or the
//! picks and asks again, the third ends the tool. A hole is a sketch of its own, in the selected
//! floor.

use std::collections::BTreeMap;

use arc_base::{Error, ObjectId, Result};
use arc_command::prompt::{Accept, Point, PromptInput};
use arc_command::{CommandLine, LineStep, Submitted, Target};
use archbuilder_commands::floor::{FLOOR_ADD_HOLE, FLOOR_CREATE};
use archbuilder_model::floor;
use serde_json::{Map, Value};

use crate::wall_tool::{Env, Runner, storey_for};

/// The state of the floor tool of one tab.
#[derive(Debug, Default)]
pub struct FloorTool {
    line: CommandLine,
    state: Option<State>,
    error: Option<Error>,
    /// The first Escape cancelled the segment that follows the pointer; the next answer brings
    /// it back.
    band_off: bool,
}

/// What the tool is doing.
#[derive(Debug, Clone, PartialEq)]
pub struct State {
    /// The storey the floor goes on.
    pub storey: ObjectId,
    /// The elevation of the storey's level, where clicks land.
    pub elevation: f64,
    /// The floor that gets a hole, for the hole tool.
    pub hole_in: Option<ObjectId>,
    /// Whether the next floor starts as a sketch (the mode the last one ended in).
    pub sketch: bool,
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

impl FloorTool {
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

    /// What the tool is doing, while it runs.
    #[must_use]
    pub fn state(&self) -> Option<&State> {
        self.state.as_ref()
    }

    /// Whether the segment that follows the pointer is cancelled (by Escape) until the next answer.
    #[must_use]
    pub fn band_off(&self) -> bool {
        self.band_off
    }

    /// The error a click or a rejected answer left, taken once.
    pub fn take_error(&mut self) -> Option<Error> {
        self.error.take()
    }

    /// True while a command of the line is asking.
    #[must_use]
    pub fn running(&self) -> bool {
        self.state.is_some() && self.line.active().is_some()
    }

    /// The mode and the points of the prompt in progress: whether it is a sketch, and the points
    /// answered since the mode was last chosen (in the walls mode, the picks).
    #[must_use]
    pub fn progress(&self) -> (bool, Vec<[f64; 2]>) {
        let Some(session) = self.line.active() else {
            return (false, Vec::new());
        };
        let mut sketch = session.preset().get("mode").and_then(Value::as_str) == Some("sketch")
            || self.state.as_ref().is_some_and(|s| s.hole_in.is_some());
        let mut points = Vec::new();
        for answer in session.answers() {
            match &answer.input {
                PromptInput::Point(p) => points.push([p.x, p.y]),
                PromptInput::Keyword(k) if k == "sketch" => {
                    sketch = true;
                    points.clear();
                }
                PromptInput::Keyword(k) if k == "undo" => {
                    points.pop();
                }
                PromptInput::Keyword(k) if k == "walls" => {
                    sketch = false;
                    points.clear();
                }
                _ => {}
            }
        }
        (sketch, points)
    }

    /// Starts the tool on the storey of the selection (else the lowest); with `hole_in`, the
    /// sketch of a hole in that floor.
    ///
    /// # Errors
    /// `ab.cmd.no_building` without a storey; the errors of the command line.
    pub fn begin(
        &mut self,
        env: &Env<'_>,
        run: Runner<'_>,
        hole_in: Option<ObjectId>,
    ) -> Result<()> {
        self.line.cancel();
        self.state = None;
        let (storey, elevation) = storey_for(&env.document.snapshot(), env.selection)
            .ok_or_else(|| Error::new(archbuilder_commands::error::codes::NO_BUILDING))?;
        self.line.set_last_point(None)?;
        self.state = Some(State {
            storey,
            elevation,
            hole_in,
            sketch: false,
        });
        self.next(env, run)
    }

    fn next(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        let Some(state) = &self.state else {
            return Ok(());
        };
        let mut preset = Map::new();
        preset.insert("storey".into(), Value::from(state.storey.to_string()));
        let command = match state.hole_in {
            Some(floor) => {
                preset.insert("floor".into(), Value::from(floor.to_string()));
                FLOOR_ADD_HOLE
            }
            None => {
                if state.sketch {
                    preset.insert("mode".into(), Value::from("sketch"));
                }
                FLOOR_CREATE
            }
        };
        let localized = BTreeMap::new();
        let step = LineStep::Start { command, preset };
        let result = self.line.drive(&target(env, &localized), step, run);
        self.follow(env, run, result)
    }

    /// Takes what a line step led to: a floor made starts the next one, a hole ends the tool, a
    /// rejected answer is kept as the pending error, a failed run ends the tool.
    fn follow(&mut self, env: &Env<'_>, run: Runner<'_>, result: Result<Submitted>) -> Result<()> {
        match result {
            Err(error) => {
                self.state = None;
                Err(error)
            }
            Ok(Submitted::Rejected { error, .. }) => {
                self.error = Some(error);
                Ok(())
            }
            Ok(Submitted::Ran(outcome)) if outcome.command == FLOOR_CREATE => self.next(env, run),
            Ok(Submitted::Ran(_) | Submitted::Cancelled | Submitted::Idle) => {
                self.state = None;
                Ok(())
            }
            Ok(_) => Ok(()),
        }
    }

    /// Remembers the mode of the prompt in progress for the next floor.
    fn remember_mode(&mut self) {
        let (sketch, _) = self.progress();
        if let Some(state) = &mut self.state {
            state.sketch = sketch;
        }
    }

    /// The inputs answered so far in the prompt in progress.
    fn answers(&self) -> Vec<PromptInput> {
        self.line
            .active()
            .map(|s| s.answers().iter().map(|a| a.input.clone()).collect())
            .unwrap_or_default()
    }

    /// Takes a step of the line. When the command refuses what the step completed (a boundary
    /// that crosses itself, walls that do not meet), the error is kept as the pending one and the
    /// prompts start again with the answers given before the step, so nothing sketched is lost.
    fn drive(&mut self, env: &Env<'_>, run: Runner<'_>, step: LineStep<'_>) -> Result<()> {
        self.band_off = false;
        self.remember_mode();
        let before = self.answers();
        let localized = BTreeMap::new();
        let result = self.line.drive(&target(env, &localized), step, run);
        match result {
            Err(error) if !before.is_empty() || self.state.is_some() => {
                // Only a run that failed after the prompts completed gets here with answers.
                self.error = Some(error);
                if before.is_empty() {
                    self.state = None;
                    return Ok(());
                }
                self.next(env, run)?;
                for input in before {
                    let localized = BTreeMap::new();
                    let _ = self
                        .line
                        .drive(&target(env, &localized), LineStep::Answer(input), run);
                }
                Ok(())
            }
            other => self.follow(env, run, other),
        }
    }

    fn answer(&mut self, env: &Env<'_>, run: Runner<'_>, input: PromptInput) -> Result<()> {
        self.drive(env, run, LineStep::Answer(input))
    }

    /// A click at `at` (project coordinates, on the storey's level plane). In the walls mode it
    /// picks the wall under it, or the loop of walls around it when nothing is picked yet; in the
    /// sketch mode it is the next point.
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
            .any(|a| matches!(a, Accept::Point { .. }))
        {
            return Ok(());
        }
        let (sketch, points) = self.progress();
        let point = PromptInput::Point(Point {
            x: at[0],
            y: at[1],
            z: None,
        });
        if sketch {
            // A click on the first point closes the outline.
            let closes = points.len() >= 3
                && points
                    .first()
                    .is_some_and(|f| (f[0] - at[0]).hypot(f[1] - at[1]) < floor::MIN_EDGE);
            let input = if closes {
                PromptInput::Keyword("close".into())
            } else {
                point
            };
            return self.answer(env, run, input);
        }
        let Some(storey) = self.state.as_ref().map(|s| s.storey) else {
            return Ok(());
        };
        let walls = floor::storey_walls(&env.document.snapshot(), storey);
        if floor::wall_at(&walls, at).is_some() {
            return self.answer(env, run, point);
        }
        if points.is_empty() && floor::loop_around(&walls, at).is_some() {
            self.answer(env, run, point)?;
            return self.answer(env, run, PromptInput::Keyword("done".into()));
        }
        // Neither a wall nor a closed loop of walls: tell the user, and wait for another click.
        if points.is_empty() {
            self.error = Some(Error::new(
                archbuilder_commands::error::codes::FLOOR_NO_LOOP,
            ));
        }
        Ok(())
    }

    /// A line the user typed. While the tool asks, empty text (Enter) finishes the floor when
    /// it has enough points or picks, else ends the tool; other text answers the prompt (a
    /// point, a distance or a keyword).
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn text(&mut self, env: &Env<'_>, run: Runner<'_>, text: &str) -> Result<()> {
        let text = text.trim();
        if !self.running() {
            if is_floor_word(text) {
                return self.begin(env, run, None);
            }
            let localized = BTreeMap::new();
            let result = self
                .line
                .drive(&target(env, &localized), LineStep::Submit(text), run);
            return self.follow(env, run, result);
        }
        let takes_point = self
            .line
            .active()
            .and_then(|s| s.prompt())
            .is_some_and(|p| p.accepts.iter().any(|a| matches!(a, Accept::Point { .. })));
        if text.is_empty() && takes_point {
            let (sketch, points) = self.progress();
            return match (sketch, points.len()) {
                (true, n) if n >= 3 => self.answer(env, run, PromptInput::Keyword("close".into())),
                (false, n) if n >= 1 => self.answer(env, run, PromptInput::Keyword("done".into())),
                _ => {
                    self.stop();
                    Ok(())
                }
            };
        }
        self.drive(env, run, LineStep::Submit(text))
    }

    /// Steps back one answer of the active command.
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn back(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        if !self.running() {
            return Ok(());
        }
        self.band_off = false;
        let localized = BTreeMap::new();
        let result = self
            .line
            .drive(&target(env, &localized), LineStep::Back, run);
        self.follow(env, run, result)
    }

    /// Escape, one level at a time. With points or picks in progress the first one cancels the
    /// segment that follows the pointer and keeps them; the second drops them (floors already
    /// made stay) and asks again; with nothing in progress it ends the tool.
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn escape(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        if self.progress().1.is_empty() {
            self.stop();
            return Ok(());
        }
        if !self.band_off {
            self.band_off = true;
            return Ok(());
        }
        self.band_off = false;
        self.remember_mode();
        self.line.cancel();
        self.next(env, run)
    }

    /// Ends the tool. A floor already made stays.
    pub fn stop(&mut self) {
        self.line.cancel();
        self.state = None;
        self.band_off = false;
    }
}

/// Whether `text` names the floor command: its identifier or the word `floor` or `doseme`.
fn is_floor_word(text: &str) -> bool {
    [FLOOR_CREATE, "floor", "slab", "döşeme", "doseme"]
        .iter()
        .any(|w| text.eq_ignore_ascii_case(w) || text.to_lowercase() == *w)
}
