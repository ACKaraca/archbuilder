//! The IFC export of ceilings, wall openings, shafts and vertical openings, read back: ceilings
//! are `IfcCovering` elements of the predefined type `CEILING` with an `IfcCoveringType`, and every
//! opening voids its host with `IfcRelVoidsElement`, a shaft once for each host it cuts.

#![allow(clippy::unwrap_used, clippy::too_many_lines, reason = "test code")]

use std::sync::Arc;

use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use arc_io_ifc::{Parameter, ReadOptions, read};
use archbuilder_io::{ExportOptions, export_ifc};
use archbuilder_model::spatial;
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
fn ceilings_and_openings_are_coverings_and_voids() {
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
    let storeys: Vec<String> = {
        let s = document.snapshot();
        let mut found: Vec<_> = spatial::objects(&s, archbuilder_model::types::STOREY)
            .iter()
            .map(|o| (spatial::storey_elevation(&s, o), o.id.to_string()))
            .collect();
        found.sort_by(|a, b| a.0.total_cmp(&b.0));
        found.into_iter().map(|(_, id)| id).collect()
    };
    let c = [[0.0, 0.0], [0.0, 6.0], [10.0, 6.0], [10.0, 0.0]];
    let mut walls = Vec::new();
    for storey in &storeys {
        for k in 0..4 {
            let made = run(
                &document,
                &registry,
                "archbuilder.wall.create",
                json!({"start": c[k], "end": c[(k + 1) % 4], "storey": storey}),
            );
            walls.push(made["wall"].as_str().unwrap().to_owned());
        }
        run(
            &document,
            &registry,
            "archbuilder.floor.create",
            json!({"picks": [[5.0, 3.0]], "storey": storey}),
        );
    }
    // A flat ceiling, a sloped one, an arched wall opening, a shaft through the storeys and a
    // vertical opening in the first floor.
    run(
        &document,
        &registry,
        "archbuilder.ceiling.create",
        json!({"picks": [[5.0, 3.0]], "storey": storeys[0]}),
    );
    run(
        &document,
        &registry,
        "archbuilder.ceiling.create",
        json!({"picks": [[5.0, 3.0]], "storey": storeys[1], "offset": 2.4,
               "slope_from": [0.1, 3.0], "slope_to": [9.9, 3.0], "end_height": 2.7}),
    );
    run(
        &document,
        &registry,
        "archbuilder.opening.place",
        json!({"wall": walls[0], "distance": 3.0, "width": 1.2, "height": 2.2, "rise": 0.5}),
    );
    run(
        &document,
        &registry,
        "archbuilder.shaft.create",
        json!({"storey": storeys[0], "points": [[4.0, 2.0], [5.0, 2.0], [5.0, 3.0], [4.0, 3.0]],
               "top_offset": 5.0}),
    );
    let floors = {
        let s = document.snapshot();
        spatial::children(
            &s,
            archbuilder_model::types::FLOOR,
            spatial::get(&s, storeys[1].parse().unwrap()).unwrap().id,
        )
    };
    run(
        &document,
        &registry,
        "archbuilder.vertical_opening.create",
        json!({"host": floors[0].id.to_string(),
               "points": [[6.0, 1.0], [7.0, 1.0], [7.0, 2.0], [6.0, 2.0]]}),
    );
    let snapshot = document.snapshot();
    let before = export_ifc(&snapshot, &ExportOptions::default()).unwrap();
    assert!(before.valid, "{:?}", before.report.items);
    let file = read(&before.bytes, &mut ReadOptions::default())
        .unwrap()
        .file;
    assert_eq!(file.instances_of("IFCCOVERING").count(), 2);
    for covering in file.instances_of("IFCCOVERING") {
        assert_eq!(
            covering.records[0].parameters[8],
            Parameter::Enumeration("CEILING".into())
        );
    }
    // The two default ceiling types are in the file; the ceilings use one of them.
    assert_eq!(file.instances_of("IFCCOVERINGTYPE").count(), 2);
    assert!(file.instances_of("IFCMATERIALLAYERSET").count() >= 2);
    // Voids: the arched wall opening, the shaft in the second and third floors and the first
    // ceiling above it (the first storey's ceiling is at 2.6 m; the shaft runs from 0 to 5), the
    // sloped ceiling of the second storey and the vertical opening: more than the doors alone.
    let voids = file.instances_of("IFCRELVOIDSELEMENT").count();
    assert!(voids >= 6, "{voids}");
    let openings = file.instances_of("IFCOPENINGELEMENT").count();
    assert_eq!(openings, voids);
    // The arched opening is the one that was approximated.
    assert!(
        before
            .report
            .items
            .iter()
            .any(|i| i.reason == "ab.io.ifc.arc_faceted")
    );
}

#[test]
fn a_shaft_larger_than_its_host_is_no_void_of_it_and_the_report_says_so() {
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
        "archbuilder.floor.create",
        json!({"points": [[2.0, 2.0], [4.0, 2.0], [4.0, 4.0], [2.0, 4.0]]}),
    );
    run(
        &document,
        &registry,
        "archbuilder.shaft.create",
        json!({"points": [[0.0, 0.0], [6.0, 0.0], [6.0, 6.0], [0.0, 6.0]],
               "base_offset": -0.5, "top_offset": 1.0}),
    );
    let out = export_ifc(&document.snapshot(), &ExportOptions::default()).unwrap();
    assert!(out.valid, "{:?}", out.report.items);
    let file = read(&out.bytes, &mut ReadOptions::default()).unwrap().file;
    assert_eq!(file.instances_of("IFCRELVOIDSELEMENT").count(), 0);
    assert!(
        out.report
            .items
            .iter()
            .any(|i| i.reason == "ab.io.ifc.cut_not_applied")
    );
}
