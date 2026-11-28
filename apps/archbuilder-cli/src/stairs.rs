//! The stairs of an `inspect` dump: for each stair record its storey, shape, riser count and
//! height, run lengths, stride and volume, derived from the dump's records with the model's own
//! rules ([`archbuilder_model::stair`]), so the CLI shows what the application shows. The shaft
//! openings feed the floors' areas ([`openings_of_floor`]); a stride outside the comfortable range
//! and overlapping stairs are warnings of `inspect --check`.

use archbuilder_model::stair::{Layout, Refusal, Shape, Stair, Turn};
use archbuilder_model::types::{HEIGHT_RANGE, LEVEL, STAIR, STOREY};
use serde_json::{Value, json};

/// Whether a stored enum (a name or a code) is the variant `name` with `code`.
fn named(value: &Value, code: u32, name: &str) -> bool {
    match value {
        Value::String(s) => s == name,
        Value::Number(n) => n.as_u64() == Some(u64::from(code)),
        _ => false,
    }
}

fn stair_of(values: &Value) -> Option<Stair> {
    let n = |f: &str| values.get(f)?.as_f64();
    Some(Stair {
        shape: Shape::ALL
            .into_iter()
            .find(|s| named(&values["shape"], s.code(), s.name()))?,
        turn: Turn::ALL
            .into_iter()
            .find(|s| named(&values["turn"], s.code(), s.name()))?,
        start: [n("x")?, n("y")?],
        direction: n("direction")?,
        width: n("width")?,
        tread: n("tread")?,
        max_riser: n("max_riser")?,
        waist: n("waist")?,
        railings: [
            values["railing_left"].as_bool() != Some(false),
            values["railing_right"].as_bool() != Some(false),
        ],
        top_level: values["top_level"].as_str().and_then(|s| s.parse().ok()),
    })
}

/// The run lines of a stair by sketch: records `[x0, y0, x1, y1]` in meters.
fn run_lines_of(values: &Value) -> Vec<archbuilder_model::stair::RunLine> {
    let n = |r: &Value, k: usize| r.get(k)?.as_f64();
    values["runs"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|r| {
                    Some(archbuilder_model::stair::RunLine {
                        from: [n(r, 0)?, n(r, 1)?],
                        to: [n(r, 2)?, n(r, 3)?],
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn level_elevation(objects: &[Value], id: &Value) -> Option<f64> {
    objects
        .iter()
        .find(|o| o["type_key"] == LEVEL && o["id"] == *id)?["values"]["elevation"]
        .as_f64()
}

/// A stair of the dump: its record, the storey's name, the top level and the layout.
struct Row<'a> {
    object: &'a Value,
    storey: Value,
    top: Option<Value>,
    layout: Result<Layout, Refusal>,
}

fn rows(objects: &[Value]) -> Vec<Row<'_>> {
    let mut out = Vec::new();
    for object in objects.iter().filter(|o| o["type_key"] == STAIR) {
        let storey = objects
            .iter()
            .find(|s| s["type_key"] == STOREY && s["id"] == object["owner"]);
        let base_level = storey.map(|s| s["values"]["level"].clone());
        let base = base_level
            .as_ref()
            .and_then(|l| level_elevation(objects, l))
            .unwrap_or(0.0);
        let Some(stair) = stair_of(&object["values"]) else {
            continue;
        };
        let top = if object["values"]["top_level"].is_string() {
            Some(object["values"]["top_level"].clone())
        } else {
            objects
                .iter()
                .filter(|s| s["type_key"] == STOREY)
                .filter_map(|s| {
                    let e = level_elevation(objects, &s["values"]["level"])?;
                    (e > base + HEIGHT_RANGE.0).then(|| (e, s["values"]["level"].clone()))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, l)| l)
        };
        let layout = match top.as_ref().and_then(|l| level_elevation(objects, l)) {
            Some(e) => Layout::build(&stair, &run_lines_of(&object["values"]), e - base),
            None => Err(Refusal::NoTop),
        };
        out.push(Row {
            object,
            storey: storey.map_or(Value::Null, |s| s["values"]["name"].clone()),
            top,
            layout,
        });
    }
    out
}

/// The shaft openings of the stairs that arrive at the level of the storey `storey` (an id).
#[must_use]
pub fn openings_of_floor(objects: &[Value], storey: &Value) -> Vec<Vec<[f64; 2]>> {
    let Some(level) = objects
        .iter()
        .find(|s| s["type_key"] == STOREY && s["id"] == *storey)
        .map(|s| s["values"]["level"].clone())
    else {
        return Vec::new();
    };
    rows(objects)
        .into_iter()
        .filter(|r| r.top.as_ref() == Some(&level))
        .filter_map(|r| r.layout.ok())
        .map(|l| l.opening())
        .collect()
}

/// The `stairs` list of the dump's `objects`, in the dump's order.
#[must_use]
pub fn summary(objects: &[Value]) -> Vec<Value> {
    rows(objects)
        .into_iter()
        .map(|r| {
            let v = &r.object["values"];
            let base = json!({
                "id": r.object["id"], "name": v["name"], "storey": r.storey,
                "shape": v["shape"], "turn": v["turn"], "width": v["width"], "tread": v["tread"],
            });
            let mut map = base.as_object().cloned().unwrap_or_default();
            match &r.layout {
                Ok(l) => {
                    map.insert("risers".into(), l.risers.into());
                    map.insert("riser".into(), l.riser.into());
                    map.insert(
                        "runs".into(),
                        l.runs
                            .iter()
                            .map(|run| run.length)
                            .collect::<Vec<_>>()
                            .into(),
                    );
                    map.insert("stride".into(), l.stride().into());
                    map.insert("stride_ok".into(), l.stride_ok().into());
                    map.insert("volume".into(), l.volume().into());
                }
                Err(e) => {
                    map.insert("error".into(), format!("{e:?}").into());
                }
            }
            Value::Object(map)
        })
        .collect()
}

/// The findings about stairs, in the form of the shared check: a stride outside the range and
/// two stairs of one storey that overlap in plan. Each names the stair's identifier as `object`.
#[must_use]
pub fn findings(objects: &[Value]) -> Vec<Value> {
    let (low, high) = archbuilder_model::stair::STRIDE_RANGE;
    let rows = rows(objects);
    let mut out = Vec::new();
    for (k, r) in rows.iter().enumerate() {
        let Ok(l) = &r.layout else {
            continue;
        };
        if !l.stride_ok() {
            out.push(json!({
                "severity": "warning",
                "code": "ab.model.stair_stride",
                "message_id": "ab-model-stair-stride",
                "object": r.object["id"],
                "arguments": {
                    "name": r.object["values"]["name"],
                    "stride": format!("{:.3}", l.stride()),
                    "low": format!("{low:.2}"), "high": format!("{high:.2}"),
                },
            }));
        }
        for other in &rows[k + 1..] {
            let Ok(o) = &other.layout else {
                continue;
            };
            if other.object["owner"] == r.object["owner"]
                && archbuilder_model::stair::outlines_overlap(&l.footprint(), &o.footprint())
            {
                out.push(json!({
                    "severity": "warning",
                    "code": "ab.model.stair_overlap",
                    "message_id": "ab-model-stair-overlap",
                    "object": r.object["id"],
                    "arguments": {
                        "name": r.object["values"]["name"],
                        "other": other.object["values"]["name"],
                    },
                }));
            }
        }
    }
    out
}
