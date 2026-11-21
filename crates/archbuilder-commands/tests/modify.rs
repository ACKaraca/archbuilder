//! `archbuilder.modify.move`, `copy`, `rotate` and `mirror`: exact geometry, hosted doors, floors
//! that follow or dissociate, doors sliding alone, and undo and redo of each.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::floor::{Derived, Floor};
use archbuilder_model::insert::{Hand, Insert, hosted};
use archbuilder_model::spatial;
use archbuilder_model::types::{DOOR, FLOOR, WALL};
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

    /// A 4 x 3 room, drawn clockwise, with a floor and a door in the first wall.
    fn room(&self) -> Vec<ObjectId> {
        let c = [[0.0, 0.0], [0.0, 3.0], [4.0, 3.0], [4.0, 0.0]];
        let walls: Vec<ObjectId> = (0..4)
            .map(|k| {
                let made = self
                    .run(
                        "archbuilder.wall.create",
                        json!({"start": c[k], "end": c[(k + 1) % 4]}),
                    )
                    .unwrap()
                    .value;
                made["wall"].as_str().unwrap().parse().unwrap()
            })
            .collect();
        self.run(
            "archbuilder.door.place",
            json!({"wall": walls[0], "distance": 1.5}),
        )
        .unwrap();
        self.run("archbuilder.floor.create", json!({"picks": [[2.0, 1.5]]}))
            .unwrap();
        walls
    }

    fn all(&self) -> Vec<ObjectId> {
        let s = self.snapshot();
        [WALL, FLOOR, DOOR]
            .iter()
            .flat_map(|k| spatial::objects(&s, k))
            .map(|o| o.id)
            .collect()
    }

    fn walls(&self) -> Vec<(ObjectId, Wall)> {
        let s = self.snapshot();
        spatial::objects(&s, WALL)
            .iter()
            .map(|o| (o.id, Wall::from_values(&o.values).unwrap()))
            .collect()
    }

    fn volume(&self, wall: ObjectId) -> f64 {
        let s = self.snapshot();
        let o = spatial::get(&s, wall).unwrap();
        archbuilder_model::insert::wall_solid(&s, &o)
            .unwrap()
            .unwrap()
            .volume()
    }

    fn floor(&self) -> Derived {
        let s = self.snapshot();
        Derived::of(&s, &spatial::objects(&s, FLOOR)[0]).unwrap()
    }

    fn undo_redo(&self, before: &str, after: &str, steps: usize) {
        for _ in 0..steps {
            self.document.undo().wait().unwrap();
        }
        assert_eq!(self.dump(), before);
        for _ in 0..steps {
            self.document.redo().wait().unwrap();
        }
        assert_eq!(self.dump(), after);
    }
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} vs {b}");
}

fn ids(list: &[ObjectId]) -> Json {
    json!(list.iter().map(ToString::to_string).collect::<Vec<_>>())
}

#[test]
fn moving_a_wall_takes_its_door_and_keeps_its_volume() {
    let env = Env::new();
    let wall: ObjectId = env
        .run(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [6.0, 0.0]}),
        )
        .unwrap()
        .value["wall"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    env.run(
        "archbuilder.door.place",
        json!({"wall": wall, "distance": 2.0}),
    )
    .unwrap();
    let (before, volume) = (env.dump(), env.volume(wall));
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [wall], "displacement": [1.0, 2.5]}),
    )
    .unwrap();
    let w = env.walls()[0].1;
    assert_eq!((w.start, w.end), ([1.0, 2.5], [7.0, 2.5]));
    let s = env.snapshot();
    let door = &hosted(&s, wall)[0].1;
    near(door.distance, 2.0);
    near(env.volume(wall), volume);
    let after = env.dump();
    env.undo_redo(&before, &after, 1);
}

#[test]
fn copying_a_closed_room_makes_a_joined_room_with_its_floor_on_the_new_walls() {
    let env = Env::new();
    env.room();
    let all = env.all();
    let before = env.dump();
    let made = env
        .run(
            "archbuilder.modify.copy",
            json!({"objects": ids(&all), "displacement": [12.0, 0.0]}),
        )
        .unwrap()
        .value;
    // 4 walls, 1 door, 1 floor.
    assert_eq!(made["created"].as_array().unwrap().len(), 6);
    assert!(made.get("dissociated_edges").is_none());
    let walls = env.walls();
    assert_eq!(walls.len(), 8);
    let s = env.snapshot();
    let new: Vec<_> = walls
        .iter()
        .filter(|(_, w)| w.start[0] >= 12.0 || w.end[0] >= 12.0)
        .collect();
    assert_eq!(new.len(), 4);
    // Joined: the room's footprint corners are closed (mitered) like the original.
    for (id, w) in &new {
        let f = archbuilder_model::join::footprint_of(&s, &spatial::get(&s, *id).unwrap()).unwrap();
        assert_ne!(f, w.plain_footprint());
    }
    let floors = spatial::objects(&s, FLOOR);
    assert_eq!(floors.len(), 2);
    let copy = floors
        .iter()
        .map(|f| Floor::from_values(&f.values).unwrap())
        .find(|f| {
            f.walls().iter().all(|w| new.iter().any(|(n, _)| n == w)) && !f.walls().is_empty()
        })
        .unwrap();
    assert_eq!(copy.walls().len(), 4);
    let d = Derived::of(
        &s,
        floors
            .iter()
            .find(|f| Floor::from_values(&f.values).unwrap() == copy)
            .unwrap(),
    )
    .unwrap();
    near(d.area(), 3.8 * 2.8);
    d.boundary.check().unwrap();
    // Names are new and sequential.
    let names: Vec<String> = spatial::objects(&s, WALL)
        .iter()
        .map(|o| {
            archbuilder_model::types::text(&o.values, 0)
                .unwrap()
                .to_owned()
        })
        .collect();
    for n in ["Wall 1", "Wall 5", "Wall 8"] {
        assert!(names.iter().any(|x| x == n), "{names:?}");
    }
    assert_eq!(spatial::objects(&s, DOOR).len(), 2);
    let after = env.dump();
    env.undo_redo(&before, &after, 1);
}

#[test]
fn rotating_a_wall_chain_a_quarter_turn_gives_exact_coordinates() {
    let env = Env::new();
    let walls = env.room();
    let before = env.dump();
    env.run(
        "archbuilder.modify.rotate",
        json!({"objects": ids(&env.all()), "base": [0.0, 0.0], "angle": "90 deg"}),
    )
    .unwrap();
    let w: Vec<_> = env.walls();
    let first = w.iter().find(|(id, _)| *id == walls[0]).unwrap().1;
    assert_eq!((first.start, first.end), ([0.0, 0.0], [-3.0, 0.0]));
    let third = w.iter().find(|(id, _)| *id == walls[2]).unwrap().1;
    assert_eq!((third.start, third.end), ([-3.0, 4.0], [0.0, 4.0]));
    // The floor turned with the walls and stays associated.
    near(env.floor().area(), 3.8 * 2.8);
    let after = env.dump();
    env.undo_redo(&before, &after, 1);
}

#[test]
fn mirroring_swaps_the_hand_of_a_door_and_keeps_the_room_valid() {
    let env = Env::new();
    let walls = env.room();
    let hand = |env: &Env| {
        let s = env.snapshot();
        let o = &spatial::objects(&s, DOOR)[0];
        Insert::of(&s, o).unwrap().hand
    };
    assert_eq!(hand(&env), Hand::Left);
    let before = env.dump();
    env.run(
        "archbuilder.modify.mirror",
        json!({"objects": ids(&env.all()), "axis": [[5.0, 0.0], [5.0, 1.0]], "keep": false}),
    )
    .unwrap();
    assert_eq!(hand(&env), Hand::Right);
    let first = env
        .walls()
        .into_iter()
        .find(|(id, _)| *id == walls[0])
        .unwrap()
        .1;
    // Mirrored across x = 5 and reversed: the baseline runs from (10, 3) to (10, 0).
    assert_eq!((first.start, first.end), ([10.0, 3.0], [10.0, 0.0]));
    let s = env.snapshot();
    near(hosted(&s, walls[0])[0].1.distance, 1.5);
    near(env.floor().area(), 3.8 * 2.8);
    let after = env.dump();
    env.undo_redo(&before, &after, 1);
    // Keeping the originals makes a copy.
    env.run(
        "archbuilder.modify.mirror",
        json!({"objects": ids(&env.all()), "axis": [[5.0, 0.0], [5.0, 1.0]]}),
    )
    .unwrap();
    assert_eq!(env.walls().len(), 8);
}

#[test]
fn a_door_alone_slides_along_its_wall_and_is_refused_beyond_it() {
    let env = Env::new();
    env.room();
    let s = env.snapshot();
    let door = spatial::objects(&s, DOOR)[0].id;
    let before = env.dump();
    // The across part of the displacement is dropped; 0.4 m along the wall remains.
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [door], "displacement": [3.0, 0.4]}),
    )
    .unwrap();
    let s = env.snapshot();
    near(
        Insert::of(&s, &spatial::objects(&s, DOOR)[0])
            .unwrap()
            .distance,
        1.9,
    );
    let after = env.dump();
    env.undo_redo(&before, &after, 1);
    let refused = env.run(
        "archbuilder.modify.move",
        json!({"objects": [door], "displacement": [0.0, 5.0]}),
    );
    assert_eq!(refused.unwrap_err().code(), "ab.model.insert_outside");
    assert_eq!(env.dump(), after);
    let turned = env.run(
        "archbuilder.modify.rotate",
        json!({"objects": [door], "base": [0.0, 0.0], "angle": 1.0}),
    );
    assert_eq!(turned.unwrap_err().code(), "ab.cmd.insert_alone");
}

#[test]
fn a_floor_moved_alone_dissociates_its_edges() {
    let env = Env::new();
    env.room();
    let s = env.snapshot();
    let floor = spatial::objects(&s, FLOOR)[0].id;
    let before = env.dump();
    let made = env
        .run(
            "archbuilder.modify.move",
            json!({"objects": [floor], "displacement": [0.0, 10.0]}),
        )
        .unwrap()
        .value;
    assert_eq!(made["dissociated_edges"], 4);
    let d = env.floor();
    assert!(d.floor.walls().is_empty());
    near(d.area(), 3.8 * 2.8);
    assert!(d.boundary.loops[0].iter().all(|p| p[1] > 9.0));
    let after = env.dump();
    env.undo_redo(&before, &after, 1);
}

#[test]
fn storeys_are_not_movable_and_nothing_selected_is_refused() {
    let env = Env::new();
    let s = env.snapshot();
    let storey = spatial::objects(&s, archbuilder_model::types::STOREY)[0].id;
    let moved = env.run(
        "archbuilder.modify.move",
        json!({"objects": [storey], "displacement": [1.0, 0.0]}),
    );
    assert_eq!(moved.unwrap_err().code(), "ab.cmd.not_movable");
    let none = env.run(
        "archbuilder.modify.move",
        json!({"displacement": [1.0, 0.0]}),
    );
    assert_eq!(none.unwrap_err().code(), "ab.cmd.nothing_selected");
}

#[test]
fn delete_takes_a_mixed_selection_in_one_undo_step() {
    let env = Env::new();
    let walls = env.room();
    let s = env.snapshot();
    let floor = spatial::objects(&s, FLOOR)[0].id;
    let door = spatial::objects(&s, DOOR)[0].id;
    let before = env.dump();
    let invocation = Invocation {
        seed: 7,
        selection: vec![walls[2], floor, door],
        ..Invocation::new("archbuilder.object.delete", json!({}))
    };
    Invoker::new(&env.registry)
        .invoke(&env.document, &env.session, invocation)
        .unwrap();
    let s = env.snapshot();
    assert_eq!(spatial::objects(&s, WALL).len(), 3);
    assert!(spatial::objects(&s, FLOOR).is_empty() && spatial::objects(&s, DOOR).is_empty());
    let after = env.dump();
    env.undo_redo(&before, &after, 1);
}

impl Env {
    fn wall_at(&self, a: [f64; 2], b: [f64; 2]) -> ObjectId {
        self.run("archbuilder.wall.create", json!({"start": a, "end": b}))
            .unwrap()
            .value["wall"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap()
    }

    /// A closed room of `w` x `h` meters from the origin, with a floor by interior faces.
    fn box_room(&self, w: f64, h: f64) -> Vec<ObjectId> {
        let c = [[0.0, 0.0], [0.0, h], [w, h], [w, 0.0]];
        let walls = (0..4).map(|k| self.wall_at(c[k], c[(k + 1) % 4])).collect();
        self.run(
            "archbuilder.floor.create",
            json!({"picks": [[w / 2.0, h / 2.0]]}),
        )
        .unwrap();
        walls
    }

    fn wall(&self, id: ObjectId) -> Wall {
        let s = self.snapshot();
        Wall::from_values(&spatial::get(&s, id).unwrap().values).unwrap()
    }

    fn warnings(&self) -> Vec<&'static str> {
        let s = self.snapshot();
        let touched = s
            .iter()
            .map(|(_, o)| (o.id, arc_doc::Change::Created))
            .collect();
        archbuilder_model::validate::warnings(&s, &touched)
            .iter()
            .map(arc_base::Error::code)
            .collect()
    }
}

#[test]
fn moving_one_wall_of_a_closed_room_stretches_its_neighbors_and_the_floor_follows() {
    let env = Env::new();
    let walls = env.box_room(6.0, 4.0);
    assert!(env.warnings().is_empty());
    let before = env.dump();
    // walls: left, top, right, bottom. Move the right wall 2 m out.
    let made = env
        .run(
            "archbuilder.modify.move",
            json!({"objects": [walls[2]], "displacement": [2.0, 0.0]}),
        )
        .unwrap()
        .value;
    assert_eq!(made["reconnected"], 2);
    let (top, bottom, right) = (env.wall(walls[1]), env.wall(walls[3]), env.wall(walls[2]));
    assert_eq!((top.start, top.end), ([0.0, 4.0], [8.0, 4.0]));
    assert_eq!((bottom.start, bottom.end), ([8.0, 0.0], [0.0, 0.0]));
    assert_eq!((right.start, right.end), ([8.0, 4.0], [8.0, 0.0]));
    near(env.floor().area(), 7.8 * 3.8);
    assert!(env.warnings().is_empty());
    let after = env.dump();
    env.undo_redo(&before, &after, 1);
}

#[test]
fn a_neighbors_door_stays_where_it_is_when_its_start_moves() {
    let env = Env::new();
    let left = env.wall_at([0.0, 0.0], [0.0, 3.0]);
    let top = env.wall_at([0.0, 3.0], [4.0, 3.0]);
    env.run(
        "archbuilder.door.place",
        json!({"wall": top, "distance": 2.0}),
    )
    .unwrap();
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [left], "displacement": [-1.0, 0.0]}),
    )
    .unwrap();
    assert_eq!(env.wall(top).start, [-1.0, 3.0]);
    let s = env.snapshot();
    // Still at x = 2: 3 m from the new start.
    near(hosted(&s, top)[0].1.distance, 3.0);
    // A move that would push the start past the door is refused and changes nothing.
    let before = env.dump();
    let refused = env.run(
        "archbuilder.modify.move",
        json!({"objects": [left], "displacement": [3.0, 0.0]}),
    );
    assert_eq!(refused.unwrap_err().code(), "ab.model.insert_outside");
    assert_eq!(env.dump(), before);
}

#[test]
fn t_joins_follow_and_slide() {
    let env = Env::new();
    let through = env.wall_at([0.0, 0.0], [6.0, 0.0]);
    let stem = env.wall_at([3.0, 0.0], [3.0, 3.0]);
    let before = env.dump();
    // The through wall moves: the stem's start follows its line.
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [through], "displacement": [0.0, 1.0]}),
    )
    .unwrap();
    let s = env.wall(stem);
    assert_eq!((s.start, s.end), ([3.0, 1.0], [3.0, 3.0]));
    let after = env.dump();
    env.undo_redo(&before, &after, 1);
    env.document.undo().wait().unwrap();
    // The stem moves: its start slides along the through wall and the stem keeps its line.
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [stem], "displacement": [1.0, 1.0]}),
    )
    .unwrap();
    let s = env.wall(stem);
    assert_eq!((s.start, s.end), ([4.0, 0.0], [4.0, 4.0]));
}

#[test]
fn a_join_that_cannot_be_kept_is_left_open_and_named() {
    let env = Env::new();
    let walls = env.box_room(6.0, 4.0);
    let before = env.dump();
    // The right wall turns about its middle and becomes parallel to the top and bottom walls.
    let made = env
        .run(
            "archbuilder.modify.rotate",
            json!({"objects": [walls[2]], "base": [6.0, 2.0], "angle": "90 deg"}),
        )
        .unwrap()
        .value;
    let named = made["unjoined"].as_array().unwrap();
    assert_eq!(named.len(), 2);
    assert_eq!(env.wall(walls[1]).end, [6.0, 4.0]);
    assert_eq!(env.wall(walls[3]).start, [6.0, 0.0]);
    let after = env.dump();
    env.undo_redo(&before, &after, 1);
}

#[test]
fn a_door_that_now_meets_a_crossing_wall_is_allowed_with_a_warning() {
    let env = Env::new();
    let a = env.wall_at([0.0, 0.0], [6.0, 0.0]);
    env.run(
        "archbuilder.door.place",
        json!({"wall": a, "distance": 3.0}),
    )
    .unwrap();
    let b = env.wall_at([20.0, -2.0], [20.0, 2.0]);
    let made = env
        .run(
            "archbuilder.modify.move",
            json!({"objects": [b], "displacement": [-17.0, 0.0]}),
        )
        .unwrap()
        .value;
    assert_eq!(made["meets"][0][0], "Door 1");
}

#[test]
fn the_validator_warns_of_an_open_corner_and_a_floor_edge_past_its_wall() {
    let env = Env::new();
    env.wall_at([0.0, 0.0], [4.0, 0.0]);
    env.wall_at([4.3, 0.0], [4.3, 3.0]);
    assert_eq!(env.warnings(), ["ab.model.wall_corner_open"]);
    // The former fault: a wall edited alone leaves the floor edges of its neighbors past them.
    let env = Env::new();
    let walls = env.box_room(6.0, 4.0);
    for (field, value) in [("start_x", 8.0), ("end_x", 8.0)] {
        env.run(
            "archbuilder.property.set",
            json!({"objects": [walls[2]], "field": field, "value": value}),
        )
        .unwrap();
    }
    let found = env.warnings();
    assert!(
        found.contains(&"ab.model.floor_edge_unsupported"),
        "{found:?}"
    );
}

#[test]
fn walls_moved_together_and_alone_keep_every_corner_closed() {
    let env = Env::new();
    let walls = env.box_room(6.0, 4.0);
    let before = env.dump();
    // Bottom and right walls move diagonally: the others stretch, the moved ones trim.
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [walls[2], walls[3]], "displacement": [1.0, -1.0]}),
    )
    .unwrap();
    let w = |k: usize| {
        let w = env.wall(walls[k]);
        (w.start, w.end)
    };
    assert_eq!(w(0), ([0.0, -1.0], [0.0, 4.0]));
    assert_eq!(w(1), ([0.0, 4.0], [7.0, 4.0]));
    assert_eq!(w(2), ([7.0, 4.0], [7.0, -1.0]));
    assert_eq!(w(3), ([7.0, -1.0], [0.0, -1.0]));
    near(env.floor().area(), 6.8 * 4.8);
    assert!(env.warnings().is_empty());
    let after = env.dump();
    env.undo_redo(&before, &after, 1);
    // One wall moved diagonally: it keeps its corners and its length.
    env.document.undo().wait().unwrap();
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [walls[2]], "displacement": [2.0, 1.0]}),
    )
    .unwrap();
    assert_eq!(w(2), ([8.0, 4.0], [8.0, 0.0]));
    assert_eq!(w(1).1, [8.0, 4.0]);
    assert_eq!(w(3).0, [8.0, 0.0]);
    near(env.floor().area(), 7.8 * 3.8);
    assert!(env.warnings().is_empty());
}

#[test]
fn a_wall_turned_ten_degrees_closes_its_corners_exactly() {
    let env = Env::new();
    let walls = env.box_room(6.0, 4.0);
    let before = env.dump();
    env.run(
        "archbuilder.modify.rotate",
        json!({"objects": [walls[2]], "base": [6.0, 0.0], "angle": "10"}),
    )
    .unwrap();
    let (top, right) = (env.wall(walls[1]), env.wall(walls[2]));
    let x = 6.0 - 4.0 / 10.0_f64.to_radians().cos() * 10.0_f64.to_radians().sin();
    near(top.end[0], x);
    near(top.end[1], 4.0);
    assert_eq!(right.start, top.end);
    near(right.end[0], 6.0);
    near(right.end[1], 0.0);
    assert!(env.warnings().is_empty());
    let after = env.dump();
    env.undo_redo(&before, &after, 1);
}
