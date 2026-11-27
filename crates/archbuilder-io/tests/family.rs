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

fn place_family(
    definition: archbuilder_model::family::Definition,
    type_name: &'static str,
) -> Document {
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
        &definition,
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
                FamilyType::record(type_name, family, &std::collections::BTreeMap::new()),
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
    document
}

#[test]
fn family_instances_are_listed_in_the_loss_report_and_never_become_proxies() {
    let document = place_family(table(), "1400x800");
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

#[test]
fn a_door_family_is_an_ifc_door_with_type_properties_and_stable_ids() {
    let document = place_family(archbuilder_model::family::fixtures::door(), "0900x2100");
    let options = ExportOptions::default();
    let a = export_ifc(&document.snapshot(), &options).unwrap();
    let b = export_ifc(&document.snapshot(), &options).unwrap();
    assert!(a.valid, "{:?}", a.report.items);
    assert_eq!(
        a.bytes, b.bytes,
        "GlobalIds and bytes are the same on every export"
    );
    let text = String::from_utf8_lossy(&a.bytes);
    for needle in [
        "IFCDOOR(",
        "IFCDOORTYPE(",
        "ArchBuilder_Type",
        "ArchBuilder_Instance",
        "ArchBuilder_Family",
        "Pset_DoorCommon",
        "IFCFACETEDBREP",
    ] {
        assert!(text.contains(needle), "{needle}");
    }
    assert!(!text.contains("IFCBUILDINGELEMENTPROXY"));
    assert!(
        !a.report
            .items
            .iter()
            .any(|i| i.reason == "ab.io.ifc.family_not_written")
    );
    if let Ok(dir) = std::env::var("AB_IFC_OUT") {
        std::fs::write(std::path::Path::new(&dir).join("door.ifc"), &a.bytes).unwrap();
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one scenario: setup, three inserts, the export"
)]
fn a_hosted_door_family_is_an_opening_in_its_wall_filled_by_an_ifc_door() {
    use archbuilder_model::family::fixtures::hosted_door;
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
    let mut seed = 5_000;
    let mut run = |command: &str, params: serde_json::Value| {
        seed += 1;
        Invoker::new(&registry)
            .invoke(
                &document,
                &session,
                Invocation {
                    seed,
                    ..Invocation::new(command, params)
                },
            )
            .unwrap()
    };
    run("archbuilder.project.new", json!({}));
    let project = spatial::project(&document.snapshot()).unwrap().id;
    let published = publish(
        &hosted_door(),
        None,
        Version {
            major: 1,
            minor: 0,
            patch: 0,
        },
    )
    .unwrap();
    let definition = published.definition().unwrap();
    let (family, ty) = (
        arc_base::ObjectId::from_u128(0xF10),
        arc_base::ObjectId::from_u128(0xF11),
    );
    document
        .submit(move |txn| {
            let header = Header {
                owner: Some(project),
                ..Header::default()
            };
            txn.create(
                FAMILY,
                family,
                header.clone(),
                Loaded::values(&published, &definition),
            )?;
            txn.create(
                FAMILY_TYPE,
                ty,
                header,
                FamilyType::record("0900x2100", family, &std::collections::BTreeMap::new()),
            )
        })
        .wait()
        .unwrap();
    let wall = run(
        "archbuilder.wall.create",
        json!({"start": [0.0, 0.0], "end": [8.0, 0.0], "height": 3.0, "thickness": 0.2}),
    )
    .value["wall"]
        .as_str()
        .unwrap()
        .to_owned();
    for (distance, extra) in [
        (1.0, json!({})),
        (3.0, json!({"flip_hand": true})),
        (5.0, json!({"flip_facing": true})),
    ] {
        let mut params =
            json!({"host": wall, "type": ty.to_string(), "distance": distance, "name": "D"});
        params
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        run("archbuilder.family_insert.place", params);
    }
    let options = ExportOptions::default();
    let a = export_ifc(&document.snapshot(), &options).unwrap();
    let b = export_ifc(&document.snapshot(), &options).unwrap();
    assert!(a.valid, "{:?}", a.report.items);
    assert_eq!(a.bytes, b.bytes, "GlobalIds are stable across exports");
    let text = String::from_utf8_lossy(&a.bytes);
    let count = |needle: &str| text.matches(needle).count();
    assert_eq!(count("IFCDOOR("), 3);
    assert_eq!(count("IFCOPENINGELEMENT("), 3);
    assert_eq!(count("IFCRELVOIDSELEMENT("), 3);
    assert_eq!(count("IFCRELFILLSELEMENT("), 3);
    let family_types = text
        .lines()
        .filter(|l| l.contains("IFCDOORTYPE(") && l.contains("'0900x2100'"))
        .count();
    assert_eq!(family_types, 1, "one type for the three instances");
    assert!(!text.contains("IFCBUILDINGELEMENTPROXY"));
    if let Ok(dir) = std::env::var("AB_IFC_OUT") {
        std::fs::write(std::path::Path::new(&dir).join("hosted.ifc"), &a.bytes).unwrap();
    }
}
