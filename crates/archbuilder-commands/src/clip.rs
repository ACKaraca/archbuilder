//! Clipboard content: a copy of walls, doors and windows, floors and rooms that carries their field
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
use archbuilder_model::types::{self, DOOR, FLOOR, ROOM, WALL, WINDOW};
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
            FLOOR | ROOM => push(
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
    for id in ids.iter().take(MAX_ITEMS) {
        let key = spatial::type_of(snapshot, *id).unwrap_or_default();
        if let (true, Some(object)) = (key == DOOR || key == WINDOW, spatial::get(snapshot, *id))
            && let Some(host) = object.header.owner.filter(|h| walls.contains(h))
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
    reason = "one pass for each kind: walls, inserts, floors, rooms"
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
        let new = create(ctx, WALL, Some(storey), values)?;
        map.insert(old, new);
        created.push(new);
    }
    for (key, _, host, mut values) in decoded.iter().cloned() {
        if key != DOOR && key != WINDOW {
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
        created.push(create(ctx, &key, Some(host), values)?);
    }
    for (key, _, _, mut values) in decoded.iter().cloned() {
        if key != FLOOR {
            continue;
        }
        let floor = Floor::from_values(&values).ok_or_else(invalid)?;
        let (vertices, _) = floor_vertices(&floor, None, &xf, &|w| map.get(&w).copied());
        let seen = ctx.txn()?.snapshot().clone();
        let name = crate::floor::next_floor_name(&seen, storey, &tongue);
        if let Some(slot) = values.get_mut(types::floor::NAME) {
            *slot = Some(Value::String(name));
        }
        if let Some(slot) = values.get_mut(types::floor::VERTICES) {
            *slot = Some(Floor::vertex_values(&vertices));
        }
        created.push(create(ctx, FLOOR, Some(storey), values)?);
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
        } else if key == FLOOR
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
