//! Elevators, stairs by sketch with the headroom check, dormers and roof edge elements: the
//! commands, what they derive, copy and move, delete and undo.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::floor::Derived as FloorDerived;
use archbuilder_model::spatial;
use archbuilder_model::types::{
    DORMER, ELEVATOR, FLOOR, ROOF, ROOF_EDGE, STAIR, STOREY, level, meters, text,
};
use serde_json::{Value as Json, json};

struct Env {
    document: Document,
    session: Session,
    registry: arc_command::Registry,
}

impl Env {
    fn new() -> Self {
        let document = Document::open(
            Snapshot::empty(Arc::new(archbuilder_commands::types().unwrap())),
            Arc::new(Evaluators::new()),
            DocumentConfig {
                workers: 0,
                ..DocumentConfig::default()
            },
        )
        .unwrap();
        archbuilder_commands::install(&document);
        let env = Self {
            document,
            session: Session::new(Surface::Ui, [Permission::Read, Permission::Edit]),
            registry: archbuilder_commands::registry().unwrap(),
        };
        env.run("archbuilder.project.new", json!({})).unwrap();
        env
    }

    fn run(&self, command: &str, params: Json) -> arc_base::Result<Outcome> {
        let invocation = Invocation {
            seed: 7,
            ..Invocation::new(command, params)
        };
        Invoker::new(&self.registry).invoke(&self.document, &self.session, invocation)
    }

    fn snapshot(&self) -> Snapshot {
        self.document.snapshot()
    }

    fn storey(&self, name: &str) -> ObjectId {
        spatial::objects(&self.snapshot(), STOREY)
            .iter()
            .find(|s| text(&s.values, 0) == Some(name))
            .unwrap()
            .id
    }

    fn slab(&self, storey: &str) -> ObjectId {
        let made = self
            .run(
                "archbuilder.floor.create",
                json!({"storey": self.storey(storey).to_string(),
                       "points": [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]]}),
            )
            .unwrap();
        made.value["floor"].as_str().unwrap().parse().unwrap()
    }

    fn area(&self, floor: ObjectId) -> f64 {
        let s = self.snapshot();
        FloorDerived::of(&s, &spatial::get(&s, floor).unwrap())
            .unwrap()
            .area()
    }

    /// The 10 x 6 house of walls on the ground floor with a hip roof over it.
    fn roof(&self) -> ObjectId {
        let c = [[0.0, 0.0], [0.0, 6.0], [10.0, 6.0], [10.0, 0.0]];
        for k in 0..4 {
            self.run(
                "archbuilder.wall.create",
                json!({"start": c[k], "end": c[(k + 1) % 4]}),
            )
            .unwrap();
        }
        let made = self
            .run(
                "archbuilder.roof.create",
                json!({"picks": [[5.0, 3.0]], "slope": std::f64::consts::FRAC_PI_4}),
            )
            .unwrap();
        made.value["roof"].as_str().unwrap().parse().unwrap()
    }

    fn count(&self, key: &str) -> usize {
        spatial::objects(&self.snapshot(), key).len()
    }
}

fn id(v: &Json) -> ObjectId {
    v.as_str().unwrap().parse().unwrap()
}

#[test]
fn an_elevator_cuts_the_floors_it_passes_and_undo_restores_them() {
    let env = Env::new();
    let (f1, f2) = (env.slab("Floor 1"), env.slab("Floor 2"));
    let ground = env.slab("Ground Floor");
    assert!((env.area(f1) - 60.0).abs() < 1e-9);
    let made = env
        .run("archbuilder.elevator.create", json!({"center": [5.0, 3.0]}))
        .unwrap();
    // The shaft is the 1.1 x 1.4 car with 0.15 m around it.
    let shaft = (1.1 + 0.3) * (1.4 + 0.3);
    assert!((made.value["shaft_area"].as_f64().unwrap() - shaft).abs() < 1e-9);
    assert_eq!(made.value["stops"], 3);
    // It stops at the three levels and passes the slabs of the two upper ones; the slab it
    // stands on is not cut.
    assert!((env.area(f1) - (60.0 - shaft)).abs() < 1e-9);
    assert!((env.area(f2) - (60.0 - shaft)).abs() < 1e-9);
    assert!((env.area(ground) - 60.0).abs() < 1e-9);
    assert_eq!(
        text(
            &spatial::get(&env.snapshot(), id(&made.value["elevator"]))
                .unwrap()
                .values,
            0
        ),
        Some("Elevator 1")
    );
    env.document.undo().wait().unwrap();
    assert_eq!(env.count(ELEVATOR), 0);
    assert!((env.area(f1) - 60.0).abs() < 1e-9);
    env.document.redo().wait().unwrap();
    assert!((env.area(f1) - (60.0 - shaft)).abs() < 1e-9);
}

#[test]
fn an_elevator_stops_only_at_the_levels_it_serves_and_refuses_bad_sizes() {
    let env = Env::new();
    let f1 = env.slab("Floor 1");
    let top = spatial::objects(&env.snapshot(), STOREY)
        .iter()
        .find(|s| text(&s.values, 0) == Some("Floor 1"))
        .and_then(|s| spatial::level_of(&env.snapshot(), s))
        .unwrap();
    assert_eq!(meters(&top.values, level::ELEVATION), Some(3.0));
    let made = env
        .run(
            "archbuilder.elevator.create",
            json!({"center": [5.0, 3.0], "top_level": top.id.to_string()}),
        )
        .unwrap();
    assert_eq!(made.value["stops"], 2);
    assert!(env.area(f1) < 60.0);
    // A door wider than the shaft is refused, and nothing is left behind.
    let before = env.count(ELEVATOR);
    assert!(
        env.run(
            "archbuilder.elevator.create",
            json!({"center": [1.0, 1.0], "door_width": 3.0})
        )
        .is_err()
    );
    assert_eq!(env.count(ELEVATOR), before);
}

#[test]
fn an_elevator_moves_and_copies_with_its_shaft() {
    let env = Env::new();
    let f1 = env.slab("Floor 1");
    let made = env
        .run("archbuilder.elevator.create", json!({"center": [5.0, 3.0]}))
        .unwrap();
    let lift = id(&made.value["elevator"]);
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [lift.to_string()], "displacement": [2.0, 0.0]}),
    )
    .unwrap();
    let s = env.snapshot();
    let moved =
        archbuilder_model::elevator::Elevator::from_values(&spatial::get(&s, lift).unwrap().values)
            .unwrap();
    assert!((moved.center[0] - 7.0).abs() < 1e-9);
    env.run(
        "archbuilder.modify.copy",
        json!({"objects": [lift.to_string()], "displacement": [0.0, 1.5]}),
    )
    .unwrap();
    assert_eq!(env.count(ELEVATOR), 2);
    let two = 60.0 - 2.0 * (1.4 * 1.7);
    assert!((env.area(f1) - two).abs() < 1e-9, "{}", env.area(f1));
}

#[test]
fn a_stair_by_sketch_has_runs_a_landing_an_opening_and_copies_whole() {
    let env = Env::new();
    let f1 = env.slab("Floor 1");
    let made = env
        .run(
            "archbuilder.stair.create_sketch",
            json!({"runs": [[[1.0, 1.0], [4.0, 1.0]], [[4.5, 1.5], [4.5, 4.5]]]}),
        )
        .unwrap();
    let stair = id(&made.value["stair"]);
    let s = env.snapshot();
    let placed =
        archbuilder_model::stair::Placed::of(&s, &spatial::get(&s, stair).unwrap()).unwrap();
    let layout = placed.layout.as_ref().unwrap();
    assert_eq!(
        layout.runs.iter().map(|r| r.risers).sum::<usize>(),
        layout.risers
    );
    assert_eq!(layout.risers, 17);
    assert_eq!(layout.landings.len(), 1);
    // The riser count is shared in proportion to the lengths of the lines (3 m and 3 m).
    assert!(layout.runs[0].risers.abs_diff(layout.runs[1].risers) <= 1);
    let solids: f64 = layout
        .solids(placed.base)
        .unwrap()
        .iter()
        .map(|p| p.volume())
        .sum();
    assert!(
        (solids - layout.volume()).abs() < 1e-6,
        "{solids} {}",
        layout.volume()
    );
    assert!(
        env.area(f1) < 60.0 - 3.0,
        "the floor above has the stair's opening"
    );
    // A copy keeps the run lines, moved.
    env.run(
        "archbuilder.modify.copy",
        json!({"objects": [stair.to_string()], "displacement": [0.0, 0.5]}),
    )
    .unwrap();
    let s = env.snapshot();
    let copies: Vec<_> = spatial::objects(&s, STAIR);
    assert_eq!(copies.len(), 2);
    let lines: Vec<_> = copies
        .iter()
        .map(|o| archbuilder_model::stair::run_lines(&o.values))
        .collect();
    assert!(lines.iter().all(|l| l.len() == 2));
    assert!(
        lines
            .iter()
            .any(|l| (l[0].from[1] - 1.5).abs() < 1e-9 && (l[1].to[1] - 5.0).abs() < 1e-9)
    );
    // Too few risers for the runs is refused: a 0.3 m rise cannot carry two runs.
    assert!(
        env.run(
            "archbuilder.stair.create_sketch",
            json!({"runs": [[[1.0, 1.0], [4.0, 1.0]], [[4.5, 1.5], [4.5, 4.5]]],
                   "max_riser": 5.0})
        )
        .is_err()
    );
}

#[test]
fn a_stair_under_a_slab_warns_about_headroom_until_it_is_given_less() {
    let env = Env::new();
    env.slab("Floor 1");
    let mezz = env
        .run(
            "archbuilder.storey.add",
            json!({"name": "Mezzanine", "elevation": 1.5}),
        )
        .unwrap();
    let level = mezz.value["level"].as_str().unwrap().to_owned();
    let made = env
        .run(
            "archbuilder.stair.create_sketch",
            json!({"runs": [[[1.0, 1.0], [3.0, 1.0]]], "top_level": level, "max_riser": 0.2}),
        )
        .unwrap();
    let stair = id(&made.value["stair"]);
    let s = env.snapshot();
    let object = spatial::get(&s, stair).unwrap();
    let found: Vec<_> = archbuilder_model::validate::stair_warnings(&s, &object)
        .into_iter()
        .filter(|e| e.code() == "ab.model.stair_headroom")
        .collect();
    assert_eq!(
        found.len(),
        1,
        "the slab 3 m up leaves under 2 m above the top treads"
    );
    // A lower minimum is met.
    env.run(
        "archbuilder.property.set",
        json!({"objects": [stair.to_string()], "field": "headroom", "value": 1.0}),
    )
    .unwrap();
    let s = env.snapshot();
    let object = spatial::get(&s, stair).unwrap();
    assert!(
        archbuilder_model::validate::stair_warnings(&s, &object)
            .iter()
            .all(|e| e.code() != "ab.model.stair_headroom")
    );
}

#[test]
fn a_dormer_cuts_its_roof_and_stands_on_it_with_its_own_roof() {
    let env = Env::new();
    let roof = env.roof();
    let area = |env: &Env| {
        let s = env.snapshot();
        archbuilder_model::roof::Derived::of(&s, &spatial::get(&s, roof).unwrap())
            .unwrap()
            .solid()
            .unwrap()
            .volume()
    };
    let before = area(&env);
    let made = env
        .run(
            "archbuilder.dormer.create",
            json!({"roof": roof.to_string(), "center": [5.0, 1.4], "depth": 1.0}),
        )
        .unwrap();
    assert!((made.value["opening_area"].as_f64().unwrap() - 1.5).abs() < 1e-9);
    assert!(made.value["volume"].as_f64().unwrap() > 0.1);
    assert!(
        area(&env) < before,
        "the opening takes material out of the roof"
    );
    let dormer = id(&made.value["dormer"]);
    // Facing down the slope: toward -y on the south slope.
    let s = env.snapshot();
    let d =
        archbuilder_model::dormer::Dormer::from_values(&spatial::get(&s, dormer).unwrap().values)
            .unwrap();
    assert!(
        (d.direction + std::f64::consts::FRAC_PI_2).abs() < 1e-6,
        "{}",
        d.direction
    );
    let built =
        archbuilder_model::dormer::Derived::of(&s, &spatial::get(&s, dormer).unwrap()).unwrap();
    // Front wall, two cheeks, two roof planes.
    assert_eq!(built.parts.len(), 5);
    // Too deep for the wall height: the eave would lie under the roof at the back.
    assert!(
        env.run(
            "archbuilder.dormer.create",
            json!({"roof": roof.to_string(), "center": [8.0, 1.4], "depth": 3.0})
        )
        .is_err()
    );
    assert_eq!(env.count(DORMER), 1);
    // The dormer goes where its roof goes and is deleted with it.
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [roof.to_string()], "displacement": [1.0, 0.0]}),
    )
    .unwrap();
    let s = env.snapshot();
    let moved =
        archbuilder_model::dormer::Dormer::from_values(&spatial::get(&s, dormer).unwrap().values)
            .unwrap();
    assert!((moved.center[0] - 6.0).abs() < 1e-9);
    Invoker::new(&env.registry)
        .invoke(
            &env.document,
            &env.session,
            Invocation {
                seed: 7,
                selection: vec![roof],
                ..Invocation::new("archbuilder.object.delete", json!({}))
            },
        )
        .unwrap();
    assert_eq!(env.count(DORMER), 0);
    env.document.undo().wait().unwrap();
    assert_eq!(env.count(DORMER), 1);
}

#[test]
fn fascia_and_gutter_follow_the_eaves_and_are_refused_on_gables() {
    let env = Env::new();
    let roof = env.roof();
    let fascia = env
        .run(
            "archbuilder.roof_edge.create",
            json!({"roof": roof.to_string(), "role": "fascia"}),
        )
        .unwrap();
    // The hip roof's four eaves: the outline is 11.2 x 7.2 m.
    assert!(
        (fascia.value["length"].as_f64().unwrap() - 36.8).abs() < 1e-6,
        "{}",
        fascia.value["length"]
    );
    assert!(fascia.value["volume"].as_f64().unwrap() > 0.0);
    let gutter = env
        .run(
            "archbuilder.roof_edge.create",
            json!({"roof": roof.to_string(), "role": "gutter", "edges": [0]}),
        )
        .unwrap();
    assert!(gutter.value["length"].as_f64().unwrap() > 6.0);
    assert_eq!(env.count(ROOF_EDGE), 2);
    // The eave line is the roof's: a closed chain at the eave elevation.
    let s = env.snapshot();
    let object = spatial::get(&s, id(&fascia.value["roof_edge"])).unwrap();
    let d = archbuilder_model::roof_edge::Derived::of(&s, &object).unwrap();
    assert_eq!(d.chains.len(), 1);
    assert!(d.chains[0].closed);
    let z = d.chains[0].points[0][2];
    assert!(d.chains[0].points.iter().all(|p| (p[2] - z).abs() < 1e-6));
    assert!(
        env.run(
            "archbuilder.roof_edge.create",
            json!({"roof": roof.to_string(), "edges": [99]})
        )
        .is_err()
    );
    // A copy of the roof takes its edge elements and dormers along.
    env.run(
        "archbuilder.modify.copy",
        json!({"objects": [roof.to_string()], "displacement": [0.0, 20.0]}),
    )
    .unwrap();
    assert_eq!(env.count(ROOF), 2);
    assert_eq!(env.count(ROOF_EDGE), 4);
    env.document.undo().wait().unwrap();
    assert_eq!(env.count(ROOF_EDGE), 2);
    let _ = FLOOR;
}
