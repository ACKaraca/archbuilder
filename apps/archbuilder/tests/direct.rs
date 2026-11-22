//! Direct manipulation, headless: the engine of `archbuilder::direct` driven with plan pointer
//! input against a real document. Each gesture is one command and so one undo step; a dragged
//! wall keeps its neighbors joined; Ctrl copies; an end handle moves one end; a door slides to
//! the edge of the clear span; the height handle and a typed dimension use the commands; Escape
//! leaves the document as it was; a press in empty space is left to the rubber band.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use arc_ui::DynamicEvent;
use archbuilder::direct::{Direct, Done, Frame, Input, parse_length};
use archbuilder_model::insert::{Insert, hosted};
use archbuilder_model::spatial;
use archbuilder_model::types::WALL;
use archbuilder_model::units::LengthUnit;
use archbuilder_model::wall::Wall;
use serde_json::{Value, json};

struct Rig {
    document: Document,
    registry: arc_command::Registry,
    session: Session,
}

const PPU: f64 = 100.0;

fn plan() -> Frame {
    Frame {
        ppu: PPU,
        right: [1.0, 0.0, 0.0],
        up: [0.0, 1.0, 0.0],
        forward: [0.0, 0.0, -1.0],
        eye: None,
        target_depth: 100.0,
        top: true,
    }
}

/// A view from the south, level: x to the right, z up.
fn side() -> Frame {
    Frame {
        right: [1.0, 0.0, 0.0],
        up: [0.0, 0.0, 1.0],
        forward: [0.0, 1.0, 0.0],
        top: false,
        ..plan()
    }
}

fn ray(frame: &Frame, p: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let f = frame.forward;
    (
        [
            p[0] - f[0] * 1.0e4,
            p[1] - f[1] * 1.0e4,
            p[2] - f[2] * 1.0e4,
        ],
        f,
    )
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
        let rig = Self {
            document,
            registry: archbuilder_commands::registry().unwrap(),
            session: Session::new(Surface::Ui, [Permission::Read, Permission::Edit]),
        };
        rig.run("archbuilder.project.new", json!({})).unwrap();
        rig
    }

    fn run(&self, command: &str, params: Value) -> arc_base::Result<arc_command::Outcome> {
        Invoker::new(&self.registry).invoke(
            &self.document,
            &self.session,
            Invocation {
                seed: 5,
                ..Invocation::new(command, params)
            },
        )
    }

    fn wall(&self, a: [f64; 2], b: [f64; 2]) -> ObjectId {
        self.run(
            "archbuilder.wall.create",
            json!({"start": a, "end": b, "height": 3.0}),
        )
        .unwrap();
        self.walls()
            .into_iter()
            .find(|(_, w)| w.start == a && w.end == b)
            .unwrap()
            .0
    }

    fn room(&self) -> Vec<ObjectId> {
        [
            ([0.0, 0.0], [6.0, 0.0]),
            ([6.0, 0.0], [6.0, 4.0]),
            ([6.0, 4.0], [0.0, 4.0]),
            ([0.0, 4.0], [0.0, 0.0]),
        ]
        .into_iter()
        .map(|(a, b)| self.wall(a, b))
        .collect()
    }

    fn walls(&self) -> Vec<(ObjectId, Wall)> {
        spatial::objects(&self.document.snapshot(), WALL)
            .iter()
            .filter_map(|o| Some((o.id, Wall::from_values(&o.values)?)))
            .collect()
    }

    fn wall_now(&self, id: ObjectId) -> Wall {
        self.walls().into_iter().find(|(i, _)| *i == id).unwrap().1
    }

    fn execute(&self, done: &Done) {
        for c in &done.commands {
            self.run(c.name, c.params.clone()).unwrap();
        }
    }

    fn undo(&self) {
        self.document.undo().wait().unwrap();
    }
}

struct Pointer {
    frame: Frame,
    selection: Vec<ObjectId>,
    ctrl: bool,
}

impl Pointer {
    fn plan(selection: &[ObjectId]) -> Self {
        Self {
            frame: plan(),
            selection: selection.to_vec(),
            ctrl: false,
        }
    }

    /// The input at the plan point `at`, `z` high, `dx` pixels right and `dy` pixels up of it.
    fn at<'a>(&'a self, rig: &Rig, snapshot: &'a Snapshot, at: [f64; 2], z: f64) -> Input<'a> {
        let _ = rig;
        let p = [at[0], at[1], z];
        Input {
            snapshot,
            selection: &self.selection,
            frame: self.frame,
            ray: Some(ray(&self.frame, p)),
            pixel: [
                at[0] * PPU + 800.0 * p[2].signum().abs() * 0.0,
                -at[1] * PPU,
            ],
            at: Some(at),
            shift: false,
            ctrl: self.ctrl,
            free: false,
            unit: LengthUnit::Meter,
            separator: '.',
        }
    }
}

/// Presses at `from`, moves in steps to `to` and releases.
fn drag(
    rig: &Rig,
    direct: &mut Direct,
    pointer: &Pointer,
    from: [f64; 2],
    to: [f64; 2],
) -> Option<Done> {
    let snapshot = rig.document.snapshot();
    assert!(
        direct.press(&pointer.at(rig, &snapshot, from, 1.0)),
        "press claimed"
    );
    for k in 1..=4 {
        let t = f64::from(k) / 4.0;
        let p = [
            from[0] + (to[0] - from[0]) * t,
            from[1] + (to[1] - from[1]) * t,
        ];
        assert!(direct.moved(&pointer.at(rig, &snapshot, p, 1.0)));
    }
    direct.release(&pointer.at(rig, &snapshot, to, 1.0))
}

#[test]
fn dragging_the_right_wall_two_meters_keeps_the_room_closed_in_one_undo_step() {
    let rig = Rig::new();
    let room = rig.room();
    let mut direct = Direct::new();
    let before = rig.walls().len();
    let done = drag(
        &rig,
        &mut direct,
        &Pointer::plan(&room[1..2]),
        [6.0, 2.0],
        [8.0, 2.0],
    )
    .unwrap();
    assert_eq!(done.commands.len(), 1);
    assert_eq!(done.commands[0].name, "archbuilder.modify.move");
    rig.execute(&done);
    assert_eq!(rig.wall_now(room[1]).start, [8.0, 0.0]);
    assert_eq!(
        rig.wall_now(room[0]).end,
        [8.0, 0.0],
        "the bottom wall followed"
    );
    assert_eq!(
        rig.wall_now(room[2]).start,
        [8.0, 4.0],
        "the top wall followed"
    );
    assert_eq!(rig.walls().len(), before);
    rig.undo();
    assert_eq!(rig.wall_now(room[1]).start, [6.0, 0.0]);
    assert_eq!(rig.wall_now(room[0]).end, [6.0, 0.0]);
}

#[test]
fn an_unselected_wall_is_selected_when_the_drag_starts_and_a_short_press_is_a_click() {
    let rig = Rig::new();
    let room = rig.room();
    let mut direct = Direct::new();
    let pointer = Pointer::plan(&[]);
    let snapshot = rig.document.snapshot();
    assert!(direct.press(&pointer.at(&rig, &snapshot, [3.0, 0.0], 1.0)));
    assert!(direct.moved(&pointer.at(&rig, &snapshot, [3.0, 0.01], 1.0)));
    assert_eq!(direct.take_select(), None, "not yet a drag");
    assert!(
        direct
            .release(&pointer.at(&rig, &snapshot, [3.0, 0.01], 1.0))
            .is_none()
    );
    assert!(!direct.take_swallow(), "the viewport's click selects");
    assert!(direct.press(&pointer.at(&rig, &snapshot, [3.0, 0.0], 1.0)));
    assert!(direct.moved(&pointer.at(&rig, &snapshot, [3.0, 1.0], 1.0)));
    assert_eq!(direct.take_select(), Some(room[0]));
}

#[test]
fn ctrl_at_release_copies() {
    let rig = Rig::new();
    let room = rig.room();
    let mut direct = Direct::new();
    let mut pointer = Pointer::plan(&room[0..1]);
    pointer.ctrl = true;
    let done = drag(&rig, &mut direct, &pointer, [3.0, 0.0], [3.0, -2.0]).unwrap();
    assert_eq!(done.commands[0].name, "archbuilder.modify.copy");
    rig.execute(&done);
    assert_eq!(rig.walls().len(), 5);
    assert_eq!(
        rig.wall_now(room[0]).start,
        [0.0, 0.0],
        "the original stays"
    );
}

#[test]
fn escape_during_a_drag_changes_nothing() {
    let rig = Rig::new();
    let room = rig.room();
    let mut direct = Direct::new();
    let pointer = Pointer::plan(&room[1..2]);
    let snapshot = rig.document.snapshot();
    assert!(direct.press(&pointer.at(&rig, &snapshot, [6.0, 2.0], 1.0)));
    assert!(direct.moved(&pointer.at(&rig, &snapshot, [8.0, 2.0], 1.0)));
    assert!(direct.dragging());
    assert!(direct.cancel());
    assert!(
        direct
            .release(&pointer.at(&rig, &snapshot, [8.0, 2.0], 1.0))
            .is_none()
    );
    assert_eq!(rig.wall_now(room[1]).start, [6.0, 0.0]);
}

#[test]
fn a_press_in_empty_space_is_left_to_the_rubber_band() {
    let rig = Rig::new();
    let room = rig.room();
    let mut direct = Direct::new();
    let pointer = Pointer::plan(&room[1..2]);
    let snapshot = rig.document.snapshot();
    assert!(!direct.press(&pointer.at(&rig, &snapshot, [3.0, 2.0], 1.0)));
    assert!(!direct.press(&pointer.at(&rig, &snapshot, [20.0, 20.0], 1.0)));
}

#[test]
fn dragging_an_end_handle_onto_another_wall_end_makes_a_new_corner() {
    let rig = Rig::new();
    let a = rig.wall([0.0, 0.0], [4.0, 0.0]);
    let b = rig.wall([6.0, 0.0], [6.0, 3.0]);
    let mut direct = Direct::new();
    let pointer = Pointer::plan(&[a]);
    let done = drag(&rig, &mut direct, &pointer, [4.0, 0.0], [6.02, 0.01]).unwrap();
    assert_eq!(done.commands[0].name, "archbuilder.wall.move_end");
    rig.execute(&done);
    assert_eq!(
        rig.wall_now(a).end,
        [6.0, 0.0],
        "snapped to the end of the other wall"
    );
    assert_eq!(rig.wall_now(a).start, [0.0, 0.0]);
    assert_eq!(rig.wall_now(b).start, [6.0, 0.0]);
    rig.undo();
    assert_eq!(rig.wall_now(a).end, [4.0, 0.0]);
}

#[test]
fn a_door_dragged_alone_slides_and_stops_at_the_end_of_the_clear_span() {
    let rig = Rig::new();
    let wall = rig.wall([0.0, 0.0], [10.0, 0.0]);
    rig.run(
        "archbuilder.door.place",
        json!({"wall": wall, "distance": 2.0}),
    )
    .unwrap();
    let door = hosted(&rig.document.snapshot(), wall)[0].0.id;
    let mut direct = Direct::new();
    let pointer = Pointer::plan(&[door]);
    let done = drag(&rig, &mut direct, &pointer, [2.0, 0.0], [30.0, 0.0]).unwrap();
    rig.execute(&done);
    let snapshot = rig.document.snapshot();
    let item = Insert::of(&snapshot, &spatial::get(&snapshot, door).unwrap()).unwrap();
    assert!(
        item.distance > 9.0 && item.extent().1 <= 10.0 - 0.01,
        "{}",
        item.distance
    );
    rig.undo();
    let snapshot = rig.document.snapshot();
    let item = Insert::of(&snapshot, &spatial::get(&snapshot, door).unwrap()).unwrap();
    assert!((item.distance - 2.0).abs() < 1e-9);
}

#[test]
fn the_top_handle_sets_the_height_in_a_view_from_the_side() {
    let rig = Rig::new();
    let wall = rig.wall([0.0, 0.0], [6.0, 0.0]);
    let mut direct = Direct::new();
    let pointer = Pointer {
        frame: side(),
        selection: vec![wall],
        ctrl: false,
    };
    let snapshot = rig.document.snapshot();
    let at = |z: f64| Input {
        pixel: [300.0, 800.0 - z * PPU],
        ray: Some(ray(&pointer.frame, [3.0, 0.0, z])),
        ..pointer.at(&rig, &snapshot, [3.0, 0.0], z)
    };
    assert!(direct.press(&at(3.0)));
    assert!(direct.moved(&at(3.5)));
    assert!(direct.moved(&at(4.02)));
    let done = direct.release(&at(4.02)).unwrap();
    assert_eq!(done.commands[0].name, "archbuilder.property.set");
    rig.execute(&done);
    assert!((rig.wall_now(wall).height - 4.0).abs() < 1e-9);
    rig.undo();
    assert!((rig.wall_now(wall).height - 3.0).abs() < 1e-9);
}

#[test]
fn a_typed_temporary_dimension_moves_the_wall_to_that_distance() {
    let rig = Rig::new();
    let w1 = rig.wall([0.0, 0.0], [6.0, 0.0]);
    let w2 = rig.wall([0.0, 4.0], [6.0, 4.0]);
    let _ = w1;
    let dims = archbuilder::direct::dims(&rig.document.snapshot(), &[w2]);
    assert_eq!(dims.len(), 1);
    assert!((dims[0].value() - 4.0).abs() < 1e-9);
    let mut direct = Direct::new();
    let pointer = Pointer::plan(&[w2]);
    let snapshot = rig.document.snapshot();
    // The value's plate lies up and to the right of the quarter of the dimension line, (3, 1).
    let click = |at: [f64; 2]| Input {
        pixel: [at[0] * PPU, -at[1] * PPU],
        ..pointer.at(&rig, &snapshot, at, 0.0)
    };
    assert!(direct.press(&click([3.1, 1.15])));
    assert!(direct.release(&click([3.1, 1.15])).is_none());
    assert!(direct.take_swallow());
    assert!(direct.input().is_some());
    assert!(
        direct
            .edited(&DynamicEvent::Edited {
                index: 0,
                text: "3".into()
            })
            .is_none()
    );
    let done = direct.edited(&DynamicEvent::Enter { index: 0 }).unwrap();
    rig.execute(&done);
    assert_eq!(rig.wall_now(w2).start, [0.0, 3.0]);
    assert!(direct.input().is_none());
    rig.undo();
    assert_eq!(rig.wall_now(w2).start, [0.0, 4.0]);
}

#[test]
fn a_wall_end_that_would_leave_a_door_outside_is_refused_naming_it() {
    let rig = Rig::new();
    let wall = rig.wall([0.0, 0.0], [10.0, 0.0]);
    rig.run(
        "archbuilder.door.place",
        json!({"wall": wall, "distance": 9.0}),
    )
    .unwrap();
    let error = rig
        .run(
            "archbuilder.wall.move_end",
            json!({"wall": wall.to_string(), "end": "end", "to": [5.0, 0.0]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "ab.model.insert_outside");
    assert_eq!(rig.wall_now(wall).end, [10.0, 0.0]);
    // Moving the start keeps the opening where it is along the wall, from the far end.
    rig.run(
        "archbuilder.wall.move_end",
        json!({"wall": wall.to_string(), "end": "start", "to": [2.0, 0.0]}),
    )
    .unwrap();
    let snapshot = rig.document.snapshot();
    let door = &hosted(&snapshot, wall)[0].1;
    assert!((door.distance - 7.0).abs() < 1e-9);
}

#[test]
fn lengths_are_read_in_the_project_unit_or_with_their_own() {
    assert_eq!(parse_length("3,5", LengthUnit::Meter), Some(3.5));
    assert_eq!(parse_length("350", LengthUnit::Centimeter), Some(3.5));
    assert_eq!(parse_length("3500 mm", LengthUnit::Meter), Some(3.5));
    assert_eq!(parse_length("abc", LengthUnit::Meter), None);
    assert_eq!(parse_length("2 parsec", LengthUnit::Meter), None);
}

fn floor_area(rig: &Rig) -> f64 {
    let snapshot = rig.document.snapshot();
    let floor = spatial::objects(&snapshot, archbuilder_model::types::FLOOR)
        .into_iter()
        .next()
        .unwrap();
    let boundary = archbuilder_model::floor::Floor::from_values(&floor.values)
        .and_then(|f| f.boundary_in(&snapshot))
        .expect("the floor has a boundary");
    boundary.check().expect("the boundary is valid");
    let ring = &boundary.loops[0];
    (0..ring.len())
        .map(|k| {
            let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
        .abs()
        / 2.0
}

#[test]
fn dragging_a_joined_end_keeps_the_corner_closed_and_the_floor_enclosed() {
    let rig = Rig::new();
    let room = rig.room();
    rig.run("archbuilder.floor.create", json!({"picks": [[3.0, 2.0]]}))
        .unwrap();
    let before = floor_area(&rig);
    let mut direct = Direct::new();
    let pointer = Pointer::plan(&room[0..1]);
    for target in [[5.0, 0.0], [7.0, 0.0]] {
        let done = drag(&rig, &mut direct, &pointer, [6.0, 0.0], target).unwrap();
        rig.execute(&done);
        assert_eq!(rig.wall_now(room[0]).end, target);
        assert_eq!(
            rig.wall_now(room[1]).start,
            target,
            "the joined wall's end came along"
        );
        let area = floor_area(&rig);
        assert!(
            if target[0] < 6.0 {
                area < before
            } else {
                area > before
            },
            "{area}"
        );
        rig.undo();
        assert_eq!(rig.wall_now(room[1]).start, [6.0, 0.0]);
        assert!((floor_area(&rig) - before).abs() < 1e-9);
    }
}

#[test]
fn a_stem_on_the_dragged_wall_follows_its_new_line_and_a_disallowed_end_moves_alone() {
    let rig = Rig::new();
    let wall = rig.wall([0.0, 0.0], [10.0, 0.0]);
    let stem = rig.wall([4.0, 0.0], [4.0, 3.0]);
    rig.run(
        "archbuilder.wall.move_end",
        json!({"wall": wall.to_string(), "end": "end", "to": [10.0, 2.0]}),
    )
    .unwrap();
    // The line through (0,0) and (10,2) passes (4, 0.8): the stem's end slid up onto it.
    let end = rig.wall_now(stem).start;
    assert!(
        (end[0] - 4.0).abs() < 1e-9 && (end[1] - 0.8).abs() < 1e-9,
        "{end:?}"
    );
    rig.undo();
    let a = rig.wall([0.0, 5.0], [4.0, 5.0]);
    let b = rig.wall([4.0, 5.0], [4.0, 8.0]);
    rig.run(
        "archbuilder.wall.disallow_join",
        json!({"walls": [a.to_string()], "end": "end"}),
    )
    .unwrap();
    rig.run(
        "archbuilder.wall.move_end",
        json!({"wall": a.to_string(), "end": "end", "to": [3.0, 5.0]}),
    )
    .unwrap();
    assert_eq!(
        rig.wall_now(b).start,
        [4.0, 5.0],
        "a disallowed join moves alone"
    );
}

#[test]
fn a_refused_end_move_tells_the_range_after_the_move() {
    let rig = Rig::new();
    let wall = rig.wall([0.0, 0.0], [10.0, 0.0]);
    rig.run(
        "archbuilder.door.place",
        json!({"wall": wall, "distance": 1.0}),
    )
    .unwrap();
    let error = rig
        .run(
            "archbuilder.wall.move_end",
            json!({"wall": wall.to_string(), "end": "start", "to": [5.0, 0.0]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "ab.model.insert_outside");
    let to = error
        .arguments()
        .get("to")
        .map(|v| format!("{v:?}"))
        .unwrap_or_default();
    assert!(to.contains("5.00"), "{to}");
}

#[test]
fn an_invalid_dimension_entry_keeps_the_input_open_and_is_reported() {
    let rig = Rig::new();
    let _w1 = rig.wall([0.0, 0.0], [6.0, 0.0]);
    let w2 = rig.wall([0.0, 4.0], [6.0, 4.0]);
    let mut direct = Direct::new();
    let pointer = Pointer::plan(&[w2]);
    let snapshot = rig.document.snapshot();
    let click = |at: [f64; 2]| Input {
        pixel: [at[0] * PPU, -at[1] * PPU],
        ..pointer.at(&rig, &snapshot, at, 0.0)
    };
    // The value stands a quarter of the way along the line from the other wall: (0.25 of 4 m) 1 m.
    assert!(direct.press(&click([3.1, 1.15])));
    assert!(direct.release(&click([3.1, 1.15])).is_none());
    let (_, field) = direct.input().expect("the input is open");
    assert!(field.select_all);
    direct.edited(&DynamicEvent::Edited {
        index: 0,
        text: "abc".into(),
    });
    assert!(direct.edited(&DynamicEvent::Enter { index: 0 }).is_none());
    assert!(direct.take_invalid());
    assert!(direct.input().is_some());
}

#[test]
fn alt_turns_snapping_off_while_dragging() {
    let rig = Rig::new();
    let a = rig.wall([0.0, 0.0], [4.0, 0.0]);
    let _b = rig.wall([6.0, 0.0], [6.0, 3.0]);
    let mut direct = Direct::new();
    let pointer = Pointer::plan(&[a]);
    let snapshot = rig.document.snapshot();
    let at = |p: [f64; 2]| Input {
        free: true,
        ..pointer.at(&rig, &snapshot, p, 1.0)
    };
    assert!(direct.press(&at([4.0, 0.0])));
    assert!(direct.moved(&at([6.013, 0.007])));
    let done = direct.release(&at([6.013, 0.007])).unwrap();
    assert_eq!(done.commands[0].params["to"], json!([6.013, 0.007]));
}

#[test]
fn escape_in_the_dimension_input_closes_it_and_changes_nothing() {
    let rig = Rig::new();
    let _w1 = rig.wall([0.0, 0.0], [6.0, 0.0]);
    let w2 = rig.wall([0.0, 4.0], [6.0, 4.0]);
    let mut direct = Direct::new();
    let pointer = Pointer::plan(&[w2]);
    let snapshot = rig.document.snapshot();
    let click = |at: [f64; 2]| Input {
        pixel: [at[0] * PPU, -at[1] * PPU],
        ..pointer.at(&rig, &snapshot, at, 0.0)
    };
    assert!(direct.press(&click([3.1, 1.15])));
    assert!(direct.release(&click([3.1, 1.15])).is_none());
    let (_, field) = direct.input().unwrap();
    assert!(field.escape_cancels);
    assert!(direct.edited(&DynamicEvent::Cancel { index: 0 }).is_none());
    assert!(direct.input().is_none());
    assert_eq!(rig.wall_now(w2).start, [0.0, 4.0]);
}
