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

fn view_settings(env: &Env, view: ObjectId) -> archbuilder_model::view::PlanView {
    archbuilder_model::view::PlanView::from_values(
        &spatial::get(&env.snapshot(), view).unwrap().values,
    )
    .unwrap()
}

#[test]
fn a_view_sets_its_own_grid_extent_and_heads_and_propagates_them() {
    let env = Env::new();
    let grid = env
        .run(
            "archbuilder.grid.create",
            json!({"start": [0.0, 0.0], "end": [10.0, 0.0]}),
        )
        .unwrap();
    let gid = id_of(&grid, "grid");
    let storeys = env.storeys();
    // Each storey's first view; a duplicate makes a second view of the first storey.
    let a = env
        .run(
            "archbuilder.view.duplicate",
            json!({"view": storeys[0].to_string()}),
        )
        .unwrap();
    let a: ObjectId = id_of(&a, "view").parse().unwrap();
    let b = env
        .run(
            "archbuilder.view.duplicate",
            json!({"view": storeys[1].to_string()}),
        )
        .unwrap();
    let b: ObjectId = id_of(&b, "view").parse().unwrap();
    let c = env
        .run(
            "archbuilder.view.duplicate",
            json!({"view": storeys[2].to_string()}),
        )
        .unwrap();
    let c: ObjectId = id_of(&c, "view").parse().unwrap();
    // The adjustment in one view: the start pulled in 2 m, the end pushed out 1 m, no end head.
    env.run(
        "archbuilder.datum.set_view_extent",
        json!({"objects": [gid], "views": [a.to_string()], "start_inset": 2.0,
               "end_inset": -1.0, "head_end": false}),
    )
    .unwrap();
    let id: ObjectId = gid.parse().unwrap();
    let g = grid_of(&env, id);
    // The model extent is untouched.
    assert_eq!((g.start, g.end), ([0.0, 0.0], [10.0, 0.0]));
    assert!(g.head_start && g.head_end);
    let ov = view_settings(&env, a).datum_views;
    assert_eq!(ov.len(), 1);
    let shown = g.in_view(ov.first()).unwrap();
    assert_eq!((shown.start, shown.end), ([2.0, 0.0], [11.0, 0.0]));
    assert!(shown.head_start && !shown.head_end);
    assert!(view_settings(&env, b).datum_views.is_empty());
    // Propagated to the two other views in one command: one undo step takes both back.
    env.run(
        "archbuilder.datum.set_view_extent",
        json!({"objects": [gid], "views": [b.to_string(), c.to_string()],
               "from_view": a.to_string()}),
    )
    .unwrap();
    for v in [b, c] {
        assert_eq!(view_settings(&env, v).datum_views, ov);
    }
    env.document.undo().wait().unwrap();
    assert!(view_settings(&env, b).datum_views.is_empty());
    assert!(view_settings(&env, c).datum_views.is_empty());
    // A reset shows the grid's own extent again; a view that pulls the line in past its other end
    // shows nothing; nothing to change is refused.
    env.run(
        "archbuilder.datum.set_view_extent",
        json!({"objects": [gid], "views": [a.to_string()], "reset": true}),
    )
    .unwrap();
    assert!(view_settings(&env, a).datum_views.is_empty());
    env.run(
        "archbuilder.datum.set_view_extent",
        json!({"objects": [gid], "views": [a.to_string()], "start_inset": 6.0, "end_inset": 6.0}),
    )
    .unwrap();
    assert!(
        grid_of(&env, id)
            .in_view(view_settings(&env, a).datum_views.first())
            .is_none()
    );
    for bad in [
        json!({"objects": [gid], "views": [a.to_string()]}),
        json!({"objects": [gid], "views": [a.to_string()], "start_inset": 1.0e9}),
        json!({"objects": [gid], "views": []}),
    ] {
        assert!(env.run("archbuilder.datum.set_view_extent", bad).is_err());
    }
    // Deleting the grid leaves no dangling override: the view still opens and draws.
    let invocation = Invocation {
        seed: env.seed,
        selection: vec![id],
        ..Invocation::new("archbuilder.object.delete", json!({}))
    };
    Invoker::new(&env.registry)
        .invoke(&env.document, &env.session, invocation)
        .unwrap();
    assert!(view_settings(&env, a).datum_views.is_empty());
}

fn active_plane(env: &Env) -> datum::Plane {
    let building = spatial::buildings(&env.snapshot())[0].id;
    datum::active(&env.snapshot(), building).unwrap().1.unwrap()
}

fn delete(env: &Env, id: ObjectId) {
    let invocation = Invocation {
        seed: env.seed,
        selection: vec![id],
        ..Invocation::new("archbuilder.object.delete", json!({}))
    };
    Invoker::new(&env.registry)
        .invoke(&env.document, &env.session, invocation)
        .unwrap();
}

#[test]
fn a_work_plane_lies_on_a_wall_face_and_follows_and_leaves_with_the_wall() {
    let env = Env::new();
    let storey = env.storeys()[0].to_string();
    let wall = env
        .run(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [8.0, 0.0], "storey": storey}),
        )
        .unwrap();
    let wall_id: ObjectId = id_of(&wall, "wall").parse().unwrap();
    let t = 0.1; // half the default thickness: a center-line wall
    // The exterior face is the baseline's left; the normal points away from the body.
    env.run(
        "archbuilder.work_plane.set",
        json!({"element": wall_id.to_string()}),
    )
    .unwrap();
    let p = active_plane(&env);
    assert!((p.origin[1] - t).abs() < 1e-9 && p.normal == [0.0, 1.0, 0.0]);
    env.run(
        "archbuilder.work_plane.set",
        json!({"element": wall_id.to_string(), "face": "interior"}),
    )
    .unwrap();
    let p = active_plane(&env);
    assert!((p.origin[1] + t).abs() < 1e-9 && p.normal == [0.0, -1.0, 0.0]);
    // Asking for the same face again reuses the plane; another face makes a second one.
    env.run(
        "archbuilder.work_plane.set",
        json!({"element": wall_id.to_string(), "face": "interior"}),
    )
    .unwrap();
    assert_eq!(spatial::objects(&env.snapshot(), WORK_PLANE).len(), 2);
    // A wall has no top face; a floor has no exterior one; an unknown name is refused.
    for bad in [
        json!({"element": wall_id.to_string(), "face": "top"}),
        json!({"element": wall_id.to_string(), "face": "left"}),
        json!({"element": storey}),
    ] {
        assert!(env.run("archbuilder.work_plane.set", bad).is_err());
    }
    // The plane follows the wall when it moves.
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [wall_id.to_string()], "displacement": [0.0, 3.0]}),
    )
    .unwrap();
    assert!((active_plane(&env).origin[1] - (3.0 - t)).abs() < 1e-9);
    // A selected wall gives its face without a name.
    let invocation = Invocation {
        seed: env.seed,
        selection: vec![wall_id],
        ..Invocation::new("archbuilder.work_plane.set", json!({"face": "exterior"}))
    };
    Invoker::new(&env.registry)
        .invoke(&env.document, &env.session, invocation)
        .unwrap();
    assert!((active_plane(&env).origin[1] - (3.0 + t)).abs() < 1e-9);
    // Deleting the wall deletes the planes on its faces; undo brings them back.
    delete(&env, wall_id);
    assert!(spatial::objects(&env.snapshot(), WORK_PLANE).is_empty());
    env.document.undo().wait().unwrap();
    assert_eq!(spatial::objects(&env.snapshot(), WORK_PLANE).len(), 3 - 1);
}

#[test]
fn a_work_plane_lies_on_a_slab_face() {
    let env = Env::new();
    let storey = env.storeys()[1].to_string();
    let floor = env
        .run(
            "archbuilder.floor.create",
            json!({"points": [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]],
                   "storey": storey, "offset": 0.05}),
        )
        .unwrap();
    let floor_id = id_of(&floor, "floor");
    let thickness = floor.value["thickness"].as_f64().unwrap();
    env.run("archbuilder.work_plane.set", json!({"element": floor_id}))
        .unwrap();
    let top = active_plane(&env);
    assert!((top.origin[2] - 3.05).abs() < 1e-9 && top.normal == [0.0, 0.0, 1.0]);
    env.run(
        "archbuilder.work_plane.set",
        json!({"element": floor_id, "face": "bottom"}),
    )
    .unwrap();
    let bottom = active_plane(&env);
    assert!((bottom.origin[2] - (3.05 - thickness)).abs() < 1e-9);
    assert_eq!(bottom.normal, [0.0, 0.0, -1.0]);
    // The level of the storey moves: the face follows the floor.
    env.run(
        "archbuilder.property.set",
        json!({"objects": [floor_id], "field": "top_offset", "value": 0.5}),
    )
    .unwrap();
    assert!((active_plane(&env).origin[2] - (3.5 - thickness)).abs() < 1e-9);
}

#[test]
fn sketches_store_their_work_plane_and_a_roof_by_extrusion_follows_it() {
    let env = Env::new();
    let grid = env
        .run(
            "archbuilder.grid.create",
            json!({"start": [0.0, 0.0], "end": [10.0, 0.0]}),
        )
        .unwrap();
    let gid = id_of(&grid, "grid");
    let plane = env
        .run("archbuilder.work_plane.set", json!({"target": gid}))
        .unwrap();
    let wp = id_of(&plane, "work_plane");
    let storey = env.storeys()[0].to_string();
    // A floor sketched on the plane keeps a reference to it; the plane goes, the reference clears.
    let floor = env
        .run(
            "archbuilder.floor.create",
            json!({"points": [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]],
                   "storey": storey, "work_plane": wp}),
        )
        .unwrap();
    let floor_id: ObjectId = id_of(&floor, "floor").parse().unwrap();
    let stored = |id: ObjectId| {
        let o = spatial::get(&env.snapshot(), id).unwrap();
        let registry = env.snapshot().registry().clone();
        let at = registry
            .get(&spatial::type_of(&env.snapshot(), id).unwrap())
            .unwrap()
            .schema
            .fields
            .iter()
            .position(|f| f.name == "work_plane")
            .unwrap();
        o.values.get(at).cloned().flatten()
    };
    assert_eq!(
        stored(floor_id),
        Some(arc_doc::Value::Reference(wp.parse().unwrap()))
    );
    // A plane that is not a work plane is refused.
    assert!(
        env.run(
            "archbuilder.floor.create",
            json!({"points": [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 5.0]],
                   "storey": storey, "work_plane": gid}),
        )
        .is_err()
    );
    // A roof by extrusion takes the plane's line and follows it when the grid moves.
    let roof = env
        .run(
            "archbuilder.roof_extrusion.create",
            json!({"work_plane": wp, "profile": [[0.0, 2.5], [10.0, 3.5]], "end": 6.0,
                   "storey": storey}),
        )
        .unwrap();
    let roof_id: ObjectId = id_of(&roof, "roof").parse().unwrap();
    let origin = |env: &Env| {
        let s = env.snapshot();
        archbuilder_model::roof_extrusion::Derived::of(&s, &spatial::get(&s, roof_id).unwrap())
            .unwrap()
            .roof
            .origin
    };
    assert_eq!(origin(&env), [0.0, 0.0]);
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [gid], "displacement": [0.0, 2.0]}),
    )
    .unwrap();
    assert_eq!(origin(&env), [0.0, 2.0]);
    // Moved by hand, the roof keeps its place and leaves the plane.
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [roof_id.to_string()], "displacement": [1.0, 0.0]}),
    )
    .unwrap();
    assert_eq!(origin(&env), [1.0, 2.0]);
    assert_eq!(stored(roof_id), None);
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [gid], "displacement": [0.0, 5.0]}),
    )
    .unwrap();
    assert_eq!(origin(&env), [1.0, 2.0]);
    env.document.undo().wait().unwrap();
    env.document.undo().wait().unwrap();
    assert_eq!(
        stored(roof_id),
        Some(arc_doc::Value::Reference(wp.parse().unwrap()))
    );
    assert_eq!(origin(&env), [0.0, 2.0]);
    // Without a plane or a work plane there is nothing to extrude along.
    assert!(
        env.run(
            "archbuilder.roof_extrusion.create",
            json!({"profile": [[0.0, 2.5], [10.0, 3.5]], "end": 6.0}),
        )
        .is_err()
    );
    // Deleting the plane clears the references; the roof is back at the plane it was sketched
    // with (its stored one), as the record holds authored data only.
    delete(&env, wp.parse().unwrap());
    assert_eq!(stored(roof_id), None);
    assert_eq!(stored(floor_id), None);
    assert_eq!(origin(&env), [0.0, 0.0]);
}

fn run_ok(env: &Env, command: &str, params: Json) -> Outcome {
    env.run(command, params).unwrap()
}

#[test]
fn the_project_location_changes_readings_and_never_moves_an_element() {
    let env = Env::new();
    let storey = env.storeys()[0].to_string();
    let wall = run_ok(
        &env,
        "archbuilder.wall.create",
        json!({"start": [10.0, 0.0], "end": [20.0, 0.0], "storey": storey}),
    );
    let wall_id: ObjectId = id_of(&wall, "wall").parse().unwrap();
    let wall_line = |env: &Env| {
        let w = archbuilder_model::wall::Wall::from_values(
            &spatial::get(&env.snapshot(), wall_id).unwrap().values,
        )
        .unwrap();
        (w.start, w.end)
    };
    // Without a location both points are the origin and nothing is georeferenced.
    let read = run_ok(
        &env,
        "archbuilder.coordinates.query",
        json!({"point": [12.0, 3.0]}),
    );
    assert_eq!(read.value["project"], json!([12.0, 3.0, 0.0]));
    assert!(read.value["map"].is_null());
    // The base point moves the project coordinates; the wall stays.
    run_ok(
        &env,
        "archbuilder.coordinates.set",
        json!({"base_point": [10.0, 0.0, 1.0], "survey_point": [100.0, 50.0]}),
    );
    assert_eq!(wall_line(&env), ([10.0, 0.0], [20.0, 0.0]));
    let read = run_ok(
        &env,
        "archbuilder.coordinates.query",
        json!({"point": [12.0, 3.0, 2.0]}),
    );
    assert_eq!(read.value["internal"], json!([12.0, 3.0, 2.0]));
    assert_eq!(read.value["project"], json!([2.0, 3.0, 1.0]));
    assert_eq!(read.value["survey"], json!([-88.0, -47.0, 2.0]));
    // The geodetic reference: the survey point at E 500000 N 6800000 H 40, north turned 90 degrees.
    let set = run_ok(
        &env,
        "archbuilder.coordinates.set",
        json!({"crs": "EPSG:3067", "eastings": 500000.0, "northings": 6800000.0,
               "height": 40.0, "true_north": "90 deg", "scale": 1.0}),
    );
    assert_eq!(set.value["georeferenced"], true);
    let read = run_ok(
        &env,
        "archbuilder.coordinates.query",
        json!({"point": [110.0, 50.0, 1.0]}),
    );
    let map = read.value["map"].as_array().unwrap();
    assert!((map[0].as_f64().unwrap() - 500_000.0).abs() < 1e-6);
    assert!((map[1].as_f64().unwrap() - (6_800_000.0 - 10.0)).abs() < 1e-6);
    assert_eq!(read.value["crs"], "EPSG:3067");
    assert_eq!(wall_line(&env), ([10.0, 0.0], [20.0, 0.0]));
    // Undo takes the georeference off; the previous points stay.
    env.document.undo().wait().unwrap();
    let read = run_ok(
        &env,
        "archbuilder.coordinates.query",
        json!({"point": [110.0, 50.0]}),
    );
    assert!(read.value["map"].is_null());
    assert_eq!(read.value["survey"], json!([10.0, 0.0, 0.0]));
    // Clearing the georeference leaves the points.
    run_ok(
        &env,
        "archbuilder.coordinates.set",
        json!({"crs": "EPSG:3067", "eastings": 1.0, "northings": 2.0}),
    );
    run_ok(
        &env,
        "archbuilder.coordinates.set",
        json!({"clear_georeference": true}),
    );
    let c = archbuilder_model::coordinates::Coordinates::of(&env.snapshot());
    assert!(!c.is_georeferenced() && c.crs.is_none() && c.survey == [100.0, 50.0, 0.0]);
    // What is out of range or malformed is refused and changes nothing.
    for bad in [
        json!({"scale": 0.0}),
        json!({"base_point": [1.0]}),
        json!({"base_point": [1.0e9, 0.0]}),
        json!({"survey_point": [0.0, 0.0, f64::INFINITY]}),
        json!({"eastings": 1.0e12}),
        json!({"true_north": "far away"}),
    ] {
        assert!(
            env.run("archbuilder.coordinates.set", bad.clone()).is_err(),
            "{bad}"
        );
    }
    assert_eq!(
        archbuilder_model::coordinates::Coordinates::of(&env.snapshot()),
        c
    );
    // With nothing to change the project is selected for its properties.
    let selected = run_ok(&env, "archbuilder.coordinates.set", json!({}));
    assert!(selected.selection.is_some());
}

#[test]
fn a_view_orients_to_true_north_without_moving_anything() {
    let env = Env::new();
    run_ok(
        &env,
        "archbuilder.coordinates.set",
        json!({"true_north": "30 deg"}),
    );
    let storey = env.storeys()[0].to_string();
    let view = run_ok(&env, "archbuilder.view.duplicate", json!({"view": storey}));
    let view: ObjectId = id_of(&view, "view").parse().unwrap();
    assert_eq!(
        view_settings(&env, view).north,
        archbuilder_model::view::North::Project
    );
    run_ok(
        &env,
        "archbuilder.view.set",
        json!({"view": view.to_string(), "field": "north", "value": "true_north"}),
    );
    let north = view_settings(&env, view).north;
    assert_eq!(north, archbuilder_model::view::North::True);
    let c = archbuilder_model::coordinates::Coordinates::of(&env.snapshot());
    assert!((c.view_rotation(north) + 30.0_f64.to_radians()).abs() < 1e-9);
    // A duplicate of the view keeps its orientation; undo returns the project north.
    let copy = run_ok(
        &env,
        "archbuilder.view.duplicate",
        json!({"view": view.to_string()}),
    );
    let copy: ObjectId = id_of(&copy, "view").parse().unwrap();
    assert_eq!(view_settings(&env, copy).north, north);
    env.document.undo().wait().unwrap();
    env.document.undo().wait().unwrap();
    assert_eq!(
        view_settings(&env, view).north,
        archbuilder_model::view::North::Project
    );
}

#[test]
fn geometry_far_from_the_origin_is_reported_and_recentering_keeps_its_map_position() {
    let env = Env::new();
    let storey = env.storeys()[0].to_string();
    // Near the origin nothing is reported.
    run_ok(
        &env,
        "archbuilder.wall.create",
        json!({"start": [0.0, 0.0], "end": [10.0, 0.0], "storey": storey}),
    );
    assert!(distant(&env).is_none());
    let far = run_ok(
        &env,
        "archbuilder.wall.create",
        json!({"start": [30_000.0, 100.0], "end": [30_010.0, 100.0], "storey": storey}),
    );
    let far_id: ObjectId = id_of(&far, "wall").parse().unwrap();
    let (count, ids) = distant(&env).unwrap();
    assert_eq!(count, "1");
    assert_eq!(ids, vec![far_id]);
    // The survey point is at the far geometry; the model is moved so that the survey point lies on
    // the origin (the first wall then lies far the other way, as it was 30 km from the survey point).
    run_ok(
        &env,
        "archbuilder.coordinates.set",
        json!({"survey_point": [30_000.0, 100.0], "base_point": [29_990.0, 100.0],
               "crs": "EPSG:3067", "eastings": 500000.0, "northings": 6800000.0}),
    );
    let map_of = |env: &Env, p: [f64; 2]| {
        run_ok(env, "archbuilder.coordinates.query", json!({"point": p})).value["map"].clone()
    };
    let before = map_of(&env, [30_010.0, 100.0]);
    let moved = run_ok(&env, "archbuilder.coordinates.recenter", json!({}));
    assert_eq!(moved.value["offset"], json!([-30_000.0, -100.0]));
    assert!(moved.value["moved"].as_u64().unwrap() >= 2);
    let wall = archbuilder_model::wall::Wall::from_values(
        &spatial::get(&env.snapshot(), far_id).unwrap().values,
    )
    .unwrap();
    assert_eq!((wall.start, wall.end), ([0.0, 0.0], [10.0, 0.0]));
    let c = archbuilder_model::coordinates::Coordinates::of(&env.snapshot());
    assert_eq!((c.survey, c.base), ([0.0, 0.0, 0.0], [-10.0, 0.0, 0.0]));
    // The same wall end, now at (10, 0), has the same map coordinates.
    assert_eq!(map_of(&env, [10.0, 0.0]), before);
    assert_eq!(distant(&env).map(|d| d.1.len()), Some(1));
    assert!(!distant(&env).unwrap().1.contains(&far_id));
    // Undo puts the model and both points back, and the warning with them.
    env.document.undo().wait().unwrap();
    assert!(distant(&env).is_some());
    assert_eq!(map_of(&env, [30_010.0, 100.0]), before);
    // A larger precision distance silences the report.
    run_ok(
        &env,
        "archbuilder.coordinates.set",
        json!({"precision_distance": 50_000.0}),
    );
    assert!(distant(&env).is_none());
    assert!(
        env.run(
            "archbuilder.coordinates.set",
            json!({"precision_distance": 0.5})
        )
        .is_err()
    );
}

/// The count and elements of the far-geometry warning, when the register has one.
fn distant(env: &Env) -> Option<(String, Vec<ObjectId>)> {
    archbuilder_model::validate::register(&env.snapshot())
        .into_iter()
        .find(|w| w.error.code() == "ab.model.location_distant")
        .map(|w| {
            let count = match w.error.arguments().get("count") {
                Some(arc_base::error::Value::Text(t)) => t.clone(),
                other => format!("{other:?}"),
            };
            (count, w.objects)
        })
}
