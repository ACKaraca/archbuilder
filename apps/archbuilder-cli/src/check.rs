//! The product findings of `inspect --check`, on top of the shared package check: an open corner
//! (the ends of two walls of a storey that lie close together and are not joined) and a floor edge
//! that follows a wall which no longer bounds it. They are warnings: the exit status is unchanged.
//! Read from the dump's JSON with the model's own rules ([`archbuilder_model::join::open_corners`],
//! [`archbuilder_model::floor::Floor::unsupported_edges`]).

use archbuilder_model::types::{FLOOR, WALL};
use serde_json::{Value, json};

use crate::floors::{floor_of, wall_of};

fn name(object: &Value) -> Value {
    object["values"]["name"].clone()
}

/// The findings of the dump's `objects`.
#[must_use]
pub fn findings(objects: &[Value]) -> Vec<Value> {
    let mut out = Vec::new();
    let walls: Vec<&Value> = objects.iter().filter(|o| o["type_key"] == WALL).collect();
    let mut storeys: Vec<&Value> = Vec::new();
    for w in &walls {
        if !storeys.contains(&&w["owner"]) {
            storeys.push(&w["owner"]);
        }
    }
    for owner in storeys {
        let members: Vec<(&Value, archbuilder_model::wall::Wall)> = walls
            .iter()
            .filter(|w| w["owner"] == *owner)
            .filter_map(|w| Some((*w, wall_of(w)?)))
            .collect();
        let plain: Vec<_> = members.iter().map(|(_, w)| *w).collect();
        for (i, j) in archbuilder_model::join::open_corners(&plain) {
            out.push(json!({
                "severity": "warning",
                "code": "ab.model.wall_corner_open",
                "message_id": "ab-model-wall-corner-open",
                "arguments": {"name": name(members[i].0), "other": name(members[j].0)},
            }));
        }
    }
    for object in objects.iter().filter(|o| o["type_key"] == FLOOR) {
        let Some(floor) = floor_of(&object["values"], objects) else {
            continue;
        };
        let wall_by_id = |id: arc_base::ObjectId| {
            objects
                .iter()
                .find(|o| o["type_key"] == WALL && o["id"] == id.to_string())
                .and_then(wall_of)
        };
        let Some(boundary) = floor.boundary(&wall_by_id) else {
            continue;
        };
        for id in floor.unsupported_edges(&boundary, &wall_by_id) {
            let wall = objects
                .iter()
                .find(|o| o["type_key"] == WALL && o["id"] == id.to_string())
                .map_or(Value::Null, name);
            out.push(json!({
                "severity": "warning",
                "code": "ab.model.floor_edge_unsupported",
                "message_id": "ab-model-floor-edge-unsupported",
                "arguments": {"name": name(object), "wall": wall},
            }));
        }
    }
    out
}
