//! The rooms of an `inspect` dump: for each room record its storey, number, name, state, area,
//! perimeter and volume, derived from the dump's wall records with the model's own rules
//! ([`archbuilder_model::room`]), so the CLI shows what the application shows. The findings
//! (unenclosed, redundant, duplicate number) are the warnings of `inspect --check`.

use archbuilder_model::room::{self, State};
use archbuilder_model::types::{LEVEL, ROOM, STOREY, WALL};
use serde_json::{Value, json};

use crate::floors::wall_of;

struct Row {
    id: Value,
    storey: Value,
    number: Option<String>,
    name: Value,
    state: State,
    area: Option<f64>,
    perimeter: Option<f64>,
    volume: Option<f64>,
}

fn elevation_of(objects: &[Value], id: &Value) -> Option<f64> {
    objects
        .iter()
        .find(|o| o["type_key"] == LEVEL && o["id"] == *id)?["values"]["elevation"]
        .as_f64()
}

fn rows(objects: &[Value]) -> Vec<Row> {
    let mut out = Vec::new();
    for storey in objects.iter().filter(|o| o["type_key"] == STOREY) {
        let walls: Vec<_> = objects
            .iter()
            .filter(|o| o["type_key"] == WALL && o["owner"] == storey["id"])
            .filter_map(|o| Some((o["id"].as_str()?.parse().ok()?, wall_of(o)?)))
            .collect();
        let records: Vec<&Value> = objects
            .iter()
            .filter(|o| o["type_key"] == ROOM && o["owner"] == storey["id"])
            .filter(|o| o["values"]["x"].is_number() && o["values"]["y"].is_number())
            .take(room::MAX_ROOMS)
            .collect();
        let points: Vec<[f64; 2]> = records
            .iter()
            .map(|r| {
                [
                    r["values"]["x"].as_f64().unwrap_or(0.0),
                    r["values"]["y"].as_f64().unwrap_or(0.0),
                ]
            })
            .collect();
        let items: Vec<([f64; 2], Option<&str>)> = records
            .iter()
            .zip(&points)
            .map(|(r, p)| (*p, r["values"]["number"].as_str()))
            .collect();
        let prints = room::footprints_ranked(&walls, &items);
        let own = elevation_of(objects, &storey["values"]["level"]).unwrap_or(0.0);
        let above = objects
            .iter()
            .filter(|o| o["type_key"] == STOREY)
            .filter_map(|s| elevation_of(objects, &s["values"]["level"]))
            .filter(|e| *e > own + archbuilder_model::types::HEIGHT_RANGE.0)
            .min_by(f64::total_cmp);
        for (record, print) in records.iter().zip(prints) {
            let v = &record["values"];
            let number = |f: &str| v[f].as_f64().unwrap_or(0.0);
            let upper = elevation_of(objects, &v["upper_level"]);
            let height = room::height(
                own,
                upper,
                above,
                number("upper_offset"),
                number("base_offset"),
            );
            out.push(Row {
                id: record["id"].clone(),
                storey: storey["values"]["name"].clone(),
                number: v["number"].as_str().map(str::to_owned),
                name: v["name"].clone(),
                state: print.state,
                area: print.area(),
                perimeter: print.perimeter(),
                volume: print.area().map(|a| a * height),
            });
        }
    }
    out
}

/// The `rooms` list of the dump's `objects`.
#[must_use]
pub fn summary(
    objects: &[Value],
    nets: &std::collections::HashMap<String, (Option<f64>, Option<f64>)>,
) -> Vec<Value> {
    rows(objects)
        .into_iter()
        .map(|r| {
            // The area and volume the application shows replace the footprint's, where known.
            let (area, volume) =
                r.id.as_str()
                    .and_then(|id| nets.get(id))
                    .copied()
                    .unwrap_or((r.area, r.volume));
            json!({
                "id": r.id, "storey": r.storey, "number": r.number, "name": r.name,
                "state": r.state.name(), "area": area, "perimeter": r.perimeter,
                "volume": volume,
            })
        })
        .collect()
}

/// The findings about rooms, in the form of the shared check.
#[must_use]
pub fn findings(objects: &[Value]) -> Vec<Value> {
    let rows = rows(objects);
    let mut out = Vec::new();
    for r in &rows {
        let duplicate = r.number.as_deref().is_some_and(|n| {
            rows.iter().any(|o| {
                o.id != r.id
                    && o.storey == r.storey
                    && o.number
                        .as_deref()
                        .is_some_and(|m| room::number_key(m) == room::number_key(n))
            })
        });
        let found = [
            (
                r.state == State::Unenclosed,
                "ab.model.room_unenclosed",
                "ab-model-room-unenclosed",
            ),
            (
                r.state == State::Redundant,
                "ab.model.room_redundant",
                "ab-model-room-redundant",
            ),
            (
                duplicate,
                "ab.model.room_number_duplicate",
                "ab-model-room-number-duplicate",
            ),
        ];
        for (_, code, message_id) in found.iter().filter(|(on, _, _)| *on) {
            out.push(json!({
                "severity": "warning",
                "code": code,
                "message_id": message_id,
                "arguments": {"name": r.name, "number": r.number},
            }));
        }
    }
    out
}
