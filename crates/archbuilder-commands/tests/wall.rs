//! `archbuilder.wall.create` on a document with the model's validator: what a wall is made with,
//! what is refused, undo and redo, and that a storey with walls is not deleted.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_base::id::Identifier;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::spatial;
use archbuilder_model::types::{WALL, text, wall};
use archbuilder_model::wall::{EXTERIOR_FACE, LocationLine, Wall, face_index};
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
        let snapshot = self.snapshot();
        let building = spatial::buildings(&snapshot)[0].id;
        spatial::storeys(&snapshot, building)
            .iter()
            .find(|s| text(&s.storey.values, 0) == Some(name))
            .unwrap()
            .storey
            .id
    }

    fn dump(&self) -> String {
        let snapshot = self.snapshot();
        let mut lines: Vec<String> = snapshot
            .iter()
            .map(|(_, o)| format!("{} {:?} {:?}", o.id, o.header.owner, o.values))
            .collect();
        lines.sort();
        lines.join("\n")
    }

    fn code_of(&self, command: &str, params: Json) -> &'static str {
        self.run(command, params).unwrap_err().code()
    }

    fn code(&self, params: Json) -> &'static str {
        let before = self.dump();
        let error = self.run("archbuilder.wall.create", params).unwrap_err();
        assert_eq!(self.dump(), before, "a refused wall changes nothing");
        error.code()
    }
}

#[test]
fn a_wall_is_created_with_defaults_and_undo_and_redo_restore_the_exact_state() {
    let env = Env::new();
    let before = env.dump();
    let made = env
        .run(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [4.0, 0.0]}),
        )
        .unwrap()
        .value;
    let after = env.dump();
    let walls = spatial::objects(&env.snapshot(), WALL);
    assert_eq!(walls.len(), 1);
    let wall = &walls[0];
    // On the lowest storey, 0.2 m thick, as high as the next storey (3 m), named in order.
    assert_eq!(wall.header.owner, Some(env.storey("Ground Floor")));
    assert_eq!(text(&wall.values, 0), Some("Wall 1"));
    let data = Wall::from_values(&wall.values).unwrap();
    assert_eq!(
        (data.thickness, data.height, data.base_offset, data.location),
        (0.2, 3.0, 0.0, LocationLine::Center)
    );
    assert_eq!(made["location_line"], "center");
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(env.dump(), before);
    assert!(env.document.redo().wait().unwrap().value);
    assert_eq!(env.dump(), after);
}

#[test]
fn the_height_is_the_distance_to_the_next_level_and_units_in_text_are_read() {
    let env = Env::new();
    env.run(
        "archbuilder.level.set_elevation",
        json!({"level": env.storey("Floor 1"), "elevation": "3.4 m"}),
    )
    .unwrap();
    env.run(
        "archbuilder.wall.create",
        json!({"start": [0.0, 0.0], "end": [1.0, 0.0], "thickness": "300 mm",
               "storey": env.storey("Ground Floor")}),
    )
    .unwrap();
    // The top storey has no storey above it: 3 m.
    env.run(
        "archbuilder.wall.create",
        json!({"start": [0.0, 0.0], "end": [1.0, 0.0], "storey": env.storey("Floor 2")}),
    )
    .unwrap();
    let mut heights: Vec<(f64, f64)> = spatial::objects(&env.snapshot(), WALL)
        .iter()
        .map(|w| {
            let d = Wall::from_values(&w.values).unwrap();
            (d.height, d.thickness)
        })
        .collect();
    heights.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(heights, [(3.0, 0.2), (3.4, 0.3)]);
}

#[test]
fn hostile_and_degenerate_walls_are_refused_and_change_nothing() {
    let env = Env::new();
    let line = |extra: Json| {
        let mut params = json!({"start": [0.0, 0.0], "end": [4.0, 0.0]});
        for (k, v) in extra.as_object().unwrap() {
            params[k] = v.clone();
        }
        params
    };
    assert_eq!(
        env.code(json!({"start": [1.0, 1.0], "end": [1.0, 1.0]})),
        "ab.model.wall_too_short"
    );
    assert_eq!(
        env.code(json!({"start": [1.0, 1.0], "end": [1.0005, 1.0]})),
        "ab.model.wall_too_short"
    );
    for (field, value) in [
        ("thickness", json!(0.0)),
        ("thickness", json!(-0.2)),
        ("thickness", json!(50.0)),
        ("height", json!(0.0)),
        ("height", json!(1.0e9)),
        ("base_offset", json!(5000.0)),
    ] {
        assert_eq!(
            env.code(line(json!({ field: value }))),
            "doc.value.out_of_range",
            "{field}"
        );
    }
    assert_eq!(
        env.code(json!({"start": [0.0, 0.0], "end": [1.0e12, 0.0]})),
        "doc.value.out_of_range"
    );
    assert!(spatial::objects(&env.snapshot(), WALL).is_empty());
    // A storey that does not exist is refused by the command.
    let ghost = ObjectId::from_u128(99);
    assert_eq!(
        env.code(line(json!({"storey": ghost}))),
        "ab.cmd.kind_mismatch"
    );
}

#[test]
fn walls_are_deleted_in_one_undoable_step_and_only_walls() {
    let env = Env::new();
    let ground = env.storey("Ground Floor");
    for x in [0.0, 1.0] {
        env.run(
            "archbuilder.wall.create",
            json!({"start": [x, 0.0], "end": [x, 3.0], "storey": ground}),
        )
        .unwrap();
    }
    let walls: Vec<ObjectId> = spatial::walls(&env.snapshot(), ground)
        .iter()
        .map(|w| w.id)
        .collect();
    let before = env.dump();
    // Nothing is named and nothing selected; a storey is no wall.
    let error = env.run("archbuilder.wall.delete", json!({})).unwrap_err();
    assert_eq!(error.code(), "ab.cmd.nothing_selected");
    let error = env
        .run("archbuilder.wall.delete", json!({"walls": [ground]}))
        .unwrap_err();
    assert_eq!(error.code(), "ab.cmd.kind_mismatch");
    assert_eq!(env.dump(), before);

    env.run("archbuilder.wall.delete", json!({"walls": walls}))
        .unwrap();
    assert!(spatial::walls(&env.snapshot(), ground).is_empty());
    env.document.undo().wait().unwrap();
    assert_eq!(env.dump(), before, "one undo brings both walls back");
    env.document.redo().wait().unwrap();
    assert!(spatial::walls(&env.snapshot(), ground).is_empty());
}

#[test]
fn a_storey_with_walls_is_not_deleted_and_the_message_names_the_count() {
    let env = Env::new();
    let ground = env.storey("Ground Floor");
    for x in [0.0, 1.0] {
        env.run(
            "archbuilder.wall.create",
            json!({"start": [x, 0.0], "end": [x, 3.0], "storey": ground}),
        )
        .unwrap();
    }
    let before = env.dump();
    let error = env
        .run("archbuilder.storey.delete", json!({"storey": ground}))
        .unwrap_err();
    assert_eq!(error.code(), "ab.cmd.storey_has_walls");
    assert!(
        error
            .arguments()
            .iter()
            .any(|(name, value)| *name == "count" && format!("{value:?}").contains('2')),
        "{error:?}"
    );
    assert_eq!(env.dump(), before);
    // A storey without walls still goes.
    env.run(
        "archbuilder.storey.delete",
        json!({"storey": env.storey("Floor 2")}),
    )
    .unwrap();
}

#[test]
fn the_derived_solid_of_a_created_wall_has_the_documented_volume_and_exterior_side() {
    let env = Env::new();
    for line in ["center", "exterior", "interior"] {
        env.run(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [6.0, 0.0], "thickness": 0.25,
                   "height": 2.8, "location_line": line}),
        )
        .unwrap();
    }
    for w in spatial::objects(&env.snapshot(), WALL) {
        let data = Wall::from_values(&w.values).unwrap();
        let solid = data.solid(0.0).unwrap();
        assert!((solid.volume() - 6.0 * 0.25 * 2.8).abs() < 1e-9);
        // The baseline runs along +x, so the exterior (left) face looks toward +y.
        let normal = solid
            .face_normal(face_index(&solid, EXTERIOR_FACE).unwrap())
            .unwrap();
        assert!((normal.y - 1.0).abs() < 1e-9);
        let (low, high) = data.extent();
        let ys: Vec<f64> = solid.vertices().iter().map(|p| p.y).collect();
        assert!(ys.iter().any(|y| (y - high).abs() < 1e-9));
        assert!(ys.iter().any(|y| (y - low).abs() < 1e-9));
        let _ = wall::NAME;
    }
}

fn wall_of(env: &Env, id: ObjectId) -> Wall {
    let snapshot = env.snapshot();
    let object = spatial::get(&snapshot, id).unwrap();
    Wall::of(&snapshot, &object).unwrap()
}

#[test]
fn the_top_follows_the_level_above_until_a_height_is_set() {
    let env = Env::new();
    let made = env
        .run(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [4.0, 0.0]}),
        )
        .unwrap()
        .value;
    let id: ObjectId = made["wall"].as_str().unwrap().parse().unwrap();
    assert!(made["top_level"].is_string(), "{made}");
    assert_eq!(wall_of(&env, id).height, 3.0);
    // Raising the level above makes the wall taller.
    env.run(
        "archbuilder.level.set_elevation",
        json!({"level": env.storey("Floor 1"), "elevation": "3.5 m"}),
    )
    .ok();
    let floor1 = env.storey("Floor 1");
    env.run(
        "archbuilder.property.set",
        json!({"objects": [floor1], "field": "elevation", "value": "3.5 m"}),
    )
    .unwrap();
    assert!((wall_of(&env, id).height - 3.5).abs() < 1e-9);
    // A given height leaves the top unconnected; the property does the same on a placed wall.
    let free = env
        .run(
            "archbuilder.wall.create",
            json!({"start": [0.0, 1.0], "end": [4.0, 1.0], "height": "2 m"}),
        )
        .unwrap()
        .value;
    assert!(free["top_level"].is_null());
    env.run(
        "archbuilder.property.set",
        json!({"objects": [id], "field": "height", "value": "2.5 m"}),
    )
    .unwrap();
    let wall = wall_of(&env, id);
    assert!((wall.height - 2.5).abs() < 1e-9);
    assert!(Wall::top_level(&spatial::get(&env.snapshot(), id).unwrap().values).is_none());
}

#[test]
fn a_location_line_change_keeps_the_body_and_flip_swaps_the_side() {
    let env = Env::new();
    let made = env
        .run(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [4.0, 0.0], "location_line": "exterior"}),
        )
        .unwrap()
        .value;
    let id: ObjectId = made["wall"].as_str().unwrap().parse().unwrap();
    env.run(
        "archbuilder.property.set",
        json!({"objects": [id], "field": "location_line", "value": "center"}),
    )
    .unwrap();
    let wall = wall_of(&env, id);
    assert_eq!(wall.location, LocationLine::Center);
    assert!((wall.start[1] + 0.1).abs() < 1e-9, "{wall:?}");
    // Flip a wall on its center line: the body stays and the exterior side swaps.
    let before = wall_of(&env, id);
    let body = |w: &Wall| {
        let (low, high) = w.extent();
        let (dx, dy) = (w.end[0] - w.start[0], w.end[1] - w.start[1]);
        let len = dx.hypot(dy);
        // The two edges of the body across the baseline, as absolute y of a horizontal wall.
        (
            w.start[1] + dx / len * low,
            w.start[1] + dx / len * high,
            dy,
        )
    };
    env.run("archbuilder.wall.flip", json!({"walls": [id]}))
        .unwrap();
    let flipped = wall_of(&env, id);
    assert_eq!((flipped.start, flipped.end), (before.end, before.start));
    assert_eq!(flipped.location, LocationLine::Center);
    let (a, b) = (body(&before), body(&flipped));
    assert!((a.0.min(a.1) - b.0.min(b.1)).abs() < 1e-9, "the body stays");
    // An exterior face on the line stays an exterior face and the body goes to the other side.
    env.run(
        "archbuilder.property.set",
        json!({"objects": [id], "field": "location_line", "value": "exterior"}),
    )
    .unwrap();
    let on_face = wall_of(&env, id);
    env.run("archbuilder.wall.flip", json!({"walls": [id]}))
        .unwrap();
    let across = wall_of(&env, id);
    assert_eq!(across.location, LocationLine::Exterior);
    assert_eq!((across.start, across.end), (on_face.end, on_face.start));
    let (a, b) = (body(&on_face), body(&across));
    assert!(a.0.min(a.1) * b.0.min(b.1) < 0.0 || (a.0.min(a.1) - b.0.min(b.1)).abs() > 0.05);
    env.document.undo().wait().unwrap();
    assert_eq!(wall_of(&env, id).start, on_face.start);
}

#[test]
fn a_level_cannot_go_below_the_walls_that_follow_it_and_deleting_it_keeps_their_height() {
    let env = Env::new();
    let made = env
        .run(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [4.0, 0.0]}),
        )
        .unwrap()
        .value;
    let id: ObjectId = made["wall"].as_str().unwrap().parse().unwrap();
    let floor1 = env.storey("Floor 1");
    let before = env.dump();
    let error = env
        .run(
            "archbuilder.property.set",
            json!({"objects": [floor1], "field": "elevation", "value": "-1 m"}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "ab.model.level_below_walls");
    assert!(
        error
            .arguments()
            .iter()
            .any(|(name, value)| *name == "count" && format!("{value:?}").contains('1')),
        "{error:?}"
    );
    assert_eq!(env.dump(), before);
    // Raised, the wall follows; deleting the level above keeps the height it has then.
    env.run(
        "archbuilder.property.set",
        json!({"objects": [floor1], "field": "elevation", "value": "4.5 m"}),
    )
    .unwrap();
    env.run("archbuilder.storey.delete", json!({"storey": floor1}))
        .unwrap();
    let object = spatial::get(&env.snapshot(), id).unwrap();
    assert!(Wall::top_level(&object.values).is_none());
    assert!((wall_of(&env, id).height - 4.5).abs() < 1e-9);
    // The grid reconnects the top to a level by name, and disconnects it again.
    env.run(
        "archbuilder.property.set",
        json!({"objects": [id], "field": "top_constraint", "value": "Floor 2"}),
    )
    .unwrap();
    assert!((wall_of(&env, id).height - 6.0).abs() < 1e-9);
    env.run(
        "archbuilder.property.set",
        json!({"objects": [id], "field": "top_constraint", "value": "unconnected"}),
    )
    .unwrap();
    let object = spatial::get(&env.snapshot(), id).unwrap();
    assert!(Wall::top_level(&object.values).is_none());
    assert!((wall_of(&env, id).height - 6.0).abs() < 1e-9);
    // The grid's own choice is a number: 1 is the first level above.
    env.run(
        "archbuilder.property.set",
        json!({"objects": [id], "field": "top_constraint", "value": 1}),
    )
    .unwrap();
    let object = spatial::get(&env.snapshot(), id).unwrap();
    assert!(Wall::top_level(&object.values).is_some());
}

/// The joined volume of every wall of the lowest storey.
fn joined_volume(env: &Env) -> f64 {
    let snapshot = env.snapshot();
    let walls = spatial::objects(&snapshot, WALL);
    walls
        .iter()
        .map(|w| {
            let data = Wall::from_values(&w.values).unwrap();
            let foot = archbuilder_model::join::footprint_of(&snapshot, w).unwrap();
            data.solid_in(foot, 0.0).unwrap().volume()
        })
        .sum()
}

#[test]
fn a_closed_chain_counts_its_corners_once_and_deleting_a_wall_squares_its_neighbors() {
    let env = Env::new();
    let corners = [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]];
    for k in 0..4 {
        env.run(
            "archbuilder.wall.create",
            json!({"start": corners[k], "end": corners[(k + 1) % 4], "height": 3.0}),
        )
        .unwrap();
    }
    let ring = (10.2 * 6.2 - 9.8 * 5.8) * 3.0;
    assert!((joined_volume(&env) - ring).abs() < 1e-9);
    let before = env.dump();
    let bottom = spatial::objects(&env.snapshot(), WALL)
        .iter()
        .find(|w| Wall::from_values(&w.values).unwrap().start == [0.0, 0.0])
        .unwrap()
        .id;
    env.run("archbuilder.wall.delete", json!({"walls": [bottom]}))
        .unwrap();
    // Two mitered corners remain; the free ends are square. Each bar counts its corner once:
    // 6.0 * 0.2 on each side (the miter averages the outer and inner lengths) and 10 * 0.2.
    let v = joined_volume(&env);
    assert!((v - (6.0 + 10.0 + 6.0) * 0.2 * 3.0).abs() < 1e-9, "{v}");
    env.document.undo().wait().unwrap();
    assert_eq!(env.dump(), before);
    assert!(
        (joined_volume(&env) - ring).abs() < 1e-9,
        "undo restores the joins"
    );
}

#[test]
fn join_styles_are_set_per_end_swap_with_a_flip_and_undo() {
    let env = Env::new();
    for (a, b) in [([0.0, 0.0], [4.0, 0.0]), ([4.0, 0.0], [4.0, 3.0])] {
        env.run("archbuilder.wall.create", json!({"start": a, "end": b}))
            .unwrap();
    }
    let ids: Vec<_> = spatial::objects(&env.snapshot(), WALL)
        .iter()
        .map(|w| w.id)
        .collect();
    let miter = joined_volume(&env);
    let style = |env: &Env, k: usize| {
        let w = spatial::get(&env.snapshot(), ids[k]).unwrap();
        let d = Wall::from_values(&w.values).unwrap();
        (d.start_join.name(), d.end_join.name())
    };
    assert_eq!(style(&env, 0), ("auto", "auto"));
    let before = env.dump();
    env.run(
        "archbuilder.wall.join_style",
        json!({"walls": [ids[0]], "end": "end", "style": "butt"}),
    )
    .unwrap();
    assert_eq!(style(&env, 0), ("auto", "butt"));
    // Butt keeps the union: the corner is still filled once.
    assert!((joined_volume(&env) - miter).abs() < 1e-9);
    env.document.undo().wait().unwrap();
    assert_eq!(env.dump(), before);
    // Disallowing both ends of a wall: it and its neighbor stay square and overlap.
    env.run("archbuilder.wall.disallow_join", json!({"walls": [ids[0]]}))
        .unwrap();
    assert_eq!(style(&env, 0), ("disallowed", "disallowed"));
    assert!((joined_volume(&env) - (4.0 + 3.0) * 0.2 * 3.0).abs() < 1e-9);
    env.run(
        "archbuilder.wall.allow_join",
        json!({"walls": [ids[0]], "end": "end"}),
    )
    .unwrap();
    assert_eq!(style(&env, 0), ("disallowed", "auto"));
    // A flip swaps the styles with the ends.
    env.run("archbuilder.wall.flip", json!({"walls": [ids[0]]}))
        .unwrap();
    assert_eq!(style(&env, 0), ("auto", "disallowed"));
    assert_eq!(
        env.code_of(
            "archbuilder.wall.join_style",
            json!({"walls": [ids[0]], "style": "bevel"})
        ),
        "cmd.params.invalid"
    );
}

#[test]
fn the_cached_footprints_of_a_storey_equal_the_uncached_ones() {
    // 400 walls: 20 rows of 10 pieces and 20 columns of 10 pieces.
    let env = Env::new();
    for i in 0..20 {
        for k in 0..10 {
            let (a, b, c) = (f64::from(k), f64::from(k + 1), f64::from(i));
            for (s, e) in [([a, c], [b, c]), ([c, a], [c, b])] {
                env.run(
                    "archbuilder.wall.create",
                    json!({"start": s, "end": e, "height": 3.0}),
                )
                .unwrap();
            }
        }
    }
    let snapshot = env.snapshot();
    let walls = spatial::objects(&snapshot, WALL);
    assert_eq!(walls.len(), 400);
    let started = std::time::Instant::now();
    let plain: Vec<_> = walls
        .iter()
        .map(|w| archbuilder_model::join::footprint_uncached(&snapshot, w).unwrap())
        .collect();
    let uncached = started.elapsed();
    let started = std::time::Instant::now();
    let shared: Vec<_> = walls
        .iter()
        .map(|w| archbuilder_model::join::footprint_of(&snapshot, w).unwrap())
        .collect();
    let cached = started.elapsed();
    assert_eq!(plain, shared);
    let _ = (uncached, cached);
}
