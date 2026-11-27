//! Model views other than plans: saved 3D views (camera, section box), sections and elevations,
//! their drawings, their markers in plan views and undo.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::{Cancel, ObjectId};
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use arc_view::VisibilityClass;
use archbuilder_model::model_view::{ModelView, ViewKind};
use archbuilder_model::plan::{self, Filter};
use archbuilder_model::spatial;
use archbuilder_model::types::MODEL_VIEW;
use serde_json::{Value as Json, json};

static SEEDS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(19_000);

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

    fn id(&self, command: &str, params: Json, key: &str) -> ObjectId {
        self.run(command, params).unwrap().value[key]
            .as_str()
            .unwrap()
            .parse()
            .unwrap()
    }

    fn settings(&self, id: ObjectId) -> ModelView {
        archbuilder_model::model_view::settings(&self.document.snapshot(), id).unwrap()
    }

    fn undo(&self) {
        assert!(self.document.undo().wait().unwrap().value);
    }

    /// A closed room of four walls, 6 by 4 meters, on the ground storey.
    fn room(&self) -> Vec<ObjectId> {
        let s = self.document.snapshot();
        let ground = spatial::storeys(&s, spatial::buildings(&s)[0].id)[0]
            .storey
            .id
            .to_string();
        [
            ([0.0, 0.0], [6.0, 0.0]),
            ([6.0, 0.0], [6.0, 4.0]),
            ([6.0, 4.0], [0.0, 4.0]),
            ([0.0, 4.0], [0.0, 0.0]),
        ]
        .into_iter()
        .map(|(a, b)| {
            self.id(
                "archbuilder.wall.create",
                json!({"start": a, "end": b, "storey": ground}),
                "wall",
            )
        })
        .collect()
    }
}

#[test]
fn a_3d_view_keeps_its_camera_and_section_box_with_undo() {
    let env = Env::new();
    env.room();
    let view = env.id(
        "archbuilder.view.create_3d",
        json!({"eye": [20.0, -15.0, 10.0], "target": [3.0, 2.0, 1.0], "perspective": false}),
        "view",
    );
    let made = env.settings(view);
    assert_eq!(made.kind, ViewKind::ThreeD);
    assert_eq!(made.name, "3D View 1");
    let camera = made.camera.unwrap();
    assert!(!camera.perspective && camera.eye == [20.0, -15.0, 10.0]);

    env.run(
        "archbuilder.view.set_camera",
        json!({"view": view.to_string(), "eye": [0.0, -30.0, 5.0], "target": [0.0, 0.0, 0.0]}),
    )
    .unwrap();
    assert_eq!(env.settings(view).camera.unwrap().eye, [0.0, -30.0, 5.0]);
    // A box switched on without corners takes the model's extent: the walls' outer faces.
    env.run(
        "archbuilder.view.set_section_box",
        json!({"view": view.to_string(), "on": true}),
    )
    .unwrap();
    let boxed = env.settings(view);
    let (lo, hi) = boxed.corners.unwrap();
    assert!(boxed.section_box);
    assert!(lo[0] < 0.0 && hi[0] > 6.0 && lo[1] < 0.0 && hi[1] > 4.0 && hi[2] > 2.0);
    // An inverted box is refused.
    assert!(
        env.run(
            "archbuilder.view.set_section_box",
            json!({"view": view.to_string(), "on": true, "min": [1.0, 1.0, 1.0], "max": [0.0, 2.0, 2.0]}),
        )
        .is_err()
    );
    env.undo();
    assert!(!env.settings(view).section_box);
    env.undo();
    assert_eq!(env.settings(view).camera.unwrap().eye, [20.0, -15.0, 10.0]);
    // A plan view's commands work on model views too.
    let copy = env.id(
        "archbuilder.view.duplicate",
        json!({"view": view.to_string()}),
        "view",
    );
    assert_eq!(env.settings(copy).name, "3D View 1 2");
    env.run("archbuilder.view.delete", json!({"view": copy.to_string()}))
        .unwrap();
    env.run("archbuilder.view.delete", json!({"view": view.to_string()}))
        .unwrap();
    assert!(spatial::objects(&env.document.snapshot(), MODEL_VIEW).is_empty());
}

#[test]
fn a_section_cuts_the_walls_it_crosses_and_shows_a_marker_in_plan() {
    let env = Env::new();
    let walls = env.room();
    // Across the room from south to north, looking west (left of the line).
    let section = env.id(
        "archbuilder.view.create_section",
        json!({"start": [3.0, -2.0], "end": [3.0, 6.0], "depth": 10.0}),
        "view",
    );
    let s = env.document.snapshot();
    let object = spatial::get(&s, section).unwrap();
    let drawing = plan::build_vertical(&s, &object, &Filter::default(), &Cancel::new()).unwrap();
    assert!(drawing.diagnostics.is_empty(), "{:?}", drawing.diagnostics);
    let cut: Vec<ObjectId> = drawing
        .links
        .iter()
        .filter(|l| l.class == VisibilityClass::Cut)
        .map(|l| l.source)
        .collect();
    // The south and north walls are cut; the west wall lies beyond; the east wall is behind the
    // viewer.
    assert!(
        cut.contains(&walls[0]) && cut.contains(&walls[2]),
        "{cut:?}"
    );
    assert!(
        drawing
            .links
            .iter()
            .any(|l| l.source == walls[3] && l.class != VisibilityClass::Cut)
    );
    assert!(drawing.links.iter().all(|l| l.source != walls[1]));
    // The drawing's y is the elevation: the walls stand from 0 up.
    let top = drawing
        .list
        .primitives
        .iter()
        .filter_map(|p| match &p.shape {
            arc_draft::display::Shape::Stroke { curve, .. } => curve.bounding_box(),
            _ => None,
        })
        .map(|b| b.max().y)
        .fold(f64::MIN, f64::max);
    assert!(top > 2.0 && top < 10.0, "{top}");
    // The plan view of the ground storey draws the section's marker.
    let ground = spatial::storeys(&s, spatial::buildings(&s)[0].id)[0]
        .storey
        .id;
    let view = archbuilder_model::view::of_storey(&s, ground)[0].clone();
    let built = plan::build(&s, &view, &Filter::default(), &Cancel::new()).unwrap();
    assert!(!built.primitives_of(section).is_empty());
    // A view hides a category: no wall is drawn.
    env.run(
        "archbuilder.view.set",
        json!({"view": section.to_string(), "field": "show_walls", "value": false}),
    )
    .unwrap();
    let s = env.document.snapshot();
    let object = spatial::get(&s, section).unwrap();
    let hidden = plan::build_vertical(&s, &object, &Filter::default(), &Cancel::new()).unwrap();
    assert!(hidden.links.iter().all(|l| !walls.contains(&l.source)));
    env.undo();
    env.undo();
    assert!(spatial::objects(&env.document.snapshot(), MODEL_VIEW).is_empty());
}

#[test]
fn a_building_gets_four_elevations_that_see_it_from_outside_in_one_step() {
    let env = Env::new();
    let walls = env.room();
    let made = env
        .run("archbuilder.view.create_elevations", json!({}))
        .unwrap();
    let views: Vec<ObjectId> = made.value["views"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().parse().unwrap())
        .collect();
    assert_eq!(views.len(), 4);
    let names: Vec<String> = views.iter().map(|v| env.settings(*v).name).collect();
    assert_eq!(
        names,
        [
            "North Elevation",
            "East Elevation",
            "South Elevation",
            "West Elevation"
        ]
    );
    let s = env.document.snapshot();
    // The south elevation sees the south wall, cuts nothing.
    let south = spatial::get(&s, views[2]).unwrap();
    let drawing = plan::build_vertical(&s, &south, &Filter::default(), &Cancel::new()).unwrap();
    assert!(
        drawing
            .links
            .iter()
            .all(|l| l.class != VisibilityClass::Cut)
    );
    assert!(drawing.links.iter().any(|l| l.source == walls[0]));
    env.undo();
    assert!(spatial::objects(&env.document.snapshot(), MODEL_VIEW).is_empty());
}
