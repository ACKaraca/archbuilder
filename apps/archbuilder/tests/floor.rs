//! The floor tool and the extraction of a floor, headless: a click inside the closed loop of the
//! house's walls makes the floor on the interior faces; the sketch mode with typed points, the
//! thickness keyword and Enter makes a free floor; a click that picks nothing leaves the tool
//! waiting; the floor's mesh follows a moved wall.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::sync::Arc;

use arc_base::Cancel;
use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use arc_gpu::RenderItem;
use archbuilder::floor_tool::FloorTool;
use archbuilder::wall_tool::Env;
use archbuilder_model::floor::Derived;
use archbuilder_model::spatial;
use archbuilder_model::types::{FLOOR, WALL};
use archbuilder_model::units::LengthUnit;
use serde_json::json;

struct Rig {
    document: Document,
    registry: arc_command::Registry,
    session: Session,
    tool: FloorTool,
    seed: u64,
}

impl Rig {
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
        let mut session = Session::new(Surface::CommandLine, [Permission::Read, Permission::Edit]);
        session.units = vec![*LengthUnit::Meter.unit().unwrap()];
        let rig = Self {
            document,
            registry: archbuilder_commands::registry().unwrap(),
            session,
            tool: FloorTool::new(),
            seed: 1,
        };
        rig.invoke("archbuilder.project.new", json!({}));
        let c = [[0.0, 0.0], [0.0, 6.0], [10.0, 6.0], [10.0, 0.0]];
        for k in 0..4 {
            rig.invoke(
                "archbuilder.wall.create",
                json!({"start": c[k], "end": c[(k + 1) % 4]}),
            );
        }
        rig
    }

    fn invoke(&self, command: &str, params: serde_json::Value) {
        Invoker::new(&self.registry)
            .invoke(
                &self.document,
                &self.session,
                Invocation {
                    seed: 99,
                    ..Invocation::new(command, params)
                },
            )
            .unwrap();
    }

    fn step(
        &mut self,
        f: impl FnOnce(
            &mut FloorTool,
            &Env<'_>,
            &mut dyn FnMut(Invocation) -> arc_base::Result<arc_command::Outcome>,
        ) -> arc_base::Result<()>,
    ) {
        self.seed += 1;
        let invoker = Invoker::new(&self.registry);
        let env = Env {
            registry: &self.registry,
            document: &self.document,
            session: &self.session,
            selection: &[],
            seed: self.seed,
        };
        let (document, session) = (&self.document, &self.session);
        f(&mut self.tool, &env, &mut |inv| {
            invoker.invoke(document, session, inv)
        })
        .unwrap();
    }

    fn floors(&self) -> Vec<Derived> {
        let snapshot = self.document.snapshot();
        spatial::objects(&snapshot, FLOOR)
            .iter()
            .map(|f| Derived::of(&snapshot, f).unwrap())
            .collect()
    }
}

#[test]
fn a_click_inside_the_walls_makes_the_floor_and_a_click_outside_waits() {
    let mut rig = Rig::new();
    rig.step(|t, env, run| t.begin(env, run, None));
    assert!(rig.tool.running());
    rig.step(|t, env, run| t.click(env, run, [50.0, 50.0]));
    assert!(rig.floors().is_empty());
    assert!(rig.tool.take_error().is_some() && rig.tool.running());
    rig.step(|t, env, run| t.click(env, run, [5.0, 3.0]));
    let floors = rig.floors();
    assert_eq!(floors.len(), 1);
    assert!((floors[0].area() - 9.8 * 5.8).abs() < 1e-9);
    // The tool asks for the next floor.
    assert!(rig.tool.running());
    rig.step(|t, _, _| {
        t.stop();
        Ok(())
    });
    assert!(!rig.tool.running());
}

#[test]
fn walls_picked_one_by_one_and_a_sketch_with_typed_points() {
    let mut rig = Rig::new();
    rig.step(|t, env, run| t.begin(env, run, None));
    // The face is chosen before picking: the exterior face.
    rig.step(|t, env, run| t.text(env, run, "exterior"));
    for at in [[0.0, 3.0], [5.0, 6.0], [10.0, 3.0], [5.0, 0.0]] {
        rig.step(|t, env, run| t.click(env, run, at));
    }
    rig.step(|t, env, run| t.text(env, run, ""));
    assert!((rig.floors()[0].area() - 10.2 * 6.2).abs() < 1e-9);
    // Sketch mode, thickness 0.3, four typed points, Enter.
    rig.step(|t, env, run| t.text(env, run, "sketch"));
    rig.step(|t, env, run| t.text(env, run, "thickness"));
    rig.step(|t, env, run| t.text(env, run, "0.3"));
    for p in ["0,0", "4,0", "4,3", "0,3"] {
        rig.step(|t, env, run| t.text(env, run, p));
    }
    rig.step(|t, env, run| t.text(env, run, ""));
    let floors = rig.floors();
    assert_eq!(floors.len(), 2);
    let sketched = floors.iter().find(|f| f.floor.walls().is_empty()).unwrap();
    assert!((sketched.area() - 12.0).abs() < 1e-9);
    assert!((sketched.floor.thickness - 0.3).abs() < 1e-12);
}

#[test]
fn the_floor_mesh_is_the_slab_and_follows_a_moved_wall() {
    let rig = Rig::new();
    rig.invoke("archbuilder.floor.create", json!({"picks": [[5.0, 3.0]]}));
    let mesh_height = |rig: &Rig| {
        let snapshot = rig.document.snapshot();
        let floor = spatial::objects(&snapshot, FLOOR)[0].clone();
        let extraction =
            archbuilder::extract_floor::extract_floor(&snapshot, &floor, &Cancel::default())
                .unwrap();
        assert!(matches!(extraction.items[0], RenderItem::Mesh(_)));
        let bytes = extraction.geometries.len();
        assert_eq!(bytes, 1);
        Derived::of(&snapshot, &floor).unwrap().area()
    };
    assert!((mesh_height(&rig) - 9.8 * 5.8).abs() < 1e-9);
    let right = spatial::objects(&rig.document.snapshot(), WALL)
        .into_iter()
        .find(|w| {
            archbuilder_model::wall::Wall::from_values(&w.values)
                .unwrap()
                .start
                == [10.0, 6.0]
        })
        .unwrap();
    for field in ["start_x", "end_x"] {
        rig.invoke(
            "archbuilder.property.set",
            json!({"objects": [right.id], "field": field, "value": 11.0}),
        );
    }
    assert!((mesh_height(&rig) - 10.8 * 5.8).abs() < 1e-9);
}

#[test]
fn a_hole_is_sketched_in_the_selected_floor() {
    let mut rig = Rig::new();
    rig.invoke("archbuilder.floor.create", json!({"picks": [[5.0, 3.0]]}));
    let floor = spatial::objects(&rig.document.snapshot(), FLOOR)[0].id;
    rig.step(|t, env, run| t.begin(env, run, Some(floor)));
    for p in ["3,2", "5,2", "5,4", "3,4"] {
        rig.step(|t, env, run| t.text(env, run, p));
    }
    rig.step(|t, env, run| t.text(env, run, ""));
    assert!(!rig.tool.running());
    assert!((rig.floors()[0].area() - (9.8 * 5.8 - 4.0)).abs() < 1e-9);
}

#[test]
fn a_refused_boundary_keeps_the_sketch_and_the_start_point_closes_it() {
    let mut rig = Rig::new();
    rig.step(|t, env, run| t.begin(env, run, None));
    rig.step(|t, env, run| t.text(env, run, "sketch"));
    // A bow tie: the finish is refused, the four points stay.
    for p in ["0,0", "4,4", "4,0", "0,4"] {
        rig.step(|t, env, run| t.text(env, run, p));
    }
    rig.step(|t, env, run| t.text(env, run, ""));
    assert!(rig.tool.take_error().is_some());
    assert!(rig.tool.running() && rig.floors().is_empty());
    assert_eq!(rig.tool.progress().1.len(), 4);
    // Undo the last two points, then fix the sketch: a click on the start closes it.
    rig.step(|t, env, run| t.text(env, run, "undo"));
    rig.step(|t, env, run| t.text(env, run, "undo"));
    assert_eq!(rig.tool.progress().1.len(), 2);
    rig.step(|t, env, run| t.text(env, run, "4,3"));
    rig.step(|t, env, run| t.click(env, run, [0.0, 0.0]));
    assert_eq!(rig.floors().len(), 1);
    assert!((rig.floors()[0].area() - 2.0).abs() < 1e-9);
}

#[test]
fn escape_drops_the_sketch_first_and_ends_the_tool_second() {
    let mut rig = Rig::new();
    rig.step(|t, env, run| t.begin(env, run, None));
    rig.step(|t, env, run| t.text(env, run, "sketch"));
    rig.step(|t, env, run| t.text(env, run, "1,1"));
    rig.step(|t, env, run| t.text(env, run, "2,1"));
    rig.step(|t, env, run| t.escape(env, run));
    assert!(rig.tool.running() && rig.tool.progress().1.is_empty());
    rig.step(|t, env, run| t.escape(env, run));
    assert!(!rig.tool.running());
}

#[test]
fn typed_picks_of_a_split_wall_house_make_the_exterior_floor() {
    let mut rig = Rig::new();
    // The rig's house has four walls; split the bottom one by adding a fifth over half of it.
    let split = spatial::objects(&rig.document.snapshot(), WALL)
        .into_iter()
        .find(|w| {
            archbuilder_model::wall::Wall::from_values(&w.values)
                .unwrap()
                .start
                == [10.0, 0.0]
        })
        .unwrap();
    rig.invoke(
        "archbuilder.property.set",
        json!({"objects": [split.id], "field": "end_x", "value": 5.0}),
    );
    rig.invoke(
        "archbuilder.wall.create",
        json!({"start": [5.0, 0.0], "end": [0.0, 0.0]}),
    );
    rig.step(|t, env, run| t.begin(env, run, None));
    rig.step(|t, env, run| t.text(env, run, "exterior"));
    for p in ["0,3", "5,6", "10,3", "7.5,0", "2.5,0"] {
        rig.step(|t, env, run| t.text(env, run, p));
    }
    rig.step(|t, env, run| t.text(env, run, ""));
    assert!((rig.floors()[0].area() - 10.2 * 6.2).abs() < 1e-9);
}

#[test]
fn raising_the_level_of_a_storey_raises_its_floor() {
    let rig = Rig::new();
    let snapshot = rig.document.snapshot();
    let building = spatial::buildings(&snapshot)[0].id;
    let storey = spatial::storeys(&snapshot, building)[1].clone();
    rig.invoke(
        "archbuilder.floor.create",
        json!({"storey": storey.storey.id,
               "points": [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]]}),
    );
    let top = |rig: &Rig| {
        let snapshot = rig.document.snapshot();
        Derived::of(&snapshot, &spatial::objects(&snapshot, FLOOR)[0])
            .unwrap()
            .top
    };
    assert!((top(&rig) - 3.0).abs() < 1e-9);
    rig.invoke(
        "archbuilder.level.set_elevation",
        json!({"level": storey.storey.id, "elevation": "5 m"}),
    );
    assert!((top(&rig) - 5.0).abs() < 1e-9);
}
