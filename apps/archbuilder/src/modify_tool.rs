//! The modify tool: the prompts of `archbuilder.modify.move`, `copy`, `rotate` and `mirror` behind
//! the viewport and the command line, without any windowing (it has the shape of the wall tool).
//!
//! A command starts on the objects that are selected (noun, then verb). With nothing selected the
//! tool waits while the viewport selects (verb, then noun: click, window, crossing) and Enter takes
//! the selection. The objects are then fixed as the preset `objects` of the command, and the tool
//! asks for its points. Move, rotate and mirror end with their command; copy asks for more copies
//! of the same objects from the same base point until Enter or Escape, one undo step each.

use std::collections::BTreeMap;

use arc_base::{Error, ObjectId, Result};
use arc_command::prompt::{Accept, Point, PromptInput};
use arc_command::{CommandLine, LineStep, Submitted};
use archbuilder_commands::modify::{
    CLIPBOARD_PASTE, MODIFY_COPY, MODIFY_MIRROR, MODIFY_MOVE, MODIFY_ROTATE,
};
use archbuilder_model::spatial;
use archbuilder_model::types::is_element;
use serde_json::{Map, Value};

use crate::wall_tool::{Env, Runner, target};

/// The objects a modify command takes from a selection: the building elements
/// ([`archbuilder_model::types::ELEMENTS`]).
#[must_use]
pub fn movable(snapshot: &arc_doc::Snapshot, selection: &[ObjectId]) -> Vec<ObjectId> {
    selection
        .iter()
        .copied()
        .filter(|id| spatial::type_of(snapshot, *id).is_some_and(|k| is_element(&k)))
        .collect()
}

/// The command a typed word names: `m`, `move`, `co`, `copy`, `ro`, `rotate`, `mi`, `mirror` and
/// the identifiers (what a toolbar button submits) and the Turkish `taşı`, `kopyala`, `döndür`, `aynala`, in any case.
#[must_use]
pub fn command_named(text: &str) -> Option<&'static str> {
    let word: String = text
        .trim()
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'ş' => 's',
            'ı' => 'i',
            'ö' => 'o',
            'ü' => 'u',
            'ç' => 'c',
            'ğ' => 'g',
            c => c,
        })
        .collect();
    Some(match word.as_str() {
        "m" | "move" | "tasi" | MODIFY_MOVE => MODIFY_MOVE,
        "co" | "cp" | "copy" | "kopyala" | MODIFY_COPY => MODIFY_COPY,
        "ro" | "rotate" | "dondur" | MODIFY_ROTATE => MODIFY_ROTATE,
        "mi" | "mirror" | "aynala" | MODIFY_MIRROR => MODIFY_MIRROR,
        _ => return None,
    })
}

/// A command in progress.
#[derive(Debug, Clone)]
pub struct Run {
    /// The command.
    pub command: &'static str,
    /// The objects, once gathered.
    pub objects: Vec<ObjectId>,
    /// Whether the tool still waits for the objects.
    pub gathering: bool,
    /// The base point of a copy, kept for the next copy.
    pub base: Option<[f64; 2]>,
    /// The copies made so far.
    pub copies: usize,
    /// The clipboard content a paste creates from, in place of `objects`.
    pub content: Option<Value>,
    /// The storey a paste goes into.
    pub storey: Option<ObjectId>,
}

/// The state of the modify tool of one tab.
#[derive(Debug, Default)]
pub struct ModifyTool {
    line: CommandLine,
    run: Option<Run>,
    error: Option<Error>,
    /// What the commands that ran returned, taken by the host.
    ran: Vec<(&'static str, Value)>,
    /// Enter ended the tool before anything was done.
    cancelled: bool,
}

impl ModifyTool {
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

    /// The command in progress.
    #[must_use]
    pub fn current(&self) -> Option<&Run> {
        self.run.as_ref()
    }

    /// True while a command runs, gathering objects or asking for points.
    #[must_use]
    pub fn running(&self) -> bool {
        self.run.is_some()
    }

    /// True while the tool asks for points (not while it gathers objects).
    #[must_use]
    pub fn prompting(&self) -> bool {
        self.line.active().is_some()
    }

    /// The error a click or a rejected answer left, taken once.
    pub fn take_error(&mut self) -> Option<Error> {
        self.error.take()
    }

    /// What the commands that ran returned since the last call.
    pub fn take_ran(&mut self) -> Vec<(&'static str, Value)> {
        std::mem::take(&mut self.ran)
    }

    /// Whether Enter ended the tool with nothing done since the last call; taken once.
    pub fn take_cancelled(&mut self) -> bool {
        std::mem::take(&mut self.cancelled)
    }

    /// True while the tool waits for the objects to be selected.
    #[must_use]
    pub fn gathering(&self) -> bool {
        self.run.as_ref().is_some_and(|r| r.gathering)
    }

    /// Whether a paste runs, where Enter pastes in place.
    #[must_use]
    pub fn pastes(&self) -> bool {
        self.run
            .as_ref()
            .is_some_and(|r| r.command == CLIPBOARD_PASTE)
    }

    /// The copies made in this run.
    #[must_use]
    pub fn copies(&self) -> usize {
        self.run.as_ref().map_or(0, |r| r.copies)
    }

    /// Starts `command` on the selection, or waits for objects when none of it can be taken.
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn begin(&mut self, env: &Env<'_>, run: Runner<'_>, command: &'static str) -> Result<()> {
        self.line.cancel();
        let objects = movable(&env.document.snapshot(), env.selection);
        let gathering = objects.is_empty();
        self.run = Some(Run {
            command,
            objects,
            gathering,
            base: None,
            copies: 0,
            content: None,
            storey: None,
        });
        if gathering {
            return Ok(());
        }
        self.next(env, run)
    }

    /// Starts a paste of the clipboard `content` into `storey`: the base point is `base`, the one
    /// point asked for is the insertion point, and Enter pastes in place.
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn begin_paste(
        &mut self,
        env: &Env<'_>,
        run: Runner<'_>,
        content: Value,
        base: [f64; 2],
        storey: ObjectId,
    ) -> Result<()> {
        self.line.cancel();
        self.run = Some(Run {
            command: CLIPBOARD_PASTE,
            objects: Vec::new(),
            gathering: false,
            base: Some(base),
            copies: 0,
            content: Some(content),
            storey: Some(storey),
        });
        self.next(env, run)
    }

    /// Enter while gathering: takes the selection as the objects.
    ///
    /// # Errors
    /// `ab.cmd.nothing_selected` while nothing movable is selected; the errors of the line.
    pub fn confirm(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        let objects = movable(&env.document.snapshot(), env.selection);
        if objects.is_empty() {
            return Err(Error::new(
                archbuilder_commands::error::codes::NOTHING_SELECTED,
            ));
        }
        if let Some(r) = &mut self.run {
            r.objects = objects;
            r.gathering = false;
        }
        self.next(env, run)
    }

    /// Starts the prompts of the next run of the command.
    fn next(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        let Some(state) = &self.run else {
            return Ok(());
        };
        let mut preset = Map::new();
        if let (Some(content), Some(storey)) = (&state.content, state.storey) {
            preset.insert("content".into(), content.clone());
            preset.insert("storey".into(), Value::from(storey.to_string()));
        } else {
            preset.insert(
                "objects".into(),
                Value::from(
                    state
                        .objects
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>(),
                ),
            );
        }
        let base = state
            .base
            .filter(|_| matches!(state.command, MODIFY_COPY | CLIPBOARD_PASTE));
        if let Some(b) = base {
            preset.insert("base".into(), Value::from(b.to_vec()));
        }
        let command = state.command;
        self.line.set_last_point(base.map(|b| Point {
            x: b[0],
            y: b[1],
            z: None,
        }))?;
        let localized = BTreeMap::new();
        let result = self.line.drive(
            &target(env, &localized),
            LineStep::Start { command, preset },
            run,
        );
        self.follow(env, run, result)
    }

    fn follow(&mut self, env: &Env<'_>, run: Runner<'_>, result: Result<Submitted>) -> Result<()> {
        match result {
            Err(error) => {
                self.run = None;
                Err(error)
            }
            Ok(Submitted::Rejected { error, .. }) => {
                self.error = Some(error);
                Ok(())
            }
            Ok(Submitted::Ran(outcome)) => {
                let command = self.run.as_ref().map_or(MODIFY_MOVE, |r| r.command);
                self.ran.push((command, outcome.value.clone()));
                let again = match &mut self.run {
                    Some(r) if r.command == MODIFY_COPY => {
                        r.copies += 1;
                        true
                    }
                    _ => false,
                };
                if again {
                    // The new copies are selected; the next copy is of the same objects.
                    self.next(env, run)
                } else {
                    self.run = None;
                    Ok(())
                }
            }
            Ok(Submitted::Cancelled | Submitted::Idle) => {
                self.run = None;
                Ok(())
            }
            Ok(Submitted::Prompt(_)) => {
                // The first point of a copy is the base of the next copies.
                if let (Some(r), Some(PromptInput::Point(p))) = (
                    &mut self.run,
                    self.line
                        .active()
                        .and_then(|s| s.answers().first())
                        .map(|a| &a.input),
                ) && r.command == MODIFY_COPY
                    && r.base.is_none()
                {
                    r.base = Some([p.x, p.y]);
                }
                Ok(())
            }
        }
    }

    /// A click at `at` answers a point prompt.
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn click(&mut self, env: &Env<'_>, run: Runner<'_>, at: [f64; 2]) -> Result<()> {
        let accepts_point = self
            .line
            .active()
            .and_then(|s| s.prompt())
            .is_some_and(|p| p.accepts.iter().any(|a| matches!(a, Accept::Point { .. })));
        if !accepts_point {
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

    /// A typed line. Empty text (Enter) takes the objects while gathering, ends the command at a
    /// point prompt and accepts the default of a keyword prompt.
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn text(&mut self, env: &Env<'_>, run: Runner<'_>, text: &str) -> Result<()> {
        let text = text.trim();
        if self.run.as_ref().is_some_and(|r| r.gathering) {
            return if text.is_empty() {
                self.confirm(env, run)
            } else {
                Err(Error::new(
                    archbuilder_commands::error::codes::SELECT_BEFORE_POINTS,
                ))
            };
        }
        let at_point = self
            .line
            .active()
            .and_then(|s| s.prompt())
            .is_some_and(|p| p.accepts.iter().any(|a| matches!(a, Accept::Point { .. })));
        // Enter at the insertion point of a paste: the copy lands where it was copied from.
        if text.is_empty()
            && at_point
            && let Some(base) = self
                .run
                .as_ref()
                .filter(|r| r.command == CLIPBOARD_PASTE)
                .and_then(|r| r.base)
        {
            return self.click(env, run, base);
        }
        if text.is_empty() && at_point {
            self.cancelled = self.copies() == 0;
            self.stop();
            return Ok(());
        }
        let localized = BTreeMap::new();
        let result = self
            .line
            .drive(&target(env, &localized), LineStep::Submit(text), run);
        self.follow(env, run, result)
    }

    /// Steps back one answer.
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn back(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        if !self.prompting() {
            return Ok(());
        }
        let localized = BTreeMap::new();
        let result = self
            .line
            .drive(&target(env, &localized), LineStep::Back, run);
        self.follow(env, run, result)
    }

    /// Points the direct distance of a typed length along the line from the base to `cursor`.
    pub fn aim(&mut self, cursor: [f64; 2]) {
        let direction = self.line.base_point().and_then(|from| {
            let (dx, dy) = (cursor[0] - from.x, cursor[1] - from.y);
            (dx.hypot(dy) > 1.0e-3).then_some([dx, dy])
        });
        let _ = self.line.set_pointer_direction(direction);
    }

    /// Ends the command; what was done stays.
    pub fn stop(&mut self) {
        self.line.cancel();
        self.run = None;
    }
}
