//! The datum tool: the prompts of `archbuilder.grid.create` and `archbuilder.reference_plane.create`
//! behind the viewport and the command line, without any windowing. A click gives the start, a
//! second click the end; the line is made and the tool asks for the next one, until Escape or
//! Enter. Grid labels count on by themselves: the tool passes the label it made last, so a new
//! line follows it (`A` then `B`, `1` then `2`) while a typed label starts a new series. A refusal
//! is kept as the pending error and the tool keeps waiting.

use std::collections::BTreeMap;

use arc_base::{Error, Result};
use arc_command::prompt::{Accept, Point, PromptInput};
use arc_command::{CommandLine, LineStep, Submitted, Target};
use archbuilder_commands::datum::GRID_CREATE;
use serde_json::{Map, Value};

use crate::wall_tool::{Env, Runner};

/// The state of the datum tool of one tab.
#[derive(Debug, Default)]
pub struct DatumTool {
    line: CommandLine,
    command: Option<&'static str>,
    error: Option<Error>,
    /// The label of the grid made last, which the next one follows.
    last: Option<String>,
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

impl DatumTool {
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

    /// The command the tool makes lines with, while it runs.
    #[must_use]
    pub fn command(&self) -> Option<&'static str> {
        self.command
    }

    /// True while the tool asks for a point.
    #[must_use]
    pub fn running(&self) -> bool {
        self.command.is_some() && self.line.active().is_some()
    }

    /// The error a click left, taken once.
    pub fn take_error(&mut self) -> Option<Error> {
        self.error.take()
    }

    /// Starts the tool with `command` (`archbuilder.grid.create` or
    /// `archbuilder.reference_plane.create`).
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn begin(&mut self, env: &Env<'_>, run: Runner<'_>, command: &'static str) -> Result<()> {
        self.line.cancel();
        self.line.set_last_point(None)?;
        self.command = Some(command);
        self.next(env, run)
    }

    fn next(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        let Some(command) = self.command else {
            return Ok(());
        };
        let mut preset = Map::new();
        if command == GRID_CREATE
            && let Some(last) = &self.last
        {
            preset.insert("after".into(), Value::from(last.clone()));
        }
        let localized = BTreeMap::new();
        let result = self.line.drive(
            &target(env, &localized),
            LineStep::Start { command, preset },
            run,
        );
        if result.is_err() {
            self.stop();
        }
        result.map(|_| ())
    }

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
                if let Some(label) = outcome.value.get("label").and_then(Value::as_str) {
                    self.last = Some(label.to_owned());
                }
                self.next(env, run)
            }
            Ok(Submitted::Cancelled | Submitted::Idle) => {
                self.stop();
                Ok(())
            }
            Ok(_) => Ok(()),
        }
    }

    /// A click at `at` (project coordinates).
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

    /// A line the user typed: empty text (Enter) ends the tool; other text answers the prompt.
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

    /// Takes back the last answer (Undo point).
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn back(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        self.step(env, run, LineStep::Back)
    }

    /// Ends the tool. Lines already made stay.
    pub fn stop(&mut self) {
        self.line.cancel();
        self.command = None;
    }
}
