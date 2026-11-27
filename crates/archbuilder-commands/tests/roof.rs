//! Roofs by footprint and walls attached to roofs: the surface of hip, gable and L-shaped roofs,
//! wall tops that follow a roof's underside, detaching, deleting and undo.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::roof::ArcKind;
use archbuilder_model::roof::Derived;
use archbuilder_model::spatial;
use archbuilder_model::types::ROOF;
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

    /// The 10 x 6 house of 0.2 m center-line walls; returns the wall ids, the wall at x = 0
    /// first.
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

    /// A 45° roof picked from the house's walls (exterior faces, 0.5 m overhang).
    fn roof(&self) -> ObjectId {
        let made = self
            .run(
                "archbuilder.roof.create",
                json!({"picks": [[5.0, 3.0]], "slope": std::f64::consts::FRAC_PI_4}),
            )
            .unwrap()
            .value;
        made["roof"].as_str().unwrap().parse().unwrap()
    }

    fn derived(&self, id: ObjectId) -> Derived {
        let snapshot = self.snapshot();
        Derived::of(&snapshot, &spatial::get(&snapshot, id).unwrap()).unwrap()
    }

    /// The edges of the roof along the y axis (the short ones).
    fn short_edges(&self, id: ObjectId) -> Vec<usize> {
        let f = self.derived(id).surface.footprint;
        (0..f.len())
            .filter(|&k| (f[k][0] - f[(k + 1) % f.len()][0]).abs() < 1e-9)
            .collect()
    }

    /// The highest point of the wall's solid.
    fn wall_top(&self, wall: ObjectId) -> f64 {
        let snapshot = self.snapshot();
        let object = spatial::get(&snapshot, wall).unwrap();
        archbuilder_model::insert::wall_solid(&snapshot, &object)
            .unwrap()
            .unwrap()
            .bounding_box()
            .max()
            .z
    }
}

fn lines(d: &Derived, kind: ArcKind) -> Vec<f64> {
    d.surface
        .lines()
        .iter()
        .filter(|l| l.kind == kind)
        .map(|l| (l.to[0] - l.from[0]).hypot(l.to[1] - l.from[1]))
        .collect()
}

#[test]
fn hip_roof_on_picked_walls_has_its_ridge_and_heights() {
    let env = Env::new();
    env.house();
    let roof = env.roof();
    let d = env.derived(roof);
    // The outline is the exterior faces (10.2 x 6.2) plus 0.5 m: 11.2 x 7.2.
    assert!((d.surface.plan_area() - 11.2 * 7.2).abs() < 1e-6);
    let ridge = lines(&d, ArcKind::Ridge);
    assert_eq!(ridge.len(), 1);
    assert!((ridge[0] - 4.0).abs() < 1e-6, "{ridge:?}");
    assert_eq!(lines(&d, ArcKind::Hip).len(), 4);
    // The base is at the wall line on the walls' top (3 m); the ridge 3.1 m up at 45° (half of
    // the 6.2 m footprint), plus the 300 mm roof square to the slope.
    let vertical = 0.3 * 2.0_f64.sqrt();
    assert!((d.surface.top() - (3.0 + 3.1 + vertical)).abs() < 1e-6);
    // The overhang drops below the base: the eaves are 0.5 m lower.
    let eave = d.surface.underside([5.0, -0.1 - 0.5 + 1e-6]).unwrap();
    assert!((eave - 2.5).abs() < 1e-5, "{eave}");
    // Undo removes the roof, redo brings it back.
    env.document.undo().wait().unwrap();
    assert!(spatial::objects(&env.snapshot(), ROOF).is_empty());
    env.document.redo().wait().unwrap();
    assert_eq!(spatial::objects(&env.snapshot(), ROOF).len(), 1);
}

#[test]
fn gable_edges_give_vertical_gables_and_a_full_ridge() {
    let env = Env::new();
    env.house();
    let roof = env.roof();
    let gables = env.short_edges(roof);
    assert_eq!(gables.len(), 2);
    env.run(
        "archbuilder.roof.set_edge",
        json!({"roof": roof, "edges": gables, "sloped": false}),
    )
    .unwrap();
    let d = env.derived(roof);
    assert_eq!(lines(&d, ArcKind::Hip).len(), 0);
    let ridge = lines(&d, ArcKind::Ridge);
    assert!((ridge.iter().sum::<f64>() - 11.2).abs() < 1e-6, "{ridge:?}");
    // The cells of a gable edge are vertical: every corner on the edge's line.
    let outline = &d.surface.outline;
    for &k in &gables {
        let x = outline[k][0];
        let cells: Vec<_> = d
            .surface
            .skeleton
            .cells
            .iter()
            .filter(|c| c.edge == k)
            .collect();
        assert!(!cells.is_empty());
        assert!(
            cells
                .iter()
                .all(|c| c.points.iter().all(|p| (p.x - x).abs() < 1e-9))
        );
    }
    // Sloping again keeps the slope it had.
    env.run(
        "archbuilder.roof.set_edge",
        json!({"roof": roof, "edges": gables, "sloped": true}),
    )
    .unwrap();
    assert_eq!(lines(&env.derived(roof), ArcKind::Hip).len(), 4);
}

#[test]
fn an_l_shaped_footprint_has_a_valley_and_bad_input_is_refused() {
    let env = Env::new();
    let made = env
        .run(
            "archbuilder.roof.create",
            json!({"points": [[0, 0], [10, 0], [10, 4], [4, 4], [4, 10], [0, 10]], "overhang": 0.0}),
        )
        .unwrap()
        .value;
    let roof: ObjectId = made["roof"].as_str().unwrap().parse().unwrap();
    let d = env.derived(roof);
    let valleys = lines(&d, ArcKind::Valley);
    assert_eq!(valleys.len(), 1);
    // A crossing footprint and a roof without a slope are refused with what to change.
    let bow = env.run(
        "archbuilder.roof.create",
        json!({"points": [[0, 0], [10, 6], [10, 0], [0, 6]]}),
    );
    assert_eq!(bow.unwrap_err().code(), "ab.model.roof_footprint");
    let flat = env.run(
        "archbuilder.roof.set_edge",
        json!({"roof": roof, "sloped": false}),
    );
    assert_eq!(flat.unwrap_err().code(), "ab.model.roof_no_slope");
    // An inward corner next to a gable has no continuous roof.
    let gable = env.run(
        "archbuilder.roof.set_edge",
        json!({"roof": roof, "edges": [2], "sloped": false}),
    );
    assert_eq!(gable.unwrap_err().code(), "ab.model.roof_unclosed");
}

#[test]
fn an_attached_wall_follows_the_gable_and_a_slope_change() {
    let env = Env::new();
    let walls = env.house();
    let roof = env.roof();
    let gables = env.short_edges(roof);
    env.run(
        "archbuilder.roof.set_edge",
        json!({"roof": roof, "edges": gables, "sloped": false}),
    )
    .unwrap();
    assert!((env.wall_top(walls[0]) - 3.0).abs() < 1e-9);
    let attached = env
        .run("archbuilder.wall.attach_top", json!({"walls": [walls[0]]}))
        .unwrap()
        .value;
    assert_eq!(attached["not_following"], 0);
    // Under the ridge of a 6.2 m wide 45° gable the underside is 3.1 m above the wall line.
    assert!(
        (env.wall_top(walls[0]) - 6.1).abs() < 1e-6,
        "{}",
        env.wall_top(walls[0])
    );
    // A new slope moves the wall's top in the same step.
    env.run(
        "archbuilder.roof.set_edge",
        json!({"roof": roof, "slope": std::f64::consts::FRAC_PI_6}),
    )
    .unwrap();
    let want = 3.0 + 3.1 * std::f64::consts::FRAC_PI_6.tan();
    assert!((env.wall_top(walls[0]) - want).abs() < 1e-6);
    env.document.undo().wait().unwrap();
    assert!((env.wall_top(walls[0]) - 6.1).abs() < 1e-6);
}

#[test]
fn detaching_restores_the_previous_top() {
    let env = Env::new();
    let walls = env.house();
    env.roof();
    let before = env.wall_top(walls[1]);
    env.run("archbuilder.wall.attach_top", json!({"walls": [walls[1]]}))
        .unwrap();
    assert!(env.wall_top(walls[1]) > before + 0.1);
    env.run("archbuilder.wall.detach_top", json!({"walls": [walls[1]]}))
        .unwrap();
    assert!((env.wall_top(walls[1]) - before).abs() < 1e-9);
}

#[test]
fn deleting_a_roof_detaches_its_walls_and_undo_reattaches_them() {
    let env = Env::new();
    let walls = env.house();
    let roof = env.roof();
    env.run("archbuilder.wall.attach_top", json!({"walls": walls}))
        .unwrap();
    let invocation = Invocation {
        seed: 7,
        selection: vec![roof],
        ..Invocation::new("archbuilder.object.delete", json!({}))
    };
    let deleted = Invoker::new(&env.registry)
        .invoke(&env.document, &env.session, invocation)
        .unwrap()
        .value;
    assert_eq!(deleted["roofs"], 1);
    assert_eq!(deleted["detached_walls"], 4);
    assert!((env.wall_top(walls[0]) - 3.0).abs() < 1e-9);
    env.document.undo().wait().unwrap();
    assert!(env.wall_top(walls[0]) > 3.1);
}

#[test]
fn moving_a_picked_wall_moves_the_roof_and_a_vertex_handle_frees_its_edges() {
    let env = Env::new();
    let walls = env.house();
    let roof = env.roof();
    let area = env.derived(roof).surface.plan_area();
    // The wall at x = 10 moves 1 m out: the footprint follows it (the roof is derived again).
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [walls[2]], "displacement": [1.0, 0.0]}),
    )
    .unwrap();
    let moved = env.derived(roof).surface.plan_area();
    assert!(moved > area + 1.0, "{area} {moved}");
    let d = env.derived(roof);
    let first = d.surface.footprint[0];
    env.run(
        "archbuilder.roof.move_vertex",
        json!({"roof": roof, "vertex": 0, "to": [first[0] - 1.0, first[1] - 1.0]}),
    )
    .unwrap();
    let r = archbuilder_model::roof::Roof::from_values(
        &spatial::get(&env.snapshot(), roof).unwrap().values,
    )
    .unwrap();
    let n = r.edges.len();
    assert!(r.edges[0].vertex.edge.is_none() && r.edges[n - 1].vertex.edge.is_none());
    assert!(r.edges[1].vertex.edge.is_some());
}

#[test]
fn a_wall_attached_to_the_floor_above_stops_at_its_underside_and_top_offset_moves_a_level_top() {
    let env = Env::new();
    let walls = env.house();
    let snapshot = env.snapshot();
    let storeys = spatial::storeys(&snapshot, spatial::buildings(&snapshot)[0].id);
    let made = env
        .run(
            "archbuilder.floor.create",
            json!({"storey": storeys[1].storey.id, "points": [[-1, -1], [11, -1], [11, 7], [-1, 7]]}),
        )
        .unwrap()
        .value;
    let floor: ObjectId = made["floor"].as_str().unwrap().parse().unwrap();
    // The wall's top follows the level above (3 m); an offset of -0.25 m lowers it.
    env.run(
        "archbuilder.property.set",
        json!({"objects": [walls[0]], "field": "top_offset", "value": -0.25}),
    )
    .unwrap();
    assert!((env.wall_top(walls[0]) - 2.75).abs() < 1e-9);
    // Attached to the 200 mm floor of the storey above, it stops at the floor's underside.
    env.run(
        "archbuilder.wall.attach_top",
        json!({"walls": [walls[0]], "target": floor}),
    )
    .unwrap();
    assert!(
        (env.wall_top(walls[0]) - 2.8).abs() < 1e-9,
        "{}",
        env.wall_top(walls[0])
    );
    env.run("archbuilder.wall.detach_top", json!({"walls": [walls[0]]}))
        .unwrap();
    assert!((env.wall_top(walls[0]) - 2.75).abs() < 1e-9);
}

#[test]
fn slopes_are_typed_in_the_project_unit_and_refused_outside_the_range() {
    let env = Env::new();
    env.house();
    let roof = env.roof();
    let project = spatial::project(&env.snapshot()).unwrap().id;
    env.run(
        "archbuilder.property.set",
        json!({"objects": [project], "field": "slope_unit", "value": "percent"}),
    )
    .unwrap();
    env.run(
        "archbuilder.property.set",
        json!({"objects": [roof], "field": "roof_slope", "value": "50"}),
    )
    .unwrap();
    let r = archbuilder_model::roof::Roof::from_values(
        &spatial::get(&env.snapshot(), roof).unwrap().values,
    )
    .unwrap();
    assert!((r.edges[0].slope.unwrap() - 0.5_f64.atan()).abs() < 1e-12);
    for text in ["1:2", "45%", "20°"] {
        env.run(
            "archbuilder.property.set",
            json!({"objects": [roof], "field": "roof_slope", "value": text}),
        )
        .unwrap();
    }
    let error = env
        .run(
            "archbuilder.property.set",
            json!({"objects": [roof], "field": "roof_slope", "value": "89°"}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "ab.cmd.slope_invalid");
    assert!(error.arguments().contains_key("high"));
}

#[test]
fn the_roof_type_cuts_the_eaves_of_its_roofs_and_undo_restores_them() {
    let env = Env::new();
    env.house();
    let roof = env.roof();
    let volume = |env: &Env| env.derived(roof).solid().unwrap().volume();
    let plumb = volume(&env);
    let snapshot = env.snapshot();
    let ty = spatial::objects(&snapshot, archbuilder_model::types::ROOF_TYPE)[0].id;
    // A type that never set the optional fields cuts plumb.
    let before = env.derived(roof).surface.cut;
    assert_eq!(before.cut, archbuilder_model::roof::RafterCut::Plumb);

    env.run(
        "archbuilder.type.set",
        json!({"type": ty, "rafter_cut": "square"}),
    )
    .unwrap();
    let square = volume(&env);
    assert!(square < plumb - 1e-3, "{square} against {plumb}");
    // The top and the outline stay; only the underside end moves in.
    let d = env.derived(roof);
    assert!((d.surface.plan_area() - 11.2 * 7.2).abs() < 1e-6);
    let solid = d.solid().unwrap();
    assert!(solid.bounding_box().min().x >= -0.6 - 1e-9);
    assert!(solid.bounding_box().max().x <= 10.6 + 1e-9);

    env.run(
        "archbuilder.type.set",
        json!({"type": ty, "rafter_cut": "two_cut_plumb", "fascia_depth": "0.1"}),
    )
    .unwrap();
    let two = volume(&env);
    assert!(two < plumb - 1e-3 && (two - square).abs() > 1e-4);

    // Undo each step, then redo one.
    env.document.undo().wait().unwrap();
    assert!((volume(&env) - square).abs() < 1e-9);
    env.document.undo().wait().unwrap();
    assert!((volume(&env) - plumb).abs() < 1e-9);
    env.document.redo().wait().unwrap();
    assert!((volume(&env) - square).abs() < 1e-9);

    // A depth outside its range, and the fields on a type of another kind, are refused.
    assert!(
        env.run(
            "archbuilder.type.set",
            json!({"type": ty, "fascia_depth": "0.001"}),
        )
        .is_err()
    );
    let wall_type = spatial::objects(&env.snapshot(), archbuilder_model::types::WALL_TYPE)[0].id;
    assert!(
        env.run(
            "archbuilder.type.set",
            json!({"type": wall_type, "rafter_cut": "square"}),
        )
        .is_err()
    );
}
