//! The location commands: `archbuilder.coordinates.set` (project base point, survey point, true
//! north and the geodetic reference), `archbuilder.coordinates.query` (a point in every system) and
//! `archbuilder.coordinates.recenter` (AB-DATUM-014, -015, -018, -019).
//!
//! The values live on the project record ([`archbuilder_model::coordinates`]). Moving the project
//! base point or the survey point, or turning true north, changes what is reported for a point and
//! never moves an element. Recentering is the one command that moves elements: it translates the
//! whole model, with the two points, so that the survey point lands on the internal origin; the
//! map coordinates of every element stay the same.

use arc_base::{Dimension, Error, ObjectId, Result};
use arc_command::prompt::{Accept, PointDims, PromptSequence, PromptStep};
use arc_command::{
    CommandSpec, Context, IdParam, Permission, QuantityParam, Registry, UndoBehavior,
};
use arc_doc::{FieldPath, Snapshot, Value};
use archbuilder_model::coordinates::{self, Coordinates};
use archbuilder_model::datum::Source;
use archbuilder_model::spatial;
use archbuilder_model::transform::Xform;
use archbuilder_model::types::{self, PROJECT, WORK_PLANE};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::codes;

/// The identifier of the set-location command.
pub const COORDINATES_SET: &str = "archbuilder.coordinates.set";
/// The identifier of the coordinate query command.
pub const COORDINATES_QUERY: &str = "archbuilder.coordinates.query";
/// The identifier of the recenter command.
pub const COORDINATES_RECENTER: &str = "archbuilder.coordinates.recenter";

/// Parameters of `archbuilder.coordinates.set`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetCoordinates {
    /// The project base point `[x, y]` or `[x, y, z]` in internal coordinates, in meters.
    #[serde(default)]
    #[schemars(length(min = 2, max = 3))]
    pub base_point: Option<Vec<f64>>,
    /// The survey point `[x, y]` or `[x, y, z]` in internal coordinates, in meters.
    #[serde(default)]
    #[schemars(length(min = 2, max = 3))]
    pub survey_point: Option<Vec<f64>>,
    /// The angle from project north to true north, counterclockwise: a number is radians, a text
    /// with a unit (`"12.5 deg"`) says its own.
    #[serde(default)]
    pub true_north: Option<QuantityParam>,
    /// The scale factor from model to map lengths (1 when absent).
    #[serde(default)]
    pub scale: Option<f64>,
    /// The coordinate reference system: an EPSG code such as `EPSG:3067` or a WKT definition;
    /// empty removes it.
    #[serde(default)]
    #[schemars(length(max = 16384))]
    pub crs: Option<String>,
    /// The eastings of the survey point in that system.
    #[serde(default)]
    pub eastings: Option<QuantityParam>,
    /// The northings of the survey point.
    #[serde(default)]
    pub northings: Option<QuantityParam>,
    /// The height of the survey point above the datum of that system.
    #[serde(default)]
    pub height: Option<QuantityParam>,
    /// The distance from the internal origin beyond which geometry is reported.
    #[serde(default)]
    pub precision_distance: Option<QuantityParam>,
    /// Removes the reference system and the map position of the survey point.
    #[serde(default)]
    pub clear_georeference: Option<bool>,
}

/// What `archbuilder.coordinates.set` returns: the location now.
#[derive(Debug, Serialize, JsonSchema)]
pub struct LocationSet {
    /// The project base point, in meters.
    pub base_point: [f64; 3],
    /// The survey point, in meters.
    pub survey_point: [f64; 3],
    /// The angle from project north to true north, in radians.
    pub true_north: f64,
    /// The scale factor.
    pub scale: f64,
    /// The coordinate reference system, when there is one.
    pub crs: Option<String>,
    /// Whether a reference system and the survey point's map position are both set.
    pub georeferenced: bool,
}

/// Parameters of `archbuilder.coordinates.query`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryCoordinates {
    /// The point `[x, y]` or `[x, y, z]` in internal coordinates, in meters; z is 0 when absent.
    #[schemars(length(min = 2, max = 3))]
    pub point: Vec<f64>,
}

/// What `archbuilder.coordinates.query` returns: the point in every system, in meters.
#[derive(Debug, Serialize, JsonSchema)]
pub struct CoordinateReading {
    /// Internal coordinates.
    pub internal: [f64; 3],
    /// Project coordinates: from the project base point.
    pub project: [f64; 3],
    /// Survey coordinates: from the survey point.
    pub survey: [f64; 3],
    /// Map coordinates (eastings, northings, height); absent without a georeference.
    pub map: Option<[f64; 3]>,
    /// The coordinate reference system the map coordinates are in.
    pub crs: Option<String>,
    /// The angle from project north to true north, in radians.
    pub true_north: f64,
}

/// What `archbuilder.coordinates.recenter` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Recentered {
    /// The elements moved.
    pub moved: usize,
    /// The displacement applied to the model, `[x, y]` in meters.
    pub offset: [f64; 2],
}

/// Parameters of `archbuilder.coordinates.recenter`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Recenter {
    /// The displacement `[x, y]` in meters; else the one that puts the survey point on the
    /// internal origin.
    #[serde(default)]
    pub offset: Option<[f64; 2]>,
}

fn point3(values: &[f64]) -> Result<[f64; 3]> {
    let (lo, hi) = types::COORDINATE_RANGE;
    let p = match values {
        [x, y] => [*x, *y, 0.0],
        [x, y, z] => [*x, *y, *z],
        _ => return Err(Error::new(codes::VALUE_INVALID)),
    };
    if p.iter().all(|v| v.is_finite() && *v >= lo && *v <= hi) {
        Ok(p)
    } else {
        Err(Error::new(codes::VALUE_INVALID))
    }
}

fn project_of(snapshot: &Snapshot) -> Result<ObjectId> {
    spatial::objects(snapshot, PROJECT)
        .first()
        .map(|p| p.id)
        .ok_or_else(|| Error::new(codes::KIND_MISMATCH))
}

fn meters(ctx: &Context<'_>, q: &QuantityParam, range: (f64, f64)) -> Result<f64> {
    let v = ctx.quantity(q, Dimension::LENGTH)?.value();
    if v.is_finite() && v >= range.0 && v <= range.1 {
        Ok(v)
    } else {
        Err(Error::new(codes::VALUE_INVALID))
    }
}

fn describe(c: &Coordinates) -> LocationSet {
    LocationSet {
        base_point: c.base,
        survey_point: c.survey,
        true_north: c.true_north,
        scale: c.scale,
        crs: c.crs.clone(),
        georeferenced: c.is_georeferenced(),
    }
}

fn set(ctx: &mut Context<'_>, p: SetCoordinates) -> Result<LocationSet> {
    let project = project_of(ctx.snapshot())?;
    let mut changes: Vec<(&str, Option<Value>)> = Vec::new();
    for (names, point) in [
        (["base_x", "base_y", "base_z"], &p.base_point),
        (["survey_x", "survey_y", "survey_z"], &p.survey_point),
    ] {
        if let Some(values) = point {
            let at = point3(values)?;
            // A point given in plan keeps the elevation it had.
            let take = if values.len() == 2 { 2 } else { 3 };
            for (name, v) in names.iter().zip(at).take(take) {
                changes.push((name, Some(types::length(v))));
            }
        }
    }
    if let Some(north) = &p.true_north {
        let radians = ctx.quantity(north, Dimension::ANGLE)?.value();
        if !radians.is_finite() {
            return Err(Error::new(codes::VALUE_INVALID));
        }
        // Kept within a half turn either way.
        let tau = std::f64::consts::TAU;
        let turned = (radians + std::f64::consts::PI).rem_euclid(tau) - std::f64::consts::PI;
        changes.push(("true_north", Some(types::angle(turned))));
    }
    if let Some(scale) = p.scale {
        let (lo, hi) = coordinates::SCALE_RANGE;
        if !scale.is_finite() || scale < lo || scale > hi {
            return Err(Error::new(codes::VALUE_INVALID));
        }
        changes.push(("map_scale", Some(Value::F64(scale))));
    }
    if let Some(text) = &p.crs {
        if text.len() as u64 > coordinates::MAX_CRS {
            return Err(Error::new(codes::VALUE_INVALID));
        }
        let text = text.trim();
        changes.push((
            "crs",
            (!text.is_empty()).then(|| Value::String(text.to_owned())),
        ));
    }
    for (name, q, range) in [
        ("eastings", &p.eastings, coordinates::MAP_RANGE),
        ("northings", &p.northings, coordinates::MAP_RANGE),
        ("survey_height", &p.height, types::ELEVATION_RANGE),
        (
            "precision_distance",
            &p.precision_distance,
            (1.0, types::COORDINATE_RANGE.1),
        ),
    ] {
        if let Some(q) = q {
            changes.push((name, Some(types::length(meters(ctx, q, range)?))));
        }
    }
    if p.clear_georeference == Some(true) {
        for name in ["crs", "eastings", "northings", "survey_height"] {
            changes.retain(|(n, _)| *n != name);
            changes.push((name, None));
        }
    }
    // Nothing to change selects the project, whose properties list the same fields.
    if changes.is_empty() {
        ctx.set_selection(vec![project])?;
        return Ok(describe(&Coordinates::of(ctx.snapshot())));
    }
    let txn = ctx.txn()?;
    for (field, value) in changes {
        txn.modify(project, FieldPath::new(field), value)?;
    }
    crate::structure::ensure_valid(ctx)?;
    Ok(describe(&Coordinates::of(ctx.snapshot())))
}

fn query(ctx: &mut Context<'_>, p: QueryCoordinates) -> Result<CoordinateReading> {
    let at = match p.point.as_slice() {
        [x, y] => [*x, *y, 0.0],
        [x, y, z] => [*x, *y, *z],
        _ => return Err(Error::new(codes::VALUE_INVALID)),
    };
    if !at.iter().all(|v| v.is_finite()) {
        return Err(Error::new(codes::VALUE_INVALID));
    }
    let c = Coordinates::of(ctx.snapshot());
    let r = c.read(at);
    Ok(CoordinateReading {
        internal: r.internal,
        project: r.project,
        survey: r.survey,
        map: r.map,
        crs: c.crs,
        true_north: c.true_north,
    })
}

/// Every element the recenter command moves.
fn movable(snapshot: &Snapshot) -> Vec<IdParam> {
    use types::{
        CEILING, FLOOR, GRID, RAILING, RAMP, REFERENCE_PLANE, ROOF, ROOF_EXTRUSION, ROOM, SHAFT,
        STAIR, VERTICAL_OPENING, WALL,
    };
    [
        WALL,
        FLOOR,
        ROOM,
        STAIR,
        RAMP,
        RAILING,
        ROOF,
        CEILING,
        SHAFT,
        VERTICAL_OPENING,
        ROOF_EXTRUSION,
        GRID,
        REFERENCE_PLANE,
    ]
    .into_iter()
    .flat_map(|key| spatial::objects(snapshot, key))
    .map(|o| IdParam(o.id))
    .collect()
}

fn recenter(ctx: &mut Context<'_>, p: Recenter) -> Result<Recentered> {
    let project = project_of(ctx.snapshot())?;
    let c = Coordinates::of(ctx.snapshot());
    let offset = p.offset.unwrap_or([-c.survey[0], -c.survey[1]]);
    if !offset.iter().all(|v| v.is_finite())
        || offset.iter().any(|v| v.abs() > types::COORDINATE_RANGE.1)
    {
        return Err(Error::new(codes::VALUE_INVALID));
    }
    let snapshot = ctx.snapshot().clone();
    let ids = movable(&snapshot);
    let mut moved = 0;
    if !ids.is_empty() && (offset[0] != 0.0 || offset[1] != 0.0) {
        let set = crate::modify::gather(ctx, Some(ids))?;
        moved = set.len();
        crate::modify::transform_in_place(ctx, &set, &Xform::Translate(offset))?;
        // A work plane through three points is authored in internal coordinates too.
        for plane in spatial::objects(&snapshot, WORK_PLANE) {
            if Source::of_values(&plane.values) != Source::Points {
                continue;
            }
            for k in 0..3 {
                for (axis, d) in [("x", offset[0]), ("y", offset[1])] {
                    let at = types::work_plane::POINTS + 3 * k + usize::from(axis == "y");
                    if let Some(v) = types::meters(&plane.values, at) {
                        crate::modify::set_length(
                            ctx,
                            plane.id,
                            &format!("p{}_{axis}", k + 1),
                            v + d,
                        )?;
                    }
                }
            }
        }
    }
    // The points travel with the model, so the map coordinates of every element stay.
    let txn = ctx.txn()?;
    for (names, at) in [
        (["base_x", "base_y"], c.base),
        (["survey_x", "survey_y"], c.survey),
    ] {
        for (name, (v, d)) in names.iter().zip(at.iter().zip(offset)) {
            txn.modify(project, FieldPath::new(*name), Some(types::length(v + d)))?;
        }
    }
    crate::structure::ensure_valid(ctx)?;
    Ok(Recentered { moved, offset })
}

/// A message of the catalog, named by identifier.
struct Msg {
    message_id: &'static str,
}

/// Registers the commands of this module.
///
/// # Errors
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    let label = |m: Msg| m.message_id;
    registry.register(CommandSpec::new(
        COORDINATES_SET,
        label(Msg {
            message_id: "ab-cmd-coordinates-set-label",
        }),
        label(Msg {
            message_id: "ab-cmd-coordinates-set-description",
        }),
        |ctx: &mut Context<'_>, p: SetCoordinates| set(ctx, p),
    ))?;
    registry.register(
        CommandSpec::new(
            COORDINATES_QUERY,
            label(Msg {
                message_id: "ab-cmd-coordinates-query-label",
            }),
            label(Msg {
                message_id: "ab-cmd-coordinates-query-description",
            }),
            |ctx: &mut Context<'_>, p: QueryCoordinates| query(ctx, p),
        )
        .with_prompts(
            PromptSequence::new().step(PromptStep::new(
                "point",
                Msg {
                    message_id: "ab-prompt-coordinates-point",
                }
                .message_id,
                Accept::Point {
                    dims: PointDims::Any,
                },
            )),
        )
        .with_undo(UndoBehavior::NotUndoable)
        .with_permissions([Permission::Read]),
    )?;
    registry.register(CommandSpec::new(
        COORDINATES_RECENTER,
        label(Msg {
            message_id: "ab-cmd-coordinates-recenter-label",
        }),
        label(Msg {
            message_id: "ab-cmd-coordinates-recenter-description",
        }),
        |ctx: &mut Context<'_>, p: Recenter| recenter(ctx, p),
    ))
}
