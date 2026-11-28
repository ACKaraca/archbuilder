//! The elevator: authored data read from a record, and everything else derived from it.
//!
//! An elevator record holds what was authored: the middle of the car, the direction its doors
//! face, the car size, the clearance between the car and the shaft walls, the width of the landing
//! doors, the pit below its base level, the overrun above its top level, the top level and, when
//! given, the levels it serves. The base is the level of its storey; the top is the named level,
//! else the highest level of the building; without a list of served levels it stops at every level
//! from the base to the top.
//!
//! Derived each time ([`Derived`]): the shaft (the car grown by the clearance), the car, a landing
//! door on the front side of the shaft at every stop and the shaft's span. The shaft cuts every
//! floor, roof and ceiling its span passes through, like a shaft element ([`crate::opening`] reads
//! [`span`]): from the base level to the top level plus the overrun. The pit lies below the base
//! level and cuts nothing. Nothing of this is stored.

use arc_base::ObjectId;
use arc_doc::{Object, Snapshot, Value};

use crate::floor::Boundary;
use crate::opening::ShaftSpan;
use crate::spatial;
use crate::types::{self, elevator};

/// The range of a car dimension and of a door width, in meters.
pub const CAR_RANGE: (f64, f64) = (0.5, 10.0);
/// The range of the clearance between the car and the shaft walls, in meters.
pub const CLEARANCE_RANGE: (f64, f64) = (0.0, 1.0);
/// The range of the pit depth and of the overrun, in meters.
pub const PIT_RANGE: (f64, f64) = (0.0, 10.0);
/// The car of an elevator without a size: width across the doors, depth, in meters.
pub const DEFAULT_CAR: [f64; 2] = [1.1, 1.4];
/// The clearance between the car and the shaft walls by default, in meters.
pub const DEFAULT_CLEARANCE: f64 = 0.15;
/// The width of the landing doors by default, in meters.
pub const DEFAULT_DOOR_WIDTH: f64 = 0.9;
/// The pit depth by default, in meters.
pub const DEFAULT_PIT: f64 = 1.2;
/// The overrun above the top level by default, in meters.
pub const DEFAULT_OVERRUN: f64 = 1.0;
/// The most levels an elevator names as served.
pub const MAX_STOPS: usize = 256;
/// The height of the car as drawn and exported, in meters.
pub const CAR_HEIGHT: f64 = 2.4;
/// The thickness of the shaft wall drawn in plan, in meters.
pub const WALL_DRAWN: f64 = 0.2;

/// The authored data of an elevator, lengths in meters.
#[derive(Debug, Clone, PartialEq)]
pub struct Elevator {
    /// The middle of the car.
    pub center: [f64; 2],
    /// The direction the doors face, in radians from the x axis.
    pub direction: f64,
    /// The car: width across the doors and depth away from them.
    pub car: [f64; 2],
    /// The clearance between the car and the shaft walls on every side.
    pub clearance: f64,
    /// The width of the landing doors.
    pub door_width: f64,
    /// The depth of the pit below the base level.
    pub pit: f64,
    /// The height of the overrun above the top level.
    pub overrun: f64,
    /// The top level; the highest level of the building when `None`.
    pub top_level: Option<ObjectId>,
    /// The levels served; every level from the base to the top when empty.
    pub stops: Vec<ObjectId>,
}

impl Elevator {
    /// The elevator of a record's field values; `None` when a field is missing or malformed.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Option<Self> {
        let m = |at| types::meters(values, at);
        let direction = match values.get(elevator::DIRECTION)? {
            Some(Value::Quantity(q)) => q.value(),
            _ => return None,
        };
        let stops = match values.get(elevator::STOPS) {
            Some(Some(Value::List(items))) => items
                .iter()
                .take(MAX_STOPS)
                .filter_map(|item| match item {
                    Value::Record(f) => match f.first() {
                        Some(Some(Value::Reference(id))) => Some(*id),
                        _ => None,
                    },
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        Some(Self {
            center: [m(elevator::X)?, m(elevator::Y)?],
            direction,
            car: [m(elevator::CAR_WIDTH)?, m(elevator::CAR_DEPTH)?],
            clearance: m(elevator::CLEARANCE)?,
            door_width: m(elevator::DOOR_WIDTH)?,
            pit: m(elevator::PIT)?,
            overrun: m(elevator::OVERRUN)?,
            top_level: match values.get(elevator::TOP_LEVEL) {
                Some(Some(Value::Reference(id))) => Some(*id),
                _ => None,
            },
            stops,
        })
    }

    /// The field values of a record of this elevator named `name`.
    #[must_use]
    pub fn values(&self, name: &str) -> Vec<Option<Value>> {
        let mut values: Vec<Option<Value>> = vec![None; elevator::STOPS + 1];
        values[elevator::NAME] = Some(Value::String(name.to_owned()));
        values[elevator::X] = Some(types::length(self.center[0]));
        values[elevator::Y] = Some(types::length(self.center[1]));
        values[elevator::DIRECTION] = Some(crate::stair::angle(self.direction));
        values[elevator::CAR_WIDTH] = Some(types::length(self.car[0]));
        values[elevator::CAR_DEPTH] = Some(types::length(self.car[1]));
        values[elevator::CLEARANCE] = Some(types::length(self.clearance));
        values[elevator::DOOR_WIDTH] = Some(types::length(self.door_width));
        values[elevator::PIT] = Some(types::length(self.pit));
        values[elevator::OVERRUN] = Some(types::length(self.overrun));
        values[elevator::TOP_LEVEL] = self.top_level.map(Value::Reference);
        values[elevator::STOPS] = (!self.stops.is_empty()).then(|| Self::stops_value(&self.stops));
        values
    }

    /// The record's value of a list of served levels.
    #[must_use]
    pub fn stops_value(stops: &[ObjectId]) -> Value {
        Value::List(
            stops
                .iter()
                .map(|id| Value::Record(vec![Some(Value::Reference(*id))]))
                .collect(),
        )
    }

    /// Whether every size is finite and in its range.
    #[must_use]
    pub fn sizes_ok(&self) -> bool {
        let within = |v: f64, (lo, hi): (f64, f64)| v.is_finite() && v >= lo && v <= hi;
        self.center.iter().all(|v| v.is_finite())
            && self.direction.is_finite()
            && self.car.iter().all(|v| within(*v, CAR_RANGE))
            && within(self.door_width, CAR_RANGE)
            && self.door_width <= self.car[0] + 2.0 * self.clearance
            && within(self.clearance, CLEARANCE_RANGE)
            && within(self.pit, PIT_RANGE)
            && within(self.overrun, PIT_RANGE)
    }

    /// The elevator under the plane motion `xf`: the middle moved, the direction turned.
    #[must_use]
    pub fn moved(&self, xf: &crate::transform::Xform) -> Self {
        let (s, c) = crate::transform::sin_cos(self.direction);
        let center = xf.point(self.center);
        let ahead = xf.point([self.center[0] + c, self.center[1] + s]);
        Self {
            center,
            direction: (ahead[1] - center[1]).atan2(ahead[0] - center[0]),
            ..self.clone()
        }
    }

    /// The point at `across` (to the left of the doors' direction) and `along` (toward the doors)
    /// from the middle of the car, in plan.
    fn at(&self, along: f64, across: f64) -> [f64; 2] {
        let (s, c) = crate::transform::sin_cos(self.direction);
        [
            self.center[0] + c * along - s * across,
            self.center[1] + s * along + c * across,
        ]
    }

    /// The car's corners in plan, counterclockwise.
    #[must_use]
    pub fn car_outline(&self) -> Vec<[f64; 2]> {
        let (w, d) = (self.car[0] / 2.0, self.car[1] / 2.0);
        vec![
            self.at(-d, -w),
            self.at(d, -w),
            self.at(d, w),
            self.at(-d, w),
        ]
    }

    /// The shaft's corners in plan, counterclockwise: the car grown by the clearance.
    #[must_use]
    pub fn shaft_outline(&self) -> Vec<[f64; 2]> {
        let (w, d) = (
            self.car[0] / 2.0 + self.clearance,
            self.car[1] / 2.0 + self.clearance,
        );
        vec![
            self.at(-d, -w),
            self.at(d, -w),
            self.at(d, w),
            self.at(-d, w),
        ]
    }

    /// The landing door in plan: the two ends of its opening on the front side of the shaft.
    #[must_use]
    pub fn door_line(&self) -> [[f64; 2]; 2] {
        let d = self.car[1] / 2.0 + self.clearance;
        let h = self.door_width / 2.0;
        [self.at(d, -h), self.at(d, h)]
    }
}

/// One stop of an elevator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stop {
    /// The level.
    pub level: ObjectId,
    /// Its elevation, in meters.
    pub elevation: f64,
}

/// What an elevator reads from a snapshot.
#[derive(Debug, Clone)]
pub struct Derived {
    /// The authored data.
    pub elevator: Elevator,
    /// The storey it starts on.
    pub storey: Option<ObjectId>,
    /// The elevation of the base level.
    pub base: f64,
    /// The elevation of the top level.
    pub top: f64,
    /// The stops from the bottom up, each with a landing door.
    pub stops: Vec<Stop>,
}

impl Derived {
    /// The elevation of the shaft's floor: the base level less the pit.
    #[must_use]
    pub fn pit_floor(&self) -> f64 {
        self.base - self.elevator.pit
    }

    /// The elevations the shaft spans: the base level to the top level plus the overrun.
    #[must_use]
    pub fn z(&self) -> (f64, f64) {
        (self.base, self.top + self.elevator.overrun)
    }
}

/// Reads the elevator `object` of `snapshot`; `None` when its record, its sizes or its levels
/// cannot be read, or the top is not above the base.
#[must_use]
pub fn derived(snapshot: &Snapshot, object: &Object) -> Option<Derived> {
    let elevator = Elevator::from_values(&object.values)?;
    if !elevator.sizes_ok() {
        return None;
    }
    let storey = object
        .header
        .owner
        .and_then(|s| spatial::get(snapshot, s))?;
    let base = spatial::storey_elevation(snapshot, &storey);
    let building = storey.header.owner?;
    let levels = spatial::levels(snapshot, building);
    let at = |l: &Object| types::meters(&l.values, types::level::ELEVATION);
    let top = match elevator.top_level {
        Some(id) => at(&*spatial::get(snapshot, id)?)?,
        None => levels.iter().filter_map(|l| at(l)).fold(base, f64::max),
    };
    if top < base + types::HEIGHT_RANGE.0 {
        return None;
    }
    let reach = |e: f64| e >= base - 1.0e-6 && e <= top + 1.0e-6;
    let mut stops: Vec<Stop> = if elevator.stops.is_empty() {
        levels
            .iter()
            .filter_map(|l| {
                Some(Stop {
                    level: l.id,
                    elevation: at(l)?,
                })
            })
            .filter(|s| reach(s.elevation))
            .collect()
    } else {
        elevator
            .stops
            .iter()
            .filter_map(|id| {
                let l = spatial::get(snapshot, *id)?;
                Some(Stop {
                    level: l.id,
                    elevation: at(&l)?,
                })
            })
            .filter(|s| reach(s.elevation))
            .collect()
    };
    stops.sort_by(|a, b| {
        a.elevation
            .total_cmp(&b.elevation)
            .then(a.level.cmp(&b.level))
    });
    stops.dedup_by_key(|s| s.level);
    Some(Derived {
        elevator,
        storey: Some(storey.id),
        base,
        top,
        stops,
    })
}

/// The shaft span of the elevator `object`, as the document's shafts have it.
#[must_use]
pub fn span(snapshot: &Snapshot, object: &Object) -> Option<ShaftSpan> {
    let d = derived(snapshot, object)?;
    let outline = d.elevator.shaft_outline();
    Some(ShaftSpan {
        id: object.id,
        storey: d.storey,
        boundary: Boundary::polygons(vec![outline.clone()]),
        outline,
        z: d.z(),
    })
}

/// Every readable elevator of the document as a shaft span.
#[must_use]
pub fn spans(snapshot: &Snapshot) -> Vec<ShaftSpan> {
    spatial::objects(snapshot, types::ELEVATOR)
        .iter()
        .take(crate::opening::MAX_CUTS)
        .filter_map(|o| span(snapshot, o))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lift() -> Elevator {
        Elevator {
            center: [2.0, 3.0],
            direction: 0.0,
            car: [1.1, 1.4],
            clearance: 0.15,
            door_width: 0.9,
            pit: 1.2,
            overrun: 1.0,
            top_level: None,
            stops: Vec::new(),
        }
    }

    #[test]
    fn the_shaft_is_the_car_grown_by_the_clearance_and_the_door_is_on_the_front() {
        let e = lift();
        let shaft = e.shaft_outline();
        assert!((crate::floor::signed_area(&shaft) - (1.4 + 0.3) * (1.1 + 0.3)).abs() < 1.0e-9);
        let door = e.door_line();
        // Facing +x: the door is on the x = 2 + 0.7 + 0.15 side, 0.9 m wide.
        assert!((door[0][0] - 2.85).abs() < 1.0e-9 && (door[1][0] - 2.85).abs() < 1.0e-9);
        assert!(((door[1][1] - door[0][1]).abs() - 0.9).abs() < 1.0e-9);
        // Counterclockwise corners.
        assert!(crate::floor::signed_area(&e.car_outline()) > 0.0);
    }

    #[test]
    fn a_record_reads_back_what_it_was_written_with_and_a_wide_door_is_refused() {
        let e = lift();
        assert_eq!(
            Elevator::from_values(&e.values("Elevator 1")),
            Some(e.clone())
        );
        assert!(e.sizes_ok());
        let wide = Elevator {
            door_width: 3.0,
            ..e
        };
        assert!(!wide.sizes_ok());
    }

    #[test]
    fn a_turn_moves_the_middle_and_turns_the_doors() {
        let e = lift().moved(&crate::transform::Xform::Rotate {
            base: [0.0, 0.0],
            angle: std::f64::consts::FRAC_PI_2,
        });
        assert!((e.center[0] + 3.0).abs() < 1.0e-9 && (e.center[1] - 2.0).abs() < 1.0e-9);
        assert!((e.direction - std::f64::consts::FRAC_PI_2).abs() < 1.0e-9);
    }
}
