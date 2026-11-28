//! The object types of the spatial structure, as the domain model document defines them:
//! `Project`, `Site`, `Building`, `Storey` and `Level`, the `Wall` that stands on a storey and the
//! `Door` and `Window` that a wall hosts, and the element types (`WallType`, `FloorType`,
//! `DoorType`, `WindowType`) that the project owns and instances refer to.
//!
//! Containment is the header owner of an object: a site is owned by the project, a building by
//! the site, a storey and a level by their building, a wall by its storey. Lengths are stored in
//! meters. A storey refers to its level; the reference blocks the deletion of the level while the
//! storey exists.

use arc_base::{Dimension, Quantity, Result};
use arc_doc::{
    Bound, Cardinality, EnumVariant, FieldDecl, Finding, OnCopy, OnDelete, ReferenceSpec,
    RelationshipRule, Schema, SchemaVersion, Severity, Strength, TypeDecl, TypeKey, TypeRegistry,
    Value, ValueType,
};

use crate::error::codes;

/// The longest name, in bytes.
pub const NAME_BYTES: u64 = 256;
/// The lowest and highest level elevation, in meters.
pub const ELEVATION_RANGE: (f64, f64) = (-1000.0, 10_000.0);

/// The type key of the project.
pub const PROJECT: &str = "archbuilder.project";
/// The type key of the site.
pub const SITE: &str = "archbuilder.site";
/// The type key of the building.
pub const BUILDING: &str = "archbuilder.building";
/// The type key of the storey.
pub const STOREY: &str = "archbuilder.storey";
/// The type key of the level.
pub const LEVEL: &str = "archbuilder.level";

/// The type key of the plan view of a storey.
pub const PLAN_VIEW: &str = "archbuilder.plan_view";

/// The type key of a model view other than a plan: a saved 3D view, a section or an elevation
/// (see [`crate::model_view`]).
pub const MODEL_VIEW: &str = "archbuilder.model_view";

/// The type key of a selection set: a named list of elements, owned by the project.
pub const SELECTION_SET: &str = "archbuilder.selection_set";

/// The type key of a document variable: a named value formulas read, owned by the project.
pub const VARIABLE: &str = "archbuilder.variable";

/// The type key of a project parameter: a definition bound to categories of physical elements,
/// owned by the project.
pub const PARAMETER: &str = "archbuilder.parameter";

/// The type key of a loaded family: a copy of a published definition (its canonical text, version
/// and checksum), owned by the project.
pub const FAMILY: &str = "archbuilder.family";
/// The type key of a family type: a named value set of a loaded family, owned by the project.
pub const FAMILY_TYPE: &str = "archbuilder.family_type";
/// The type key of a component: a freestanding instance of a family type on a storey.
pub const COMPONENT: &str = "archbuilder.component";

/// The type key of a hosted family instance: a family type placed in a host element (a wall).
pub const FAMILY_INSERT: &str = "archbuilder.family_insert";
/// A model group definition (AB-PHASE-010): the member records of one repeated unit in
/// definition-local coordinates, owned by the definition. Contained in the project.
pub const GROUP: &str = "archbuilder.group";
/// A placed instance of a group definition (AB-PHASE-010, AB-PHASE-011): its placement, level
/// mapping, exclusions and exceptions. Its members are generated elements of the storeys.
pub const GROUP_INSTANCE: &str = "archbuilder.group_instance";

/// The type key of the wall.
pub const WALL: &str = "archbuilder.wall";

/// The type key of the floor (slab).
pub const FLOOR: &str = "archbuilder.floor";

/// The type key of the door.
pub const DOOR: &str = "archbuilder.door";
/// The type key of the window.
pub const WINDOW: &str = "archbuilder.window";

/// The type key of the room.
pub const ROOM: &str = "archbuilder.room";

/// A stair on a storey: runs and landings from the storey's level up to the level above.
pub const STAIR: &str = "archbuilder.stair";

/// A railing by path on a storey: a sketched path (lines and arcs, with heights) or a side of a
/// stair or ramp it is hosted on; a type, extensions at its ends and a continuation.
pub const RAILING: &str = "archbuilder.railing";
/// A railing type: height, top rail and baluster profiles and spacing, posts, corner join; owned
/// by the project.
pub const RAILING_TYPE: &str = "archbuilder.railing_type";
/// A straight ramp on a storey: a run with landings from the storey's level up to the level above
/// (or a rise); hosts railings.
pub const RAMP: &str = "archbuilder.ramp";
/// A ramp type: the largest slope and the default thickness; owned by the project.
pub const RAMP_TYPE: &str = "archbuilder.ramp_type";

/// A roof by footprint on a storey: edges with slopes and overhangs, a base offset and a type.
pub const ROOF: &str = "archbuilder.roof";

/// A ceiling on a storey: a boundary, a height above the level, a type and optionally a slope.
pub const CEILING: &str = "archbuilder.ceiling";
/// A rectangular or arched opening cut in a wall, without a door or window; hosted by the wall.
pub const WALL_OPENING: &str = "archbuilder.wall_opening";
/// A vertical shaft: one footprint cut through every floor, roof and ceiling between a base and a
/// top; owned by the storey it starts on.
pub const SHAFT: &str = "archbuilder.shaft";
/// A vertical opening: a sketched cut in one floor, roof or ceiling, which hosts it.
pub const VERTICAL_OPENING: &str = "archbuilder.vertical_opening";
/// A roof by extrusion on a storey: an open profile sketched in a vertical plane, extruded along
/// the plane's normal between two distances, thickened normal to the profile.
pub const ROOF_EXTRUSION: &str = "archbuilder.roof_extrusion";
/// An edge element: a profile swept along edges of one floor; the floor owns it.
pub const FLOOR_EDGE: &str = "archbuilder.floor_edge";
/// An elevator on its base storey: a car in a shaft that passes the floors between its base and
/// its top level and stops at the levels it serves.
pub const ELEVATOR: &str = "archbuilder.elevator";
/// A dormer: a gable or shed dormer on one roof, which cuts the roof and owns its walls and its
/// small roof as derived geometry; the roof owns the record.
pub const DORMER: &str = "archbuilder.dormer";
/// A roof edge element: a fascia or gutter profile swept along sloped edges of one roof; the roof
/// owns the record.
pub const ROOF_EDGE: &str = "archbuilder.roof_edge";
/// A ceiling type: layers, a core, the height above the level and the grid pattern; owned by the
/// project.
pub const CEILING_TYPE: &str = "archbuilder.ceiling_type";

/// A wall type: layers, a core and the function flags; owned by the project.
pub const WALL_TYPE: &str = "archbuilder.wall_type";
/// A floor type: layers and a core; owned by the project.
pub const FLOOR_TYPE: &str = "archbuilder.floor_type";
/// A door type: the size, the frame, the operation and the default sill; owned by the project.
pub const DOOR_TYPE: &str = "archbuilder.door_type";
/// A window type, with the layout of a door type.
pub const WINDOW_TYPE: &str = "archbuilder.window_type";

/// A roof type: layers and a core, from the top down; owned by the project.
pub const ROOF_TYPE: &str = "archbuilder.roof_type";

/// A grid line: a labeled line (straight or an arc) drawn in plan views and in 3D, with an
/// extent across levels. Contained in a building.
pub const GRID: &str = "archbuilder.grid";
/// A named reference plane: a vertical plane through a line, bounded or not, with a reference
/// strength. Contained in a building.
pub const REFERENCE_PLANE: &str = "archbuilder.reference_plane";
/// A work plane: the plane placement and sketching work on, from a level, a grid, a reference
/// plane or three points. Contained in a building.
pub const WORK_PLANE: &str = "archbuilder.work_plane";
/// The type key of a foreign element: a product of an imported file that has no native
/// counterpart, kept with its class, its source identity and its geometry as a mesh (read-only
/// reference element, AB-IO-018).
pub const FOREIGN_ELEMENT: &str = "archbuilder.foreign_element";

/// The element types, in the order of [`crate::element_type::TypeKind::ALL`].
pub const TYPE_KEYS: [&str; 6] = [
    WALL_TYPE,
    FLOOR_TYPE,
    DOOR_TYPE,
    WINDOW_TYPE,
    ROOF_TYPE,
    CEILING_TYPE,
];

/// The building elements: what is selected, copied, hidden, moved and deleted as an object of a
/// storey (doors and windows through their wall). One list, so a new kind is added once.
pub const ELEMENTS: [&str; 24] = [
    COMPONENT,
    FAMILY_INSERT,
    WALL,
    DOOR,
    WINDOW,
    FLOOR,
    ROOM,
    STAIR,
    RAILING,
    RAMP,
    ROOF,
    CEILING,
    WALL_OPENING,
    SHAFT,
    VERTICAL_OPENING,
    FLOOR_EDGE,
    ROOF_EXTRUSION,
    GRID,
    REFERENCE_PLANE,
    ELEVATOR,
    DORMER,
    ROOF_EDGE,
    GROUP_INSTANCE,
    FOREIGN_ELEMENT,
];

/// The physical elements: the elements that carry project parameter values in their
/// `parameters` field (every element but grids and reference planes).
pub const PARAMETER_HOSTS: [&str; 20] = [
    COMPONENT,
    FAMILY_INSERT,
    WALL,
    DOOR,
    WINDOW,
    FLOOR,
    ROOM,
    STAIR,
    RAILING,
    RAMP,
    ROOF,
    CEILING,
    WALL_OPENING,
    SHAFT,
    VERTICAL_OPENING,
    FLOOR_EDGE,
    ROOF_EXTRUSION,
    ELEVATOR,
    DORMER,
    ROOF_EDGE,
];

/// The datums a building owns directly: they have no storey and span levels.
pub const DATUMS: [&str; 3] = [GRID, REFERENCE_PLANE, WORK_PLANE];

/// Whether `key` is one of [`DATUMS`].
#[must_use]
pub fn is_datum(key: &str) -> bool {
    DATUMS.contains(&key)
}

/// The elements a storey owns directly.
pub const STOREY_ELEMENTS: [&str; 13] = [
    COMPONENT,
    WALL,
    FLOOR,
    ROOM,
    STAIR,
    RAILING,
    RAMP,
    ROOF,
    CEILING,
    SHAFT,
    ROOF_EXTRUSION,
    ELEVATOR,
    GROUP_INSTANCE,
];

/// The elements that host a vertical opening.
pub const VERTICAL_HOSTS: [&str; 3] = [FLOOR, ROOF, CEILING];

/// Whether `key` is one of [`ELEMENTS`].
#[must_use]
pub fn is_element(key: &str) -> bool {
    ELEMENTS.contains(&key)
}

/// Whether `key` is one of [`STOREY_ELEMENTS`].
#[must_use]
pub fn is_storey_element(key: &str) -> bool {
    STOREY_ELEMENTS.contains(&key)
}

/// Every type key of the model, from the root down.
pub const KEYS: [&str; 46] = [
    FOREIGN_ELEMENT,
    FAMILY,
    FAMILY_TYPE,
    COMPONENT,
    FAMILY_INSERT,
    PROJECT,
    SITE,
    BUILDING,
    STOREY,
    LEVEL,
    WALL,
    DOOR,
    WINDOW,
    FLOOR,
    ROOM,
    STAIR,
    ROOF,
    WALL_TYPE,
    FLOOR_TYPE,
    DOOR_TYPE,
    WINDOW_TYPE,
    ROOF_TYPE,
    CEILING,
    WALL_OPENING,
    SHAFT,
    VERTICAL_OPENING,
    CEILING_TYPE,
    PLAN_VIEW,
    SELECTION_SET,
    FLOOR_EDGE,
    ROOF_EXTRUSION,
    RAILING,
    RAILING_TYPE,
    RAMP,
    RAMP_TYPE,
    GRID,
    REFERENCE_PLANE,
    WORK_PLANE,
    ELEVATOR,
    DORMER,
    ROOF_EDGE,
    VARIABLE,
    PARAMETER,
    MODEL_VIEW,
    GROUP,
    GROUP_INSTANCE,
];

/// The range of an insert's distance along its wall, in meters.
pub const DISTANCE_RANGE: (f64, f64) = (0.0, 100_000.0);
/// The range of an insert's width, in meters.
pub const INSERT_WIDTH_RANGE: (f64, f64) = (0.1, 20.0);
/// The range of an insert's height, in meters.
pub const INSERT_HEIGHT_RANGE: (f64, f64) = (0.1, 20.0);
/// The range of a window's sill height, in meters.
pub const SILL_RANGE: (f64, f64) = (0.0, 100.0);
/// The range of the width of an insert's frame bars, in meters.
pub const FRAME_WIDTH_RANGE: (f64, f64) = (0.01, 0.5);
/// The range of the depth of an insert's frame across the wall, in meters.
pub const FRAME_DEPTH_RANGE: (f64, f64) = (0.01, 2.0);

/// The range of a wall's baseline coordinates, in meters.
pub const COORDINATE_RANGE: (f64, f64) = (-100_000.0, 100_000.0);
/// The range of a wall's thickness, in meters.
pub const THICKNESS_RANGE: (f64, f64) = (0.001, 5.0);
/// The range of a wall's height above its base, in meters.
pub const HEIGHT_RANGE: (f64, f64) = (0.01, 1000.0);
/// The range of a wall's base offset from its storey's level, in meters.
pub const BASE_OFFSET_RANGE: (f64, f64) = (-1000.0, 1000.0);
/// The shortest baseline of a wall, in meters; a shorter one is a zero-length wall.
pub const MIN_WALL_LENGTH: f64 = 0.001;
/// The thickness of a wall created without one, in meters.
pub const DEFAULT_THICKNESS: f64 = 0.2;
/// The range of the thickness of the layers outside a wall's core on one side, in meters.
pub const THICKNESS_RANGE_OR_ZERO: (f64, f64) = (0.0, 5.0);

/// Field positions of `archbuilder.family`.
pub mod family {
    /// The name.
    pub const NAME: usize = 0;
    /// The family identifier (text).
    pub const ID: usize = 1;
    /// The version.
    pub const VERSION: usize = 2;
    /// The category.
    pub const CATEGORY: usize = 3;
    /// The hosting kind.
    pub const HOSTING: usize = 4;
    /// The checksum.
    pub const CHECKSUM: usize = 5;
    /// The IFC entity.
    pub const IFC_ENTITY: usize = 6;
    /// The IFC predefined type.
    pub const IFC_TYPE: usize = 7;
    /// The source.
    pub const SOURCE: usize = 8;
    /// The license.
    pub const LICENSE: usize = 9;
    /// The canonical text.
    pub const TEXT: usize = 10;
}

/// Field positions of `archbuilder.family_type`.
pub mod family_type {
    /// The name.
    pub const NAME: usize = 0;
    /// The family record.
    pub const FAMILY: usize = 1;
    /// The values, keyed by parameter key.
    pub const VALUES: usize = 2;
}

/// Field positions of `archbuilder.component`.
pub mod component {
    /// The name (a mark).
    pub const NAME: usize = 0;
    /// The family type.
    pub const FAMILY_TYPE: usize = 1;
    /// The distance above the storey's level, in meters.
    pub const OFFSET: usize = 2;
    /// The x coordinate of the insertion point, in meters.
    pub const X: usize = 3;
    /// The y coordinate of the insertion point, in meters.
    pub const Y: usize = 4;
    /// The rotation about the vertical axis, in radians.
    pub const ROTATION: usize = 5;
    /// Whether the component is mirrored across its own left-right axis.
    pub const MIRROR: usize = 6;
    /// The instance values, keyed by parameter key.
    pub const VALUES: usize = 7;
}

/// Field positions of `archbuilder.family_insert`; the host is the header owner.
pub mod family_insert {
    /// The name (a mark).
    pub const NAME: usize = 0;
    /// The family type.
    pub const FAMILY_TYPE: usize = 1;
    /// The distance along the host's baseline to the insertion point, in meters.
    pub const DISTANCE: usize = 2;
    /// The height of the insertion point above the host's base, in meters.
    pub const OFFSET: usize = 3;
    /// The hand.
    pub const HAND: usize = 4;
    /// The facing.
    pub const FACING: usize = 5;
    /// The instance values, keyed by parameter key.
    pub const VALUES: usize = 6;
}

/// Field positions of `archbuilder.project`.
pub mod project {
    /// The project name.
    pub const NAME: usize = 0;
    /// The project number.
    pub const NUMBER: usize = 1;
    /// The client.
    pub const CLIENT: usize = 2;
    /// The address.
    pub const ADDRESS: usize = 3;
    /// The length unit lengths are shown and typed in ([`crate::units::LengthUnit`] code).
    pub const LENGTH_UNIT: usize = 4;
    /// Whether the document is a template.
    pub const TEMPLATE: usize = 5;
    /// How slopes are shown and typed ([`crate::units::SlopeUnit`] code); appended in schema 1.1.
    pub const SLOPE_UNIT: usize = 6;
    /// The x of the project base point, in internal coordinates, in meters; appended in schema
    /// 1.2 with the rest of the location fields (see [`crate::coordinates`]), all optional.
    pub const BASE_X: usize = 7;
    /// The y of the project base point.
    pub const BASE_Y: usize = 8;
    /// The elevation of the project base point.
    pub const BASE_Z: usize = 9;
    /// The x of the survey point, in internal coordinates.
    pub const SURVEY_X: usize = 10;
    /// The y of the survey point.
    pub const SURVEY_Y: usize = 11;
    /// The elevation of the survey point.
    pub const SURVEY_Z: usize = 12;
    /// The angle from project north to true north, counterclockwise.
    pub const TRUE_NORTH: usize = 13;
    /// The scale factor from model to map lengths; 1 when absent.
    pub const SCALE: usize = 14;
    /// The coordinate reference system: an EPSG code (`EPSG:3067`) or a WKT text.
    pub const CRS: usize = 15;
    /// The eastings of the survey point in the coordinate reference system, in meters.
    pub const EASTINGS: usize = 16;
    /// The northings of the survey point, in meters.
    pub const NORTHINGS: usize = 17;
    /// The height of the survey point above the datum of the reference system, in meters.
    pub const HEIGHT: usize = 18;
    /// The distance from the internal origin beyond which geometry is reported, in meters.
    pub const PRECISION_DISTANCE: usize = 19;
    /// The unit areas are shown in ([`crate::units::AreaUnit`] code); appended in schema 1.3 with
    /// the rest of the display unit fields, all optional.
    pub const AREA_UNIT: usize = 20;
    /// The unit volumes are shown in ([`crate::units::VolumeUnit`] code).
    pub const VOLUME_UNIT: usize = 21;
    /// The unit angles are shown in ([`crate::units::AngleUnit`] code).
    pub const ANGLE_UNIT: usize = 22;
    /// The decimals of lengths.
    pub const LENGTH_PRECISION: usize = 23;
    /// The decimals of areas.
    pub const AREA_PRECISION: usize = 24;
    /// The decimals of volumes.
    pub const VOLUME_PRECISION: usize = 25;
    /// The decimals of angles.
    pub const ANGLE_PRECISION: usize = 26;
    /// The decimals of slopes.
    pub const SLOPE_PRECISION: usize = 27;
    /// The rounding increment of lengths, in the length unit.
    pub const LENGTH_ROUNDING: usize = 28;
    /// The rounding increment of areas, in the area unit.
    pub const AREA_ROUNDING: usize = 29;
    /// The rounding increment of volumes, in the volume unit.
    pub const VOLUME_ROUNDING: usize = 30;
    /// The rounding increment of angles, in the angle unit.
    pub const ANGLE_ROUNDING: usize = 31;
    /// The rounding increment of slopes, in the slope unit.
    pub const SLOPE_ROUNDING: usize = 32;
    /// The project status, a short phase name (`Design`, `Permit`); appended in schema 1.4 with
    /// the rest of the project information, all optional.
    pub const STATUS: usize = 33;
    /// The issue date, `YYYY-MM-DD`.
    pub const ISSUE_DATE: usize = 34;
    /// The author of the project information.
    pub const AUTHOR: usize = 35;
    /// The organization that issues the project.
    pub const ORGANIZATION: usize = 36;
    /// A description of the project.
    pub const DESCRIPTION: usize = 37;
    /// The long name of the project.
    pub const LONG_NAME: usize = 38;
    /// What the building is used for, in words.
    pub const OCCUPANCY_TYPE: usize = 39;
    /// The year the building is or was built, four digits.
    pub const YEAR_OF_CONSTRUCTION: usize = 40;
    /// How the building is constructed, in words.
    pub const CONSTRUCTION_METHOD: usize = 41;
    /// The file name of the template the project was made from (AB-DATUM-001).
    pub const TEMPLATE_SOURCE: usize = 42;
}

/// The longest project information text, in bytes.
pub const INFO_BYTES: u64 = 128;
/// The longest project description, in bytes.
pub const DESCRIPTION_BYTES: u64 = 1024;

/// Field positions of `archbuilder.site`.
pub mod site {
    /// The site name.
    pub const NAME: usize = 0;
}

/// Field positions of `archbuilder.building`.
pub mod building {
    /// The building name.
    pub const NAME: usize = 0;
    /// The elevation of the building's base point, in meters.
    pub const ELEVATION: usize = 1;
    /// The address.
    pub const ADDRESS: usize = 2;
}

/// Field positions of `archbuilder.storey`.
pub mod storey {
    /// The storey name.
    pub const NAME: usize = 0;
    /// The reference to the storey's level.
    pub const LEVEL: usize = 1;
}

/// Field positions of `archbuilder.level`.
pub mod level {
    /// The level name.
    pub const NAME: usize = 0;
    /// The elevation, in meters.
    pub const ELEVATION: usize = 1;
    /// What the elevation is measured from ([`ELEVATION_BASE_PROJECT`] or
    /// [`ELEVATION_BASE_SURVEY`]).
    pub const ELEVATION_BASE: usize = 2;
    /// Whether the level has a storey.
    pub const STOREY: usize = 3;
}

/// Field positions of `archbuilder.wall`.
pub mod wall {
    /// The wall name.
    pub const NAME: usize = 0;
    /// The x coordinate of the baseline's start, in meters.
    pub const START_X: usize = 1;
    /// The y coordinate of the baseline's start, in meters.
    pub const START_Y: usize = 2;
    /// The x coordinate of the baseline's end, in meters.
    pub const END_X: usize = 3;
    /// The y coordinate of the baseline's end, in meters.
    pub const END_Y: usize = 4;
    /// The thickness, in meters.
    pub const THICKNESS: usize = 5;
    /// The height above the base, in meters.
    pub const HEIGHT: usize = 6;
    /// The base's distance above the storey's level, in meters.
    pub const BASE_OFFSET: usize = 7;
    /// The location line ([`crate::wall::LocationLine`] code).
    pub const LOCATION_LINE: usize = 8;
    /// The level the wall's top is constrained to; absent for an unconnected wall, whose top is
    /// at the base plus the stored height.
    pub const TOP_LEVEL: usize = 9;
    /// How the start end joins ([`crate::wall::JoinStyle`] code); appended in schema 1.1.
    pub const START_JOIN: usize = 10;
    /// How the end end joins ([`crate::wall::JoinStyle`] code); appended in schema 1.1.
    pub const END_JOIN: usize = 11;
    /// The wall type; appended in schema 1.2, absent in older files until they are upgraded.
    pub const WALL_TYPE: usize = 12;
    /// The thickness of the type's layers outside the core on the exterior side, in meters: a
    /// copy of the type kept by the commands; appended in schema 1.2.
    pub const CORE_EXTERIOR: usize = 13;
    /// The same on the interior side; appended in schema 1.2.
    pub const CORE_INTERIOR: usize = 14;
    /// Whether the wall is pinned: the modify commands refuse to change it.
    pub const PINNED: usize = 15;
    /// The distance of the top above its top level, in meters; appended in schema 1.4.
    pub const TOP_OFFSET: usize = 16;
    /// The roof or floor the top is attached to; appended in schema 1.4. While it is set the
    /// top follows that element's underside and the top level and height wait unchanged.
    pub const TOP_ATTACH: usize = 17;
    /// The authored grid-line edits of a curtain wall: a list of records (see
    /// [`crate::curtain::grid_edit`]).
    pub const GRID_EDITS: usize = 18;
    /// The authored per-cell panel overrides of a curtain wall: a list of records (see
    /// [`crate::curtain::cell_override`]).
    pub const CELL_OVERRIDES: usize = 19;
    /// The bulge of the baseline (see [`crate::wall::Wall::bulge`]): 0 or absent for a straight
    /// wall; appended in schema 1.6.
    pub const BULGE: usize = 20;
    /// The lean of the exterior face from the vertical, in radians; appended in schema 1.6.
    pub const LEAN_EXTERIOR: usize = 21;
    /// The lean of the interior face from the vertical, in radians; appended in schema 1.6.
    pub const LEAN_INTERIOR: usize = 22;
    /// The sketched elevation profile: a list of records (see [`crate::wall::profile_point`]);
    /// appended in schema 1.6.
    pub const PROFILE: usize = 23;
    /// The bands of the wall: a list of records (see [`crate::wall::band_record`]); appended in
    /// schema 1.6.
    pub const BANDS: usize = 24;
}

/// Field positions of `archbuilder.door` and `archbuilder.window`, which share one layout.
pub mod insert {
    /// The name.
    pub const NAME: usize = 0;
    /// The distance from the wall's baseline start to the insert's center, in meters.
    pub const DISTANCE: usize = 1;
    /// The width of the opening, in meters.
    pub const WIDTH: usize = 2;
    /// The height of the opening, in meters.
    pub const HEIGHT: usize = 3;
    /// The sill height above the wall's base, in meters; zero for a door.
    pub const SILL: usize = 4;
    /// The width of the frame bars, in meters.
    pub const FRAME_WIDTH: usize = 5;
    /// The depth of the frame across the wall, in meters.
    pub const FRAME_DEPTH: usize = 6;
    /// The operation ([`crate::insert::Operation`] code).
    pub const OPERATION: usize = 7;
    /// The hand ([`crate::insert::Hand`] code).
    pub const HAND: usize = 8;
    /// The facing ([`crate::insert::Facing`] code).
    pub const FACING: usize = 9;
    /// The door or window type; appended in schema 1.1.
    pub const INSERT_TYPE: usize = 10;
    /// Whether the door or window is pinned.
    pub const PINNED: usize = 11;
}

/// Field positions of `archbuilder.wall_type` and `archbuilder.floor_type` (a floor type has the
/// first four).
pub mod layered_type {
    /// The name.
    pub const NAME: usize = 0;
    /// The layers, from the exterior face (a wall) or the top (a floor).
    pub const LAYERS: usize = 1;
    /// The index of the first core layer.
    pub const CORE_START: usize = 2;
    /// The index after the last core layer.
    pub const CORE_END: usize = 3;
    /// Walls: the function ([`crate::element_type::Exposure`] code).
    pub const EXPOSURE: usize = 4;
    /// Walls: whether the wall carries loads.
    pub const LOAD_BEARING: usize = 5;
    /// Walls: whether the wall bounds rooms.
    pub const ROOM_BOUNDING: usize = 6;
    /// Walls: the sides whose finish layers wrap around free ends ([`crate::element_type::Wrap`]
    /// code); appended in schema 1.1, absent in older files, which wrap nothing.
    pub const WRAP_ENDS: usize = 7;
    /// Walls: the sides whose finish layers wrap around the openings of doors and windows.
    pub const WRAP_INSERTS: usize = 8;

    /// Field positions of one layer record.
    pub mod layer {
        /// The function ([`crate::element_type::Function`] code).
        pub const FUNCTION: usize = 0;
        /// The material name.
        pub const MATERIAL: usize = 1;
        /// The thickness, in meters.
        pub const THICKNESS: usize = 2;
    }
}

/// Field positions of `archbuilder.door_type` and `archbuilder.window_type`.
pub mod insert_type {
    /// The name.
    pub const NAME: usize = 0;
    /// The width of the opening, in meters.
    pub const WIDTH: usize = 1;
    /// The height of the opening, in meters.
    pub const HEIGHT: usize = 2;
    /// The width of the frame bars, in meters.
    pub const FRAME_WIDTH: usize = 3;
    /// The depth of the frame, in meters.
    pub const FRAME_DEPTH: usize = 4;
    /// The operation ([`crate::insert::Operation`] code).
    pub const OPERATION: usize = 5;
    /// The sill height a new instance gets, in meters.
    pub const SILL: usize = 6;
}

/// Field positions of `archbuilder.floor`.
pub mod floor {
    /// The floor name.
    pub const NAME: usize = 0;
    /// The thickness of the slab, in meters.
    pub const THICKNESS: usize = 1;
    /// The top's distance above the storey's level, in meters.
    pub const TOP_OFFSET: usize = 2;
    /// The boundary vertices of all loops ([`crate::floor::Vertex`] records).
    pub const VERTICES: usize = 3;
    /// The floor type; appended in schema 1.1.
    pub const FLOOR_TYPE: usize = 4;
    /// Whether the floor is pinned.
    pub const PINNED: usize = 5;
    /// The arc edges: records of a vertex number and a bulge; appended in schema 1.3, absent in
    /// older files, whose edges are all straight.
    pub const ARCS: usize = 6;
    /// The x of a point on the slope arrow's start line; absent for a flat floor; appended in
    /// schema 1.4. The top offset is the height of the top at that line.
    pub const SLOPE_X: usize = 7;
    /// The y of that point.
    pub const SLOPE_Y: usize = 8;
    /// The direction of the slope arrow, an angle from the x axis.
    pub const SLOPE_DIRECTION: usize = 9;
    /// The length of the arrow, in meters.
    pub const SLOPE_LENGTH: usize = 10;
    /// The slope as an angle; absent when the end height gives it.
    pub const SLOPE: usize = 11;
    /// The top's height above the level at the arrow's end, in meters.
    pub const END_HEIGHT: usize = 12;
    /// The slots of the slope arrow.
    pub const ARROW: crate::ceiling::ArrowSlots = crate::ceiling::ArrowSlots {
        x: SLOPE_X,
        y: SLOPE_Y,
        direction: SLOPE_DIRECTION,
        length: SLOPE_LENGTH,
        slope: SLOPE,
        end_height: END_HEIGHT,
    };

    /// Field positions of one vertex record.
    pub mod vertex {
        /// The loop: 0 outer, 1 and up holes.
        pub const LOOP: usize = 0;
        /// The x coordinate, in meters.
        pub const X: usize = 1;
        /// The y coordinate, in meters.
        pub const Y: usize = 2;
        /// The wall the edge from this vertex follows; absent for a free edge.
        pub const WALL: usize = 3;
        /// The face of that wall ([`crate::floor::Face`] code).
        pub const FACE: usize = 4;
        /// The offset from that face, in meters.
        pub const OFFSET: usize = 5;
    }
}

/// Field positions of `archbuilder.room`.
pub mod room {
    /// The name.
    pub const NAME: usize = 0;
    /// The number.
    pub const NUMBER: usize = 1;
    /// The x coordinate of the placement point, in meters.
    pub const X: usize = 2;
    /// The y coordinate of the placement point, in meters.
    pub const Y: usize = 3;
    /// The department.
    pub const DEPARTMENT: usize = 4;
    /// The floor finish.
    pub const FLOOR_FINISH: usize = 5;
    /// The wall finish.
    pub const WALL_FINISH: usize = 6;
    /// The ceiling finish.
    pub const CEILING_FINISH: usize = 7;
    /// The base finish.
    pub const BASE_FINISH: usize = 8;
    /// The level that limits the room above; absent for the level above its storey.
    pub const UPPER_LEVEL: usize = 9;
    /// The offset of the top from that level, in meters.
    pub const UPPER_OFFSET: usize = 10;
    /// The offset of the base from the level of the storey, in meters.
    pub const BASE_OFFSET: usize = 11;
    /// Whether the room is pinned.
    pub const PINNED: usize = 12;
}

/// Field positions of `archbuilder.roof`.
pub mod roof {
    /// The name.
    pub const NAME: usize = 0;
    /// The thickness of the slab, a copy of the type's total, in meters.
    pub const THICKNESS: usize = 1;
    /// The height of the underside at the eaves above the storey's level, in meters.
    pub const BASE_OFFSET: usize = 2;
    /// The edges of the footprint ([`crate::roof::Edge`] records).
    pub const EDGES: usize = 3;
    /// The overhang of the edges without their own, in meters.
    pub const OVERHANG: usize = 4;
    /// The roof type.
    pub const ROOF_TYPE: usize = 5;
    /// Whether the roof is pinned.
    pub const PINNED: usize = 6;

    /// Field positions of one edge record.
    pub mod edge {
        /// The x coordinate of the edge's start, in meters.
        pub const X: usize = 0;
        /// The y coordinate, in meters.
        pub const Y: usize = 1;
        /// The wall the edge follows; absent for a free edge.
        pub const WALL: usize = 2;
        /// The face of that wall ([`crate::floor::Face`] code).
        pub const FACE: usize = 3;
        /// The offset from that face, in meters.
        pub const OFFSET: usize = 4;
        /// Whether the edge defines a slope.
        pub const SLOPED: usize = 5;
        /// The slope, an angle; kept while the edge defines none.
        pub const SLOPE: usize = 6;
        /// The edge's own overhang, in meters; absent takes the roof's.
        pub const OVERHANG: usize = 7;
    }
}

/// Field positions of `archbuilder.plan_view`.
pub mod plan_view {
    /// The name.
    pub const NAME: usize = 0;
    /// The cut plane's height above the storey's level, in meters.
    pub const CUT: usize = 1;
    /// The top of the view range above the level; absent takes the next level, else 3 m.
    pub const TOP: usize = 2;
    /// Whether what lies between the cut and the top is drawn (overhead).
    pub const OVERHEAD: usize = 3;
    /// The scale denominator (100 is 1:100).
    pub const SCALE: usize = 4;
    /// The detail level: coarse, medium or fine.
    pub const DETAIL: usize = 5;
    /// Whether the storey below is shown faint under the plan.
    pub const UNDERLAY: usize = 6;
    /// The first category flag (see [`crate::view::Category`]); one flag per category follows, as
    /// many as [`CATEGORY_COUNT`]. Categories added later are at the end of the record.
    pub const CATEGORIES: usize = 7;
    /// The number of category flags that stand together after the underlay.
    pub const CATEGORY_COUNT: usize = 9;
    /// The elements hidden in this view (records with one optional reference each).
    pub const HIDDEN: usize = 16;
    /// The field of the reference inside one entry of [`HIDDEN`].
    pub const HIDDEN_OBJECT: usize = 0;
    /// The bottom of the view range relative to the level, in meters (0 is the level).
    pub const BOTTOM: usize = 17;
    /// The view depth relative to the level, in meters: elements down to it draw beyond the cut.
    pub const DEPTH: usize = 18;
    /// The grids whose extent or heads this view overrides (records, see [`crate::view::DatumView`]);
    /// appended in schema 1.1.
    pub const DATUM_VIEWS: usize = 19;
    /// Which north the view is oriented to ([`crate::view::North`] code); appended in schema 1.2.
    pub const NORTH: usize = 20;
    /// Whether railings are drawn; appended in schema 1.3.
    pub const SHOW_RAILINGS: usize = 21;
    /// Whether ramps are drawn; appended in schema 1.3.
    pub const SHOW_RAMPS: usize = 22;
    /// Whether components are drawn; appended in schema 1.4.
    pub const SHOW_COMPONENTS: usize = 23;
}

/// Field positions of `archbuilder.variable`.
pub mod variable {
    /// The display name.
    pub const NAME: usize = 0;
    /// The formula identifier.
    pub const IDENTIFIER: usize = 1;
    /// The data type.
    pub const DATA_TYPE: usize = 2;
    /// The stored value: a literal or a formula.
    pub const VALUE: usize = 3;
}

/// Field positions of `archbuilder.parameter`.
pub mod parameter {
    /// The display name.
    pub const NAME: usize = 0;
    /// The formula identifier.
    pub const IDENTIFIER: usize = 1;
    /// The data type.
    pub const DATA_TYPE: usize = 2;
    /// Type or instance.
    pub const SCOPE: usize = 3;
    /// The bound categories (element type keys).
    pub const CATEGORIES: usize = 4;
    /// The group it is shown in.
    pub const GROUP: usize = 5;
    /// The order within the group.
    pub const ORDER: usize = 6;
    /// The default value.
    pub const DEFAULT: usize = 7;
    /// The rule.
    pub const RULE: usize = 8;
}

/// Field positions of `archbuilder.model_view` (see [`crate::model_view::ModelView`]).
pub mod model_view {
    /// The name.
    pub const NAME: usize = 0;
    /// What the view shows ([`crate::model_view::ViewKind`] code).
    pub const KIND: usize = 1;
    /// The scale denominator.
    pub const SCALE: usize = 2;
    /// The detail level.
    pub const DETAIL: usize = 3;
    /// The visual style ([`crate::model_view::VisualStyle`] code).
    pub const STYLE: usize = 4;
    /// The elements hidden in this view (records with one optional reference each).
    pub const HIDDEN: usize = 5;
    /// The first category flag, one per [`crate::view::Category::ALL`] in that order.
    pub const CATEGORIES: usize = 6;
    /// Whether the camera is a perspective one.
    pub const PERSPECTIVE: usize = 17;
    /// The eye's x; y and z follow.
    pub const EYE: usize = 18;
    /// The target's x; y and z follow.
    pub const TARGET: usize = 21;
    /// The vertical field of view.
    pub const FOV: usize = 24;
    /// Whether the section box crops the view.
    pub const SECTION_BOX: usize = 25;
    /// The section box's lowest x; y and z follow.
    pub const BOX_MIN: usize = 26;
    /// The section box's highest x; y and z follow.
    pub const BOX_MAX: usize = 29;
    /// The x of the start of the line of a section or elevation; y follows.
    pub const START: usize = 32;
    /// The x of the end of the line; y follows.
    pub const END: usize = 34;
    /// The far clip distance behind the cut.
    pub const DEPTH: usize = 36;
    /// Whether the drawing is cropped.
    pub const CROP: usize = 37;
    /// The lowest elevation drawn.
    pub const BOTTOM: usize = 38;
    /// The highest elevation drawn.
    pub const TOP: usize = 39;
    /// Whether components are drawn; appended in schema 1.1 after the other fields.
    pub const SHOW_COMPONENTS: usize = 40;
}

/// Field positions of `archbuilder.selection_set`.
pub mod selection_set {
    /// The name.
    pub const NAME: usize = 0;
    /// The members (records with one optional reference each).
    pub const MEMBERS: usize = 1;
    /// The field of the reference inside one entry of [`MEMBERS`].
    pub const MEMBER_OBJECT: usize = 0;
}

/// The most members one selection set holds.
pub const MAX_SET_MEMBERS: usize = 100_000;

/// The range of a plan view's cut height and top, in meters above the level.
pub const VIEW_HEIGHT_RANGE: (f64, f64) = (0.05, 1000.0);
/// The range of a plan view's bottom and view depth, in meters relative to the level.
pub const VIEW_DEPTH_RANGE: (f64, f64) = (-1000.0, 1000.0);
/// The range of a plan view's scale denominator.
pub const VIEW_SCALE_RANGE: (f64, f64) = (1.0, 10_000.0);
/// The most elements one view hides.
pub const MAX_HIDDEN: usize = 100_000;
/// The most datum overrides one view holds.
pub const MAX_PER_VIEW_DATUMS: usize = 20_000;

/// Field positions of `archbuilder.ceiling`. The first seven are those of a floor, so the
/// boundary is read and edited by the floor's code.
pub mod ceiling {
    /// The name.
    pub const NAME: usize = 0;
    /// The thickness of the body above the underside, a copy of the type's total, in meters.
    pub const THICKNESS: usize = 1;
    /// The underside's height above the storey's level, in meters.
    pub const HEIGHT_OFFSET: usize = 2;
    /// The boundary vertices ([`crate::floor::Vertex`] records).
    pub const VERTICES: usize = 3;
    /// The ceiling type.
    pub const CEILING_TYPE: usize = 4;
    /// Whether the ceiling is pinned.
    pub const PINNED: usize = 5;
    /// The arc edges, as in a floor.
    pub const ARCS: usize = 6;
    /// The grid pattern's origin offset along x, in meters.
    pub const GRID_X: usize = 7;
    /// The grid pattern's origin offset along y, in meters.
    pub const GRID_Y: usize = 8;
    /// The grid pattern's rotation added to the type's angle.
    pub const GRID_ROTATION: usize = 9;
    /// The x of a point on the slope arrow's start line; absent for a flat ceiling.
    pub const SLOPE_X: usize = 10;
    /// The y of that point.
    pub const SLOPE_Y: usize = 11;
    /// The direction of the slope arrow, an angle from the x axis.
    pub const SLOPE_DIRECTION: usize = 12;
    /// The length of the slope arrow, in meters.
    pub const SLOPE_LENGTH: usize = 13;
    /// The slope as an angle; absent when the end height gives it.
    pub const SLOPE: usize = 14;
    /// The underside's height above the level at the arrow's end, in meters.
    pub const END_HEIGHT: usize = 15;
    /// The slots of the slope arrow.
    pub const ARROW: crate::ceiling::ArrowSlots = crate::ceiling::ArrowSlots {
        x: SLOPE_X,
        y: SLOPE_Y,
        direction: SLOPE_DIRECTION,
        length: SLOPE_LENGTH,
        slope: SLOPE,
        end_height: END_HEIGHT,
    };
}

/// Field positions of `archbuilder.wall_opening`.
pub mod wall_opening {
    /// The name.
    pub const NAME: usize = 0;
    /// The distance from the wall's baseline start to the center of the opening, in meters.
    pub const DISTANCE: usize = 1;
    /// The width, in meters.
    pub const WIDTH: usize = 2;
    /// The height of the opening's highest point above its sill (the rise included), in meters.
    pub const HEIGHT: usize = 3;
    /// The sill's height above the wall's base, in meters.
    pub const SILL: usize = 4;
    /// The rise of the arched top, in meters; 0 for a rectangle.
    pub const RISE: usize = 5;
    /// Whether the opening is pinned.
    pub const PINNED: usize = 6;
    /// The sketched outline: points in meters from the opening's lower left corner, x along the
    /// baseline and y up; appended in schema 1.1, absent for a rectangle or an arch.
    pub const PROFILE: usize = 7;
}

/// Field positions of `archbuilder.shaft`.
pub mod shaft {
    /// The name.
    pub const NAME: usize = 0;
    /// The base's offset from its level, in meters.
    pub const BASE_OFFSET: usize = 1;
    /// The top's offset from its level, in meters.
    pub const TOP_OFFSET: usize = 2;
    /// The footprint vertices.
    pub const VERTICES: usize = 3;
    /// The arc edges.
    pub const ARCS: usize = 4;
    /// Whether the shaft is pinned.
    pub const PINNED: usize = 5;
    /// The level of the base; the level of the shaft's storey when absent.
    pub const BASE_LEVEL: usize = 6;
    /// The level of the top; the level above the shaft's storey when absent.
    pub const TOP_LEVEL: usize = 7;
}

/// Field positions of `archbuilder.roof_extrusion`.
pub mod roof_extrusion {
    /// The name.
    pub const NAME: usize = 0;
    /// The thickness normal to the profile: a copy of the type's total, in meters.
    pub const THICKNESS: usize = 1;
    /// The start of the extrusion: its distance from the plane along the plane's normal, in
    /// meters.
    pub const START: usize = 2;
    /// The end of the extrusion, in the same distance.
    pub const END: usize = 3;
    /// The direction of the plane's horizontal axis, an angle from the x axis.
    pub const DIRECTION: usize = 4;
    /// The roof type (the same position as a roof's, for the readers of types).
    pub const ROOF_TYPE: usize = 5;
    /// Whether the roof is pinned.
    pub const PINNED: usize = 6;
    /// The x of the plane's origin, in meters.
    pub const X: usize = 7;
    /// The y of the plane's origin, in meters.
    pub const Y: usize = 8;
    /// The profile: records of `u` (along the plane's axis) and `z` (above the storey's level).
    pub const PROFILE: usize = 9;
    /// The work plane the roof was sketched on; while it is set and vertical the roof's plane
    /// follows it. Appended in schema 1.1.
    pub const WORK_PLANE: usize = 10;
}

/// Field positions of `archbuilder.floor_edge`.
pub mod floor_edge {
    /// The name.
    pub const NAME: usize = 0;
    /// The profile: records of `u` (outward from the edge) and `v` (above the top of the slab).
    pub const PROFILE: usize = 1;
    /// The edges of the floor's boundary, by the number of the vertex each starts at (a list of
    /// whole numbers).
    pub const EDGES: usize = 2;
    /// Whether the edge element is pinned.
    pub const PINNED: usize = 3;
}

/// Field positions of `archbuilder.grid`.
pub mod grid {
    /// The label.
    pub const NAME: usize = 0;
    /// The x coordinate of the start, in meters.
    pub const START_X: usize = 1;
    /// The y coordinate of the start, in meters.
    pub const START_Y: usize = 2;
    /// The x coordinate of the end, in meters.
    pub const END_X: usize = 3;
    /// The y coordinate of the end, in meters.
    pub const END_Y: usize = 4;
    /// The bulge of the line: 0 for a straight line, else the tangent of a quarter of the arc's
    /// included angle (as a floor edge).
    pub const BULGE: usize = 5;
    /// Whether the start end shows its head (bubble and label).
    pub const HEAD_START: usize = 6;
    /// Whether the end shows its head.
    pub const HEAD_END: usize = 7;
    /// The lowest level the grid reaches; the lowest level of the building when absent.
    pub const BOTTOM_LEVEL: usize = 8;
    /// The highest level the grid reaches; the highest level of the building when absent.
    pub const TOP_LEVEL: usize = 9;
    /// Whether the grid is pinned.
    pub const PINNED: usize = 10;
    /// The vertices between the start and the end of a multi-segment grid: records of `x`, `y`
    /// and the `bulge` of the segment that arrives at the vertex (the grid's own bulge is that of
    /// the last segment); appended in schema 1.1, absent for a grid of one segment.
    pub const PATH: usize = 11;
}

/// Field positions of `archbuilder.foreign_element`.
pub mod foreign_element {
    /// The name.
    pub const NAME: usize = 0;
    /// The entity class of the source file in its schema's spelling (`IfcColumn`).
    pub const CLASS: usize = 1;
    /// The predefined type of the source element; `USERDEFINED` types carry their text in
    /// [`OBJECT_TYPE`].
    pub const PREDEFINED_TYPE: usize = 2;
    /// The object type text of the source element.
    pub const OBJECT_TYPE: usize = 3;
    /// The tag of the source element.
    pub const TAG: usize = 4;
    /// The geometry of the source element as an encoded mesh in project coordinates, meters; absent
    /// when the file gave none or the foundation could not evaluate it.
    pub const MESH: usize = 5;
    /// The schema identifier of the source file (`IFC4`).
    pub const SOURCE_SCHEMA: usize = 6;
}

/// The name and version of the encoding of the mesh of a foreign element.
pub const FOREIGN_MESH_ENCODING: (&str, u32) = ("arc.mesh", 1);

/// Field positions of `archbuilder.reference_plane`.
pub mod reference_plane {
    /// The name.
    pub const NAME: usize = 0;
    /// The x coordinate of the start, in meters.
    pub const START_X: usize = 1;
    /// The y coordinate of the start, in meters.
    pub const START_Y: usize = 2;
    /// The x coordinate of the end, in meters.
    pub const END_X: usize = 3;
    /// The y coordinate of the end, in meters.
    pub const END_Y: usize = 4;
    /// The reference strength ([`crate::datum::Strength`] code).
    pub const STRENGTH: usize = 5;
    /// Whether the plane ends at its two points; else it runs on without end.
    pub const BOUNDED: usize = 6;
    /// Whether the plane is pinned.
    pub const PINNED: usize = 7;
}

/// Field positions of `archbuilder.work_plane`.
pub mod work_plane {
    /// The name.
    pub const NAME: usize = 0;
    /// Where the plane comes from ([`crate::datum::Source`] code).
    pub const SOURCE: usize = 1;
    /// The level, grid or reference plane the plane follows; absent for three points.
    pub const TARGET: usize = 2;
    /// The three points of a plane from points: `x`, `y`, `z` of each, in meters, from here.
    pub const POINTS: usize = 3;
    /// Whether this is the active plane: at most one is.
    pub const ACTIVE: usize = 12;
    /// Whether the plane is displayed.
    pub const SHOWN: usize = 13;
    /// The wall, floor or ceiling a plane from an element face takes its face from; appended in
    /// schema 1.1.
    pub const FACE_ELEMENT: usize = 14;
    /// Which face of that element ([`crate::datum::FaceSide`] code); appended in schema 1.1.
    pub const FACE: usize = 15;
}

/// Field positions of `archbuilder.vertical_opening`.
pub mod vertical_opening {
    /// The name.
    pub const NAME: usize = 0;
    /// The footprint vertices.
    pub const VERTICES: usize = 1;
    /// The arc edges.
    pub const ARCS: usize = 2;
    /// Whether the opening is pinned.
    pub const PINNED: usize = 3;
}

/// Field positions of `archbuilder.ceiling_type`: the layered fields, then these.
pub mod ceiling_type {
    /// The height of the underside above the level a new ceiling gets, in meters.
    pub const HEIGHT_OFFSET: usize = 4;
    /// The grid's tile size along its first axis, in meters; 0 for no pattern.
    pub const GRID_X: usize = 5;
    /// The grid's tile size along its second axis, in meters.
    pub const GRID_Y: usize = 6;
    /// The grid's angle from the x axis.
    pub const GRID_ANGLE: usize = 7;
}

/// Field positions of `archbuilder.roof_type` after the layers and the core. Both are optional
/// and appended in schema 1.1: a type without them cuts its eaves plumb.
pub mod roof_type {
    /// How the eave end of the slab is cut ([`crate::roof::RafterCut`] code).
    pub const RAFTER_CUT: usize = 4;
    /// The depth of the fascia face of a two-cut eave, in meters.
    pub const FASCIA_DEPTH: usize = 5;
}

/// The range of the fascia depth of a roof type, in meters.
pub const FASCIA_DEPTH_RANGE: (f64, f64) = (0.01, 2.0);

/// The default height of a ceiling's underside above its level, in meters.
pub const DEFAULT_CEILING_HEIGHT: f64 = 2.6;
/// The range of a wall opening's rise, in meters.
pub const RISE_RANGE: (f64, f64) = (0.0, 20.0);
/// The range of the grid tile size of a ceiling type, in meters; 0 is no pattern.
pub const GRID_RANGE: (f64, f64) = (0.0, 10.0);

/// The field positions of a stair record.
pub mod stair {
    /// The name.
    pub const NAME: usize = 0;
    /// The shape: straight, an L turn or a U turn.
    pub const SHAPE: usize = 1;
    /// The side an L or U turn goes to, seen walking up.
    pub const TURN: usize = 2;
    /// The x of the start: the middle of the first riser.
    pub const X: usize = 3;
    /// The y of the start.
    pub const Y: usize = 4;
    /// The direction of the first run, an angle from the x axis.
    pub const DIRECTION: usize = 5;
    /// The width of the runs and landings.
    pub const WIDTH: usize = 6;
    /// The tread depth.
    pub const TREAD: usize = 7;
    /// The highest riser allowed.
    pub const MAX_RISER: usize = 8;
    /// The waist thickness of the monolithic construction.
    pub const WAIST: usize = 9;
    /// Whether the left side (walking up) has a railing.
    pub const RAILING_LEFT: usize = 10;
    /// Whether the right side has a railing.
    pub const RAILING_RIGHT: usize = 11;
    /// The level the stair arrives at; the level above its storey when absent.
    pub const TOP_LEVEL: usize = 12;
    /// Whether the stair is pinned.
    pub const PINNED: usize = 13;
    /// The run lines of a stair by sketch: records of the start and the end of each run line
    /// `x0`, `y0`, `x1`, `y1`; empty for the stairs of the other shapes.
    pub const RUNS: usize = 14;
    /// The least clear height above the treads and landings, a headroom check reports below it;
    /// the default when absent.
    pub const HEADROOM: usize = 15;
}

/// Field positions of `archbuilder.elevator`.
pub mod elevator {
    /// The name.
    pub const NAME: usize = 0;
    /// The x of the middle of the car, in meters.
    pub const X: usize = 1;
    /// The y of the middle of the car, in meters.
    pub const Y: usize = 2;
    /// The direction the doors face, an angle from the x axis.
    pub const DIRECTION: usize = 3;
    /// The width of the car across the doors, in meters.
    pub const CAR_WIDTH: usize = 4;
    /// The depth of the car away from the doors, in meters.
    pub const CAR_DEPTH: usize = 5;
    /// The clearance between the car and the shaft walls on every side, in meters.
    pub const CLEARANCE: usize = 6;
    /// The width of the landing doors, in meters.
    pub const DOOR_WIDTH: usize = 7;
    /// The depth of the pit below the base level, in meters.
    pub const PIT: usize = 8;
    /// The height of the overrun above the top level, in meters.
    pub const OVERRUN: usize = 9;
    /// The top level; the level above the base storey when absent.
    pub const TOP_LEVEL: usize = 10;
    /// The levels served, a list of records of one level reference; every level from the base
    /// to the top when empty.
    pub const STOPS: usize = 11;
    /// Whether the elevator is pinned.
    pub const PINNED: usize = 12;
}

/// Field positions of `archbuilder.dormer`.
pub mod dormer {
    /// The name.
    pub const NAME: usize = 0;
    /// The x of the middle of the opening in the roof, in plan.
    pub const X: usize = 1;
    /// The y of the middle of the opening in the roof, in plan.
    pub const Y: usize = 2;
    /// The direction the front of the dormer faces, an angle from the x axis.
    pub const DIRECTION: usize = 3;
    /// The width across the front, in meters.
    pub const WIDTH: usize = 4;
    /// The depth from the front into the roof, in meters.
    pub const DEPTH: usize = 5;
    /// The pitch of the dormer's own roof.
    pub const PITCH: usize = 6;
    /// The style: gable or shed.
    pub const STYLE: usize = 7;
    /// The height of the front wall's eave above the roof at the front, in meters.
    pub const WALL_HEIGHT: usize = 8;
    /// The thickness of the walls and of the dormer roof, in meters.
    pub const THICKNESS: usize = 9;
    /// Whether the dormer is pinned.
    pub const PINNED: usize = 10;
}

/// Field positions of `archbuilder.roof_edge`.
pub mod roof_edge {
    /// The name.
    pub const NAME: usize = 0;
    /// What the element is: fascia or gutter.
    pub const ROLE: usize = 1;
    /// The profile: records of `u` (outward from the eave) and `v` (above the roof's underside
    /// at the eave).
    pub const PROFILE: usize = 2;
    /// The edges of the roof, by the number of the edge in the roof's edge list.
    pub const EDGES: usize = 3;
    /// Whether the element is pinned.
    pub const PINNED: usize = 4;
}

/// The field positions of a railing record.
pub mod railing {
    /// The name.
    pub const NAME: usize = 0;
    /// The sketched path: points with a height and the bulge of the arc to the next point.
    pub const PATH: usize = 1;
    /// The railing type.
    pub const RAILING_TYPE: usize = 2;
    /// The stair or ramp the railing is hosted on; the path is derived from it.
    pub const HOST: usize = 3;
    /// The side of the host, and the side of the path the wall is on for a return.
    pub const SIDE: usize = 4;
    /// The extension at the start of the rail.
    pub const START_EXTENSION: usize = 5;
    /// The extension at the end of the rail.
    pub const END_EXTENSION: usize = 6;
    /// The horizontal length of an extension.
    pub const EXTENSION_LENGTH: usize = 7;
    /// The railing whose start the top rail continues to.
    pub const CONTINUES_TO: usize = 8;
    /// Whether the railing is pinned.
    pub const PINNED: usize = 9;
    /// The edge of a floor the railing stands on (the number of the vertex it starts at), or, for
    /// a floor edge element, the number of its chain; the whole outer boundary without it.
    pub const HOST_EDGE: usize = 10;
}

/// The field positions of a railing type record.
pub mod railing_type {
    /// The name.
    pub const NAME: usize = 0;
    /// The height of the top rail above the walking surface.
    pub const HEIGHT: usize = 1;
    /// The profile of the top rail.
    pub const RAIL_PROFILE: usize = 2;
    /// The width of the top rail.
    pub const RAIL_WIDTH: usize = 3;
    /// The depth (height) of the top rail's section.
    pub const RAIL_DEPTH: usize = 4;
    /// The height of a handrail; 0 for none.
    pub const HANDRAIL_HEIGHT: usize = 5;
    /// The profile of the balusters.
    pub const BALUSTER_PROFILE: usize = 6;
    /// The size of a baluster.
    pub const BALUSTER_SIZE: usize = 7;
    /// The largest distance between the axes of two balusters, along the path.
    pub const BALUSTER_SPACING: usize = 8;
    /// The size of a post.
    pub const POST_SIZE: usize = 9;
    /// How the top rail turns at a corner.
    pub const CORNER: usize = 10;
    /// The radius of a filleted corner.
    pub const FILLET_RADIUS: usize = 11;
}

/// The field positions of a ramp record.
pub mod ramp {
    /// The name.
    pub const NAME: usize = 0;
    /// The x of the foot.
    pub const X: usize = 1;
    /// The y of the foot.
    pub const Y: usize = 2;
    /// The direction up the ramp, an angle from the x axis.
    pub const DIRECTION: usize = 3;
    /// The width.
    pub const WIDTH: usize = 4;
    /// The horizontal length of the sloped run.
    pub const LENGTH: usize = 5;
    /// The flat landing at the foot.
    pub const LANDING_START: usize = 6;
    /// The flat landing at the top.
    pub const LANDING_END: usize = 7;
    /// The thickness.
    pub const THICKNESS: usize = 8;
    /// The form: a thick slab or a solid down to the floor construction.
    pub const FORM: usize = 9;
    /// The rise, when it is not the height to the level above.
    pub const RISE: usize = 10;
    /// The level the ramp arrives at; the level above its storey when absent.
    pub const TOP_LEVEL: usize = 11;
    /// The ramp type.
    pub const RAMP_TYPE: usize = 12;
    /// Whether the ramp is pinned.
    pub const PINNED: usize = 13;
    /// The slope of the first run, when it is not the one shared by the runs.
    pub const SLOPE: usize = 14;
    /// The runs after the first (records, see [`run`]).
    pub const RUNS: usize = 15;
    /// The boundary of a ramp by sketch (records of `x` and `y`).
    pub const BOUNDARY: usize = 16;

    /// The fields of one entry of [`RUNS`].
    pub mod run {
        /// The turn at the landing before the run.
        pub const TURN: usize = 0;
        /// The horizontal length of the sloped part.
        pub const LENGTH: usize = 1;
        /// The landing after the run.
        pub const LANDING: usize = 2;
        /// The slope of the run, when it has its own.
        pub const SLOPE: usize = 3;
    }
}

/// The field positions of a ramp type record.
pub mod ramp_type {
    /// The name.
    pub const NAME: usize = 0;
    /// The largest slope (rise over run) of a run.
    pub const MAX_SLOPE: usize = 1;
    /// The thickness of a ramp made with the type.
    pub const THICKNESS: usize = 2;
}

/// The range of a stair's width, in meters.
pub const STAIR_WIDTH_RANGE: (f64, f64) = (0.5, 10.0);
/// The range of a stair's tread depth, in meters.
pub const TREAD_RANGE: (f64, f64) = (0.1, 2.0);
/// The range of a stair's highest riser, in meters.
pub const RISER_RANGE: (f64, f64) = (0.05, 0.5);
/// The range of a stair's waist thickness, in meters.
pub const WAIST_RANGE: (f64, f64) = (0.05, 1.0);

/// The range of a floor's thickness, in meters.
pub const FLOOR_THICKNESS_RANGE: (f64, f64) = (0.001, 5.0);
/// The thickness of a floor created without one, in meters.
pub const DEFAULT_FLOOR_THICKNESS: f64 = 0.2;

/// The elevation base code of the project origin.
pub const ELEVATION_BASE_PROJECT: u32 = 0;
/// The elevation base code of the survey point.
pub const ELEVATION_BASE_SURVEY: u32 = 1;

pub(crate) fn name_field(aspect: &str) -> FieldDecl {
    FieldDecl::new("name", ValueType::String)
        .with_bound(Bound::Length { max: NAME_BYTES })
        .in_aspect(aspect)
}

fn text_field(name: &str, max: u64) -> FieldDecl {
    FieldDecl::new(name, ValueType::String)
        .optional()
        .with_bound(Bound::Length { max })
        .in_aspect("identity")
}

pub(crate) fn length_field(name: &str, bound: Option<Bound>) -> FieldDecl {
    let field = FieldDecl::new(name, ValueType::Quantity(Dimension::LENGTH))
        .with_default(Value::Quantity(Quantity::new(0.0, Dimension::LENGTH)))
        .in_aspect("geometry");
    match bound {
        Some(bound) => field.with_bound(bound),
        None => field,
    }
}

pub(crate) fn real((min, max): (f64, f64)) -> Bound {
    Bound::Real { min, max }
}

/// An optional reference to a level that is dropped when the level is deleted.
fn optional_level() -> ReferenceSpec {
    let mut spec = reference_to(LEVEL);
    spec.rule.on_delete = OnDelete::Clear;
    spec.rule.cardinality = Cardinality::Optional;
    spec
}

pub(crate) fn reference_to(target: &str) -> ReferenceSpec {
    ReferenceSpec {
        targets: TypeKey::new(target).into_iter().collect(),
        rule: RelationshipRule {
            strength: Strength::Associative,
            on_delete: OnDelete::Block,
            on_copy: OnCopy::ShareTarget,
            cardinality: Cardinality::One,
        },
    }
}

/// The unit variants of [`project::LENGTH_UNIT`], in code order.
fn length_unit_variants() -> Vec<EnumVariant> {
    crate::units::LengthUnit::ALL
        .iter()
        .map(|u| EnumVariant::new(u.name(), u.code()))
        .collect()
}

/// The issue date is a real calendar date written `YYYY-MM-DD` and the year of construction is four
/// digits; the other project information is free text.
fn project_info_findings(values: &[Option<Value>]) -> Vec<Finding> {
    let text = |at: usize| match values.get(at) {
        Some(Some(Value::String(t))) if !t.is_empty() => Some(t.as_str()),
        _ => None,
    };
    let invalid = |field: &str| Finding {
        severity: Severity::Error,
        error: arc_base::Error::new(codes::PROJECT_INFO_INVALID).with_arg("field", field),
    };
    let mut found = Vec::new();
    if let Some(date) = text(project::ISSUE_DATE)
        && !is_iso_date(date)
    {
        found.push(invalid("issue_date"));
    }
    if let Some(year) = text(project::YEAR_OF_CONSTRUCTION)
        && !(year.len() == 4 && year.bytes().all(|b| b.is_ascii_digit()))
    {
        found.push(invalid("year_of_construction"));
    }
    found
}

/// Whether `text` is a calendar date `YYYY-MM-DD`.
#[must_use]
pub fn is_iso_date(text: &str) -> bool {
    let parts: Vec<&str> = text.split('-').collect();
    let [y, m, d] = parts.as_slice() else {
        return false;
    };
    let digits = |s: &str, n: usize| s.len() == n && s.bytes().all(|b| b.is_ascii_digit());
    if !(digits(y, 4) && digits(m, 2) && digits(d, 2)) {
        return false;
    }
    let (Ok(y), Ok(m), Ok(d)) = (y.parse::<u32>(), m.parse::<u32>(), d.parse::<u32>()) else {
        return false;
    };
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&d)
}

/// A non-blank name is checked by every type that has one.
fn blank_name(values: &[Option<Value>]) -> Vec<Finding> {
    match values.first() {
        Some(Some(Value::String(name))) if name.trim().is_empty() => vec![Finding {
            severity: Severity::Error,
            error: arc_base::Error::new(codes::NAME_EMPTY),
        }],
        _ => Vec::new(),
    }
}

fn declaration(key: &str, fields: Vec<FieldDecl>) -> Result<TypeDecl> {
    // The minor version is the one that appended the latest field.
    let minor = fields.iter().map(|f| f.since_minor).max().unwrap_or(0);
    let mut decl = TypeDecl::new(
        TypeKey::new(key)?,
        Schema::new(SchemaVersion::new(1, minor), fields),
    );
    decl.validators.push(blank_name);
    Ok(decl)
}

/// Registers the five types of the spatial structure (version 1.0) and the wall (1.1).
///
/// # Errors
/// The registry's errors; none occur for these declarations.
#[allow(clippy::too_many_lines, reason = "one field list per type")]
pub fn register(registry: &mut TypeRegistry) -> Result<()> {
    let mut project = vec![
        name_field("identity"),
        text_field("number", 64),
        text_field("client", NAME_BYTES),
        text_field("address", 512),
        FieldDecl::new("length_unit", ValueType::Enum(length_unit_variants()))
            .with_default(Value::Enum(crate::units::LengthUnit::Meter.code()))
            .in_aspect("units"),
        FieldDecl::new("template", ValueType::Bool)
            .with_default(Value::Bool(false))
            .in_aspect("identity"),
        FieldDecl::new(
            "slope_unit",
            ValueType::Enum(
                crate::units::SlopeUnit::ALL
                    .iter()
                    .map(|u| EnumVariant::new(u.name(), u.code()))
                    .collect(),
            ),
        )
        .optional()
        .in_aspect("units")
        .since_minor(1),
    ];
    // The location of the project: where its base point and survey point are, which way north
    // is and where the survey point is on the map. Optional, so older files load unchanged.
    let located = |name: &str, range: (f64, f64)| optional_length(name, range).since_minor(2);
    project.extend([
        located("base_x", COORDINATE_RANGE),
        located("base_y", COORDINATE_RANGE),
        located("base_z", ELEVATION_RANGE),
        located("survey_x", COORDINATE_RANGE),
        located("survey_y", COORDINATE_RANGE),
        located("survey_z", ELEVATION_RANGE),
        angle_field("true_north").optional().since_minor(2),
        FieldDecl::new("map_scale", ValueType::F64)
            .optional()
            .with_bound(real(crate::coordinates::SCALE_RANGE))
            .in_aspect("geometry")
            .since_minor(2),
        text_field("crs", crate::coordinates::MAX_CRS).since_minor(2),
        located("eastings", crate::coordinates::MAP_RANGE),
        located("northings", crate::coordinates::MAP_RANGE),
        located("survey_height", ELEVATION_RANGE),
        located("precision_distance", (1.0, COORDINATE_RANGE.1)),
    ]);
    // The display units of areas, volumes and angles, and the decimals and rounding of every
    // kind (AB-DATUM-004). Optional, so older files load unchanged.
    let unit_enum = |name: &str, variants: Vec<EnumVariant>| {
        FieldDecl::new(name, ValueType::Enum(variants))
            .optional()
            .in_aspect("units")
            .since_minor(3)
    };
    project.push(unit_enum(
        "area_unit",
        crate::units::AreaUnit::ALL
            .iter()
            .map(|u| EnumVariant::new(u.name(), u.code()))
            .collect(),
    ));
    project.push(unit_enum(
        "volume_unit",
        crate::units::VolumeUnit::ALL
            .iter()
            .map(|u| EnumVariant::new(u.name(), u.code()))
            .collect(),
    ));
    project.push(unit_enum(
        "angle_unit",
        crate::units::AngleUnit::ALL
            .iter()
            .map(|u| EnumVariant::new(u.name(), u.code()))
            .collect(),
    ));
    for kind in ["length", "area", "volume", "angle", "slope"] {
        project.push(
            FieldDecl::new(format!("{kind}_precision"), ValueType::I64)
                .optional()
                .with_bound(Bound::Int {
                    min: 0,
                    max: crate::units::MAX_PRECISION,
                })
                .in_aspect("units")
                .since_minor(3),
        );
    }
    for kind in ["length", "area", "volume", "angle", "slope"] {
        project.push(
            FieldDecl::new(format!("{kind}_rounding"), ValueType::F64)
                .optional()
                .with_bound(real(crate::units::ROUNDING_RANGE))
                .in_aspect("units")
                .since_minor(3),
        );
    }
    // The project information beyond the name, number, client and address (AB-DATUM-002).
    for (name, max) in [
        ("status", 64),
        ("issue_date", 10),
        ("author", INFO_BYTES),
        ("organization", INFO_BYTES),
        ("description", DESCRIPTION_BYTES),
        ("long_name", INFO_BYTES),
        ("occupancy_type", INFO_BYTES),
        ("year_of_construction", 4),
        ("construction_method", INFO_BYTES),
        ("template_source", NAME_BYTES),
    ] {
        project.push(text_field(name, max).since_minor(4));
    }
    let site = vec![name_field("identity")];
    let building = vec![
        name_field("identity"),
        length_field("elevation", None),
        text_field("address", 512),
    ];
    let storey = vec![
        name_field("identity"),
        FieldDecl::new("level", ValueType::Reference(reference_to(LEVEL))).in_aspect("data"),
    ];
    let (low, high) = ELEVATION_RANGE;
    let level = vec![
        name_field("identity"),
        length_field(
            "elevation",
            Some(Bound::Real {
                min: low,
                max: high,
            }),
        ),
        FieldDecl::new(
            "elevation_base",
            ValueType::Enum(vec![
                EnumVariant::new("project", ELEVATION_BASE_PROJECT),
                EnumVariant::new("survey", ELEVATION_BASE_SURVEY),
            ]),
        )
        .with_default(Value::Enum(ELEVATION_BASE_PROJECT))
        .in_aspect("geometry"),
        FieldDecl::new("storey", ValueType::Bool)
            .with_default(Value::Bool(true))
            .in_aspect("data"),
    ];
    let wall = vec![
        name_field("identity"),
        length_field("start_x", Some(real(COORDINATE_RANGE))),
        length_field("start_y", Some(real(COORDINATE_RANGE))),
        length_field("end_x", Some(real(COORDINATE_RANGE))),
        length_field("end_y", Some(real(COORDINATE_RANGE))),
        length_field("thickness", Some(real(THICKNESS_RANGE)))
            .with_default(length(DEFAULT_THICKNESS)),
        length_field("height", Some(real(HEIGHT_RANGE))).with_default(length(3.0)),
        length_field("base_offset", Some(real(BASE_OFFSET_RANGE))),
        FieldDecl::new(
            "location_line",
            ValueType::Enum(
                crate::wall::LocationLine::ALL
                    .iter()
                    .map(|l| EnumVariant::new(l.name(), l.code()))
                    .collect(),
            ),
        )
        .with_default(Value::Enum(crate::wall::LocationLine::Center.code()))
        .in_aspect("geometry"),
        FieldDecl::new("top_level", ValueType::Reference(optional_level()))
            .optional()
            .in_aspect("geometry"),
        join_field("start_join"),
        join_field("end_join"),
        type_field("wall_type", WALL_TYPE, 2),
        length_field("core_exterior", Some(real(THICKNESS_RANGE_OR_ZERO))).since_minor(2),
        length_field("core_interior", Some(real(THICKNESS_RANGE_OR_ZERO))).since_minor(2),
        pinned_field(3),
        length_field("top_offset", Some(real(BASE_OFFSET_RANGE)))
            .optional()
            .since_minor(4),
        FieldDecl::new("top_attach", ValueType::Reference(attach_target()))
            .optional()
            .in_aspect("geometry")
            .since_minor(4),
        crate::curtain::grid_edits_field(5),
        crate::curtain::cell_overrides_field(5),
        FieldDecl::new("bulge", ValueType::F64)
            .optional()
            .with_bound(real((-crate::floor::MAX_BULGE, crate::floor::MAX_BULGE)))
            .in_aspect("geometry")
            .since_minor(6),
        lean_field("lean_exterior"),
        lean_field("lean_interior"),
        crate::wall::profile_field(),
        crate::wall::bands_field(),
    ];
    let floor = vec![
        name_field("identity"),
        length_field("thickness", Some(real(FLOOR_THICKNESS_RANGE)))
            .with_default(length(DEFAULT_FLOOR_THICKNESS)),
        length_field("top_offset", Some(real(BASE_OFFSET_RANGE))),
        vertices_field(),
        type_field("floor_type", FLOOR_TYPE, 1),
        pinned_field(2),
        arcs_field(3),
        optional_length("slope_x", COORDINATE_RANGE).since_minor(4),
        optional_length("slope_y", COORDINATE_RANGE).since_minor(4),
        angle_field("slope_direction").optional().since_minor(4),
        optional_length("slope_length", (0.001, 100_000.0)).since_minor(4),
        FieldDecl::new("slope", ValueType::Quantity(Dimension::ANGLE))
            .optional()
            .with_bound(real((
                -crate::ceiling::MAX_SLOPE,
                crate::ceiling::MAX_SLOPE,
            )))
            .in_aspect("geometry")
            .since_minor(4),
        optional_length("end_height", BASE_OFFSET_RANGE).since_minor(4),
    ];
    for (key, fields) in [
        (FOREIGN_ELEMENT, foreign_element_fields()),
        (PLAN_VIEW, plan_view_fields()),
        (SELECTION_SET, selection_set_fields()),
        (VARIABLE, crate::variable::fields()),
        (FAMILY, family_fields()),
        (FAMILY_TYPE, family_type_fields()),
        (COMPONENT, component_fields()),
        (FAMILY_INSERT, family_insert_fields()),
        (GROUP, group_fields()),
        (GROUP_INSTANCE, group_instance_fields()),
        (PARAMETER, crate::parameter::fields()),
        (MODEL_VIEW, model_view_fields()),
        (CEILING, ceiling_fields()),
        (WALL_OPENING, wall_opening_fields()),
        (GRID, grid_fields()),
        (REFERENCE_PLANE, reference_plane_fields()),
        (WORK_PLANE, work_plane_fields()),
        (SHAFT, shaft_fields()),
        (VERTICAL_OPENING, vertical_opening_fields()),
        (FLOOR_EDGE, floor_edge_fields()),
        (ELEVATOR, elevator_fields()),
        (DORMER, dormer_fields()),
        (ROOF_EDGE, roof_edge_fields()),
        (ROOF_EXTRUSION, roof_extrusion_fields()),
        (CEILING_TYPE, crate::element_type::ceiling_type_fields()),
        (ROOF, roof_fields()),
        (ROOF_TYPE, crate::element_type::roof_type_fields()),
        (STAIR, stair_fields()),
        (RAILING, railing_fields()),
        (RAILING_TYPE, railing_type_fields()),
        (RAMP, ramp_fields()),
        (RAMP_TYPE, ramp_type_fields()),
        (ROOM, room_fields()),
        (FLOOR, floor),
        (DOOR, insert_fields(DOOR_TYPE)),
        (WINDOW, insert_fields(WINDOW_TYPE)),
        (WALL_TYPE, crate::element_type::wall_type_fields()),
        (FLOOR_TYPE, crate::element_type::floor_type_fields()),
        (DOOR_TYPE, crate::element_type::insert_type_fields()),
        (WINDOW_TYPE, crate::element_type::insert_type_fields()),
        (PROJECT, project),
        (SITE, site),
        (BUILDING, building),
        (STOREY, storey),
        (LEVEL, level),
        (WALL, wall),
    ] {
        let mut fields = fields;
        if ON_WORK_PLANE.contains(&key) {
            let field = work_plane_ref(&fields);
            fields.push(field);
        }
        if PARAMETER_HOSTS.contains(&key) || TYPE_KEYS.contains(&key) {
            let field = crate::parameter::values_field(&fields);
            fields.push(field);
        }
        let mut decl = declaration(key, fields)?;
        match key {
            WALL_TYPE | FLOOR_TYPE | ROOF_TYPE | CEILING_TYPE => {
                decl.validators.push(crate::element_type::layer_findings);
                if key == WALL_TYPE {
                    decl.validators.push(crate::curtain::type_findings);
                }
            }
            DOOR_TYPE | WINDOW_TYPE => decl.validators.push(crate::element_type::size_findings),
            PROJECT => decl.validators.push(project_info_findings),
            _ => {}
        }
        registry.register(decl)?;
    }
    Ok(())
}

/// A keyed list of (parameter key, stored value) as a field.
fn keyed_values_field(name: &str) -> FieldDecl {
    FieldDecl::new(
        name,
        ValueType::List(Box::new(ValueType::Record(vec![
            FieldDecl::new("param", ValueType::String)
                .with_bound(Bound::Length {
                    max: crate::expr::MAX_IDENTIFIER as u64,
                })
                .optional(),
            FieldDecl::new("value", ValueType::Record(crate::expr::stored_fields())).optional(),
        ]))),
    )
    .with_bound(Bound::Length {
        max: crate::expr::MAX_DEFINITIONS as u64,
    })
    .optional()
    .in_aspect("data")
}

/// The fields of a loaded family: the identity and the canonical text it was loaded from.
fn family_fields() -> Vec<FieldDecl> {
    let t = |name: &str, max: u64| text_field(name, max);
    vec![
        name_field("identity"),
        FieldDecl::new("family", ValueType::String)
            .with_bound(Bound::Length { max: 64 })
            .in_aspect("identity"),
        FieldDecl::new("version", ValueType::String)
            .with_bound(Bound::Length { max: 32 })
            .in_aspect("identity"),
        t("category", 64),
        t("hosting", 64),
        FieldDecl::new("checksum", ValueType::String)
            .with_bound(Bound::Length { max: 64 })
            .in_aspect("identity"),
        t("ifc_entity", 64),
        t("ifc_type", 64),
        t("source", 1024),
        t("license", 128),
        FieldDecl::new("text", ValueType::String)
            .with_bound(Bound::Length {
                max: crate::family::def::limits::TEXT_BYTES as u64,
            })
            .in_aspect("data"),
    ]
}

/// The fields of a family type: its name, its family and its values.
fn family_type_fields() -> Vec<FieldDecl> {
    vec![
        name_field("identity"),
        FieldDecl::new("family", ValueType::Reference(reference_to(FAMILY))).in_aspect("identity"),
        keyed_values_field("values"),
    ]
}

/// The fields of a component: a mark, its family type, the placement and its values.
fn component_fields() -> Vec<FieldDecl> {
    let mut family_type = reference_to(FAMILY_TYPE);
    family_type.rule.cardinality = Cardinality::One;
    vec![
        FieldDecl::new("name", ValueType::String)
            .with_bound(Bound::Length { max: NAME_BYTES })
            .optional()
            .in_aspect("identity"),
        FieldDecl::new("family_type", ValueType::Reference(family_type)).in_aspect("identity"),
        length_field("offset", Some(real(BASE_OFFSET_RANGE))),
        length_field("x", Some(real(COORDINATE_RANGE))),
        length_field("y", Some(real(COORDINATE_RANGE))),
        angle_field("rotation"),
        FieldDecl::new("mirror", ValueType::Bool)
            .with_default(Value::Bool(false))
            .in_aspect("geometry"),
        keyed_values_field("values"),
    ]
}

/// The fields of a hosted family instance: a mark, its family type, the position in the host,
/// the hand and facing, and its values.
fn family_insert_fields() -> Vec<FieldDecl> {
    use crate::insert::{Facing, Hand};
    let variants = |items: Vec<(&'static str, u32)>| {
        ValueType::Enum(
            items
                .into_iter()
                .map(|(n, c)| EnumVariant::new(n, c))
                .collect(),
        )
    };
    let mut family_type = reference_to(FAMILY_TYPE);
    family_type.rule.cardinality = Cardinality::One;
    vec![
        FieldDecl::new("name", ValueType::String)
            .with_bound(Bound::Length { max: NAME_BYTES })
            .optional()
            .in_aspect("identity"),
        FieldDecl::new("family_type", ValueType::Reference(family_type)).in_aspect("identity"),
        length_field("distance", Some(real(DISTANCE_RANGE))),
        length_field("offset", Some(real(BASE_OFFSET_RANGE))),
        FieldDecl::new(
            "hand",
            variants(Hand::ALL.iter().map(|o| (o.name(), o.code())).collect()),
        )
        .with_default(Value::Enum(Hand::Left.code()))
        .in_aspect("geometry"),
        FieldDecl::new(
            "facing",
            variants(Facing::ALL.iter().map(|o| (o.name(), o.code())).collect()),
        )
        .with_default(Value::Enum(Facing::Exterior.code()))
        .in_aspect("geometry"),
        keyed_values_field("values"),
    ]
}

/// The fields of a model group definition.
pub mod group {
    /// The name.
    pub const NAME: usize = 0;
    /// The x coordinate of the definition's reference point, in meters.
    pub const ORIGIN_X: usize = 1;
    /// The y coordinate of the definition's reference point, in meters.
    pub const ORIGIN_Y: usize = 2;
    /// The elevations of the levels the members are on, relative to the first level, ascending.
    pub const LEVELS: usize = 3;
    /// The level index of each member, by the member's identifier in decimal text.
    pub const MEMBER_LEVELS: usize = 4;
    /// A counter the group commands and the sanctioned propagations raise whenever they write a
    /// member; the commit validator takes it as the mark that a member's change is theirs.
    pub const REVISION: usize = 5;
}

/// The fields of a group instance.
pub mod group_instance {
    /// The mark.
    pub const NAME: usize = 0;
    /// The definition.
    pub const DEFINITION: usize = 1;
    /// The x coordinate of the instance's reference point, in meters.
    pub const ORIGIN_X: usize = 2;
    /// The y coordinate of the instance's reference point, in meters.
    pub const ORIGIN_Y: usize = 3;
    /// The rotation about the vertical axis, in radians.
    pub const ROTATION: usize = 4;
    /// Whether the definition is mirrored across its own vertical plane.
    pub const MIRROR: usize = 5;
    /// The storey each definition level is mapped to, by the level's index.
    pub const LEVEL_MAP: usize = 6;
    /// The definition members this instance leaves out.
    pub const EXCLUSIONS: usize = 7;
    /// The text exceptions, by member identifier and kind (`name` or `number`).
    pub const TEXT_EXCEPTIONS: usize = 8;
    /// The offset exceptions, by member identifier and axis (`x` or `y`), in meters.
    pub const OFFSET_EXCEPTIONS: usize = 9;
    /// The fingerprint of each generated member as it was last generated, by member identifier.
    pub const CHECKS: usize = 10;
}

/// The fields of a model group definition: the name, the reference point, the levels and the
/// level of each member.
fn group_fields() -> Vec<FieldDecl> {
    vec![
        name_field("identity"),
        length_field("origin_x", Some(real(COORDINATE_RANGE))),
        length_field("origin_y", Some(real(COORDINATE_RANGE))),
        FieldDecl::new(
            "levels",
            ValueType::List(Box::new(ValueType::Quantity(Dimension::LENGTH))),
        )
        .with_bound(Bound::Length {
            max: crate::group::MAX_LEVELS as u64,
        })
        .optional()
        .in_aspect("geometry"),
        FieldDecl::new("member_levels", ValueType::Map(Box::new(ValueType::I64)))
            .with_bound(Bound::Length {
                max: crate::group::MAX_MEMBERS as u64,
            })
            .optional()
            .in_aspect("data"),
        FieldDecl::new("revision", ValueType::I64)
            .optional()
            .in_aspect("data"),
    ]
}

/// The fields of a group instance: the mark, the definition, the placement, the level mapping,
/// the exclusions and exceptions, and the fingerprints of the generated members.
fn group_instance_fields() -> Vec<FieldDecl> {
    let mut definition = reference_to(GROUP);
    definition.rule.cardinality = Cardinality::One;
    let mut storey = reference_to(STOREY);
    storey.rule.cardinality = Cardinality::Many {
        max: u32::try_from(crate::group::MAX_LEVELS).unwrap_or(u32::MAX),
    };
    let mut member = reference_to(WALL);
    member.targets = crate::group::MEMBER_KINDS
        .iter()
        .filter_map(|k| TypeKey::new(*k).ok())
        .collect();
    member.rule.on_delete = OnDelete::Clear;
    member.rule.cardinality = Cardinality::Many {
        max: u32::try_from(crate::group::MAX_MEMBERS).unwrap_or(u32::MAX),
    };
    vec![
        name_field("identity"),
        FieldDecl::new("definition", ValueType::Reference(definition)).in_aspect("identity"),
        length_field("origin_x", Some(real(COORDINATE_RANGE))),
        length_field("origin_y", Some(real(COORDINATE_RANGE))),
        angle_field("rotation"),
        bool_field("mirror", false),
        FieldDecl::new(
            "level_map",
            ValueType::List(Box::new(ValueType::Reference(storey))),
        )
        .with_bound(Bound::Length {
            max: crate::group::MAX_LEVELS as u64,
        })
        .optional()
        .in_aspect("data"),
        FieldDecl::new(
            "exclusions",
            ValueType::List(Box::new(ValueType::Reference(member))),
        )
        .with_bound(Bound::Length {
            max: crate::group::MAX_MEMBERS as u64,
        })
        .optional()
        .in_aspect("data"),
        FieldDecl::new(
            "text_exceptions",
            ValueType::Map(Box::new(ValueType::String)),
        )
        .with_bound(Bound::Length {
            max: crate::group::MAX_MEMBERS as u64,
        })
        .optional()
        .in_aspect("data"),
        FieldDecl::new(
            "offset_exceptions",
            ValueType::Map(Box::new(ValueType::Quantity(Dimension::LENGTH))),
        )
        .with_bound(Bound::Length {
            max: crate::group::MAX_MEMBERS as u64,
        })
        .optional()
        .in_aspect("data"),
        FieldDecl::new("checks", ValueType::Map(Box::new(ValueType::String)))
            .with_bound(Bound::Length {
                max: crate::group::MAX_MEMBERS as u64,
            })
            .optional()
            .in_aspect("data"),
    ]
}

/// The fields of a selection set: a name and its member elements. A deleted element leaves the
/// set (the reference clears).
fn selection_set_fields() -> Vec<FieldDecl> {
    let mut member = reference_to(ELEMENTS[0]);
    member.targets = ELEMENTS
        .iter()
        .filter_map(|k| TypeKey::new(*k).ok())
        .collect();
    member.rule.on_delete = OnDelete::Clear;
    member.rule.cardinality = Cardinality::Optional;
    vec![
        name_field("identity"),
        FieldDecl::new(
            "members",
            ValueType::List(Box::new(ValueType::Record(vec![
                FieldDecl::new("object", ValueType::Reference(member)).optional(),
            ]))),
        )
        .with_bound(Bound::Length {
            max: MAX_SET_MEMBERS as u64,
        })
        .optional()
        .in_aspect("data"),
    ]
}

/// The fields of a model view: kind, graphics, the per-view visibility of a plan view, the camera
/// and section box of a 3D view and the line, far clip and crop of a section or elevation.
#[allow(
    clippy::too_many_lines,
    reason = "one declaration per field, in record order"
)]
fn model_view_fields() -> Vec<FieldDecl> {
    use crate::model_view::{DEPTH_RANGE, FOV_RANGE, ViewKind, VisualStyle};
    let mut hidden = reference_to(ELEMENTS[0]);
    hidden.targets = ELEMENTS
        .iter()
        .filter_map(|k| TypeKey::new(*k).ok())
        .collect();
    hidden.rule.on_delete = OnDelete::Clear;
    hidden.rule.cardinality = Cardinality::Optional;
    let enumeration = |name: &str, variants: Vec<EnumVariant>, default: u32| {
        FieldDecl::new(name, ValueType::Enum(variants))
            .with_default(Value::Enum(default))
            .in_aspect("graphics")
    };
    let flag = |name: &str, on: bool, aspect: &str| {
        FieldDecl::new(name, ValueType::Bool)
            .with_default(Value::Bool(on))
            .in_aspect(aspect)
    };
    let coordinate =
        |name: &str, aspect: &str| optional_length(name, COORDINATE_RANGE).in_aspect(aspect);
    let mut fields = vec![
        name_field("identity"),
        enumeration(
            "kind",
            ViewKind::ALL
                .iter()
                .map(|k| EnumVariant::new(k.name(), k.code()))
                .collect(),
            ViewKind::ThreeD.code(),
        )
        .in_aspect("identity"),
        FieldDecl::new("scale", ValueType::F64)
            .with_default(Value::F64(crate::view::DEFAULT_SCALE))
            .with_bound(real(VIEW_SCALE_RANGE))
            .in_aspect("graphics"),
        enumeration(
            "detail",
            crate::view::Detail::ALL
                .iter()
                .map(|d| EnumVariant::new(d.name(), d.code()))
                .collect(),
            crate::view::Detail::Medium.code(),
        ),
        enumeration(
            "style",
            VisualStyle::ALL
                .iter()
                .map(|v| EnumVariant::new(v.name(), v.code()))
                .collect(),
            VisualStyle::Shaded.code(),
        ),
        FieldDecl::new(
            "hidden",
            ValueType::List(Box::new(ValueType::Record(vec![
                FieldDecl::new("object", ValueType::Reference(hidden)).optional(),
            ]))),
        )
        .with_bound(Bound::Length {
            max: MAX_HIDDEN as u64,
        })
        .optional()
        .in_aspect("graphics"),
    ];
    // The components flag stands after the top, below; the others stay in their block.
    fields.extend(
        crate::view::Category::ALL
            .iter()
            .filter(|c| **c != crate::view::Category::Component)
            .map(|c| flag(c.field(), true, "graphics")),
    );
    fields.push(
        FieldDecl::new("perspective", ValueType::Bool)
            .optional()
            .in_aspect("camera"),
    );
    for name in [
        "eye_x", "eye_y", "eye_z", "target_x", "target_y", "target_z",
    ] {
        fields.push(coordinate(name, "camera"));
    }
    fields.push(
        FieldDecl::new("fov", ValueType::Quantity(Dimension::ANGLE))
            .optional()
            .with_bound(real(FOV_RANGE))
            .in_aspect("camera"),
    );
    fields.push(flag("section_box", false, "section_box"));
    for name in [
        "box_min_x",
        "box_min_y",
        "box_min_z",
        "box_max_x",
        "box_max_y",
        "box_max_z",
    ] {
        fields.push(coordinate(name, "section_box"));
    }
    for name in ["start_x", "start_y", "end_x", "end_y"] {
        fields.push(coordinate(name, "geometry"));
    }
    fields.push(
        length_field("depth", Some(real(DEPTH_RANGE)))
            .with_default(length(crate::model_view::SECTION_DEPTH))
            .in_aspect("range"),
    );
    fields.push(flag("crop", true, "range"));
    fields.push(optional_length("bottom", ELEVATION_RANGE).in_aspect("range"));
    fields.push(optional_length("top", ELEVATION_RANGE).in_aspect("range"));
    fields.push(flag(crate::view::Category::Component.field(), true, "graphics").since_minor(1));
    fields
}

/// The fields of a plan view: the range, the scale, the detail level, the underlay, one flag per
/// category and the elements hidden in the view.
#[allow(
    clippy::too_many_lines,
    reason = "one declaration per field, in record order"
)]
fn plan_view_fields() -> Vec<FieldDecl> {
    let mut hidden = reference_to(ELEMENTS[0]);
    hidden.targets = ELEMENTS
        .iter()
        .filter_map(|k| TypeKey::new(*k).ok())
        .collect();
    hidden.rule.on_delete = OnDelete::Clear;
    hidden.rule.cardinality = Cardinality::Optional;
    let flag = |name: &str, on: bool| {
        FieldDecl::new(name, ValueType::Bool)
            .with_default(Value::Bool(on))
            .in_aspect("graphics")
    };
    let mut fields = vec![
        name_field("identity"),
        length_field("cut", Some(real(VIEW_HEIGHT_RANGE)))
            .with_default(length(crate::view::DEFAULT_CUT))
            .in_aspect("range"),
        optional_length("top", VIEW_HEIGHT_RANGE).in_aspect("range"),
        flag("overhead", true).in_aspect("range"),
        FieldDecl::new("scale", ValueType::F64)
            .with_default(Value::F64(crate::view::DEFAULT_SCALE))
            .with_bound(real(VIEW_SCALE_RANGE))
            .in_aspect("graphics"),
        FieldDecl::new(
            "detail",
            ValueType::Enum(
                crate::view::Detail::ALL
                    .iter()
                    .map(|d| EnumVariant::new(d.name(), d.code()))
                    .collect(),
            ),
        )
        .with_default(Value::Enum(crate::view::Detail::Medium.code()))
        .in_aspect("graphics"),
        flag("underlay", false),
    ];
    fields.extend(
        crate::view::Category::ALL
            .iter()
            .take(plan_view::CATEGORY_COUNT)
            .map(|c| flag(c.field(), c.shown_by_default())),
    );
    fields.push(
        FieldDecl::new(
            "hidden",
            ValueType::List(Box::new(ValueType::Record(vec![
                FieldDecl::new("object", ValueType::Reference(hidden)).optional(),
            ]))),
        )
        .with_bound(Bound::Length {
            max: MAX_HIDDEN as u64,
        })
        .optional()
        .in_aspect("graphics"),
    );
    fields.push(
        length_field("bottom", Some(real(VIEW_DEPTH_RANGE)))
            .with_default(length(0.0))
            .in_aspect("range"),
    );
    fields.push(
        length_field("depth", Some(real(VIEW_DEPTH_RANGE)))
            .with_default(length(-crate::view::BOTTOM_MARGIN))
            .in_aspect("range"),
    );
    let mut grid_ref = reference_to(GRID);
    grid_ref.rule.on_delete = OnDelete::Clear;
    grid_ref.rule.cardinality = Cardinality::Optional;
    fields.push(
        FieldDecl::new(
            "datum_views",
            ValueType::List(Box::new(ValueType::Record(vec![
                FieldDecl::new("datum", ValueType::Reference(grid_ref)).optional(),
                optional_length("start_inset", crate::view::INSET_RANGE),
                optional_length("end_inset", crate::view::INSET_RANGE),
                FieldDecl::new("head_start", ValueType::Bool).optional(),
                FieldDecl::new("head_end", ValueType::Bool).optional(),
            ]))),
        )
        .with_bound(Bound::Length {
            max: MAX_PER_VIEW_DATUMS as u64,
        })
        .optional()
        .in_aspect("graphics")
        .since_minor(1),
    );
    fields.push(
        FieldDecl::new(
            "north",
            ValueType::Enum(
                crate::view::North::ALL
                    .iter()
                    .map(|n| EnumVariant::new(n.name(), n.code()))
                    .collect(),
            ),
        )
        .optional()
        .in_aspect("graphics")
        .since_minor(2),
    );
    // The categories added after the first release stand at the end of the record, so that the
    // slots of the others stay where files have them.
    fields.extend(
        crate::view::Category::ALL
            .iter()
            .skip(plan_view::CATEGORY_COUNT)
            .map(|c| {
                // Components came in schema 1.4, after railings and ramps in 1.3.
                let minor = if *c == crate::view::Category::Component {
                    4
                } else {
                    3
                };
                flag(c.field(), c.shown_by_default()).since_minor(minor)
            }),
    );
    fields
}

/// The boundary vertices of a floor, ceiling, shaft or vertical opening.
fn vertices_field() -> FieldDecl {
    FieldDecl::new("vertices", ValueType::List(Box::new(vertex_type())))
        .with_bound(Bound::Length {
            max: crate::floor::MAX_VERTICES as u64,
        })
        .in_aspect("geometry")
}

/// The arc edges of a boundary, appended in schema 1.`minor`.
fn arcs_field(minor: u32) -> FieldDecl {
    FieldDecl::new(
        "arcs",
        ValueType::List(Box::new(ValueType::Record(vec![
            FieldDecl::new("vertex", ValueType::I64).with_default(Value::I64(0)),
            FieldDecl::new("bulge", ValueType::F64)
                .with_default(Value::F64(0.0))
                .with_bound(real((-crate::floor::MAX_BULGE, crate::floor::MAX_BULGE))),
        ]))),
    )
    .with_bound(Bound::Length {
        max: crate::floor::MAX_VERTICES as u64,
    })
    .optional()
    .in_aspect("geometry")
    .since_minor(minor)
}

fn optional_length(name: &str, range: (f64, f64)) -> FieldDecl {
    FieldDecl::new(name, ValueType::Quantity(Dimension::LENGTH))
        .optional()
        .with_bound(real(range))
        .in_aspect("geometry")
}

/// An optional lean angle of a wall face, appended in schema 1.6.
fn lean_field(name: &str) -> FieldDecl {
    FieldDecl::new(name, ValueType::Quantity(Dimension::ANGLE))
        .optional()
        .with_bound(real((-crate::wall::MAX_LEAN, crate::wall::MAX_LEAN)))
        .in_aspect("geometry")
        .since_minor(6)
}

fn angle_field(name: &str) -> FieldDecl {
    FieldDecl::new(name, ValueType::Quantity(Dimension::ANGLE))
        .with_default(Value::Quantity(Quantity::new(0.0, Dimension::ANGLE)))
        .with_bound(real((-1000.0, 1000.0)))
        .in_aspect("geometry")
}

fn ceiling_fields() -> Vec<FieldDecl> {
    vec![
        name_field("identity"),
        length_field("thickness", Some(real(FLOOR_THICKNESS_RANGE))).with_default(length(0.02)),
        length_field("height_offset", Some(real(BASE_OFFSET_RANGE)))
            .with_default(length(DEFAULT_CEILING_HEIGHT)),
        vertices_field(),
        type_field("ceiling_type", CEILING_TYPE, 0),
        pinned_field(0),
        arcs_field(0),
        length_field("grid_x", Some(real(COORDINATE_RANGE))),
        length_field("grid_y", Some(real(COORDINATE_RANGE))),
        angle_field("grid_rotation"),
        optional_length("slope_x", COORDINATE_RANGE),
        optional_length("slope_y", COORDINATE_RANGE),
        angle_field("slope_direction"),
        optional_length("slope_length", (0.001, 100_000.0)),
        FieldDecl::new("slope", ValueType::Quantity(Dimension::ANGLE))
            .optional()
            .with_bound(real((
                -crate::ceiling::MAX_SLOPE,
                crate::ceiling::MAX_SLOPE,
            )))
            .in_aspect("geometry"),
        optional_length("end_height", BASE_OFFSET_RANGE),
    ]
}

fn wall_opening_fields() -> Vec<FieldDecl> {
    vec![
        name_field("identity"),
        length_field("distance", Some(real(DISTANCE_RANGE))),
        length_field("width", Some(real(INSERT_WIDTH_RANGE))).with_default(length(0.9)),
        length_field("height", Some(real(INSERT_HEIGHT_RANGE))).with_default(length(2.1)),
        length_field("sill", Some(real(SILL_RANGE))),
        length_field("rise", Some(real(RISE_RANGE))),
        pinned_field(0),
        FieldDecl::new(
            "profile",
            ValueType::List(Box::new(ValueType::Record(vec![
                length_field("x", Some(real((0.0, INSERT_WIDTH_RANGE.1)))),
                length_field("y", Some(real((0.0, INSERT_HEIGHT_RANGE.1)))),
            ]))),
        )
        .with_bound(Bound::Length {
            max: crate::insert::MAX_PROFILE as u64,
        })
        .optional()
        .in_aspect("geometry")
        .since_minor(1),
    ]
}

fn shaft_fields() -> Vec<FieldDecl> {
    vec![
        name_field("identity"),
        length_field("base_offset", Some(real(BASE_OFFSET_RANGE))),
        length_field("top_offset", Some(real(BASE_OFFSET_RANGE))),
        vertices_field(),
        arcs_field(0),
        pinned_field(0),
        FieldDecl::new("base_level", ValueType::Reference(optional_level()))
            .optional()
            .in_aspect("geometry"),
        FieldDecl::new("top_level", ValueType::Reference(optional_level()))
            .optional()
            .in_aspect("geometry"),
    ]
}

fn roof_extrusion_fields() -> Vec<FieldDecl> {
    use crate::roof_extrusion::{MAX_PROFILE, PROFILE_RANGE, SPAN_RANGE};
    let point = ValueType::Record(vec![
        length_field("u", Some(real(PROFILE_RANGE))),
        length_field("z", Some(real(BASE_OFFSET_RANGE))),
    ]);
    vec![
        name_field("identity"),
        length_field("thickness", Some(real(THICKNESS_RANGE)))
            .with_default(length(crate::roof::DEFAULT_THICKNESS)),
        length_field("start", Some(real(SPAN_RANGE))),
        length_field("end", Some(real(SPAN_RANGE))).with_default(length(1.0)),
        angle_field("direction"),
        type_field("roof_type", ROOF_TYPE, 0),
        pinned_field(0),
        length_field("x", Some(real(COORDINATE_RANGE))),
        length_field("y", Some(real(COORDINATE_RANGE))),
        FieldDecl::new("profile", ValueType::List(Box::new(point)))
            .with_bound(Bound::Length {
                max: MAX_PROFILE as u64,
            })
            .in_aspect("geometry"),
    ]
}

fn floor_edge_fields() -> Vec<FieldDecl> {
    let profile = ValueType::Record(vec![
        length_field("u", Some(real(crate::floor_edge::PROFILE_RANGE))),
        length_field("v", Some(real(crate::floor_edge::PROFILE_RANGE))),
    ]);
    vec![
        name_field("identity"),
        FieldDecl::new("profile", ValueType::List(Box::new(profile)))
            .with_bound(Bound::Length {
                max: crate::floor_edge::MAX_PROFILE as u64,
            })
            .in_aspect("geometry"),
        FieldDecl::new("edges", ValueType::List(Box::new(ValueType::I64)))
            .with_bound(Bound::Length {
                max: crate::floor::MAX_VERTICES as u64,
            })
            .in_aspect("geometry"),
        pinned_field(0),
    ]
}

fn elevator_fields() -> Vec<FieldDecl> {
    use crate::elevator as e;
    let stop = {
        let mut spec = optional_level();
        spec.rule.on_delete = OnDelete::Clear;
        spec
    };
    vec![
        name_field("identity"),
        length_field("x", Some(real(COORDINATE_RANGE))),
        length_field("y", Some(real(COORDINATE_RANGE))),
        angle_field("direction"),
        length_field("car_width", Some(real(e::CAR_RANGE))).with_default(length(e::DEFAULT_CAR[0])),
        length_field("car_depth", Some(real(e::CAR_RANGE))).with_default(length(e::DEFAULT_CAR[1])),
        length_field("clearance", Some(real(e::CLEARANCE_RANGE)))
            .with_default(length(e::DEFAULT_CLEARANCE)),
        length_field("door_width", Some(real(e::CAR_RANGE)))
            .with_default(length(e::DEFAULT_DOOR_WIDTH)),
        length_field("pit", Some(real(e::PIT_RANGE))).with_default(length(e::DEFAULT_PIT)),
        length_field("overrun", Some(real(e::PIT_RANGE))).with_default(length(e::DEFAULT_OVERRUN)),
        FieldDecl::new("top_level", ValueType::Reference(optional_level()))
            .optional()
            .in_aspect("geometry"),
        FieldDecl::new(
            "stops",
            ValueType::List(Box::new(ValueType::Record(vec![
                FieldDecl::new("level", ValueType::Reference(stop)).optional(),
            ]))),
        )
        .with_bound(Bound::Length {
            max: e::MAX_STOPS as u64,
        })
        .optional()
        .in_aspect("geometry"),
        pinned_field(0),
    ]
}

fn dormer_fields() -> Vec<FieldDecl> {
    use crate::dormer as d;
    vec![
        name_field("identity"),
        length_field("x", Some(real(COORDINATE_RANGE))),
        length_field("y", Some(real(COORDINATE_RANGE))),
        angle_field("direction"),
        length_field("width", Some(real(d::SIZE_RANGE))).with_default(length(d::DEFAULT_WIDTH)),
        length_field("depth", Some(real(d::SIZE_RANGE))).with_default(length(d::DEFAULT_DEPTH)),
        FieldDecl::new("pitch", ValueType::Quantity(Dimension::ANGLE))
            .with_default(Value::Quantity(Quantity::new(
                d::DEFAULT_PITCH,
                Dimension::ANGLE,
            )))
            .with_bound(real(d::PITCH_RANGE))
            .in_aspect("geometry"),
        coded_field(
            "style",
            [
                (d::Style::Gable.name(), d::Style::Gable.code()),
                (d::Style::Shed.name(), d::Style::Shed.code()),
            ],
            d::Style::Gable.code(),
        ),
        length_field("wall_height", Some(real(d::SIZE_RANGE)))
            .with_default(length(d::DEFAULT_WALL_HEIGHT)),
        length_field("thickness", Some(real(d::THICKNESS_RANGE)))
            .with_default(length(d::DEFAULT_THICKNESS)),
        pinned_field(0),
    ]
}

fn roof_edge_fields() -> Vec<FieldDecl> {
    use crate::roof_edge as r;
    let profile = ValueType::Record(vec![
        length_field("u", Some(real(crate::floor_edge::PROFILE_RANGE))),
        length_field("v", Some(real(crate::floor_edge::PROFILE_RANGE))),
    ]);
    vec![
        name_field("identity"),
        coded_field(
            "role",
            [
                (r::Role::Fascia.name(), r::Role::Fascia.code()),
                (r::Role::Gutter.name(), r::Role::Gutter.code()),
            ],
            r::Role::Fascia.code(),
        ),
        FieldDecl::new("profile", ValueType::List(Box::new(profile)))
            .with_bound(Bound::Length {
                max: crate::floor_edge::MAX_PROFILE as u64,
            })
            .in_aspect("geometry"),
        FieldDecl::new("edges", ValueType::List(Box::new(ValueType::I64)))
            .with_bound(Bound::Length {
                max: crate::roof::MAX_EDGES as u64,
            })
            .in_aspect("geometry"),
        pinned_field(0),
    ]
}

fn bool_field(name: &str, on: bool) -> FieldDecl {
    FieldDecl::new(name, ValueType::Bool)
        .with_default(Value::Bool(on))
        .in_aspect("graphics")
}

/// The optional level a grid's extent ends at; dropped when the level is deleted.
fn extent_field(name: &str) -> FieldDecl {
    FieldDecl::new(name, ValueType::Reference(optional_level()))
        .optional()
        .in_aspect("geometry")
}

fn grid_fields() -> Vec<FieldDecl> {
    vec![
        name_field("identity"),
        length_field("start_x", Some(real(COORDINATE_RANGE))),
        length_field("start_y", Some(real(COORDINATE_RANGE))),
        length_field("end_x", Some(real(COORDINATE_RANGE))),
        length_field("end_y", Some(real(COORDINATE_RANGE))),
        FieldDecl::new("bulge", ValueType::F64)
            .with_default(Value::F64(0.0))
            .with_bound(real((-crate::floor::MAX_BULGE, crate::floor::MAX_BULGE)))
            .in_aspect("geometry"),
        bool_field("head_start", true),
        bool_field("head_end", true),
        extent_field("bottom_level"),
        extent_field("top_level"),
        pinned_field(0),
        FieldDecl::new(
            "path",
            ValueType::List(Box::new(ValueType::Record(vec![
                length_field("x", Some(real(COORDINATE_RANGE))),
                length_field("y", Some(real(COORDINATE_RANGE))),
                FieldDecl::new("bulge", ValueType::F64)
                    .with_default(Value::F64(0.0))
                    .with_bound(real((-crate::floor::MAX_BULGE, crate::floor::MAX_BULGE))),
            ]))),
        )
        .with_bound(Bound::Length {
            max: crate::datum::MAX_PATH as u64,
        })
        .optional()
        .in_aspect("geometry")
        .since_minor(1),
    ]
}

fn strength_variants() -> ValueType {
    ValueType::Enum(
        crate::datum::Strength::ALL
            .iter()
            .map(|s| EnumVariant::new(s.name(), s.code()))
            .collect(),
    )
}

fn foreign_element_fields() -> Vec<FieldDecl> {
    vec![
        name_field("identity"),
        text_field("ifc_class", 64),
        text_field("predefined_type", 64),
        text_field("object_type", NAME_BYTES),
        text_field("tag", NAME_BYTES),
        FieldDecl::new(
            "mesh",
            ValueType::Geometry(arc_doc::GeometryEncoding {
                name: FOREIGN_MESH_ENCODING.0.to_owned(),
                version: FOREIGN_MESH_ENCODING.1,
            }),
        )
        .optional()
        .in_aspect("geometry"),
        text_field("source_schema", 16),
    ]
}

fn reference_plane_fields() -> Vec<FieldDecl> {
    vec![
        name_field("identity"),
        length_field("start_x", Some(real(COORDINATE_RANGE))),
        length_field("start_y", Some(real(COORDINATE_RANGE))),
        length_field("end_x", Some(real(COORDINATE_RANGE))),
        length_field("end_y", Some(real(COORDINATE_RANGE))),
        FieldDecl::new("strength", strength_variants())
            .with_default(Value::Enum(crate::datum::Strength::Weak.code()))
            .in_aspect("identity"),
        bool_field("bounded", true).in_aspect("geometry"),
        pinned_field(0),
    ]
}

fn work_plane_fields() -> Vec<FieldDecl> {
    let mut target = reference_to(LEVEL);
    target.targets = [LEVEL, GRID, REFERENCE_PLANE]
        .iter()
        .filter_map(|k| TypeKey::new(*k).ok())
        .collect();
    // A plane that follows a datum goes with it.
    target.rule.on_delete = OnDelete::Cascade;
    target.rule.cardinality = Cardinality::Optional;
    let mut fields = vec![
        name_field("identity"),
        FieldDecl::new(
            "source",
            ValueType::Enum(
                crate::datum::Source::ALL
                    .iter()
                    .map(|s| EnumVariant::new(s.name(), s.code()))
                    .collect(),
            ),
        )
        .with_default(Value::Enum(crate::datum::Source::Level.code()))
        .in_aspect("geometry"),
        FieldDecl::new("target", ValueType::Reference(target))
            .optional()
            .in_aspect("geometry"),
    ];
    for point in 1..=3 {
        for axis in ["x", "y", "z"] {
            fields.push(
                length_field(&format!("p{point}_{axis}"), Some(real(COORDINATE_RANGE))).optional(),
            );
        }
    }
    fields.push(bool_field("active", false).in_aspect("identity"));
    fields.push(bool_field("shown", false));
    // A plane from an element face goes with the element.
    let mut element = reference_to(WALL);
    element.targets = [WALL, FLOOR, CEILING]
        .iter()
        .filter_map(|k| TypeKey::new(*k).ok())
        .collect();
    element.rule.on_delete = OnDelete::Cascade;
    element.rule.cardinality = Cardinality::Optional;
    fields.push(
        FieldDecl::new("face_element", ValueType::Reference(element))
            .optional()
            .in_aspect("geometry")
            .since_minor(1),
    );
    fields.push(
        FieldDecl::new(
            "face",
            ValueType::Enum(
                crate::datum::FaceSide::ALL
                    .iter()
                    .map(|s| EnumVariant::new(s.name(), s.code()))
                    .collect(),
            ),
        )
        .optional()
        .in_aspect("geometry")
        .since_minor(1),
    );
    fields
}

/// The element types whose sketch or plane is made on a work plane, and so store it.
pub const ON_WORK_PLANE: [&str; 7] = [
    FLOOR,
    CEILING,
    ROOF,
    ROOF_EXTRUSION,
    RAILING,
    SHAFT,
    VERTICAL_OPENING,
];

/// The work plane an element was sketched on, appended after the latest field of `fields`: an
/// optional reference that clears when the plane is deleted, so older files load without one.
fn work_plane_ref(fields: &[FieldDecl]) -> FieldDecl {
    let minor = fields.iter().map(|f| f.since_minor).max().unwrap_or(0) + 1;
    let mut target = reference_to(WORK_PLANE);
    target.rule.on_delete = OnDelete::Clear;
    target.rule.cardinality = Cardinality::Optional;
    FieldDecl::new("work_plane", ValueType::Reference(target))
        .optional()
        .in_aspect("geometry")
        .since_minor(minor)
}

fn vertical_opening_fields() -> Vec<FieldDecl> {
    vec![
        name_field("identity"),
        vertices_field(),
        arcs_field(0),
        pinned_field(0),
    ]
}

fn stair_fields() -> Vec<FieldDecl> {
    use crate::stair::{
        DEFAULT_MAX_RISER, DEFAULT_TREAD, DEFAULT_WAIST, DEFAULT_WIDTH, Shape, Turn,
    };
    let variants = |items: Vec<(&'static str, u32)>| {
        ValueType::Enum(
            items
                .into_iter()
                .map(|(n, c)| EnumVariant::new(n, c))
                .collect(),
        )
    };
    let flag = |name: &str| {
        FieldDecl::new(name, ValueType::Bool)
            .with_default(Value::Bool(true))
            .in_aspect("geometry")
    };
    vec![
        name_field("identity"),
        FieldDecl::new(
            "shape",
            variants(Shape::ALL.iter().map(|s| (s.name(), s.code())).collect()),
        )
        .with_default(Value::Enum(Shape::Straight.code()))
        .in_aspect("geometry"),
        FieldDecl::new(
            "turn",
            variants(Turn::ALL.iter().map(|s| (s.name(), s.code())).collect()),
        )
        .with_default(Value::Enum(Turn::Left.code()))
        .in_aspect("geometry"),
        length_field("x", Some(real(COORDINATE_RANGE))),
        length_field("y", Some(real(COORDINATE_RANGE))),
        FieldDecl::new("direction", ValueType::Quantity(Dimension::ANGLE))
            .with_default(Value::Quantity(Quantity::new(0.0, Dimension::ANGLE)))
            .with_bound(real((-1000.0, 1000.0)))
            .in_aspect("geometry"),
        length_field("width", Some(real(STAIR_WIDTH_RANGE))).with_default(length(DEFAULT_WIDTH)),
        length_field("tread", Some(real(TREAD_RANGE))).with_default(length(DEFAULT_TREAD)),
        length_field("max_riser", Some(real(RISER_RANGE))).with_default(length(DEFAULT_MAX_RISER)),
        length_field("waist", Some(real(WAIST_RANGE))).with_default(length(DEFAULT_WAIST)),
        flag("railing_left"),
        flag("railing_right"),
        FieldDecl::new("top_level", ValueType::Reference(optional_level()))
            .optional()
            .in_aspect("geometry"),
        pinned_field(1),
        FieldDecl::new(
            "runs",
            ValueType::List(Box::new(ValueType::Record(vec![
                length_field("x0", Some(real(COORDINATE_RANGE))),
                length_field("y0", Some(real(COORDINATE_RANGE))),
                length_field("x1", Some(real(COORDINATE_RANGE))),
                length_field("y1", Some(real(COORDINATE_RANGE))),
            ]))),
        )
        .with_bound(Bound::Length {
            max: crate::stair::MAX_RUN_LINES as u64,
        })
        .optional()
        .in_aspect("geometry")
        .since_minor(2),
        optional_length("headroom", (0.5, 10.0)).since_minor(2),
    ]
}

fn coded_field<const N: usize>(
    name: &str,
    variants: [(&'static str, u32); N],
    default: u32,
) -> FieldDecl {
    FieldDecl::new(
        name,
        ValueType::Enum(
            variants
                .into_iter()
                .map(|(n, c)| EnumVariant::new(n, c))
                .collect(),
        ),
    )
    .with_default(Value::Enum(default))
    .in_aspect("geometry")
}

fn railing_fields() -> Vec<FieldDecl> {
    use crate::railing::{DEFAULT_EXTENSION, EXTENSION_RANGE, Extension, MAX_POINTS, Side};
    let host = {
        let mut spec = reference_to(STAIR);
        spec.targets = [STAIR, RAMP, FLOOR, FLOOR_EDGE]
            .iter()
            .filter_map(|k| TypeKey::new(*k).ok())
            .collect();
        // A railing hosted on a stair, ramp, floor or floor edge goes with it.
        spec.rule.on_delete = OnDelete::Cascade;
        spec.rule.cardinality = Cardinality::Optional;
        spec
    };
    let next = {
        let mut spec = reference_to(RAILING);
        spec.rule.on_delete = OnDelete::Clear;
        spec.rule.cardinality = Cardinality::Optional;
        spec
    };
    let extension = |name: &str| {
        coded_field(
            name,
            [
                (Extension::None.name(), Extension::None.code()),
                (Extension::Horizontal.name(), Extension::Horizontal.code()),
                (Extension::ReturnWall.name(), Extension::ReturnWall.code()),
                (Extension::ReturnFloor.name(), Extension::ReturnFloor.code()),
            ],
            Extension::None.code(),
        )
    };
    vec![
        name_field("identity"),
        FieldDecl::new(
            "path",
            ValueType::List(Box::new(ValueType::Record(vec![
                length_field("x", Some(real(COORDINATE_RANGE))),
                length_field("y", Some(real(COORDINATE_RANGE))),
                length_field("z", Some(real(BASE_OFFSET_RANGE))),
                FieldDecl::new("bulge", ValueType::F64)
                    .with_default(Value::F64(0.0))
                    .with_bound(real((-crate::floor::MAX_BULGE, crate::floor::MAX_BULGE))),
            ]))),
        )
        .with_bound(Bound::Length {
            max: MAX_POINTS as u64,
        })
        .optional()
        .in_aspect("geometry"),
        FieldDecl::new(
            "railing_type",
            ValueType::Reference({
                let mut spec = reference_to(RAILING_TYPE);
                spec.rule.cardinality = Cardinality::Optional;
                spec
            }),
        )
        .optional()
        .in_aspect("identity"),
        FieldDecl::new("host", ValueType::Reference(host))
            .optional()
            .in_aspect("geometry"),
        coded_field(
            "side",
            [
                (Side::Left.name(), Side::Left.code()),
                (Side::Right.name(), Side::Right.code()),
            ],
            Side::Left.code(),
        ),
        extension("start_extension"),
        extension("end_extension"),
        length_field("extension_length", Some(real(EXTENSION_RANGE)))
            .with_default(length(DEFAULT_EXTENSION)),
        FieldDecl::new("continues_to", ValueType::Reference(next))
            .optional()
            .in_aspect("geometry"),
        pinned_field(0),
        FieldDecl::new("host_edge", ValueType::I64)
            .with_bound(Bound::Int {
                min: 0,
                max: i64::try_from(crate::floor::MAX_VERTICES).unwrap_or(i64::MAX),
            })
            .optional()
            .in_aspect("geometry")
            .since_minor(1),
    ]
}

fn railing_type_fields() -> Vec<FieldDecl> {
    use crate::railing::{
        Corner, DEFAULT_BALUSTER_SIZE, DEFAULT_BALUSTER_SPACING, DEFAULT_HEIGHT, DEFAULT_POST_SIZE,
        DEFAULT_RAIL_DEPTH, DEFAULT_RAIL_WIDTH, HEIGHT_LIMITS, Profile, SECTION_RANGE,
        SPACING_RANGE,
    };
    let profile = |name: &str| {
        coded_field(
            name,
            [
                (Profile::Square.name(), Profile::Square.code()),
                (Profile::Round.name(), Profile::Round.code()),
            ],
            Profile::Square.code(),
        )
    };
    vec![
        name_field("identity"),
        length_field("height", Some(real(HEIGHT_LIMITS))).with_default(length(DEFAULT_HEIGHT)),
        profile("rail_profile"),
        length_field("rail_width", Some(real(SECTION_RANGE)))
            .with_default(length(DEFAULT_RAIL_WIDTH)),
        length_field("rail_depth", Some(real(SECTION_RANGE)))
            .with_default(length(DEFAULT_RAIL_DEPTH)),
        length_field("handrail_height", Some(real((0.0, HEIGHT_LIMITS.1)))),
        profile("baluster_profile"),
        length_field("baluster_size", Some(real(SECTION_RANGE)))
            .with_default(length(DEFAULT_BALUSTER_SIZE)),
        length_field("baluster_spacing", Some(real(SPACING_RANGE)))
            .with_default(length(DEFAULT_BALUSTER_SPACING)),
        length_field("post_size", Some(real(SECTION_RANGE)))
            .with_default(length(DEFAULT_POST_SIZE)),
        coded_field(
            "corner",
            [
                (Corner::Miter.name(), Corner::Miter.code()),
                (Corner::Fillet.name(), Corner::Fillet.code()),
            ],
            Corner::Miter.code(),
        ),
        length_field("fillet_radius", Some(real((0.0, 5.0)))).with_default(length(0.15)),
    ]
}

fn ramp_fields() -> Vec<FieldDecl> {
    use crate::ramp::{
        DEFAULT_LENGTH, DEFAULT_THICKNESS, DEFAULT_WIDTH, Form, LANDING_RANGE, LENGTH_RANGE,
        MAX_BOUNDARY, MAX_RUNS, RISE_LIMITS, SLOPE_RANGE, THICKNESS_RANGE as RAMP_THICKNESS,
        WIDTH_RANGE,
    };
    vec![
        name_field("identity"),
        length_field("x", Some(real(COORDINATE_RANGE))),
        length_field("y", Some(real(COORDINATE_RANGE))),
        FieldDecl::new("direction", ValueType::Quantity(Dimension::ANGLE))
            .with_default(Value::Quantity(Quantity::new(0.0, Dimension::ANGLE)))
            .with_bound(real((-1000.0, 1000.0)))
            .in_aspect("geometry"),
        length_field("width", Some(real(WIDTH_RANGE))).with_default(length(DEFAULT_WIDTH)),
        length_field("length", Some(real(LENGTH_RANGE))).with_default(length(DEFAULT_LENGTH)),
        length_field("landing_start", Some(real(LANDING_RANGE))),
        length_field("landing_end", Some(real(LANDING_RANGE))),
        length_field("thickness", Some(real(RAMP_THICKNESS)))
            .with_default(length(DEFAULT_THICKNESS)),
        coded_field(
            "form",
            [
                (Form::Slab.name(), Form::Slab.code()),
                (Form::Solid.name(), Form::Solid.code()),
            ],
            Form::Slab.code(),
        ),
        optional_length("rise", RISE_LIMITS),
        FieldDecl::new("top_level", ValueType::Reference(optional_level()))
            .optional()
            .in_aspect("geometry"),
        FieldDecl::new(
            "ramp_type",
            ValueType::Reference({
                let mut spec = reference_to(RAMP_TYPE);
                spec.rule.cardinality = Cardinality::Optional;
                spec
            }),
        )
        .optional()
        .in_aspect("identity"),
        pinned_field(0),
        FieldDecl::new("slope", ValueType::F64)
            .with_bound(real((0.0, SLOPE_RANGE.1)))
            .optional()
            .in_aspect("geometry")
            .since_minor(1),
        FieldDecl::new(
            "runs",
            ValueType::List(Box::new(ValueType::Record(vec![
                angle_field("turn"),
                length_field("length", Some(real(LENGTH_RANGE)))
                    .with_default(length(DEFAULT_LENGTH)),
                length_field("landing", Some(real(LANDING_RANGE))),
                FieldDecl::new("slope", ValueType::F64)
                    .with_bound(real((0.0, SLOPE_RANGE.1)))
                    .optional(),
            ]))),
        )
        .with_bound(Bound::Length {
            max: MAX_RUNS as u64,
        })
        .optional()
        .in_aspect("geometry")
        .since_minor(1),
        FieldDecl::new(
            "boundary",
            ValueType::List(Box::new(ValueType::Record(vec![
                length_field("x", Some(real(COORDINATE_RANGE))),
                length_field("y", Some(real(COORDINATE_RANGE))),
            ]))),
        )
        .with_bound(Bound::Length {
            max: MAX_BOUNDARY as u64,
        })
        .optional()
        .in_aspect("geometry")
        .since_minor(1),
    ]
}

fn ramp_type_fields() -> Vec<FieldDecl> {
    use crate::ramp::{
        DEFAULT_MAX_SLOPE, DEFAULT_THICKNESS, SLOPE_RANGE, THICKNESS_RANGE as RAMP_THICKNESS,
    };
    vec![
        name_field("identity"),
        FieldDecl::new("max_slope", ValueType::F64)
            .with_default(Value::F64(DEFAULT_MAX_SLOPE))
            .with_bound(real(SLOPE_RANGE))
            .in_aspect("geometry"),
        length_field("thickness", Some(real(RAMP_THICKNESS)))
            .with_default(length(DEFAULT_THICKNESS)),
    ]
}

/// The roof or floor a wall's top attaches to: cleared when it is deleted (the delete commands
/// report the walls they detach).
fn attach_target() -> ReferenceSpec {
    let mut spec = reference_to(ROOF);
    spec.targets = [ROOF, FLOOR]
        .iter()
        .filter_map(|k| TypeKey::new(*k).ok())
        .collect();
    spec.rule.on_delete = OnDelete::Clear;
    spec.rule.cardinality = Cardinality::Optional;
    spec
}

fn roof_fields() -> Vec<FieldDecl> {
    use crate::roof::{
        DEFAULT_OVERHANG, DEFAULT_SLOPE, DEFAULT_THICKNESS, MAX_EDGES, OVERHANG_RANGE, SLOPE_RANGE,
    };
    let mut wall = reference_to(WALL);
    wall.rule.on_delete = OnDelete::Clear;
    wall.rule.cardinality = Cardinality::Optional;
    let edge = ValueType::Record(vec![
        length_field("x", Some(real(COORDINATE_RANGE))),
        length_field("y", Some(real(COORDINATE_RANGE))),
        FieldDecl::new("wall", ValueType::Reference(wall)).optional(),
        FieldDecl::new(
            "face",
            ValueType::Enum(
                crate::floor::Face::ALL
                    .iter()
                    .map(|f| EnumVariant::new(f.name(), f.code()))
                    .collect(),
            ),
        )
        .with_default(Value::Enum(crate::floor::Face::Exterior.code())),
        length_field("offset", Some(real(BASE_OFFSET_RANGE))),
        FieldDecl::new("sloped", ValueType::Bool).with_default(Value::Bool(true)),
        FieldDecl::new("slope", ValueType::Quantity(Dimension::ANGLE))
            .with_default(crate::stair::angle(DEFAULT_SLOPE))
            .with_bound(real(SLOPE_RANGE)),
        length_field("overhang", Some(real(OVERHANG_RANGE))).optional(),
    ]);
    vec![
        name_field("identity"),
        length_field("thickness", Some(real(THICKNESS_RANGE)))
            .with_default(length(DEFAULT_THICKNESS)),
        length_field("base_offset", Some(real(BASE_OFFSET_RANGE))),
        FieldDecl::new("edges", ValueType::List(Box::new(edge)))
            .with_bound(Bound::Length {
                max: MAX_EDGES as u64,
            })
            .in_aspect("geometry"),
        length_field("overhang", Some(real(OVERHANG_RANGE))).with_default(length(DEFAULT_OVERHANG)),
        type_field("roof_type", ROOF_TYPE, 0),
        pinned_field(0),
    ]
}

fn room_fields() -> Vec<FieldDecl> {
    vec![
        name_field("identity"),
        text_field("number", 64),
        length_field("x", Some(real(COORDINATE_RANGE))),
        length_field("y", Some(real(COORDINATE_RANGE))),
        text_field("department", NAME_BYTES),
        text_field("floor_finish", NAME_BYTES),
        text_field("wall_finish", NAME_BYTES),
        text_field("ceiling_finish", NAME_BYTES),
        text_field("base_finish", NAME_BYTES),
        FieldDecl::new("upper_level", ValueType::Reference(optional_level()))
            .optional()
            .in_aspect("geometry"),
        length_field("upper_offset", Some(real(BASE_OFFSET_RANGE))),
        length_field("base_offset", Some(real(BASE_OFFSET_RANGE))),
        pinned_field(1),
    ]
}

/// The pinned flag of an element, appended in schema 1.`minor`: optional, so older files load
/// with every element unpinned.
fn pinned_field(minor: u32) -> FieldDecl {
    FieldDecl::new("pinned", ValueType::Bool)
        .optional()
        .in_aspect("identity")
        .since_minor(minor)
}

/// The position of the pinned flag in the fields of the element type `key`; `None` for a type
/// that cannot be pinned.
#[must_use]
pub fn pinned_slot(key: &str) -> Option<usize> {
    Some(match key {
        WALL => wall::PINNED,
        DOOR | WINDOW => insert::PINNED,
        FLOOR => floor::PINNED,
        ROOM => room::PINNED,
        STAIR => stair::PINNED,
        RAILING => railing::PINNED,
        RAMP => ramp::PINNED,
        ROOF => roof::PINNED,
        CEILING => ceiling::PINNED,
        WALL_OPENING => wall_opening::PINNED,
        SHAFT => shaft::PINNED,
        VERTICAL_OPENING => vertical_opening::PINNED,
        FLOOR_EDGE => floor_edge::PINNED,
        ELEVATOR => elevator::PINNED,
        DORMER => dormer::PINNED,
        ROOF_EDGE => roof_edge::PINNED,
        ROOF_EXTRUSION => roof_extrusion::PINNED,
        GRID => grid::PINNED,
        REFERENCE_PLANE => reference_plane::PINNED,
        _ => return None,
    })
}

/// Whether the element of type `key` with `values` is pinned.
#[must_use]
pub fn is_pinned(key: &str, values: &[Option<Value>]) -> bool {
    pinned_slot(key).is_some_and(|at| matches!(values.get(at), Some(Some(Value::Bool(true)))))
}

/// One boundary vertex of a floor: the loop, the authored point and the optional wall face the
/// edge that starts there follows.
fn vertex_type() -> ValueType {
    let mut wall = reference_to(WALL);
    wall.rule.on_delete = OnDelete::Clear;
    wall.rule.cardinality = Cardinality::Optional;
    ValueType::Record(vec![
        FieldDecl::new("loop", ValueType::I64).with_default(Value::I64(0)),
        length_field("x", Some(real(COORDINATE_RANGE))),
        length_field("y", Some(real(COORDINATE_RANGE))),
        FieldDecl::new("wall", ValueType::Reference(wall)).optional(),
        FieldDecl::new(
            "face",
            ValueType::Enum(
                crate::floor::Face::ALL
                    .iter()
                    .map(|f| EnumVariant::new(f.name(), f.code()))
                    .collect(),
            ),
        )
        .with_default(Value::Enum(crate::floor::Face::Interior.code())),
        length_field("offset", Some(real(BASE_OFFSET_RANGE))),
    ])
}

/// The reference of an instance to its type, appended in schema 1.`minor`: optional, so files
/// from before types load; a type in use cannot be deleted.
fn type_field(name: &str, target: &str, minor: u32) -> FieldDecl {
    let mut spec = reference_to(target);
    spec.rule.cardinality = Cardinality::Optional;
    FieldDecl::new(name, ValueType::Reference(spec))
        .optional()
        .in_aspect("identity")
        .since_minor(minor)
}

/// The fields of a door or a window, which share one layout; the host is the header owner.
fn insert_fields(type_key: &str) -> Vec<FieldDecl> {
    use crate::insert::{Facing, Hand, Operation};
    let variants = |items: Vec<(&'static str, u32)>| {
        ValueType::Enum(
            items
                .into_iter()
                .map(|(n, c)| EnumVariant::new(n, c))
                .collect(),
        )
    };
    vec![
        name_field("identity"),
        length_field("distance", Some(real(DISTANCE_RANGE))),
        length_field("width", Some(real(INSERT_WIDTH_RANGE))).with_default(length(0.9)),
        length_field("height", Some(real(INSERT_HEIGHT_RANGE))).with_default(length(2.1)),
        length_field("sill", Some(real(SILL_RANGE))),
        length_field("frame_width", Some(real(FRAME_WIDTH_RANGE))).with_default(length(0.05)),
        length_field("frame_depth", Some(real(FRAME_DEPTH_RANGE))).with_default(length(0.1)),
        FieldDecl::new(
            "operation",
            variants(
                Operation::ALL
                    .iter()
                    .map(|o| (o.name(), o.code()))
                    .collect(),
            ),
        )
        .with_default(Value::Enum(Operation::SingleSwing.code()))
        .in_aspect("geometry"),
        FieldDecl::new(
            "hand",
            variants(Hand::ALL.iter().map(|o| (o.name(), o.code())).collect()),
        )
        .with_default(Value::Enum(Hand::Left.code()))
        .in_aspect("geometry"),
        FieldDecl::new(
            "facing",
            variants(Facing::ALL.iter().map(|o| (o.name(), o.code())).collect()),
        )
        .with_default(Value::Enum(Facing::Exterior.code()))
        .in_aspect("geometry"),
        type_field("insert_type", type_key, 1),
        pinned_field(2),
    ]
}

/// The join style of one end of a wall, appended to the wall in schema 1.1.
fn join_field(name: &str) -> FieldDecl {
    FieldDecl::new(
        name,
        ValueType::Enum(
            crate::wall::JoinStyle::ALL
                .iter()
                .map(|s| EnumVariant::new(s.name(), s.code()))
                .collect(),
        ),
    )
    .with_default(Value::Enum(crate::wall::JoinStyle::Auto.code()))
    .in_aspect("geometry")
    .since_minor(1)
}

/// A text value, when the field is set to one.
#[must_use]
pub fn text(values: &[Option<Value>], at: usize) -> Option<&str> {
    match values.get(at)? {
        Some(Value::String(s)) => Some(s),
        _ => None,
    }
}

/// A length in meters, when the field is set to one.
#[must_use]
pub fn meters(values: &[Option<Value>], at: usize) -> Option<f64> {
    match values.get(at)? {
        Some(Value::Quantity(q)) => Some(q.value()),
        _ => None,
    }
}

/// An angle value of `radians` radians.
#[must_use]
pub fn angle(radians: f64) -> Value {
    crate::stair::angle(radians)
}

/// A length value of `meters` meters.
#[must_use]
pub fn length(meters: f64) -> Value {
    Value::Quantity(Quantity::new(meters, Dimension::LENGTH))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_storey_element_is_an_element_and_stairs_are_both() {
        assert!(STOREY_ELEMENTS.iter().all(|k| is_element(k)));
        assert!(is_element(STAIR) && is_storey_element(STAIR));
        assert!(!is_element(STOREY) && !is_storey_element(DOOR));
    }

    #[test]
    #[allow(clippy::too_many_lines, reason = "one table of every type")]
    fn field_positions_match_the_declarations() {
        let mut registry = TypeRegistry::new();
        register(&mut registry).unwrap();
        let insert_fields: &[(&str, usize)] = &[
            ("name", insert::NAME),
            ("distance", insert::DISTANCE),
            ("width", insert::WIDTH),
            ("height", insert::HEIGHT),
            ("sill", insert::SILL),
            ("frame_width", insert::FRAME_WIDTH),
            ("frame_depth", insert::FRAME_DEPTH),
            ("operation", insert::OPERATION),
            ("hand", insert::HAND),
            ("facing", insert::FACING),
            ("insert_type", insert::INSERT_TYPE),
            ("pinned", insert::PINNED),
        ];
        let expect: [(&str, &[(&str, usize)]); 17] = [
            (
                ROOF,
                &[
                    ("name", roof::NAME),
                    ("thickness", roof::THICKNESS),
                    ("base_offset", roof::BASE_OFFSET),
                    ("edges", roof::EDGES),
                    ("overhang", roof::OVERHANG),
                    ("roof_type", roof::ROOF_TYPE),
                    ("pinned", roof::PINNED),
                ],
            ),
            (
                STAIR,
                &[
                    ("name", stair::NAME),
                    ("shape", stair::SHAPE),
                    ("turn", stair::TURN),
                    ("x", stair::X),
                    ("y", stair::Y),
                    ("direction", stair::DIRECTION),
                    ("width", stair::WIDTH),
                    ("tread", stair::TREAD),
                    ("max_riser", stair::MAX_RISER),
                    ("waist", stair::WAIST),
                    ("railing_left", stair::RAILING_LEFT),
                    ("railing_right", stair::RAILING_RIGHT),
                    ("top_level", stair::TOP_LEVEL),
                    ("pinned", stair::PINNED),
                    ("runs", stair::RUNS),
                    ("headroom", stair::HEADROOM),
                ],
            ),
            (
                ELEVATOR,
                &[
                    ("name", elevator::NAME),
                    ("x", elevator::X),
                    ("y", elevator::Y),
                    ("direction", elevator::DIRECTION),
                    ("car_width", elevator::CAR_WIDTH),
                    ("car_depth", elevator::CAR_DEPTH),
                    ("clearance", elevator::CLEARANCE),
                    ("door_width", elevator::DOOR_WIDTH),
                    ("pit", elevator::PIT),
                    ("overrun", elevator::OVERRUN),
                    ("top_level", elevator::TOP_LEVEL),
                    ("stops", elevator::STOPS),
                    ("pinned", elevator::PINNED),
                ],
            ),
            (
                DORMER,
                &[
                    ("name", dormer::NAME),
                    ("x", dormer::X),
                    ("y", dormer::Y),
                    ("direction", dormer::DIRECTION),
                    ("width", dormer::WIDTH),
                    ("depth", dormer::DEPTH),
                    ("pitch", dormer::PITCH),
                    ("style", dormer::STYLE),
                    ("wall_height", dormer::WALL_HEIGHT),
                    ("thickness", dormer::THICKNESS),
                    ("pinned", dormer::PINNED),
                ],
            ),
            (
                ROOF_EDGE,
                &[
                    ("name", roof_edge::NAME),
                    ("role", roof_edge::ROLE),
                    ("profile", roof_edge::PROFILE),
                    ("edges", roof_edge::EDGES),
                    ("pinned", roof_edge::PINNED),
                ],
            ),
            (
                ROOM,
                &[
                    ("name", room::NAME),
                    ("number", room::NUMBER),
                    ("x", room::X),
                    ("y", room::Y),
                    ("department", room::DEPARTMENT),
                    ("floor_finish", room::FLOOR_FINISH),
                    ("wall_finish", room::WALL_FINISH),
                    ("ceiling_finish", room::CEILING_FINISH),
                    ("base_finish", room::BASE_FINISH),
                    ("upper_level", room::UPPER_LEVEL),
                    ("upper_offset", room::UPPER_OFFSET),
                    ("base_offset", room::BASE_OFFSET),
                    ("pinned", room::PINNED),
                ],
            ),
            (
                FLOOR,
                &[
                    ("name", floor::NAME),
                    ("thickness", floor::THICKNESS),
                    ("top_offset", floor::TOP_OFFSET),
                    ("vertices", floor::VERTICES),
                    ("floor_type", floor::FLOOR_TYPE),
                    ("pinned", floor::PINNED),
                    ("arcs", floor::ARCS),
                    ("slope_x", floor::SLOPE_X),
                    ("slope_y", floor::SLOPE_Y),
                    ("slope_direction", floor::SLOPE_DIRECTION),
                    ("slope_length", floor::SLOPE_LENGTH),
                    ("slope", floor::SLOPE),
                    ("end_height", floor::END_HEIGHT),
                ],
            ),
            (DOOR, insert_fields),
            (WINDOW, insert_fields),
            (
                PROJECT,
                &[
                    ("name", project::NAME),
                    ("number", project::NUMBER),
                    ("client", project::CLIENT),
                    ("address", project::ADDRESS),
                    ("length_unit", project::LENGTH_UNIT),
                    ("template", project::TEMPLATE),
                    ("slope_unit", project::SLOPE_UNIT),
                    ("base_x", project::BASE_X),
                    ("base_y", project::BASE_Y),
                    ("base_z", project::BASE_Z),
                    ("survey_x", project::SURVEY_X),
                    ("survey_y", project::SURVEY_Y),
                    ("survey_z", project::SURVEY_Z),
                    ("true_north", project::TRUE_NORTH),
                    ("map_scale", project::SCALE),
                    ("crs", project::CRS),
                    ("eastings", project::EASTINGS),
                    ("northings", project::NORTHINGS),
                    ("survey_height", project::HEIGHT),
                    ("precision_distance", project::PRECISION_DISTANCE),
                    ("area_unit", project::AREA_UNIT),
                    ("volume_unit", project::VOLUME_UNIT),
                    ("angle_unit", project::ANGLE_UNIT),
                    ("length_precision", project::LENGTH_PRECISION),
                    ("area_precision", project::AREA_PRECISION),
                    ("volume_precision", project::VOLUME_PRECISION),
                    ("angle_precision", project::ANGLE_PRECISION),
                    ("slope_precision", project::SLOPE_PRECISION),
                    ("length_rounding", project::LENGTH_ROUNDING),
                    ("area_rounding", project::AREA_ROUNDING),
                    ("volume_rounding", project::VOLUME_ROUNDING),
                    ("angle_rounding", project::ANGLE_ROUNDING),
                    ("slope_rounding", project::SLOPE_ROUNDING),
                    ("status", project::STATUS),
                    ("issue_date", project::ISSUE_DATE),
                    ("author", project::AUTHOR),
                    ("organization", project::ORGANIZATION),
                    ("description", project::DESCRIPTION),
                    ("long_name", project::LONG_NAME),
                    ("occupancy_type", project::OCCUPANCY_TYPE),
                    ("year_of_construction", project::YEAR_OF_CONSTRUCTION),
                    ("construction_method", project::CONSTRUCTION_METHOD),
                    ("template_source", project::TEMPLATE_SOURCE),
                ],
            ),
            (SITE, &[("name", site::NAME)]),
            (
                BUILDING,
                &[
                    ("name", building::NAME),
                    ("elevation", building::ELEVATION),
                    ("address", building::ADDRESS),
                ],
            ),
            (STOREY, &[("name", storey::NAME), ("level", storey::LEVEL)]),
            (
                LEVEL,
                &[
                    ("name", level::NAME),
                    ("elevation", level::ELEVATION),
                    ("elevation_base", level::ELEVATION_BASE),
                    ("storey", level::STOREY),
                ],
            ),
            (
                WALL,
                &[
                    ("name", wall::NAME),
                    ("start_x", wall::START_X),
                    ("start_y", wall::START_Y),
                    ("end_x", wall::END_X),
                    ("end_y", wall::END_Y),
                    ("thickness", wall::THICKNESS),
                    ("height", wall::HEIGHT),
                    ("base_offset", wall::BASE_OFFSET),
                    ("location_line", wall::LOCATION_LINE),
                    ("top_level", wall::TOP_LEVEL),
                    ("start_join", wall::START_JOIN),
                    ("end_join", wall::END_JOIN),
                    ("wall_type", wall::WALL_TYPE),
                    ("core_exterior", wall::CORE_EXTERIOR),
                    ("core_interior", wall::CORE_INTERIOR),
                    ("pinned", wall::PINNED),
                    ("top_offset", wall::TOP_OFFSET),
                    ("top_attach", wall::TOP_ATTACH),
                    ("grid_edits", wall::GRID_EDITS),
                    ("cell_overrides", wall::CELL_OVERRIDES),
                    ("bulge", wall::BULGE),
                    ("lean_exterior", wall::LEAN_EXTERIOR),
                    ("lean_interior", wall::LEAN_INTERIOR),
                    ("profile", wall::PROFILE),
                    ("bands", wall::BANDS),
                ],
            ),
            (
                WALL_TYPE,
                &[
                    ("name", layered_type::NAME),
                    ("layers", layered_type::LAYERS),
                    ("core_start", layered_type::CORE_START),
                    ("core_end", layered_type::CORE_END),
                    ("exposure", layered_type::EXPOSURE),
                    ("load_bearing", layered_type::LOAD_BEARING),
                    ("room_bounding", layered_type::ROOM_BOUNDING),
                    ("wrap_ends", layered_type::WRAP_ENDS),
                    ("wrap_inserts", layered_type::WRAP_INSERTS),
                    ("curtain", crate::curtain::slot::CURTAIN),
                    ("vertical_rule", crate::curtain::slot::VERTICAL),
                    ("vertical_value", crate::curtain::slot::VERTICAL + 1),
                    ("vertical_justify", crate::curtain::slot::VERTICAL + 2),
                    ("vertical_offset", crate::curtain::slot::VERTICAL + 3),
                    ("horizontal_rule", crate::curtain::slot::HORIZONTAL),
                    ("horizontal_value", crate::curtain::slot::HORIZONTAL + 1),
                    ("horizontal_justify", crate::curtain::slot::HORIZONTAL + 2),
                    ("horizontal_offset", crate::curtain::slot::HORIZONTAL + 3),
                    ("panel", crate::curtain::slot::PANEL),
                    ("panel_thickness", crate::curtain::slot::PANEL_THICKNESS),
                    ("panel_material", crate::curtain::slot::PANEL_MATERIAL),
                    ("interior_profile", crate::curtain::slot::INTERIOR),
                    ("interior_width", crate::curtain::slot::INTERIOR + 1),
                    ("interior_depth", crate::curtain::slot::INTERIOR + 2),
                    ("border_profile", crate::curtain::slot::BORDER),
                    ("border_width", crate::curtain::slot::BORDER + 1),
                    ("border_depth", crate::curtain::slot::BORDER + 2),
                    ("corner_profile", crate::curtain::slot::CORNER),
                    ("corner_width", crate::curtain::slot::CORNER + 1),
                    ("corner_depth", crate::curtain::slot::CORNER + 2),
                    ("priority", crate::curtain::slot::PRIORITY),
                    ("vertical_angle", crate::curtain::slot::VERTICAL_ANGLE),
                    ("horizontal_angle", crate::curtain::slot::HORIZONTAL_ANGLE),
                    ("stack", crate::element_type::STACK),
                ],
            ),
            (
                DOOR_TYPE,
                &[
                    ("name", insert_type::NAME),
                    ("width", insert_type::WIDTH),
                    ("height", insert_type::HEIGHT),
                    ("frame_width", insert_type::FRAME_WIDTH),
                    ("frame_depth", insert_type::FRAME_DEPTH),
                    ("operation", insert_type::OPERATION),
                    ("sill", insert_type::SILL),
                ],
            ),
        ];
        for (key, fields) in expect {
            let decl = registry.get(key).unwrap();
            for (name, at) in fields {
                assert_eq!(decl.schema.fields[*at].name, *name, "{key}");
            }
            // A type whose sketch is made on a work plane ends with the reference to it, then a
            // physical element or element type with its parameter values.
            let stored = usize::from(ON_WORK_PLANE.contains(&key));
            let values = usize::from(PARAMETER_HOSTS.contains(&key) || TYPE_KEYS.contains(&key));
            assert_eq!(
                decl.schema.fields.len(),
                fields.len() + stored + values,
                "{key}"
            );
            if stored == 1 {
                assert_eq!(decl.schema.fields[fields.len()].name, "work_plane", "{key}");
            }
            if values == 1 {
                assert_eq!(
                    decl.schema.fields.last().unwrap().name,
                    "parameters",
                    "{key}"
                );
            }
        }
        assert_eq!(
            registry.get(ROOF_EXTRUSION).unwrap().schema.fields[roof_extrusion::WORK_PLANE].name,
            "work_plane"
        );
    }
}
