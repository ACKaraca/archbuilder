//! Attach top in the window: the walls are the selected ones, else the prompt asks to select
//! them (Enter takes the selection); then a click on a roof or a floor attaches their tops to
//! it, and Enter attaches each to the roof or floor above it. A wall that cannot follow what it
//! is attached to is said in the history.

use arc_app::shell::InteractionContext;
use arc_base::{ObjectId, Result};
use arc_command::Invocation;
use arc_command::prompt::{Accept, Prompt};
use arc_ui::egui::PointerButton;
use arc_ui::{PointerEvent, PointerKind};
use archbuilder_commands::roof::WALL_ATTACH_TOP;
use archbuilder_model::spatial;
use archbuilder_model::types::{FLOOR, ROOF, WALL};

use super::{PICK_PIXELS, WallInteraction};

/// A message of the application, named by identifier.
struct Msg {
    message_id: &'static str,
}

const SELECT_WALLS: Msg = Msg {
    message_id: "ab-prompt-attach-walls",
};
const PICK_TARGET: Msg = Msg {
    message_id: "ab-prompt-attach-target",
};
const NOTHING_THERE: Msg = Msg {
    message_id: "ab-notice-attach-nothing-there",
};
const NOT_FOLLOWING: Msg = Msg {
    message_id: "ab-notice-attach-not-following",
};
const ATTACHED: Msg = Msg {
    message_id: "ab-notice-attached",
};

/// Where attach top is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Attach {
    /// Waiting for walls to be selected.
    Walls,
    /// Waiting for the roof or floor these walls attach to.
    Target(Vec<ObjectId>),
}

fn walls_in(cx: &InteractionContext<'_>) -> Vec<ObjectId> {
    let Ok(document) = cx.document() else {
        return Vec::new();
    };
    let snapshot = document.snapshot();
    cx.selection()
        .iter()
        .copied()
        .filter(|id| spatial::type_of(&snapshot, *id).as_deref() == Some(WALL))
        .collect()
}

impl WallInteraction {
    /// Starts attach top with the selected walls, else asks for them.
    pub(super) fn start_attach(&mut self, cx: &InteractionContext<'_>) {
        self.stop_all();
        let walls = walls_in(cx);
        self.attaching = Some(if walls.is_empty() {
            Attach::Walls
        } else {
            Attach::Target(walls)
        });
    }

    /// The prompt of attach top while it runs.
    pub(super) fn attach_prompt(&self) -> Option<Prompt> {
        Some(match self.attaching.as_ref()? {
            Attach::Walls => Prompt::new(
                "attach-walls",
                SELECT_WALLS.message_id,
                Accept::Selection {
                    min: 1,
                    max: None,
                    types: vec![WALL.to_owned()],
                },
            ),
            Attach::Target(walls) => Prompt::new(
                "attach-target",
                PICK_TARGET.message_id,
                Accept::Pick {
                    types: vec![ROOF.to_owned(), FLOOR.to_owned()],
                },
            )
            .with_arg("count", walls.len().to_string()),
        })
    }

    /// Enter while attach top runs: takes the selected walls, or attaches to what is above.
    pub(super) fn attach_enter(&mut self, cx: &InteractionContext<'_>) {
        match self.attaching.take() {
            Some(Attach::Walls) => {
                let walls = walls_in(cx);
                self.attaching = Some(if walls.is_empty() {
                    Attach::Walls
                } else {
                    Attach::Target(walls)
                });
            }
            Some(Attach::Target(walls)) => self.run_attach(cx, &walls, None),
            None => {}
        }
    }

    /// The pointer while attach top runs: while it asks for walls the viewport selects as
    /// always; while it asks for the target a click on a roof or floor attaches to it.
    pub(super) fn attach_pointer(
        &mut self,
        cx: &InteractionContext<'_>,
        event: &PointerEvent,
    ) -> Option<bool> {
        let Some(Attach::Target(walls)) = self.attaching.clone() else {
            return self.attaching.as_ref().map(|_| false);
        };
        if event.kind != PointerKind::Click || event.button != Some(PointerButton::Primary) {
            return Some(false);
        }
        let (Some(model), Ok(document)) = (event.model, cx.document()) else {
            return Some(true);
        };
        let snapshot = document.snapshot();
        let aperture = event
            .pixels_per_unit
            .filter(|p| *p > 0.0)
            .map_or(0.05, |p| PICK_PIXELS / p);
        let rooms = archbuilder_model::pick::all_rooms(&snapshot);
        let target =
            archbuilder_model::pick::stack(&snapshot, &rooms, [model.x, model.y], aperture, None)
                .into_iter()
                .find(|id| {
                    matches!(
                        spatial::type_of(&snapshot, *id).as_deref(),
                        Some(ROOF | FLOOR)
                    )
                });
        match target {
            Some(target) => {
                self.attaching = None;
                self.run_attach(cx, &walls, Some(target));
            }
            None => self.say(cx, NOTHING_THERE.message_id, &[]),
        }
        Some(true)
    }

    fn run_attach(
        &mut self,
        cx: &InteractionContext<'_>,
        walls: &[ObjectId],
        target: Option<ObjectId>,
    ) {
        let mut params = serde_json::json!({
            "walls": walls.iter().map(ToString::to_string).collect::<Vec<_>>()
        });
        if let (Some(t), Some(map)) = (target, params.as_object_mut()) {
            map.insert("target".into(), serde_json::Value::from(t.to_string()));
        }
        let mut invocation = Invocation::new(WALL_ATTACH_TOP, params);
        invocation.seed = self.next_seed();
        let result: Result<_> = cx.invoke(invocation);
        match result {
            Ok(outcome) => {
                self.after_command(cx);
                let count = |key: &str| outcome.value.get(key).and_then(serde_json::Value::as_i64);
                let done = count("walls").unwrap_or(0);
                let not = count("not_following").unwrap_or(0);
                self.say(
                    cx,
                    ATTACHED.message_id,
                    &[("count", arc_base::error::Value::Int(done - not))],
                );
                if not > 0 {
                    self.say(
                        cx,
                        NOT_FOLLOWING.message_id,
                        &[("count", arc_base::error::Value::Int(not))],
                    );
                }
            }
            Err(error) => self.notice_error(cx, &error),
        }
    }
}
