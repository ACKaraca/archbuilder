//! Opening recipes, symbols per detail, room relations, outlines sketched in a wall and dormer
//! openings, on a document with the model's validator.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::insert::{Insert, elevation_symbol, pieces, symbol_at, wall_solid};
use archbuilder_model::spatial;
use archbuilder_model::types::{DOOR, ROOF, VERTICAL_OPENING, WALL, WALL_OPENING, WINDOW};
use archbuilder_model::view::Detail;
use archbuilder_model::wall::Wall;
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

    fn dump(&self) -> String {
        let mut lines: Vec<String> = self
            .snapshot()
            .iter()
            .map(|(_, o)| format!("{} {:?} {:?}", o.id, o.header.owner, o.values))
            .collect();
        lines.sort();
        lines.join("\n")
    }

    fn id(&self, outcome: &Outcome, key: &str) -> ObjectId {
        outcome.value[key].as_str().unwrap().parse().unwrap()
    }

    fn wall(&self, a: [f64; 2], b: [f64; 2]) -> ObjectId {
        let made = self
            .run(
                "archbuilder.wall.create",
                json!({"start": a, "end": b, "height": 3.0}),
            )
            .unwrap();
        self.id(&made, "wall")
    }

    fn volume(&self, wall: ObjectId) -> f64 {
        let snapshot = self.snapshot();
        let object = spatial::get(&snapshot, wall).unwrap();
        wall_solid(&snapshot, &object).unwrap().unwrap().volume()
    }

    /// The wall and the insert of a record, for the symbol functions.
    fn insert(&self, id: ObjectId) -> (Wall, Insert) {
        let snapshot = self.snapshot();
        let object = spatial::get(&snapshot, id).unwrap();
        let host = spatial::get(&snapshot, object.header.owner.unwrap()).unwrap();
        (
            Wall::of(&snapshot, &host).unwrap(),
            Insert::of(&snapshot, &object).unwrap(),
        )
    }
}

#[test]
fn recipes_make_their_operation_with_leaves_and_symbols_and_refuse_a_kind_that_cannot_operate() {
    let env = Env::new();
    let wall = env.wall([0.0, 0.0], [10.0, 0.0]);
    for (kind, operation, distance) in [
        ("door", "folding", 1.5),
        ("door", "pivot", 3.5),
        ("window", "awning", 5.5),
        ("window", "hopper", 7.0),
        ("window", "pivot", 8.5),
    ] {
        let placed = env
            .run(
                &format!("archbuilder.{kind}.place"),
                json!({"wall": wall, "distance": distance, "operation": operation, "width": 1.4}),
            )
            .unwrap();
        let (host, item) = env.insert(env.id(&placed, "insert"));
        assert_eq!(item.operation.name(), operation);
        // The recipe has a frame, panels and glazing of its own.
        let parts = pieces(&host, 0.0, &item).unwrap();
        let leaves = if operation == "folding" { 2 } else { 1 };
        assert_eq!(parts.len(), 4 + leaves, "{operation}");
        assert!(env.volume(wall) > 0.0);
    }
    // A door cannot be an awning and a window cannot be a pivot door recipe's folding twin: the
    // model refuses what the kind does not have, naming the insert, and nothing changes.
    let other = env.wall([0.0, 5.0], [10.0, 5.0]);
    let before = env.dump();
    let dump = env.dump();
    let refused = env
        .run(
            "archbuilder.door.place",
            json!({"wall": other, "distance": 5.0, "operation": "awning"}),
        )
        .unwrap_err();
    assert!(refused.code().contains("operation"), "{}", refused.code());
    assert_eq!(env.dump(), dump);
    assert_eq!(dump, before);
    // One undo removes the last insert and the type made for it.
    assert!(env.document.undo().wait().unwrap().value);
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(spatial::objects(&env.snapshot(), WINDOW).len(), 2);
}

#[test]
fn symbols_follow_the_detail_level_and_the_view_kind() {
    let env = Env::new();
    let wall = env.wall([0.0, 0.0], [10.0, 0.0]);
    let door = env
        .run(
            "archbuilder.door.place",
            json!({"wall": wall, "distance": 2.0}),
        )
        .unwrap();
    let window = env
        .run(
            "archbuilder.window.place",
            json!({"wall": wall, "distance": 6.0, "operation": "sliding"}),
        )
        .unwrap();
    let (host, d) = env.insert(env.id(&door, "insert"));
    let (_, w) = env.insert(env.id(&window, "insert"));
    let count = |item: &Insert, detail| symbol_at(&host, 0.0, item, detail).unwrap().len();
    // A hinged door keeps its swing at every detail; a window has only its outline when coarse.
    assert_eq!(count(&d, Detail::Coarse), 3);
    assert_eq!(count(&w, Detail::Coarse), 1);
    assert!(count(&w, Detail::Medium) > 1);
    // Fine detail adds the arrow and the sill line of a sliding window.
    assert!(count(&w, Detail::Fine) > count(&w, Detail::Medium));
    // The elevation of a hinged door has a direction triangle whose apex is at the hinge.
    let lines = elevation_symbol(&host, 0.0, &d, Detail::Medium).unwrap();
    assert_eq!(lines.len(), 2);
    let triangle = &lines[1];
    let hinge = d.distance + d.hinge_direction() * d.width / 2.0;
    assert!((triangle[1][0] - hinge).abs() < 1e-9 && (triangle[1][2] - 1.05).abs() < 1e-9);
    // Fine detail has the inner frame; the window's sill line is there too.
    assert_eq!(
        elevation_symbol(&host, 0.0, &w, Detail::Coarse)
            .unwrap()
            .len(),
        1
    );
    assert!(
        elevation_symbol(&host, 0.0, &w, Detail::Fine)
            .unwrap()
            .len()
            >= 4
    );
}

#[test]
fn a_door_knows_its_rooms_and_flipping_it_swaps_them() {
    let env = Env::new();
    let c = [[0.0, 0.0], [0.0, 6.0], [10.0, 6.0], [10.0, 0.0]];
    for k in 0..4 {
        env.wall(c[k], c[(k + 1) % 4]);
    }
    let partition = env.wall([5.0, 0.0], [5.0, 6.0]);
    env.run("archbuilder.room.place", json!({"points": [[2.5, 3.0]]}))
        .unwrap();
    env.run("archbuilder.room.place", json!({"points": [[7.5, 3.0]]}))
        .unwrap();
    let door = env
        .run(
            "archbuilder.door.place",
            json!({"wall": partition, "distance": 3.0}),
        )
        .unwrap();
    let door = env.id(&door, "insert");
    let rooms = |env: &Env| {
        let out = env
            .run("archbuilder.insert.rooms", json!({"inserts": [door]}))
            .unwrap();
        let entry = out.value["inserts"][0].clone();
        (
            entry["from_room"].as_str().map(str::to_owned),
            entry["to_room"].as_str().map(str::to_owned),
        )
    };
    let (from, to) = rooms(&env);
    assert!(from.is_some() && to.is_some() && from != to);
    env.run("archbuilder.insert.flip_facing", json!({"inserts": [door]}))
        .unwrap();
    assert_eq!(rooms(&env), (to.clone(), from.clone()), "flipping swaps");
    // A door in the outer wall has a room on one side only.
    let outer = spatial::objects(&env.snapshot(), WALL)
        .iter()
        .map(|o| o.id)
        .find(|id| {
            Wall::of(
                &env.snapshot(),
                &spatial::get(&env.snapshot(), *id).unwrap(),
            )
            .is_some_and(|w| w.start == [10.0, 6.0] && w.end == [10.0, 0.0])
        })
        .unwrap();
    let exit = env
        .run(
            "archbuilder.door.place",
            json!({"wall": outer, "distance": 3.0}),
        )
        .unwrap();
    let exit = env.id(&exit, "insert");
    let out = env
        .run("archbuilder.insert.rooms", json!({"inserts": [exit]}))
        .unwrap();
    let entry = &out.value["inserts"][0];
    assert!(entry["from_room"].is_null() != entry["to_room"].is_null());
    // The query changes nothing.
    let dump = env.dump();
    env.run("archbuilder.insert.rooms", json!({})).unwrap();
    assert_eq!(env.dump(), dump);
}

#[test]
fn an_opening_takes_a_sketched_outline_and_the_wall_loses_exactly_its_area() {
    let env = Env::new();
    let wall = env.wall([0.0, 0.0], [10.0, 0.0]);
    // A pentagon house shape: 2 m wide, 2.5 m high with a pitched top.
    let outline = json!([[4.0, 0.0], [6.0, 0.0], [6.0, 1.5], [5.0, 2.5], [4.0, 1.5]]);
    let placed = env
        .run(
            "archbuilder.opening.place",
            json!({"wall": wall, "profile": outline}),
        )
        .unwrap();
    let id = env.id(&placed, "insert");
    let (_, item) = env.insert(id);
    assert!((item.distance - 5.0).abs() < 1e-9 && (item.width - 2.0).abs() < 1e-9);
    assert!((item.height - 2.5).abs() < 1e-9 && item.sill.abs() < 1e-9);
    let area = 2.0 * 1.5 + 0.5 * 2.0 * 1.0;
    assert!((env.volume(wall) - (6.0 - area * 0.2)).abs() < 1e-9);
    // A crossing outline, a size next to it and too few points are refused; nothing changes.
    let dump = env.dump();
    for params in [
        json!({"wall": wall, "profile": [[1.0, 0.0], [2.0, 1.0], [2.0, 0.0], [1.0, 1.0]]}),
        json!({"wall": wall, "profile": [[1.0, 0.0], [2.0, 0.0]]}),
        json!({"wall": wall, "profile": [[1.0, 0.0], [2.0, 0.0], [2.0, 1.0]], "width": 1.0}),
    ] {
        assert!(env.run("archbuilder.opening.place", params).is_err());
    }
    assert_eq!(env.dump(), dump);
    assert!(env.document.undo().wait().unwrap().value);
    assert!(spatial::objects(&env.snapshot(), WALL_OPENING).is_empty());
}

#[test]
fn a_dormer_opening_follows_its_walls_and_the_roof_loses_the_cut() {
    let env = Env::new();
    let c = [[0.0, 0.0], [0.0, 6.0], [10.0, 6.0], [10.0, 0.0]];
    for k in 0..4 {
        env.wall(c[k], c[(k + 1) % 4]);
    }
    let made = env
        .run(
            "archbuilder.roof.create",
            json!({"picks": [[5.0, 3.0]], "slope": std::f64::consts::FRAC_PI_4}),
        )
        .unwrap();
    let roof = env.id(&made, "roof");
    let walls = [
        env.wall([4.0, 2.0], [4.0, 4.0]),
        env.wall([4.0, 4.0], [6.0, 4.0]),
        env.wall([6.0, 4.0], [6.0, 2.0]),
    ];
    let before = env.dump();
    let cut = env
        .run(
            "archbuilder.dormer_opening.create",
            json!({"roof": roof, "walls": walls}),
        )
        .unwrap();
    // Between the interior faces of the cheeks and the front, closed by the roof: 1.8 x 1.9.
    assert!((cut.value["area"].as_f64().unwrap() - 1.8 * 1.9).abs() < 1e-9);
    let opening = env.id(&cut, "opening");
    assert_eq!(
        spatial::get(&env.snapshot(), opening).unwrap().header.owner,
        Some(roof)
    );
    let area = |env: &Env| {
        let snapshot = env.snapshot();
        let object = spatial::get(&snapshot, opening).unwrap();
        archbuilder_model::floor::Floor::read(
            &object.values,
            archbuilder_model::floor::VERTICAL_SLOTS,
        )
        .unwrap()
        .boundary_in(&snapshot)
        .unwrap()
        .area()
    };
    // The roof is cut where the dormer meets it.
    let snapshot = env.snapshot();
    let roof_object = spatial::get(&snapshot, roof).unwrap();
    let derived = archbuilder_model::roof::Derived::of(&snapshot, &roof_object).unwrap();
    assert_eq!(derived.cuts.len(), 1);
    // Moving the front wall one meter moves the cut with it.
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [walls[1].to_string()], "displacement": [0.0, 1.0]}),
    )
    .unwrap();
    assert!((area(&env) - 1.8 * 2.9).abs() < 1e-6, "{}", area(&env));
    // One undo removes the move, another the opening.
    assert!(env.document.undo().wait().unwrap().value);
    assert!((area(&env) - 1.8 * 1.9).abs() < 1e-9);
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(env.dump(), before);
    assert!(env.document.redo().wait().unwrap().value);
    assert_eq!(spatial::objects(&env.snapshot(), VERTICAL_OPENING).len(), 1);
    // A wall that is no wall, and a host that is no roof, are refused.
    let refused = env.run(
        "archbuilder.dormer_opening.create",
        json!({"roof": walls[0], "walls": walls}),
    );
    assert!(refused.is_err());
    assert_eq!(spatial::objects(&env.snapshot(), ROOF).len(), 1);
    assert_eq!(spatial::objects(&env.snapshot(), DOOR).len(), 0);
}
