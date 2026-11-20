//! The walls of an `inspect` dump: for each wall record, the storey it stands on, the length of
//! its baseline, its thickness and height, and the volume of the solid derived from it, all in
//! meters. A wall whose top follows a level shows the height that level gives
//! ([`Wall::constrained`], the derivation the application uses). Read from the dump's JSON, so the CLI shows what a file holds without opening it as a
//! document.

use archbuilder_model::types::{LEVEL, STOREY, WALL};
use archbuilder_model::wall::{LocationLine, Wall};
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
    of(WALL)
        .map(|object| {
            let values = &object["values"];
            let owner = &object["owner"];
            let storey_object = of(STOREY).find(|s| s["id"] == *owner);
            let elevation = storey_object
                .and_then(|s| {
                    let level_id = &s["values"]["level"];
                    of(LEVEL).find(|l| l["id"] == *level_id)
                })
                .and_then(|l| number(&l["values"], "elevation"))
                .unwrap_or(0.0);
            let location = match &values["location_line"] {
                Value::String(name) => LocationLine::ALL
                    .into_iter()
                    .find(|l| l.name() == name)
                    .unwrap_or_default(),
                Value::Number(code) => LocationLine::from_code(
                    code.as_u64()
                        .and_then(|c| u32::try_from(c).ok())
                        .unwrap_or(0),
                ),
                _ => LocationLine::Center,
            };
            let top_elevation = of(LEVEL)
                .find(|l| l["id"] == values["top_level"])
                .and_then(|l| number(&l["values"], "elevation"));
            let data = (|| {
                Some(Wall {
                    start: [number(values, "start_x")?, number(values, "start_y")?],
                    end: [number(values, "end_x")?, number(values, "end_y")?],
                    thickness: number(values, "thickness")?,
                    height: number(values, "height")?,
                    base_offset: number(values, "base_offset")?,
                    location,
                })
                .map(|wall| wall.constrained(elevation, top_elevation))
            })();
            let volume = data
                .as_ref()
                .and_then(|w| w.solid(elevation).ok())
                .map(|s| s.volume());
            json!({
                "id": object["id"],
                "name": values["name"],
                "storey": name_of(owner, STOREY),
                "length": data.as_ref().map(Wall::length),
                "thickness": data.as_ref().map(|w| w.thickness),
                "height": data.as_ref().map(|w| w.height),
                "volume": volume,
            })
        })
        .collect()
}
