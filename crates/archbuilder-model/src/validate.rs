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
use crate::types::{
    self, BUILDING, CEILING, DOOR, FLOOR, LEVEL, PROJECT, ROOF, ROOM, SHAFT, SITE, STAIR, STOREY,
    VERTICAL_OPENING, WALL, WALL_OPENING, WINDOW,
};

/// The key a name is compared by ([`crate::names::fold`]).
fn folded(object: &Object) -> String {
    names::fold(types::text(&object.values, 0).unwrap_or_default())
}

/// The type a container of `key` must have; `None` for the root.
fn container_of(key: &str) -> Option<&'static [&'static str]> {
    match key {
        SITE => Some(&[PROJECT]),
        BUILDING => Some(&[SITE]),
        STOREY | LEVEL => Some(&[BUILDING]),
        k if types::is_datum(k) => Some(&[BUILDING]),
        types::PLAN_VIEW => Some(&[STOREY]),
        k if types::is_storey_element(k) => Some(&[STOREY]),
        DOOR | WINDOW | WALL_OPENING => Some(&[WALL]),
        VERTICAL_OPENING => Some(&types::VERTICAL_HOSTS),
        k if types::TYPE_KEYS.contains(&k) => Some(&[PROJECT]),
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
    if let Some(containers) = container_of(key)
        && !object
            .header
            .owner
            .and_then(|owner| type_of(candidate, owner))
            .is_some_and(|owner| containers.contains(&owner.as_str()))
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
    if key == DOOR || key == WINDOW || key == WALL_OPENING {
        out.extend(crate::insert::problems(candidate, object));
    }
    if key == STAIR {
        out.extend(stair_problem(candidate, object));
    }
    if key == types::PLAN_VIEW {
        out.extend(crate::view::problem(object));
    }
    if types::is_datum(key) && crate::datum::is_degenerate(candidate, key, object) {
        out.push(Error::new(codes::DATUM_DEGENERATE).with_arg("name", display_name(object)));
    }
    out.extend(type_problem(candidate, key, object));
    let own = folded(object);
    // Rooms are told apart by number, not by name.
    if key != PROJECT
        && key != ROOM
        && key != types::GRID
        && objects(candidate, key)
            .iter()
            .any(|o| o.id != object.id && o.header.owner == object.header.owner && folded(o) == own)
    {
        out.push(name_taken(object));
    }
}

/// What breaks the rules of types in `object`: an instance whose copies of its type's values
/// differ from the type, or a door or window type with an operation its kind does not allow.
fn type_problem(candidate: &Snapshot, key: &str, object: &Object) -> Option<Error> {
    use crate::element_type::{self as et, TypeKind};
    if let Some(kind) = TypeKind::of_key(key) {
        let insert = kind.insert()?;
        let operation = et::InsertType::from_values(&object.values)?.operation;
        return (!operation.allowed(insert)).then(|| {
            Error::new(codes::TYPE_OPERATION)
                .with_arg("name", display_name(object))
                .with_arg("operation", operation.name())
        });
    }
    let kind = TypeKind::of_instance_key(key)?;
    let found = et::type_of(candidate, kind, &object.values)?;
    let field = et::mismatch(kind, &found.values, &object.values)?;
    Some(
        Error::new(codes::TYPE_MISMATCH)
            .with_arg("name", display_name(object))
            .with_arg("field", field),
    )
}

/// The instances of every type that changed must still match it: a type is changed together
/// with its instances.
fn check_instances_of_types(
    candidate: &Snapshot,
    touched: &BTreeMap<ObjectId, Change>,
    out: &mut Vec<Error>,
) {
    use crate::element_type::{self as et, TypeKind};
    for (id, change) in touched {
        if *change == Change::Deleted {
            continue;
        }
        let Some(kind) = type_of(candidate, *id).and_then(|k| TypeKind::of_key(&k)) else {
            continue;
        };
        for instance in et::instances(candidate, kind, *id) {
            if !touched.contains_key(&instance.id) {
                out.extend(type_problem(candidate, kind.instance_key(), &instance));
            }
        }
    }
}

/// Why a stair has no layout: no level above its base, a riser count out of bounds, or a record
/// that cannot be read.
#[must_use]
pub fn stair_problem(candidate: &Snapshot, object: &Object) -> Option<Error> {
    use crate::stair::{MAX_RISERS, Placed, Refusal};
    let name = display_name(object);
    let Some(placed) = Placed::of(candidate, object) else {
        return Some(Error::new(codes::STAIR_MALFORMED).with_arg("name", name));
    };
    match placed.layout {
        Ok(_) => None,
        Err(Refusal::NoTop) => Some(Error::new(codes::STAIR_NO_TOP).with_arg("name", name)),
        Err(Refusal::Malformed) => Some(Error::new(codes::STAIR_MALFORMED).with_arg("name", name)),
        Err(Refusal::Risers { count, least }) => Some(
            Error::new(codes::STAIR_RISERS)
                .with_arg("name", name)
                .with_arg("count", count.to_string())
                .with_arg("least", least.to_string())
                .with_arg("most", MAX_RISERS.to_string()),
        ),
    }
}

/// The warnings of a stair: a stride outside the comfortable range, another stair of its storey
/// that overlaps it, and a shaft opening that reaches a floor of the top level but could not be
/// cut out of it. Each carries the stair's identifier as the detail `object`, so a surface can
/// select it.
#[must_use]
pub fn stair_warnings(candidate: &Snapshot, object: &Object) -> Vec<Error> {
    let mut out = Vec::new();
    let Some(placed) = crate::stair::Placed::of(candidate, object) else {
        return out;
    };
    let Ok(layout) = &placed.layout else {
        return out;
    };
    let own = layout.footprint();
    for other in object
        .header
        .owner
        .map(|s| crate::spatial::children(candidate, STAIR, s))
        .unwrap_or_default()
        .iter()
        .filter(|o| o.id != object.id)
        .take(crate::room::MAX_ROOMS)
    {
        let theirs = crate::stair::Placed::of(candidate, other).map(|p| p.outline());
        if theirs.is_some_and(|t| crate::stair::outlines_overlap(&own, &t)) {
            out.push(
                Error::new(codes::STAIR_OVERLAP)
                    .with_arg("name", display_name(object))
                    .with_arg("other", display_name(other))
                    .with_detail(OTHER, other.id.to_string()),
            );
        }
    }
    if !layout.stride_ok() {
        let (low, high) = crate::stair::STRIDE_RANGE;
        out.push(
            Error::new(codes::STAIR_STRIDE)
                .with_arg("name", display_name(object))
                .with_arg("stride", format!("{:.3}", layout.stride()))
                .with_arg("low", format!("{low:.2}"))
                .with_arg("high", format!("{high:.2}")),
        );
    }
    let Some(top) = placed.top_level else {
        return with_object(out, object.id);
    };
    for storey in objects(candidate, STOREY) {
        if crate::spatial::level_of(candidate, &storey).is_none_or(|l| l.id != top) {
            continue;
        }
        for floor in crate::spatial::children(candidate, FLOOR, storey.id) {
            if crate::floor::Derived::of(candidate, &floor).is_some_and(|d| d.uncut > 0) {
                out.push(
                    Error::new(codes::STAIR_OPENING_UNCUT)
                        .with_arg("name", display_name(object))
                        .with_arg("floor", display_name(&floor)),
                );
            }
        }
    }
    with_object(out, object.id)
}

/// The detail of a finding that names the element it is about, as an identifier.
const OBJECT: &str = "object";
/// The detail of a finding that names the second element it involves.
const OTHER: &str = "other_object";

/// `errors` with the detail `object` set to `id`.
fn with_object(errors: Vec<Error>, id: ObjectId) -> Vec<Error> {
    errors
        .into_iter()
        .map(|e| e.with_detail(OBJECT, id.to_string()))
        .collect()
}

/// The pairs of walls of `storey` that lie on top of each other, as warnings.
#[must_use]
pub fn overlap_findings(candidate: &Snapshot, storey: ObjectId) -> Vec<Error> {
    let walls = crate::floor::storey_walls(candidate, storey);
    crate::floor::overlapping_walls(&walls)
        .into_iter()
        .map(|(a, b)| {
            let name = |id| {
                get(candidate, id)
                    .map(|o| display_name(&o))
                    .unwrap_or_default()
            };
            Error::new(codes::WALL_OVERLAP)
                .with_arg("name", name(a))
                .with_arg("other", name(b))
                .with_detail(OBJECT, a.to_string())
                .with_detail(OTHER, b.to_string())
        })
        .collect()
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
    check_ceilings(candidate, touched, &mut out);
    check_openings(candidate, touched, &mut out);
    check_roofs(candidate, touched, &mut out);
    check_instances_of_types(candidate, touched, &mut out);
    out
}

/// The roofs a change can break: those that changed, and those with an edge that follows a wall
/// that changed. Each surface is derived; a roof without one is refused.
fn check_roofs(candidate: &Snapshot, touched: &BTreeMap<ObjectId, Change>, out: &mut Vec<Error>) {
    let walls: Vec<ObjectId> = touched
        .iter()
        .filter(|(id, c)| {
            **c != Change::Deleted && type_of(candidate, **id).as_deref() == Some(WALL)
        })
        .map(|(id, _)| *id)
        .collect();
    for roof in objects(candidate, ROOF) {
        let changed = matches!(touched.get(&roof.id), Some(c) if *c != Change::Deleted);
        let follows = !walls.is_empty()
            && crate::roof::Roof::from_values(&roof.values)
                .is_some_and(|r| r.walls().iter().any(|w| walls.contains(w)));
        if !changed && !follows {
            continue;
        }
        if let Err(refusal) = crate::roof::Derived::of(candidate, &roof) {
            out.push(crate::roof::refusal_error(refusal, &display_name(&roof)));
        }
    }
}

/// Why a wall attached to a roof or a floor cannot follow its underside, as a warning; `None`
/// for a wall that is not attached or follows it.
#[must_use]
pub fn attach_problem(candidate: &Snapshot, wall: &Object) -> Option<Error> {
    let target = crate::wall::attachment(&wall.values).and_then(|id| get(candidate, id))?;
    let data = crate::wall::Wall::from_values(&wall.values)?;
    let level = wall
        .header
        .owner
        .and_then(|id| get(candidate, id))
        .map_or(0.0, |s| crate::spatial::storey_elevation(candidate, &s));
    let fails = match type_of(candidate, target.id).as_deref() {
        Some(ROOF) => match crate::roof::Derived::of(candidate, &target) {
            Ok(d) => {
                let footprint = crate::join::footprint_of(candidate, wall)
                    .unwrap_or_else(|| data.plain_footprint());
                crate::wall::under_roof(&data, footprint, level, &d.surface).is_err()
            }
            Err(_) => true,
        },
        Some(FLOOR) => crate::floor::Derived::of(candidate, &target).is_none_or(|d| {
            d.top - d.floor.thickness - (level + data.base_offset) < types::HEIGHT_RANGE.0
        }),
        _ => true,
    };
    fails.then(|| {
        Error::new(codes::WALL_ATTACH_FAILED)
            .with_arg("name", display_name(wall))
            .with_arg("target", display_name(&target))
            .with_detail(OBJECT, wall.id.to_string())
            .with_detail(OTHER, target.id.to_string())
    })
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

/// The ceilings a change can break: those that changed, and those on the storey of a wall that
/// changed (their associated edges follow the wall). Each boundary is checked as derived.
fn check_ceilings(
    candidate: &Snapshot,
    touched: &BTreeMap<ObjectId, Change>,
    out: &mut Vec<Error>,
) {
    let mut storeys: Vec<ObjectId> = Vec::new();
    let mut found: Vec<Arc<Object>> = Vec::new();
    for (id, change) in touched {
        if *change == Change::Deleted {
            continue;
        }
        match type_of(candidate, *id).as_deref() {
            Some(WALL) => storeys.extend(get(candidate, *id).and_then(|w| w.header.owner)),
            Some(CEILING) => found.extend(get(candidate, *id)),
            _ => {}
        }
    }
    for storey in storeys {
        found.extend(crate::spatial::children(candidate, CEILING, storey));
    }
    found.sort_by_key(|f| f.id);
    found.dedup_by_key(|f| f.id);
    let checked: Vec<ObjectId> = found.iter().map(|f| f.id).collect();
    for ceiling in found {
        let error = match crate::ceiling::derived(candidate, &ceiling) {
            None => codes::CEILING_MALFORMED,
            Some(d) => match d.raw.check() {
                Ok(()) if !d.ceiling.slope_ok() => codes::CEILING_SLOPE,
                Ok(()) => match outside_range(candidate, &ceiling, &d) {
                    Some(error) => {
                        out.push(error);
                        continue;
                    }
                    None => continue,
                },
                Err(_) => codes::CEILING_BOUNDARY,
            },
        };
        out.push(Error::new(error).with_arg("name", display_name(&ceiling)));
    }
    // What lies above a ceiling (slabs, roofs, the walls they follow, levels, storeys) or cuts it
    // can change under it: the others are checked for their range alone, as warnings.
    let moves_bounds = touched.iter().any(|(id, c)| {
        *c != Change::Deleted
            && matches!(
                type_of(candidate, *id).as_deref(),
                Some(WALL | FLOOR | ROOF | LEVEL | STOREY | SHAFT | VERTICAL_OPENING)
            )
    });
    if moves_bounds {
        for ceiling in objects(candidate, CEILING) {
            if checked.contains(&ceiling.id) {
                continue;
            }
            if let Some(d) = crate::ceiling::derived(candidate, &ceiling)
                && let Some(error) = outside_range(candidate, &ceiling, &d)
            {
                out.push(error);
            }
        }
    }
}

/// The error when the underside of `d` leaves the range from its storey's level up to what bounds
/// it above: the slab or roof over its own footprint, else the next level (a slope arrow far from
/// the ceiling can carry it below its floor or through what is above).
fn outside_range(
    candidate: &Snapshot,
    ceiling: &Object,
    d: &crate::ceiling::Derived,
) -> Option<Error> {
    let name = display_name(ceiling);
    let (lo, hi) = d.z_range();
    let top = hi;
    let storey = ceiling.header.owner.and_then(|s| get(candidate, s));
    if lo < d.level - 1.0e-6 {
        return Some(
            Error::new(codes::CEILING_RANGE)
                .with_arg("name", name)
                .with_detail(OBJECT, ceiling.id.to_string()),
        );
    }
    if let Some(s) = &storey
        && let Some(bound) =
            crate::room::bound_passed_through(candidate, s, &d.pieces(), d.ceiling.thickness())
    {
        let id = match bound {
            crate::room::Bound::Ceiling(id)
            | crate::room::Bound::Roof(id)
            | crate::room::Bound::Slab(id) => id,
        };
        let over = get(candidate, id)
            .map(|o| display_name(&o))
            .unwrap_or_default();
        return Some(
            Error::new(codes::CEILING_RANGE_ELEMENT)
                .with_arg("name", name)
                .with_arg("over", over)
                .with_detail(OBJECT, ceiling.id.to_string()),
        );
    }
    let level_above = storey
        .as_ref()
        .and_then(|s| crate::room::levels_above(candidate, s).into_iter().next())
        .and_then(|l| crate::types::meters(&l.values, crate::types::level::ELEVATION));
    level_above.is_some_and(|a| top > a + 1.0e-6).then(|| {
        Error::new(codes::CEILING_RANGE)
            .with_arg("name", name)
            .with_detail(OBJECT, ceiling.id.to_string())
    })
}

/// The shafts and vertical openings that changed must have a footprint the model can read; one
/// that cannot is refused. A vertical opening that reaches outside its host is a warning
/// ([`warnings`]).
fn check_openings(
    candidate: &Snapshot,
    touched: &BTreeMap<ObjectId, Change>,
    out: &mut Vec<Error>,
) {
    use crate::floor::{SHAFT_SLOTS, VERTICAL_SLOTS};
    for (id, change) in touched {
        if *change == Change::Deleted {
            continue;
        }
        let (Some(key), Some(object)) = (type_of(candidate, *id), get(candidate, *id)) else {
            continue;
        };
        let slots = match key.as_str() {
            SHAFT => SHAFT_SLOTS,
            VERTICAL_OPENING => VERTICAL_SLOTS,
            _ => continue,
        };
        let ok = crate::floor::Floor::read(&object.values, slots)
            .and_then(|f| f.boundary_in(candidate))
            .is_some_and(|b| b.loops.len() == 1 && b.check().is_ok());
        if !ok {
            out.push(Error::new(codes::OPENING_FOOTPRINT).with_arg("name", display_name(&object)));
        }
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

/// The grid labels used twice, when a grid changed.
fn label_warnings(candidate: &Snapshot, touched: &BTreeMap<ObjectId, Change>) -> Vec<Error> {
    let changed = touched.iter().any(|(id, c)| {
        *c != Change::Deleted && type_of(candidate, *id).as_deref() == Some(types::GRID)
    });
    if !changed {
        return Vec::new();
    }
    crate::datum::duplicate_labels(candidate)
        .into_iter()
        .map(|(label, found)| {
            Error::new(codes::DATUM_LABEL_DUPLICATE)
                .with_arg("name", label)
                .with_arg("count", found.len().to_string())
        })
        .collect()
}

/// The findings that are not errors: walls on top of each other, a stair's stride or uncut
/// shaft opening, an open corner (the ends of two walls close together and not
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
        if type_of(candidate, *id).is_some_and(|k| types::is_storey_element(&k)) {
            storeys.extend(get(candidate, *id).and_then(|o| o.header.owner));
        }
    }
    storeys.sort();
    storeys.dedup();
    for (id, change) in touched {
        if *change != Change::Deleted
            && type_of(candidate, *id).as_deref() == Some(STAIR)
            && let Some(object) = get(candidate, *id)
        {
            out.extend(stair_warnings(candidate, &object));
        }
    }
    // Walls attached to a roof or floor that changed, wherever they stand.
    let targets: Vec<ObjectId> = touched
        .iter()
        .filter(|(id, c)| {
            **c != Change::Deleted
                && matches!(type_of(candidate, **id).as_deref(), Some(ROOF | FLOOR))
        })
        .map(|(id, _)| *id)
        .collect();
    for wall in objects(candidate, WALL) {
        let attached = crate::wall::attachment(&wall.values);
        let mine = matches!(touched.get(&wall.id), Some(c) if *c != Change::Deleted);
        if attached.is_some_and(|t| mine || targets.contains(&t)) {
            out.extend(attach_problem(candidate, &wall));
        }
    }
    out.extend(label_warnings(candidate, touched));
    for storey in storeys {
        out.extend(room_findings(candidate, storey));
        if touched.iter().any(|(id, c)| {
            *c != Change::Deleted
                && type_of(candidate, *id).as_deref() == Some(WALL)
                && get(candidate, *id).and_then(|o| o.header.owner) == Some(storey)
        }) {
            out.extend(overlap_findings(candidate, storey));
        }
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
                        .with_arg("other", display_name(b))
                        .with_detail(OBJECT, a.id.to_string())
                        .with_detail(OTHER, b.id.to_string()),
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
            // The edges are matched to the vertices, so the boundary without the shaft openings.
            let Some(raw) = derived.floor.boundary_in(candidate) else {
                continue;
            };
            for wall in derived.floor.unsupported_edges(&raw, &wall_of) {
                out.push(
                    Error::new(codes::FLOOR_EDGE_UNSUPPORTED)
                        .with_arg("name", display_name(&floor))
                        .with_arg(
                            "wall",
                            get(candidate, wall)
                                .map(|w| display_name(&w))
                                .unwrap_or_default(),
                        )
                        .with_detail(OBJECT, floor.id.to_string())
                        .with_detail(OTHER, wall.to_string()),
                );
            }
        }
    }
    out.extend(ceiling_warnings(candidate, touched));
    out
}

/// The warnings about ceilings and vertical openings: an edge that follows a wall that no longer
/// bounds the ceiling there, and an opening that reaches outside the element that hosts it.
fn ceiling_warnings(candidate: &Snapshot, touched: &BTreeMap<ObjectId, Change>) -> Vec<Error> {
    let mut out = Vec::new();
    let wall_of =
        |id: ObjectId| get(candidate, id).and_then(|o| crate::wall::Wall::from_values(&o.values));
    for ceiling in objects(candidate, CEILING) {
        let storey_touched = ceiling.header.owner.is_some_and(|s| {
            touched.iter().any(|(id, c)| {
                *c != Change::Deleted
                    && get(candidate, *id).is_some_and(|o| o.header.owner == Some(s))
            })
        });
        if !(touched.contains_key(&ceiling.id) || storey_touched) {
            continue;
        }
        let Some(data) = crate::ceiling::Ceiling::from_values(&ceiling.values) else {
            continue;
        };
        let Some(raw) = data.shape.boundary_in(candidate) else {
            continue;
        };
        for wall in data.shape.unsupported_edges(&raw, &wall_of) {
            out.push(
                Error::new(codes::CEILING_EDGE_UNSUPPORTED)
                    .with_arg("name", display_name(&ceiling))
                    .with_arg(
                        "wall",
                        get(candidate, wall)
                            .map(|w| display_name(&w))
                            .unwrap_or_default(),
                    )
                    .with_detail(OBJECT, ceiling.id.to_string())
                    .with_detail(OTHER, wall.to_string()),
            );
        }
    }
    for opening in objects(candidate, VERTICAL_OPENING) {
        if !touched.contains_key(&opening.id) {
            continue;
        }
        let Some(host) = opening.header.owner.and_then(|h| get(candidate, h)) else {
            continue;
        };
        let (Some(cut), Some(area)) = (
            crate::opening::outline_of(candidate, &opening, crate::floor::VERTICAL_SLOTS),
            crate::opening::host_outline(candidate, &host),
        ) else {
            continue;
        };
        let area = crate::floor::Boundary::polygons(vec![area]);
        if cut.iter().any(|p| !area.contains(*p)) {
            out.push(
                Error::new(codes::OPENING_ABOVE_HOST)
                    .with_arg("name", display_name(&opening))
                    .with_arg("host", display_name(&host))
                    .with_detail(OBJECT, opening.id.to_string())
                    .with_detail(OTHER, host.id.to_string()),
            );
        }
    }
    out
}

/// The findings about the rooms of `storey`: unenclosed, redundant and duplicate numbers.
#[must_use]
pub fn room_findings(candidate: &Snapshot, storey: ObjectId) -> Vec<Error> {
    let entries = crate::room::of_storey(candidate, storey);
    let duplicates = crate::room::duplicate_numbers(&entries);
    let mut out = Vec::new();
    for e in &entries {
        let name = e.room.name.clone();
        let number = e.room.number.clone().unwrap_or_default();
        let code = match e.state() {
            crate::room::State::Unenclosed => Some(codes::ROOM_UNENCLOSED),
            crate::room::State::Redundant => Some(codes::ROOM_REDUNDANT),
            crate::room::State::Enclosed => None,
        };
        for code in code.into_iter().chain(
            duplicates
                .contains(&e.id)
                .then_some(codes::ROOM_NUMBER_DUPLICATE),
        ) {
            out.push(
                Error::new(code)
                    .with_arg("name", name.clone())
                    .with_arg("number", number.clone())
                    .with_detail(OBJECT, e.id.to_string()),
            );
        }
    }
    out
}

/// The fields of an element of type `key` that a pinned element may still change: its name, its
/// flag, the values copied from its type (a type edit reaches every instance) and, for a room, its
/// number and finishes.
fn pin_free(key: &str) -> &'static [usize] {
    use types::{floor, insert, room, wall};
    match key {
        WALL => &[
            wall::NAME,
            wall::THICKNESS,
            wall::CORE_EXTERIOR,
            wall::CORE_INTERIOR,
            wall::PINNED,
        ],
        DOOR | WINDOW => &[
            insert::NAME,
            insert::WIDTH,
            insert::HEIGHT,
            insert::FRAME_WIDTH,
            insert::FRAME_DEPTH,
            insert::OPERATION,
            insert::PINNED,
        ],
        FLOOR => &[floor::NAME, floor::THICKNESS, floor::PINNED],
        ROOM => &[
            room::NAME,
            room::NUMBER,
            room::DEPARTMENT,
            room::FLOOR_FINISH,
            room::WALL_FINISH,
            room::CEILING_FINISH,
            room::BASE_FINISH,
            room::PINNED,
        ],
        ROOF => &[
            types::roof::NAME,
            types::roof::THICKNESS,
            types::roof::PINNED,
        ],
        CEILING => &[
            types::ceiling::NAME,
            types::ceiling::THICKNESS,
            types::ceiling::PINNED,
        ],
        WALL_OPENING => &[types::wall_opening::NAME, types::wall_opening::PINNED],
        SHAFT => &[types::shaft::NAME, types::shaft::PINNED],
        types::GRID => &[types::grid::NAME, types::grid::PINNED],
        types::REFERENCE_PLANE => &[
            types::reference_plane::NAME,
            types::reference_plane::STRENGTH,
            types::reference_plane::PINNED,
        ],
        VERTICAL_OPENING => &[
            types::vertical_opening::NAME,
            types::vertical_opening::PINNED,
        ],
        _ => &[types::stair::NAME, types::stair::PINNED],
    }
}

/// The refusal of a change that moves, reshapes or deletes an element that was pinned in `base`:
/// every field but the ones of [`pin_free`] and the container must stay as they were. The error
/// names the element; `None` when no pinned element changed that way.
#[must_use]
pub fn pin_refusal(
    base: &Snapshot,
    candidate: &Snapshot,
    touched: &BTreeMap<ObjectId, Change>,
) -> Option<Error> {
    for (id, change) in touched {
        let (Some(key), Some(before)) = (type_of(base, *id), get(base, *id)) else {
            continue;
        };
        if !types::is_pinned(&key, &before.values) {
            continue;
        }
        let changed = match (change, get(candidate, *id)) {
            (Change::Deleted, _) | (_, None) => true,
            (_, Some(after)) => {
                let free = pin_free(&key);
                after.header.owner != before.header.owner
                    || (0..before.values.len().max(after.values.len()))
                        .any(|k| !free.contains(&k) && before.values.get(k) != after.values.get(k))
            }
        };
        if changed {
            return Some(Error::new(codes::PINNED).with_arg("name", display_name(&before)));
        }
    }
    None
}

/// What a warning of the register is about; the warnings panel filters by it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WarningKind {
    /// A stair: no layout, its stride, an overlap or an uncut opening.
    Stair,
    /// A room: not enclosed, redundant or a duplicate number.
    Room,
    /// Walls on top of each other or an open corner.
    Wall,
    /// A floor edge that follows a wall that no longer bounds it.
    Floor,
    /// A door or window that opens into another wall.
    Insert,
    /// An instance whose copies of its type's values differ from the type.
    Type,
    /// A ceiling edge that follows a wall that no longer bounds it, or an opening outside its host.
    Ceiling,
    /// A grid label used twice in a building.
    Datum,
}

impl WarningKind {
    /// Every kind, in the order the panel lists them.
    pub const ALL: [Self; 8] = [
        Self::Stair,
        Self::Room,
        Self::Wall,
        Self::Floor,
        Self::Ceiling,
        Self::Insert,
        Self::Type,
        Self::Datum,
    ];

    /// The kind of a finding, by its code.
    fn of(error: &Error) -> Self {
        let code = error.code();
        if code.starts_with("ab.model.stair") {
            Self::Stair
        } else if code.starts_with("ab.model.room") {
            Self::Room
        } else if code.starts_with("ab.model.floor") {
            Self::Floor
        } else if code.starts_with("ab.model.ceiling") || code.starts_with("ab.model.opening") {
            Self::Ceiling
        } else if code == codes::INSERT_MEETS_WALL.code {
            Self::Insert
        } else if code.starts_with("ab.model.datum") {
            Self::Datum
        } else if code.starts_with("ab.model.type") {
            Self::Type
        } else {
            Self::Wall
        }
    }
}

/// One entry of the warnings register: what is wrong, how serious it is and the elements involved.
#[derive(Debug)]
pub struct Warning {
    /// What it is about.
    pub kind: WarningKind,
    /// [`Severity::Error`] for an element the model cannot build (a stair without a layout),
    /// [`Severity::Warning`] for the rest.
    pub severity: Severity,
    /// The finding, with the arguments its message names.
    pub error: Error,
    /// The elements involved, the first one the finding is about.
    pub objects: Vec<ObjectId>,
}

/// The most entries the register lists.
pub const MAX_WARNINGS: usize = 2_000;

/// The warnings register of `snapshot`: every finding the commands report as a warning, found
/// over the whole model with the same rules ([`warnings`], [`stair_problem`], the checks of doors
/// and windows and of types), so a condition that no longer holds is no longer listed. Recomputed
/// from the document; nothing of it is stored.
#[must_use]
pub fn register(snapshot: &Snapshot) -> Vec<Warning> {
    let mut touched = BTreeMap::new();
    for key in types::ELEMENTS {
        for object in objects(snapshot, key) {
            touched.insert(object.id, Change::Modified);
        }
    }
    let ids = |error: &Error| -> Vec<ObjectId> {
        [OBJECT, OTHER]
            .iter()
            .filter_map(|k| match error.details().get(k) {
                Some(arc_base::error::Value::Text(t)) => t.parse().ok(),
                _ => None,
            })
            .collect()
    };
    let mut out: Vec<Warning> = Vec::new();
    let mut push = |severity, error: Error, objects: Vec<ObjectId>| {
        if out.len() < MAX_WARNINGS {
            out.push(Warning {
                kind: WarningKind::of(&error),
                severity,
                error,
                objects,
            });
        }
    };
    for stair in objects(snapshot, STAIR) {
        if let Some(error) = stair_problem(snapshot, &stair) {
            push(Severity::Error, error, vec![stair.id]);
        }
    }
    // A pair of stairs that overlap is one problem, though each stair reports it.
    let mut pairs = std::collections::BTreeSet::new();
    for error in warnings(snapshot, &touched) {
        let found = ids(&error);
        if error.code() == codes::STAIR_OVERLAP.code {
            let mut pair = found.clone();
            pair.sort();
            if !pairs.insert(pair) {
                continue;
            }
        }
        push(Severity::Warning, error, found);
    }
    for key in [DOOR, WINDOW, WALL_OPENING] {
        for object in objects(snapshot, key) {
            for error in crate::insert::problems(snapshot, &object) {
                if is_warning(&error) {
                    let mut found = vec![object.id];
                    found.extend(ids(&error).into_iter().filter(|id| *id != object.id));
                    push(Severity::Warning, error, found);
                }
            }
        }
    }
    for error in ceiling_range_findings(snapshot) {
        let found = ids(&error);
        push(Severity::Warning, error, found);
    }
    for (label, found) in crate::datum::duplicate_labels(snapshot) {
        let error = Error::new(codes::DATUM_LABEL_DUPLICATE)
            .with_arg("name", label)
            .with_arg("count", found.len().to_string());
        push(Severity::Warning, error, found);
    }
    for key in [WALL, FLOOR, DOOR, WINDOW, ROOF, CEILING] {
        for object in objects(snapshot, key) {
            if let Some(error) = type_problem(snapshot, key, &object) {
                push(Severity::Warning, error, vec![object.id]);
            }
        }
    }
    out
}

/// The ceilings whose underside or top leaves its range (below the level, or through the slab or
/// roof over it): warnings, listed until the ceiling or what is above it is changed.
#[must_use]
pub fn ceiling_range_findings(snapshot: &Snapshot) -> Vec<Error> {
    objects(snapshot, CEILING)
        .iter()
        .filter_map(|c| {
            let d = crate::ceiling::derived(snapshot, c)?;
            outside_range(snapshot, c, &d)
        })
        .collect()
}

/// The element a finding is about (the door that opens into a wall).
#[must_use]
pub fn object_of(error: &Error) -> Option<ObjectId> {
    match error.details().get(OBJECT) {
        Some(arc_base::error::Value::Text(t)) => t.parse().ok(),
        _ => None,
    }
}

/// The element a warning involves besides the one it is about (the wall a door opens into).
#[must_use]
pub fn other_object(error: &Error) -> Option<ObjectId> {
    match error.details().get(OTHER) {
        Some(arc_base::error::Value::Text(t)) => t.parse().ok(),
        _ => None,
    }
}

/// Whether `error` is a ceiling leaving its range (below its level, or through what is above it).
#[must_use]
pub fn is_ceiling_range(error: &Error) -> bool {
    error.code() == codes::CEILING_RANGE.code || error.code() == codes::CEILING_RANGE_ELEMENT.code
}

/// Whether `error` is a warning of the structure: it is reported, never refused.
#[must_use]
pub fn is_warning(error: &Error) -> bool {
    error.code() == codes::INSERT_MEETS_WALL.code || is_ceiling_range(error)
}

/// The commit validator of the structure: every violation of [`check`] is an error, except the
/// warnings ([`is_warning`]), which are findings of the lower severity.
#[must_use]
pub fn validator() -> DocumentValidator {
    Arc::new(|base, candidate, touched| {
        pin_refusal(base, candidate, touched)
            .into_iter()
            .chain(check(candidate, touched))
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
