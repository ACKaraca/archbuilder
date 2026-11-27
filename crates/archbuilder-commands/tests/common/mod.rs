//! The environment of the wall shape tests: a document with the commands, and helpers.

#![allow(dead_code, clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::spatial;
use archbuilder_model::types::WALL;
use serde_json::{Value as Json, json};

pub struct Env {
    pub document: Document,
    pub session: Session,
    pub registry: arc_command::Registry,
}

impl Env {
    pub fn new() -> Self {
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

    pub fn run(&self, command: &str, params: Json) -> arc_base::Result<Outcome> {
        let invocation = Invocation {
            seed: 7,
            ..Invocation::new(command, params)
        };
        Invoker::new(&self.registry).invoke(&self.document, &self.session, invocation)
    }

    pub fn snapshot(&self) -> Snapshot {
        self.document.snapshot()
    }

    pub fn dump(&self) -> String {
        let snapshot = self.snapshot();
        let mut lines: Vec<String> = snapshot
            .iter()
            .map(|(_, o)| format!("{} {:?} {:?}", o.id, o.header.owner, o.values))
            .collect();
        lines.sort();
        lines.join("\n")
    }
}

pub fn walls(env: &Env) -> Vec<Arc<arc_doc::Object>> {
    spatial::objects(&env.snapshot(), WALL)
}

pub fn volume(env: &Env, id: ObjectId) -> f64 {
    let snapshot = env.snapshot();
    let object = spatial::get(&snapshot, id).unwrap();
    archbuilder_model::cleanup::wall_net_volume(&snapshot, &object).unwrap()
}

pub fn arc(env: &Env) -> ObjectId {
    // A quarter circle of radius 5 about the origin, from (5, 0) to (0, 5), counter-clockwise.
    let made = env
        .run(
            "archbuilder.wall.create",
            json!({"start": [5.0, 0.0], "end": [0.0, 5.0], "through": [3.5355339059327378, 3.5355339059327378],
                   "thickness": 0.3, "height": 3.0}),
        )
        .unwrap()
        .value;
    made["wall"].as_str().unwrap().parse().unwrap()
}
