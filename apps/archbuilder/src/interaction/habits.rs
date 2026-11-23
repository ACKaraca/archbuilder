//! The everyday habits of the viewport that sit around the tools: Tab through the candidates under
//! the pointer, the context menu, double clicks, the clipboard, typed two-letter shortcuts, Space
//! and Repeat, selection commands, temporary hide and isolate, and the snap switch. The state is
//! [`Habits`]; the behavior is more methods of [`WallInteraction`], so the tools stay as they are.

use std::collections::BTreeSet;

use arc_app::settings::SettingValue;
use arc_app::shell::{
    ContextEntry, Interaction, InteractionContext, KeyInput, KeyReply, ViewRequest,
};
use arc_base::{Error, ObjectId, Result};
use arc_command::{Invocation, Invoker, Roots};
use arc_ui::egui::Key;
use arc_ui::{DynamicEvent, DynamicField, Intent, LineAction, PointerEvent, PointerKind};
use archbuilder_commands::error::codes;
use archbuilder_commands::floor::FLOOR_CREATE;
use archbuilder_commands::modify::{CLIPBOARD_PASTE, CLIPBOARD_PASTE_TO_STOREY};
use archbuilder_commands::room::ROOM_PLACE;
use archbuilder_commands::structure::{OBJECT_SELECT, PROPERTY_SET};
use archbuilder_commands::wall::WALL_CREATE;
use archbuilder_io::command::{EXPORT_SHORTCUTS, IMPORT_SHORTCUTS};
use archbuilder_model::spatial;
use archbuilder_model::types::{FLOOR, ROOM, STOREY, WALL};

use super::{GRID, WallInteraction, export_base, insert_command};
use crate::shortcuts::{self, Table, ui};
use crate::visibility::{self, Lists, Visibility};
use crate::{menu, selection};

/// The setting that switches snapping on and off.
pub const SNAP_SETTING: &str = "archbuilder.snap.enabled";
/// The setting of the fine grid step, in millimeters.
pub const GRID_SETTING: &str = "archbuilder.snap.grid_mm";

/// A message of the catalog, named by identifier.
struct Message {
    message_id: &'static str,
}

const BANNER_HIDDEN: Message = Message {
    message_id: "ab-banner-hidden",
};
const BANNER_ISOLATED: Message = Message {
    message_id: "ab-banner-isolated",
};
const BANNER_RESET: Message = Message {
    message_id: "ab-banner-reset",
};
const COPIED: Message = Message {
    message_id: "ab-notice-copied",
};
const PASTED_STOREY: Message = Message {
    message_id: "ab-notice-pasted-storey",
};
const SELECTED_ID: Message = Message {
    message_id: "ab-notice-selected-id",
};
const SHORTCUTS_EXPORTED: Message = Message {
    message_id: "ab-notice-shortcuts-exported",
};
const SHORTCUTS_IMPORTED: Message = Message {
    message_id: "ab-notice-shortcuts-imported",
};
const SNAP_NOTICE: Message = Message {
    message_id: "ab-notice-snap",
};
const STATE_ON: Message = Message {
    message_id: "ab-state-on",
};
const STATE_OFF: Message = Message {
    message_id: "ab-state-off",
};
const RENAME: Message = Message {
    message_id: "ab-rename-room",
};

/// A room being renamed in the dynamic input.
#[derive(Debug, Clone)]
pub(super) struct Rename {
    room: ObjectId,
    text: String,
}

/// The state of the habits of one tab.
pub(super) struct Habits {
    /// The command Repeat starts again.
    pub(super) last: Option<String>,
    memory: selection::Memory,
    table: Option<(String, Table)>,
    /// The command names the shortcuts must not cut short (see [`shortcuts::command_names`]).
    names: Option<(String, BTreeSet<String>)>,
    pub(super) vis: Visibility,
    pub(super) lists: Lists,
    /// The last pointer event over the viewport, for Tab.
    pub(super) pointer: Option<PointerEvent>,
    pub(super) rename: Option<Rename>,
    pub(super) banner: Option<String>,
    view: Option<ViewRequest>,
    /// Whether snapping to points is on.
    pub(super) snap_on: bool,
    /// The fine grid step, in meters.
    pub(super) grid: f64,
    /// Set when a typed shortcut fired: an empty Enter that is the very next input confirms the
    /// shortcut instead of answering the prompt it started (see `EnterMark`).
    pub(super) swallow_enter: Option<EnterMark>,
}

/// The state of an Enter that may only confirm a typed shortcut.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum EnterMark {
    /// The shortcut fired; its command has not started yet.
    Armed,
    /// The command started and waits at the prompt with this key, the view as it was then.
    Waiting {
        prompt: String,
        camera: arc_geom::view::ViewFrame,
    },
}

impl Default for Habits {
    fn default() -> Self {
        Self {
            last: None,
            memory: selection::Memory::default(),
            table: None,
            names: None,
            vis: Visibility::default(),
            lists: Lists::default(),
            pointer: None,
            rename: None,
            banner: None,
            view: None,
            snap_on: true,
            grid: GRID,
            swallow_enter: None,
        }
    }
}

impl Habits {
    /// The view change asked for since the last call, taken once.
    pub(super) fn take_view(&mut self) -> Option<ViewRequest> {
        self.view.take()
    }

    /// The field of the room rename, when one is open.
    pub(super) fn rename_field(&self) -> Option<DynamicField> {
        self.rename.as_ref().map(|r| DynamicField {
            label: RENAME.message_id.to_owned(),
            text: r.text.clone(),
            locked: true,
            focused: true,
            select_all: true,
            escape_cancels: true,
        })
    }
}

/// The prompt and label of the room rename.
pub(super) fn rename_label() -> &'static str {
    RENAME.message_id
}

/// The command a typed word names that is not a command of the tools, with its arguments.
pub(super) fn typed_word(text: &str) -> Option<Intent> {
    let mut words = text.split_whitespace();
    let first = words.next()?.to_lowercase();
    let rest: Vec<String> = words.map(str::to_owned).collect();
    Some(match first.as_str() {
        "selectid" | "selid" if !rest.is_empty() => {
            Intent::new(ui::SELECT_ID).with("objects", rest)
        }
        "selectall" if rest.is_empty() => Intent::new(ui::SELECT_ALL),
        "paste" if rest.is_empty() => Intent::new(CLIPBOARD_PASTE),
        "pastestorey" if rest.is_empty() => Intent::new(CLIPBOARD_PASTE_TO_STOREY),
        "hide" if rest.is_empty() => Intent::new(ui::HIDE),
        "isolate" if rest.is_empty() => Intent::new(ui::ISOLATE),
        "unhide" if rest.is_empty() => Intent::new(ui::RESET_VISIBILITY),
        "snap" if rest.is_empty() => Intent::new(ui::SNAP),
        "repeat" if rest.is_empty() => Intent::new(ui::REPEAT),
        "cut" if rest.is_empty() => Intent::new(ui::CUT),
        "createsimilar" | "benzerini" if rest.is_empty() => Intent::new(ui::CREATE_SIMILAR),
        "selecttype" | "tipisec" if rest.is_empty() => Intent::new(ui::SELECT_TYPE),
        "matchtype" | "tipesle" if rest.is_empty() => Intent::new(ui::MATCH_TYPE),
        "edittype" | "tipduzenle" if rest.is_empty() => Intent::new(ui::EDIT_TYPE),
        "editboundary" | "sinirduzenle" if rest.is_empty() => {
            Intent::new(archbuilder_commands::floor::FLOOR_EDIT_BOUNDARY)
        }
        "exportkeys" if rest.len() == 1 => {
            Intent::new(EXPORT_SHORTCUTS).with("path", rest[0].clone())
        }
        "importkeys" if rest.len() == 1 => {
            Intent::new(IMPORT_SHORTCUTS).with("path", rest[0].clone())
        }
        _ => return None,
    })
}

/// The intent that selects `ids` in the tab's selection.
pub(super) fn select_intent(ids: &[ObjectId], mode: &str) -> Intent {
    Intent::new(OBJECT_SELECT)
        .with(
            "objects",
            ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
        )
        .with("mode", mode)
        .with("gesture", "command")
}

/// Whether the command is one Repeat starts again.
fn repeatable(command: &str) -> bool {
    use archbuilder_commands::edit;
    use archbuilder_commands::insert::{DOOR_PLACE, WINDOW_PLACE};
    use archbuilder_commands::modify::{MODIFY_COPY, MODIFY_MIRROR, MODIFY_MOVE, MODIFY_ROTATE};
    use archbuilder_commands::wall::OBJECT_DELETE;
    [
        edit::MODIFY_ALIGN,
        edit::MODIFY_OFFSET,
        edit::MODIFY_TRIM,
        edit::MODIFY_TRIM_TO,
        edit::MODIFY_TRIM_MANY,
        edit::WALL_SPLIT,
        edit::WALL_MERGE,
        edit::MODIFY_ARRAY,
        edit::MODIFY_ARRAY_RADIAL,
        WALL_CREATE,
        DOOR_PLACE,
        WINDOW_PLACE,
        FLOOR_CREATE,
        ROOM_PLACE,
        archbuilder_commands::stair::STAIR_CREATE,
        archbuilder_commands::roof::ROOF_CREATE,
        MODIFY_MOVE,
        MODIFY_COPY,
        MODIFY_ROTATE,
        MODIFY_MIRROR,
        OBJECT_DELETE,
        CLIPBOARD_PASTE,
    ]
    .contains(&command)
}

impl WallInteraction {
    /// Whether an empty Enter only confirms the typed shortcut `mark` remembers: no input and
    /// no view change came between, the command still waits at the prompt it started with, and
    /// Enter does not answer that prompt (a default, an optional text, a selection to take, a
    /// paste in place).
    pub(super) fn confirms_shortcut(
        &self,
        cx: &InteractionContext<'_>,
        mark: Option<&EnterMark>,
    ) -> bool {
        let Some(EnterMark::Waiting { prompt, camera }) = mark else {
            return false;
        };
        let out = self.output();
        let Some(now) = out.prompt else {
            // An instant command: nothing runs now. Its own view change (zoom extents) does not
            // count as something between.
            return prompt.is_empty() && !self.line_active();
        };
        let enter_answers = now.default.is_some()
            || now
                .accepts
                .iter()
                .any(|a| matches!(a, arc_command::prompt::Accept::Text { min_chars: 0, .. }))
            || self.modify.gathering()
            || self.modify.pastes();
        cx.camera() == camera && now.key == *prompt && out.status.answers == 0 && !enter_answers
    }

    /// Whether a command or an input is running, so that typing belongs to it.
    fn busy(&self) -> bool {
        self.tool_running()
            || self.modify.running()
            || self.direct.active()
            || self.habits.rename.is_some()
            || self.sketching.is_some()
    }

    /// The command that is running, for Repeat.
    fn running_command(&self) -> Option<&'static str> {
        if self.modify.running() {
            self.modify.current().map(|r| r.command)
        } else if self.rooms.running() {
            Some(ROOM_PLACE)
        } else if self.stairs.running() {
            Some(archbuilder_commands::stair::STAIR_CREATE)
        } else if self.floors.running() {
            Some(if self.floors.state().is_some_and(|s| s.roof) {
                archbuilder_commands::roof::ROOF_CREATE
            } else {
                FLOOR_CREATE
            })
        } else if let Some(state) = self.inserts.state() {
            Some(insert_command(state.kind))
        } else if self.tool.active() {
            Some(WALL_CREATE)
        } else {
            None
        }
    }

    /// Remembers the running command as the one Repeat starts.
    pub(super) fn note_last(&mut self) {
        if let Some(command) = self.running_command() {
            self.habits.last = Some(command.to_owned());
        }
    }

    /// Notes a command started by an intent as the one to repeat, when it is repeatable.
    pub(super) fn note_intent(&mut self, intent: &Intent) {
        if intent.params.is_empty() && repeatable(&intent.command) {
            self.habits.last = Some(intent.command.clone());
        }
    }

    /// Says `id` in the history, in the interface language.
    pub(super) fn say(
        &mut self,
        cx: &InteractionContext<'_>,
        id: &str,
        args: &[(&str, arc_base::error::Value)],
    ) {
        if let Some(l) = self.localizer(cx.language()) {
            let text = l.format(id, args).text;
            self.push_notice(text);
        }
    }

    fn note_result(&mut self, cx: &InteractionContext<'_>, result: Result<()>) {
        if let Err(error) = result {
            self.notice_error(cx, &error);
        }
    }

    /// The step of every frame: the selection memory, the settings the habits read, and the lists
    /// of what is hidden.
    pub(super) fn habit_frame(&mut self, cx: &InteractionContext<'_>) {
        self.habits.memory.observe(cx.selection());
        self.habits.snap_on = !matches!(cx.setting(SNAP_SETTING), Some(SettingValue::Bool(false)));
        #[allow(clippy::cast_precision_loss, reason = "a step of at most 1000 mm")]
        let grid = match cx.setting(GRID_SETTING) {
            Some(SettingValue::Int(mm)) if mm > 0 => mm as f64 / 1000.0,
            _ => GRID,
        };
        self.habits.grid = grid;
        let Ok(document) = cx.document() else {
            return;
        };
        let snapshot = document.snapshot();
        self.habits.lists = self.habits.vis.lists(&snapshot).clone();
        // In a view from above the roofs of the storey are drawn as their plan symbol, so the
        // walls the plan cuts stay in view.
        let rooms = crate::room_view::shared();
        if rooms.view().is_some()
            && let Some(storey) = rooms.active()
        {
            self.habits.lists.hidden.extend(
                spatial::children(&snapshot, archbuilder_model::types::ROOF, storey)
                    .iter()
                    .map(|o| o.id),
            );
        }
        let hidden_rooms: Vec<ObjectId> = if self.habits.vis.active() {
            // The stairs' plan symbols are drawn with the rooms and hide with them.
            spatial::objects(&snapshot, ROOM)
                .iter()
                .chain(spatial::objects(&snapshot, archbuilder_model::types::STAIR).iter())
                .chain(spatial::objects(&snapshot, archbuilder_model::types::ROOF).iter())
                .map(|o| o.id)
                .filter(|id| !self.habits.vis.shows(&snapshot, *id))
                .collect()
        } else {
            Vec::new()
        };
        crate::room_view::shared().set_hidden(hidden_rooms);
        self.habits.banner = self.banner(cx);
    }

    /// The text of the banner while something is hidden or isolated.
    fn banner(&mut self, cx: &InteractionContext<'_>) -> Option<String> {
        if !self.habits.vis.active() {
            return None;
        }
        let (hidden, isolated) = (
            self.habits.lists.hidden.len(),
            self.habits.lists.isolated.as_ref().map(Vec::len),
        );
        let count = |n: usize| {
            (
                "count",
                arc_base::error::Value::Int(i64::try_from(n).unwrap_or(0)),
            )
        };
        let localizer = self.localizer(cx.language())?;
        let mut parts: Vec<String> = Vec::new();
        if hidden > 0 {
            parts.push(
                localizer
                    .format(BANNER_HIDDEN.message_id, &[count(hidden)])
                    .text,
            );
        }
        if let Some(n) = isolated {
            parts.push(
                localizer
                    .format(BANNER_ISOLATED.message_id, &[count(n)])
                    .text,
            );
        }
        parts.push(localizer.format(BANNER_RESET.message_id, &[]).text);
        Some(parts.join("  |  "))
    }

    /// The centers of the rooms of `storey`, for the snap; none while snapping is off.
    pub(super) fn room_centers(
        &mut self,
        cx: &InteractionContext<'_>,
        storey: ObjectId,
    ) -> Vec<[f64; 2]> {
        if !self.habits.snap_on {
            return Vec::new();
        }
        let Ok(document) = cx.document() else {
            return Vec::new();
        };
        self.picker.centers(&document.snapshot(), storey)
    }

    /// The snap input of an event, with the grid and the switch of the habits.
    pub(super) fn snap_input<'a>(
        &self,
        pointer: [f64; 2],
        event: &PointerEvent,
        ends: &'a [[f64; 2]],
        walls: &'a [[[f64; 2]; 2]],
        from: Option<[f64; 2]>,
        centers: &'a [[f64; 2]],
    ) -> crate::snap::Input<'a> {
        let mut input = crate::snap::Input::new(pointer, event.pixels_per_unit, ends, walls, from);
        input.centers = centers;
        input.grid = self.habits.grid;
        input.object_snaps = self.habits.snap_on;
        input
    }

    // ----- keys ----------------------------------------------------------------------------

    /// A key or typed text over the viewport.
    pub(super) fn habit_key(&mut self, cx: &InteractionContext<'_>, input: &KeyInput) -> KeyReply {
        let command = input.modifiers.command && !input.modifiers.alt;
        match (input.key, input.text.as_deref()) {
            (Some(Key::Tab), _) if !self.busy() => self.tab_step(cx),
            (Some(Key::C), _) if command => {
                let r = self.copy_to_clipboard(cx);
                self.note_result(cx, r);
                KeyReply::Consumed
            }
            (Some(Key::X), _) if command => {
                let r = self.cut(cx);
                self.note_result(cx, r);
                KeyReply::Consumed
            }
            (Some(Key::V), _) if command => {
                let intent = Intent::new(if input.modifiers.shift {
                    CLIPBOARD_PASTE_TO_STOREY
                } else {
                    CLIPBOARD_PASTE
                });
                let r = self.intent(cx, &intent);
                self.note_result(cx, r);
                KeyReply::Consumed
            }
            (Some(Key::A), _) if command && !self.busy() => {
                let r = self.intent(cx, &Intent::new(ui::SELECT_ALL));
                self.note_result(cx, r);
                KeyReply::Consumed
            }
            (None, Some(" ")) => {
                // Space is Enter while a command runs, and repeats the last command otherwise.
                let r = if self.busy() && self.habits.rename.is_none() {
                    self.line(cx, LineAction::Submit(String::new()))
                } else if self.habits.rename.is_none() {
                    self.repeat(cx)
                } else {
                    return KeyReply::Pass;
                };
                self.note_result(cx, r);
                KeyReply::Consumed
            }
            _ => KeyReply::Pass,
        }
    }

    /// The shortcut table of the setting, made again only when the setting changed.
    fn table(&mut self, cx: &InteractionContext<'_>) -> &Table {
        let setting = match cx.setting(shortcuts::SETTING) {
            Some(SettingValue::Text(t)) => t,
            _ => String::new(),
        };
        if self
            .habits
            .table
            .as_ref()
            .is_none_or(|(text, _)| *text != setting)
        {
            let table = Table::with_overrides(&setting);
            self.habits.table = Some((setting, table));
        }
        &self
            .habits
            .table
            .get_or_insert_with(|| (String::new(), Table::default()))
            .1
    }

    /// The command a typed line names when it is exactly a shortcut, while nothing runs.
    pub(super) fn shortcut_for(
        &mut self,
        cx: &InteractionContext<'_>,
        text: &str,
    ) -> Option<String> {
        if self.busy() {
            return None;
        }
        self.table(cx).command(text.trim()).map(str::to_owned)
    }

    /// The text of the command line, offered each frame: a complete shortcut starts at once, and
    /// a bare command word followed by Space is carried out like Enter.
    pub(super) fn habit_input(
        &mut self,
        cx: &InteractionContext<'_>,
        text: &str,
    ) -> Option<LineAction> {
        if self.busy() {
            return None;
        }
        self.table(cx);
        self.names(cx);
        let table = &self.habits.table.as_ref()?.1;
        let names = &self.habits.names.as_ref()?.1;
        if let Some(command) = shortcuts::immediate(table, names, text) {
            self.habits.swallow_enter = Some(EnterMark::Armed);
            return Some(LineAction::Invoke(Intent::new(command)));
        }
        let word = text.strip_suffix(' ')?.trim();
        if word.is_empty() || word.contains(' ') {
            return None;
        }
        let lower = word.to_lowercase();
        if shortcuts::ARGUMENT_NAMES.contains(&lower.as_str()) {
            return None;
        }
        if let Some(command) = table.command(&lower) {
            return Some(LineAction::Invoke(Intent::new(command)));
        }
        names
            .contains(&lower)
            .then(|| LineAction::Submit(word.to_owned()))
    }

    /// The command names of the product and the shell, made again when the language or the table
    /// changes.
    fn names(&mut self, cx: &InteractionContext<'_>) {
        let key = format!(
            "{}|{}",
            cx.language(),
            self.habits.table.as_ref().map_or("", |(t, _)| t.as_str())
        );
        if self.habits.names.as_ref().is_some_and(|(k, _)| *k == key) {
            return;
        }
        let mut shell = arc_command::Registry::new();
        let _ = arc_app::commands::register(&mut shell);
        let english = crate::localizer(&["en-US".to_owned()]).ok();
        let turkish = crate::localizer(&["tr-TR".to_owned()]).ok();
        let localizers: Vec<&arc_app::locale::Localizer> =
            english.iter().chain(turkish.iter()).collect();
        let table = self
            .habits
            .table
            .as_ref()
            .map_or_else(Table::default, |(_, t)| t.clone());
        let registries: Vec<&arc_command::Registry> = self
            .registry
            .iter()
            .chain(std::iter::once(&shell))
            .collect();
        let names = shortcuts::command_names(&registries, &localizers, &table);
        self.habits.names = Some((key, names));
    }

    /// Tab: the hover and the next click move to the next candidate under the pointer.
    fn tab_step(&mut self, cx: &InteractionContext<'_>) -> KeyReply {
        let Some(mut event) = self.habits.pointer else {
            return KeyReply::Pass;
        };
        if !self.picker.step() {
            return KeyReply::Pass;
        }
        event.kind = PointerKind::Move;
        event.button = None;
        self.pick_idle(cx, &event);
        KeyReply::Consumed
    }

    // ----- the context menu ------------------------------------------------------------------

    /// The entries of the context menu for the state the tab is in.
    pub(super) fn habit_menu(&self, cx: &InteractionContext<'_>) -> Vec<ContextEntry> {
        let running = self.tool_running() || self.modify.running();
        let snapshot = cx.document().ok().map(arc_doc::Document::snapshot);
        let kinds = snapshot
            .as_ref()
            .map(|s| visibility::kinds_of(s, cx.selection()))
            .unwrap_or_default();
        let can_back = self
            .current_line()
            .active()
            .is_some_and(|s| !s.answers().is_empty())
            || self.tool.chain().is_some_and(|c| c.walls > 0);
        menu::entries(&menu::State {
            running,
            can_back,
            selected: cx.selection().len(),
            kinds,
            clipboard: crate::clipboard::get().is_some(),
            has_last: self.habits.last.is_some(),
            temporary: self.habits.vis.active(),
            pinned: snapshot.as_ref().map_or((false, false), |s| {
                let flags = cx.selection().iter().filter_map(|id| {
                    let key = spatial::type_of(s, *id)?;
                    let object = spatial::get(s, *id)?;
                    archbuilder_model::types::pinned_slot(&key)
                        .map(|_| archbuilder_model::types::is_pinned(&key, &object.values))
                });
                flags.fold((false, false), |(p, u), f| (p || f, u || !f))
            }),
        })
    }

    // ----- double click ----------------------------------------------------------------------

    /// A double click: on a wall, the walls joined to it; on a floor, its hole flow; on a room, an
    /// inline rename.
    pub(super) fn double_click(&mut self, cx: &InteractionContext<'_>, event: &PointerEvent) {
        if self.busy() || cx.camera().forward().z >= -0.98 {
            return;
        }
        let (Some(id), Ok(document)) = (self.picker.last_picked(event.pixel), cx.document()) else {
            return;
        };
        let snapshot = document.snapshot();
        match spatial::type_of(&snapshot, id).as_deref() {
            Some(WALL) => {
                let chain = selection::wall_chain(&snapshot, id, &self.habits.vis);
                cx.ask(select_intent(&chain, "replace"));
            }
            Some(FLOOR) => {
                // A double click on a floor opens its boundary sketch.
                let r = self.start_sketch(cx, Some(id));
                self.note_result(cx, r);
            }
            Some(ROOM) => {
                let name = spatial::get(&snapshot, id)
                    .and_then(|o| archbuilder_model::room::Room::from_values(&o.values))
                    .map(|r| r.name)
                    .unwrap_or_default();
                cx.ask(select_intent(&[id], "replace"));
                self.habits.rename = Some(Rename {
                    room: id,
                    text: name,
                });
            }
            _ => {}
        }
    }

    /// An edit of the room rename; true when it was for the rename.
    pub(super) fn rename_event(
        &mut self,
        cx: &InteractionContext<'_>,
        event: &DynamicEvent,
    ) -> bool {
        let Some(rename) = self.habits.rename.as_mut() else {
            return false;
        };
        match event {
            DynamicEvent::Edited { text, .. } => rename.text.clone_from(text),
            DynamicEvent::Cancel { .. } => self.habits.rename = None,
            DynamicEvent::Enter { .. } => {
                let (room, name) = (rename.room, rename.text.trim().to_owned());
                self.habits.rename = None;
                if !name.is_empty() {
                    let mut invocation = Invocation::new(
                        PROPERTY_SET,
                        serde_json::json!({"objects": [room.to_string()], "field": "name", "value": name}),
                    );
                    invocation.seed = self.next_seed();
                    let r = cx.invoke(invocation).map(drop);
                    self.note_result(cx, r);
                }
            }
            DynamicEvent::Tab { .. } => {}
        }
        true
    }

    // ----- commands that are not documents' -------------------------------------------------------

    /// Repeats the last command (the wall tool when there is none).
    pub(super) fn repeat(&mut self, cx: &InteractionContext<'_>) -> Result<()> {
        let command = self
            .habits
            .last
            .clone()
            .unwrap_or_else(|| WALL_CREATE.to_owned());
        self.intent(cx, &Intent::new(command))
    }

    /// Copies the selection (with the doors and windows its walls host) to the clipboard.
    fn copy_to_clipboard(&mut self, cx: &InteractionContext<'_>) -> Result<()> {
        let snapshot = cx.document()?.snapshot();
        let objects: Vec<ObjectId> = cx
            .selection()
            .iter()
            .copied()
            .filter(|id| {
                spatial::type_of(&snapshot, *id)
                    .is_some_and(|k| archbuilder_model::types::is_element(&k))
                    && self.habits.vis.shows(&snapshot, *id)
            })
            .collect();
        let Some(base) = selection::base_point(&snapshot, &objects) else {
            return Err(Error::new(codes::NOTHING_SELECTED));
        };
        let content = archbuilder_commands::clip::capture(&snapshot, &objects)?;
        let count = content
            .get("items")
            .and_then(serde_json::Value::as_array)
            .map_or(0, Vec::len);
        let count = i64::try_from(count).unwrap_or(0);
        crate::clipboard::set(content, base);
        // The paste shortcut reaches the application while the system clipboard holds something.
        let noun = if count == 1 { "object" } else { "objects" };
        cx.ask_clipboard_text(&format!("ArchBuilder: {count} {noun}"));
        self.say(
            cx,
            COPIED.message_id,
            &[("count", arc_base::error::Value::Int(count))],
        );
        Ok(())
    }

    /// The clipboard content and its base point.
    fn clip_content(&self) -> Result<(serde_json::Value, [f64; 2])> {
        let clip = crate::clipboard::get().ok_or_else(|| Error::new(codes::CLIPBOARD_EMPTY))?;
        Ok(clip)
    }

    /// Paste: asks for the insertion point with the copy's base at the pointer; Enter or Space
    /// pastes in place. The copy goes into the storey the selection is in, else the lowest.
    fn paste(&mut self, cx: &InteractionContext<'_>) -> Result<()> {
        let (content, base) = self.clip_content()?;
        let snapshot = cx.document()?.snapshot();
        let (storey, _) = crate::wall_tool::storey_for(&snapshot, cx.selection())
            .ok_or_else(|| Error::new(codes::STOREY_NOT_SELECTED))?;
        self.stop_all();
        self.with_modify(cx, |tool, env, run| {
            tool.begin_paste(env, run, content, base, storey)
        })
    }

    /// Paste to storey: into the selected storey, at the same plan position.
    fn paste_to_storey(&mut self, cx: &InteractionContext<'_>) -> Result<()> {
        let (content, _) = self.clip_content()?;
        let snapshot = cx.document()?.snapshot();
        let storey = cx
            .selection()
            .iter()
            .copied()
            .find(|id| spatial::type_of(&snapshot, *id).as_deref() == Some(STOREY))
            .ok_or_else(|| Error::new(codes::STOREY_NOT_SELECTED))?;
        self.stop_all();
        let mut invocation = Invocation::new(
            CLIPBOARD_PASTE_TO_STOREY,
            serde_json::json!({"content": content, "storey": storey.to_string()}),
        );
        invocation.seed = self.next_seed();
        cx.invoke(invocation)?;
        self.say(cx, PASTED_STOREY.message_id, &[]);
        Ok(())
    }

    /// Cut: copies the selection and deletes it, one undo step.
    fn cut(&mut self, cx: &InteractionContext<'_>) -> Result<()> {
        self.copy_to_clipboard(cx)?;
        self.intent(cx, &Intent::new(archbuilder_commands::wall::OBJECT_DELETE))
    }

    /// Undo while a command runs steps back inside it, and cancels it at its first prompt; the
    /// document is undone only when nothing runs.
    pub(super) fn undo(&mut self, cx: &InteractionContext<'_>) -> Result<()> {
        let running = self.tool_running() || self.modify.running();
        if !running {
            cx.ask(Intent::new(arc_app::commands::EDIT_UNDO));
            return Ok(());
        }
        let answered = self
            .current_line()
            .active()
            .is_some_and(|s| !s.answers().is_empty())
            || self.tool.chain().is_some_and(|c| c.walls > 0);
        if answered {
            self.line(cx, LineAction::Back)
        } else {
            self.line(cx, LineAction::Cancel)
        }
    }

    /// The objects of the selection that hide, isolate and copy work on.
    fn hideable(&self, cx: &InteractionContext<'_>) -> Result<(arc_doc::Snapshot, Vec<ObjectId>)> {
        let snapshot = cx.document()?.snapshot();
        let ids: Vec<ObjectId> = cx
            .selection()
            .iter()
            .copied()
            .filter(|id| !visibility::kinds_of(&snapshot, &[*id]).is_empty())
            .collect();
        if ids.is_empty() {
            return Err(Error::new(codes::NOTHING_SELECTED));
        }
        Ok((snapshot, ids))
    }

    /// Switches snapping to `on`, or to the opposite of now.
    fn toggle_snap(&mut self, cx: &InteractionContext<'_>, on: Option<bool>) {
        let on = on.unwrap_or(!self.habits.snap_on);
        self.habits.snap_on = on;
        cx.ask_setting(SNAP_SETTING, Some(SettingValue::Bool(on)));
        let state = if on {
            STATE_ON.message_id
        } else {
            STATE_OFF.message_id
        };
        let state = self
            .localizer(cx.language())
            .map_or_else(|| state.to_owned(), |l| l.format(state, &[]).text);
        self.say(cx, SNAP_NOTICE.message_id, &[("state", state.into())]);
    }

    /// The interface commands: `None` when `intent` is not one of them.
    #[allow(clippy::too_many_lines, reason = "one arm per interface command")]
    pub(super) fn ui_intent(
        &mut self,
        cx: &InteractionContext<'_>,
        intent: &Intent,
    ) -> Option<Result<()>> {
        if let Some(result) = self.type_intent(cx, intent) {
            return Some(result);
        }
        let command = intent.command.as_str();
        let plain = intent.params.is_empty();
        let result = match command {
            ui::REPEAT => self.repeat(cx),
            ui::COPY => self.copy_to_clipboard(cx),
            ui::CUT => self.cut(cx),
            arc_app::commands::EDIT_UNDO => self.undo(cx),
            CLIPBOARD_PASTE if plain => self.paste(cx),
            CLIPBOARD_PASTE_TO_STOREY if plain => self.paste_to_storey(cx),
            ui::SELECT_ALL => cx.document().map(|d| {
                let ids = selection::all(&d.snapshot(), &self.habits.vis);
                cx.ask(select_intent(&ids, "replace"));
            }),
            ui::SELECT_TYPE => self.hideable(cx).map(|(snapshot, ids)| {
                let all = selection::all_of_types_in(&snapshot, &ids, &self.habits.vis);
                cx.ask(select_intent(&all, "replace"));
            }),
            ui::SELECT_KIND => self.hideable(cx).map(|(snapshot, ids)| {
                let all = selection::all_of_kinds_in(&snapshot, &ids, &self.habits.vis);
                cx.ask(select_intent(&all, "replace"));
            }),
            ui::SELECT_ONLY => {
                let kind = intent
                    .params
                    .get("kind")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                cx.document().map(|d| {
                    let ids = selection::only(&d.snapshot(), cx.selection(), kind);
                    cx.ask(select_intent(&ids, "replace"));
                })
            }
            ui::SELECT_PREVIOUS => cx.document().map(|d| {
                let ids = self.habits.memory.previous(&d.snapshot());
                if !ids.is_empty() {
                    cx.ask(select_intent(&ids, "replace"));
                }
            }),
            ui::SELECT_ID => self.select_by_id(cx, intent),
            ui::HIDE => self.hideable(cx).map(|(snapshot, ids)| {
                self.habits.vis.hide(&snapshot, &ids);
                cx.ask(Intent::new(arc_app::commands::SELECT_CLEAR));
            }),
            ui::ISOLATE => self.hideable(cx).map(|(snapshot, ids)| {
                self.habits.vis.isolate(&snapshot, &ids);
            }),
            ui::HIDE_KIND => self.hideable(cx).map(|(snapshot, ids)| {
                self.habits
                    .vis
                    .hide_kinds(&visibility::kinds_of(&snapshot, &ids));
                cx.ask(Intent::new(arc_app::commands::SELECT_CLEAR));
            }),
            ui::ISOLATE_KIND => self.hideable(cx).map(|(snapshot, ids)| {
                self.habits
                    .vis
                    .isolate_kinds(&visibility::kinds_of(&snapshot, &ids));
            }),
            ui::RESET_VISIBILITY => {
                self.habits.vis.reset();
                Ok(())
            }
            ui::ZOOM_EXTENTS => {
                self.habits.view = Some(ViewRequest::Extents);
                Ok(())
            }
            ui::SNAP => {
                let on = intent.params.get("on").and_then(serde_json::Value::as_bool);
                self.toggle_snap(cx, on);
                Ok(())
            }
            menu::PROPERTIES => {
                cx.ask_panel(crate::PROPERTIES);
                Ok(())
            }
            ui::WARNINGS => {
                cx.ask_panel(crate::WARNINGS);
                Ok(())
            }
            ui::ZOOM_TO => cx.document().map(|d| {
                let snapshot = d.snapshot();
                let ids: Vec<ObjectId> = intent
                    .params
                    .get("objects")
                    .and_then(serde_json::Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(serde_json::Value::as_str)
                    .filter_map(|t| t.parse::<ObjectId>().ok())
                    .filter(|id| spatial::get(&snapshot, *id).is_some())
                    .take(selection::MAX_SELECTED)
                    .collect();
                if !ids.is_empty() {
                    cx.ask(select_intent(&ids, "replace"));
                }
                if let Some((a, b)) = crate::warnings_panel::bounds(&snapshot, &ids) {
                    self.habits.view = Some(ViewRequest::Window { a, b });
                }
            }),
            EXPORT_SHORTCUTS => self.export_shortcuts(cx, intent),
            IMPORT_SHORTCUTS => self.import_shortcuts(cx, intent),
            _ => return None,
        };
        Some(result)
    }

    fn select_by_id(&mut self, cx: &InteractionContext<'_>, intent: &Intent) -> Result<()> {
        let snapshot = cx.document()?.snapshot();
        let mut ids: Vec<ObjectId> = Vec::new();
        for text in intent
            .params
            .get("objects")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .take(selection::MAX_SELECTED)
        {
            let found = text
                .parse::<ObjectId>()
                .ok()
                .filter(|id| spatial::get(&snapshot, *id).is_some());
            match found {
                Some(id) => ids.push(id),
                None => return Err(Error::new(codes::OBJECT_NOT_FOUND).with_arg("id", text)),
            }
        }
        if ids.is_empty() {
            return Err(Error::new(codes::NOTHING_SELECTED));
        }
        let count = i64::try_from(ids.len()).unwrap_or(0);
        cx.ask(select_intent(&ids, "replace"));
        self.say(
            cx,
            SELECTED_ID.message_id,
            &[("count", arc_base::error::Value::Int(count))],
        );
        Ok(())
    }

    /// Runs the file command `command` on the file `path` names (a relative name is taken in the
    /// document's folder, as an export is) with a session that reaches that folder only.
    fn run_file_command(
        &mut self,
        cx: &InteractionContext<'_>,
        command: &str,
        intent: &Intent,
        extra: Option<(&str, String)>,
    ) -> Result<(String, arc_command::Outcome)> {
        let path = intent
            .params
            .get("path")
            .and_then(serde_json::Value::as_str)
            .filter(|p| !p.trim().is_empty())
            .ok_or_else(|| Error::new(archbuilder_io::error::codes::PATH_REQUIRED))?;
        let target = std::path::absolute(export_base(cx.document_path()).join(path.trim()))
            .map_err(|e| Error::new(arc_command::error::codes::PATH_IO).with_source(e))?;
        let directory = target
            .parent()
            .map_or_else(|| target.clone(), std::path::PathBuf::from);
        let mut session = Self::session(cx);
        session.roots = Roots::new([&directory], [&directory])?;
        let mut params = serde_json::json!({ "path": target.display().to_string() });
        if let (Some((key, value)), Some(map)) = (extra, params.as_object_mut()) {
            map.insert(key.to_owned(), serde_json::Value::String(value));
        }
        let mut invocation = Invocation::new(command.to_owned(), params);
        invocation.seed = self.next_seed();
        let (Some(registry), document) = (self.registry.as_ref(), cx.document()?) else {
            return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
        };
        let outcome = Invoker::new(registry).invoke(document, &session, invocation)?;
        Ok((target.display().to_string(), outcome))
    }

    /// Writes the effective shortcut table to the file of the `path` parameter.
    fn export_shortcuts(&mut self, cx: &InteractionContext<'_>, intent: &Intent) -> Result<()> {
        let setting = match cx.setting(shortcuts::SETTING) {
            Some(SettingValue::Text(t)) => t,
            _ => String::new(),
        };
        let text = Table::with_overrides(&setting).to_json();
        let (name, _) =
            self.run_file_command(cx, EXPORT_SHORTCUTS, intent, Some(("text", text)))?;
        self.say(cx, SHORTCUTS_EXPORTED.message_id, &[("path", name.into())]);
        Ok(())
    }

    /// Reads a shortcut table from the file of the `path` parameter and makes it the table.
    fn import_shortcuts(&mut self, cx: &InteractionContext<'_>, intent: &Intent) -> Result<()> {
        let (_, outcome) = self.run_file_command(cx, IMPORT_SHORTCUTS, intent, None)?;
        let text = outcome
            .value
            .get("text")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let registry = self.registry.as_ref();
        let known = |c: &str| {
            shortcuts::defaults().iter().any(|(_, d)| *d == c)
                || registry.is_some_and(|r| r.get(c).is_some())
        };
        let table = Table::from_json(text, &known)?;
        let count = i64::try_from(table.iter().count()).unwrap_or(0);
        cx.ask_setting(
            shortcuts::SETTING,
            Some(SettingValue::Text(table.overrides())),
        );
        self.habits.table = None;
        self.say(
            cx,
            SHORTCUTS_IMPORTED.message_id,
            &[("count", arc_base::error::Value::Int(count))],
        );
        Ok(())
    }

    /// A word typed in the command line that names one of these commands.
    pub(super) fn habit_word(&self, text: &str) -> Option<Intent> {
        typed_word(text)
    }
}
