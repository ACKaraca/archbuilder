//! Element types: the wall, floor, door, window and roof types a project owns and its instances refer
//! to (fixed-parameter types of the built-in kinds; there is no family editor).
//!
//! A wall type, a floor type and a roof type are an ordered list of layers ([`Layer`]: a function, a material
//! name and a thickness; a membrane has no thickness) and a core, the index range of the layers
//! that form the core ([`Layers`]). A wall's layers run from its exterior face to its interior
//! face, a floor's from the top down. A wall type also carries its function flags: exterior or
//! interior ([`Exposure`]), load-bearing and room-bounding. A door type and a window type hold the
//! size, the frame, the operation and the sill height a new instance starts with ([`InsertType`]).
//!
//! An instance refers to its type. The values the type decides are also kept on the instance so
//! that its geometry reads from the instance alone: a wall's thickness and the thickness of the
//! layers outside the core on either side, a floor's thickness, a door's or window's width,
//! height, frame and operation ([`copies`]). Only the commands that assign or change a type write
//! them, and every commit checks that they match the type ([`mismatch`]).

use std::sync::Arc;

use arc_base::{Error, ObjectId};
use arc_doc::{
    Bound, EnumVariant, FieldDecl, Finding, Object, Severity, Snapshot, Value, ValueType,
};

use crate::error::codes;
use crate::insert::{Kind, Operation};
use crate::roof::RafterCut;
use crate::types::{
    self, CEILING, CEILING_TYPE, DOOR, DOOR_TYPE, FLOOR, FLOOR_TYPE, FRAME_DEPTH_RANGE,
    FRAME_WIDTH_RANGE, INSERT_HEIGHT_RANGE, INSERT_WIDTH_RANGE, NAME_BYTES, ROOF, ROOF_TYPE,
    SILL_RANGE, THICKNESS_RANGE, WALL, WALL_TYPE, WINDOW, WINDOW_TYPE, insert_type, layered_type,
};

/// The most layers a type has.
pub const MAX_LAYERS: usize = 32;
/// The thickest single layer, in meters.
pub const MAX_LAYER_THICKNESS: f64 = 5.0;
/// How close a copy on an instance must be to its type's value, in meters.
const SAME: f64 = 1e-9;

coded! {
    /// What a layer does in the assembly.
    Function {
        /// Carries the loads: the core of most assemblies.
        #[default]
        Structure = 0, "structure";
        /// A base for another layer, such as a board or a screed.
        Substrate = 1, "substrate";
        /// Thermal insulation or an air gap.
        Thermal = 2, "thermal";
        /// A membrane: a vapor or water barrier without thickness.
        Membrane = 3, "membrane";
        /// The visible finish.
        Finish = 4, "finish";
    }
}

coded! {
    /// Whether a wall type is for exterior or interior walls.
    Exposure {
        /// An exterior wall: one face is outside.
        #[default]
        Exterior = 0, "exterior";
        /// An interior wall.
        Interior = 1, "interior";
    }
}

coded! {
    /// The sides of a wall whose finish layers wrap around a free end or the reveal of an opening.
    Wrap {
        /// No layer wraps.
        #[default]
        None = 0, "none";
        /// The finish layers on the exterior side wrap.
        Exterior = 1, "exterior";
        /// The finish layers on the interior side wrap.
        Interior = 2, "interior";
        /// The finish layers on both sides wrap.
        Both = 3, "both";
    }
}

impl Wrap {
    /// Whether the exterior side wraps.
    #[must_use]
    pub const fn exterior(self) -> bool {
        matches!(self, Self::Exterior | Self::Both)
    }

    /// Whether the interior side wraps.
    #[must_use]
    pub const fn interior(self) -> bool {
        matches!(self, Self::Interior | Self::Both)
    }
}

/// The kinds of element type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TypeKind {
    /// Wall types.
    Wall,
    /// Floor types.
    Floor,
    /// Door types.
    Door,
    /// Window types.
    Window,
    /// Roof types.
    Roof,
    /// Ceiling types.
    Ceiling,
    /// Family types (loaded families, ADR 0009).
    Family,
}

impl TypeKind {
    /// Every kind, in the order of [`types::TYPE_KEYS`], then the family types.
    pub const ALL: [Self; 7] = [
        Self::Wall,
        Self::Floor,
        Self::Door,
        Self::Window,
        Self::Roof,
        Self::Ceiling,
        Self::Family,
    ];

    /// The key of the type records.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Wall => WALL_TYPE,
            Self::Floor => FLOOR_TYPE,
            Self::Door => DOOR_TYPE,
            Self::Window => WINDOW_TYPE,
            Self::Roof => ROOF_TYPE,
            Self::Ceiling => CEILING_TYPE,
            Self::Family => types::FAMILY_TYPE,
        }
    }

    /// The key of the instances.
    #[must_use]
    pub const fn instance_key(self) -> &'static str {
        match self {
            Self::Wall => WALL,
            Self::Floor => FLOOR,
            Self::Door => DOOR,
            Self::Window => WINDOW,
            Self::Roof => ROOF,
            Self::Ceiling => CEILING,
            Self::Family => types::COMPONENT,
        }
    }

    /// The kind whose type records have the key `key`.
    #[must_use]
    pub fn of_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.key() == key)
    }

    /// The kind whose instances have the key `key`.
    #[must_use]
    pub fn of_instance_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.instance_key() == key)
    }

    /// The position of an instance's reference to its type.
    #[must_use]
    pub const fn field(self) -> usize {
        match self {
            Self::Wall => types::wall::WALL_TYPE,
            Self::Floor => types::floor::FLOOR_TYPE,
            Self::Door | Self::Window => types::insert::INSERT_TYPE,
            Self::Roof => types::roof::ROOF_TYPE,
            Self::Ceiling => types::ceiling::CEILING_TYPE,
            Self::Family => types::component::FAMILY_TYPE,
        }
    }

    /// The name of an instance's reference to its type.
    #[must_use]
    pub const fn field_name(self) -> &'static str {
        match self {
            Self::Wall => "wall_type",
            Self::Floor => "floor_type",
            Self::Door | Self::Window => "insert_type",
            Self::Roof => "roof_type",
            Self::Ceiling => "ceiling_type",
            Self::Family => "family_type",
        }
    }

    /// Whether the types of this kind have layers.
    #[must_use]
    pub const fn layered(self) -> bool {
        matches!(self, Self::Wall | Self::Floor | Self::Roof | Self::Ceiling)
    }

    /// The insert kind of a door or window type.
    #[must_use]
    pub const fn insert(self) -> Option<Kind> {
        match self {
            Self::Door => Some(Kind::Door),
            Self::Window => Some(Kind::Window),
            Self::Wall | Self::Floor | Self::Roof | Self::Ceiling | Self::Family => None,
        }
    }
}

/// One layer of a wall or floor type.
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    /// What the layer does.
    pub function: Function,
    /// The material's name.
    pub material: String,
    /// The thickness in meters; zero for a membrane.
    pub thickness: f64,
}

impl Layer {
    /// A layer.
    #[must_use]
    pub fn new(function: Function, material: &str, thickness: f64) -> Self {
        Self {
            function,
            material: material.to_owned(),
            thickness,
        }
    }
}

/// The layers of a wall or floor type and its core: `core.0` is the first core layer and `core.1`
/// the index after the last.
#[derive(Debug, Clone, PartialEq)]
pub struct Layers {
    /// The layers, from the exterior face (wall) or the top (floor).
    pub layers: Vec<Layer>,
    /// The core: the index range of the core layers.
    pub core: (usize, usize),
}

impl Layers {
    /// One structure layer of `thickness`, all core.
    #[must_use]
    pub fn single(material: &str, thickness: f64) -> Self {
        Self {
            layers: vec![Layer::new(Function::Structure, material, thickness)],
            core: (0, 1),
        }
    }

    /// The layers of a type record's values; `None` when the list is missing or malformed.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Option<Self> {
        let Some(Some(Value::List(items))) = values.get(layered_type::LAYERS) else {
            return None;
        };
        let mut layers = Vec::with_capacity(items.len().min(MAX_LAYERS + 1));
        for item in items.iter().take(MAX_LAYERS + 1) {
            let Value::Record(fields) = item else {
                return None;
            };
            layers.push(Layer {
                function: Function::of_field(fields, layered_type::layer::FUNCTION),
                material: types::text(fields, layered_type::layer::MATERIAL)
                    .unwrap_or_default()
                    .to_owned(),
                thickness: types::meters(fields, layered_type::layer::THICKNESS)?,
            });
        }
        let index = |at| match values.get(at) {
            Some(Some(Value::I64(n))) => usize::try_from(*n).ok(),
            _ => None,
        };
        Some(Self {
            layers,
            core: (
                index(layered_type::CORE_START)?,
                index(layered_type::CORE_END)?,
            ),
        })
    }

    /// The value of the record's layer list.
    #[must_use]
    pub fn list_value(&self) -> Value {
        Value::List(
            self.layers
                .iter()
                .map(|l| {
                    Value::Record(vec![
                        Some(Value::Enum(l.function.code())),
                        Some(Value::String(l.material.trim().to_owned())),
                        Some(types::length(l.thickness)),
                    ])
                })
                .collect(),
        )
    }

    /// The values of the record's core fields: start and end.
    #[must_use]
    pub fn core_values(&self) -> [Value; 2] {
        let int = |n: usize| Value::I64(i64::try_from(n).unwrap_or(i64::MAX));
        [int(self.core.0), int(self.core.1)]
    }

    /// The total thickness: the sum of the layers.
    #[must_use]
    pub fn total(&self) -> f64 {
        self.layers.iter().map(|l| l.thickness).sum()
    }

    /// The thickness of the layers before the core (the exterior side of a wall, the top of a
    /// floor) and after it.
    #[must_use]
    pub fn core_offsets(&self) -> [f64; 2] {
        let before = self.layers.iter().take(self.core.0).map(|l| l.thickness);
        let after = self.layers.iter().skip(self.core.1).map(|l| l.thickness);
        [before.sum(), after.sum()]
    }

    /// The distance of each boundary between two layers from the first face, in order: one
    /// fewer than the layers.
    #[must_use]
    pub fn boundaries(&self) -> Vec<f64> {
        let mut at = 0.0;
        let mut out = Vec::with_capacity(self.layers.len().saturating_sub(1));
        for layer in self.layers.iter().take(self.layers.len().saturating_sub(1)) {
            at += layer.thickness;
            out.push(at);
        }
        out
    }

    /// The rules of a layer list, each broken one as an error that says what to change: between
    /// 1 and [`MAX_LAYERS`] layers; every layer thicker than zero and at most
    /// [`MAX_LAYER_THICKNESS`], except a membrane, which has no thickness; a core of at least one
    /// layer inside the list; a total thickness within [`THICKNESS_RANGE`].
    #[must_use]
    pub fn problems(&self) -> Vec<Error> {
        let mut out = Vec::new();
        let count = self.layers.len();
        if count == 0 || count > MAX_LAYERS {
            out.push(
                Error::new(codes::TYPE_LAYER_COUNT)
                    .with_arg("count", i64::try_from(count).unwrap_or(i64::MAX))
                    .with_arg("most", i64::try_from(MAX_LAYERS).unwrap_or(i64::MAX)),
            );
            return out;
        }
        for (at, layer) in self.layers.iter().enumerate() {
            let number = i64::try_from(at + 1).unwrap_or(i64::MAX);
            let t = layer.thickness;
            let ok = if layer.function == Function::Membrane {
                t == 0.0
            } else {
                t.is_finite() && t > 0.0 && t <= MAX_LAYER_THICKNESS
            };
            if !ok {
                let code = if layer.function == Function::Membrane {
                    codes::TYPE_MEMBRANE_THICKNESS
                } else {
                    codes::TYPE_LAYER_THICKNESS
                };
                out.push(
                    Error::new(code)
                        .with_arg("layer", number)
                        .with_arg("most", format!("{MAX_LAYER_THICKNESS}")),
                );
            }
        }
        let (start, end) = self.core;
        if start >= end || end > count {
            out.push(Error::new(codes::TYPE_CORE));
        }
        let total = self.total();
        if out.is_empty() && !(THICKNESS_RANGE.0..=THICKNESS_RANGE.1).contains(&total) {
            out.push(
                Error::new(codes::TYPE_TOTAL)
                    .with_arg("total", format!("{:.0}", total * 1000.0))
                    .with_arg("least", format!("{:.0}", THICKNESS_RANGE.0 * 1000.0))
                    .with_arg("most", format!("{:.0}", THICKNESS_RANGE.1 * 1000.0)),
            );
        }
        out
    }
}

/// A wall type's layers and function flags.
#[derive(Debug, Clone, PartialEq)]
pub struct WallType {
    /// The layers and the core.
    pub layers: Layers,
    /// Exterior or interior.
    pub exposure: Exposure,
    /// Whether the wall carries loads.
    pub load_bearing: bool,
    /// Whether the wall bounds rooms.
    pub room_bounding: bool,
    /// The sides whose finish layers wrap around free ends.
    pub wrap_ends: Wrap,
    /// The sides whose finish layers wrap around the openings of doors and windows.
    pub wrap_inserts: Wrap,
}

impl WallType {
    /// The wall type of a record's values.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Option<Self> {
        let flag = |at, default| match values.get(at) {
            Some(Some(Value::Bool(b))) => *b,
            _ => default,
        };
        Some(Self {
            layers: Layers::from_values(values)?,
            exposure: Exposure::of_field(values, layered_type::EXPOSURE),
            load_bearing: flag(layered_type::LOAD_BEARING, false),
            room_bounding: flag(layered_type::ROOM_BOUNDING, true),
            wrap_ends: Wrap::of_field(values, layered_type::WRAP_ENDS),
            wrap_inserts: Wrap::of_field(values, layered_type::WRAP_INSERTS),
        })
    }
}

/// A door or window type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InsertType {
    /// The width of the opening.
    pub width: f64,
    /// The height of the opening.
    pub height: f64,
    /// The width of the frame bars.
    pub frame_width: f64,
    /// The depth of the frame across the wall.
    pub frame_depth: f64,
    /// The operation.
    pub operation: Operation,
    /// The sill height a new instance gets.
    pub sill: f64,
}

impl InsertType {
    /// The door or window type of a record's values.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Option<Self> {
        let m = |at| types::meters(values, at);
        Some(Self {
            width: m(insert_type::WIDTH)?,
            height: m(insert_type::HEIGHT)?,
            frame_width: m(insert_type::FRAME_WIDTH)?,
            frame_depth: m(insert_type::FRAME_DEPTH)?,
            operation: Operation::of_field(values, insert_type::OPERATION),
            sill: m(insert_type::SILL)?,
        })
    }

    /// The record values of the type named `name`.
    #[must_use]
    pub fn values(&self, name: &str) -> Vec<Option<Value>> {
        vec![
            Some(Value::String(name.trim().to_owned())),
            Some(types::length(self.width)),
            Some(types::length(self.height)),
            Some(types::length(self.frame_width)),
            Some(types::length(self.frame_depth)),
            Some(Value::Enum(self.operation.code())),
            Some(types::length(self.sill)),
        ]
    }
}

/// The record values of a wall type named `name`.
#[must_use]
pub fn wall_type_values(name: &str, wall: &WallType) -> Vec<Option<Value>> {
    let [start, end] = wall.layers.core_values();
    vec![
        Some(Value::String(name.trim().to_owned())),
        Some(wall.layers.list_value()),
        Some(start),
        Some(end),
        Some(Value::Enum(wall.exposure.code())),
        Some(Value::Bool(wall.load_bearing)),
        Some(Value::Bool(wall.room_bounding)),
        Some(Value::Enum(wall.wrap_ends.code())),
        Some(Value::Enum(wall.wrap_inserts.code())),
    ]
}

/// The record values of a floor type named `name`.
#[must_use]
pub fn floor_type_values(name: &str, layers: &Layers) -> Vec<Option<Value>> {
    let [start, end] = layers.core_values();
    vec![
        Some(Value::String(name.trim().to_owned())),
        Some(layers.list_value()),
        Some(start),
        Some(end),
    ]
}

/// The grid pattern of a ceiling type: tile size along the two axes and the angle of the first
/// axis from the x axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grid {
    /// The tile size along the first axis and along the second, in meters.
    pub spacing: [f64; 2],
    /// The angle of the first axis from the x axis, in radians.
    pub angle: f64,
}

/// A ceiling type: layers, the height a new ceiling gets and the grid pattern.
#[derive(Debug, Clone, PartialEq)]
pub struct CeilingType {
    /// The layers, from the top down.
    pub layers: Layers,
    /// The height of the underside above the level, in meters.
    pub height_offset: f64,
    /// The grid pattern; `None` for a plain ceiling.
    pub grid: Option<Grid>,
}

impl CeilingType {
    /// The type of a record's values.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Option<Self> {
        let m = |at| types::meters(values, at);
        let angle = match values.get(types::ceiling_type::GRID_ANGLE) {
            Some(Some(Value::Quantity(q))) => q.value(),
            _ => 0.0,
        };
        let spacing = [
            m(types::ceiling_type::GRID_X).unwrap_or(0.0),
            m(types::ceiling_type::GRID_Y).unwrap_or(0.0),
        ];
        Some(Self {
            layers: Layers::from_values(values)?,
            height_offset: m(types::ceiling_type::HEIGHT_OFFSET)
                .unwrap_or(types::DEFAULT_CEILING_HEIGHT),
            grid: (spacing[0] > 1.0e-3 && spacing[1] > 1.0e-3).then_some(Grid { spacing, angle }),
        })
    }
}

/// The record values of a ceiling type called `name`.
#[must_use]
pub fn ceiling_type_values(name: &str, t: &CeilingType) -> Vec<Option<Value>> {
    let mut values = floor_type_values(name, &t.layers);
    let (spacing, angle) = t.grid.map_or(([0.0; 2], 0.0), |g| (g.spacing, g.angle));
    values.extend([
        Some(types::length(t.height_offset)),
        Some(types::length(spacing[0])),
        Some(types::length(spacing[1])),
        Some(types::angle(angle)),
    ]);
    values
}

/// The name of a type record.
#[must_use]
pub fn name(object: &Object) -> &str {
    types::text(&object.values, 0).unwrap_or_default()
}

/// The types of `kind` in the document, by name (case folded) and then identifier: the order of
/// every list of types.
#[must_use]
pub fn of_kind(snapshot: &Snapshot, kind: TypeKind) -> Vec<Arc<Object>> {
    let mut found = crate::spatial::objects(snapshot, kind.key());
    found.sort_by(|a, b| {
        crate::names::fold(name(a))
            .cmp(&crate::names::fold(name(b)))
            .then_with(|| a.id.cmp(&b.id))
    });
    found
}

/// The type an instance's values refer to.
#[must_use]
pub fn type_id(kind: TypeKind, values: &[Option<Value>]) -> Option<ObjectId> {
    match values.get(kind.field())? {
        Some(Value::Reference(id)) => Some(*id),
        _ => None,
    }
}

/// The type record of an instance, when it has one in the document.
#[must_use]
pub fn type_of(
    snapshot: &Snapshot,
    kind: TypeKind,
    values: &[Option<Value>],
) -> Option<Arc<Object>> {
    type_id(kind, values)
        .and_then(|id| crate::spatial::get(snapshot, id))
        .filter(|t| crate::spatial::type_of(snapshot, t.id).as_deref() == Some(kind.key()))
}

/// The kind and type record of an instance of any kind, when it has a type.
#[must_use]
pub fn type_of_object(snapshot: &Snapshot, object: &Object) -> Option<(TypeKind, Arc<Object>)> {
    let key = crate::spatial::type_of(snapshot, object.id)?;
    let kind = TypeKind::of_instance_key(&key)?;
    Some((kind, type_of(snapshot, kind, &object.values)?))
}

/// The instances that refer to the type `id` of `kind`.
#[must_use]
pub fn instances(snapshot: &Snapshot, kind: TypeKind, id: ObjectId) -> Vec<Arc<Object>> {
    crate::spatial::objects(snapshot, kind.instance_key())
        .into_iter()
        .filter(|o| type_id(kind, &o.values) == Some(id))
        .collect()
}

/// The values a type of `kind` (its record values `values`) decides on its instances: the field
/// position, the field name and the value. Empty when the record cannot be read.
#[must_use]
pub fn copies(kind: TypeKind, values: &[Option<Value>]) -> Vec<(usize, &'static str, Value)> {
    match kind {
        TypeKind::Wall => Layers::from_values(values)
            .map(|l| {
                let [exterior, interior] = l.core_offsets();
                vec![
                    (
                        types::wall::THICKNESS,
                        "thickness",
                        types::length(l.total()),
                    ),
                    (
                        types::wall::CORE_EXTERIOR,
                        "core_exterior",
                        types::length(exterior),
                    ),
                    (
                        types::wall::CORE_INTERIOR,
                        "core_interior",
                        types::length(interior),
                    ),
                ]
            })
            .unwrap_or_default(),
        TypeKind::Floor => Layers::from_values(values)
            .map(|l| {
                vec![(
                    types::floor::THICKNESS,
                    "thickness",
                    types::length(l.total()),
                )]
            })
            .unwrap_or_default(),
        TypeKind::Roof => Layers::from_values(values)
            .map(|l| {
                vec![(
                    types::roof::THICKNESS,
                    "thickness",
                    types::length(l.total()),
                )]
            })
            .unwrap_or_default(),
        TypeKind::Ceiling => Layers::from_values(values)
            .map(|l| {
                vec![(
                    types::ceiling::THICKNESS,
                    "thickness",
                    types::length(l.total()),
                )]
            })
            .unwrap_or_default(),
        TypeKind::Door | TypeKind::Window => InsertType::from_values(values)
            .map(|t| {
                use types::insert as i;
                vec![
                    (i::WIDTH, "width", types::length(t.width)),
                    (i::HEIGHT, "height", types::length(t.height)),
                    (i::FRAME_WIDTH, "frame_width", types::length(t.frame_width)),
                    (i::FRAME_DEPTH, "frame_depth", types::length(t.frame_depth)),
                    (i::OPERATION, "operation", Value::Enum(t.operation.code())),
                ]
            })
            .unwrap_or_default(),
        // A family instance reads its type at evaluation; nothing is copied (ADR 0009).
        TypeKind::Family => Vec::new(),
    }
}

/// Whether a field of an instance of `kind` is decided by its type.
#[must_use]
pub fn is_copied(kind: TypeKind, field: &str) -> bool {
    match kind {
        TypeKind::Wall => matches!(field, "thickness" | "core_exterior" | "core_interior"),
        TypeKind::Floor | TypeKind::Roof | TypeKind::Ceiling => field == "thickness",
        TypeKind::Door | TypeKind::Window => matches!(
            field,
            "width" | "height" | "frame_width" | "frame_depth" | "operation"
        ),
        TypeKind::Family => false,
    }
}

/// The first field of an instance (values `instance`) that differs from what its type (values
/// `type_values`) decides.
#[must_use]
pub fn mismatch(
    kind: TypeKind,
    type_values: &[Option<Value>],
    instance: &[Option<Value>],
) -> Option<&'static str> {
    copies(kind, type_values)
        .into_iter()
        .find(|(at, _, want)| {
            let have = instance.get(*at).and_then(Option::as_ref);
            match (have, want) {
                (Some(Value::Quantity(a)), Value::Quantity(b)) => {
                    (a.value() - b.value()).abs() > SAME
                }
                (Some(a), b) => a != b,
                (None, _) => true,
            }
        })
        .map(|(_, name, _)| name)
}

fn findings(errors: Vec<Error>) -> Vec<Finding> {
    errors
        .into_iter()
        .map(|error| Finding {
            severity: Severity::Error,
            error,
        })
        .collect()
}

/// The type validator of wall and floor types: the rules of [`Layers::problems`].
#[must_use]
pub fn layer_findings(values: &[Option<Value>]) -> Vec<Finding> {
    match Layers::from_values(values) {
        Some(layers) => findings(layers.problems()),
        None => findings(vec![Error::new(codes::TYPE_CORE)]),
    }
}

/// The type validator of door and window types: the frame leaves an opening inside.
#[must_use]
pub fn size_findings(values: &[Option<Value>]) -> Vec<Finding> {
    let Some(t) = InsertType::from_values(values) else {
        return Vec::new();
    };
    let bar = 2.0 * t.frame_width + crate::insert::MIN_CLEAR;
    if t.width < bar || t.height < bar {
        let name = types::text(values, 0).unwrap_or_default().to_owned();
        return findings(vec![Error::new(codes::INSERT_FRAME).with_arg("name", name)]);
    }
    Vec::new()
}

fn variants(items: impl Iterator<Item = (&'static str, u32)>) -> ValueType {
    ValueType::Enum(items.map(|(n, c)| EnumVariant::new(n, c)).collect())
}

fn layer_fields() -> Vec<FieldDecl> {
    let mut decl = vec![
        FieldDecl::new(
            "function",
            variants(Function::ALL.iter().map(|f| (f.name(), f.code()))),
        )
        .with_default(Value::Enum(Function::Structure.code())),
        FieldDecl::new("material", ValueType::String)
            .with_default(Value::String(String::new()))
            .with_bound(Bound::Length { max: NAME_BYTES }),
    ];
    decl.push(types::length_field(
        "thickness",
        Some(types::real((0.0, MAX_LAYER_THICKNESS))),
    ));
    decl
}

fn layered_fields() -> Vec<FieldDecl> {
    let max = i64::try_from(MAX_LAYERS).unwrap_or(i64::MAX);
    vec![
        types::name_field("identity"),
        FieldDecl::new(
            "layers",
            ValueType::List(Box::new(ValueType::Record(layer_fields()))),
        )
        .with_bound(Bound::Length {
            max: MAX_LAYERS as u64,
        })
        .in_aspect("geometry"),
        FieldDecl::new("core_start", ValueType::I64)
            .with_default(Value::I64(0))
            .with_bound(Bound::Int { min: 0, max })
            .in_aspect("geometry"),
        FieldDecl::new("core_end", ValueType::I64)
            .with_default(Value::I64(1))
            .with_bound(Bound::Int { min: 0, max })
            .in_aspect("geometry"),
    ]
}

/// The fields of `archbuilder.wall_type`.
#[must_use]
pub fn wall_type_fields() -> Vec<FieldDecl> {
    let mut fields = layered_fields();
    fields.extend([
        FieldDecl::new(
            "exposure",
            variants(Exposure::ALL.iter().map(|f| (f.name(), f.code()))),
        )
        .with_default(Value::Enum(Exposure::Exterior.code()))
        .in_aspect("data"),
        FieldDecl::new("load_bearing", ValueType::Bool)
            .with_default(Value::Bool(false))
            .in_aspect("data"),
        FieldDecl::new("room_bounding", ValueType::Bool)
            .with_default(Value::Bool(true))
            .in_aspect("data"),
        wrap_field("wrap_ends"),
        wrap_field("wrap_inserts"),
    ]);
    fields.extend(crate::curtain::type_fields());
    fields.push(stack_field());
    fields
}

/// The position of the stack of a stacked wall type in the type record (appended in schema 1.6).
pub const STACK: usize = 33;
/// The most segments of a stacked wall type.
pub const MAX_STACK: usize = 16;

/// One segment of a stacked wall type: the wall type that fills it and its height, which is 0
/// for the one variable segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StackEntry {
    /// The type of the segment.
    pub type_id: ObjectId,
    /// The fixed height in meters; 0 for the variable segment.
    pub height: f64,
}

/// The declaration of the stack of a wall type.
#[must_use]
pub fn stack_field() -> FieldDecl {
    FieldDecl::new(
        "stack",
        ValueType::List(Box::new(ValueType::Record(vec![
            FieldDecl::new("type", ValueType::Reference(types::reference_to(WALL_TYPE))),
            types::length_field("height", Some(types::real((0.0, types::HEIGHT_RANGE.1)))),
        ]))),
    )
    .with_bound(Bound::Length {
        max: MAX_STACK as u64,
    })
    .optional()
    .in_aspect("data")
    .since_minor(6)
}

impl StackEntry {
    /// The stack of a wall type record, from the base up; empty for a type that is not stacked.
    #[must_use]
    pub fn of(values: &[Option<Value>]) -> Vec<Self> {
        let Some(Some(Value::List(items))) = values.get(STACK) else {
            return Vec::new();
        };
        items
            .iter()
            .take(MAX_STACK)
            .filter_map(|item| {
                let Value::Record(f) = item else { return None };
                let Some(Some(Value::Reference(id))) = f.first() else {
                    return None;
                };
                Some(Self {
                    type_id: *id,
                    height: types::meters(f, 1)?,
                })
            })
            .collect()
    }

    /// The records of a stack, for the type's field.
    #[must_use]
    pub fn value(stack: &[Self]) -> Value {
        Value::List(
            stack
                .iter()
                .map(|e| {
                    Value::Record(vec![
                        Some(Value::Reference(e.type_id)),
                        Some(types::length(e.height)),
                    ])
                })
                .collect(),
        )
    }
}

/// The height ranges of the segments of a stack in a wall `height` high, from the base: each
/// fixed segment its height, the variable one the rest. `None` when the stack has no single
/// variable segment, fewer than two segments, or the rest is under 1 mm.
#[must_use]
pub fn stack_ranges(stack: &[StackEntry], height: f64) -> Option<Vec<(f64, f64)>> {
    let variable = stack.iter().filter(|e| e.height <= 0.0).count();
    if stack.len() < 2 || variable != 1 {
        return None;
    }
    let fixed: f64 = stack.iter().map(|e| e.height).sum();
    let rest = height - fixed;
    if rest < 1.0e-3 {
        return None;
    }
    let mut at = 0.0;
    Some(
        stack
            .iter()
            .map(|e| {
                let h = if e.height <= 0.0 { rest } else { e.height };
                at += h;
                (at - h, at)
            })
            .collect(),
    )
}

/// A wrapping setting of a wall type, appended in schema 1.1.
fn wrap_field(name: &str) -> FieldDecl {
    FieldDecl::new(
        name,
        variants(Wrap::ALL.iter().map(|w| (w.name(), w.code()))),
    )
    .with_default(Value::Enum(Wrap::None.code()))
    .in_aspect("data")
    .since_minor(1)
}

/// The fields of `archbuilder.floor_type`.
#[must_use]
pub fn floor_type_fields() -> Vec<FieldDecl> {
    layered_fields()
}

/// The fields of `archbuilder.roof_type`: the layered fields, then the optional `rafter_cut` and
/// `fascia_depth` (schema 1.1) that shape the eave end of every roof of the type.
#[must_use]
pub fn roof_type_fields() -> Vec<FieldDecl> {
    let mut fields = layered_fields();
    fields.extend([
        FieldDecl::new(
            "rafter_cut",
            variants(RafterCut::ALL.iter().map(|c| (c.name(), c.code()))),
        )
        .with_default(Value::Enum(RafterCut::Plumb.code()))
        .optional()
        .in_aspect("geometry")
        .since_minor(1),
        types::length_field("fascia_depth", Some(types::real(types::FASCIA_DEPTH_RANGE)))
            .with_default(types::length(crate::roof::DEFAULT_FASCIA_DEPTH))
            .optional()
            .since_minor(1),
    ]);
    fields
}

/// The fields of `archbuilder.ceiling_type`: the layered fields, the height of a new ceiling and
/// the grid pattern.
#[must_use]
pub fn ceiling_type_fields() -> Vec<FieldDecl> {
    let mut fields = layered_fields();
    fields.extend([
        types::length_field("height_offset", Some(types::real(types::BASE_OFFSET_RANGE)))
            .with_default(types::length(types::DEFAULT_CEILING_HEIGHT)),
        types::length_field("grid_x", Some(types::real(types::GRID_RANGE))),
        types::length_field("grid_y", Some(types::real(types::GRID_RANGE))),
        FieldDecl::new(
            "grid_angle",
            ValueType::Quantity(arc_base::Dimension::ANGLE),
        )
        .with_default(types::angle(0.0))
        .in_aspect("geometry"),
    ]);
    fields
}

/// The fields of `archbuilder.door_type` and `archbuilder.window_type`.
#[must_use]
pub fn insert_type_fields() -> Vec<FieldDecl> {
    let length = |name, range, default| {
        types::length_field(name, Some(types::real(range))).with_default(types::length(default))
    };
    vec![
        types::name_field("identity"),
        length("width", INSERT_WIDTH_RANGE, 0.9),
        length("height", INSERT_HEIGHT_RANGE, 2.1),
        length("frame_width", FRAME_WIDTH_RANGE, 0.05),
        length("frame_depth", FRAME_DEPTH_RANGE, 0.1),
        FieldDecl::new(
            "operation",
            variants(Operation::ALL.iter().map(|o| (o.name(), o.code()))),
        )
        .with_default(Value::Enum(Operation::SingleSwing.code()))
        .in_aspect("geometry"),
        length("sill", SILL_RANGE, 0.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cavity() -> Layers {
        Layers {
            layers: vec![
                Layer::new(Function::Finish, "Brick", 0.1),
                Layer::new(Function::Thermal, "Air", 0.05),
                Layer::new(Function::Membrane, "Barrier", 0.0),
                Layer::new(Function::Structure, "Block", 0.15),
                Layer::new(Function::Finish, "Plaster", 0.015),
            ],
            core: (3, 4),
        }
    }

    #[test]
    fn totals_core_offsets_and_boundaries() {
        let layers = cavity();
        assert!((layers.total() - 0.315).abs() < 1e-12);
        let [exterior, interior] = layers.core_offsets();
        assert!((exterior - 0.15).abs() < 1e-12 && (interior - 0.015).abs() < 1e-12);
        assert_eq!(layers.boundaries().len(), 4);
        assert!(layers.problems().is_empty());
        let back = Layers::from_values(&floor_type_values("x", &layers)).unwrap();
        assert_eq!(back, layers);
    }

    #[test]
    fn layer_rules_name_what_to_change() {
        let code = |l: &Layers| l.problems().first().map(|e| e.code().to_owned());
        let mut layers = cavity();
        layers.layers[1].thickness = 0.0;
        assert_eq!(
            code(&layers).as_deref(),
            Some("ab.model.type_layer_thickness")
        );
        let mut layers = cavity();
        layers.layers[2].thickness = 0.01;
        assert_eq!(
            code(&layers).as_deref(),
            Some("ab.model.type_membrane_thickness")
        );
        let mut layers = cavity();
        layers.core = (2, 2);
        assert_eq!(code(&layers).as_deref(), Some("ab.model.type_core"));
        let layers = Layers {
            layers: Vec::new(),
            core: (0, 0),
        };
        assert_eq!(code(&layers).as_deref(), Some("ab.model.type_layer_count"));
        let mut layers = Layers::single("Concrete", 4.0);
        layers
            .layers
            .push(Layer::new(Function::Finish, "Paint", 2.0));
        assert_eq!(code(&layers).as_deref(), Some("ab.model.type_total"));
    }
}
