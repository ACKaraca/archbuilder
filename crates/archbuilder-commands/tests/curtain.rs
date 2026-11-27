//! Curtain walls: the type, grid edits, panel overrides, embedding, corners and undo.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::curtain::Derived;
use archbuilder_model::element_type::{self as et, TypeKind};
use archbuilder_model::spatial;
use serde_json::{Value as Json, json};

struct Env {
    document: Document,
    session: Session,
    registry: arc_command::Registry,
}

impl Env {
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

    fn type_named(&self, kind: TypeKind, name: &str) -> ObjectId {
        et::of_kind(&self.snapshot(), kind)
            .iter()
            .find(|t| et::name(t) == name)
            .unwrap()
            .id
    }

    fn wall(&self, start: [f64; 2], end: [f64; 2], extra: Json) -> ObjectId {
        let mut params = json!({"start": start, "end": end});
        if let (Some(p), Some(e)) = (params.as_object_mut(), extra.as_object()) {
            p.extend(e.clone());
        }
        let made = self.run("archbuilder.wall.create", params).unwrap().value;
        made["wall"].as_str().unwrap().parse().unwrap()
    }
}

fn curtain_type(env: &Env, extra: Json) -> ObjectId {
    let made = env.run("archbuilder.curtain.type", extra).unwrap().value;
    made["type"].as_str().unwrap().parse().unwrap()
}

fn derived(env: &Env, wall: ObjectId) -> Derived {
    let s = env.snapshot();
    Derived::of(&s, &spatial::get(&s, wall).unwrap()).unwrap()
}

fn setup() -> (Env, ObjectId, ObjectId) {
    let env = Env::new();
    let ty = curtain_type(
        &env,
        json!({"name": "CW", "vertical": {"kind": "fixed_distance", "value": 1.0},
               "horizontal": {"kind": "fixed_number", "value": 1}}),
    );
    let wall = env.wall(
        [0.0, 0.0],
        [3.0, 0.0],
        json!({"type": ty.to_string(), "height": 3.0}),
    );
    (env, ty, wall)
}

#[test]
fn a_curtain_wall_is_a_wall_of_a_curtain_type_with_a_derived_grid() {
    let (env, ty, wall) = setup();
    let d = derived(&env, wall);
    assert_eq!(d.panels.len(), 6);
    assert!(!d.mullions.is_empty());
    assert!(!d.solids().unwrap().is_empty());
    // Plain walls never take the curtain type by default.
    let plain = env.wall([0.0, 5.0], [3.0, 5.0], json!({}));
    let s = env.snapshot();
    assert!(Derived::of(&s, &spatial::get(&s, plain).unwrap()).is_none());
    // Editing the type regenerates the grid of the wall without touching the wall's grid data.
    env.run(
        "archbuilder.curtain.type",
        json!({"type": ty, "vertical": {"value": 1.5}, "interior": {"profile": "circular"}}),
    )
    .unwrap();
    assert_eq!(derived(&env, wall).panels.len(), 4);
    // An invalid rule is refused.
    let e = env
        .run(
            "archbuilder.curtain.type",
            json!({"type": ty, "vertical": {"value": 0.0}}),
        )
        .unwrap_err();
    assert_eq!(e.code(), "ab.model.curtain_rule");
}

#[test]
fn grid_edits_are_authored_data_and_undoable() {
    let (env, _ty, wall) = setup();
    env.run(
        "archbuilder.curtain.grid_add",
        json!({"wall": wall, "direction": "vertical", "at": 0.5}),
    )
    .unwrap();
    assert_eq!(derived(&env, wall).panels.len(), 8);
    env.run(
        "archbuilder.curtain.grid_move",
        json!({"wall": wall, "direction": "vertical", "line": "s1", "to": 1.25}),
    )
    .unwrap();
    assert!((derived(&env, wall).layout.vertical[2].at - 1.25).abs() < 1e-9);
    env.run(
        "archbuilder.curtain.grid_remove",
        json!({"wall": wall, "direction": "vertical", "line": "s2"}),
    )
    .unwrap();
    assert_eq!(derived(&env, wall).panels.len(), 6);
    let e = env
        .run(
            "archbuilder.curtain.grid_remove",
            json!({"wall": wall, "direction": "vertical", "line": "nope"}),
        )
        .unwrap_err();
    assert_eq!(e.code(), "ab.model.curtain_unknown");
    env.document.undo().wait().unwrap();
    assert_eq!(derived(&env, wall).panels.len(), 8);
    env.document.undo().wait().unwrap();
    env.document.undo().wait().unwrap();
    assert_eq!(derived(&env, wall).panels.len(), 6);
}

#[test]
fn a_panel_override_keeps_its_cell_and_is_dropped_when_the_cell_dissolves() {
    let (env, _ty, wall) = setup();
    let cell = derived(&env, wall).layout.cells[0].key.clone();
    let door = env.type_named(TypeKind::Door, "Single 900 × 2100");
    env.run(
        "archbuilder.curtain.panel",
        json!({"wall": wall, "cell": cell, "panel": "door", "type": door}),
    )
    .unwrap();
    let d = derived(&env, wall);
    assert_eq!(
        d.panels[0].kind,
        archbuilder_model::curtain::PanelKind::Door
    );
    assert!(
        d.solids()
            .unwrap()
            .iter()
            .any(|s| s.class == archbuilder_model::curtain::Class::Leaf)
    );
    // A moved line keeps the cell's key and its override.
    env.run(
        "archbuilder.curtain.grid_move",
        json!({"wall": wall, "direction": "vertical", "line": "s1", "to": 1.2}),
    )
    .unwrap();
    assert!(derived(&env, wall).panels[0].overridden);
    // Removing the line merges the cell away and reconciles the override in the same step.
    env.run(
        "archbuilder.curtain.grid_remove",
        json!({"wall": wall, "direction": "vertical", "line": "s1"}),
    )
    .unwrap();
    let d = derived(&env, wall);
    assert!(d.panels.iter().all(|p| !p.overridden));
    assert!(d.layout.orphans.overrides.is_empty());
    // Doors are not hosted by a curtain wall.
    let e = env
        .run("archbuilder.door.place", json!({"wall": wall}))
        .unwrap_err();
    assert_eq!(e.code(), "ab.model.curtain_insert");
    env.run(
        "archbuilder.curtain.panel",
        json!({"wall": wall, "cell": "nope", "panel": "solid"}),
    )
    .unwrap_err();
}

#[test]
fn generated_children_keep_their_identifiers_across_regeneration() {
    let (env, ty, wall) = setup();
    let ids = |env: &Env| -> Vec<(String, u128)> {
        derived(env, wall)
            .solids()
            .unwrap()
            .into_iter()
            .map(|s| (s.key, s.id))
            .collect()
    };
    let before = ids(&env);
    env.run(
        "archbuilder.curtain.type",
        json!({"type": ty, "panel": "solid"}),
    )
    .unwrap();
    let after = ids(&env);
    assert_eq!(before.len(), after.len());
    for (k, id) in &before {
        assert_eq!(
            after.iter().find(|(a, _)| a == k).map(|(_, i)| *i),
            Some(*id)
        );
    }
}

#[test]
fn a_curtain_wall_inside_a_basic_wall_cuts_an_opening_that_follows_it() {
    let (env, _ty, _wall) = setup();
    let host = env.wall([0.0, 10.0], [8.0, 10.0], json!({"height": 3.0}));
    let ty = env.type_named(TypeKind::Wall, "CW");
    let cw = env.wall(
        [2.0, 10.0],
        [4.0, 10.0],
        json!({"type": ty.to_string(), "height": 2.0}),
    );
    let volume = |env: &Env| {
        let s = env.snapshot();
        archbuilder_model::insert::wall_solid(&s, &spatial::get(&s, host).unwrap())
            .unwrap()
            .unwrap()
            .volume()
    };
    let full = 8.0 * 0.2 * 3.0;
    assert!((volume(&env) - (full - 2.0 * 2.0 * 0.2)).abs() < 1e-6);
    // Moving the curtain wall moves the opening; no overlap warning is raised for the pair.
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [cw], "displacement": [2.0, 0.0]}),
    )
    .unwrap();
    assert!((volume(&env) - (full - 2.0 * 2.0 * 0.2)).abs() < 1e-6);
    env.document.undo().wait().unwrap();
    env.document.undo().wait().unwrap();
    assert!((volume(&env) - full).abs() < 1e-6);
}

#[test]
fn a_corner_of_two_curtain_walls_gets_one_corner_mullion() {
    let (env, ty, a) = setup();
    let b = env.wall(
        [3.0, 0.0],
        [3.0, 3.0],
        json!({"type": ty.to_string(), "height": 3.0}),
    );
    let corners = |w| {
        derived(&env, w)
            .mullions
            .iter()
            .filter(|m| m.role == archbuilder_model::curtain::MullionRole::Corner)
            .count()
    };
    assert_eq!(corners(a) + corners(b), 1);
}

fn glass_volume(d: &Derived) -> f64 {
    d.solids()
        .unwrap()
        .iter()
        .filter(|s| s.class == archbuilder_model::curtain::Class::Glass)
        .map(|s| s.solid.volume())
        .sum()
}

#[test]
fn a_turned_grid_leans_its_lines_cuts_its_panels_to_the_wall_and_undoes() {
    let env = Env::new();
    let ty = curtain_type(
        &env,
        json!({"name": "Turned",
               "vertical": {"kind": "fixed_distance", "value": 1.0, "angle": "20"},
               "horizontal": {"kind": "fixed_number", "value": 1, "angle": "-10"}}),
    );
    let wall = env.wall(
        [0.0, 0.0],
        [4.0, 0.0],
        json!({"type": ty.to_string(), "height": 3.0}),
    );
    let d = derived(&env, wall);
    // The cells partition the wall.
    let cells: f64 = d
        .layout
        .cells
        .iter()
        .map(|c| {
            let ring = c.outline.as_ref().unwrap();
            (0..ring.len())
                .map(|k| {
                    let (p, q) = (ring[k], ring[(k + 1) % ring.len()]);
                    p[0] * q[1] - q[0] * p[1]
                })
                .sum::<f64>()
                .abs()
                / 2.0
        })
        .sum();
    assert!((cells - 12.0).abs() < 1e-9);
    // A vertical line turned counter-clockwise leans toward the start as it rises.
    let line = &d.layout.vertical[1];
    assert!(line.slope < 0.0 && (line.slope + 20f64.to_radians().tan()).abs() < 1e-12);
    // The glass is the panels' area through the thickness; the bars lean, so they are longer
    // than the stretch they cover.
    let area: f64 = d.panels.iter().map(|p| p.area()).sum();
    assert!(area > 0.0 && area < 12.0);
    assert!((glass_volume(&d) - area * d.ty.panel_thickness).abs() < 1e-6);
    assert!(
        d.mullions
            .iter()
            .any(|m| m.vertical && m.slope != 0.0 && m.length() > m.span.1 - m.span.0)
    );
    assert!(d.solids().unwrap().iter().all(|s| s.solid.volume() > 0.0));
    // The glass lies inside the wall's extent.
    for s in d
        .solids()
        .unwrap()
        .iter()
        .filter(|s| s.class == archbuilder_model::curtain::Class::Glass)
    {
        assert!(s.solid.vertices().iter().all(|v| {
            (-1e-9..=4.0 + 1e-9).contains(&v.x)
                && (-1e-9..=3.0 + 1e-9).contains(&v.z)
                && v.y.abs() < 0.1
        }));
    }
    // Turning the grid is one undo step.
    env.run(
        "archbuilder.curtain.type",
        json!({"type": ty.to_string(), "vertical": {"angle": "0"}, "horizontal": {"angle": "0"}}),
    )
    .unwrap();
    assert!(
        derived(&env, wall)
            .layout
            .cells
            .iter()
            .all(|c| c.outline.is_none())
    );
    env.document.undo().wait().unwrap();
    assert!(derived(&env, wall).layout.vertical[1].slope < 0.0);
}

#[test]
fn an_angle_past_the_limit_or_parallel_families_are_refused() {
    let env = Env::new();
    let e = env
        .run(
            "archbuilder.curtain.type",
            json!({"vertical": {"angle": "75"}}),
        )
        .unwrap_err();
    assert_eq!(e.code(), "ab.model.curtain_rule");
    // 45 degrees counter-clockwise for the verticals and the horizontals alike makes them parallel.
    let e = env
        .run(
            "archbuilder.curtain.type",
            json!({"vertical": {"angle": "-45"}, "horizontal": {"angle": "45"}}),
        )
        .unwrap_err();
    assert_eq!(e.code(), "ab.model.curtain_rule");
}

#[test]
fn equal_division_rules_take_justification_and_offset_and_undo() {
    let env = Env::new();
    let ty = curtain_type(
        &env,
        json!({"name": "Offset",
               "vertical": {"kind": "fixed_number", "value": 2, "justify": "start", "offset": 0.5},
               "horizontal": {"kind": "max_spacing", "value": 1.5, "justify": "end", "offset": 0.3}}),
    );
    let wall = env.wall(
        [0.0, 0.0],
        [5.0, 0.0],
        json!({"type": ty.to_string(), "height": 3.0}),
    );
    let at = |lines: &[archbuilder_model::curtain::Line]| -> Vec<f64> {
        lines.iter().map(|l| (l.at * 1e6).round() / 1e6).collect()
    };
    let d = derived(&env, wall);
    // A line at the offset, then two more dividing the rest in thirds.
    assert_eq!(at(&d.layout.vertical), vec![0.0, 0.5, 2.0, 3.5, 5.0]);
    // Up to the end less the offset (2.7 m): two cells of at most 1.5 m, and a line at 2.7.
    assert_eq!(at(&d.layout.horizontal), vec![0.0, 1.35, 2.7, 3.0]);
    assert_eq!(d.panels.len(), 12);
    // Changing the rule is one undo step and the grid regenerates.
    env.run(
        "archbuilder.curtain.type",
        json!({"type": ty.to_string(), "vertical": {"offset": 0}}),
    )
    .unwrap();
    let thirds = at(&derived(&env, wall).layout.vertical);
    assert_eq!(thirds, vec![0.0, 1.666_667, 3.333_333, 5.0]);
    env.document.undo().wait().unwrap();
    assert_eq!(derived(&env, wall).layout.vertical.len(), 5);
}

fn four_meter_wall(env: &Env) -> ObjectId {
    let ty = curtain_type(
        env,
        json!({"name": "Four",
               "vertical": {"kind": "fixed_distance", "value": 1.0},
               "horizontal": {"kind": "fixed_number", "value": 1}}),
    );
    env.wall(
        [0.0, 0.0],
        [4.0, 0.0],
        json!({"type": ty.to_string(), "height": 3.0}),
    )
}

fn edited(env: &Env, wall: ObjectId) -> (Vec<archbuilder_model::curtain::GridEdit>, Vec<String>) {
    let s = env.snapshot();
    let object = spatial::get(&s, wall).unwrap();
    (
        archbuilder_model::curtain::GridEdit::list(&object.values),
        archbuilder_model::curtain::CellOverride::list(&object.values)
            .into_iter()
            .map(|o| o.cell)
            .collect(),
    )
}

#[test]
fn splitting_a_curtain_wall_carries_its_edits_where_unambiguous_and_reports_the_rest() {
    let env = Env::new();
    let wall = four_meter_wall(&env);
    for at in [0.5, 3.5] {
        env.run(
            "archbuilder.curtain.grid_add",
            json!({"wall": wall, "direction": "vertical", "at": at}),
        )
        .unwrap();
    }
    // A horizontal line along the last meter only.
    env.run(
        "archbuilder.curtain.grid_add",
        json!({"wall": wall, "direction": "horizontal", "at": 0.5, "from": 3.0, "until": 4.0}),
    )
    .unwrap();
    for cell in ["start-a1.base-n1", "s3-a2.base-a3", "a2-end.base-a3"] {
        env.run(
            "archbuilder.curtain.panel",
            json!({"wall": wall, "cell": cell, "panel": "solid"}),
        )
        .unwrap();
    }
    let split = env
        .run("archbuilder.wall.split", json!({"at": [2.5, 0.0]}))
        .unwrap()
        .value;
    let second: ObjectId = split["created"][0].as_str().unwrap().parse().unwrap();
    // The first piece keeps the line and the override that lie in it.
    let (edits, cells) = edited(&env, wall);
    assert_eq!(edits.iter().map(|e| e.key).collect::<Vec<_>>(), vec![1]);
    assert_eq!(cells, vec!["start-a1.base-n1"]);
    // The second piece gets its line moved to its own start and the horizontal line's segment.
    let (edits, cells) = edited(&env, second);
    assert_eq!(edits.iter().map(|e| e.key).collect::<Vec<_>>(), vec![2, 3]);
    assert!((edits[0].at - 1.0).abs() < 1e-9);
    let span = edits[1].span.unwrap();
    assert!((span.0 - 0.5).abs() < 1e-9 && (span.1 - 1.5).abs() < 1e-9);
    assert_eq!(cells, vec!["a2-end.base-a3"]);
    assert_eq!(
        derived(&env, second)
            .panels
            .iter()
            .filter(|p| p.overridden)
            .count(),
        1
    );
    // The override of the cell that the second piece's own grid dissolved is reported, not
    // rebound to a neighbor.
    let lost = split["unkept"].as_array().unwrap();
    assert_eq!(lost.len(), 1);
    assert_eq!(lost[0][1], "cell s3-a2.base-a3");
    // One undo gives the single edited wall back.
    env.document.undo().wait().unwrap();
    let (edits, cells) = edited(&env, wall);
    assert_eq!((edits.len(), cells.len()), (3, 3));
    assert!(spatial::get(&env.snapshot(), second).is_none());
}

#[test]
fn merging_curtain_walls_carries_the_edits_of_both_with_fresh_keys() {
    let env = Env::new();
    let wall = four_meter_wall(&env);
    for at in [0.5, 3.5] {
        env.run(
            "archbuilder.curtain.grid_add",
            json!({"wall": wall, "direction": "vertical", "at": at}),
        )
        .unwrap();
    }
    for cell in ["start-a1.base-n1", "a2-end.base-n1"] {
        env.run(
            "archbuilder.curtain.panel",
            json!({"wall": wall, "cell": cell, "panel": "solid"}),
        )
        .unwrap();
    }
    let split = env
        .run("archbuilder.wall.split", json!({"at": [2.5, 0.0]}))
        .unwrap()
        .value;
    assert!(split.get("unkept").is_none());
    let merged = env
        .run(
            "archbuilder.wall.merge",
            json!({"first": [1.0, 0.0], "other": [3.0, 0.0]}),
        )
        .unwrap()
        .value;
    assert!(merged.get("unkept").is_none());
    let (edits, cells) = edited(&env, wall);
    // The second wall's key 2 is raised past the first wall's, so the two lines keep their places.
    let at: Vec<_> = edits
        .iter()
        .map(|e| (e.key, (e.at * 1e6).round() / 1e6))
        .collect();
    assert_eq!(at, vec![(1, 0.5), (3, 3.5)]);
    assert_eq!(cells, vec!["start-a1.base-n1", "a3-end.base-n1"]);
    let d = derived(&env, wall);
    assert_eq!(d.panels.iter().filter(|p| p.overridden).count(), 2);
    assert!(d.layout.orphans.edits.is_empty() && d.layout.orphans.overrides.is_empty());
    env.document.undo().wait().unwrap();
    assert!(spatial::get(&env.snapshot(), wall).is_some());
    assert_eq!(edited(&env, wall).0.len(), 1);
}

#[test]
fn a_pinned_curtain_wall_takes_grid_edits_and_panel_overrides_but_cannot_move_or_split() {
    let env = Env::new();
    let wall = four_meter_wall(&env);
    env.run("archbuilder.element.pin", json!({"objects": [wall]}))
        .unwrap();
    // The grid divides and fills the wall without moving or reshaping it.
    env.run(
        "archbuilder.curtain.grid_add",
        json!({"wall": wall, "direction": "vertical", "at": 0.5}),
    )
    .unwrap();
    env.run(
        "archbuilder.curtain.panel",
        json!({"wall": wall, "cell": "start-a1.base-n1", "panel": "solid"}),
    )
    .unwrap();
    assert_eq!(edited(&env, wall).0.len(), 1);
    assert!(derived(&env, wall).panels[0].overridden);
    // Its place and shape stay protected.
    let moved = env
        .run(
            "archbuilder.modify.move",
            json!({"objects": [wall], "displacement": [1.0, 0.0]}),
        )
        .unwrap_err();
    assert_eq!(moved.code(), "ab.model.pinned");
    let split = env
        .run("archbuilder.wall.split", json!({"at": [2.5, 0.0]}))
        .unwrap_err();
    assert_eq!(split.code(), "ab.model.pinned");
    // The edit is one undo step like any other.
    env.document.undo().wait().unwrap();
    assert!(!derived(&env, wall).panels[0].overridden);
}

#[test]
fn the_schedule_command_lists_panels_and_mullions_and_changes_nothing() {
    let (env, ty, wall) = setup();
    let plain = env.wall([0.0, 5.0], [3.0, 5.0], json!({}));
    let cell = derived(&env, wall).layout.cells[0].key.clone();
    env.run(
        "archbuilder.curtain.panel",
        json!({"wall": wall, "cell": cell, "panel": "solid"}),
    )
    .unwrap();
    let rows = env
        .run("archbuilder.curtain.schedule", json!({}))
        .unwrap()
        .value;
    let one = &rows["walls"][0];
    assert_eq!(one["id"], wall.to_string());
    let panels = one["panels"].as_array().unwrap();
    assert_eq!(panels.len(), 6);
    assert_eq!(panels[0]["key"], cell);
    assert_eq!(panels[0]["kind"], "solid");
    assert_eq!(panels[0]["overridden"], true);
    assert_eq!(panels[1]["kind"], "glazed");
    // Each row equals what the derived wall says, and the totals are the sums of the rows.
    let d = derived(&env, wall);
    let area: f64 = panels.iter().map(|p| p["area"].as_f64().unwrap()).sum();
    assert!((area - d.panels.iter().map(|p| p.area()).sum::<f64>()).abs() < 1e-9);
    assert!((one["panel_area"].as_f64().unwrap() - area).abs() < 1e-9);
    let mullions = one["mullions"].as_array().unwrap();
    assert_eq!(mullions.len(), d.mullions.len());
    let length: f64 = mullions.iter().map(|m| m["length"].as_f64().unwrap()).sum();
    assert!((one["mullion_length"].as_f64().unwrap() - length).abs() < 1e-9);
    assert!(mullions.iter().all(|m| m["profile"] == "rectangular"));
    // The cell keys stay when the grid regenerates, so a schedule can follow its rows.
    env.run(
        "archbuilder.curtain.type",
        json!({"type": ty.to_string(), "vertical": {"value": 1.2}}),
    )
    .unwrap();
    let again = env
        .run("archbuilder.curtain.schedule", json!({"wall": wall}))
        .unwrap()
        .value;
    assert_eq!(again["walls"][0]["panels"][0]["key"], cell);
    // A wall that is not a curtain wall is refused; reading the schedule is not an undo step.
    let e = env
        .run("archbuilder.curtain.schedule", json!({"wall": plain}))
        .unwrap_err();
    assert_eq!(e.code(), "ab.model.curtain_not_curtain");
    env.document.undo().wait().unwrap();
    assert_eq!(derived(&env, wall).layout.vertical[1].at, 1.0);
}
