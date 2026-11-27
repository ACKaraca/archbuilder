//! `archbuilder.elevator.create`: an elevator on its base storey.
//!
//! The elevator is its authored data ([`archbuilder_model::elevator::Elevator`]): the middle of the
//! car (`center`), the direction the doors face (`to`, a point they face, or `direction`, an angle;
//! along the x axis without either), the car size, the clearance to the shaft walls, the width of
//! the landing doors, the pit, the overrun, the top level (the highest level of the building
//! without one) and the levels it serves (every level from the base to the top without a list).
//! The shaft it makes cuts the floors, roofs and ceilings it passes through; nothing of that is
//! stored. A car that does not fit its sizes, or a top that is not above the base, is refused.
//! One call is one transaction, one undo step.

use arc_base::{Dimension, Error, ObjectId, Result};
use arc_command::{CommandSpec, Context, IdParam, QuantityParam, Registry};
use archbuilder_model::elevator::{
    DEFAULT_CAR, DEFAULT_CLEARANCE, DEFAULT_DOOR_WIDTH, DEFAULT_OVERRUN, DEFAULT_PIT, Elevator,
};
use archbuilder_model::types::{ELEVATOR, LEVEL};
use archbuilder_model::{defaults, names, spatial, types};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::structure::{create, ensure_valid, expect, language};

/// The identifier of the create-elevator command.
pub const ELEVATOR_CREATE: &str = "archbuilder.elevator.create";

/// Parameters of `archbuilder.elevator.create`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateElevator {
    /// The base storey; else the storey of the selection, else the lowest storey.
    #[serde(default)]
    pub storey: Option<IdParam>,
    /// The middle of the car, `[x, y]` in meters.
    pub center: [f64; 2],
    /// A point the doors face.
    #[serde(default)]
    pub to: Option<[f64; 2]>,
    /// The direction the doors face as an angle, used without `to` (a number is radians, a text
    /// number such as `"90"` degrees).
    #[serde(default)]
    pub direction: Option<QuantityParam>,
    /// The car width across the doors; 1.1 m when absent.
    #[serde(default)]
    pub car_width: Option<QuantityParam>,
    /// The car depth; 1.4 m when absent.
    #[serde(default)]
    pub car_depth: Option<QuantityParam>,
    /// The clearance between the car and the shaft walls; 0.15 m when absent.
    #[serde(default)]
    pub clearance: Option<QuantityParam>,
    /// The width of the landing doors; 0.9 m when absent.
    #[serde(default)]
    pub door_width: Option<QuantityParam>,
    /// The pit depth below the base level; 1.2 m when absent.
    #[serde(default)]
    pub pit: Option<QuantityParam>,
    /// The overrun above the top level; 1 m when absent.
    #[serde(default)]
    pub overrun: Option<QuantityParam>,
    /// The top level; else the highest level of the building.
    #[serde(default)]
    pub top_level: Option<IdParam>,
    /// The levels served; else every level from the base to the top.
    #[serde(default)]
    pub stops: Option<Vec<IdParam>>,
    /// The name; else the first free `Elevator <n>` of the storey.
    #[serde(default)]
    pub name: Option<String>,
}

/// What `archbuilder.elevator.create` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ElevatorCreated {
    /// The new elevator.
    pub elevator: IdParam,
    /// Its base storey.
    pub storey: IdParam,
    /// The levels it stops at, from the bottom up.
    pub stops: usize,
    /// The elevations the shaft spans (base level, top of the overrun), in meters.
    pub span: [f64; 2],
    /// The area of the shaft in plan, in square meters.
    pub shaft_area: f64,
}

fn invalid() -> Error {
    Error::new(arc_command::error::codes::PARAMS_INVALID)
}

pub(crate) fn next_name(snapshot: &arc_doc::Snapshot, storey: ObjectId, tongue: &str) -> String {
    let used = spatial::children(snapshot, ELEVATOR, storey);
    let taken = |name: &str| {
        used.iter()
            .any(|f| types::text(&f.values, 0).is_some_and(|n| names::same(n, name)))
    };
    (used.len() + 1..)
        .map(|n| defaults::elevator(tongue, n))
        .find(|name| !taken(name))
        .unwrap_or_default()
}

fn create_elevator(ctx: &mut Context<'_>, p: CreateElevator) -> Result<ElevatorCreated> {
    let storey = crate::wall::storey_for(ctx, p.storey)?.id;
    let meters = |ctx: &Context<'_>, param: &Option<QuantityParam>, default: f64| match param {
        Some(param) => crate::length(ctx, param).map(|q| q.value()),
        None => Ok(default),
    };
    let direction = match (&p.to, &p.direction) {
        (Some(to), _) => {
            let d = [to[0] - p.center[0], to[1] - p.center[1]];
            if !d.iter().all(|v| v.is_finite()) || d[0].hypot(d[1]) < 1.0e-6 {
                return Err(invalid());
            }
            d[1].atan2(d[0])
        }
        (None, Some(QuantityParam::Text(t))) if t.trim().parse::<f64>().is_ok() => {
            t.trim().parse::<f64>().unwrap_or(0.0).to_radians()
        }
        (None, Some(param)) => ctx.quantity(param, Dimension::ANGLE)?.value(),
        (None, None) => 0.0,
    };
    let top_level = match p.top_level {
        Some(IdParam(id)) => Some(expect(ctx.snapshot(), id, LEVEL)?.id),
        None => None,
    };
    let mut stops = Vec::new();
    for IdParam(id) in p.stops.unwrap_or_default() {
        stops.push(expect(ctx.snapshot(), id, LEVEL)?.id);
    }
    let elevator = Elevator {
        center: p.center,
        direction,
        car: [
            meters(ctx, &p.car_width, DEFAULT_CAR[0])?,
            meters(ctx, &p.car_depth, DEFAULT_CAR[1])?,
        ],
        clearance: meters(ctx, &p.clearance, DEFAULT_CLEARANCE)?,
        door_width: meters(ctx, &p.door_width, DEFAULT_DOOR_WIDTH)?,
        pit: meters(ctx, &p.pit, DEFAULT_PIT)?,
        overrun: meters(ctx, &p.overrun, DEFAULT_OVERRUN)?,
        top_level,
        stops,
    };
    if !elevator.sizes_ok() {
        return Err(invalid());
    }
    let tongue = language(ctx);
    let name = match p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => name.to_owned(),
        None => next_name(ctx.snapshot(), storey, &tongue),
    };
    let id = create(ctx, ELEVATOR, Some(storey), elevator.values(&name))?;
    ensure_valid(ctx)?;
    let derived = {
        let txn = ctx.txn()?;
        spatial::get(txn.snapshot(), id)
            .and_then(|o| archbuilder_model::elevator::derived(txn.snapshot(), &o))
    };
    // A top that is not above the base leaves the elevator without a shaft: refused.
    let derived = derived.ok_or_else(invalid)?;
    ctx.set_selection_after(vec![id]);
    let outline = derived.elevator.shaft_outline();
    Ok(ElevatorCreated {
        elevator: IdParam(id),
        storey: IdParam(storey),
        stops: derived.stops.len(),
        span: [derived.z().0, derived.z().1],
        shaft_area: archbuilder_model::floor::signed_area(&outline).abs(),
    })
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
        ELEVATOR_CREATE,
        Msg {
            message_id: "ab-cmd-elevator-create-label",
        }
        .message_id,
        Msg {
            message_id: "ab-cmd-elevator-create-description",
        }
        .message_id,
        |ctx: &mut Context<'_>, p: CreateElevator| create_elevator(ctx, p),
    ))
}
