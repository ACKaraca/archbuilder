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
    // 9 walls, 1 door, 2 windows and 3 floors; the three openings are not counted.
    assert_eq!(first.elements, 15);
    assert_eq!(first.report.summary.mapped, 18);
    assert_eq!(
        first.report.header.time.as_deref(),
        Some("2026-01-02T03:04:05")
    );
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
        near(volumes[&global_id(wall.id.as_u128())], gross);
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
        let net = insert::wall_solid(&snapshot, &wall)
            .unwrap()
            .unwrap()
            .volume();
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
    house.run(
        "archbuilder.property.set",
        json!({"objects": [window], "field": "operation", "value": "casement"}),
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
        near(
            volumes[&global_id(entry.id.as_u128())],
            entry.volume().unwrap(),
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
        near(areas[&(id, "NetVolume".into())], entry.volume().unwrap());
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
