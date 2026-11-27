//! Railings by path and their types, rail extensions and continuous rails, and ramps: creation,
//! hosting, slope warnings, the opening in the floor above, the modify commands, delete, undo and
//! redo.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::railing::Derived;
use archbuilder_model::ramp::Placed;
use archbuilder_model::spatial;
use archbuilder_model::types::{RAILING, RAILING_TYPE, RAMP, STOREY, text};
use serde_json::{Value as Json, json};

struct Env {
    document: Document,
    session: Session,
    registry: arc_command::Registry,
}

impl Env {
    fn new() -> Self {
        let config = DocumentConfig {
            workers: 0,
            ..DocumentConfig::default()
        };
        let document = Document::open(
            Snapshot::empty(Arc::new(archbuilder_commands::types().unwrap())),
            Arc::new(Evaluators::new()),
            config,
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

    /// The 10 x 6 house on the ground floor and a slab over it on Floor 1.
    fn house(&self) -> ObjectId {
        let c = [[0.0, 0.0], [0.0, 6.0], [10.0, 6.0], [10.0, 0.0]];
        for k in 0..4 {
            self.run(
                "archbuilder.wall.create",
                json!({"start": c[k], "end": c[(k + 1) % 4]}),
            )
            .unwrap();
        }
        let floor = self
            .run(
                "archbuilder.floor.create",
                json!({"storey": self.storey("Floor 1").to_string(),
                       "points": [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]]}),
            )
            .unwrap();
        floor.value["floor"].as_str().unwrap().parse().unwrap()
    }

    fn ramp(&self, params: Json) -> arc_base::Result<Outcome> {
        self.run("archbuilder.ramp.create", params)
    }

    fn railing(&self, id: ObjectId) -> Derived {
        let snapshot = self.snapshot();
        Derived::of(&snapshot, &spatial::get(&snapshot, id).unwrap()).unwrap()
    }

    fn floor_area(&self, id: ObjectId) -> f64 {
        let snapshot = self.snapshot();
        archbuilder_model::floor::Derived::of(&snapshot, &spatial::get(&snapshot, id).unwrap())
            .unwrap()
            .area()
    }
}

fn id_at(outcome: &Outcome, key: &str) -> ObjectId {
    outcome.value[key].as_str().unwrap().parse().unwrap()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn count(env: &Env, key: &str) -> usize {
    spatial::objects(&env.snapshot(), key).len()
}

#[test]
fn a_railing_by_path_gets_the_default_type_posts_balusters_and_one_undo_step() {
    let env = Env::new();
    let made = env
        .run(
            "archbuilder.railing.create",
            json!({"points": [[0.0, 0.0], [3.0, 0.0, 0.0], [3.0, 2.0, 0.0]], "arcs": [[1, 0.0]]}),
        )
        .unwrap();
    let id = id_at(&made, "railing");
    assert_eq!(made.value["posts"], 3);
    assert!(close(made.value["length"].as_f64().unwrap(), 5.0));
    // 3 m / 0.12 -> 25 parts (24 balusters), 2 m / 0.12 -> 17 parts (16).
    assert_eq!(made.value["balusters"], 40);
    assert_eq!(
        count(&env, RAILING_TYPE),
        1,
        "the default type is made once"
    );
    let d = env.railing(id);
    assert!(close(d.kind.height, 0.9));
    assert_eq!(d.rail[0][2], 0.9);
    // A second railing reuses the type.
    env.run(
        "archbuilder.railing.create",
        json!({"points": [[0.0, 3.0], [1.0, 3.0]]}),
    )
    .unwrap();
    assert_eq!(count(&env, RAILING_TYPE), 1);
    env.document.undo().wait().unwrap();
    assert_eq!(count(&env, RAILING), 1);
    env.document.undo().wait().unwrap();
    assert_eq!(count(&env, RAILING), 0);
    assert_eq!(count(&env, RAILING_TYPE), 0);
    env.document.redo().wait().unwrap();
    assert_eq!(count(&env, RAILING), 1);
}

#[test]
fn an_arc_path_and_a_bad_path_are_handled() {
    let env = Env::new();
    let made = env
        .run(
            "archbuilder.railing.create",
            json!({"points": [[0.0, 0.0], [2.0, 0.0]], "arcs": [[0, 1.0]]}),
        )
        .unwrap();
    // A half circle of diameter 2 m: pi meters.
    let length = made.value["length"].as_f64().unwrap();
    assert!((length - std::f64::consts::PI).abs() < 0.02, "{length}");
    assert!(
        env.run(
            "archbuilder.railing.create",
            json!({"points": [[0.0, 0.0]]})
        )
        .is_err()
    );
    assert!(
        env.run(
            "archbuilder.railing.create",
            json!({"points": [[0.0, 0.0], [0.0, 0.0]]})
        )
        .is_err()
    );
    assert!(
        env.run(
            "archbuilder.railing.create",
            json!({"points": [[0.0, 0.0], [1.0, 0.0]], "start_extension": "bogus"})
        )
        .is_err()
    );
    assert_eq!(count(&env, RAILING), 1);
}

#[test]
fn a_railing_type_sets_height_profiles_and_spacing_and_wider_spacing_warns() {
    let env = Env::new();
    let made = env
        .run(
            "archbuilder.railing_type.define",
            json!({"name": "Wide", "height": 1.1, "rail_profile": "round",
                   "baluster_spacing": 0.3, "handrail_height": 0.7, "corner": "fillet"}),
        )
        .unwrap();
    let kind = made.value["type"].as_str().unwrap().to_owned();
    let r = env
        .run(
            "archbuilder.railing.create",
            json!({"points": [[0.0, 0.0], [3.0, 0.0], [3.0, 3.0]], "railing_type": "Wide"}),
        )
        .unwrap();
    let id = id_at(&r, "railing");
    let d = env.railing(id);
    assert!(close(d.kind.height, 1.1) && !d.handrail.is_empty());
    assert!(d.widest_gap() > 0.1);
    let snapshot = env.snapshot();
    let object = spatial::get(&snapshot, id).unwrap();
    let warnings = archbuilder_model::validate::circulation_warnings(&snapshot, &object);
    assert_eq!(warnings[0].code(), "ab.model.railing_gap");
    // Editing the type through its name changes every railing that uses it.
    env.run(
        "archbuilder.railing_type.define",
        json!({"type": kind, "baluster_spacing": 0.1}),
    )
    .unwrap();
    let snapshot = env.snapshot();
    let object = spatial::get(&snapshot, id).unwrap();
    assert!(archbuilder_model::validate::circulation_warnings(&snapshot, &object).is_empty());
    // A type that is in use cannot be deleted by itself, and the property grid reassigns it.
    env.run(
        "archbuilder.property.set",
        json!({"objects": [id.to_string()], "field": "railing_type", "value": ""}),
    )
    .unwrap();
    assert!(
        close(env.railing(id).kind.height, 0.9),
        "no type: the default values"
    );
}

#[test]
fn extensions_and_a_continuing_rail_are_authored_and_derived() {
    let env = Env::new();
    let a = id_at(
        &env.run(
            "archbuilder.railing.create",
            json!({"points": [[0.0, 0.0], [2.0, 0.0]]}),
        )
        .unwrap(),
        "railing",
    );
    let plain = env.railing(a).rail.len();
    env.run(
        "archbuilder.property.set",
        json!({"objects": [a.to_string()], "field": "end_extension", "value": "return_wall"}),
    )
    .unwrap();
    env.run(
        "archbuilder.property.set",
        json!({"objects": [a.to_string()], "field": "start_extension", "value": "horizontal"}),
    )
    .unwrap();
    let d = env.railing(a);
    assert_eq!(d.rail.len(), plain + 3);
    assert_eq!(d.rail[0], [-0.3, 0.0, 0.9]);
    assert_eq!(d.rail[d.rail.len() - 1], [2.3, 0.1, 0.9]);
    // A second railing, and the first continues to it: one rail across the gap, no end extension.
    let b = id_at(
        &env.run(
            "archbuilder.railing.create",
            json!({"points": [[2.5, 0.0], [2.5, 2.0]], "name": "Second"}),
        )
        .unwrap(),
        "railing",
    );
    env.run(
        "archbuilder.property.set",
        json!({"objects": [a.to_string()], "field": "continues_to", "value": "Second"}),
    )
    .unwrap();
    let d = env.railing(a);
    assert_eq!(d.rail.last(), Some(&[2.5, 0.0, 0.9]));
    assert_eq!(
        d.rail.len(),
        plain + 2,
        "start extension and the continuation"
    );
    // The railing cannot continue to itself; deleting the target clears the link.
    assert!(
        env.run(
            "archbuilder.property.set",
            json!({"objects": [a.to_string()], "field": "continues_to", "value": a.to_string()}),
        )
        .is_err()
    );
    let invocation = Invocation {
        seed: 7,
        selection: vec![b],
        ..Invocation::new("archbuilder.object.delete", json!({}))
    };
    Invoker::new(&env.registry)
        .invoke(&env.document, &env.session, invocation)
        .unwrap();
    assert_eq!(env.railing(a).rail.len(), plain + 3);
}

#[test]
fn a_ramp_warns_over_its_slope_cuts_the_floor_above_and_gets_hosted_railings() {
    let env = Env::new();
    let floor = env.house();
    // 3 m up over 6 m: 50 %, far over 1 in 12.
    let made = env
        .ramp(json!({"start": [1.0, 1.0], "to": [7.0, 1.0], "landing_end": 1}))
        .unwrap();
    let ramp = id_at(&made, "ramp");
    assert!(close(made.value["slope"].as_f64().unwrap(), 0.5));
    assert_eq!(made.value["slope_exceeded"], true);
    assert_eq!(made.value["railings"].as_array().unwrap().len(), 2);
    assert_eq!(count(&env, RAILING), 2);
    let snapshot = env.snapshot();
    let object = spatial::get(&snapshot, ramp).unwrap();
    let w = archbuilder_model::validate::circulation_warnings(&snapshot, &object);
    assert_eq!(w[0].code(), "ab.model.ramp_slope");
    assert!(
        archbuilder_model::validate::register(&snapshot)
            .iter()
            .any(|w| w.error.code() == "ab.model.ramp_slope")
    );
    // The opening covers the run (6 m x 1.3 m) and the arrival edge stays.
    let cut = 60.0 - env.floor_area(floor);
    assert!((cut - 6.05 * 1.3).abs() < 1e-6, "{cut}");
    let placed = Placed::of(&snapshot, &object).unwrap();
    let layout = placed.layout.unwrap();
    assert!(layout.solid(placed.base).unwrap().volume() > 1.0);
    // Both railings follow the ramp: their rail ends 3 m up, 0.9 above the landing.
    for r in spatial::objects(&snapshot, RAILING) {
        let d = Derived::of(&snapshot, &r).unwrap();
        assert!(close(d.rail.last().unwrap()[2], 3.9));
        assert_eq!(
            d.posts().len(),
            3,
            "the ends and the foot of the top landing"
        );
    }
    // Undo is one step for the ramp, its railings, its type and the opening.
    env.document.undo().wait().unwrap();
    assert_eq!(count(&env, RAMP) + count(&env, RAILING), 0);
    assert!(close(env.floor_area(floor), 60.0));
    env.document.redo().wait().unwrap();
    assert_eq!(count(&env, RAILING), 2);
}

#[test]
fn a_gentle_ramp_has_no_warning_and_a_rise_keeps_the_floor_whole() {
    let env = Env::new();
    let floor = env.house();
    let made = env
        .ramp(json!({"start": [1.0, 1.0], "to": [7.0, 1.0], "rise": 0.5,
                     "railings": ["left"], "form": "solid"}))
        .unwrap();
    assert_eq!(made.value["slope_exceeded"], false);
    assert_eq!(made.value["railings"].as_array().unwrap().len(), 1);
    assert!(close(env.floor_area(floor), 60.0));
    // A type with a smaller largest slope makes the same ramp a warning.
    env.run(
        "archbuilder.ramp_type.define",
        json!({"name": "Strict", "max_slope": 0.05}),
    )
    .unwrap();
    let strict = env
        .ramp(json!({"start": [1.0, 4.0], "to": [7.0, 4.0], "rise": 0.5, "ramp_type": "Strict"}))
        .unwrap();
    assert_eq!(strict.value["slope_exceeded"], true);
    assert!(
        env.ramp(json!({"start": [1.0, 4.0], "to": [1.0, 4.0]}))
            .is_err()
    );
}

#[test]
fn ramps_and_hosted_railings_move_copy_and_delete_together() {
    let env = Env::new();
    env.house();
    let made = env
        .ramp(json!({"start": [1.0, 1.0], "to": [7.0, 1.0], "rise": 0.4}))
        .unwrap();
    let ramp = id_at(&made, "ramp");
    let path_before = {
        let snapshot = env.snapshot();
        let r = spatial::objects(&snapshot, RAILING).remove(0);
        Derived::of(&snapshot, &r).unwrap().rail[0]
    };
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [ramp.to_string()], "displacement": [0.0, 2.0]}),
    )
    .unwrap();
    let snapshot = env.snapshot();
    let moved = Derived::of(&snapshot, &spatial::objects(&snapshot, RAILING).remove(0)).unwrap();
    assert!(
        close(moved.rail[0][1] - path_before[1], 2.0),
        "the hosted railing follows"
    );
    let copied = env
        .run(
            "archbuilder.modify.copy",
            json!({"objects": [ramp.to_string()], "displacement": [0.0, 1.5]}),
        )
        .unwrap();
    assert_eq!(copied.value["created"].as_array().unwrap().len(), 3);
    assert_eq!((count(&env, RAMP), count(&env, RAILING)), (2, 4));
    // A railing by path moves by itself.
    let free = id_at(
        &env.run(
            "archbuilder.railing.create",
            json!({"points": [[0.0, 0.0], [1.0, 0.0]]}),
        )
        .unwrap(),
        "railing",
    );
    env.run(
        "archbuilder.modify.rotate",
        json!({"objects": [free.to_string()], "base": [0.0, 0.0], "angle": "90"}),
    )
    .unwrap();
    let d = env.railing(free);
    assert!(close(d.rail[1][0], 0.0) && close(d.rail[1][1], 1.0));
    // Deleting a ramp takes its railings with it, and undo brings them back.
    let invocation = Invocation {
        seed: 7,
        selection: vec![ramp],
        ..Invocation::new("archbuilder.object.delete", json!({}))
    };
    Invoker::new(&env.registry)
        .invoke(&env.document, &env.session, invocation)
        .unwrap();
    assert_eq!((count(&env, RAMP), count(&env, RAILING)), (1, 3));
    env.document.undo().wait().unwrap();
    assert_eq!((count(&env, RAMP), count(&env, RAILING)), (2, 5));
}

fn type_of_railing(env: &Env, id: ObjectId) -> ObjectId {
    match spatial::get(&env.snapshot(), id).unwrap().values
        [archbuilder_model::types::railing::RAILING_TYPE]
    {
        Some(arc_doc::Value::Reference(t)) => t,
        _ => panic!("no type"),
    }
}

#[test]
fn railing_and_ramp_types_duplicate_match_and_delete_with_undo() {
    let env = Env::new();
    let a = id_at(
        &env.run(
            "archbuilder.railing.create",
            json!({"points": [[0.0, 0.0], [2.0, 0.0]]}),
        )
        .unwrap(),
        "railing",
    );
    let b = id_at(
        &env.run(
            "archbuilder.railing.create",
            json!({"points": [[0.0, 1.0], [2.0, 1.0]]}),
        )
        .unwrap(),
        "railing",
    );
    let first = type_of_railing(&env, a);
    // Duplicate: the copy has the values of the original and the next free name.
    let copy = id_at(
        &env.run(
            "archbuilder.type.duplicate",
            json!({"type": first.to_string()}),
        )
        .unwrap(),
        "type",
    );
    assert_eq!(count(&env, RAILING_TYPE), 2);
    let snapshot = env.snapshot();
    let (o, c) = (
        spatial::get(&snapshot, first).unwrap(),
        spatial::get(&snapshot, copy).unwrap(),
    );
    assert_eq!(text(&c.values, 0), Some("Railing 900 (2)"));
    assert_eq!(o.values[1..], c.values[1..]);
    // Editing the copy through the define command, then matching a railing to it.
    env.run(
        "archbuilder.railing_type.define",
        json!({"type": copy.to_string(), "height": 1.1}),
    )
    .unwrap();
    env.run(
        "archbuilder.property.set",
        json!({"objects": [b.to_string()], "field": "railing_type", "value": copy.to_string()}),
    )
    .unwrap();
    env.run(
        "archbuilder.type.match",
        json!({"source": b.to_string(), "objects": [a.to_string()]}),
    )
    .unwrap();
    assert_eq!(type_of_railing(&env, a), copy);
    assert!(close(env.railing(a).kind.height, 1.1));
    env.document.undo().wait().unwrap();
    assert_eq!(type_of_railing(&env, a), first);
    // A type in use is not deleted without a replacement; with one its users move over.
    let refused = env.run("archbuilder.type.delete", json!({"type": copy.to_string()}));
    assert!(refused.is_err());
    env.run(
        "archbuilder.type.delete",
        json!({"type": copy.to_string(), "replacement": first.to_string()}),
    )
    .unwrap();
    assert_eq!(count(&env, RAILING_TYPE), 1);
    assert_eq!(type_of_railing(&env, b), first);
    env.document.undo().wait().unwrap();
    assert_eq!(count(&env, RAILING_TYPE), 2);
    assert_eq!(type_of_railing(&env, b), copy);
}

#[test]
fn ramp_types_duplicate_and_match_and_a_ramp_type_does_not_retype_a_railing() {
    let env = Env::new();
    let a = id_at(
        &env.run(
            "archbuilder.railing.create",
            json!({"points": [[0.0, 0.0], [2.0, 0.0]]}),
        )
        .unwrap(),
        "railing",
    );
    let first = type_of_railing(&env, a);
    let r1 = id_at(
        &env.ramp(json!({"start": [0.0, 0.0], "to": [6.0, 0.0], "rise": 0.5}))
            .unwrap(),
        "ramp",
    );
    let r2 = id_at(
        &env.ramp(json!({"start": [0.0, 3.0], "to": [6.0, 3.0], "rise": 0.5}))
            .unwrap(),
        "ramp",
    );
    let kind = |id: ObjectId| match spatial::get(&env.snapshot(), id).unwrap().values
        [archbuilder_model::types::ramp::RAMP_TYPE]
    {
        Some(arc_doc::Value::Reference(t)) => t,
        _ => panic!("no type"),
    };
    let ramp_type = kind(r1);
    let copy = id_at(
        &env.run(
            "archbuilder.type.duplicate",
            json!({"type": ramp_type.to_string(), "name": "Steep"}),
        )
        .unwrap(),
        "type",
    );
    env.run(
        "archbuilder.ramp_type.define",
        json!({"type": "Steep", "max_slope": 0.2}),
    )
    .unwrap();
    env.run(
        "archbuilder.property.set",
        json!({"objects": [r1.to_string()], "field": "ramp_type", "value": "Steep"}),
    )
    .unwrap();
    env.run(
        "archbuilder.type.match",
        json!({"source": r1.to_string(), "objects": [r2.to_string()]}),
    )
    .unwrap();
    assert_eq!((kind(r1), kind(r2)), (copy, copy));
    // A railing is not retyped by a ramp's type.
    env.run(
        "archbuilder.type.match",
        json!({"source": r1.to_string(), "objects": [a.to_string()]}),
    )
    .unwrap();
    assert_eq!(type_of_railing(&env, a), first);
    env.document.undo().wait().unwrap();
    env.document.undo().wait().unwrap();
    assert_eq!(kind(r2), ramp_type);
}

fn quantity(env: &Env, id: ObjectId, f: impl Fn(&Derived) -> f64) -> f64 {
    f(&env.railing(id))
}

#[test]
fn a_railing_on_a_floor_edge_or_boundary_follows_the_floor_and_goes_with_it() {
    let env = Env::new();
    let floor = env.house();
    let host = |params: Json| {
        id_at(
            &env.run("archbuilder.railing.create", params).unwrap(),
            "railing",
        )
    };
    // The whole outer boundary, one boundary edge, and a floor edge element's chain.
    let all = host(json!({"host": floor.to_string()}));
    let one = host(json!({"host": floor.to_string(), "host_edge": 0}));
    let element = id_at(
        &env.run(
            "archbuilder.floor_edge.create",
            json!({"floor": floor.to_string(), "edges": [1]}),
        )
        .unwrap(),
        "floor_edge",
    );
    let chain = host(json!({"host": element.to_string()}));
    // Closed loop of 10 x 6 stood in 0.05 m, a single edge, a single edge.
    assert!(close(
        quantity(&env, all, Derived::length),
        2.0 * (9.9 + 5.9)
    ));
    assert!(close(quantity(&env, one, Derived::length), 10.0));
    assert!(close(quantity(&env, chain, Derived::length), 6.0));
    // The rail stands on the slab's top and 0.05 m inside the edge (edge 0 runs along y = 0).
    let d = env.railing(one);
    assert!(close(d.surface[0].0[1], 0.05) && close(d.surface[1].0[1], 0.05));
    assert!(close(d.surface[0].0[2], d.surface[1].0[2]));
    let z = d.surface[0].0[2];
    // The railing is in the storey of the floor.
    let storey = spatial::get(&env.snapshot(), one).unwrap().header.owner;
    assert_eq!(storey, Some(env.storey("Floor 1")));
    // It follows the floor: moving the floor moves the rail; the length stays.
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [floor.to_string()], "displacement": [1.0, 2.0]}),
    )
    .unwrap();
    let moved = env.railing(one);
    assert!(close(moved.surface[0].0[0], d.surface[0].0[0] + 1.0));
    assert!(close(moved.surface[0].0[1], 2.05));
    assert!(close(moved.surface[0].0[2], z));
    env.document.undo().wait().unwrap();
    assert!(close(env.railing(one).surface[0].0[0], d.surface[0].0[0]));
    // An edge the floor does not have is refused.
    assert!(
        env.run(
            "archbuilder.railing.create",
            json!({"host": floor.to_string(), "host_edge": 99})
        )
        .is_err()
    );
    // A railing is not a host.
    assert!(
        env.run(
            "archbuilder.railing.create",
            json!({"host": one.to_string()})
        )
        .is_err()
    );
    // Deleting the floor takes its railings, and those of its floor edge, with it; undo brings
    // them back in one step.
    let before = count(&env, RAILING);
    assert_eq!(before, 3);
    let invocation = Invocation {
        seed: 7,
        selection: vec![floor],
        ..Invocation::new("archbuilder.object.delete", json!({}))
    };
    Invoker::new(&env.registry)
        .invoke(&env.document, &env.session, invocation)
        .unwrap();
    assert_eq!(count(&env, RAILING), 0);
    env.document.undo().wait().unwrap();
    assert_eq!(count(&env, RAILING), 3);
    assert!(close(env.railing(chain).length(), 6.0));
}

fn max_y(d: &Derived) -> f64 {
    d.solids()
        .unwrap()
        .iter()
        .flat_map(|s| s.vertices().iter().map(|v| v.y))
        .fold(f64::MIN, f64::max)
}

#[test]
fn a_continuing_rail_runs_on_through_the_next_one_which_drops_its_own_start_extension() {
    let env = Env::new();
    // A runs along x to (2, 0); B turns up along y from there and has a start extension of its own.
    let b = id_at(
        &env.run(
            "archbuilder.railing.create",
            json!({"points": [[2.0, 0.0], [2.0, 2.0]], "start_extension": "horizontal"}),
        )
        .unwrap(),
        "railing",
    );
    let before = env.railing(b);
    assert_eq!(
        before.rail[0],
        [2.0, -0.3, 0.9],
        "alone, B has its extension"
    );
    assert!(!before.preceded);
    let a = id_at(
        &env.run(
            "archbuilder.railing.create",
            json!({"points": [[0.0, 0.0], [2.0, 0.0]], "continues_to": b.to_string(),
                   "end_extension": "horizontal"}),
        )
        .unwrap(),
        "railing",
    );
    // A's rail is joined to B's start and has no end extension; B has no start extension.
    let (da, db) = (env.railing(a), env.railing(b));
    assert!(db.preceded && !da.preceded);
    assert_eq!(db.rail[0], [2.0, 0.0, 0.9]);
    assert_eq!(da.rail.last(), Some(&[2.0, 0.0, 0.9]));
    // A's rail is mitered at B's first point and runs on to the middle of B's first bar: A's
    // solids reach y = 1 and no farther (the posts and balusters of A stay at y = 0).
    assert!(close(max_y(&da), 1.0));
    // Deleting the continuation clears the reference and B takes its extension back.
    let invocation = Invocation {
        seed: 7,
        selection: vec![a],
        ..Invocation::new("archbuilder.object.delete", json!({}))
    };
    Invoker::new(&env.registry)
        .invoke(&env.document, &env.session, invocation)
        .unwrap();
    assert!(!env.railing(b).preceded);
    assert_eq!(env.railing(b).rail[0], [2.0, -0.3, 0.9]);
    env.document.undo().wait().unwrap();
    assert!(env.railing(b).preceded);
}

#[test]
fn hosted_rails_have_transitions_at_landings_and_turns() {
    let env = Env::new();
    let made = env
        .run(
            "archbuilder.ramp.create",
            json!({"start": [0.0, 0.0], "to": [6.0, 0.0], "rise": 1.0, "landing_start": 1.0,
                   "landing_end": 1.5, "railings": ["left"]}),
        )
        .unwrap();
    let rail = made.value["railings"][0].as_str().unwrap().parse().unwrap();
    // The slope starts and ends at the landings; the heading never changes.
    let t = env.railing(rail).transitions();
    assert_eq!(t.len(), 2);
    assert!(
        t.iter()
            .all(|t| t.join == archbuilder_model::railing::Join::Landing)
    );
    assert!(close(t[0].point[0], 1.0) && close(t[1].point[0], 7.0));
    // A railing along a floor's boundary turns at its corners.
    let floor = env.house();
    let edge = id_at(
        &env.run(
            "archbuilder.railing.create",
            json!({"host": floor.to_string()}),
        )
        .unwrap(),
        "railing",
    );
    let t = env.railing(edge).transitions();
    assert_eq!(t.len(), 3);
    assert!(
        t.iter()
            .all(|t| t.join == archbuilder_model::railing::Join::Turn)
    );
    // The solids of a path with corners are mitered sweeps: one solid per rail, posts and
    // balusters apart.
    let solids = env.railing(edge).solids().unwrap();
    assert!(!solids.is_empty());
}
