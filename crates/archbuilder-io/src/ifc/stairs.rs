//! Stairs, ramps and railings. A stair is an `IfcStair` and a ramp an `IfcRamp` with no body of
//! their own: each aggregates its parts (`IfcRelAggregates`), an `IfcStairFlight` for each run
//! (an `IfcRampFlight` for each run of a ramp), an `IfcSlab` of the predefined type `LANDING` for each
//! landing and an `IfcRailing` for each railing on it. A railing that is a record of its own and
//! is hosted on the stair or ramp is one of those parts; a railing by path, or one hosted on a
//! floor or a floor edge, is contained in its storey. Every part has a body of its own as one faceted B-rep in the storey's frame, and the
//! identifier of a part is derived from the identifier of its stair or ramp.
//!
//! The standard property sets of these elements (`Pset_StairCommon`, `Pset_StairFlightCommon`,
//! `Pset_RampCommon`, `Pset_RampFlightCommon`, `Pset_RailingCommon`) hold the counts, lengths and
//! plane angles the model knows, with the measure types of the IFC4 templates; the counts and
//! dimensions of a stair flight are also attributes of the `IfcStairFlight`.

use arc_base::ObjectId;
use arc_base::id::Identifier;
use arc_doc::Object;
use arc_io_ifc::write::{
    Element, ElementKind, FlightDimensions, Measure, Quantity, QuantitySet, QuantityValue,
    derived_id, role,
};
use archbuilder_model::floor::signed_area;
use archbuilder_model::railing::Derived as Railing;
use archbuilder_model::ramp::{self, Part};
use archbuilder_model::stair::{self, Layout, Shape};

use super::{Mapper, WORLD, brep, name_of, psets};
use crate::error::reasons;

/// The role of the flights of a stair or ramp in [`derived_id`]; the index is the run's.
const FLIGHT: u32 = role::CALLER_FIRST + 1;
/// The role of the landings; the index is 0 for a stair's landing, a ramp's count from the foot.
const LANDING: u32 = role::CALLER_FIRST + 2;
/// The role of the railings a stair has on its own sides; the index is the side, left first.
const RAIL: u32 = role::CALLER_FIRST + 3;

fn length(v: f64) -> Quantity {
    Quantity::new("Length", QuantityValue::Length(v))
}

fn volumes(v: f64) -> [Quantity; 2] {
    [
        Quantity::new("GrossVolume", QuantityValue::Volume(v)),
        Quantity::new("NetVolume", QuantityValue::Volume(v)),
    ]
}

/// The length of a path in three dimensions.
fn path_length(path: &[[f64; 3]]) -> f64 {
    path.windows(2)
        .map(|w| {
            (w[1][0] - w[0][0])
                .hypot(w[1][1] - w[0][1])
                .hypot(w[1][2] - w[0][2])
        })
        .sum()
}

/// A landing: an `IfcSlab` of the predefined type `LANDING`, a part of its stair or ramp.
fn landing(id: u128, name: &str, storey: ObjectId, host: u128) -> Element {
    let mut element = Element::new(id, ElementKind::Slab, name, storey.as_u128());
    element.predefined_type = Some("LANDING".to_owned());
    element.host = Some(host);
    element
}

impl Mapper<'_> {
    /// One part of a stair or ramp: `element` with its body from `solid`, which is reported when
    /// it has none, and the quantities of the solid added by `quantities`.
    fn part(
        &mut self,
        mut element: Element,
        solid: arc_base::Result<arc_geom::Polyhedron>,
        owner: &Object,
        level: f64,
        quantities: impl FnOnce(f64) -> QuantitySet,
    ) {
        let label = match element.kind {
            ElementKind::StairFlight => "IFCSTAIRFLIGHT",
            ElementKind::RampFlight => "IFCRAMPFLIGHT",
            _ => "IFCSLAB",
        };
        match solid.ok().and_then(|s| {
            let volume = s.volume();
            brep([&s], &WORLD, level).map(|b| (b, volume))
        }) {
            Some((body, volume)) => {
                element.body = Some(body);
                element.quantity_sets.push(quantities(volume));
            }
            None => self.lose(reasons::BODY_NOT_EXPORTED, label, owner),
        }
        self.elements.push(element);
    }

    /// A stair: the `IfcStair`, its flights, its landing and the railings of its sides.
    pub(super) fn stair(&mut self, object: &Object, storey: ObjectId, level: f64) {
        let Some(placed) = stair::Placed::of(self.snapshot, object) else {
            return self.lose(reasons::NOT_EXPORTED, "IFCSTAIR", object);
        };
        let id = object.id.as_u128();
        let name = name_of(object);
        let mut element = Element::new(id, ElementKind::Stair, &name, storey.as_u128());
        element.predefined_type = Some(
            match placed.stair.shape {
                Shape::Straight => "STRAIGHT_RUN_STAIR",
                Shape::LTurn => "QUARTER_TURN_STAIR",
                Shape::UTurn => "HALF_TURN_STAIR",
                Shape::Sketch => "NOTDEFINED",
            }
            .to_owned(),
        );
        if let Ok(layout) = &placed.layout {
            // The last riser of a run meets the landing or the floor above: a tread less.
            let treads: usize = layout.runs.iter().map(|r| r.risers.saturating_sub(1)).sum();
            psets::attach(
                &mut element,
                "Pset_StairCommon",
                vec![
                    psets::count("NumberOfRiser", layout.risers),
                    psets::count("NumberOfTreads", treads),
                    psets::measure("RiserHeight", Measure::PositiveLength, layout.riser),
                    psets::measure("TreadLength", Measure::PositiveLength, layout.stair.tread),
                    psets::measure(
                        "WaistThickness",
                        Measure::PositiveLength,
                        layout.stair.waist,
                    ),
                    psets::measure(
                        "RequiredHeadroom",
                        Measure::PositiveLength,
                        stair::headroom_of(&object.values),
                    ),
                ],
            );
        }
        self.elements.push(element);
        self.exported += 1;
        let Ok(layout) = &placed.layout else {
            return self.lose(reasons::BODY_NOT_EXPORTED, "IFCSTAIR", object);
        };
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        for (k, run) in layout.runs.iter().enumerate() {
            let mut flight = Element::new(
                derived_id(id, FLIGHT, count(k)),
                ElementKind::StairFlight,
                &format!("{name} flight {}", k + 1),
                storey.as_u128(),
            );
            flight.predefined_type = Some("STRAIGHT".to_owned());
            flight.host = Some(id);
            psets::attach(
                &mut flight,
                "Pset_StairFlightCommon",
                vec![
                    psets::count("NumberOfRiser", run.risers),
                    psets::count("NumberOfTreads", run.risers.saturating_sub(1)),
                    psets::measure("RiserHeight", Measure::PositiveLength, layout.riser),
                    psets::measure("TreadLength", Measure::PositiveLength, layout.stair.tread),
                ],
            );
            // The last riser meets the landing or the floor above, so a run has one tread less
            // than it has risers.
            flight.flight = Some(FlightDimensions::new(
                count(run.risers),
                count(run.risers.saturating_sub(1)),
                layout.riser,
                layout.stair.tread,
            ));
            // The walking line from the first tread to the last.
            let walking =
                run.risers.saturating_sub(1) as f64 * layout.riser.hypot(layout.stair.tread);
            self.part(flight, layout.run_solid(run, level), object, level, |v| {
                let [gross, net] = volumes(v);
                QuantitySet::new(
                    "Qto_StairFlightBaseQuantities",
                    vec![length(walking), gross, net],
                )
            });
        }
        if let Some(solid) = layout.landing_solid(level) {
            let thickness = layout.landing_thickness();
            let area = signed_area(&layout.landing).abs();
            let part = landing(
                derived_id(id, LANDING, 0),
                &format!("{name} landing"),
                storey,
                id,
            );
            self.part(part, solid, object, level, |v| {
                landing_quantities(Some(thickness), area, v)
            });
        }
        for (side, path) in layout.rails().into_iter().enumerate() {
            if path.len() < 2 {
                continue;
            }
            let mut rail = Element::new(
                derived_id(id, RAIL, count(side)),
                ElementKind::Railing,
                &format!(
                    "{name} railing {}",
                    if side == 0 { "left" } else { "right" }
                ),
                storey.as_u128(),
            );
            rail.predefined_type = Some("GUARDRAIL".to_owned());
            rail.host = Some(id);
            psets::attach(
                &mut rail,
                "Pset_RailingCommon",
                vec![psets::measure(
                    "Height",
                    Measure::PositiveLength,
                    archbuilder_model::railing::DEFAULT_HEIGHT,
                )],
            );
            rail.quantity_sets.push(QuantitySet::new(
                "Qto_RailingBaseQuantities",
                vec![length(path_length(&path))],
            ));
            match Layout::rail_solids(&path, level)
                .ok()
                .and_then(|s| brep(&s, &WORLD, level))
            {
                Some(body) => rail.body = Some(body),
                None => self.lose(reasons::BODY_NOT_EXPORTED, "IFCRAILING", object),
            }
            self.elements.push(rail);
        }
    }

    /// A ramp: the `IfcRamp`, an `IfcRampFlight` for each run (one for a ramp by sketch) and a
    /// landing for the foot, each landing between two runs and the top; the railings on it are
    /// records of their own.
    pub(super) fn ramp(&mut self, object: &Object, storey: ObjectId, level: f64) {
        let Some(placed) = ramp::Placed::of(self.snapshot, object) else {
            return self.lose(reasons::NOT_EXPORTED, "IFCRAMP", object);
        };
        let id = object.id.as_u128();
        let name = name_of(object);
        let mut element = Element::new(id, ElementKind::Ramp, &name, storey.as_u128());
        element.predefined_type = ramp_type(&placed.ramp);
        if let Ok(layout) = &placed.layout {
            // The largest slope of the ramp's type, as an angle.
            psets::attach(
                &mut element,
                "Pset_RampCommon",
                vec![psets::measure(
                    "RequiredSlope",
                    Measure::PlaneAngle,
                    layout.max_slope.atan(),
                )],
            );
        }
        self.elements.push(element);
        self.exported += 1;
        let Ok(layout) = &placed.layout else {
            return self.lose(reasons::BODY_NOT_EXPORTED, "IFCRAMP", object);
        };
        let Ok(parts) = layout.parts(level) else {
            return self.lose(reasons::BODY_NOT_EXPORTED, "IFCRAMP", object);
        };
        // The thickness is one number only for a slab of constant thickness.
        let thickness = (layout.ramp.form == ramp::Form::Slab).then_some(layout.ramp.thickness);
        let sketch = !layout.ramp.boundary.is_empty();
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        for ramp::BodyPart {
            part,
            length: long,
            area,
            solid,
        } in parts
        {
            match part {
                Part::Run(k) => {
                    let mut flight = Element::new(
                        derived_id(id, FLIGHT, count(k)),
                        ElementKind::RampFlight,
                        &format!("{name} flight {}", k + 1),
                        storey.as_u128(),
                    );
                    // A ramp by sketch has a boundary of any shape.
                    flight.predefined_type = (!sketch).then(|| "STRAIGHT".to_owned());
                    flight.host = Some(id);
                    psets::attach(
                        &mut flight,
                        "Pset_RampFlightCommon",
                        vec![
                            layout.slopes().get(k).and_then(|s| {
                                psets::measure("Slope", Measure::PlaneAngle, s.atan())
                            }),
                            (!sketch)
                                .then(|| {
                                    psets::measure(
                                        "ClearWidth",
                                        Measure::PositiveLength,
                                        layout.ramp.width,
                                    )
                                })
                                .flatten(),
                        ],
                    );
                    self.part(flight, Ok(solid), object, level, |v| {
                        let mut quantities = vec![length(long)];
                        quantities.extend(
                            thickness.map(|t| Quantity::new("Width", QuantityValue::Length(t))),
                        );
                        quantities.extend([
                            Quantity::new("GrossArea", QuantityValue::Area(area)),
                            Quantity::new("NetArea", QuantityValue::Area(area)),
                        ]);
                        quantities.extend(volumes(v));
                        QuantitySet::new("Qto_RampFlightBaseQuantities", quantities)
                    });
                }
                Part::Landing(k) => {
                    let slab = landing(
                        derived_id(id, LANDING, count(k)),
                        &format!("{name} landing {}", k + 1),
                        storey,
                        id,
                    );
                    self.part(slab, Ok(solid), object, level, |v| {
                        landing_quantities(thickness, area, v)
                    });
                }
            }
        }
    }

    /// A railing by path, or one hosted on a stair, ramp, floor or floor edge, as an `IfcRailing`:
    /// a guardrail when it has a host, else a balustrade, contained in its storey; one hosted on a
    /// stair or ramp is a part of it instead. Its body is the bars of the rail and the handrail, the posts
    /// and the balusters.
    pub(super) fn railing(&mut self, object: &Object, storey: ObjectId, level: f64) {
        let Ok(derived) = Railing::of(self.snapshot, object) else {
            return self.lose(reasons::NOT_EXPORTED, "IFCRAILING", object);
        };
        let mut element = Element::new(
            object.id.as_u128(),
            ElementKind::Railing,
            &name_of(object),
            storey.as_u128(),
        );
        // The host is a part's container only when it is in the file.
        let host = derived.railing.host.map(|h| h.as_u128()).filter(|h| {
            self.elements
                .iter()
                .any(|e| e.id == *h && matches!(e.kind, ElementKind::Stair | ElementKind::Ramp))
        });
        element.host = host;
        element.predefined_type = Some(
            if derived.railing.host.is_some() {
                "GUARDRAIL"
            } else {
                "BALUSTRADE"
            }
            .to_owned(),
        );
        psets::attach(
            &mut element,
            "Pset_RailingCommon",
            vec![psets::measure(
                "Height",
                Measure::PositiveLength,
                derived.kind.height,
            )],
        );
        element.quantity_sets.push(QuantitySet::new(
            "Qto_RailingBaseQuantities",
            vec![length(derived.length())],
        ));
        match derived
            .solids()
            .ok()
            .and_then(|solids| brep(&solids, &WORLD, level))
        {
            Some(body) => element.body = Some(body),
            None => self.lose(reasons::BODY_NOT_EXPORTED, "IFCRAILING", object),
        }
        self.elements.push(element);
        self.exported += 1;
    }
}

/// The `IfcRampTypeEnum` value of a ramp from its runs and turns: one straight run, two in line,
/// two with a quarter or a half turn, or three with two quarter turns the same way. Any other
/// layout of runs is user-defined (the text becomes the object type); a ramp by sketch is not
/// defined.
fn ramp_type(r: &ramp::Ramp) -> Option<String> {
    use std::f64::consts::{FRAC_PI_2, PI};
    if !r.boundary.is_empty() {
        return None;
    }
    let near = |a: f64, b: f64| (a - b).abs() < 1.0e-6;
    let turns: Vec<f64> = r.runs.iter().map(|run| run.turn).collect();
    Some(
        match turns.as_slice() {
            [] => "STRAIGHT_RUN_RAMP",
            [t] if near(*t, 0.0) => "TWO_STRAIGHT_RUN_RAMP",
            [t] if near(t.abs(), FRAC_PI_2) => "QUARTER_TURN_RAMP",
            [t] if near(t.abs(), PI) => "HALF_TURN_RAMP",
            [a, b] if near(a.abs(), FRAC_PI_2) && near(*a, *b) => "TWO_QUARTER_TURN_RAMP",
            _ => "RUNS_WITH_TURNS",
        }
        .to_owned(),
    )
}

/// The base quantities of a landing: a flat top of `area` and a body of `volume`.
fn landing_quantities(thickness: Option<f64>, area: f64, volume: f64) -> QuantitySet {
    let mut quantities: Vec<Quantity> = thickness
        .map(|t| Quantity::new("Width", QuantityValue::Length(t)))
        .into_iter()
        .collect();
    quantities.extend([
        Quantity::new("GrossArea", QuantityValue::Area(area)),
        Quantity::new("NetArea", QuantityValue::Area(area)),
    ]);
    quantities.extend(volumes(volume));
    QuantitySet::new("Qto_SlabBaseQuantities", quantities)
}
