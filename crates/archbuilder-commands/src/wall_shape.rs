//! The shape of a wall beyond the straight, vertical rectangle: `archbuilder.wall.set_arc`,
//! `archbuilder.wall.set_lean`, `archbuilder.wall.set_profile` and `archbuilder.wall.set_bands`.
//!
//! Each takes the walls (or the wall) named, else the walls of the selection, and writes only
//! authored numbers of the record: the bulge of the baseline, the lean of each face, the sketched
//! elevation profile, the list of bands. What follows from them (the solids, joins, openings)
//! is derived. One call is one transaction; the model's rules refuse a shape that makes no solid,
//! a profile that leaves a door or window outside it, and shapes that cannot be combined.

use std::collections::BTreeSet;

use arc_base::{Error, ObjectId, Result};
use arc_command::{Context, IdParam, QuantityParam, Registry};
use arc_doc::{FieldPath, Value};
use archbuilder_model::spatial;
use archbuilder_model::types::{self, WALL};
use archbuilder_model::wall::{Band, BandFace, Profile, Wall};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::codes;
use crate::structure::{ensure_valid, expect};

/// A message of the catalog, named by identifier.
struct Msg {
    message_id: &'static str,
}

const WALL_SET_ARC_DESCRIPTION: Msg = Msg {
    message_id: "ab-cmd-wall-set-arc-description",
};
const WALL_SET_ARC_LABEL: Msg = Msg {
    message_id: "ab-cmd-wall-set-arc-label",
};
const WALL_SET_BANDS_DESCRIPTION: Msg = Msg {
    message_id: "ab-cmd-wall-set-bands-description",
};
const WALL_SET_BANDS_LABEL: Msg = Msg {
    message_id: "ab-cmd-wall-set-bands-label",
};
const WALL_SET_LEAN_DESCRIPTION: Msg = Msg {
    message_id: "ab-cmd-wall-set-lean-description",
};
const WALL_SET_LEAN_LABEL: Msg = Msg {
    message_id: "ab-cmd-wall-set-lean-label",
};
const WALL_SET_PROFILE_DESCRIPTION: Msg = Msg {
    message_id: "ab-cmd-wall-set-profile-description",
};
const WALL_SET_PROFILE_LABEL: Msg = Msg {
    message_id: "ab-cmd-wall-set-profile-label",
};
const WALL_STACK_TYPE_DESCRIPTION: Msg = Msg {
    message_id: "ab-cmd-wall-stack-type-description",
};
const WALL_STACK_TYPE_LABEL: Msg = Msg {
    message_id: "ab-cmd-wall-stack-type-label",
};

/// The identifier of the command that makes walls arcs or straight.
pub const WALL_SET_ARC: &str = "archbuilder.wall.set_arc";
/// The identifier of the command that leans the faces of walls.
pub const WALL_SET_LEAN: &str = "archbuilder.wall.set_lean";
/// The identifier of the command that sets or clears a wall's elevation profile.
pub const WALL_SET_PROFILE: &str = "archbuilder.wall.set_profile";
/// The identifier of the command that sets the bands of a wall.
pub const WALL_SET_BANDS: &str = "archbuilder.wall.set_bands";

/// The identifier of the command that makes a stacked wall type of basic wall types.
pub const WALL_STACK_TYPE: &str = "archbuilder.wall.stack_type";

/// The most walls one command names.
const MAX_WALLS: usize = 100_000;

fn invalid() -> Error {
    Error::new(arc_command::error::codes::PARAMS_INVALID)
}

fn walls_of(ctx: &Context<'_>, named: Option<Vec<IdParam>>) -> Result<BTreeSet<ObjectId>> {
    let ids: BTreeSet<ObjectId> = match named {
        Some(list) if list.len() > MAX_WALLS => return Err(invalid()),
        Some(list) => list.into_iter().map(|IdParam(id)| id).collect(),
        None => ctx
            .selection()
            .iter()
            .copied()
            .filter(|id| spatial::type_of(ctx.snapshot(), *id).as_deref() == Some(WALL))
            .collect(),
    };
    if ids.is_empty() {
        return Err(Error::new(codes::NOTHING_SELECTED));
    }
    for id in &ids {
        expect(ctx.snapshot(), *id, WALL)?;
    }
    Ok(ids)
}

/// What the shape commands return.
#[derive(Debug, Default, Serialize, JsonSchema)]
pub struct Shaped {
    /// The walls changed.
    pub walls: usize,
}

/// Parameters of `archbuilder.wall.set_arc`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetArc {
    /// The walls; else the walls of the selection. `through` needs exactly one.
    #[serde(default)]
    pub walls: Option<Vec<IdParam>>,
    /// The bulge of the baseline (the tangent of a quarter of its sweep, positive
    /// counter-clockwise from the start to the end); 0 makes the wall straight.
    #[serde(default)]
    pub bulge: Option<f64>,
    /// A point `[x, y]` the baseline passes through; the wall becomes the arc through its two
    /// end points and this one.
    #[serde(default)]
    pub through: Option<[f64; 2]>,
}

fn set_arc(ctx: &mut Context<'_>, p: SetArc) -> Result<Shaped> {
    let ids = walls_of(ctx, p.walls)?;
    if p.through.is_some() && ids.len() != 1 {
        return Err(invalid());
    }
    let bulge_for = |w: &Wall| -> Result<f64> {
        match (p.bulge, p.through) {
            (Some(_), Some(_)) | (None, None) => Err(invalid()),
            (Some(b), None) if b.is_finite() && b.abs() <= archbuilder_model::floor::MAX_BULGE => {
                Ok(b)
            }
            (Some(_), None) => Err(invalid()),
            (None, Some(via)) => {
                archbuilder_model::sketch::bulge_through(w.start, via, w.end).ok_or_else(invalid)
            }
        }
    };
    let snapshot = ctx.snapshot().clone();
    for id in &ids {
        let object =
            spatial::get(&snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let wall =
            Wall::from_values(&object.values).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let bulge = bulge_for(&wall)?;
        let new = Wall { bulge, ..wall };
        // The doors and windows keep their place along the wall: distances follow the length.
        let scale = if wall.length() > 0.0 {
            new.length() / wall.length()
        } else {
            1.0
        };
        let txn = ctx.txn()?;
        let value =
            (bulge.abs() >= archbuilder_model::floor::MIN_BULGE).then_some(Value::F64(bulge));
        txn.modify(*id, FieldPath::new("bulge"), value)?;
        for (object, item) in archbuilder_model::insert::hosted(&snapshot, *id) {
            txn.modify(
                object.id,
                FieldPath::new("distance"),
                Some(types::length(item.distance * scale)),
            )?;
        }
    }
    ensure_valid(ctx)?;
    Ok(Shaped { walls: ids.len() })
}

/// Parameters of `archbuilder.wall.set_lean`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetLean {
    /// The walls; else the walls of the selection.
    #[serde(default)]
    pub walls: Option<Vec<IdParam>>,
    /// A slant: both faces lean by this angle from the vertical, in degrees, positive toward the
    /// left (exterior) side at the top. Not together with the two below.
    #[serde(default)]
    pub degrees: Option<f64>,
    /// The lean of the exterior face, in degrees: a taper when it differs from the interior's.
    #[serde(default)]
    pub exterior_degrees: Option<f64>,
    /// The lean of the interior face, in degrees.
    #[serde(default)]
    pub interior_degrees: Option<f64>,
}

fn set_lean(ctx: &mut Context<'_>, p: SetLean) -> Result<Shaped> {
    let ids = walls_of(ctx, p.walls)?;
    let (exterior, interior) = match (p.degrees, p.exterior_degrees, p.interior_degrees) {
        (Some(d), None, None) => (d, d),
        (None, e, i) => (e.unwrap_or(0.0), i.unwrap_or(0.0)),
        _ => return Err(invalid()),
    };
    if ![exterior, interior]
        .iter()
        .all(|d| d.is_finite() && d.abs() <= 60.0)
    {
        return Err(invalid());
    }
    for id in &ids {
        let txn = ctx.txn()?;
        for (field, degrees) in [("lean_exterior", exterior), ("lean_interior", interior)] {
            let value = (degrees != 0.0).then(|| types::angle(degrees.to_radians()));
            txn.modify(*id, FieldPath::new(field), value)?;
        }
    }
    ensure_valid(ctx)?;
    Ok(Shaped { walls: ids.len() })
}

/// Parameters of `archbuilder.wall.set_profile`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetProfile {
    /// The wall; else the selected wall.
    #[serde(default)]
    pub wall: Option<IdParam>,
    /// The loops of the profile as `[distance along the baseline, height above the base]` points
    /// in meters: the outline first, then the openings in it. Absent or empty restores the
    /// wall's rectangle.
    #[serde(default)]
    #[schemars(length(max = 64))]
    pub loops: Option<Vec<Vec<[f64; 2]>>>,
}

fn set_profile(ctx: &mut Context<'_>, p: SetProfile) -> Result<Shaped> {
    let id = walls_of(ctx, p.wall.map(|w| vec![w]))?;
    let [id] = id.into_iter().collect::<Vec<_>>()[..] else {
        return Err(invalid());
    };
    let loops = p.loops.unwrap_or_default();
    let value = if loops.is_empty() {
        None
    } else {
        if loops.iter().flatten().flatten().any(|v| !v.is_finite()) {
            return Err(invalid());
        }
        let profile = Profile { loops };
        Some(Value::List(profile.records()))
    };
    ctx.txn()?.modify(id, FieldPath::new("profile"), value)?;
    ensure_valid(ctx)?;
    Ok(Shaped { walls: 1 })
}

/// The side of a wall a band is on.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BandSide {
    /// The exterior face, on the left of the baseline.
    Exterior,
    /// The interior face.
    Interior,
}

/// One band of a wall.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BandParam {
    /// The face the band is on.
    pub face: BandSide,
    /// A vertical band (along the height) instead of a horizontal one (along the length).
    #[serde(default)]
    pub vertical: bool,
    /// Whether `position` counts from the top of the wall (the end of the baseline for a
    /// vertical band) instead of the base (the start).
    #[serde(default)]
    pub from_top: bool,
    /// The distance of the band's near edge from the base or start, in meters.
    pub position: QuantityParam,
    /// The extent of the band in the direction of the measurement.
    pub width: QuantityParam,
    /// How far the band stands out of the face; negative for a recess.
    pub depth: QuantityParam,
}

/// Parameters of `archbuilder.wall.set_bands`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetBands {
    /// The walls; else the walls of the selection.
    #[serde(default)]
    pub walls: Option<Vec<IdParam>>,
    /// The bands, replacing those the walls have; absent or empty removes them.
    #[serde(default)]
    #[schemars(length(max = 32))]
    pub bands: Option<Vec<BandParam>>,
}

fn set_bands(ctx: &mut Context<'_>, p: SetBands) -> Result<Shaped> {
    let ids = walls_of(ctx, p.walls)?;
    let mut bands = Vec::new();
    for b in p.bands.unwrap_or_default() {
        bands.push(Band {
            face: match b.face {
                BandSide::Exterior => BandFace::Exterior,
                BandSide::Interior => BandFace::Interior,
            },
            vertical: b.vertical,
            from_top: b.from_top,
            position: crate::length(ctx, &b.position)?.value(),
            width: crate::length(ctx, &b.width)?.value(),
            depth: crate::length(ctx, &b.depth)?.value(),
        });
    }
    if bands.len() > archbuilder_model::wall::MAX_BANDS {
        return Err(invalid());
    }
    let value = (!bands.is_empty()).then(|| Value::List(bands.iter().map(Band::record).collect()));
    for id in &ids {
        ctx.txn()?
            .modify(*id, FieldPath::new("bands"), value.clone())?;
    }
    ensure_valid(ctx)?;
    Ok(Shaped { walls: ids.len() })
}

/// One segment of a stacked wall type.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StackSegment {
    /// The basic wall type of the segment: its identifier or name.
    #[serde(rename = "type")]
    #[schemars(length(max = 256))]
    pub wall_type: String,
    /// The fixed height of the segment; absent for the one variable segment, which takes the
    /// rest of the wall's height.
    #[serde(default)]
    pub height: Option<QuantityParam>,
}

/// Parameters of `archbuilder.wall.stack_type`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StackType {
    /// The name of the new type; else `Stacked wall`.
    #[serde(default)]
    #[schemars(length(max = 256))]
    pub name: Option<String>,
    /// The segments from the base up: fixed heights and one variable one. All of the same total
    /// thickness, none stacked itself.
    #[schemars(length(min = 2, max = 16))]
    pub segments: Vec<StackSegment>,
}

/// What `archbuilder.wall.stack_type` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Stacked {
    /// The new stacked wall type.
    #[serde(rename = "type")]
    pub wall_type: IdParam,
}

/// The wall type named by `key`: an identifier or a name.
fn wall_type_named(ctx: &Context<'_>, key: &str) -> Result<ObjectId> {
    use archbuilder_model::element_type::{self as et, TypeKind};
    if let Ok(id) = key.parse::<ObjectId>() {
        return Ok(id);
    }
    et::of_kind(ctx.snapshot(), TypeKind::Wall)
        .iter()
        .find(|t| archbuilder_model::names::same(et::name(t), key))
        .map(|t| t.id)
        .ok_or_else(invalid)
}

fn stack_type(ctx: &mut Context<'_>, p: StackType) -> Result<Stacked> {
    let mut stack = Vec::with_capacity(p.segments.len());
    for segment in &p.segments {
        let type_id = wall_type_named(ctx, &segment.wall_type)?;
        let height = match &segment.height {
            Some(h) => crate::length(ctx, h)?.value(),
            None => 0.0,
        };
        stack.push(archbuilder_model::element_type::StackEntry { type_id, height });
    }
    let id = crate::element_type::create_stacked_type(ctx, p.name.as_deref(), &stack)?;
    ensure_valid(ctx)?;
    Ok(Stacked {
        wall_type: IdParam(id),
    })
}

/// Registers the commands of this module.
///
/// # Errors
/// The registry's errors for an identifier registered twice.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(arc_command::CommandSpec::new(
        WALL_SET_ARC,
        WALL_SET_ARC_LABEL.message_id,
        WALL_SET_ARC_DESCRIPTION.message_id,
        set_arc,
    ))?;
    registry.register(arc_command::CommandSpec::new(
        WALL_SET_LEAN,
        WALL_SET_LEAN_LABEL.message_id,
        WALL_SET_LEAN_DESCRIPTION.message_id,
        set_lean,
    ))?;
    registry.register(arc_command::CommandSpec::new(
        WALL_SET_PROFILE,
        WALL_SET_PROFILE_LABEL.message_id,
        WALL_SET_PROFILE_DESCRIPTION.message_id,
        set_profile,
    ))?;
    registry.register(arc_command::CommandSpec::new(
        WALL_SET_BANDS,
        WALL_SET_BANDS_LABEL.message_id,
        WALL_SET_BANDS_DESCRIPTION.message_id,
        set_bands,
    ))?;
    registry.register(arc_command::CommandSpec::new(
        WALL_STACK_TYPE,
        WALL_STACK_TYPE_LABEL.message_id,
        WALL_STACK_TYPE_DESCRIPTION.message_id,
        stack_type,
    ))?;
    Ok(())
}
