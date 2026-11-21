//! `archbuilder.floor.create` and `archbuilder.floor.add_hole`: floors by picked walls and by
//! sketch, their derived area and volume, how they follow walls, and what is refused.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::floor::{Derived, Floor};
use archbuilder_model::spatial;
use archbuilder_model::types::{FLOOR, WALL, text};
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

    /// The 10 x 6 house of 0.2 m center-line walls, drawn clockwise; returns the wall ids.
    fn house(&self) -> Vec<ObjectId> {
        let c = [[0.0, 0.0], [0.0, 6.0], [10.0, 6.0], [10.0, 0.0]];
        (0..4)
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
            .collect()
    }

    fn floors(&self) -> Vec<Arc<arc_doc::Object>> {
        spatial::objects(&self.snapshot(), FLOOR)
    }

    fn area_volume(&self) -> (f64, f64) {
        let snapshot = self.snapshot();
        let d = Derived::of(&snapshot, &self.floors()[0]).unwrap();
        d.boundary.check().unwrap();
        (d.area(), d.volume())
    }

    fn set(&self, object: ObjectId, field: &str, value: Json) {
        self.run(
            "archbuilder.property.set",
            json!({"objects": [object], "field": field, "value": value}),
        )
        .unwrap();
    }
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} vs {b}");
}

#[test]
fn a_click_inside_the_house_makes_a_floor_on_the_interior_faces() {
    let env = Env::new();
    env.house();
    let made = env
        .run("archbuilder.floor.create", json!({"picks": [[5.0, 3.0]]}))
        .unwrap()
        .value;
    near(made["area"].as_f64().unwrap(), 9.8 * 5.8);
    near(made["volume"].as_f64().unwrap(), 9.8 * 5.8 * 0.2);
    assert_eq!(made["walls"], 4);
    let (area, volume) = env.area_volume();
    near(area, 9.8 * 5.8);
    near(volume, 9.8 * 5.8 * 0.2);
    let floor = &env.floors()[0];
    assert_eq!(text(&floor.values, 0), Some("Slab 1"));
    // The solid hangs down from the level: top at 0, bottom at -0.2.
    let snapshot = env.snapshot();
    let solid = Derived::of(&snapshot, floor).unwrap().solid().unwrap();
    near(solid.bounding_box().max().z, 0.0);
    near(solid.bounding_box().min().z, -0.2);
    // The exterior faces give the outer footprint.
    env.run(
        "archbuilder.floor.create",
        json!({"picks": [[5.0, 3.0]], "face": "exterior", "offset": 0.1, "thickness": "250 mm"}),
    )
    .unwrap();
    let outer = Derived::of(&env.snapshot(), &env.floors()[1]).unwrap();
    near(outer.area(), 10.2 * 6.2);
    near(outer.volume(), 10.2 * 6.2 * 0.25);
    near(outer.top, 0.1);
}

#[test]
fn the_floor_follows_a_moved_wall_and_a_thicker_wall() {
    let env = Env::new();
    let walls = env.house();
    env.run("archbuilder.floor.create", json!({"walls": walls}))
        .unwrap();
    // The right wall (10,6)-(10,0) moves one meter to the right.
    env.set(walls[2], "start_x", json!(11.0));
    env.set(walls[2], "end_x", json!(11.0));
    near(env.area_volume().0, 10.8 * 5.8);
    // The left wall gets thicker: its interior face moves by 0.1 m.
    env.set(walls[0], "thickness", json!(0.4));
    near(env.area_volume().0, 10.7 * 5.8);
}

#[test]
fn deleting_an_associated_wall_freezes_the_edge_and_undo_restores_it() {
    let env = Env::new();
    let walls = env.house();
    env.run("archbuilder.floor.create", json!({"picks": [[5.0, 3.0]]}))
        .unwrap();
    let before = env.dump();
    let gone = env
        .run("archbuilder.wall.delete", json!({"walls": [walls[2]]}))
        .unwrap()
        .value;
    assert_eq!(gone["frozen_edges"], 1);
    near(env.area_volume().0, 9.8 * 5.8);
    let floor = Floor::from_values(&env.floors()[0].values).unwrap();
    assert_eq!(
        floor.vertices.iter().filter(|v| v.edge.is_some()).count(),
        3
    );
    // The top wall (it ends at the deleted wall) moves down: the free edge stays at x = 9.9.
    env.set(walls[1], "start_y", json!(5.0));
    env.set(walls[1], "end_y", json!(5.0));
    let snapshot = env.snapshot();
    let b = Derived::of(&snapshot, &env.floors()[0]).unwrap().boundary;
    assert!(b.loops[0].iter().any(|p| (p[0] - 9.9).abs() < 1e-9));
    env.document.undo().wait().unwrap();
    env.document.undo().wait().unwrap();
    env.document.undo().wait().unwrap();
    assert_eq!(env.dump(), before);
    env.document.redo().wait().unwrap();
    assert_eq!(spatial::objects(&env.snapshot(), WALL).len(), 3);
}

#[test]
fn a_sketch_with_a_hole_subtracts_the_hole_and_undo_redo_restore_it() {
    let env = Env::new();
    let empty = env.dump();
    env.run(
        "archbuilder.floor.create",
        json!({"points": [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]],
               "storey": spatial::storeys(&env.snapshot(), spatial::buildings(&env.snapshot())[0].id)[1].storey.id}),
    )
    .unwrap();
    let floor = env.floors()[0].clone();
    let storey = floor.header.owner.unwrap();
    assert_eq!(
        text(&spatial::get(&env.snapshot(), storey).unwrap().values, 0),
        Some("Floor 1")
    );
    let with_floor = env.dump();
    near(env.area_volume().0, 60.0);
    env.run(
        "archbuilder.floor.add_hole",
        json!({"floor": floor.id, "points": [[2.0, 2.0], [4.0, 2.0], [4.0, 4.0], [2.0, 4.0]]}),
    )
    .unwrap();
    near(env.area_volume().0, 56.0);
    near(env.area_volume().1, 56.0 * 0.2);
    // A floor on Floor 1 stands at 3 m.
    let top = Derived::of(&env.snapshot(), &env.floors()[0]).unwrap().top;
    near(top, 3.0);
    let with_hole = env.dump();
    env.document.undo().wait().unwrap();
    assert_eq!(env.dump(), with_floor);
    env.document.redo().wait().unwrap();
    assert_eq!(env.dump(), with_hole);
    env.document.undo().wait().unwrap();
    env.document.undo().wait().unwrap();
    assert_eq!(env.dump(), empty);
    // Thickness and offset are edited through the property command, and the floor is deleted by
    // the selection command.
    env.document.redo().wait().unwrap();
    env.set(floor.id, "thickness", json!(0.3));
    env.set(floor.id, "top_offset", json!(-0.1));
    let d = Derived::of(&env.snapshot(), &env.floors()[0]).unwrap();
    near(d.floor.thickness, 0.3);
    near(d.top, 2.9);
    env.run(
        "archbuilder.object.select",
        json!({"objects": [floor.id], "mode": "replace"}),
    )
    .unwrap();
}

#[test]
fn bad_boundaries_are_refused_and_change_nothing() {
    let env = Env::new();
    let house = env.house();
    let before = env.dump();
    let code = |params: Json| {
        let error = env.run("archbuilder.floor.create", params).unwrap_err();
        assert_eq!(env.dump(), before, "a refused floor changes nothing");
        error.code()
    };
    let bow = json!([[0.0, 0.0], [4.0, 4.0], [4.0, 0.0], [0.0, 4.0]]);
    assert_eq!(
        code(json!({"points": bow})),
        "ab.model.floor_self_intersecting"
    );
    assert_eq!(
        code(json!({"points": [[0.0, 0.0], [0.0005, 0.0], [0.0005, 4.0], [0.0, 4.0]]})),
        "ab.model.floor_edge_short"
    );
    assert_eq!(
        code(json!({"points": [[0.0, 0.0], [4.0, 0.0]]})),
        "ab.model.floor_malformed"
    );
    let square = json!([[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]);
    let outside = json!([[8.0, 4.0], [12.0, 4.0], [12.0, 6.0], [8.0, 6.0]]);
    assert_eq!(
        code(json!({"points": square, "holes": [outside]})),
        "ab.model.floor_hole_outside"
    );
    let (a, b) = (
        json!([[1.0, 1.0], [4.0, 1.0], [4.0, 4.0], [1.0, 4.0]]),
        json!([[3.0, 3.0], [6.0, 3.0], [6.0, 6.0], [3.0, 6.0]]),
    );
    assert_eq!(
        code(json!({"points": square, "holes": [a, b]})),
        "ab.model.floor_holes_overlap"
    );
    // A click outside any loop, two walls, both a source and nothing at all.
    assert_eq!(
        code(json!({"picks": [[50.0, 50.0]]})),
        "ab.cmd.floor_no_loop"
    );
    let walls = house;
    assert_eq!(
        code(json!({"walls": [walls[0], walls[1]]})),
        "ab.cmd.floor_no_loop"
    );
    assert_eq!(code(json!({})), "cmd.params.invalid");
    assert_eq!(
        code(json!({"points": square, "picks": [[5.0, 3.0]]})),
        "cmd.params.invalid"
    );
    // A thickness out of range is refused by the record.
    assert!(
        env.run(
            "archbuilder.floor.create",
            json!({"points": square, "thickness": 9.0})
        )
        .is_err()
    );
    // A wall move that would break the boundary is refused: parallel opposite walls cross over.
    env.run("archbuilder.floor.create", json!({"picks": [[5.0, 3.0]]}))
        .unwrap();
    // The top wall comes down to the bottom wall's interior face: no area is left.
    env.set(walls[1], "start_y", json!(0.2));
    let kept = env.dump();
    let moved = env.run(
        "archbuilder.property.set",
        json!({"objects": [walls[1]], "field": "end_y", "value": 0.2}),
    );
    assert!(moved.is_err());
    assert_eq!(env.dump(), kept, "a refused move changes nothing");
}

#[test]
fn flipping_a_bounding_wall_moves_no_floor_edge() {
    let env = Env::new();
    let walls = env.house();
    env.run("archbuilder.floor.create", json!({"picks": [[5.0, 3.0]]}))
        .unwrap();
    near(env.area_volume().0, 9.8 * 5.8);
    let before = env.floors()[0].clone();
    // A center-line wall, then a wall whose baseline is its exterior face (its body moves on a flip).
    env.run("archbuilder.wall.flip", json!({"walls": [walls[0]]}))
        .unwrap();
    near(env.area_volume().0, 9.8 * 5.8);
    env.set(walls[2], "location_line", json!("exterior"));
    let area = env.area_volume().0;
    env.run(
        "archbuilder.wall.flip",
        json!({"walls": [walls[2], walls[1]]}),
    )
    .unwrap();
    near(env.area_volume().0, area);
    for _ in 0..3 {
        env.document.undo().wait().unwrap();
    }
    assert_eq!(env.floors()[0].values, before.values);
}

#[test]
fn a_wall_split_in_two_makes_one_straight_edge_and_follows_both_parts() {
    let env = Env::new();
    let make = |a: [f64; 2], b: [f64; 2]| -> ObjectId {
        let made = env
            .run("archbuilder.wall.create", json!({"start": a, "end": b}))
            .unwrap()
            .value;
        made["wall"].as_str().unwrap().parse().unwrap()
    };
    let walls = [
        make([0.0, 0.0], [0.0, 6.0]),
        make([0.0, 6.0], [10.0, 6.0]),
        make([10.0, 6.0], [10.0, 0.0]),
        make([10.0, 0.0], [5.0, 0.0]),
        make([5.0, 0.0], [0.0, 0.0]),
    ];
    env.run(
        "archbuilder.floor.create",
        json!({"walls": walls, "face": "exterior"}),
    )
    .unwrap();
    let by_name = |n: &str| {
        env.floors()
            .into_iter()
            .find(|f| text(&f.values, 0) == Some(n))
            .unwrap()
    };
    near(
        Derived::of(&env.snapshot(), &by_name("Slab 1"))
            .unwrap()
            .area(),
        10.2 * 6.2,
    );
    // The same loop is found by a click, on the interior faces.
    env.run("archbuilder.floor.create", json!({"picks": [[5.0, 3.0]]}))
        .unwrap();
    let inner = Derived::of(&env.snapshot(), &by_name("Slab 2")).unwrap();
    near(inner.area(), 9.8 * 5.8);
    assert_eq!(inner.floor.walls().len(), 5);
    // One part gets thicker: its half of the edge follows, and the floor stays valid and smaller.
    env.set(walls[3], "thickness", json!(0.4));
    let snapshot = env.snapshot();
    let d = Derived::of(&snapshot, &by_name("Slab 2")).unwrap();
    d.boundary.check().unwrap();
    assert!(d.area() < 9.8 * 5.8 && d.area() > 9.8 * 5.8 - 0.3 * 5.0);
}

/// A floor by click inside a box with a courtyard of walls gets the courtyard as a hole, and a
/// room in the courtyard and the room around it do not overlap.
#[test]
fn a_floor_by_click_around_a_courtyard_has_the_hole() {
    let env = Env::new();
    for (side, at) in [(12.0, 0.0), (4.0, 4.0)] {
        let c = [
            [at, at],
            [at, at + side],
            [at + side, at + side],
            [at + side, at],
        ];
        for k in 0..4 {
            env.run(
                "archbuilder.wall.create",
                json!({"start": c[k], "end": c[(k + 1) % 4]}),
            )
            .unwrap();
        }
    }
    env.run("archbuilder.floor.create", json!({"picks": [[2.0, 2.0]]}))
        .unwrap();
    // The floor in the courtyard is its own, without a hole.
    env.run("archbuilder.floor.create", json!({"picks": [[6.0, 6.0]]}))
        .unwrap();
    let snapshot = env.snapshot();
    let all: Vec<Derived> = env
        .floors()
        .iter()
        .map(|f| Derived::of(&snapshot, f).unwrap())
        .collect();
    assert_eq!(all.len(), 2);
    let around = all.iter().find(|d| d.boundary.loops.len() == 2).unwrap();
    let inner = all.iter().find(|d| d.boundary.loops.len() == 1).unwrap();
    near(around.area(), 11.8 * 11.8 - 4.2 * 4.2);
    near(inner.area(), 3.8 * 3.8);
}
