//! One document with every element kind, exported: the file reads back and passes
//! self-validation, and every kind not written is named in the loss report.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

use std::sync::Arc;

use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_io::{ExportOptions, export_ifc};
use archbuilder_model::spatial;
use archbuilder_model::types::{self, ELEMENTS};
use serde_json::{Value as Json, json};

fn run(document: &Document, registry: &arc_command::Registry, command: &str, params: Json) -> Json {
    let session = Session::new(Surface::Ui, [Permission::Read, Permission::Edit]);
    let invocation = Invocation {
        seed: 4_242,
        ..Invocation::new(command, params)
    };
    match Invoker::new(registry).invoke(document, &session, invocation) {
        Ok(o) => o.value,
        Err(e) => panic!("{command}: {e:?}"),
    }
}

#[test]
fn a_document_with_every_element_kind_exports_and_validates() {
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
    let r = |c: &str, p: Json| run(&document, &registry, c, p);
    r("archbuilder.project.new", json!({}));
    let corners = [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]];
    let mut walls = Vec::new();
    for k in 0..4 {
        let made = r(
            "archbuilder.wall.create",
            json!({"start": corners[k], "end": corners[(k + 1) % 4]}),
        );
        walls.push(made["wall"].as_str().unwrap().to_owned());
    }
    r(
        "archbuilder.door.place",
        json!({"wall": walls[0], "distance": 2.0}),
    );
    r(
        "archbuilder.window.place",
        json!({"wall": walls[0], "distance": 6.0}),
    );
    r(
        "archbuilder.opening.place",
        json!({"wall": walls[2], "distance": 3.0, "width": 1.0, "height": 2.1}),
    );
    let cw = r(
        "archbuilder.curtain.type",
        json!({"name": "CW", "vertical": {"kind": "fixed_distance", "value": 1.0}}),
    );
    r(
        "archbuilder.wall.create",
        json!({"start": [10.0, 1.0], "end": [10.0, 4.0], "type": cw["type"], "height": 2.5}),
    );
    r(
        "archbuilder.wall.create",
        json!({"start": [20.0, 0.0], "end": [24.0, 0.0], "type": cw["type"]}),
    );
    let floor = r(
        "archbuilder.floor.create",
        json!({"points": [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]]}),
    );
    r(
        "archbuilder.floor_edge.create",
        json!({"floor": floor["floor"]}),
    );
    let sloped = r(
        "archbuilder.floor.create",
        json!({"points": [[30.0, 0.0], [34.0, 0.0], [34.0, 4.0], [30.0, 4.0]]}),
    );
    r(
        "archbuilder.floor.set_slope",
        json!({"floor": sloped["floor"], "from": [30.0, 2.0], "to": [34.0, 2.0], "slope": 0.1}),
    );
    r(
        "archbuilder.vertical_opening.create",
        json!({"host": floor["floor"],
        "points": [[1.0, 1.0], [2.0, 1.0], [2.0, 2.0], [1.0, 2.0]]}),
    );
    r("archbuilder.room.place", json!({"points": [[5.0, 3.0]]}));
    r("archbuilder.ceiling.create", json!({"picks": [[5.0, 3.0]]}));
    let roof = r(
        "archbuilder.roof.create",
        json!({"picks": [[5.0, 3.0]], "slope": 0.5}),
    );
    r(
        "archbuilder.dormer.create",
        json!({"roof": roof["roof"], "center": [5.0, 1.2], "depth": 0.8}),
    );
    r(
        "archbuilder.roof_edge.create",
        json!({"roof": roof["roof"], "role": "fascia"}),
    );
    r(
        "archbuilder.roof_edge.create",
        json!({"roof": roof["roof"], "role": "gutter"}),
    );
    r(
        "archbuilder.elevator.create",
        json!({"center": [-8.0, 3.0]}),
    );
    r(
        "archbuilder.roof_extrusion.create",
        json!({"plane": [[40.0, 0.0], [41.0, 0.0]], "profile": [[0.0, 2.5], [6.0, 3.5]],
               "start": 0.0, "end": 5.0}),
    );
    let storey = {
        let s = document.snapshot();
        spatial::storeys(&s, spatial::buildings(&s)[0].id)[0]
            .storey
            .id
    };
    r(
        "archbuilder.shaft.create",
        json!({"storey": storey.to_string(),
        "points": [[7.0, 4.0], [8.0, 4.0], [8.0, 5.0], [7.0, 5.0]], "top_offset": 1.0}),
    );
    r(
        "archbuilder.stair.create",
        json!({"start": [50.0, 0.0], "direction": 0.0}),
    );
    r(
        "archbuilder.ramp.create",
        json!({"start": [60.0, 5.0], "to": [66.0, 5.0]}),
    );
    r(
        "archbuilder.railing.create",
        json!({"points": [[70.0, 0.0], [73.0, 0.0], [73.0, 2.0]]}),
    );
    r(
        "archbuilder.grid.create",
        json!({"start": [-2.0, 0.0], "end": [-2.0, 10.0]}),
    );
    r(
        "archbuilder.reference_plane.create",
        json!({"start": [0.0, -2.0], "end": [4.0, -2.0]}),
    );
    r(
        "archbuilder.work_plane.set",
        json!({"points": [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]}),
    );
    r(
        "archbuilder.set.create",
        json!({"name": "All walls", "objects": walls}),
    );

    let s = document.snapshot();
    for key in ELEMENTS
        .iter()
        .chain(&[types::WORK_PLANE, types::SELECTION_SET])
    {
        assert!(!spatial::objects(&s, key).is_empty(), "no {key}");
    }
    let out = export_ifc(&s, &ExportOptions::default()).unwrap();
    assert!(out.valid, "{:#?}", out.report.items);
    if let Ok(path) = std::env::var("AB_EVERY_KIND_IFC") {
        std::fs::write(path, &out.bytes).unwrap();
    }
    // A dormer is written as the opening in its roof, walls and a roof; a fascia as a molding.
    let text = String::from_utf8_lossy(&out.bytes);
    for written in [
        "Dormer 1 (front)",
        "Dormer 1 (cheek)",
        "Dormer 1 (roof)",
        "Fascia 1",
    ] {
        assert!(text.contains(written), "{written} not written");
    }
    assert!(
        !text.contains("Gutter 1"),
        "a gutter is reported, not written"
    );
    // Each kind that has no entity of its own is named in the report.
    for kind in [
        "IFCCURTAINWALL",
        "IFCSTAIR",
        "IFCRAMP",
        "IFCRAILING",
        "IFCTRANSPORTELEMENT",
        "IFCPIPESEGMENT",
        "IFCGRID",
        "IFCGRIDAXIS",
    ] {
        assert!(
            out.report
                .items
                .iter()
                .any(|i| format!("{i:?}").contains(kind)),
            "{kind} not reported"
        );
    }
}
