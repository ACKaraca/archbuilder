//! Every opening kind of the opening capabilities in one exported file: recipe doors and windows
//! (with their fills), an opening of a sketched outline, a stacked opening, a door moved to
//! another wall and a dormer opening in a roof. The file reads back and passes self-validation;
//! set `AB_OPENINGS_IFC` to also write it, for the independent EXPRESS check.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_io::{ExportOptions, export_ifc};
use serde_json::{Value as Json, json};

fn run(document: &Document, registry: &arc_command::Registry, command: &str, params: Json) -> Json {
    let session = Session::new(Surface::Ui, [Permission::Read, Permission::Edit]);
    let invocation = Invocation {
        seed: 99,
        ..Invocation::new(command, params)
    };
    match Invoker::new(registry).invoke(document, &session, invocation) {
        Ok(o) => o.value,
        Err(e) => panic!("{command}: {e:?}"),
    }
}

#[test]
fn recipes_outlines_stacked_openings_and_dormers_export_as_valid_voids_and_fills() {
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
            json!({"start": corners[k], "end": corners[(k + 1) % 4], "height": 3.0}),
        );
        walls.push(made["wall"].as_str().unwrap().to_owned());
    }
    for (command, distance, operation) in [
        ("door", 1.5, "folding"),
        ("door", 3.5, "pivot"),
        ("window", 5.5, "awning"),
        ("window", 7.0, "hopper"),
        ("window", 8.5, "pivot"),
    ] {
        r(
            &format!("archbuilder.{command}.place"),
            json!({"wall": walls[0], "distance": distance, "operation": operation, "width": 1.4}),
        );
    }
    r(
        "archbuilder.window.place",
        json!({"wall": walls[2], "distance": 2.0, "operation": "folding", "width": 1.8}),
    );
    // An opening of a sketched outline, and one above a door at the same stretch.
    r(
        "archbuilder.opening.place",
        json!({"wall": walls[2], "profile": [[5.0, 0.0], [7.0, 0.0], [7.0, 1.5], [6.0, 2.5], [5.0, 1.5]]}),
    );
    let door = r(
        "archbuilder.door.place",
        json!({"wall": walls[1], "distance": 3.0}),
    );
    r(
        "archbuilder.opening.place",
        json!({"wall": walls[1], "distance": 3.0, "width": 0.9, "height": 0.5, "sill": 2.3}),
    );
    // The door changes wall.
    r(
        "archbuilder.insert.rehost",
        json!({"inserts": [door["insert"]], "wall": walls[3], "distance": 3.0}),
    );
    let roof = r(
        "archbuilder.roof.create",
        json!({"picks": [[5.0, 3.0]], "slope": std::f64::consts::FRAC_PI_4}),
    );
    let dormer: Vec<String> = [
        ([4.0, 2.0], [4.0, 4.0]),
        ([4.0, 4.0], [6.0, 4.0]),
        ([6.0, 4.0], [6.0, 2.0]),
    ]
    .iter()
    .map(|(a, b)| {
        r(
            "archbuilder.wall.create",
            json!({"start": a, "end": b, "height": 1.0}),
        )["wall"]
            .as_str()
            .unwrap()
            .to_owned()
    })
    .collect();
    r(
        "archbuilder.dormer_opening.create",
        json!({"roof": roof["roof"], "walls": dormer}),
    );
    let snapshot = document.snapshot();
    let out = export_ifc(&snapshot, &ExportOptions::default()).unwrap();
    assert!(out.valid, "{:#?}", out.report.items);
    let text = String::from_utf8(out.bytes.clone()).unwrap();
    // Seven fills with a void each, the sketched and the stacked openings, the dormer's cut.
    assert!(text.matches("IFCRELFILLSELEMENT").count() >= 7);
    assert!(text.matches("IFCOPENINGELEMENT").count() >= 10);
    assert!(text.contains("Qto_DoorBaseQuantities") && text.contains("Qto_WindowBaseQuantities"));
    if let Ok(path) = std::env::var("AB_OPENINGS_IFC") {
        std::fs::write(path, &out.bytes).unwrap();
    }
}
