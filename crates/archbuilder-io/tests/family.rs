//! Family instances in the IFC export: each entity a family declares is written natively, with
//! its type object (property sets, shared geometry as a representation map) and its occurrences;
//! a family that names no entity is listed in the loss report, never written as a proxy. The
//! export stays valid.

#![allow(clippy::unwrap_used, reason = "test code")]

use std::sync::Arc;

use arc_base::Limits;
use arc_base::id::Identifier;
use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Header, Snapshot};
use arc_io_ifc::{Parameter, ReadOptions};
use archbuilder_io::{ExportOptions, export_ifc};
use archbuilder_model::family::Version;
use archbuilder_model::family::def::Category;
use archbuilder_model::family::fixtures::{cabinet, table};
use archbuilder_model::family::loaded::{Component, FamilyType, Loaded};
use archbuilder_model::family::package::publish;
use archbuilder_model::spatial;
use archbuilder_model::types::{COMPONENT, FAMILY, FAMILY_TYPE, STOREY};
use serde_json::json;

fn place_family(
    definition: archbuilder_model::family::Definition,
    type_name: &'static str,
) -> Document {
    place_families(vec![(definition, type_name)])
}

/// Places one freestanding instance of each family type in one storey, the i-th at `[1, 1 + 4i]`.
fn place_families(
    placements: Vec<(archbuilder_model::family::Definition, &'static str)>,
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
    let mut records = Vec::new();
    for (index, (definition, type_name)) in placements.into_iter().enumerate() {
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
        records.push((published, definition, type_name, index));
    }
    document
        .submit(move |txn| {
            let header = |owner| Header {
                owner: Some(owner),
                ..Header::default()
            };
            for (published, definition, type_name, index) in records {
                let offset = u128::try_from(index).unwrap() * 0x10;
                let family = arc_base::ObjectId::from_u128(0xF00D + offset);
                let ty = arc_base::ObjectId::from_u128(0xF00E + offset);
                let comp = arc_base::ObjectId::from_u128(0xF00F + offset);
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
                    origin: [1.0, 1.0 + 4.0 * f64::from(u32::try_from(index).unwrap())],
                    rotation: 0.0,
                    mirror: false,
                    values: std::collections::BTreeMap::new(),
                };
                txn.create(COMPONENT, comp, header(storey), c.record())?;
            }
            Ok(())
        })
        .wait()
        .unwrap();
    document
}

#[test]
fn a_generic_model_without_an_entity_is_listed_and_never_a_proxy() {
    let mut definition = table();
    definition.header.category = Category::GenericModel;
    let document = place_family(definition, "1400x800");
    let export = export_ifc(&document.snapshot(), &ExportOptions::default()).unwrap();
    assert!(export.valid);
    let text = String::from_utf8_lossy(&export.bytes);
    assert!(!text.contains("IFCBUILDINGELEMENTPROXY"), "no silent proxy");
    assert!(
        !text.contains("IFCFURNITURE("),
        "no entity of another category"
    );
    assert!(
        export
            .report
            .items
            .iter()
            .any(|i| i.reason == "ab.io.ifc.family_not_written"),
        "the instance is in the loss report"
    );
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

/// The names of the property sets a type object holds in `HasPropertySets` (attribute 5 of
/// `IfcTypeObject`), read back from the file.
fn type_set_names(file: &arc_io_ifc::IfcFile, type_name: &str) -> Vec<String> {
    let Some(instance) = file.instances_of(type_name).next() else {
        return Vec::new();
    };
    let Parameter::List(sets) = &instance.records[0].parameters[5] else {
        return Vec::new();
    };
    sets.iter()
        .filter_map(|set| match set {
            Parameter::Reference(name) => file.instance(*name),
            _ => None,
        })
        .filter_map(|set| match set.records[0].parameters.get(2) {
            Some(Parameter::Text(name)) => Some(name.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn furniture_casework_and_explicit_entities_are_written_natively() {
    let document = place_families(vec![(table(), "1400x800"), (cabinet(), "600")]);
    let export = export_ifc(&document.snapshot(), &ExportOptions::default()).unwrap();
    assert!(export.valid, "{:?}", export.report.items);
    let text = String::from_utf8_lossy(&export.bytes);
    for needle in [
        "IFCFURNITURE(",
        "IFCFURNITURETYPE(",
        "IFCRELDEFINESBYTYPE(",
        "IFCREPRESENTATIONMAP(",
        "IFCMAPPEDITEM(",
        "USERDEFINED",
    ] {
        assert!(text.contains(needle), "{needle}");
    }
    assert!(
        text.matches("IFCMAPPEDITEM(").count() >= 2,
        "both occurrences map the type's shape"
    );
    assert!(!text.contains("IFCBUILDINGELEMENTPROXY"), "no silent proxy");
    assert!(
        !export
            .report
            .items
            .iter()
            .any(|i| i.reason == "ab.io.ifc.family_not_written"),
    );
    let read = arc_io_ifc::read(
        &export.bytes,
        &mut ReadOptions::with_limits(Limits::DEFAULT),
    )
    .unwrap();
    assert_eq!(
        type_set_names(&read.file, "IFCFURNITURETYPE"),
        vec![
            "ArchBuilder_Type".to_owned(),
            "ArchBuilder_Family".to_owned()
        ],
        "the type's sets are on the type object"
    );
    assert!(read.file.instances_of("IFCRELDEFINESBYPROPERTIES").count() > 0);
    if let Ok(dir) = std::env::var("AB_IFC_OUT") {
        std::fs::write(
            std::path::Path::new(&dir).join("furniture.ifc"),
            &export.bytes,
        )
        .unwrap();
    }
    // Each explicit entity of a family is written as its own IFC class.
    for (entity, class) in [
        ("IfcSanitaryTerminal", "IFCSANITARYTERMINAL("),
        ("IfcLightFixture", "IFCLIGHTFIXTURE("),
        ("IfcElectricAppliance", "IFCELECTRICAPPLIANCE("),
        ("IfcUnitaryEquipment", "IFCUNITARYEQUIPMENT("),
        ("IfcColumn", "IFCCOLUMN("),
        ("IfcBeam", "IFCBEAM("),
        ("IfcBuildingElementProxy", "IFCBUILDINGELEMENTPROXY("),
    ] {
        let mut definition = table();
        definition.header.ifc_entity = entity.to_owned();
        let document = place_family(definition, "1400x800");
        let export = export_ifc(&document.snapshot(), &ExportOptions::default()).unwrap();
        assert!(export.valid, "{entity}: {:?}", export.report.items);
        let text = String::from_utf8_lossy(&export.bytes);
        assert!(text.contains(class), "{entity}");
        assert!(text.contains("IFCRELDEFINESBYTYPE("), "{entity}");
        assert!(text.contains("IFCMAPPEDITEM("), "{entity}");
    }
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
    // The mesh export does not derive hosted bodies yet: the report says so, never silence.
    let mesh = archbuilder_io::mesh::export_mesh(
        &document.snapshot(),
        &archbuilder_io::mesh::MeshOptions::default(),
    )
    .unwrap();
    let hosted = mesh
        .report
        .items
        .iter()
        .find(|i| i.reason == "ab.io.mesh.hosted_family_not_exported")
        .unwrap();
    assert_eq!(
        hosted.arguments.get("count"),
        Some(&arc_io_common::loss::Argument::Int(3))
    );
}

#[test]
fn shared_nested_children_are_in_the_loss_reports_or_the_mesh() {
    use archbuilder_model::family::def::{Anchor, NestedUse};
    let child = table();
    let mut parent = cabinet();
    parent.header.id = arc_base::ObjectId::from_u128(0x7001);
    parent.nested = vec![NestedUse {
        key: "tbl".into(),
        family: child.header.id,
        version: Version {
            major: 1,
            minor: 0,
            patch: 0,
        },
        type_name: Some("1400x800".into()),
        type_param: None,
        shared: true,
        at: [Anchor::default(), Anchor::default(), Anchor::default()],
        rotation: None,
        params: std::collections::BTreeMap::new(),
    }];
    parent.tests.clear();
    let document = place_families(vec![(child, "1400x800"), (parent, "600")]);
    let snapshot = document.snapshot();
    // The IFC export does not write shared children yet, and says so.
    let export = export_ifc(&snapshot, &ExportOptions::default()).unwrap();
    assert!(export.valid);
    if let Ok(dir) = std::env::var("AB_IFC_OUT") {
        std::fs::write(std::path::Path::new(&dir).join("shared.ifc"), &export.bytes).unwrap();
    }
    let item = export
        .report
        .items
        .iter()
        .find(|i| i.reason == "ab.io.ifc.family_shared_not_written")
        .unwrap();
    assert_eq!(
        item.arguments.get("count"),
        Some(&arc_io_common::loss::Argument::Int(1))
    );
    // The mesh has no aggregation: the child goes with its parent's triangles.
    let parent_id = arc_base::ObjectId::from_u128(0xF00F + 0x10);
    let mesh = archbuilder_io::mesh::export_mesh(
        &snapshot,
        &archbuilder_io::mesh::MeshOptions {
            only: Some(std::iter::once(parent_id).collect()),
            ..archbuilder_io::mesh::MeshOptions::default()
        },
    )
    .unwrap();
    let placed = archbuilder_model::family::loaded::place(
        &snapshot,
        &Component::of(&spatial::get(&snapshot, parent_id).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(placed.shared.len(), 1);
    let own: usize = placed
        .solids
        .iter()
        .map(|s| {
            arc_mesh::TriMesh::from_polyhedron(
                &s.solid,
                &arc_base::Tolerance::DEFAULT,
                &Limits::DEFAULT,
            )
            .unwrap()
            .triangle_count()
        })
        .sum();
    assert!(mesh.triangles > own, "{} > {own}", mesh.triangles);
}

#[test]
fn a_mirrored_occurrence_never_becomes_the_shared_geometry_of_its_type() {
    let document = place_family(table(), "1400x800");
    let (mirrored, plain) = (
        arc_base::ObjectId::from_u128(0xF00F),
        arc_base::ObjectId::from_u128(0xF0FF),
    );
    let snapshot = document.snapshot();
    let mut second = Component::of(&spatial::get(&snapshot, mirrored).unwrap()).unwrap();
    let storey = spatial::objects(&snapshot, STOREY)[0].id;
    second.id = plain;
    second.origin = [6.0, 1.0];
    document
        .submit(move |txn| {
            // The first occurrence (by identifier) is the mirrored one.
            txn.modify(
                mirrored,
                arc_doc::FieldPath::new("mirror"),
                Some(arc_doc::Value::Bool(true)),
            )?;
            txn.create(
                COMPONENT,
                plain,
                Header {
                    owner: Some(storey),
                    ..Header::default()
                },
                second.record(),
            )?;
            Ok(())
        })
        .wait()
        .unwrap();
    let export = export_ifc(&document.snapshot(), &ExportOptions::default()).unwrap();
    assert!(export.valid, "{:?}", export.report.items);
    if let Ok(dir) = std::env::var("AB_IFC_OUT") {
        std::fs::write(
            std::path::Path::new(&dir).join("mirrored.ifc"),
            &export.bytes,
        )
        .unwrap();
    }
    let text = String::from_utf8_lossy(&export.bytes);
    let line = |name: &str| {
        text.lines()
            .find(|l| l.starts_with(&format!("{name}=")))
            .unwrap()
            .to_owned()
    };
    // The body of an occurrence: the representation its product shape lists.
    let body = |id: arc_base::ObjectId| {
        let gid = arc_io_ifc::write::global_id(id.as_u128());
        let element = text
            .lines()
            .find(|l| l.contains("IFCFURNITURE(") && l.contains(&format!("'{gid}'")))
            .unwrap();
        let shape = line(element.split(',').nth(6).unwrap());
        let inner = shape.rsplit('(').next().unwrap();
        line(inner.split([',', ')']).next().unwrap())
    };
    assert!(
        body(plain).contains("'MappedRepresentation'"),
        "{}",
        body(plain)
    );
    assert!(!body(mirrored).contains("'MappedRepresentation'"));
}
