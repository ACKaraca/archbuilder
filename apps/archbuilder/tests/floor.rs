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
fn escape_cancels_the_band_first_drops_the_sketch_second_and_ends_the_tool_third() {
    let mut rig = Rig::new();
    rig.step(|t, env, run| t.begin(env, run, None));
    rig.step(|t, env, run| t.text(env, run, "sketch"));
    rig.step(|t, env, run| t.text(env, run, "1,1"));
    rig.step(|t, env, run| t.text(env, run, "2,1"));
    // The first Escape cancels the segment that follows the pointer and keeps both points.
    rig.step(|t, env, run| t.escape(env, run));
    assert!(rig.tool.running() && rig.tool.band_off());
    assert_eq!(rig.tool.progress().1, [[1.0, 1.0], [2.0, 1.0]]);
    // The sketch stays editable: undo removes the last point, a new point brings the band back.
    rig.step(|t, env, run| t.text(env, run, "undo"));
    assert_eq!(rig.tool.progress().1, [[1.0, 1.0]]);
    assert!(!rig.tool.band_off());
    rig.step(|t, env, run| t.text(env, run, "3,1"));
    rig.step(|t, env, run| t.escape(env, run));
    rig.step(|t, env, run| t.text(env, run, "3,2"));
    assert_eq!(rig.tool.progress().1.len(), 3);
    assert!(!rig.tool.band_off());
    // The second Escape (after the first again) discards the sketch and stays in the tool.
    rig.step(|t, env, run| t.escape(env, run));
    rig.step(|t, env, run| t.escape(env, run));
    assert!(rig.tool.running() && rig.tool.progress().1.is_empty());
    assert!(rig.floors().is_empty());
    // The third leaves the tool.
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

#[test]
fn a_type_picked_while_the_floor_tool_runs_makes_the_next_floor() {
    use archbuilder_model::element_type::{self as et, TypeKind};
    let mut rig = Rig::new();
    rig.step(|t, env, run| t.begin(env, run, None));
    // The type selector picks the slab while the prompt waits: the prompt starts again with it.
    let slab = et::of_kind(&rig.document.snapshot(), TypeKind::Floor)
        .into_iter()
        .find(|t| et::name(t).starts_with("Concrete slab"))
        .unwrap()
        .id;
    archbuilder::tool_type::choose(TypeKind::Floor, slab);
    rig.step(|t, env, run| t.retype(env, run));
    rig.step(|t, env, run| t.text(env, run, "sketch"));
    for p in ["0,0", "4,0", "4,3", "0,3"] {
        rig.step(|t, env, run| t.text(env, run, p));
    }
    rig.step(|t, env, run| t.text(env, run, ""));
    let floors = rig.floors();
    assert_eq!(floors.len(), 1);
    assert!((floors[0].floor.thickness - 0.26).abs() < 1e-9);
}

#[test]
fn the_sketch_moves_a_vertex_adds_an_arc_and_finishes_as_one_step_or_cancels() {
    use archbuilder::floor_sketch::{FloorSketch, Mode};
    let rig = Rig::new();
    rig.invoke("archbuilder.floor.create", json!({"picks": [[5.0, 3.0]]}));
    let snapshot = rig.document.snapshot();
    let floor = spatial::objects(&snapshot, FLOOR)[0].clone();
    let area = Derived::of(&snapshot, &floor).unwrap().area();
    let walls = archbuilder_model::floor::storey_walls(&snapshot, floor.header.owner.unwrap());
    let no_wall = |_: [f64; 2]| None;
    let mut sketch = FloorSketch::begin(&snapshot, floor.id).unwrap();
    // Drag the vertex at the corner (0.1, 0.1) of the interior faces out: it frees two edges.
    let corner = sketch.sketch.vertices[0].point;
    assert!(sketch.press(corner, 100.0));
    assert!(sketch.moved([corner[0] - 1.0, corner[1] - 1.0]));
    assert!(sketch.release());
    assert_eq!(sketch.take_freed(), 2);
    // An arc on the first edge: click the edge, then a point beyond it.
    sketch.mode = Mode::Arc;
    let (a, b) = (
        sketch.sketch.vertices[0].point,
        sketch.sketch.vertices[1].point,
    );
    let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    sketch.click(mid, 100.0, &no_wall);
    sketch.click([mid[0] - 1.0, mid[1]], 100.0, &no_wall);
    assert!(sketch.sketch.vertices[0].bulge != 0.0);
    let of = |id| walls.iter().find(|(w, _)| *w == id).map(|(_, w)| *w);
    sketch.check(&of).unwrap();
    // Applying it is one command and one undo step; the floor then has an arc.
    let params = sketch.params();
    rig.invoke("archbuilder.floor.edit_boundary", params);
    let after = rig.document.snapshot();
    let derived = Derived::of(&after, &spatial::get(&after, floor.id).unwrap()).unwrap();
    assert!(derived.boundary.has_arcs());
    assert!((derived.area() - area).abs() > 0.1);
    rig.document.undo().wait().unwrap();
    let undone = rig.document.snapshot();
    let back = Derived::of(&undone, &spatial::get(&undone, floor.id).unwrap()).unwrap();
    assert!((back.area() - area).abs() < 1e-9 && !back.boundary.has_arcs());
    // An invalid loop is refused by the check; nothing was applied.
    let mut bad = FloorSketch::begin(&undone, floor.id).unwrap();
    bad.sketch.move_vertex(0, [5.0, -3.0]);
    assert!(bad.check(&of).is_err());
}

#[test]
fn a_floor_edge_on_the_exterior_face_is_cut_back_to_the_core_and_does_not_meet_the_wall() {
    use archbuilder_model::cleanup;
    let rig = Rig::new();
    // A floor on the exterior faces of the walls, at the level of the walls' bases and with its
    // top raised so that it reaches into them.
    rig.invoke(
        "archbuilder.floor.create",
        json!({"picks": [[5.0, 3.0]], "face": "exterior", "offset": 0.1}),
    );
    let snapshot = rig.document.snapshot();
    let floor = spatial::objects(&snapshot, FLOOR)[0].clone();
    let derived = Derived::of(&snapshot, &floor).unwrap();
    let cut = cleanup::floor_solid(&snapshot, &floor).unwrap().unwrap();
    // The reported area is that of the authored boundary; the body has lost the walls' cores.
    assert!(
        (derived.area() - 10.2 * 6.2).abs() < 1e-6,
        "{}",
        derived.area()
    );
    assert!(cut.volume() < derived.volume() - 1e-6);
    // No wall solid enters the slab body.
    for wall in spatial::objects(&snapshot, WALL) {
        let pieces = cleanup::wall_pieces(&snapshot, &wall).unwrap().unwrap();
        let bounds = cut.bounding_box();
        for p in pieces {
            let b = p.solid.bounding_box();
            let inside_z = b.min().z < bounds.max().z - 1e-6 && b.max().z > bounds.min().z + 1e-6;
            if !inside_z {
                continue;
            }
            // Where it shares the slab's heights, its plan lies outside the slab's plan.
            let c = b.min();
            let probe = [(c.x + b.max().x) / 2.0, (c.y + b.max().y) / 2.0];
            let flat = cleanup::clean_floor(&snapshot, &floor).unwrap();
            let in_plan = flat.flat.iter().any(|poly| {
                let boundary = archbuilder_model::floor::Boundary::polygons(vec![poly.clone()]);
                boundary.contains(probe)
            });
            assert!(!in_plan, "a wall piece stands in the slab at {probe:?}");
        }
    }
    let _ = Cancel::default();
    let _ = RenderItem::Mesh;
}

#[test]
fn a_changed_slab_redraws_every_wall_and_a_changed_wall_every_slab() {
    use arc_doc::events::{ChangeGroups, Changes};
    use arc_doc::schema::TypeKey;
    let rig = Rig::new();
    let snapshot = rig.document.snapshot();
    let upper = spatial::storeys(&snapshot, spatial::buildings(&snapshot)[0].id)[1]
        .storey
        .id;
    rig.invoke(
        "archbuilder.floor.create",
        json!({"storey": upper, "points": [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]]}),
    );
    let snapshot = rig.document.snapshot();
    let slab = spatial::objects(&snapshot, FLOOR)[0].id;
    let walls: Vec<_> = spatial::objects(&snapshot, WALL)
        .iter()
        .map(|w| w.id)
        .collect();
    let listed = |key: &str, group: ChangeGroups| {
        Changes::Listed([(TypeKey::new(key).unwrap(), group)].into())
    };
    // The slab changed: the walls below, on another storey, whose layers stop at it extract again.
    let slab_changed = listed(
        FLOOR,
        ChangeGroups {
            modified: [slab].into(),
            ..ChangeGroups::default()
        },
    );
    let redrawn = archbuilder::extract_wall::dependents(&snapshot, &slab_changed);
    assert!(walls.iter().all(|w| redrawn.contains(w)));
    // A wall changed: the slab on the storey above extracts again.
    let wall_changed = listed(
        WALL,
        ChangeGroups {
            modified: [walls[0]].into(),
            ..ChangeGroups::default()
        },
    );
    assert!(archbuilder::extract_floor::dependents(&snapshot, &wall_changed).contains(&slab));
}

#[test]
fn the_sketch_modes_pick_edges_add_and_delete_vertices_and_preview_the_fill() {
    use arc_ui::ProductOverlay;
    use archbuilder::floor_sketch::{FloorSketch, Hint, Mode, Sel};
    let rig = Rig::new();
    rig.invoke(
        "archbuilder.floor.create",
        json!({"points": [[20.0, 0.0], [24.0, 0.0], [24.0, 4.0], [20.0, 4.0]]}),
    );
    let snapshot = rig.document.snapshot();
    let floor = spatial::objects(&snapshot, FLOOR)[0].id;
    let none = |_: [f64; 2]| None;
    let mut s = FloorSketch::begin(&snapshot, floor).unwrap();
    // Pick-wall mode: a click on an edge selects it, and a click on a wall with no edge says so.
    s.mode = Mode::Wall;
    s.click([22.0, 0.0], 100.0, &none);
    assert_eq!(s.selected, Some(Sel::Edge(0)));
    s.selected = None;
    let walls = archbuilder_model::floor::storey_walls(
        &snapshot,
        spatial::get(&snapshot, floor)
            .unwrap()
            .header
            .owner
            .unwrap(),
    );
    let on_wall = |_: [f64; 2]| walls.first().copied();
    s.click([2.0, 2.0], 100.0, &on_wall);
    assert_eq!(s.take_hint(), Some(Hint::PickEdgeFirst));
    // Add mode puts a vertex on an edge and then leaves, so the next press drags.
    s.mode = Mode::Add;
    s.click([22.0, 0.0], 100.0, &none);
    assert_eq!(s.sketch.vertices.len(), 5);
    assert_eq!(s.mode, Mode::Select);
    assert!(s.press([22.0, 0.0], 100.0));
    assert!(s.release());
    // Delete mode removes the vertex that is clicked; a loop keeps three.
    s.mode = Mode::Delete;
    s.click([22.0, 0.0], 100.0, &none);
    assert_eq!(s.sketch.vertices.len(), 4);
    s.click([24.0, 0.0], 100.0, &none);
    assert_eq!(s.sketch.vertices.len(), 3);
    s.click([24.0, 4.0], 100.0, &none);
    assert_eq!(s.sketch.vertices.len(), 3);
    assert_eq!(s.take_hint(), Some(Hint::LoopTooSmall));
    // The preview fill follows an arc: more points than the straight outline.
    let mut a = FloorSketch::begin(&snapshot, floor).unwrap();
    let fill_points = |sk: &FloorSketch| {
        sk.overlays(100.0, true)
            .iter()
            .find_map(|o| match o {
                ProductOverlay::Styled { item, .. } => match item.as_ref() {
                    ProductOverlay::Fill { rings } => Some(rings[0].len()),
                    _ => None,
                },
                _ => None,
            })
            .unwrap()
    };
    let straight = fill_points(&a);
    a.sketch.set_arc_through(0, [22.0, -1.5]);
    assert!(fill_points(&a) > straight + 5);
}
