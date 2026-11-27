//! Family instances in the IFC export, until their mapping exists: each is listed in the loss
//! report with an explicit reason, never written as a proxy, and the export stays valid.

#![allow(clippy::unwrap_used, reason = "test code")]

use std::sync::Arc;

use arc_base::id::Identifier;
use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Header, Snapshot};
use archbuilder_io::{ExportOptions, export_ifc};
use archbuilder_model::family::loaded::{Component, FamilyType, Loaded};
use archbuilder_model::family::package::publish;
use archbuilder_model::family::{Version, fixtures::table};
use archbuilder_model::spatial;
use archbuilder_model::types::{COMPONENT, FAMILY, FAMILY_TYPE, STOREY};
use serde_json::json;

#[test]
fn family_instances_are_listed_in_the_loss_report_and_never_become_proxies() {
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
    let session = Session::new(Surface::Ui, [Permission::Read, Permission::Edit]);
    Invoker::new(&registry)
        .invoke(
            &document,
            &session,
            Invocation {
                seed: 4_040,
                ..Invocation::new("archbuilder.project.new", json!({}))
            },
        )
        .unwrap();
    let snapshot = document.snapshot();
    let project = spatial::project(&snapshot).unwrap().id;
    let storey = spatial::objects(&snapshot, STOREY)[0].id;
    let published = publish(
        &table(),
        None,
        Version {
            major: 1,
            minor: 0,
            patch: 0,
        },
    )
    .unwrap();
    let definition = published.definition().unwrap();
    document
        .submit(move |txn| {
            let header = |owner| Header {
                owner: Some(owner),
                ..Header::default()
            };
            let family = arc_base::ObjectId::from_u128(0xF00D);
            let ty = arc_base::ObjectId::from_u128(0xF00E);
            let comp = arc_base::ObjectId::from_u128(0xF00F);
            txn.create(
                FAMILY,
                family,
                header(project),
                Loaded::values(&published, &definition),
            )?;
            txn.create(
                FAMILY_TYPE,
                ty,
                header(project),
                FamilyType::record("1400x800", family, &std::collections::BTreeMap::new()),
            )?;
            let c = Component {
                id: comp,
                storey: Some(storey),
                name: "T1".into(),
                family_type: ty,
                offset: 0.0,
                origin: [1.0, 1.0],
                rotation: 0.0,
                mirror: false,
                values: std::collections::BTreeMap::new(),
            };
            txn.create(COMPONENT, comp, header(storey), c.record())
        })
        .wait()
        .unwrap();
    let export = export_ifc(&document.snapshot(), &ExportOptions::default()).unwrap();
    assert!(export.valid);
    let text = String::from_utf8_lossy(&export.bytes);
    assert!(!text.contains("IFCBUILDINGELEMENTPROXY"), "no silent proxy");
    let item = export
        .report
        .items
        .iter()
        .find(|i| i.reason == "ab.io.ifc.family_not_written")
        .expect("the instance is in the loss report");
    assert_eq!(item.reason, "ab.io.ifc.family_not_written");
    // The mesh export writes the solids a component shows in 3D.
    let mesh = archbuilder_io::mesh::export_mesh(
        &document.snapshot(),
        &archbuilder_io::mesh::MeshOptions {
            only: Some(std::collections::BTreeSet::from([
                arc_base::ObjectId::from_u128(0xF00F),
            ])),
            ..archbuilder_io::mesh::MeshOptions::default()
        },
    )
    .unwrap();
    assert_eq!(mesh.elements, 1);
    assert!(mesh.triangles > 0);
}
