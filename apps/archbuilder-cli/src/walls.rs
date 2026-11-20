//! The walls of an `inspect` dump: for each wall record, the storey it stands on, the length of
//! its baseline, its thickness and height, the style of its two ends and the volume of its joined solid (corners counted once), all in
//! meters. A wall whose top follows a level shows the height that level gives
//! ([`Wall::constrained`], the derivation the application uses). Read from the dump's JSON, so the CLI shows what a file holds without opening it as a
//! document.

use archbuilder_model::join::footprints;
use archbuilder_model::types::{LEVEL, STOREY, WALL};
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
            let volume = data
                .zip(*foot)
                .and_then(|((w, elevation), f)| w.solid_in(f, elevation).ok())
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
