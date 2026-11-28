//! `archbuilder.group.*`: model groups (AB-PHASE-010 to AB-PHASE-013).
//!
//! `create` makes a group definition from a selection closed under its dependencies: a wall takes
//! its doors, windows and wall openings, a hosted element takes its wall and the wall's openings.
//! The selection stays as it is. `place` puts an instance on a storey, mapping each level of the
//! definition to a storey of the same building (by elevation, unless `levels` names them). Each
//! instance regenerates its members under [`archbuilder_model::group::member_id`]; one call is one
//! transaction and one undo step. `edit` takes what an instance's generated elements now are and
//! writes it into the definition, then regenerates every instance. An element edited directly is
//! not overwritten silently: its fingerprint no longer matches the instance's checks, so a
//! regeneration that would overwrite it is refused. `separate` turns an instance into ordinary
//! elements with new identifiers and returns the pairs.

use std::collections::{BTreeMap, BTreeSet};

use arc_base::id::Identifier;
use arc_base::{Error, ObjectId, Result};
use arc_command::prompt::{Accept, PointDims, PromptSequence, PromptStep};
use arc_command::{CommandSpec, Context, IdParam, Registry};
use arc_doc::{FieldPath, Header, Snapshot, Value};
use archbuilder_model::group::{self, Definition, Instance, Member};
use archbuilder_model::spatial;
use archbuilder_model::types::{self, GROUP, GROUP_INSTANCE, WALL, component, room};
use archbuilder_model::wall::Wall;
use archbuilder_model::{defaults, names};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::codes;
use crate::structure::{create, ensure_valid, expect, language};

/// A message identifier, written as `message_id: "..."` so the catalog check finds the catalog
/// entry it names.
struct MessageId {
    message_id: &'static str,
}

/// The identifier of the create-group command.
pub const GROUP_CREATE: &str = "archbuilder.group.create";
/// The identifier of the place-group command.
pub const GROUP_PLACE: &str = "archbuilder.group.place";
/// The identifier of the edit-group command: the instance's changes go to the definition.
pub const GROUP_EDIT: &str = "archbuilder.group.edit";
/// The identifier of the command that excludes elements of an instance, or includes them again.
pub const GROUP_EXCLUDE: &str = "archbuilder.group.exclude";
/// The identifier of the command that sets or clears an exception of an instance element.
pub const GROUP_SET_EXCEPTION: &str = "archbuilder.group.set_exception";
/// The identifier of the command that separates an instance into ordinary elements.
pub const GROUP_SEPARATE: &str = "archbuilder.group.separate";
/// The identifier of the command that deletes an unused group definition.
pub const GROUP_DELETE: &str = "archbuilder.group.delete";

/// Parameters of `archbuilder.group.create`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateGroup {
    /// The name; else the first free `Group <n>`.
    #[serde(default)]
    #[schemars(length(max = 256))]
    pub name: Option<String>,
    /// The elements; else the selection.
    #[serde(default)]
    pub ids: Option<Vec<IdParam>>,
    /// The reference point in plan, in meters; else the first wall's start.
    #[serde(default)]
    pub origin: Option<[f64; 2]>,
}

/// What `archbuilder.group.create` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct GroupCreated {
    /// The definition.
    pub group: IdParam,
    /// The number of members it holds, hosted ones included.
    pub members: usize,
}

/// Parameters of `archbuilder.group.place`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceGroup {
    /// The definition: its identifier, or its name.
    #[schemars(length(max = 256))]
    pub group: String,
    /// The storey of the definition's first level; else the selection's, else the lowest.
    #[serde(default)]
    pub storey: Option<IdParam>,
    /// The storey of each level of the definition, in order. Else each level goes to the storey
    /// of the same building at the same distance above the first level.
    #[serde(default)]
    pub levels: Option<Vec<IdParam>>,
    /// The plan point of the definition's reference point, in meters.
    pub origin: [f64; 2],
    /// The rotation about the vertical axis, in radians.
    #[serde(default)]
    pub rotation: Option<f64>,
    /// Whether the definition is mirrored across its own vertical plane.
    #[serde(default)]
    pub mirror: bool,
    /// The mark; else the first free `Group <n>`.
    #[serde(default)]
    #[schemars(length(max = 256))]
    pub name: Option<String>,
}

/// What `archbuilder.group.place` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct GroupPlaced {
    /// The instance.
    pub instance: IdParam,
    /// The number of elements it generated.
    pub members: usize,
}

/// Parameters of `archbuilder.group.edit` and `archbuilder.group.separate`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InstanceParam {
    /// The instance; else the selected one.
    #[serde(default)]
    pub instance: Option<IdParam>,
}

/// Parameters of `archbuilder.group.exclude`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Exclude {
    /// The instance; else the selected one.
    #[serde(default)]
    pub instance: Option<IdParam>,
    /// Generated elements of the instance.
    pub elements: Vec<IdParam>,
    /// Whether to include them again instead of leaving them out.
    #[serde(default)]
    pub include: bool,
}

/// Parameters of `archbuilder.group.set_exception`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetException {
    /// The instance; else the selected one.
    #[serde(default)]
    pub instance: Option<IdParam>,
    /// The generated element.
    pub element: IdParam,
    /// `name`, `number` (rooms only) or `offset`.
    pub kind: String,
    /// The text of a `name` or `number` exception.
    #[serde(default)]
    #[schemars(length(max = 256))]
    pub text: Option<String>,
    /// The offset of an `offset` exception, in plan meters.
    #[serde(default)]
    pub offset: Option<[f64; 2]>,
    /// Clears the exception of that kind instead of setting it.
    #[serde(default)]
    pub clear: bool,
}

/// Parameters of `archbuilder.group.delete`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteGroup {
    /// The definition.
    pub group: IdParam,
}

/// What `archbuilder.group.separate` returns: each generated element and its ordinary copy.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Separated {
    /// The generated element and the new identifier of its copy, as `[generated, ordinary]`.
    pub pairs: Vec<[IdParam; 2]>,
}

fn instance_in(ctx: &Context<'_>, given: Option<IdParam>) -> Result<ObjectId> {
    if let Some(IdParam(id)) = given {
        return Ok(id);
    }
    ctx.selection()
        .iter()
        .copied()
        .find(|id| spatial::type_of(ctx.snapshot(), *id).as_deref() == Some(GROUP_INSTANCE))
        .ok_or_else(|| Error::new(codes::NOTHING_SELECTED))
}

fn load(snapshot: &Snapshot, instance: ObjectId) -> Result<(Instance, Definition)> {
    let object = expect(snapshot, instance, GROUP_INSTANCE)?;
    let inst = Instance::of(&object).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let def = group::definition(snapshot, inst.definition)
        .ok_or_else(|| Error::new(archbuilder_model::error::codes::GROUP_DEFINITION_MISSING))?;
    Ok((inst, def))
}

fn set(values: &mut [Option<Value>], at: usize, value: Value) {
    if let Some(slot) = values.get_mut(at) {
        *slot = Some(value);
    }
}

fn meters_of(values: &[Option<Value>], at: usize) -> f64 {
    types::meters(values, at).unwrap_or(0.0)
}

/// The position fields of a member of `kind`: the x and y indices, when it has a point.
fn point_fields(kind: &str) -> Option<(usize, usize)> {
    match kind {
        types::COMPONENT => Some((component::X, component::Y)),
        types::ROOM => Some((room::X, room::Y)),
        _ => None,
    }
}

/// The insertion of a generated element: its name, `mark name`, or the exception's text, and its
/// number and offset exceptions.
fn apply_exceptions(values: &mut [Option<Value>], member: &Member, inst: &Instance) {
    let base = types::text(&member.object.values, 0).unwrap_or_default();
    let name = match inst.text_of(member.id, "name") {
        Some(text) => text.to_owned(),
        None if inst.name.is_empty() || base.is_empty() => base.to_owned(),
        None => format!("{} {base}", inst.name),
    };
    set(values, 0, Value::String(name));
    if member.kind == types::ROOM
        && let Some(number) = inst.text_of(member.id, "number")
    {
        set(values, room::NUMBER, Value::String(number.to_owned()));
    }
    if let (Some((xi, yi)), Some(offset)) =
        (point_fields(member.kind), inst.offsets.get(&member.id))
    {
        set(values, xi, types::length(meters_of(values, xi) + offset[0]));
        set(values, yi, types::length(meters_of(values, yi) + offset[1]));
    }
}

/// Writes the fields of `new` that differ from `old`, by field name.
fn write_changed(
    ctx: &mut Context<'_>,
    id: ObjectId,
    key: &str,
    old: &[Option<Value>],
    new: &[Option<Value>],
) -> Result<()> {
    let names: Vec<String> = ctx
        .snapshot()
        .registry()
        .get(key)
        .map(|d| d.schema.fields.iter().map(|f| f.name.clone()).collect())
        .unwrap_or_default();
    for (at, (o, n)) in old.iter().zip(new).enumerate() {
        if o != n
            && let Some(name) = names.get(at)
        {
            ctx.txn()?
                .modify(id, FieldPath::new(name.as_str()), n.clone())?;
        }
    }
    Ok(())
}

/// Deletes the elements `ids` that exist; a hosted element (`true`) goes before its host.
fn delete_hosted_first(ctx: &mut Context<'_>, mut ids: Vec<(ObjectId, bool)>) -> Result<()> {
    ids.sort_by_key(|(_, hosted)| !hosted);
    for (id, _) in ids {
        if spatial::get(ctx.txn()?.snapshot(), id).is_some() {
            ctx.txn()?.delete(id)?;
        }
    }
    Ok(())
}

/// The generated elements of `instance`, as `(identifier, hosted)`.
fn generated(snapshot: &Snapshot, instance: ObjectId, def: ObjectId) -> Vec<(ObjectId, bool)> {
    group::members(snapshot, def)
        .iter()
        .map(|m| {
            (
                group::member_id(instance, m.id),
                group::is_insert_kind(m.kind),
            )
        })
        .collect()
}

/// Regenerates the members of `instance` from its definition: the generated elements are removed
/// and created again under their identifiers. `accept` names the members whose elements were
/// changed on purpose (by an edit); any other element changed since it was generated refuses it.
fn regenerate(
    ctx: &mut Context<'_>,
    instance: ObjectId,
    accept: &BTreeSet<ObjectId>,
) -> Result<usize> {
    let snap = ctx.txn()?.snapshot().clone();
    let (inst, def) = load(&snap, instance)?;
    if inst.level_map.len() != def.levels.len() {
        return Err(Error::new(
            archbuilder_model::error::codes::GROUP_LEVELS_UNMAPPED,
        ));
    }
    let members = group::members(&snap, def.id);
    for m in &members {
        let child = group::member_id(instance, m.id);
        if let (Some(object), Some(check)) = (spatial::get(&snap, child), inst.checks.get(&m.id))
            && !inst.excluded(m.id)
            && !accept.contains(&m.id)
            && &group::fingerprint(&object.values) != check
        {
            return Err(Error::new(codes::GROUP_EDITED).with_arg("name", inst.name.clone()));
        }
    }
    delete_hosted_first(ctx, generated(&snap, instance, def.id))?;
    let chain = group::placing(inst.origin, inst.rotation, inst.mirror);
    let mut created: BTreeSet<ObjectId> = BTreeSet::new();
    let mut checks: BTreeMap<String, Value> = BTreeMap::new();
    let mut ordered: Vec<&Member> = members.iter().collect();
    ordered.sort_by_key(|m| group::is_insert_kind(m.kind));
    for m in ordered {
        if inst.excluded(m.id) {
            continue;
        }
        let child = group::member_id(instance, m.id);
        let (owner, wall_length) = if group::is_insert_kind(m.kind) {
            let Some(host) = m.object.header.owner else {
                continue;
            };
            let host_child = group::member_id(instance, host);
            if !created.contains(&host_child) {
                continue;
            }
            let length = members
                .iter()
                .find(|h| h.id == host)
                .and_then(|h| Wall::from_values(&h.object.values))
                .map_or(0.0, |w| w.length());
            (host_child, length)
        } else {
            let level = m.level.ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
            let storey = inst.level_map.get(level).copied().ok_or_else(|| {
                Error::new(archbuilder_model::error::codes::GROUP_LEVELS_UNMAPPED)
            })?;
            (storey, 0.0)
        };
        let mut values =
            group::placed_values(m.kind, &m.object.values, &chain, inst.mirror, wall_length)
                .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        apply_exceptions(&mut values, m, &inst);
        ctx.txn()?.create(
            m.kind,
            child,
            Header {
                owner: Some(owner),
                ..Header::default()
            },
            values,
        )?;
        let stored = ctx.txn()?.get(child)?.values.clone();
        checks.insert(
            m.id.as_u128().to_string(),
            Value::String(group::fingerprint(&stored)),
        );
        created.insert(child);
    }
    ctx.txn()?
        .modify(instance, FieldPath::new("checks"), Some(Value::Map(checks)))?;
    Ok(created.len())
}

/// Takes the generated elements in `changed` (identifier and fingerprint before the change) as
/// written by their group: a change that reached the definition member as well (a type's values)
/// is no direct edit. An element whose fingerprint did not match before stays a direct edit.
pub(crate) fn accept_derived(
    ctx: &mut Context<'_>,
    changed: &BTreeMap<ObjectId, String>,
) -> Result<()> {
    if changed.is_empty() {
        return Ok(());
    }
    let snap = ctx.txn()?.snapshot().clone();
    for inst in group::instances(&snap) {
        let mut checks: BTreeMap<String, Value> = inst
            .checks
            .iter()
            .map(|(m, f)| (m.as_u128().to_string(), Value::String(f.clone())))
            .collect();
        let mut any = false;
        for (member, check) in &inst.checks {
            let child = group::member_id(inst.id, *member);
            if let (Some(before), Some(now)) = (changed.get(&child), spatial::get(&snap, child))
                && before == check
            {
                checks.insert(
                    member.as_u128().to_string(),
                    Value::String(group::fingerprint(&now.values)),
                );
                any = true;
            }
        }
        if any {
            ctx.txn()?
                .modify(inst.id, FieldPath::new("checks"), Some(Value::Map(checks)))?;
        }
    }
    Ok(())
}

/// Writes the changes of the generated elements of `instance` into its definition, and removes
/// the members whose elements were deleted. Returns the members taken as they are now.
fn capture(ctx: &mut Context<'_>, instance: ObjectId) -> Result<BTreeSet<ObjectId>> {
    let snap = ctx.txn()?.snapshot().clone();
    let (inst, def) = load(&snap, instance)?;
    let unchain = group::unplacing(inst.origin, inst.rotation, inst.mirror);
    let members = group::members(&snap, def.id);
    let mut accepted = BTreeSet::new();
    let mut gone: Vec<(ObjectId, bool)> = Vec::new();
    for m in members.iter().filter(|m| !inst.excluded(m.id)) {
        if spatial::get(&snap, group::member_id(instance, m.id)).is_none() {
            gone.push((m.id, group::is_insert_kind(m.kind)));
        }
    }
    delete_hosted_first(ctx, gone)?;
    let mut levels = BTreeMap::new();
    for (id, level) in &def.member_levels {
        levels.insert(
            id.as_u128().to_string(),
            Value::I64(i64::try_from(*level).unwrap_or(0)),
        );
    }
    let mut level_changed = false;
    for m in &members {
        if inst.excluded(m.id) {
            continue;
        }
        let child = group::member_id(instance, m.id);
        let Some(object) = spatial::get(&snap, child) else {
            levels.remove(&m.id.as_u128().to_string());
            continue;
        };
        accepted.insert(m.id);
        let mut source = object.values.clone();
        let mut wall_length = 0.0;
        if group::is_insert_kind(m.kind) {
            let host_child = object.header.owner.map(|h| group::member_id(instance, h));
            if let Some(host) = host_child.and_then(|h| spatial::get(&snap, h))
                && let Some(w) = Wall::from_values(&host.values)
            {
                wall_length = w.length();
            }
        } else if let Some(storey) = object.header.owner
            && let Some(level) = inst.level_map.iter().position(|s| *s == storey)
        {
            if m.level != Some(level) {
                level_changed = true;
            }
            levels.insert(
                m.id.as_u128().to_string(),
                Value::I64(i64::try_from(level).unwrap_or(0)),
            );
        }
        if let Some((xi, yi)) = point_fields(m.kind)
            && let Some(offset) = inst.offsets.get(&m.id)
        {
            let (x, y) = (
                meters_of(&source, xi) - offset[0],
                meters_of(&source, yi) - offset[1],
            );
            set(&mut source, xi, types::length(x));
            set(&mut source, yi, types::length(y));
        }
        let mut local = group::placed_values(m.kind, &source, &unchain, inst.mirror, wall_length)
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        // The names and numbers of the definition stay: exceptions belong to the instance.
        set_opt(&mut local, 0, m.object.values.first().cloned().flatten());
        if m.kind == types::ROOM {
            set_opt(
                &mut local,
                room::NUMBER,
                m.object.values.get(room::NUMBER).cloned().flatten(),
            );
        }
        write_changed(ctx, m.id, m.kind, &m.object.values, &local)?;
    }
    if level_changed || levels.len() != def.member_levels.len() {
        ctx.txn()?.modify(
            def.id,
            FieldPath::new("member_levels"),
            Some(Value::Map(levels)),
        )?;
    }
    Ok(accepted)
}

fn set_opt(values: &mut [Option<Value>], at: usize, value: Option<Value>) {
    if let Some(slot) = values.get_mut(at) {
        *slot = value;
    }
}

/// The instances of the definition `group`.
fn instances_of(snapshot: &Snapshot, group: ObjectId) -> Vec<ObjectId> {
    group::instances_of(snapshot, group)
        .iter()
        .map(|i| i.id)
        .collect()
}

/// The members of a definition that are referenced by an element of an instance: the element
/// identifiers of the instance map back to members.
fn member_of(snapshot: &Snapshot, instance: &Instance, element: ObjectId) -> Option<ObjectId> {
    group::members(snapshot, instance.definition)
        .iter()
        .find(|m| group::member_id(instance.id, m.id) == element)
        .map(|m| m.id)
}

/// The closure of a selection under dependencies: the walls, the doors, windows and wall openings,
/// the components and the rooms, all of them members of the new definition.
struct Closure {
    walls: BTreeSet<ObjectId>,
    inserts: BTreeSet<ObjectId>,
    components: BTreeSet<ObjectId>,
    rooms: BTreeSet<ObjectId>,
}

fn closure(snapshot: &Snapshot, ids: &[ObjectId]) -> Result<Closure> {
    let mut c = Closure {
        walls: BTreeSet::new(),
        inserts: BTreeSet::new(),
        components: BTreeSet::new(),
        rooms: BTreeSet::new(),
    };
    let take_wall = |c: &mut Closure, wall: ObjectId| -> Result<()> {
        if c.walls.insert(wall) {
            for kind in [types::DOOR, types::WINDOW, types::WALL_OPENING] {
                for item in spatial::children(snapshot, kind, wall) {
                    c.inserts.insert(item.id);
                }
            }
            if !spatial::children(snapshot, types::FAMILY_INSERT, wall).is_empty() {
                return Err(Error::new(codes::GROUP_KIND_UNSUPPORTED)
                    .with_arg("kind", types::FAMILY_INSERT));
            }
        }
        Ok(())
    };
    for id in ids {
        let kind = spatial::type_of(snapshot, *id).unwrap_or_default();
        match kind.as_str() {
            WALL => take_wall(&mut c, *id)?,
            types::DOOR | types::WINDOW | types::WALL_OPENING => {
                let host = spatial::get(snapshot, *id)
                    .and_then(|o| o.header.owner)
                    .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
                take_wall(&mut c, host)?;
            }
            types::COMPONENT => {
                c.components.insert(*id);
            }
            types::ROOM => {
                c.rooms.insert(*id);
            }
            other => {
                return Err(Error::new(codes::GROUP_KIND_UNSUPPORTED).with_arg("kind", other));
            }
        }
    }
    if c.walls.is_empty() && c.components.is_empty() && c.rooms.is_empty() {
        return Err(Error::new(codes::NOTHING_SELECTED));
    }
    Ok(c)
}

/// The level index as a stored value; a level that is none is stored as the first.
fn level_value(level: Option<usize>) -> Value {
    Value::I64(level.and_then(|l| i64::try_from(l).ok()).unwrap_or(0))
}

/// The storeys of a closure, the lowest first, and the elevation of each above the first. Refused
/// when the elements are in more than one building or the levels exceed the limit.
fn levels_of(snapshot: &Snapshot, c: &Closure) -> Result<(Vec<ObjectId>, Vec<f64>)> {
    let mut storeys: Vec<ObjectId> = Vec::new();
    let mut owners: BTreeSet<Option<ObjectId>> = BTreeSet::new();
    for id in c.walls.iter().chain(&c.components).chain(&c.rooms) {
        let storey = spatial::get(snapshot, *id).and_then(|o| o.header.owner);
        owners.insert(
            storey
                .and_then(|s| spatial::get(snapshot, s))
                .and_then(|s| s.header.owner),
        );
        if let Some(s) = storey
            && !storeys.contains(&s)
        {
            storeys.push(s);
        }
    }
    if owners.len() > 1 {
        return Err(Error::new(
            archbuilder_model::error::codes::GROUP_LEVELS_UNMAPPED,
        ));
    }
    let elevation = |s: &ObjectId| {
        spatial::get(snapshot, *s).map_or(0.0, |o| spatial::storey_elevation(snapshot, &o))
    };
    storeys.sort_by(|a, b| elevation(a).total_cmp(&elevation(b)).then(a.cmp(b)));
    let base = storeys.first().map_or(0.0, elevation);
    let levels: Vec<f64> = storeys.iter().map(|s| elevation(s) - base).collect();
    if levels.len() > group::MAX_LEVELS {
        return Err(Error::new(
            archbuilder_model::error::codes::GROUP_LEVELS_UNMAPPED,
        ));
    }
    Ok((storeys, levels))
}

/// The reference point of a new definition: the given one, else the first wall's start.
fn origin_of(snapshot: &Snapshot, c: &Closure, given: Option<[f64; 2]>) -> [f64; 2] {
    if let Some(o) = given {
        return o;
    }
    c.walls
        .iter()
        .next()
        .and_then(|w| spatial::get(snapshot, *w))
        .and_then(|w| Wall::from_values(&w.values))
        .map_or([0.0, 0.0], |w| w.start)
}

/// Copies the members of a closure into the definition `group_id`, with their plan points taken
/// from `origin`; returns the number of members. A copied wall whose name another wall of the
/// definition has is renumbered, so the names of a definition are unique.
fn copy_members(
    ctx: &mut Context<'_>,
    snapshot: &Snapshot,
    c: &Closure,
    group_id: ObjectId,
    storeys: &[ObjectId],
    origin: [f64; 2],
) -> Result<(usize, BTreeMap<String, Value>)> {
    let tongue = language(ctx);
    let unplace = group::unplacing(origin, 0.0, false);
    let level_of =
        |storey: Option<ObjectId>| storey.and_then(|s| storeys.iter().position(|x| *x == s));
    let mut levels_map: BTreeMap<String, Value> = BTreeMap::new();
    let mut used_wall_names: Vec<String> = Vec::new();
    let mut count = 0;
    for id in &c.walls {
        let object = expect(snapshot, *id, WALL)?;
        let wall =
            Wall::from_values(&object.values).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let mut values = group::placed_values(WALL, &object.values, &unplace, false, wall.length())
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let base = types::text(&object.values, 0)
            .unwrap_or_default()
            .to_owned();
        if used_wall_names.contains(&names::fold(&base)) {
            let fresh = (1..)
                .map(|n| defaults::wall(&tongue, n))
                .find(|n| !used_wall_names.contains(&names::fold(n)))
                .unwrap_or_default();
            set(&mut values, 0, Value::String(fresh));
        }
        used_wall_names.push(names::fold(types::text(&values, 0).unwrap_or_default()));
        let new = create(ctx, WALL, Some(group_id), values)?;
        levels_map.insert(
            new.as_u128().to_string(),
            level_value(level_of(object.header.owner)),
        );
        count += 1;
        for kind in [types::DOOR, types::WINDOW, types::WALL_OPENING] {
            for item in spatial::children(snapshot, kind, *id) {
                if !c.inserts.contains(&item.id) {
                    continue;
                }
                let values =
                    group::placed_values(kind, &item.values, &unplace, false, wall.length())
                        .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
                create(ctx, kind, Some(new), values)?;
                count += 1;
            }
        }
    }
    for (ids, key) in [(&c.components, types::COMPONENT), (&c.rooms, types::ROOM)] {
        for id in ids {
            let object = expect(snapshot, *id, key)?;
            let values = group::placed_values(key, &object.values, &unplace, false, 0.0)
                .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
            let new = create(ctx, key, Some(group_id), values)?;
            levels_map.insert(
                new.as_u128().to_string(),
                level_value(level_of(object.header.owner)),
            );
            count += 1;
        }
    }
    Ok((count, levels_map))
}

fn create_group(ctx: &mut Context<'_>, p: CreateGroup) -> Result<GroupCreated> {
    let snap = ctx.snapshot().clone();
    let ids: Vec<ObjectId> = match p.ids {
        Some(ids) => ids.into_iter().map(|i| i.0).collect(),
        None => ctx.selection().to_vec(),
    };
    let c = closure(&snap, &ids)?;
    let (storeys, levels) = levels_of(&snap, &c)?;
    let origin = origin_of(&snap, &c, p.origin);
    let project = spatial::project(&snap).ok_or_else(|| Error::new(codes::NO_BUILDING))?;
    let tongue = language(ctx);
    let name = match p
        .name
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty())
    {
        Some(name) => name,
        None => {
            let taken: Vec<String> = group::definitions(&snap)
                .into_iter()
                .map(|d| names::fold(&d.name))
                .collect();
            (1..)
                .map(|n| defaults::group(&tongue, n))
                .find(|n| !taken.contains(&names::fold(n)))
                .unwrap_or_default()
        }
    };
    let level_values: Vec<Value> = levels.iter().map(|l| types::length(*l)).collect();
    let group_id = create(
        ctx,
        GROUP,
        Some(project.id),
        vec![
            Some(Value::String(name)),
            Some(types::length(origin[0])),
            Some(types::length(origin[1])),
            Some(Value::List(level_values)),
            Some(Value::Map(BTreeMap::new())),
        ],
    )?;
    let (count, levels_map) = copy_members(ctx, &snap, &c, group_id, &storeys, origin)?;
    ctx.txn()?.modify(
        group_id,
        FieldPath::new("member_levels"),
        Some(Value::Map(levels_map)),
    )?;
    ensure_valid(ctx)?;
    Ok(GroupCreated {
        group: IdParam(group_id),
        members: count,
    })
}

/// The definition named by `text`: an identifier, else the definition of that name.
fn definition_named(snapshot: &Snapshot, text: &str) -> Result<Definition> {
    let text = text.trim();
    if let Ok(id) = text.parse::<ObjectId>() {
        return group::definition(snapshot, id)
            .ok_or_else(|| Error::new(archbuilder_model::error::codes::GROUP_DEFINITION_MISSING));
    }
    group::definitions(snapshot)
        .into_iter()
        .find(|d| names::same(&d.name, text))
        .ok_or_else(|| Error::new(archbuilder_model::error::codes::GROUP_DEFINITION_MISSING))
}

fn place_group(ctx: &mut Context<'_>, p: PlaceGroup) -> Result<GroupPlaced> {
    let snap = ctx.snapshot().clone();
    let def = definition_named(&snap, &p.group)?;
    let first = match p.storey {
        Some(IdParam(id)) => expect(&snap, id, types::STOREY)?,
        None => crate::wall::storey_for(ctx, None)?,
    };
    let building = first.header.owner;
    let level_map: Vec<ObjectId> = match p.levels {
        Some(list) => list.into_iter().map(|i| i.0).collect(),
        None => {
            let storeys = building
                .map(|b| spatial::storeys(&snap, b))
                .unwrap_or_default();
            let base = spatial::storey_elevation(&snap, &first);
            let mut found = Vec::new();
            for offset in &def.levels {
                let target = base + offset;
                let hit = storeys
                    .iter()
                    .find(|s| (s.elevation() - target).abs() < 1.0e-6)
                    .map(|s| s.storey.id);
                found.push(hit.ok_or_else(|| {
                    Error::new(archbuilder_model::error::codes::GROUP_LEVELS_UNMAPPED)
                })?);
            }
            found
        }
    };
    if level_map.len() != def.levels.len() {
        return Err(Error::new(
            archbuilder_model::error::codes::GROUP_LEVELS_UNMAPPED,
        ));
    }
    let storey = *level_map
        .first()
        .ok_or_else(|| Error::new(archbuilder_model::error::codes::GROUP_LEVELS_UNMAPPED))?;
    let tongue = language(ctx);
    let taken: Vec<String> = group::instances(&snap)
        .into_iter()
        .map(|i| names::fold(&i.name))
        .collect();
    let mark = match p
        .name
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty())
    {
        Some(name) => name,
        None => (1..)
            .map(|n| defaults::group(&tongue, n))
            .find(|n| !taken.contains(&names::fold(n)))
            .unwrap_or_default(),
    };
    let mut refs = Vec::new();
    for s in &level_map {
        refs.push(Value::Reference(*s));
    }
    let instance = create(
        ctx,
        GROUP_INSTANCE,
        Some(storey),
        vec![
            Some(Value::String(mark)),
            Some(Value::Reference(def.id)),
            Some(types::length(p.origin[0])),
            Some(types::length(p.origin[1])),
            Some(Value::Quantity(arc_base::Quantity::new(
                p.rotation.unwrap_or(0.0),
                arc_base::Dimension::ANGLE,
            ))),
            Some(Value::Bool(p.mirror)),
            Some(Value::List(refs)),
        ],
    )?;
    let members = regenerate(ctx, instance, &BTreeSet::new())?;
    ensure_valid(ctx)?;
    Ok(GroupPlaced {
        instance: IdParam(instance),
        members,
    })
}

fn edit_group(ctx: &mut Context<'_>, p: InstanceParam) -> Result<()> {
    let instance = instance_in(ctx, p.instance)?;
    let snap = ctx.snapshot().clone();
    let (inst, _) = load(&snap, instance)?;
    let accepted = capture(ctx, instance)?;
    for other in instances_of(ctx.txn()?.snapshot(), inst.definition) {
        let accept = if other == instance {
            accepted.clone()
        } else {
            BTreeSet::new()
        };
        regenerate(ctx, other, &accept)?;
    }
    ensure_valid(ctx)
}

fn exclude(ctx: &mut Context<'_>, p: Exclude) -> Result<()> {
    let instance = instance_in(ctx, p.instance)?;
    let snap = ctx.snapshot().clone();
    let (mut inst, _) = load(&snap, instance)?;
    let mut changed = BTreeSet::new();
    for element in &p.elements {
        let member = member_of(&snap, &inst, element.0)
            .ok_or_else(|| Error::new(archbuilder_model::error::codes::GROUP_MEMBER_UNKNOWN))?;
        changed.insert(member);
    }
    if p.include {
        inst.exclusions.retain(|m| !changed.contains(m));
    } else {
        for m in changed {
            if !inst.exclusions.contains(&m) {
                inst.exclusions.push(m);
            }
        }
    }
    // An empty list is written as absent, so an instance that excludes nothing stores the same.
    let refs: Vec<Value> = inst
        .exclusions
        .iter()
        .map(|m| Value::Reference(*m))
        .collect();
    let value = (!refs.is_empty()).then_some(Value::List(refs));
    ctx.txn()?
        .modify(instance, FieldPath::new("exclusions"), value)?;
    regenerate(ctx, instance, &BTreeSet::new())?;
    ensure_valid(ctx)
}

fn set_exception(ctx: &mut Context<'_>, p: SetException) -> Result<()> {
    let instance = instance_in(ctx, p.instance)?;
    let snap = ctx.snapshot().clone();
    let (inst, _) = load(&snap, instance)?;
    let member = member_of(&snap, &inst, p.element.0)
        .ok_or_else(|| Error::new(archbuilder_model::error::codes::GROUP_MEMBER_UNKNOWN))?;
    let mut text = inst.text.clone();
    let mut offsets = BTreeMap::new();
    for (k, v) in &inst.offsets {
        offsets.insert(*k, *v);
    }
    let kind = p.kind.as_str();
    match kind {
        "name" | "number" => {
            let member_kind = group::members(&snap, inst.definition)
                .iter()
                .find(|m| m.id == member)
                .map(|m| m.kind);
            if kind == "number" && member_kind != Some(types::ROOM) {
                return Err(Error::new(codes::VALUE_INVALID).with_arg("field", "number"));
            }
            let key = group::text_key(member, kind);
            match (p.clear, p.text) {
                (true, _) => {
                    text.remove(&key);
                }
                (false, Some(t)) => {
                    text.insert(key, t.trim().to_owned());
                }
                (false, None) => {
                    return Err(Error::new(codes::VALUE_INVALID).with_arg("field", kind));
                }
            }
        }
        "offset" => {
            if p.clear {
                offsets.remove(&member);
            } else {
                let o = p
                    .offset
                    .ok_or_else(|| Error::new(codes::VALUE_INVALID).with_arg("field", "offset"))?;
                offsets.insert(member, o);
            }
        }
        _ => return Err(Error::new(codes::VALUE_INVALID).with_arg("field", kind.to_owned())),
    }
    let mut text_map = BTreeMap::new();
    for (k, v) in text {
        text_map.insert(k, Value::String(v));
    }
    let mut offset_map = BTreeMap::new();
    for (m, o) in offsets {
        offset_map.insert(group::offset_key(m, "x"), types::length(o[0]));
        offset_map.insert(group::offset_key(m, "y"), types::length(o[1]));
    }
    ctx.txn()?.modify(
        instance,
        FieldPath::new("text_exceptions"),
        Some(Value::Map(text_map)),
    )?;
    ctx.txn()?.modify(
        instance,
        FieldPath::new("offset_exceptions"),
        Some(Value::Map(offset_map)),
    )?;
    regenerate(ctx, instance, &BTreeSet::new())?;
    ensure_valid(ctx)
}

/// Separates an instance: each generated element becomes an ordinary element with a new
/// identifier, hosted ones keep their host, and the instance goes.
pub(crate) fn separate_instance(
    ctx: &mut Context<'_>,
    instance: ObjectId,
) -> Result<Vec<[ObjectId; 2]>> {
    let snap = ctx.txn()?.snapshot().clone();
    let (inst, def) = load(&snap, instance)?;
    let mut members = group::members(&snap, def.id);
    members.sort_by_key(|m| group::is_insert_kind(m.kind));
    let mut pairs: Vec<[ObjectId; 2]> = Vec::new();
    let mut map: BTreeMap<ObjectId, ObjectId> = BTreeMap::new();
    for m in &members {
        if inst.excluded(m.id) {
            continue;
        }
        let child = group::member_id(instance, m.id);
        let Some(object) = spatial::get(&snap, child) else {
            continue;
        };
        let owner = if group::is_insert_kind(m.kind) {
            match object.header.owner.and_then(|h| map.get(&h).copied()) {
                Some(owner) => Some(owner),
                None => continue,
            }
        } else {
            object.header.owner
        };
        let new = create(ctx, m.kind, owner, object.values.clone())?;
        map.insert(child, new);
        pairs.push([child, new]);
    }
    delete_hosted_first(ctx, generated(&snap, instance, def.id))?;
    ctx.txn()?.delete(instance)?;
    Ok(pairs)
}

/// Deletes instances and the elements they generated, one transaction.
pub(crate) fn delete_instances(ctx: &mut Context<'_>, instances: &[ObjectId]) -> Result<()> {
    for instance in instances {
        let snap = ctx.txn()?.snapshot().clone();
        let (_, def) = load(&snap, *instance)?;
        delete_hosted_first(ctx, generated(&snap, *instance, def.id))?;
        ctx.txn()?.delete(*instance)?;
    }
    Ok(())
}

fn separate(ctx: &mut Context<'_>, p: InstanceParam) -> Result<Separated> {
    let instance = instance_in(ctx, p.instance)?;
    let pairs = separate_instance(ctx, instance)?;
    ensure_valid(ctx)?;
    Ok(Separated {
        pairs: pairs
            .into_iter()
            .map(|[a, b]| [IdParam(a), IdParam(b)])
            .collect(),
    })
}

fn delete_group(ctx: &mut Context<'_>, p: DeleteGroup) -> Result<()> {
    let snap = ctx.snapshot().clone();
    expect(&snap, p.group.0, GROUP)?;
    if !instances_of(&snap, p.group.0).is_empty() {
        return Err(Error::new(codes::GROUP_IN_USE));
    }
    delete_hosted_first(
        ctx,
        group::members(&snap, p.group.0)
            .iter()
            .map(|m| (m.id, group::is_insert_kind(m.kind)))
            .collect(),
    )?;
    ctx.txn()?.delete(p.group.0)?;
    ensure_valid(ctx)
}

/// Registers the group commands.
///
/// # Errors
/// `cmd.duplicate` when a group command is registered twice.
#[allow(clippy::too_many_lines, reason = "one registration after another")]
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(CommandSpec::new(
        GROUP_CREATE,
        MessageId {
            message_id: "ab-cmd-group-create-label",
        }
        .message_id,
        MessageId {
            message_id: "ab-cmd-group-create-description",
        }
        .message_id,
        create_group,
    ))?;
    registry.register(
        CommandSpec::new(
            GROUP_PLACE,
            MessageId {
                message_id: "ab-cmd-group-place-label",
            }
            .message_id,
            MessageId {
                message_id: "ab-cmd-group-place-description",
            }
            .message_id,
            place_group,
        )
        .with_prompts(
            PromptSequence::new()
                .step(PromptStep::new(
                    "group",
                    MessageId {
                        message_id: "ab-prompt-group",
                    }
                    .message_id,
                    Accept::Text {
                        min_chars: 1,
                        max_chars: Some(256),
                    },
                ))
                .step(PromptStep::new(
                    "origin",
                    MessageId {
                        message_id: "ab-prompt-group-origin",
                    }
                    .message_id,
                    Accept::Point {
                        dims: PointDims::Two,
                    },
                )),
        ),
    )?;
    registry.register(CommandSpec::new(
        GROUP_EDIT,
        MessageId {
            message_id: "ab-cmd-group-edit-label",
        }
        .message_id,
        MessageId {
            message_id: "ab-cmd-group-edit-description",
        }
        .message_id,
        edit_group,
    ))?;
    registry.register(CommandSpec::new(
        GROUP_EXCLUDE,
        MessageId {
            message_id: "ab-cmd-group-exclude-label",
        }
        .message_id,
        MessageId {
            message_id: "ab-cmd-group-exclude-description",
        }
        .message_id,
        exclude,
    ))?;
    registry.register(CommandSpec::new(
        GROUP_SET_EXCEPTION,
        MessageId {
            message_id: "ab-cmd-group-exception-label",
        }
        .message_id,
        MessageId {
            message_id: "ab-cmd-group-exception-description",
        }
        .message_id,
        set_exception,
    ))?;
    registry.register(CommandSpec::new(
        GROUP_SEPARATE,
        MessageId {
            message_id: "ab-cmd-group-separate-label",
        }
        .message_id,
        MessageId {
            message_id: "ab-cmd-group-separate-description",
        }
        .message_id,
        separate,
    ))?;
    registry.register(CommandSpec::new(
        GROUP_DELETE,
        MessageId {
            message_id: "ab-cmd-group-delete-label",
        }
        .message_id,
        MessageId {
            message_id: "ab-cmd-group-delete-description",
        }
        .message_id,
        delete_group,
    ))?;
    Ok(())
}
