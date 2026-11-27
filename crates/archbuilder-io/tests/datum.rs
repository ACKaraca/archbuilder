//! The IFC export of datums: a grid needs two directions of lines (see `parts.rs`); reference planes
//! and work planes are named in the loss report and never written as proxies.

#![allow(clippy::unwrap_used, clippy::too_many_lines, reason = "test code")]

use std::sync::Arc;

use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_io::{ExportOptions, export_ifc};
use serde_json::{Value as Json, json};

fn run(document: &Document, registry: &arc_command::Registry, command: &str, params: Json) -> Json {
    let session = Session::new(Surface::Ui, [Permission::Read, Permission::Edit]);
    let invocation = Invocation {
        seed: 1_000_003,
        ..Invocation::new(command, params)
    };
    Invoker::new(registry)
        .invoke(document, &session, invocation)
        .unwrap()
        .value
}

#[test]
fn a_grid_of_one_direction_and_the_planes_are_reported_and_not_written() {
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
    run(&document, &registry, "archbuilder.project.new", json!({}));
    run(
        &document,
        &registry,
        "archbuilder.grid.create",
        json!({"start": [0.0, 0.0], "end": [0.0, 5.0]}),
    );
    run(
        &document,
        &registry,
        "archbuilder.reference_plane.create",
        json!({"start": [0.0, 0.0], "end": [5.0, 0.0]}),
    );
    run(
        &document,
        &registry,
        "archbuilder.work_plane.set",
        json!({"target": "Floor 1"}),
    );
    let exported = export_ifc(&document.snapshot(), &ExportOptions::default()).unwrap();
    let mut codes: Vec<&str> = exported
        .report
        .items
        .iter()
        .map(|i| i.reason.as_str())
        .collect();
    codes.sort_unstable();
    // The one grid line has no cross direction, so there is no `IfcGrid`; the planes have no
    // building element.
    assert_eq!(
        codes,
        [
            "ab.io.ifc.datum_not_written",
            "ab.io.ifc.datum_not_written",
            "ab.io.ifc.grid_one_direction"
        ]
    );
    assert_eq!(exported.elements, 0);
    let text = String::from_utf8_lossy(&exported.bytes);
    assert!(!text.contains("IFCGRID") && !text.contains("IFCBUILDINGELEMENTPROXY"));
}
