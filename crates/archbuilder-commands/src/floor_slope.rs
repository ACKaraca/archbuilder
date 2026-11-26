//! `archbuilder.floor.set_slope`, `set_slope_edge` and `set_slope_points`: the slope of a floor.
//!
//! A sloped floor stores a slope arrow like a ceiling does (authored data only): a start line
//! through `from`, square to the direction of the arrow, a direction and a length, and either a
//! slope angle or the top's height at the arrow's end. The floor's top offset is the height of the
//! top at the start line. The two other ways to give a slope are conveniences that write that
//! arrow: a boundary edge that stays where it is while the slab rises away from it, and the
//! elevations of the top at three points of the plan.

use arc_base::{Dimension, Error, ObjectId, Result};
use arc_command::{CommandSpec, Context, IdParam, QuantityParam, Registry};
use arc_doc::FieldPath;
use archbuilder_model::floor::{Floor, signed_area};
use archbuilder_model::spatial;
use archbuilder_model::types::{self, BASE_OFFSET_RANGE, FLOOR};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::ceiling::{clear_slope, slope_angle, write_slope};
use crate::error::codes;
use crate::structure::{ensure_valid, expect};

/// The identifier of the command that gives a floor a slope arrow, or takes it away.
pub const FLOOR_SET_SLOPE: &str = "archbuilder.floor.set_slope";
/// The identifier of the command that slopes a floor away from one of its edges.
pub const FLOOR_SET_SLOPE_EDGE: &str = "archbuilder.floor.set_slope_edge";
/// The identifier of the command that slopes a floor through three elevations.
pub const FLOOR_SET_SLOPE_POINTS: &str = "archbuilder.floor.set_slope_points";

/// Parameters of `archbuilder.floor.set_slope`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetSlope {
    /// The floor; else the selected floor.
    #[serde(default)]
    pub floor: Option<IdParam>,
    /// A point of the arrow's start line.
    #[serde(default)]
    pub from: Option<[f64; 2]>,
    /// The end of the arrow.
    #[serde(default)]
    pub to: Option<[f64; 2]>,
    /// The slope, an angle, or text in the project's slope unit.
    #[serde(default)]
    pub slope: Option<QuantityParam>,
    /// The height of the top above the storey's level at the arrow's end.
    #[serde(default)]
    pub end_height: Option<QuantityParam>,
    /// Takes the slope away: the floor is level again.
    #[serde(default)]
    pub clear: Option<bool>,
}

/// Parameters of `archbuilder.floor.set_slope_edge`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetSlopeEdge {
    /// The floor; else the selected floor.
    #[serde(default)]
    pub floor: Option<IdParam>,
    /// The edge of the outer loop, by the number of the vertex it starts at. It stays at the
    /// top offset; the slab rises from it into the floor.
    pub edge: usize,
    /// The slope, an angle, or text in the project's slope unit.
    pub slope: QuantityParam,
}

/// Parameters of `archbuilder.floor.set_slope_points`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetSlopePoints {
    /// The floor; else the selected floor.
    #[serde(default)]
    pub floor: Option<IdParam>,
    /// Three points of the plan, not on one line, each `[x, y, z]`: z is the height of the top
    /// above the storey's level there, in meters.
    pub points: [[f64; 3]; 3],
}

fn floor_of(ctx: &Context<'_>, given: Option<IdParam>) -> Result<ObjectId> {
    let id = match given {
        Some(IdParam(id)) => id,
        None => {
            let snapshot = ctx.snapshot();
            ctx.selection()
                .iter()
                .copied()
                .find(|id| spatial::type_of(snapshot, *id).as_deref() == Some(FLOOR))
                .ok_or_else(|| Error::new(codes::NO_FLOOR_SELECTED))?
        }
    };
    expect(ctx.snapshot(), id, FLOOR)?;
    Ok(id)
}

fn set_slope(ctx: &mut Context<'_>, p: SetSlope) -> Result<IdParam> {
    let id = floor_of(ctx, p.floor)?;
    if p.clear.unwrap_or(false) {
        clear_slope(ctx, id)?;
    } else {
        let (Some(from), Some(to)) = (p.from, p.to) else {
            return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
        };
        let slope = p.slope.as_ref().map(|s| slope_angle(ctx, s)).transpose()?;
        let end = p
            .end_height
            .as_ref()
            .map(|h| ctx.quantity(h, Dimension::LENGTH).map(|q| q.value()))
            .transpose()?;
        write_slope(ctx, id, from, to, slope, end)?;
    }
    ensure_valid(ctx)?;
    ctx.set_selection_after(vec![id]);
    Ok(IdParam(id))
}

fn set_slope_edge(ctx: &mut Context<'_>, p: SetSlopeEdge) -> Result<IdParam> {
    let id = floor_of(ctx, p.floor)?;
    let slope = slope_angle(ctx, &p.slope)?;
    let object = expect(ctx.snapshot(), id, FLOOR)?;
    let floor =
        Floor::from_values(&object.values).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let boundary = floor
        .boundary_in(ctx.snapshot())
        .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let outer = boundary
        .loops
        .first()
        .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let (a, b) = match (
        outer.get(p.edge),
        outer.get((p.edge + 1) % outer.len().max(1)),
    ) {
        (Some(a), Some(b)) => (*a, *b),
        _ => return Err(Error::new(arc_command::error::codes::PARAMS_INVALID)),
    };
    let len = (b[0] - a[0]).hypot(b[1] - a[1]);
    if len < types::MIN_WALL_LENGTH {
        return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
    }
    // The inward normal: the left of the edge for a counterclockwise loop, the right otherwise.
    let side = if signed_area(outer) >= 0.0 { 1.0 } else { -1.0 };
    let n = [-(b[1] - a[1]) / len * side, (b[0] - a[0]) / len * side];
    // The arrow crosses the whole floor along the normal.
    let reach = outer
        .iter()
        .map(|q| (q[0] - a[0]) * n[0] + (q[1] - a[1]) * n[1])
        .fold(0.0_f64, f64::max)
        .max(0.001);
    let from = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    let to = [from[0] + n[0] * reach, from[1] + n[1] * reach];
    write_slope(ctx, id, from, to, Some(slope), None)?;
    ensure_valid(ctx)?;
    ctx.set_selection_after(vec![id]);
    Ok(IdParam(id))
}

fn set_slope_points(ctx: &mut Context<'_>, p: SetSlopePoints) -> Result<IdParam> {
    let id = floor_of(ctx, p.floor)?;
    let [a, b, c] = p.points;
    if !p.points.iter().flatten().all(|v| v.is_finite())
        || !p.points.iter().all(|q| {
            q[0].abs() <= types::COORDINATE_RANGE.1
                && q[1].abs() <= types::COORDINATE_RANGE.1
                && (BASE_OFFSET_RANGE.0..=BASE_OFFSET_RANGE.1).contains(&q[2])
        })
    {
        return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
    }
    // The plane z = a.z + gx (x - a.x) + gy (y - a.y) through the three points.
    let (u, v) = (
        [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
        [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
    );
    let det = u[0] * v[1] - u[1] * v[0];
    if det.abs() < 1.0e-9 {
        return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
    }
    let gx = (u[2] * v[1] - u[1] * v[2]) / det;
    let gy = (u[0] * v[2] - u[2] * v[0]) / det;
    let g = gx.hypot(gy);
    let txn = ctx.txn()?;
    txn.modify(id, FieldPath::new("top_offset"), Some(types::length(a[2])))?;
    if g < 1.0e-9 {
        clear_slope(ctx, id)?;
    } else {
        let from = [a[0], a[1]];
        let to = [a[0] + gx / g, a[1] + gy / g];
        write_slope(ctx, id, from, to, Some(g.atan()), None)?;
    }
    ensure_valid(ctx)?;
    ctx.set_selection_after(vec![id]);
    Ok(IdParam(id))
}

/// Gives the floor `id` the slope `text` names (an angle, a percentage or a rise over a run in the
/// project's unit): on its slope arrow, or, when it has none, on a new arrow along x across its
/// boundary, rising toward +x. An empty text, or a slope of zero, takes the slope away.
pub(crate) fn set_slope_text(ctx: &mut Context<'_>, id: ObjectId, text: &str) -> Result<()> {
    let text = text.trim();
    let flat = text.is_empty()
        || text
            .replace(['°', '%'], "")
            .trim()
            .parse::<f64>()
            .is_ok_and(|v| v == 0.0);
    if flat {
        return clear_slope(ctx, id);
    }
    let slope = crate::roof::parse_slope(ctx.snapshot(), text)?;
    let object = expect(ctx.snapshot(), id, FLOOR)?;
    let floor =
        Floor::from_values(&object.values).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let arrow = archbuilder_model::ceiling::Arrow::read(&object.values, types::floor::ARROW);
    let (from, to) = match arrow {
        Some(a) => (a.start, a.end()),
        None => {
            let boundary = floor
                .boundary_in(ctx.snapshot())
                .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
            let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
            for q in boundary.loops.iter().flatten() {
                lo = [lo[0].min(q[0]), lo[1].min(q[1])];
                hi = [hi[0].max(q[0]), hi[1].max(q[1])];
            }
            let y = (lo[1] + hi[1]) / 2.0;
            ([lo[0], y], [hi[0].max(lo[0] + 1.0), y])
        }
    };
    write_slope(ctx, id, from, to, Some(slope), None)
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
    registry.register(CommandSpec::new(
        FLOOR_SET_SLOPE,
        Msg {
            message_id: "ab-cmd-floor-slope-label",
        }
        .message_id,
        Msg {
            message_id: "ab-cmd-floor-slope-description",
        }
        .message_id,
        |ctx: &mut Context<'_>, p: SetSlope| set_slope(ctx, p),
    ))?;
    registry.register(CommandSpec::new(
        FLOOR_SET_SLOPE_EDGE,
        Msg {
            message_id: "ab-cmd-floor-slope-edge-label",
        }
        .message_id,
        Msg {
            message_id: "ab-cmd-floor-slope-edge-description",
        }
        .message_id,
        |ctx: &mut Context<'_>, p: SetSlopeEdge| set_slope_edge(ctx, p),
    ))?;
    registry.register(CommandSpec::new(
        FLOOR_SET_SLOPE_POINTS,
        Msg {
            message_id: "ab-cmd-floor-slope-points-label",
        }
        .message_id,
        Msg {
            message_id: "ab-cmd-floor-slope-points-description",
        }
        .message_id,
        |ctx: &mut Context<'_>, p: SetSlopePoints| set_slope_points(ctx, p),
    ))
}
