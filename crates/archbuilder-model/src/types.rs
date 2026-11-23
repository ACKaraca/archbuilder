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

/// A roof by footprint on a storey: edges with slopes and overhangs, a base offset and a type.
pub const ROOF: &str = "archbuilder.roof";

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

/// The element types, in the order of [`crate::element_type::TypeKind::ALL`].
pub const TYPE_KEYS: [&str; 5] = [WALL_TYPE, FLOOR_TYPE, DOOR_TYPE, WINDOW_TYPE, ROOF_TYPE];

/// The building elements: what is selected, copied, hidden, moved and deleted as an object of a
/// storey (doors and windows through their wall). One list, so a new kind is added once.
pub const ELEMENTS: [&str; 7] = [WALL, DOOR, WINDOW, FLOOR, ROOM, STAIR, ROOF];

/// The elements a storey owns directly.
pub const STOREY_ELEMENTS: [&str; 5] = [WALL, FLOOR, ROOM, STAIR, ROOF];

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
pub const KEYS: [&str; 17] = [
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
}

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
    let project = vec![
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
    ];
    let floor = vec![
        name_field("identity"),
        length_field("thickness", Some(real(FLOOR_THICKNESS_RANGE)))
            .with_default(length(DEFAULT_FLOOR_THICKNESS)),
        length_field("top_offset", Some(real(BASE_OFFSET_RANGE))),
        FieldDecl::new("vertices", ValueType::List(Box::new(vertex_type())))
            .with_bound(Bound::Length {
                max: crate::floor::MAX_VERTICES as u64,
            })
            .in_aspect("geometry"),
        type_field("floor_type", FLOOR_TYPE, 1),
        pinned_field(2),
    ];
    for (key, fields) in [
        (ROOF, roof_fields()),
        (ROOF_TYPE, crate::element_type::floor_type_fields()),
        (STAIR, stair_fields()),
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
        let mut decl = declaration(key, fields)?;
        match key {
            WALL_TYPE | FLOOR_TYPE | ROOF_TYPE => {
                decl.validators.push(crate::element_type::layer_findings);
            }
            DOOR_TYPE | WINDOW_TYPE => decl.validators.push(crate::element_type::size_findings),
            _ => {}
        }
        registry.register(decl)?;
    }
    Ok(())
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
        ROOF => roof::PINNED,
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
        let expect: [(&str, &[(&str, usize)]); 14] = [
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
            assert_eq!(decl.schema.fields.len(), fields.len(), "{key}");
        }
    }
}
