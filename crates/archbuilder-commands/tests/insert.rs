//! Doors and windows on a document with the model's validator: what is refused, the exact volume
//! of a wall with openings, the cascade on a wall's deletion, wall changes with inserts, flips.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::insert::{Facing, Hand, Insert, wall_solid};
use archbuilder_model::spatial;
use archbuilder_model::types::{DOOR, WALL, WINDOW};
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
        env.run("archbuilder.project.new", json!({}), &[]).unwrap();
        env
    }

    fn run(
        &self,
        command: &str,
        params: Json,
        selection: &[ObjectId],
    ) -> arc_base::Result<Outcome> {
        let invocation = Invocation {
            seed: 7,
            selection: selection.to_vec(),
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

    /// A wall along the x axis, 10 m long, 0.2 m thick, 3 m high.
    fn wall(&self) -> ObjectId {
        self.run(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [10.0, 0.0], "height": 3.0}),
            &[],
        )
        .unwrap();
        spatial::objects(&self.snapshot(), WALL)[0].id
    }

    fn place(&self, command: &str, params: Json) -> arc_base::Result<Outcome> {
        self.run(command, params, &[])
    }

    /// The code of a refusal, which leaves the document as it was.
    fn refused(&self, command: &str, params: Json) -> String {
        let before = self.dump();
        let error = self.place(command, params).unwrap_err();
        assert_eq!(self.dump(), before, "a refused insert changes nothing");
        format!("{} {:?}", error.code(), error)
    }

    fn volume(&self, wall: ObjectId) -> f64 {
        let snapshot = self.snapshot();
        let object = spatial::get(&snapshot, wall).unwrap();
        wall_solid(&snapshot, &object).unwrap().unwrap().volume()
    }
}

#[test]
fn the_volume_of_a_wall_is_the_joined_solid_minus_its_openings_exactly() {
    let env = Env::new();
    let wall = env.wall();
    assert!((env.volume(wall) - 6.0).abs() < 1e-9);
    env.place(
        "archbuilder.door.place",
        json!({"wall": wall, "distance": 2.0}),
    )
    .unwrap();
    env.place(
        "archbuilder.window.place",
        json!({"wall": wall, "distance": 6.0}),
    )
    .unwrap();
    // 10 x 0.2 x 3, minus a 0.9 x 2.1 door and a 1.2 x 1.2 window through 0.2 m.
    let expected = 6.0 - 0.9 * 2.1 * 0.2 - 1.2 * 1.2 * 0.2;
    assert!(
        (env.volume(wall) - expected).abs() < 1e-9,
        "{}",
        env.volume(wall)
    );
    let inserts = spatial::children(&env.snapshot(), DOOR, wall);
    assert_eq!(inserts.len(), 1);
    let door = Insert::of(&env.snapshot(), &inserts[0]).unwrap();
    assert_eq!(
        (door.width, door.height, door.sill, door.distance),
        (0.9, 2.1, 0.0, 2.0)
    );
    let windows = spatial::children(&env.snapshot(), WINDOW, wall);
    let window = Insert::of(&env.snapshot(), &windows[0]).unwrap();
    assert_eq!((window.width, window.height, window.sill), (1.2, 1.2, 0.9));
}

#[test]
fn inserts_outside_the_wall_overlapping_or_too_tall_are_refused_and_change_nothing() {
    let env = Env::new();
    let wall = env.wall();
    env.place(
        "archbuilder.door.place",
        json!({"wall": wall, "distance": 2.0}),
    )
    .unwrap();
    let door = |p: Json| env.refused("archbuilder.door.place", p);
    assert!(door(json!({"wall": wall, "distance": 0.3})).contains("insert_outside"));
    assert!(door(json!({"wall": wall, "distance": 9.8})).contains("insert_outside"));
    assert!(door(json!({"wall": wall, "distance": 2.5})).contains("insert_overlap"));
    assert!(
        door(json!({"wall": wall, "distance": 5.0, "height": 3.0})).contains("insert_too_tall")
    );
    let window = env.refused(
        "archbuilder.window.place",
        json!({"wall": wall, "distance": 5.0, "sill": 2.5, "height": 1.2}),
    );
    assert!(window.contains("insert_too_tall"), "{window}");
    // The refusal names the insert in the way.
    assert!(door(json!({"wall": wall, "distance": 2.5})).contains("Door 1"));
    // Hostile numbers are refused by the record's ranges.
    assert!(door(json!({"wall": wall, "distance": 5.0, "width": 1e30})).len() > 3);
    assert!(door(json!({"wall": wall, "distance": "abc"})).len() > 3);
    // No wall named and none selected.
    assert!(door(json!({"distance": 5.0})).contains("no_wall_selected"));
}

#[test]
fn deleting_a_wall_deletes_its_inserts_in_one_undoable_step() {
    let env = Env::new();
    let wall = env.wall();
    for (command, at) in [
        ("archbuilder.door.place", 2.0),
        ("archbuilder.window.place", 6.0),
    ] {
        env.place(command, json!({"wall": wall, "distance": at}))
            .unwrap();
    }
    let before = env.dump();
    let done = env
        .run("archbuilder.wall.delete", json!({"walls": [wall]}), &[])
        .unwrap();
    assert_eq!(done.value, json!({"walls": 1, "inserts": 2}));
    let snapshot = env.snapshot();
    assert!(spatial::objects(&snapshot, DOOR).is_empty());
    assert!(spatial::objects(&snapshot, WINDOW).is_empty());
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(
        env.dump(),
        before,
        "one undo restores the wall and both inserts"
    );
    assert!(env.document.redo().wait().unwrap().value);
    assert!(spatial::objects(&env.snapshot(), DOOR).is_empty());
}

#[test]
fn a_selected_insert_is_deleted_by_the_delete_command_and_its_wall_stays() {
    let env = Env::new();
    let wall = env.wall();
    let id = env
        .place(
            "archbuilder.door.place",
            json!({"wall": wall, "distance": 2.0}),
        )
        .unwrap()
        .value["insert"]
        .as_str()
        .unwrap()
        .parse::<ObjectId>()
        .unwrap();
    let before = env.dump();
    let done = env
        .run("archbuilder.object.delete", json!({}), &[id])
        .unwrap();
    assert_eq!(done.value, json!({"walls": 0, "inserts": 1}));
    assert!(spatial::get(&env.snapshot(), wall).is_some());
    assert!(spatial::get(&env.snapshot(), id).is_none());
    env.document.undo().wait().unwrap();
    assert_eq!(env.dump(), before);
}

#[test]
fn a_moved_wall_keeps_its_inserts_at_their_distance_and_a_shortened_one_that_cuts_one_is_refused() {
    let env = Env::new();
    let wall = env.wall();
    let placed = env
        .place(
            "archbuilder.door.place",
            json!({"wall": wall, "distance": 5.0}),
        )
        .unwrap();
    let door: ObjectId = placed.value["insert"].as_str().unwrap().parse().unwrap();
    let distance = |env: &Env| {
        let snapshot = env.snapshot();
        Insert::of(&snapshot, &spatial::get(&snapshot, door).unwrap())
            .unwrap()
            .distance
    };
    // Move the wall 2 m up the y axis: both ends.
    for (field, value) in [("start_y", 2.0), ("end_y", 2.0)] {
        env.run(
            "archbuilder.property.set",
            json!({"objects": [wall], "field": field, "value": value}),
            &[],
        )
        .unwrap();
    }
    assert_eq!(distance(&env), 5.0);
    // Shortening the wall to 4 m would leave the door (4.55 to 5.45) outside: refused, and the
    // message names the door.
    let before = env.dump();
    let error = env
        .run(
            "archbuilder.property.set",
            json!({"objects": [wall], "field": "end_x", "value": 4.0}),
            &[],
        )
        .unwrap_err();
    assert_eq!(error.code(), "ab.model.insert_outside");
    assert!(format!("{error:?}").contains("Door 1"));
    assert_eq!(env.dump(), before);
    // Shortening it to 6 m is fine.
    env.run(
        "archbuilder.property.set",
        json!({"objects": [wall], "field": "end_x", "value": 6.0}),
        &[],
    )
    .unwrap();
}

#[test]
fn an_insert_may_not_reach_into_a_mitered_corner() {
    let env = Env::new();
    let corners = [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]];
    for k in 0..4 {
        env.run(
            "archbuilder.wall.create",
            json!({"start": corners[k], "end": corners[(k + 1) % 4], "height": 3.0}),
            &[],
        )
        .unwrap();
    }
    let bottom = spatial::objects(&env.snapshot(), WALL)
        .iter()
        .find(|w| {
            archbuilder_model::wall::Wall::from_values(&w.values)
                .unwrap()
                .start
                == [0.0, 0.0]
        })
        .unwrap()
        .id;
    // The inner corner of the miter is 0.1 m from the baseline's start: the clear span starts at
    // 0.1 m, and an opening keeps 0.01 m from it.
    let refused = env.refused(
        "archbuilder.door.place",
        json!({"wall": bottom, "distance": 0.5}),
    );
    assert!(refused.contains("insert_outside"), "{refused}");
    env.place(
        "archbuilder.door.place",
        json!({"wall": bottom, "distance": 0.57}),
    )
    .unwrap();
    // The ring is 10.2 x 6.2 outside and 9.8 x 5.8 inside; the door takes 0.9 x 2.1 x 0.2.
    let ring = (10.2 * 6.2 - 9.8 * 5.8) * 3.0;
    let total: f64 = spatial::objects(&env.snapshot(), WALL)
        .iter()
        .map(|w| env.volume(w.id))
        .sum();
    assert!((total - (ring - 0.9 * 2.1 * 0.2)).abs() < 1e-9, "{total}");
}

#[test]
fn hand_and_facing_flip_with_undo_and_a_wall_flip_keeps_the_inserts_in_place() {
    let env = Env::new();
    let wall = env.wall();
    let id: ObjectId = env
        .place(
            "archbuilder.door.place",
            json!({"wall": wall, "distance": 2.0}),
        )
        .unwrap()
        .value["insert"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let read = |env: &Env| {
        let snapshot = env.snapshot();
        Insert::of(&snapshot, &spatial::get(&snapshot, id).unwrap()).unwrap()
    };
    assert_eq!(
        (read(&env).hand, read(&env).facing),
        (Hand::Left, Facing::Exterior)
    );
    let before = env.dump();
    env.run("archbuilder.insert.flip_hand", json!({}), &[id])
        .unwrap();
    assert_eq!(read(&env).hand, Hand::Right);
    env.run(
        "archbuilder.insert.flip_facing",
        json!({"inserts": [id]}),
        &[],
    )
    .unwrap();
    assert_eq!(read(&env).facing, Facing::Interior);
    env.document.undo().wait().unwrap();
    env.document.undo().wait().unwrap();
    assert_eq!(env.dump(), before);
    // Nothing selected, nothing named.
    assert!(
        env.run("archbuilder.insert.flip_hand", json!({}), &[])
            .is_err()
    );
    // Flipping the wall reverses its baseline: the door stays 8 m from the new start.
    env.run("archbuilder.wall.flip", json!({"walls": [wall]}), &[])
        .unwrap();
    assert_eq!(read(&env).distance, 8.0);
    assert!((env.volume(wall) - (6.0 - 0.9 * 2.1 * 0.2)).abs() < 1e-9);
}

#[test]
fn an_opening_may_not_open_into_a_wall_that_meets_its_wall_in_a_t() {
    let env = Env::new();
    let wall = env.wall();
    // A second wall ends on the middle of the first: a T at x = 5.
    let tee = |env: &Env| {
        env.run(
            "archbuilder.wall.create",
            json!({"start": [5.0, 0.0], "end": [5.0, 4.0], "height": 3.0}),
            &[],
        )
    };
    tee(&env).unwrap();
    let door = |at: f64| json!({"wall": wall, "distance": at});
    // A door at 5 would open into the second wall; the refusal names it. One clear of it is fine.
    let refused = env.refused("archbuilder.door.place", door(5.0));
    assert!(
        refused.contains("insert_meets_wall") && refused.contains("Wall 2"),
        "{refused}"
    );
    let refused = env.refused("archbuilder.door.place", door(5.4));
    assert!(refused.contains("insert_meets_wall"), "{refused}");
    env.place("archbuilder.door.place", door(3.0)).unwrap();
    env.place("archbuilder.window.place", door(7.0)).unwrap();
    // The other way round: with a door at 5 first, drawing the T wall is refused, naming the door.
    let other = Env::new();
    let first = other.wall();
    other
        .place(
            "archbuilder.door.place",
            json!({"wall": first, "distance": 5.0}),
        )
        .unwrap();
    let before = other.dump();
    let error = tee(&other).unwrap_err();
    assert_eq!(error.code(), "ab.model.insert_meets_wall");
    assert!(format!("{error:?}").contains("Door 1"));
    assert_eq!(other.dump(), before);
}

#[test]
fn flipping_the_facing_keeps_the_hinge_on_the_same_jamb() {
    let env = Env::new();
    let wall = env.wall();
    let id: ObjectId = env
        .place(
            "archbuilder.door.place",
            json!({"wall": wall, "distance": 2.0}),
        )
        .unwrap()
        .value["insert"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let read = |env: &Env| {
        let snapshot = env.snapshot();
        Insert::of(&snapshot, &spatial::get(&snapshot, id).unwrap()).unwrap()
    };
    let before = read(&env);
    env.run("archbuilder.insert.flip_facing", json!({}), &[id])
        .unwrap();
    let after = read(&env);
    assert_eq!(after.facing, before.facing.flipped());
    assert_eq!(after.hinge_direction(), before.hinge_direction());
    assert_eq!(after.hand, before.hand.flipped());
}
