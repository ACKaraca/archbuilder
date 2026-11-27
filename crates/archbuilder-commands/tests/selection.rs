//! Selection sets and model statistics.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::selection;
use archbuilder_model::spatial;
use archbuilder_model::types::SELECTION_SET;
use serde_json::{Value as Json, json};

static SEEDS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(9_500);

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

    fn wall(&self, x: f64) -> String {
        self.run(
            "archbuilder.wall.create",
            json!({"start": [x, 0.0], "end": [x, 4.0]}),
        )
        .unwrap()
        .value["wall"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    fn members(&self, set: &str) -> Vec<String> {
        let s = self.document.snapshot();
        let object = spatial::get(&s, set.parse().unwrap()).unwrap();
        selection::members(&object.values)
            .iter()
            .map(ToString::to_string)
            .collect()
    }
}

fn make(env: &Env, name: &str, objects: &[&String]) -> String {
    env.run(
        "archbuilder.set.create",
        json!({"name": name, "objects": objects}),
    )
    .unwrap()
    .value["set"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn a_set_is_created_updated_renamed_and_undone() {
    let env = Env::new();
    let (a, b) = (env.wall(0.0), env.wall(5.0));
    let set = make(&env, "Exterior", &[&a]);
    assert_eq!(env.members(&set), vec![a.clone()]);
    env.run(
        "archbuilder.set.update",
        json!({"set": set, "add": [b, a], "remove": []}),
    )
    .unwrap();
    assert_eq!(env.members(&set).len(), 2);
    env.run(
        "archbuilder.set.rename",
        json!({"set": set, "name": "Shell"}),
    )
    .unwrap();
    // A second set with the same name is refused.
    assert!(
        env.run(
            "archbuilder.set.create",
            json!({"name": "shell", "objects": []})
        )
        .is_err()
    );
    // Only elements are members.
    let storey = spatial::objects(&env.document.snapshot(), archbuilder_model::types::STOREY)[0]
        .id
        .to_string();
    assert!(
        env.run(
            "archbuilder.set.update",
            json!({"set": set, "add": [storey]})
        )
        .is_err()
    );
    let name = |env: &Env| {
        let s = env.document.snapshot();
        let object = spatial::get(&s, set.parse().unwrap()).unwrap();
        archbuilder_model::types::text(&object.values, 0)
            .unwrap()
            .to_owned()
    };
    assert_eq!(name(&env), "Shell");
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(name(&env), "Exterior");
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(env.members(&set).len(), 1);
    assert!(env.document.undo().wait().unwrap().value);
    assert!(spatial::objects(&env.document.snapshot(), SELECTION_SET).is_empty());
}

#[test]
fn deleting_a_set_is_undoable_and_deleting_an_element_leaves_the_set() {
    let env = Env::new();
    let (a, b) = (env.wall(0.0), env.wall(5.0));
    let set = make(&env, "Walls", &[&a, &b]);
    env.run("archbuilder.wall.delete", json!({"walls": [a]}))
        .unwrap();
    assert_eq!(env.members(&set), vec![b.clone()]);
    env.run("archbuilder.set.delete", json!({"set": set}))
        .unwrap();
    assert!(spatial::objects(&env.document.snapshot(), SELECTION_SET).is_empty());
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(env.members(&set), vec![b]);
}

#[test]
fn selecting_a_set_selects_its_members_and_statistics_count_them() {
    let env = Env::new();
    let (a, b) = (env.wall(0.0), env.wall(5.0));
    let set = make(&env, "Walls", &[&a, &b]);
    let out = env
        .run("archbuilder.set.select", json!({"set": set}))
        .unwrap();
    assert_eq!(out.selection.map(|s| s.len()), Some(2));
    let stats = env
        .run("archbuilder.model.statistics", json!({}))
        .unwrap()
        .value;
    assert_eq!(stats["elements"], 2);
    assert_eq!(stats["by_kind"][0]["kind"], "archbuilder.wall");
    assert_eq!(stats["by_kind"][0]["count"], 2);
    assert_eq!(stats["by_storey"][0]["kinds"][0]["count"], 2);
    assert_eq!(stats["selection_sets"], 1);
    assert_eq!(stats["views"], 3);
}
