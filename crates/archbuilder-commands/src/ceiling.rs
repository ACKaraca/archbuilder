//! `archbuilder.ceiling.create` and `archbuilder.ceiling.set_slope`: ceilings on a storey.
//!
//! A ceiling is made like a floor: from `picks` (a click inside a room, or on walls, takes the
//! loop of walls that bound rooms, so the boundary is the room's region at the wall faces), from
//! `walls` or from a sketch (`points`, with `holes`). Its height is `offset`, the distance of its
//! underside above the storey's level; the type's height, 2.6 m by default, when absent. A slope
//! arrow is `slope_from` (a point of the start line) and `slope_to` (the arrow's end: it gives the
//! direction and the length) with either `slope` (an angle) or `end_height` (the height of the
//! underside at the arrow's end). One call is one transaction, one undo step. The boundary of an
//! existing ceiling is edited with `archbuilder.floor.edit_boundary`.

use std::sync::Arc;

use arc_base::{Dimension, Error, ObjectId, Result};
use arc_command::{CommandSpec, Context, IdParam, QuantityParam, Registry};
use arc_doc::{FieldPath, Snapshot, Value};
use archbuilder_model::ceiling::{self, Ceiling, MAX_SLOPE};
use archbuilder_model::element_type::{CeilingType, TypeKind};
use archbuilder_model::floor::Face;
use archbuilder_model::transform::Xform;
use archbuilder_model::types::{self, CEILING, DEFAULT_CEILING_HEIGHT};
use archbuilder_model::{defaults, names, spatial};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::codes;
use crate::floor::{CreateFloor, FaceParam, FloorPrompts, Subject};
use crate::structure::{create, ensure_valid, expect, language, named};

/// The identifier of the create-ceiling command.
pub const CEILING_CREATE: &str = "archbuilder.ceiling.create";
/// The identifier of the command that gives a ceiling a slope arrow, or takes it away.
pub const CEILING_SET_SLOPE: &str = "archbuilder.ceiling.set_slope";

/// Parameters of `archbuilder.ceiling.create`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateCeiling {
    /// The storey; else the storey of the selection, else the lowest storey.
    #[serde(default)]
    pub storey: Option<IdParam>,
    /// A sketched outer boundary: points `[x, y]` in meters, in order, closed implicitly.
    #[serde(default)]
    pub points: Option<Vec<[f64; 2]>>,
    /// Holes for a sketched ceiling: each a list of points.
    #[serde(default)]
    pub holes: Option<Vec<Vec<[f64; 2]>>>,
    /// Arc edges of the sketch: pairs of the number of the vertex the edge starts at and its bulge.
    #[serde(default)]
    pub arcs: Option<Vec<[f64; 2]>>,
    /// Walls of the storey in order around the area, each meeting the next.
    #[serde(default)]
    pub walls: Option<Vec<IdParam>>,
    /// Points that pick walls or, a single one inside a closed loop of walls, the room around it.
    #[serde(default)]
    pub picks: Option<Vec<[f64; 2]>>,
    /// The face the edges of picked walls follow; the interior face when absent.
    #[serde(default)]
    pub face: Option<FaceParam>,
    /// The offset of the edges from the wall face, in meters; 0 when absent.
    #[serde(default)]
    pub edge_offset: Option<QuantityParam>,
    /// The ceiling type: its identifier or name; else the document's first ceiling type.
    #[serde(default, rename = "type")]
    #[schemars(length(max = 256))]
    pub ceiling_type: Option<String>,
    /// The thickness. One other than the type's gives the ceiling a type of one layer of that
    /// thickness (an existing one, else a new one).
    #[serde(default)]
    pub thickness: Option<QuantityParam>,
    /// The height of the underside above the storey's level; the type's when absent.
    #[serde(default)]
    pub offset: Option<QuantityParam>,
    /// A point of the slope arrow's start line.
    #[serde(default)]
    pub slope_from: Option<[f64; 2]>,
    /// The end of the slope arrow.
    #[serde(default)]
    pub slope_to: Option<[f64; 2]>,
    /// The slope, an angle, or text in the project's slope unit.
    #[serde(default)]
    pub slope: Option<QuantityParam>,
    /// The height of the underside above the level at the arrow's end.
    #[serde(default)]
    pub end_height: Option<QuantityParam>,
    /// The name; else the first free `Ceiling <n>` of the storey.
    #[serde(default)]
    pub name: Option<String>,
    /// A prompt aid: the mode the prompts start in (`walls` or `sketch`). The command ignores it.
    #[serde(default)]
    pub mode: Option<String>,
}

/// What `archbuilder.ceiling.create` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct CeilingCreated {
    /// The new ceiling.
    pub ceiling: IdParam,
    /// Its storey.
    pub storey: IdParam,
    /// The area in square meters.
    pub area: f64,
    /// The height of the underside above the level at the start line, in meters.
    pub height: f64,
    /// The thickness in meters.
    pub thickness: f64,
}

/// Parameters of `archbuilder.ceiling.set_slope`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetSlope {
    /// The ceiling; else the selected ceiling.
    #[serde(default)]
    pub ceiling: Option<IdParam>,
    /// A point of the arrow's start line.
    #[serde(default)]
    pub from: Option<[f64; 2]>,
    /// The end of the arrow.
    #[serde(default)]
    pub to: Option<[f64; 2]>,
    /// The slope, an angle, or text in the project's slope unit.
    #[serde(default)]
    pub slope: Option<QuantityParam>,
    /// The height of the underside above the level at the arrow's end.
    #[serde(default)]
    pub end_height: Option<QuantityParam>,
    /// Takes the slope away: the ceiling is flat again.
    #[serde(default)]
    pub clear: Option<bool>,
}

/// The slope in radians a parameter gives: an angle, or text in the project's slope unit.
fn slope_angle(ctx: &Context<'_>, param: &QuantityParam) -> Result<f64> {
    let value = match param {
        QuantityParam::Text(t) => crate::roof::parse_slope(ctx.snapshot(), t)?,
        QuantityParam::Si(_) => ctx.quantity(param, Dimension::ANGLE)?.value(),
    };
    if value.is_finite() && value.abs() <= MAX_SLOPE {
        Ok(value)
    } else {
        Err(crate::roof::slope_invalid(ctx.snapshot()))
    }
}

/// Writes the slope fields of ceiling `id`: the arrow from `from` to `to` and its rise.
fn write_slope(
    ctx: &mut Context<'_>,
    id: ObjectId,
    from: [f64; 2],
    to: [f64; 2],
    slope: Option<f64>,
    end_height: Option<f64>,
) -> Result<()> {
    let length = (to[0] - from[0]).hypot(to[1] - from[1]);
    // The slope the arrow gives, whichever way it is given, stays within the limit.
    let height = spatial::get(ctx.txn()?.snapshot(), id)
        .and_then(|o| types::meters(&o.values, types::ceiling::HEIGHT_OFFSET))
        .unwrap_or(0.0);
    let angle = match (slope, end_height) {
        (Some(a), _) => a,
        (None, Some(h)) => ((h - height) / length.max(1.0e-9)).atan(),
        (None, None) => 0.0,
    };
    if !angle.is_finite() || angle.abs() > MAX_SLOPE + 1.0e-9 {
        return Err(crate::roof::slope_invalid(ctx.snapshot()));
    }
    if length < archbuilder_model::types::MIN_WALL_LENGTH
        || (slope.is_none() && end_height.is_none())
        || ![from[0], from[1], to[0], to[1]]
            .iter()
            .all(|v| v.is_finite())
    {
        return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
    }
    let direction = (to[1] - from[1]).atan2(to[0] - from[0]);
    let txn = ctx.txn()?;
    for (field, value) in [
        ("slope_x", Some(types::length(from[0]))),
        ("slope_y", Some(types::length(from[1]))),
        ("slope_direction", Some(types::angle(direction))),
        ("slope_length", Some(types::length(length))),
        ("slope", slope.map(types::angle)),
        ("end_height", end_height.map(types::length)),
    ] {
        txn.modify(id, FieldPath::new(field), value)?;
    }
    Ok(())
}

fn clear_slope(ctx: &mut Context<'_>, id: ObjectId) -> Result<()> {
    let txn = ctx.txn()?;
    for field in ["slope_x", "slope_y", "slope_length", "slope", "end_height"] {
        txn.modify(id, FieldPath::new(field), None)?;
    }
    txn.modify(
        id,
        FieldPath::new("slope_direction"),
        Some(types::angle(0.0)),
    )
}

pub(crate) fn next_ceiling_name(snapshot: &Snapshot, storey: ObjectId, language: &str) -> String {
    let used = spatial::children(snapshot, CEILING, storey);
    let taken = |name: &str| {
        used.iter()
            .any(|f| types::text(&f.values, 0).is_some_and(|n| names::same(n, name)))
    };
    (used.len() + 1..)
        .map(|n| defaults::ceiling(language, n))
        .find(|name| !taken(name))
        .unwrap_or_default()
}

fn create_ceiling(ctx: &mut Context<'_>, p: CreateCeiling) -> Result<CeilingCreated> {
    let storey = crate::wall::storey_for(ctx, p.storey)?.id;
    crate::element_type::ensure_ceiling_types(ctx)?;
    let meters = |ctx: &Context<'_>, param: &Option<QuantityParam>| match param {
        Some(param) => crate::length(ctx, param).map(|q| Some(q.value())),
        None => Ok(None),
    };
    let chosen = crate::element_type::layered_for(
        ctx,
        TypeKind::Ceiling,
        p.ceiling_type.as_deref(),
        p.thickness.as_ref(),
    )?;
    let type_height = spatial::get(ctx.txn()?.snapshot(), chosen)
        .and_then(|t| CeilingType::from_values(&t.values))
        .map_or(DEFAULT_CEILING_HEIGHT, |t| t.height_offset);
    let height = meters(ctx, &p.offset)?.unwrap_or(type_height);
    let edge_offset = meters(ctx, &p.edge_offset)?.unwrap_or(0.0);
    let face = p.face.map_or(Face::Interior, Into::into);
    // The boundary is asked for like a floor's; a ceiling follows the walls that bound rooms.
    let shape = CreateFloor {
        storey: None,
        points: p.points,
        holes: p.holes,
        walls: p.walls,
        picks: p.picks,
        face: p.face,
        edge_offset: None,
        floor_type: None,
        thickness: None,
        offset: None,
        name: None,
        mode: None,
    };
    let walls = crate::floor::picked_walls(ctx, storey, &shape, true)?;
    let vertices = crate::opening::with_arcs(
        crate::floor::boundary_vertices(&walls, &shape, face, edge_offset)?,
        p.arcs.as_deref(),
    );
    let slope = match (&p.slope, &p.end_height) {
        (Some(s), _) => Some(slope_angle(ctx, s)?),
        _ => None,
    };
    let end_height = meters(ctx, &p.end_height)?;
    let name = match p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => name.to_owned(),
        None => next_ceiling_name(ctx.snapshot(), storey, &language(ctx)),
    };
    let id = create(
        ctx,
        CEILING,
        Some(storey),
        vec![
            Some(named(&name)),
            Some(types::length(ceiling::DEFAULT_THICKNESS)),
            Some(types::length(height)),
            Some(archbuilder_model::floor::Floor::vertex_values(&vertices)),
            None,
            None,
            None,
            Some(types::length(0.0)),
            Some(types::length(0.0)),
            Some(types::angle(0.0)),
            None,
            None,
            Some(types::angle(0.0)),
        ],
    )?;
    crate::floor::set_vertices(ctx, id, &vertices)?;
    crate::element_type::assign(ctx, id, TypeKind::Ceiling, chosen)?;
    if let (Some(from), Some(to)) = (p.slope_from, p.slope_to) {
        write_slope(ctx, id, from, to, slope, end_height)?;
    } else if p.slope_from.is_some() || p.slope_to.is_some() {
        return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
    }
    ensure_valid(ctx)?;
    let txn = ctx.txn()?;
    let found = spatial::get(txn.snapshot(), id);
    let area = found
        .as_ref()
        .and_then(|o| ceiling::derived(txn.snapshot(), o))
        .map_or(0.0, |d| d.area());
    let thickness = found
        .as_ref()
        .and_then(|o| Ceiling::from_values(&o.values))
        .map_or(ceiling::DEFAULT_THICKNESS, |c| c.thickness());
    ctx.set_selection_after(vec![id]);
    Ok(CeilingCreated {
        ceiling: IdParam(id),
        storey: IdParam(storey),
        area,
        height,
        thickness,
    })
}

fn set_slope(ctx: &mut Context<'_>, p: SetSlope) -> Result<IdParam> {
    let id = match p.ceiling {
        Some(IdParam(id)) => id,
        None => {
            let snapshot = ctx.snapshot();
            ctx.selection()
                .iter()
                .copied()
                .find(|id| spatial::type_of(snapshot, *id).as_deref() == Some(CEILING))
                .ok_or_else(|| Error::new(codes::NO_CEILING_SELECTED))?
        }
    };
    expect(ctx.snapshot(), id, CEILING)?;
    if p.clear.unwrap_or(false) {
        clear_slope(ctx, id)?;
    } else {
        let (Some(from), Some(to)) = (p.from, p.to) else {
            return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
        };
        let slope = match &p.slope {
            Some(s) => Some(slope_angle(ctx, s)?),
            None => None,
        };
        let end = match &p.end_height {
            Some(h) => Some(crate::length(ctx, h)?.value()),
            None => None,
        };
        write_slope(ctx, id, from, to, slope, end)?;
    }
    ensure_valid(ctx)?;
    ctx.set_selection_after(vec![id]);
    Ok(IdParam(id))
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
    registry.register(
        CommandSpec::new(
            CEILING_CREATE,
            Msg {
                message_id: "ab-cmd-ceiling-create-label",
            }
            .message_id,
            Msg {
                message_id: "ab-cmd-ceiling-create-description",
            }
            .message_id,
            |ctx: &mut Context<'_>, p: CreateCeiling| create_ceiling(ctx, p),
        )
        .with_prompt(Arc::new(FloorPrompts::of(Subject::Ceiling))),
    )?;
    registry.register(CommandSpec::new(
        CEILING_SET_SLOPE,
        Msg {
            message_id: "ab-cmd-ceiling-slope-label",
        }
        .message_id,
        Msg {
            message_id: "ab-cmd-ceiling-slope-description",
        }
        .message_id,
        |ctx: &mut Context<'_>, p: SetSlope| set_slope(ctx, p),
    ))
}

/// The grid origin and rotation and the slope arrow of the ceiling values `values` under `xf`.
pub(crate) fn moved_values(values: &mut [Option<Value>], xf: &Xform) {
    use types::ceiling as c;
    let m = |values: &[Option<Value>], at| types::meters(values, at);
    let angle = |values: &[Option<Value>], at| match values.get(at) {
        Some(Some(Value::Quantity(q))) => q.value(),
        _ => 0.0,
    };
    let toward = |from: [f64; 2], theta: f64| {
        let (a, b) = (
            xf.point(from),
            xf.point([from[0] + theta.cos(), from[1] + theta.sin()]),
        );
        (b[1] - a[1]).atan2(b[0] - a[0])
    };
    let origin = [
        m(values, c::GRID_X).unwrap_or(0.0),
        m(values, c::GRID_Y).unwrap_or(0.0),
    ];
    let rotation = toward(origin, angle(values, c::GRID_ROTATION));
    let at = xf.point(origin);
    let slope = match (m(values, c::SLOPE_X), m(values, c::SLOPE_Y)) {
        (Some(x), Some(y)) => {
            let direction = toward([x, y], angle(values, c::SLOPE_DIRECTION));
            Some((xf.point([x, y]), direction))
        }
        _ => None,
    };
    let mut put = |slot: usize, value: Value| {
        if let Some(v) = values.get_mut(slot) {
            *v = Some(value);
        }
    };
    put(c::GRID_X, types::length(at[0]));
    put(c::GRID_Y, types::length(at[1]));
    put(c::GRID_ROTATION, types::angle(rotation));
    if let Some((p, direction)) = slope {
        put(c::SLOPE_X, types::length(p[0]));
        put(c::SLOPE_Y, types::length(p[1]));
        put(c::SLOPE_DIRECTION, types::angle(direction));
    }
}

/// Moves the ceilings `ids` in place by `xf`; an edge keeps its wall only when the wall is in
/// `walls` (it moves too). Returns the number of edges that lost their wall.
pub(crate) fn move_ceilings(
    ctx: &mut Context<'_>,
    snapshot: &Snapshot,
    ids: &[ObjectId],
    xf: &Xform,
    walls: &std::collections::BTreeSet<ObjectId>,
) -> Result<usize> {
    let mut lost = 0;
    for id in ids {
        let object = spatial::get(snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let c =
            Ceiling::from_values(&object.values).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let boundary = c.shape.boundary_in(snapshot);
        let (vertices, dropped) =
            crate::modify::floor_vertices(&c.shape, boundary.as_ref(), xf, &|w| {
                walls.contains(&w).then_some(w)
            });
        lost += dropped;
        crate::floor::set_vertices(ctx, *id, &vertices)?;
        let mut values = object.values.clone();
        moved_values(&mut values, xf);
        let txn = ctx.txn()?;
        for (field, at) in [
            ("grid_x", types::ceiling::GRID_X),
            ("grid_y", types::ceiling::GRID_Y),
            ("grid_rotation", types::ceiling::GRID_ROTATION),
            ("slope_x", types::ceiling::SLOPE_X),
            ("slope_y", types::ceiling::SLOPE_Y),
            ("slope_direction", types::ceiling::SLOPE_DIRECTION),
        ] {
            if values.get(at) != object.values.get(at) {
                txn.modify(
                    *id,
                    FieldPath::new(field),
                    values.get(at).cloned().flatten(),
                )?;
            }
        }
    }
    Ok(lost)
}

/// Copies the ceilings `ids` moved by `xf` into `target` (else their own storey): the new
/// ceilings with the originals they came from, and the edges that lost their wall.
pub(crate) fn copy_ceilings(
    ctx: &mut Context<'_>,
    snapshot: &Snapshot,
    ids: &[ObjectId],
    xf: &Xform,
    keep: &dyn Fn(ObjectId) -> Option<ObjectId>,
    target: Option<ObjectId>,
) -> Result<(Vec<(ObjectId, ObjectId)>, usize)> {
    let tongue = language(ctx);
    let mut out = Vec::new();
    let mut lost = 0;
    for id in ids {
        let object = spatial::get(snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let c =
            Ceiling::from_values(&object.values).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let boundary = c.shape.boundary_in(snapshot);
        let (vertices, dropped) =
            crate::modify::floor_vertices(&c.shape, boundary.as_ref(), xf, keep);
        lost += dropped;
        let storey = target
            .or(object.header.owner)
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let seen = ctx.txn()?.snapshot().clone();
        let mut values = object.values.clone();
        moved_values(&mut values, xf);
        if let Some(slot) = values.get_mut(types::ceiling::NAME) {
            *slot = Some(Value::String(next_ceiling_name(&seen, storey, &tongue)));
        }
        if let Some(slot) = values.get_mut(types::ceiling::VERTICES) {
            *slot = Some(archbuilder_model::floor::Floor::vertex_values(&vertices));
        }
        if let Some(slot) = values.get_mut(types::ceiling::ARCS) {
            *slot = archbuilder_model::floor::Floor::arc_values(&vertices);
        }
        out.push((*id, create(ctx, CEILING, Some(storey), values)?));
    }
    Ok((out, lost))
}

/// Gives the ceiling `id` the slope `text` names (an angle, a percentage or a rise over a run in
/// the project's unit): on its slope arrow, or, when it has none, on a new arrow along x across
/// its boundary, rising toward +x. An empty text, or a slope of zero, takes the slope away.
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
    let object = expect(ctx.snapshot(), id, CEILING)?;
    let c = Ceiling::from_values(&object.values).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let (from, to) = match c.arrow {
        Some(a) => (a.start, a.end()),
        None => {
            let boundary = c
                .shape
                .boundary_in(ctx.snapshot())
                .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
            let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
            for p in boundary.loops.iter().flatten() {
                lo = [lo[0].min(p[0]), lo[1].min(p[1])];
                hi = [hi[0].max(p[0]), hi[1].max(p[1])];
            }
            let y = (lo[1] + hi[1]) / 2.0;
            ([lo[0], y], [hi[0].max(lo[0] + 1.0), y])
        }
    };
    write_slope(ctx, id, from, to, Some(slope), None)
}
