//! Floor slope, floor edges and roofs by extrusion.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::spatial;
use serde_json::{Value as Json, json};

/// Every environment draws its identifiers from its own seed: the derived caches are keyed by
/// identifier, and tests that run side by side must not share them.
static SEEDS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

struct Env {
    seed: u64,
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
            seed: SEEDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            document,
            session: Session::new(Surface::Ui, [Permission::Read, Permission::Edit]),
            registry: archbuilder_commands::registry().unwrap(),
        };
        env.run("archbuilder.project.new", json!({})).unwrap();
        env
    }

    fn run(&self, command: &str, params: Json) -> arc_base::Result<Outcome> {
        let invocation = Invocation {
            seed: self.seed,
            ..Invocation::new(command, params)
        };
        Invoker::new(&self.registry).invoke(&self.document, &self.session, invocation)
    }

    fn snapshot(&self) -> Snapshot {
        self.document.snapshot()
    }

    fn storeys(&self) -> Vec<ObjectId> {
        let s = self.snapshot();
        let mut found: Vec<_> = spatial::objects(&s, archbuilder_model::types::STOREY)
            .iter()
            .map(|o| (spatial::storey_elevation(&s, o), o.id))
            .collect();
        found.sort_by(|a, b| a.0.total_cmp(&b.0));
        found.into_iter().map(|(_, id)| id).collect()
    }

    /// A 10 x 6 house of 0.2 m walls, 3 m high, on `storey`.
    fn house(&self, storey: ObjectId) -> Vec<ObjectId> {
        let c = [[0.0, 0.0], [0.0, 6.0], [10.0, 6.0], [10.0, 0.0]];
        (0..4)
            .map(|k| {
                let made = self
                    .run(
                        "archbuilder.wall.create",
                        json!({"start": c[k], "end": c[(k + 1) % 4], "height": 3.0,
                               "storey": storey.to_string()}),
                    )
                    .unwrap()
                    .value;
                made["wall"].as_str().unwrap().parse().unwrap()
            })
            .collect()
    }

    #[allow(dead_code)]
    fn room(&self, storey: ObjectId) -> ObjectId {
        let made = self
            .run(
                "archbuilder.room.place",
                json!({"points": [[5.0, 3.0]], "storey": storey.to_string()}),
            )
            .unwrap()
            .value;
        made["rooms"][0].as_str().unwrap().parse().unwrap()
    }

    #[allow(dead_code)]
    fn volume(&self, room: ObjectId) -> (f64, f64) {
        let s = self.snapshot();
        let entries = archbuilder_model::room::of_storey(
            &s,
            spatial::get(&s, room).unwrap().header.owner.unwrap(),
        );
        let entry = entries.iter().find(|e| e.id == room).unwrap();
        let shape = archbuilder_model::room::shape(&s, entry).unwrap();
        (shape.volume, shape.height)
    }

    /// Deletes `ids`, as the delete command does for a selection.
    #[allow(dead_code)]
    fn delete(&self, ids: &[ObjectId]) -> arc_base::Result<Outcome> {
        let invocation = Invocation {
            seed: self.seed,
            selection: ids.to_vec(),
            ..Invocation::new("archbuilder.object.delete", json!({}))
        };
        Invoker::new(&self.registry).invoke(&self.document, &self.session, invocation)
    }

    fn id(outcome: &Outcome, key: &str) -> ObjectId {
        outcome.value[key].as_str().unwrap().parse().unwrap()
    }
}

fn derived(env: &Env, floor: ObjectId) -> archbuilder_model::floor::Derived {
    let s = env.snapshot();
    archbuilder_model::floor::Derived::of(&s, &spatial::get(&s, floor).unwrap()).unwrap()
}

#[test]
fn a_floor_slope_is_authored_as_an_arrow_and_the_slab_keeps_its_volume() {
    let env = Env::new();
    let ground = env.storeys()[0];
    env.house(ground);
    let made = env
        .run("archbuilder.floor.create", json!({"picks": [[5.0, 3.0]]}))
        .unwrap();
    let floor = Env::id(&made, "floor");
    let level = derived(&env, floor);
    assert!(!level.sloped());
    let (area, thickness) = (level.area(), level.floor.thickness);
    // Rises 0.5 m over the 9.8 m of the room's width, from the top offset at x = 0.1.
    env.run(
        "archbuilder.floor.set_slope",
        json!({"floor": floor.to_string(), "from": [0.1, 3.0], "to": [9.9, 3.0],
               "end_height": 0.5}),
    )
    .unwrap();
    let d = derived(&env, floor);
    assert!(d.sloped());
    assert!((d.plane.z([9.9, 3.0]) - 0.5).abs() < 1e-9);
    let (lo, hi) = d.z_range();
    assert!(
        (hi - 0.5).abs() < 0.01 && (lo - (-thickness)).abs() < 0.01,
        "{lo} {hi}"
    );
    // The solid is sheared: the volume is the plan area times the thickness.
    let solid = d.solid().unwrap();
    let volume = solid.volume();
    assert!((volume - area * thickness).abs() < 1e-6, "{volume}");
    // The cleaned solid the viewport and the export use is sheared too.
    let s = env.snapshot();
    let clean = archbuilder_model::cleanup::floor_solid(&s, &spatial::get(&s, floor).unwrap())
        .unwrap()
        .unwrap();
    let top = clean
        .vertices()
        .iter()
        .map(|v| v.z)
        .fold(f64::MIN, f64::max);
    assert!((top - 0.5).abs() < 0.01, "{top}");
    // Too steep is refused; a typed property slope works; undo restores the level slab.
    assert!(
        env.run(
            "archbuilder.floor.set_slope",
            json!({"floor": floor.to_string(), "from": [0.0, 0.0], "to": [1.0, 0.0],
                   "slope": 1.5}),
        )
        .is_err()
    );
    env.run(
        "archbuilder.property.set",
        json!({"objects": [floor.to_string()], "field": "floor_slope", "value": "5%"}),
    )
    .unwrap();
    assert!((derived(&env, floor).plane.grad[0] - 0.05).abs() < 1e-9);
    env.document.undo().wait().unwrap();
    assert!((derived(&env, floor).plane.grad[0] - 0.5 / 9.8).abs() < 1e-9);
    env.document.undo().wait().unwrap();
    assert!(!derived(&env, floor).sloped());
}

#[test]
fn a_floor_slopes_from_an_edge_or_through_three_elevations() {
    let env = Env::new();
    let made = env
        .run(
            "archbuilder.floor.create",
            json!({"points": [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]]}),
        )
        .unwrap();
    let floor = Env::id(&made, "floor");
    // The edge from the first vertex (y = 0) stays; the slab rises away from it by 10 %.
    env.run(
        "archbuilder.floor.set_slope_edge",
        json!({"floor": floor.to_string(), "edge": 0, "slope": "10%"}),
    )
    .unwrap();
    let d = derived(&env, floor);
    assert!(d.plane.z([2.0, 0.0]).abs() < 1e-9);
    assert!((d.plane.z([2.0, 4.0]) - 0.4).abs() < 1e-9);
    // Three elevations: 0.1 at the origin, 0.3 at x = 4, 0.5 at y = 4.
    env.run(
        "archbuilder.floor.set_slope_points",
        json!({"floor": floor.to_string(), "points": [[0.0, 0.0, 0.1], [4.0, 0.0, 0.3], [0.0, 4.0, 0.5]]}),
    )
    .unwrap();
    let d = derived(&env, floor);
    assert!((d.plane.z([4.0, 4.0]) - 0.7).abs() < 1e-9);
    assert!((d.plane.z([0.0, 0.0]) - 0.1).abs() < 1e-9);
    // Three points on one line give no plane.
    assert!(
        env.run(
            "archbuilder.floor.set_slope_points",
            json!({"floor": floor.to_string(), "points": [[0.0, 0.0, 0.0], [1.0, 0.0, 0.1], [2.0, 0.0, 0.2]]}),
        )
        .is_err()
    );
    // Moving the floor moves its arrow with it.
    let before = derived(&env, floor).plane.z([4.0, 4.0]);
    let invocation = Invocation {
        seed: env.seed,
        selection: vec![floor],
        ..Invocation::new(
            "archbuilder.modify.move",
            json!({"displacement": [10.0, 0.0]}),
        )
    };
    Invoker::new(&env.registry)
        .invoke(&env.document, &env.session, invocation)
        .unwrap();
    assert!((derived(&env, floor).plane.z([14.0, 4.0]) - before).abs() < 1e-9);
}
