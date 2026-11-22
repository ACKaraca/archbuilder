//! The property grid: the fields of the selected objects with readable names and values, lengths
//! in the project's unit. A storey shows its level's elevation as its own. A wall, floor, door or
//! window shows its type as a choice second (one choice retypes the whole selection) and the
//! values its type decides read-only. An edit becomes an
//! intent for `archbuilder.property.set`.

use arc_app::shell::{Action, Panel, PanelContext};
use arc_base::{Dimension, DisplayUnits};
use arc_doc::schema::FieldDecl;
use arc_doc::{Bound, Object, Snapshot, Value, ValueType};
use arc_ui::egui::{ScrollArea, Ui};
use arc_ui::{GridContext, Localize, PropertyGrid, PropertyModel, PropertyObject};
use archbuilder_commands::structure::PROPERTY_SET;
use archbuilder_model::element_type::{self as et, TypeKind};
use archbuilder_model::spatial;
use archbuilder_model::types::{
    self, BUILDING, DOOR, ELEVATION_RANGE, FLOOR, LEVEL, PROJECT, ROOM, SITE, STAIR, STOREY, WALL,
    WINDOW, level, storey,
};

/// A message of the application.
struct Message {
    message_id: &'static str,
}

/// The readable name of every field, enum variant and aspect the grid can show.
const LABELS: &[(&str, Message)] = &[
    (
        "type",
        Message {
            message_id: "ab-prop-type",
        },
    ),
    (
        "double_swing",
        Message {
            message_id: "ab-op-double-swing",
        },
    ),
    (
        "sliding",
        Message {
            message_id: "ab-op-sliding",
        },
    ),
    (
        "core_center",
        Message {
            message_id: "ab-loc-core-center",
        },
    ),
    (
        "core_exterior",
        Message {
            message_id: "ab-loc-core-exterior",
        },
    ),
    (
        "core_interior",
        Message {
            message_id: "ab-loc-core-interior",
        },
    ),
    (
        "shape",
        Message {
            message_id: "ab-prop-stair-shape",
        },
    ),
    (
        "turn",
        Message {
            message_id: "ab-prop-stair-turn",
        },
    ),
    (
        "tread",
        Message {
            message_id: "ab-prop-stair-tread",
        },
    ),
    (
        "max_riser",
        Message {
            message_id: "ab-prop-stair-max-riser",
        },
    ),
    (
        "waist",
        Message {
            message_id: "ab-prop-stair-waist",
        },
    ),
    (
        "railing_left",
        Message {
            message_id: "ab-prop-stair-railing-left",
        },
    ),
    (
        "railing_right",
        Message {
            message_id: "ab-prop-stair-railing-right",
        },
    ),
    (
        "straight",
        Message {
            message_id: "ab-stair-shape-straight",
        },
    ),
    (
        "l_turn",
        Message {
            message_id: "ab-stair-shape-l-turn",
        },
    ),
    (
        "u_turn",
        Message {
            message_id: "ab-stair-shape-u-turn",
        },
    ),
    (
        "stair_direction",
        Message {
            message_id: "ab-prop-stair-direction",
        },
    ),
    (
        "stair_risers",
        Message {
            message_id: "ab-prop-stair-risers",
        },
    ),
    (
        "stair_riser",
        Message {
            message_id: "ab-prop-stair-riser",
        },
    ),
    (
        "stair_runs",
        Message {
            message_id: "ab-prop-stair-runs",
        },
    ),
    (
        "stair_stride",
        Message {
            message_id: "ab-prop-stair-stride",
        },
    ),
    (
        "stair_volume",
        Message {
            message_id: "ab-prop-stair-volume",
        },
    ),
    (
        "stair_storey",
        Message {
            message_id: "ab-prop-stair-storey",
        },
    ),
    (
        "stair_level_above",
        Message {
            message_id: "ab-stair-top-default",
        },
    ),
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
        "start_x",
        Message {
            message_id: "ab-prop-start-x",
        },
    ),
    (
        "start_y",
        Message {
            message_id: "ab-prop-start-y",
        },
    ),
    (
        "end_x",
        Message {
            message_id: "ab-prop-end-x",
        },
    ),
    (
        "end_y",
        Message {
            message_id: "ab-prop-end-y",
        },
    ),
    (
        "thickness",
        Message {
            message_id: "ab-prop-thickness",
        },
    ),
    (
        "height",
        Message {
            message_id: "ab-prop-height",
        },
    ),
    (
        "base_offset",
        Message {
            message_id: "ab-prop-base-offset",
        },
    ),
    (
        "location_line",
        Message {
            message_id: "ab-prop-location-line",
        },
    ),
    (
        "top_constraint",
        Message {
            message_id: "ab-prop-top-constraint",
        },
    ),
    (
        "start_join",
        Message {
            message_id: "ab-prop-start-join",
        },
    ),
    (
        "end_join",
        Message {
            message_id: "ab-prop-end-join",
        },
    ),
    (
        "top_offset",
        Message {
            message_id: "ab-prop-top-offset",
        },
    ),
    (
        "floor_area",
        Message {
            message_id: "ab-prop-floor-area",
        },
    ),
    (
        "floor_volume",
        Message {
            message_id: "ab-prop-floor-volume",
        },
    ),
    (
        "floor_walls",
        Message {
            message_id: "ab-prop-floor-walls",
        },
    ),
    (
        "floor_storey",
        Message {
            message_id: "ab-prop-floor-storey",
        },
    ),
    (
        "wall_volume",
        Message {
            message_id: "ab-prop-wall-volume",
        },
    ),
    (
        "department",
        Message {
            message_id: "ab-prop-department",
        },
    ),
    (
        "floor_finish",
        Message {
            message_id: "ab-prop-floor-finish",
        },
    ),
    (
        "wall_finish",
        Message {
            message_id: "ab-prop-wall-finish",
        },
    ),
    (
        "ceiling_finish",
        Message {
            message_id: "ab-prop-ceiling-finish",
        },
    ),
    (
        "base_finish",
        Message {
            message_id: "ab-prop-base-finish",
        },
    ),
    (
        "upper_constraint",
        Message {
            message_id: "ab-prop-upper-constraint",
        },
    ),
    (
        "upper_offset",
        Message {
            message_id: "ab-prop-upper-offset",
        },
    ),
    (
        "x",
        Message {
            message_id: "ab-prop-point-x",
        },
    ),
    (
        "y",
        Message {
            message_id: "ab-prop-point-y",
        },
    ),
    (
        "room_area",
        Message {
            message_id: "ab-prop-room-area",
        },
    ),
    (
        "room_perimeter",
        Message {
            message_id: "ab-prop-room-perimeter",
        },
    ),
    (
        "room_volume",
        Message {
            message_id: "ab-prop-room-volume",
        },
    ),
    (
        "room_state",
        Message {
            message_id: "ab-prop-room-state",
        },
    ),
    (
        "enclosed",
        Message {
            message_id: "ab-room-state-enclosed",
        },
    ),
    (
        "unenclosed",
        Message {
            message_id: "ab-room-state-unenclosed",
        },
    ),
    (
        "redundant",
        Message {
            message_id: "ab-room-state-redundant",
        },
    ),
    (
        "level_above",
        Message {
            message_id: "ab-room-upper-default",
        },
    ),
    (
        "auto",
        Message {
            message_id: "ab-join-auto",
        },
    ),
    (
        "butt",
        Message {
            message_id: "ab-join-butt",
        },
    ),
    (
        "square",
        Message {
            message_id: "ab-join-square",
        },
    ),
    (
        "disallowed",
        Message {
            message_id: "ab-join-disallowed",
        },
    ),
    (
        "unconnected",
        Message {
            message_id: "ab-top-unconnected",
        },
    ),
    (
        "wall_storey",
        Message {
            message_id: "ab-prop-wall-storey",
        },
    ),
    (
        "distance",
        Message {
            message_id: "ab-prop-distance",
        },
    ),
    (
        "width",
        Message {
            message_id: "ab-prop-width",
        },
    ),
    (
        "sill",
        Message {
            message_id: "ab-prop-sill",
        },
    ),
    (
        "frame_width",
        Message {
            message_id: "ab-prop-frame-width",
        },
    ),
    (
        "frame_depth",
        Message {
            message_id: "ab-prop-frame-depth",
        },
    ),
    (
        "operation",
        Message {
            message_id: "ab-prop-operation",
        },
    ),
    (
        "hand",
        Message {
            message_id: "ab-prop-hand",
        },
    ),
    (
        "facing",
        Message {
            message_id: "ab-prop-facing",
        },
    ),
    (
        "insert_wall",
        Message {
            message_id: "ab-prop-insert-wall",
        },
    ),
    (
        "single_swing",
        Message {
            message_id: "ab-op-single-swing",
        },
    ),
    (
        "fixed",
        Message {
            message_id: "ab-op-fixed",
        },
    ),
    (
        "casement",
        Message {
            message_id: "ab-op-casement",
        },
    ),
    (
        "left",
        Message {
            message_id: "ab-hand-left",
        },
    ),
    (
        "right",
        Message {
            message_id: "ab-hand-right",
        },
    ),
    (
        "center",
        Message {
            message_id: "ab-loc-center",
        },
    ),
    (
        "exterior",
        Message {
            message_id: "ab-loc-exterior",
        },
    ),
    (
        "interior",
        Message {
            message_id: "ab-loc-interior",
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
    (
        WALL,
        Message {
            message_id: "ab-kind-wall",
        },
    ),
    (
        FLOOR,
        Message {
            message_id: "ab-kind-floor",
        },
    ),
    (
        ROOM,
        Message {
            message_id: "ab-kind-room",
        },
    ),
    (
        DOOR,
        Message {
            message_id: "ab-kind-door",
        },
    ),
    (
        WINDOW,
        Message {
            message_id: "ab-kind-window",
        },
    ),
    (
        STAIR,
        Message {
            message_id: "ab-kind-stair",
        },
    ),
];

/// The heading of a selection of several kinds of object.
const OBJECTS: Message = Message {
    message_id: "ab-prop-objects",
};
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
#[allow(clippy::too_many_lines, reason = "one arm per kind of object")]
fn shown(
    snapshot: &Snapshot,
    key: &str,
    object: &Object,
    label: &dyn Fn(&str) -> String,
) -> Option<Shown> {
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
        // A wall shows the storey it stands on, which only moving the wall could change.
        WALL => {
            let storey = object
                .header
                .owner
                .and_then(|id| spatial::get(snapshot, id))
                .and_then(|s| types::text(&s.values, storey::NAME).map(str::to_owned));
            // The join choices of the two ends stay in the grid; the rest of the record is shown
            // through the fields below.
            let joins: Vec<(FieldDecl, Option<Value>)> = (types::wall::START_JOIN
                ..=types::wall::END_JOIN)
                .filter_map(|at| Some((fields.get(at)?.clone(), values.get(at)?.clone())))
                .collect();
            fields.truncate(types::wall::TOP_LEVEL);
            values.truncate(types::wall::TOP_LEVEL);
            fields.push(text_field("wall_storey"));
            values.push(storey.map(Value::String));
            read_only.push("wall_storey".to_owned());
            // The volume of the joined solid, which counts a corner once, minus the openings.
            // Openings of the wall's doors and windows are subtracted.
            let volume = archbuilder_model::insert::wall_solid(snapshot, object)
                .and_then(Result::ok)
                .map(|s| s.volume());
            fields.push(text_field("wall_volume"));
            values.push(volume.map(|v| Value::String(format!("{v:.3} m³"))));
            read_only.push("wall_volume".to_owned());
            // The top level is a choice below; editing the height disconnects the top.
            // A constrained wall shows the height its level gives.
            if let Some(wall) = archbuilder_model::wall::Wall::of(snapshot, object)
                && let Some(slot) = values.get_mut(types::wall::HEIGHT)
            {
                *slot = Some(types::length(wall.height));
            }
            // The top constraint is a choice: unconnected, or one of the levels above the base.
            let above = archbuilder_model::wall::Wall::levels_above(snapshot, object);
            let mut variants = vec![arc_doc::EnumVariant::new("unconnected", 0)];
            let mut current = 0;
            let top = archbuilder_model::wall::Wall::top_level(&object.values);
            for (code, l) in (1_u32..).zip(&above) {
                if let Some(name) = types::text(&l.values, level::NAME) {
                    variants.push(arc_doc::EnumVariant::new(name, code));
                    if top == Some(l.id) {
                        current = code;
                    }
                }
            }
            fields.push(
                FieldDecl::new("top_constraint", ValueType::Enum(variants)).in_aspect("geometry"),
            );
            values.push(Some(Value::Enum(current)));
            for (field, value) in joins {
                fields.push(field);
                values.push(value);
            }
            // Last: the fields above are found by their positions in the record.
            with_type(
                (snapshot, label),
                TypeKind::Wall,
                object,
                &mut fields,
                &mut values,
                &mut read_only,
            );
        }
        // A floor shows its storey, the area and volume derived from its boundary, and the walls
        // its edges follow; the boundary itself is edited by sketching.
        FLOOR => {
            let storey = object
                .header
                .owner
                .and_then(|id| spatial::get(snapshot, id))
                .and_then(|s| types::text(&s.values, storey::NAME).map(str::to_owned));
            let derived = archbuilder_model::floor::Derived::of(snapshot, object);
            let walls: Vec<String> = derived
                .as_ref()
                .map(|d| d.floor.walls())
                .unwrap_or_default()
                .into_iter()
                .filter_map(|w| spatial::get(snapshot, w))
                .filter_map(|w| types::text(&w.values, 0).map(str::to_owned))
                .collect();
            fields.truncate(types::floor::VERTICES);
            values.truncate(types::floor::VERTICES);
            with_type(
                (snapshot, label),
                TypeKind::Floor,
                object,
                &mut fields,
                &mut values,
                &mut read_only,
            );
            for (name, value) in [
                ("floor_storey", storey),
                (
                    "floor_area",
                    derived.as_ref().map(|d| format!("{:.3} m²", d.area())),
                ),
                (
                    "floor_volume",
                    derived.as_ref().map(|d| format!("{:.3} m³", d.volume())),
                ),
                (
                    "floor_walls",
                    Some(walls.join(", ")).filter(|w| !w.is_empty()),
                ),
            ] {
                fields.push(text_field(name));
                values.push(value.map(Value::String));
                read_only.push(name.to_owned());
            }
        }
        // A room shows its record, the upper limit as a choice, and what the walls make of it.
        ROOM => {
            use archbuilder_model::types::room as r;
            let storey = object
                .header
                .owner
                .and_then(|id| spatial::get(snapshot, id));
            let above = storey
                .as_ref()
                .map(|s| archbuilder_model::room::levels_above(snapshot, s))
                .unwrap_or_default();
            let mut variants = vec![arc_doc::EnumVariant::new("level_above", 0)];
            let mut current = 0;
            let upper = match values.get(r::UPPER_LEVEL) {
                Some(Some(Value::Reference(id))) => Some(*id),
                _ => None,
            };
            for (code, l) in (1_u32..).zip(&above) {
                if let Some(name) = types::text(&l.values, level::NAME) {
                    variants.push(arc_doc::EnumVariant::new(name, code));
                    if upper == Some(l.id) {
                        current = code;
                    }
                }
            }
            let entry = object.header.owner.and_then(|owner| {
                archbuilder_model::room::of_storey(snapshot, owner)
                    .into_iter()
                    .find(|e| e.id == object.id)
            });
            let order = [
                r::NAME,
                r::NUMBER,
                r::DEPARTMENT,
                r::FLOOR_FINISH,
                r::WALL_FINISH,
                r::CEILING_FINISH,
                r::BASE_FINISH,
            ];
            let (decls, vals) = (fields.clone(), values.clone());
            fields = order
                .iter()
                .filter_map(|k| decls.get(*k).cloned())
                .collect();
            values = order.iter().filter_map(|k| vals.get(*k).cloned()).collect();
            fields.push(
                FieldDecl::new("upper_constraint", ValueType::Enum(variants)).in_aspect("geometry"),
            );
            values.push(Some(Value::Enum(current)));
            for k in [r::UPPER_OFFSET, r::BASE_OFFSET, r::X, r::Y] {
                fields.extend(decls.get(k).cloned());
                values.push(vals.get(k).cloned().flatten());
            }
            let derived = |f: fn(&archbuilder_model::room::Entry) -> Option<f64>, unit: &str| {
                entry
                    .as_ref()
                    .and_then(f)
                    .map(|v| Value::String(format!("{v:.3} {unit}")))
            };
            for (name, value) in [
                (
                    "room_area",
                    derived(archbuilder_model::room::Entry::area, "m²"),
                ),
                (
                    "room_perimeter",
                    derived(archbuilder_model::room::Entry::perimeter, "m"),
                ),
                (
                    "room_volume",
                    derived(archbuilder_model::room::Entry::volume, "m³"),
                ),
            ] {
                fields.push(text_field(name));
                values.push(value);
                read_only.push(name.to_owned());
            }
            let state = entry.as_ref().map(|e| e.state());
            fields.push(
                FieldDecl::new(
                    "room_state",
                    ValueType::Enum(vec![
                        arc_doc::EnumVariant::new("enclosed", 0),
                        arc_doc::EnumVariant::new("unenclosed", 1),
                        arc_doc::EnumVariant::new("redundant", 2),
                    ]),
                )
                .in_aspect("data"),
            );
            values.push(state.map(|s| {
                Value::Enum(match s {
                    archbuilder_model::room::State::Enclosed => 0,
                    archbuilder_model::room::State::Unenclosed => 1,
                    archbuilder_model::room::State::Redundant => 2,
                })
            }));
            read_only.push("room_state".to_owned());
        }
        // A stair shows its record (the direction as a reading in degrees: it is turned with the
        // rotate tools), the top level as a choice and what the levels make of it.
        STAIR => {
            use archbuilder_model::types::stair as st;
            let storey = object
                .header
                .owner
                .and_then(|id| spatial::get(snapshot, id));
            let above = storey
                .as_ref()
                .map(|s| archbuilder_model::room::levels_above(snapshot, s))
                .unwrap_or_default();
            let mut variants = vec![arc_doc::EnumVariant::new("stair_level_above", 0)];
            let mut current = 0;
            let top = match values.get(st::TOP_LEVEL) {
                Some(Some(Value::Reference(id))) => Some(*id),
                _ => None,
            };
            for (code, l) in (1_u32..).zip(&above) {
                if let Some(name) = types::text(&l.values, level::NAME) {
                    variants.push(arc_doc::EnumVariant::new(name, code));
                    if top == Some(l.id) {
                        current = code;
                    }
                }
            }
            let placed = archbuilder_model::stair::Placed::of(snapshot, object);
            let layout = placed.as_ref().and_then(|p| p.layout.as_ref().ok());
            let direction = placed
                .as_ref()
                .map(|p| format!("{:.1}°", p.stair.direction.to_degrees()));
            let order = [
                st::NAME,
                st::SHAPE,
                st::TURN,
                st::WIDTH,
                st::TREAD,
                st::MAX_RISER,
                st::WAIST,
                st::RAILING_LEFT,
                st::RAILING_RIGHT,
                st::X,
                st::Y,
            ];
            let (decls, vals) = (fields.clone(), values.clone());
            fields = order
                .iter()
                .filter_map(|k| decls.get(*k).cloned())
                .collect();
            values = order.iter().filter_map(|k| vals.get(*k).cloned()).collect();
            fields.push(
                FieldDecl::new("top_constraint", ValueType::Enum(variants)).in_aspect("geometry"),
            );
            values.push(Some(Value::Enum(current)));
            let runs = layout.map(|l| {
                l.runs
                    .iter()
                    .map(|r| format!("{:.3} m", r.length))
                    .collect::<Vec<_>>()
                    .join(" + ")
            });
            for (name, value) in [
                (
                    "stair_storey",
                    storey
                        .as_ref()
                        .and_then(|s| types::text(&s.values, storey::NAME).map(str::to_owned)),
                ),
                ("stair_direction", direction),
                ("stair_risers", layout.map(|l| l.risers.to_string())),
                ("stair_riser", layout.map(|l| format!("{:.5} m", l.riser))),
                ("stair_runs", runs),
                (
                    "stair_stride",
                    layout.map(|l| {
                        let warn = if l.stride_ok() { "" } else { " ⚠" };
                        format!("{:.3} m{warn}", l.stride())
                    }),
                ),
                (
                    "stair_volume",
                    layout.map(|l| format!("{:.3} m³", l.volume())),
                ),
            ] {
                fields.push(text_field(name));
                values.push(value.map(Value::String));
                read_only.push(name.to_owned());
            }
        }
        // A door or a window shows the wall that hosts it; a door's sill is the wall's base.
        DOOR | WINDOW => {
            let host = object
                .header
                .owner
                .and_then(|id| spatial::get(snapshot, id))
                .and_then(|w| types::text(&w.values, 0).map(str::to_owned));
            fields.truncate(types::insert::INSERT_TYPE);
            values.truncate(types::insert::INSERT_TYPE);
            let kind = if key == DOOR {
                TypeKind::Door
            } else {
                TypeKind::Window
            };
            with_type(
                (snapshot, label),
                kind,
                object,
                &mut fields,
                &mut values,
                &mut read_only,
            );
            fields.push(text_field("insert_wall"));
            values.push(host.map(Value::String));
            read_only.push("insert_wall".to_owned());
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

/// The type choice of an instance of `kind`: the types of the kind in their order (the grid sends
/// the number, which `archbuilder.property.set` of `type` reads), and the instance's.
fn type_field(snapshot: &Snapshot, kind: TypeKind, object: &Object) -> (FieldDecl, Option<Value>) {
    let all = et::of_kind(snapshot, kind);
    let own = et::type_id(kind, &object.values);
    let variants = (0_u32..)
        .zip(&all)
        .map(|(code, t)| arc_doc::EnumVariant::new(et::name(t), code))
        .collect();
    let current = (0_u32..)
        .zip(&all)
        .find(|(_, t)| Some(t.id) == own)
        .map(|(code, _)| Value::Enum(code));
    (
        FieldDecl::new("type", ValueType::Enum(variants)).in_aspect("identity"),
        current,
    )
}

/// Puts the type choice of `object` second, after the name, and makes the fields its type
/// decides read-only; a choice among them shows its value's label as text.
fn with_type(
    (snapshot, label): (&Snapshot, &dyn Fn(&str) -> String),
    kind: TypeKind,
    object: &Object,
    fields: &mut Vec<FieldDecl>,
    values: &mut Vec<Option<Value>>,
    read_only: &mut Vec<String>,
) {
    for (field, value) in fields.iter_mut().zip(values.iter_mut()) {
        if !et::is_copied(kind, &field.name) {
            continue;
        }
        read_only.push(field.name.clone());
        if let (ValueType::Enum(variants), Some(Value::Enum(code))) = (&field.value_type, &*value) {
            let text = variants
                .iter()
                .find(|v| v.code == *code)
                .map(|v| label(&v.name))
                .unwrap_or_default();
            *field = FieldDecl::new(&field.name, ValueType::String).in_aspect(&field.aspect);
            *value = Some(Value::String(text));
        }
    }
    let (field, value) = type_field(snapshot, kind, object);
    let at = 1.min(fields.len());
    fields.insert(at, field);
    values.insert(at, value);
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
    #[allow(
        clippy::too_many_lines,
        reason = "one pass over the selection and the grid"
    )]
    fn show(&mut self, ui: &mut Ui, cx: &mut PanelContext<'_>) {
        let (Some(view), Some(tab)) = (cx.view.as_deref(), cx.tab) else {
            return;
        };
        let Ok(tab) = cx.app.tab(tab) else {
            return;
        };
        let localize = cx.localize;
        let label_of = |name: &str| {
            LABELS
                .iter()
                .find(|(n, _)| *n == name)
                .map_or_else(|| name.to_owned(), |(_, m)| say(localize, m))
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
                Some((shown(&snapshot, &key, &object, &label_of)?, id))
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
        let mixed = items.iter().any(|(s, _)| s.key != first_key);
        let heading = if mixed {
            cx.localize
                .message(
                    OBJECTS.message_id,
                    &[(
                        "count",
                        arc_base::error::Value::Int(i64::try_from(items.len()).unwrap_or(i64::MAX)),
                    )],
                )
                .unwrap_or_else(|| items.len().to_string())
        } else if items.len() > 1 {
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
