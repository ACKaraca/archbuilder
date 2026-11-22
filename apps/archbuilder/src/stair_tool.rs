//! The stair tool: the prompts of `archbuilder.stair.create` behind the viewport and the command
//! line, without any windowing. A click gives the start, a second click the direction of the
//! first run; the keywords change the shape, the turn, the width and the tread. The stair is
//! created in one step and the tool asks for the next one, until Escape or Enter. A refusal (no
//! level above the storey, too few risers) is kept as the pending error and the tool keeps
//! waiting.

use std::collections::BTreeMap;

use arc_base::{Error, ObjectId, Result};
use arc_command::prompt::{Accept, Point, PromptInput};
use arc_command::{CommandLine, LineStep, Submitted, Target};
use archbuilder_commands::stair::{STAIR_CREATE, previewed};
use archbuilder_model::stair::Stair;
use serde_json::{Map, Value};

use crate::wall_tool::{Env, Runner, storey_for};

/// The state of the stair tool of one tab.
#[derive(Debug, Default)]
pub struct StairTool {
    line: CommandLine,
    storey: Option<(ObjectId, f64)>,
    error: Option<Error>,
    /// The shape, turn, width and tread of the last stair made, which the next one starts with.
    last: Map<String, Value>,
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

impl StairTool {
    /// An idle tool.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The command line, for the prompt it shows.
    #[must_use]
    pub fn line(&self) -> &CommandLine {
        &self.line
    }

    /// The storey and its level's elevation, while the tool runs.
    #[must_use]
    pub fn storey(&self) -> Option<(ObjectId, f64)> {
        self.storey
    }

    /// True while the tool asks for something.
    #[must_use]
    pub fn running(&self) -> bool {
        self.storey.is_some() && self.line.active().is_some()
    }

    /// The error a click left, taken once.
    pub fn take_error(&mut self) -> Option<Error> {
        self.error.take()
    }

    /// The stair the pointer at `at` would make as the direction point, once the start is given.
    #[must_use]
    pub fn preview(&self, env_snapshot: &arc_doc::Snapshot, at: [f64; 2]) -> Option<Stair> {
        let session = self.line.active()?;
        let pointer = PromptInput::Point(Point {
            x: at[0],
            y: at[1],
            z: None,
        });
        previewed(&session.preview(env_snapshot, Some(&pointer))?)
    }

    /// Starts the tool on the storey of the selection (else the lowest).
    ///
    /// # Errors
    /// `ab.cmd.no_building` without a storey; the errors of the command line.
    pub fn begin(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        self.line.cancel();
        let storey = storey_for(&env.document.snapshot(), env.selection)
            .ok_or_else(|| Error::new(archbuilder_commands::error::codes::NO_BUILDING))?;
        self.line.set_last_point(None)?;
        self.storey = Some(storey);
        self.next(env, run)
    }

    fn next(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        let Some((storey, _)) = self.storey else {
            return Ok(());
        };
        let mut preset = self.last.clone();
        preset.insert("storey".into(), Value::from(storey.to_string()));
        let localized = BTreeMap::new();
        let step = LineStep::Start {
            command: STAIR_CREATE,
            preset,
        };
        let result = self.line.drive(&target(env, &localized), step, run);
        if result.is_err() {
            self.stop();
        }
        result.map(|_| ())
    }

    /// Takes a step of the line: a stair made asks for the next one; a refusal is kept as the
    /// pending error and the tool starts again.
    fn step(&mut self, env: &Env<'_>, run: Runner<'_>, step: LineStep<'_>) -> Result<()> {
        let localized = BTreeMap::new();
        match self.line.drive(&target(env, &localized), step, run) {
            Err(error) => {
                self.error = Some(error);
                self.next(env, run)
            }
            Ok(Submitted::Rejected { error, .. }) => {
                self.error = Some(error);
                Ok(())
            }
            Ok(Submitted::Ran(outcome)) => {
                self.remember(env, &outcome.value);
                self.next(env, run)
            }
            Ok(Submitted::Cancelled | Submitted::Idle) => {
                self.stop();
                Ok(())
            }
            Ok(_) => Ok(()),
        }
    }

    /// Remembers the options of the stair the command made.
    fn remember(&mut self, env: &Env<'_>, value: &Value) {
        let snapshot = env.document.snapshot();
        let Some(object) = value
            .get("stair")
            .and_then(Value::as_str)
            .and_then(|s| s.parse().ok())
            .and_then(|id| archbuilder_model::spatial::get(&snapshot, id))
        else {
            return;
        };
        let Some(stair) = Stair::from_values(&object.values) else {
            return;
        };
        self.last = Map::new();
        self.last
            .insert("shape".into(), Value::from(stair.shape.name()));
        self.last
            .insert("turn".into(), Value::from(stair.turn.name()));
        self.last.insert("width".into(), Value::from(stair.width));
        self.last.insert("tread".into(), Value::from(stair.tread));
    }

    /// A click at `at` (project coordinates, on the storey's level plane).
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn click(&mut self, env: &Env<'_>, run: Runner<'_>, at: [f64; 2]) -> Result<()> {
        let takes_point = self
            .line
            .active()
            .and_then(|s| s.prompt())
            .is_some_and(|p| p.accepts.iter().any(|a| matches!(a, Accept::Point { .. })));
        if !takes_point {
            return Ok(());
        }
        let point = PromptInput::Point(Point {
            x: at[0],
            y: at[1],
            z: None,
        });
        self.step(env, run, LineStep::Answer(point))
    }

    /// A line the user typed: empty text (Enter) at the start ends the tool and elsewhere takes
    /// the prompt's default; other text answers the prompt (a point, a keyword, a length).
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn text(&mut self, env: &Env<'_>, run: Runner<'_>, text: &str) -> Result<()> {
        let text = text.trim();
        let defaulted = self
            .line
            .active()
            .and_then(|s| s.prompt())
            .is_some_and(|p| p.default.is_some());
        if text.is_empty() && !defaulted {
            self.stop();
            return Ok(());
        }
        self.step(env, run, LineStep::Submit(text))
    }

    /// Takes back the last answer (Undo point).
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn back(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        self.step(env, run, LineStep::Back)
    }

    /// Ends the tool. Stairs already made stay.
    pub fn stop(&mut self) {
        self.line.cancel();
        self.storey = None;
    }
}
