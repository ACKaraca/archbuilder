//! The object types of the spatial structure, as the domain model document defines them:
//! `Project`, `Site`, `Building`, `Storey` and `Level`, the `Wall` that stands on a storey and the
//! `Door` and `Window` that a wall hosts.
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

/// Every type key of the model, from the root down.
pub const KEYS: [&str; 9] = [
    PROJECT, SITE, BUILDING, STOREY, LEVEL, WALL, DOOR, WINDOW, FLOOR,
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

/// The range of a floor's thickness, in meters.
pub const FLOOR_THICKNESS_RANGE: (f64, f64) = (0.001, 5.0);
/// The thickness of a floor created without one, in meters.
pub const DEFAULT_FLOOR_THICKNESS: f64 = 0.2;

/// The elevation base code of the project origin.
pub const ELEVATION_BASE_PROJECT: u32 = 0;
/// The elevation base code of the survey point.
pub const ELEVATION_BASE_SURVEY: u32 = 1;

fn name_field(aspect: &str) -> FieldDecl {
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

fn length_field(name: &str, bound: Option<Bound>) -> FieldDecl {
    let field = FieldDecl::new(name, ValueType::Quantity(Dimension::LENGTH))
        .with_default(Value::Quantity(Quantity::new(0.0, Dimension::LENGTH)))
        .in_aspect("geometry");
    match bound {
        Some(bound) => field.with_bound(bound),
        None => field,
    }
}

fn real((min, max): (f64, f64)) -> Bound {
    Bound::Real { min, max }
}

/// An optional reference to a level that is dropped when the level is deleted.
fn optional_level() -> ReferenceSpec {
    let mut spec = reference_to(LEVEL);
    spec.rule.on_delete = OnDelete::Clear;
    spec.rule.cardinality = Cardinality::Optional;
    spec
}

fn reference_to(target: &str) -> ReferenceSpec {
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
    ];
    for (key, fields) in [
        (FLOOR, floor),
        (DOOR, insert_fields()),
        (WINDOW, insert_fields()),
        (PROJECT, project),
        (SITE, site),
        (BUILDING, building),
        (STOREY, storey),
        (LEVEL, level),
        (WALL, wall),
    ] {
        registry.register(declaration(key, fields)?)?;
    }
    Ok(())
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

/// The fields of a door or a window, which share one layout; the host is the header owner.
fn insert_fields() -> Vec<FieldDecl> {
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

/// A length value of `meters` meters.
#[must_use]
pub fn length(meters: f64) -> Value {
    Value::Quantity(Quantity::new(meters, Dimension::LENGTH))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
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
        ];
        let expect: [(&str, &[(&str, usize)]); 9] = [
            (
                FLOOR,
                &[
                    ("name", floor::NAME),
                    ("thickness", floor::THICKNESS),
                    ("top_offset", floor::TOP_OFFSET),
                    ("vertices", floor::VERTICES),
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
