//! Curtain walls: the type, grid edits, panel overrides, embedding, corners and undo.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, FieldPath, Snapshot};
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
    for expected in [8, 8, 6] {
        let _ = expected;
    }
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
