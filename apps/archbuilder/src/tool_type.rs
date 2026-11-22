//! The type selector: the toolbar's choice list of the types of one kind. While the wall, floor,
//! door or window tool runs it lists that kind and picks the type the tool places next; with no
//! tool running it lists the kind of the selected walls, floors, doors or windows and retypes them
//! all at once (a mixed selection of one kind included).
//!
//! The tools and the toolbar are separate parts of the shell, so the kind of the running tool and
//! the type picked for each kind are kept here, shared by the application's tabs. A picked type
//! that the active document does not have falls back to the document's default type.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use arc_app::application::{Application, TabId};
use arc_app::shell::{ChoiceState, ToolItem};
use arc_base::ObjectId;
use arc_doc::Snapshot;
use arc_ui::Intent;
use archbuilder_model::element_type::{self as et, TypeKind};
use archbuilder_model::spatial;

use crate::shortcuts::ui;

#[derive(Debug, Default)]
struct State {
    active: Option<TypeKind>,
    chosen: BTreeMap<TypeKind, ObjectId>,
    /// What the selector shows, published by the active tab's controller each frame.
    shown: Option<(TypeKind, Option<ObjectId>)>,
}

static STATE: Mutex<State> = Mutex::new(State {
    active: None,
    chosen: BTreeMap::new(),
    shown: None,
});

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    f(&mut STATE.lock().unwrap_or_else(PoisonError::into_inner))
}

/// Says which kind of placement tool runs now, if any.
pub fn set_active(kind: Option<TypeKind>) {
    with(|s| s.active = kind);
}

/// The kind of the placement tool that runs now.
#[must_use]
pub fn active() -> Option<TypeKind> {
    with(|s| s.active)
}

/// Picks the type the tool of `kind` places next.
pub fn choose(kind: TypeKind, id: ObjectId) {
    with(|s| {
        s.chosen.insert(kind, id);
    });
}

/// The type picked for `kind`, when the document has it; else the document's default type.
#[must_use]
pub fn current(snapshot: &Snapshot, kind: TypeKind) -> Option<ObjectId> {
    with(|s| s.chosen.get(&kind).copied())
        .filter(|id| spatial::type_of(snapshot, *id).as_deref() == Some(kind.key()))
        .or_else(|| archbuilder_commands::element_type::default_of(snapshot, kind))
}

/// The type picked for `kind` as text for a command's `type` parameter.
#[must_use]
pub fn current_param(snapshot: &Snapshot, kind: TypeKind) -> Option<String> {
    current(snapshot, kind).map(|id| id.to_string())
}

/// The kind the selector lists and the type it shows as current: the running tool's kind and
/// its next type, else the kind of the first typed object of the selection and its type (none
/// when the selection's types differ).
#[must_use]
pub fn shown(snapshot: &Snapshot, selection: &[ObjectId]) -> Option<(TypeKind, Option<ObjectId>)> {
    if let Some(kind) = active() {
        return Some((kind, current(snapshot, kind)));
    }
    let typed: Vec<(TypeKind, Option<ObjectId>)> = selection
        .iter()
        .filter_map(|id| {
            let kind = TypeKind::of_instance_key(&spatial::type_of(snapshot, *id)?)?;
            let object = spatial::get(snapshot, *id)?;
            Some((kind, et::type_id(kind, &object.values)))
        })
        .collect();
    let (kind, first) = *typed.first()?;
    let same = typed
        .iter()
        .filter(|(k, _)| *k == kind)
        .all(|(_, t)| *t == first);
    Some((kind, first.filter(|_| same)))
}

/// Publishes what the selector shows for the active tab (see [`shown`]).
pub fn publish(snapshot: &Snapshot, selection: &[ObjectId]) {
    let value = shown(snapshot, selection);
    with(|s| s.shown = value);
}

fn probe(app: &Application, tab: TabId) -> ChoiceState {
    let Ok(t) = app.tab(tab) else {
        return ChoiceState::default();
    };
    let snapshot = t.document().snapshot();
    let Some((kind, current)) = with(|s| s.shown) else {
        return ChoiceState::default();
    };
    let types = et::of_kind(&snapshot, kind);
    ChoiceState {
        options: types.iter().map(|t| et::name(t).to_owned()).collect(),
        current: current
            .and_then(|id| types.iter().find(|t| t.id == id))
            .map(|t| et::name(t).to_owned()),
    }
}

/// The toolbar item of the type selector.
#[must_use]
pub fn toolbar_item() -> ToolItem {
    ToolItem::Choice {
        hint: Some(HINT.message_id.to_owned()),
        probe: Arc::new(probe),
        pick: Arc::new(|name| Intent::new(ui::TYPE_PICK).with("name", name)),
    }
}

/// A message of the application.
struct Message {
    message_id: &'static str,
}

const HINT: Message = Message {
    message_id: "ab-tool-type-hint",
};
