//! The floors of an `inspect` dump: for each floor record, the storey it is on, its thickness and
//! top offset, the area and the volume of its slab (the area of the derived boundary times the
//! thickness; the full prism, whatever walls it overlaps), and the names of the walls its edges
//! follow, all in meters. The boundary is derived from the dump's wall records with the model's own
//! rules ([`archbuilder_model::floor`]), so the CLI shows what the application shows.

use archbuilder_model::floor::{Association, Face, Floor, Vertex};
use archbuilder_model::types::{FLOOR, LEVEL, SHAFT, STOREY, WALL};
use archbuilder_model::wall::{JoinStyle, LocationLine, Wall};
use serde_json::{Value, json};

fn number(values: &Value, field: &str) -> Option<f64> {
    values.get(field)?.as_f64()
}

pub(crate) fn wall_of(object: &Value) -> Option<Wall> {
    let v = &object["values"];
    Some(Wall {
        start: [number(v, "start_x")?, number(v, "start_y")?],
        end: [number(v, "end_x")?, number(v, "end_y")?],
        thickness: number(v, "thickness")?,
        core: [
            number(v, "core_exterior").unwrap_or(0.0),
            number(v, "core_interior").unwrap_or(0.0),
        ],
        height: number(v, "height")?,
        base_offset: number(v, "base_offset")?,
        location: LocationLine::ALL
            .into_iter()
            .find(|l| named(&v["location_line"], l.code(), l.name()))
            .unwrap_or_default(),
        start_join: JoinStyle::default(),
        end_join: JoinStyle::default(),
    })
}

/// Whether a stored enum (a name or a code) is the variant `name` with `code`.
fn named(value: &Value, code: u32, name: &str) -> bool {
    match value {
        Value::String(s) => s == name,
        Value::Number(n) => n.as_u64() == Some(u64::from(code)),
        _ => false,
    }
}

pub(crate) fn floor_of(values: &Value, objects: &[Value]) -> Option<Floor> {
    let shape = shape_of(values, objects)?;
    Some(Floor {
        thickness: number(values, "thickness")?,
        top_offset: number(values, "top_offset")?,
        ..shape
    })
}

/// The boundary data of a dump object (a floor, or a shaft, which has no thickness).
fn shape_of(values: &Value, objects: &[Value]) -> Option<Floor> {
    let mut vertices = Vec::new();
    // A vertex is stored as its fields in order: loop, x, y, wall, face, offset.
    for item in values.get("vertices")?.as_array()? {
        let edge = item[3].as_str().and_then(|w| {
            objects
                .iter()
                .find(|o| o["type_key"] == WALL && o["id"] == w)?;
            Some(Association {
                wall: w.parse().ok()?,
                face: Face::ALL
                    .into_iter()
                    .find(|f| named(&item[4], f.code(), f.name()))
                    .unwrap_or_default(),
                offset: item[5].as_f64().unwrap_or(0.0),
            })
        });
        vertices.push(Vertex {
            loop_index: u32::try_from(item[0].as_i64()?).ok()?,
            point: [item[1].as_f64()?, item[2].as_f64()?],
            edge,
            bulge: 0.0,
        });
    }
    // The arc edges are records of a vertex number and a bulge.
    for arc in values
        .get("arcs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let (Some(at), Some(bulge)) = (arc[0].as_u64(), arc[1].as_f64())
            && let Some(v) = usize::try_from(at).ok().and_then(|at| vertices.get_mut(at))
        {
            v.bulge = bulge;
        }
    }
    Some(Floor {
        thickness: 0.0,
        top_offset: 0.0,
        vertices,
    })
}

/// The elevation of the level the dump object `level` (an id) names.
fn elevation(objects: &[Value], level: &Value) -> Option<f64> {
    objects
        .iter()
        .find(|o| o["type_key"] == LEVEL && o["id"] == *level)
        .and_then(|o| number(&o["values"], "elevation"))
}

/// The footprints of the shafts that pass through the slab of the floor `object`: those whose
/// span, from the base to the top, overlaps the slab's body.
fn shaft_cuts(objects: &[Value], z: (f64, f64)) -> Vec<Vec<[f64; 2]>> {
    let own = |s: &Value| elevation(objects, &s["values"]["level"]);
    objects
        .iter()
        .filter(|o| o["type_key"] == SHAFT)
        .filter_map(|shaft| {
            let v = &shaft["values"];
            let home = objects
                .iter()
                .find(|s| s["type_key"] == STOREY && s["id"] == shaft["owner"]);
            let base = elevation(objects, &v["base_level"]).or_else(|| home.and_then(own))?
                + number(v, "base_offset").unwrap_or(0.0);
            // The level above the shaft's storey: the next storey's level up.
            let above = home.and_then(own).and_then(|here| {
                objects
                    .iter()
                    .filter(|s| s["type_key"] == STOREY)
                    .filter_map(own)
                    .filter(|e| *e > here + 1.0e-9)
                    .min_by(f64::total_cmp)
            });
            let top = elevation(objects, &v["top_level"]).or(above)?
                + number(v, "top_offset").unwrap_or(0.0);
            if !(top > z.0 + 1.0e-6 && base < z.1 - 1.0e-6) {
                return None;
            }
            let footprint = shape_of(v, objects)?.boundary(&|id| {
                objects
                    .iter()
                    .find(|o| o["type_key"] == WALL && o["id"] == id.to_string())
                    .and_then(wall_of)
            })?;
            footprint
                .flattened(archbuilder_model::floor::FLAT_CHORD)
                .into_iter()
                .next()
        })
        .collect()
}

/// The `floors` list of the dump's `objects`, in the dump's order.
#[must_use]
pub fn summary(objects: &[Value]) -> Vec<Value> {
    objects
        .iter()
        .filter(|o| o["type_key"] == FLOOR)
        .map(|object| {
            let values = &object["values"];
            let storey = objects
                .iter()
                .find(|s| s["type_key"] == STOREY && s["id"] == object["owner"])
                .and_then(|s| s["values"]["name"].as_str().map(str::to_owned));
            let floor = floor_of(values, objects);
            let boundary = floor.as_ref().and_then(|f| {
                f.boundary(&|id| {
                    objects
                        .iter()
                        .find(|o| o["type_key"] == WALL && o["id"] == id.to_string())
                        .and_then(wall_of)
                })
            });
            // The shaft openings of the stairs arriving at the floor's level are taken out.
            let mut openings = crate::stairs::openings_of_floor(objects, &object["owner"]);
            // Shafts pass through the slab like stairs do.
            if let (Some(f), Some(level)) = (
                floor.as_ref(),
                objects
                    .iter()
                    .find(|s| s["type_key"] == STOREY && s["id"] == object["owner"])
                    .and_then(|s| elevation(objects, &s["values"]["level"])),
            ) {
                let top = level + f.top_offset;
                openings.extend(shaft_cuts(objects, (top - f.thickness, top)));
            }
            let area = boundary
                .as_ref()
                .map(|b| b.with_openings(&openings).0.area());
            let walls: Vec<Value> = floor
                .as_ref()
                .map(Floor::walls)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|id| {
                    objects
                        .iter()
                        .find(|o| o["type_key"] == WALL && o["id"] == id.to_string())
                })
                .map(|w| w["values"]["name"].clone())
                .collect();
            json!({
                "id": object["id"],
                "name": values["name"],
                "storey": storey,
                "thickness": floor.as_ref().map(|f| f.thickness),
                "top_offset": floor.as_ref().map(|f| f.top_offset),
                "area": area,
                "volume": area.zip(floor.as_ref()).map(|(a, f)| a * f.thickness),
                "walls": walls,
            })
        })
        .collect()
}
