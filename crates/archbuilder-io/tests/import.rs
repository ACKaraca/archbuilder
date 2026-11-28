//! IFC import: files read into a document, one undo step, the identity map, foreign elements and
//! the loss report. The sample-file test runs when `ARC_IFC_SAMPLES` names a directory of `.ifc`
//! files (it never reads the repository's own files).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

use std::sync::Arc;

use arc_base::id::Identifier;
use arc_command::{Invocation, Invoker, Permission, Roots, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use serde_json::{Value as Json, json};

struct Env {
    document: Document,
    session: Session,
    registry: arc_command::Registry,
}

impl Env {
    fn new(dir: &std::path::Path) -> Self {
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
        let mut registry = archbuilder_commands::registry().unwrap();
        archbuilder_io::register(&mut registry).unwrap();
        let mut session = Session::new(
            Surface::Ui,
            [
                Permission::Read,
                Permission::Edit,
                Permission::FileRead,
                Permission::FileWrite,
            ],
        );
        session.roots = Roots::new([dir], [dir]).unwrap();
        Self {
            document,
            session,
            registry,
        }
    }

    fn run(&self, command: &str, params: Json) -> arc_base::Result<arc_command::Outcome> {
        let invocation = Invocation {
            seed: 77,
            ..Invocation::new(command, params)
        };
        Invoker::new(&self.registry).invoke(&self.document, &self.session, invocation)
    }
}

/// A small IFC4X3 file in millimeters: a storey 3 m up with an extruded wall of two layers, a door
/// and a window in it, a slab, a space, a grid and a column with a property set.
fn fixture() -> String {
    let g = |n: u32| format!("{n:022}");
    let mut out = String::new();
    out.push_str("ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('ViewDefinition [ReferenceView_V1.2]'),'2;1');\nFILE_NAME('fixture.ifc','2026-01-01T00:00:00',(''),(''),'test','test','');\nFILE_SCHEMA(('IFC4X3_ADD2'));\nENDSEC;\nDATA;\n");
    let lines = [
        "#1=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);".to_owned(),
        "#2=IFCUNITASSIGNMENT((#1));".to_owned(),
        "#3=IFCCARTESIANPOINT((0.,0.,0.));".to_owned(),
        "#4=IFCAXIS2PLACEMENT3D(#3,$,$);".to_owned(),
        "#5=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-05,#4,$);".to_owned(),
        "#6=IFCGEOMETRICREPRESENTATIONSUBCONTEXT('Body','Model',*,*,*,*,#5,$,.MODEL_VIEW.,$);"
            .to_owned(),
        format!(
            "#10=IFCPROJECT('{}',$,'Fixture project','A test',$,'Long fixture name','Design',(#5),#2);",
            g(10)
        ),
        format!(
            "#11=IFCSITE('{}',$,'Site',$,$,#12,$,$,.ELEMENT.,$,$,$,$,$);",
            g(11)
        ),
        "#12=IFCLOCALPLACEMENT($,#4);".to_owned(),
        format!(
            "#13=IFCBUILDING('{}',$,'House',$,$,#14,$,$,.ELEMENT.,$,$,$);",
            g(13)
        ),
        "#14=IFCLOCALPLACEMENT(#12,#4);".to_owned(),
        format!(
            "#15=IFCBUILDINGSTOREY('{}',$,'Level 1',$,$,#17,$,'First',.ELEMENT.,3000.);",
            g(15)
        ),
        "#16=IFCCARTESIANPOINT((0.,0.,3000.));".to_owned(),
        "#17=IFCLOCALPLACEMENT(#14,#18);".to_owned(),
        "#18=IFCAXIS2PLACEMENT3D(#16,$,$);".to_owned(),
        format!("#19=IFCRELAGGREGATES('{}',$,$,$,#10,(#11));", g(19)),
        format!("#20=IFCRELAGGREGATES('{}',$,$,$,#11,(#13));", g(20)),
        format!("#21=IFCRELAGGREGATES('{}',$,$,$,#13,(#15));", g(21)),
        // The wall: 4 m along x, 0.2 m thick, 2.8 m high, two layers.
        "#30=IFCCARTESIANPOINT((1000.,2000.,0.));".to_owned(),
        "#31=IFCAXIS2PLACEMENT3D(#30,$,$);".to_owned(),
        "#32=IFCLOCALPLACEMENT(#17,#31);".to_owned(),
        "#33=IFCCARTESIANPOINT((2000.,0.));".to_owned(),
        "#34=IFCAXIS2PLACEMENT2D(#33,$);".to_owned(),
        "#35=IFCRECTANGLEPROFILEDEF(.AREA.,$,#34,4000.,200.);".to_owned(),
        "#36=IFCDIRECTION((0.,0.,1.));".to_owned(),
        "#37=IFCEXTRUDEDAREASOLID(#35,#4,#36,2800.);".to_owned(),
        "#38=IFCSHAPEREPRESENTATION(#6,'Body','SweptSolid',(#37));".to_owned(),
        "#39=IFCPRODUCTDEFINITIONSHAPE($,$,(#38));".to_owned(),
        format!(
            "#40=IFCWALL('{}',$,'Wall A',$,$,#32,#39,$,.STANDARD.);",
            g(40)
        ),
        "#41=IFCMATERIAL('Brick',$,$);".to_owned(),
        "#42=IFCMATERIAL('Plaster',$,$);".to_owned(),
        "#43=IFCMATERIALLAYER(#41,100.,$,'Outer',$,$,$);".to_owned(),
        "#44=IFCMATERIALLAYER(#42,100.,$,'Inner',$,$,$);".to_owned(),
        "#45=IFCMATERIALLAYERSET((#43,#44),'Brick wall',$);".to_owned(),
        "#46=IFCMATERIALLAYERSETUSAGE(#45,.AXIS2.,.POSITIVE.,-100.,$);".to_owned(),
        format!("#47=IFCRELASSOCIATESMATERIAL('{}',$,$,$,(#40),#46);", g(47)),
        // The opening and the door in it: 0.9 m wide, 2.1 m high, 1 m along the wall.
        "#50=IFCCARTESIANPOINT((1000.,0.,0.));".to_owned(),
        "#51=IFCAXIS2PLACEMENT3D(#50,$,$);".to_owned(),
        "#52=IFCLOCALPLACEMENT(#32,#51);".to_owned(),
        "#53=IFCRECTANGLEPROFILEDEF(.AREA.,$,#34,900.,400.);".to_owned(),
        "#54=IFCEXTRUDEDAREASOLID(#53,#4,#36,2100.);".to_owned(),
        "#55=IFCSHAPEREPRESENTATION(#6,'Body','SweptSolid',(#54));".to_owned(),
        "#56=IFCPRODUCTDEFINITIONSHAPE($,$,(#55));".to_owned(),
        format!(
            "#57=IFCOPENINGELEMENT('{}',$,'Opening 1',$,$,#52,#56,$,.OPENING.);",
            g(57)
        ),
        format!("#58=IFCRELVOIDSELEMENT('{}',$,$,$,#40,#57);", g(58)),
        "#59=IFCCARTESIANPOINT((0.,0.));".to_owned(),
        "#60=IFCAXIS2PLACEMENT2D(#59,$);".to_owned(),
        "#61=IFCRECTANGLEPROFILEDEF(.AREA.,$,#60,900.,100.);".to_owned(),
        "#62=IFCEXTRUDEDAREASOLID(#61,#4,#36,2100.);".to_owned(),
        "#63=IFCSHAPEREPRESENTATION(#6,'Body','SweptSolid',(#62));".to_owned(),
        "#64=IFCPRODUCTDEFINITIONSHAPE($,$,(#63));".to_owned(),
        format!(
            "#65=IFCDOOR('{}',$,'Door 1',$,$,#52,#64,$,2100.,900.,.DOOR.,.SINGLE_SWING_LEFT.,$);",
            g(65)
        ),
        format!("#66=IFCRELFILLSELEMENT('{}',$,$,$,#57,#65);", g(66)),
        // A window 2.5 m along the wall, 1.2 m wide and 1.2 m high, 0.9 m above its base.
        "#70=IFCCARTESIANPOINT((2500.,0.,900.));".to_owned(),
        "#71=IFCAXIS2PLACEMENT3D(#70,$,$);".to_owned(),
        "#72=IFCLOCALPLACEMENT(#32,#71);".to_owned(),
        "#73=IFCRECTANGLEPROFILEDEF(.AREA.,$,#34,1200.,400.);".to_owned(),
        "#74=IFCEXTRUDEDAREASOLID(#73,#4,#36,1200.);".to_owned(),
        "#75=IFCSHAPEREPRESENTATION(#6,'Body','SweptSolid',(#74));".to_owned(),
        "#76=IFCPRODUCTDEFINITIONSHAPE($,$,(#75));".to_owned(),
        format!(
            "#77=IFCOPENINGELEMENT('{}',$,'Opening 2',$,$,#72,#76,$,.OPENING.);",
            g(77)
        ),
        format!("#78=IFCRELVOIDSELEMENT('{}',$,$,$,#40,#77);", g(78)),
        "#79=IFCRECTANGLEPROFILEDEF(.AREA.,$,#60,1200.,100.);".to_owned(),
        "#80=IFCEXTRUDEDAREASOLID(#79,#4,#36,1200.);".to_owned(),
        "#81=IFCSHAPEREPRESENTATION(#6,'Body','SweptSolid',(#80));".to_owned(),
        "#82=IFCPRODUCTDEFINITIONSHAPE($,$,(#81));".to_owned(),
        format!(
            "#83=IFCWINDOW('{}',$,'Window 1',$,$,#72,#82,$,1200.,1200.,.WINDOW.,.SINGLE_PANEL.,$);",
            g(83)
        ),
        format!("#84=IFCRELFILLSELEMENT('{}',$,$,$,#77,#83);", g(84)),
        // A slab: 5 m by 4 m, 0.2 m thick, its underside at the storey.
        "#90=IFCLOCALPLACEMENT(#17,#4);".to_owned(),
        "#91=IFCCARTESIANPOINT((2500.,2000.));".to_owned(),
        "#92=IFCAXIS2PLACEMENT2D(#91,$);".to_owned(),
        "#93=IFCRECTANGLEPROFILEDEF(.AREA.,$,#92,5000.,4000.);".to_owned(),
        "#94=IFCEXTRUDEDAREASOLID(#93,#4,#36,200.);".to_owned(),
        "#95=IFCSHAPEREPRESENTATION(#6,'Body','SweptSolid',(#94));".to_owned(),
        "#96=IFCPRODUCTDEFINITIONSHAPE($,$,(#95));".to_owned(),
        format!("#97=IFCSLAB('{}',$,'Slab 1',$,$,#90,#96,$,.FLOOR.);", g(97)),
        // A space inside.
        "#100=IFCCARTESIANPOINT((2000.,2000.));".to_owned(),
        "#101=IFCAXIS2PLACEMENT2D(#100,$);".to_owned(),
        "#102=IFCRECTANGLEPROFILEDEF(.AREA.,$,#101,3000.,3000.);".to_owned(),
        "#103=IFCEXTRUDEDAREASOLID(#102,#4,#36,2800.);".to_owned(),
        "#104=IFCSHAPEREPRESENTATION(#6,'Body','SweptSolid',(#103));".to_owned(),
        "#105=IFCPRODUCTDEFINITIONSHAPE($,$,(#104));".to_owned(),
        format!(
            "#106=IFCSPACE('{}',$,'A101',$,$,#90,#105,'Living room',.ELEMENT.,.INTERNAL.,$);",
            g(106)
        ),
        // A column: no native counterpart, so a foreign element with a property set.
        "#110=IFCCARTESIANPOINT((500.,500.,0.));".to_owned(),
        "#111=IFCAXIS2PLACEMENT3D(#110,$,$);".to_owned(),
        "#112=IFCLOCALPLACEMENT(#17,#111);".to_owned(),
        "#113=IFCRECTANGLEPROFILEDEF(.AREA.,$,#60,300.,300.);".to_owned(),
        "#114=IFCEXTRUDEDAREASOLID(#113,#4,#36,3000.);".to_owned(),
        "#115=IFCSHAPEREPRESENTATION(#6,'Body','SweptSolid',(#114));".to_owned(),
        "#116=IFCPRODUCTDEFINITIONSHAPE($,$,(#115));".to_owned(),
        format!(
            "#117=IFCCOLUMN('{}',$,'Column C1',$,$,#112,#116,'tag-7',.COLUMN.);",
            g(117)
        ),
        "#118=IFCPROPERTYSINGLEVALUE('Reference',$,IFCIDENTIFIER('C1'),$);".to_owned(),
        "#119=IFCPROPERTYSINGLEVALUE('IsExternal',$,IFCBOOLEAN(.T.),$);".to_owned(),
        "#120=IFCPROPERTYSINGLEVALUE('Span',$,IFCLENGTHMEASURE(2500.),$);".to_owned(),
        format!(
            "#121=IFCPROPERTYSET('{}',$,'Pset_ColumnCommon',$,(#118,#119,#120));",
            g(121)
        ),
        format!(
            "#122=IFCRELDEFINESBYPROPERTIES('{}',$,$,$,(#117),#121);",
            g(122)
        ),
        // The grid: two axes in x.
        "#130=IFCCARTESIANPOINT((0.,0.));".to_owned(),
        "#131=IFCCARTESIANPOINT((6000.,0.));".to_owned(),
        "#132=IFCPOLYLINE((#130,#131));".to_owned(),
        "#133=IFCCARTESIANPOINT((0.,4000.));".to_owned(),
        "#134=IFCCARTESIANPOINT((6000.,4000.));".to_owned(),
        "#135=IFCPOLYLINE((#133,#134));".to_owned(),
        "#136=IFCGRIDAXIS('A',#132,.T.);".to_owned(),
        "#137=IFCGRIDAXIS('B',#135,.T.);".to_owned(),
        // A circular axis: not a line the import carries, so it is listed.
        "#141=IFCCARTESIANPOINT((0.,0.));".to_owned(),
        "#142=IFCAXIS2PLACEMENT2D(#141,$);".to_owned(),
        "#143=IFCCIRCLE(#142,1000.);".to_owned(),
        "#144=IFCGRIDAXIS('C',#143,.T.);".to_owned(),
        format!(
            "#138=IFCGRID('{}',$,'Grid',$,$,#90,$,(#136,#137),(#144),$,$);",
            g(138)
        ),
        format!(
            "#139=IFCRELCONTAINEDINSPATIALSTRUCTURE('{}',$,$,$,(#40,#65,#83,#97,#117,#138),#15);",
            g(139)
        ),
        format!("#140=IFCRELAGGREGATES('{}',$,$,$,#15,(#106));", g(140)),
    ];
    for l in lines {
        out.push_str(&l);
        out.push('\n');
    }
    out.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    out
}

/// An empty directory below the temporary one.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("archbuilder-import-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn count(document: &Document, key: &str) -> usize {
    archbuilder_model::spatial::objects(&document.snapshot(), key).len()
}

#[test]
fn a_file_is_imported_in_one_undo_step_with_native_and_foreign_elements() {
    let dir = scratch("one-step");
    std::fs::write(dir.join("fixture.ifc"), fixture()).unwrap();
    let env = Env::new(&dir);
    let path = dir.join("fixture.ifc");
    let out = env
        .run(
            "archbuilder.file.import_ifc",
            json!({"path": path.to_str().unwrap()}),
        )
        .unwrap()
        .value;
    assert_eq!(out["schema"], "IFC4X3");
    use archbuilder_model::types;
    for (key, expected) in [
        (types::PROJECT, 1),
        (types::BUILDING, 1),
        (types::STOREY, 1),
        (types::WALL, 1),
        (types::DOOR, 1),
        (types::WINDOW, 1),
        (types::FLOOR, 1),
        (types::ROOM, 1),
        (types::GRID, 2),
        (types::FOREIGN_ELEMENT, 1),
        (types::WALL_TYPE, 1),
    ] {
        assert_eq!(count(&env.document, key), expected, "{key}");
    }
    // The wall: 4 m along the axis, two layers, 2.8 m high, 3 m up.
    let snapshot = env.document.snapshot();
    let wall = archbuilder_model::spatial::objects(&snapshot, types::WALL)[0].clone();
    let wall_type = archbuilder_model::spatial::objects(&snapshot, types::WALL_TYPE)[0].clone();
    let layers = archbuilder_model::element_type::Layers::from_values(&wall_type.values).unwrap();
    assert_eq!(layers.layers.len(), 2);
    assert!((layers.total() - 0.2).abs() < 1e-9);
    let w = archbuilder_model::wall::Wall::from_values(&wall.values).unwrap();
    assert!((w.length() - 4.0).abs() < 1e-6 && (w.height - 2.8).abs() < 1e-6);
    // The identity: the wall's identifier is the source GlobalId, so a re-export writes it again.
    assert_eq!(
        arc_io_ifc::write::global_id(wall.id.as_u128()),
        format!("{:022}", 40)
    );
    // The column is a foreign element with its class, mesh and property set.
    let column = archbuilder_model::spatial::objects(&snapshot, types::FOREIGN_ELEMENT)[0].clone();
    assert_eq!(
        types::text(&column.values, types::foreign_element::CLASS),
        Some("IfcColumn")
    );
    assert!(matches!(
        column.values[types::foreign_element::MESH],
        Some(arc_doc::Value::Geometry(_))
    ));
    // Every product is accounted for: mapped, kept as foreign, or listed as not carried.
    let classes = out["classes"].as_array().unwrap();
    for c in classes {
        let total = c["native"].as_u64().unwrap()
            + c["foreign"].as_u64().unwrap()
            + c["lost"].as_u64().unwrap()
            + c["matched"].as_u64().unwrap();
        assert_eq!(total, c["seen"].as_u64().unwrap(), "{c}");
    }
    assert!(out["report"]["items"].as_array().unwrap().iter().any(|i| i["reason"] == "ab.io.ifc.import_preserved" && i["record_kind"] == "IfcColumn"));
    assert!(
        out["report"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["reason"] == "ab.io.ifc.import_not_carried"
                && i["record_kind"] == "IfcGridAxis"),
        "the circular grid axis is listed"
    );
    // The mesh export writes the column with its stored mesh.
    let mesh = archbuilder_io::mesh::export_mesh(
        &snapshot,
        &archbuilder_io::mesh::MeshOptions {
            only: Some(std::iter::once(column.id).collect()),
            ..archbuilder_io::mesh::MeshOptions::default()
        },
    )
    .unwrap();
    assert_eq!(mesh.elements, 1);
    assert!(mesh.triangles >= 12);
    // One undo step takes everything back.
    env.document.undo().wait().unwrap();
    assert_eq!(count(&env.document, types::WALL), 0);
    assert_eq!(count(&env.document, types::FOREIGN_ELEMENT), 0);
    assert_eq!(count(&env.document, types::PROJECT), 0);
}

#[test]
fn an_imported_file_exports_with_its_identities_foreign_elements_and_properties() {
    let dir = scratch("export");
    std::fs::write(dir.join("fixture.ifc"), fixture()).unwrap();
    let env = Env::new(&dir);
    env.run(
        "archbuilder.file.import_ifc",
        json!({"path": dir.join("fixture.ifc").to_str().unwrap()}),
    )
    .unwrap();
    let target = dir.join("out.ifc");
    let exported = env
        .run(
            "archbuilder.file.export_ifc",
            json!({"path": target.to_str().unwrap(), "schema": "ifc4x3"}),
        )
        .unwrap()
        .value;
    assert_eq!(exported["valid"], true, "{}", exported["report"]["items"]);
    let bytes = std::fs::read(&target).unwrap();
    let file = arc_io_ifc::read(&bytes, &mut arc_io_ifc::ReadOptions::default())
        .unwrap()
        .file;
    let gids: std::collections::BTreeSet<String> = file
        .instances()
        .iter()
        .filter_map(|i| match i.records.first()?.parameters.first()? {
            arc_io_ifc::Parameter::Text(t) => Some(t.clone()),
            _ => None,
        })
        .collect();
    for n in [40_u32, 65, 83, 97, 106, 117] {
        assert!(
            gids.contains(&format!("{n:022}")),
            "GlobalId {n} is not in the export"
        );
    }
    // The column is an IfcColumn again, with its predefined type, tag-less stand-in replaced.
    assert!(
        file.instances_of("IFCCOLUMN").count() == 1,
        "the column is written as its class"
    );
    assert_eq!(
        file.instances_of("IFCMEMBER").count(),
        0,
        "no stand-in is left"
    );
    let views = file
        .typed_views(&arc_base::Limits::DEFAULT, &arc_base::Cancel::new())
        .unwrap();
    let kept = views
        .import
        .objects
        .iter()
        .find(|o| o.type_name == "IFCCOLUMN")
        .unwrap();
    assert_eq!(kept.effective_predefined_type.as_deref(), Some("COLUMN"));
    let pset = kept
        .property_sets
        .iter()
        .find(|s| s.name.as_deref() == Some("Pset_ColumnCommon"))
        .expect("the property set is written back");
    assert_eq!(pset.properties.len(), 3);
    // The span is a length: 2500 mm in the file, 2.5 m in the export.
    let span = pset
        .properties
        .iter()
        .find(|p| p.property.name.as_deref() == Some("Span"))
        .unwrap();
    assert!(span.si_numbers().iter().any(|v| (v - 2.5).abs() < 1e-9));
    // Importing the export again changes nothing: every product is matched by its identifier.
    let again = env
        .run(
            "archbuilder.file.import_ifc",
            json!({"path": target.to_str().unwrap()}),
        )
        .unwrap()
        .value;
    // The openings are carried by their doors and windows and have no identity of their own.
    let made: u64 = again["classes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["class"] != "IfcOpeningElement")
        .map(|c| c["native"].as_u64().unwrap() + c["foreign"].as_u64().unwrap())
        .sum();
    assert_eq!(made, 0);
    assert_eq!(again["matched"].as_u64().unwrap(), 6);
    assert_eq!(count(&env.document, archbuilder_model::types::WALL), 1);
    // The building and storeys the document already has are matched too, not made again.
    for key in [
        archbuilder_model::types::BUILDING,
        archbuilder_model::types::STOREY,
        archbuilder_model::types::LEVEL,
    ] {
        assert_eq!(count(&env.document, key), 1, "{key}");
    }
}

fn find(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.is_dir() {
            find(&path, out);
        } else if path.extension().is_some_and(|e| e == "ifc") {
            out.push(path);
        }
    }
}

/// Import, export and import again every sample under `ARC_IFC_SAMPLES`; the exported files go to
/// `ARC_IFC_OUT` for the independent comparison (`tools/ifc-oracle`).
#[test]
fn samples_round_trip() {
    let (Ok(dir), Ok(out)) = (
        std::env::var("ARC_IFC_SAMPLES"),
        std::env::var("ARC_IFC_OUT"),
    ) else {
        return;
    };
    let mut files = Vec::new();
    find(std::path::Path::new(&dir), &mut files);
    files.sort();
    std::fs::create_dir_all(&out).unwrap();
    let mut lines: Vec<String> = Vec::new();
    for file in files {
        let rel = file
            .strip_prefix(&dir)
            .unwrap()
            .to_str()
            .unwrap()
            .replace('/', "__");
        let exported = std::path::Path::new(&out).join(&rel);
        let env = Env::new(std::path::Path::new(&dir));
        let out_env = std::path::Path::new(&out);
        let first = env
            .run(
                "archbuilder.file.import_ifc",
                json!({"path": file.to_str().unwrap()}),
            )
            .unwrap()
            .value;
        // The session of the test may write only below the sample directory's root: give it the
        // output directory too.
        let mut session = Session::new(
            Surface::Ui,
            [
                Permission::Read,
                Permission::Edit,
                Permission::FileRead,
                Permission::FileWrite,
            ],
        );
        session.roots = Roots::new([out_env, std::path::Path::new(&dir)], [out_env]).unwrap();
        let run = |c: &str, p: Json| {
            let invocation = Invocation {
                seed: 78,
                ..Invocation::new(c, p)
            };
            Invoker::new(&env.registry).invoke(&env.document, &session, invocation)
        };
        let exp = run(
            "archbuilder.file.export_ifc",
            json!({"path": exported.to_str().unwrap()}),
        );
        match exp {
            Ok(o) => {
                let v = o.value;
                let failed: Vec<String> = v["report"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|i| i["outcome"] == "failed")
                    .map(|i| format!("{} {} {}", i["reason"], i["record_kind"], i["arguments"]))
                    .collect();
                // Import the exported file into a fresh document.
                let env2 = Env::new(out_env);
                let second = env2
                    .run(
                        "archbuilder.file.import_ifc",
                        json!({"path": exported.to_str().unwrap()}),
                    )
                    .unwrap()
                    .value;
                lines.push(format!(
                    "{rel}: in {}/{}/{}/{} -> out valid={} elements={} failed={} -> re {}/{}/{}/{}",
                    first["products"],
                    first["native"],
                    first["foreign"],
                    first["lost"],
                    v["valid"],
                    v["elements"],
                    failed.len(),
                    second["products"],
                    second["native"],
                    second["foreign"],
                    second["lost"],
                ));
                for f in failed.iter().take(3) {
                    lines.push(format!("    FAILED {}", &f[..f.len().min(300)]));
                }
                assert!(failed.is_empty(), "{rel}: {failed:?}");
            }
            Err(e) => panic!("{rel}: export failed: {e:?}"),
        }
    }
    std::fs::write(
        std::path::Path::new(&out).join("round-trip.txt"),
        lines.join("\n"),
    )
    .unwrap();
}

#[test]
fn a_file_that_is_not_ifc_is_refused_and_changes_nothing() {
    let dir = scratch("garbage");
    std::fs::write(
        dir.join("bad.ifc"),
        b"ISO-10303-21; this is not a model \x00\xff",
    )
    .unwrap();
    let env = Env::new(&dir);
    let error = env
        .run(
            "archbuilder.file.import_ifc",
            json!({"path": dir.join("bad.ifc").to_str().unwrap()}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "ab.io.import_unreadable");
    assert_eq!(count(&env.document, archbuilder_model::types::PROJECT), 0);
}

#[test]
fn the_elements_of_an_export_come_back_with_their_identifiers() {
    let dir = scratch("own");
    let env = Env::new(&dir);
    let run = |c: &str, p: Json| env.run(c, p).unwrap().value;
    run("archbuilder.project.new", json!({}));
    let corners = [[0.0, 0.0], [6.0, 0.0], [6.0, 4.0], [0.0, 4.0]];
    let mut walls = Vec::new();
    for k in 0..4 {
        let made = run(
            "archbuilder.wall.create",
            json!({"start": corners[k], "end": corners[(k + 1) % 4], "height": 2.8}),
        );
        walls.push(made["wall"].as_str().unwrap().to_owned());
    }
    run(
        "archbuilder.door.place",
        json!({"wall": walls[0], "distance": 2.0}),
    );
    run(
        "archbuilder.window.place",
        json!({"wall": walls[1], "distance": 2.0}),
    );
    run(
        "archbuilder.floor.create",
        json!({"points": [[0.0, 0.0], [6.0, 0.0], [6.0, 4.0], [0.0, 4.0]]}),
    );
    let target = dir.join("own.ifc");
    let exported = run(
        "archbuilder.file.export_ifc",
        json!({"path": target.to_str().unwrap(), "schema": "ifc4x3"}),
    );
    assert_eq!(exported["valid"], true);
    // A fresh document reads the file: the same elements, under the same identifiers.
    let other = Env::new(&dir);
    let imported = other
        .run(
            "archbuilder.file.import_ifc",
            json!({"path": target.to_str().unwrap()}),
        )
        .unwrap()
        .value;
    use archbuilder_model::types;
    for key in [types::WALL, types::DOOR, types::WINDOW, types::FLOOR] {
        let mine: std::collections::BTreeSet<_> =
            archbuilder_model::spatial::objects(&env.document.snapshot(), key)
                .iter()
                .map(|o| o.id)
                .collect();
        let theirs: std::collections::BTreeSet<_> =
            archbuilder_model::spatial::objects(&other.document.snapshot(), key)
                .iter()
                .map(|o| o.id)
                .collect();
        assert_eq!(mine, theirs, "{key}: {}", imported["classes"]);
    }
}

#[test]
fn a_foreign_element_is_deleted_in_one_step_and_blocks_deleting_its_storey() {
    use archbuilder_model::types;
    let dir = scratch("delete");
    std::fs::write(dir.join("fixture.ifc"), fixture()).unwrap();
    let env = Env::new(&dir);
    env.run(
        "archbuilder.file.import_ifc",
        json!({"path": dir.join("fixture.ifc").to_str().unwrap()}),
    )
    .unwrap();
    let snapshot = env.document.snapshot();
    let column = archbuilder_model::spatial::objects(&snapshot, types::FOREIGN_ELEMENT)[0].id;
    let storey = archbuilder_model::spatial::objects(&snapshot, types::STOREY)[0].id;
    let with_selection = |command: &str, selection: Vec<arc_base::ObjectId>| {
        let invocation = Invocation {
            seed: 79,
            selection,
            ..Invocation::new(command, json!({}))
        };
        Invoker::new(&env.registry).invoke(&env.document, &env.session, invocation)
    };
    // The storey holds the foreign element, so it is not deleted with it silently.
    assert!(with_selection("archbuilder.storey.delete", vec![storey]).is_err());
    with_selection("archbuilder.object.delete", vec![column]).unwrap();
    assert_eq!(count(&env.document, types::FOREIGN_ELEMENT), 0);
    env.document.undo().wait().unwrap();
    assert_eq!(count(&env.document, types::FOREIGN_ELEMENT), 1);
}

#[test]
fn a_door_that_does_not_fit_its_wall_becomes_a_foreign_element() {
    let dir = scratch("overlap");
    // The window overlaps the door: the model refuses that, the import keeps going.
    let text = fixture().replace(
        "#70=IFCCARTESIANPOINT((2500.,0.,900.));",
        "#70=IFCCARTESIANPOINT((1200.,0.,900.));",
    );
    std::fs::write(dir.join("overlap.ifc"), text).unwrap();
    let env = Env::new(&dir);
    let out = env
        .run(
            "archbuilder.file.import_ifc",
            json!({"path": dir.join("overlap.ifc").to_str().unwrap()}),
        )
        .unwrap()
        .value;
    use archbuilder_model::types;
    assert_eq!(count(&env.document, types::DOOR), 1);
    assert_eq!(count(&env.document, types::WINDOW), 0);
    assert_eq!(count(&env.document, types::FOREIGN_ELEMENT), 2);
    let snapshot = env.document.snapshot();
    let window = archbuilder_model::spatial::objects(&snapshot, types::FOREIGN_ELEMENT)
        .into_iter()
        .find(|o| types::text(&o.values, types::foreign_element::CLASS) == Some("IfcWindow"))
        .unwrap();
    assert_eq!(
        arc_io_ifc::write::global_id(window.id.as_u128()),
        format!("{:022}", 83),
        "the window keeps its identity"
    );
    assert!(out["report"]["items"].as_array().unwrap().iter().any(|i| {
        i["reason"] == "ab.io.ifc.import_insert_refused" && i["record_kind"] == "IfcWindow"
    }));
    let window_class = out["classes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["class"] == "IfcWindow")
        .unwrap();
    assert_eq!(window_class["native"], 0);
    assert_eq!(window_class["foreign"], 1);
}
