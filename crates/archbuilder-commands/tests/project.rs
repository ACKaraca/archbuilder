//! The project record: display units, project information, templates, purge and standards
//! transfer.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::spatial;
use serde_json::{Value as Json, json};

/// Every environment draws its identifiers from its own seed: the derived caches are keyed by
/// identifier, and tests that run side by side must not share them.
static SEEDS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

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

    fn snapshot(&self) -> Snapshot {
        self.document.snapshot()
    }

    #[allow(dead_code, reason = "shared with the other tests of this file")]
    fn storeys(&self) -> Vec<ObjectId> {
        let s = self.snapshot();
        let mut found: Vec<_> = spatial::objects(&s, archbuilder_model::types::STOREY)
            .iter()
            .map(|o| (spatial::storey_elevation(&s, o), o.id))
            .collect();
        found.sort_by(|a, b| a.0.total_cmp(&b.0));
        found.into_iter().map(|(_, id)| id).collect()
    }
}

fn run_ok(env: &Env, command: &str, params: Json) -> Outcome {
    env.run(command, params).unwrap()
}

#[test]
fn display_units_change_text_and_typed_values_never_stored_values_and_undo() {
    let env = Env::new();
    let before = spatial::unit_settings(&env.snapshot());
    assert_eq!(before.area_text(12.3456, '.'), "12.35 m²");
    run_ok(
        &env,
        "archbuilder.project.set_units",
        json!({"area_unit": "square_foot", "angle_unit": "gradian", "area_precision": 1,
               "length_rounding": 0.05, "slope_unit": "percent"}),
    );
    let u = spatial::unit_settings(&env.snapshot());
    assert_eq!(u.area_text(1.0, '.'), "10.8 ft²");
    assert_eq!(u.length_text(1.234, '.'), "1.25 m");
    // A bare angle is read in the project's angle unit; a unit says its own.
    run_ok(
        &env,
        "archbuilder.coordinates.set",
        json!({"true_north": "100"}),
    );
    let north = archbuilder_model::coordinates::Coordinates::of(&env.snapshot()).true_north;
    assert!(
        (north - std::f64::consts::FRAC_PI_2).abs() < 1e-9,
        "{north}"
    );
    run_ok(
        &env,
        "archbuilder.coordinates.set",
        json!({"true_north": "45 deg"}),
    );
    // Reset puts the defaults back; one undo step per command.
    run_ok(
        &env,
        "archbuilder.project.set_units",
        json!({"reset": true}),
    );
    assert_eq!(
        spatial::unit_settings(&env.snapshot()).length_text(1.234, '.'),
        "1.234 m"
    );
    env.document.undo().wait().unwrap();
    assert_eq!(
        spatial::unit_settings(&env.snapshot()).length_text(1.234, '.'),
        "1.25 m"
    );
    // A bad unit, a precision over 8 and a non-positive rounding are refused, nothing changes.
    for bad in [
        json!({"area_unit": "furlong"}),
        json!({"length_precision": 9}),
        json!({"angle_rounding": 0.0}),
        json!({"volume_unit": "square_foot"}),
    ] {
        assert!(env.run("archbuilder.project.set_units", bad).is_err());
    }
    let stats = archbuilder_commands::stats::statistics(&env.snapshot());
    assert_eq!(stats.display_units["area"], "square_foot");
    assert_eq!(stats.display_units["angle"], "gradian");
    // The property command takes the same fields.
    let project = spatial::project(&env.snapshot()).unwrap().id.to_string();
    run_ok(
        &env,
        "archbuilder.property.set",
        json!({"objects": [project], "field": "volume_unit", "value": "liter"}),
    );
    assert_eq!(
        spatial::unit_settings(&env.snapshot()).volume_text(0.5, '.'),
        "500 L"
    );
}

fn info(env: &Env, at: usize) -> Option<String> {
    let project = spatial::project(&env.snapshot()).unwrap();
    archbuilder_model::types::text(&project.values, at).map(str::to_owned)
}

#[test]
fn project_information_is_validated_cleared_edited_by_the_property_command_and_undone() {
    use archbuilder_model::types::project as p;
    let env = Env::new();
    run_ok(
        &env,
        "archbuilder.project.set_info",
        json!({"number": "P-1", "status": "Design", "issue_date": "2028-02-29",
               "year_of_construction": "2030", "author": " Ada Lovelace "}),
    );
    assert_eq!(info(&env, p::NUMBER).as_deref(), Some("P-1"));
    assert_eq!(info(&env, p::AUTHOR).as_deref(), Some("Ada Lovelace"));
    // A date that is not on the calendar, a year that is not four digits and an empty name are
    // refused and leave the record as it was.
    for bad in [
        json!({"issue_date": "2027-02-29"}),
        json!({"issue_date": "27.11.2026"}),
        json!({"year_of_construction": "28"}),
        json!({"name": "  "}),
    ] {
        assert!(env.run("archbuilder.project.set_info", bad).is_err());
    }
    assert_eq!(info(&env, p::ISSUE_DATE).as_deref(), Some("2028-02-29"));
    // The property grid edits the same fields; an empty text clears one.
    let project = spatial::project(&env.snapshot()).unwrap().id.to_string();
    run_ok(
        &env,
        "archbuilder.property.set",
        json!({"objects": [project], "field": "organization", "value": "Arc Studio"}),
    );
    assert!(
        env.run(
            "archbuilder.property.set",
            json!({"objects": [project], "field": "issue_date", "value": "tomorrow"}),
        )
        .is_err()
    );
    run_ok(&env, "archbuilder.project.set_info", json!({"status": ""}));
    assert_eq!(info(&env, p::STATUS), None);
    env.document.undo().wait().unwrap();
    assert_eq!(info(&env, p::STATUS).as_deref(), Some("Design"));
    env.document.undo().wait().unwrap();
    assert_eq!(info(&env, p::ORGANIZATION), None);
    // Nothing to set selects the project.
    run_ok(&env, "archbuilder.project.set_info", json!({}));
}
