//! Group instances in the IFC export (AB-PHASE-010, AB-PHASE-013): each instance is an `IfcGroup`
//! that relates the elements it generated, which are written as ordinary elements of their storeys.
//! The definition's own members are not part of the building and are not written.
//! With `ARCHBUILDER_IFC_DUMP=<directory>` set, the file is written there for the external oracle.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_io::{ExportOptions, export_ifc};
use archbuilder_model::spatial;
use archbuilder_model::types::{GROUP_INSTANCE, STOREY, WALL};
use serde_json::{Value as Json, json};

struct Rig {
    document: Document,
    registry: arc_command::Registry,
    session: Session,
}

impl Rig {
    fn new() -> Self {
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
        let rig = Self {
            document,
            registry: archbuilder_commands::registry().unwrap(),
            session: Session::new(Surface::Ui, [Permission::Read, Permission::Edit]),
        };
        rig.run("archbuilder.project.new", json!({}));
        rig
    }

    fn run(&self, command: &str, params: Json) -> Json {
        let invocation = Invocation {
            seed: 31,
            ..Invocation::new(command, params)
        };
        Invoker::new(&self.registry)
            .invoke(&self.document, &self.session, invocation)
            .unwrap_or_else(|e| panic!("{command}: {e:?}"))
            .value
    }
}

#[test]
fn a_group_instance_is_an_ifc_group_of_its_elements() {
    let rig = Rig::new();
    let snapshot = rig.document.snapshot();
    let storey = spatial::objects(&snapshot, STOREY)[0].id.to_string();
    let corners = [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]];
    let mut walls = Vec::new();
    for k in 0..4 {
        let made = rig.run(
            "archbuilder.wall.create",
            json!({"storey": storey, "start": corners[k], "end": corners[(k + 1) % 4]}),
        );
        walls.push(made["wall"].as_str().unwrap().to_owned());
    }
    rig.run(
        "archbuilder.door.place",
        json!({"wall": walls[0], "distance": 1.5}),
    );
    let group = rig.run(
        "archbuilder.group.create",
        json!({"name": "Unit", "ids": walls.clone()}),
    )["group"]
        .as_str()
        .unwrap()
        .to_owned();
    rig.run(
        "archbuilder.group.place",
        json!({"group": group, "storey": storey, "origin": [20.0, 0.0]}),
    );

    let snapshot = rig.document.snapshot();
    let instances = spatial::objects(&snapshot, GROUP_INSTANCE);
    assert_eq!(instances.len(), 1);
    let export = export_ifc(&snapshot, &ExportOptions::default()).unwrap();
    let text = String::from_utf8_lossy(&export.bytes).to_string();
    assert!(text.contains("IFCGROUP("), "an IfcGroup is written");
    assert!(
        text.contains("IFCRELASSIGNSTOGROUP("),
        "the group relates its members"
    );
    assert!(
        text.contains("'Group instance'"),
        "the group is typed as a group instance"
    );
    assert!(
        !format!("{:?}", export.report).contains("IFCGROUP"),
        "no loss is reported for the instance"
    );
    assert_eq!(
        spatial::objects(&snapshot, WALL).len(),
        8,
        "four walls and four generated"
    );
    assert!(export.valid);
    if let Ok(dir) = std::env::var("ARCHBUILDER_IFC_DUMP") {
        std::fs::write(
            std::path::Path::new(&dir).join("group_instance.ifc"),
            &export.bytes,
        )
        .unwrap();
    }
}
