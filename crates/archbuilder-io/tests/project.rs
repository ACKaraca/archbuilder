//! The project information in the IFC file: the long name, phase, addresses, actors and the
//! building properties, read back; the file is checked independently by the IFC validator of the
//! checks (set `ARCHB_IFC_OUT` to keep the file).

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
fn project_information_is_written_and_the_export_reads_back() {
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
    let plain = export_ifc(&document.snapshot(), &ExportOptions::default()).unwrap();
    assert!(plain.valid);
    run(
        &document,
        &registry,
        "archbuilder.project.set_info",
        json!({
            "name": "Lakeside Library", "number": "P-2026-014", "client": "City of O'Brien",
            "address": "12 Shore Road\nÇankaya, Ankara", "status": "Permit",
            "issue_date": "2026-11-27", "author": "Ayşe Yılmaz", "organization": "Arc Studio",
            "description": "A public library.", "long_name": "Lakeside Public Library",
            "occupancy_type": "Library", "year_of_construction": "2028",
            "construction_method": "Reinforced concrete"
        }),
    );
    let exported = export_ifc(&document.snapshot(), &ExportOptions::default()).unwrap();
    assert!(exported.valid, "{:?}", exported.report.items);
    let text = String::from_utf8(exported.bytes.clone()).unwrap();
    for needle in [
        "IFCPROJECT(",
        "'Lakeside Public Library'",
        "'Permit'",
        "IFCPOSTALADDRESS",
        "IFCRELASSIGNSTOACTOR",
        "IFCACTORROLE(.CLIENT.",
        "Pset_BuildingCommon",
        "IFCIDENTIFIER('P-2026-014')",
        "IFCLABEL('Library')",
        "IFCLABEL('2028')",
        "IFCLABEL('Reinforced concrete')",
        "'City of O''Brien'",
    ] {
        assert!(text.contains(needle), "{needle}");
    }
    // Only the issue date has nowhere to go, and it is reported rather than dropped silently.
    let codes: Vec<&str> = exported
        .report
        .items
        .iter()
        .map(|i| i.reason.as_str())
        .collect();
    assert_eq!(codes, ["ab.io.ifc.issue_date_not_written"]);
    // The same project gives the same bytes.
    let again = export_ifc(&document.snapshot(), &ExportOptions::default()).unwrap();
    assert_eq!(again.bytes, exported.bytes);
    if let Ok(path) = std::env::var("ARCHB_IFC_OUT") {
        std::fs::write(path, &exported.bytes).unwrap();
    }
}
