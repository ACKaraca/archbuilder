//! The editing commands: align, offset, trim, split, merge, arrays and pin, and the warnings
//! register that clears when its cause is fixed. One test per behavior that would break silently.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::insert::hosted;
use archbuilder_model::spatial;
use archbuilder_model::types::{DOOR, WALL};
use archbuilder_model::wall::Wall;
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

    fn wall(&self, start: [f64; 2], end: [f64; 2]) -> ObjectId {
        self.run(
            "archbuilder.wall.create",
            json!({"start": start, "end": end}),
        )
        .unwrap()
        .value["wall"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap()
    }

    fn door(&self, wall: ObjectId, distance: f64) -> ObjectId {
        self.run(
            "archbuilder.door.place",
            json!({"wall": wall, "distance": distance}),
        )
        .unwrap();
        let s = self.snapshot();
        hosted(&s, wall)
            .into_iter()
            .find(|(_, i)| (i.distance - distance).abs() < 1e-9)
            .unwrap()
            .0
            .id
    }

    fn snapshot(&self) -> Snapshot {
        self.document.snapshot()
    }

    fn get(&self, id: ObjectId) -> Wall {
        let s = self.snapshot();
        Wall::from_values(&spatial::get(&s, id).unwrap().values).unwrap()
    }

    fn walls(&self) -> Vec<(ObjectId, Wall)> {
        let s = self.snapshot();
        spatial::objects(&s, WALL)
            .iter()
            .map(|o| (o.id, Wall::from_values(&o.values).unwrap()))
            .collect()
    }

    fn host_of(&self, insert: ObjectId) -> (ObjectId, f64) {
        let s = self.snapshot();
        let o = spatial::get(&s, insert).unwrap();
        let host = o.header.owner.unwrap();
        let d = hosted(&s, host)
            .into_iter()
            .find(|(x, _)| x.id == insert)
            .unwrap()
            .1
            .distance;
        (host, d)
    }
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} vs {b}");
}

#[test]
fn split_sends_each_door_to_the_piece_that_holds_it_and_refuses_a_door_across_the_cut() {
    let env = Env::new();
    let wall = env.wall([0.0, 0.0], [10.0, 0.0]);
    let near_start = env.door(wall, 2.0);
    let near_end = env.door(wall, 8.0);
    // Across the cut: refused, nothing changes.
    let refused = env
        .run("archbuilder.wall.split", json!({"at": [2.2, 0.0]}))
        .unwrap_err();
    assert_eq!(refused.code(), "ab.cmd.split_insert");
    assert_eq!(env.walls().len(), 1);
    let done = env
        .run(
            "archbuilder.wall.split",
            json!({"at": [5.0, 0.05], "gap": 0.4}),
        )
        .unwrap()
        .value;
    let second: ObjectId = done["created"][0].as_str().unwrap().parse().unwrap();
    let (a, b) = (env.get(wall), env.get(second));
    assert_eq!((a.start, a.end), ([0.0, 0.0], [4.8, 0.0]));
    assert_eq!((b.start, b.end), ([5.2, 0.0], [10.0, 0.0]));
    assert_eq!(env.host_of(near_start), (wall, 2.0));
    let (host, d) = env.host_of(near_end);
    assert_eq!(host, second);
    near(d, 2.8);
    env.document.undo().wait().unwrap();
    assert_eq!(env.walls().len(), 1);
    assert_eq!(env.host_of(near_end), (wall, 8.0));
}

#[test]
fn merge_joins_two_collinear_walls_and_keeps_both_walls_doors_in_place() {
    let env = Env::new();
    let first = env.wall([0.0, 0.0], [4.0, 0.0]);
    // Drawn the other way: its door keeps its place and its side.
    let second = env.wall([9.0, 0.0], [4.0, 0.0]);
    let own = env.door(first, 1.0);
    let moved = env.door(second, 2.0);
    env.run(
        "archbuilder.wall.merge",
        json!({"first": [2.0, 0.0], "other": [6.0, 0.0]}),
    )
    .unwrap();
    let walls = env.walls();
    assert_eq!(walls.len(), 1);
    assert_eq!(walls[0].0, first);
    assert_eq!((walls[0].1.start, walls[0].1.end), ([0.0, 0.0], [9.0, 0.0]));
    assert_eq!(env.host_of(own), (first, 1.0));
    let (host, d) = env.host_of(moved);
    assert_eq!(host, first);
    near(d, 7.0);
    // Different heights do not merge.
    let env = Env::new();
    env.wall([0.0, 0.0], [4.0, 0.0]);
    let tall = env.wall([4.0, 0.0], [8.0, 0.0]);
    env.run(
        "archbuilder.property.set",
        json!({"objects": [tall], "field": "height", "value": 4.0}),
    )
    .unwrap();
    let refused = env
        .run(
            "archbuilder.wall.merge",
            json!({"first": [2.0, 0.0], "other": [6.0, 0.0]}),
        )
        .unwrap_err();
    assert_eq!(refused.code(), "ab.cmd.merge_refused");
}

#[test]
fn trim_to_corner_keeps_the_picked_parts_and_meets_at_the_corner() {
    let env = Env::new();
    // One wall runs past the corner, the other stops short of it.
    let a = env.wall([0.0, 0.0], [6.0, 0.0]);
    let b = env.wall([4.0, 1.0], [4.0, 5.0]);
    env.run(
        "archbuilder.modify.trim",
        json!({"first": [1.0, 0.0], "other": [4.0, 3.0]}),
    )
    .unwrap();
    assert_eq!((env.get(a).start, env.get(a).end), ([0.0, 0.0], [4.0, 0.0]));
    assert_eq!((env.get(b).start, env.get(b).end), ([4.0, 0.0], [4.0, 5.0]));
}

#[test]
fn a_pinned_wall_refuses_move_and_delete_naming_it_until_unpinned() {
    let env = Env::new();
    let wall = env.wall([0.0, 0.0], [4.0, 0.0]);
    env.run("archbuilder.element.pin", json!({"objects": [wall]}))
        .unwrap();
    let moved = env
        .run(
            "archbuilder.modify.move",
            json!({"objects": [wall], "displacement": [1.0, 0.0]}),
        )
        .unwrap_err();
    assert_eq!(moved.code(), "ab.model.pinned");
    assert!(format!("{:?}", moved.arguments()).contains("Wall 1"));
    let deleted = env
        .run("archbuilder.wall.delete", json!({"walls": [wall]}))
        .unwrap_err();
    assert_eq!(deleted.code(), "ab.model.pinned");
    // A rename still works; unpinned, the move does.
    env.run(
        "archbuilder.property.set",
        json!({"objects": [wall], "field": "name", "value": "North"}),
    )
    .unwrap();
    env.run("archbuilder.element.unpin", json!({"objects": [wall]}))
        .unwrap();
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [wall], "displacement": [1.0, 0.0]}),
    )
    .unwrap();
    assert_eq!(env.get(wall).start, [1.0, 0.0]);
}

#[test]
fn a_linear_array_makes_count_minus_one_copies_at_the_spacing_or_up_to_the_last() {
    let env = Env::new();
    let wall = env.wall([0.0, 0.0], [0.0, 3.0]);
    let made = env
        .run(
            "archbuilder.modify.array",
            json!({"objects": [wall], "count": 4, "displacement": [2.0, 0.0]}),
        )
        .unwrap()
        .value;
    assert_eq!(made["created"].as_array().unwrap().len(), 3);
    let mut xs: Vec<f64> = env.walls().iter().map(|(_, w)| w.start[0]).collect();
    xs.sort_by(f64::total_cmp);
    assert_eq!(xs, [0.0, 2.0, 4.0, 6.0]);
    env.document.undo().wait().unwrap();
    env.run(
        "archbuilder.modify.array",
        json!({"objects": [wall], "count": 3, "displacement": [9.0, 0.0], "last": true}),
    )
    .unwrap();
    let mut xs: Vec<f64> = env.walls().iter().map(|(_, w)| w.start[0]).collect();
    xs.sort_by(f64::total_cmp);
    assert_eq!(xs, [0.0, 4.5, 9.0]);
    // A door alone arrays only along its wall.
    let door = env.door(wall, 1.0);
    let refused = env
        .run(
            "archbuilder.modify.array",
            json!({"objects": [door], "count": 2, "displacement": [1.0, 0.0]}),
        )
        .unwrap_err();
    assert_eq!(refused.code(), "ab.cmd.array_insert");
    env.run(
        "archbuilder.modify.array",
        json!({"objects": [door], "count": 2, "displacement": [0.0, 1.2]}),
    )
    .unwrap();
    assert_eq!(spatial::objects(&env.snapshot(), DOOR).len(), 2);
}

#[test]
fn offset_moves_by_the_distance_toward_the_side_and_a_joined_chain_keeps_its_corner() {
    let env = Env::new();
    let a = env.wall([0.0, 0.0], [4.0, 0.0]);
    let b = env.wall([4.0, 0.0], [4.0, 3.0]);
    // The side point lies inside the corner: the chain moves up and left.
    env.run(
        "archbuilder.modify.offset",
        json!({"objects": [a, b], "side": [3.0, 1.0], "distance": 0.5, "copy": false}),
    )
    .unwrap();
    assert_eq!((env.get(a).start, env.get(a).end), ([0.0, 0.5], [3.5, 0.5]));
    assert_eq!((env.get(b).start, env.get(b).end), ([3.5, 0.5], [3.5, 3.0]));
    // A copy, with the distance taken from the point, on the other side.
    let made = env
        .run(
            "archbuilder.modify.offset",
            json!({"objects": [a], "side": [1.0, -1.5]}),
        )
        .unwrap()
        .value;
    let copy: ObjectId = made["created"][0].as_str().unwrap().parse().unwrap();
    assert_eq!(
        (env.get(copy).start, env.get(copy).end),
        ([0.0, -1.5], [3.5, -1.5])
    );
    assert_eq!(env.get(a).start, [0.0, 0.5]);
}

#[test]
fn align_turns_and_moves_a_wall_until_its_face_lies_on_the_reference_face() {
    let env = Env::new();
    // The reference: the top face (y = 0.1) of a 0.2 m wall along the x axis.
    env.wall([0.0, 0.0], [6.0, 0.0]);
    // A slightly turned wall above it; its lower face is picked.
    let moving = env.wall([1.0, 2.0], [5.0, 2.2]);
    let target = {
        let w = env.get(moving);
        let f = w.plain_footprint();
        [(f[0][0] + f[1][0]) / 2.0, (f[0][1] + f[1][1]) / 2.0]
    };
    env.run(
        "archbuilder.modify.align",
        json!({"reference": [3.0, 0.1], "target": target}),
    )
    .unwrap();
    let w = env.get(moving);
    let f = w.plain_footprint();
    // The lower face now lies on y = 0.1, along the reference.
    assert!(
        (f[0][1] - 0.1).abs() < 1e-9 && (f[1][1] - 0.1).abs() < 1e-9,
        "{f:?}"
    );
    near(w.length(), 4.0_f64.hypot(0.2));
}

#[test]
fn the_warnings_register_lists_overlapping_stairs_and_clears_when_one_moves_away() {
    let env = Env::new();
    env.run("archbuilder.storey.add", json!({})).unwrap();
    let stair = |at: [f64; 2]| -> ObjectId {
        env.run(
            "archbuilder.stair.create",
            json!({"start": at, "direction": 0.0}),
        )
        .unwrap()
        .value["stair"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap()
    };
    let first = stair([0.0, 0.0]);
    let second = stair([0.5, 0.2]);
    let overlaps = |s: &Snapshot| {
        archbuilder_model::validate::register(s)
            .into_iter()
            .filter(|w| w.error.code() == "ab.model.stair_overlap")
            .collect::<Vec<_>>()
    };
    let found = overlaps(&env.snapshot());
    assert_eq!(found.len(), 1, "one entry for the pair");
    assert!(found[0].objects.contains(&first) && found[0].objects.contains(&second));
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [second], "displacement": [0.0, 20.0]}),
    )
    .unwrap();
    assert!(overlaps(&env.snapshot()).is_empty());
}

#[test]
fn a_door_that_opens_into_a_wall_is_a_warning_and_never_locks_the_storey() {
    let env = Env::new();
    let host = env.wall([0.0, 0.0], [6.0, 0.0]);
    env.door(host, 3.0);
    // A wall moved into the door's swing: a warning in the register, as a move reports it.
    let crossing = env.wall([13.2, -1.5], [13.2, 1.5]);
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [crossing], "displacement": [-10.0, 0.0]}),
    )
    .unwrap();
    let s = env.snapshot();
    assert!(
        archbuilder_model::validate::register(&s)
            .iter()
            .any(|w| w.error.code() == "ab.model.insert_meets_wall")
    );
    // Unrelated work on the storey still goes through.
    env.wall([10.0, 0.0], [12.0, 0.0]);
    env.run("archbuilder.element.pin", json!({"objects": [crossing]}))
        .unwrap();
    env.run("archbuilder.element.unpin", json!({"objects": [crossing]}))
        .unwrap();
}

#[test]
fn splitting_a_wall_under_a_floor_keeps_the_floor_whole_without_a_warning_and_merge_restores_it() {
    let env = Env::new();
    let c = [[0.0, 0.0], [6.0, 0.0], [6.0, 4.0], [0.0, 4.0]];
    for k in 0..4 {
        env.wall(c[k], c[(k + 1) % 4]);
    }
    env.run("archbuilder.floor.create", json!({"picks": [[3.0, 2.0]]}))
        .unwrap();
    let area = |env: &Env| {
        let s = env.snapshot();
        let floor = spatial::objects(&s, archbuilder_model::types::FLOOR)[0].clone();
        archbuilder_model::floor::Derived::of(&s, &floor)
            .unwrap()
            .area()
    };
    let floor_warnings = |env: &Env| {
        archbuilder_model::validate::register(&env.snapshot())
            .iter()
            .filter(|w| w.error.code().starts_with("ab.model.floor"))
            .count()
    };
    let before = area(&env);
    // East wall, without and with a gap.
    env.run("archbuilder.wall.split", json!({"at": [6.0, 2.0]}))
        .unwrap();
    near(area(&env), before);
    assert_eq!(floor_warnings(&env), 0);
    env.run(
        "archbuilder.wall.merge",
        json!({"first": [6.0, 1.0], "other": [6.0, 3.0]}),
    )
    .unwrap();
    near(area(&env), before);
    assert_eq!(floor_warnings(&env), 0);
    env.run(
        "archbuilder.wall.split",
        json!({"at": [6.0, 2.0], "gap": 0.5}),
    )
    .unwrap();
    near(area(&env), before);
    assert_eq!(floor_warnings(&env), 0);
}
