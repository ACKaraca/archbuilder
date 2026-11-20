//! The commands of the spatial structure on a document with the model's validator: what a new
//! project holds, what each command changes, that undo of each restores the exact prior state, and
//! that a refused change leaves the document unchanged.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, FieldPath, Snapshot, Value};
use archbuilder_model::spatial;
use archbuilder_model::types::{BUILDING, LEVEL, PROJECT, SITE, STOREY, level, meters, text};
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
        Self {
            document,
            session: Session::new(Surface::Ui, [Permission::Read, Permission::Edit]),
            registry: archbuilder_commands::registry().unwrap(),
        }
    }

    fn run(&self, command: &str, params: Json) -> arc_base::Result<Outcome> {
        let invocation = Invocation {
            seed: 7,
            ..Invocation::new(command, params)
        };
        Invoker::new(&self.registry).invoke(&self.document, &self.session, invocation)
    }

    fn ok(&self, command: &str, params: Json) -> Json {
        self.run(command, params).unwrap().value
    }

    fn id(value: &Json) -> ObjectId {
        value.as_str().unwrap().parse().unwrap()
    }

    fn snapshot(&self) -> Snapshot {
        self.document.snapshot()
    }

    /// Every object with its owner and values, in a form that compares.
    fn dump(&self) -> Vec<String> {
        let snapshot = self.snapshot();
        let mut lines: Vec<String> = snapshot
            .iter()
            .map(|(handle, o)| {
                let key = snapshot.type_key(handle).unwrap();
                format!(
                    "{key} {} {:?} {}",
                    o.id,
                    o.header.owner,
                    serde_json::to_string(&o.values).unwrap()
                )
            })
            .collect();
        lines.sort();
        lines
    }

    fn building(&self) -> ObjectId {
        spatial::buildings(&self.snapshot())[0].id
    }

    /// The names and elevations (meters) of the storeys of the building, bottom up.
    fn storeys(&self) -> Vec<(String, f64)> {
        spatial::storeys(&self.snapshot(), self.building())
            .iter()
            .map(|s| (text(&s.storey.values, 0).unwrap().to_owned(), s.elevation()))
            .collect()
    }

    fn storey_named(&self, name: &str) -> ObjectId {
        spatial::storeys(&self.snapshot(), self.building())
            .iter()
            .find(|s| text(&s.storey.values, 0) == Some(name))
            .unwrap()
            .storey
            .id
    }

    /// Runs `command`, checks that it changed the document, that undo restores the exact state
    /// and that redo restores the changed one.
    fn undoes(&self, command: &str, params: Json) {
        let before = self.dump();
        self.ok(command, params);
        let after = self.dump();
        assert_ne!(before, after, "{command} changed nothing");
        assert!(self.document.undo().wait().unwrap().value);
        assert_eq!(self.dump(), before, "undo of {command}");
        assert!(self.document.redo().wait().unwrap().value);
        assert_eq!(self.dump(), after, "redo of {command}");
    }
}

fn started() -> Env {
    let env = Env::new();
    env.ok("archbuilder.project.new", json!({}));
    env
}

#[test]
fn a_new_project_holds_the_default_structure_and_only_once() {
    let env = Env::new();
    let made = env.ok("archbuilder.project.new", json!({"name": "House"}));
    let snapshot = env.snapshot();
    for (key, count) in [
        (PROJECT, 1),
        (SITE, 1),
        (BUILDING, 1),
        (STOREY, 3),
        (LEVEL, 3),
    ] {
        assert_eq!(spatial::objects(&snapshot, key).len(), count, "{key}");
    }
    let project = spatial::project(&snapshot).unwrap();
    assert_eq!(text(&project.values, 0), Some("House"));
    assert_eq!(project.id, Env::id(&made["project"]));
    assert_eq!(
        env.storeys(),
        [
            ("Ground Floor".to_owned(), 0.0),
            ("Floor 1".to_owned(), 3.0),
            ("Floor 2".to_owned(), 6.0)
        ]
    );
    assert_eq!(
        env.run("archbuilder.project.new", json!({}))
            .unwrap_err()
            .code(),
        "ab.cmd.document_not_empty"
    );
}

#[test]
fn add_storey_puts_a_level_above_the_highest_and_selects_the_storey() {
    let env = started();
    let outcome = env.run("archbuilder.storey.add", json!({})).unwrap();
    let added = outcome.value;
    assert_eq!(env.storeys().last().unwrap(), &("Floor 3".to_owned(), 9.0));
    // The outcome hands the shell the new storey and its level as the selection.
    assert_eq!(
        outcome.selection,
        Some(vec![Env::id(&added["storey"]), Env::id(&added["level"])])
    );
    let level = spatial::get(&env.snapshot(), Env::id(&added["level"])).unwrap();
    assert_eq!(meters(&level.values, level::ELEVATION), Some(9.0));
    // A text with a unit is read by the foundation's unit parser.
    env.ok(
        "archbuilder.storey.add",
        json!({"name": "Roof", "elevation": "10.5 m"}),
    );
    assert_eq!(env.storeys().last().unwrap(), &("Roof".to_owned(), 10.5));
}

#[test]
fn every_command_undoes_to_the_exact_prior_state() {
    let env = started();
    let building = env.building();
    let floor1 = env.storey_named("Floor 1");
    env.undoes(
        "archbuilder.storey.add",
        json!({"name": "Attic", "elevation": 9}),
    );
    env.undoes(
        "archbuilder.storey.rename",
        json!({"storey": floor1.to_string(), "name": "First"}),
    );
    env.undoes(
        "archbuilder.level.set_elevation",
        json!({"level": floor1.to_string(), "elevation": "3500 mm"}),
    );
    env.undoes(
        "archbuilder.building.rename",
        json!({"building": building.to_string(), "name": "Main house"}),
    );
    env.undoes(
        "archbuilder.property.set",
        json!({"objects": [floor1.to_string()], "field": "elevation", "value": 4.0}),
    );
    env.undoes(
        "archbuilder.storey.delete",
        json!({"storey": floor1.to_string()}),
    );
}

#[test]
fn renaming_a_storey_renames_its_level_and_deleting_it_removes_both() {
    let env = started();
    let floor2 = env.storey_named("Floor 2");
    env.ok(
        "archbuilder.storey.rename",
        json!({"storey": floor2.to_string(), "name": "Top"}),
    );
    let level_names: Vec<String> = spatial::levels(&env.snapshot(), env.building())
        .iter()
        .map(|l| text(&l.values, 0).unwrap().to_owned())
        .collect();
    assert_eq!(level_names, ["Ground Floor", "Floor 1", "Top"]);
    env.ok(
        "archbuilder.storey.delete",
        json!({"storey": floor2.to_string()}),
    );
    assert_eq!(env.storeys().len(), 2);
    assert_eq!(spatial::levels(&env.snapshot(), env.building()).len(), 2);
}

#[test]
fn refused_changes_name_the_rule_and_leave_the_document_unchanged() {
    let env = started();
    let before = env.dump();
    let floor1 = env.storey_named("Floor 1").to_string();
    let refused = [
        (
            "archbuilder.storey.add",
            json!({"name": " ground floor "}),
            "ab.model.name_taken",
        ),
        (
            "archbuilder.storey.rename",
            json!({"storey": floor1, "name": "Floor 2"}),
            "ab.model.name_taken",
        ),
        (
            "archbuilder.level.set_elevation",
            json!({"level": floor1, "elevation": 1e9}),
            "doc.value.out_of_range",
        ),
        (
            "archbuilder.building.rename",
            json!({"building": floor1, "name": "X"}),
            "ab.cmd.kind_mismatch",
        ),
    ];
    for (command, params, code) in refused {
        let error = env.run(command, params).unwrap_err();
        assert_eq!(error.code(), code, "{command}");
    }
    assert_eq!(env.dump(), before);
}

#[test]
fn a_level_cannot_be_deleted_while_its_storey_uses_it() {
    let env = started();
    let storey = spatial::get(&env.snapshot(), env.storey_named("Floor 1")).unwrap();
    let level = spatial::level_of(&env.snapshot(), &storey).unwrap().id;
    let before = env.dump();
    let error = env
        .document
        .submit(move |txn| txn.delete(level))
        .wait()
        .unwrap_err();
    assert_eq!(error.code(), "doc.reference.delete_blocked");
    assert_eq!(env.dump(), before);
    // Setting a field of the storey record itself is not a way around the rule.
    let renamed = env
        .document
        .submit(move |txn| {
            txn.modify(
                storey.id,
                FieldPath::new("name"),
                Some(Value::String(" ".into())),
            )
        })
        .wait()
        .unwrap_err();
    assert_eq!(renamed.code(), "ab.model.name_empty");
}

#[test]
fn selecting_a_level_outline_selects_its_storey_and_the_last_storey_stays() {
    let env = started();
    let floor1 = env.storey_named("Floor 1");
    let level = spatial::level_of(
        &env.snapshot(),
        &spatial::get(&env.snapshot(), floor1).unwrap(),
    )
    .unwrap();
    let picked = env
        .run(
            "archbuilder.object.select",
            json!({"objects": [level.id.to_string()]}),
        )
        .unwrap();
    assert_eq!(picked.selection, Some(vec![floor1, level.id]));
    // Deleting down to one storey: the last one is refused with its own message.
    for name in ["Floor 2", "Floor 1"] {
        let id = env.storey_named(name).to_string();
        env.ok("archbuilder.storey.delete", json!({ "storey": id }));
    }
    let last = env.storey_named("Ground Floor").to_string();
    let error = env
        .run("archbuilder.storey.delete", json!({ "storey": last }))
        .unwrap_err();
    assert_eq!(error.code(), "ab.cmd.last_storey");
    assert_eq!(env.storeys().len(), 1);
}

#[test]
fn a_new_project_is_named_in_the_language_the_session_was_told() {
    let env = Env::new();
    env.session
        .set_setting("app.language", json!("tr"))
        .unwrap();
    env.ok("archbuilder.project.new", json!({}));
    let names: Vec<String> = env.storeys().into_iter().map(|s| s.0).collect();
    assert_eq!(names, ["Zemin Kat", "1. Kat", "2. Kat"]);
    env.ok("archbuilder.storey.add", json!({}));
    assert_eq!(env.storeys().last().unwrap().0, "3. Kat");
    let project = spatial::project(&env.snapshot()).unwrap();
    assert_eq!(text(&project.values, 0), Some("Adsız Proje"));
}
