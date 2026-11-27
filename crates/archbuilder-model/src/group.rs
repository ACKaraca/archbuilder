//! Model groups (AB-PHASE-010 to AB-PHASE-013): a repeated unit defined once and placed many times.
//!
//! A group definition (`archbuilder.group`) owns its member records: walls, doors, windows,
//! wall openings, family components and rooms, copied with new identifiers and kept in
//! definition-local coordinates (the plan point of the definition's reference point is the
//! origin, no rotation). The members are not part of the building: [`crate::spatial::objects`]
//! leaves them out, so every reader, view, schedule and export sees only what the instances
//! generate.
//!
//! A group instance (`archbuilder.group_instance`) places the definition: its reference point,
//! rotation and mirror; the storey each definition level maps to (so a repeated apartment adapts
//! to each storey's elevation); the members it leaves out; and its exceptions (room numbers and
//! names, furniture offsets, marks). The instance's members are generated elements of the storeys:
//! ordinary records whose identifiers are [`member_id`] of the instance and the definition member,
//! so they are the same after every regeneration. Generated elements are derived data. The
//! definition and the instance are the authority; the generated elements are regenerated from
//! them by the commands. [`Instance::checks`] keeps the fingerprint of each generated element as it
//! was last written, so a direct edit of a generated element is found before a regeneration
//! overwrites it.
//!
//! Groups share no machinery with families beyond the package format (ADR 0009, section 5).

use std::collections::BTreeMap;
use std::sync::Arc;

use arc_base::id::Identifier;
use arc_base::{Error, ObjectId};
use arc_doc::{Object, Snapshot, Value};

use crate::curtain::child_id;
use crate::error::codes;
use crate::family::loaded::Component;
use crate::insert::Hand;
use crate::spatial;
use crate::transform::Xform;
use crate::types::{self, GROUP, GROUP_INSTANCE, STOREY, group, group_instance};
use crate::wall::Wall;

/// The most members a definition or an instance holds.
pub const MAX_MEMBERS: usize = 512;
/// The most levels a definition spans.
pub const MAX_LEVELS: usize = 16;
/// The role of the generated members in [`crate::curtain::child_id`].
pub const MEMBER_ROLE: u32 = 0x4752_5550;
/// The element kinds a definition may hold. Hosted kinds (doors, windows and wall openings) are
/// members with their wall as owner.
pub const MEMBER_KINDS: [&str; 6] = [
    types::WALL,
    types::DOOR,
    types::WINDOW,
    types::WALL_OPENING,
    types::COMPONENT,
    types::ROOM,
];
/// The text exception kinds: the name of an element, and the number of a room.
pub const TEXT_KINDS: [&str; 2] = ["name", "number"];

/// Whether `key` is a kind a definition may hold.
#[must_use]
pub fn is_member_kind(key: &str) -> bool {
    MEMBER_KINDS.contains(&key)
}

/// Whether `key` is a hosted kind, owned by its wall.
#[must_use]
pub fn is_insert_kind(key: &str) -> bool {
    matches!(key, types::DOOR | types::WINDOW | types::WALL_OPENING)
}

/// The identifier of the element a definition member generates in `instance`: a function of the
/// two alone, so it is the same after every regeneration.
#[must_use]
pub fn member_id(instance: ObjectId, member: ObjectId) -> ObjectId {
    ObjectId::from_u128(child_id(
        instance,
        MEMBER_ROLE,
        &member.as_u128().to_string(),
    ))
}

/// The key of a text exception, `member/kind`.
#[must_use]
pub fn text_key(member: ObjectId, kind: &str) -> String {
    format!("{}/{kind}", member.as_u128())
}

/// The key of an offset exception on one axis, `member/x` or `member/y`.
#[must_use]
pub fn offset_key(member: ObjectId, axis: &str) -> String {
    format!("{}/{axis}", member.as_u128())
}

/// The fingerprint of the values of a generated element: a BLAKE3 hash of their text form.
#[must_use]
pub fn fingerprint(values: &[Option<Value>]) -> String {
    blake3::hash(format!("{values:?}").as_bytes())
        .to_hex()
        .to_string()
}

fn meters_at(values: &[Option<Value>], at: usize) -> f64 {
    types::meters(values, at).unwrap_or(0.0)
}

fn references(value: Option<&Option<Value>>) -> Vec<ObjectId> {
    match value {
        Some(Some(Value::List(items))) => items
            .iter()
            .filter_map(|v| match v {
                Value::Reference(id) => Some(*id),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// A group definition.
#[derive(Debug, Clone, PartialEq)]
pub struct Definition {
    /// The record.
    pub id: ObjectId,
    /// The name.
    pub name: String,
    /// The reference point, in project coordinates when the definition was made.
    pub origin: [f64; 2],
    /// The elevations of the levels, relative to the first, ascending.
    pub levels: Vec<f64>,
    /// The level index of each member, by the member's identifier.
    pub member_levels: BTreeMap<ObjectId, usize>,
}

impl Definition {
    /// The definition stored in `object`.
    #[must_use]
    pub fn of(object: &Object) -> Option<Self> {
        let v = &object.values;
        let levels = match v.get(group::LEVELS) {
            Some(Some(Value::List(items))) => items
                .iter()
                .filter_map(|q| match q {
                    Value::Quantity(q) => Some(q.value()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        let member_levels = match v.get(group::MEMBER_LEVELS) {
            Some(Some(Value::Map(map))) => map
                .iter()
                .filter_map(|(k, val)| {
                    let id = ObjectId::from_u128(k.parse::<u128>().ok()?);
                    match val {
                        Value::I64(i) => usize::try_from(*i).ok().map(|i| (id, i)),
                        _ => None,
                    }
                })
                .collect(),
            _ => BTreeMap::new(),
        };
        Some(Self {
            id: object.id,
            name: types::text(v, group::NAME).unwrap_or_default().to_owned(),
            origin: [meters_at(v, group::ORIGIN_X), meters_at(v, group::ORIGIN_Y)],
            levels,
            member_levels,
        })
    }
}

/// The definition `id`, when it is one.
#[must_use]
pub fn definition(snapshot: &Snapshot, id: ObjectId) -> Option<Definition> {
    spatial::get(snapshot, id).and_then(|o| {
        (spatial::type_of(snapshot, id).as_deref() == Some(GROUP))
            .then(|| Definition::of(&o))
            .flatten()
    })
}

/// Every group definition of the document, in identifier order.
#[must_use]
pub fn definitions(snapshot: &Snapshot) -> Vec<Definition> {
    spatial::objects_raw(snapshot, GROUP)
        .iter()
        .filter_map(|o| Definition::of(o))
        .collect()
}

/// A member of a definition: its record and, for an element on a level, the level's index.
#[derive(Debug, Clone)]
pub struct Member {
    /// The identifier of the member in the definition.
    pub id: ObjectId,
    /// The type key of the member.
    pub kind: &'static str,
    /// The record.
    pub object: Arc<Object>,
    /// The level index; the host's for a hosted member.
    pub level: Option<usize>,
}

/// The members of the definition `group`, in identifier order. A hosted member is listed when
/// its host is; a member whose host is missing is left out.
#[must_use]
pub fn members(snapshot: &Snapshot, group: ObjectId) -> Vec<Member> {
    let Some(def) = definition(snapshot, group) else {
        return Vec::new();
    };
    let mut out: Vec<Member> = Vec::new();
    for kind in MEMBER_KINDS {
        if is_insert_kind(kind) {
            continue;
        }
        for object in spatial::objects_raw(snapshot, kind) {
            if object.header.owner == Some(group) {
                let level = def.member_levels.get(&object.id).copied();
                out.push(Member {
                    id: object.id,
                    kind,
                    object,
                    level,
                });
            }
        }
    }
    let walls: BTreeMap<ObjectId, Option<usize>> = out.iter().map(|m| (m.id, m.level)).collect();
    for kind in [types::DOOR, types::WINDOW, types::WALL_OPENING] {
        for object in spatial::objects_raw(snapshot, kind) {
            if let Some(host) = object.header.owner
                && let Some(level) = walls.get(&host)
            {
                out.push(Member {
                    id: object.id,
                    kind,
                    object,
                    level: *level,
                });
            }
        }
    }
    out.sort_by_key(|m| m.id);
    out.truncate(MAX_MEMBERS);
    out
}

/// A placed instance of a definition.
#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    /// The record.
    pub id: ObjectId,
    /// The storey the instance is placed on, its first level's.
    pub storey: Option<ObjectId>,
    /// The mark.
    pub name: String,
    /// The definition.
    pub definition: ObjectId,
    /// The plan point of the definition's reference point.
    pub origin: [f64; 2],
    /// The rotation about the vertical axis, in radians.
    pub rotation: f64,
    /// Whether the definition is mirrored across its own vertical plane.
    pub mirror: bool,
    /// The storey of each definition level, by the level's index.
    pub level_map: Vec<ObjectId>,
    /// The definition members left out.
    pub exclusions: Vec<ObjectId>,
    /// The text exceptions, by [`text_key`].
    pub text: BTreeMap<String, String>,
    /// The offset exceptions by member, `[x, y]` in meters.
    pub offsets: BTreeMap<ObjectId, [f64; 2]>,
    /// The fingerprints of the generated elements, by member.
    pub checks: BTreeMap<ObjectId, String>,
}

impl Instance {
    /// The instance stored in `object`.
    #[must_use]
    pub fn of(object: &Object) -> Option<Self> {
        let v = &object.values;
        let Some(Some(Value::Reference(definition))) = v.get(group_instance::DEFINITION) else {
            return None;
        };
        let offsets = offsets_of(v.get(group_instance::OFFSET_EXCEPTIONS));
        Some(Self {
            id: object.id,
            storey: object.header.owner,
            name: types::text(v, group_instance::NAME)
                .unwrap_or_default()
                .to_owned(),
            definition: *definition,
            origin: [
                meters_at(v, group_instance::ORIGIN_X),
                meters_at(v, group_instance::ORIGIN_Y),
            ],
            rotation: match v.get(group_instance::ROTATION) {
                Some(Some(Value::Quantity(q))) => q.value(),
                _ => 0.0,
            },
            mirror: matches!(v.get(group_instance::MIRROR), Some(Some(Value::Bool(true)))),
            level_map: references(v.get(group_instance::LEVEL_MAP)),
            exclusions: references(v.get(group_instance::EXCLUSIONS)),
            text: match v.get(group_instance::TEXT_EXCEPTIONS) {
                Some(Some(Value::Map(map))) => map
                    .iter()
                    .filter_map(|(k, val)| match val {
                        Value::String(s) => Some((k.clone(), s.clone())),
                        _ => None,
                    })
                    .collect(),
                _ => BTreeMap::new(),
            },
            offsets,
            checks: match v.get(group_instance::CHECKS) {
                Some(Some(Value::Map(map))) => map
                    .iter()
                    .filter_map(|(k, val)| match (k.parse::<u128>(), val) {
                        (Ok(id), Value::String(s)) => Some((ObjectId::from_u128(id), s.clone())),
                        _ => None,
                    })
                    .collect(),
                _ => BTreeMap::new(),
            },
        })
    }

    /// The text exception of `member` of `kind`, when one is set.
    #[must_use]
    pub fn text_of(&self, member: ObjectId, kind: &str) -> Option<&str> {
        self.text.get(&text_key(member, kind)).map(String::as_str)
    }

    /// The members the instance leaves out, as a set.
    #[must_use]
    pub fn excluded(&self, member: ObjectId) -> bool {
        self.exclusions.contains(&member)
    }
}

/// The offset exceptions of a map of `member/x` and `member/y` entries.
fn offsets_of(value: Option<&Option<Value>>) -> BTreeMap<ObjectId, [f64; 2]> {
    let mut out: BTreeMap<ObjectId, [f64; 2]> = BTreeMap::new();
    let Some(Some(Value::Map(map))) = value else {
        return out;
    };
    for (key, val) in map {
        let Value::Quantity(q) = val else {
            continue;
        };
        let Some((member, axis)) = key.split_once('/') else {
            continue;
        };
        let Ok(raw) = member.parse::<u128>() else {
            continue;
        };
        let slot = out.entry(ObjectId::from_u128(raw)).or_insert([0.0, 0.0]);
        match axis {
            "x" => slot[0] = q.value(),
            "y" => slot[1] = q.value(),
            _ => {}
        }
    }
    out
}

/// Every group instance of the document, in identifier order.
#[must_use]
pub fn instances(snapshot: &Snapshot) -> Vec<Instance> {
    spatial::objects_raw(snapshot, GROUP_INSTANCE)
        .iter()
        .filter_map(|o| Instance::of(o))
        .collect()
}

/// The instances of the definition `group`.
#[must_use]
pub fn instances_of(snapshot: &Snapshot, group: ObjectId) -> Vec<Instance> {
    instances(snapshot)
        .into_iter()
        .filter(|i| i.definition == group)
        .collect()
}

/// The transform from definition-local coordinates to the plan: the mirror across the
/// definition's own vertical plane first, then the turn and the move to the instance's point.
#[must_use]
pub fn placing(origin: [f64; 2], rotation: f64, mirror: bool) -> Vec<Xform> {
    let mut chain = Vec::with_capacity(3);
    if mirror {
        chain.push(Xform::Mirror {
            origin: [0.0, 0.0],
            dir: [0.0, 1.0],
        });
    }
    chain.push(Xform::Rotate {
        base: [0.0, 0.0],
        angle: rotation,
    });
    chain.push(Xform::Translate(origin));
    chain
}

/// The inverse of [`placing`]: from the plan back to definition-local coordinates.
#[must_use]
pub fn unplacing(origin: [f64; 2], rotation: f64, mirror: bool) -> Vec<Xform> {
    let mut chain = vec![
        Xform::Translate([-origin[0], -origin[1]]),
        Xform::Rotate {
            base: [0.0, 0.0],
            angle: -rotation,
        },
    ];
    if mirror {
        chain.push(Xform::Mirror {
            origin: [0.0, 0.0],
            dir: [0.0, 1.0],
        });
    }
    chain
}

/// The image of `p` under the motions of `chain`, applied in order.
#[must_use]
pub fn through(chain: &[Xform], p: [f64; 2]) -> [f64; 2] {
    chain.iter().fold(p, |q, xf| xf.point(q))
}

/// The values of a member under `chain`, which maps definition-local coordinates to the plan.
/// `wall_length` is the length of the member's host wall, for a hosted member; `mirrored` says
/// whether the chain reflects. `None` when the values do not hold a member of `kind`.
#[must_use]
pub fn placed_values(
    kind: &str,
    values: &[Option<Value>],
    chain: &[Xform],
    mirrored: bool,
    wall_length: f64,
) -> Option<Vec<Option<Value>>> {
    let mut out = values.to_vec();
    let mut put = |at: usize, value: Value| {
        if let Some(slot) = out.get_mut(at) {
            *slot = Some(value);
        }
    };
    match kind {
        types::WALL => {
            let wall = Wall::from_values(values)?;
            let (a, b) = (through(chain, wall.start), through(chain, wall.end));
            let (start, end, start_join, end_join) = if mirrored {
                (b, a, wall.end_join, wall.start_join)
            } else {
                (a, b, wall.start_join, wall.end_join)
            };
            for (at, v) in [
                (types::wall::START_X, start[0]),
                (types::wall::START_Y, start[1]),
                (types::wall::END_X, end[0]),
                (types::wall::END_Y, end[1]),
            ] {
                put(at, types::length(v));
            }
            put(types::wall::START_JOIN, Value::Enum(start_join.code()));
            put(types::wall::END_JOIN, Value::Enum(end_join.code()));
        }
        types::DOOR | types::WINDOW | types::WALL_OPENING => {
            let at = types::insert::DISTANCE;
            let distance = types::meters(values, at)?;
            let distance = if mirrored {
                (wall_length - distance).max(0.0)
            } else {
                distance
            };
            put(at, types::length(distance));
            if kind != types::WALL_OPENING {
                let hand = match values.get(types::insert::HAND) {
                    Some(Some(Value::Enum(code))) => Hand::from_code(*code),
                    _ => Hand::default(),
                };
                let hand = if mirrored { hand.flipped() } else { hand };
                put(types::insert::HAND, Value::Enum(hand.code()));
            }
        }
        types::COMPONENT => {
            let mut comp = Component {
                id: ObjectId::from_u128(0),
                storey: None,
                name: String::new(),
                family_type: ObjectId::from_u128(0),
                offset: 0.0,
                origin: [
                    meters_at(values, types::component::X),
                    meters_at(values, types::component::Y),
                ],
                rotation: match values.get(types::component::ROTATION) {
                    Some(Some(Value::Quantity(q))) => q.value(),
                    _ => 0.0,
                },
                mirror: matches!(
                    values.get(types::component::MIRROR),
                    Some(Some(Value::Bool(true)))
                ),
                values: BTreeMap::new(),
            };
            for xf in chain {
                comp = comp.moved(xf);
            }
            put(types::component::X, types::length(comp.origin[0]));
            put(types::component::Y, types::length(comp.origin[1]));
            put(
                types::component::ROTATION,
                Value::Quantity(arc_base::Quantity::new(
                    comp.rotation,
                    arc_base::Dimension::ANGLE,
                )),
            );
            put(types::component::MIRROR, Value::Bool(comp.mirror));
        }
        types::ROOM => {
            let p = through(
                chain,
                [
                    meters_at(values, types::room::X),
                    meters_at(values, types::room::Y),
                ],
            );
            put(types::room::X, types::length(p[0]));
            put(types::room::Y, types::length(p[1]));
        }
        _ => return None,
    }
    Some(out)
}

/// Problems with a group instance: its definition must exist, its level mapping must name one
/// storey of the same building for each level of the definition, and each member it excludes or
/// takes an exception for must be a member of that definition.
#[must_use]
pub fn instance_problems(snapshot: &Snapshot, object: &Object) -> Vec<Error> {
    let mut out = Vec::new();
    let Some(instance) = Instance::of(object) else {
        return vec![Error::new(codes::GROUP_DEFINITION_MISSING)];
    };
    let Some(def) = definition(snapshot, instance.definition) else {
        out.push(Error::new(codes::GROUP_DEFINITION_MISSING));
        return out;
    };
    let mapped: Vec<Option<Arc<Object>>> = instance
        .level_map
        .iter()
        .map(|s| spatial::get(snapshot, *s))
        .collect();
    let building = instance
        .storey
        .and_then(|s| spatial::get(snapshot, s))
        .and_then(|s| s.header.owner);
    let same_building = mapped.iter().all(|s| {
        s.as_ref().is_some_and(|s| {
            spatial::type_of(snapshot, s.id).as_deref() == Some(STOREY)
                && s.header.owner == building
        })
    });
    if mapped.len() != def.levels.len() || !same_building {
        out.push(Error::new(codes::GROUP_LEVELS_UNMAPPED));
    }
    let known: Vec<ObjectId> = members(snapshot, def.id).iter().map(|m| m.id).collect();
    if instance.exclusions.iter().any(|m| !known.contains(m)) {
        out.push(Error::new(codes::GROUP_MEMBER_UNKNOWN));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placing_and_unplacing_are_inverse() {
        let at = [3.0, -2.0];
        let chain = placing(at, 0.7, true);
        let back = unplacing(at, 0.7, true);
        for p in [[1.0, 2.0], [-4.5, 0.25]] {
            let q = through(&chain, p);
            let r = through(&back, q);
            assert!((r[0] - p[0]).abs() < 1e-12 && (r[1] - p[1]).abs() < 1e-12);
        }
    }

    #[test]
    fn a_mirrored_placement_reverses_a_hosted_distance() {
        let values = vec![None; 12];
        let mut values = values;
        values[types::insert::DISTANCE] = Some(types::length(1.0));
        values[types::insert::HAND] = Some(Value::Enum(Hand::Left.code()));
        let chain = placing([0.0, 0.0], 0.0, true);
        let placed = placed_values(types::DOOR, &values, &chain, true, 6.0);
        let Some(placed) = placed else {
            panic!("a door is placed");
        };
        assert_eq!(types::meters(&placed, types::insert::DISTANCE), Some(5.0));
        assert_eq!(
            placed[types::insert::HAND],
            Some(Value::Enum(Hand::Left.flipped().code()))
        );
    }

    #[test]
    fn member_identifiers_depend_on_instance_and_member_alone() {
        let a = ObjectId::from_u128(41);
        let b = ObjectId::from_u128(42);
        let m = ObjectId::from_u128(7);
        assert_eq!(member_id(a, m), member_id(a, m));
        assert_ne!(member_id(a, m), member_id(b, m));
        assert_ne!(member_id(a, m), member_id(a, ObjectId::from_u128(8)));
    }
}
