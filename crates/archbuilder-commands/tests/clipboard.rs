//! The clipboard and `archbuilder.modify.copy` for every element kind besides walls, floors and
//! their inserts: railings (alone, and hosted on a ramp or stair, which follow their host), ramps,
//! grids (the label increments), reference planes, roofs by extrusion and curtain walls (their
//! grid edits and panel overrides go with them), pasted after a displacement, into another
//! document, and undone in one step.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::spatial;
use archbuilder_model::types::{
    self, GRID, RAILING, RAMP, REFERENCE_PLANE, ROOF_EXTRUSION, STAIR, WALL,
};
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

    fn ids(&self, key: &str) -> Vec<ObjectId> {
        spatial::objects(&self.snapshot(), key)
            .iter()
            .map(|o| o.id)
            .collect()
    }

    fn storey(&self) -> ObjectId {
        let s = self.snapshot();
        spatial::storeys(&s, spatial::buildings(&s)[0].id)[0]
            .storey
            .id
    }

    fn dump(&self) -> String {
        let mut lines: Vec<String> = self
            .snapshot()
            .iter()
            .map(|(_, o)| format!("{} {:?} {:?}", o.id, o.header.owner, o.values))
            .collect();
        lines.sort();
        lines.join("\n")
    }

    /// Pastes the copy of `objects` displaced by 10 m along x: the objects created.
    fn paste(&self, objects: &[ObjectId]) -> Vec<ObjectId> {
        let made = self
            .run(
                "archbuilder.clipboard.paste",
                json!({"objects": objects, "displacement": [10.0, 0.0]}),
            )
            .unwrap()
            .value;
        made["created"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().parse().unwrap())
            .collect()
    }

    /// Pastes `content` into this document's lowest storey, displaced by 10 m along x.
    fn paste_content(&self, content: &Json) -> usize {
        let made = self
            .run(
                "archbuilder.clipboard.paste",
                json!({"content": content, "displacement": [10.0, 0.0], "storey": self.storey()}),
            )
            .unwrap()
            .value;
        made["created"].as_array().unwrap().len()
    }

    fn undone(&self, before: &str) {
        self.document.undo().wait().unwrap();
        assert_eq!(self.dump(), before);
    }
}

fn kind(s: &Snapshot, id: ObjectId) -> String {
    spatial::type_of(s, id).unwrap()
}

fn railing(s: &Snapshot, id: ObjectId) -> archbuilder_model::railing::Railing {
    archbuilder_model::railing::Railing::from_values(&spatial::get(s, id).unwrap().values).unwrap()
}

#[test]
fn a_railing_by_path_pastes_moved_and_a_hosted_one_only_with_its_host() {
    let env = Env::new();
    env.run(
        "archbuilder.railing.create",
        json!({"points": [[0.0, 0.0], [3.0, 0.0], [3.0, 2.0]]}),
    )
    .unwrap();
    let alone = env.ids(RAILING)[0];
    let before = env.dump();
    let made = env.paste(&[alone]);
    assert_eq!(made.len(), 1);
    let s = env.snapshot();
    let copy = railing(&s, made[0]);
    assert_eq!(copy.path[0].point[..2], [10.0, 0.0]);
    assert_eq!(copy.path[2].point[..2], [13.0, 2.0]);
    env.undone(&before);

    // A ramp takes its railings along; a hosted railing alone is not copied.
    env.run(
        "archbuilder.ramp.create",
        json!({"start": [0.0, 5.0], "to": [6.0, 5.0]}),
    )
    .unwrap();
    let ramp = env.ids(RAMP)[0];
    let s = env.snapshot();
    let hosted: Vec<ObjectId> = env
        .ids(RAILING)
        .into_iter()
        .filter(|r| railing(&s, *r).host == Some(ramp))
        .collect();
    assert_eq!(hosted.len(), 2);
    let err = env
        .run(
            "archbuilder.clipboard.paste",
            json!({"objects": [hosted[0]], "displacement": [10.0, 0.0]}),
        )
        .unwrap_err();
    assert_eq!(err.code(), "ab.cmd.nothing_selected");
    let made = env.paste(&[ramp]);
    let s = env.snapshot();
    let new_ramp = made
        .iter()
        .copied()
        .find(|id| kind(&s, *id) == RAMP)
        .unwrap();
    let on_copy = made
        .iter()
        .filter(|id| kind(&s, **id) == RAILING && railing(&s, **id).host == Some(new_ramp))
        .count();
    assert_eq!((made.len(), on_copy), (3, 2));
    let r = archbuilder_model::ramp::Ramp::from_values(&spatial::get(&s, new_ramp).unwrap().values)
        .unwrap();
    assert_eq!(r.start, [10.0, 5.0]);
}

#[test]
fn a_stair_takes_its_hosted_railings_in_a_paste_and_in_a_copy() {
    let env = Env::new();
    let made = env
        .run(
            "archbuilder.stair.create",
            json!({"start": [0.0, 0.0], "to": [1.0, 0.0]}),
        )
        .unwrap()
        .value;
    let stair: ObjectId = made["stair"].as_str().unwrap().parse().unwrap();
    env.run(
        "archbuilder.railing.create",
        json!({"host": stair, "side": "left"}),
    )
    .unwrap();
    let made = env.paste(&[stair]);
    let s = env.snapshot();
    let new_stair = made
        .iter()
        .copied()
        .find(|id| kind(&s, *id) == STAIR)
        .unwrap();
    assert!(
        made.iter()
            .any(|id| kind(&s, *id) == RAILING && railing(&s, *id).host == Some(new_stair))
    );
    let copied = env
        .run(
            "archbuilder.modify.copy",
            json!({"objects": [stair], "displacement": [0.0, 10.0]}),
        )
        .unwrap()
        .value;
    assert_eq!(copied["created"].as_array().unwrap().len(), 2);
}

#[test]
fn grids_and_reference_planes_paste_with_the_next_label_and_name() {
    let env = Env::new();
    env.run(
        "archbuilder.grid.create",
        json!({"start": [0.0, 0.0], "end": [0.0, 8.0], "label": "A"}),
    )
    .unwrap();
    env.run(
        "archbuilder.reference_plane.create",
        json!({"start": [0.0, 0.0], "end": [8.0, 0.0]}),
    )
    .unwrap();
    let all = [env.ids(GRID), env.ids(REFERENCE_PLANE)].concat();
    let before = env.dump();
    let made = env.paste(&all);
    assert_eq!(made.len(), 2);
    let s = env.snapshot();
    let grid = made.iter().find(|id| kind(&s, **id) == GRID).unwrap();
    let object = spatial::get(&s, *grid).unwrap();
    assert_eq!(types::text(&object.values, 0), Some("B"));
    let g = archbuilder_model::datum::Grid::from_values(&object.values).unwrap();
    assert_eq!((g.start, g.end), ([10.0, 0.0], [10.0, 8.0]));
    env.undone(&before);

    // Into another document: the grids go to the building of the storey pasted into.
    let content = archbuilder_commands::clip::capture(&env.snapshot(), &all).unwrap();
    let other = Env::new();
    assert_eq!(other.paste_content(&content), 2);
    let s = other.snapshot();
    let building = spatial::buildings(&s)[0].id;
    assert_eq!(spatial::children(&s, GRID, building).len(), 1);
    assert_eq!(spatial::children(&s, REFERENCE_PLANE, building).len(), 1);
}

#[test]
fn a_roof_by_extrusion_pastes_copies_moves_and_mirrors_with_its_plane() {
    let env = Env::new();
    env.run(
        "archbuilder.roof_extrusion.create",
        json!({"plane": [[0.0, 0.0], [4.0, 0.0]], "profile": [[0.0, 3.0], [2.0, 4.0], [4.0, 3.0]],
               "end": 5.0}),
    )
    .unwrap();
    let roof = env.ids(ROOF_EXTRUSION)[0];
    let read = |env: &Env, id| {
        archbuilder_model::roof_extrusion::RoofExtrusion::from_values(
            &spatial::get(&env.snapshot(), id).unwrap().values,
        )
        .unwrap()
    };
    let before = env.dump();
    let made = env.paste(&[roof]);
    assert_eq!(made.len(), 1);
    assert_eq!(read(&env, made[0]).origin, [10.0, 0.0]);
    env.undone(&before);
    // Copy and move are commands of the modify set too.
    env.run(
        "archbuilder.modify.copy",
        json!({"objects": [roof], "displacement": [0.0, 7.0]}),
    )
    .unwrap();
    assert_eq!(env.ids(ROOF_EXTRUSION).len(), 2);
    env.run(
        "archbuilder.modify.move",
        json!({"objects": [roof], "displacement": [1.0, 0.0]}),
    )
    .unwrap();
    assert_eq!(read(&env, roof).origin, [1.0, 0.0]);
    // A mirror across the plane's line turns the extrusion to the other side.
    env.run(
        "archbuilder.modify.mirror",
        json!({"objects": [roof], "axis": [[0.0, 0.0], [1.0, 0.0]], "keep": false}),
    )
    .unwrap();
    let r = read(&env, roof);
    assert!((r.start + 5.0).abs() < 1e-9 && r.end.abs() < 1e-9, "{r:?}");
    // Into another document: the roof takes that document's roof type.
    let content = archbuilder_commands::clip::capture(&env.snapshot(), &[roof]).unwrap();
    let other = Env::new();
    assert_eq!(other.paste_content(&content), 1);
}

#[test]
fn a_curtain_wall_pastes_with_its_grid_edits_and_panel_overrides() {
    let env = Env::new();
    let ty = env
        .run(
            "archbuilder.curtain.type",
            json!({"name": "CW", "vertical": {"kind": "fixed_distance", "value": 1.0},
                   "horizontal": {"kind": "fixed_number", "value": 1}}),
        )
        .unwrap()
        .value["type"]
        .as_str()
        .unwrap()
        .to_owned();
    let wall: ObjectId = env
        .run(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [3.0, 0.0], "type": ty, "height": 3.0}),
        )
        .unwrap()
        .value["wall"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let derived = |env: &Env, id| {
        let s = env.snapshot();
        archbuilder_model::curtain::Derived::of(&s, &spatial::get(&s, id).unwrap()).unwrap()
    };
    let cell = derived(&env, wall).layout.cells[0].key.clone();
    env.run(
        "archbuilder.curtain.panel",
        json!({"wall": wall, "cell": cell, "panel": "empty"}),
    )
    .unwrap();
    env.run(
        "archbuilder.curtain.grid_add",
        json!({"wall": wall, "direction": "vertical", "at": 0.5}),
    )
    .unwrap();
    let original = derived(&env, wall);
    let made = env.paste(&[wall]);
    assert_eq!(made.len(), 1);
    let copy = derived(&env, made[0]);
    assert_eq!(copy.panels.len(), original.panels.len());
    assert_eq!(
        copy.panels.iter().filter(|p| p.overridden).count(),
        original.panels.iter().filter(|p| p.overridden).count()
    );
    assert!(copy.layout.orphans.overrides.is_empty());
    assert_eq!(env.ids(WALL).len(), 2);
    // Into another document: still a curtain wall with its edits.
    let content = archbuilder_commands::clip::capture(&env.snapshot(), &[wall]).unwrap();
    let other = Env::new();
    assert_eq!(other.paste_content(&content), 1);
    let pasted = derived(&other, other.ids(WALL)[0]);
    assert_eq!(pasted.panels.len(), original.panels.len());
}

#[test]
fn a_curtain_door_panel_pastes_into_another_document_without_its_door_type() {
    let env = Env::new();
    let ty = env
        .run(
            "archbuilder.curtain.type",
            json!({"name": "CW", "vertical": {"kind": "fixed_distance", "value": 1.0},
                   "horizontal": {"kind": "fixed_number", "value": 1}}),
        )
        .unwrap()
        .value["type"]
        .as_str()
        .unwrap()
        .to_owned();
    let wall: ObjectId = env
        .run(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [3.0, 0.0], "type": ty, "height": 3.0}),
        )
        .unwrap()
        .value["wall"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let door = spatial::objects(&env.snapshot(), types::DOOR_TYPE)[0].id;
    let made = env
        .run("archbuilder.type.duplicate", json!({"type": door}))
        .unwrap()
        .value;
    let door_copy = made["type"].as_str().unwrap().to_owned();
    let s = env.snapshot();
    let cell = archbuilder_model::curtain::Derived::of(&s, &spatial::get(&s, wall).unwrap())
        .unwrap()
        .layout
        .cells[0]
        .key
        .clone();
    env.run(
        "archbuilder.curtain.panel",
        json!({"wall": wall, "cell": cell, "panel": "door", "type": door_copy}),
    )
    .unwrap();
    let content = archbuilder_commands::clip::capture(&env.snapshot(), &[wall]).unwrap();
    let other = Env::new();
    let r = other.run(
        "archbuilder.clipboard.paste",
        json!({"content": content, "displacement": [10.0, 0.0], "storey": other.storey()}),
    );
    // The duplicated door type is not in the other document: the panel keeps a plain door.
    assert!(r.is_ok(), "{r:?}");
}
