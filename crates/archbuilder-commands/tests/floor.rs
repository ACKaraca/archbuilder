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

    /// Gives `object` (a wall or floor) a type of its own of one layer of `thickness`: the
    /// thickness comes from the type.
    fn thicken(&self, object: ObjectId, kind: &str, thickness: f64) {
        let snapshot = self.snapshot();
        let instance = spatial::get(&snapshot, object).unwrap();
        let (_, current) =
            archbuilder_model::element_type::type_of_object(&snapshot, &instance).unwrap();
        let copy = self
            .run(
                "archbuilder.type.duplicate",
                json!({"type": current.id, "name": format!("T {thickness}")}),
            )
            .unwrap();
        let copy = copy.value["type"].clone();
        self.run(
            "archbuilder.type.set",
            json!({"type": copy, "layers": [{"function": "structure", "thickness": thickness}], "core": [0, 1]}),
        )
        .unwrap();
        self.run(
            "archbuilder.type.assign",
            json!({"type": copy, "kind": kind, "objects": [object]}),
        )
        .unwrap();
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
    env.thicken(walls[0], "wall", 0.4);
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
    env.thicken(floor.id, "floor", 0.3);
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
    env.thicken(walls[3], "wall", 0.4);
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

fn sketch_of(env: &Env, id: ObjectId) -> Vec<Json> {
    let snapshot = env.snapshot();
    let object = spatial::get(&snapshot, id).unwrap();
    let floor = Floor::from_values(&object.values).unwrap();
    floor
        .vertices
        .iter()
        .map(|v| json!({"loop_index": v.loop_index, "point": v.point, "bulge": v.bulge}))
        .collect()
}

#[test]
fn editing_the_boundary_moves_a_vertex_and_adds_an_arc_in_one_undo_step() {
    let env = Env::new();
    env.run(
        "archbuilder.floor.create",
        json!({"points": [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]]}),
    )
    .unwrap();
    let floor = env.floors()[0].id;
    let before = env.dump();
    let mut v = sketch_of(&env, floor);
    // Move the vertex (10, 6) to (12, 6) and bend the edge from (0, 0) to (10, 0) outward into a
    // half circle of radius 5: the bulge of a half circle is 1, and a positive bulge bends
    // counterclockwise, which for this loop is outward.
    v[2]["point"] = json!([12.0, 6.0]);
    v[0]["bulge"] = json!(1.0);
    env.run(
        "archbuilder.floor.edit_boundary",
        json!({"floor": floor, "vertices": v}),
    )
    .unwrap();
    let snapshot = env.snapshot();
    let derived = Derived::of(&snapshot, &spatial::get(&snapshot, floor).unwrap()).unwrap();
    derived.boundary.check().unwrap();
    assert!(derived.boundary.has_arcs());
    // The quadrilateral (0,0) (10,0) (12,6) (0,6) has area 66; the half disc adds 25 pi / 2.
    near(derived.area(), 66.0 + 25.0 * std::f64::consts::PI / 2.0);
    // The faceted outline stays within the model tolerance of the arc.
    let flat = derived
        .boundary
        .faceted(archbuilder_model::floor::FLAT_CHORD);
    assert!(flat.loops[0].len() > 10);
    // The solid is made over the exact arc.
    assert!(derived.solid().unwrap().volume() > 66.0 * 0.2);
    env.document.undo().wait().unwrap();
    assert_eq!(env.dump(), before, "one undo step restores the floor");
    env.document.redo().wait().unwrap();
    assert!(
        Floor::from_values(&env.floors()[0].values)
            .unwrap()
            .vertices
            .iter()
            .any(|v| v.bulge != 0.0)
    );
}

#[test]
fn editing_the_boundary_refuses_a_crossing_loop_and_leaves_the_floor() {
    let env = Env::new();
    env.run(
        "archbuilder.floor.create",
        json!({"points": [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]]}),
    )
    .unwrap();
    let floor = env.floors()[0].id;
    let before = env.dump();
    let mut v = sketch_of(&env, floor);
    // A bow tie: the second and third vertices swap places.
    v[1]["point"] = json!([10.0, 6.0]);
    v[2]["point"] = json!([10.0, 0.0]);
    let refused = env.run(
        "archbuilder.floor.edit_boundary",
        json!({"floor": floor, "vertices": v}),
    );
    assert!(refused.is_err());
    assert_eq!(env.dump(), before);
}

#[test]
fn a_floor_from_before_arcs_loads_with_straight_edges() {
    let env = Env::new();
    env.run(
        "archbuilder.floor.create",
        json!({"points": [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]]}),
    )
    .unwrap();
    let mut values = env.floors()[0].values.clone();
    // An older record has no arc list at all.
    values.truncate(archbuilder_model::types::floor::ARCS);
    let floor = Floor::from_values(&values).unwrap();
    assert!(floor.vertices.iter().all(|v| v.bulge == 0.0));
}

#[test]
fn offset_and_split_refuse_a_floor_with_arcs_and_name_it() {
    let env = Env::new();
    let walls = env.house();
    env.run("archbuilder.floor.create", json!({"picks": [[5.0, 3.0]]}))
        .unwrap();
    let floor = env.floors()[0].id;
    let mut v: Vec<Json> = {
        let snapshot = env.snapshot();
        let data = Floor::from_values(&spatial::get(&snapshot, floor).unwrap().values).unwrap();
        data.vertices
            .iter()
            .map(|v| {
                let mut o = json!({"loop_index": 0, "point": v.point});
                if let Some(a) = v.edge {
                    o["wall"] = json!(a.wall.to_string());
                    o["face"] = json!(a.face.name());
                }
                o
            })
            .collect()
    };
    v[0]["bulge"] = json!(0.3);
    v[0].as_object_mut().unwrap().remove("wall");
    env.run(
        "archbuilder.floor.edit_boundary",
        json!({"floor": floor, "vertices": v}),
    )
    .unwrap();
    let before = env.dump();
    // Splitting a wall the floor follows would cut its edge: refused, nothing changed.
    let err = env
        .run("archbuilder.wall.split", json!({"at": [10.0, 3.0]}))
        .unwrap_err();
    assert_eq!(err.code(), "ab.cmd.floor_arcs_unsupported");
    assert_eq!(env.dump(), before);
    // So is an offset of the floor.
    let err = env
        .run(
            "archbuilder.modify.offset",
            json!({"objects": [floor], "side": [20.0, 3.0], "distance": 0.5, "copy": false}),
        )
        .unwrap_err();
    assert_eq!(err.code(), "ab.cmd.floor_arcs_unsupported");
    assert!(err.arguments().contains_key("name"));
    let _ = walls;
}

#[test]
fn a_slab_over_the_walls_below_stays_one_solid_and_the_walls_stop_under_it() {
    use archbuilder_model::cleanup;
    let env = Env::new();
    env.house();
    let snapshot = env.snapshot();
    let upper = spatial::storeys(&snapshot, spatial::buildings(&snapshot)[0].id)[1]
        .storey
        .id;
    // A slab with a hole on the upper storey, over the four 3 m walls of the ground storey.
    env.run(
        "archbuilder.floor.create",
        json!({"storey": upper, "points": [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]],
               "holes": [[[2.0, 2.0], [4.0, 2.0], [4.0, 4.0], [2.0, 4.0]]]}),
    )
    .unwrap();
    let snapshot = env.snapshot();
    let floor = env.floors()[0].clone();
    let derived = Derived::of(&snapshot, &floor).unwrap();
    let solid = cleanup::floor_solid(&snapshot, &floor).unwrap().unwrap();
    assert_eq!(solid.shell_count(), 1, "the slab is one solid");
    near(solid.volume(), (60.0 - 4.0) * 0.2);
    near(solid.volume(), derived.volume());
    // The walls below end at the slab's underside (2.8 m) where it covers them.
    for wall in spatial::objects(&snapshot, WALL) {
        let pieces = cleanup::wall_pieces(&snapshot, &wall).unwrap().unwrap();
        let top = pieces
            .iter()
            .map(|p| p.solid.bounding_box().max().z)
            .fold(f64::MIN, f64::max);
        assert!((2.8 - 1e-9..=3.0 + 1e-9).contains(&top), "{top}");
    }
}

#[test]
fn the_layers_of_a_wall_are_never_stacked_twice_under_a_cut_slab() {
    use archbuilder_model::cleanup;
    let env = Env::new();
    let walls = env.house();
    for w in &walls {
        env.run(
            "archbuilder.type.assign",
            json!({"type": "Exterior masonry cavity", "kind": "wall", "objects": [w]}),
        )
        .unwrap();
    }
    // A slab with a hole, raised into the walls of its own storey: its parts and the hole all
    // matter to the layers that stop at it.
    env.run(
        "archbuilder.floor.create",
        json!({"points": [[-2.0, -2.0], [12.0, -2.0], [12.0, 8.0], [-2.0, 8.0]],
               "holes": [[[2.0, 2.0], [4.0, 2.0], [4.0, 4.0], [2.0, 4.0]]],
               "offset": 0.1}),
    )
    .unwrap();
    let snapshot = env.snapshot();
    for wall in spatial::objects(&snapshot, WALL) {
        let data = archbuilder_model::wall::Wall::of(&snapshot, &wall).unwrap();
        let footprint = archbuilder_model::join::footprint_of(&snapshot, &wall).unwrap();
        let gross = data.solid_in(footprint, 0.0).unwrap().volume();
        let body: f64 = cleanup::wall_pieces(&snapshot, &wall)
            .unwrap()
            .unwrap()
            .iter()
            .map(|p| p.solid.volume())
            .sum();
        // The layers that stop at the slab lose volume; none is counted twice.
        assert!(
            body <= gross + 1e-9 && body > 0.9 * gross,
            "{body} vs {gross}"
        );
    }
}

#[test]
fn the_outline_of_a_wall_at_an_acute_corner_follows_its_layers_and_not_its_square_footprint() {
    use archbuilder_model::cleanup;
    let env = Env::new();
    let c = env
        .run(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [4.0, 0.0], "type": "Exterior masonry cavity"}),
        )
        .unwrap()
        .value["wall"]
        .as_str()
        .unwrap()
        .parse::<ObjectId>()
        .unwrap();
    let p = env
        .run(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [2.0, 3.4641016], "type": "Interior partition 100"}),
        )
        .unwrap()
        .value["wall"]
        .as_str()
        .unwrap()
        .parse::<ObjectId>()
        .unwrap();
    let snapshot = env.snapshot();
    let masonry = spatial::get(&snapshot, c).unwrap();
    let partition = spatial::get(&snapshot, p).unwrap();
    let own = cleanup::wall_outline(&snapshot, &partition);
    assert!(!own.is_empty());
    // The square end of the partition's plain footprint would lie inside the masonry wall's body:
    // no corner of the outline of the layers does.
    let body = cleanup::wall_outline(&snapshot, &masonry);
    let inside = |poly: &[[f64; 2]], q: [f64; 2]| {
        let boundary = archbuilder_model::floor::Boundary::polygons(vec![poly.to_vec()]);
        let near_edge = (0..poly.len()).any(|k| {
            let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
            let d = [b[0] - a[0], b[1] - a[1]];
            let l = d[0].hypot(d[1]).max(1e-12);
            let t = (((q[0] - a[0]) * d[0] + (q[1] - a[1]) * d[1]) / (l * l)).clamp(0.0, 1.0);
            (q[0] - (a[0] + d[0] * t)).hypot(q[1] - (a[1] + d[1] * t)) < 1e-6
        });
        boundary.contains(q) && !near_edge
    };
    for q in own.iter().flatten() {
        assert!(
            !body.iter().any(|l| inside(l, *q)),
            "a corner of the outline lies inside the other wall: {q:?}"
        );
    }
}

#[test]
fn the_layers_of_a_storey_are_kept_across_commits_that_do_not_touch_them() {
    use archbuilder_model::cleanup;
    let env = Env::new();
    let walls = env.house();
    env.run("archbuilder.room.place_all", json!({})).unwrap();
    let snapshot = env.snapshot();
    let storey = spatial::get(&snapshot, walls[0])
        .unwrap()
        .header
        .owner
        .unwrap();
    let layers = cleanup::storey_layers(&snapshot, storey);
    let lines = cleanup::plan_lines(&snapshot, storey, 1.0);
    // A room renamed touches no wall: nothing is made again.
    let room = spatial::objects(&snapshot, archbuilder_model::types::ROOM)[0].id;
    env.run(
        "archbuilder.property.set",
        json!({"objects": [room], "field": "name", "value": "Study"}),
    )
    .unwrap();
    let after = env.snapshot();
    assert!(Arc::ptr_eq(
        &layers,
        &cleanup::storey_layers(&after, storey)
    ));
    assert!(Arc::ptr_eq(
        &lines,
        &cleanup::plan_lines(&after, storey, 1.0)
    ));
    // A wall moved: the storey is made again.
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [walls[0]], "displacement": [0.0, 0.5]}),
    )
    .unwrap();
    let moved = env.snapshot();
    assert!(!Arc::ptr_eq(
        &layers,
        &cleanup::storey_layers(&moved, storey)
    ));
}
