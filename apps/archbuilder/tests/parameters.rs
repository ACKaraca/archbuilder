//! A document from before parameters opens unchanged, takes a document variable, a project
//! parameter and a value on a wall, saves and opens again with them.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::sync::Arc;

use arc_app::application::{AppConfig, Application};
use arc_app::recent::RecentFiles;
use arc_base::{IdGenerator, Limits};
use arc_command::{Invocation, Permission, Session, Surface};
use arc_doc::package::LockOwner;
use archbuilder_model::spatial;
use archbuilder_model::types::{PARAMETER, VARIABLE, WALL};
use serde_json::json;

fn app(dir: &std::path::Path) -> Application {
    let owner = LockOwner {
        user: "test".into(),
        host: "test".into(),
        application: "archbuilder tests".into(),
        time_unix_ms: 0,
    };
    let product = archbuilder::product(Arc::new(arc_command::FixedClock(0)), owner).unwrap();
    let mut next = 90;
    Application::new(
        product,
        AppConfig {
            data_dir: dir.to_path_buf(),
            limits: Limits::DEFAULT,
            id_generators: Box::new(move || {
                next += 1;
                IdGenerator::seeded(next)
            }),
            recent: RecentFiles::new(4),
        },
    )
}

#[test]
fn an_old_document_loads_unchanged_and_keeps_parameter_values_after_reopen() {
    let dir = std::env::temp_dir().join(format!("archbuilder-params-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let old = dir.join("pre-types.archb");
    std::fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/pre-types.archb"
        ),
        &old,
    )
    .unwrap();
    let mut first = app(&dir);
    let tab = first.open(&old).unwrap();
    let snapshot = first.tab(tab).unwrap().document().snapshot();
    // No definitions and no values: the walls read as they were written.
    assert!(spatial::objects(&snapshot, VARIABLE).is_empty());
    assert!(spatial::objects(&snapshot, PARAMETER).is_empty());
    let walls = spatial::objects(&snapshot, WALL);
    assert!(!walls.is_empty());
    for wall in &walls {
        assert!(archbuilder_model::parameter::stored(&snapshot, wall).is_empty());
    }
    let session = Session::new(Surface::Ui, [Permission::Read, Permission::Edit]);
    if let Some(invocation) = archbuilder::interaction::upgrade_invocation(&snapshot) {
        first.invoke(tab, &session, invocation).unwrap();
    }
    let run = |command: &str, params: serde_json::Value| {
        first
            .invoke(tab, &session, Invocation::new(command, params))
            .unwrap()
            .value
    };
    run(
        "archbuilder.variable.create",
        json!({"name": "Module", "data_type": "length", "value": 0.6}),
    );
    let mark = run(
        "archbuilder.parameter.create",
        json!({"name": "Mark", "data_type": "length", "categories": [WALL]}),
    )["parameter"]
        .as_str()
        .unwrap()
        .to_owned();
    let wall = walls[0].id.to_string();
    run(
        "archbuilder.parameter.set",
        json!({"objects": [wall], "parameter": mark, "formula": "Module * 4"}),
    );
    let new = dir.join("with-parameters.archb");
    first.save_as(tab, &new).unwrap();
    drop(first);
    let mut second = app(&dir);
    let tab = second.open(&new).unwrap();
    let reopened = second.tab(tab).unwrap().document().snapshot();
    let list =
        archbuilder_commands::project_parameter::parameter_list(&reopened, Some(&[walls[0].id]));
    let row = &list.values[0];
    assert_eq!(
        (row.parameter.as_str(), row.formula.as_deref()),
        ("Mark", Some("Module * 4"))
    );
    assert!((row.value.as_f64().unwrap() - 2.4).abs() < 1e-9);
    let _ = std::fs::remove_dir_all(&dir);
}
