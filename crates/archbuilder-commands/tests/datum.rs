//! Grids, reference planes, work planes and datum extents.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::datum;
use archbuilder_model::spatial;
use archbuilder_model::types::{GRID, REFERENCE_PLANE, WORK_PLANE};
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
}

fn labels(env: &Env) -> Vec<String> {
    let s = env.snapshot();
    let mut found: Vec<String> = spatial::objects(&s, GRID)
        .iter()
        .filter_map(|g| archbuilder_model::types::text(&g.values, 0).map(str::to_owned))
        .collect();
    found.sort();
    found
}

fn id_of(outcome: &Outcome, key: &str) -> String {
    outcome.value[key].as_str().unwrap().to_owned()
}

#[test]
fn grids_label_themselves_in_sequence_and_undo() {
    let env = Env::new();
    for k in 0..3 {
        let x = f64::from(k) * 5.0;
        env.run(
            "archbuilder.grid.create",
            json!({"start": [x, 0.0], "end": [x, 10.0]}),
        )
        .unwrap();
    }
    assert_eq!(labels(&env), ["1", "2", "3"]);
    // An alphabetic series follows its own last label; a sub-grid increments after the dot.
    let a = env
        .run(
            "archbuilder.grid.create",
            json!({"start": [0.0, 0.0], "end": [10.0, 0.0], "label": "A"}),
        )
        .unwrap();
    let b = env
        .run(
            "archbuilder.grid.create",
            json!({"start": [0.0, 5.0], "end": [10.0, 5.0], "after": "A"}),
        )
        .unwrap();
    assert_eq!(b.value["label"], "B");
    let sub = env
        .run(
            "archbuilder.grid.create",
            json!({"start": [0.0, 7.0], "end": [10.0, 7.0], "label": "A.1"}),
        )
        .unwrap();
    let next = env
        .run(
            "archbuilder.grid.create",
            json!({"start": [0.0, 8.0], "end": [10.0, 8.0], "after": "A.1"}),
        )
        .unwrap();
    assert_eq!(next.value["label"], "A.2");
    let _ = (a, sub);
    env.document.undo().wait().unwrap();
    assert!(!labels(&env).contains(&"A.2".to_owned()));
    // A grid shorter than a millimeter is refused and leaves the document alone.
    let before = labels(&env);
    assert!(
        env.run(
            "archbuilder.grid.create",
            json!({"start": [0.0, 0.0], "end": [0.0, 0.0]})
        )
        .is_err()
    );
    assert_eq!(labels(&env), before);
}

#[test]
fn duplicate_labels_are_reported_not_refused() {
    let env = Env::new();
    for x in [0.0, 5.0] {
        env.run(
            "archbuilder.grid.create",
            json!({"start": [x, 0.0], "end": [x, 10.0], "label": "A"}),
        )
        .unwrap();
    }
    assert_eq!(datum::duplicate_labels(&env.snapshot()).len(), 1);
    let register = archbuilder_model::validate::register(&env.snapshot());
    assert!(
        register
            .iter()
            .any(|w| w.error.code() == "ab.model.datum_label_duplicate")
    );
}

#[test]
fn a_grid_takes_a_walls_baseline_and_modify_moves_copies_and_mirrors_it() {
    let env = Env::new();
    let storey = env.storeys()[0];
    let wall = env
        .run(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [8.0, 0.0], "storey": storey.to_string()}),
        )
        .unwrap();
    let wall_id = id_of(&wall, "wall");
    let grid = env
        .run(
            "archbuilder.grid.create",
            json!({"from_wall": wall_id, "bulge": 0.2}),
        )
        .unwrap();
    let id: ObjectId = id_of(&grid, "grid").parse().unwrap();
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [id.to_string()], "displacement": [1.0, 2.0]}),
    )
    .unwrap();
    let read = |id: ObjectId| {
        datum::Grid::from_values(&spatial::get(&env.snapshot(), id).unwrap().values).unwrap()
    };
    assert_eq!(read(id).start, [1.0, 2.0]);
    assert_eq!(read(id).end, [9.0, 2.0]);
    let copy = env
        .run(
            "archbuilder.modify.copy",
            json!({"objects": [id.to_string()], "displacement": [0.0, 5.0]}),
        )
        .unwrap();
    let new: ObjectId = copy.value["created"][0].as_str().unwrap().parse().unwrap();
    assert_eq!(read(new).label, "2");
    assert_eq!(read(new).start, [1.0, 7.0]);
    env.run(
        "archbuilder.modify.mirror",
        json!({"objects": [id.to_string()], "axis": [[0.0, 0.0], [1.0, 0.0]], "keep": false}),
    )
    .unwrap();
    assert!((read(id).bulge + 0.2).abs() < 1e-12);
    env.document.undo().wait().unwrap();
    assert!((read(id).bulge - 0.2).abs() < 1e-12);
}

#[test]
fn a_pinned_grid_cannot_be_moved() {
    let env = Env::new();
    let grid = env
        .run(
            "archbuilder.grid.create",
            json!({"start": [0.0, 0.0], "end": [0.0, 10.0]}),
        )
        .unwrap();
    let id = id_of(&grid, "grid");
    env.run("archbuilder.element.pin", json!({"objects": [id]}))
        .unwrap();
    assert!(
        env.run(
            "archbuilder.modify.move",
            json!({"objects": [id], "displacement": [1.0, 0.0]})
        )
        .is_err()
    );
}

#[test]
fn extents_choose_the_levels_a_grid_shows_on_and_its_heads() {
    let env = Env::new();
    let grid = env
        .run(
            "archbuilder.grid.create",
            json!({"start": [0.0, 0.0], "end": [0.0, 10.0]}),
        )
        .unwrap();
    let id: ObjectId = id_of(&grid, "grid").parse().unwrap();
    let object = |env: &Env| spatial::get(&env.snapshot(), id).unwrap();
    // Everywhere by default: 0 m, 3 m and 6 m levels.
    for z in [0.0, 3.0, 6.0] {
        assert!(datum::shows_at(&env.snapshot(), &object(&env), z));
    }
    env.run(
        "archbuilder.datum.set_extent",
        json!({"objects": [id.to_string()], "top_level": "Floor 1", "head_end": false}),
    )
    .unwrap();
    assert!(datum::shows_at(&env.snapshot(), &object(&env), 3.0));
    assert!(!datum::shows_at(&env.snapshot(), &object(&env), 6.0));
    let g = datum::Grid::from_values(&object(&env).values).unwrap();
    assert!(g.head_start && !g.head_end);
    env.document.undo().wait().unwrap();
    assert!(datum::shows_at(&env.snapshot(), &object(&env), 6.0));
}

#[test]
fn reference_planes_have_a_strength_and_unsnapped_ones_leave_the_snap_lines() {
    let env = Env::new();
    for (name, strength) in [("S", "strong"), ("N", "none")] {
        env.run(
            "archbuilder.reference_plane.create",
            json!({"start": [0.0, 0.0], "end": [4.0, 0.0], "name": name, "strength": strength,
                   "bounded": false}),
        )
        .unwrap();
    }
    assert_eq!(spatial::objects(&env.snapshot(), REFERENCE_PLANE).len(), 2);
    let lines = datum::lines(&env.snapshot(), None);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].strong);
    assert!(
        env.run(
            "archbuilder.reference_plane.create",
            json!({"start": [0.0, 0.0], "end": [4.0, 0.0], "strength": "heavy"}),
        )
        .is_err()
    );
}

#[test]
fn a_work_plane_follows_its_datum_and_only_one_is_active() {
    let env = Env::new();
    let grid = env
        .run(
            "archbuilder.grid.create",
            json!({"start": [0.0, 0.0], "end": [10.0, 0.0]}),
        )
        .unwrap();
    let gid = id_of(&grid, "grid");
    // Nothing to build a plane from is refused.
    assert!(env.run("archbuilder.work_plane.set", json!({})).is_err());
    env.run("archbuilder.work_plane.set", json!({"target": gid}))
        .unwrap();
    let level = env
        .run("archbuilder.work_plane.set", json!({"target": "Floor 1"}))
        .unwrap();
    let building = spatial::buildings(&env.snapshot())[0].id;
    let (active, plane) = datum::active(&env.snapshot(), building).unwrap();
    assert_eq!(active.id.to_string(), id_of(&level, "work_plane"));
    assert_eq!(plane.unwrap().origin[2], 3.0);
    // The first plane is the grid's: moving the grid moves the plane.
    env.run("archbuilder.work_plane.set", json!({"target": gid}))
        .unwrap();
    assert_eq!(spatial::objects(&env.snapshot(), WORK_PLANE).len(), 2);
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [gid], "displacement": [0.0, 4.0]}),
    )
    .unwrap();
    let (_, plane) = datum::active(&env.snapshot(), building).unwrap();
    assert_eq!(plane.unwrap().origin[1], 4.0);
    // Three points on a line make no plane.
    assert!(
        env.run(
            "archbuilder.work_plane.set",
            json!({"points": [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]}),
        )
        .is_err()
    );
    env.run(
        "archbuilder.work_plane.set",
        json!({"points": [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]}),
    )
    .unwrap();
    let (_, plane) = datum::active(&env.snapshot(), building).unwrap();
    assert_eq!(plane.unwrap().normal, [0.0, 0.0, 1.0]);
    env.run("archbuilder.work_plane.show", json!({"shown": false}))
        .unwrap();
    // Deleting the grid takes the plane that follows it along; undo brings both back.
    let invocation = Invocation {
        seed: env.seed,
        selection: vec![gid.parse().unwrap()],
        ..Invocation::new("archbuilder.object.delete", json!({}))
    };
    Invoker::new(&env.registry)
        .invoke(&env.document, &env.session, invocation)
        .unwrap();
    assert_eq!(spatial::objects(&env.snapshot(), WORK_PLANE).len(), 2);
    env.document.undo().wait().unwrap();
    assert_eq!(spatial::objects(&env.snapshot(), WORK_PLANE).len(), 3);
    assert_eq!(spatial::objects(&env.snapshot(), GRID).len(), 1);
}

fn grid_of(env: &Env, id: ObjectId) -> datum::Grid {
    datum::Grid::from_values(&spatial::get(&env.snapshot(), id).unwrap().values).unwrap()
}

#[test]
fn a_multi_segment_grid_runs_through_its_vertices_and_follows_modify() {
    let env = Env::new();
    // Two straight segments and a last one that is an arc.
    let made = env
        .run(
            "archbuilder.grid.create",
            json!({"points": [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [4.0, 6.0]],
                   "bulges": [0.0, 0.0, 0.4]}),
        )
        .unwrap();
    let id: ObjectId = id_of(&made, "grid").parse().unwrap();
    let g = grid_of(&env, id);
    assert_eq!(g.via.len(), 2);
    assert_eq!(g.points().len(), 4);
    assert!((g.bulge - 0.4).abs() < 1e-12);
    let line = g.polyline();
    assert!(line.contains(&[10.0, 0.0]) && line.contains(&[10.0, 6.0]));
    assert!(line.len() > 4, "the arc is flattened");
    assert_eq!(line.last(), Some(&[4.0, 6.0]));
    assert!((g.length() - 22.0).abs() < 1e-9);
    // Modify moves every vertex; a mirror flips the arc.
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [id.to_string()], "displacement": [1.0, 2.0]}),
    )
    .unwrap();
    assert_eq!(grid_of(&env, id).via[0].0, [11.0, 2.0]);
    env.run(
        "archbuilder.modify.mirror",
        json!({"objects": [id.to_string()], "axis": [[0.0, 0.0], [1.0, 0.0]], "keep": false}),
    )
    .unwrap();
    let mirrored = grid_of(&env, id);
    assert_eq!(mirrored.via[1].0, [11.0, -8.0]);
    assert!((mirrored.bulge + 0.4).abs() < 1e-12);
    // A copy keeps the vertices, moved.
    let copy = env
        .run(
            "archbuilder.modify.copy",
            json!({"objects": [id.to_string()], "displacement": [0.0, 20.0]}),
        )
        .unwrap();
    let new: ObjectId = copy.value["created"][0].as_str().unwrap().parse().unwrap();
    assert_eq!(grid_of(&env, new).via.len(), 2);
    assert_eq!(grid_of(&env, new).via[0].0, [11.0, 18.0]);
    env.document.undo().wait().unwrap();
    env.document.undo().wait().unwrap();
    assert_eq!(grid_of(&env, id).via[0].0, [11.0, 2.0]);
    // Wrong counts and loose bulges are refused.
    for bad in [
        json!({"points": [[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]], "bulges": [0.0]}),
        json!({"points": [[0.0, 0.0]]}),
        json!({"points": [[0.0, 0.0], [1.0, 0.0]], "bulges": [99.0]}),
    ] {
        assert!(env.run("archbuilder.grid.create", bad).is_err());
    }
}

#[test]
fn a_grid_follows_a_chain_of_walls_in_either_order() {
    let env = Env::new();
    let storey = env.storeys()[0].to_string();
    let mut ids = Vec::new();
    // The second wall is drawn backwards, the first meets it at its start.
    for (a, b) in [([8.0, 0.0], [0.0, 0.0]), ([8.0, 0.0], [8.0, 5.0])] {
        let w = env
            .run(
                "archbuilder.wall.create",
                json!({"start": a, "end": b, "storey": storey}),
            )
            .unwrap();
        ids.push(id_of(&w, "wall"));
    }
    let made = env
        .run("archbuilder.grid.create", json!({"from_walls": ids}))
        .unwrap();
    let g = grid_of(&env, id_of(&made, "grid").parse().unwrap());
    assert_eq!(g.points(), vec![[0.0, 0.0], [8.0, 0.0], [8.0, 5.0]]);
    // Walls that do not meet are refused.
    let far = env
        .run(
            "archbuilder.wall.create",
            json!({"start": [30.0, 0.0], "end": [40.0, 0.0], "storey": storey}),
        )
        .unwrap();
    assert!(
        env.run(
            "archbuilder.grid.create",
            json!({"from_walls": [ids[0].clone(), id_of(&far, "wall")]})
        )
        .is_err()
    );
}
