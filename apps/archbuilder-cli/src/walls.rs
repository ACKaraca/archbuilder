//! The walls of an `inspect` dump: for each wall record, the storey it stands on, the length of
//! its baseline, its thickness and height, the style of its two ends, the volume of its joined solid
//! (corners counted once) as `gross_volume` and that solid minus the openings of its doors and
//! windows as `volume`, all in meters. A wall whose top follows a level shows the height that level gives
//! ([`Wall::constrained`], the derivation the application uses). Read from the dump's JSON, so the CLI shows what a file holds without opening it as a
//! document.

use archbuilder_model::insert::{Facing, Hand, Insert, Kind, Operation, cut};
use archbuilder_model::join::footprints;
use archbuilder_model::types::{DOOR, LEVEL, STOREY, WALL, WINDOW};
use archbuilder_model::wall::{JoinStyle, LocationLine, Wall};
use serde_json::{Value, json};

fn number(values: &Value, field: &str) -> Option<f64> {
    values.get(field)?.as_f64()
}

/// The `walls` list of the dump's `objects`, in the dump's order.
#[must_use]
pub fn summary(objects: &[Value]) -> Vec<Value> {
    let of = |key: &'static str| objects.iter().filter(move |o| o["type_key"] == key);
    let name_of = |id: &Value, key: &'static str| {
        of(key)
            .find(|o| o["id"] == *id)
            .and_then(|o| o["values"]["name"].as_str().map(str::to_owned))
    };
    // The walls of the dump with their data, in the dump's order; joins are derived per storey.
    let read = |object: &Value| -> Option<(Wall, f64)> {
        let values = &object["values"];
        let owner = &object["owner"];
        let elevation = of(STOREY)
            .find(|s| s["id"] == *owner)
            .and_then(|s| {
                let level_id = &s["values"]["level"];
                of(LEVEL).find(|l| l["id"] == *level_id)
            })
            .and_then(|l| number(&l["values"], "elevation"))
            .unwrap_or(0.0);
        let top_elevation = of(LEVEL)
            .find(|l| l["id"] == values["top_level"])
            .and_then(|l| number(&l["values"], "elevation"));
        Some((
            Wall {
                start: [number(values, "start_x")?, number(values, "start_y")?],
                end: [number(values, "end_x")?, number(values, "end_y")?],
                thickness: number(values, "thickness")?,
                height: number(values, "height")?,
                base_offset: number(values, "base_offset")?,
                location: LocationLine::ALL
                    .into_iter()
                    .find(|l| code_of(&values["location_line"], |c| l.code() == c, l.name()))
                    .unwrap_or_default(),
                start_join: join_of(&values["start_join"]),
                end_join: join_of(&values["end_join"]),
            }
            .constrained(elevation, top_elevation),
            elevation,
        ))
    };
    let all: Vec<(&Value, Option<(Wall, f64)>)> = of(WALL).map(|o| (o, read(o))).collect();
    let mut feet: Vec<Option<[[f64; 2]; 4]>> = vec![None; all.len()];
    let mut storeys: Vec<&Value> = Vec::new();
    for (object, _) in &all {
        if !storeys.contains(&&object["owner"]) {
            storeys.push(&object["owner"]);
        }
    }
    for owner in storeys {
        let members: Vec<usize> = (0..all.len())
            .filter(|&k| all[k].0["owner"] == *owner && all[k].1.is_some())
            .collect();
        let walls: Vec<Wall> = members
            .iter()
            .filter_map(|&k| all[k].1.map(|d| d.0))
            .collect();
        for (&k, f) in members.iter().zip(footprints(&walls)) {
            feet[k] = Some(f);
        }
    }
    all.iter()
        .zip(&feet)
        .map(|((object, data), foot)| {
            let values = &object["values"];
            let owner = &object["owner"];
            let solid = data.zip(*foot).and_then(|((w, elevation), f)| {
                w.solid_in(f, elevation).ok().map(|s| (w, elevation, s))
            });
            let gross = solid.as_ref().map(|(_, _, s)| s.volume());
            let mut hosted = inserts_of(objects, &object["id"]);
            hosted.sort_by(|a, b| a.distance.total_cmp(&b.distance));
            let volume = solid
                .and_then(|(w, elevation, s)| cut(&w, elevation, s, &hosted).ok())
                .map(|s| s.volume());
            let data = data.map(|d| d.0);
            json!({
                "id": object["id"],
                "name": values["name"],
                "storey": name_of(owner, STOREY),
                "length": data.as_ref().map(Wall::length),
                "thickness": data.as_ref().map(|w| w.thickness),
                "height": data.as_ref().map(|w| w.height),
                "start_join": data.as_ref().map(|w| w.start_join.name()),
                "end_join": data.as_ref().map(|w| w.end_join.name()),
                "gross_volume": gross,
                "openings": hosted.len(),
                "volume": volume,
            })
        })
        .collect()
}

/// Whether a stored enum value (a name or a code) is the one `matches` accepts or `name` names.
fn code_of(value: &Value, matches: impl Fn(u32) -> bool, name: &str) -> bool {
    match value {
        Value::String(s) => s == name,
        Value::Number(n) => n
            .as_u64()
            .and_then(|c| u32::try_from(c).ok())
            .is_some_and(matches),
        _ => false,
    }
}

/// The join style a stored value names; the default when it is absent.
fn join_of(value: &Value) -> JoinStyle {
    JoinStyle::ALL
        .into_iter()
        .find(|s| code_of(value, |c| s.code() == c, s.name()))
        .unwrap_or_default()
}

fn named<T: Copy>(
    value: &Value,
    all: &[T],
    code: fn(T) -> u32,
    name: fn(T) -> &'static str,
) -> Option<T> {
    all.iter()
        .copied()
        .find(|v| code_of(value, |c| code(*v) == c, name(*v)))
}

/// The insert a dump object describes; `None` when a field is missing.
fn insert_of(kind: Kind, object: &Value) -> Option<Insert> {
    let values = &object["values"];
    Some(Insert {
        kind,
        distance: number(values, "distance")?,
        width: number(values, "width")?,
        height: number(values, "height")?,
        sill: number(values, "sill")?,
        frame_width: number(values, "frame_width")?,
        frame_depth: number(values, "frame_depth")?,
        operation: named(
            &values["operation"],
            &Operation::ALL,
            Operation::code,
            Operation::name,
        )
        .unwrap_or_default(),
        hand: named(&values["hand"], &Hand::ALL, Hand::code, Hand::name).unwrap_or_default(),
        facing: named(&values["facing"], &Facing::ALL, Facing::code, Facing::name)
            .unwrap_or_default(),
    })
}

/// The inserts of the wall `wall` in the dump.
fn inserts_of(objects: &[Value], wall: &Value) -> Vec<Insert> {
    [(DOOR, Kind::Door), (WINDOW, Kind::Window)]
        .into_iter()
        .flat_map(|(key, kind)| {
            objects
                .iter()
                .filter(move |o| o["type_key"] == key && o["owner"] == *wall)
                .filter_map(move |o| insert_of(kind, o))
        })
        .collect()
}

/// The doors (`kind` door) or windows of a dump: each with its host wall, the distance of its
/// center along the wall, width, height and sill, all in meters, and its hand and facing.
#[must_use]
pub fn inserts(objects: &[Value], kind: Kind) -> Vec<Value> {
    let key = kind.key();
    objects
        .iter()
        .filter(|o| o["type_key"] == key)
        .map(|object| {
            let values = &object["values"];
            let item = insert_of(kind, object);
            let wall = objects
                .iter()
                .find(|w| w["type_key"] == WALL && w["id"] == object["owner"])
                .and_then(|w| w["values"]["name"].as_str().map(str::to_owned));
            json!({
                "id": object["id"],
                "name": values["name"],
                "wall": wall,
                "distance": item.as_ref().map(|i| i.distance),
                "width": item.as_ref().map(|i| i.width),
                "height": item.as_ref().map(|i| i.height),
                "sill": item.as_ref().map(|i| i.sill),
                "operation": item.as_ref().map(|i| i.operation.name()),
                "hand": item.as_ref().map(|i| i.hand.name()),
                "facing": item.as_ref().map(|i| i.facing.name()),
            })
        })
        .collect()
}
