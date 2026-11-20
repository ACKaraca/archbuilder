//! The property grid: the fields of the selected objects with readable names and values, lengths
//! in the project's unit. A storey shows its level's elevation as its own. An edit becomes an
//! intent for `archbuilder.property.set`.

use arc_app::shell::{Action, Panel, PanelContext};
use arc_base::{Dimension, DisplayUnits};
use arc_doc::schema::FieldDecl;
use arc_doc::{Bound, Object, Snapshot, Value, ValueType};
use arc_ui::egui::{ScrollArea, Ui};
use arc_ui::{GridContext, Localize, PropertyGrid, PropertyModel, PropertyObject};
use archbuilder_commands::structure::PROPERTY_SET;
use archbuilder_model::spatial;
use archbuilder_model::types::{
    self, BUILDING, ELEVATION_RANGE, LEVEL, PROJECT, SITE, STOREY, level, storey,
};

/// A message of the application.
struct Message {
    message_id: &'static str,
}

/// The readable name of every field, enum variant and aspect the grid can show.
const LABELS: &[(&str, Message)] = &[
    (
        "name",
        Message {
            message_id: "ab-prop-name",
        },
    ),
    (
        "number",
        Message {
            message_id: "ab-prop-number",
        },
    ),
    (
        "client",
        Message {
            message_id: "ab-prop-client",
        },
    ),
    (
        "address",
        Message {
            message_id: "ab-prop-address",
        },
    ),
    (
        "length_unit",
        Message {
            message_id: "ab-prop-length-unit",
        },
    ),
    (
        "template",
        Message {
            message_id: "ab-prop-template",
        },
    ),
    (
        "elevation",
        Message {
            message_id: "ab-prop-elevation",
        },
    ),
    (
        "elevation_base",
        Message {
            message_id: "ab-prop-elevation-base",
        },
    ),
    (
        "storey",
        Message {
            message_id: "ab-prop-storey",
        },
    ),
    (
        "level",
        Message {
            message_id: "ab-prop-level",
        },
    ),
    (
        "meter",
        Message {
            message_id: "ab-unit-meter",
        },
    ),
    (
        "millimeter",
        Message {
            message_id: "ab-unit-millimeter",
        },
    ),
    (
        "centimeter",
        Message {
            message_id: "ab-unit-centimeter",
        },
    ),
    (
        "foot",
        Message {
            message_id: "ab-unit-foot",
        },
    ),
    (
        "inch",
        Message {
            message_id: "ab-unit-inch",
        },
    ),
    (
        "project",
        Message {
            message_id: "ab-base-project",
        },
    ),
    (
        "survey",
        Message {
            message_id: "ab-base-survey",
        },
    ),
    (
        "identity",
        Message {
            message_id: "ab-aspect-identity",
        },
    ),
    (
        "geometry",
        Message {
            message_id: "ab-aspect-geometry",
        },
    ),
    (
        "units",
        Message {
            message_id: "ab-aspect-units",
        },
    ),
    (
        "data",
        Message {
            message_id: "ab-aspect-data",
        },
    ),
];

/// The readable name of each kind of object, for the heading of the grid.
const KINDS: &[(&str, Message)] = &[
    (
        PROJECT,
        Message {
            message_id: "ab-kind-project",
        },
    ),
    (
        SITE,
        Message {
            message_id: "ab-kind-site",
        },
    ),
    (
        BUILDING,
        Message {
            message_id: "ab-kind-building",
        },
    ),
    (
        STOREY,
        Message {
            message_id: "ab-kind-storey",
        },
    ),
    (
        LEVEL,
        Message {
            message_id: "ab-kind-level",
        },
    ),
];

const EMPTY: Message = Message {
    message_id: "ab-prop-empty",
};
/// Why the base elevation of a building and the reference of a level's elevation are not
/// editable.
const NOT_EDITABLE: Message = Message {
    message_id: "ab-prop-not-editable",
};

fn say(localize: &dyn Localize, message: &Message) -> String {
    localize
        .message(message.message_id, &[])
        .unwrap_or_else(|| message.message_id.to_owned())
}

/// What the grid shows of one object.
struct Shown {
    key: String,
    fields: Vec<FieldDecl>,
    values: Vec<Option<Value>>,
    read_only: Vec<String>,
}

/// The elevation field of a storey, with the range of a level's elevation, so a value outside it is
/// refused at the field with the range named.
fn length_field() -> FieldDecl {
    let (min, max) = ELEVATION_RANGE;
    FieldDecl::new("elevation", ValueType::Quantity(Dimension::LENGTH))
        .with_bound(Bound::Real { min, max })
        .in_aspect("geometry")
}

/// The fields of `object` as the grid shows them. A storey's fields are its name and its level's
/// name and elevation; a level's `storey` flag is shown and not editable.
fn shown(snapshot: &Snapshot, key: &str, object: &Object) -> Option<Shown> {
    let decl = snapshot.registry().get(key)?;
    let mut fields = decl.schema.fields.clone();
    let mut values = object.values.clone();
    let mut read_only = Vec::new();
    match key {
        STOREY => {
            let level = spatial::level_of(snapshot, object);
            fields = vec![
                fields.remove(storey::NAME),
                length_field(),
                text_field("level"),
            ];
            let elevation = level
                .as_ref()
                .and_then(|l| types::meters(&l.values, level::ELEVATION));
            let level_name = level
                .as_ref()
                .and_then(|l| types::text(&l.values, level::NAME))
                .map(|n| Value::String(n.to_owned()));
            values = vec![
                values.swap_remove(storey::NAME),
                elevation.map(types::length),
                level_name,
            ];
            read_only.push("level".to_owned());
        }
        LEVEL => {
            // The flag that says whether the level has a storey is the model's bookkeeping, not
            // something to read or edit; the reference of the elevation has no effect yet.
            fields.truncate(level::STOREY);
            values.truncate(level::STOREY);
            read_only.push("elevation_base".to_owned());
        }
        // The building's base elevation has no effect yet: the levels carry the elevations.
        BUILDING => read_only.push("elevation".to_owned()),
        _ => {}
    }
    Some(Shown {
        key: key.to_owned(),
        fields,
        values,
        read_only,
    })
}

fn text_field(name: &str) -> FieldDecl {
    FieldDecl::new(name, ValueType::String)
        .optional()
        .in_aspect("data")
}

/// The property grid panel.
#[derive(Default)]
pub struct PropertiesPanel {
    grid: PropertyGrid,
}

impl Panel for PropertiesPanel {
    fn show(&mut self, ui: &mut Ui, cx: &mut PanelContext<'_>) {
        let (Some(view), Some(tab)) = (cx.view.as_deref(), cx.tab) else {
            return;
        };
        let Ok(tab) = cx.app.tab(tab) else {
            return;
        };
        let snapshot = tab.document().snapshot();
        // A selected storey stands for its level, which the grid shows through it.
        let covered: Vec<arc_base::ObjectId> = view
            .selection()
            .iter()
            .filter_map(|id| {
                spatial::get(&snapshot, *id)
                    .filter(|_| spatial::type_of(&snapshot, *id).as_deref() == Some(STOREY))
            })
            .filter_map(|storey| spatial::level_of(&snapshot, &storey))
            .map(|level| level.id)
            .collect();
        let items: Vec<(Shown, &arc_base::ObjectId)> = view
            .selection()
            .iter()
            .filter(|id| !covered.contains(id))
            .filter_map(|id| {
                let key = spatial::type_of(&snapshot, *id)?;
                let object = spatial::get(&snapshot, *id)?;
                Some((shown(&snapshot, &key, &object)?, id))
            })
            .collect();
        let Some(first) = items.first().map(|(s, _)| s) else {
            ui.label(say(cx.localize, &EMPTY));
            return;
        };
        let first_key = first.key.clone();
        let kind = KINDS
            .iter()
            .find(|(k, _)| *k == first.key)
            .map(|(_, m)| say(cx.localize, m))
            .unwrap_or_default();
        let heading = if items.len() > 1 {
            format!("{kind} ({})", items.len())
        } else {
            kind
        };
        ui.heading(heading);
        let objects: Vec<PropertyObject<'_>> = items
            .iter()
            .map(|(s, id)| PropertyObject {
                id: **id,
                fields: &s.fields,
                values: &s.values,
                read_only: &s.read_only,
                overridden: &[],
            })
            .collect();
        let model = match PropertyModel::build(&objects) {
            Ok(model) => model,
            Err(error) => {
                cx.actions.push(Action::Error(error));
                return;
            }
        };
        let mut units = DisplayUnits::new();
        if let Some(format) = spatial::length_unit(&snapshot).format() {
            units.set(format);
        }
        let localize = cx.localize;
        let label = |name: &str| {
            LABELS
                .iter()
                .find(|(n, _)| *n == name)
                .map_or_else(|| name.to_owned(), |(_, m)| say(localize, m))
        };
        let context = GridContext {
            localize,
            label: &label,
            units: &units,
            decimal_separator: crate::labels::decimal_separator(cx.language),
            edit_command: PROPERTY_SET,
        };
        let grid = &mut self.grid;
        let note =
            matches!(first_key.as_str(), BUILDING | LEVEL).then(|| say(cx.localize, &NOT_EDITABLE));
        let intent = ScrollArea::vertical()
            .id_salt("archbuilder-properties")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let intent = grid.show(ui, &model, &context);
                // Why some fields cannot be edited, under the grid and inside its scroll area.
                if let Some(note) = note {
                    ui.weak(note);
                }
                intent
            })
            .inner;
        if let Some(intent) = intent {
            cx.actions.push(Action::Intent(intent));
        }
    }
}
