//! Reading the spatial structure from a snapshot: the project, the children of an object and
//! the storeys of a building with their levels.

use std::cmp::Ordering;
use std::sync::Arc;

use arc_base::ObjectId;
use arc_doc::{Object, Snapshot, Value};

use crate::types::{self, BUILDING, LEVEL, PROJECT, SITE, STOREY, WALL};
use crate::units::LengthUnit;

/// A storey with its level, when the reference resolves.
#[derive(Debug, Clone)]
pub struct StoreyLevel {
    /// The storey.
    pub storey: Arc<Object>,
    /// The storey's level.
    pub level: Option<Arc<Object>>,
}

impl StoreyLevel {
    /// The elevation of the storey's level in meters; zero without a level.
    #[must_use]
    pub fn elevation(&self) -> f64 {
        self.level
            .as_ref()
            .and_then(|l| types::meters(&l.values, types::level::ELEVATION))
            .unwrap_or(0.0)
    }
}

/// The type key of the object `id`, when it is live.
#[must_use]
pub fn type_of(snapshot: &Snapshot, id: ObjectId) -> Option<String> {
    let handle = snapshot.resolve(id).ok()?;
    Some(snapshot.type_key(handle)?.as_str().to_owned())
}

/// The live object `id`.
#[must_use]
pub fn get(snapshot: &Snapshot, id: ObjectId) -> Option<Arc<Object>> {
    let handle = snapshot.resolve(id).ok()?;
    snapshot.get(handle).ok().cloned()
}

/// The objects of type `key`, in identifier order.
#[must_use]
pub fn objects(snapshot: &Snapshot, key: &str) -> Vec<Arc<Object>> {
    let mut found: Vec<Arc<Object>> = snapshot
        .iter_type(key)
        .map(|it| it.map(|(_, o)| Arc::clone(o)).collect())
        .unwrap_or_default();
    found.sort_by_key(|o| o.id);
    found
}

/// The objects of type `key` contained in `owner`, in identifier order.
#[must_use]
pub fn children(snapshot: &Snapshot, key: &str, owner: ObjectId) -> Vec<Arc<Object>> {
    let mut found = objects(snapshot, key);
    found.retain(|o| o.header.owner == Some(owner));
    found
}

/// The project: the first by identifier when a damaged document holds several.
#[must_use]
pub fn project(snapshot: &Snapshot) -> Option<Arc<Object>> {
    objects(snapshot, PROJECT).into_iter().next()
}

/// The first site of the project.
#[must_use]
pub fn site(snapshot: &Snapshot) -> Option<Arc<Object>> {
    objects(snapshot, SITE).into_iter().next()
}

/// The buildings of the document, in identifier order.
#[must_use]
pub fn buildings(snapshot: &Snapshot) -> Vec<Arc<Object>> {
    objects(snapshot, BUILDING)
}

/// The level a storey refers to.
#[must_use]
pub fn level_of(snapshot: &Snapshot, storey: &Object) -> Option<Arc<Object>> {
    match storey.values.get(types::storey::LEVEL)? {
        Some(Value::Reference(id)) => get(snapshot, *id),
        _ => None,
    }
}

/// The storeys of `building` with their levels, from the lowest level up (equal elevations by
/// name, then identifier).
#[must_use]
pub fn storeys(snapshot: &Snapshot, building: ObjectId) -> Vec<StoreyLevel> {
    let mut found: Vec<StoreyLevel> = children(snapshot, STOREY, building)
        .into_iter()
        .map(|storey| StoreyLevel {
            level: level_of(snapshot, &storey),
            storey,
        })
        .collect();
    found.sort_by(|a, b| {
        a.elevation()
            .partial_cmp(&b.elevation())
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                types::text(&a.storey.values, types::storey::NAME)
                    .cmp(&types::text(&b.storey.values, types::storey::NAME))
            })
            .then_with(|| a.storey.id.cmp(&b.storey.id))
    });
    found
}

/// The levels of `building` (storey levels and others), from the lowest up.
#[must_use]
pub fn levels(snapshot: &Snapshot, building: ObjectId) -> Vec<Arc<Object>> {
    let mut found = children(snapshot, LEVEL, building);
    found.sort_by(|a, b| {
        let at = |o: &Object| types::meters(&o.values, types::level::ELEVATION).unwrap_or(0.0);
        at(a).partial_cmp(&at(b)).unwrap_or(Ordering::Equal)
    });
    found
}

/// The walls standing on `storey`, in identifier order.
#[must_use]
pub fn walls(snapshot: &Snapshot, storey: ObjectId) -> Vec<Arc<Object>> {
    children(snapshot, WALL, storey)
}

/// The elevation of the level of `storey` in meters; zero without a level.
#[must_use]
pub fn storey_elevation(snapshot: &Snapshot, storey: &Object) -> f64 {
    level_of(snapshot, storey)
        .and_then(|l| types::meters(&l.values, types::level::ELEVATION))
        .unwrap_or(0.0)
}

/// The default height of a wall on `storey`: the distance from its level to the next higher
/// level of a storey of the same building, else [`crate::defaults::STOREY_HEIGHT`].
#[must_use]
pub fn default_wall_height(snapshot: &Snapshot, storey: &Object) -> f64 {
    let own = storey_elevation(snapshot, storey);
    storey
        .header
        .owner
        .into_iter()
        .flat_map(|building| storeys(snapshot, building))
        .map(|s| s.elevation())
        .find(|e| *e > own + types::HEIGHT_RANGE.0)
        .map_or(crate::defaults::STOREY_HEIGHT, |next| next - own)
}

/// The level of the next higher storey of the building of `storey` (the lowest one above its
/// level by more than the least wall height); `None` for the highest storey.
#[must_use]
pub fn level_above(snapshot: &Snapshot, storey: &Object) -> Option<Arc<Object>> {
    let own = storey_elevation(snapshot, storey);
    storeys(snapshot, storey.header.owner?)
        .into_iter()
        .find(|s| s.elevation() > own + types::HEIGHT_RANGE.0)
        .and_then(|s| s.level)
}

/// The lowest storey of the first building.
#[must_use]
pub fn lowest_storey(snapshot: &Snapshot) -> Option<Arc<Object>> {
    let building = buildings(snapshot).into_iter().next()?;
    storeys(snapshot, building.id)
        .into_iter()
        .next()
        .map(|s| s.storey)
}

/// The display units of the project (AB-DATUM-004); the defaults without a project.
#[must_use]
pub fn unit_settings(snapshot: &Snapshot) -> crate::units::UnitSettings {
    project(snapshot).map_or_else(Default::default, |p| {
        crate::units::UnitSettings::of(&p.values)
    })
}

/// The length unit of the project; meters without one.
#[must_use]
pub fn length_unit(snapshot: &Snapshot) -> LengthUnit {
    project(snapshot)
        .and_then(|p| match p.values.get(types::project::LENGTH_UNIT)? {
            Some(Value::Enum(code)) => Some(LengthUnit::from_code(*code)),
            _ => None,
        })
        .unwrap_or_default()
}
