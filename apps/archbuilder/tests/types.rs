//! Types in the application, headless: a document saved before types opens, its walls, floors,
//! doors and windows get types matching their sizes (no undo step), and it saves and reopens with
//! them; create similar starts the wall tool with the type of the selected wall.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::sync::Arc;

use arc_app::application::{AppConfig, Application};
use arc_app::recent::RecentFiles;
use arc_base::{IdGenerator, Limits, ObjectId};
use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::package::LockOwner;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder::wall_tool::{Env, Tool};
use archbuilder_commands::element_type::needs_upgrade;
use archbuilder_model::element_type::{self as et, TypeKind};
use archbuilder_model::spatial;
use archbuilder_model::types::{DOOR, FLOOR, WALL, WINDOW};
use archbuilder_model::units::LengthUnit;
use serde_json::json;

/// One step of the wall tool with a runner of invocations.
type Step<'a> = dyn FnMut(
        &mut Tool,
        &Env<'_>,
        &mut dyn FnMut(Invocation) -> arc_base::Result<arc_command::Outcome>,
    ) -> arc_base::Result<()>
    + 'a;

fn app(dir: &std::path::Path) -> Application {
    let owner = LockOwner {
        user: "test".into(),
        host: "test".into(),
        application: "archbuilder tests".into(),
        time_unix_ms: 0,
    };
    let product = archbuilder::product(Arc::new(arc_command::FixedClock(0)), owner).unwrap();
    let mut next = 40;
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

fn type_names(snapshot: &Snapshot, key: &str, kind: TypeKind) -> Vec<String> {
    spatial::objects(snapshot, key)
        .iter()
        .map(|o| et::name(&et::type_of(snapshot, kind, &o.values).unwrap()).to_owned())
        .collect()
}

#[test]
fn a_document_from_before_types_opens_with_types_and_round_trips() {
    let dir = std::env::temp_dir().join(format!("archbuilder-types-{}", std::process::id()));
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
    // Four walls (0.2 and 0.25 m), a door, a window, a floor and a room, none with a type.
    assert_eq!(spatial::objects(&snapshot, WALL).len(), 4);
    assert!(needs_upgrade(&snapshot));
    assert!(spatial::objects(&snapshot, archbuilder_model::types::WALL_TYPE).is_empty());
    // What the controller does when it sees the document: one starting step, no undo step.
    let invocation = archbuilder::interaction::upgrade_invocation(&snapshot).unwrap();
    let session = Session::new(Surface::Ui, [Permission::Read, Permission::Edit]);
    let done = first.invoke(tab, &session, invocation).unwrap();
    assert_eq!(done.value["instances"], 7);
    assert!(!first.undo(tab).unwrap(), "the types are no undo step");
    let snapshot = first.tab(tab).unwrap().document().snapshot();
    assert!(!needs_upgrade(&snapshot));
    let mut walls = type_names(&snapshot, WALL, TypeKind::Wall);
    walls.sort();
    assert_eq!(
        walls,
        ["Generic 200", "Generic 200", "Generic 250", "Generic 250"]
    );
    assert_eq!(
        type_names(&snapshot, FLOOR, TypeKind::Floor),
        ["Generic 200"]
    );
    assert_eq!(
        type_names(&snapshot, DOOR, TypeKind::Door),
        ["Single 900 × 2100"]
    );
    assert_eq!(
        type_names(&snapshot, WINDOW, TypeKind::Window),
        ["Window 1000 × 1200"]
    );
    // Saved and opened again: the same types, nothing left to upgrade.
    let new = dir.join("with-types.archb");
    first.save_as(tab, &new).unwrap();
    drop(first);
    let mut second = app(&dir);
    let tab = second.open(&new).unwrap();
    let reopened = second.tab(tab).unwrap().document().snapshot();
    assert!(archbuilder::interaction::upgrade_invocation(&reopened).is_none());
    assert_eq!(
        type_names(&reopened, WALL, TypeKind::Wall).len(),
        4,
        "every wall keeps its type"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_similar_draws_with_the_type_of_the_selected_wall() {
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
    let registry = archbuilder_commands::registry().unwrap();
    let mut session = Session::new(Surface::CommandLine, [Permission::Read, Permission::Edit]);
    session.units = vec![*LengthUnit::Meter.unit().unwrap()];
    let invoker = Invoker::new(&registry);
    let run = |command: &str, params: serde_json::Value| {
        invoker
            .invoke(
                &document,
                &session,
                Invocation {
                    seed: 3,
                    ..Invocation::new(command, params)
                },
            )
            .unwrap()
    };
    run("archbuilder.project.new", json!({}));
    let made = run(
        "archbuilder.wall.create",
        json!({"start": [0.0, 0.0], "end": [4.0, 0.0], "type": "Interior partition 100"}),
    );
    let source: ObjectId = made.value["wall"].as_str().unwrap().parse().unwrap();
    // Create similar picks the source's type for the wall tool, then starts it.
    let snapshot = document.snapshot();
    let (kind, picked) = archbuilder::tool_type::shown(&snapshot, &[source]).unwrap();
    assert_eq!(kind, TypeKind::Wall);
    archbuilder::tool_type::choose(kind, picked.unwrap());
    let mut tool = Tool::new();
    let mut seed = 10;
    let mut step = |f: &mut Step<'_>| {
        seed += 1;
        let env = Env {
            registry: &registry,
            document: &document,
            session: &session,
            selection: &[],
            seed,
        };
        f(&mut tool, &env, &mut |inv| {
            invoker.invoke(&document, &session, inv)
        })
        .unwrap();
    };
    step(&mut |t, env, run| t.begin(env, run));
    step(&mut |t, env, run| t.text(env, run, "0,3"));
    step(&mut |t, env, run| t.text(env, run, "4,3"));
    let snapshot = document.snapshot();
    let mut names = type_names(&snapshot, WALL, TypeKind::Wall);
    names.dedup();
    assert_eq!(names, ["Interior partition 100"]);
    assert_eq!(spatial::objects(&snapshot, WALL).len(), 2);
}

#[test]
fn a_file_whose_walls_disagree_with_their_type_is_resynchronized_on_open() {
    let dir = std::env::temp_dir().join(format!("archbuilder-resync-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut first = app(&dir);
    let tab = first.new_document().unwrap();
    let session = Session::new(Surface::Ui, [Permission::Read, Permission::Edit]);
    let made = first
        .invoke(
            tab,
            &session,
            Invocation::new(
                "archbuilder.wall.create",
                json!({"start": [0.0, 0.0], "end": [4.0, 0.0]}),
            ),
        )
        .unwrap();
    let wall: ObjectId = made.value["wall"].as_str().unwrap().parse().unwrap();
    // A hand-edited file: the wall's thickness no longer is its type's.
    first
        .tab(tab)
        .unwrap()
        .document()
        .submit(move |txn: &mut arc_doc::Txn| {
            txn.modify(
                wall,
                arc_doc::FieldPath::new("thickness"),
                Some(archbuilder_model::types::length(0.5)),
            )
        })
        .wait()
        .unwrap();
    let crafted = dir.join("crafted.archb");
    first.save_as(tab, &crafted).unwrap();
    drop(first);
    let mut second = app(&dir);
    let tab = second.open(&crafted).unwrap();
    let snapshot = second.tab(tab).unwrap().document().snapshot();
    let invocation = archbuilder::interaction::upgrade_invocation(&snapshot).unwrap();
    second.invoke(tab, &session, invocation).unwrap();
    let snapshot = second.tab(tab).unwrap().document().snapshot();
    let thickness =
        archbuilder_model::wall::Wall::from_values(&spatial::get(&snapshot, wall).unwrap().values)
            .unwrap()
            .thickness;
    assert!((thickness - 0.2).abs() < 1e-9, "{thickness}");
    // Later edits are accepted.
    second
        .invoke(
            tab,
            &session,
            Invocation::new(
                "archbuilder.property.set",
                json!({"objects": [wall.to_string()], "field": "height", "value": 2.5}),
            ),
        )
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
