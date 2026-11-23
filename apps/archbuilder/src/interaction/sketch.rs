//! The floor boundary sketch in the window: Edit boundary (double click on a floor, the context
//! menu, the menu, or the typed word) opens the floor's outline and openings as editable loops
//! ([`crate::floor_sketch`]); the prompt offers the modes, Finish and Cancel. Enter or Finish
//! validates the sketch and applies it as one command; Escape or Cancel drops it.

use arc_app::shell::InteractionContext;
use arc_base::{Error, ObjectId, Result};
use arc_command::Invocation;
use arc_command::prompt::{Accept, KeywordOption, Prompt};
use arc_ui::egui::PointerButton;
use arc_ui::{PointerEvent, PointerKind, ProductOverlay};
use archbuilder_commands::floor::FLOOR_EDIT_BOUNDARY;
use archbuilder_model::floor;
use archbuilder_model::spatial;
use archbuilder_model::wall::Wall;

use super::WallInteraction;
use crate::floor_sketch::{FloorSketch, Hint, Mode};

/// A message of the application, named by identifier.
struct Msg {
    message_id: &'static str,
}

const PROMPT: Msg = Msg {
    message_id: "ab-prompt-sketch",
};
const PROMPT_ARC: Msg = Msg {
    message_id: "ab-prompt-sketch-arc",
};
const PROMPT_WALL: Msg = Msg {
    message_id: "ab-prompt-sketch-wall",
};
const FREED: Msg = Msg {
    message_id: "ab-notice-sketch-freed",
};
const NOT_FINISHED: Msg = Msg {
    message_id: "ab-notice-sketch-refused",
};
const CANCELLED: Msg = Msg {
    message_id: "ab-notice-sketch-cancelled",
};
const APPLIED: Msg = Msg {
    message_id: "ab-notice-sketch-applied",
};
const NO_FLOOR: Msg = Msg {
    message_id: "ab-notice-sketch-no-floor",
};
const PICK_EDGE: Msg = Msg {
    message_id: "ab-notice-sketch-pick-edge",
};
const PROMPT_DELETE: Msg = Msg {
    message_id: "ab-prompt-sketch-delete",
};
const TOO_FEW: Msg = Msg {
    message_id: "ab-notice-sketch-too-few",
};

/// The keywords of the sketch prompt, in order: the identifier, the label and the shortcut
/// letter.
const KEYWORDS: [(&str, Msg, char); 7] = [
    (
        "finish",
        Msg {
            message_id: "ab-keyword-sketch-finish",
        },
        'f',
    ),
    (
        "cancel",
        Msg {
            message_id: "ab-keyword-sketch-cancel",
        },
        'c',
    ),
    (
        "add",
        Msg {
            message_id: "ab-keyword-sketch-add",
        },
        'a',
    ),
    (
        "arc",
        Msg {
            message_id: "ab-keyword-sketch-arc",
        },
        'r',
    ),
    (
        "wall",
        Msg {
            message_id: "ab-keyword-sketch-wall",
        },
        'w',
    ),
    (
        "straight",
        Msg {
            message_id: "ab-keyword-sketch-straight",
        },
        's',
    ),
    (
        "delete",
        Msg {
            message_id: "ab-keyword-sketch-delete",
        },
        'd',
    ),
];

/// Whether `text` is the typed word that starts the sketch.
pub(super) fn is_sketch_word(text: &str) -> bool {
    let t = text.trim().to_lowercase().replace(' ', "");
    [
        archbuilder_commands::floor::FLOOR_EDIT_BOUNDARY,
        "editboundary",
        "sinirduzenle",
        "sınırdüzenle",
    ]
    .contains(&t.as_str())
}

impl WallInteraction {
    /// Opens the sketch of the floor `id`, else of the selected floor.
    pub(super) fn start_sketch(
        &mut self,
        cx: &InteractionContext<'_>,
        id: Option<ObjectId>,
    ) -> Result<()> {
        self.stop_all();
        let snapshot = cx.document()?.snapshot();
        let floor = id.or_else(|| {
            cx.selection().iter().copied().find(|i| {
                spatial::type_of(&snapshot, *i)
                    .is_some_and(|k| archbuilder_model::floor::slots_of(&k).is_some())
            })
        });
        let Some(sketch) = floor.and_then(|f| FloorSketch::begin(&snapshot, f)) else {
            self.say(cx, NO_FLOOR.message_id, &[]);
            return Ok(());
        };
        self.sketching = Some(sketch);
        self.sketch_valid = true;
        Ok(())
    }

    /// The prompt of the sketch while it is open.
    pub(super) fn sketch_prompt(&self) -> Option<Prompt> {
        let s = self.sketching.as_ref()?;
        let message = match (s.mode, s.arc_pending()) {
            (Mode::Arc, _) => PROMPT_ARC.message_id,
            (Mode::Wall, _) => PROMPT_WALL.message_id,
            (Mode::Delete, _) => PROMPT_DELETE.message_id,
            _ => PROMPT.message_id,
        };
        Some(Prompt::new(
            "floor-sketch",
            message,
            Accept::keywords(KEYWORDS.iter().map(|(id, label, key)| {
                KeywordOption::new(*id, label.message_id).with_shortcut(*key)
            })),
        ))
    }

    /// What the sketch draws.
    pub(super) fn sketch_overlays(&self) -> Vec<ProductOverlay> {
        self.sketching
            .as_ref()
            .map(|s| s.overlays(self.ppu, self.sketch_valid))
            .unwrap_or_default()
    }

    fn sketch_walls(
        &self,
        cx: &InteractionContext<'_>,
    ) -> Option<(arc_doc::Snapshot, Vec<(ObjectId, Wall)>)> {
        let snapshot = cx.document().ok()?.snapshot();
        let floor = self.sketching.as_ref()?.floor;
        let storey = spatial::get(&snapshot, floor)?.header.owner?;
        let walls = floor::storey_walls(&snapshot, storey);
        Some((snapshot, walls))
    }

    /// Checks the sketch after an edit, and says the edges it freed.
    fn sketch_after(&mut self, cx: &InteractionContext<'_>) {
        let Some((snapshot, walls)) = self.sketch_walls(cx) else {
            return;
        };
        let of = |id: ObjectId| {
            walls
                .iter()
                .find(|(w, _)| *w == id)
                .map(|(_, w)| *w)
                .or_else(|| spatial::get(&snapshot, id).and_then(|o| Wall::from_values(&o.values)))
        };
        let freed = match self.sketching.as_mut() {
            Some(s) => {
                s.refresh(&of);
                self.sketch_valid = s.check(&of).is_ok();
                s.take_freed()
            }
            None => 0,
        };
        if freed > 0 {
            self.say(
                cx,
                FREED.message_id,
                &[(
                    "count",
                    arc_base::error::Value::Int(i64::try_from(freed).unwrap_or(0)),
                )],
            );
        }
    }

    /// The pointer while the sketch is open.
    pub(super) fn sketch_pointer(
        &mut self,
        cx: &InteractionContext<'_>,
        event: &PointerEvent,
    ) -> bool {
        let Some(model) = event.model else {
            return false;
        };
        let p = [model.x, model.y];
        let ppu = self.ppu.max(1.0e-6);
        let primary = event.button == Some(PointerButton::Primary);
        let Some((_, walls)) = self.sketch_walls(cx) else {
            return false;
        };
        let taken = match (event.kind, self.sketching.as_mut()) {
            (PointerKind::Move, Some(s)) => s.moved(p),
            (PointerKind::Press, Some(s)) if primary => s.press(p, ppu),
            (PointerKind::Release, Some(s)) if primary => s.release(),
            (PointerKind::Click, Some(s)) if primary => {
                let lookup = |q: [f64; 2]| {
                    floor::wall_at(&walls, q)
                        .and_then(|id| walls.iter().find(|(w, _)| *w == id).copied())
                };
                s.click(p, ppu, &lookup);
                true
            }
            _ => false,
        };
        if taken {
            self.sketch_after(cx);
        }
        let hint = self.sketching.as_mut().and_then(FloorSketch::take_hint);
        match hint {
            Some(Hint::PickEdgeFirst) => self.say(cx, PICK_EDGE.message_id, &[]),
            Some(Hint::LoopTooSmall) => self.say(cx, TOO_FEW.message_id, &[]),
            None => {}
        }
        taken
    }

    /// Applies the sketch as one command.
    fn sketch_finish(&mut self, cx: &InteractionContext<'_>) -> Result<()> {
        let Some(sketch) = self.sketching.clone() else {
            return Ok(());
        };
        let Some((snapshot, walls)) = self.sketch_walls(cx) else {
            return Ok(());
        };
        let of = |id: ObjectId| {
            walls
                .iter()
                .find(|(w, _)| *w == id)
                .map(|(_, w)| *w)
                .or_else(|| spatial::get(&snapshot, id).and_then(|o| Wall::from_values(&o.values)))
        };
        if let Err(refusal) = sketch.check(&of) {
            // The sketch stays open so it can be fixed.
            self.sketch_valid = false;
            let error =
                Error::new(archbuilder_model::validate::refusal_code(refusal)).with_arg("name", "");
            self.say(cx, NOT_FINISHED.message_id, &[]);
            self.notice_error(cx, &error);
            return Ok(());
        }
        if !sketch.changed {
            self.sketching = None;
            return Ok(());
        }
        let mut invocation = Invocation::new(FLOOR_EDIT_BOUNDARY, sketch.params());
        invocation.seed = self.next_seed();
        match cx.invoke(invocation) {
            Ok(_) => {
                self.sketching = None;
                self.say(cx, APPLIED.message_id, &[]);
                Ok(())
            }
            Err(error) => {
                self.sketch_valid = false;
                self.notice_error(cx, &error);
                Ok(())
            }
        }
    }

    /// Escape: the sketch is dropped and the floor stays as it is.
    pub(super) fn sketch_cancel(&mut self, cx: &InteractionContext<'_>) {
        if self.sketching.take().is_some() {
            self.say(cx, CANCELLED.message_id, &[]);
        }
    }

    /// A line typed or a button pressed while the sketch is open.
    pub(super) fn sketch_text(&mut self, cx: &InteractionContext<'_>, text: &str) -> Result<()> {
        let word = text.trim().to_lowercase();
        if word.is_empty() {
            return self.sketch_finish(cx);
        }
        let language = cx.language().to_owned();
        let named = KEYWORDS.iter().find(|(id, label, key)| {
            word == *id
                || (word.chars().count() == 1 && word.starts_with(*key))
                || self.localizer(&language).is_some_and(|l| {
                    arc_ui::fold_case(&l.format(label.message_id, &[]).text, &language)
                        == arc_ui::fold_case(&word, &language)
                })
        });
        match named.map(|k| k.0) {
            Some("finish") => return self.sketch_finish(cx),
            Some("cancel") => self.sketch_cancel(cx),
            Some(mode @ ("add" | "arc" | "wall")) => {
                if let Some(s) = self.sketching.as_mut() {
                    s.mode = match (mode, s.mode) {
                        ("add", Mode::Add) | ("arc", Mode::Arc) | ("wall", Mode::Wall) => {
                            Mode::Select
                        }
                        ("add", _) => Mode::Add,
                        ("arc", _) => Mode::Arc,
                        _ => Mode::Wall,
                    };
                    s.reset_pending();
                }
            }
            Some("straight") => {
                if let Some(s) = self.sketching.as_mut() {
                    s.straighten_selected();
                }
                self.sketch_after(cx);
            }
            Some("delete") => {
                // A selected vertex goes at once; else the next click on a vertex deletes it.
                let selected = self.sketching.as_ref().is_some_and(|s| {
                    matches!(s.selected, Some(crate::floor_sketch::Sel::Vertex(_)))
                });
                if selected {
                    let done = self
                        .sketching
                        .as_mut()
                        .is_some_and(FloorSketch::delete_selected);
                    if !done {
                        self.say(cx, TOO_FEW.message_id, &[]);
                    }
                    self.sketch_after(cx);
                } else if let Some(s) = self.sketching.as_mut() {
                    s.mode = if s.mode == Mode::Delete {
                        Mode::Select
                    } else {
                        Mode::Delete
                    };
                    s.reset_pending();
                }
            }
            _ => {}
        }
        Ok(())
    }
}
