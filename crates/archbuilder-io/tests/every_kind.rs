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
    // An arc wall, a slanted wall and a curved curtain wall.
    r(
        "archbuilder.wall.create",
        json!({"start": [20.0, 5.0], "end": [26.0, 5.0], "bulge": 0.4}),
    );
    let slanted = r(
        "archbuilder.wall.create",
        json!({"start": [20.0, 8.0], "end": [24.0, 8.0]}),
    );
    r(
        "archbuilder.wall.set_lean",
        json!({"walls": [slanted["wall"]], "degrees": 10.0}),
    );
    r(
        "archbuilder.wall.create",
        json!({"start": [20.0, 12.0], "end": [26.0, 12.0], "type": cw["type"], "bulge": 0.3}),
    );
    let floor = r(
        "archbuilder.floor.create",
        json!({"points": [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]]}),
    );
    let floor_edge = r(
        "archbuilder.floor_edge.create",
        json!({"floor": floor["floor"]}),
    );
    // Railings hosted on a floor edge of the slab and on the edge element.
    r(
        "archbuilder.railing.create",
        json!({"host": floor["floor"], "host_edge": 0}),
    );
    r(
        "archbuilder.railing.create",
        json!({"host": floor_edge["floor_edge"]}),
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
    // A ramp of three runs with two quarter turns the same way and landings at both ends, and a
    // ramp by sketch.
    r(
        "archbuilder.ramp.create",
        json!({"start": [80.0, 0.0], "to": [86.0, 0.0], "rise": 1.5, "landing_start": 1.0,
               "landing_end": 2.0, "runs": [
                   {"turn": "90", "length": 6.0, "landing": 2.0},
                   {"turn": "90", "length": 6.0, "landing": 1.5}]}),
    );
    r(
        "archbuilder.ramp.create",
        json!({"start": [100.0, 2.0], "to": [105.0, 2.0], "rise": 0.4, "landing_start": 0.5,
               "landing_end": 1.0,
               "boundary": [[100.0, 1.0], [106.5, 1.0], [106.5, 3.0], [100.0, 3.0]]}),
    );
    r(
        "archbuilder.grid.create",
        json!({"start": [-2.0, 0.0], "end": [-2.0, 10.0]}),
    );
    r(
        "archbuilder.grid.create",
        json!({"start": [-3.0, 8.0], "end": [14.0, 8.0]}),
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

    // A family instance has no IFC mapping yet: it is listed in the loss report.
    {
        use archbuilder_model::family::fixtures::table;
        use archbuilder_model::family::loaded::{Component, FamilyType, Loaded};
        use archbuilder_model::family::package::publish;
        let s = document.snapshot();
        let project = spatial::project(&s).unwrap().id;
        let storey = spatial::objects(&s, types::STOREY)[0].id;
        let definition = table();
        let published = publish(
            &definition,
            None,
            archbuilder_model::family::Version {
                major: 1,
                minor: 0,
                patch: 0,
            },
        )
        .unwrap();
        let values = definition.type_named("1400x800").unwrap().values.clone();
        document
            .submit(move |txn| {
                use arc_base::id::Identifier;
                let owned = |owner| arc_doc::Header {
                    owner: Some(owner),
                    ..Default::default()
                };
                let (f, t, c) = (
                    arc_base::ObjectId::from_u128(0xC1),
                    arc_base::ObjectId::from_u128(0xC2),
                    arc_base::ObjectId::from_u128(0xC3),
                );
                txn.create(
                    types::FAMILY,
                    f,
                    owned(project),
                    Loaded::values(&published, &definition),
                )?;
                txn.create(
                    types::FAMILY_TYPE,
                    t,
                    owned(project),
                    FamilyType::record("1400x800", f, &values),
                )?;
                let comp = Component {
                    id: c,
                    storey: Some(storey),
                    name: String::new(),
                    family_type: t,
                    offset: 0.0,
                    origin: [30.0, 30.0],
                    rotation: 0.0,
                    mirror: false,
                    values: Default::default(),
                };
                txn.create(types::COMPONENT, c, owned(storey), comp.record())
            })
            .wait()
            .unwrap();
    }
    let s = document.snapshot();
    for key in ELEMENTS
        .iter()
        .chain(&[types::WORK_PLANE, types::SELECTION_SET])
    {
        assert!(!spatial::objects(&s, key).is_empty(), "no {key}");
    }
    let out = export_ifc(&s, &ExportOptions::default()).unwrap();
    assert!(out.valid, "{:#?}", out.report.items);
    let text = String::from_utf8_lossy(&out.bytes);
    let count = |entity: &str| text.matches(&format!("={entity}(")).count();
    // One flight per run: one each for the ramp of one run and the ramp by sketch, three for the
    // ramp of three runs; that one's landings are at the foot, at both turns and at the top.
    assert_eq!(count("IFCRAMPFLIGHT"), 5);
    assert!(
        text.contains(".TWO_QUARTER_TURN_RAMP."),
        "no two-quarter-turn ramp"
    );
    assert!(text.contains(".STRAIGHT_RUN_RAMP."), "no straight ramp");
    // Railings: the two sides of the stair, the one by path, two on each ramp and the two on the
    // floor.
    assert_eq!(count("IFCRAILING"), 11);
    if let Ok(path) = std::env::var("AB_EVERY_KIND_IFC") {
        std::fs::write(path, &out.bytes).unwrap();
    }
    // Only what has no IFC building element is named in the report.
    let mut reasons: Vec<&str> = out.report.items.iter().map(|i| i.reason.as_str()).collect();
    reasons.sort_unstable();
    reasons.dedup();
    assert!(
        !reasons.iter().any(|r| r.ends_with("_not_written")
            && !matches!(
                *r,
                "ab.io.ifc.datum_not_written" | "ab.io.ifc.family_not_written"
            )),
        "{reasons:?}"
    );
    for reason in [
        "ab.io.ifc.family_not_written",
        "ab.io.ifc.dormer_as_parts",
        "ab.io.ifc.fascia_as_covering",
    ] {
        assert!(reasons.contains(&reason), "{reason} not reported");
    }
    // The standard property sets of the kinds the model knows values for.
    for set in [
        "Pset_StairCommon",
        "Pset_StairFlightCommon",
        "Pset_RampCommon",
        "Pset_RampFlightCommon",
        "Pset_RailingCommon",
        "Pset_DoorCommon",
        "Pset_WindowCommon",
        "Pset_SlabCommon",
        "Pset_SpaceCommon",
        "Pset_TransportElementElevator",
    ] {
        assert!(text.contains(&format!("'{set}'")), "{set} not written");
    }
    // Conformance is checked only for a schema the build has tables of; IFC4 has none here, so
    // the export must not claim it.
    assert!(out.conformance.is_none() || out.valid);
    // An elevator is a transport element with its type, a gutter a pipe segment, a selection set a
    // group with the walls it names.
    assert_eq!(count("IFCTRANSPORTELEMENT"), 1);
    assert_eq!(count("IFCTRANSPORTELEMENTTYPE"), 1);
    assert!(text.contains(".ELEVATOR."));
    assert_eq!(count("IFCPIPESEGMENT"), 1);
    assert!(text.contains(".GUTTER."));
    assert_eq!(count("IFCGROUP"), 1);
    assert_eq!(count("IFCRELASSIGNSTOGROUP"), 1);
    // A dormer is written as the opening in its roof, walls and a roof; a fascia as a molding.
    for written in [
        "Dormer 1 (front)",
        "Dormer 1 (cheek)",
        "Dormer 1 (roof)",
        "Fascia 1",
    ] {
        assert!(text.contains(written), "{written} not written");
    }
}
