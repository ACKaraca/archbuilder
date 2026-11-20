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
use crate::types::{self, BUILDING, LEVEL, PROJECT, SITE, STOREY};

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
    let own = folded(object);
    if key != PROJECT
        && objects(candidate, key)
            .iter()
            .any(|o| o.id != object.id && o.header.owner == object.header.owner && folded(o) == own)
    {
        out.push(name_taken(object));
    }
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
    out
}

/// The commit validator of the structure: every violation of [`check`] is an error.
#[must_use]
pub fn validator() -> DocumentValidator {
    Arc::new(|_, candidate, touched| {
        check(candidate, touched)
            .into_iter()
            .map(|error| Finding {
                severity: Severity::Error,
                error,
            })
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
