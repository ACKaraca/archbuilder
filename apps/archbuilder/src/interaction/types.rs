//! Element types in the viewport: the type selector's state (the running tool's kind and what the
//! selection shows), the types an older document gets when it opens, create similar, match type
//! (a source, then targets clicked one after another) and edit type.

use arc_app::shell::InteractionContext;
use arc_base::{Error, ObjectId, Result};
use arc_command::Invocation;
use arc_doc::Snapshot;
use arc_ui::egui::PointerButton;
use arc_ui::{Intent, PointerEvent, PointerKind};
use archbuilder_commands::circulation::{RAILING_CREATE, RAMP_CREATE};
use archbuilder_commands::element_type::{TYPE_ASSIGN, TYPE_MATCH, TYPE_UPGRADE, needs_upgrade};
use archbuilder_commands::error::codes;
use archbuilder_commands::floor::FLOOR_CREATE;
use archbuilder_commands::insert::{DOOR_PLACE, WINDOW_PLACE};
use archbuilder_commands::wall::WALL_CREATE;
use archbuilder_model::element_type::{self as et, TypeKind};
use archbuilder_model::spatial;

use super::{PICK_PIXELS, WallInteraction, view_ray};
use crate::circ_types::CircKind;
use crate::shortcuts::ui;
use crate::tool_type;

/// A message of the catalog, named by identifier.
struct Message {
    message_id: &'static str,
}

const MATCH_SOURCE: Message = Message {
    message_id: "ab-notice-match-source",
};
const MATCH_TARGETS: Message = Message {
    message_id: "ab-notice-match-targets",
};
const TYPES_ADDED: Message = Message {
    message_id: "ab-notice-types-added",
};
const TYPES_RESYNCED: Message = Message {
    message_id: "ab-notice-types-resynced",
};
const NO_TYPED: Message = Message {
    message_id: "ab-notice-no-typed",
};

/// The invocation that gives an older document's instances types, when the document needs it:
/// a document just opened (revision 0) with walls, floors, doors or windows without a type. It
/// runs as the document's starting content, so it is no undo step; saving writes the types.
#[must_use]
pub fn upgrade_invocation(snapshot: &Snapshot) -> Option<Invocation> {
    (snapshot.revision() == 0 && needs_upgrade(snapshot)).then(|| Invocation {
        initial: true,
        ..Invocation::new(TYPE_UPGRADE, serde_json::json!({}))
    })
}

/// The command that starts the placement tool of `kind`.
fn tool_command(kind: TypeKind) -> &'static str {
    match kind {
        TypeKind::Wall => WALL_CREATE,
        TypeKind::Floor => FLOOR_CREATE,
        TypeKind::Door => DOOR_PLACE,
        TypeKind::Window => WINDOW_PLACE,
        TypeKind::Roof => archbuilder_commands::roof::ROOF_CREATE,
        TypeKind::Ceiling => archbuilder_commands::ceiling::CEILING_CREATE,
        TypeKind::Family => archbuilder_commands::family_project::COMPONENT_PLACE,
    }
}

/// The name of the type of the railing or ramp `id`, when it has one.
fn circulation_type_name(snapshot: &Snapshot, id: ObjectId) -> Option<String> {
    let (_, found) = CircKind::of_object(snapshot, id)?;
    let object = spatial::get(snapshot, found?)?;
    archbuilder_model::types::text(&object.values, 0).map(str::to_owned)
}

/// The first selected object that has a type, with its kind and type.
fn typed_in(snapshot: &Snapshot, selection: &[ObjectId]) -> Option<(ObjectId, TypeKind, ObjectId)> {
    selection.iter().find_map(|id| {
        let object = spatial::get(snapshot, *id)?;
        let (kind, found) = et::type_of_object(snapshot, &object)?;
        Some((*id, kind, found.id))
    })
}

impl WallInteraction {
    /// Each frame: the kind of the running placement tool and what the selection shows go to
    /// the type selector; a document from before types gets its types once.
    pub(super) fn type_frame(&mut self, cx: &InteractionContext<'_>) {
        let active = if self.tool.active() {
            Some(TypeKind::Wall)
        } else if self.floors.running() {
            Some(match self.floors.state() {
                Some(s) if s.roof => TypeKind::Roof,
                Some(s) if s.command == archbuilder_commands::ceiling::CEILING_CREATE => {
                    TypeKind::Ceiling
                }
                _ => TypeKind::Floor,
            })
        } else {
            // A bare opening has no type: the selector stays empty.
            self.inserts.state().and_then(|s| match s.kind {
                archbuilder_model::insert::Kind::Door => Some(TypeKind::Door),
                archbuilder_model::insert::Kind::Window => Some(TypeKind::Window),
                archbuilder_model::insert::Kind::Opening => None,
            })
        };
        tool_type::set_active(active);
        let Ok(document) = cx.document() else {
            return;
        };
        let snapshot = document.snapshot();
        tool_type::publish(&snapshot, cx.selection());
        if cx.read_only() {
            return;
        }
        // Once per document: a refused upgrade is said once, not every frame.
        if self.upgrade_tried {
            return;
        }
        self.upgrade_tried = true;
        if let Some(mut invocation) = upgrade_invocation(&snapshot) {
            invocation.seed = self.next_seed();
            match cx.invoke(invocation) {
                Ok(outcome) => {
                    let count = |key: &str| {
                        outcome
                            .value
                            .get(key)
                            .and_then(serde_json::Value::as_i64)
                            .unwrap_or(0)
                    };
                    for (key, message) in [
                        ("instances", TYPES_ADDED.message_id),
                        ("resynced", TYPES_RESYNCED.message_id),
                    ] {
                        let n = count(key);
                        if n > 0 {
                            self.say(cx, message, &[("count", arc_base::error::Value::Int(n))]);
                        }
                    }
                }
                Err(error) => self.notice_error(cx, &error),
            }
        }
    }

    /// The type commands of the interface: `None` when `intent` is not one of them.
    pub(super) fn type_intent(
        &mut self,
        cx: &InteractionContext<'_>,
        intent: &Intent,
    ) -> Option<Result<()>> {
        Some(match intent.command.as_str() {
            ui::TYPE_PICK => self.pick_type(cx, intent),
            ui::CREATE_SIMILAR => self.create_similar(cx),
            ui::MATCH_TYPE => self.start_matching(cx),
            ui::EDIT_TYPE => {
                cx.ask_panel(crate::TYPES);
                Ok(())
            }
            _ => return None,
        })
    }

    /// A type picked in the selector: the running tool's next type, else the type of the
    /// selected objects of the kind the selector lists.
    fn pick_type(&mut self, cx: &InteractionContext<'_>, intent: &Intent) -> Result<()> {
        let name = intent
            .params
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let snapshot = cx.document()?.snapshot();
        let Some((kind, _)) = tool_type::shown(&snapshot, cx.selection()) else {
            return Err(Error::new(codes::NOTHING_SELECTED));
        };
        let id = archbuilder_commands::element_type::resolve(&snapshot, kind, name)?;
        if tool_type::active() == Some(kind) {
            tool_type::choose(kind, id);
            // The prompt in progress takes the new type when it can.
            return match kind {
                TypeKind::Wall => self.with_tool(cx, |tool, env, run| tool.retype(env, run)),
                TypeKind::Door | TypeKind::Window => {
                    self.with_inserts(cx, |tool, env, run| tool.retype(env, run))
                }
                TypeKind::Floor | TypeKind::Roof | TypeKind::Ceiling => {
                    self.with_floors(cx, |tool, env, run| tool.retype(env, run))
                }
                // The prompts of the component command take the type when they ask for it.
                TypeKind::Family => Ok(()),
            };
        }
        let mut invocation = Invocation::new(
            TYPE_ASSIGN,
            serde_json::json!({"type": id.to_string(), "objects": cx.selection().iter().map(ToString::to_string).collect::<Vec<_>>()}),
        );
        invocation.seed = self.next_seed();
        cx.invoke(invocation)?;
        self.after_command(cx);
        Ok(())
    }

    /// Starts the tool of the selected wall, floor, door or window with its type.
    fn create_similar(&mut self, cx: &InteractionContext<'_>) -> Result<()> {
        let snapshot = cx.document()?.snapshot();
        if let Some((circ, Some(found))) = cx
            .selection()
            .iter()
            .find_map(|id| CircKind::of_object(&snapshot, *id))
        {
            let (command, key) = match circ {
                CircKind::Railing => (RAILING_CREATE, "railing_type"),
                CircKind::Ramp => (RAMP_CREATE, "ramp_type"),
            };
            self.stop_all();
            return self.with_stairs(cx, |tool, env, run| {
                tool.begin_similar(env, run, command, key, found)
            });
        }
        let Some((_, kind, found)) = typed_in(&snapshot, cx.selection()) else {
            return Err(Error::new(codes::NOTHING_SELECTED));
        };
        tool_type::choose(kind, found);
        self.intent(cx, &Intent::new(tool_command(kind)))
    }

    /// Match type: with a typed object selected it is the source and clicks pick the targets;
    /// else the first click picks the source.
    fn start_matching(&mut self, cx: &InteractionContext<'_>) -> Result<()> {
        self.stop_all();
        let snapshot = cx.document()?.snapshot();
        let source = typed_in(&snapshot, cx.selection())
            .map(|(id, _, _)| id)
            .or_else(|| {
                cx.selection()
                    .iter()
                    .copied()
                    .find(|id| circulation_type_name(&snapshot, *id).is_some())
            });
        self.matching = Some(source);
        self.note_match_source(&snapshot);
        Ok(())
    }

    fn note_match_source(&mut self, snapshot: &Snapshot) {
        // The command line shows the step through [`Self::match_prompt`]; only the name is kept.
        if let Some(Some(source)) = self.matching {
            self.match_type_name = spatial::get(snapshot, source)
                .and_then(|o| et::type_of_object(snapshot, &o))
                .map(|(_, t)| et::name(&t).to_owned())
                .or_else(|| circulation_type_name(snapshot, source))
                .unwrap_or_default();
        }
    }

    /// The prompt of match type while it runs, so the command line shows it and Escape cancels it
    /// as any prompt.
    pub(super) fn match_prompt(&self) -> Option<arc_command::prompt::Prompt> {
        let source = self.matching?;
        let accept = arc_command::prompt::Accept::Selection {
            min: 1,
            max: Some(1),
            types: Vec::new(),
        };
        Some(match source {
            None => {
                arc_command::prompt::Prompt::new("match-source", MATCH_SOURCE.message_id, accept)
            }
            Some(_) => {
                arc_command::prompt::Prompt::new("match-target", MATCH_TARGETS.message_id, accept)
                    .with_arg("type", self.match_type_name.as_str())
            }
        })
    }

    /// The pointer while match type runs: a primary click picks the source, then each target.
    /// `None` when match type does not run.
    pub(super) fn match_pointer(
        &mut self,
        cx: &InteractionContext<'_>,
        event: &PointerEvent,
    ) -> Option<bool> {
        let source = self.matching?;
        if event.kind != PointerKind::Click || event.button != Some(PointerButton::Primary) {
            return Some(false);
        }
        let (Some(model), Ok(document)) = (event.model, cx.document()) else {
            return Some(false);
        };
        let snapshot = document.snapshot();
        let aperture = event
            .pixels_per_unit
            .filter(|p| *p > 0.0)
            .map_or(0.05, |p| PICK_PIXELS / p);
        let vis = &self.habits.vis;
        let target = self
            .picker
            .target(
                &snapshot,
                &crate::picker::Probe {
                    pixel: event.pixel,
                    at: [model.x, model.y],
                    aperture,
                    ray: view_ray(cx, model),
                    now: std::time::Instant::now(),
                    shown: &|id| vis.shows(&snapshot, id),
                },
            )
            .map(|t| t.id);
        let Some(target) = target else {
            return Some(true);
        };
        match source {
            None => {
                if typed_in(&snapshot, &[target]).is_some()
                    || circulation_type_name(&snapshot, target).is_some()
                {
                    self.matching = Some(Some(target));
                    self.note_match_source(&snapshot);
                } else {
                    self.say(cx, NO_TYPED.message_id, &[]);
                }
            }
            Some(source) => {
                let mut invocation = Invocation::new(
                    TYPE_MATCH,
                    serde_json::json!({"source": source.to_string(), "objects": [target.to_string()]}),
                );
                invocation.seed = self.next_seed();
                match cx.invoke(invocation) {
                    Ok(_) => self.after_command(cx),
                    Err(error) => self.notice_error(cx, &error),
                }
            }
        }
        Some(true)
    }
}
