//! Model statistics: element counts by kind, by type and by storey, plus the number of views,
//! selection sets and element types. The report is derived from the document and never stored;
//! `archbuilder.model.statistics` returns it and `archbuilder-cli inspect` prints it.

use std::collections::BTreeMap;

use arc_base::{ObjectId, Result};
use arc_command::{CommandSpec, Context, Permission, Registry, UndoBehavior};
use arc_doc::Snapshot;
use archbuilder_model::element_type::{self as et, TypeKind};
use archbuilder_model::spatial;
use archbuilder_model::types::{self, PLAN_VIEW, SELECTION_SET, STOREY};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A message of the catalog, named by identifier.
struct MessageId {
    message_id: &'static str,
}

/// The identifier of the statistics command.
pub const STATISTICS: &str = "archbuilder.model.statistics";

/// How many elements of one kind.
#[derive(Debug, Serialize, JsonSchema)]
pub struct KindCount {
    /// The type key of the kind (`archbuilder.wall`).
    pub kind: String,
    /// The count.
    pub count: usize,
}

/// How many instances use one element type.
#[derive(Debug, Serialize, JsonSchema)]
pub struct TypeCount {
    /// The type key of the instances.
    pub kind: String,
    /// The type's name; empty for the instances without a type.
    pub type_name: String,
    /// The count.
    pub count: usize,
}

/// The elements of one storey.
#[derive(Debug, Serialize, JsonSchema)]
pub struct StoreyCount {
    /// The storey.
    pub storey: String,
    /// The storey's name.
    pub name: String,
    /// Its elements by kind.
    pub kinds: Vec<KindCount>,
}

/// The statistics of a document.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Statistics {
    /// Every element of the document.
    pub elements: usize,
    /// Elements by kind, kinds without an element left out.
    pub by_kind: Vec<KindCount>,
    /// Instances of the kinds that have types, by type.
    pub by_type: Vec<TypeCount>,
    /// Elements by storey and kind; elements without a storey are not counted here.
    pub by_storey: Vec<StoreyCount>,
    /// Plan views.
    pub views: usize,
    /// Selection sets.
    pub selection_sets: usize,
    /// Element types (wall, floor, door, window, roof and ceiling types).
    pub element_types: usize,
    /// The display units of the project, by kind (`length`, `area`, `volume`, `angle`, `slope`),
    /// the names of the unit registry; the quantities below are written in them.
    pub display_units: BTreeMap<String, String>,
    /// The sum of the floors' areas, in the project's area unit.
    pub floor_area: String,
}

/// The storey an element stands on, walking up through its hosts.
fn storey_of(snapshot: &Snapshot, id: ObjectId) -> Option<ObjectId> {
    let mut at = id;
    for _ in 0..8 {
        let owner = spatial::get(snapshot, at)?.header.owner?;
        if spatial::type_of(snapshot, owner).as_deref() == Some(STOREY) {
            return Some(owner);
        }
        at = owner;
    }
    None
}

fn kinds(map: BTreeMap<String, usize>) -> Vec<KindCount> {
    map.into_iter()
        .map(|(kind, count)| KindCount { kind, count })
        .collect()
}

/// The statistics of `snapshot`.
#[must_use]
pub fn statistics(snapshot: &Snapshot) -> Statistics {
    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_storey: BTreeMap<ObjectId, BTreeMap<String, usize>> = BTreeMap::new();
    let mut by_type: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut elements = 0;
    for key in types::ELEMENTS {
        let objects = spatial::objects(snapshot, key);
        if objects.is_empty() {
            continue;
        }
        elements += objects.len();
        by_kind.insert(key.to_owned(), objects.len());
        let kind = TypeKind::of_instance_key(key);
        for object in &objects {
            if let Some(storey) = storey_of(snapshot, object.id) {
                *by_storey
                    .entry(storey)
                    .or_default()
                    .entry(key.to_owned())
                    .or_default() += 1;
            }
            if let Some(kind) = kind {
                let name = et::type_of(snapshot, kind, &object.values)
                    .map(|t| et::name(&t).to_owned())
                    .unwrap_or_default();
                *by_type.entry((key.to_owned(), name)).or_default() += 1;
            }
        }
    }
    let storeys = spatial::objects(snapshot, STOREY);
    let by_storey = by_storey
        .into_iter()
        .map(|(id, counts)| StoreyCount {
            storey: id.to_string(),
            name: storeys
                .iter()
                .find(|s| s.id == id)
                .and_then(|s| types::text(&s.values, types::storey::NAME))
                .unwrap_or_default()
                .to_owned(),
            kinds: kinds(counts),
        })
        .collect();
    let units = spatial::unit_settings(snapshot);
    let floor_area: f64 = spatial::objects(snapshot, types::FLOOR)
        .iter()
        .filter_map(|f| archbuilder_model::floor::Derived::of(snapshot, f))
        .map(|d| d.area())
        .sum();
    let mut display_units = BTreeMap::new();
    for kind in archbuilder_model::units::UnitKind::ALL {
        let name = units
            .unit_name(kind)
            .unwrap_or_else(|| units.slope.name())
            .to_owned();
        display_units.insert(kind.name().to_owned(), name);
    }
    Statistics {
        elements,
        by_kind: kinds(by_kind),
        by_type: by_type
            .into_iter()
            .map(|((kind, type_name), count)| TypeCount {
                kind,
                type_name,
                count,
            })
            .collect(),
        by_storey,
        views: spatial::objects(snapshot, PLAN_VIEW).len(),
        selection_sets: spatial::objects(snapshot, SELECTION_SET).len(),
        element_types: types::TYPE_KEYS
            .iter()
            .map(|k| spatial::objects(snapshot, k).len())
            .sum(),
        display_units,
        floor_area: units.area_text(floor_area, '.'),
    }
}

/// Parameters of `archbuilder.model.statistics`: none.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoParams {}

/// Registers `archbuilder.model.statistics` (read-only, not undoable).
///
/// # Errors
/// `cmd.duplicate` when the identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(
        CommandSpec::new(
            STATISTICS,
            MessageId {
                message_id: "ab-cmd-statistics-label",
            }
            .message_id,
            MessageId {
                message_id: "ab-cmd-statistics-description",
            }
            .message_id,
            |ctx: &mut Context<'_>, _: NoParams| Ok(statistics(ctx.snapshot())),
        )
        .with_undo(UndoBehavior::NotUndoable)
        .with_permissions([Permission::Read]),
    )
}
