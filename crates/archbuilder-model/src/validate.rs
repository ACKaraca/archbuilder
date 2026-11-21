//! The validation rules of the spatial structure, as one pure check over a candidate snapshot
//! and the objects a transaction touched.
//!
//! [`check`] is shared by the commit validator ([`validator`], installed on a document with
//! [`install`]) and by the commands, which run it on their open transaction so that a refused
//! change gives a precise message and leaves the document unchanged. The rules:
//!
//! - a name is not blank;
//! - names are unique, compared by [`crate::names::fold`], among the objects of one type with the same
//!   container: sites of a project, buildings of a site, storeys of a building and levels of a
//!   building;
//! - a site is contained in the project, a building in a site, a storey and a level in a
//!   building, and a document holds one project;
//! - a wall is contained in a storey and its baseline is at least [`types::MIN_WALL_LENGTH`] long;
//! - a storey refers to a level of its own building that is flagged as a storey level, and no
//!   two storeys share a level.
//!
//! The type declarations carry the rules that need one object only (the blank name, the range
//! of an elevation) and the reference that blocks deleting a level while its storey exists.

use std::collections::BTreeMap;
use std::sync::Arc;

use arc_base::{Error, ObjectId};
use arc_doc::{Change, Document, DocumentValidator, Finding, Object, Severity, Snapshot, Value};

use crate::error::codes;
use crate::names;
use crate::spatial::{get, objects, type_of};
use crate::types::{self, BUILDING, DOOR, FLOOR, LEVEL, PROJECT, SITE, STOREY, WALL, WINDOW};

/// The key a name is compared by ([`crate::names::fold`]).
fn folded(object: &Object) -> String {
    names::fold(types::text(&object.values, 0).unwrap_or_default())
}

/// The type a container of `key` must have; `None` for the root.
fn container_of(key: &str) -> Option<&'static str> {
    match key {
        SITE => Some(PROJECT),
        BUILDING => Some(SITE),
        STOREY | LEVEL => Some(BUILDING),
        WALL | FLOOR => Some(STOREY),
        DOOR | WINDOW => Some(WALL),
        _ => None,
    }
}

fn name_taken(object: &Object) -> Error {
    Error::new(codes::NAME_TAKEN).with_arg(
        "name",
        types::text(&object.values, 0).unwrap_or_default().trim(),
    )
}

fn check_object(candidate: &Snapshot, key: &str, object: &Arc<Object>, out: &mut Vec<Error>) {
    if let Some(container) = container_of(key)
        && object
            .header
            .owner
            .and_then(|owner| type_of(candidate, owner))
            .as_deref()
            != Some(container)
    {
        out.push(Error::new(codes::PARENT_INVALID).with_arg("name", folded(object)));
    }
    if key == PROJECT && objects(candidate, PROJECT).len() > 1 {
        out.push(Error::new(codes::PROJECT_NOT_UNIQUE));
    }
    if key == WALL {
        if !crate::wall::Wall::from_values(&object.values)
            .is_some_and(|w| w.length() >= types::MIN_WALL_LENGTH)
        {
            out.push(Error::new(codes::WALL_TOO_SHORT).with_arg("name", display_name(object)));
        }
        if !top_is_above(candidate, object) {
            out.push(Error::new(codes::WALL_TOP_INVALID).with_arg("name", display_name(object)));
        }
    }
    if key == DOOR || key == WINDOW {
        out.extend(crate::insert::problems(candidate, object));
    }
    let own = folded(object);
    if key != PROJECT
        && objects(candidate, key)
            .iter()
            .any(|o| o.id != object.id && o.header.owner == object.header.owner && folded(o) == own)
    {
        out.push(name_taken(object));
    }
}

/// The name of an object as written.
fn display_name(object: &Object) -> String {
    types::text(&object.values, 0)
        .unwrap_or_default()
        .to_owned()
}

/// Whether the level a wall's top refers to is above the wall's base by at least the least height.
fn top_is_above(snapshot: &Snapshot, wall: &Object) -> bool {
    let Some(top) = crate::wall::Wall::top_level(&wall.values).and_then(|id| get(snapshot, id))
    else {
        return true;
    };
    let base = wall
        .header
        .owner
        .and_then(|id| get(snapshot, id))
        .map_or(0.0, |s| crate::spatial::storey_elevation(snapshot, &s))
        + types::meters(&wall.values, types::wall::BASE_OFFSET).unwrap_or(0.0);
    types::meters(&top.values, types::level::ELEVATION)
        .is_some_and(|e| e - base >= types::HEIGHT_RANGE.0)
}

fn check_storey(candidate: &Snapshot, storey: &Object, out: &mut Vec<Error>) {
    let level = match storey.values.get(types::storey::LEVEL) {
        Some(Some(Value::Reference(id))) => get(candidate, *id),
        _ => None,
    };
    let Some(level) = level else {
        // A missing level is the reference check's finding.
        return;
    };
    let flagged = matches!(
        level.values.get(types::level::STOREY),
        Some(Some(Value::Bool(true)))
    );
    let shared = objects(candidate, STOREY).iter().any(|other| {
        other.id != storey.id
            && matches!(other.values.get(types::storey::LEVEL),
                Some(Some(Value::Reference(id))) if *id == level.id)
    });
    if level.header.owner != storey.header.owner || !flagged || shared {
        out.push(
            Error::new(codes::LEVEL_INVALID)
                .with_arg("name", types::text(&storey.values, 0).unwrap_or_default()),
        );
    }
}

/// The violations of the rules among the touched objects of `candidate`.
#[must_use]
pub fn check(candidate: &Snapshot, touched: &BTreeMap<ObjectId, Change>) -> Vec<Error> {
    let mut out = Vec::new();
    let mut storeys: Vec<Arc<Object>> = Vec::new();
    for (&id, &change) in touched {
        if change == Change::Deleted {
            continue;
        }
        let (Some(key), Some(object)) = (type_of(candidate, id), get(candidate, id)) else {
            continue;
        };
        if !types::KEYS.contains(&key.as_str()) {
            continue;
        }
        check_object(candidate, &key, &object, &mut out);
        if key == STOREY {
            storeys.push(object);
        }
    }
    // A level that changed affects the storey that refers to it.
    for storey in objects(candidate, STOREY) {
        let level_touched = matches!(storey.values.get(types::storey::LEVEL),
            Some(Some(Value::Reference(id))) if touched.contains_key(id));
        if level_touched && !storeys.iter().any(|s| s.id == storey.id) {
            storeys.push(storey);
        }
    }
    for storey in &storeys {
        check_storey(candidate, storey, &mut out);
    }
    check_walls_of_levels(candidate, touched, &mut out);
    check_inserts_of_walls(candidate, touched, &mut out);
    check_floors(candidate, touched, &mut out);
    out
}

/// The floors a change can break: those that changed, and those on the storey of a wall that
/// changed (their associated edges follow the wall). Each boundary is checked as derived.
fn check_floors(candidate: &Snapshot, touched: &BTreeMap<ObjectId, Change>, out: &mut Vec<Error>) {
    let mut storeys: Vec<ObjectId> = Vec::new();
    let mut floors: Vec<Arc<Object>> = Vec::new();
    for (id, change) in touched {
        if *change == Change::Deleted {
            continue;
        }
        match type_of(candidate, *id).as_deref() {
            Some(WALL) => storeys.extend(get(candidate, *id).and_then(|w| w.header.owner)),
            Some(FLOOR) => floors.extend(get(candidate, *id)),
            _ => {}
        }
    }
    for storey in storeys {
        floors.extend(crate::spatial::children(candidate, FLOOR, storey));
    }
    floors.sort_by_key(|f| f.id);
    floors.dedup_by_key(|f| f.id);
    for floor in floors {
        let error = match crate::floor::Derived::of(candidate, &floor) {
            None => codes::FLOOR_MALFORMED,
            Some(d) => match d.boundary.check() {
                Ok(()) => continue,
                Err(r) => refusal_code(r),
            },
        };
        out.push(Error::new(error).with_arg("name", display_name(&floor)));
    }
}

/// The error code of a boundary refusal.
#[must_use]
pub fn refusal_code(refusal: crate::floor::Refusal) -> arc_base::ErrorCode {
    use crate::floor::Refusal;
    match refusal {
        Refusal::Malformed => codes::FLOOR_MALFORMED,
        Refusal::EdgeTooShort => codes::FLOOR_EDGE_SHORT,
        Refusal::SelfIntersecting => codes::FLOOR_SELF_INTERSECTING,
        Refusal::Area => codes::FLOOR_AREA,
        Refusal::HoleOutside => codes::FLOOR_HOLE_OUTSIDE,
        Refusal::HolesOverlap => codes::FLOOR_HOLES_OVERLAP,
    }
}

/// The inserts a change of other objects can break: those of every wall that changed or stands on
/// a storey that has a changed wall (joins move the ends), and those of walls on a storey or a
/// level that changed or whose top follows one (the base and the height move). An insert that
/// changed is checked on its own.
fn check_inserts_of_walls(
    candidate: &Snapshot,
    touched: &BTreeMap<ObjectId, Change>,
    out: &mut Vec<Error>,
) {
    let live = |id: &ObjectId| matches!(touched.get(id), Some(c) if *c != Change::Deleted);
    let mut storeys: Vec<ObjectId> = Vec::new();
    let mut walls: Vec<Arc<Object>> = Vec::new();
    for (id, change) in touched {
        if *change == Change::Deleted {
            continue;
        }
        match type_of(candidate, *id).as_deref() {
            Some(WALL) => storeys.extend(get(candidate, *id).and_then(|w| w.header.owner)),
            Some(STOREY) => storeys.push(*id),
            _ => {}
        }
    }
    let level_moved = touched
        .keys()
        .any(|id| live(id) && type_of(candidate, *id).as_deref() == Some(LEVEL));
    for wall in objects(candidate, WALL) {
        let on_level = level_moved
            && (crate::wall::Wall::top_level(&wall.values).is_some_and(|l| live(&l))
                || wall
                    .header
                    .owner
                    .and_then(|o| get(candidate, o))
                    .and_then(|s| crate::spatial::level_of(candidate, &s))
                    .is_some_and(|l| live(&l.id)));
        if on_level || wall.header.owner.is_some_and(|o| storeys.contains(&o)) {
            walls.push(wall);
        }
    }
    for wall in walls {
        for (insert, _) in crate::insert::hosted(candidate, wall.id) {
            if !live(&insert.id) {
                out.extend(crate::insert::problems(candidate, &insert));
            }
        }
    }
}

/// A level that moved must stay above the base of every wall whose top follows it. The walls are
/// not touched by the change, so they are found from the level: the walls whose top refers to
/// it, and the walls standing on the storey whose level it is. One error names the count.
fn check_walls_of_levels(
    candidate: &Snapshot,
    touched: &BTreeMap<ObjectId, Change>,
    out: &mut Vec<Error>,
) {
    let moved = |id: &ObjectId| {
        matches!(touched.get(id), Some(c) if *c != Change::Deleted)
            && type_of(candidate, *id).as_deref() == Some(LEVEL)
    };
    if !touched.keys().any(moved) {
        return;
    }
    let mut broken: Vec<String> = Vec::new();
    for wall in objects(candidate, WALL) {
        let on_moved = crate::wall::Wall::top_level(&wall.values).is_some_and(|l| moved(&l))
            || wall
                .header
                .owner
                .and_then(|o| get(candidate, o))
                .and_then(|s| crate::spatial::level_of(candidate, &s))
                .is_some_and(|l| moved(&l.id));
        // Walls that changed themselves are checked on their own.
        if on_moved && !touched.contains_key(&wall.id) && !top_is_above(candidate, &wall) {
            broken.push(display_name(&wall));
        }
    }
    if let Some(first) = broken.first() {
        out.push(
            Error::new(codes::LEVEL_BELOW_WALLS)
                .with_arg("count", broken.len().to_string())
                .with_arg("name", first.clone()),
        );
    }
}

/// The findings that are not errors: an open corner (the ends of two walls close together and not
/// joined), a floor edge that follows a wall that no longer bounds it, and a door or window that
/// would open into another wall. They concern the touched walls and the floors of their storeys.
#[must_use]
pub fn warnings(candidate: &Snapshot, touched: &BTreeMap<ObjectId, Change>) -> Vec<Error> {
    let mut out = Vec::new();
    let mut storeys: Vec<ObjectId> = Vec::new();
    for (id, change) in touched {
        if *change == Change::Deleted {
            continue;
        }
        if matches!(type_of(candidate, *id).as_deref(), Some(WALL | FLOOR)) {
            storeys.extend(get(candidate, *id).and_then(|o| o.header.owner));
        }
    }
    storeys.sort();
    storeys.dedup();
    for storey in storeys {
        let walls = crate::spatial::walls(candidate, storey);
        let data: Vec<(usize, crate::wall::Wall)> = walls
            .iter()
            .enumerate()
            .filter_map(|(k, o)| crate::wall::Wall::from_values(&o.values).map(|w| (k, w)))
            .collect();
        let plain: Vec<crate::wall::Wall> = data.iter().map(|(_, w)| *w).collect();
        for (i, j) in crate::join::open_corners(&plain) {
            let (a, b) = (&walls[data[i].0], &walls[data[j].0]);
            if touched.contains_key(&a.id) || touched.contains_key(&b.id) {
                out.push(
                    Error::new(codes::WALL_CORNER_OPEN)
                        .with_arg("name", display_name(a))
                        .with_arg("other", display_name(b)),
                );
            }
        }
        for floor in crate::spatial::children(candidate, FLOOR, storey) {
            let Some(derived) = crate::floor::Derived::of(candidate, &floor) else {
                continue;
            };
            let wall_of = |id: ObjectId| {
                get(candidate, id).and_then(|o| crate::wall::Wall::from_values(&o.values))
            };
            for wall in derived.floor.unsupported_edges(&derived.boundary, &wall_of) {
                out.push(
                    Error::new(codes::FLOOR_EDGE_UNSUPPORTED)
                        .with_arg("name", display_name(&floor))
                        .with_arg(
                            "wall",
                            get(candidate, wall)
                                .map(|w| display_name(&w))
                                .unwrap_or_default(),
                        ),
                );
            }
        }
    }
    out
}

/// Whether `error` is a warning of the structure: it is reported, never refused.
#[must_use]
pub fn is_warning(error: &Error) -> bool {
    error.code() == codes::INSERT_MEETS_WALL.code
}

/// The commit validator of the structure: every violation of [`check`] is an error, except the
/// warnings ([`is_warning`]), which are findings of the lower severity.
#[must_use]
pub fn validator() -> DocumentValidator {
    Arc::new(|_, candidate, touched| {
        check(candidate, touched)
            .into_iter()
            .map(|error| Finding {
                severity: if is_warning(&error) {
                    Severity::Warning
                } else {
                    Severity::Error
                },
                error,
            })
            .chain(
                warnings(candidate, touched)
                    .into_iter()
                    .map(|error| Finding {
                        severity: Severity::Warning,
                        error,
                    }),
            )
            .collect()
    })
}

/// Adds [`validator`] to `document`. A document the application opens through its own package
/// path has none; the headless runner and the tests install it.
pub fn install(document: &Document) {
    document.add_validator(validator());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{register_types, types::length};
    use arc_base::id::Identifier;
    use arc_doc::{Header, SchemaVersion, TypeRegistry, WorkingSet};

    fn id(n: u128) -> ObjectId {
        ObjectId::from_u128(n)
    }

    /// A snapshot of a project, a site, a building and what `extra` adds.
    fn snapshot(extra: impl FnOnce(&mut Vec<(&'static str, Object)>)) -> (Snapshot, Vec<ObjectId>) {
        let mut registry = TypeRegistry::new();
        register_types(&mut registry).unwrap();
        let base = Snapshot::empty(Arc::new(registry));
        let mut set = WorkingSet::new(&base);
        let v = SchemaVersion::new(1, 0);
        let owned = |owner: u128| Header {
            owner: Some(id(owner)),
            ..Header::default()
        };
        let named = |n: &str| Some(Value::String(n.to_owned()));
        let mut items: Vec<(&'static str, Object)> = vec![
            (
                PROJECT,
                Object::new(
                    id(1),
                    v,
                    Header::default(),
                    vec![
                        named("P"),
                        None,
                        None,
                        None,
                        Some(Value::Enum(0)),
                        Some(Value::Bool(false)),
                    ],
                ),
            ),
            (SITE, Object::new(id(2), v, owned(1), vec![named("S")])),
            (
                BUILDING,
                Object::new(
                    id(3),
                    v,
                    owned(2),
                    vec![named("B"), Some(length(0.0)), None],
                ),
            ),
        ];
        extra(&mut items);
        let ids: Vec<ObjectId> = items.iter().map(|(_, o)| o.id).collect();
        for (key, object) in items {
            set.create(key, object).unwrap();
        }
        (set.current().clone(), ids)
    }

    fn level(n: u128, name: &str, z: f64, flag: bool) -> (&'static str, Object) {
        (
            LEVEL,
            Object::new(
                id(n),
                SchemaVersion::new(1, 0),
                Header {
                    owner: Some(id(3)),
                    ..Header::default()
                },
                vec![
                    Some(Value::String(name.to_owned())),
                    Some(length(z)),
                    Some(Value::Enum(0)),
                    Some(Value::Bool(flag)),
                ],
            ),
        )
    }

    fn storey(n: u128, name: &str, level: u128) -> (&'static str, Object) {
        (
            STOREY,
            Object::new(
                id(n),
                SchemaVersion::new(1, 0),
                Header {
                    owner: Some(id(3)),
                    ..Header::default()
                },
                vec![
                    Some(Value::String(name.to_owned())),
                    Some(Value::Reference(id(level))),
                ],
            ),
        )
    }

    fn violations(extra: impl FnOnce(&mut Vec<(&'static str, Object)>)) -> Vec<&'static str> {
        let (snapshot, ids) = snapshot(extra);
        let touched = ids.into_iter().map(|i| (i, Change::Created)).collect();
        check(&snapshot, &touched).iter().map(Error::code).collect()
    }

    #[test]
    fn a_consistent_structure_has_no_violations() {
        let found = violations(|v| {
            v.extend([level(10, "L0", 0.0, true), storey(11, "S0", 10)]);
        });
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn storey_names_are_unique_within_a_building_without_regard_to_case() {
        let found = violations(|v| {
            v.extend([
                level(10, "A", 0.0, true),
                level(12, "B", 3.0, true),
                storey(11, "Ground", 10),
                storey(13, " ground ", 12),
            ]);
        });
        assert_eq!(found, ["ab.model.name_taken", "ab.model.name_taken"]);
    }

    #[test]
    fn a_storey_needs_its_own_storey_level_that_no_other_storey_uses() {
        let shared = violations(|v| {
            v.extend([
                level(10, "A", 0.0, true),
                storey(11, "S1", 10),
                storey(12, "S2", 10),
            ]);
        });
        assert_eq!(shared, ["ab.model.level_invalid", "ab.model.level_invalid"]);
        let unflagged =
            violations(|v| v.extend([level(10, "A", 0.0, false), storey(11, "S1", 10)]));
        assert_eq!(unflagged, ["ab.model.level_invalid"]);
    }

    #[test]
    fn containment_is_by_kind() {
        let found = violations(|v| {
            let mut stray = level(10, "A", 0.0, true);
            stray.1.header.owner = Some(id(2));
            v.push(stray);
        });
        assert_eq!(found, ["ab.model.parent_invalid"]);
    }
}
