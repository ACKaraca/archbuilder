//! The IFC export of a house, read back with the reader of `arc-io-ifc`: counts per class,
//! containment per storey, voids and fills, `GlobalId`s from the object identifiers, the volumes
//! of the bodies against the model's derived volumes, and the quantity sets.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

use std::collections::BTreeMap;
use std::sync::Arc;

use arc_base::id::Identifier;
use arc_base::{Cancel, Limits, ObjectId};
use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use arc_io_ifc::write::{derived_id, global_id, role};
use arc_io_ifc::{IfcFile, Number, Parameter, ReadOptions, read};
use arc_mesh::measure::volume;
use archbuilder_io::{ExportOptions, export_ifc};
use archbuilder_model::floor::Derived;
use archbuilder_model::insert::{self, Insert};
use archbuilder_model::types::{DOOR, FLOOR, WALL, WINDOW, text};
use archbuilder_model::{join, spatial};
use serde_json::{Value as Json, json};

struct House {
    document: Document,
    session: Session,
    registry: arc_command::Registry,
}

impl House {
    fn run(&self, command: &str, params: Json) -> Json {
        let invocation = Invocation {
            seed: 7,
            ..Invocation::new(command, params)
        };
        Invoker::new(&self.registry)
            .invoke(&self.document, &self.session, invocation)
            .unwrap()
            .value
    }

    fn id(value: &Json) -> ObjectId {
        value.as_str().unwrap().parse().unwrap()
    }

    /// The QA house: on the ground storey a 10 x 6 m box of four walls with joined corners, a
    /// partition that meets two of them in a T, a door, two windows and a floor in each room; on
    /// the first storey four walls and a floor with a hole.
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
        let house = Self {
            document,
            session: Session::new(Surface::Ui, [Permission::Read, Permission::Edit]),
            registry: archbuilder_commands::registry().unwrap(),
        };
        house.run("archbuilder.project.new", json!({}));
        let storeys = spatial::storeys(
            &house.document.snapshot(),
            spatial::buildings(&house.document.snapshot())[0].id,
        );
        let corners = [[0.0, 0.0], [0.0, 6.0], [10.0, 6.0], [10.0, 0.0]];
        let mut ground = Vec::new();
        for storey in &storeys[..2] {
            for k in 0..4 {
                let made = house.run(
                    "archbuilder.wall.create",
                    json!({"storey": storey.storey.id, "start": corners[k], "end": corners[(k + 1) % 4]}),
                );
                ground.push(Self::id(&made["wall"]));
            }
        }
        let partition = house.run(
            "archbuilder.wall.create",
            json!({"storey": storeys[0].storey.id, "start": [5.0, 0.0], "end": [5.0, 6.0], "thickness": 0.1}),
        );
        let _ = partition;
        house.run(
            "archbuilder.door.place",
            json!({"wall": ground[0], "distance": 3.0}),
        );
        for distance in [2.5, 7.5] {
            house.run(
                "archbuilder.window.place",
                json!({"wall": ground[1], "distance": distance}),
            );
        }
        for pick in [[2.5, 3.0], [7.5, 3.0]] {
            house.run(
                "archbuilder.floor.create",
                json!({"storey": storeys[0].storey.id, "picks": [pick]}),
            );
        }
        let upper = house.run(
            "archbuilder.floor.create",
            json!({"storey": storeys[1].storey.id,
                   "points": [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]],
                   "holes": [[[2.0, 2.0], [4.0, 2.0], [4.0, 4.0], [2.0, 4.0]]]}),
        );
        let _ = upper;
        house
    }

    fn snapshot(&self) -> Snapshot {
        self.document.snapshot()
    }
}

fn options() -> ExportOptions {
    ExportOptions {
        file_name: "house.ifc".into(),
        timestamp: "2026-01-02T03:04:05".into(),
        application: "archbuilder test".into(),
        ..ExportOptions::default()
    }
}

fn gid(file: &IfcFile, name: u64) -> String {
    match &file.instance(name).unwrap().records[0].parameters[0] {
        Parameter::Text(t) => t.clone(),
        other => panic!("{other:?}"),
    }
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} vs {b}");
}

fn ids_of(snapshot: &Snapshot, key: &str) -> Vec<ObjectId> {
    spatial::objects(snapshot, key)
        .iter()
        .map(|o| o.id)
        .collect()
}

#[test]
fn the_house_exports_without_loss_and_deterministically() {
    let house = House::new();
    let first = export_ifc(&house.snapshot(), &options()).unwrap();
    let second = export_ifc(&house.snapshot(), &options()).unwrap();
    assert_eq!(first.bytes, second.bytes);
    assert!(!first.report.has_loss(), "{:?}", first.report);
    assert_eq!(first.report.summary.approximated, 0);
    // 9 walls, 1 door, 2 windows and 3 floors; the three openings are not counted. The writer
    // also counts the openings and the element types of the document.
    assert_eq!(first.elements, 15);
    let types: usize = archbuilder_model::element_type::TypeKind::ALL
        .iter()
        .map(|k| archbuilder_model::element_type::of_kind(&house.snapshot(), *k).len())
        .sum();
    assert_eq!(
        first.report.summary.mapped,
        18 + u64::try_from(types).unwrap()
    );
    assert_eq!(
        first.report.header.time.as_deref(),
        Some("2026-01-02T03:04:05")
    );
}

/// Every element type is written with its kind; walls and floors take their type's layer set
/// with a usage, walls state whether they are external and load-bearing.
#[test]
fn types_layer_sets_and_wall_flags_are_exported() {
    let house = House::new();
    let snapshot = house.snapshot();
    let wall = spatial::objects(&snapshot, WALL)[0].id;
    house.run(
        "archbuilder.type.assign",
        json!({"type": "Exterior masonry cavity", "kind": "wall", "objects": [wall]}),
    );
    let snapshot = house.snapshot();
    let exported = export_ifc(&snapshot, &options()).unwrap();
    assert!(!exported.report.has_loss(), "{:?}", exported.report);
    let file = read(&exported.bytes, &mut ReadOptions::default())
        .unwrap()
        .file;
    let views = file.typed_views(&Limits::DEFAULT, &Cancel::new()).unwrap();
    // Every wall, floor, door and window has a type of its kind.
    let typed = |entity: &str| {
        views
            .types
            .iter()
            .filter(|t| t.element_type.type_name == entity)
            .count()
    };
    assert_eq!(typed("IFCWALLTYPE"), 9);
    assert_eq!(typed("IFCSLABTYPE"), 3);
    assert_eq!(typed("IFCDOORTYPE"), 1);
    assert_eq!(typed("IFCWINDOWTYPE"), 2);
    let cavity = views
        .types
        .iter()
        .find(|t| t.element.global_id.as_deref() == Some(&*global_id(wall.as_u128())))
        .unwrap();
    assert_eq!(
        cavity.element_type.name.as_deref(),
        Some("Exterior masonry cavity")
    );
    // The cavity wall's layer set has its six layers, the membrane without thickness.
    let layers: Vec<f64> = file
        .instances_of("IFCMATERIALLAYER")
        .filter_map(|i| match i.records[0].parameters[1] {
            arc_io_ifc::Parameter::Real(t) => Some(t),
            _ => None,
        })
        .collect();
    assert!(
        layers
            .windows(6)
            .any(|w| w == [0.1, 0.05, 0.05, 0.0, 0.15, 0.015])
    );
    // One usage per wall and floor, and the wall flags in Pset_WallCommon.
    assert_eq!(file.instances_of("IFCMATERIALLAYERSETUSAGE").count(), 12);
    let flags: Vec<&arc_io_ifc::PropertySet> = views
        .property_sets
        .iter()
        .filter(|p| p.definition.name.as_deref() == Some("Pset_WallCommon"))
        .collect();
    assert_eq!(flags.len(), 9);
    assert!(flags.iter().any(|p| {
        p.properties
            .iter()
            .any(|q| q.name.as_deref() == Some("LoadBearing"))
    }));
}

#[test]
fn the_export_reads_back_as_the_model() {
    let house = House::new();
    let snapshot = house.snapshot();
    let exported = export_ifc(&snapshot, &options()).unwrap();
    let file = read(&exported.bytes, &mut ReadOptions::default())
        .unwrap()
        .file;
    let views = file.typed_views(&Limits::DEFAULT, &Cancel::new()).unwrap();
    assert!(!views.report.has_loss(), "{:?}", views.report);

    let count = |class: &str| file.instances_of(class).count();
    assert_eq!(
        [
            count("IFCWALL"),
            count("IFCDOOR"),
            count("IFCWINDOW"),
            count("IFCSLAB"),
            count("IFCOPENINGELEMENT"),
            count("IFCBUILDINGSTOREY"),
            count("IFCRELVOIDSELEMENT"),
            count("IFCRELFILLSELEMENT"),
        ],
        [9, 1, 2, 3, 3, 3, 3, 3]
    );

    // Containment per storey, and the GlobalIds of the spatial structure and the elements.
    let project = &views.spatial[0];
    let building = &project.children[0].children[0];
    let names = |ids: &[ObjectId]| -> Vec<String> {
        ids.iter()
            .map(|id| global_id(id.as_u128()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        project.entity.global_id.as_deref(),
        Some(&*global_id(
            spatial::project(&snapshot).unwrap().id.as_u128()
        ))
    );
    let storeys = spatial::storeys(&snapshot, spatial::buildings(&snapshot)[0].id);
    assert_eq!(building.children.len(), 3);
    for (node, entry) in building.children.iter().zip(&storeys) {
        let mut expected: Vec<ObjectId> = spatial::walls(&snapshot, entry.storey.id)
            .iter()
            .flat_map(|w| {
                let mut ids = vec![w.id];
                ids.extend(insert::hosted(&snapshot, w.id).iter().map(|(o, _)| o.id));
                ids
            })
            .collect();
        expected.extend(
            spatial::children(&snapshot, FLOOR, entry.storey.id)
                .iter()
                .map(|f| f.id),
        );
        let got: Vec<String> = node
            .contained
            .iter()
            .map(|e| e.global_id.clone().unwrap())
            .collect();
        assert_eq!(got, names(&expected));
        assert_eq!(node.entity.name.as_deref(), text(&entry.storey.values, 0));
    }
    assert_eq!(building.children[0].contained.len(), 5 + 3 + 2);
    assert_eq!(building.children[1].contained.len(), 4 + 1);
    assert!(building.children[2].contained.is_empty());

    // Voids and fills pair each opening with its wall and its insert.
    let pairs = |class: &str| -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = file
            .instances_of(class)
            .map(|i| match &i.records[0].parameters[4..] {
                [Parameter::Reference(a), Parameter::Reference(b)] => {
                    (gid(&file, *a), gid(&file, *b))
                }
                other => panic!("{other:?}"),
            })
            .collect();
        out.sort();
        out
    };
    let (mut voids, mut fills) = (Vec::new(), Vec::new());
    for wall in spatial::objects(&snapshot, WALL) {
        for (object, _) in insert::hosted(&snapshot, wall.id) {
            let opening = derived_id(object.id.as_u128(), role::INSERT_OPENING, 0);
            voids.push((global_id(wall.id.as_u128()), global_id(opening)));
            fills.push((global_id(opening), global_id(object.id.as_u128())));
        }
    }
    voids.sort();
    fills.sort();
    assert_eq!(pairs("IFCRELVOIDSELEMENT"), voids);
    assert_eq!(pairs("IFCRELFILLSELEMENT"), fills);

    // Storey elevations and the door's operation and size.
    let elevations: Vec<f64> = file
        .instances_of("IFCBUILDINGSTOREY")
        .map(|s| match s.records[0].parameters[9] {
            Parameter::Real(v) => v,
            ref other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(elevations, [0.0, 3.0, 6.0]);
    let door = file.instances_of("IFCDOOR").next().unwrap();
    assert_eq!(door.records[0].parameters[8], Parameter::Real(2.1));
    assert_eq!(door.records[0].parameters[9], Parameter::Real(0.9));
    assert!(matches!(
        &door.records[0].parameters[11],
        Parameter::Enumeration(e) if e.starts_with("SINGLE_SWING_")
    ));
}

/// IFC4: `SINGLE_SWING_LEFT` hinges on the left looking along the placement's +y (the swing
/// direction), so its hinge is at -x. The hinge is found from the exported body: the handle is
/// on the side opposite the hinge, along the wall, and the door's local x axis comes from the
/// transform the reader gives its shape.
#[test]
fn the_door_operation_names_the_side_of_the_hinge_in_the_exported_body() {
    for (flip_hand, flip_facing) in [(false, false), (true, false), (false, true), (true, true)] {
        let house = House::new();
        let door = spatial::objects(&house.snapshot(), DOOR)[0].id;
        for (flip, command) in [
            (flip_hand, "archbuilder.insert.flip_hand"),
            (flip_facing, "archbuilder.insert.flip_facing"),
        ] {
            if flip {
                house.run(command, json!({"inserts": [door]}));
            }
        }
        let exported = export_ifc(&house.snapshot(), &options()).unwrap();
        let file = read(&exported.bytes, &mut ReadOptions::default())
            .unwrap()
            .file;
        let geometry = file.geometry(&Limits::DEFAULT, &Cancel::new()).unwrap();
        let door_gid = global_id(door.as_u128());
        let item = geometry
            .items
            .iter()
            .find(|i| gid(&file, i.product) == door_gid)
            .unwrap();
        let mesh = item.world_mesh(&Limits::DEFAULT).unwrap().unwrap();
        // The door stands in a wall along world y; its handle is the only part at 0.95-1.05 m.
        let ys: Vec<f64> = mesh.positions().iter().map(|p| p.y).collect();
        let center = (ys.iter().copied().fold(f64::MAX, f64::min)
            + ys.iter().copied().fold(f64::MIN, f64::max))
            / 2.0;
        let handle: Vec<f64> = mesh
            .positions()
            .iter()
            .filter(|p| (0.949..1.051).contains(&p.z))
            .map(|p| p.y - center)
            .collect();
        assert!(!handle.is_empty());
        let handle_y = handle.iter().sum::<f64>() / handle.len() as f64;
        assert!(handle_y.abs() > 0.1);
        // The hinge is opposite the handle; its side along the door's local x axis.
        let local_x_y = item.transform.matrix()[(1, 0)];
        assert!(
            local_x_y.abs() > 0.99,
            "the door's x axis runs along the wall"
        );
        let hinge_at_minus_x = -handle_y * local_x_y < 0.0;
        let expected = if hinge_at_minus_x {
            "SINGLE_SWING_LEFT"
        } else {
            "SINGLE_SWING_RIGHT"
        };
        let record = &file.instances_of("IFCDOOR").next().unwrap().records[0];
        assert_eq!(
            record.parameters[11],
            Parameter::Enumeration(expected.into()),
            "flip_hand {flip_hand}, flip_facing {flip_facing}"
        );
    }
}

#[test]
fn body_volumes_and_quantities_equal_the_derived_volumes() {
    let house = House::new();
    let snapshot = house.snapshot();
    let exported = export_ifc(&snapshot, &options()).unwrap();
    let file = read(&exported.bytes, &mut ReadOptions::default())
        .unwrap()
        .file;
    let geometry = file.geometry(&Limits::DEFAULT, &Cancel::new()).unwrap();
    let mut volumes: BTreeMap<String, f64> = BTreeMap::new();
    for item in &geometry.items {
        let mesh = item.world_mesh(&Limits::DEFAULT).unwrap().unwrap();
        *volumes.entry(gid(&file, item.product)).or_default() +=
            volume(&mesh, &Limits::DEFAULT).unwrap();
    }
    for wall in spatial::objects(&snapshot, WALL) {
        let data = archbuilder_model::wall::Wall::of(&snapshot, &wall).unwrap();
        let level = spatial::storey_elevation(
            &snapshot,
            &spatial::get(&snapshot, wall.header.owner.unwrap()).unwrap(),
        );
        let footprint = join::footprint_of(&snapshot, &wall).unwrap();
        let gross = data.solid_in(footprint, level).unwrap().volume();
        // A wall ends under the slab above it: at most the slab's thickness (0.2 m) is missing.
        let body = volumes[&global_id(wall.id.as_u128())];
        assert!(
            body <= gross + 1e-9 && gross - body <= data.length() * data.thickness * 0.2 + 1e-9,
            "{body} vs {gross}"
        );
    }
    for floor in spatial::objects(&snapshot, FLOOR) {
        let derived = Derived::of(&snapshot, &floor).unwrap();
        near(volumes[&global_id(floor.id.as_u128())], derived.volume());
    }
    // An opening's body is the prism through its wall: width x thickness x height.
    for key in [DOOR, WINDOW] {
        for object in spatial::objects(&snapshot, key) {
            let item = Insert::of(&snapshot, &object).unwrap();
            let wall = spatial::get(&snapshot, object.header.owner.unwrap()).unwrap();
            let thickness = archbuilder_model::wall::Wall::of(&snapshot, &wall)
                .unwrap()
                .thickness;
            let opening = derived_id(object.id.as_u128(), role::INSERT_OPENING, 0);
            near(
                volumes[&global_id(opening)],
                item.width * item.height * thickness,
            );
        }
    }

    // The quantity sets carry the model's gross and net volumes.
    let views = file.typed_views(&Limits::DEFAULT, &Cancel::new()).unwrap();
    let mut quantities: BTreeMap<(String, String), f64> = BTreeMap::new();
    for set in &views.quantity_sets {
        for related in &set.related {
            for q in &set.quantities {
                if let Some(Number::Real(v)) = q.value {
                    quantities.insert(
                        (related.global_id.clone().unwrap(), q.name.clone().unwrap()),
                        v,
                    );
                }
            }
        }
    }
    for wall in spatial::objects(&snapshot, WALL) {
        // The layers with the openings cut, under the slab above the wall.
        let net = archbuilder_model::cleanup::wall_pieces(&snapshot, &wall)
            .unwrap()
            .unwrap()
            .iter()
            .map(|p| p.solid.volume())
            .sum::<f64>();
        let id = global_id(wall.id.as_u128());
        near(quantities[&(id.clone(), "NetVolume".into())], net);
        near(
            quantities[&(id.clone(), "GrossVolume".into())],
            volumes[&id],
        );
    }
    for floor in spatial::objects(&snapshot, FLOOR) {
        let derived = Derived::of(&snapshot, &floor).unwrap();
        let id = global_id(floor.id.as_u128());
        near(quantities[&(id.clone(), "NetArea".into())], derived.area());
        near(
            quantities[&(id.clone(), "NetVolume".into())],
            derived.volume(),
        );
        near(quantities[&(id, "Width".into())], derived.floor.thickness);
    }
    let _ = ids_of(&snapshot, WALL);
}

/// A casement window is written as a single panel and reported as approximated; nothing else is.
#[test]
fn a_casement_window_is_reported_as_approximated() {
    let house = House::new();
    let window = spatial::objects(&house.snapshot(), WINDOW)[0].id;
    // The operation comes from the type: a copy of the window's type that opens.
    let snapshot = house.snapshot();
    let own = archbuilder_model::element_type::type_of_object(
        &snapshot,
        &spatial::get(&snapshot, window).unwrap(),
    )
    .unwrap()
    .1
    .id;
    let copy = house.run("archbuilder.type.duplicate", json!({"type": own}));
    house.run(
        "archbuilder.type.set",
        json!({"type": copy["type"], "operation": "casement"}),
    );
    house.run(
        "archbuilder.type.assign",
        json!({"type": copy["type"], "objects": [window]}),
    );
    let exported = export_ifc(&house.snapshot(), &options()).unwrap();
    assert!(!exported.report.has_loss());
    assert_eq!(exported.report.summary.approximated, 1);
    let [item] = &exported.report.items[..] else {
        panic!("{:?}", exported.report.items);
    };
    assert_eq!(item.reason, "ab.io.ifc.window_operation_approximated");
    assert_eq!(item.record_kind.as_deref(), Some("IFCWINDOW"));
    assert_eq!(item.object_id, Some(window.to_string()));
    assert_eq!(exported.elements, 15);
}

/// Enclosed rooms are spaces under their storey with the model's derived quantities and body
/// volumes; an unenclosed room and a second room in a region are named in the loss report.
#[test]
fn enclosed_rooms_are_spaces_and_the_others_are_reported() {
    use archbuilder_model::room;
    let house = House::new();
    let storey = spatial::storeys(
        &house.snapshot(),
        spatial::buildings(&house.snapshot())[0].id,
    )[0]
    .storey
    .id;
    // A small box on its own, whose room loses a wall; two rooms in the house, which then lose
    // the partition between them.
    let corners = [[20.0, 0.0], [24.0, 0.0], [24.0, 4.0], [20.0, 4.0]];
    let mut box_walls = Vec::new();
    for k in 0..4 {
        let made = house.run(
            "archbuilder.wall.create",
            json!({"storey": storey, "start": corners[k], "end": corners[(k + 1) % 4]}),
        );
        box_walls.push(House::id(&made["wall"]));
    }
    for (point, name, number) in [
        ([2.5, 3.0], "Living", "101"),
        ([7.5, 3.0], "Kitchen", "102"),
        ([22.0, 2.0], "Outside", "103"),
    ] {
        house.run(
            "archbuilder.room.place",
            json!({"storey": storey, "points": [point], "name": name, "number": number}),
        );
    }
    let partition = spatial::walls(&house.snapshot(), storey)
        .into_iter()
        .find(|w| {
            archbuilder_model::wall::Wall::of(&house.snapshot(), w)
                .is_some_and(|d| d.thickness < 0.11)
        })
        .unwrap();
    house.run(
        "archbuilder.wall.delete",
        json!({"walls": [partition.id.to_string(), box_walls[3].to_string()]}),
    );
    let snapshot = house.snapshot();
    let entries = room::of_storey(&snapshot, storey);
    let enclosed: Vec<_> = entries
        .iter()
        .filter(|e| e.state() == room::State::Enclosed)
        .collect();
    assert_eq!(enclosed.len(), 1);
    let exported = export_ifc(&snapshot, &options()).unwrap();
    let reasons: Vec<_> = exported
        .report
        .items
        .iter()
        .map(|i| i.reason.as_str())
        .collect();
    assert!(
        reasons.contains(&"ab.io.ifc.room_redundant")
            && reasons.contains(&"ab.io.ifc.room_unenclosed"),
        "{reasons:?}"
    );
    let file = read(&exported.bytes, &mut ReadOptions::default())
        .unwrap()
        .file;
    let views = file.typed_views(&Limits::DEFAULT, &Cancel::new()).unwrap();
    let ground = &views.spatial[0].children[0].children[0].children[0];
    let names: Vec<_> = ground
        .children
        .iter()
        .map(|n| (n.entity.type_name.clone(), n.entity.name.clone().unwrap()))
        .collect();
    assert_eq!(names, [("IFCSPACE".to_owned(), "101".to_owned())]);
    assert!(ground.contained.iter().all(|e| e.type_name != "IFCSPACE"));
    let geometry = file.geometry(&Limits::DEFAULT, &Cancel::new()).unwrap();
    let mut volumes: BTreeMap<String, f64> = BTreeMap::new();
    for item in &geometry.items {
        let mesh = item.world_mesh(&Limits::DEFAULT).unwrap().unwrap();
        *volumes.entry(gid(&file, item.product)).or_default() +=
            volume(&mesh, &Limits::DEFAULT).unwrap();
    }
    for entry in &enclosed {
        // A room's body is its solid: the room is 9.8 x 5.8 m, 2.8 m under the slab above it
        // (3.0 m to its underside less the 0.2 m slab) except under the 2 x 2 m hole of that slab,
        // where it reaches its upper limit, 3.0 m.
        near(
            volumes[&global_id(entry.id.as_u128())],
            (9.8 * 5.8 - 4.0) * 2.8 + 4.0 * 3.0,
        );
        let space = file
            .instances_of("IFCSPACE")
            .find(|i| gid(&file, i.name) == global_id(entry.id.as_u128()))
            .unwrap();
        assert_eq!(
            space.records[0].parameters[7],
            Parameter::Text(entry.room.name.clone())
        );
    }
    let mut areas = BTreeMap::new();
    for set in &views.quantity_sets {
        for related in &set.related {
            for q in &set.quantities {
                if let Some(Number::Real(v)) = q.value {
                    areas.insert(
                        (related.global_id.clone().unwrap(), q.name.clone().unwrap()),
                        v,
                    );
                }
            }
        }
    }
    for entry in &enclosed {
        let id = global_id(entry.id.as_u128());
        near(
            areas[&(id.clone(), "NetFloorArea".into())],
            entry.area().unwrap(),
        );
        near(
            areas[&(id.clone(), "NetPerimeter".into())],
            entry.perimeter().unwrap(),
        );
        near(
            areas[&(id, "NetVolume".into())],
            (9.8 * 5.8 - 4.0) * 2.8 + 4.0 * 3.0,
        );
    }
}

/// Rooms around a courtyard: the room around has the courtyard as a void in its body, its area,
/// perimeter and volume leave the courtyard out, and the room inside it does not overlap.
#[test]
fn a_room_around_a_courtyard_is_a_space_with_a_void() {
    let house = House::new();
    let snapshot = house.snapshot();
    let storey = spatial::storeys(&snapshot, spatial::buildings(&snapshot)[0].id)[0]
        .storey
        .id;
    for (side, at) in [(12.0, 30.0), (4.0, 34.0)] {
        let c = [
            [at, at],
            [at, at + side],
            [at + side, at + side],
            [at + side, at],
        ];
        for k in 0..4 {
            house.run(
                "archbuilder.wall.create",
                json!({"storey": storey, "start": c[k], "end": c[(k + 1) % 4]}),
            );
        }
    }
    for (point, name, number) in [
        ([32.0, 32.0], "Ring", "201"),
        ([36.0, 36.0], "Court", "202"),
    ] {
        house.run(
            "archbuilder.room.place",
            json!({"storey": storey, "points": [point], "name": name, "number": number}),
        );
    }
    let snapshot = house.snapshot();
    let entries = archbuilder_model::room::of_storey(&snapshot, storey);
    let by = |n: &str| {
        entries
            .iter()
            .find(|e| e.room.number.as_deref() == Some(n))
            .unwrap()
    };
    near(by("201").area().unwrap(), 11.8 * 11.8 - 4.2 * 4.2);
    near(by("202").area().unwrap(), 3.8 * 3.8);
    near(by("201").perimeter().unwrap(), 4.0 * 11.8 + 4.0 * 4.2);
    near(
        by("201").footprint.gross_area().unwrap(),
        12.0 * 12.0 - 4.0 * 4.0,
    );
    let exported = export_ifc(&snapshot, &options()).unwrap();
    let file = read(&exported.bytes, &mut ReadOptions::default())
        .unwrap()
        .file;
    let geometry = file.geometry(&Limits::DEFAULT, &Cancel::new()).unwrap();
    let mut volumes: BTreeMap<String, f64> = BTreeMap::new();
    for item in &geometry.items {
        let mesh = item.world_mesh(&Limits::DEFAULT).unwrap().unwrap();
        *volumes.entry(gid(&file, item.product)).or_default() +=
            volume(&mesh, &Limits::DEFAULT).unwrap();
    }
    for n in ["201", "202"] {
        near(
            volumes[&global_id(by(n).id.as_u128())],
            by(n).volume().unwrap(),
        );
    }
    near(volumes[&global_id(by("201").id.as_u128())], 3.0 * 121.6);
}

/// A stair is named in the loss report (the writer has no stair entities); the shaft opening it
/// cuts in the slab of the level above is a hole of the slab's profile in the file.
#[test]
fn a_stair_is_reported_and_its_opening_is_a_hole_of_the_slab_above() {
    let house = House::new();
    let made = house.run(
        "archbuilder.stair.create",
        json!({"start": [7.0, 0.5], "direction": "90"}),
    );
    let stair = House::id(&made["stair"]);
    let snapshot = house.snapshot();
    let upper = spatial::objects(&snapshot, FLOOR)
        .into_iter()
        .find(|f| Derived::of(&snapshot, f).is_some_and(|d| d.top > 1.0))
        .unwrap();
    let derived = Derived::of(&snapshot, &upper).unwrap();
    // The outer loop, the sketched hole and the shaft opening (4.53 x 1.1).
    assert_eq!(derived.boundary.loops.len(), 3);
    near(derived.area(), 60.0 - 4.0 - 4.53 * 1.1);
    let exported = export_ifc(&snapshot, &options()).unwrap();
    let [item] = &exported.report.items[..] else {
        panic!("{:?}", exported.report.items);
    };
    assert_eq!(item.reason, "ab.io.ifc.stair_not_written");
    assert_eq!(item.record_kind.as_deref(), Some("IFCSTAIR"));
    assert_eq!(item.object_id, Some(stair.to_string()));
    assert_eq!(exported.elements, 15);
    // The body read back has the opening as a hole: its volume is the net area times 0.2 m.
    let file = read(&exported.bytes, &mut ReadOptions::default())
        .unwrap()
        .file;
    let geometry = file.geometry(&Limits::DEFAULT, &Cancel::new()).unwrap();
    let mut body = 0.0;
    for item in &geometry.items {
        if gid(&file, item.product) == global_id(upper.id.as_u128()) {
            let mesh = item.world_mesh(&Limits::DEFAULT).unwrap().unwrap();
            body += volume(&mesh, &Limits::DEFAULT).unwrap();
        }
    }
    near(body, derived.volume());
    near(body, (60.0 - 4.0 - 4.53 * 1.1) * 0.2);
}

/// A hip roof over the upper walls, with those walls attached to it: an `IfcRoof` of the roof
/// type whose body is the slab, and walls whose bodies are their clipped solids.
#[test]
fn a_roof_and_its_attached_walls_read_back_with_their_bodies() {
    let house = House::new();
    let snapshot = house.snapshot();
    let upper_storey = spatial::storeys(&snapshot, spatial::buildings(&snapshot)[0].id)[1]
        .storey
        .id;
    let upper_walls: Vec<ObjectId> = spatial::walls(&snapshot, upper_storey)
        .iter()
        .map(|w| w.id)
        .collect();
    let made = house.run(
        "archbuilder.roof.create",
        json!({"storey": upper_storey, "picks": [[5.0, 3.0]], "type": "Timber rafters with insulation and tiles"}),
    );
    let roof = House::id(&made["roof"]);
    house.run("archbuilder.wall.attach_top", json!({"walls": upper_walls}));
    let snapshot = house.snapshot();
    let exported = export_ifc(&snapshot, &options()).unwrap();
    assert!(!exported.report.has_loss(), "{:?}", exported.report);
    let file = read(&exported.bytes, &mut ReadOptions::default())
        .unwrap()
        .file;
    assert_eq!(file.instances_of("IFCROOF").count(), 1);
    let roof_instance = file.instances_of("IFCROOF").next().unwrap();
    assert_eq!(
        roof_instance.records[0].parameters[8],
        Parameter::Enumeration("HIP_ROOF".into())
    );
    let views = file.typed_views(&Limits::DEFAULT, &Cancel::new()).unwrap();
    assert!(views.types.iter().any(|t| {
        t.element.global_id.as_deref() == Some(global_id(roof.as_u128()).as_str())
            && t.element_type.type_name == "IFCROOFTYPE"
    }));
    let geometry = file.geometry(&Limits::DEFAULT, &Cancel::new()).unwrap();
    let mut volumes: BTreeMap<String, f64> = BTreeMap::new();
    for item in &geometry.items {
        let mesh = item.world_mesh(&Limits::DEFAULT).unwrap().unwrap();
        *volumes.entry(gid(&file, item.product)).or_default() +=
            volume(&mesh, &Limits::DEFAULT).unwrap();
    }
    let object = spatial::get(&snapshot, roof).unwrap();
    let derived = archbuilder_model::roof::Derived::of(&snapshot, &object).unwrap();
    let slab = derived.surface.solid().unwrap().volume();
    assert!((volumes[&global_id(roof.as_u128())] - slab).abs() < 1e-6);
    for wall in upper_walls {
        let object = spatial::get(&snapshot, wall).unwrap();
        let clipped = insert::wall_solid(&snapshot, &object).unwrap().unwrap();
        // Taller than the 3 m prism: the top follows the roof's underside up from the wall line.
        assert!(clipped.bounding_box().max().z > 6.0 + 1e-3);
        // The layers of the wall are clipped under the roof too, in the viewport and in the file.
        let pieces = archbuilder_model::cleanup::wall_pieces(&snapshot, &object)
            .unwrap()
            .unwrap();
        assert!(
            pieces
                .iter()
                .any(|p| p.solid.bounding_box().max().z > 6.0 + 1e-3)
        );
        assert!(
            pieces
                .iter()
                .all(|p| p.solid.bounding_box().max().z <= clipped.bounding_box().max().z + 1e-6)
        );
        let gross = volumes[&global_id(wall.as_u128())];
        let data = archbuilder_model::wall::Wall::of(&snapshot, &object).unwrap();
        let footprint = join::footprint_of(&snapshot, &object).unwrap();
        let level =
            spatial::storey_elevation(&snapshot, &spatial::get(&snapshot, upper_storey).unwrap());
        let (_, surface) = insert::attached_roof(&snapshot, &object).unwrap();
        let want = archbuilder_model::wall::under_roof(&data, footprint, level, &surface)
            .unwrap()
            .volume();
        assert!((gross - want).abs() < 1e-6, "{gross} {want}");
    }
}

/// A floor with an arc edge and the layered walls of the house export without loss and read
/// back: the slab body is the arc written as chords within the model tolerance.
#[test]
fn a_floor_with_an_arc_edge_and_layered_walls_read_back() {
    let house = House::new();
    let made = house.run(
        "archbuilder.floor.create",
        json!({"points": [[20.0, 0.0], [24.0, 0.0], [24.0, 4.0], [20.0, 4.0]]}),
    );
    let floor = House::id(&made["floor"]);
    let snapshot = house.snapshot();
    let object = spatial::get(&snapshot, floor).unwrap();
    let mut vertices: Vec<Json> = archbuilder_model::floor::Floor::from_values(&object.values)
        .unwrap()
        .vertices
        .iter()
        .map(|v| json!({"loop_index": 0, "point": v.point}))
        .collect();
    // A half circle of radius 2 bulging out of the bottom edge.
    vertices[0]["bulge"] = json!(1.0);
    house.run(
        "archbuilder.floor.edit_boundary",
        json!({"floor": floor, "vertices": vertices}),
    );
    let snapshot = house.snapshot();
    let exported = export_ifc(&snapshot, &options()).unwrap();
    assert!(!exported.report.has_loss(), "{:?}", exported.report);
    let file = read(&exported.bytes, &mut ReadOptions::default())
        .unwrap()
        .file;
    let geometry = file.geometry(&Limits::DEFAULT, &Cancel::new()).unwrap();
    let mut body = 0.0;
    for item in &geometry.items {
        if gid(&file, item.product) == global_id(floor.as_u128()) {
            let mesh = item.world_mesh(&Limits::DEFAULT).unwrap().unwrap();
            body += volume(&mesh, &Limits::DEFAULT).unwrap();
        }
    }
    let object = spatial::get(&snapshot, floor).unwrap();
    let derived = Derived::of(&snapshot, &object).unwrap();
    assert!(derived.boundary.has_arcs());
    let exact = derived.volume();
    // The chords lie inside the arc: slightly less, within a tolerance of its length.
    assert!(
        body <= exact + 1e-9 && exact - body < 0.01 * exact,
        "{body} {exact}"
    );
}

/// A slab cut into parts by the walls of its own storey, and the walls below it: the slab sits at
/// its own height in the file (not shifted by the storey), the walls below end under it, and the
/// volume of each wall body is its net volume.
#[test]
fn a_cut_slab_keeps_its_height_and_wall_bodies_are_their_net_volume() {
    let house = House::new();
    let snapshot = house.snapshot();
    let upper_storey = spatial::storeys(&snapshot, spatial::buildings(&snapshot)[0].id)[1]
        .storey
        .id;
    // A slab over the upper walls, raised 0.15 m so that it reaches into them: the walls' cores
    // cut it into an inner and an outer part (a B-rep in the file).
    let made = house.run(
        "archbuilder.floor.create",
        json!({"storey": upper_storey, "points": [[-2.0, -2.0], [12.0, -2.0], [12.0, 8.0], [-2.0, 8.0]], "offset": 0.15}),
    );
    let slab = House::id(&made["floor"]);
    let snapshot = house.snapshot();
    let exported = export_ifc(&snapshot, &options()).unwrap();
    assert!(!exported.report.has_loss(), "{:?}", exported.report);
    let file = read(&exported.bytes, &mut ReadOptions::default())
        .unwrap()
        .file;
    let geometry = file.geometry(&Limits::DEFAULT, &Cancel::new()).unwrap();
    let (mut zmin, mut zmax) = (f64::MAX, f64::MIN);
    let mut volumes: BTreeMap<String, f64> = BTreeMap::new();
    for item in &geometry.items {
        let mesh = item.world_mesh(&Limits::DEFAULT).unwrap().unwrap();
        *volumes.entry(gid(&file, item.product)).or_default() +=
            volume(&mesh, &Limits::DEFAULT).unwrap();
        if gid(&file, item.product) == global_id(slab.as_u128()) {
            for p in mesh.positions() {
                zmin = zmin.min(p.z);
                zmax = zmax.max(p.z);
            }
        }
    }
    // The upper storey is at 3 m: the slab top is 3.15 and its underside 2.95.
    near(zmin, 2.95);
    near(zmax, 3.15);
    // The body of a wall is its layers (cleaned, under the slab above it): without openings the
    // body is the net volume, and a wall with openings is voided by them to its net volume.
    for wall in spatial::objects(&snapshot, WALL) {
        let id = global_id(wall.id.as_u128());
        let whole = archbuilder_model::cleanup::wall_pieces_whole(&snapshot, &wall)
            .unwrap()
            .unwrap()
            .iter()
            .map(|p| p.solid.volume())
            .sum::<f64>();
        let net = archbuilder_model::cleanup::wall_pieces(&snapshot, &wall)
            .unwrap()
            .unwrap()
            .iter()
            .map(|p| p.solid.volume())
            .sum::<f64>();
        assert!(
            (volumes[&id] - whole).abs() < 1e-6,
            "{id}: {} vs {whole}",
            volumes[&id]
        );
        if insert::hosted(&snapshot, wall.id).is_empty() {
            assert!((whole - net).abs() < 1e-9);
        } else {
            assert!(net < whole);
        }
    }
}

/// A curtain wall is never written as a wall or a proxy: its wall, panels and mullions are named
/// in the loss report with their counts, and a curtain wall inside a basic wall is a void of it.
#[test]
fn a_curtain_wall_is_reported_and_its_embedding_is_a_void_of_the_host() {
    let house = House::new();
    let ty = House::id(&house.run("archbuilder.curtain.type", json!({"name": "CW"}))["type"]);
    let host = House::id(
        &house.run(
            "archbuilder.wall.create",
            json!({"start": [20.0, 0.0], "end": [28.0, 0.0], "height": 3.0}),
        )["wall"],
    );
    let curtain = House::id(
        &house.run(
            "archbuilder.wall.create",
            json!({"start": [22.0, 0.0], "end": [25.0, 0.0], "height": 2.0, "type": ty.to_string()}),
        )["wall"],
    );
    let snapshot = house.snapshot();
    let d = archbuilder_model::curtain::Derived::of(
        &snapshot,
        &spatial::get(&snapshot, curtain).unwrap(),
    )
    .unwrap();
    let (panels, mullions) = d.counts();
    let exported = export_ifc(&snapshot, &options()).unwrap();
    let kinds: BTreeMap<String, i64> = exported
        .report
        .items
        .iter()
        .filter(|i| i.reason == "ab.io.ifc.curtain_not_written")
        .map(|i| {
            assert_eq!(i.object_id, Some(curtain.to_string()));
            let count = match i.arguments.get("count") {
                Some(arc_io_common::loss::Argument::Int(n)) => *n,
                other => panic!("{other:?}"),
            };
            (i.record_kind.clone().unwrap(), count)
        })
        .collect();
    assert_eq!(kinds["IFCCURTAINWALL"], 1);
    assert_eq!(kinds["IFCPLATE"], i64::try_from(panels).unwrap());
    assert_eq!(kinds["IFCMEMBER"], i64::try_from(mullions).unwrap());
    let file = read(&exported.bytes, &mut ReadOptions::default())
        .unwrap()
        .file;
    let ids: Vec<String> = file
        .instances()
        .iter()
        .filter(|i| i.records[0].type_name == "IFCOPENINGELEMENT")
        .map(|i| gid(&file, i.name))
        .collect();
    assert!(ids.contains(&global_id(derived_id(
        curtain.as_u128(),
        role::INSERT_OPENING,
        0
    ))));
    assert!(host != curtain);
    // Neither the wall nor the type is written as a wall.
    assert!(
        !file
            .instances()
            .iter()
            .any(|i| i.records[0].type_name == "IFCWALL"
                && gid(&file, i.name) == global_id(curtain.as_u128()))
    );
}
