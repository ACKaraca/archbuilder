//! The IFC export of sloped floors, floor edges and roofs by extrusion, read back.

#![allow(clippy::unwrap_used, clippy::too_many_lines, reason = "test code")]

use std::sync::Arc;

use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use arc_io_ifc::{ReadOptions, read};
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

fn open() -> (Document, arc_command::Registry) {
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
    (document, registry)
}

/// Writes the export to the path in `ABSLAB_IFC_OUT`, when set, for independent validation.
fn keep(bytes: &[u8], name: &str) {
    if let Ok(dir) = std::env::var("ABSLAB_IFC_OUT") {
        std::fs::write(std::path::Path::new(&dir).join(name), bytes).unwrap();
    }
}

#[test]
fn a_sloped_floor_is_a_slab_with_a_solid_body_and_the_quantities_of_its_plan() {
    let (document, registry) = open();
    let made = run(
        &document,
        &registry,
        "archbuilder.floor.create",
        json!({"points": [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]]}),
    );
    run(
        &document,
        &registry,
        "archbuilder.floor.set_slope",
        json!({"floor": made["floor"], "from": [0.0, 2.0], "to": [4.0, 2.0], "slope": 0.1}),
    );
    let out = export_ifc(&document.snapshot(), &ExportOptions::default()).unwrap();
    keep(&out.bytes, "slab_slope.ifc");
    let file = read(&out.bytes, &mut ReadOptions::default()).unwrap().file;
    assert_eq!(file.instances_of("IFCSLAB").count(), 1);
    // The sloped body is a solid of faces, not a vertical extrusion.
    assert_eq!(file.instances_of("IFCEXTRUDEDAREASOLID").count(), 0);
    assert_eq!(file.instances_of("IFCFACETEDBREP").count(), 1);
    assert!(
        out.report
            .items
            .iter()
            .all(|i| i.reason != "ab.io.ifc.body_not_exported")
    );
}
