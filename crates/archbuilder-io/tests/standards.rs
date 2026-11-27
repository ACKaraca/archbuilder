//! A project started from a template document and standards transferred from another document
//! file: new identifiers, the template untouched, name conflicts settled per type, one undo step.

#![allow(clippy::unwrap_used, clippy::too_many_lines, reason = "test code")]

use std::sync::Arc;

use arc_command::{Invocation, Invoker, Permission, Roots, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::package::{LockOwner, Metadata, Package};
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::spatial;
use archbuilder_model::types::{PLAN_VIEW, WALL_TYPE, text};
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
        let env = Self {
            document,
            session,
            registry,
        };
        env.run("archbuilder.project.new", json!({})).unwrap();
        env
    }

    fn run(&self, command: &str, params: Json) -> arc_base::Result<arc_command::Outcome> {
        let invocation = Invocation {
            seed: 99,
            ..Invocation::new(command, params)
        };
        Invoker::new(&self.registry).invoke(&self.document, &self.session, invocation)
    }

    fn ok(&self, command: &str, params: Json) -> Json {
        self.run(command, params).unwrap().value
    }

    fn save(&self, path: &std::path::Path) {
        let owner = LockOwner {
            user: "test".into(),
            host: "test".into(),
            application: "archbuilder tests".into(),
            time_unix_ms: 0,
        };
        let mut package = Package::create(path, &Metadata::default(), &owner).unwrap();
        package
            .save_document(&self.document, &[], None, &mut |_| Ok(()))
            .unwrap();
    }

    fn type_names(&self) -> Vec<String> {
        let mut names: Vec<String> = spatial::objects(&self.document.snapshot(), WALL_TYPE)
            .iter()
            .map(|o| text(&o.values, 0).unwrap().to_owned())
            .collect();
        names.sort();
        names
    }
}

#[test]
fn a_project_starts_from_a_template_with_new_identifiers_and_one_undo_step() {
    let dir = std::env::temp_dir().join(format!("archbuilder-template-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let template = Env::new(&dir);
    template.ok(
        "archbuilder.project.set_units",
        json!({"length_unit": "millimeter", "area_unit": "square_foot", "area_precision": 1}),
    );
    template.ok(
        "archbuilder.project.set_info",
        json!({"name": "Office template", "organization": "Arc Studio", "status": "Design"}),
    );
    let generic = spatial::objects(&template.document.snapshot(), WALL_TYPE)[0]
        .id
        .to_string();
    template.ok(
        "archbuilder.type.duplicate",
        json!({"type": generic, "name": "Office partition"}),
    );
    let file = dir.join("office.archb");
    template.save(&file);
    let before = std::fs::read(&file).unwrap();
    let template_ids: Vec<_> = spatial::objects(&template.document.snapshot(), PLAN_VIEW)
        .iter()
        .map(|o| o.id)
        .collect();

    let project = Env::new(&dir);
    let old_name = {
        let s = project.document.snapshot();
        text(&spatial::project(&s).unwrap().values, 0)
            .unwrap()
            .to_owned()
    };
    let made = project.ok(
        "archbuilder.file.new_from_template",
        json!({"path": file.to_str().unwrap()}),
    );
    assert!(made["copied"].as_u64().unwrap() > 10, "{made}");
    let s = project.document.snapshot();
    let p = spatial::project(&s).unwrap();
    assert_eq!(text(&p.values, 0), Some("Office template"));
    assert_eq!(
        text(
            &p.values,
            archbuilder_model::types::project::TEMPLATE_SOURCE
        ),
        Some("office.archb")
    );
    assert_eq!(spatial::unit_settings(&s).area_text(1.0, '.'), "10.8 ft²");
    assert!(
        project
            .type_names()
            .contains(&"Office partition".to_owned())
    );
    // New identities: nothing of the template's identifiers is in the project.
    let views: Vec<_> = spatial::objects(&s, PLAN_VIEW)
        .iter()
        .map(|o| o.id)
        .collect();
    assert_eq!(views.len(), template_ids.len());
    assert!(views.iter().all(|id| !template_ids.contains(id)));
    // The template is unchanged, and one undo brings the old structure back.
    assert_eq!(std::fs::read(&file).unwrap(), before);
    project.document.undo().wait().unwrap();
    let s = project.document.snapshot();
    assert_eq!(
        text(&spatial::project(&s).unwrap().values, 0),
        Some(old_name.as_str())
    );
    // A document with elements is refused, as is a file that is no document.
    let storey = spatial::storeys(&s, spatial::buildings(&s)[0].id)[0]
        .storey
        .id
        .to_string();
    project.ok(
        "archbuilder.wall.create",
        json!({"storey": storey, "start": [0.0, 0.0], "end": [4.0, 0.0]}),
    );
    assert!(
        project
            .run(
                "archbuilder.file.new_from_template",
                json!({"path": file.to_str().unwrap()})
            )
            .is_err()
    );
    let junk = dir.join("junk.archb");
    std::fs::write(&junk, b"not a document").unwrap();
    let empty = Env::new(&dir);
    assert!(
        empty
            .run(
                "archbuilder.file.new_from_template",
                json!({"path": junk.to_str().unwrap()})
            )
            .is_err()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn standards_are_copied_with_conflicts_settled_per_type_and_undone_in_one_step() {
    let dir = std::env::temp_dir().join(format!("archbuilder-standards-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let source = Env::new(&dir);
    let generic = spatial::objects(&source.document.snapshot(), WALL_TYPE)[0]
        .id
        .to_string();
    let generic_name = source.type_names()[0].clone();
    source.ok(
        "archbuilder.type.duplicate",
        json!({"type": generic, "name": "Brick 20"}),
    );
    let brick = spatial::objects(&source.document.snapshot(), WALL_TYPE)
        .iter()
        .find(|o| text(&o.values, 0) == Some("Brick 20"))
        .unwrap()
        .id
        .to_string();
    source.ok(
        "archbuilder.type.set",
        json!({"type": brick, "load_bearing": true}),
    );
    source.ok(
        "archbuilder.project.set_units",
        json!({"angle_unit": "gradian", "length_precision": 2}),
    );
    let file = dir.join("standards.archb");
    source.save(&file);

    let target = Env::new(&dir);
    let before = target.type_names();
    // The default conflict keeps what the target has: the generic type of both documents.
    let done = target.ok(
        "archbuilder.file.transfer_standards",
        json!({"path": file.to_str().unwrap(), "kinds": ["wall_type"]}),
    );
    assert_eq!(done["copied"], 1, "{done}");
    assert_eq!(done["settings_copied"], true);
    let kept = done["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["name"] == generic_name)
        .unwrap();
    assert_eq!(kept["action"], "kept");
    assert!(target.type_names().contains(&"Brick 20".to_owned()));
    assert_eq!(
        spatial::unit_settings(&target.document.snapshot()).length_text(1.0, '.'),
        "1 m"
    );
    assert_eq!(
        spatial::unit_settings(&target.document.snapshot()).angle,
        archbuilder_model::units::AngleUnit::Gradian
    );
    // One undo takes the copied type and the settings back.
    target.document.undo().wait().unwrap();
    assert_eq!(target.type_names(), before);
    assert_eq!(
        spatial::unit_settings(&target.document.snapshot()).angle,
        archbuilder_model::units::AngleUnit::Degree
    );
    // Rename for one name, overwrite for another; a type in use is not overwritten.
    let done = target.ok(
        "archbuilder.file.transfer_standards",
        json!({"path": file.to_str().unwrap(), "kinds": ["wall_type"], "settings": false,
               "conflict": "rename", "resolutions": {generic_name.clone(): "overwrite"}}),
    );
    assert_eq!(done["overwritten"], 1, "{done}");
    assert_eq!(done["settings_copied"], false);
    assert!(
        target
            .run(
                "archbuilder.file.transfer_standards",
                json!({"path": file.to_str().unwrap(), "conflict": "merge"})
            )
            .is_err()
    );
    let again = target.ok(
        "archbuilder.file.transfer_standards",
        json!({"path": file.to_str().unwrap(), "kinds": ["wall_type"], "settings": false,
               "conflict": "rename"}),
    );
    assert!(
        target.type_names().contains(&"Brick 20 (2)".to_owned()),
        "{again}"
    );
    // A type a wall uses is never overwritten.
    let snapshot = target.document.snapshot();
    let storey = spatial::storeys(&snapshot, spatial::buildings(&snapshot)[0].id)[0]
        .storey
        .id
        .to_string();
    target.ok(
        "archbuilder.wall.create",
        json!({"storey": storey, "start": [0.0, 0.0], "end": [4.0, 0.0]}),
    );
    let used = target.ok(
        "archbuilder.file.transfer_standards",
        json!({"path": file.to_str().unwrap(), "kinds": ["wall_type"], "settings": false,
               "conflict": "overwrite"}),
    );
    let reasons: Vec<&str> = used["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i["reason"].as_str())
        .collect();
    assert!(reasons.contains(&"in_use"), "{used}");
    let _ = std::fs::remove_dir_all(&dir);
}
