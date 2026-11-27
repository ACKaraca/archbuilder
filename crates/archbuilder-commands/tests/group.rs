//! Model groups (AB-PHASE-010 to AB-PHASE-013): creation from a closed selection, placement by
//! level mapping, exclusions, exceptions, the refusal of a direct edit that a regeneration would
//! overwrite, edit propagation, separation, the storey refusal and undo.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::group;
use archbuilder_model::spatial;
use archbuilder_model::types::{self, GROUP, GROUP_INSTANCE, STOREY, WALL};
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
        env.run("archbuilder.project.new", json!({}), &[]).unwrap();
        env
    }

    fn run(
        &self,
        command: &str,
        params: Json,
        selection: &[ObjectId],
    ) -> arc_base::Result<Outcome> {
        let invocation = Invocation {
            seed: 23,
            selection: selection.to_vec(),
            ..Invocation::new(command, params)
        };
        Invoker::new(&self.registry).invoke(&self.document, &self.session, invocation)
    }

    /// A command that must succeed; its returned value.
    fn ok(&self, command: &str, params: Json) -> Json {
        self.run(command, params, &[])
            .unwrap_or_else(|e| panic!("{command}: {e:?}"))
            .value
    }

    fn snapshot(&self) -> Snapshot {
        self.document.snapshot()
    }

    fn undo(&self) {
        assert!(self.document.undo().wait().unwrap().value);
    }

    /// The storeys of the document, from the ground up.
    fn storeys(&self) -> Vec<ObjectId> {
        let snap = self.snapshot();
        let mut found = spatial::objects(&snap, STOREY);
        found.sort_by(|a, b| {
            spatial::storey_elevation(&snap, a).total_cmp(&spatial::storey_elevation(&snap, b))
        });
        found.iter().map(|s| s.id).collect()
    }

    fn wall_on(&self, storey: ObjectId, start: [f64; 2], end: [f64; 2]) -> ObjectId {
        let made = self.ok(
            "archbuilder.wall.create",
            json!({"storey": storey.to_string(), "start": start, "end": end}),
        );
        parse(made["wall"].as_str().unwrap())
    }

    /// The walls, the doors and windows of the storey `storey` (generated ones included).
    fn count_on(&self, storey: ObjectId, key: &str) -> usize {
        spatial::objects(&self.snapshot(), key)
            .iter()
            .filter(|o| o.header.owner == Some(storey))
            .count()
    }

    fn instances(&self) -> Vec<ObjectId> {
        spatial::objects(&self.snapshot(), GROUP_INSTANCE)
            .iter()
            .map(|o| o.id)
            .collect()
    }

    /// Every element, its values included, as text: equal texts mean equal documents.
    fn dump(&self) -> String {
        let snap = self.snapshot();
        let mut out = String::new();
        for key in ELEMENTS_AND_GROUPS {
            out.push_str(&format!("{:?}", spatial::objects_raw(&snap, key)));
        }
        out
    }

    /// Runs `command`, checks that undo restores the exact prior document.
    fn undoes(&self, command: &str, params: Json, selection: &[ObjectId]) {
        let before = self.dump();
        self.run(command, params, selection)
            .unwrap_or_else(|e| panic!("{command}: {e:?}"));
        assert_ne!(self.dump(), before, "{command} changed the document");
        self.undo();
        assert_eq!(self.dump(), before, "undo of {command}");
    }
}

const ELEMENTS_AND_GROUPS: [&str; 7] = [
    WALL,
    types::DOOR,
    types::WINDOW,
    types::ROOM,
    types::COMPONENT,
    GROUP,
    GROUP_INSTANCE,
];

fn parse(text: &str) -> ObjectId {
    text.parse().unwrap()
}

fn ids(list: &[ObjectId]) -> Vec<String> {
    list.iter().map(ObjectId::to_string).collect()
}

fn all_walls(env: &Env) -> Vec<ObjectId> {
    spatial::objects(&env.snapshot(), WALL)
        .iter()
        .map(|w| w.id)
        .collect()
}

/// The definition created from every wall and door of the document.
fn group_all(env: &Env, name: &str) -> ObjectId {
    let mut selection = all_walls(env);
    selection.extend(
        spatial::objects(&env.snapshot(), types::DOOR)
            .iter()
            .map(|d| d.id),
    );
    let made = env.ok(
        "archbuilder.group.create",
        json!({"name": name, "ids": ids(&selection)}),
    );
    parse(made["group"].as_str().unwrap())
}

fn place(env: &Env, def: ObjectId, storey: ObjectId, origin: [f64; 2]) -> ObjectId {
    let placed = env.ok(
        "archbuilder.group.place",
        json!({"group": def.to_string(), "storey": storey.to_string(), "origin": origin}),
    );
    parse(placed["instance"].as_str().unwrap())
}

/// A four-wall room on `storey` at `origin`, with a door in its first wall.
fn room_on(env: &Env, storey: ObjectId, origin: [f64; 2]) -> Vec<ObjectId> {
    let [x, y] = origin;
    let corners = [[x, y], [x + 4.0, y], [x + 4.0, y + 3.0], [x, y + 3.0]];
    let walls: Vec<ObjectId> = (0..4)
        .map(|k| env.wall_on(storey, corners[k], corners[(k + 1) % 4]))
        .collect();
    env.ok(
        "archbuilder.door.place",
        json!({"wall": walls[0].to_string(), "distance": 1.5}),
    );
    walls
}

#[test]
fn a_wall_takes_its_door_and_the_definition_stays_out_of_the_building() {
    let env = Env::new();
    let ground = env.storeys()[0];
    let walls = room_on(&env, ground, [0.0, 0.0]);
    let made = env.ok(
        "archbuilder.group.create",
        json!({"ids": [walls[0].to_string()]}),
    );
    assert_eq!(made["members"], 2, "the wall and its door");
    let snap = env.snapshot();
    assert_eq!(
        spatial::objects(&snap, WALL).len(),
        4,
        "the building keeps its walls"
    );
    assert_eq!(
        spatial::objects_raw(&snap, WALL).len(),
        5,
        "the definition holds its own wall"
    );
    assert_eq!(
        spatial::objects(&snap, GROUP).len(),
        1,
        "the definition is a project record"
    );
}

#[test]
fn an_unsupported_kind_is_refused_and_a_door_pulls_in_its_wall() {
    let env = Env::new();
    let ground = env.storeys()[0];
    room_on(&env, ground, [0.0, 0.0]);
    let floor = env.ok(
        "archbuilder.floor.create",
        json!({"points": [[1.0, 1.0], [3.0, 1.0], [3.0, 2.0], [1.0, 2.0]]}),
    );
    let floor_id = parse(floor["floor"].as_str().unwrap());
    let refused = env.run(
        "archbuilder.group.create",
        json!({"ids": [floor_id.to_string()]}),
        &[],
    );
    assert!(refused.is_err(), "a floor is not a group member");
    let door = spatial::objects(&env.snapshot(), types::DOOR)[0].id;
    let made = env.ok(
        "archbuilder.group.create",
        json!({"ids": [door.to_string()]}),
    );
    assert_eq!(
        made["members"], 2,
        "the door takes its wall, and the wall's openings"
    );
}

#[test]
fn placing_maps_each_level_to_a_storey_and_regeneration_is_deterministic() {
    let env = Env::new();
    let storeys = env.storeys();
    let (ground, first) = (storeys[0], storeys[1]);
    room_on(&env, ground, [0.0, 0.0]);
    env.wall_on(first, [0.0, 0.0], [4.0, 0.0]);
    let def = group_all(&env, "Unit");
    let snap = env.snapshot();
    let definition = group::definition(&snap, def).unwrap();
    assert_eq!(
        definition.levels,
        vec![0.0, 3.0],
        "the second level is 3 m above the first"
    );
    assert_eq!(group::members(&snap, def).len(), 6, "five walls and a door");

    let instance = place(&env, def, ground, [20.0, 0.0]);
    assert_eq!(
        env.count_on(ground, WALL),
        4 + 4,
        "four walls of the room and four placed"
    );
    assert_eq!(
        env.count_on(first, WALL),
        1 + 1,
        "the wall of the upper level and its placed copy"
    );
    let before = env.dump();
    env.ok(
        "archbuilder.group.exclude",
        json!({"instance": instance.to_string(), "elements": [], "include": true}),
    );
    assert_eq!(
        env.dump(),
        before,
        "a regeneration without a change keeps every identifier"
    );
}

#[test]
fn an_exclusion_and_an_exception_belong_to_one_instance() {
    let env = Env::new();
    let ground = env.storeys()[0];
    room_on(&env, ground, [0.0, 0.0]);
    let def = group_all(&env, "Unit");
    let a = place(&env, def, ground, [20.0, 0.0]);
    let b = place(&env, def, ground, [40.0, 0.0]);
    assert_eq!(env.instances().len(), 2);
    assert_eq!(
        spatial::objects(&env.snapshot(), WALL).len(),
        12,
        "four walls and two copies of them"
    );

    let members = group::members(&env.snapshot(), def);
    let first_wall = members.iter().find(|m| m.kind == WALL).unwrap().id;
    let second_wall = members.iter().filter(|m| m.kind == WALL).nth(1).unwrap().id;
    let gen_a = group::member_id(a, first_wall);
    env.ok(
        "archbuilder.group.exclude",
        json!({"instance": a.to_string(), "elements": [gen_a.to_string()]}),
    );
    assert!(
        spatial::get(&env.snapshot(), gen_a).is_none(),
        "excluded in A"
    );
    assert_eq!(
        spatial::objects(&env.snapshot(), WALL).len(),
        11,
        "B keeps its walls"
    );

    let gen_b = group::member_id(b, second_wall);
    env.ok(
        "archbuilder.group.set_exception",
        json!({"instance": b.to_string(), "element": gen_b.to_string(), "kind": "name", "text": "Party wall"}),
    );
    let renamed = spatial::get(&env.snapshot(), gen_b).unwrap();
    assert_eq!(types::text(&renamed.values, 0), Some("Party wall"));
    env.ok(
        "archbuilder.group.set_exception",
        json!({"instance": b.to_string(), "element": gen_b.to_string(), "kind": "name", "clear": true}),
    );
    let restored = spatial::get(&env.snapshot(), gen_b).unwrap();
    assert_ne!(
        types::text(&restored.values, 0),
        Some("Party wall"),
        "the generated name is back"
    );
}

#[test]
fn a_direct_edit_is_refused_by_a_regeneration_and_edit_group_propagates_it() {
    let env = Env::new();
    let ground = env.storeys()[0];
    room_on(&env, ground, [0.0, 0.0]);
    let def = group_all(&env, "Unit");
    let a = place(&env, def, ground, [20.0, 0.0]);
    let b = place(&env, def, ground, [40.0, 0.0]);
    let members = group::members(&env.snapshot(), def);
    let walls: Vec<ObjectId> = members
        .iter()
        .filter(|m| m.kind == WALL)
        .map(|m| m.id)
        .collect();
    let gen_a0 = group::member_id(a, walls[0]);
    let gen_b0 = group::member_id(b, walls[0]);
    env.ok(
        "archbuilder.modify.move",
        json!({"objects": [gen_a0.to_string()], "displacement": [0.0, 0.5]}),
    );
    let refused = env.run(
        "archbuilder.group.exclude",
        json!({"instance": a.to_string(), "elements": [group::member_id(a, walls[1]).to_string()]}),
        &[],
    );
    assert!(refused.is_err(), "the moved element would be overwritten");

    env.ok("archbuilder.group.edit", json!({"instance": a.to_string()}));
    let snap = env.snapshot();
    let in_a = spatial::get(&snap, gen_a0).unwrap();
    let in_b = spatial::get(&snap, gen_b0).unwrap();
    assert_eq!(
        types::meters(&in_a.values, types::wall::START_Y),
        types::meters(&in_b.values, types::wall::START_Y),
        "both instances carry the edit"
    );
}

#[test]
fn separating_gives_ordinary_elements_with_new_identifiers() {
    let env = Env::new();
    let ground = env.storeys()[0];
    room_on(&env, ground, [0.0, 0.0]);
    let def = group_all(&env, "Unit");
    let instance = place(&env, def, ground, [20.0, 0.0]);
    let separated = env.ok(
        "archbuilder.group.separate",
        json!({"instance": instance.to_string()}),
    );
    let pairs = separated["pairs"].as_array().unwrap();
    assert_eq!(pairs.len(), 5, "four walls and the door");
    assert!(env.instances().is_empty());
    let snap = env.snapshot();
    for pair in pairs {
        let new = parse(pair[1].as_str().unwrap());
        assert!(spatial::get(&snap, new).is_some(), "each copy exists");
    }
    let door_pair = pairs
        .iter()
        .find(|p| {
            spatial::type_of(&snap, parse(p[1].as_str().unwrap())).as_deref() == Some(types::DOOR)
        })
        .unwrap();
    let host = spatial::get(&snap, parse(door_pair[1].as_str().unwrap()))
        .unwrap()
        .header
        .owner;
    assert!(
        pairs
            .iter()
            .any(|p| Some(parse(p[1].as_str().unwrap())) == host),
        "the door keeps a copy of its wall as host"
    );
}

#[test]
fn a_mapped_storey_cannot_be_removed_until_the_instance_is_separated() {
    let env = Env::new();
    let storeys = env.storeys();
    let (ground, first) = (storeys[0], storeys[1]);
    room_on(&env, ground, [0.0, 0.0]);
    env.wall_on(first, [0.0, 0.0], [4.0, 0.0]);
    let def = group_all(&env, "Unit");
    let instance = place(&env, def, ground, [20.0, 0.0]);
    let refused = env.run(
        "archbuilder.storey.delete",
        json!({"storey": first.to_string()}),
        &[],
    );
    let message = format!("{:?}", refused.err());
    assert!(
        message.contains("group_level_mapped"),
        "refused by the mapping: {message}"
    );
    env.ok(
        "archbuilder.group.separate",
        json!({"instance": instance.to_string()}),
    );
    let after = env.run(
        "archbuilder.storey.delete",
        json!({"storey": first.to_string()}),
        &[],
    );
    let after = format!("{:?}", after.err());
    assert!(
        !after.contains("group_level_mapped"),
        "the mapping no longer stands: {after}"
    );
}

#[test]
fn every_group_command_undoes_to_the_exact_prior_state() {
    let env = Env::new();
    let ground = env.storeys()[0];
    let walls = room_on(&env, ground, [0.0, 0.0]);
    let selection = ids(&walls);
    env.undoes(
        "archbuilder.group.create",
        json!({"name": "Unit", "ids": selection}),
        &[],
    );
    let def = group_all(&env, "Unit");
    env.undoes(
        "archbuilder.group.place",
        json!({"group": def.to_string(), "storey": ground.to_string(), "origin": [20.0, 0.0]}),
        &[],
    );
    let instance = place(&env, def, ground, [20.0, 0.0]);
    let child = group::member_id(instance, group::members(&env.snapshot(), def)[0].id);
    env.undoes(
        "archbuilder.group.exclude",
        json!({"instance": instance.to_string(), "elements": [child.to_string()]}),
        &[],
    );
    env.undoes(
        "archbuilder.group.set_exception",
        json!({"instance": instance.to_string(), "element": child.to_string(), "kind": "name", "text": "X"}),
        &[],
    );
    env.undoes(
        "archbuilder.group.separate",
        json!({"instance": instance.to_string()}),
        &[],
    );
    env.ok(
        "archbuilder.group.separate",
        json!({"instance": instance.to_string()}),
    );
    env.undoes(
        "archbuilder.group.delete",
        json!({"group": def.to_string()}),
        &[],
    );
}
