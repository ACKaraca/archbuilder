//! A pasted copy of an imported element is a new element: the export writes it with its own
//! `GlobalId`, not the source's.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::collections::BTreeSet;
use std::sync::Arc;

use arc_base::ObjectId;
use arc_base::id::Identifier;
use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Header, Snapshot, Value};
use archbuilder_io::{ExportOptions, export_ifc};
use archbuilder_model::foreign;
use archbuilder_model::spatial;
use archbuilder_model::types::{self, FOREIGN_ELEMENT, STOREY};
use serde_json::{Value as Json, json};

fn run(document: &Document, registry: &arc_command::Registry, command: &str, params: Json) -> Json {
    let session = Session::new(Surface::Ui, [Permission::Read, Permission::Edit]);
    let invocation = Invocation {
        seed: 17,
        ..Invocation::new(command, params)
    };
    Invoker::new(registry)
        .invoke(document, &session, invocation)
        .unwrap_or_else(|e| panic!("{command}: {e:?}"))
        .value
}

/// The `GlobalId`s of the `IFCCOLUMN` records of an exported file.
fn column_ids(text: &str) -> Vec<String> {
    text.lines()
        .filter(|l| l.contains("=IFCCOLUMN("))
        .filter_map(|l| l.split('\'').nth(1).map(str::to_owned))
        .collect()
}

#[test]
fn a_pasted_copy_of_an_imported_element_exports_with_its_own_global_id() {
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
    let snapshot = document.snapshot();
    let storey = spatial::objects(&snapshot, STOREY)[0].id;
    let mut values: Vec<Option<Value>> = snapshot
        .registry()
        .get(FOREIGN_ELEMENT)
        .map(|d| d.schema.fields.iter().map(|f| f.default.clone()).collect())
        .unwrap_or_default();
    if let Some(slot) = values.get_mut(types::foreign_element::NAME) {
        *slot = Some(Value::String("Column".to_owned()));
    }
    if let Some(slot) = values.get_mut(types::foreign_element::CLASS) {
        *slot = Some(Value::String("IfcColumn".to_owned()));
    }
    if let Some(slot) = values.get_mut(types::foreign_element::MESH) {
        *slot = foreign::box_value([1.0, 1.0, 0.0], [2.0, 2.0, 3.0]);
    }
    let source = ObjectId::from_u128(0xA1);
    document
        .submit(move |txn| {
            txn.create(
                FOREIGN_ELEMENT,
                source,
                Header {
                    owner: Some(storey),
                    ..Default::default()
                },
                values,
            )
        })
        .wait()
        .unwrap();

    let pasted = run(
        &document,
        &registry,
        "archbuilder.clipboard.paste",
        json!({"objects": [source.to_string()], "displacement": [5.0, 0.0]}),
    );
    assert_eq!(pasted["created"].as_array().unwrap().len(), 1);

    let out = export_ifc(&document.snapshot(), &ExportOptions::default()).unwrap();
    assert!(out.valid, "{:#?}", out.report.items);
    let text = String::from_utf8_lossy(&out.bytes);
    let ids = column_ids(&text);
    assert_eq!(ids.len(), 2, "{text}");
    let distinct: BTreeSet<&String> = ids.iter().collect();
    assert_eq!(distinct.len(), 2, "the copy repeats the source's GlobalId");
}
