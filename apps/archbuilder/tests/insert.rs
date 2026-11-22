//! The insert tool and the application's handling of doors and windows, headless: inserts typed
//! and clicked in a row (one undo step each), the pointer's snapping along a wall, a refused click
//! that leaves the chain going, the shaded extraction of an insert (the glass apart from the frame),
//! the walls and inserts that a change makes extract again, and the messages in both catalogs.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::sync::Arc;

use arc_base::{Cancel, ObjectId};
use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use arc_gpu::RenderItem;
use archbuilder::insert_tool::{InsertTool, hover};
use archbuilder::wall_tool::Env;
use archbuilder_model::insert::Kind;
use archbuilder_model::spatial;
use archbuilder_model::types::{DOOR, WALL, WINDOW};
use archbuilder_model::units::LengthUnit;
use serde_json::json;

struct Rig {
    document: Document,
    registry: arc_command::Registry,
    session: Session,
    tool: InsertTool,
    selection: Vec<ObjectId>,
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
        let mut rig = Self {
            document,
            registry: archbuilder_commands::registry().unwrap(),
            session,
            tool: InsertTool::new(),
            selection: Vec::new(),
            seed: 1,
        };
        rig.invoke("archbuilder.project.new", json!({}));
        rig.invoke(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [10.0, 0.0], "height": 3.0}),
        );
        rig.selection = vec![rig.wall()];
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

    fn wall(&self) -> ObjectId {
        spatial::objects(&self.document.snapshot(), WALL)[0].id
    }

    fn step(
        &mut self,
        f: impl FnOnce(
            &mut InsertTool,
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
            selection: &self.selection,
            seed: self.seed,
        };
        let (document, session) = (&self.document, &self.session);
        f(&mut self.tool, &env, &mut |inv| {
            invoker.invoke(document, session, inv)
        })
        .unwrap();
    }

    fn count(&self, key: &str) -> usize {
        spatial::objects(&self.document.snapshot(), key).len()
    }
}

#[test]
fn inserts_typed_and_clicked_in_a_row_are_one_undo_step_each_and_a_refusal_keeps_the_chain() {
    let mut rig = Rig::new();
    let wall = rig.wall();
    rig.step(|t, env, run| t.begin(env, run, Kind::Door));
    assert!(rig.tool.running());
    // Typed distances along the selected wall.
    rig.step(|t, env, run| t.text(env, run, "2", None));
    rig.step(|t, env, run| t.text(env, run, "4.5", None));
    assert_eq!(rig.count(DOOR), 2);
    // A click at 4.9 m overlaps the door at 4.5 m: refused, the error is kept, the chain goes on.
    rig.step(|t, env, run| t.click(env, run, wall, 4.9, false));
    assert_eq!(rig.count(DOOR), 2);
    assert!(rig.tool.take_error().is_some());
    assert!(rig.tool.running());
    rig.step(|t, env, run| t.click(env, run, wall, 8.0, false));
    assert_eq!(rig.count(DOOR), 3);
    // Options set on the way carry along the chain: a wider door, hand flipped.
    rig.step(|t, env, run| t.text(env, run, "width", None));
    rig.step(|t, env, run| t.text(env, run, "1.1", None));
    rig.step(|t, env, run| t.text(env, run, "flip_hand", None));
    rig.step(|t, env, run| t.text(env, run, "6.2", None));
    let snapshot = rig.document.snapshot();
    let last = archbuilder_model::insert::hosted(&snapshot, wall)
        .into_iter()
        .find(|(_, i)| i.distance == 6.2)
        .unwrap()
        .1;
    assert_eq!(last.width, 1.1);
    assert_eq!(last.hand, archbuilder_model::insert::Hand::Right);
    // Enter ends the chain; the doors stay. One undo per door.
    rig.step(|t, env, run| t.text(env, run, "", None));
    assert!(!rig.tool.running());
    assert_eq!(rig.count(DOOR), 4);
    for left in [3, 2, 1, 0] {
        assert!(rig.document.undo().wait().unwrap().value);
        assert_eq!(rig.count(DOOR), left);
    }
}

#[test]
fn a_distance_typed_with_the_pointer_on_another_wall_goes_to_that_wall() {
    let mut rig = Rig::new();
    rig.invoke(
        "archbuilder.wall.create",
        json!({"start": [0.0, 5.0], "end": [6.0, 5.0], "height": 3.0}),
    );
    let first = rig.wall();
    let other = spatial::objects(&rig.document.snapshot(), WALL)
        .iter()
        .map(|w| w.id)
        .find(|id| *id != first)
        .unwrap();
    rig.step(|t, env, run| t.begin(env, run, Kind::Window));
    rig.step(|t, env, run| t.text(env, run, "3", Some(other)));
    let snapshot = rig.document.snapshot();
    assert_eq!(spatial::children(&snapshot, WINDOW, other).len(), 1);
    assert!(spatial::children(&snapshot, WINDOW, first).is_empty());
}

#[test]
fn the_pointer_snaps_the_center_to_five_centimeters_and_stays_in_the_clear_span() {
    let rig = Rig::new();
    let snapshot = rig.document.snapshot();
    let storey = spatial::lowest_storey(&snapshot).unwrap().id;
    let at = |x: f64, y: f64| hover(&snapshot, storey, [x, y], None, Some(100.0), 0.9);
    let found = at(3.12, 0.05).unwrap();
    assert!((found.distance - 3.10).abs() < 1e-9);
    assert_eq!(found.wall, rig.wall());
    // Near the start the door keeps clear of the end: 0.45 + 0.01 at least.
    assert!((at(0.1, 0.0).unwrap().distance - 0.46).abs() < 1e-9);
    assert!((at(9.99, 0.0).unwrap().distance - 9.54).abs() < 1e-9);
    // Away from the wall nothing is under the pointer.
    assert!(at(3.0, 1.0).is_none());
    // A view ray that enters the wall's front face picks it where the face is, whatever point it
    // has on the level plane: here the plane point is 4 m behind the wall.
    let ray = Some(([3.12, -5.0, 1.5], [0.0, 1.0, 0.0]));
    let by_ray = hover(&snapshot, storey, [3.12, 4.0], ray, Some(100.0), 0.9).unwrap();
    assert!((by_ray.distance - 3.10).abs() < 1e-9);
    // A ray over the wall's top misses nothing it should not: it passes above, no wall.
    let above = Some(([3.12, -5.0, 3.5], [0.0, 1.0, 0.0]));
    assert!(hover(&snapshot, storey, [3.12, 4.0], above, Some(100.0), 0.9).is_none());
}

#[test]
fn a_door_and_a_window_extract_with_a_leaf_a_pane_apart_in_color_and_the_wall_has_the_opening() {
    let rig = Rig::new();
    let wall = rig.wall();
    rig.invoke(
        "archbuilder.door.place",
        json!({"wall": wall, "distance": 2.0}),
    );
    rig.invoke(
        "archbuilder.window.place",
        json!({"wall": wall, "distance": 6.0}),
    );
    let snapshot = rig.document.snapshot();
    let cancel = Cancel::default();
    let door = spatial::objects(&snapshot, DOOR).remove(0);
    let window = spatial::objects(&snapshot, WINDOW).remove(0);
    let colors = |object: &arc_doc::Object| -> Vec<[f32; 3]> {
        let extraction =
            archbuilder::extract_insert::extract_insert(&snapshot, object, &cancel).unwrap();
        extraction
            .items
            .iter()
            .map(|item| match item {
                RenderItem::Mesh(m) => {
                    let c = m.instances[0].color;
                    [c.r, c.g, c.b]
                }
                _ => panic!("a mesh"),
            })
            .collect()
    };
    // Four frame bars, a leaf and a handle; four bars and a pane.
    assert_eq!(colors(&door).len(), 6);
    let glass = colors(&window);
    assert_eq!(glass.len(), 5);
    assert_ne!(glass[4], glass[0], "the glass differs from the frame");
    let wall_object = spatial::get(&snapshot, wall).unwrap();
    let extraction =
        archbuilder::extract_wall::extract_wall(&snapshot, &wall_object, &cancel).unwrap();
    assert_eq!(extraction.items.len(), 1);
}

#[test]
fn a_changed_insert_extracts_its_wall_and_a_changed_wall_its_inserts_again() {
    use arc_doc::events::{ChangeGroups, Changes};
    use arc_doc::schema::TypeKey;
    let rig = Rig::new();
    let wall = rig.wall();
    rig.invoke(
        "archbuilder.door.place",
        json!({"wall": wall, "distance": 2.0}),
    );
    let snapshot = rig.document.snapshot();
    let door = spatial::objects(&snapshot, DOOR)[0].id;
    let listed = |key: &str, group: ChangeGroups| {
        Changes::Listed([(TypeKey::new(key).unwrap(), group)].into())
    };
    let sorted = |mut v: Vec<ObjectId>| {
        v.sort();
        v
    };
    let mut both = vec![wall, door];
    both.sort();
    let dependents = archbuilder::extract_wall::dependents;
    let door_changed = listed(
        DOOR,
        ChangeGroups {
            modified: [door].into(),
            ..ChangeGroups::default()
        },
    );
    assert_eq!(sorted(dependents(&snapshot, &door_changed)), both);
    let wall_changed = listed(
        WALL,
        ChangeGroups {
            modified: [wall].into(),
            ..ChangeGroups::default()
        },
    );
    assert_eq!(sorted(dependents(&snapshot, &wall_changed)), both);
    // A deleted insert extracted once names its wall, which is no longer cut there.
    let cancel = Cancel::default();
    archbuilder::extract_insert::extract_insert(
        &snapshot,
        &spatial::get(&snapshot, door).unwrap(),
        &cancel,
    )
    .unwrap();
    Invoker::new(&rig.registry)
        .invoke(
            &rig.document,
            &rig.session,
            Invocation {
                seed: 5,
                selection: vec![door],
                ..Invocation::new("archbuilder.object.delete", json!({}))
            },
        )
        .unwrap();
    let door_deleted = listed(
        DOOR,
        ChangeGroups {
            deleted: [door].into(),
            ..ChangeGroups::default()
        },
    );
    assert_eq!(dependents(&rig.document.snapshot(), &door_deleted), [wall]);
}

#[test]
fn every_message_of_the_insert_tool_is_in_both_catalogs() {
    let ids = [
        "ab-tool-door",
        "ab-tool-window",
        "ab-kind-door",
        "ab-prop-hand",
        "ab-prop-facing",
        "ab-prop-frame-width",
        "ab-prop-sill",
        "ab-hand-left",
        "ab-hand-right",
        "ab-notice-inserts-deleted",
        "ab-prompt-door-place",
        "ab-prompt-window-place",
        "ab-keyword-insert-width",
        "ab-keyword-insert-hand",
        "ab-keyword-insert-facing",
        "ab-cmd-door-place-label",
        "ab-cmd-insert-flip-hand-label",
        "ab-model-insert-outside",
        "ab-model-insert-overlap",
        "ab-cmd-no-wall-selected",
    ];
    for language in ["en-US", "tr-TR"] {
        let localizer = archbuilder::localizer(&[language.to_owned()]).unwrap();
        for id in ids {
            let text = localizer.format(id, &[]);
            assert_eq!(
                text.locale.as_ref().map(ToString::to_string).as_deref(),
                Some(language),
                "{id} in {language}"
            );
        }
    }
}

#[test]
fn a_door_is_picked_in_plan_and_in_3d_ahead_of_its_wall_and_a_wall_in_front_hides_it() {
    use archbuilder_model::insert::pick;
    let rig = Rig::new();
    let wall = rig.wall();
    rig.invoke(
        "archbuilder.door.place",
        json!({"wall": wall, "distance": 2.0}),
    );
    let snapshot = rig.document.snapshot();
    let door = spatial::objects(&snapshot, DOOR)[0].id;
    // From above at the door's center: the wall's top is in the way, the door is still picked.
    assert_eq!(
        pick(&snapshot, ([2.0, 0.0, 20.0], [0.0, 0.0, -1.0])),
        Some(door)
    );
    // From above, beside the door, nothing is picked: the wall is left to the viewport.
    assert_eq!(pick(&snapshot, ([5.0, 0.0, 20.0], [0.0, 0.0, -1.0])), None);
    // From the front, through the opening, and obliquely from above.
    assert_eq!(
        pick(&snapshot, ([2.0, -5.0, 1.0], [0.0, 1.0, 0.0])),
        Some(door)
    );
    assert_eq!(
        pick(&snapshot, ([2.0, -5.0, 6.0], [0.0, 1.0, -1.0])),
        Some(door)
    );
    // Above the door's head the ray meets only the wall.
    assert_eq!(pick(&snapshot, ([2.0, -5.0, 2.6], [0.0, 1.0, 0.0])), None);
    // Another wall in front of the opening hides the door.
    rig.invoke(
        "archbuilder.wall.create",
        json!({"start": [0.0, -3.0], "end": [10.0, -3.0], "height": 3.0}),
    );
    assert_eq!(
        pick(
            &rig.document.snapshot(),
            ([2.0, -8.0, 1.0], [0.0, 1.0, 0.0])
        ),
        None
    );
}

#[test]
fn a_click_decides_the_facing_by_the_side_of_the_pointer_and_keeps_the_hinge_side() {
    let mut rig = Rig::new();
    let wall = rig.wall();
    rig.step(|t, env, run| t.begin(env, run, Kind::Door));
    rig.step(|t, env, run| t.click(env, run, wall, 2.0, false));
    rig.step(|t, env, run| t.click(env, run, wall, 5.0, true));
    // `flip hand` moves the hinge toward the start; the next click on the exterior side follows.
    rig.step(|t, env, run| t.text(env, run, "flip_hand", None));
    rig.step(|t, env, run| t.click(env, run, wall, 8.0, false));
    let snapshot = rig.document.snapshot();
    let read = |d: f64| {
        archbuilder_model::insert::hosted(&snapshot, wall)
            .into_iter()
            .find(|(_, i)| i.distance == d)
            .unwrap()
            .1
    };
    use archbuilder_model::insert::{Facing, Hand};
    // Hinge toward the end seen from the exterior is the left hand; from the interior, the right.
    assert_eq!(
        (read(2.0).facing, read(2.0).hand),
        (Facing::Exterior, Hand::Left)
    );
    assert_eq!(
        (read(5.0).facing, read(5.0).hand),
        (Facing::Interior, Hand::Right)
    );
    assert_eq!(
        (read(8.0).facing, read(8.0).hand),
        (Facing::Exterior, Hand::Right)
    );
    // Every hinge of the first two is toward the end, the third toward the start.
    assert_eq!(read(2.0).hinge_direction(), 1.0);
    assert_eq!(read(5.0).hinge_direction(), 1.0);
    assert_eq!(read(8.0).hinge_direction(), -1.0);
}

#[test]
fn the_plan_symbol_has_the_leaf_open_on_the_facing_side_and_the_swing_on_the_hand_side() {
    use archbuilder_model::insert::{Facing, Hand, Insert, Kind, Operation, symbol};
    use archbuilder_model::wall::{JoinStyle, LocationLine, Wall};
    let wall = Wall {
        start: [0.0, 0.0],
        end: [10.0, 0.0],
        thickness: 0.2,
        core: [0.0, 0.0],
        height: 3.0,
        base_offset: 0.0,
        location: LocationLine::Center,
        start_join: JoinStyle::Auto,
        end_join: JoinStyle::Auto,
    };
    let door = Insert {
        kind: Kind::Door,
        distance: 5.0,
        width: 0.9,
        height: 2.1,
        sill: 0.0,
        frame_width: 0.05,
        frame_depth: 0.1,
        operation: Operation::SingleSwing,
        hand: Hand::Left,
        facing: Facing::Exterior,
    };
    let lines = symbol(&wall, 0.0, &door).unwrap();
    // The outline, the leaf and the arc.
    assert_eq!(lines.len(), 3);
    // Left hand, exterior: the hinge is toward the end (x = 5.4), the leaf runs 0.8 m out from the
    // exterior face (y = 0.1) to the left of the baseline; the arc ends there and starts at the
    // closed position on the face.
    let (leaf, arc) = (&lines[1], &lines[2]);
    assert!((leaf[0][0] - 5.4).abs() < 1e-9 && (leaf[0][1] - 0.1).abs() < 1e-9);
    assert!((leaf[1][1] - 0.9).abs() < 1e-9);
    assert!((arc[0][0] - 4.6).abs() < 1e-9 && (arc[0][1] - 0.1).abs() < 1e-9);
    let end = arc.last().unwrap();
    assert!((end[0] - 5.4).abs() < 1e-9 && (end[1] - 0.9).abs() < 1e-9);
    // Interior facing swings to the other side of the wall.
    let inside = Insert {
        facing: Facing::Interior,
        ..door
    };
    assert!(symbol(&wall, 0.0, &inside).unwrap()[1][1][1] < 0.0);
    // A window: outline, frame outline and the glass line along the wall's center.
    let window = Insert {
        kind: Kind::Window,
        operation: Operation::Fixed,
        sill: 0.9,
        ..door
    };
    let lines = symbol(&wall, 0.0, &window).unwrap();
    assert_eq!(lines.len(), 3);
    assert!((lines[2][0][1]).abs() < 1e-9 && (lines[2][1][1]).abs() < 1e-9);
}

#[test]
fn options_work_before_any_wall_is_chosen_and_survive_a_refused_placement() {
    let mut rig = Rig::new();
    let wall = rig.wall();
    rig.selection.clear();
    rig.step(|t, env, run| t.begin(env, run, Kind::Door));
    // Width, with nothing selected and nothing hovered: the value is taken, the placement prompt
    // is back, and the flips are keywords too.
    rig.step(|t, env, run| t.text(env, run, "width", None));
    rig.step(|t, env, run| t.text(env, run, "1.0", None));
    assert!(rig.tool.take_error().is_none());
    rig.step(|t, env, run| t.text(env, run, "flip_hand", None));
    rig.step(|t, env, run| t.text(env, run, "flip_facing", None));
    // A distance with no wall says so and leaves the placement prompt.
    rig.step(|t, env, run| t.text(env, run, "3", None));
    assert!(rig.tool.take_error().is_some());
    assert_eq!(rig.count(DOOR), 0);
    // A click on a wall is a 1.0 m door.
    rig.step(|t, env, run| t.click(env, run, wall, 5.0, false));
    let snapshot = rig.document.snapshot();
    let door = archbuilder_model::insert::hosted(&snapshot, wall)
        .remove(0)
        .1;
    assert_eq!(door.width, 1.0);
    // A refused placement (overlap) leaves the options as they were, for the next one.
    rig.step(|t, env, run| t.text(env, run, "5.2", Some(wall)));
    assert!(rig.tool.take_error().is_some());
    rig.step(|t, env, run| t.text(env, run, "8", Some(wall)));
    let snapshot = rig.document.snapshot();
    let second = archbuilder_model::insert::hosted(&snapshot, wall)
        .into_iter()
        .find(|(_, i)| i.distance == 8.0)
        .unwrap()
        .1;
    assert_eq!(second.width, 1.0);
}
