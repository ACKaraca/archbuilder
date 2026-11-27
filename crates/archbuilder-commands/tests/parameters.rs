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

impl Env {
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

    fn parameter(&self, params: Json) -> String {
        self.run("archbuilder.parameter.create", params)
            .unwrap()
            .value["parameter"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    /// The value, source and formula of `identifier` on `object`.
    fn read(&self, object: &str, identifier: &str) -> (Json, String, Option<String>) {
        let list = archbuilder_commands::project_parameter::parameter_list(
            &self.document.snapshot(),
            Some(&[object.parse().unwrap()]),
        );
        let row = list
            .values
            .into_iter()
            .find(|v| v.parameter == identifier)
            .unwrap_or_else(|| panic!("{identifier} on {object}: {:?}", list.errors));
        (row.value, row.source, row.formula)
    }

    fn type_of(&self, wall: &str) -> String {
        let s = self.document.snapshot();
        let object = archbuilder_model::spatial::get(&s, wall.parse().unwrap()).unwrap();
        archbuilder_model::element_type::type_of_object(&s, &object)
            .unwrap()
            .1
            .id
            .to_string()
    }
}

fn number(value: &Json) -> f64 {
    value.as_f64().unwrap()
}

#[test]
fn instance_values_override_defaults_and_each_set_is_one_undo_step() {
    let env = Env::new();
    let wall = env.wall(0.0);
    env.variable("Module", "length", json!({"value": 1.2}));
    env.parameter(json!({
        "name": "Sill mark", "data_type": "length",
        "categories": ["archbuilder.wall"], "formula": "Module * 2", "group": "Marks"
    }));
    let (value, source, _) = env.read(&wall, "Sill_mark");
    assert!((number(&value) - 2.4).abs() < 1e-9);
    assert_eq!(source, "default");
    env.run(
        "archbuilder.parameter.set",
        json!({"objects": [wall], "parameter": env.parameter_id("Sill_mark"), "formula": "Module + 1 m"}),
    )
    .unwrap();
    let (value, source, formula) = env.read(&wall, "Sill_mark");
    assert!((number(&value) - 2.2).abs() < 1e-9);
    assert_eq!(
        (source.as_str(), formula.as_deref()),
        ("own", Some("Module + 1 m"))
    );
    // Changing the variable regenerates the value.
    let module = env.variables().variables[0].id.clone();
    env.run(
        "archbuilder.variable.set",
        json!({"variable": module, "value": 2.0}),
    )
    .unwrap();
    assert!((number(&env.read(&wall, "Sill_mark").0) - 3.0).abs() < 1e-9);
    env.undo();
    env.undo();
    assert_eq!(env.read(&wall, "Sill_mark").1, "default");
    // The property grid's field sets a formula (`=`) or clears the own value (empty).
    env.run(
        "archbuilder.property.set",
        json!({"objects": [wall], "field": "parameter:Sill_mark", "value": "=Module * 3"}),
    )
    .unwrap();
    assert!((number(&env.read(&wall, "Sill_mark").0) - 3.6).abs() < 1e-9);
    env.run(
        "archbuilder.property.set",
        json!({"objects": [wall], "field": "parameter:Sill_mark", "value": "450 mm"}),
    )
    .unwrap();
    assert!((number(&env.read(&wall, "Sill_mark").0) - 0.45).abs() < 1e-9);
    env.run(
        "archbuilder.property.set",
        json!({"objects": [wall], "field": "parameter:Sill_mark", "value": ""}),
    )
    .unwrap();
    assert_eq!(env.read(&wall, "Sill_mark").1, "default");
    // A value of another dimension is refused with its span; an unbound object is refused.
    let error = env
        .run(
            "archbuilder.parameter.set",
            json!({"objects": [wall], "parameter": env.parameter_id("Sill_mark"), "formula": "Module * Module"}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "solve.expr.dimension_mismatch");
    let storey = archbuilder_model::spatial::objects(
        &env.document.snapshot(),
        archbuilder_model::types::STOREY,
    )[0]
    .id
    .to_string();
    let error = env
        .run(
            "archbuilder.parameter.set",
            json!({"objects": [storey], "parameter": env.parameter_id("Sill_mark"), "value": 1.0}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "ab.model.param_category");
}

impl Env {
    fn parameter_id(&self, identifier: &str) -> String {
        archbuilder_commands::project_parameter::parameter_list(&self.document.snapshot(), None)
            .parameters
            .into_iter()
            .find(|p| p.identifier == identifier)
            .unwrap()
            .id
    }
}

#[test]
fn a_scope_change_keeps_the_values_and_undoes_in_one_step() {
    let env = Env::new();
    let (a, b) = (env.wall(0.0), env.wall(5.0));
    let cost = env.parameter(json!({
        "name": "Cost", "data_type": "number", "categories": ["archbuilder.wall"]
    }));
    env.run(
        "archbuilder.parameter.set",
        json!({"objects": [a], "parameter": cost, "value": 5.0}),
    )
    .unwrap();
    let c = env.wall(10.0);
    env.run(
        "archbuilder.parameter.set",
        json!({"objects": [c], "parameter": cost, "value": 7.0}),
    )
    .unwrap();
    let done = env
        .run(
            "archbuilder.parameter.update",
            json!({"parameter": cost, "scope": "type"}),
        )
        .unwrap();
    assert_eq!(done.value["copied"], 1);
    // The type takes the value of its first instance by identifier; the other wall keeps its own,
    // different value, and is reported.
    let a_first = a.parse::<arc_base::ObjectId>().unwrap() < c.parse().unwrap();
    let (first, other) = if a_first { (5.0, &c) } else { (7.0, &a) };
    assert_eq!(done.value["diverging"], json!([other]));
    // Both walls share the type, which now holds the value.
    assert_eq!(env.type_of(&a), env.type_of(&b));
    for wall in [&a, &b, &c] {
        let (value, source, _) = env.read(wall, "Cost");
        assert_eq!((number(&value), source.as_str()), (first, "type"));
    }
    let (value, source, _) = env.read(&env.type_of(&a), "Cost");
    assert_eq!((number(&value), source.as_str()), (first, "own"));
    // Setting a type-scope value on an element is refused: it is set on the type.
    assert!(
        env.run(
            "archbuilder.parameter.set",
            json!({"objects": [a], "parameter": cost, "value": 6.0}),
        )
        .is_err()
    );
    env.run(
        "archbuilder.parameter.update",
        json!({"parameter": cost, "scope": "instance"}),
    )
    .unwrap();
    // Back in instance scope every value stored before is read again; the wall without one
    // took its type's.
    for (wall, value) in [(&a, 5.0), (&b, first), (&c, 7.0)] {
        let (read, source, _) = env.read(wall, "Cost");
        assert_eq!((number(&read), source.as_str()), (value, "own"));
    }
    env.undo();
    assert_eq!(env.read(&b, "Cost").1, "type");
    env.undo();
    assert_eq!(env.read(&b, "Cost").1, "unset");
    assert_eq!(number(&env.read(&a, "Cost").0), 5.0);
}

#[test]
fn reject_rules_refuse_and_other_rules_warn() {
    let env = Env::new();
    let wall = env.wall(0.0);
    let module = env.variable("Module", "length", json!({"value": 1.2}));
    let reach = env.parameter(json!({
        "name": "Reach", "data_type": "length", "categories": ["archbuilder.wall"],
        "formula": "Module", "rule": {"minimum": 1.0, "reject": true}
    }));
    let error = env
        .run(
            "archbuilder.parameter.set",
            json!({"objects": [wall], "parameter": reach, "value": 0.5}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "ab.model.param_rule");
    assert_eq!(env.read(&wall, "Reach").1, "default");
    // A variable change that breaks the rule through a formula is refused and names the wall.
    let error = env
        .run(
            "archbuilder.variable.set",
            json!({"variable": module, "value": 0.5}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "ab.model.param_rule");
    assert!(matches!(
        error.arguments().get("count"),
        Some(arc_base::error::Value::Int(n)) if *n >= 1
    ));
    // A rule that only warns lets the value through and says so.
    let note = env.parameter(json!({
        "name": "Note", "data_type": "text", "categories": ["archbuilder.wall"],
        "rule": {"allowed": ["A", "B"]}
    }));
    let done = env
        .run(
            "archbuilder.parameter.set",
            json!({"objects": [wall], "parameter": note, "value": "C"}),
        )
        .unwrap();
    assert_eq!(done.value["warnings"].as_array().unwrap().len(), 1);
    // A required value with no default is refused for the elements that exist.
    assert!(
        env.run(
            "archbuilder.parameter.create",
            json!({"name": "Code", "data_type": "text", "categories": ["archbuilder.wall"],
                   "rule": {"required": true, "reject": true}})
        )
        .is_err()
    );
}

#[test]
fn renaming_and_deleting_a_parameter_carry_its_values() {
    let env = Env::new();
    let wall = env.wall(0.0);
    let w = env.parameter(json!({
        "name": "Width", "data_type": "length", "categories": ["archbuilder.wall"], "value": 1.0
    }));
    let d = env.parameter(json!({
        "name": "Depth", "data_type": "length", "categories": ["archbuilder.wall"]
    }));
    env.run(
        "archbuilder.parameter.set",
        json!({"objects": [wall], "parameter": d, "formula": "Width / 2"}),
    )
    .unwrap();
    // Closing a cycle between an element's values is refused.
    let error = env
        .run(
            "archbuilder.parameter.set",
            json!({"objects": [wall], "parameter": w, "formula": "Depth * 2"}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "ab.model.param_cycle");
    env.run(
        "archbuilder.parameter.rename",
        json!({"parameter": w, "identifier": "span"}),
    )
    .unwrap();
    assert_eq!(env.read(&wall, "Depth").2.as_deref(), Some("span / 2"));
    // Deleting a parameter a formula reads is refused; the reader goes first.
    assert!(
        env.run("archbuilder.parameter.delete", json!({"parameter": w}))
            .is_err()
    );
    env.run("archbuilder.parameter.delete", json!({"parameter": d}))
        .unwrap();
    let s = env.document.snapshot();
    let object = archbuilder_model::spatial::get(&s, wall.parse().unwrap()).unwrap();
    assert!(archbuilder_model::parameter::stored(&s, &object).is_empty());
    env.undo();
    assert_eq!(env.read(&wall, "Depth").1, "own");
    env.undo();
    assert_eq!(env.read(&wall, "Depth").2.as_deref(), Some("Width / 2"));
}

#[test]
fn a_pinned_element_takes_parameter_values() {
    let env = Env::new();
    let wall = env.wall(0.0);
    env.run("archbuilder.element.pin", json!({"objects": [wall]}))
        .unwrap();
    // A pinned wall refuses to move.
    assert!(
        env.run(
            "archbuilder.modify.move",
            json!({"objects": [wall], "displacement": [1.0, 0.0]})
        )
        .is_err_and(|e| e.code() == "ab.model.pinned")
    );
    let note = env.parameter(json!({
        "name": "Note", "data_type": "text", "categories": ["archbuilder.wall"]
    }));
    env.run(
        "archbuilder.parameter.set",
        json!({"objects": [wall], "parameter": note, "value": "Fire wall"}),
    )
    .unwrap();
    assert_eq!(env.read(&wall, "Note").0, json!("Fire wall"));
}
