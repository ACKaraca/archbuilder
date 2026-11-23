//! Plan views: one default view per storey, duplicate, rename, delete, hide in view and the
//! views a document made before they existed gets.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::spatial;
use archbuilder_model::types::{PLAN_VIEW, STOREY};
use archbuilder_model::view::PlanView;
use serde_json::{Value as Json, json};

static SEEDS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(9_000);

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

    fn views(&self, storey: ObjectId) -> Vec<(ObjectId, String)> {
        let s = self.document.snapshot();
        archbuilder_model::view::of_storey(&s, storey)
            .iter()
            .map(|v| {
                (
                    v.id,
                    archbuilder_model::types::text(&v.values, 0)
                        .unwrap()
                        .to_owned(),
                )
            })
            .collect()
    }
}

#[test]
fn every_storey_gets_a_default_view_and_loses_it_with_the_storey() {
    let env = Env::new();
    let s = env.document.snapshot();
    for storey in spatial::objects(&s, STOREY) {
        assert_eq!(env.views(storey.id).len(), 1);
    }
    let added = env
        .run("archbuilder.storey.add", json!({"name": "Attic"}))
        .unwrap();
    let attic: ObjectId = added.value["storey"].as_str().unwrap().parse().unwrap();
    assert_eq!(
        env.views(attic),
        vec![(env.views(attic)[0].0, "Attic".to_owned())]
    );
    env.run(
        "archbuilder.storey.delete",
        json!({"storey": attic.to_string()}),
    )
    .unwrap();
    assert!(
        spatial::objects(&env.document.snapshot(), PLAN_VIEW)
            .iter()
            .all(|v| v.header.owner != Some(attic))
    );
}

#[test]
fn duplicate_and_rename_are_undoable_and_the_last_view_stays() {
    let env = Env::new();
    let ground = env.storey_first();
    let (first, name) = env.views(ground)[0].clone();
    let made = env
        .run(
            "archbuilder.view.duplicate",
            json!({"view": first.to_string()}),
        )
        .unwrap();
    let copy: ObjectId = made.value["view"].as_str().unwrap().parse().unwrap();
    assert_eq!(env.views(ground).len(), 2);
    assert!(
        env.views(ground)
            .iter()
            .any(|(_, n)| *n == format!("{name} 2"))
    );
    env.run(
        "archbuilder.view.rename",
        json!({"view": copy.to_string(), "name": "Furniture"}),
    )
    .unwrap();
    assert!(env.views(ground).iter().any(|(_, n)| n == "Furniture"));
    // A name that another view of the storey has is refused.
    assert!(
        env.run(
            "archbuilder.view.rename",
            json!({"view": copy.to_string(), "name": name})
        )
        .is_err()
    );
    assert!(env.document.undo().wait().unwrap().value);
    assert!(
        env.views(ground)
            .iter()
            .any(|(_, n)| *n == format!("{name} 2"))
    );
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(env.views(ground).len(), 1);
    let error = env
        .run(
            "archbuilder.view.delete",
            json!({"view": first.to_string()}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "ab.cmd.last_view");
}

#[test]
fn hiding_an_element_in_a_view_is_kept_in_the_view_only() {
    let env = Env::new();
    let ground = env.storey_first();
    let wall = env
        .run(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [4.0, 0.0], "storey": ground.to_string()}),
        )
        .unwrap()
        .value["wall"]
        .as_str()
        .unwrap()
        .to_owned();
    let view = env.views(ground)[0].0;
    env.run(
        "archbuilder.view.hide_elements",
        json!({"view": view.to_string(), "objects": [wall]}),
    )
    .unwrap();
    let s = env.document.snapshot();
    let settings = PlanView::from_values(&spatial::get(&s, view).unwrap().values).unwrap();
    assert_eq!(settings.hidden, vec![wall.parse().unwrap()]);
    env.run(
        "archbuilder.view.show_elements",
        json!({"view": view.to_string()}),
    )
    .unwrap();
    let s = env.document.snapshot();
    assert!(
        PlanView::from_values(&spatial::get(&s, view).unwrap().values)
            .unwrap()
            .hidden
            .is_empty()
    );
}

#[test]
fn a_document_without_views_gets_its_default_views_once() {
    let env = Env::new();
    let s = env.document.snapshot();
    let ids: Vec<ObjectId> = spatial::objects(&s, PLAN_VIEW)
        .iter()
        .map(|v| v.id)
        .collect();
    env.document
        .submit(move |txn| {
            for id in ids {
                txn.delete(id)?;
            }
            Ok(())
        })
        .wait()
        .unwrap();
    assert!(spatial::objects(&env.document.snapshot(), PLAN_VIEW).is_empty());
    let made = env
        .run("archbuilder.view.ensure_defaults", json!({}))
        .unwrap();
    assert_eq!(made.value["made"], 3);
    assert_eq!(
        env.run("archbuilder.view.ensure_defaults", json!({}))
            .unwrap()
            .value["made"],
        0
    );
}

impl Env {
    fn storey_first(&self) -> ObjectId {
        let s = self.document.snapshot();
        spatial::storeys(&s, spatial::buildings(&s)[0].id)[0]
            .storey
            .id
    }
}

impl Env {
    /// A storey with no view: a document from before plan views.
    fn storey_without_view(&self) -> ObjectId {
        let ground = self.storey_first();
        let ids: Vec<ObjectId> = self.views(ground).into_iter().map(|(id, _)| id).collect();
        self.document
            .submit(move |txn| {
                for id in ids {
                    txn.delete(id)?;
                }
                Ok(())
            })
            .wait()
            .unwrap();
        assert!(self.views(ground).is_empty());
        ground
    }
}

#[test]
fn an_edit_of_a_storeys_default_view_makes_the_view_in_one_undo_step() {
    let env = Env::new();
    let ground = env.storey_without_view();
    let before = env.document.snapshot().revision();
    let made = env
        .run(
            "archbuilder.view.set",
            json!({"view": ground.to_string(), "field": "cut", "value": 1.5}),
        )
        .unwrap();
    let view: ObjectId = made.value["view"].as_str().unwrap().parse().unwrap();
    let s = env.document.snapshot();
    let settings = PlanView::from_values(&spatial::get(&s, view).unwrap().values).unwrap();
    assert!((settings.cut - 1.5).abs() < 1e-9);
    assert_eq!(env.views(ground).len(), 1);
    // One undo step takes the view and the change back: the default view again, nothing held.
    assert!(env.document.undo().wait().unwrap().value);
    assert!(env.views(ground).is_empty());
    assert!(env.document.snapshot().revision() > before);
    // A second edit of a view that exists only sets it.
    env.run(
        "archbuilder.view.set",
        json!({"view": ground.to_string(), "field": "scale", "value": 50.0}),
    )
    .unwrap();
    let again = env.views(ground)[0].0;
    env.run(
        "archbuilder.view.set",
        json!({"view": ground.to_string(), "field": "cut", "value": 1.4}),
    )
    .unwrap();
    assert_eq!(
        env.views(ground),
        vec![(again, env.views(ground)[0].1.clone())]
    );
}

#[test]
fn rename_duplicate_and_hide_of_a_default_view_make_the_view_in_the_same_step() {
    let env = Env::new();
    let ground = env.storey_without_view();
    env.run(
        "archbuilder.view.rename",
        json!({"view": ground.to_string(), "name": "Ground plan"}),
    )
    .unwrap();
    assert_eq!(env.views(ground)[0].1, "Ground plan");
    assert!(env.document.undo().wait().unwrap().value);
    assert!(env.views(ground).is_empty());
    // Duplicate keeps the default as a view and adds its copy.
    env.run(
        "archbuilder.view.duplicate",
        json!({"view": ground.to_string()}),
    )
    .unwrap();
    assert_eq!(env.views(ground).len(), 2);
    assert!(env.document.undo().wait().unwrap().value);
    assert!(env.views(ground).is_empty());
    // Hide in view makes the view with the element hidden.
    let wall = env
        .run(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [4.0, 0.0]}),
        )
        .unwrap()
        .value["wall"]
        .as_str()
        .unwrap()
        .parse::<ObjectId>()
        .unwrap();
    env.run(
        "archbuilder.view.hide_elements",
        json!({"view": ground.to_string(), "objects": [wall.to_string()]}),
    )
    .unwrap();
    let s = env.document.snapshot();
    let view = env.views(ground)[0].0;
    let settings = PlanView::from_values(&spatial::get(&s, view).unwrap().values).unwrap();
    assert_eq!(settings.hidden, vec![wall]);
    // There is no default view to delete.
    assert!(
        env.run(
            "archbuilder.view.delete",
            json!({"view": ground.to_string()})
        )
        .is_err()
    );
}
