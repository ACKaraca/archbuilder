//! Stairs: `archbuilder.stair.create` (riser count and height, stride warning, L and U shapes),
//! the shaft opening in the floor of the level above, the modify commands, delete, undo and redo;
//! rooms are not bounded by stairs; the region follow-ups (overlapping walls, slivers).

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::floor::Derived;
use archbuilder_model::spatial;
use archbuilder_model::stair::{Layout, Placed, Shape, Turn};
use archbuilder_model::types::{STAIR, STOREY, text};
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
        spatial::objects(&self.snapshot(), STOREY)
            .iter()
            .find(|s| text(&s.values, 0) == Some(name))
            .unwrap()
            .id
    }

    /// The 10 x 6 house on the ground floor and a slab over it on Floor 1.
    fn house(&self) -> ObjectId {
        let c = [[0.0, 0.0], [0.0, 6.0], [10.0, 6.0], [10.0, 0.0]];
        for k in 0..4 {
            self.run(
                "archbuilder.wall.create",
                json!({"start": c[k], "end": c[(k + 1) % 4]}),
            )
            .unwrap();
        }
        let floor = self
            .run(
                "archbuilder.floor.create",
                json!({"storey": self.storey("Floor 1").to_string(),
                       "points": [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]]}),
            )
            .unwrap();
        floor.value["floor"].as_str().unwrap().parse().unwrap()
    }

    fn stair(&self, params: Json) -> arc_base::Result<Outcome> {
        self.run("archbuilder.stair.create", params)
    }

    fn placed(&self, id: ObjectId) -> Placed {
        let snapshot = self.snapshot();
        Placed::of(&snapshot, &spatial::get(&snapshot, id).unwrap()).unwrap()
    }

    fn floor_area(&self, id: ObjectId) -> f64 {
        let snapshot = self.snapshot();
        Derived::of(&snapshot, &spatial::get(&snapshot, id).unwrap())
            .unwrap()
            .area()
    }
}

fn id_of(outcome: &Outcome) -> ObjectId {
    outcome.value["stair"].as_str().unwrap().parse().unwrap()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn a_storey_of_three_meters_takes_seventeen_risers_and_the_slab_above_gets_the_opening() {
    let env = Env::new();
    let floor = env.house();
    assert!(close(env.floor_area(floor), 60.0));
    let made = env
        .stair(json!({"start": [1.0, 1.0], "to": [5.0, 1.0]}))
        .unwrap();
    let v = &made.value;
    assert_eq!(v["risers"], 17);
    assert!(close(v["riser"].as_f64().unwrap(), 3.0 / 17.0));
    assert!((v["riser"].as_f64().unwrap() - 0.17647).abs() < 1e-5);
    assert!(close(v["runs"][0].as_f64().unwrap(), 4.48));
    assert_eq!(v["stride_ok"], true);
    let id = id_of(&made);
    assert_eq!(
        text(&spatial::get(&env.snapshot(), id).unwrap().values, 0),
        Some("Stair 1")
    );
    // The opening: the footprint (4.48 x 1.0) with 5 cm on three sides, none at the arrival.
    let opening = (4.48 + 0.05) * (1.0 + 0.1);
    assert!(close(env.floor_area(floor), 60.0 - opening));
    // The volume equals the solids'.
    let placed = env.placed(id);
    let layout = placed.layout.as_ref().unwrap();
    let solids: f64 = layout
        .solids(placed.base)
        .unwrap()
        .iter()
        .map(|s| s.volume())
        .sum();
    assert!((solids - v["volume"].as_f64().unwrap()).abs() < 1e-6);
    // Undo takes the stair and the opening back in one step; redo brings both.
    env.document.undo().wait().unwrap();
    assert!(spatial::objects(&env.snapshot(), STAIR).is_empty());
    assert!(close(env.floor_area(floor), 60.0));
    env.document.redo().wait().unwrap();
    assert_eq!(spatial::objects(&env.snapshot(), STAIR).len(), 1);
    assert!(close(env.floor_area(floor), 60.0 - opening));
}

#[test]
fn a_stair_at_the_slab_edge_cuts_a_notch_and_a_steep_one_is_a_warning() {
    let env = Env::new();
    let floor = env.house();
    // The opening reaches 5 cm past the slab's edge at y = 0: a notch of 4.53 x 1.05.
    let made = env
        .stair(json!({"start": [0.5, 0.5], "direction": "0"}))
        .unwrap();
    assert!(close(env.floor_area(floor), 60.0 - 4.53 * 1.05));
    let snapshot = env.snapshot();
    let object = spatial::get(&snapshot, id_of(&made)).unwrap();
    assert!(archbuilder_model::validate::stair_warnings(&snapshot, &object).is_empty());
    // A 20 cm tread: 2 x 0.176 + 0.20 = 0.553, below the range: made, with a warning.
    let steep = env
        .stair(json!({"start": [6.0, 3.0], "direction": "0", "tread": 0.2}))
        .unwrap();
    assert_eq!(steep.value["stride_ok"], false);
    let snapshot = env.snapshot();
    let object = spatial::get(&snapshot, id_of(&steep)).unwrap();
    let codes: Vec<_> = archbuilder_model::validate::stair_warnings(&snapshot, &object)
        .iter()
        .map(|e| e.code().to_owned())
        .collect();
    assert_eq!(codes, ["ab.model.stair_stride"]);
    // The warning names the stair, so a surface can select it.
    let warning = &archbuilder_model::validate::stair_warnings(&snapshot, &object)[0];
    assert!(format!("{:?}", warning.details()).contains(&id_of(&steep).to_string()));
    // A second stair on top of the first is a warning on both.
    let over = env
        .stair(json!({"start": [6.5, 3.0], "direction": "0"}))
        .unwrap();
    let snapshot = env.snapshot();
    let object = spatial::get(&snapshot, id_of(&over)).unwrap();
    let codes: Vec<_> = archbuilder_model::validate::stair_warnings(&snapshot, &object)
        .iter()
        .map(|e| e.code().to_owned())
        .collect();
    assert_eq!(codes, ["ab.model.stair_overlap"]);
}

#[test]
fn l_and_u_stairs_have_the_exact_runs_and_the_top_storey_has_no_stair() {
    let env = Env::new();
    env.house();
    let l = env
        .stair(json!({"start": [1.0, 1.0], "to": [2.0, 1.0], "shape": "l_turn", "turn": "left"}))
        .unwrap();
    let runs: Vec<f64> = l.value["runs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r.as_f64().unwrap())
        .collect();
    assert!(close(runs[0], 8.0 * 0.28) && close(runs[1], 7.0 * 0.28));
    let placed = env.placed(id_of(&l));
    let layout = placed.layout.unwrap();
    assert_eq!(placed.stair.shape, Shape::LTurn);
    // The first run, the 1 x 1 landing and the second run going to +y.
    let fp = layout.footprint();
    assert!(close(
        archbuilder_model::floor::signed_area(&fp),
        2.24 + 1.0 + 1.96
    ));
    assert!(fp.contains(&[1.0 + 2.24 + 1.0, 1.5 + 1.96]));
    let u = env
        .stair(
            json!({"start": [6.0, 1.0], "to": [6.0, 0.0], "shape": "u_turn", "turn": "right",
                      "width": 1.2}),
        )
        .unwrap();
    let placed = env.placed(id_of(&u));
    assert_eq!(placed.stair.turn, Turn::Right);
    let layout = placed.layout.unwrap();
    // Down the y axis, turning right; the landing spans 2 x 1.2 + 0.1.
    assert!(close(
        archbuilder_model::floor::signed_area(&layout.landing),
        1.2 * 2.5
    ));
    // The second run comes back up beside the first, on its right (-x).
    assert!(close(layout.runs[1].dir[0], 0.0) && close(layout.runs[1].dir[1], 1.0));
    assert!(layout.runs[1].start[0] < 6.0 - 1.2);
    // From the top storey there is no level above: refused, nothing created.
    let top = env.storey("Floor 2").to_string();
    let error = env
        .stair(json!({"storey": top, "start": [1.0, 1.0], "direction": "0"}))
        .unwrap_err();
    assert_eq!(error.code(), "ab.model.stair_no_top");
    assert_eq!(spatial::objects(&env.snapshot(), STAIR).len(), 2);
}

#[test]
fn stairs_move_turn_mirror_copy_and_delete_and_the_opening_follows() {
    let env = Env::new();
    let floor = env.house();
    let id = id_of(
        &env.stair(json!({"start": [1.0, 1.0], "direction": "0", "shape": "l_turn"}))
            .unwrap(),
    );
    let area = env.floor_area(floor);
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [id.to_string()], "displacement": [1.0, 0.5]}),
    )
    .unwrap();
    assert_eq!(env.placed(id).stair.start, [2.0, 1.5]);
    assert!(close(env.floor_area(floor), area));
    env.run(
        "archbuilder.modify.rotate",
        json!({"objects": [id.to_string()], "base": [2.0, 1.5], "angle": "90"}),
    )
    .unwrap();
    assert!(close(
        env.placed(id).stair.direction,
        std::f64::consts::FRAC_PI_2
    ));
    env.run(
        "archbuilder.modify.mirror",
        json!({"objects": [id.to_string()], "axis": [[2.0, 0.0], [2.0, 5.0]], "keep": false}),
    )
    .unwrap();
    assert_eq!(env.placed(id).stair.turn, Turn::Right);
    let copied = env
        .run(
            "archbuilder.modify.copy",
            json!({"objects": [id.to_string()], "displacement": [4.0, 0.0]}),
        )
        .unwrap();
    assert_eq!(copied.value["created"].as_array().unwrap().len(), 1);
    assert_eq!(spatial::objects(&env.snapshot(), STAIR).len(), 2);
    // Two openings now.
    let two = env.floor_area(floor);
    assert!(two < area - 1.0);
    // Undo the copy, then delete the stair: the slab is whole again.
    env.document.undo().wait().unwrap();
    let invocation = Invocation {
        seed: 7,
        selection: vec![id],
        ..Invocation::new("archbuilder.object.delete", json!({}))
    };
    let deleted = Invoker::new(&env.registry)
        .invoke(&env.document, &env.session, invocation)
        .unwrap();
    assert_eq!(deleted.value["stairs"], 1);
    assert!(spatial::objects(&env.snapshot(), STAIR).is_empty());
    assert!(close(env.floor_area(floor), 60.0));
    env.document.undo().wait().unwrap();
    assert_eq!(spatial::objects(&env.snapshot(), STAIR).len(), 1);
}

#[test]
fn a_stair_copied_to_the_clipboard_pastes_back_with_its_opening() {
    let env = Env::new();
    let floor = env.house();
    let id = id_of(
        &env.stair(json!({"start": [1.0, 1.0], "direction": "0", "shape": "l_turn", "width": 1.2}))
            .unwrap(),
    );
    let cut = env.floor_area(floor);
    assert!(cut < 60.0 - 1.0);
    // Copy, then delete (a cut): the slab is whole.
    let content = archbuilder_commands::clip::capture(&env.snapshot(), &[id]).unwrap();
    let invocation = Invocation {
        seed: 7,
        selection: vec![id],
        ..Invocation::new("archbuilder.object.delete", json!({}))
    };
    Invoker::new(&env.registry)
        .invoke(&env.document, &env.session, invocation)
        .unwrap();
    assert!(close(env.floor_area(floor), 60.0));
    // Paste in place: the stair is back with its data and its opening.
    let ground = env.storey("Ground Floor").to_string();
    let pasted = env
        .run(
            "archbuilder.clipboard.paste",
            json!({"content": content, "storey": ground, "displacement": [0.0, 0.0]}),
        )
        .unwrap();
    let new: ObjectId = pasted.value["created"][0]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let placed = env.placed(new);
    assert_eq!(placed.stair.shape, Shape::LTurn);
    assert!(close(placed.stair.width, 1.2));
    assert!(close(env.floor_area(floor), cut));
    // A second paste beside it, overlapping: one hole for both, and a warning.
    env.run(
        "archbuilder.clipboard.paste",
        json!({"content": content, "storey": ground, "displacement": [0.5, 0.0]}),
    )
    .unwrap();
    let snapshot = env.snapshot();
    let d = Derived::of(&snapshot, &spatial::get(&snapshot, floor).unwrap()).unwrap();
    assert_eq!(d.uncut, 0);
    assert_eq!(d.boundary.loops.len(), 2);
    let object = spatial::get(&snapshot, new).unwrap();
    let codes: Vec<_> = archbuilder_model::validate::stair_warnings(&snapshot, &object)
        .iter()
        .map(|e| e.code().to_owned())
        .collect();
    assert_eq!(codes, ["ab.model.stair_overlap"]);
}

#[test]
fn a_stair_does_not_bound_rooms_and_its_width_and_tread_are_properties() {
    let env = Env::new();
    env.house();
    let id = id_of(
        &env.stair(json!({"start": [1.0, 3.0], "direction": "0"}))
            .unwrap(),
    );
    env.run("archbuilder.room.place_all", json!({})).unwrap();
    let snapshot = env.snapshot();
    let rooms = archbuilder_model::room::of_storey(&snapshot, env.storey("Ground Floor"));
    assert_eq!(rooms.len(), 1);
    assert!(close(rooms[0].area().unwrap(), 9.8 * 5.8));
    for (field, value) in [("width", json!(1.2)), ("tread", json!(0.3))] {
        env.run(
            "archbuilder.property.set",
            json!({"objects": [id.to_string()], "field": field, "value": value}),
        )
        .unwrap();
    }
    let placed = env.placed(id);
    let layout: &Layout = placed.layout.as_ref().unwrap();
    assert!(close(placed.stair.width, 1.2));
    assert!(close(layout.runs[0].length, 16.0 * 0.3));
    // The top level is a choice: Floor 2 at 6 m gives 34 risers.
    env.run(
        "archbuilder.property.set",
        json!({"objects": [id.to_string()], "field": "top_constraint", "value": 2}),
    )
    .unwrap();
    assert_eq!(env.placed(id).layout.unwrap().risers, 34);
    // A railing toggle.
    env.run(
        "archbuilder.property.set",
        json!({"objects": [id.to_string()], "field": "railing_left", "value": false}),
    )
    .unwrap();
    assert_eq!(env.placed(id).stair.railings, [false, true]);
}

#[test]
fn overlapping_walls_keep_the_room_and_are_reported_and_slivers_get_no_room() {
    let env = Env::new();
    env.house();
    // A partition drawn twice, once each way, from the top wall down to y = 2.
    env.run(
        "archbuilder.wall.create",
        json!({"start": [6.0, 6.0], "end": [6.0, 2.0]}),
    )
    .unwrap();
    env.run(
        "archbuilder.wall.create",
        json!({"start": [6.0, 2.0], "end": [6.0, 6.0]}),
    )
    .unwrap();
    let placed = env.run("archbuilder.room.place_all", json!({})).unwrap();
    assert_eq!(placed.value["rooms"].as_array().unwrap().len(), 1);
    let snapshot = env.snapshot();
    let found =
        archbuilder_model::validate::overlap_findings(&snapshot, env.storey("Ground Floor"));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].code(), "ab.model.wall_overlap");

    // A wall lying on part of an outer wall: the house is still one room of 9.8 x 5.8.
    for (a, b) in [
        ([3.0, 0.0], [8.0, 0.0]),
        ([2.0, 0.0], [4.0, 0.0]),
        ([10.0, 1.0], [10.0, 4.0]),
    ] {
        let env = Env::new();
        env.run("archbuilder.wall.create", json!({"start": a, "end": b}))
            .unwrap();
        env.house();
        let placed = env.run("archbuilder.room.place_all", json!({})).unwrap();
        assert!(
            close(placed.value["areas"][0].as_f64().unwrap(), 56.84),
            "{a:?}"
        );
        let found = archbuilder_model::validate::overlap_findings(
            &env.snapshot(),
            env.storey("Ground Floor"),
        );
        assert_eq!(found.len(), 1);
    }

    // A wall 0.28 m inside the right wall leaves a 0.08 m wide sliver of 0.464 m².
    let env = Env::new();
    env.house();
    env.run(
        "archbuilder.wall.create",
        json!({"start": [9.72, 0.0], "end": [9.72, 6.0]}),
    )
    .unwrap();
    let placed = env.run("archbuilder.room.place_all", json!({})).unwrap();
    assert_eq!(placed.value["rooms"].as_array().unwrap().len(), 1);
    assert_eq!(placed.value["slivers"], 1);
    let error = env
        .run("archbuilder.room.place", json!({"points": [[9.86, 3.0]]}))
        .unwrap_err();
    assert_eq!(error.code(), "ab.cmd.room_region_sliver");
    // Only the sliver is left: place_all says so.
    let error = env
        .run("archbuilder.room.place_all", json!({}))
        .unwrap_err();
    assert_eq!(error.code(), "ab.cmd.room_only_slivers");
}
