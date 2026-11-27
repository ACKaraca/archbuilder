//! Clipboard content: a copy of walls, doors and windows, floors, rooms and stairs that carries their field
//! values, so it can be pasted after the originals are deleted and into another document of the
//! same product. [`capture`] reads the objects of a snapshot into JSON; the paste commands
//! (`archbuilder.clipboard.paste`, `paste_to_storey`, with the `content` parameter) create new
//! objects from it in a storey, translated by a displacement.
//!
//! What is kept: every field value except references to objects outside the content. A wall's top
//! constraint is replaced by its effective height (the top becomes unconnected); the edges of a
//! floor follow the pasted walls they followed; a door or window needs its wall in the content.

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

/// The content of the clipboard for `ids`: the walls, floors and rooms among them, the doors and
/// windows their walls host, and the doors and windows of the selection whose wall is also in it.
///
/// # Errors
/// `ab.cmd.nothing_selected` when none of `ids` can be copied.
#[allow(clippy::too_many_lines, reason = "one arm for each kind captured")]
pub fn capture(snapshot: &Snapshot, ids: &[ObjectId]) -> Result<Json> {
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
            FLOOR | ROOM | ROOF | CEILING | SHAFT => push(
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
            key == DOOR || key == WINDOW || key == WALL_OPENING,
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
        crate::element_type::rebind(ctx, WALL, &mut values)?;
        let new = create(ctx, WALL, Some(storey), values)?;
        map.insert(old, new);
        created.push(new);
    }
    for (key, _, host, mut values) in decoded.iter().cloned() {
        if key != DOOR && key != WINDOW && key != WALL_OPENING {
            continue;
        }
        let (Some(kind), Some(host)) =
            (Kind::of_key(&key), host.and_then(|h| map.get(&h).copied()))
        else {
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
    for (key, _, _, values) in decoded.iter().cloned() {
        if key != STAIR {
            continue;
        }
        let mut moved = archbuilder_model::stair::Stair::from_values(&values)
            .ok_or_else(invalid)?
            .moved(&xf);
        moved.top_level = None;
        let seen = ctx.txn()?.snapshot().clone();
        let name = crate::stair::next_stair_name(&seen, storey, &tongue);
        created.push(create(ctx, STAIR, Some(storey), moved.values(&name))?);
    }
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

/// The plan outlines of the content, closed polygons at their original position: the footprint of
/// each wall and the boundary points of each floor. A ghost of the paste draws them moved.
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
            if let Some(Ok(layout)) = stair.map(|s| archbuilder_model::stair::Layout::new(&s, 3.0))
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
