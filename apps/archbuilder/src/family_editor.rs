//! The family editor (ADR 0009, unit 12): the definition of the family of the selected component,
//! hosted instance or family type, as its canonical text, edited in place. Preview reports what a
//! commit changes (the next version, the types and instances it affects, the parameters that need
//! a mapping, the instances that would fail) without changing anything. Commit publishes the text
//! through `archbuilder.family.commit`, one undo step; cancel drops the edit. A starter text per
//! category opens a new family.

use arc_app::shell::{Action, Panel, PanelContext};
use arc_base::ObjectId;
use arc_doc::Snapshot;
use arc_ui::egui::{self, ComboBox, RichText, ScrollArea, TextEdit, Ui};
use arc_ui::{Intent, Localize};
use archbuilder_commands::family_project::{
    CommitFamily, FAMILY_COMMIT, UpdateReport, preview_commit,
};
use archbuilder_model::family::def::{Category, Definition};
use archbuilder_model::family::loaded::{Component, FamilyInsert, FamilyType, Loaded};
use archbuilder_model::family::templates::{STARTER_CATEGORIES, starter};
use archbuilder_model::spatial;
use archbuilder_model::types::{self, COMPONENT, FAMILY, FAMILY_INSERT};

/// A message of the catalog, named by identifier.
struct Message {
    message_id: &'static str,
}

const NONE: Message = Message {
    message_id: "ab-family-editor-none",
};
const UNSAVED: Message = Message {
    message_id: "ab-family-editor-unsaved",
};
const PREVIEW: Message = Message {
    message_id: "ab-family-editor-preview",
};
const COMMIT: Message = Message {
    message_id: "ab-family-editor-commit",
};
const CANCEL: Message = Message {
    message_id: "ab-family-editor-cancel",
};
const FROM_STARTER: Message = Message {
    message_id: "ab-family-editor-from-starter",
};
const NEW_FAMILY: Message = Message {
    message_id: "ab-family-editor-new",
};
const REPORT: Message = Message {
    message_id: "ab-family-editor-report",
};
const MAJOR: Message = Message {
    message_id: "ab-family-editor-major",
};
const NEEDS_MAPPING: Message = Message {
    message_id: "ab-family-editor-needs-mapping",
};
const FAILING: Message = Message {
    message_id: "ab-family-editor-failing",
};
const CHANGES: Message = Message {
    message_id: "ab-family-editor-changes",
};

fn say(
    localize: &dyn Localize,
    message: &Message,
    args: &[(&str, arc_base::error::Value)],
) -> String {
    localize
        .message(message.message_id, args)
        .unwrap_or_else(|| message.message_id.to_owned())
}

/// The name of a starter category in the catalog.
fn category_name(category: Category) -> Message {
    match category {
        Category::Door => Message {
            message_id: "ab-family-category-door",
        },
        Category::Window => Message {
            message_id: "ab-family-category-window",
        },
        Category::Furniture => Message {
            message_id: "ab-family-category-furniture",
        },
        Category::Casework => Message {
            message_id: "ab-family-category-casework",
        },
        _ => Message {
            message_id: "ab-family-category-generic",
        },
    }
}

/// The family record of the selection's first component, hosted instance or family type; the
/// family itself when it is selected.
fn family_of_selection(snapshot: &Snapshot, selection: &[ObjectId]) -> Option<ObjectId> {
    selection.iter().find_map(|id| {
        let object = spatial::get(snapshot, *id)?;
        let ty = match spatial::type_of(snapshot, *id)?.as_str() {
            COMPONENT => Component::of(&object)?.family_type,
            FAMILY_INSERT => FamilyInsert::of(&object)?.family_type,
            types::FAMILY_TYPE => return FamilyType::of(&object).map(|t| t.family),
            FAMILY => return Some(*id),
            _ => return None,
        };
        spatial::get(snapshot, ty)
            .and_then(|o| FamilyType::of(&o))
            .map(|t| t.family)
    })
}

/// The family editor panel. It keeps the text being edited; the family is the one of the
/// selection, followed while there are no unsaved changes.
#[derive(Debug)]
pub struct FamilyEditorPanel {
    /// The family record being edited; `None` for a new family from a starter.
    family: Option<ObjectId>,
    /// The text the family has in the project (empty for a new family).
    loaded: String,
    /// The text being edited.
    buffer: String,
    /// The starter category a new family takes.
    starter: Category,
    /// A commit was sent, with the version the family had then (empty for a new family); the
    /// family is looked at again on the next frame, and taken over only when its version moved.
    committing: Option<String>,
    /// The last preview: what the commit would report.
    preview: Option<UpdateReport>,
}

impl Default for FamilyEditorPanel {
    fn default() -> Self {
        Self {
            family: None,
            loaded: String::new(),
            buffer: String::new(),
            starter: Category::Door,
            committing: None,
            preview: None,
        }
    }
}

impl FamilyEditorPanel {
    fn dirty(&self) -> bool {
        self.buffer != self.loaded
    }

    /// Opens the text of `family` (the loaded family record) unless the person has unsaved edits.
    fn open(&mut self, snapshot: &Snapshot, family: ObjectId) {
        if self.family == Some(family) || self.dirty() {
            return;
        }
        if let Some(loaded) = spatial::get(snapshot, family).and_then(|o| Loaded::of(&o)) {
            self.family = Some(family);
            self.loaded.clone_from(&loaded.text);
            self.buffer = loaded.text;
            self.preview = None;
        }
    }

    /// After a commit: the family's text in the project is the buffer, or a new family with the
    /// buffer's identifier has been loaded.
    fn settle(&mut self, snapshot: &Snapshot) {
        let Some(before) = self.committing.take() else {
            return;
        };
        let Ok(definition) = Definition::from_text(&self.buffer) else {
            return;
        };
        let found = spatial::objects(snapshot, FAMILY)
            .into_iter()
            .filter_map(|o| Loaded::of(&o))
            .find(|f| f.family == definition.header.id);
        // A refused commit leaves the version as it was, and the edit stays as it is.
        if let Some(loaded) = found
            && loaded.version.to_string() != before
        {
            self.family = Some(loaded.record);
            self.loaded.clone_from(&loaded.text);
            self.buffer = loaded.text;
            self.preview = None;
        }
    }

    fn body(&mut self, ui: &mut Ui, cx: &mut PanelContext<'_>) {
        let Some(tab) = cx.tab.and_then(|t| cx.app.tab(t).ok()) else {
            return;
        };
        let snapshot = tab.document().snapshot();
        let localize = cx.localize;
        self.settle(&snapshot);
        if let Some(view) = cx.view.as_deref()
            && let Some(family) = family_of_selection(&snapshot, view.selection())
        {
            self.open(&snapshot, family);
        }
        let current = self
            .family
            .and_then(|f| spatial::get(&snapshot, f))
            .and_then(|o| Loaded::of(&o));
        match &current {
            Some(loaded) => {
                ui.label(RichText::new(format!("{} {}", loaded.name, loaded.version)).strong());
            }
            None => {
                ui.label(RichText::new(say(localize, &NEW_FAMILY, &[])).strong());
            }
        }
        if current.is_none() && !self.dirty() && self.buffer.is_empty() {
            ui.label(say(localize, &NONE, &[]));
        }
        self.starter_row(ui, localize);
        if self.dirty() {
            ui.weak(say(localize, &UNSAVED, &[]));
        }
        ScrollArea::vertical()
            .id_salt("archbuilder-family-text")
            .max_height(360.0)
            .show(ui, |ui| {
                ui.add(
                    TextEdit::multiline(&mut self.buffer)
                        .code_editor()
                        .desired_rows(18)
                        .desired_width(f32::INFINITY),
                );
            });
        self.buttons(ui, cx, &snapshot, current.as_ref(), localize);
        if let Some(report) = &self.preview {
            self.report(ui, localize, report);
        }
    }

    fn starter_row(&mut self, ui: &mut Ui, localize: &dyn Localize) {
        ui.horizontal(|ui| {
            ComboBox::from_id_salt("archbuilder-family-starter")
                .selected_text(say(localize, &category_name(self.starter), &[]))
                .show_ui(ui, |ui| {
                    for category in STARTER_CATEGORIES {
                        let label = say(localize, &category_name(category), &[]);
                        if ui
                            .selectable_label(self.starter == category, label)
                            .clicked()
                        {
                            self.starter = category;
                        }
                    }
                });
            if ui.button(say(localize, &FROM_STARTER, &[])).clicked()
                && let Some(text) = starter(self.starter)
            {
                self.family = None;
                self.loaded.clear();
                self.buffer = text.to_owned();
                self.preview = None;
            }
        });
    }

    fn buttons(
        &mut self,
        ui: &mut Ui,
        cx: &mut PanelContext<'_>,
        snapshot: &Snapshot,
        current: Option<&Loaded>,
        localize: &dyn Localize,
    ) {
        let editing = self.family.is_some() || !self.buffer.trim().is_empty();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(editing, egui::Button::new(say(localize, &PREVIEW, &[])))
                .clicked()
            {
                let request = CommitFamily {
                    family: self.family.map(|f| f.to_string()),
                    text: self.buffer.clone(),
                    mapping: Default::default(),
                    preview: true,
                };
                match preview_commit(snapshot, &request) {
                    Ok(report) => self.preview = Some(report),
                    Err(error) => {
                        self.preview = None;
                        cx.actions.push(Action::Error(error));
                    }
                }
            }
            if ui
                .add_enabled(
                    editing && self.dirty(),
                    egui::Button::new(say(localize, &COMMIT, &[])),
                )
                .clicked()
            {
                let mut intent = Intent::new(FAMILY_COMMIT).with("text", self.buffer.clone());
                if let Some(family) = self.family {
                    intent = intent.with("family", family.to_string());
                }
                cx.actions.push(Action::Intent(intent));
                self.committing = Some(current.map(|l| l.version.to_string()).unwrap_or_default());
            }
            if ui
                .add_enabled(self.dirty(), egui::Button::new(say(localize, &CANCEL, &[])))
                .clicked()
            {
                self.buffer.clone_from(&self.loaded);
                self.preview = None;
            }
        });
    }

    fn report(&self, ui: &mut Ui, localize: &dyn Localize, report: &UpdateReport) {
        let count = |n: usize| arc_base::error::Value::Int(i64::try_from(n).unwrap_or(i64::MAX));
        ui.label(say(
            localize,
            &REPORT,
            &[
                ("version", arc_base::error::Value::from(report.to.as_str())),
                ("types", count(report.types)),
                ("instances", count(report.instances)),
            ],
        ));
        if report.major {
            ui.label(say(localize, &MAJOR, &[]));
        }
        if !report.needs_mapping.is_empty() {
            ui.label(say(
                localize,
                &NEEDS_MAPPING,
                &[(
                    "parameters",
                    arc_base::error::Value::from(report.needs_mapping.join(", ")),
                )],
            ));
        }
        if !report.failing.is_empty() {
            ui.label(say(
                localize,
                &FAILING,
                &[("count", count(report.failing.len()))],
            ));
        }
        ui.label(say(
            localize,
            &CHANGES,
            &[("count", count(report.changes.len()))],
        ));
    }
}

impl Panel for FamilyEditorPanel {
    fn show(&mut self, ui: &mut Ui, cx: &mut PanelContext<'_>) {
        self.body(ui, cx);
    }
}
