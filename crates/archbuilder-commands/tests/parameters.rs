//! Document variables and project parameters (ADR 0009, units 2 and 3).

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_commands::parameter;
use serde_json::{Value as Json, json};

static SEEDS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(19_500);

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

    fn undo(&self) {
        assert!(self.document.undo().wait().unwrap().value);
    }

    fn variable(&self, name: &str, data: &str, value: Json) -> String {
        let mut params = json!({"name": name, "data_type": data});
        params
            .as_object_mut()
            .unwrap()
            .extend(value.as_object().unwrap().clone());
        self.run("archbuilder.variable.create", params)
            .unwrap()
            .value["variable"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    fn variables(&self) -> parameter::VariableList {
        parameter::variable_list(&self.document.snapshot())
    }

    fn value_of(&self, identifier: &str) -> Option<Json> {
        self.variables()
            .variables
            .into_iter()
            .find(|v| v.identifier == identifier)
            .and_then(|v| v.value)
    }

    fn formula_of(&self, identifier: &str) -> Option<String> {
        self.variables()
            .variables
            .into_iter()
            .find(|v| v.identifier == identifier)
            .and_then(|v| v.formula)
    }
}

fn close(value: Option<Json>, expected: f64) -> bool {
    value
        .and_then(|v| v.as_f64())
        .is_some_and(|v| (v - expected).abs() < 1e-9)
}

#[test]
fn variables_evaluate_in_order_and_each_command_is_one_undo_step() {
    let env = Env::new();
    // A formula entered before the variable it reads exists is refused.
    assert!(
        env.run(
            "archbuilder.variable.create",
            json!({"name": "Bay", "data_type": "length", "formula": "Module * 3"})
        )
        .is_err()
    );
    env.variable("Module", "length", json!({"value": "1200 mm"}));
    let bay = env.variable(
        "Bay",
        "length",
        json!({"formula": "Module * 3", "unit": "mm"}),
    );
    assert!(close(env.value_of("Bay"), 3.6));
    assert!(close(env.value_of("Module"), 1.2));
    let module = env.variables().variables[1].id.clone();
    // Changing the value regenerates what reads it.
    env.run(
        "archbuilder.variable.set",
        json!({"variable": module, "formula": "1000 + 500", "unit": "mm"}),
    )
    .unwrap();
    assert!(close(env.value_of("Bay"), 4.5));
    env.undo();
    assert!(close(env.value_of("Bay"), 3.6));
    // A dimension error names the variable and the span; the document is unchanged.
    let error = env
        .run(
            "archbuilder.variable.set",
            json!({"variable": bay, "formula": "Module * Module"}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "solve.expr.dimension_mismatch");
    assert!(arc_solve_span(&error).is_some());
    assert_eq!(env.formula_of("Bay").as_deref(), Some("Module * 3"));
    // Deleting a variable a formula reads is refused; deleting one nobody reads is undoable.
    assert!(
        env.run("archbuilder.variable.delete", json!({"variable": module}))
            .is_err()
    );
    env.run("archbuilder.variable.delete", json!({"variable": bay}))
        .unwrap();
    assert_eq!(env.variables().variables.len(), 1);
    env.undo();
    assert_eq!(env.variables().variables.len(), 2);
}

fn arc_solve_span(error: &arc_base::Error) -> Option<(i64, i64)> {
    let read = |name| match error.details().get(name) {
        Some(arc_base::error::Value::Int(v)) => Some(*v),
        _ => None,
    };
    Some((read("start")?, read("end")?))
}

#[test]
fn a_cycle_is_refused_with_the_cycle_named() {
    let env = Env::new();
    let a = env.variable("Width A", "length", json!({"value": 1.0}));
    env.variable("Width B", "length", json!({"formula": "Width_A * 2"}));
    env.variable("Width C", "length", json!({"formula": "Width_B + 1 m"}));
    let error = env
        .run(
            "archbuilder.variable.set",
            json!({"variable": a, "formula": "Width_C / 4"}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "ab.model.param_cycle");
    let Some(arc_base::error::Value::Text(cycle)) = error.arguments().get("cycle") else {
        panic!("{error:?}");
    };
    for name in ["Width A", "Width B", "Width C"] {
        assert!(cycle.contains(name), "{cycle}");
    }
    assert!(close(env.value_of("Width_C"), 3.0));
}

#[test]
fn renaming_an_identifier_rewrites_formulas_in_one_step() {
    let env = Env::new();
    let w = env.variable("Genişlik", "length", json!({"value": 0.9}));
    assert!(env.value_of("Genislik").is_some());
    env.variable("Genislik x", "length", json!({"value": 2.0}));
    env.variable(
        "Area",
        "area",
        json!({"formula": "Genislik * Genislik + 0 m * Genislik_x", "unit": "m²"}),
    );
    env.run(
        "archbuilder.variable.rename",
        json!({"variable": w, "identifier": "width", "name": "Width"}),
    )
    .unwrap();
    assert_eq!(
        env.formula_of("Area").as_deref(),
        Some("width * width + 0 m * Genislik_x")
    );
    assert!(close(env.value_of("Area"), 0.81));
    // A taken or invalid identifier is refused.
    for bad in ["Genislik_x", "sin", "2w", "mm"] {
        assert!(
            env.run(
                "archbuilder.variable.rename",
                json!({"variable": w, "identifier": bad})
            )
            .is_err(),
            "{bad}"
        );
    }
    env.undo();
    assert_eq!(
        env.formula_of("Area").as_deref(),
        Some("Genislik * Genislik + 0 m * Genislik_x")
    );
    assert!(env.value_of("Genislik").is_some());
}

#[test]
fn localized_formulas_are_stored_canonically_and_listed_stably() {
    let env = Env::new();
    let session = Session::new(Surface::Ui, [Permission::Read, Permission::Edit]);
    session.set_setting("app.language", json!("tr-TR")).unwrap();
    let invocation = Invocation {
        seed: env.seed,
        ..Invocation::new(
            "archbuilder.variable.create",
            json!({"name": "Rate", "data_type": "number", "formula": "max(1,5; 2,25)"}),
        )
    };
    Invoker::new(&env.registry)
        .invoke(&env.document, &session, invocation)
        .unwrap();
    assert_eq!(env.formula_of("Rate").as_deref(), Some("max(1.5; 2.25)"));
    assert!(close(env.value_of("Rate"), 2.25));
    let first = serde_json::to_value(env.variables()).unwrap();
    let listed = env
        .run("archbuilder.variable.list", json!({}))
        .unwrap()
        .value;
    assert_eq!(listed, first);
}
