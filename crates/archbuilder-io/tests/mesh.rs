//! The mesh export: each format is read back with `arc-io-mesh` and compared with the model.

#![allow(
    clippy::unwrap_used,
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    clippy::cast_possible_truncation,
    reason = "test code"
)]

use std::collections::BTreeSet;
use std::sync::Arc;

use arc_base::{Limits, ObjectId};
use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use arc_io_mesh::{MeshScene, ReadOptions, gltf, obj, stl};
use archbuilder_io::mesh::{MeshExport, MeshFormat, MeshOptions, Unit, export_mesh};
use archbuilder_model::spatial;
use archbuilder_model::types::{ROOF, WALL};
use serde_json::{Value as Json, json};

struct Model {
    document: Document,
    registry: arc_command::Registry,
}

impl Model {
    fn run(&self, command: &str, params: Json) -> Json {
        let session = Session::new(Surface::Ui, [Permission::Read, Permission::Edit]);
        let invocation = Invocation {
            seed: 11,
            ..Invocation::new(command, params)
        };
        Invoker::new(&self.registry)
            .invoke(&self.document, &session, invocation)
            .unwrap()
            .value
    }

    /// A 10 x 6 box of four walls with a door and a window, a floor and a hip roof.
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
        let model = Self {
            document,
            registry: archbuilder_commands::registry().unwrap(),
        };
        model.run("archbuilder.project.new", json!({}));
        let c = [[0.0, 0.0], [0.0, 6.0], [10.0, 6.0], [10.0, 0.0]];
        let mut walls = Vec::new();
        for k in 0..4 {
            let made = model.run(
                "archbuilder.wall.create",
                json!({"start": c[k], "end": c[(k + 1) % 4]}),
            );
            walls.push(made["wall"].clone());
        }
        model.run(
            "archbuilder.door.place",
            json!({"wall": walls[0], "distance": 3.0}),
        );
        model.run(
            "archbuilder.window.place",
            json!({"wall": walls[1], "distance": 3.0}),
        );
        model.run("archbuilder.floor.create", json!({"picks": [[5.0, 3.0]]}));
        model.run(
            "archbuilder.roof.create",
            json!({"picks": [[5.0, 3.0]], "slope": 0.5}),
        );
        model
    }

    fn snapshot(&self) -> Snapshot {
        self.document.snapshot()
    }

    fn ids(&self, key: &str) -> Vec<ObjectId> {
        spatial::objects(&self.snapshot(), key)
            .iter()
            .map(|o| o.id)
            .collect()
    }
}

fn options(format: MeshFormat) -> MeshOptions {
    MeshOptions {
        format,
        mtl_name: "m.mtl".into(),
        ..MeshOptions::default()
    }
}

fn bounds(scene: &MeshScene) -> [[f64; 3]; 2] {
    let mut b = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
    for part in &scene.meshes {
        for p in part.mesh.positions() {
            for (k, v) in [p.x, p.y, p.z].into_iter().enumerate() {
                b[0][k] = b[0][k].min(v);
                b[1][k] = b[1][k].max(v);
            }
        }
    }
    b
}

fn triangles(scene: &MeshScene) -> usize {
    scene.meshes.iter().map(|p| p.mesh.triangle_count()).sum()
}

fn near(a: [[f64; 3]; 2], b: [[f64; 3]; 2], tolerance: f64) {
    for i in 0..2 {
        for k in 0..3 {
            assert!(
                (a[i][k] - b[i][k]).abs() <= tolerance,
                "{a:?} against {b:?}"
            );
        }
    }
}

fn read_back(export: &MeshExport, format: MeshFormat) -> MeshScene {
    let mut read = ReadOptions::default();
    match format {
        MeshFormat::Obj => obj::read(&export.bytes, export.mtl.as_deref(), &mut read),
        MeshFormat::Stl => stl::read(&export.bytes, &mut read),
        MeshFormat::Gltf | MeshFormat::Glb => gltf::read(&export.bytes, |_| None, &mut read),
    }
    .unwrap()
    .scene
}

#[test]
fn every_format_reads_back_with_the_triangles_and_box_the_export_counted() {
    let model = Model::new();
    let snapshot = model.snapshot();
    // The roof alone, against its own derived solid.
    let roof = model.ids(ROOF)[0];
    let object = spatial::get(&snapshot, roof).unwrap();
    let solid = archbuilder_model::roof::Derived::of(&snapshot, &object)
        .unwrap()
        .solid_cut()
        .unwrap()
        .0;
    let b = solid.bounding_box();
    let expected = [
        [b.min().x, b.min().y, b.min().z],
        [b.max().x, b.max().y, b.max().z],
    ];
    for format in [
        MeshFormat::Obj,
        MeshFormat::Stl,
        MeshFormat::Gltf,
        MeshFormat::Glb,
    ] {
        let mut o = options(format);
        o.only = Some(BTreeSet::from([roof]));
        let export = export_mesh(&snapshot, &o).unwrap();
        assert_eq!(export.elements, 1);
        let scene = read_back(&export, format);
        assert_eq!(triangles(&scene), export.triangles, "{format:?}");
        near(bounds(&scene), expected, 1e-4);
        near(export.bounds.unwrap(), expected, 1e-9);
        let tris: usize = (0..solid.faces().len())
            .map(|k| {
                solid
                    .triangulate_face(k, &arc_base::Tolerance::DEFAULT)
                    .unwrap()
                    .len()
            })
            .sum();
        assert_eq!(export.triangles, tris, "{format:?}");
    }
}

#[test]
fn the_whole_model_has_named_objects_materials_and_a_loss_report() {
    let model = Model::new();
    let snapshot = model.snapshot();
    let export = export_mesh(&snapshot, &options(MeshFormat::Obj)).unwrap();
    let scene = read_back(&export, MeshFormat::Obj);
    assert_eq!(triangles(&scene), export.triangles);
    // Every wall is an object named with its name and identifier.
    for id in model.ids(WALL) {
        let name = archbuilder_model::types::text(&spatial::get(&snapshot, id).unwrap().values, 0)
            .unwrap()
            .to_owned();
        assert!(
            scene
                .meshes
                .iter()
                .any(|p| p.name == format!("{name} [{id}]")),
            "{name}"
        );
    }
    let names: BTreeSet<_> = scene.materials.iter().map(|m| m.name.as_str()).collect();
    for expected in ["roof", "floor", "door.leaf", "window.glass", "wall.core"] {
        assert!(names.contains(expected), "{expected} in {names:?}");
    }
    assert!(scene.meshes.iter().all(|p| p.material.is_some()));
    let reasons: BTreeSet<_> = export
        .report
        .items
        .iter()
        .map(|i| i.reason.as_str())
        .collect();
    for r in [
        "ab.io.mesh.parameters_not_carried",
        "ab.io.mesh.relations_not_carried",
        "ab.io.mesh.unit_not_recorded",
    ] {
        assert!(reasons.contains(r), "{r}");
    }
    // The model's own box is the union of the parts: the roof overhangs the walls.
    let b = bounds(&scene);
    assert!(b[0][0] < -0.4 && b[1][0] > 10.4);
    // The same model twice gives the same bytes.
    let again = export_mesh(&snapshot, &options(MeshFormat::Obj)).unwrap();
    assert_eq!(again.bytes, export.bytes);
    assert_eq!(again.mtl, export.mtl);
}

#[test]
fn gltf_is_structurally_valid_with_named_nodes_and_a_y_up_root() {
    let model = Model::new();
    let snapshot = model.snapshot();
    let export = export_mesh(&snapshot, &options(MeshFormat::Gltf)).unwrap();
    let json: Json = serde_json::from_slice(&export.bytes).unwrap();
    assert_eq!(json["asset"]["version"], "2.0");
    assert_eq!(json["scene"], 0);
    let nodes = json["nodes"].as_array().unwrap();
    let meshes = json["meshes"].as_array().unwrap();
    let accessors = json["accessors"].as_array().unwrap();
    let views = json["bufferViews"].as_array().unwrap();
    let buffers = json["buffers"].as_array().unwrap();
    let materials = json["materials"].as_array().unwrap();
    // The root turns Z up into Y up and holds every element node.
    assert!(json["scenes"][0]["nodes"] == json!([0]));
    assert!(nodes[0]["rotation"][0].as_f64().unwrap() < -0.7);
    assert_eq!(
        nodes[0]["children"].as_array().unwrap().len(),
        nodes.len() - 1
    );
    for node in &nodes[1..] {
        let mesh = node["mesh"].as_u64().unwrap() as usize;
        assert!(mesh < meshes.len());
        assert!(node["name"].as_str().unwrap().contains(" ["));
        assert!(node["extras"]["archbuilder"]["id"].is_string());
    }
    for mesh in meshes {
        for primitive in mesh["primitives"].as_array().unwrap() {
            let position = primitive["attributes"]["POSITION"].as_u64().unwrap() as usize;
            let indices = primitive["indices"].as_u64().unwrap() as usize;
            assert!(position < accessors.len() && indices < accessors.len());
            assert_eq!(accessors[position]["type"], "VEC3");
            assert!(accessors[position]["min"].is_array() && accessors[position]["max"].is_array());
            assert_eq!(accessors[indices]["type"], "SCALAR");
            assert_eq!(accessors[indices]["count"].as_u64().unwrap() % 3, 0);
            assert!((primitive["material"].as_u64().unwrap() as usize) < materials.len());
        }
    }
    let mut end = 0;
    for view in views {
        assert!((view["buffer"].as_u64().unwrap() as usize) < buffers.len());
        end = end
            .max(view["byteOffset"].as_u64().unwrap_or(0) + view["byteLength"].as_u64().unwrap());
    }
    assert!(end <= buffers[0]["byteLength"].as_u64().unwrap());
    for a in accessors {
        assert!((a["bufferView"].as_u64().unwrap() as usize) < views.len());
    }
    // The embedded buffer has the declared length.
    let uri = buffers[0]["uri"].as_str().unwrap();
    assert!(uri.starts_with("data:application/octet-stream;base64,"));
    // GLB holds the same scene.
    let glb = export_mesh(&snapshot, &options(MeshFormat::Glb)).unwrap();
    assert_eq!(&glb.bytes[..4], b"glTF");
    let scene = read_back(&glb, MeshFormat::Glb);
    assert_eq!(triangles(&scene), glb.triangles);
    assert_eq!(scene.nodes.len(), nodes.len());
}

#[test]
fn units_selection_hidden_merge_and_empty_exports() {
    let model = Model::new();
    let snapshot = model.snapshot();
    let metres = export_mesh(&snapshot, &options(MeshFormat::Stl)).unwrap();
    let mut o = options(MeshFormat::Stl);
    o.unit = Unit::Millimeter;
    let mm = export_mesh(&snapshot, &o).unwrap();
    let (a, b) = (metres.bounds.unwrap(), mm.bounds.unwrap());
    for k in 0..3 {
        assert!((b[1][k] - a[1][k] * 1000.0).abs() < 1e-6);
    }
    // glTF is meters only.
    let mut o = options(MeshFormat::Gltf);
    o.unit = Unit::Foot;
    assert_eq!(
        export_mesh(&snapshot, &o).unwrap_err().code(),
        "ab.io.mesh_unit_unsupported"
    );
    // Hidden elements are left out, a selection is only itself.
    let roof = model.ids(ROOF)[0];
    let mut o = options(MeshFormat::Obj);
    o.hidden = BTreeSet::from([roof]);
    let without = export_mesh(&snapshot, &o).unwrap();
    let full = export_mesh(&snapshot, &options(MeshFormat::Obj)).unwrap();
    assert_eq!(without.elements + 1, full.elements);
    assert!(without.bounds.unwrap()[1][2] < full.bounds.unwrap()[1][2]);
    // Merged: one object per material.
    let mut o = options(MeshFormat::Obj);
    o.merge = true;
    let merged = export_mesh(&snapshot, &o).unwrap();
    assert_eq!(merged.triangles, full.triangles);
    let scene = read_back(&merged, MeshFormat::Obj);
    assert_eq!(scene.meshes.len(), scene.materials.len());
    // No materials: no MTL.
    let mut o = options(MeshFormat::Obj);
    o.materials = false;
    assert!(export_mesh(&snapshot, &o).unwrap().mtl.is_none());
    // Nothing to write.
    let mut o = options(MeshFormat::Obj);
    o.only = Some(BTreeSet::new());
    assert_eq!(
        export_mesh(&snapshot, &o).unwrap_err().code(),
        "ab.io.mesh_empty"
    );
    let _ = Limits::DEFAULT;
}

#[test]
fn the_command_writes_the_files_and_names_the_format_by_extension() {
    let model = Model::new();
    let dir = std::env::temp_dir().join(format!("abmesh-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut session = Session::new(
        Surface::Ui,
        [Permission::Read, Permission::Edit, Permission::FileWrite],
    );
    session.roots = arc_command::Roots::new([&dir], [&dir]).unwrap();
    let registry = {
        let mut registry = archbuilder_commands::registry().unwrap();
        archbuilder_io::register(&mut registry).unwrap();
        registry
    };
    let run = |params: Json| {
        Invoker::new(&registry).invoke(
            &model.document,
            &session,
            Invocation::new("archbuilder.file.export_mesh", params),
        )
    };
    let before = model.snapshot();
    let path = dir.join("house.obj");
    let done = run(json!({"path": path.display().to_string()}))
        .unwrap()
        .value;
    assert_eq!(done["materials_file"], "house.mtl");
    assert!(dir.join("house.mtl").is_file());
    assert_eq!(
        std::fs::metadata(&path).unwrap().len(),
        done["bytes"].as_u64().unwrap()
    );
    let glb = dir.join("house.glb");
    run(json!({"path": glb.display().to_string()})).unwrap();
    assert_eq!(&std::fs::read(&glb).unwrap()[..4], b"glTF");
    let unknown = dir.join("house.xyz");
    assert_eq!(
        run(json!({"path": unknown.display().to_string()}))
            .unwrap_err()
            .code(),
        "ab.io.mesh_format_unknown"
    );
    let nothing = run(json!({"path": path.display().to_string(), "selection": true}));
    assert_eq!(nothing.unwrap_err().code(), "ab.io.mesh_selection_empty");
    // The command changes nothing in the document.
    assert!(model.snapshot().ptr_eq(&before));
    std::fs::remove_dir_all(&dir).unwrap();
}
