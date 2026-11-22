//! Element types: the default set of a new project, a type change that moves every instance with
//! its joins, openings, floors and rooms in one undo step, the layer rules, retyping, matching,
//! deleting a type in use, and the upgrade of instances without a type.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, FieldPath, Snapshot};
use archbuilder_model::element_type::{self as et, TypeKind};
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

    fn type_named(&self, kind: TypeKind, name: &str) -> ObjectId {
        et::of_kind(&self.snapshot(), kind)
            .iter()
            .find(|t| et::name(t) == name)
            .unwrap()
            .id
    }

    fn wall(&self, start: [f64; 2], end: [f64; 2], extra: Json) -> ObjectId {
        let mut params = json!({"start": start, "end": end});
        if let (Some(p), Some(e)) = (params.as_object_mut(), extra.as_object()) {
            p.extend(e.clone());
        }
        let made = self.run("archbuilder.wall.create", params).unwrap().value;
        made["wall"].as_str().unwrap().parse().unwrap()
    }

    fn thickness(&self, wall: ObjectId) -> f64 {
        let snapshot = self.snapshot();
        Wall::from_values(&spatial::get(&snapshot, wall).unwrap().values)
            .unwrap()
            .thickness
    }
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} vs {b}");
}

#[test]
fn a_new_project_has_the_default_types_and_walls_take_the_generic_one() {
    let env = Env::new();
    let names = |kind| -> Vec<String> {
        et::of_kind(&env.snapshot(), kind)
            .iter()
            .map(|t| et::name(t).to_owned())
            .collect()
    };
    assert_eq!(
        names(TypeKind::Wall),
        [
            "Exterior masonry cavity",
            "Generic 200",
            "Interior partition 100"
        ]
    );
    assert_eq!(names(TypeKind::Floor).len(), 2);
    assert_eq!(
        names(TypeKind::Door),
        [
            "Double 1500 × 2100",
            "Single 900 × 2100",
            "Sliding 1800 × 2100"
        ]
    );
    assert_eq!(names(TypeKind::Window).len(), 3);
    let wall = env.wall([0.0, 0.0], [4.0, 0.0], json!({}));
    let snapshot = env.snapshot();
    let object = spatial::get(&snapshot, wall).unwrap();
    let (_, found) = et::type_of_object(&snapshot, &object).unwrap();
    assert_eq!(et::name(&found), "Generic 200");
    // The cavity wall measures its core faces against its layers: 0.2 m of finish and air
    // outside the block, 15 mm of plaster inside.
    let cavity = env.wall(
        [0.0, 2.0],
        [4.0, 2.0],
        json!({"type": "Exterior masonry cavity", "location_line": "core_exterior"}),
    );
    let snapshot = env.snapshot();
    let w = Wall::from_values(&spatial::get(&snapshot, cavity).unwrap().values).unwrap();
    near(w.thickness, 0.365);
    let (low, high) = w.extent();
    near(high, 0.2);
    near(low, -0.165);
    let (core_low, core_high) = w.core_extent();
    near(core_low, -0.15);
    near(core_high, 0.0);
}

#[test]
fn a_type_change_moves_every_wall_with_its_joins_openings_floor_and_room_in_one_step() {
    let env = Env::new();
    let c = [[0.0, 0.0], [0.0, 6.0], [10.0, 6.0], [10.0, 0.0]];
    let walls: Vec<ObjectId> = (0..4)
        .map(|k| env.wall(c[k], c[(k + 1) % 4], json!({})))
        .collect();
    env.run(
        "archbuilder.door.place",
        json!({"wall": walls[0], "distance": 3.0}),
    )
    .unwrap();
    env.run("archbuilder.floor.create", json!({"walls": walls}))
        .unwrap();
    env.run("archbuilder.room.place", json!({"points": [[5.0, 3.0]]}))
        .unwrap();
    let room_area = |env: &Env| {
        let snapshot = env.snapshot();
        let storey = spatial::get(&snapshot, walls[0])
            .unwrap()
            .header
            .owner
            .unwrap();
        archbuilder_model::room::of_storey(&snapshot, storey)[0]
            .area()
            .unwrap()
    };
    near(room_area(&env), 9.8 * 5.8);
    let generic = env.type_named(TypeKind::Wall, "Generic 200");
    let done = env
        .run(
            "archbuilder.type.set",
            json!({"type": generic, "layers": [
                {"function": "finish", "material": "Plaster", "thickness": 0.02},
                {"function": "structure", "material": "Concrete", "thickness": 0.28}
            ], "core": [1, 2]}),
        )
        .unwrap();
    assert_eq!(done.value["instances"], 4);
    for wall in &walls {
        near(env.thickness(*wall), 0.3);
    }
    // The walls stay on their center lines: the room and the floor lose 0.05 m on each side.
    near(room_area(&env), 9.7 * 5.7);
    let snapshot = env.snapshot();
    let floor = &spatial::objects(&snapshot, archbuilder_model::types::FLOOR)[0];
    let derived = archbuilder_model::floor::Derived::of(&snapshot, floor).unwrap();
    near(derived.area(), 9.7 * 5.7);
    // The corners stay mitered: four walls of 10 and 6 m around the center line.
    let volume: f64 = walls
        .iter()
        .map(|w| {
            let o = spatial::get(&snapshot, *w).unwrap();
            archbuilder_model::join::footprint_of(&snapshot, &o).unwrap();
            archbuilder_model::insert::wall_solid(&snapshot, &o)
                .unwrap()
                .unwrap()
                .volume()
        })
        .sum();
    let door_cut = 0.9 * 2.1 * 0.3;
    near(volume, 2.0 * (10.0 + 6.0) * 0.3 * 3.0 - door_cut);
    // One undo brings every wall back.
    env.document.undo().wait().unwrap();
    for wall in &walls {
        near(env.thickness(*wall), 0.2);
    }
    near(room_area(&env), 9.8 * 5.8);
}

#[test]
fn a_type_change_that_would_break_an_opening_is_refused_naming_it() {
    let env = Env::new();
    let wall = env.wall([0.0, 0.0], [4.0, 0.0], json!({}));
    env.run("archbuilder.door.place", json!({"wall": wall}))
        .unwrap();
    let single = env.type_named(TypeKind::Door, "Single 900 × 2100");
    let error = env
        .run(
            "archbuilder.type.set",
            json!({"type": single, "width": 5.0}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "ab.model.insert_outside");
    // A door's sill comes from its type and stays the instance's own afterwards.
    env.run(
        "archbuilder.type.set",
        json!({"type": single, "sill": 0.15}),
    )
    .unwrap();
    let made = env
        .run(
            "archbuilder.door.place",
            json!({"wall": wall, "distance": 0.6, "width": 0.7}),
        )
        .unwrap()
        .value;
    let snapshot = env.snapshot();
    let id: ObjectId = made["insert"].as_str().unwrap().parse().unwrap();
    let door =
        archbuilder_model::insert::Insert::of(&snapshot, &spatial::get(&snapshot, id).unwrap())
            .unwrap();
    near(door.sill, 0.15);
    near(door.width, 0.7);
    assert_eq!(spatial::objects(&snapshot, DOOR).len(), 2);
}

#[test]
fn layer_rules_refuse_with_the_reason_and_change_nothing() {
    let env = Env::new();
    let generic = env.type_named(TypeKind::Wall, "Generic 200");
    let before = env.snapshot().revision();
    let code = |layers: Json, core: Json| {
        env.run(
            "archbuilder.type.set",
            json!({"type": generic, "layers": layers, "core": core}),
        )
        .unwrap_err()
        .code()
        .to_owned()
    };
    assert_eq!(
        code(
            json!([{"function": "membrane", "thickness": 0.01}, {"function": "structure", "thickness": 0.2}]),
            json!([1, 2])
        ),
        "ab.model.type_membrane_thickness"
    );
    assert_eq!(
        code(
            json!([{"function": "finish", "thickness": 0.0}]),
            json!([0, 1])
        ),
        "ab.model.type_layer_thickness"
    );
    assert_eq!(
        code(
            json!([{"function": "structure", "thickness": 0.2}]),
            json!([1, 1])
        ),
        "ab.model.type_core"
    );
    assert_eq!(code(json!([]), json!([0, 0])), "ab.model.type_layer_count");
    assert_eq!(env.snapshot().revision(), before);
    // A value a type decides cannot be edited on the instance.
    let wall = env.wall([0.0, 0.0], [4.0, 0.0], json!({}));
    let error = env
        .run(
            "archbuilder.property.set",
            json!({"objects": [wall], "field": "thickness", "value": 0.3}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "ab.cmd.field_from_type");
}

#[test]
fn retyping_matching_and_deleting_a_type_in_use() {
    let env = Env::new();
    let a = env.wall([0.0, 0.0], [4.0, 0.0], json!({}));
    let b = env.wall([0.0, 3.0], [4.0, 3.0], json!({}));
    let c = env.wall(
        [0.0, 6.0],
        [4.0, 6.0],
        json!({"type": "Interior partition 100"}),
    );
    // Match type: b and c get the type of a source wall of the cavity type.
    env.run(
        "archbuilder.type.assign",
        json!({"type": "Exterior masonry cavity", "objects": [a]}),
    )
    .unwrap();
    near(env.thickness(a), 0.365);
    let matched = env
        .run(
            "archbuilder.type.match",
            json!({"source": a, "objects": [b, c]}),
        )
        .unwrap();
    assert_eq!(matched.value["instances"], 2);
    near(env.thickness(c), 0.365);
    // The property grid's choice is the type's number in the list.
    env.run(
        "archbuilder.property.set",
        json!({"objects": [b, c], "field": "type", "value": 2}),
    )
    .unwrap();
    near(env.thickness(b), 0.1);
    // Deleting a type in use needs a replacement.
    let partition = env.type_named(TypeKind::Wall, "Interior partition 100");
    let error = env
        .run("archbuilder.type.delete", json!({"type": partition}))
        .unwrap_err();
    assert_eq!(error.code(), "ab.cmd.type_in_use");
    let generic = env.type_named(TypeKind::Wall, "Generic 200");
    env.run(
        "archbuilder.type.delete",
        json!({"type": partition, "replacement": generic}),
    )
    .unwrap();
    near(env.thickness(b), 0.2);
    assert_eq!(et::of_kind(&env.snapshot(), TypeKind::Wall).len(), 2);
    // A duplicate is named after its source.
    let copy = env
        .run("archbuilder.type.duplicate", json!({"type": generic}))
        .unwrap();
    let snapshot = env.snapshot();
    let id: ObjectId = copy.value["type"].as_str().unwrap().parse().unwrap();
    assert_eq!(
        et::name(&spatial::get(&snapshot, id).unwrap()),
        "Generic 200 (2)"
    );
}

#[test]
fn instances_without_a_type_get_one_per_distinct_thickness() {
    let env = Env::new();
    let walls = [
        env.wall([0.0, 0.0], [4.0, 0.0], json!({"thickness": 0.25})),
        env.wall([0.0, 3.0], [4.0, 3.0], json!({"thickness": 0.25})),
        env.wall([0.0, 6.0], [4.0, 6.0], json!({"thickness": 0.2})),
    ];
    // Take the types away as a document from before types has none.
    env.document
        .submit(move |txn: &mut arc_doc::Txn| {
            for wall in walls {
                txn.modify(wall, FieldPath::new("wall_type"), None)?;
            }
            Ok(())
        })
        .wait()
        .unwrap();
    assert!(archbuilder_commands::element_type::needs_upgrade(
        &env.snapshot()
    ));
    let done = env.run("archbuilder.type.upgrade", json!({})).unwrap();
    assert_eq!(done.value["instances"], 3);
    let snapshot = env.snapshot();
    let ids: Vec<ObjectId> = walls
        .iter()
        .map(|w| et::type_id(TypeKind::Wall, &spatial::get(&snapshot, *w).unwrap().values).unwrap())
        .collect();
    assert_eq!(ids[0], ids[1]);
    assert_eq!(
        et::name(&spatial::get(&snapshot, ids[0]).unwrap()),
        "Generic 250"
    );
    assert_eq!(
        et::name(&spatial::get(&snapshot, ids[2]).unwrap()),
        "Generic 200"
    );
    assert!(!archbuilder_commands::element_type::needs_upgrade(
        &snapshot
    ));
    assert_eq!(spatial::objects(&snapshot, WALL).len(), 3);
}

#[test]
fn a_new_location_line_keeps_the_faces_the_joins_and_the_room() {
    let env = Env::new();
    let c = [[0.0, 0.0], [0.0, 6.0], [10.0, 6.0], [10.0, 0.0]];
    let walls: Vec<ObjectId> = (0..4)
        .map(|k| {
            env.wall(
                c[k],
                c[(k + 1) % 4],
                json!({"type": "Exterior masonry cavity", "location_line": "exterior"}),
            )
        })
        .collect();
    env.run("archbuilder.room.place", json!({"points": [[5.0, 3.0]]}))
        .unwrap();
    let area = |env: &Env| {
        let snapshot = env.snapshot();
        let storey = spatial::get(&snapshot, walls[0])
            .unwrap()
            .header
            .owner
            .unwrap();
        archbuilder_model::room::of_storey(&snapshot, storey)[0].area()
    };
    let before = area(&env).unwrap();
    for line in ["core_center", "interior", "core_exterior"] {
        env.run(
            "archbuilder.property.set",
            json!({"objects": [walls[1]], "field": "location_line", "value": line}),
        )
        .unwrap();
        // The room stays enclosed with the same area: the wall's faces did not move.
        let after = area(&env).unwrap_or_else(|| panic!("room unenclosed after {line}"));
        near(after, before);
    }
}
