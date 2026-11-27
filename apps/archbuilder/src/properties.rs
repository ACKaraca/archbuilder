//! The property grid: the fields of the selected objects with readable names and values, lengths
//! in the project's unit. A storey shows its level's elevation as its own. A wall, floor, door or
//! window shows its type as a choice second (one choice retypes the whole selection) and the
//! values its type decides read-only. An edit becomes an
//! intent for `archbuilder.property.set`.

use arc_app::shell::{Action, Panel, PanelContext};
use arc_base::Dimension;
use arc_doc::schema::FieldDecl;
use arc_doc::{Bound, Object, Snapshot, Value, ValueType};
use arc_ui::egui::{ScrollArea, Ui};
use arc_ui::{GridContext, Intent, Localize, PropertyGrid, PropertyModel, PropertyObject};
use archbuilder_commands::project_parameter::FIELD_PREFIX as PARAMETER_FIELD;
use archbuilder_commands::structure::PROPERTY_SET;
use archbuilder_model::element_type::{self as et, TypeKind};
use archbuilder_model::expr::{DataType, Evaluated, Stored};
use archbuilder_model::spatial;
use archbuilder_model::types::{
    self, BUILDING, DOOR, ELEVATION_RANGE, FLOOR, LEVEL, PROJECT, ROOF, ROOM, SITE, STAIR, STOREY,
    WALL, WINDOW, level, storey,
};
use archbuilder_model::units::UnitKind;
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
        "parameters",
        Message {
            message_id: "ab-prop-parameters",
        },
    ),
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
        "show_railings",
        Message {
            message_id: "ab-prop-view-show-railings",
        },
    ),
    (
        "show_ramps",
        Message {
            message_id: "ab-prop-view-show-ramps",
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
        "area_unit",
        Message {
            message_id: "ab-prop-area-unit",
        },
    ),
    (
        "volume_unit",
        Message {
            message_id: "ab-prop-volume-unit",
        },
    ),
    (
        "angle_unit",
        Message {
            message_id: "ab-prop-angle-unit",
        },
    ),
    (
        "length_precision",
        Message {
            message_id: "ab-prop-length-precision",
        },
    ),
    (
        "area_precision",
        Message {
            message_id: "ab-prop-area-precision",
        },
    ),
    (
        "volume_precision",
        Message {
            message_id: "ab-prop-volume-precision",
        },
    ),
    (
        "angle_precision",
        Message {
            message_id: "ab-prop-angle-precision",
        },
    ),
    (
        "slope_precision",
        Message {
            message_id: "ab-prop-slope-precision",
        },
    ),
    (
        "length_rounding",
        Message {
            message_id: "ab-prop-length-rounding",
        },
    ),
    (
        "area_rounding",
        Message {
            message_id: "ab-prop-area-rounding",
        },
    ),
    (
        "volume_rounding",
        Message {
            message_id: "ab-prop-volume-rounding",
        },
    ),
    (
        "angle_rounding",
        Message {
            message_id: "ab-prop-angle-rounding",
        },
    ),
    (
        "slope_rounding",
        Message {
            message_id: "ab-prop-slope-rounding",
        },
    ),
    (
        "square_meter",
        Message {
            message_id: "ab-unit-square-meter",
        },
    ),
    (
        "square_centimeter",
        Message {
            message_id: "ab-unit-square-centimeter",
        },
    ),
    (
        "square_millimeter",
        Message {
            message_id: "ab-unit-square-millimeter",
        },
    ),
    (
        "square_foot",
        Message {
            message_id: "ab-unit-square-foot",
        },
    ),
    (
        "cubic_meter",
        Message {
            message_id: "ab-unit-cubic-meter",
        },
    ),
    (
        "liter",
        Message {
            message_id: "ab-unit-liter",
        },
    ),
    (
        "cubic_foot",
        Message {
            message_id: "ab-unit-cubic-foot",
        },
    ),
    (
        "cubic_yard",
        Message {
            message_id: "ab-unit-cubic-yard",
        },
    ),
    (
        "degree",
        Message {
            message_id: "ab-unit-degree",
        },
    ),
    (
        "radian",
        Message {
            message_id: "ab-unit-radian",
        },
    ),
    (
        "gradian",
        Message {
            message_id: "ab-unit-gradian",
        },
    ),
    (
        "status",
        Message {
            message_id: "ab-prop-status",
        },
    ),
    (
        "issue_date",
        Message {
            message_id: "ab-prop-issue-date",
        },
    ),
    (
        "author",
        Message {
            message_id: "ab-prop-author",
        },
    ),
    (
        "organization",
        Message {
            message_id: "ab-prop-organization",
        },
    ),
    (
        "description",
        Message {
            message_id: "ab-prop-description",
        },
    ),
    (
        "long_name",
        Message {
            message_id: "ab-prop-long-name",
        },
    ),
    (
        "occupancy_type",
        Message {
            message_id: "ab-prop-occupancy-type",
        },
    ),
    (
        "year_of_construction",
        Message {
            message_id: "ab-prop-year-of-construction",
        },
    ),
    (
        "construction_method",
        Message {
            message_id: "ab-prop-construction-method",
        },
    ),
    (
        "base_x",
        Message {
            message_id: "ab-prop-base-x",
        },
    ),
    (
        "base_y",
        Message {
            message_id: "ab-prop-base-y",
        },
    ),
    (
        "base_z",
        Message {
            message_id: "ab-prop-base-z",
        },
    ),
    (
        "survey_x",
        Message {
            message_id: "ab-prop-survey-x",
        },
    ),
    (
        "survey_y",
        Message {
            message_id: "ab-prop-survey-y",
        },
    ),
    (
        "survey_z",
        Message {
            message_id: "ab-prop-survey-z",
        },
    ),
    (
        "true_north",
        Message {
            message_id: "ab-prop-true-north",
        },
    ),
    (
        "map_scale",
        Message {
            message_id: "ab-prop-map-scale",
        },
    ),
    (
        "crs",
        Message {
            message_id: "ab-prop-crs",
        },
    ),
    (
        "eastings",
        Message {
            message_id: "ab-prop-eastings",
        },
    ),
    (
        "northings",
        Message {
            message_id: "ab-prop-northings",
        },
    ),
    (
        "survey_height",
        Message {
            message_id: "ab-prop-survey-height",
        },
    ),
    (
        "precision_distance",
        Message {
            message_id: "ab-prop-precision-distance",
        },
    ),
    (
        "north",
        Message {
            message_id: "ab-prop-view-north",
        },
    ),
    (
        "project_north",
        Message {
            message_id: "ab-prop-north-project",
        },
    ),
    (
        "face",
        Message {
            message_id: "ab-prop-work-plane-face",
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
        "bulge",
        Message {
            message_id: "ab-prop-wall-bulge",
        },
    ),
    (
        "lean_exterior",
        Message {
            message_id: "ab-prop-wall-lean-exterior",
        },
    ),
    (
        "lean_interior",
        Message {
            message_id: "ab-prop-wall-lean-interior",
        },
    ),
    (
        "wall_profile",
        Message {
            message_id: "ab-prop-wall-profile",
        },
    ),
    (
        "wall_bands",
        Message {
            message_id: "ab-prop-wall-bands",
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
        archbuilder_model::types::COMPONENT,
        Message {
            message_id: "ab-kind-component",
        },
    ),
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
        types::MODEL_VIEW,
        Message {
            message_id: "ab-kind-model-view",
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
        types::ELEVATOR,
        Message {
            message_id: "ab-kind-elevator",
        },
    ),
    (
        types::DORMER,
        Message {
            message_id: "ab-kind-dormer",
        },
    ),
    (
        types::ROOF_EDGE,
        Message {
            message_id: "ab-kind-roof-edge",
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

/// Labels of the fields of elevators, dormers, roof edge elements and stairs by sketch.
const ROOF_LABELS: [(&str, Message); 24] = [
    (
        "car_width",
        Message {
            message_id: "ab-prop-car-width",
        },
    ),
    (
        "car_depth",
        Message {
            message_id: "ab-prop-car-depth",
        },
    ),
    (
        "clearance",
        Message {
            message_id: "ab-prop-clearance",
        },
    ),
    (
        "door_width",
        Message {
            message_id: "ab-prop-door-width",
        },
    ),
    (
        "pit",
        Message {
            message_id: "ab-prop-pit",
        },
    ),
    (
        "overrun",
        Message {
            message_id: "ab-prop-overrun",
        },
    ),
    (
        "style",
        Message {
            message_id: "ab-prop-style",
        },
    ),
    (
        "wall_height",
        Message {
            message_id: "ab-prop-wall-height",
        },
    ),
    (
        "pitch",
        Message {
            message_id: "ab-prop-pitch",
        },
    ),
    (
        "role",
        Message {
            message_id: "ab-prop-role",
        },
    ),
    (
        "headroom",
        Message {
            message_id: "ab-prop-headroom",
        },
    ),
    (
        "direction",
        Message {
            message_id: "ab-prop-direction",
        },
    ),
    (
        "elevator_stops",
        Message {
            message_id: "ab-prop-elevator-stops",
        },
    ),
    (
        "elevator_shaft",
        Message {
            message_id: "ab-prop-elevator-shaft",
        },
    ),
    (
        "elevator_span",
        Message {
            message_id: "ab-prop-elevator-span",
        },
    ),
    (
        "dormer_roof",
        Message {
            message_id: "ab-prop-dormer-roof",
        },
    ),
    (
        "dormer_opening",
        Message {
            message_id: "ab-prop-dormer-opening",
        },
    ),
    (
        "dormer_volume",
        Message {
            message_id: "ab-prop-dormer-volume",
        },
    ),
    (
        "roof_edge_roof",
        Message {
            message_id: "ab-prop-roof-edge-roof",
        },
    ),
    (
        "gable",
        Message {
            message_id: "ab-prop-gable",
        },
    ),
    (
        "shed",
        Message {
            message_id: "ab-prop-shed",
        },
    ),
    (
        "fascia",
        Message {
            message_id: "ab-prop-fascia",
        },
    ),
    (
        "gutter",
        Message {
            message_id: "ab-prop-gutter",
        },
    ),
    (
        "sketch",
        Message {
            message_id: "ab-prop-sketch",
        },
    ),
];

/// Labels of the fields and choices of model views (3D views, sections, elevations).
const VIEW_LABELS: [(&str, Message); 26] = [
    (
        "kind",
        Message {
            message_id: "ab-prop-mv-kind",
        },
    ),
    (
        "perspective",
        Message {
            message_id: "ab-prop-mv-perspective",
        },
    ),
    (
        "eye_x",
        Message {
            message_id: "ab-prop-mv-eye-x",
        },
    ),
    (
        "eye_y",
        Message {
            message_id: "ab-prop-mv-eye-y",
        },
    ),
    (
        "eye_z",
        Message {
            message_id: "ab-prop-mv-eye-z",
        },
    ),
    (
        "target_x",
        Message {
            message_id: "ab-prop-mv-target-x",
        },
    ),
    (
        "target_y",
        Message {
            message_id: "ab-prop-mv-target-y",
        },
    ),
    (
        "target_z",
        Message {
            message_id: "ab-prop-mv-target-z",
        },
    ),
    (
        "fov",
        Message {
            message_id: "ab-prop-mv-fov",
        },
    ),
    (
        "section_box",
        Message {
            message_id: "ab-prop-mv-section-box",
        },
    ),
    (
        "box_min_x",
        Message {
            message_id: "ab-prop-mv-box-min-x",
        },
    ),
    (
        "box_min_y",
        Message {
            message_id: "ab-prop-mv-box-min-y",
        },
    ),
    (
        "box_min_z",
        Message {
            message_id: "ab-prop-mv-box-min-z",
        },
    ),
    (
        "box_max_x",
        Message {
            message_id: "ab-prop-mv-box-max-x",
        },
    ),
    (
        "box_max_y",
        Message {
            message_id: "ab-prop-mv-box-max-y",
        },
    ),
    (
        "box_max_z",
        Message {
            message_id: "ab-prop-mv-box-max-z",
        },
    ),
    (
        "crop",
        Message {
            message_id: "ab-prop-mv-crop",
        },
    ),
    (
        "camera",
        Message {
            message_id: "ab-prop-mv-camera",
        },
    ),
    (
        "three_d_view",
        Message {
            message_id: "ab-prop-mv-three-d",
        },
    ),
    (
        "section_view",
        Message {
            message_id: "ab-prop-mv-section",
        },
    ),
    (
        "elevation_view",
        Message {
            message_id: "ab-prop-mv-elevation",
        },
    ),
    (
        "wireframe",
        Message {
            message_id: "ab-prop-mv-wireframe",
        },
    ),
    (
        "hidden_line",
        Message {
            message_id: "ab-prop-mv-hidden-line",
        },
    ),
    (
        "shaded",
        Message {
            message_id: "ab-prop-mv-shaded",
        },
    ),
    (
        "shaded_with_edges",
        Message {
            message_id: "ab-prop-mv-shaded-edges",
        },
    ),
    (
        "consistent_colors",
        Message {
            message_id: "ab-prop-mv-consistent",
        },
    ),
];

/// Labels of the fields of ramps and railings.
const CIRC_LABELS: [(&str, Message); 26] = [
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
        "ramp_runs",
        Message {
            message_id: "ab-prop-circ-ramp-runs",
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
        .chain(ROOF_LABELS.iter())
        .chain(VIEW_LABELS.iter())
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
    let us = spatial::unit_settings(snapshot);
    let mut fields = decl.schema.fields.clone();
    let mut values = object.values.clone();
    let mut read_only = Vec::new();
    match key {
        // The hidden elements of a model view are counted by the view, not listed.
        types::MODEL_VIEW
            if fields.len() > types::model_view::HIDDEN
                && values.len() > types::model_view::HIDDEN =>
        {
            fields.remove(types::model_view::HIDDEN);
            values.remove(types::model_view::HIDDEN);
        }
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
                Some(v) => v.map(|v| Value::String(us.volume_text(v, '.'))),
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
                let mut panel_text = format!("{panels} / {}", us.area_text(area, '.'));
                if orphans > 0 {
                    panel_text.push_str(&format!(" ({orphans} orphaned)"));
                }
                fields.push(text_field("curtain_panels"));
                values.push(Some(Value::String(panel_text)));
                read_only.push("curtain_panels".to_owned());
                fields.push(text_field("curtain_mullions"));
                values.push(Some(Value::String(format!(
                    "{mullions} / {}",
                    us.length_text(length, '.')
                ))));
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
            // The shape of the wall: the bulge of an arc and the lean of each face are edited
            // here; the profile and the bands are edited by their commands and shown as a count.
            for at in [
                types::wall::BULGE,
                types::wall::LEAN_EXTERIOR,
                types::wall::LEAN_INTERIOR,
            ] {
                if let Some(decl) = decl.schema.fields.get(at) {
                    fields.push(decl.clone());
                    values.push(object.values.get(at).cloned().flatten());
                }
            }
            if let Some(profile) = archbuilder_model::wall::Profile::of(&object.values) {
                let points: usize = profile.loops.iter().map(Vec::len).sum();
                fields.push(text_field("wall_profile"));
                values.push(Some(Value::String(format!(
                    "{points} / {}",
                    profile.loops.len().saturating_sub(1)
                ))));
                read_only.push("wall_profile".to_owned());
            }
            let bands = archbuilder_model::wall::Band::of(&object.values).len();
            if bands > 0 {
                fields.push(text_field("wall_bands"));
                values.push(Some(Value::String(bands.to_string())));
                read_only.push("wall_bands".to_owned());
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
                    let unit = spatial::unit_settings(snapshot);
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
                    derived.as_ref().map(|d| us.area_text(d.area(), '.')),
                ),
                (
                    "floor_volume",
                    derived.as_ref().map(|d| us.volume_text(d.volume(), '.')),
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
            let derived = |f: fn(&archbuilder_model::room::Entry) -> Option<f64>, kind| {
                entry
                    .as_ref()
                    .and_then(f)
                    .map(|v| Value::String(us.text(kind, v, '.')))
            };
            for (name, value) in [
                (
                    "room_area",
                    entry
                        .as_ref()
                        .and_then(|e| shape.as_ref().map(|s| s.area).or_else(|| e.area()))
                        .map(|a| Value::String(us.area_text(a, '.'))),
                ),
                (
                    "room_perimeter",
                    derived(archbuilder_model::room::Entry::perimeter, UnitKind::Length),
                ),
                (
                    "room_volume",
                    shape
                        .as_ref()
                        .map(|s| Value::String(us.volume_text(s.volume, '.')))
                        .or_else(|| {
                            derived(archbuilder_model::room::Entry::volume, UnitKind::Volume)
                        }),
                ),
                (
                    "room_height",
                    shape
                        .as_ref()
                        .map(|s| Value::String(us.length_text(s.height, '.'))),
                ),
                (
                    "room_unbounded",
                    shape
                        .as_ref()
                        .map(|s| Value::String(us.length_text(s.unbounded, '.'))),
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
                .map(|p| us.angle_text(p.stair.direction, '.'));
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
                st::HEADROOM,
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
                    .map(|r| us.length_text(r.length, '.'))
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
                ("stair_riser", layout.map(|l| us.length_text(l.riser, '.'))),
                ("stair_runs", runs),
                (
                    "stair_stride",
                    layout.map(|l| {
                        let warn = if l.stride_ok() { "" } else { " ⚠" };
                        format!("{}{warn}", us.length_text(l.stride(), '.'))
                    }),
                ),
                (
                    "stair_volume",
                    layout.map(|l| us.volume_text(l.volume(), '.')),
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
                ("ramp_rise", layout.map(|l| us.length_text(l.height, '.'))),
                (
                    "ramp_runs",
                    layout.filter(|l| l.slopes().len() > 1).map(|l| {
                        l.slopes()
                            .iter()
                            .map(|s| format!("{:.1} %", s * 100.0))
                            .collect::<Vec<_>>()
                            .join(" · ")
                    }),
                ),
                (
                    "ramp_volume",
                    layout.map(|l| us.volume_text(l.volume(), '.')),
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
                    derived.as_ref().map(|d| us.length_text(d.length(), '.')),
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
                        let unit = spatial::unit_settings(snapshot);
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
                    derived.as_ref().map(|d| us.area_text(d.area(), '.')),
                ),
                (
                    "ceiling_volume",
                    derived.as_ref().map(|d| us.volume_text(d.volume(), '.')),
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
                    span.as_ref().map(|s| {
                        format!(
                            "{} – {}",
                            us.length_text(s.z.0, '.'),
                            us.length_text(s.z.1, '.')
                        )
                    }),
                ),
                (
                    "shaft_area",
                    span.as_ref().map(|s| us.area_text(s.boundary.area(), '.')),
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
            .map(|b| us.area_text(b.area(), '.'));
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
                        .map(|d| us.area_text(d.surface_area(), '.')),
                ),
                (
                    "extrusion_volume",
                    derived.as_ref().map(|d| us.volume_text(d.volume(), '.')),
                ),
            ] {
                fields.push(text_field(name));
                values.push(value.map(Value::String));
                read_only.push(name.to_owned());
            }
        }
        // An elevator shows its sizes, the levels it stops at and the span of its shaft.
        types::ELEVATOR => {
            let derived = archbuilder_model::elevator::derived(snapshot, object);
            fields.truncate(types::elevator::TOP_LEVEL);
            values.truncate(types::elevator::TOP_LEVEL);
            for (name, value) in [
                (
                    "elevator_stops",
                    derived.as_ref().map(|d| d.stops.len().to_string()),
                ),
                (
                    "elevator_span",
                    derived
                        .as_ref()
                        .map(|d| format!("{:.2} m … {:.2} m", d.z().0, d.z().1)),
                ),
                (
                    "elevator_shaft",
                    derived.as_ref().map(|d| {
                        format!(
                            "{:.3} m²",
                            archbuilder_model::floor::signed_area(&d.elevator.shaft_outline())
                                .abs()
                        )
                    }),
                ),
            ] {
                fields.push(text_field(name));
                values.push(value.map(Value::String));
                read_only.push(name.to_owned());
            }
        }
        // A dormer shows its roof, the area of its opening and the volume of its parts.
        types::DORMER => {
            let host = object
                .header
                .owner
                .and_then(|id| spatial::get(snapshot, id))
                .and_then(|w| types::text(&w.values, 0).map(str::to_owned));
            let derived = archbuilder_model::dormer::Derived::of(snapshot, object).ok();
            fields.truncate(types::dormer::PINNED);
            values.truncate(types::dormer::PINNED);
            for (name, value) in [
                ("dormer_roof", host),
                (
                    "dormer_opening",
                    archbuilder_model::dormer::Dormer::from_values(&object.values).map(|d| {
                        format!(
                            "{:.3} m²",
                            archbuilder_model::floor::signed_area(&d.opening()).abs()
                        )
                    }),
                ),
                (
                    "dormer_volume",
                    derived.as_ref().map(|d| format!("{:.4} m³", d.volume())),
                ),
            ] {
                fields.push(text_field(name));
                values.push(value.map(Value::String));
                read_only.push(name.to_owned());
            }
        }
        // A roof edge element shows its role, its roof, the length it runs along and the volume of
        // its body; the edges are chosen by the command, the profile edited there.
        types::ROOF_EDGE => {
            let host = object
                .header
                .owner
                .and_then(|id| spatial::get(snapshot, id))
                .and_then(|w| types::text(&w.values, 0).map(str::to_owned));
            let derived = archbuilder_model::roof_edge::Derived::of(snapshot, object);
            fields.truncate(types::roof_edge::PROFILE);
            values.truncate(types::roof_edge::PROFILE);
            for (name, value) in [
                ("roof_edge_roof", host),
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
                    derived.as_ref().map(|d| us.length_text(d.length(), '.')),
                ),
                (
                    "edge_volume",
                    derived.as_ref().map(|d| us.volume_text(d.volume(), '.')),
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
    parameter_rows(
        snapshot,
        key,
        object,
        &mut fields,
        &mut values,
        &mut read_only,
    );
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

/// The field of a parameter's value shows its formula after `=`; this one shows what it gives.
const PARAMETER_RESULT: &str = "parameter_value:";

/// The label of a parameter row (`parameter:<identifier>`): the parameter's display name; of its
/// result row, the name marked as the value.
fn parameter_label(snapshot: &Snapshot, localize: &dyn Localize, name: &str) -> Option<String> {
    const VALUE: Message = Message {
        message_id: "ab-prop-parameter-value",
    };
    let (identifier, result) = match name.strip_prefix(PARAMETER_FIELD) {
        Some(i) => (i, false),
        None => (name.strip_prefix(PARAMETER_RESULT)?, true),
    };
    let shown = archbuilder_model::parameter::all(snapshot)
        .into_iter()
        .find(|p| p.identifier == identifier)
        .map_or_else(|| identifier.to_owned(), |p| p.name);
    if !result {
        return Some(shown);
    }
    Some(
        localize
            .message(
                VALUE.message_id,
                &[("name", arc_base::error::Value::Text(shown.clone()))],
            )
            .unwrap_or(shown),
    )
}

/// A number with at most six decimals, without trailing zeros.
fn short(v: f64) -> String {
    let text = format!("{v:.6}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// A parameter value as the grid shows it: lengths in the project's unit, areas, volumes and
/// angles with their unit, the rest as they are.
fn parameter_text(snapshot: &Snapshot, data: DataType, value: &Evaluated) -> String {
    match value {
        Evaluated::Number(v) => match data {
            DataType::Length => {
                archbuilder_model::units::length_text(*v, spatial::length_unit(snapshot), '.')
            }
            DataType::Area => format!("{} m²", short(*v)),
            DataType::Volume => format!("{} m³", short(*v)),
            DataType::Angle => format!("{}°", short(v.to_degrees())),
            _ => short(*v),
        },
        Evaluated::Bool(b) => b.to_string(),
        Evaluated::Text(t) => t.clone(),
    }
}

/// The project parameters of `object` as rows grouped by the parameters' groups: a value or
/// `=formula` that edits through `archbuilder.property.set`, and for a formula a read-only row
/// with what it gives. A value set on the type is read-only on the element. The stored list
/// itself is not shown.
fn parameter_rows(
    snapshot: &Snapshot,
    key: &str,
    object: &Object,
    fields: &mut Vec<FieldDecl>,
    values: &mut Vec<Option<Value>>,
    read_only: &mut Vec<String>,
) {
    use archbuilder_model::parameter;
    if let Some(at) = fields.iter().position(|f| f.name == "parameters") {
        fields.remove(at);
        if at < values.len() {
            values.remove(at);
        }
    }
    if parameter::field_at(snapshot, key).is_none() {
        return;
    }
    let variables = archbuilder_model::variable::evaluate(snapshot).unwrap_or_default();
    // A document whose values do not evaluate (one from a newer build) shows no rows; the
    // commands refuse to leave one in that state.
    let rows = parameter::evaluate(snapshot, object, &variables).unwrap_or_default();
    for row in rows {
        let p = &row.parameter;
        let aspect = if p.group.is_empty() {
            "parameters"
        } else {
            p.group.as_str()
        };
        let name = format!("{PARAMETER_FIELD}{}", p.identifier);
        let formula = row.stored.as_ref().and_then(Stored::formula);
        let text = match formula {
            Some(f) => format!("={}", f.text()),
            None if row.source == parameter::Source::Unset => String::new(),
            None => parameter_text(snapshot, p.data, &row.value),
        };
        fields.push(
            FieldDecl::new(&name, ValueType::String)
                .optional()
                .in_aspect(aspect),
        );
        values.push(Some(Value::String(text)));
        if !p.set_on(key) {
            read_only.push(name);
        }
        if formula.is_some() {
            let result = format!("{PARAMETER_RESULT}{}", p.identifier);
            fields.push(
                FieldDecl::new(&result, ValueType::String)
                    .optional()
                    .in_aspect(aspect),
            );
            values.push(Some(Value::String(parameter_text(
                snapshot, p.data, &row.value,
            ))));
            read_only.push(result);
        }
    }
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
    let us = spatial::unit_settings(snapshot);
    let text = |a: f64| us.slope_text(a, '.');
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
                .map(|d| us.area_text(d.surface.plan_area(), '.')),
        ),
        (
            "roof_top",
            derived
                .as_ref()
                .map(|d| us.length_text(d.surface.top(), '.')),
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
        let snapshot_for_labels = tab.document().snapshot();
        let label_of = |name: &str| {
            parameter_label(&snapshot_for_labels, localize, name)
                .unwrap_or_else(|| label_text(localize, name))
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
        // With nothing selected, the grid shows the settings of the open plan view.
        let open_view: Vec<arc_base::ObjectId> = cx
            .tab
            .filter(|_| view.selection().is_empty())
            .and_then(|t| {
                crate::plan_state::current_in(t, &snapshot)
                    .or_else(|| crate::plan_state::model_in(t, &snapshot))
            })
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
        let units = spatial::unit_settings(&snapshot).display_units();
        let localize = cx.localize;
        let label = |name: &str| {
            parameter_label(&snapshot, localize, name).unwrap_or_else(|| label_text(localize, name))
        };
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use std::sync::Arc;

    use arc_command::{Invocation, Invoker, Permission, Session, Surface};
    use arc_doc::graph::Evaluators;
    use arc_doc::{Document, DocumentConfig};
    use serde_json::json;

    use super::*;

    #[test]
    fn parameters_show_as_rows_with_formulas_and_their_values() {
        let document = Document::open(
            Snapshot::empty(Arc::new(archbuilder_commands::types().unwrap())),
            Arc::new(Evaluators::new()),
            DocumentConfig {
                workers: 0,
                ..DocumentConfig::default()
            },
        )
        .unwrap();
        let registry = archbuilder_commands::registry().unwrap();
        let session = Session::new(Surface::Ui, [Permission::Read, Permission::Edit]);
        let run = |command: &str, params: serde_json::Value| {
            Invoker::new(&registry)
                .invoke(&document, &session, Invocation::new(command, params))
                .unwrap()
                .value
        };
        run("archbuilder.project.new", json!({}));
        let wall = run(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [4.0, 0.0]}),
        )["wall"]
            .as_str()
            .unwrap()
            .to_owned();
        run(
            "archbuilder.parameter.create",
            json!({"name": "Mark", "data_type": "number", "categories": [WALL],
                   "formula": "2 * 3", "group": "Marks"}),
        );
        run(
            "archbuilder.parameter.create",
            json!({"name": "Cost", "data_type": "number", "categories": [WALL], "scope": "type"}),
        );
        let snapshot = document.snapshot();
        let object = spatial::get(&snapshot, wall.parse().unwrap()).unwrap();
        let shown = shown(&snapshot, WALL, &object, &|n: &str| n.to_owned()).unwrap();
        let at = |name: &str| shown.fields.iter().position(|f| f.name == name);
        assert!(at("parameters").is_none());
        let mark = at("parameter:Mark").unwrap();
        assert_eq!(shown.fields[mark].aspect, "Marks");
        assert_eq!(shown.values[mark], Some(Value::String("=2 * 3".into())));
        let result = at("parameter_value:Mark").unwrap();
        assert_eq!(shown.values[result], Some(Value::String("6".into())));
        assert!(shown.read_only.contains(&"parameter_value:Mark".to_owned()));
        // A type-scope value is edited on the type, not on the wall.
        assert!(at("parameter:Cost").is_some());
        assert!(shown.read_only.contains(&"parameter:Cost".to_owned()));
        assert!(!shown.read_only.contains(&"parameter:Mark".to_owned()));
    }
}
