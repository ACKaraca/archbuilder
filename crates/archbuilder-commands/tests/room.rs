//! Rooms: placement by click point and in every region, derived areas, moving walls, deleting a
//! partition (redundant) and an outer wall (unenclosed), numbering, undo and save/reopen.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::room::{self, Entry, State};
use archbuilder_model::spatial;
use archbuilder_model::types::STOREY;
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

    fn wall(&self, a: [f64; 2], b: [f64; 2]) -> ObjectId {
        let made = self
            .run("archbuilder.wall.create", json!({"start": a, "end": b}))
            .unwrap()
            .value;
        made["wall"].as_str().unwrap().parse().unwrap()
    }

    /// The 10 x 6 house of 0.2 m center-line walls; returns the outer walls.
    fn house(&self) -> Vec<ObjectId> {
        let c = [[0.0, 0.0], [0.0, 6.0], [10.0, 6.0], [10.0, 0.0]];
        (0..4).map(|k| self.wall(c[k], c[(k + 1) % 4])).collect()
    }

    fn rooms(&self) -> Vec<Entry> {
        let snapshot = self.document.snapshot();
        let storey = spatial::lowest_storey(&snapshot).unwrap();
        let mut rooms = room::of_storey(&snapshot, storey.id);
        // Identifiers are random; the numbers keep the order of placement.
        rooms.sort_by(|a, b| a.room.number.cmp(&b.room.number));
        rooms
    }

    fn undo(&self) {
        self.document.undo().wait().unwrap();
    }

    fn findings(&self) -> Vec<&'static str> {
        let snapshot = self.document.snapshot();
        let storey = spatial::objects(&snapshot, STOREY)[0].id;
        archbuilder_model::validate::room_findings(&snapshot, storey)
            .iter()
            .map(arc_base::Error::code)
            .collect()
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn clicks_in_both_rooms_of_the_house_give_the_hand_calculated_areas_and_numbers() {
    let env = Env::new();
    env.house();
    env.wall([5.0, 0.0], [5.0, 6.0]);
    env.run("archbuilder.room.place", json!({"points": [[2.5, 3.0]]}))
        .unwrap();
    env.run("archbuilder.room.place", json!({"points": [[7.5, 3.0]]}))
        .unwrap();
    let rooms = env.rooms();
    assert_eq!(rooms.len(), 2);
    for (room, number) in rooms.iter().zip(["101", "102"]) {
        assert_eq!(room.room.number.as_deref(), Some(number));
        assert_eq!(room.room.name, "Room");
        assert_eq!(room.state(), State::Enclosed);
        // 4.8 x 5.8 clear; 2 x (4.8 + 5.8); 3 m high by default.
        assert!(close(room.area().unwrap(), 27.84));
        assert!(close(room.perimeter().unwrap(), 21.2));
        assert!(close(room.volume().unwrap(), 27.84 * 3.0));
    }
    assert!(env.findings().is_empty());
    // A second room in an occupied region and a click outside are refused.
    assert!(
        env.run("archbuilder.room.place", json!({"points": [[3.0, 2.0]]}))
            .is_err()
    );
    assert!(
        env.run("archbuilder.room.place", json!({"points": [[30.0, 2.0]]}))
            .is_err()
    );
    assert_eq!(env.rooms().len(), 2);
}

#[test]
fn moving_the_partition_updates_both_areas() {
    let env = Env::new();
    env.house();
    let partition = env.wall([5.0, 0.0], [5.0, 6.0]);
    env.run("archbuilder.room.place_all", json!({})).unwrap();
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [partition.to_string()], "displacement": [1.0, 0.0]}),
    )
    .unwrap();
    let areas: Vec<f64> = env.rooms().iter().map(|r| r.area().unwrap()).collect();
    // Clear widths 5.8 and 3.8 by 5.8.
    assert!(
        close(areas[0], 5.8 * 5.8) && close(areas[1], 3.8 * 5.8),
        "{areas:?}"
    );
}

#[test]
fn deleting_the_partition_makes_the_rooms_redundant_and_undo_restores_them() {
    let env = Env::new();
    env.house();
    let partition = env.wall([5.0, 0.0], [5.0, 6.0]);
    env.run("archbuilder.room.place_all", json!({})).unwrap();
    env.run(
        "archbuilder.wall.delete",
        json!({"walls": [partition.to_string()]}),
    )
    .unwrap();
    // The first placed (101) keeps its area; only 102 is redundant.
    let states: Vec<State> = env.rooms().iter().map(Entry::state).collect();
    assert_eq!(states, [State::Enclosed, State::Redundant]);
    assert!(env.rooms()[0].area().is_some() && env.rooms()[1].area().is_none());
    assert_eq!(env.findings(), ["ab.model.room_redundant"]);
    env.undo();
    assert!(env.rooms().iter().all(|r| r.state() == State::Enclosed));
}

#[test]
fn deleting_an_outer_wall_leaves_an_unenclosed_room_with_a_warning() {
    let env = Env::new();
    let walls = env.house();
    env.run("archbuilder.room.place", json!({"points": [[5.0, 3.0]]}))
        .unwrap();
    env.run(
        "archbuilder.wall.delete",
        json!({"walls": [walls[2].to_string()]}),
    )
    .unwrap();
    let rooms = env.rooms();
    assert_eq!(rooms[0].state(), State::Unenclosed);
    assert_eq!(rooms[0].area(), None);
    assert_eq!(env.findings(), ["ab.model.room_unenclosed"]);
    env.undo();
    assert_eq!(env.rooms()[0].state(), State::Enclosed);
}

#[test]
fn place_all_places_exactly_the_missing_rooms_and_a_duplicate_number_is_a_warning() {
    let env = Env::new();
    env.house();
    env.wall([4.0, 0.0], [4.0, 6.0]);
    env.wall([7.0, 0.0], [7.0, 6.0]);
    env.run("archbuilder.room.place", json!({"points": [[2.0, 3.0]]}))
        .unwrap();
    let placed = env
        .run("archbuilder.room.place_all", json!({}))
        .unwrap()
        .value;
    assert_eq!(placed["rooms"].as_array().unwrap().len(), 2);
    assert_eq!(env.rooms().len(), 3);
    let numbers: Vec<_> = env
        .rooms()
        .iter()
        .map(|r| r.room.number.clone().unwrap())
        .collect();
    assert_eq!(numbers, ["101", "102", "103"]);
    // Nothing is missing now.
    assert!(env.run("archbuilder.room.place_all", json!({})).is_err());
    // The number of a room is set to another's: reported, not refused.
    let second = env.rooms()[1].id;
    env.run(
        "archbuilder.property.set",
        json!({"objects": [second.to_string()], "field": "number", "value": "101"}),
    )
    .unwrap();
    assert_eq!(
        env.findings(),
        [
            "ab.model.room_number_duplicate",
            "ab.model.room_number_duplicate"
        ]
    );
}

#[test]
fn rooms_move_and_are_deleted_with_undo() {
    let env = Env::new();
    env.house();
    env.run("archbuilder.room.place", json!({"points": [[5.0, 3.0]]}))
        .unwrap();
    let id = env.rooms()[0].id;
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [id.to_string()], "displacement": [1.0, 1.0]}),
    )
    .unwrap();
    assert_eq!(env.rooms()[0].room.point, [6.0, 4.0]);
    let invocation = Invocation {
        seed: 7,
        selection: vec![id],
        ..Invocation::new("archbuilder.object.delete", json!({}))
    };
    Invoker::new(&env.registry)
        .invoke(&env.document, &env.session, invocation)
        .unwrap();
    assert!(env.rooms().is_empty());
    env.undo();
    assert_eq!(env.rooms().len(), 1);
}

#[test]
fn a_partition_that_runs_past_the_outer_walls_still_makes_two_rooms() {
    let env = Env::new();
    // A 12 x 5 box; the partition at x = 6 runs from y = -0.5 to y = 5.5.
    let c = [[0.0, 0.0], [12.0, 0.0], [12.0, 5.0], [0.0, 5.0]];
    for k in 0..4 {
        env.wall(c[k], c[(k + 1) % 4]);
    }
    env.wall([6.0, -0.5], [6.0, 5.5]);
    env.run("archbuilder.room.place_all", json!({})).unwrap();
    let rooms = env.rooms();
    assert_eq!(rooms.len(), 2);
    // 5.8 x 4.8 clear each.
    for room in &rooms {
        assert_eq!(room.state(), State::Enclosed);
        assert!(close(room.area().unwrap(), 5.8 * 4.8));
    }
    assert!(env.findings().is_empty());
}

#[test]
fn a_grid_of_long_crossing_walls_makes_four_rooms_and_floors_by_click_agree() {
    let env = Env::new();
    // Three horizontal and three vertical lines at 0, 4 and 8, each 1 m longer at both ends:
    // 2 x 2 cells of 3.8 x 3.8 clear.
    for v in [0.0, 4.0, 8.0] {
        env.wall([-1.0, v], [9.0, v]);
        env.wall([v, -1.0], [v, 9.0]);
    }
    env.run("archbuilder.room.place_all", json!({})).unwrap();
    let rooms = env.rooms();
    assert_eq!(rooms.len(), 4);
    for room in &rooms {
        assert_eq!(room.state(), State::Enclosed);
        assert!(close(room.area().unwrap(), 3.8 * 3.8));
        assert!(close(room.perimeter().unwrap(), 4.0 * 3.8));
    }
    let points: Vec<[f64; 2]> = rooms.iter().map(|r| r.room.point).collect();
    assert!(
        points
            .iter()
            .all(|p| (p[0] - 2.0).abs() < 1e-9 || (p[0] - 6.0).abs() < 1e-9)
    );
    assert!(env.findings().is_empty());
    // Nothing is missing, and a click in a cell with a room is refused.
    assert!(env.run("archbuilder.room.place_all", json!({})).is_err());
    assert!(
        env.run("archbuilder.room.place", json!({"points": [[2.5, 1.5]]}))
            .is_err()
    );
    // A floor by a click inside a cell is that cell.
    let made = env
        .run("archbuilder.floor.create", json!({"picks": [[6.0, 6.0]]}))
        .unwrap()
        .value;
    assert!(close(made["area"].as_f64().unwrap(), 3.8 * 3.8));
}

#[test]
fn place_all_puts_an_l_shaped_room_well_inside_it() {
    let env = Env::new();
    let l = [
        [0.0, 0.0],
        [10.0, 0.0],
        [10.0, 4.0],
        [4.0, 4.0],
        [4.0, 10.0],
        [0.0, 10.0],
    ];
    for k in 0..6 {
        env.wall(l[k], l[(k + 1) % 6]);
    }
    env.run("archbuilder.room.place_all", json!({})).unwrap();
    let rooms = env.rooms();
    assert_eq!(rooms.len(), 1);
    let (polygon, at) = (&rooms[0].footprint.polygon, rooms[0].room.point);
    // The distance to the nearest edge of the interior boundary.
    let depth = |p: [f64; 2]| {
        (0..polygon.len())
            .map(|k| {
                let (a, b) = (polygon[k], polygon[(k + 1) % polygon.len()]);
                let d = [b[0] - a[0], b[1] - a[1]];
                let t = (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1])
                    / (d[0] * d[0] + d[1] * d[1]))
                    .clamp(0.0, 1.0);
                (p[0] - a[0] - t * d[0]).hypot(p[1] - a[1] - t * d[1])
            })
            .fold(f64::MAX, f64::min)
    };
    // The arms are 3.8 m wide: the best a point can do is 1.9 m from the walls.
    assert!(depth(at) > 1.85, "{at:?} {}", depth(at));
}
