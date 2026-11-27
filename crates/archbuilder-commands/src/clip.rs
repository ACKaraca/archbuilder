//! Clipboard content: a copy of the elements of a selection (every kind `archbuilder.modify.copy`
//! copies) that carries their field values, so it can be pasted after the originals are deleted and into another document of the
//! same product. [`capture`] reads the objects of a snapshot into JSON; the paste commands
//! (`archbuilder.clipboard.paste`, `paste_to_storey`, with the `content` parameter) create new
//! objects from it in a storey, translated by a displacement.
//!
//! What is kept: every field value except references to objects outside the content. A wall's top
//! constraint is replaced by its effective height (the top becomes unconnected); the edges of a
//! floor follow the pasted walls they followed; a door or window needs its wall in the content, a
//! floor edge or vertical opening its host, a hosted railing its stair or ramp. Grids and reference
//! planes go to the building of the storey pasted into, a grid with the label after its
//! original's; a curtain wall keeps its grid edits and panel overrides (they are keyed per wall).
//! A type, level or host that the document does not have is dropped (the default type, the
//! storey's own levels).

use std::collections::BTreeMap;

use arc_base::{Error, ObjectId, Result};
use arc_command::Context;
use arc_doc::{Snapshot, Value};
use archbuilder_model::floor::Floor;
use archbuilder_model::insert::{self, Kind};
use archbuilder_model::spatial;
use archbuilder_model::transform::Xform;
use archbuilder_model::types::{
    self, CEILING, DOOR, FLOOR, ROOF, ROOM, SHAFT, STAIR, VERTICAL_OPENING, WALL, WALL_OPENING,
    WINDOW,
};
use archbuilder_model::wall::Wall;
use serde_json::{Value as Json, json};

use crate::error::codes;
use crate::modify::{Modified, ensure_valid_lenient, floor_vertices, moved_wall};
use crate::structure::{create, language};

/// The most items a clipboard holds.
pub const MAX_ITEMS: usize = 20_000;

fn encode(value: &Option<Value>) -> Json {
    let Some(value) = value else {
        return Json::Null;
    };
    match value {
        Value::Bool(b) => json!({ "b": b }),
        Value::I64(i) => json!({ "i": i }),
        Value::F64(f) => json!({ "f": f }),
        Value::String(s) => json!({ "s": s }),
        Value::Quantity(q) => json!({ "q": serde_json::to_value(q).unwrap_or(Json::Null) }),
        Value::Enum(e) => json!({ "e": e }),
        Value::Reference(id) => json!({ "r": id.to_string() }),
        Value::List(items) => {
            json!({ "l": items.iter().map(|v| encode(&Some(v.clone()))).collect::<Vec<_>>() })
        }
        Value::Record(fields) => json!({ "c": fields.iter().map(encode).collect::<Vec<_>>() }),
        _ => Json::Null,
    }
}

fn decode(json: &Json) -> Option<Value> {
    let o = json.as_object()?;
    let (tag, v) = o.iter().next()?;
    Some(match tag.as_str() {
        "b" => Value::Bool(v.as_bool()?),
        "i" => Value::I64(v.as_i64()?),
        "f" => Value::F64(v.as_f64()?),
        "s" => Value::String(v.as_str()?.to_owned()),
        "q" => Value::Quantity(serde_json::from_value(v.clone()).ok()?),
        "e" => Value::Enum(u32::try_from(v.as_u64()?).ok()?),
        "r" => Value::Reference(v.as_str()?.parse().ok()?),
        "l" => Value::List(v.as_array()?.iter().filter_map(decode).collect()),
        "c" => Value::Record(v.as_array()?.iter().map(decode).collect()),
        _ => return None,
    })
}

/// The ids to copy: the selection, with the generated elements of each group instance in it. A
/// group instance is copied as its elements, which are ordinary once copied; its own record is
/// kept in the content for identity and is not pasted as an element.
fn with_group_elements(snapshot: &Snapshot, selected: &[ObjectId]) -> Vec<ObjectId> {
    let mut ids: Vec<ObjectId> = Vec::new();
    for id in selected {
        ids.push(*id);
        let instance = spatial::get(snapshot, *id)
            .filter(|_| spatial::type_of(snapshot, *id).as_deref() == Some(types::GROUP_INSTANCE))
            .and_then(|o| archbuilder_model::group::Instance::of(&o));
        let Some(instance) = instance else {
            continue;
        };
        for m in archbuilder_model::group::members(snapshot, instance.definition) {
            let child = archbuilder_model::group::member_id(instance.id, m.id);
            if !instance.excluded(m.id) && spatial::get(snapshot, child).is_some() {
                ids.push(child);
            }
        }
    }
    ids
}

/// The content of the clipboard for `ids`: the walls, floors and rooms among them, the doors and
/// windows their walls host, and the doors and windows of the selection whose wall is also in it.
///
/// # Errors
/// `ab.cmd.nothing_selected` when none of `ids` can be copied.
#[allow(clippy::too_many_lines, reason = "one arm for each kind captured")]
pub fn capture(snapshot: &Snapshot, selected: &[ObjectId]) -> Result<Json> {
    let ids = with_group_elements(snapshot, selected);
    let mut items: Vec<Json> = Vec::new();
    let mut walls: Vec<ObjectId> = Vec::new();
    let mut seen: Vec<ObjectId> = Vec::new();
    let mut push = |items: &mut Vec<Json>,
                    id: ObjectId,
                    host: Option<ObjectId>,
                    values: Json,
                    key: &str| {
        if !seen.contains(&id) && items.len() < MAX_ITEMS {
            seen.push(id);
            items.push(json!({
                "key": key, "id": id.to_string(), "host": host.map(|h| h.to_string()), "values": values
            }));
        }
    };
    for id in ids.iter().take(MAX_ITEMS) {
        let Some(object) = spatial::get(snapshot, *id) else {
            continue;
        };
        let key = spatial::type_of(snapshot, *id).unwrap_or_default();
        match key.as_str() {
            WALL => {
                walls.push(*id);
                let mut values = object.values.clone();
                if let Some(w) = Wall::of(snapshot, &object)
                    && let Some(slot) = values.get_mut(types::wall::HEIGHT)
                {
                    *slot = Some(types::length(w.height));
                }
                if let Some(slot) = values.get_mut(types::wall::TOP_LEVEL) {
                    *slot = None;
                }
                push(
                    &mut items,
                    *id,
                    None,
                    Json::Array(values.iter().map(encode).collect()),
                    WALL,
                );
                // A curtain wall's grid and panels are its type's: the type goes with it.
                if archbuilder_model::curtain::is_curtain(snapshot, &object)
                    && let Some(ty) = archbuilder_model::element_type::type_of(
                        snapshot,
                        archbuilder_model::element_type::TypeKind::Wall,
                        &object.values,
                    )
                    && let Some(item) = items.last_mut().and_then(Json::as_object_mut)
                    && item.get("id").and_then(Json::as_str) == Some(id.to_string().as_str())
                {
                    item.insert(
                        "type".into(),
                        json!({
                            "id": ty.id.to_string(),
                            "values": ty.values.iter().map(encode).collect::<Vec<_>>(),
                        }),
                    );
                }
            }
            STAIR => {
                // The top follows the level above the storey the stair is pasted into.
                let mut values = object.values.clone();
                if let Some(slot) = values.get_mut(types::stair::TOP_LEVEL) {
                    *slot = None;
                }
                push(
                    &mut items,
                    *id,
                    None,
                    Json::Array(values.iter().map(encode).collect()),
                    STAIR,
                );
            }
            types::RAILING
                if archbuilder_model::railing::Railing::from_values(&object.values)
                    .is_some_and(|r| r.host.is_none()) =>
            {
                push(
                    &mut items,
                    *id,
                    None,
                    Json::Array(object.values.iter().map(encode).collect()),
                    &key,
                );
            }
            FLOOR
            | ROOM
            | ROOF
            | CEILING
            | SHAFT
            | types::RAMP
            | types::GRID
            | types::REFERENCE_PLANE
            | types::ELEVATOR
            | types::COMPONENT
            | types::GROUP_INSTANCE
            | types::ROOF_EXTRUSION => push(
                &mut items,
                *id,
                None,
                Json::Array(object.values.iter().map(encode).collect()),
                &key,
            ),
            _ => {}
        }
    }
    for wall in &walls {
        for (host, _) in insert::hosted(snapshot, *wall) {
            let key = spatial::type_of(snapshot, host.id).unwrap_or_default();
            push(
                &mut items,
                host.id,
                Some(*wall),
                Json::Array(host.values.iter().map(encode).collect()),
                &key,
            );
        }
    }
    // The vertical openings of the floors, roofs and ceilings copied go with them.
    for id in ids.iter().take(MAX_ITEMS) {
        let key = spatial::type_of(snapshot, *id).unwrap_or_default();
        if key == FLOOR {
            for edge in spatial::children(snapshot, types::FLOOR_EDGE, *id) {
                push(
                    &mut items,
                    edge.id,
                    Some(*id),
                    Json::Array(edge.values.iter().map(encode).collect()),
                    types::FLOOR_EDGE,
                );
            }
        }
        if key == ROOF {
            for key in [types::DORMER, types::ROOF_EDGE] {
                for part in spatial::children(snapshot, key, *id) {
                    push(
                        &mut items,
                        part.id,
                        Some(*id),
                        Json::Array(part.values.iter().map(encode).collect()),
                        key,
                    );
                }
            }
        }
        // The railings hosted on a stair or ramp go with it.
        if key == STAIR || key == types::RAMP {
            for railing in spatial::objects(snapshot, types::RAILING) {
                let hosted = archbuilder_model::railing::Railing::from_values(&railing.values)
                    .is_some_and(|r| r.host == Some(*id));
                if hosted {
                    push(
                        &mut items,
                        railing.id,
                        Some(*id),
                        Json::Array(railing.values.iter().map(encode).collect()),
                        types::RAILING,
                    );
                }
            }
        }
        if types::VERTICAL_HOSTS.contains(&key.as_str()) {
            for cut in spatial::children(snapshot, VERTICAL_OPENING, *id) {
                push(
                    &mut items,
                    cut.id,
                    Some(*id),
                    Json::Array(cut.values.iter().map(encode).collect()),
                    VERTICAL_OPENING,
                );
            }
        }
    }
    for id in ids.iter().take(MAX_ITEMS) {
        let key = spatial::type_of(snapshot, *id).unwrap_or_default();
        if let (true, Some(object)) = (
            matches!(
                key.as_str(),
                DOOR | WINDOW | WALL_OPENING | types::FAMILY_INSERT
            ),
            spatial::get(snapshot, *id),
        ) && let Some(host) = object.header.owner.filter(|h| walls.contains(h))
        {
            push(
                &mut items,
                *id,
                Some(host),
                Json::Array(object.values.iter().map(encode).collect()),
                &key,
            );
        }
    }
    if items.is_empty() {
        return Err(Error::new(codes::NOTHING_SELECTED));
    }
    Ok(json!({ "format": "archbuilder-clip", "version": 1, "items": items }))
}

fn invalid() -> Error {
    Error::new(arc_command::error::codes::PARAMS_INVALID)
}

/// A content item: kind, original identifier, original host wall and field values.
type Decoded = (String, ObjectId, Option<ObjectId>, Vec<Option<Value>>);

#[allow(
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    reason = "one pass for each kind: walls, inserts, floors, ceilings, rooms, roofs, stairs, cuts"
)]
/// Creates the content in `storey`, translated by `displacement`, and selects it after the command.
pub(crate) fn paste_content(
    ctx: &mut Context<'_>,
    content: &Json,
    displacement: [f64; 2],
    storey: ObjectId,
) -> Result<Modified> {
    if content.get("format").and_then(Json::as_str) != Some("archbuilder-clip") {
        return Err(invalid());
    }
    let items = content
        .get("items")
        .and_then(Json::as_array)
        .filter(|i| i.len() <= MAX_ITEMS)
        .ok_or_else(invalid)?;
    let xf = Xform::Translate(displacement);
    if !xf.is_finite() {
        return Err(invalid());
    }
    let tongue = language(ctx);
    let parts = |item: &Json| -> Option<Decoded> {
        let key = item.get("key")?.as_str()?.to_owned();
        let id: ObjectId = item.get("id")?.as_str()?.parse().ok()?;
        let host = item
            .get("host")
            .and_then(Json::as_str)
            .and_then(|h| h.parse().ok());
        let values = item.get("values")?.as_array()?.iter().map(decode).collect();
        Some((key, id, host, values))
    };
    let decoded: Vec<_> = items.iter().filter_map(parts).collect();
    // The types that curtain walls carry, by wall.
    let wall_types: BTreeMap<ObjectId, (ObjectId, Vec<Option<Value>>)> = items
        .iter()
        .filter_map(|item| {
            let wall: ObjectId = item.get("id")?.as_str()?.parse().ok()?;
            let ty = item.get("type")?;
            let id: ObjectId = ty.get("id")?.as_str()?.parse().ok()?;
            let values = ty.get("values")?.as_array()?.iter().map(decode).collect();
            Some((wall, (id, values)))
        })
        .collect();
    let mut map: BTreeMap<ObjectId, ObjectId> = BTreeMap::new();
    let mut created: Vec<ObjectId> = Vec::new();
    for (key, old, _, mut values) in decoded.iter().cloned() {
        if key != WALL {
            continue;
        }
        let wall = Wall::from_values(&values).ok_or_else(invalid)?;
        let moved = moved_wall(&wall, &xf);
        let seen = ctx.txn()?.snapshot().clone();
        let name = crate::wall::next_wall_name(&seen, storey, &tongue);
        let mut put = |at: usize, v: Value| {
            if let Some(slot) = values.get_mut(at) {
                *slot = Some(v);
            }
        };
        put(types::wall::NAME, Value::String(name));
        for (at, v) in [
            (types::wall::START_X, moved.start[0]),
            (types::wall::START_Y, moved.start[1]),
            (types::wall::END_X, moved.end[0]),
            (types::wall::END_Y, moved.end[1]),
        ] {
            put(at, types::length(v));
        }
        if let Some((original, type_values)) = wall_types.get(&old).cloned() {
            let kind = archbuilder_model::element_type::TypeKind::Wall;
            let ty = crate::element_type::adopt(ctx, kind, original, type_values)?;
            put(kind.field(), Value::Reference(ty));
        }
        crate::element_type::rebind(ctx, WALL, &mut values)?;
        // A door or window type of a panel that this document does not have is dropped (the
        // panel becomes a plain door or window).
        let mut overrides = archbuilder_model::curtain::CellOverride::list(&values);
        if !overrides.is_empty() {
            for o in &mut overrides {
                let kind = o.type_id.and_then(|t| spatial::type_of(ctx.snapshot(), t));
                if !matches!(kind.as_deref(), Some(types::DOOR_TYPE | types::WINDOW_TYPE)) {
                    o.type_id = None;
                }
            }
            if let Some(slot) = values.get_mut(types::wall::CELL_OVERRIDES) {
                *slot = Some(archbuilder_model::curtain::CellOverride::list_value(
                    &overrides,
                ));
            }
        }
        let new = create(ctx, WALL, Some(storey), values)?;
        map.insert(old, new);
        created.push(new);
    }
    for (key, _, host, mut values) in decoded.iter().cloned() {
        if !matches!(
            key.as_str(),
            DOOR | WINDOW | WALL_OPENING | types::FAMILY_INSERT
        ) {
            continue;
        }
        let Some(host) = host.and_then(|h| map.get(&h).copied()) else {
            continue;
        };
        // A family instance keeps its mark and its family type, which must exist here as for a
        // component.
        if key == types::FAMILY_INSERT {
            created.push(create(ctx, &key, Some(host), values)?);
            continue;
        }
        let Some(kind) = Kind::of_key(&key) else {
            continue;
        };
        let seen = ctx.txn()?.snapshot().clone();
        let name = crate::insert::next_name(&seen, kind, host, &tongue);
        if let Some(slot) = values.get_mut(types::insert::NAME) {
            *slot = Some(Value::String(name));
        }
        crate::element_type::rebind(ctx, &key, &mut values)?;
        created.push(create(ctx, &key, Some(host), values)?);
    }
    let mut hosts: BTreeMap<ObjectId, ObjectId> = BTreeMap::new();
    for (key, old, _, mut values) in decoded.iter().cloned() {
        if key != FLOOR && key != CEILING {
            continue;
        }
        let floor = Floor::from_values(&values).ok_or_else(invalid)?;
        let (vertices, _) = floor_vertices(&floor, None, &xf, &|w| map.get(&w).copied());
        let seen = ctx.txn()?.snapshot().clone();
        let name = if key == FLOOR {
            crate::floor::next_floor_name(&seen, storey, &tongue)
        } else {
            crate::ceiling::next_ceiling_name(&seen, storey, &tongue)
        };
        // A floor and a ceiling keep their name, boundary and arcs in the same places.
        if let Some(slot) = values.get_mut(types::floor::NAME) {
            *slot = Some(Value::String(name));
        }
        if let Some(slot) = values.get_mut(types::floor::VERTICES) {
            *slot = Some(Floor::vertex_values(&vertices));
        }
        if let Some(slot) = values.get_mut(types::floor::ARCS) {
            *slot = Floor::arc_values(&vertices);
        }
        if key == CEILING {
            crate::ceiling::moved_values(&mut values, &xf);
        } else {
            crate::ceiling::moved_floor_slope(&mut values, &xf);
        }
        crate::element_type::rebind(ctx, &key, &mut values)?;
        let new = create(ctx, &key, Some(storey), values)?;
        hosts.insert(old, new);
        created.push(new);
    }
    for (key, _, _, mut values) in decoded.iter().cloned() {
        if key != SHAFT {
            continue;
        }
        let shape =
            Floor::read(&values, archbuilder_model::floor::SHAFT_SLOTS).ok_or_else(invalid)?;
        let (vertices, _) = floor_vertices(&shape, None, &xf, &|_| None);
        let seen = ctx.txn()?.snapshot().clone();
        let name = crate::opening::next_name(&seen, SHAFT, None, &tongue);
        for (at, value) in [
            (types::shaft::NAME, Some(Value::String(name))),
            (
                types::shaft::VERTICES,
                Some(Floor::vertex_values(&vertices)),
            ),
            (types::shaft::ARCS, Floor::arc_values(&vertices)),
            // The levels belong to the storeys of the document the shaft came from.
            (types::shaft::BASE_LEVEL, None),
            (types::shaft::TOP_LEVEL, None),
        ] {
            if let Some(slot) = values.get_mut(at) {
                *slot = value;
            }
        }
        created.push(create(ctx, SHAFT, Some(storey), values)?);
    }
    for (key, _, _, mut values) in decoded.iter().cloned() {
        if key != ROOM {
            continue;
        }
        let room = archbuilder_model::room::Room::from_values(&values).ok_or_else(invalid)?;
        let at = xf.point(room.point);
        let seen = ctx.txn()?.snapshot().clone();
        let rooms = spatial::children(&seen, ROOM, storey);
        let number = archbuilder_model::room::next_number(
            rooms
                .iter()
                .filter_map(|r| types::text(&r.values, types::room::NUMBER)),
        );
        for (slot, value) in [
            (types::room::NUMBER, Value::String(number)),
            (types::room::X, types::length(at[0])),
            (types::room::Y, types::length(at[1])),
        ] {
            if let Some(v) = values.get_mut(slot) {
                *v = Some(value);
            }
        }
        if let Some(v) = values.get_mut(types::room::UPPER_LEVEL) {
            *v = None;
        }
        created.push(create(ctx, ROOM, Some(storey), values)?);
    }
    for (key, old, _, mut values) in decoded.iter().cloned() {
        if key != ROOF {
            continue;
        }
        let roof = archbuilder_model::roof::Roof::from_values(&values).ok_or_else(invalid)?;
        let kept = archbuilder_model::roof::Roof::kept_slopes(&values);
        let (vertices, _) = floor_vertices(&roof.as_floor(), None, &xf, &|w| map.get(&w).copied());
        let edges: Vec<archbuilder_model::roof::Edge> = roof
            .edges
            .iter()
            .zip(vertices)
            .map(|(e, vertex)| archbuilder_model::roof::Edge { vertex, ..*e })
            .collect();
        let seen = ctx.txn()?.snapshot().clone();
        let name = crate::roof::next_roof_name(&seen, storey, &tongue);
        if let Some(slot) = values.get_mut(types::roof::NAME) {
            *slot = Some(Value::String(name));
        }
        if let Some(slot) = values.get_mut(types::roof::EDGES) {
            *slot = Some(archbuilder_model::roof::Roof::edge_values(&edges, &kept));
        }
        crate::element_type::rebind(ctx, ROOF, &mut values)?;
        let new = create(ctx, ROOF, Some(storey), values)?;
        hosts.insert(old, new);
        created.push(new);
    }
    for (key, _, _, values) in decoded.iter().cloned() {
        if key != types::ELEVATOR {
            continue;
        }
        let mut moved = archbuilder_model::elevator::Elevator::from_values(&values)
            .ok_or_else(invalid)?
            .moved(&xf);
        // The levels belong to the storeys of the document the elevator came from.
        moved.top_level = None;
        moved.stops.clear();
        let seen = ctx.txn()?.snapshot().clone();
        let name = crate::elevator::next_name(&seen, storey, &tongue);
        created.push(create(
            ctx,
            types::ELEVATOR,
            Some(storey),
            moved.values(&name),
        )?);
    }
    // A component moves with the paste; its family type stays (the type must exist here).
    for (key, _, _, values) in decoded.iter().cloned() {
        if key != types::COMPONENT {
            continue;
        }
        let mut values = values;
        let at = |i: usize, v: &[Option<Value>]| types::meters(v, i).unwrap_or(0.0);
        let moved = xf.point([
            at(types::component::X, &values),
            at(types::component::Y, &values),
        ]);
        for (slot, m) in [
            (types::component::X, moved[0]),
            (types::component::Y, moved[1]),
        ] {
            if let Some(v) = values.get_mut(slot) {
                *v = Some(types::length(m));
            }
        }
        crate::element_type::rebind(ctx, types::COMPONENT, &mut values)?;
        created.push(create(ctx, types::COMPONENT, Some(storey), values)?);
    }
    for (key, _, host, values) in decoded.iter().cloned() {
        if key != types::DORMER && key != types::ROOF_EDGE {
            continue;
        }
        let Some(host) = host.and_then(|h| hosts.get(&h).copied()) else {
            continue;
        };
        let seen = ctx.txn()?.snapshot().clone();
        let mut values = values;
        if key == types::DORMER {
            let moved = archbuilder_model::dormer::Dormer::from_values(&values)
                .ok_or_else(invalid)?
                .moved(&xf);
            let name = crate::roof_parts::next_dormer_name(&seen, host, &tongue);
            values = moved.values(&name);
        } else {
            let gutter = archbuilder_model::roof_edge::RoofEdge::from_values(&values)
                .is_some_and(|e| e.role == archbuilder_model::roof_edge::Role::Gutter);
            let name = crate::roof_parts::next_roof_edge_name(&seen, host, gutter, &tongue);
            if let Some(slot) = values.get_mut(types::roof_edge::NAME) {
                *slot = Some(Value::String(name));
            }
        }
        created.push(create(ctx, &key, Some(host), values)?);
    }
    for (key, _, host, mut values) in decoded.iter().cloned() {
        if key != types::FLOOR_EDGE {
            continue;
        }
        let Some(host) = host.and_then(|h| hosts.get(&h).copied()) else {
            continue;
        };
        let seen = ctx.txn()?.snapshot().clone();
        if let Some(slot) = values.get_mut(types::floor_edge::NAME) {
            *slot = Some(Value::String(crate::floor_edge::next_name(
                &seen, host, &tongue,
            )));
        }
        created.push(create(ctx, types::FLOOR_EDGE, Some(host), values)?);
    }
    for (key, _, host, mut values) in decoded.iter().cloned() {
        if key != VERTICAL_OPENING {
            continue;
        }
        let Some(host) = host.and_then(|h| hosts.get(&h).copied()) else {
            continue;
        };
        let shape =
            Floor::read(&values, archbuilder_model::floor::VERTICAL_SLOTS).ok_or_else(invalid)?;
        let (vertices, _) = floor_vertices(&shape, None, &xf, &|_| None);
        let seen = ctx.txn()?.snapshot().clone();
        let name = crate::opening::next_name(&seen, VERTICAL_OPENING, Some(host), &tongue);
        for (at, value) in [
            (types::vertical_opening::NAME, Some(Value::String(name))),
            (
                types::vertical_opening::VERTICES,
                Some(Floor::vertex_values(&vertices)),
            ),
            (types::vertical_opening::ARCS, Floor::arc_values(&vertices)),
        ] {
            if let Some(slot) = values.get_mut(at) {
                *slot = value;
            }
        }
        created.push(create(ctx, VERTICAL_OPENING, Some(host), values)?);
    }
    // The stairs and ramps pasted, by original: their hosted railings follow.
    let mut circulation: BTreeMap<ObjectId, ObjectId> = BTreeMap::new();
    for (key, old, _, values) in decoded.iter().cloned() {
        if key != STAIR {
            continue;
        }
        let seen = ctx.txn()?.snapshot().clone();
        let name = crate::stair::next_stair_name(&seen, storey, &tongue);
        let moved = archbuilder_model::stair::Stair::moved_values(&values, &name, &xf, true)
            .ok_or_else(invalid)?;
        let new = create(ctx, STAIR, Some(storey), moved)?;
        circulation.insert(old, new);
        created.push(new);
    }
    for (key, old, _, values) in decoded.iter().cloned() {
        if key != types::RAMP {
            continue;
        }
        let mut moved = archbuilder_model::ramp::Ramp::from_values(&values)
            .ok_or_else(invalid)?
            .moved(&xf);
        // The top follows the level above the storey the ramp is pasted into.
        moved.top_level = None;
        moved.ramp_type =
            crate::circulation::own(ctx.snapshot(), moved.ramp_type, types::RAMP_TYPE);
        let name =
            crate::circulation::next_name(ctx.txn()?.snapshot(), types::RAMP, storey, &tongue);
        let new = create(ctx, types::RAMP, Some(storey), moved.values(&name))?;
        circulation.insert(old, new);
        created.push(new);
    }
    for (key, _, host, values) in decoded.iter().cloned() {
        if key != types::RAILING {
            continue;
        }
        let railing =
            archbuilder_model::railing::Railing::from_values(&values).ok_or_else(invalid)?;
        match host {
            Some(host) => {
                let Some(new) = circulation.get(&host).copied() else {
                    continue;
                };
                created.extend(crate::circulation::copy_hosted_railings(
                    ctx,
                    vec![railing],
                    new,
                    storey,
                    false,
                    &tongue,
                )?);
            }
            None => {
                let moved = archbuilder_model::railing::Railing {
                    continues_to: None,
                    railing_type: crate::circulation::own(
                        ctx.snapshot(),
                        railing.railing_type,
                        types::RAILING_TYPE,
                    ),
                    ..railing.moved(&xf)
                };
                let name = crate::circulation::next_name(
                    ctx.txn()?.snapshot(),
                    types::RAILING,
                    storey,
                    &tongue,
                );
                created.push(create(
                    ctx,
                    types::RAILING,
                    Some(storey),
                    moved.values(&name),
                )?);
            }
        }
    }
    let building = spatial::get(ctx.snapshot(), storey).and_then(|s| s.header.owner);
    for (key, _, _, values) in decoded.iter().cloned() {
        if key != types::GRID && key != types::REFERENCE_PLANE {
            continue;
        }
        let building = building.ok_or_else(invalid)?;
        created.push(crate::datum::copy_datum(ctx, &key, values, building, &xf)?);
    }
    let extruded: Vec<_> = decoded
        .iter()
        .filter(|(key, ..)| key == types::ROOF_EXTRUSION)
        .map(|(_, _, _, values)| (None, values.clone()))
        .collect();
    created.extend(crate::roof_extrusion::copy_roofs(
        ctx,
        extruded,
        &xf,
        Some(storey),
    )?);
    if created.is_empty() {
        return Err(Error::new(codes::CLIPBOARD_EMPTY));
    }
    let meets = ensure_valid_lenient(ctx)?;
    ctx.set_selection_after(created.clone());
    Ok(Modified {
        objects: created.len(),
        created: created.into_iter().map(arc_command::IdParam).collect(),
        meets,
        ..Modified::default()
    })
}

/// The plan outlines of the content at their original position: the footprint of each wall and
/// the boundary points of each floor (closed polygons), the paths and lines of railings, ramps,
/// grids, reference planes and roofs by extrusion (open lines). A ghost of the paste draws them
/// moved.
#[must_use]
pub fn outlines(content: &Json) -> Vec<Vec<[f64; 2]>> {
    let items = content.get("items").and_then(Json::as_array);
    let mut out = Vec::new();
    for item in items.into_iter().flatten().take(MAX_ITEMS) {
        let key = item.get("key").and_then(Json::as_str).unwrap_or_default();
        let values: Vec<Option<Value>> = item
            .get("values")
            .and_then(Json::as_array)
            .map(|v| v.iter().map(decode).collect())
            .unwrap_or_default();
        if key == WALL {
            if let Some(w) = Wall::from_values(&values) {
                let f = w.plain_footprint();
                out.push(f.iter().chain(f.first()).copied().collect());
            }
        } else if key == STAIR {
            let stair = archbuilder_model::stair::Stair::from_values(&values);
            // The outline at a rise of three meters stands for the stair; the storey sets the rise.
            let lines = archbuilder_model::stair::run_lines(&values);
            if let Some(Ok(layout)) =
                stair.map(|s| archbuilder_model::stair::Layout::build(&s, &lines, 3.0))
            {
                let f = layout.footprint();
                out.push(f.iter().chain(f.first()).copied().collect());
            }
        } else if key == ROOF
            && let Some(r) = archbuilder_model::roof::Roof::from_values(&values)
        {
            let mut l: Vec<[f64; 2]> = r.edges.iter().map(|e| e.vertex.point).collect();
            l.extend(l.first().copied());
            out.push(l);
        } else if key == types::RAILING
            && let Some(r) = archbuilder_model::railing::Railing::from_values(&values)
            && r.path.len() >= 2
        {
            out.push(r.path.iter().map(|v| [v.point[0], v.point[1]]).collect());
        } else if key == types::RAMP
            && let Some(r) = archbuilder_model::ramp::Ramp::from_values(&values)
        {
            let (s, c) = r.direction.sin_cos();
            let run = r.landing_start + r.length + r.landing_end;
            out.push(vec![r.start, [r.start[0] + c * run, r.start[1] + s * run]]);
        } else if key == types::GRID
            && let Some(g) = archbuilder_model::datum::Grid::from_values(&values)
        {
            out.push(g.points());
        } else if key == types::REFERENCE_PLANE
            && let Some(r) = archbuilder_model::datum::RefPlane::from_values(&values)
        {
            out.push(vec![r.start, r.end]);
        } else if key == types::ROOF_EXTRUSION
            && let Some(r) = archbuilder_model::roof_extrusion::RoofExtrusion::from_values(&values)
        {
            // The plane's line in plan, as far as the profile reaches along it.
            let (s, c) = r.direction.sin_cos();
            let at = |u: f64| [r.origin[0] + c * u, r.origin[1] + s * u];
            let (lo, hi) = r.profile.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
                (lo.min(p[0]), hi.max(p[0]))
            });
            if lo <= hi {
                out.push(vec![at(lo), at(hi)]);
            }
        } else if (key == FLOOR || key == CEILING)
            && let Some(f) = Floor::from_values(&values)
        {
            let mut loops: BTreeMap<u32, Vec<[f64; 2]>> = BTreeMap::new();
            for v in &f.vertices {
                loops.entry(v.loop_index).or_default().push(v.point);
            }
            for mut l in loops.into_values() {
                l.extend(l.first().copied());
                out.push(l);
            }
        }
    }
    out
}
