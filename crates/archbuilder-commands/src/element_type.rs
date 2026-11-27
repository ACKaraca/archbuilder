//! The commands of element types: `archbuilder.type.set` (edit a type and every instance of it in
//! one transaction), `archbuilder.type.duplicate`, `archbuilder.type.delete` (a type in use needs
//! a replacement), `archbuilder.type.assign` (retype objects), `archbuilder.type.match` (give
//! objects the type of a source object) and `archbuilder.type.upgrade` (types for a document
//! from before types: one type per distinct thickness or size).
//!
//! Also the helpers the other commands share: the default type set a new project starts with
//! ([`create_defaults`]), the names of types in the interface language (formatted from this
//! crate's message catalogs), and [`assign`], [`ensure`] and [`rebind`], which give an instance a
//! type and the values it takes from it.

use std::sync::Arc;

use arc_base::{Dimension, Error, ObjectId, Result};
use arc_command::{CommandSpec, Context, IdParam, QuantityParam, Registry};
use arc_doc::{FieldPath, Object, Snapshot, Value};
use archbuilder_model::element_type::{
    self as et, Exposure, Function, InsertType, Layer, Layers, TypeKind, WallType, Wrap,
};
use archbuilder_model::insert::Operation;
use archbuilder_model::{names, spatial, types};
use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::codes;
use crate::structure::{create, ensure_valid, language};

/// The identifier of the command that edits a type.
pub const TYPE_SET: &str = "archbuilder.type.set";
/// The identifier of the command that duplicates a type.
pub const TYPE_DUPLICATE: &str = "archbuilder.type.duplicate";
/// The identifier of the command that deletes a type.
pub const TYPE_DELETE: &str = "archbuilder.type.delete";
/// The identifier of the command that gives objects a type.
pub const TYPE_ASSIGN: &str = "archbuilder.type.assign";
/// The identifier of the command that gives objects the type of a source object.
pub const TYPE_MATCH: &str = "archbuilder.type.match";
/// The identifier of the command that gives the instances of an older document types.
pub const TYPE_UPGRADE: &str = "archbuilder.type.upgrade";

/// The most objects one command retypes.
const MAX_OBJECTS: usize = 100_000;
/// How close two lengths must be to count as the same, in meters.
const SAME: f64 = 1e-9;

/// A message of the catalog, named by identifier.
struct Msg {
    message_id: &'static str,
}

const GENERIC_WALL: Msg = Msg {
    message_id: "ab-type-wall-generic",
};
const CAVITY_WALL: Msg = Msg {
    message_id: "ab-type-wall-cavity",
};
const PARTITION_WALL: Msg = Msg {
    message_id: "ab-type-wall-partition",
};
const GENERIC_FLOOR: Msg = Msg {
    message_id: "ab-type-floor-generic",
};
const SLAB_FLOOR: Msg = Msg {
    message_id: "ab-type-floor-slab",
};
const GENERIC_CEILING: Msg = Msg {
    message_id: "ab-type-ceiling-generic",
};
const GRID_CEILING: Msg = Msg {
    message_id: "ab-type-ceiling-grid",
};
const GENERATED_CEILING: Msg = Msg {
    message_id: "ab-type-ceiling-generated",
};
const M_BOARD: Msg = Msg {
    message_id: "ab-material-ceiling-board",
};
const M_TILE: Msg = Msg {
    message_id: "ab-material-ceiling-tile",
};
const GENERIC_ROOF: Msg = Msg {
    message_id: "ab-type-roof-generic",
};
const RAFTER_ROOF: Msg = Msg {
    message_id: "ab-type-roof-rafters",
};
const GENERATED_ROOF: Msg = Msg {
    message_id: "ab-type-roof-generated",
};
const M_TILES: Msg = Msg {
    message_id: "ab-material-roof-tiles",
};
const M_BATTENS: Msg = Msg {
    message_id: "ab-material-battens",
};
const M_UNDERLAY: Msg = Msg {
    message_id: "ab-material-roof-underlay",
};
const M_RAFTERS: Msg = Msg {
    message_id: "ab-material-rafters-insulation",
};
const SINGLE_DOOR: Msg = Msg {
    message_id: "ab-type-door-single",
};
const DOUBLE_DOOR: Msg = Msg {
    message_id: "ab-type-door-double",
};
const LARGE_WINDOW: Msg = Msg {
    message_id: "ab-type-window-large",
};
const SLIDING_DOOR: Msg = Msg {
    message_id: "ab-type-door-sliding",
};
const SLIDING_WINDOW: Msg = Msg {
    message_id: "ab-type-window-sliding",
};
const SMALL_WINDOW: Msg = Msg {
    message_id: "ab-type-window-small",
};
const GENERATED_WALL: Msg = Msg {
    message_id: "ab-type-wall-generated",
};
const GENERATED_FLOOR: Msg = Msg {
    message_id: "ab-type-floor-generated",
};
const GENERATED_DOOR: Msg = Msg {
    message_id: "ab-type-door-generated",
};
const GENERATED_WINDOW: Msg = Msg {
    message_id: "ab-type-window-generated",
};
const COPY_NAME: Msg = Msg {
    message_id: "ab-type-copy-name",
};
const M_GENERIC: Msg = Msg {
    message_id: "ab-material-generic",
};
const M_BRICK: Msg = Msg {
    message_id: "ab-material-brick",
};
const M_AIR: Msg = Msg {
    message_id: "ab-material-air",
};
const M_INSULATION: Msg = Msg {
    message_id: "ab-material-insulation",
};
const M_BARRIER: Msg = Msg {
    message_id: "ab-material-vapor-barrier",
};
const M_BLOCK: Msg = Msg {
    message_id: "ab-material-concrete-block",
};
const M_PLASTER: Msg = Msg {
    message_id: "ab-material-plaster",
};
const M_GYPSUM: Msg = Msg {
    message_id: "ab-material-gypsum-board",
};
const M_STUD: Msg = Msg {
    message_id: "ab-material-steel-stud",
};
const M_FINISH: Msg = Msg {
    message_id: "ab-material-floor-finish",
};
const M_SCREED: Msg = Msg {
    message_id: "ab-material-screed",
};
const M_SEPARATION: Msg = Msg {
    message_id: "ab-material-separation-layer",
};
const M_CONCRETE: Msg = Msg {
    message_id: "ab-material-concrete",
};

/// The names of types and materials in one language, formatted from the message catalog of
/// this crate. They are document content: written once when a type is made, user data after.
struct Names {
    bundle: FluentBundle<FluentResource>,
}

impl Names {
    fn new(language: &str) -> Self {
        let turkish = language
            .get(..2)
            .is_some_and(|l| l.eq_ignore_ascii_case("tr"));
        let (tag, source) = if turkish {
            (
                "tr-TR",
                include_str!("../../../locales/tr-TR/archbuilder-commands.ftl"),
            )
        } else {
            (
                "en-US",
                include_str!("../../../locales/en-US/archbuilder-commands.ftl"),
            )
        };
        let locale: unic_langid::LanguageIdentifier = tag.parse().unwrap_or_default();
        let mut bundle = FluentBundle::new(vec![locale]);
        bundle.set_use_isolating(false);
        // A catalog that does not parse leaves the messages it could read; the gate checks both.
        let resource = FluentResource::try_new(source.to_owned()).unwrap_or_else(|(r, _)| r);
        // Identifiers are unique in a catalog (the catalog gate checks it), so nothing overrides.
        let _ = bundle.add_resource(resource);
        Self { bundle }
    }

    fn text(&self, message: &Msg, args: &[(&str, FluentValue<'_>)]) -> String {
        let Some(pattern) = self
            .bundle
            .get_message(message.message_id)
            .and_then(|m| m.value())
        else {
            return message.message_id.to_owned();
        };
        let mut fluent = FluentArgs::new();
        for (name, value) in args {
            fluent.set(*name, value.clone());
        }
        let mut errors = Vec::new();
        self.bundle
            .format_pattern(pattern, Some(&fluent), &mut errors)
            .into_owned()
    }
}

fn mm(meters: f64) -> FluentValue<'static> {
    FluentValue::from(format!("{:.0}", meters * 1000.0))
}

/// The project a document's types belong to.
pub(crate) fn project(snapshot: &Snapshot) -> Result<ObjectId> {
    spatial::objects(snapshot, types::PROJECT)
        .first()
        .map(|p| p.id)
        .ok_or_else(|| Error::new(codes::NO_BUILDING))
}

/// `name`, or the first `name (n)` no type of `kind` has.
fn free_name(snapshot: &Snapshot, kind: TypeKind, name: &str, names: &Names) -> String {
    let taken = |candidate: &str| {
        spatial::objects(snapshot, kind.key())
            .iter()
            .any(|t| names::same(et::name(t), candidate))
    };
    if !taken(name) {
        return name.to_owned();
    }
    (2..10_000)
        .map(|n| {
            names.text(
                &COPY_NAME,
                &[
                    ("name", FluentValue::from(name.to_owned())),
                    ("n", FluentValue::from(n)),
                ],
            )
        })
        .find(|candidate| !taken(candidate))
        .unwrap_or_else(|| name.to_owned())
}

/// Creates a type of `kind` owned by the project.
fn create_type(
    ctx: &mut Context<'_>,
    kind: TypeKind,
    values: Vec<Option<Value>>,
) -> Result<ObjectId> {
    let owner = project(ctx.txn()?.snapshot())?;
    create(ctx, kind.key(), Some(owner), values)
}

/// Creates the default types of a new project, named in the interface language: three wall
/// types, two floor types, two roof types, three door types and three window types.
///
/// # Errors
/// The errors of the transaction.
#[allow(
    clippy::too_many_lines,
    reason = "the default set, one type after another"
)]
pub(crate) fn create_defaults(ctx: &mut Context<'_>) -> Result<()> {
    let n = Names::new(&language(ctx));
    let layer = |f: Function, m: &Msg, t: f64| Layer::new(f, &n.text(m, &[]), t);
    let walls = [
        (
            &GENERIC_WALL,
            WallType {
                layers: Layers::single(&n.text(&M_GENERIC, &[]), 0.2),
                exposure: Exposure::Exterior,
                load_bearing: false,
                room_bounding: true,
                wrap_ends: Wrap::None,
                wrap_inserts: Wrap::None,
            },
        ),
        (
            &CAVITY_WALL,
            WallType {
                layers: Layers {
                    layers: vec![
                        layer(Function::Finish, &M_BRICK, 0.1),
                        layer(Function::Thermal, &M_AIR, 0.05),
                        layer(Function::Thermal, &M_INSULATION, 0.05),
                        layer(Function::Membrane, &M_BARRIER, 0.0),
                        layer(Function::Structure, &M_BLOCK, 0.15),
                        layer(Function::Finish, &M_PLASTER, 0.015),
                    ],
                    core: (4, 5),
                },
                exposure: Exposure::Exterior,
                load_bearing: true,
                room_bounding: true,
                wrap_ends: Wrap::None,
                wrap_inserts: Wrap::None,
            },
        ),
        (
            &PARTITION_WALL,
            WallType {
                layers: Layers {
                    layers: vec![
                        layer(Function::Finish, &M_GYPSUM, 0.0125),
                        layer(Function::Structure, &M_STUD, 0.075),
                        layer(Function::Finish, &M_GYPSUM, 0.0125),
                    ],
                    core: (1, 2),
                },
                exposure: Exposure::Interior,
                load_bearing: false,
                room_bounding: true,
                wrap_ends: Wrap::None,
                wrap_inserts: Wrap::None,
            },
        ),
    ];
    for (name, wall) in &walls {
        let values = et::wall_type_values(&n.text(name, &[]), wall);
        create_type(ctx, TypeKind::Wall, values)?;
    }
    let floors = [
        (
            &GENERIC_FLOOR,
            Layers::single(&n.text(&M_GENERIC, &[]), 0.2),
        ),
        (
            &SLAB_FLOOR,
            Layers {
                layers: vec![
                    layer(Function::Finish, &M_FINISH, 0.01),
                    layer(Function::Substrate, &M_SCREED, 0.05),
                    layer(Function::Membrane, &M_SEPARATION, 0.0),
                    layer(Function::Structure, &M_CONCRETE, 0.2),
                ],
                core: (3, 4),
            },
        ),
    ];
    for (name, layers) in &floors {
        let values = et::floor_type_values(&n.text(name, &[]), layers);
        create_type(ctx, TypeKind::Floor, values)?;
    }
    let insert = |width, height, operation, sill| InsertType {
        width,
        height,
        frame_width: 0.05,
        frame_depth: 0.1,
        operation,
        sill,
    };
    for (kind, name, t) in [
        (
            TypeKind::Door,
            &SINGLE_DOOR,
            insert(0.9, 2.1, Operation::SingleSwing, 0.0),
        ),
        (
            TypeKind::Door,
            &DOUBLE_DOOR,
            insert(1.5, 2.1, Operation::DoubleSwing, 0.0),
        ),
        (
            TypeKind::Window,
            &LARGE_WINDOW,
            insert(1.2, 1.5, Operation::Fixed, 0.9),
        ),
        (
            TypeKind::Door,
            &SLIDING_DOOR,
            insert(1.8, 2.1, Operation::Sliding, 0.0),
        ),
        (
            TypeKind::Window,
            &SMALL_WINDOW,
            insert(0.6, 0.6, Operation::Fixed, 1.5),
        ),
        (
            TypeKind::Window,
            &SLIDING_WINDOW,
            insert(1.8, 1.2, Operation::Sliding, 0.9),
        ),
    ] {
        create_type(ctx, kind, t.values(&n.text(name, &[])))?;
    }
    create_roof_defaults(ctx, &n)?;
    create_ceiling_defaults(ctx, &n)
}

/// The two ceiling types of a new project: a generic board of 20 mm, and a suspended grid of
/// 600 by 600 mm tiles. Also made for a document from before ceiling types the first time it needs
/// one ([`ensure_ceiling_types`]).
fn create_ceiling_defaults(ctx: &mut Context<'_>, n: &Names) -> Result<()> {
    use archbuilder_model::element_type::{CeilingType, Grid};
    let height = types::DEFAULT_CEILING_HEIGHT;
    let types = [
        (
            &GENERIC_CEILING,
            CeilingType {
                layers: Layers::single(&n.text(&M_BOARD, &[]), 0.02),
                height_offset: height,
                grid: None,
            },
        ),
        (
            &GRID_CEILING,
            CeilingType {
                layers: Layers::single(&n.text(&M_TILE, &[]), 0.015),
                height_offset: height,
                grid: Some(Grid {
                    spacing: [0.6, 0.6],
                    angle: 0.0,
                }),
            },
        ),
    ];
    for (name, t) in &types {
        create_type(
            ctx,
            TypeKind::Ceiling,
            et::ceiling_type_values(&n.text(name, &[]), t),
        )?;
    }
    Ok(())
}

/// Makes the default ceiling types in a document that has none yet.
///
/// # Errors
/// The errors of the transaction.
pub(crate) fn ensure_ceiling_types(ctx: &mut Context<'_>) -> Result<()> {
    if et::of_kind(ctx.txn()?.snapshot(), TypeKind::Ceiling).is_empty() {
        let n = Names::new(&language(ctx));
        create_ceiling_defaults(ctx, &n)?;
    }
    Ok(())
}

/// The two roof types of a new project: one generic layer of 300 mm, and timber rafters with
/// insulation under tiles. Also made for a document from before roof types the first time it
/// needs a roof type ([`ensure_roof_types`]).
fn create_roof_defaults(ctx: &mut Context<'_>, n: &Names) -> Result<()> {
    let layer = |f: Function, m: &Msg, t: f64| Layer::new(f, &n.text(m, &[]), t);
    let roofs = [
        (&GENERIC_ROOF, Layers::single(&n.text(&M_GENERIC, &[]), 0.3)),
        (
            &RAFTER_ROOF,
            Layers {
                layers: vec![
                    layer(Function::Finish, &M_TILES, 0.03),
                    layer(Function::Substrate, &M_BATTENS, 0.04),
                    layer(Function::Membrane, &M_UNDERLAY, 0.0),
                    layer(Function::Structure, &M_RAFTERS, 0.2),
                    layer(Function::Finish, &M_GYPSUM, 0.0125),
                ],
                core: (3, 4),
            },
        ),
    ];
    for (name, layers) in &roofs {
        let values = et::floor_type_values(&n.text(name, &[]), layers);
        create_type(ctx, TypeKind::Roof, values)?;
    }
    Ok(())
}

/// Makes the default roof types in a document that has none yet (a document from before roof
/// types), so a new roof gets the same choice a new project offers.
///
/// # Errors
/// The errors of the transaction.
pub(crate) fn ensure_roof_types(ctx: &mut Context<'_>) -> Result<()> {
    if et::of_kind(ctx.txn()?.snapshot(), TypeKind::Roof).is_empty() {
        let n = Names::new(&language(ctx));
        create_roof_defaults(ctx, &n)?;
    }
    Ok(())
}

/// The values of the type a new instance of `kind` gets when the document has no type that
/// fits better: one generic layer of 0.2 m, a single door of 0.9 by 2.1 m, a fixed window of
/// 1.2 by 1.5 m.
#[must_use]
pub fn generic(kind: TypeKind) -> Vec<(usize, Value)> {
    match kind {
        TypeKind::Wall => vec![
            (
                types::wall::THICKNESS,
                types::length(types::DEFAULT_THICKNESS),
            ),
            (types::wall::CORE_EXTERIOR, types::length(0.0)),
            (types::wall::CORE_INTERIOR, types::length(0.0)),
        ],
        TypeKind::Floor => vec![(
            types::floor::THICKNESS,
            types::length(types::DEFAULT_FLOOR_THICKNESS),
        )],
        TypeKind::Roof => vec![(
            types::roof::THICKNESS,
            types::length(archbuilder_model::roof::DEFAULT_THICKNESS),
        )],
        TypeKind::Ceiling => vec![(
            types::ceiling::THICKNESS,
            types::length(archbuilder_model::ceiling::DEFAULT_THICKNESS),
        )],
        TypeKind::Door | TypeKind::Window => {
            let window = kind == TypeKind::Window;
            let (width, height, operation) = if window {
                (1.2, 1.5, Operation::Fixed)
            } else {
                (0.9, 2.1, Operation::SingleSwing)
            };
            vec![
                (types::insert::WIDTH, types::length(width)),
                (types::insert::HEIGHT, types::length(height)),
                (types::insert::FRAME_WIDTH, types::length(0.05)),
                (types::insert::FRAME_DEPTH, types::length(0.1)),
                (types::insert::OPERATION, Value::Enum(operation.code())),
            ]
        }
    }
}

/// The first type of `kind` (in the order of [`et::of_kind`]) whose values match `wanted`.
fn find(snapshot: &Snapshot, kind: TypeKind, wanted: &[(usize, Value)]) -> Option<ObjectId> {
    et::of_kind(snapshot, kind)
        .into_iter()
        .find(|t| {
            let copies = et::copies(kind, &t.values);
            wanted.iter().all(|(at, value)| {
                copies
                    .iter()
                    .find(|(field, _, _)| field == at)
                    .is_some_and(|(_, _, v)| same(v, value))
            })
        })
        .map(|t| t.id)
}

/// The type of `kind` that a new instance gets without a choice: the one with the
/// [`generic`] values, else the first in name order.
#[must_use]
pub fn default_of(snapshot: &Snapshot, kind: TypeKind) -> Option<ObjectId> {
    find(snapshot, kind, &generic(kind))
        .or_else(|| et::of_kind(snapshot, kind).first().map(|t| t.id))
}

/// The wall or floor type a new instance gets: the one `given` names (else the default), unless
/// `thickness` is given and differs from its total, which gives a one-layer type of that
/// thickness ([`ensure`]).
///
/// # Errors
/// `ab.cmd.type_unknown` for a name no type has; the errors of [`ensure`].
pub(crate) fn layered_for(
    ctx: &mut Context<'_>,
    kind: TypeKind,
    given: Option<&str>,
    thickness: Option<&QuantityParam>,
) -> Result<ObjectId> {
    let chosen = crate::wall::choose_type(ctx, kind, given)?;
    let Some(param) = thickness else {
        return Ok(chosen);
    };
    let thickness = ctx.quantity(param, Dimension::LENGTH)?.value();
    let total = spatial::get(ctx.txn()?.snapshot(), chosen)
        .and_then(|t| Layers::from_values(&t.values))
        .map(|l| l.total());
    if total.is_some_and(|t| (t - thickness).abs() <= SAME) {
        return Ok(chosen);
    }
    let mut wanted = vec![(
        match kind {
            TypeKind::Wall => types::wall::THICKNESS,
            TypeKind::Roof => types::roof::THICKNESS,
            TypeKind::Ceiling => types::ceiling::THICKNESS,
            _ => types::floor::THICKNESS,
        },
        types::length(thickness),
    )];
    if kind == TypeKind::Wall {
        wanted.push((types::wall::CORE_EXTERIOR, types::length(0.0)));
        wanted.push((types::wall::CORE_INTERIOR, types::length(0.0)));
    }
    ensure(ctx, kind, &wanted)
}

/// The type of `kind` that `text` names: its identifier or its name.
///
/// # Errors
/// `ab.cmd.type_unknown` when no type of the kind matches.
pub fn resolve(snapshot: &Snapshot, kind: TypeKind, text: &str) -> Result<ObjectId> {
    let text = text.trim();
    let all = spatial::objects(snapshot, kind.key());
    if let Ok(id) = text.parse::<ObjectId>()
        && all.iter().any(|t| t.id == id)
    {
        return Ok(id);
    }
    all.iter()
        .find(|t| names::same(et::name(t), text))
        .map(|t| t.id)
        .ok_or_else(|| Error::new(codes::TYPE_UNKNOWN).with_arg("name", text.to_owned()))
}

/// Gives the instance `id` of `kind` the type `type_id`: the reference and every value the type
/// decides; a door or window also takes the type's sill.
///
/// # Errors
/// The errors of the transaction; `ab.cmd.type_unknown` for a type that is not in the document.
pub fn assign(
    ctx: &mut Context<'_>,
    id: ObjectId,
    kind: TypeKind,
    type_id: ObjectId,
) -> Result<()> {
    let snapshot = ctx.txn()?.snapshot().clone();
    let found = spatial::get(&snapshot, type_id)
        .filter(|_| spatial::type_of(&snapshot, type_id).as_deref() == Some(kind.key()))
        .ok_or_else(|| Error::new(codes::TYPE_UNKNOWN).with_arg("name", type_id.to_string()))?;
    let mut fields: Vec<(&str, Value)> = vec![(kind.field_name(), Value::Reference(type_id))];
    fields.extend(
        et::copies(kind, &found.values)
            .into_iter()
            .map(|(_, name, value)| (name, value)),
    );
    if kind.insert().is_some()
        && let Some(t) = InsertType::from_values(&found.values)
    {
        fields.push(("sill", types::length(t.sill)));
    }
    let txn = ctx.txn()?;
    for (field, value) in fields {
        txn.modify(id, FieldPath::new(field), Some(value))?;
    }
    Ok(())
}

/// The values `kind` decides, read from an instance's values: what [`ensure`] matches.
fn wanted(kind: TypeKind, values: &[Option<Value>]) -> Vec<(usize, Value)> {
    let fields: &[usize] = match kind {
        TypeKind::Wall => &[
            types::wall::THICKNESS,
            types::wall::CORE_EXTERIOR,
            types::wall::CORE_INTERIOR,
        ],
        TypeKind::Floor => &[types::floor::THICKNESS],
        TypeKind::Roof => &[types::roof::THICKNESS],
        TypeKind::Ceiling => &[types::ceiling::THICKNESS],
        TypeKind::Door | TypeKind::Window => &[
            types::insert::WIDTH,
            types::insert::HEIGHT,
            types::insert::FRAME_WIDTH,
            types::insert::FRAME_DEPTH,
            types::insert::OPERATION,
        ],
    };
    fields
        .iter()
        .map(|&at| {
            let value = values.get(at).cloned().flatten().unwrap_or_else(|| {
                // A missing core thickness is a wall that is all core.
                types::length(0.0)
            });
            (at, value)
        })
        .collect()
}

fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Quantity(x), Value::Quantity(y)) => (x.value() - y.value()).abs() <= SAME,
        _ => a == b,
    }
}

/// A type of `kind` whose values match `wanted` (field position and value of the instance);
/// when none does, a new type made to match it, named after its size.
///
/// # Errors
/// The errors of the transaction.
#[allow(clippy::too_many_lines, reason = "one arm for each kind of type")]
pub fn ensure(
    ctx: &mut Context<'_>,
    kind: TypeKind,
    wanted: &[(usize, Value)],
) -> Result<ObjectId> {
    let snapshot = ctx.txn()?.snapshot().clone();
    if let Some(found) = find(&snapshot, kind, wanted) {
        return Ok(found);
    }
    let n = Names::new(&language(ctx));
    let get = |at: usize| {
        wanted
            .iter()
            .find(|(field, _)| *field == at)
            .map(|(_, v)| v.clone())
    };
    let length = |at: usize| match get(at) {
        Some(Value::Quantity(q)) => q.value(),
        _ => 0.0,
    };
    let generic = n.text(&M_GENERIC, &[]);
    let values = match kind {
        TypeKind::Wall => {
            let total = length(types::wall::THICKNESS);
            let (exterior, interior) = (
                length(types::wall::CORE_EXTERIOR),
                length(types::wall::CORE_INTERIOR),
            );
            let mut layers = Vec::new();
            if exterior > 0.0 {
                layers.push(Layer::new(Function::Finish, &generic, exterior));
            }
            let start = layers.len();
            layers.push(Layer::new(
                Function::Structure,
                &generic,
                total - exterior - interior,
            ));
            if interior > 0.0 {
                layers.push(Layer::new(Function::Finish, &generic, interior));
            }
            let name = n.text(&GENERATED_WALL, &[("mm", mm(total))]);
            et::wall_type_values(
                &free_name(&snapshot, kind, &name, &n),
                &WallType {
                    layers: Layers {
                        layers,
                        core: (start, start + 1),
                    },
                    exposure: Exposure::Exterior,
                    load_bearing: false,
                    room_bounding: true,
                    wrap_ends: Wrap::None,
                    wrap_inserts: Wrap::None,
                },
            )
        }
        TypeKind::Floor => {
            let total = length(types::floor::THICKNESS);
            let name = n.text(&GENERATED_FLOOR, &[("mm", mm(total))]);
            et::floor_type_values(
                &free_name(&snapshot, kind, &name, &n),
                &Layers::single(&generic, total),
            )
        }
        TypeKind::Roof => {
            let total = length(types::roof::THICKNESS);
            let name = n.text(&GENERATED_ROOF, &[("mm", mm(total))]);
            et::floor_type_values(
                &free_name(&snapshot, kind, &name, &n),
                &Layers::single(&generic, total),
            )
        }
        TypeKind::Ceiling => {
            let total = length(types::ceiling::THICKNESS);
            let name = n.text(&GENERATED_CEILING, &[("mm", mm(total))]);
            et::ceiling_type_values(
                &free_name(&snapshot, kind, &name, &n),
                &et::CeilingType {
                    layers: Layers::single(&generic, total),
                    height_offset: types::DEFAULT_CEILING_HEIGHT,
                    grid: None,
                },
            )
        }
        TypeKind::Door | TypeKind::Window => {
            let t = InsertType {
                width: length(types::insert::WIDTH),
                height: length(types::insert::HEIGHT),
                frame_width: length(types::insert::FRAME_WIDTH),
                frame_depth: length(types::insert::FRAME_DEPTH),
                operation: match get(types::insert::OPERATION) {
                    Some(Value::Enum(code)) => Operation::from_code(code),
                    _ => Operation::default(),
                },
                sill: if kind == TypeKind::Door { 0.0 } else { 0.9 },
            };
            let message = if kind == TypeKind::Door {
                &GENERATED_DOOR
            } else {
                &GENERATED_WINDOW
            };
            let name = n.text(message, &[("width", mm(t.width)), ("height", mm(t.height))]);
            t.values(&free_name(&snapshot, kind, &name, &n))
        }
    };
    create_type(ctx, kind, values)
}

/// Makes the type reference in `values` (an instance of `kind` about to be created, such as a
/// pasted copy) name a type of this document that matches the values: the type it names when
/// that fits, else a matching or new type ([`ensure`]). The list is padded to the record's length.
///
/// # Errors
/// The errors of [`ensure`].
pub fn rebind(ctx: &mut Context<'_>, key: &str, values: &mut Vec<Option<Value>>) -> Result<()> {
    let Some(kind) = TypeKind::of_instance_key(key) else {
        return Ok(());
    };
    let length = ctx
        .snapshot()
        .registry()
        .get(key)
        .map_or(0, |d| d.schema.fields.len());
    if values.len() < length {
        values.resize(length, None);
    }
    let snapshot = ctx.txn()?.snapshot().clone();
    let keep = et::type_of(&snapshot, kind, values)
        .is_some_and(|t| et::mismatch(kind, &t.values, values).is_none());
    if keep {
        return Ok(());
    }
    let want = wanted(kind, values);
    let id = ensure(ctx, kind, &want)?;
    if let Some(slot) = values.get_mut(kind.field()) {
        *slot = Some(Value::Reference(id));
    }
    for (at, _, value) in spatial::get(ctx.txn()?.snapshot(), id)
        .map(|t| et::copies(kind, &t.values))
        .unwrap_or_default()
    {
        if let Some(slot) = values.get_mut(at) {
            *slot = Some(value);
        }
    }
    Ok(())
}

/// The kind and record of a type.
fn type_record(snapshot: &Snapshot, id: ObjectId) -> Result<(TypeKind, Arc<Object>)> {
    let kind = spatial::type_of(snapshot, id)
        .and_then(|k| TypeKind::of_key(&k))
        .ok_or_else(|| Error::new(codes::TYPE_UNKNOWN).with_arg("name", id.to_string()))?;
    let object = spatial::get(snapshot, id)
        .ok_or_else(|| Error::new(codes::TYPE_UNKNOWN).with_arg("name", id.to_string()))?;
    Ok((kind, object))
}

/// A layer function as a command parameter.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FunctionParam {
    /// Carries loads.
    Structure,
    /// A base for another layer.
    Substrate,
    /// Thermal insulation or an air gap.
    Thermal,
    /// A membrane, without thickness.
    Membrane,
    /// A finish.
    Finish,
}

impl From<FunctionParam> for Function {
    fn from(f: FunctionParam) -> Self {
        match f {
            FunctionParam::Structure => Self::Structure,
            FunctionParam::Substrate => Self::Substrate,
            FunctionParam::Thermal => Self::Thermal,
            FunctionParam::Membrane => Self::Membrane,
            FunctionParam::Finish => Self::Finish,
        }
    }
}

/// One layer as a command parameter.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayerParam {
    /// What the layer does.
    pub function: FunctionParam,
    /// The material's name.
    #[serde(default)]
    #[schemars(length(max = 256))]
    pub material: String,
    /// The thickness, a number in meters or a text with a unit; 0 for a membrane.
    pub thickness: QuantityParam,
}

/// Exterior or interior, as a command parameter.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExposureParam {
    /// An exterior wall.
    Exterior,
    /// An interior wall.
    Interior,
}

/// The wrapping of a wall type as a command parameter.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WrapParam {
    /// No layer wraps.
    None,
    /// The finish layers on the exterior side wrap.
    Exterior,
    /// The finish layers on the interior side wrap.
    Interior,
    /// The finish layers on both sides wrap.
    Both,
}

impl From<WrapParam> for Wrap {
    fn from(w: WrapParam) -> Self {
        match w {
            WrapParam::None => Self::None,
            WrapParam::Exterior => Self::Exterior,
            WrapParam::Interior => Self::Interior,
            WrapParam::Both => Self::Both,
        }
    }
}

/// An operation as a command parameter.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OperationParam {
    /// A door with one hinged leaf.
    SingleSwing,
    /// A door with two hinged leaves.
    DoubleSwing,
    /// A door or window that slides.
    Sliding,
    /// A window that does not open.
    Fixed,
    /// A window with a hinged leaf.
    Casement,
}

impl From<OperationParam> for Operation {
    fn from(o: OperationParam) -> Self {
        match o {
            OperationParam::SingleSwing => Self::SingleSwing,
            OperationParam::DoubleSwing => Self::DoubleSwing,
            OperationParam::Sliding => Self::Sliding,
            OperationParam::Fixed => Self::Fixed,
            OperationParam::Casement => Self::Casement,
        }
    }
}

/// Parameters of `archbuilder.type.set`; every field but the type is optional and only the given
/// ones change.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetType {
    /// The type.
    #[serde(rename = "type")]
    pub element_type: IdParam,
    /// The new name.
    #[serde(default)]
    #[schemars(length(min = 1, max = 256))]
    pub name: Option<String>,
    /// Walls and floors: the layers, from the exterior face or the top.
    #[serde(default)]
    #[schemars(length(max = 32))]
    pub layers: Option<Vec<LayerParam>>,
    /// Walls and floors: the core, the index of the first core layer and the index after the
    /// last. Required with new layers unless the old core still fits.
    #[serde(default)]
    pub core: Option<[usize; 2]>,
    /// Walls: exterior or interior.
    #[serde(default)]
    pub exposure: Option<ExposureParam>,
    /// Walls: whether the wall carries loads.
    #[serde(default)]
    pub load_bearing: Option<bool>,
    /// Walls: whether the wall bounds rooms.
    #[serde(default)]
    pub room_bounding: Option<bool>,
    /// Walls: the sides whose finish layers wrap around free ends.
    #[serde(default)]
    pub wrap_ends: Option<WrapParam>,
    /// Walls: the sides whose finish layers wrap around the openings of doors and windows.
    #[serde(default)]
    pub wrap_inserts: Option<WrapParam>,
    /// Doors and windows: the width.
    #[serde(default)]
    pub width: Option<QuantityParam>,
    /// Doors and windows: the height.
    #[serde(default)]
    pub height: Option<QuantityParam>,
    /// Doors and windows: the width of the frame bars.
    #[serde(default)]
    pub frame_width: Option<QuantityParam>,
    /// Doors and windows: the depth of the frame.
    #[serde(default)]
    pub frame_depth: Option<QuantityParam>,
    /// Doors and windows: the operation.
    #[serde(default)]
    pub operation: Option<OperationParam>,
    /// Doors and windows: the sill height new instances get.
    #[serde(default)]
    pub sill: Option<QuantityParam>,
}

/// What a type command returns.
#[derive(Debug, Default, Serialize, JsonSchema)]
pub struct TypeChanged {
    /// The type made or changed.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub element_type: Option<IdParam>,
    /// The instances that changed with it; for an upgrade, those that got a type.
    pub instances: usize,
    /// For an upgrade: the instances whose copies of their type's values were taken again.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resynced: Option<usize>,
}

fn meters(ctx: &Context<'_>, param: Option<&QuantityParam>) -> Result<Option<f64>> {
    param
        .map(|p| ctx.quantity(p, Dimension::LENGTH).map(|q| q.value()))
        .transpose()
}

/// Edits a type and, in the same transaction, every instance that takes values from it.
fn set_type(ctx: &mut Context<'_>, p: SetType) -> Result<TypeChanged> {
    let IdParam(id) = p.element_type;
    let (kind, object) = type_record(ctx.snapshot(), id)?;
    let mut fields: Vec<(&str, Option<Value>)> = Vec::new();
    if let Some(name) = &p.name {
        fields.push(("name", Some(Value::String(name.trim().to_owned()))));
    }
    if kind.layered() {
        let mut layers =
            Layers::from_values(&object.values).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        if let Some(list) = &p.layers {
            if list.len() > et::MAX_LAYERS {
                return Err(
                    Error::new(archbuilder_model::error::codes::TYPE_LAYER_COUNT)
                        .with_arg("count", i64::try_from(list.len()).unwrap_or(i64::MAX))
                        .with_arg("most", i64::try_from(et::MAX_LAYERS).unwrap_or(i64::MAX)),
                );
            }
            let mut new = Vec::with_capacity(list.len());
            for l in list {
                let thickness = ctx.quantity(&l.thickness, Dimension::LENGTH)?.value();
                new.push(Layer::new(l.function.into(), &l.material, thickness));
            }
            layers.layers = new;
        }
        if let Some([start, end]) = p.core {
            layers.core = (start, end);
        }
        if let Some(problem) = layers.problems().into_iter().next() {
            return Err(problem);
        }
        let [start, end] = layers.core_values();
        fields.extend([
            ("layers", Some(layers.list_value())),
            ("core_start", Some(start)),
            ("core_end", Some(end)),
        ]);
        if kind == TypeKind::Wall {
            if let Some(e) = p.exposure {
                let e = match e {
                    ExposureParam::Exterior => Exposure::Exterior,
                    ExposureParam::Interior => Exposure::Interior,
                };
                fields.push(("exposure", Some(Value::Enum(e.code()))));
            }
            for (name, flag) in [
                ("load_bearing", p.load_bearing),
                ("room_bounding", p.room_bounding),
            ] {
                if let Some(flag) = flag {
                    fields.push((name, Some(Value::Bool(flag))));
                }
            }
            for (name, wrap) in [("wrap_ends", p.wrap_ends), ("wrap_inserts", p.wrap_inserts)] {
                if let Some(wrap) = wrap {
                    fields.push((name, Some(Value::Enum(Wrap::from(wrap).code()))));
                }
            }
        }
    } else {
        for (name, param) in [
            ("width", &p.width),
            ("height", &p.height),
            ("frame_width", &p.frame_width),
            ("frame_depth", &p.frame_depth),
            ("sill", &p.sill),
        ] {
            if let Some(m) = meters(ctx, param.as_ref())? {
                fields.push((name, Some(types::length(m))));
            }
        }
        if let Some(o) = p.operation {
            fields.push(("operation", Some(Value::Enum(Operation::from(o).code()))));
        }
    }
    if fields.is_empty() {
        return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
    }
    let txn = ctx.txn()?;
    for (field, value) in fields {
        txn.modify(id, FieldPath::new(field), value)?;
    }
    let changed = propagate(ctx, kind, id)?;
    ensure_valid(ctx)?;
    Ok(TypeChanged {
        element_type: Some(IdParam(id)),
        instances: changed,
        resynced: None,
    })
}

/// Writes the values the type `id` decides on each of its instances that differ; returns how
/// many changed.
fn propagate(ctx: &mut Context<'_>, kind: TypeKind, id: ObjectId) -> Result<usize> {
    let snapshot = ctx.txn()?.snapshot().clone();
    let Some(found) = spatial::get(&snapshot, id) else {
        return Ok(0);
    };
    let copies = et::copies(kind, &found.values);
    let mut changed = 0;
    for instance in et::instances(&snapshot, kind, id) {
        let mut touched = false;
        for (at, name, value) in &copies {
            let differs = instance
                .values
                .get(*at)
                .and_then(Option::as_ref)
                .is_none_or(|have| !same(have, value));
            if differs {
                ctx.txn()?
                    .modify(instance.id, FieldPath::new(*name), Some(value.clone()))?;
                touched = true;
            }
        }
        changed += usize::from(touched);
    }
    Ok(changed)
}

/// Parameters of `archbuilder.type.duplicate`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DuplicateType {
    /// The type to copy.
    #[serde(rename = "type")]
    pub element_type: IdParam,
    /// The name of the copy; else the type's name with the next free number.
    #[serde(default)]
    #[schemars(length(min = 1, max = 256))]
    pub name: Option<String>,
}

fn duplicate_type(ctx: &mut Context<'_>, p: DuplicateType) -> Result<TypeChanged> {
    let (kind, object) = type_record(ctx.snapshot(), p.element_type.0)?;
    let n = Names::new(&language(ctx));
    let base = p
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| et::name(&object))
        .to_owned();
    let name = free_name(ctx.snapshot(), kind, &base, &n);
    let mut values = object.values.clone();
    if let Some(slot) = values.get_mut(0) {
        *slot = Some(Value::String(name));
    }
    let id = create_type(ctx, kind, values)?;
    ensure_valid(ctx)?;
    Ok(TypeChanged {
        element_type: Some(IdParam(id)),
        instances: 0,
        resynced: None,
    })
}

/// Parameters of `archbuilder.type.delete`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteType {
    /// The type to delete.
    #[serde(rename = "type")]
    pub element_type: IdParam,
    /// The type its instances get instead; required while it has instances.
    #[serde(default)]
    pub replacement: Option<IdParam>,
}

fn delete_type(ctx: &mut Context<'_>, p: DeleteType) -> Result<TypeChanged> {
    let IdParam(id) = p.element_type;
    let (kind, object) = type_record(ctx.snapshot(), id)?;
    if spatial::objects(ctx.snapshot(), kind.key()).len() <= 1 {
        return Err(Error::new(codes::TYPE_LAST).with_arg("name", et::name(&object).to_owned()));
    }
    let users = et::instances(ctx.snapshot(), kind, id);
    let replacement = match p.replacement {
        Some(IdParam(r)) => {
            let (other, _) = type_record(ctx.snapshot(), r)?;
            if other != kind || r == id {
                return Err(Error::new(codes::TYPE_KIND));
            }
            Some(r)
        }
        None if users.is_empty() => None,
        None => {
            return Err(Error::new(codes::TYPE_IN_USE)
                .with_arg("name", et::name(&object).to_owned())
                .with_arg("count", i64::try_from(users.len()).unwrap_or(i64::MAX)));
        }
    };
    if let Some(r) = replacement {
        for user in &users {
            assign(ctx, user.id, kind, r)?;
        }
    }
    ctx.txn()?.delete(id)?;
    ensure_valid(ctx)?;
    Ok(TypeChanged {
        element_type: replacement.map(IdParam),
        instances: users.len(),
        resynced: None,
    })
}

/// Parameters of `archbuilder.type.assign`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssignType {
    /// The type: its identifier, or its name with `kind`.
    #[serde(rename = "type")]
    #[schemars(length(min = 1, max = 256))]
    pub element_type: String,
    /// With a type name: the kind of instance (`wall`, `floor`, `door`, `window`, `roof`); else the kind
    /// of the selection.
    #[serde(default)]
    #[schemars(length(max = 16))]
    pub kind: Option<String>,
    /// The objects to retype; else the selection. Objects of another kind are left alone.
    #[serde(default)]
    pub objects: Option<Vec<IdParam>>,
}

fn kind_named(name: &str) -> Option<TypeKind> {
    TypeKind::ALL.into_iter().find(|k| match k {
        TypeKind::Wall => name == "wall",
        TypeKind::Floor => name == "floor",
        TypeKind::Door => name == "door",
        TypeKind::Window => name == "window",
        TypeKind::Roof => name == "roof",
        TypeKind::Ceiling => name == "ceiling",
    })
}

/// The objects a retyping command works on: the given ones, else the selection.
fn targets(ctx: &Context<'_>, given: Option<Vec<IdParam>>) -> Result<Vec<ObjectId>> {
    match given {
        Some(list) if list.len() > MAX_OBJECTS => {
            Err(Error::new(arc_command::error::codes::PARAMS_INVALID))
        }
        Some(list) => Ok(list.into_iter().map(|IdParam(id)| id).collect()),
        None => Ok(ctx.selection().to_vec()),
    }
}

/// Gives each of `objects` that is an instance of `kind` the type `type_id`.
fn retype(
    ctx: &mut Context<'_>,
    kind: TypeKind,
    type_id: ObjectId,
    objects: &[ObjectId],
) -> Result<TypeChanged> {
    let mine: Vec<ObjectId> = objects
        .iter()
        .copied()
        .filter(|id| spatial::type_of(ctx.snapshot(), *id).as_deref() == Some(kind.instance_key()))
        .collect();
    if mine.is_empty() {
        return Err(Error::new(if objects.is_empty() {
            codes::NOTHING_SELECTED
        } else {
            codes::TYPE_KIND
        }));
    }
    for id in &mine {
        assign(ctx, *id, kind, type_id)?;
    }
    ensure_valid(ctx)?;
    Ok(TypeChanged {
        element_type: Some(IdParam(type_id)),
        instances: mine.len(),
        resynced: None,
    })
}

fn assign_type(ctx: &mut Context<'_>, p: AssignType) -> Result<TypeChanged> {
    let objects = targets(ctx, p.objects)?;
    let snapshot = ctx.snapshot();
    let by_id = p
        .element_type
        .trim()
        .parse::<ObjectId>()
        .ok()
        .and_then(|id| type_record(snapshot, id).ok().map(|(k, _)| (k, id)));
    let (kind, type_id) = match by_id {
        Some(found) => found,
        None => {
            let kind = match p.kind.as_deref() {
                Some(name) => kind_named(name).ok_or_else(|| Error::new(codes::TYPE_KIND))?,
                None => objects
                    .iter()
                    .find_map(|id| {
                        spatial::type_of(snapshot, *id).and_then(|k| TypeKind::of_instance_key(&k))
                    })
                    .ok_or_else(|| Error::new(codes::NOTHING_SELECTED))?,
            };
            (kind, resolve(snapshot, kind, &p.element_type)?)
        }
    };
    retype(ctx, kind, type_id, &objects)
}

/// Parameters of `archbuilder.type.match`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MatchType {
    /// The object whose type the others get.
    pub source: IdParam,
    /// The objects that get it; else the selection. Objects of another kind are left alone.
    #[serde(default)]
    pub objects: Option<Vec<IdParam>>,
}

fn match_type(ctx: &mut Context<'_>, p: MatchType) -> Result<TypeChanged> {
    let objects = targets(ctx, p.objects)?;
    let source =
        spatial::get(ctx.snapshot(), p.source.0).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let (kind, found) =
        et::type_of_object(ctx.snapshot(), &source).ok_or_else(|| Error::new(codes::TYPE_KIND))?;
    let objects: Vec<ObjectId> = objects.into_iter().filter(|id| *id != source.id).collect();
    retype(ctx, kind, found.id, &objects)
}

/// Parameters of `archbuilder.type.upgrade`: none.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Upgrade {}

/// Gives a document without any type the default set, resynchronizes instances whose copies of
/// their type's values differ, and gives every instance without a type a type that matches its
/// values, made when no type matches: one per distinct thickness or size.
fn upgrade(ctx: &mut Context<'_>, _: Upgrade) -> Result<TypeChanged> {
    let mut count = 0;
    let mut resynced = 0;
    // A document without any type gets the default set first; instances match it where they can.
    let snapshot = ctx.txn()?.snapshot().clone();
    if TypeKind::ALL
        .iter()
        .all(|k| spatial::objects(&snapshot, k.key()).is_empty())
        && !spatial::objects(&snapshot, types::PROJECT).is_empty()
    {
        create_defaults(ctx)?;
    }
    // An instance whose copies of its type's values differ (a file edited by hand) takes them
    // from its type again.
    for kind in TypeKind::ALL {
        let snapshot = ctx.txn()?.snapshot().clone();
        for found in spatial::objects(&snapshot, kind.key()) {
            resynced += propagate(ctx, kind, found.id)?;
        }
    }
    for kind in TypeKind::ALL {
        let snapshot = ctx.txn()?.snapshot().clone();
        let length = snapshot
            .registry()
            .get(kind.instance_key())
            .map_or(0, |d| d.schema.fields.len());
        for object in spatial::objects(&snapshot, kind.instance_key()) {
            if et::type_of(&snapshot, kind, &object.values).is_some() {
                continue;
            }
            let mut values = object.values.clone();
            values.resize(length.max(values.len()), None);
            let id = ensure(ctx, kind, &wanted(kind, &values))?;
            ctx.txn()?.modify(
                object.id,
                FieldPath::new(kind.field_name()),
                Some(Value::Reference(id)),
            )?;
            count += 1;
        }
    }
    ensure_valid(ctx)?;
    Ok(TypeChanged {
        element_type: None,
        instances: count,
        resynced: Some(resynced).filter(|n| *n > 0),
    })
}

/// Whether the document has instances without a type (a document from before types) or whose
/// copies of their type's values differ from it (a file edited by hand).
#[must_use]
pub fn needs_upgrade(snapshot: &Snapshot) -> bool {
    TypeKind::ALL.into_iter().any(|kind| {
        spatial::objects(snapshot, kind.instance_key())
            .iter()
            .any(|o| match et::type_of(snapshot, kind, &o.values) {
                None => true,
                Some(t) => et::mismatch(kind, &t.values, &o.values).is_some(),
            })
    })
}

fn spec<P, R, F>(id: &str, label: Msg, description: Msg, run: F) -> CommandSpec
where
    P: serde::de::DeserializeOwned + JsonSchema,
    R: Serialize + JsonSchema,
    F: Fn(&mut Context<'_>, P) -> Result<R> + Send + Sync + 'static,
{
    CommandSpec::new(id, label.message_id, description.message_id, run)
}

/// Registers the commands of this module.
///
/// # Errors
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    let specs = [
        spec(
            TYPE_SET,
            Msg {
                message_id: "ab-cmd-type-set-label",
            },
            Msg {
                message_id: "ab-cmd-type-set-description",
            },
            set_type,
        ),
        spec(
            TYPE_DUPLICATE,
            Msg {
                message_id: "ab-cmd-type-duplicate-label",
            },
            Msg {
                message_id: "ab-cmd-type-duplicate-description",
            },
            duplicate_type,
        ),
        spec(
            TYPE_DELETE,
            Msg {
                message_id: "ab-cmd-type-delete-label",
            },
            Msg {
                message_id: "ab-cmd-type-delete-description",
            },
            delete_type,
        ),
        spec(
            TYPE_ASSIGN,
            Msg {
                message_id: "ab-cmd-type-assign-label",
            },
            Msg {
                message_id: "ab-cmd-type-assign-description",
            },
            assign_type,
        ),
        spec(
            TYPE_MATCH,
            Msg {
                message_id: "ab-cmd-type-match-label",
            },
            Msg {
                message_id: "ab-cmd-type-match-description",
            },
            match_type,
        ),
        spec(
            TYPE_UPGRADE,
            Msg {
                message_id: "ab-cmd-type-upgrade-label",
            },
            Msg {
                message_id: "ab-cmd-type-upgrade-description",
            },
            upgrade,
        ),
    ];
    for spec in specs {
        registry.register(spec)?;
    }
    Ok(())
}
