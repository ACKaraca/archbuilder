//! The IFC export of stairs, ramps, railings, curtain walls and grids, read back with the reader
//! of `arc-io-ifc`: the entities and their attributes, the aggregation of the parts, containment,
//! the property and quantity sets, the types, and the loss report. With `AB_PARTS_IFC_DIR` set,
//! each scenario also writes its file there for an independent validator.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    reason = "test code"
)]

use std::collections::BTreeMap;
use std::sync::Arc;

use arc_base::ObjectId;
use arc_base::id::Identifier;
use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use arc_io_ifc::write::{derived_id, global_id, role};
use arc_io_ifc::{IfcFile, Instance, Parameter, ReadOptions, read};
use archbuilder_io::{Export, ExportOptions, export_ifc};
use archbuilder_model::curtain::{self, PanelKind};
use archbuilder_model::types::{RAILING, RAMP, STAIR};
use archbuilder_model::{ramp, spatial, stair};
use serde_json::{Value as Json, json};

struct Doc {
    document: Document,
    registry: arc_command::Registry,
}

impl Doc {
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
        let doc = Self {
            document,
            registry: archbuilder_commands::registry().unwrap(),
        };
        doc.run("archbuilder.project.new", json!({}));
        doc
    }

    fn run(&self, command: &str, params: Json) -> Json {
        let session = Session::new(Surface::Ui, [Permission::Read, Permission::Edit]);
        let invocation = Invocation {
            seed: 11,
            ..Invocation::new(command, params)
        };
        Invoker::new(&self.registry)
            .invoke(&self.document, &session, invocation)
            .unwrap_or_else(|e| panic!("{command}: {e:?}"))
            .value
    }

    fn make(&self, command: &str, field: &str, params: Json) -> ObjectId {
        self.run(command, params)[field]
            .as_str()
            .unwrap()
            .parse()
            .unwrap()
    }

    fn snapshot(&self) -> Snapshot {
        self.document.snapshot()
    }

    fn export(&self, scenario: &str) -> (Export, IfcFile) {
        let exported = export_ifc(&self.snapshot(), &ExportOptions::default()).unwrap();
        assert!(exported.valid, "{:#?}", exported.report.items);
        if let Ok(dir) = std::env::var("AB_PARTS_IFC_DIR") {
            std::fs::write(format!("{dir}/{scenario}.ifc"), &exported.bytes).unwrap();
        }
        let file = read(&exported.bytes, &mut ReadOptions::default())
            .unwrap()
            .file;
        (exported, file)
    }
}

fn reference(p: &Parameter) -> u64 {
    match p {
        Parameter::Reference(n) => *n,
        other => panic!("{other:?}"),
    }
}

fn references(p: &Parameter) -> Vec<u64> {
    match p {
        Parameter::List(items) => items.iter().map(reference).collect(),
        other => panic!("{other:?}"),
    }
}

fn real(p: &Parameter) -> f64 {
    match p {
        Parameter::Real(v) => *v,
        other => panic!("{other:?}"),
    }
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} vs {b}");
}

fn enumeration(p: &Parameter) -> &str {
    match p {
        Parameter::Enumeration(e) => e,
        other => panic!("{other:?}"),
    }
}

fn text(p: &Parameter) -> &str {
    match p {
        Parameter::Text(t) => t,
        other => panic!("{other:?}"),
    }
}

/// The instance of `class` whose `GlobalId` is that of `id`.
fn find<'a>(file: &'a IfcFile, class: &str, id: u128) -> &'a Instance {
    let wanted = global_id(id);
    file.instances_of(class)
        .find(|i| text(&i.records[0].parameters[0]) == wanted)
        .unwrap_or_else(|| panic!("no {class} {wanted}"))
}

/// The instances an `IfcRelAggregates` makes parts of `whole`.
fn parts<'a>(file: &'a IfcFile, whole: &Instance) -> Vec<&'a Instance> {
    file.instances_of("IFCRELAGGREGATES")
        .filter(|r| reference(&r.records[0].parameters[4]) == whole.name)
        .flat_map(|r| references(&r.records[0].parameters[5]))
        .map(|n| file.instance(n).unwrap())
        .collect()
}

fn class(i: &Instance) -> &str {
    &i.records[0].type_name
}

/// The elements the storeys contain, by instance name.
fn contained(file: &IfcFile) -> Vec<u64> {
    file.instances_of("IFCRELCONTAINEDINSPATIALSTRUCTURE")
        .flat_map(|r| references(&r.records[0].parameters[4]))
        .collect()
}

/// The sets of a product: set name to property or quantity name to its parameter (the typed
/// nominal value of a property, the number of a quantity).
fn sets(file: &IfcFile, product: &Instance) -> BTreeMap<String, BTreeMap<String, Parameter>> {
    let mut out = BTreeMap::new();
    for rel in file.instances_of("IFCRELDEFINESBYPROPERTIES") {
        let p = &rel.records[0].parameters;
        if !references(&p[4]).contains(&product.name) {
            continue;
        }
        let set = file.instance(reference(&p[5])).unwrap();
        let s = &set.records[0].parameters;
        let items = references(if class(set) == "IFCPROPERTYSET" {
            &s[4]
        } else {
            &s[5]
        });
        let mut map = BTreeMap::new();
        for item in items {
            let r = &file.instance(item).unwrap().records[0];
            let value = if r.type_name == "IFCPROPERTYSINGLEVALUE" {
                r.parameters[2].clone()
            } else {
                r.parameters[3].clone()
            };
            map.insert(text(&r.parameters[0]).to_owned(), value);
        }
        assert!(out.insert(text(&s[2]).to_owned(), map).is_none());
    }
    out
}

fn quantities(file: &IfcFile, product: &Instance, set: &str) -> BTreeMap<String, f64> {
    sets(file, product)
        .remove(set)
        .unwrap_or_else(|| panic!("no {set}"))
        .into_iter()
        .map(|(k, v)| (k, real(&v)))
        .collect()
}

/// Whether the product has a body (a representation).
fn has_body(i: &Instance) -> bool {
    !matches!(i.records[0].parameters[6], Parameter::Omitted)
}

fn not_written(exported: &Export) -> Vec<&str> {
    exported
        .report
        .items
        .iter()
        .map(|i| i.reason.as_str())
        .filter(|r| r.ends_with("_not_written"))
        .collect()
}

#[test]
fn a_stair_is_a_stair_of_flights_a_landing_and_railings() {
    let doc = Doc::new();
    let stairs: Vec<(ObjectId, &str, usize)> = [
        ("straight", "STRAIGHT_RUN_STAIR", 1),
        ("l_turn", "QUARTER_TURN_STAIR", 2),
        ("u_turn", "HALF_TURN_STAIR", 2),
    ]
    .iter()
    .enumerate()
    .map(|(k, (shape, predefined, runs))| {
        let id = doc.make(
            "archbuilder.stair.create",
            "stair",
            json!({"start": [10.0 * k as f64, 0.0], "direction": "0", "shape": shape}),
        );
        (id, *predefined, *runs)
    })
    .collect();
    let (exported, file) = doc.export("stairs");
    assert!(
        exported.report.items.is_empty(),
        "{:#?}",
        exported.report.items
    );
    assert_eq!(exported.elements, 3);
    let snapshot = doc.snapshot();
    let in_storeys = contained(&file);
    for (id, predefined, runs) in stairs {
        let object = spatial::get(&snapshot, id).unwrap();
        let layout = stair::Placed::of(&snapshot, &object)
            .unwrap()
            .layout
            .unwrap();
        let whole = find(&file, "IFCSTAIR", id.as_u128());
        // No body of its own, the type named after the shape, contained in the storey.
        assert!(!has_body(whole));
        assert_eq!(enumeration(&whole.records[0].parameters[8]), predefined);
        assert!(in_storeys.contains(&whole.name));
        let all = parts(&file, whole);
        // A part is aggregated, never contained in the storey.
        assert!(all.iter().all(|p| !in_storeys.contains(&p.name)));
        let flights: Vec<&&Instance> = all
            .iter()
            .filter(|p| class(p) == "IFCSTAIRFLIGHT")
            .collect();
        let landings: Vec<&&Instance> = all.iter().filter(|p| class(p) == "IFCSLAB").collect();
        let rails: Vec<&&Instance> = all.iter().filter(|p| class(p) == "IFCRAILING").collect();
        assert_eq!(flights.len(), runs);
        assert_eq!(landings.len(), runs - 1);
        assert_eq!(rails.len(), 2);
        assert_eq!(all.len(), flights.len() + landings.len() + rails.len());
        let mut total = 0.0;
        for (flight, run) in flights.iter().zip(&layout.runs) {
            let p = &flight.records[0].parameters;
            assert!(has_body(flight));
            assert_eq!(p[8], Parameter::Integer(i64::try_from(run.risers).unwrap()));
            assert_eq!(
                p[9],
                Parameter::Integer(i64::try_from(run.risers - 1).unwrap())
            );
            near(real(&p[10]), layout.riser);
            near(real(&p[11]), layout.stair.tread);
            assert_eq!(enumeration(&p[12]), "STRAIGHT");
            let q = quantities(&file, flight, "Qto_StairFlightBaseQuantities");
            near(
                q["Length"],
                (run.risers - 1) as f64 * layout.riser.hypot(layout.stair.tread),
            );
            near(q["GrossVolume"], q["NetVolume"]);
            total += q["GrossVolume"];
        }
        for landing in &landings {
            assert!(has_body(landing));
            assert_eq!(enumeration(&landing.records[0].parameters[8]), "LANDING");
            let q = quantities(&file, landing, "Qto_SlabBaseQuantities");
            near(q["Width"], layout.landing_thickness());
            total += q["GrossVolume"];
        }
        // The concrete of the stair is its flights and its landing.
        near(total, layout.volume());
        for rail in rails {
            assert!(has_body(rail));
            assert_eq!(enumeration(&rail.records[0].parameters[8]), "GUARDRAIL");
            assert!(quantities(&file, rail, "Qto_RailingBaseQuantities")["Length"] > 2.0);
        }
        // The identifiers of the parts come from the stair's.
        for part in all {
            assert_ne!(
                text(&part.records[0].parameters[0]),
                global_id(id.as_u128())
            );
        }
    }
    let again = doc.export("stairs").0;
    assert_eq!(again.bytes, exported.bytes);
}

#[test]
fn a_ramp_is_a_ramp_of_a_flight_landings_and_railings() {
    let doc = Doc::new();
    let slab = doc.make(
        "archbuilder.ramp.create",
        "ramp",
        json!({"start": [1.0, 0.0], "to": [7.0, 0.0], "landing_start": 1.5, "landing_end": 2.0}),
    );
    let solid = doc.make(
        "archbuilder.ramp.create",
        "ramp",
        json!({"start": [1.0, 10.0], "to": [7.0, 10.0], "form": "solid", "rise": 0.4,
               "landing_end": 1.0, "railings": ["left"]}),
    );
    let free = doc.make(
        "archbuilder.railing.create",
        "railing",
        json!({"points": [[20.0, 0.0], [23.0, 0.0], [23.0, 2.0]]}),
    );
    let (exported, file) = doc.export("ramps");
    assert!(not_written(&exported).is_empty());
    assert!(
        exported.report.items.is_empty(),
        "{:#?}",
        exported.report.items
    );
    // Two ramps and three railings by their own record: the two of the first, one of the second.
    assert_eq!(exported.elements, 2 + 3 + 1);
    let snapshot = doc.snapshot();
    let in_storeys = contained(&file);
    for (id, landings, hosted) in [(slab, 2, 2), (solid, 1, 1)] {
        let object = spatial::get(&snapshot, id).unwrap();
        let layout = ramp::Placed::of(&snapshot, &object)
            .unwrap()
            .layout
            .unwrap();
        let whole = find(&file, "IFCRAMP", id.as_u128());
        assert_eq!(
            enumeration(&whole.records[0].parameters[8]),
            "STRAIGHT_RUN_RAMP"
        );
        assert!(!has_body(whole) && in_storeys.contains(&whole.name));
        let all = parts(&file, whole);
        assert!(all.iter().all(|p| !in_storeys.contains(&p.name)));
        let of = |c: &str| all.iter().filter(|p| class(p) == c).collect::<Vec<_>>();
        let flights = of("IFCRAMPFLIGHT");
        assert_eq!(flights.len(), 1);
        assert_eq!(of("IFCSLAB").len(), landings);
        let rails = of("IFCRAILING");
        assert_eq!(rails.len(), hosted);
        for rail in rails {
            assert_eq!(enumeration(&rail.records[0].parameters[8]), "GUARDRAIL");
            assert!(has_body(rail));
        }
        let flight = flights[0];
        assert_eq!(enumeration(&flight.records[0].parameters[8]), "STRAIGHT");
        let q = quantities(&file, flight, "Qto_RampFlightBaseQuantities");
        let slope = layout.ramp.length.hypot(layout.height);
        near(q["Length"], slope);
        near(q["GrossArea"], slope * layout.ramp.width);
        // A slab has one thickness; a solid down to the floor construction has none.
        assert_eq!(
            q.get("Width").copied(),
            (layout.ramp.form == ramp::Form::Slab).then_some(layout.ramp.thickness)
        );
        // The flight and the landings are the body of the ramp.
        let mut volume = q["GrossVolume"];
        for landing in of("IFCSLAB") {
            assert_eq!(enumeration(&landing.records[0].parameters[8]), "LANDING");
            volume += quantities(&file, landing, "Qto_SlabBaseQuantities")["GrossVolume"];
        }
        near(volume, layout.volume());
    }
    // A railing by path is a balustrade in its storey; its length is the path's.
    let whole = find(&file, "IFCRAILING", free.as_u128());
    assert_eq!(enumeration(&whole.records[0].parameters[8]), "BALUSTRADE");
    assert!(in_storeys.contains(&whole.name) && has_body(whole));
    let object = spatial::get(&snapshot, free).unwrap();
    let derived = archbuilder_model::railing::Derived::of(&snapshot, &object).unwrap();
    near(
        quantities(&file, whole, "Qto_RailingBaseQuantities")["Length"],
        derived.length(),
    );
    assert_eq!(spatial::objects(&snapshot, RAILING).len(), 4);
    assert_eq!(spatial::objects(&snapshot, RAMP).len(), 2);
    assert!(spatial::objects(&snapshot, STAIR).is_empty());
}

/// Names the hosted railings' hosts and checks the ids of every part are stable and distinct.
#[test]
fn the_identifiers_of_parts_derive_from_their_whole() {
    let doc = Doc::new();
    let id = doc.make(
        "archbuilder.stair.create",
        "stair",
        json!({"start": [0.0, 0.0], "shape": "l_turn"}),
    );
    let (_, file) = doc.export("ids");
    let whole = find(&file, "IFCSTAIR", id.as_u128());
    let wanted = [
        derived_id(id.as_u128(), role::CALLER_FIRST + 1, 0),
        derived_id(id.as_u128(), role::CALLER_FIRST + 1, 1),
        derived_id(id.as_u128(), role::CALLER_FIRST + 2, 0),
        derived_id(id.as_u128(), role::CALLER_FIRST + 3, 0),
        derived_id(id.as_u128(), role::CALLER_FIRST + 3, 1),
    ]
    .map(global_id);
    let got: Vec<String> = parts(&file, whole)
        .iter()
        .map(|p| text(&p.records[0].parameters[0]).to_owned())
        .collect();
    for w in wanted {
        assert!(got.contains(&w), "{w} not in {got:?}");
    }
}

#[test]
fn a_curtain_wall_is_aggregated_plates_and_mullions_with_their_types() {
    let doc = Doc::new();
    let ty = doc.make(
        "archbuilder.curtain.type",
        "type",
        json!({"name": "CW", "vertical": {"kind": "fixed_distance", "value": 1.0}}),
    );
    // Two curtain walls that meet at a corner, so a corner mullion is made; one of them has a
    // door and a window as panels and a solid panel.
    let first = doc.make(
        "archbuilder.wall.create",
        "wall",
        json!({"start": [0.0, 0.0], "end": [4.0, 0.0], "type": ty.to_string(), "height": 3.0}),
    );
    let second = doc.make(
        "archbuilder.wall.create",
        "wall",
        json!({"start": [4.0, 0.0], "end": [4.0, 3.0], "type": ty.to_string(), "height": 3.0}),
    );
    let cells: Vec<String> = {
        let snapshot = doc.snapshot();
        let d = curtain::Derived::of(&snapshot, &spatial::get(&snapshot, first).unwrap()).unwrap();
        d.layout.cells.iter().map(|c| c.key.clone()).collect()
    };
    for (cell, panel) in [
        (&cells[0], "door"),
        (&cells[1], "window"),
        (&cells[2], "solid"),
    ] {
        doc.run(
            "archbuilder.curtain.panel",
            json!({"wall": first.to_string(), "cell": cell, "panel": panel}),
        );
    }
    let (exported, file) = doc.export("curtain");
    assert!(not_written(&exported).is_empty());
    let snapshot = doc.snapshot();
    let in_storeys = contained(&file);
    let mut plates = 0;
    let mut members = 0;
    for id in [first, second] {
        let object = spatial::get(&snapshot, id).unwrap();
        let d = curtain::Derived::of(&snapshot, &object).unwrap();
        let whole = find(&file, "IFCCURTAINWALL", id.as_u128());
        assert!(!has_body(whole) && in_storeys.contains(&whole.name));
        // No wall is written for it.
        assert!(
            !file
                .instances_of("IFCWALL")
                .any(|w| text(&w.records[0].parameters[0]) == global_id(id.as_u128()))
        );
        let all = parts(&file, whole);
        assert!(all.iter().all(|p| !in_storeys.contains(&p.name)));
        let solid = d
            .panels
            .iter()
            .filter(|p| matches!(p.kind, PanelKind::Glazed | PanelKind::Solid))
            .count();
        let of = |c: &str| all.iter().filter(|p| class(p) == c).count();
        assert_eq!(of("IFCPLATE"), solid);
        assert_eq!(of("IFCMEMBER"), d.mullions.len());
        assert_eq!(all.len(), solid + d.mullions.len());
        plates += solid;
        members += d.mullions.len();
        // Every part names the curtain wall's placement frame: the storey's, so it is the wall's.
        let q = quantities(&file, whole, "Qto_CurtainWallQuantities");
        near(q["Length"], d.wall.length());
        near(q["Height"], 3.0);
        near(q["GrossSideArea"], d.wall.length() * 3.0);
        assert!(q["Width"] > 0.0);
        let s = sets(&file, whole);
        assert_eq!(
            s["Pset_CurtainWallCommon"]["IsExternal"],
            Parameter::Typed {
                type_name: "IFCBOOLEAN".into(),
                value: Box::new(Parameter::Enumeration("T".into()))
            }
        );
        for part in all {
            let p = &part.records[0].parameters;
            assert!(has_body(part));
            let s = sets(&file, part);
            if class(part) == "IFCPLATE" {
                assert_eq!(enumeration(&p[8]), "CURTAIN_PANEL");
                assert!(s.contains_key("Pset_PlateCommon"));
                let q = quantities(&file, part, "Qto_PlateBaseQuantities");
                near(q["GrossVolume"], q["Width"] * q["GrossArea"]);
            } else {
                assert_eq!(enumeration(&p[8]), "MULLION");
                assert!(s.contains_key("Pset_MemberCommon"));
                let q = quantities(&file, part, "Qto_MemberBaseQuantities");
                near(q["GrossVolume"], q["CrossSectionArea"] * q["Length"]);
            }
        }
    }
    // The corner mullion exists once for the two walls.
    let corner: Vec<_> = [first, second]
        .iter()
        .flat_map(|id| {
            let object = spatial::get(&snapshot, *id).unwrap();
            curtain::Derived::of(&snapshot, &object)
                .unwrap()
                .mullions
                .into_iter()
                .filter(|m| m.role == curtain::MullionRole::Corner)
        })
        .collect();
    assert_eq!(corner.len(), 1);
    // Types: the wall type, a plate type for each kind of panel, a member type for each position.
    let wall_type = find(&file, "IFCCURTAINWALLTYPE", ty.as_u128());
    assert_eq!(text(&wall_type.records[0].parameters[2]), "CW");
    let typed = |type_name: u64| -> usize {
        file.instances_of("IFCRELDEFINESBYTYPE")
            .filter(|r| reference(&r.records[0].parameters[5]) == type_name)
            .map(|r| references(&r.records[0].parameters[4]).len())
            .sum()
    };
    assert_eq!(typed(wall_type.name), 2);
    let plate_types: Vec<&Instance> = file.instances_of("IFCPLATETYPE").collect();
    let member_types: Vec<&Instance> = file.instances_of("IFCMEMBERTYPE").collect();
    assert_eq!(plate_types.len(), 2);
    assert_eq!(member_types.len(), 3);
    assert_eq!(
        plate_types.iter().map(|t| typed(t.name)).sum::<usize>(),
        plates
    );
    assert_eq!(
        member_types.iter().map(|t| typed(t.name)).sum::<usize>(),
        members
    );
    assert!(
        plate_types
            .iter()
            .all(|t| enumeration(&t.records[0].parameters[9]) == "CURTAIN_PANEL")
    );
    assert!(
        member_types
            .iter()
            .all(|t| enumeration(&t.records[0].parameters[9]) == "MULLION")
    );
    // The door and the window panel are a door and a window that fill openings in the curtain
    // wall, and the report names the approximation.
    let whole = find(&file, "IFCCURTAINWALL", first.as_u128());
    let voided: Vec<u64> = file
        .instances_of("IFCRELVOIDSELEMENT")
        .filter(|r| reference(&r.records[0].parameters[4]) == whole.name)
        .map(|r| reference(&r.records[0].parameters[5]))
        .collect();
    assert_eq!(voided.len(), 2);
    let filled: Vec<&str> = file
        .instances_of("IFCRELFILLSELEMENT")
        .filter(|r| voided.contains(&reference(&r.records[0].parameters[4])))
        .map(|r| {
            class(
                file.instance(reference(&r.records[0].parameters[5]))
                    .unwrap(),
            )
        })
        .collect();
    assert_eq!(filled.len(), 2);
    assert!(filled.contains(&"IFCDOOR") && filled.contains(&"IFCWINDOW"));
    let item = exported
        .report
        .items
        .iter()
        .find(|i| i.reason == "ab.io.ifc.curtain_panel_filled")
        .unwrap();
    assert_eq!(item.object_id, Some(first.to_string()));
    assert_eq!(
        item.arguments.get("count"),
        Some(&arc_io_common::loss::Argument::Int(2))
    );
    assert_eq!(exported.report.items.len(), 1);
    // The door and the window are in the storey; the plates and mullions are not.
    assert_eq!(exported.elements, 2 + 2);
}

#[test]
fn a_curtain_wall_in_a_basic_wall_keeps_its_void_in_the_host() {
    let doc = Doc::new();
    let ty = doc.make("archbuilder.curtain.type", "type", json!({"name": "CW"}));
    let host = doc.make(
        "archbuilder.wall.create",
        "wall",
        json!({"start": [20.0, 0.0], "end": [28.0, 0.0], "height": 3.0}),
    );
    let curtain = doc.make(
        "archbuilder.wall.create",
        "wall",
        json!({"start": [22.0, 0.0], "end": [25.0, 0.0], "height": 2.0, "type": ty.to_string()}),
    );
    let (exported, file) = doc.export("curtain_embedded");
    assert!(
        exported.report.items.is_empty(),
        "{:?}",
        exported.report.items
    );
    let opening = global_id(derived_id(curtain.as_u128(), role::INSERT_OPENING, 0));
    let voids = file.instances_of("IFCRELVOIDSELEMENT").find(|r| {
        let relating = file
            .instance(reference(&r.records[0].parameters[4]))
            .unwrap();
        let related = file
            .instance(reference(&r.records[0].parameters[5]))
            .unwrap();
        text(&relating.records[0].parameters[0]) == global_id(host.as_u128())
            && text(&related.records[0].parameters[0]) == opening
    });
    assert!(voids.is_some());
    find(&file, "IFCCURTAINWALL", curtain.as_u128());
    assert!(
        !file
            .instances_of("IFCWALL")
            .any(|w| text(&w.records[0].parameters[0]) == global_id(curtain.as_u128()))
    );
}

#[test]
fn grids_are_one_grid_with_u_and_v_axes() {
    let doc = Doc::new();
    let grid = |start: [f64; 2], end: [f64; 2], label: &str, bulge: Option<f64>| {
        let mut p = json!({"start": start, "end": end, "label": label});
        if let Some(b) = bulge {
            p["bulge"] = json!(b);
        }
        doc.run("archbuilder.grid.create", p);
    };
    grid([0.0, -1.0], [0.0, 9.0], "A", None);
    grid([4.0, -1.0], [4.0, 9.0], "B", None);
    grid([-1.0, 0.0], [9.0, 0.0], "1", None);
    grid([-1.0, 8.0], [9.0, 8.0], "2", None);
    let (exported, file) = doc.export("grid_rectangular");
    assert!(exported.report.items.is_empty());
    let whole = file.instances_of("IFCGRID").next().unwrap();
    let p = &whole.records[0].parameters;
    assert_eq!(enumeration(&p[10]), "RECTANGULAR");
    let tags = |list: &Parameter| -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = references(list)
            .into_iter()
            .map(|n| {
                let axis = file.instance(n).unwrap();
                let a = &axis.records[0].parameters;
                let curve = class(file.instance(reference(&a[1])).unwrap()).to_owned();
                assert_eq!(a[2], Parameter::Enumeration("T".into()));
                (text(&a[0]).to_owned(), curve)
            })
            .collect();
        out.sort();
        out
    };
    let line = "IFCPOLYLINE".to_owned();
    // The first line starts the U family: the two lines parallel to it, then the cross lines.
    assert_eq!(
        tags(&p[7]),
        [
            ("A".to_owned(), line.clone()),
            ("B".to_owned(), line.clone())
        ]
    );
    assert_eq!(
        tags(&p[8]),
        [("1".to_owned(), line.clone()), ("2".to_owned(), line)]
    );
    assert_eq!(p[9], Parameter::Omitted);
    // Contained in the lowest storey, one grid for the building.
    let lowest = file
        .instances_of("IFCBUILDINGSTOREY")
        .min_by(|a, b| {
            real(&a.records[0].parameters[9]).total_cmp(&real(&b.records[0].parameters[9]))
        })
        .unwrap();
    let rel = file
        .instances_of("IFCRELCONTAINEDINSPATIALSTRUCTURE")
        .find(|r| reference(&r.records[0].parameters[5]) == lowest.name)
        .unwrap();
    assert!(references(&rel.records[0].parameters[4]).contains(&whole.name));
    assert_eq!(file.instances_of("IFCGRID").count(), 1);
    // The footprint representation holds the axis curves.
    assert!(has_body(whole));
    // An arc is a trimmed circle: counterclockwise from its start for a positive bulge, and
    // against the axis for a negative one.
    grid([10.0, 0.0], [10.0, 6.0], "C", Some(0.5));
    grid([12.0, 0.0], [12.0, 6.0], "D", Some(-0.5));
    let (exported, file) = doc.export("grid_arcs");
    assert!(exported.report.items.is_empty());
    let whole = file.instances_of("IFCGRID").next().unwrap();
    let p = &whole.records[0].parameters;
    assert_eq!(enumeration(&p[10]), "IRREGULAR");
    let arcs: Vec<(String, bool)> = references(&p[8])
        .into_iter()
        .chain(references(&p[7]))
        .filter_map(|n| {
            let a = &file.instance(n).unwrap().records[0].parameters;
            (class(file.instance(reference(&a[1])).unwrap()) == "IFCTRIMMEDCURVE").then(|| {
                (
                    text(&a[0]).to_owned(),
                    a[2] == Parameter::Enumeration("T".into()),
                )
            })
        })
        .collect();
    assert_eq!(arcs.len(), 2);
    assert!(arcs.contains(&("C".to_owned(), true)));
    assert!(arcs.contains(&("D".to_owned(), false)));
}

#[test]
fn grids_of_one_direction_are_reported_and_not_padded() {
    let doc = Doc::new();
    doc.run(
        "archbuilder.grid.create",
        json!({"start": [0.0, 0.0], "end": [0.0, 5.0]}),
    );
    doc.run(
        "archbuilder.grid.create",
        json!({"start": [4.0, 0.0], "end": [4.0, 5.0]}),
    );
    let (exported, file) = doc.export("grid_one_direction");
    assert_eq!(file.instances_of("IFCGRID").count(), 0);
    assert_eq!(file.instances_of("IFCGRIDAXIS").count(), 0);
    assert_eq!(exported.report.items.len(), 2);
    for item in &exported.report.items {
        assert_eq!(item.reason, "ab.io.ifc.grid_one_direction");
        assert_eq!(item.record_kind.as_deref(), Some("IFCGRID"));
    }
}

#[test]
fn a_grid_of_several_segments_is_reported_not_shortened() {
    let doc = Doc::new();
    doc.run(
        "archbuilder.grid.create",
        json!({"start": [0.0, 0.0], "end": [0.0, 8.0], "label": "A"}),
    );
    doc.run(
        "archbuilder.grid.create",
        json!({"points": [[-1.0, 0.0], [4.0, 0.0], [8.0, 3.0]], "label": "1"}),
    );
    doc.run(
        "archbuilder.grid.create",
        json!({"start": [-1.0, 6.0], "end": [9.0, 6.0], "label": "2"}),
    );
    let (exported, file) = doc.export("grid_multi_segment");
    let reported: Vec<_> = exported
        .report
        .items
        .iter()
        .filter(|i| i.reason == "ab.io.ifc.grid_multi_segment")
        .collect();
    assert_eq!(reported.len(), 1);
    // The other lines still make a grid; the bent line is not among its axes.
    let axes: Vec<String> = file
        .instances_of("IFCGRIDAXIS")
        .map(|a| text(&a.records[0].parameters[0]).to_owned())
        .collect();
    assert_eq!(axes.len(), 2);
    assert!(!axes.contains(&"1".to_owned()));
}

#[test]
fn planes_and_selection_sets_are_reported_with_their_own_kinds() {
    let doc = Doc::new();
    doc.run(
        "archbuilder.reference_plane.create",
        json!({"start": [0.0, 0.0], "end": [5.0, 0.0]}),
    );
    doc.run("archbuilder.work_plane.set", json!({"target": "Floor 1"}));
    let wall = doc.make(
        "archbuilder.wall.create",
        "wall",
        json!({"start": [0.0, 0.0], "end": [4.0, 0.0]}),
    );
    let set = doc.make(
        "archbuilder.set.create",
        "set",
        json!({"name": "Walls", "objects": [wall.to_string()]}),
    );
    let (exported, file) = doc.export("planes_and_sets");
    let mut kinds: Vec<(String, String)> = exported
        .report
        .items
        .iter()
        .map(|i| (i.reason.clone(), i.record_kind.clone().unwrap()))
        .collect();
    kinds.sort();
    assert_eq!(
        kinds,
        [
            (
                "ab.io.ifc.datum_not_written".to_owned(),
                "IFCANNOTATION".to_owned()
            ),
            (
                "ab.io.ifc.datum_not_written".to_owned(),
                "IFCANNOTATION".to_owned()
            ),
            (
                "ab.io.ifc.selection_set_not_written".to_owned(),
                "IFCGROUP".to_owned()
            ),
        ]
    );
    assert!(
        exported
            .report
            .items
            .iter()
            .any(|i| i.object_id == Some(set.to_string()))
    );
    let bytes = String::from_utf8_lossy(&exported.bytes);
    assert!(!bytes.contains("IFCGRIDAXIS") && !bytes.contains("IFCBUILDINGELEMENTPROXY"));
    assert_eq!(file.instances_of("IFCWALL").count(), 1);
}
