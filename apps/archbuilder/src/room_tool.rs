//! The room tool: the prompts of `archbuilder.room.place` behind the viewport and the command
//! line, without any windowing. A click inside a region closed by walls places a room there and
//! the tool asks for the next point, until Escape or Enter. A click the command refuses (outside
//! any loop of walls, or in a region that has a room) is kept as the pending error and the tool
//! keeps waiting.

use std::collections::BTreeMap;

use arc_base::{Error, ObjectId, Result};
use arc_command::prompt::{Accept, Point, PromptInput};
use arc_command::{CommandLine, LineStep, Submitted, Target};
use archbuilder_commands::room::ROOM_PLACE;
use serde_json::{Map, Value};

use crate::wall_tool::{Env, Runner, storey_for};

/// The state of the room tool of one tab.
#[derive(Debug, Default)]
pub struct RoomTool {
    line: CommandLine,
    storey: Option<ObjectId>,
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

impl RoomTool {
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

    /// The storey rooms are placed on, while the tool runs.
    #[must_use]
    pub fn storey(&self) -> Option<ObjectId> {
        self.storey
    }

    /// True while the tool asks for a point.
    #[must_use]
    pub fn running(&self) -> bool {
        self.storey.is_some() && self.line.active().is_some()
    }

    /// The error a click left, taken once.
    pub fn take_error(&mut self) -> Option<Error> {
        self.error.take()
    }

    /// Starts the tool on the storey of the selection (else the lowest).
    ///
    /// # Errors
    /// `ab.cmd.no_building` without a storey; the errors of the command line.
    pub fn begin(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        self.line.cancel();
        let (storey, _) = storey_for(&env.document.snapshot(), env.selection)
            .ok_or_else(|| Error::new(archbuilder_commands::error::codes::NO_BUILDING))?;
        self.line.set_last_point(None)?;
        self.storey = Some(storey);
        self.next(env, run)
    }

    fn next(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        let Some(storey) = self.storey else {
            return Ok(());
        };
        let mut preset = Map::new();
        preset.insert("storey".into(), Value::from(storey.to_string()));
        let localized = BTreeMap::new();
        let step = LineStep::Start {
            command: ROOM_PLACE,
            preset,
        };
        let result = self.line.drive(&target(env, &localized), step, run);
        if result.is_err() {
            self.stop();
        }
        result.map(|_| ())
    }

    /// Takes a step of the line: a room placed asks for the next point; a refusal is kept as the
    /// pending error and the tool asks again.
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
            Ok(Submitted::Ran(_)) => self.next(env, run),
            Ok(Submitted::Cancelled | Submitted::Idle) => {
                self.stop();
                Ok(())
            }
            Ok(_) => Ok(()),
        }
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

    /// A line the user typed: empty text (Enter) ends the tool, other text answers the prompt (a
    /// point such as `5,3`).
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn text(&mut self, env: &Env<'_>, run: Runner<'_>, text: &str) -> Result<()> {
        let text = text.trim();
        if text.is_empty() {
            self.stop();
            return Ok(());
        }
        self.step(env, run, LineStep::Submit(text))
    }

    /// Ends the tool. Rooms already placed stay.
    pub fn stop(&mut self) {
        self.line.cancel();
        self.storey = None;
    }
}
