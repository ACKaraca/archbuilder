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
use arc_ui::{GridContext, Intent, Localize, PropertyGrid, PropertyModel, PropertyObject};
use archbuilder_commands::structure::PROPERTY_SET;
use archbuilder_model::element_type::{self as et, TypeKind};
use archbuilder_model::spatial;
use archbuilder_model::types::{
    self, BUILDING, DOOR, ELEVATION_RANGE, FLOOR, LEVEL, PROJECT, ROOF, ROOM, SITE, STAIR, STOREY,
    WALL, WINDOW, level, storey,
};
use archbuilder_model::view::PlanView;

/// A message of the application.
struct Message {
    message_id: &'static str,
}

/// What a derived value shows while the extraction workers still derive it.
const PENDING: &str = "…";

/// The readable name of every field, enum variant and aspect the grid can show.
const LABELS: &[(&str, Message)] = &[
    (
        "cut",
        Message {
            message_id: "ab-prop-view-cut",
        },
    ),
    (
        "bottom",
        Message {
            message_id: "ab-prop-view-bottom",
        },
    ),
    (
        "depth",
        Message {
            message_id: "ab-prop-view-depth",
        },
    ),
    (
        "top",
        Message {
            message_id: "ab-prop-view-top",
        },
    ),
    (
        "overhead",
        Message {
            message_id: "ab-prop-view-overhead",
        },
    ),
    (
        "scale",
        Message {
            message_id: "ab-prop-view-scale",
        },
    ),
    (
        "detail",
        Message {
            message_id: "ab-prop-view-detail",
        },
    ),
    (
        "underlay",
        Message {
            message_id: "ab-prop-view-underlay",
        },
    ),
    (
        "show_walls",
        Message {
            message_id: "ab-prop-view-show-walls",
        },
    ),
    (
        "show_floors",
        Message {
            message_id: "ab-prop-view-show-floors",
        },
    ),
    (
        "show_roofs",
        Message {
            message_id: "ab-prop-view-show-roofs",
        },
    ),
    (
        "show_ceilings",
        Message {
            message_id: "ab-prop-view-show-ceilings",
        },
    ),
    (
        "show_doors",
        Message {
            message_id: "ab-prop-view-show-doors",
        },
    ),
    (
        "show_windows",
        Message {
            message_id: "ab-prop-view-show-windows",
        },
    ),
    (
        "show_stairs",
        Message {
            message_id: "ab-prop-view-show-stairs",
        },
    ),
    (
        "show_rooms",
        Message {
            message_id: "ab-prop-view-show-rooms",
        },
    ),
    (
        "show_openings",
        Message {
            message_id: "ab-prop-view-show-openings",
        },
    ),
    (
        "view_storey",
        Message {
            message_id: "ab-prop-view-view-storey",
        },
    ),
    (
        "view_hidden",
        Message {
            message_id: "ab-prop-view-view-hidden",
        },
    ),
    (
        "coarse",
        Message {
            message_id: "ab-prop-view-coarse",
        },
    ),
    (
        "medium",
        Message {
            message_id: "ab-prop-view-medium",
        },
    ),
    (
        "fine",
        Message {
            message_id: "ab-prop-view-fine",
        },
    ),
    (
        "height_offset",
        Message {
            message_id: "ab-prop-height-offset",
        },
    ),
    (
        "grid_x",
        Message {
            message_id: "ab-prop-grid-x",
        },
    ),
    (
        "grid_y",
        Message {
            message_id: "ab-prop-grid-y",
        },
    ),
    (
        "grid_rotation",
        Message {
            message_id: "ab-prop-grid-rotation",
        },
    ),
    (
        "ceiling_area",
        Message {
            message_id: "ab-prop-ceiling-area",
        },
    ),
    (
        "ceiling_volume",
        Message {
            message_id: "ab-prop-ceiling-volume",
        },
    ),
    (
        "floor_slope",
        Message {
            message_id: "ab-prop-floor-slope",
        },
    ),
    (
        "ceiling_slope",
        Message {
            message_id: "ab-prop-ceiling-slope",
        },
    ),
    (
        "ceiling_storey",
        Message {
            message_id: "ab-prop-ceiling-storey",
        },
    ),
    (
        "rise",
        Message {
            message_id: "ab-prop-rise",
        },
    ),
    (
        "opening_wall",
        Message {
            message_id: "ab-prop-opening-wall",
        },
    ),
    (
        "shaft_base",
        Message {
            message_id: "ab-prop-shaft-base",
        },
    ),
    (
        "shaft_top",
        Message {
            message_id: "ab-prop-shaft-top",
        },
    ),
    (
        "shaft_span",
        Message {
            message_id: "ab-prop-shaft-span",
        },
    ),
    (
        "shaft_area",
        Message {
            message_id: "ab-prop-shaft-area",
        },
    ),
    (
        "shaft_default",
        Message {
            message_id: "ab-prop-shaft-default",
        },
    ),
    (
        "cut_host",
        Message {
            message_id: "ab-prop-cut-host",
        },
    ),
    (
        "cut_area",
        Message {
            message_id: "ab-prop-cut-area",
        },
    ),
    (
        "extrusion_type",
        Message {
            message_id: "ab-prop-extrusion-type",
        },
    ),
    (
        "extrusion_area",
        Message {
            message_id: "ab-prop-extrusion-area",
        },
    ),
    (
        "extrusion_volume",
        Message {
            message_id: "ab-prop-extrusion-volume",
        },
    ),
    (
        "edge_floor",
        Message {
            message_id: "ab-prop-edge-floor",
        },
    ),
    (
        "edge_length",
        Message {
            message_id: "ab-prop-edge-length",
        },
    ),
    (
        "edge_volume",
        Message {
            message_id: "ab-prop-edge-volume",
        },
    ),
    (
        "room_height",
        Message {
            message_id: "ab-prop-room-height",
        },
    ),
    (
        "room_unbounded",
        Message {
            message_id: "ab-prop-room-unbounded",
        },
    ),
    (
        "room_bounded_by",
        Message {
            message_id: "ab-prop-room-bounded-by",
        },
    ),
    (
        "roof_storey",
        Message {
            message_id: "ab-prop-roof-storey",
        },
    ),
    (
        "roof_slope",
        Message {
            message_id: "ab-prop-roof-slope",
        },
    ),
    (
        "roof_area",
        Message {
            message_id: "ab-prop-roof-area",
        },
    ),
    (
        "roof_top",
        Message {
            message_id: "ab-prop-roof-top",
        },
    ),
    (
        "roof_follows",
        Message {
            message_id: "ab-prop-roof-follows",
        },
    ),
    (
        "roof_attached",
        Message {
            message_id: "ab-prop-roof-attached",
        },
    ),
    (
        "overhang",
        Message {
            message_id: "ab-prop-overhang",
        },
    ),
    (
        "wall_attached",
        Message {
            message_id: "ab-prop-wall-attached",
        },
    ),
    (
        "slope_unit",
        Message {
            message_id: "ab-prop-slope-unit",
        },
    ),
    (
        "degrees",
        Message {
            message_id: "ab-slope-degrees",
        },
    ),
    (
        "percent",
        Message {
            message_id: "ab-slope-percent",
        },
    ),
    (
        "roof_slopes_mixed",
        Message {
            message_id: "ab-prop-roof-slopes-mixed",
        },
    ),
    (
        "rise_run",
        Message {
            message_id: "ab-slope-rise-run",
        },
    ),
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
        "pinned",
        Message {
            message_id: "ab-prop-pinned",
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
        "curtain_panels",
        Message {
            message_id: "ab-prop-curtain-panels",
        },
    ),
    (
        "curtain_mullions",
        Message {
            message_id: "ab-prop-curtain-mullions",
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
        "strong",
        Message {
            message_id: "ab-strength-strong",
        },
    ),
    (
        "weak",
        Message {
            message_id: "ab-strength-weak",
        },
    ),
    (
        "none",
        Message {
            message_id: "ab-strength-none",
        },
    ),
    (
        "points",
        Message {
            message_id: "ab-source-points",
        },
    ),
    (
        "grid",
        Message {
            message_id: "ab-source-grid",
        },
    ),
    (
        "reference_plane",
        Message {
            message_id: "ab-source-reference-plane",
        },
    ),
    (
        "bulge",
        Message {
            message_id: "ab-prop-bulge",
        },
    ),
    (
        "head_start",
        Message {
            message_id: "ab-prop-head-start",
        },
    ),
    (
        "head_end",
        Message {
            message_id: "ab-prop-head-end",
        },
    ),
    (
        "strength",
        Message {
            message_id: "ab-prop-strength",
        },
    ),
    (
        "bounded",
        Message {
            message_id: "ab-prop-bounded",
        },
    ),
    (
        "source",
        Message {
            message_id: "ab-prop-source",
        },
    ),
    (
        "active",
        Message {
            message_id: "ab-prop-active",
        },
    ),
    (
        "shown",
        Message {
            message_id: "ab-prop-shown",
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
        "range",
        Message {
            message_id: "ab-aspect-range",
        },
    ),
    (
        "graphics",
        Message {
            message_id: "ab-aspect-graphics",
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
    (
        types::RAILING,
        Message {
            message_id: "ab-kind-railing",
        },
    ),
    (
        types::RAMP,
        Message {
            message_id: "ab-kind-ramp",
        },
    ),
    (
        ROOF,
        Message {
            message_id: "ab-kind-roof",
        },
    ),
    (
        types::CEILING,
        Message {
            message_id: "ab-kind-ceiling",
        },
    ),
    (
        types::PLAN_VIEW,
        Message {
            message_id: "ab-kind-plan-view",
        },
    ),
    (
        types::WALL_OPENING,
        Message {
            message_id: "ab-kind-wall-opening",
        },
    ),
    (
        types::SHAFT,
        Message {
            message_id: "ab-kind-shaft",
        },
    ),
    (
        types::VERTICAL_OPENING,
        Message {
            message_id: "ab-kind-vertical-opening",
        },
    ),
    (
        types::FLOOR_EDGE,
        Message {
            message_id: "ab-kind-floor-edge",
        },
    ),
    (
        types::ROOF_EXTRUSION,
        Message {
            message_id: "ab-kind-roof-extrusion",
        },
    ),
    (
        types::GRID,
        Message {
            message_id: "ab-kind-grid",
        },
    ),
    (
        types::REFERENCE_PLANE,
        Message {
            message_id: "ab-kind-reference-plane",
        },
    ),
    (
        types::WORK_PLANE,
        Message {
            message_id: "ab-kind-work-plane",
        },
    ),
];

/// The message identifier of the readable name of the kind `key`.
#[must_use]
pub fn kind_message(key: &str) -> Option<&'static str> {
    KINDS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, m)| m.message_id)
}

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
const DEFAULT_VIEW: Message = Message {
    message_id: "ab-prop-view-default-note",
};

/// Labels of the fields of ramps and railings.
const CIRC_LABELS: [(&str, Message); 25] = [
    (
        "length",
        Message {
            message_id: "ab-prop-circ-length",
        },
    ),
    (
        "landing_start",
        Message {
            message_id: "ab-prop-circ-landing-start",
        },
    ),
    (
        "landing_end",
        Message {
            message_id: "ab-prop-circ-landing-end",
        },
    ),
    (
        "form",
        Message {
            message_id: "ab-prop-circ-form",
        },
    ),
    (
        "slab",
        Message {
            message_id: "ab-prop-circ-slab",
        },
    ),
    (
        "solid",
        Message {
            message_id: "ab-prop-circ-solid",
        },
    ),
    (
        "side",
        Message {
            message_id: "ab-prop-circ-side",
        },
    ),
    (
        "none",
        Message {
            message_id: "ab-prop-circ-none",
        },
    ),
    (
        "start_extension",
        Message {
            message_id: "ab-prop-circ-start-extension",
        },
    ),
    (
        "end_extension",
        Message {
            message_id: "ab-prop-circ-end-extension",
        },
    ),
    (
        "extension_length",
        Message {
            message_id: "ab-prop-circ-extension-length",
        },
    ),
    (
        "horizontal",
        Message {
            message_id: "ab-prop-circ-horizontal",
        },
    ),
    (
        "return_wall",
        Message {
            message_id: "ab-prop-circ-return-wall",
        },
    ),
    (
        "return_floor",
        Message {
            message_id: "ab-prop-circ-return-floor",
        },
    ),
    (
        "ramp_slope",
        Message {
            message_id: "ab-prop-circ-ramp-slope",
        },
    ),
    (
        "ramp_rise",
        Message {
            message_id: "ab-prop-circ-ramp-rise",
        },
    ),
    (
        "ramp_volume",
        Message {
            message_id: "ab-prop-circ-ramp-volume",
        },
    ),
    (
        "ramp_type_name",
        Message {
            message_id: "ab-prop-circ-ramp-type-name",
        },
    ),
    (
        "railing_type_name",
        Message {
            message_id: "ab-prop-circ-railing-type-name",
        },
    ),
    (
        "railing_host",
        Message {
            message_id: "ab-prop-circ-railing-host",
        },
    ),
    (
        "railing_length",
        Message {
            message_id: "ab-prop-circ-railing-length",
        },
    ),
    (
        "railing_posts",
        Message {
            message_id: "ab-prop-circ-railing-posts",
        },
    ),
    (
        "railing_balusters",
        Message {
            message_id: "ab-prop-circ-railing-balusters",
        },
    ),
    (
        "railing_gap",
        Message {
            message_id: "ab-prop-circ-railing-gap",
        },
    ),
    (
        "railing_continues",
        Message {
            message_id: "ab-prop-circ-railing-continues",
        },
    ),
];

/// The readable name of a field or variant; the rows of a roof's edges are numbered from 1.
fn label_text(localize: &dyn Localize, name: &str) -> String {
    const SLOPED: Message = Message {
        message_id: "ab-prop-roof-edge-sloped",
    };
    const SLOPE: Message = Message {
        message_id: "ab-prop-roof-edge-slope",
    };
    const OVERHANG: Message = Message {
        message_id: "ab-prop-roof-edge-overhang",
    };
    for (prefix, message) in [
        ("roof_edge_sloped_", &SLOPED),
        ("roof_edge_slope_", &SLOPE),
        ("roof_edge_overhang_", &OVERHANG),
    ] {
        if let Some(k) = name
            .strip_prefix(prefix)
            .and_then(|k| k.parse::<i64>().ok())
        {
            return localize
                .message(
                    message.message_id,
                    &[("n", arc_base::error::Value::Int(k + 1))],
                )
                .unwrap_or_else(|| name.to_owned());
        }
    }
    LABELS
        .iter()
        .chain(CIRC_LABELS.iter())
        .find(|(n, _)| *n == name)
        .map_or_else(|| name.to_owned(), |(_, m)| say(localize, m))
}

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
#[allow(
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    reason = "one arm per kind of object"
)]
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
        types::PLAN_VIEW => {
            // The hidden elements are counted, not listed (Show hidden in view of the context
            // menu shows them again); the storey is shown for orientation.
            // The grid overrides are set by `archbuilder.datum.set_view_extent`.
            if fields.len() > types::plan_view::DATUM_VIEWS
                && values.len() > types::plan_view::DATUM_VIEWS
            {
                fields.remove(types::plan_view::DATUM_VIEWS);
                values.remove(types::plan_view::DATUM_VIEWS);
            }
            if fields.len() > types::plan_view::HIDDEN && values.len() > types::plan_view::HIDDEN {
                fields.remove(types::plan_view::HIDDEN);
                values.remove(types::plan_view::HIDDEN);
            }
            // A top the view does not set shows the one it takes: the next level, else 3 m.
            if let Some(slot) = values.get_mut(types::plan_view::TOP)
                && slot.is_none()
                && let Some(settings) =
                    archbuilder_model::view::PlanView::from_values(&object.values)
                && let Some((_, level, above)) =
                    archbuilder_model::view::levels_of(snapshot, object)
            {
                let top = PlanView {
                    overhead: true,
                    ..settings
                }
                .range(level, above)
                .2;
                *slot = top.map(|t| types::length(t - level));
            }
            let storey = object
                .header
                .owner
                .and_then(|id| spatial::get(snapshot, id))
                .and_then(|s| types::text(&s.values, storey::NAME).map(str::to_owned));
            fields.push(text_field("view_storey"));
            values.push(storey.map(Value::String));
            read_only.push("view_storey".to_owned());
            let hidden = archbuilder_model::view::PlanView::from_values(&object.values)
                .map_or(0, |v| v.hidden.len());
            fields.push(text_field("view_hidden"));
            values.push(Some(Value::String(hidden.to_string())));
            read_only.push("view_hidden".to_owned());
        }
        // A grid's extent levels are set by `archbuilder.datum.set_extent`; a work plane shows
        // where it comes from and whether it is active and shown.
        types::GRID | types::WORK_PLANE => {
            let hidden: &[usize] = if key == types::GRID {
                &[
                    types::grid::BOTTOM_LEVEL,
                    types::grid::TOP_LEVEL,
                    types::grid::PATH,
                ]
            } else {
                &[
                    types::work_plane::TARGET,
                    3,
                    4,
                    5,
                    6,
                    7,
                    8,
                    9,
                    10,
                    11,
                    types::work_plane::FACE_ELEMENT,
                    types::work_plane::FACE,
                ]
            };
            for at in hidden.iter().rev() {
                if *at < fields.len() && *at < values.len() {
                    fields.remove(*at);
                    values.remove(*at);
                }
            }
            if key == types::WORK_PLANE {
                read_only.push("source".to_owned());
            }
        }
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
            // The layers of the storey are derived on the extraction workers after an edit: until
            // they are, the volume shows as pending rather than holding the frame up.
            let volume = match archbuilder_model::cleanup::wall_net_volume_ready(snapshot, object) {
                Some(v) => v.map(|v| Value::String(format!("{v:.3} m³"))),
                None => Some(Value::String(PENDING.to_owned())),
            };
            fields.push(text_field("wall_volume"));
            values.push(volume);
            read_only.push("wall_volume".to_owned());
            // A curtain wall: the derived panels and mullions, which schedules and the export
            // read the same way.
            if let Some(curtain) = archbuilder_model::curtain::Derived::of(snapshot, object) {
                let (panels, mullions) = curtain.counts();
                let area: f64 = curtain.panels.iter().map(|p| p.area()).sum();
                let length: f64 = curtain.mullions.iter().map(|m| m.length()).sum();
                let orphans =
                    curtain.layout.orphans.edits.len() + curtain.layout.orphans.overrides.len();
                let mut panel_text = format!("{panels} / {area:.2} m²");
                if orphans > 0 {
                    panel_text.push_str(&format!(" ({orphans} orphaned)"));
                }
                fields.push(text_field("curtain_panels"));
                values.push(Some(Value::String(panel_text)));
                read_only.push("curtain_panels".to_owned());
                fields.push(text_field("curtain_mullions"));
                values.push(Some(Value::String(format!("{mullions} / {length:.2} m"))));
                read_only.push("curtain_mullions".to_owned());
            }
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
            // The offset of a constrained top, and what the top is attached to (attach and
            // detach are commands of the context menu).
            if let Some(decl) = decl.schema.fields.get(types::wall::TOP_OFFSET) {
                fields.push(decl.clone());
                values.push(Some(types::length(archbuilder_model::wall::top_offset(
                    &object.values,
                ))));
            }
            let attached = archbuilder_model::wall::attachment(&object.values)
                .and_then(|id| spatial::get(snapshot, id))
                .and_then(|t| types::text(&t.values, 0).map(str::to_owned));
            fields.push(text_field("wall_attached"));
            values.push(attached.map(Value::String));
            read_only.push("wall_attached".to_owned());
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
            let slope = derived.as_ref().and_then(|d| {
                d.arrow.map(|a| {
                    let unit = spatial::project(snapshot).map_or_else(Default::default, |p| {
                        archbuilder_model::units::slope_unit(&p.values)
                    });
                    archbuilder_model::units::slope_text(
                        a.gradient(d.floor.top_offset).atan(),
                        unit,
                        '.',
                    )
                })
            });
            for (name, value) in [
                ("floor_storey", storey),
                ("floor_slope", slope),
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
                // The slope is typed: an angle, a percentage or a rise over a run.
                if name != "floor_slope" {
                    read_only.push(name.to_owned());
                }
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
            // The rooms and their solids are derived on the extraction workers after an edit: until
            // they are, the derived values show as pending rather than holding the frame up.
            let rooms = object
                .header
                .owner
                .and_then(|owner| archbuilder_model::room::of_storey_ready(snapshot, owner));
            let entry = rooms
                .as_ref()
                .and_then(|r| r.iter().find(|e| e.id == object.id).cloned());
            let shape_ready = entry
                .as_ref()
                .and_then(|e| archbuilder_model::room::shape_ready(snapshot, e));
            let pending = rooms.is_none() || (entry.is_some() && shape_ready.is_none());
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
            // The room's solid: under the lowest ceiling, roof or slab above it.
            let shape = shape_ready.flatten();
            let derived = |f: fn(&archbuilder_model::room::Entry) -> Option<f64>, unit: &str| {
                entry
                    .as_ref()
                    .and_then(f)
                    .map(|v| Value::String(format!("{v:.3} {unit}")))
            };
            for (name, value) in [
                (
                    "room_area",
                    entry
                        .as_ref()
                        .and_then(|e| shape.as_ref().map(|s| s.area).or_else(|| e.area()))
                        .map(|a| Value::String(format!("{a:.3} m²"))),
                ),
                (
                    "room_perimeter",
                    derived(archbuilder_model::room::Entry::perimeter, "m"),
                ),
                (
                    "room_volume",
                    shape
                        .as_ref()
                        .map(|s| Value::String(format!("{:.3} m³", s.volume)))
                        .or_else(|| derived(archbuilder_model::room::Entry::volume, "m³")),
                ),
                (
                    "room_height",
                    shape
                        .as_ref()
                        .map(|s| Value::String(format!("{:.3} m", s.height))),
                ),
                (
                    "room_unbounded",
                    shape
                        .as_ref()
                        .map(|s| Value::String(format!("{:.3} m", s.unbounded))),
                ),
                (
                    "room_bounded_by",
                    shape.as_ref().map(|s| {
                        let names: Vec<String> = s
                            .bounds
                            .iter()
                            .filter_map(|b| {
                                let id = match b {
                                    archbuilder_model::room::Bound::Ceiling(id)
                                    | archbuilder_model::room::Bound::Roof(id)
                                    | archbuilder_model::room::Bound::Slab(id) => *id,
                                };
                                spatial::get(snapshot, id)
                                    .and_then(|o| types::text(&o.values, 0).map(str::to_owned))
                            })
                            .collect();
                        Value::String(names.join(", "))
                    }),
                ),
            ] {
                fields.push(text_field(name));
                values.push(if pending {
                    Some(Value::String(PENDING.to_owned()))
                } else {
                    value
                });
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
        // A ramp shows its record and what its levels make of it.
        types::RAMP => {
            use archbuilder_model::types::ramp as rp;
            let order = [
                rp::NAME,
                rp::WIDTH,
                rp::LENGTH,
                rp::LANDING_START,
                rp::LANDING_END,
                rp::THICKNESS,
                rp::FORM,
                rp::RISE,
                rp::X,
                rp::Y,
            ];
            let (decls, vals) = (fields.clone(), values.clone());
            fields = order
                .iter()
                .filter_map(|k| decls.get(*k).cloned())
                .collect();
            values = order.iter().filter_map(|k| vals.get(*k).cloned()).collect();
            let placed = archbuilder_model::ramp::Placed::of(snapshot, object);
            let layout = placed.as_ref().and_then(|p| p.layout.as_ref().ok());
            let kind = placed
                .as_ref()
                .and_then(|p| p.ramp.ramp_type)
                .and_then(|t| spatial::get(snapshot, t))
                .and_then(|t| types::text(&t.values, 0).map(str::to_owned));
            for (name, value) in [
                (
                    "ramp_slope",
                    layout.map(|l| {
                        let warn = if l.exceeds() { " ⚠" } else { "" };
                        format!("{:.1} %{warn}", l.slope() * 100.0)
                    }),
                ),
                ("ramp_rise", layout.map(|l| format!("{:.3} m", l.height))),
                (
                    "ramp_volume",
                    layout.map(|l| format!("{:.3} m³", l.volume())),
                ),
                ("ramp_type_name", kind),
            ] {
                fields.push(text_field(name));
                values.push(value.map(Value::String));
                read_only.push(name.to_owned());
            }
        }
        // A railing shows its side and extensions, and what its path and type make of it.
        types::RAILING => {
            use archbuilder_model::types::railing as rl;
            let order = [
                rl::NAME,
                rl::SIDE,
                rl::START_EXTENSION,
                rl::END_EXTENSION,
                rl::EXTENSION_LENGTH,
            ];
            let (decls, vals) = (fields.clone(), values.clone());
            fields = order
                .iter()
                .filter_map(|k| decls.get(*k).cloned())
                .collect();
            values = order.iter().filter_map(|k| vals.get(*k).cloned()).collect();
            let derived = archbuilder_model::railing::Derived::of(snapshot, object).ok();
            let name_of = |at: usize| match object.values.get(at) {
                Some(Some(Value::Reference(id))) => spatial::get(snapshot, *id)
                    .and_then(|o| types::text(&o.values, 0).map(str::to_owned)),
                _ => None,
            };
            for (name, value) in [
                ("railing_type_name", name_of(rl::RAILING_TYPE)),
                ("railing_host", name_of(rl::HOST)),
                ("railing_continues", name_of(rl::CONTINUES_TO)),
                (
                    "railing_length",
                    derived.as_ref().map(|d| format!("{:.3} m", d.length())),
                ),
                (
                    "railing_posts",
                    derived.as_ref().map(|d| d.posts().len().to_string()),
                ),
                (
                    "railing_balusters",
                    derived.as_ref().map(|d| d.balusters().0.len().to_string()),
                ),
                (
                    "railing_gap",
                    derived.as_ref().map(|d| {
                        let gap = d.widest_gap();
                        let warn = if gap > archbuilder_model::railing::CLEAR_GAP_LIMIT {
                            " ⚠"
                        } else {
                            ""
                        };
                        format!("{:.0} mm{warn}", gap * 1000.0)
                    }),
                ),
            ] {
                fields.push(text_field(name));
                values.push(value.map(Value::String));
                read_only.push(name.to_owned());
            }
        }
        // A roof shows its type, base offset and overhang, one slope for its sloped edges, then
        // each edge (whether it slopes, its slope, its own overhang), and what derives from it.
        ROOF => roof_rows(
            (snapshot, label),
            object,
            &mut fields,
            &mut values,
            &mut read_only,
        ),
        // A ceiling shows its type, height and grid origin, what its slope arrow makes of it,
        // and the area and volume derived from its boundary (the boundary is edited by sketching).
        types::CEILING => {
            use archbuilder_model::types::ceiling as c;
            let storey = object
                .header
                .owner
                .and_then(|id| spatial::get(snapshot, id))
                .and_then(|s| types::text(&s.values, storey::NAME).map(str::to_owned));
            let derived = spatial::get(snapshot, object.id)
                .and_then(|o| archbuilder_model::ceiling::derived(snapshot, &o));
            let order = [
                c::NAME,
                c::THICKNESS,
                c::HEIGHT_OFFSET,
                c::GRID_X,
                c::GRID_Y,
                c::GRID_ROTATION,
            ];
            let (decls, vals) = (fields.clone(), values.clone());
            fields = order
                .iter()
                .filter_map(|k| decls.get(*k).cloned())
                .collect();
            values = order.iter().filter_map(|k| vals.get(*k).cloned()).collect();
            with_type(
                (snapshot, label),
                TypeKind::Ceiling,
                object,
                &mut fields,
                &mut values,
                &mut read_only,
            );
            let slope =
                archbuilder_model::ceiling::Ceiling::from_values(&object.values).and_then(|c| {
                    c.arrow.map(|a| {
                        let unit = spatial::project(snapshot).map_or_else(Default::default, |p| {
                            archbuilder_model::units::slope_unit(&p.values)
                        });
                        archbuilder_model::units::slope_text(
                            a.gradient(c.height()).atan(),
                            unit,
                            '.',
                        )
                    })
                });
            for (name, value) in [
                ("ceiling_storey", storey),
                ("ceiling_slope", slope),
                (
                    "ceiling_area",
                    derived.as_ref().map(|d| format!("{:.3} m²", d.area())),
                ),
                (
                    "ceiling_volume",
                    derived.as_ref().map(|d| format!("{:.3} m³", d.volume())),
                ),
            ] {
                fields.push(text_field(name));
                values.push(value.map(Value::String));
                // The slope is typed: an angle, a percentage or a rise over a run.
                if name != "ceiling_slope" {
                    read_only.push(name.to_owned());
                }
            }
        }
        // A bare wall opening shows the wall that hosts it, its size, sill and arch.
        types::WALL_OPENING => {
            let host = object
                .header
                .owner
                .and_then(|id| spatial::get(snapshot, id))
                .and_then(|w| types::text(&w.values, 0).map(str::to_owned));
            fields.push(text_field("opening_wall"));
            values.push(host.map(Value::String));
            read_only.push("opening_wall".to_owned());
        }
        // A shaft shows its base and top as choices among the levels, and its footprint's area.
        types::SHAFT => {
            use archbuilder_model::types::shaft as sh;
            let building = object
                .header
                .owner
                .and_then(|id| spatial::get(snapshot, id))
                .and_then(|s| s.header.owner);
            let levels = building
                .map(|b| spatial::levels(snapshot, b))
                .unwrap_or_default();
            let choice = |name: &str, at: usize| {
                let mut variants = vec![arc_doc::EnumVariant::new("shaft_default", 0)];
                let mut current = 0;
                let set = match object.values.get(at) {
                    Some(Some(Value::Reference(id))) => Some(*id),
                    _ => None,
                };
                for (code, l) in (1_u32..).zip(&levels) {
                    if let Some(n) = types::text(&l.values, level::NAME) {
                        variants.push(arc_doc::EnumVariant::new(n, code));
                        if set == Some(l.id) {
                            current = code;
                        }
                    }
                }
                (
                    FieldDecl::new(name, ValueType::Enum(variants)).in_aspect("geometry"),
                    Some(Value::Enum(current)),
                )
            };
            let span = archbuilder_model::opening::shaft_span(snapshot, object);
            let order = [sh::NAME, sh::BASE_OFFSET, sh::TOP_OFFSET];
            let (decls, vals) = (fields.clone(), values.clone());
            fields = order
                .iter()
                .filter_map(|k| decls.get(*k).cloned())
                .collect();
            values = order.iter().filter_map(|k| vals.get(*k).cloned()).collect();
            for (name, at) in [("shaft_base", sh::BASE_LEVEL), ("shaft_top", sh::TOP_LEVEL)] {
                let (f, v) = choice(name, at);
                fields.push(f);
                values.push(v);
            }
            for (name, value) in [
                (
                    "shaft_span",
                    span.as_ref()
                        .map(|s| format!("{:.3} m – {:.3} m", s.z.0, s.z.1)),
                ),
                (
                    "shaft_area",
                    span.as_ref()
                        .map(|s| format!("{:.3} m²", s.boundary.area())),
                ),
            ] {
                fields.push(text_field(name));
                values.push(value.map(Value::String));
                read_only.push(name.to_owned());
            }
        }
        // A vertical opening shows the element that hosts it and the area it cuts.
        types::VERTICAL_OPENING => {
            let host = object
                .header
                .owner
                .and_then(|id| spatial::get(snapshot, id))
                .and_then(|w| types::text(&w.values, 0).map(str::to_owned));
            let area = archbuilder_model::floor::Floor::read(
                &object.values,
                archbuilder_model::floor::VERTICAL_SLOTS,
            )
            .and_then(|f| f.boundary_in(snapshot))
            .map(|b| format!("{:.3} m²", b.area()));
            fields.truncate(types::vertical_opening::VERTICES);
            values.truncate(types::vertical_opening::VERTICES);
            for (name, value) in [("cut_host", host), ("cut_area", area)] {
                fields.push(text_field(name));
                values.push(value.map(Value::String));
                read_only.push(name.to_owned());
            }
        }
        // A roof by extrusion shows its plane's direction and distances, its type and what derives
        // from it; the profile is edited by the command.
        types::ROOF_EXTRUSION => {
            let derived = archbuilder_model::roof_extrusion::Derived::of(snapshot, object).ok();
            let kind =
                archbuilder_model::element_type::type_of(snapshot, TypeKind::Roof, &object.values)
                    .and_then(|t| types::text(&t.values, 0).map(str::to_owned));
            fields.truncate(types::roof_extrusion::ROOF_TYPE);
            values.truncate(types::roof_extrusion::ROOF_TYPE);
            read_only.push("thickness".to_owned());
            for (name, value) in [
                ("extrusion_type", kind),
                (
                    "extrusion_area",
                    derived
                        .as_ref()
                        .map(|d| format!("{:.3} m²", d.surface_area())),
                ),
                (
                    "extrusion_volume",
                    derived.as_ref().map(|d| format!("{:.3} m³", d.volume())),
                ),
            ] {
                fields.push(text_field(name));
                values.push(value.map(Value::String));
                read_only.push(name.to_owned());
            }
        }
        // A floor edge shows its floor, the length it runs along and the volume of its body; the
        // edges are chosen by the command, the profile edited there.
        types::FLOOR_EDGE => {
            let host = object
                .header
                .owner
                .and_then(|id| spatial::get(snapshot, id))
                .and_then(|w| types::text(&w.values, 0).map(str::to_owned));
            let derived = archbuilder_model::floor_edge::Derived::of(snapshot, object);
            fields.truncate(types::floor_edge::PROFILE);
            values.truncate(types::floor_edge::PROFILE);
            for (name, value) in [
                ("edge_floor", host),
                (
                    "edge_length",
                    derived.as_ref().map(|d| format!("{:.3} m", d.length())),
                ),
                (
                    "edge_volume",
                    derived.as_ref().map(|d| format!("{:.4} m³", d.volume())),
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
        // The slope unit of a file from before it is degrees, shown as such.
        PROJECT => {
            if let Some(slot) = values.get_mut(types::project::SLOPE_UNIT)
                && slot.is_none()
            {
                *slot = Some(Value::Enum(
                    archbuilder_model::units::SlopeUnit::Degrees.code(),
                ));
            }
        }
        // The building's base elevation has no effect yet: the levels carry the elevations.
        BUILDING => read_only.push("elevation".to_owned()),
        _ => {}
    }
    // Every element shows its pinned flag last, whatever the arms above kept of its record.
    if let Some(at) = types::pinned_slot(key)
        && !fields.iter().any(|f| f.name == "pinned")
        && let Some(decl) = decl.schema.fields.get(at)
    {
        fields.push(decl.clone());
        values.push(object.values.get(at).cloned().flatten());
    }
    Some(Shown {
        key: key.to_owned(),
        fields,
        values,
        read_only,
    })
}

/// The rows of a roof: its type, base offset and overhang, one slope for its sloped edges, then
/// each edge (whether it slopes, its slope, its own overhang), and what derives from it.
fn roof_rows(
    (snapshot, label): (&Snapshot, &dyn Fn(&str) -> String),
    object: &Object,
    fields: &mut Vec<FieldDecl>,
    values: &mut Vec<Option<Value>>,
    read_only: &mut Vec<String>,
) {
    use archbuilder_model::types::roof as rf;
    let derived = archbuilder_model::roof::Derived::of(snapshot, object).ok();
    let roof = archbuilder_model::roof::Roof::from_values(&object.values);
    let kept = archbuilder_model::roof::Roof::kept_slopes(&object.values);
    let unit = spatial::project(snapshot).map_or_else(Default::default, |p| {
        archbuilder_model::units::slope_unit(&p.values)
    });
    let text = |a: f64| archbuilder_model::units::slope_text(a, unit, '.');
    let (decls, vals) = (fields.clone(), values.clone());
    let order = [rf::NAME, rf::THICKNESS, rf::BASE_OFFSET, rf::OVERHANG];
    *fields = order
        .iter()
        .filter_map(|k| decls.get(*k).cloned())
        .collect();
    *values = order.iter().filter_map(|k| vals.get(*k).cloned()).collect();
    with_type(
        (snapshot, label),
        TypeKind::Roof,
        object,
        fields,
        values,
        read_only,
    );
    // One slope when the sloped edges share it, else a note that they differ.
    let slopes: Vec<f64> = roof
        .iter()
        .flat_map(|r| r.edges.iter().filter_map(|e| e.slope))
        .collect();
    let shared = slopes
        .first()
        .copied()
        .filter(|a| slopes.iter().all(|b| (a - b).abs() < 1e-9));
    fields.push(FieldDecl::new("roof_slope", ValueType::String).in_aspect("geometry"));
    values.push(Some(Value::String(
        shared.map_or_else(|| label("roof_slopes_mixed"), text),
    )));
    let length = |name: String| {
        FieldDecl::new(&name, ValueType::Quantity(Dimension::LENGTH))
            .optional()
            .in_aspect("geometry")
    };
    for (k, edge) in roof.iter().flat_map(|r| r.edges.iter().enumerate()) {
        fields.push(
            FieldDecl::new(format!("roof_edge_sloped_{k}"), ValueType::Bool).in_aspect("geometry"),
        );
        values.push(Some(Value::Bool(edge.slope.is_some())));
        let slope = edge.slope.or_else(|| kept.get(k).copied());
        fields.push(
            FieldDecl::new(format!("roof_edge_slope_{k}"), ValueType::String).in_aspect("geometry"),
        );
        values.push(slope.map(|a| Value::String(text(a))));
        fields.push(length(format!("roof_edge_overhang_{k}")));
        // The overhang the edge has: its own, else the roof's.
        values
            .push(Some(types::length(edge.overhang.unwrap_or_else(|| {
                roof.as_ref().map_or(0.0, |r| r.overhang)
            }))));
    }
    let storey = object
        .header
        .owner
        .and_then(|id| spatial::get(snapshot, id))
        .and_then(|s| types::text(&s.values, storey::NAME).map(str::to_owned));
    let attached = spatial::objects(snapshot, WALL)
        .iter()
        .filter(|w| archbuilder_model::wall::attachment(&w.values) == Some(object.id))
        .count();
    let follows: Vec<String> = roof
        .as_ref()
        .map(archbuilder_model::roof::Roof::walls)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|w| spatial::get(snapshot, w))
        .filter_map(|w| types::text(&w.values, 0).map(str::to_owned))
        .collect();
    for (name, value) in [
        ("roof_storey", storey),
        (
            "roof_area",
            derived
                .as_ref()
                .map(|d| format!("{:.3} m²", d.surface.plan_area())),
        ),
        (
            "roof_top",
            derived
                .as_ref()
                .map(|d| format!("{:.3} m", d.surface.top())),
        ),
        ("roof_attached", Some(attached.to_string())),
        (
            "roof_follows",
            Some(follows.join(", ")).filter(|w| !w.is_empty()),
        ),
    ] {
        fields.push(text_field(name));
        values.push(value.map(Value::String));
        read_only.push(name.to_owned());
    }
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
        let label_of = |name: &str| label_text(localize, name);
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
        // With nothing selected, the grid shows the settings of the open plan view.
        let open_view: Vec<arc_base::ObjectId> = cx
            .tab
            .filter(|_| view.selection().is_empty())
            .and_then(|t| crate::plan_state::current_in(t, &snapshot))
            .into_iter()
            .collect();
        // A storey shown as its default view: its settings are those of a new view, and the first
        // edit makes the view (`archbuilder.view.set`).
        let default_view = open_view
            .first()
            .copied()
            .filter(|id| spatial::type_of(&snapshot, *id).as_deref() == Some(STOREY));
        let shown_ids = if open_view.is_empty() {
            view.selection()
        } else {
            open_view.as_slice()
        };
        let items: Vec<(Shown, &arc_base::ObjectId)> = shown_ids
            .iter()
            .filter(|id| !covered.contains(id))
            .filter_map(|id| {
                if default_view == Some(*id) {
                    let storey = spatial::get(&snapshot, *id)?;
                    let settings = archbuilder_model::view::settings_of(&snapshot, &storey)?;
                    let object = Object::new(
                        *id,
                        storey.version,
                        arc_doc::Header {
                            owner: Some(*id),
                            ..arc_doc::Header::default()
                        },
                        settings.values(),
                    );
                    return Some((shown(&snapshot, types::PLAN_VIEW, &object, &label_of)?, id));
                }
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
        let label = |name: &str| label_text(localize, name);
        let context = GridContext {
            localize,
            label: &label,
            units: &units,
            decimal_separator: crate::labels::decimal_separator(cx.language),
            edit_command: PROPERTY_SET,
        };
        let grid = &mut self.grid;
        let note = if default_view.is_some() {
            Some(say(cx.localize, &DEFAULT_VIEW))
        } else {
            matches!(first_key.as_str(), BUILDING | LEVEL).then(|| say(cx.localize, &NOT_EDITABLE))
        };
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
            // An edit of a storey's default view sets the view, made by the same step.
            let intent = match default_view {
                Some(storey) => Intent::new(archbuilder_commands::view::VIEW_SET)
                    .with("view", storey.to_string())
                    .with(
                        "field",
                        intent.params.get("field").cloned().unwrap_or_default(),
                    )
                    .with(
                        "value",
                        intent.params.get("value").cloned().unwrap_or_default(),
                    ),
                None => intent,
            };
            cx.actions.push(Action::Intent(intent));
        }
    }
}
