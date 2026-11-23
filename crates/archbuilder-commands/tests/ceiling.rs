//! Ceilings, wall openings, shafts, vertical openings and the room solid under them.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::spatial;
use archbuilder_model::types::{CEILING, ROOM, SHAFT, VERTICAL_OPENING, WALL_OPENING};
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

    /// A 10 x 6 house of 0.2 m walls, 3 m high, on `storey`.
    fn house(&self, storey: ObjectId) -> Vec<ObjectId> {
        let c = [[0.0, 0.0], [0.0, 6.0], [10.0, 6.0], [10.0, 0.0]];
        (0..4)
            .map(|k| {
                let made = self
                    .run(
                        "archbuilder.wall.create",
                        json!({"start": c[k], "end": c[(k + 1) % 4], "height": 3.0,
                               "storey": storey.to_string()}),
                    )
                    .unwrap()
                    .value;
                made["wall"].as_str().unwrap().parse().unwrap()
            })
            .collect()
    }

    fn room(&self, storey: ObjectId) -> ObjectId {
        let made = self
            .run(
                "archbuilder.room.place",
                json!({"points": [[5.0, 3.0]], "storey": storey.to_string()}),
            )
            .unwrap()
            .value;
        made["rooms"][0].as_str().unwrap().parse().unwrap()
    }

    fn volume(&self, room: ObjectId) -> (f64, f64) {
        let s = self.snapshot();
        let entries = archbuilder_model::room::of_storey(
            &s,
            spatial::get(&s, room).unwrap().header.owner.unwrap(),
        );
        let entry = entries.iter().find(|e| e.id == room).unwrap();
        let shape = archbuilder_model::room::shape(&s, entry).unwrap();
        (shape.volume, shape.height)
    }

    /// Deletes `ids`, as the delete command does for a selection.
    fn delete(&self, ids: &[ObjectId]) -> arc_base::Result<Outcome> {
        let invocation = Invocation {
            seed: self.seed,
            selection: ids.to_vec(),
            ..Invocation::new("archbuilder.object.delete", json!({}))
        };
        Invoker::new(&self.registry).invoke(&self.document, &self.session, invocation)
    }

    fn id(outcome: &Outcome, key: &str) -> ObjectId {
        outcome.value[key].as_str().unwrap().parse().unwrap()
    }
}

#[test]
fn a_ceiling_from_a_room_click_matches_the_room_and_its_grid_is_clipped() {
    let env = Env::new();
    let ground = env.storeys()[0];
    env.house(ground);
    let room = env.room(ground);
    let made = env
        .run(
            "archbuilder.ceiling.create",
            json!({"picks": [[5.0, 3.0]], "type": "Suspended grid 600 x 600"}),
        )
        .unwrap();
    let ceiling = Env::id(&made, "ceiling");
    let s = env.snapshot();
    let entry = archbuilder_model::room::of_storey(&s, ground)
        .into_iter()
        .find(|e| e.id == room)
        .unwrap();
    // The ceiling's boundary is the room's region at the wall faces.
    assert!((made.value["area"].as_f64().unwrap() - entry.area().unwrap()).abs() < 1e-9);
    assert!((made.value["height"].as_f64().unwrap() - 2.6).abs() < 1e-9);
    let derived =
        archbuilder_model::ceiling::derived(&s, &spatial::get(&s, ceiling).unwrap()).unwrap();
    let lines = derived.grid_lines();
    assert!(!lines.is_empty());
    let region = entry.footprint.boundary();
    let tol = 1e-6;
    for [a, b] in &lines {
        for p in [a, b] {
            assert!(
                p[0] >= 0.1 - tol && p[0] <= 9.9 + tol && p[1] >= 0.1 - tol && p[1] <= 5.9 + tol,
                "{p:?}"
            );
        }
        assert!(region.contains([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]));
    }
    // 9.8 x 5.8 interior: lines every 0.6 m along both axes.
    let along_x = lines
        .iter()
        .filter(|[a, b]| (a[1] - b[1]).abs() < 1e-9)
        .count();
    assert_eq!(along_x, 9);
}

#[test]
fn a_sloped_ceiling_has_the_heights_of_its_arrow() {
    let env = Env::new();
    let ground = env.storeys()[0];
    env.house(ground);
    let made = env
        .run(
            "archbuilder.ceiling.create",
            json!({"picks": [[5.0, 3.0]], "offset": 2.3,
                   "slope_from": [0.1, 3.0], "slope_to": [9.9, 3.0], "end_height": 2.9}),
        )
        .unwrap();
    let id = Env::id(&made, "ceiling");
    let s = env.snapshot();
    let d = archbuilder_model::ceiling::derived(&s, &spatial::get(&s, id).unwrap()).unwrap();
    assert!((d.underside_at([0.1, 1.0]) - 2.3).abs() < 1e-9);
    assert!((d.underside_at([9.9, 5.0]) - 2.9).abs() < 1e-9);
    assert!((d.underside_at([5.0, 3.0]) - 2.6).abs() < 1e-9);
    let (lo, hi) = d.z_range();
    assert!((lo - 2.3).abs() < 1e-9 && (hi - (2.9 + 0.02)).abs() < 1e-9);
    // The body follows the slope: its top is a thickness above the underside everywhere.
    let solid = d.solid().unwrap();
    let (min, max) = solid
        .vertices()
        .iter()
        .fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(v.z), b.max(v.z)));
    assert!(
        (min - 2.3).abs() < 1e-6 && (max - 2.92).abs() < 1e-6,
        "{min} {max}"
    );
}

fn solid_volume(env: &Env, id: ObjectId) -> f64 {
    let s = env.snapshot();
    let o = spatial::get(&s, id).unwrap();
    archbuilder_model::insert::wall_solid(&s, &o)
        .unwrap()
        .unwrap()
        .volume()
}

#[test]
fn a_wall_opening_cuts_the_wall_and_moves_with_it() {
    let env = Env::new();
    let ground = env.storeys()[0];
    let walls = env.house(ground);
    let before = solid_volume(&env, walls[0]);
    // The wall at x = 0 runs 6 m along y; a 1 x 2.1 opening in its middle.
    let made = env
        .run(
            "archbuilder.opening.place",
            json!({"wall": walls[0].to_string(), "distance": 3.0, "width": 1.0, "height": 2.1}),
        )
        .unwrap();
    let opening = Env::id(&made, "insert");
    let cut = before - solid_volume(&env, walls[0]);
    assert!((cut - 1.0 * 2.1 * 0.2).abs() < 1e-6, "{cut}");
    // An arched opening of the same size loses less: the top corners stay.
    env.run(
        "archbuilder.property.set",
        json!({"objects": [opening.to_string()], "field": "rise", "value": 0.4}),
    )
    .unwrap();
    // A rise typed with its unit is taken too, and one over the opening's height is refused.
    env.run(
        "archbuilder.property.set",
        json!({"objects": [opening.to_string()], "field": "rise", "value": "0.4 m"}),
    )
    .unwrap();
    let arched = before - solid_volume(&env, walls[0]);
    assert!(
        arched < cut - 0.01 && arched > cut - 0.2 * 0.2,
        "{arched} {cut}"
    );
    // Moving the wall moves the opening: its host stays, the cut follows the baseline.
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [walls[0].to_string()], "displacement": [-1.0, 0.0]}),
    )
    .unwrap();
    let s = env.snapshot();
    assert_eq!(
        spatial::get(&s, opening).unwrap().header.owner,
        Some(walls[0])
    );
    assert!((before - solid_volume(&env, walls[0]) - arched).abs() < 1e-6);
    // The plan of the wall shows the gap: no wall layer line crosses the opening at 1 m.
    let sill = archbuilder_model::insert::hosted(&s, walls[0]);
    assert_eq!(sill.len(), 1);
    assert_eq!(spatial::objects(&s, WALL_OPENING).len(), 1);
    // Undo takes the opening away, redo brings it back.
    for _ in 0..4 {
        env.document.undo().wait().unwrap();
    }
    assert!(spatial::objects(&env.snapshot(), WALL_OPENING).is_empty());
    env.document.redo().wait().unwrap();
    assert_eq!(spatial::objects(&env.snapshot(), WALL_OPENING).len(), 1);
}

/// Three storeys with a house on each, a floor on the upper two, and a roof on the top.
fn tower(env: &Env) -> (Vec<ObjectId>, Vec<ObjectId>, ObjectId) {
    let storeys = env.storeys();
    let mut floors = Vec::new();
    for s in &storeys {
        env.house(*s);
        let made = env
            .run(
                "archbuilder.floor.create",
                json!({"picks": [[5.0, 3.0]], "storey": s.to_string()}),
            )
            .unwrap();
        floors.push(Env::id(&made, "floor"));
    }
    let roof = env
        .run(
            "archbuilder.roof.create",
            json!({"picks": [[5.0, 3.0]], "storey": storeys[2].to_string(),
                   "base_offset": 3.0}),
        )
        .unwrap();
    (storeys, floors, Env::id(&roof, "roof"))
}

fn floor_area(env: &Env, id: ObjectId) -> (f64, f64) {
    let s = env.snapshot();
    let d = archbuilder_model::floor::Derived::of(&s, &spatial::get(&s, id).unwrap()).unwrap();
    (d.area(), d.volume())
}

#[test]
fn a_shaft_through_three_storeys_cuts_each_floor_and_the_roof() {
    let env = Env::new();
    let (storeys, floors, roof) = tower(&env);
    let before: Vec<(f64, f64)> = floors.iter().map(|f| floor_area(&env, *f)).collect();
    let roof_before = {
        let s = env.snapshot();
        archbuilder_model::roof::Derived::of(&s, &spatial::get(&s, roof).unwrap())
            .unwrap()
            .solid()
            .unwrap()
            .volume()
    };
    // A 1 x 1 shaft from the ground floor's level to the level of the top storey plus 4 m:
    // the first floor and the second floor and the roof are cut, the ground slab is not.
    let made = env
        .run(
            "archbuilder.shaft.create",
            json!({"storey": storeys[0].to_string(),
                   "points": [[4.0, 0.8], [5.0, 0.8], [5.0, 1.8], [4.0, 1.8]],
                   "top_level": spatial::get(&env.snapshot(), storeys[2]).map(|_| {
                       let s = env.snapshot();
                       let st = spatial::get(&s, storeys[2]).unwrap();
                       spatial::level_of(&s, &st).unwrap().id.to_string()
                   }).unwrap(),
                   "top_offset": 4.0}),
        )
        .unwrap();
    let span = made.value["span"].as_array().unwrap();
    assert!((span[1].as_f64().unwrap() - span[0].as_f64().unwrap() - 10.0).abs() < 1e-9);
    let after: Vec<(f64, f64)> = floors.iter().map(|f| floor_area(&env, *f)).collect();
    assert!(
        (after[0].0 - before[0].0).abs() < 1e-9,
        "the ground slab is not cut"
    );
    for k in 1..3 {
        assert!((before[k].0 - after[k].0 - 1.0).abs() < 1e-9, "storey {k}");
        assert!((before[k].1 - after[k].1 - 0.2).abs() < 1e-9, "storey {k}");
    }
    let s = env.snapshot();
    let cut = archbuilder_model::roof::Derived::of(&s, &spatial::get(&s, roof).unwrap()).unwrap();
    assert_eq!(cut.cuts.len(), 1);
    assert!(cut.solid().unwrap().volume() < roof_before - 0.1);
    // Deleting the shaft gives the floors back; undo cuts them again.
    let shaft = Env::id(&made, "opening");
    env.delete(&[shaft]).unwrap();
    assert!((floor_area(&env, floors[1]).0 - before[1].0).abs() < 1e-9);
    env.document.undo().wait().unwrap();
    assert!((before[1].0 - floor_area(&env, floors[1]).0 - 1.0).abs() < 1e-9);
    assert_eq!(spatial::objects(&env.snapshot(), SHAFT).len(), 1);
}

#[test]
fn a_vertical_opening_cuts_its_host_only() {
    let env = Env::new();
    let (storeys, floors, _) = tower(&env);
    let before = floor_area(&env, floors[1]);
    let other = floor_area(&env, floors[2]);
    let made = env
        .run(
            "archbuilder.vertical_opening.create",
            json!({"host": floors[1].to_string(),
                   "points": [[4.0, 2.0], [6.0, 2.0], [6.0, 3.0], [4.0, 3.0]]}),
        )
        .unwrap();
    let after = floor_area(&env, floors[1]);
    assert!((before.0 - after.0 - 2.0).abs() < 1e-9);
    assert!((floor_area(&env, floors[2]).0 - other.0).abs() < 1e-9);
    let cut = Env::id(&made, "opening");
    assert_eq!(
        spatial::get(&env.snapshot(), cut).unwrap().header.owner,
        Some(floors[1])
    );
    // A copy of the floor to the next storey brings its opening with it.
    env.run(
        "archbuilder.object.select",
        json!({"objects": [floors[1].to_string()]}),
    )
    .unwrap();
    let copied = env
        .run(
            "archbuilder.modify.copy",
            json!({"objects": [floors[1].to_string()], "displacement": [0.0, 0.0]}),
        )
        .unwrap();
    assert!(copied.value["created"].as_array().unwrap().len() >= 2);
    assert_eq!(spatial::objects(&env.snapshot(), VERTICAL_OPENING).len(), 2);
    let _ = storeys;
}

#[test]
fn the_room_volume_follows_a_flat_ceiling_a_sloped_ceiling_and_a_roof() {
    let env = Env::new();
    let ground = env.storeys()[0];
    env.house(ground);
    let room = env.room(ground);
    // Nothing above: the room runs to its upper limit, the level above (3 m).
    let area = 9.8 * 5.8;
    let (v, h) = env.volume(room);
    assert!(
        (v - area * 3.0).abs() < 1e-6 && (h - 3.0).abs() < 1e-9,
        "{v} {h}"
    );
    // A flat ceiling at 2.6 m bounds it.
    let made = env
        .run("archbuilder.ceiling.create", json!({"picks": [[5.0, 3.0]]}))
        .unwrap();
    let ceiling = Env::id(&made, "ceiling");
    let (v, h) = env.volume(room);
    assert!(
        (v - area * 2.6).abs() < 1e-6 && (h - 2.6).abs() < 1e-9,
        "{v} {h}"
    );
    // Raising the ceiling updates the volume.
    env.run(
        "archbuilder.property.set",
        json!({"objects": [ceiling.to_string()], "field": "height_offset", "value": 2.8}),
    )
    .unwrap();
    assert!((env.volume(room).0 - area * 2.8).abs() < 1e-6);
    // A slope from 2.3 m to 2.9 m over the room's width: the mean height is 2.6 m.
    env.run(
        "archbuilder.ceiling.set_slope",
        json!({"ceiling": ceiling.to_string(), "from": [0.1, 3.0], "to": [9.9, 3.0],
               "end_height": 2.9}),
    )
    .unwrap();
    env.run(
        "archbuilder.property.set",
        json!({"objects": [ceiling.to_string()], "field": "height_offset", "value": 2.3}),
    )
    .unwrap();
    let (v, h) = env.volume(room);
    assert!(
        (v - area * 2.6).abs() < 1e-6 && (h - 2.6).abs() < 1e-6,
        "{v} {h}"
    );
    // Without the ceiling, a roof 2.5 m above the room's storey level bounds it at its eaves.
    env.delete(&[ceiling]).unwrap();
    env.run(
        "archbuilder.roof.create",
        json!({"picks": [[5.0, 3.0]], "slope": 0.2, "base_offset": 2.5, "overhang": 0.0}),
    )
    .unwrap();
    let (v, h) = env.volume(room);
    assert!(v < area * 3.0 && v > area * 2.5 && h < 3.0, "{v} {h}");
}

#[test]
fn editing_a_ceiling_updates_the_room_and_a_room_rename_recomputes_no_ceiling() {
    let env = Env::new();
    let ground = env.storeys()[0];
    env.house(ground);
    let room = env.room(ground);
    let made = env
        .run("archbuilder.ceiling.create", json!({"picks": [[5.0, 3.0]]}))
        .unwrap();
    let ceiling = Env::id(&made, "ceiling");
    let first = env.snapshot();
    let object = spatial::get(&first, ceiling).unwrap();
    let derived = archbuilder_model::ceiling::derived(&first, &object).unwrap();
    // Renaming the room changes the document but not what the ceiling reads.
    env.run(
        "archbuilder.property.set",
        json!({"objects": [room.to_string()], "field": "name", "value": "Studio"}),
    )
    .unwrap();
    let second = env.snapshot();
    assert!(!first.ptr_eq(&second));
    let again =
        archbuilder_model::ceiling::derived(&second, &spatial::get(&second, ceiling).unwrap())
            .unwrap();
    assert!(Arc::ptr_eq(&derived, &again), "the ceiling was recomputed");
    // Editing the ceiling does update it, and the room volume with it.
    let (before, _) = env.volume(room);
    env.run(
        "archbuilder.property.set",
        json!({"objects": [ceiling.to_string()], "field": "height_offset", "value": 2.0}),
    )
    .unwrap();
    let third = env.snapshot();
    let changed =
        archbuilder_model::ceiling::derived(&third, &spatial::get(&third, ceiling).unwrap())
            .unwrap();
    assert!(!Arc::ptr_eq(&derived, &changed));
    assert!(env.volume(room).0 < before);
    // A ceiling of another storey is not recomputed by an edit of this one either.
    let _ = (ROOM, CEILING);
}

#[test]
fn a_room_volume_follows_the_walls_an_upper_slab_follows() {
    let env = Env::new();
    let (storeys, floors, _) = tower(&env);
    let room = env.room(storeys[0]);
    let before = env.volume(room).0;
    // The first storey's slab follows its walls: moving the wall at x = 10 in shrinks it, and the
    // room below loses the slab over that strip as its top.
    let s = env.snapshot();
    let wall = spatial::walls(&s, storeys[1])
        .into_iter()
        .find(|w| {
            archbuilder_model::wall::Wall::from_values(&w.values)
                .is_some_and(|d| d.start[0] > 9.0 && d.end[0] > 9.0)
        })
        .unwrap();
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [wall.id.to_string()], "displacement": [-2.0, 0.0]}),
    )
    .unwrap();
    let area = floor_area(&env, floors[1]).0;
    assert!(area < 9.8 * 5.8 - 1.0, "{area}");
    let after = env.volume(room).0;
    assert!(after > before + 1.0, "{before} {after}");
}

#[test]
fn a_shaft_elsewhere_in_plan_leaves_a_sloped_ceiling_and_a_roof_whole() {
    let env = Env::new();
    let (storeys, _, roof) = tower(&env);
    let made = env
        .run(
            "archbuilder.ceiling.create",
            json!({"picks": [[5.0, 3.0]], "storey": storeys[0].to_string(), "offset": 2.4,
                   "slope_from": [0.1, 3.0], "slope_to": [9.9, 3.0], "end_height": 2.7}),
        )
        .unwrap();
    // A shaft far from the house, spanning every height.
    env.run(
        "archbuilder.shaft.create",
        json!({"storey": storeys[0].to_string(),
               "points": [[30.0, 0.0], [31.0, 0.0], [31.0, 1.0], [30.0, 1.0]], "top_offset": 9.0}),
    )
    .unwrap();
    let s = env.snapshot();
    let d = archbuilder_model::ceiling::derived(
        &s,
        &spatial::get(&s, Env::id(&made, "ceiling")).unwrap(),
    )
    .unwrap();
    assert!(d.cuts.is_empty());
    assert!(d.solid().is_ok());
    let r = archbuilder_model::roof::Derived::of(&s, &spatial::get(&s, roof).unwrap()).unwrap();
    assert!(r.cuts.is_empty() && r.solid_cut().unwrap().1);
}

#[test]
fn a_slope_over_the_limit_is_refused_whichever_way_it_is_given() {
    let env = Env::new();
    let ground = env.storeys()[0];
    env.house(ground);
    let made = env
        .run("archbuilder.ceiling.create", json!({"picks": [[5.0, 3.0]]}))
        .unwrap();
    let id = Env::id(&made, "ceiling").to_string();
    for params in [
        json!({"ceiling": id, "from": [1.0, 1.0], "to": [1.01, 1.0], "end_height": 500.0}),
        json!({"ceiling": id, "from": [1.0, 1.0], "to": [9.0, 1.0], "slope": 1.55}),
    ] {
        assert!(env.run("archbuilder.ceiling.set_slope", params).is_err());
    }
    assert!(
        env.run(
            "archbuilder.property.set",
            json!({"objects": [id], "field": "ceiling_slope", "value": "89"}),
        )
        .is_err()
    );
    // A hand-edited end height past the limit is refused by the model's rules.
    env.run(
        "archbuilder.ceiling.set_slope",
        json!({"ceiling": id, "from": [1.0, 1.0], "to": [1.01, 1.0], "slope": 0.03}),
    )
    .unwrap();
    env.run(
        "archbuilder.property.set",
        json!({"objects": [id], "field": "slope", "value": ""}),
    )
    .unwrap();
    assert!(
        env.run(
            "archbuilder.property.set",
            json!({"objects": [id], "field": "end_height", "value": 500.0}),
        )
        .is_err()
    );
}

#[test]
fn a_room_under_a_shaft_loses_the_shaft_from_its_solid() {
    let env = Env::new();
    let ground = env.storeys()[0];
    env.house(ground);
    let room = env.room(ground);
    let before = env.volume(room).0;
    env.run(
        "archbuilder.shaft.create",
        json!({"storey": ground.to_string(),
               "points": [[4.0, 2.0], [5.0, 2.0], [5.0, 3.0], [4.0, 3.0]], "top_offset": 1.0}),
    )
    .unwrap();
    let after = env.volume(room).0;
    assert!(
        (before - after - 1.0 * 3.0).abs() < 1e-6,
        "{before} {after}"
    );
}

#[test]
fn a_ceiling_inside_a_room_is_picked_by_its_edge_before_the_room() {
    let env = Env::new();
    let ground = env.storeys()[0];
    env.house(ground);
    env.room(ground);
    let made = env
        .run("archbuilder.ceiling.create", json!({"picks": [[5.0, 3.0]]}))
        .unwrap();
    let ceiling = Env::id(&made, "ceiling");
    let s = env.snapshot();
    let rooms = archbuilder_model::pick::all_rooms(&s);
    // On the ceiling's edge (the room's region edge, a little inside the wall) it comes first
    // of what the plan has there; in the middle of the room the room does.
    let edge = archbuilder_model::pick::stack(&s, &rooms, [5.0, 0.12], 0.05, None);
    let rooms_first = archbuilder_model::pick::stack(&s, &rooms, [5.0, 3.0], 0.05, None);
    let (c, r) = (
        rooms_first.iter().position(|i| *i == ceiling).unwrap(),
        rooms_first
            .iter()
            .position(|i| spatial::type_of(&s, *i).as_deref() == Some(ROOM))
            .unwrap(),
    );
    assert!(r < c);
    let on_edge = edge.iter().position(|i| *i == ceiling).unwrap();
    let room_at_edge = edge
        .iter()
        .position(|i| spatial::type_of(&s, *i).as_deref() == Some(ROOM));
    assert!(room_at_edge.is_none_or(|r| on_edge < r), "{edge:?}");
}

#[test]
fn a_sloped_arc_ceiling_keeps_its_body_with_a_shaft_and_a_vertical_opening() {
    let env = Env::new();
    let ground = env.storeys()[0];
    env.house(ground);
    let made = env
        .run(
            "archbuilder.ceiling.create",
            json!({"points": [[1.0, 1.0], [9.0, 1.0], [9.0, 5.0], [1.0, 5.0]],
                   "arcs": [[0.0, 0.3]], "offset": 2.3,
                   "slope_from": [0.1, 3.0], "slope_to": [9.9, 3.0], "end_height": 2.9}),
        )
        .unwrap();
    let ceiling = Env::id(&made, "ceiling");
    env.run(
        "archbuilder.shaft.create",
        json!({"storey": ground.to_string(),
               "points": [[2.0, 2.0], [3.0, 2.0], [3.0, 3.0], [2.0, 3.0]], "top_offset": 1.0}),
    )
    .unwrap();
    env.run(
        "archbuilder.vertical_opening.create",
        json!({"host": ceiling.to_string(),
               "points": [[6.0, 2.0], [7.0, 2.0], [7.0, 3.0], [6.0, 3.0]]}),
    )
    .unwrap();
    let s = env.snapshot();
    let d = archbuilder_model::ceiling::derived(&s, &spatial::get(&s, ceiling).unwrap()).unwrap();
    assert_eq!(d.cuts.len(), 2);
    let solid = d.solid().unwrap();
    let (flat, _) = (d.boundary.area(), 0);
    assert!(
        (solid.volume() - flat * 0.02).abs() < 0.05,
        "{} {}",
        solid.volume(),
        flat * 0.02
    );
}

#[test]
fn a_room_with_a_shaft_under_a_sloped_ceiling_agrees_in_area_volume_and_body() {
    let env = Env::new();
    let ground = env.storeys()[0];
    env.house(ground);
    let room = env.room(ground);
    env.run(
        "archbuilder.ceiling.create",
        json!({"picks": [[5.0, 3.0]], "offset": 2.3,
               "slope_from": [0.1, 3.0], "slope_to": [9.9, 3.0], "end_height": 2.9}),
    )
    .unwrap();
    env.run(
        "archbuilder.shaft.create",
        json!({"storey": ground.to_string(),
               "points": [[4.0, 2.0], [5.0, 2.0], [5.0, 3.0], [4.0, 3.0]], "top_offset": 1.0}),
    )
    .unwrap();
    let s = env.snapshot();
    let entries = archbuilder_model::room::of_storey(&s, ground);
    let entry = entries.iter().find(|e| e.id == room).unwrap();
    let shape = archbuilder_model::room::shape(&s, entry).unwrap();
    // The underside at the shaft's middle (x = 4.5) is 2.3 + 0.6 * 4.4 / 9.8.
    let hand = 9.8 * 5.8 * 2.6 - (2.3 + 0.6 * 4.4 / 9.8);
    assert!(
        (shape.area - (9.8 * 5.8 - 1.0)).abs() < 1e-6,
        "{}",
        shape.area
    );
    assert!(
        (shape.volume - hand).abs() < 1e-6,
        "{} {hand}",
        shape.volume
    );
    let body = shape.solid().unwrap().volume();
    assert!((body - hand).abs() < 1e-6, "{body} {hand}");
}

#[test]
fn property_set_reads_a_json_number_as_meters_for_a_wall_height() {
    let env = Env::new();
    let ground = env.storeys()[0];
    let walls = env.house(ground);
    let wall = walls[0].to_string();
    env.run(
        "archbuilder.property.set",
        json!({"objects": [wall], "field": "height", "value": 3.1}),
    )
    .unwrap();
    let s = env.snapshot();
    let w = archbuilder_model::wall::Wall::from_values(&spatial::get(&s, walls[0]).unwrap().values)
        .unwrap();
    assert!((w.height - 3.1).abs() < 1e-9, "{}", w.height);
}

#[test]
fn a_slope_that_carries_the_ceiling_out_of_its_storey_is_refused() {
    let env = Env::new();
    let ground = env.storeys()[0];
    env.house(ground);
    let made = env
        .run("archbuilder.ceiling.create", json!({"picks": [[5.0, 3.0]]}))
        .unwrap();
    let id = Env::id(&made, "ceiling").to_string();
    // The arrow starts 45 m from the ceiling and falls 0.6 m per meter: the ceiling is 24 m down.
    let err = env
        .run(
            "archbuilder.ceiling.set_slope",
            json!({"ceiling": id, "from": [50.0, 3.0], "to": [49.0, 3.0], "end_height": 2.0}),
        )
        .unwrap_err();
    assert_eq!(err.code(), "ab.model.ceiling_range");
    // One that stays within the storey is taken.
    env.run(
        "archbuilder.ceiling.set_slope",
        json!({"ceiling": id, "from": [0.1, 3.0], "to": [9.9, 3.0], "end_height": 2.9}),
    )
    .unwrap();
}

#[test]
fn a_room_under_a_partial_sloped_arc_ceiling_with_cuts_keeps_its_body() {
    let env = Env::new();
    let ground = env.storeys()[0];
    let (storeys, _, _) = tower(&env);
    let room = env.room(ground);
    let made = env
        .run(
            "archbuilder.ceiling.create",
            json!({"points": [[1.0, 1.0], [9.0, 1.0], [9.0, 5.0], [1.0, 5.0]],
                   "arcs": [[0.0, 0.3]], "offset": 2.4,
                   "slope_from": [0.1, 3.0], "slope_to": [9.9, 3.0], "end_height": 2.7}),
        )
        .unwrap();
    let top = {
        let s = env.snapshot();
        let st = spatial::get(&s, storeys[2]).unwrap();
        spatial::level_of(&s, &st).unwrap().id.to_string()
    };
    env.run(
        "archbuilder.shaft.create",
        json!({"storey": ground.to_string(),
               "points": [[2.0, 2.0], [3.0, 2.0], [3.0, 3.0], [2.0, 3.0]],
               "top_level": top}),
    )
    .unwrap();
    env.run(
        "archbuilder.vertical_opening.create",
        json!({"host": Env::id(&made, "ceiling").to_string(),
               "points": [[6.0, 2.0], [7.0, 2.0], [7.0, 3.0], [6.0, 3.0]]}),
    )
    .unwrap();
    let s = env.snapshot();
    let entries = archbuilder_model::room::of_storey(&s, ground);
    let entry = entries.iter().find(|e| e.id == room).unwrap();
    let shape = archbuilder_model::room::shape(&s, entry).unwrap();
    let body = shape.solid().unwrap().volume();
    assert!(
        (body - shape.volume).abs() < 1e-6,
        "{body} {}",
        shape.volume
    );
}

#[test]
fn a_ceiling_may_not_pass_through_the_slab_above_it() {
    let env = Env::new();
    let (storeys, _, _) = tower(&env);
    let made = env
        .run(
            "archbuilder.ceiling.create",
            json!({"picks": [[5.0, 3.0]], "storey": storeys[0].to_string()}),
        )
        .unwrap();
    let id = Env::id(&made, "ceiling").to_string();
    // The slab above has its underside at 2.8 m, under the level above (3.0 m).
    let refused = env.run(
        "archbuilder.property.set",
        json!({"objects": [id], "field": "height_offset", "value": 2.9}),
    );
    assert_eq!(
        refused.unwrap_err().code(),
        "ab.model.ceiling_range_element"
    );
    env.run(
        "archbuilder.property.set",
        json!({"objects": [id], "field": "height_offset", "value": 2.7}),
    )
    .unwrap();
}

#[test]
fn every_room_value_reads_the_net_area_with_the_shaft_out() {
    let env = Env::new();
    let ground = env.storeys()[0];
    env.house(ground);
    let room = env.room(ground);
    env.run(
        "archbuilder.shaft.create",
        json!({"storey": ground.to_string(),
               "points": [[4.0, 2.0], [5.0, 2.0], [5.0, 3.0], [4.0, 3.0]], "top_offset": 1.0}),
    )
    .unwrap();
    let s = env.snapshot();
    let entries = archbuilder_model::room::of_storey(&s, ground);
    let entry = entries.iter().find(|e| e.id == room).unwrap();
    let net = archbuilder_model::room::net_area(&s, entry).unwrap();
    assert!((net - (9.8 * 5.8 - 1.0)).abs() < 1e-6, "{net}");
    let volume = archbuilder_model::room::net_volume(&s, entry).unwrap();
    assert!((volume - net * 3.0).abs() < 1e-6, "{volume}");
}

#[test]
fn a_ceiling_is_bounded_by_what_is_over_its_own_footprint() {
    let env = Env::new();
    let (storeys, _, roof) = tower(&env);
    // The slab of the first floor is cut back to x 0.1 - 7.9 by a vertical opening over the rest.
    let s = env.snapshot();
    let floor = spatial::children(&s, archbuilder_model::types::FLOOR, storeys[1])[0].id;
    env.run(
        "archbuilder.vertical_opening.create",
        json!({"host": floor.to_string(),
               "points": [[7.9, 0.0], [10.0, 0.0], [10.0, 6.0], [7.9, 6.0]]}),
    )
    .unwrap();
    let ceiling = |x: [f64; 2], offset: f64| {
        env.run(
            "archbuilder.ceiling.create",
            json!({"storey": storeys[0].to_string(), "offset": offset,
                   "points": [[x[0], 1.0], [x[1], 1.0], [x[1], 5.0], [x[0], 5.0]]}),
        )
    };
    // Uncovered, only the level above (3.0 m) bounds it; covered, the slab's underside (2.8 m).
    assert!(ceiling([8.2, 9.8], 2.9).is_ok());
    let err = ceiling([2.0, 4.0], 2.9).unwrap_err();
    assert_eq!(err.code(), "ab.model.ceiling_range_element");
    assert!(ceiling([2.0, 4.0], 2.7).is_ok());
    // Under the middle of the hip roof of the top storey: above its eave, under its own underside.
    let s = env.snapshot();
    let r = archbuilder_model::roof::Derived::of(&s, &spatial::get(&s, roof).unwrap()).unwrap();
    let under = |p: [f64; 2]| {
        r.surface
            .pieces()
            .iter()
            .map(|pc| pc.plane.z(p))
            .fold(f64::MAX, f64::min)
    };
    let eave = r
        .surface
        .pieces()
        .iter()
        .flat_map(|pc| pc.points.iter().map(|q| pc.plane.z(*q)))
        .fold(f64::MAX, f64::min);
    let local = under([5.0, 3.0]);
    assert!(local > eave + 0.2, "{local} {eave}");
    let level = 6.0;
    let top_ceiling = |offset: f64| {
        env.run(
            "archbuilder.ceiling.create",
            json!({"storey": storeys[2].to_string(), "offset": offset,
                   "points": [[4.0, 2.5], [6.0, 2.5], [6.0, 3.5], [4.0, 3.5]]}),
        )
    };
    assert!(top_ceiling((eave + local) / 2.0 - level).is_ok());
    let err = top_ceiling(local + 0.1 - level).unwrap_err();
    assert_eq!(err.code(), "ab.model.ceiling_range_element");
}

#[test]
fn a_change_above_a_ceiling_warns_and_never_locks_other_edits() {
    let env = Env::new();
    let (storeys, _, _) = tower(&env);
    let made = env
        .run(
            "archbuilder.ceiling.create",
            json!({"storey": storeys[0].to_string(), "offset": 2.7,
                   "points": [[2.0, 1.0], [4.0, 1.0], [4.0, 5.0], [2.0, 5.0]]}),
        )
        .unwrap();
    let id = Env::id(&made, "ceiling").to_string();
    let range = |env: &Env| {
        archbuilder_model::validate::register(&env.snapshot())
            .into_iter()
            .filter(|w| w.error.code() == "ab.model.ceiling_range_element")
            .count()
    };
    assert_eq!(range(&env), 0);
    // The level of the first floor drops: its slab's underside (2.65 m) passes the ceiling's top.
    env.run(
        "archbuilder.property.set",
        json!({"objects": [storeys[1].to_string()], "field": "elevation", "value": 2.85}),
    )
    .unwrap();
    assert_eq!(range(&env), 1);
    // Other edits are not refused: a rename, an unrelated wall.
    env.run(
        "archbuilder.property.set",
        json!({"objects": [id], "field": "name", "value": "Hall ceiling"}),
    )
    .unwrap();
    let wall = {
        let s = env.snapshot();
        spatial::walls(&s, storeys[0])[0].id
    };
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [wall.to_string()], "displacement": [0.0, 0.1]}),
    )
    .unwrap();
    // The ceiling itself may not go further up, and may come down to fit.
    let up = env.run(
        "archbuilder.property.set",
        json!({"objects": [id], "field": "height_offset", "value": 2.8}),
    );
    assert_eq!(up.unwrap_err().code(), "ab.model.ceiling_range_element");
    env.run(
        "archbuilder.property.set",
        json!({"objects": [id], "field": "height_offset", "value": 2.6}),
    )
    .unwrap();
    assert_eq!(range(&env), 0);
}

#[test]
fn a_paste_a_copy_or_a_move_that_puts_a_ceiling_through_a_slab_is_refused() {
    let env = Env::new();
    let (storeys, _, _) = tower(&env);
    // The slab of the first floor is cut back to x 0.1 - 7.9: nothing but the level is over x 8 - 10.
    let s = env.snapshot();
    let floor = spatial::children(&s, archbuilder_model::types::FLOOR, storeys[1])[0].id;
    env.run(
        "archbuilder.vertical_opening.create",
        json!({"host": floor.to_string(),
               "points": [[7.9, 0.0], [10.0, 0.0], [10.0, 6.0], [7.9, 6.0]]}),
    )
    .unwrap();
    let made = env
        .run(
            "archbuilder.ceiling.create",
            json!({"storey": storeys[0].to_string(), "offset": 2.9,
                   "points": [[8.2, 1.0], [9.8, 1.0], [9.8, 5.0], [8.2, 5.0]]}),
        )
        .unwrap();
    let id = Env::id(&made, "ceiling").to_string();
    let count = |env: &Env| spatial::objects(&env.snapshot(), CEILING).len();
    let before = count(&env);
    // Under the slab (underside 2.8 m) the 2.9 m ceiling passes through it.
    for (command, dx) in [
        ("archbuilder.clipboard.paste", -3.0),
        ("archbuilder.modify.copy", -2.0),
        ("archbuilder.modify.move", -2.0),
    ] {
        let refused = env.run(command, json!({"objects": [id], "displacement": [dx, 0.0]}));
        assert_eq!(
            refused.unwrap_err().code(),
            "ab.model.ceiling_range_element",
            "{command}"
        );
    }
    assert_eq!(count(&env), before);
    // Clear of the slab, the same tools are accepted.
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [id], "displacement": [0.0, 0.5]}),
    )
    .unwrap();
}
