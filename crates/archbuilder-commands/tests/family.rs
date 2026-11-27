//! Families in a project (ADR 0009, unit 7): load with preview and conflicts, update with a
//! mapping, family types in the type commands, components that evaluate and survive a save.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::path::PathBuf;
use std::sync::Arc;

use arc_command::roots::Roots;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::package::{LockOwner, Metadata, Mode, Package};
use arc_doc::{Document, DocumentConfig, LoadOptions, Snapshot};
use archbuilder_model::expr::{Evaluated, Stored};
use archbuilder_model::family::def::{Definition, ParamDef, Role, Scope};
use archbuilder_model::family::fixtures::{fm, table};
use archbuilder_model::family::loaded;
use archbuilder_model::spatial;
use archbuilder_model::types::{COMPONENT, FAMILY, FAMILY_TYPE};
use serde_json::{Value as Json, json};

static SEEDS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(77_000);

fn owner() -> LockOwner {
    LockOwner {
        user: "test".into(),
        host: "test".into(),
        application: "family tests".into(),
        time_unix_ms: 0,
    }
}

struct Env {
    document: Document,
    session: Session,
    registry: arc_command::Registry,
    dir: PathBuf,
}

impl Env {
    fn new(name: &str, project: bool) -> Self {
        // Unique per instance: `package` opens nested environments with repeated names, and tests
        // run in parallel, so a name-only path would let one test delete another's directory.
        static DIRS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let unique = DIRS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "archbuilder-fam7-{name}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
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
        let mut session = Session::new(
            Surface::Ui,
            [Permission::Read, Permission::Edit, Permission::FileRead],
        );
        session.roots = Roots::new([&dir], [&dir]).unwrap();
        let env = Self {
            document,
            session,
            registry: archbuilder_commands::registry().unwrap(),
            dir,
        };
        if project {
            env.run("archbuilder.project.new", json!({})).unwrap();
        }
        env
    }

    fn run(&self, command: &str, params: Json) -> arc_base::Result<Outcome> {
        let invocation = Invocation {
            seed: SEEDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            ..Invocation::new(command, params)
        };
        Invoker::new(&self.registry).invoke(&self.document, &self.session, invocation)
    }

    fn snapshot(&self) -> Snapshot {
        self.document.snapshot()
    }

    fn undo(&self) {
        assert!(self.document.undo().wait().unwrap().value);
    }

    fn count(&self, key: &str) -> usize {
        spatial::objects(&self.snapshot(), key).len()
    }

    /// A package of `defs` at the versions given, saved under the environment's directory.
    fn package(&self, file: &str, defs: &[(&Definition, &str)]) -> String {
        let env = Env::new(&format!("pkg-{file}"), false);
        env.run(
            "archbuilder.family.manifest",
            json!({"name": "pkg", "version": "1.0.0", "license": "Apache-2.0"}),
        )
        .unwrap();
        for (d, v) in defs {
            env.run(
                "archbuilder.family.publish",
                json!({"text": d.to_text().unwrap(), "version": v}),
            )
            .unwrap();
        }
        let path = self.dir.join(file);
        let mut package = Package::create(&path, &Metadata::default(), &owner()).unwrap();
        package
            .save_document(&env.document, &[], None, &mut |_| Ok(()))
            .unwrap();
        path.to_str().unwrap().to_owned()
    }

    fn load(&self, path: &str, extra: Json) -> arc_base::Result<Outcome> {
        let mut params = json!({"path": path});
        params
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        self.run("archbuilder.family.load", params)
    }
}

fn code<T>(r: arc_base::Result<T>) -> &'static str {
    match r {
        Err(e) => e.code(),
        Ok(_) => panic!("expected an error"),
    }
}

fn variant(mut d: Definition, f: impl FnOnce(&mut Definition)) -> Definition {
    f(&mut d);
    d
}

fn type_id(env: &Env, name: &str) -> String {
    loaded::family_types(&env.snapshot())
        .into_iter()
        .find(|t| t.name == name)
        .unwrap()
        .id
        .to_string()
}

fn volume(env: &Env, id: &str) -> f64 {
    let c = loaded::components(&env.snapshot())
        .into_iter()
        .find(|c| c.id.to_string() == id)
        .unwrap();
    loaded::place(&env.snapshot(), &c)
        .unwrap()
        .evaluation
        .volume
}

#[test]
fn a_preview_changes_nothing_and_a_load_is_one_undo_step() {
    let env = Env::new("load", true);
    let path = env.package("t.archbfam", &[(&table(), "1.0.0")]);
    let before = env.snapshot().revision();
    let report = env
        .run("archbuilder.family.load_preview", json!({"path": path}))
        .unwrap()
        .value;
    assert_eq!(report["preview"], true);
    assert_eq!(report["families"][0]["action"], "added");
    assert_eq!(report["types"].as_array().unwrap().len(), 2);
    assert_eq!(env.snapshot().revision(), before);
    assert_eq!(env.count(FAMILY), 0);
    env.load(&path, json!({})).unwrap();
    assert_eq!((env.count(FAMILY), env.count(FAMILY_TYPE)), (1, 2));
    assert_eq!(env.snapshot().revision(), before + 1, "one transaction");
    env.undo();
    assert_eq!((env.count(FAMILY), env.count(FAMILY_TYPE)), (0, 0));
    // The user picks types.
    env.load(&path, json!({"types": ["1800x900"]})).unwrap();
    assert_eq!(env.count(FAMILY_TYPE), 1);
    // Loading the same family and version again adds only what is missing.
    let again = env.load(&path, json!({})).unwrap().value;
    assert_eq!(again["families"][0]["action"], "unchanged");
    assert_eq!((env.count(FAMILY), env.count(FAMILY_TYPE)), (1, 2));
}

#[test]
fn type_name_conflicts_are_kept_replaced_or_renamed_each_in_one_step() {
    let env = Env::new("conflict", true);
    let path = env.package("t.archbfam", &[(&table(), "1.0.0")]);
    env.load(&path, json!({})).unwrap();
    let id = type_id(&env, "1400x800");
    env.run(
        "archbuilder.type.set",
        json!({"type": id, "values": {"Width": 2.0}}),
    )
    .unwrap();
    let width = |env: &Env, name: &str| {
        let t = loaded::family_types(&env.snapshot())
            .into_iter()
            .find(|t| t.name == name)
            .unwrap();
        match &t.values["Width"] {
            Stored::Literal(Evaluated::Number(v)) => *v,
            other => panic!("{other:?}"),
        }
    };
    let revision = env.snapshot().revision();
    let kept = env.load(&path, json!({"conflict": "keep"})).unwrap().value;
    assert_eq!(kept["types"][0]["action"], "kept");
    assert!((width(&env, "1400x800") - 2.0).abs() < 1e-12);
    for (conflict, action) in [("replace", "replaced"), ("rename", "renamed")] {
        let before = env.snapshot().revision();
        let report = env
            .load(&path, json!({"conflict": conflict, "types": ["1400x800"]}))
            .unwrap()
            .value;
        assert_eq!(report["types"][0]["action"], action);
        assert_eq!(
            env.snapshot().revision(),
            before + 1,
            "{conflict}: one step"
        );
        if conflict == "replace" {
            assert!((width(&env, "1400x800") - 1.4).abs() < 1e-12);
            env.undo();
            assert!((width(&env, "1400x800") - 2.0).abs() < 1e-12);
        } else {
            assert_eq!(env.count(FAMILY_TYPE), 3);
            assert!(
                loaded::family_types(&env.snapshot())
                    .iter()
                    .any(|t| t.name == "1400x800 2")
            );
            env.undo();
            assert_eq!(env.count(FAMILY_TYPE), 2);
        }
    }
    assert!(env.snapshot().revision() > revision);
}

#[test]
fn another_checksum_at_the_same_version_conflicts_and_may_load_as_a_new_family() {
    let env = Env::new("checksum", true);
    let original = env.package("a.archbfam", &[(&table(), "1.0.0")]);
    env.load(&original, json!({})).unwrap();
    let altered = variant(table(), |d| d.header.source = "elsewhere".into());
    let other = env.package("b.archbfam", &[(&altered, "1.0.0")]);
    let before = env.snapshot().revision();
    let preview = env
        .run("archbuilder.family.load_preview", json!({"path": other}))
        .unwrap()
        .value;
    assert_eq!(preview["families"][0]["action"], "conflict");
    assert_eq!(
        code(env.load(&other, json!({}))),
        "ab.family.family_conflict"
    );
    assert_eq!(env.snapshot().revision(), before, "nothing applied");
    env.load(&other, json!({"as_new": true})).unwrap();
    let families = loaded::families(&env.snapshot());
    assert_eq!(families.len(), 2);
    assert_ne!(families[0].family, families[1].family);
    let new = families
        .iter()
        .find(|f| f.family != altered.header.id)
        .unwrap();
    let text = new.definition().unwrap();
    assert!(
        text.header.source.starts_with("Table 1.0.0 "),
        "{}",
        text.header.source
    );
    assert_eq!(new.name, "Table 2", "named apart in the project");
    // Another version is pinned: it needs the update command.
    let newer = env.package("c.archbfam", &[(&table(), "1.1.0")]);
    assert_eq!(
        code(env.load(&newer, json!({}))),
        "ab.family.version_invalid"
    );
}

#[test]
fn a_component_evaluates_survives_save_and_reopen_and_refuses_a_failing_type() {
    let env = Env::new("place", true);
    env.load(
        &env.package("t.archbfam", &[(&table(), "1.0.0")]),
        json!({}),
    )
    .unwrap();
    let ty = type_id(&env, "1800x900");
    let before = env.snapshot().revision();
    let placed = env
        .run(
            "archbuilder.component.place",
            json!({"type": "1800x900", "point": [1.0, 2.0], "rotation": "90 deg", "values": {"LegSize": 0.08}, "name": "T1"}),
        )
        .unwrap()
        .value;
    let id = placed["component"].as_str().unwrap().to_owned();
    assert_eq!(env.snapshot().revision(), before + 1);
    let expect = 1.8 * 0.9 * 0.04 + 4.0 * 0.08 * 0.08 * 0.71;
    assert!((volume(&env, &id) - expect).abs() < 1e-9);
    let c = loaded::components(&env.snapshot()).remove(0);
    let placed = loaded::place(&env.snapshot(), &c).unwrap();
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for s in &placed.solids {
        let b = s.solid.bounding_box();
        for i in 0..3 {
            lo[i] = lo[i].min(b.min()[i]);
            hi[i] = hi[i].max(b.max()[i]);
        }
    }
    // Local x runs along +y and local y along -x after a quarter turn.
    assert!(
        (lo[0] - 0.1).abs() < 1e-9 && (hi[0] - 1.0).abs() < 1e-9,
        "{lo:?} {hi:?}"
    );
    assert!((lo[1] - 2.0).abs() < 1e-9 && (hi[1] - 3.8).abs() < 1e-9);
    // Mirrored across its own left-right axis, the component has the same volume.
    env.run(
        "archbuilder.component.place",
        json!({"type": ty, "point": [5.0, 5.0], "mirror": true}),
    )
    .unwrap();
    assert_eq!(env.count(COMPONENT), 2);
    env.undo();
    assert_eq!(env.count(COMPONENT), 1);
    // Save and reopen: the instance is the same.
    let file = env.dir.join("project.archb");
    let mut package = Package::create(&file, &Metadata::default(), &owner()).unwrap();
    package
        .save_document(&env.document, &[], None, &mut |_| Ok(()))
        .unwrap();
    drop(package);
    let reopened = Package::open(&file, Mode::ReadOnly, &arc_base::Limits::DEFAULT)
        .unwrap()
        .open_document(
            Arc::new(archbuilder_commands::types().unwrap()),
            Arc::new(Evaluators::new()),
            &LoadOptions::default(),
            DocumentConfig::default(),
        )
        .unwrap();
    let snapshot = reopened.snapshot();
    let comps = loaded::components(&snapshot);
    assert_eq!(comps.len(), 1);
    assert_eq!(comps[0].name, "T1");
    assert!(
        (loaded::place(&snapshot, &comps[0])
            .unwrap()
            .evaluation
            .volume
            - expect)
            .abs()
            < 1e-9
    );
    // A type that does not evaluate is refused at placement, naming the form; nothing changes.
    env.run(
        "archbuilder.type.set",
        json!({"type": ty, "values": {"Width": 0.0}}),
    )
    .unwrap_err();
    let fresh = type_id(&env, "1400x800");
    env.run(
        "archbuilder.type.set",
        json!({"type": fresh, "values": {"Width": 0.0}}),
    )
    .unwrap();
    let before = env.snapshot().revision();
    let e = env
        .run(
            "archbuilder.component.place",
            json!({"type": fresh, "point": [0.0, 0.0]}),
        )
        .unwrap_err();
    assert_eq!(e.code(), "ab.family.evaluation_failed");
    assert!(
        matches!(e.arguments().get("form"), Some(arc_base::error::Value::Text(f)) if f == "top")
    );
    assert_eq!(env.snapshot().revision(), before);
    assert_eq!(
        code(env.run(
            "archbuilder.component.place",
            json!({"type": "none", "point": [0, 0]})
        )),
        "ab.cmd.kind_mismatch"
    );
}

#[test]
fn the_type_commands_work_on_family_types() {
    let env = Env::new("types", true);
    env.load(
        &env.package("t.archbfam", &[(&table(), "1.0.0")]),
        json!({}),
    )
    .unwrap();
    let ty = type_id(&env, "1400x800");
    let copy = env
        .run("archbuilder.type.duplicate", json!({"type": ty}))
        .unwrap();
    assert_eq!(env.count(FAMILY_TYPE), 3, "{copy:?}");
    env.undo();
    // A type with components cannot be deleted.
    env.run(
        "archbuilder.component.place",
        json!({"type": ty, "point": [0, 0]}),
    )
    .unwrap();
    assert!(
        env.run("archbuilder.type.delete", json!({"type": ty}))
            .is_err()
    );
    let other = type_id(&env, "1800x900");
    env.run(
        "archbuilder.type.assign",
        json!({"type": other, "objects": [loaded::components(&env.snapshot())[0].id.to_string()]}),
    )
    .unwrap();
    assert_eq!(
        loaded::components(&env.snapshot())[0]
            .family_type
            .to_string(),
        other
    );
    env.run("archbuilder.type.delete", json!({"type": ty}))
        .unwrap();
    assert_eq!(env.count(FAMILY_TYPE), 1);
}

fn added(d: &mut Definition) {
    let mut p: ParamDef = d.parameters[0].clone();
    p.id = arc_base::id::Identifier::from_u128(0x99);
    p.key = "Extra".into();
    p.name = "Extra".into();
    p.scope = Scope::Instance;
    p.role = Role::Driving;
    d.parameters.push(p);
}

fn renamed(d: &mut Definition) {
    let text = d.to_text().unwrap().replace("LegSize", "Leg");
    *d = Definition::from_text(&text).unwrap();
}

#[test]
fn an_update_carries_values_by_identifier_and_is_one_undo_step() {
    let env = Env::new("update", true);
    env.load(
        &env.package("a.archbfam", &[(&table(), "1.0.0")]),
        json!({}),
    )
    .unwrap();
    let ty = type_id(&env, "1800x900");
    let id = env
        .run(
            "archbuilder.component.place",
            json!({"type": ty, "point": [0, 0], "values": {"LegSize": 0.08}}),
        )
        .unwrap()
        .value["component"]
        .as_str()
        .unwrap()
        .to_owned();
    // A minor update adds a parameter.
    let minor = env.package("b.archbfam", &[(&variant(table(), added), "1.1.0")]);
    let params = json!({"family": "Table", "path": minor});
    let preview = env
        .run("archbuilder.family.update_preview", params.clone())
        .unwrap()
        .value;
    assert_eq!(preview["major"], false);
    assert_eq!(preview["added"][0], "Extra");
    assert_eq!(preview["instances"], 1);
    let before = env.snapshot().revision();
    env.run("archbuilder.family.update", params).unwrap();
    assert_eq!(env.snapshot().revision(), before + 1);
    assert_eq!(
        loaded::families(&env.snapshot())[0].version.to_string(),
        "1.1.0"
    );
    env.undo();
    assert_eq!(
        loaded::families(&env.snapshot())[0].version.to_string(),
        "1.0.0"
    );
    // An older or equal version is not an update.
    let same = env.package("c.archbfam", &[(&table(), "1.0.0")]);
    assert_eq!(
        code(env.run(
            "archbuilder.family.update",
            json!({"family": "Table", "path": same})
        )),
        "ab.family.version_invalid"
    );
    // A major update renames an identifier: values follow by parameter identifier.
    let major = env.package("d.archbfam", &[(&variant(table(), renamed), "2.0.0")]);
    let report = env
        .run(
            "archbuilder.family.update",
            json!({"family": "Table", "path": major}),
        )
        .unwrap()
        .value;
    assert_eq!(report["major"], true);
    assert_eq!(report["renamed"][0], "LegSize -> Leg");
    let c = loaded::components(&env.snapshot()).remove(0);
    assert!(c.values.contains_key("Leg") && !c.values.contains_key("LegSize"));
    let expect = 1.8 * 0.9 * 0.04 + 4.0 * 0.08 * 0.08 * 0.71;
    assert!((volume(&env, &id) - expect).abs() < 1e-9);
    env.undo();
    assert!(
        loaded::components(&env.snapshot())[0]
            .values
            .contains_key("LegSize")
    );
}

#[test]
fn a_major_update_needs_a_mapping_for_removed_parameters_and_a_failing_instance_refuses() {
    let env = Env::new("major", true);
    env.load(
        &env.package("a.archbfam", &[(&table(), "1.0.0")]),
        json!({}),
    )
    .unwrap();
    let ty = type_id(&env, "1400x800");
    env.run(
        "archbuilder.component.place",
        json!({"type": ty, "point": [0, 0]}),
    )
    .unwrap();
    // The new version drops TopThickness and uses a constant.
    let dropped = variant(table(), |d| {
        d.parameters.retain(|p| p.key != "TopThickness");
        d.planes
            .iter_mut()
            .find(|p| p.key == "under")
            .unwrap()
            .offset = fm("Height - 0.05");
        d.tests.clear();
    });
    let path = env.package("b.archbfam", &[(&dropped, "2.0.0")]);
    let before = env.snapshot().revision();
    let preview = env
        .run(
            "archbuilder.family.update_preview",
            json!({"family": "Table", "path": path}),
        )
        .unwrap()
        .value;
    assert_eq!(preview["major"], true);
    assert_eq!(preview["removed"][0], "Top thickness");
    assert_eq!(preview["needs_mapping"][0], "TopThickness");
    assert_eq!(
        code(env.run(
            "archbuilder.family.update",
            json!({"family": "Table", "path": path})
        )),
        "ab.family.mapping_required"
    );
    assert_eq!(
        env.snapshot().revision(),
        before,
        "refused: nothing changed"
    );
    env.run(
        "archbuilder.family.update",
        json!({"family": "Table", "path": path, "mapping": {"TopThickness": null}}),
    )
    .unwrap();
    assert_eq!(env.snapshot().revision(), before + 1);
    env.undo();
    // A new version that leaves an instance unable to evaluate is refused, naming it.
    let broken = variant(table(), |d| {
        d.planes
            .iter_mut()
            .find(|p| p.key == "under")
            .unwrap()
            .offset = fm("Height - Missing");
        d.tests.clear();
    });
    let path = env.package("c.archbfam", &[(&broken, "1.0.1")]);
    let e = env
        .run(
            "archbuilder.family.update",
            json!({"family": "Table", "path": path}),
        )
        .unwrap_err();
    assert_eq!(e.code(), "ab.family.evaluation_failed");
    assert_eq!(
        env.snapshot().revision(),
        before + 2,
        "refused: the document is as the undone update left it"
    );
}

#[test]
fn hiding_the_components_category_hides_components_in_a_plan_view() {
    let env = Env::new("category", true);
    env.load(
        &env.package("k.archbfam", &[(&table(), "1.0.0")]),
        json!({}),
    )
    .unwrap();
    let id: arc_base::ObjectId = env
        .run(
            "archbuilder.component.place",
            json!({"type": "1800x900", "point": [1.0, 2.0]}),
        )
        .unwrap()
        .value["component"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let storey = {
        let s = env.snapshot();
        spatial::storeys(&s, spatial::buildings(&s)[0].id)[0]
            .storey
            .id
    };
    let view = || {
        let s = env.snapshot();
        archbuilder_model::view::of_storey(&s, storey)
            .into_iter()
            .next()
            .unwrap()
    };
    let drawn = || {
        let built = archbuilder_model::plan::build(
            &env.snapshot(),
            &view(),
            &archbuilder_model::plan::Filter::default(),
            &arc_base::Cancel::new(),
        )
        .unwrap();
        !built.primitives_of(id).is_empty()
    };
    assert!(drawn());
    let set = |value: bool| {
        env.run(
            "archbuilder.property.set",
            json!({"objects": [view().id.to_string()], "field": "show_components", "value": value}),
        )
        .unwrap();
    };
    set(false);
    assert!(!drawn());
    set(true);
    assert!(drawn());
    // A plan view saved before the flag existed shows components.
    let mut values = spatial::get(&env.snapshot(), view().id)
        .unwrap()
        .values
        .clone();
    values.truncate(archbuilder_model::types::plan_view::SHOW_COMPONENTS);
    let settings = archbuilder_model::view::PlanView::from_values(&values).unwrap();
    assert!(settings.shows(archbuilder_model::view::Category::Component));
}

#[test]
fn nested_families_place_with_derived_shared_ids_and_profile_families_are_not_placed() {
    use archbuilder_model::family::def::{
        Anchor, ArrayDef, ArrayKind, Category, NestedUse, Version,
    };
    use archbuilder_model::family::fixtures::{cabinet, len};
    let child = table();
    let mut parent = cabinet();
    parent.header.id = arc_base::id::Identifier::from_u128(0x7001);
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
    parent.arrays = vec![ArrayDef {
        key: "pair".into(),
        target: "tbl".into(),
        kind: ArrayKind::Linear,
        count: len(2.0),
        spacing: Some(len(2.0)),
        axis: archbuilder_model::family::def::Axis::X,
        angle: None,
        center: [None, None],
    }];
    parent.tests.clear();
    let mut profile = variant(table(), |d| {
        d.header.id = arc_base::id::Identifier::from_u128(0x7002);
        d.header.category = Category::Profile;
        d.header.name = "Strip".into();
    });
    profile.forms.clear();
    profile.tests.clear();
    profile.parameters.retain(|p| p.role != Role::Reporting);
    let env = Env::new("nested", true);
    let pkg = env.package(
        "n.archbfam",
        &[(&child, "1.0.0"), (&parent, "1.0.0"), (&profile, "1.0.0")],
    );
    env.load(&pkg, json!({})).unwrap();
    let place = |name: &str| {
        env.run(
            "archbuilder.component.place",
            json!({"type": name, "point": [1.0, 1.0]}),
        )
    };
    let id = place("Cabinet: 600").unwrap();
    let snapshot = env.snapshot();
    let component = loaded::components(&snapshot).remove(0);
    let first = loaded::place(&snapshot, &component).unwrap();
    assert_eq!(first.shared.len(), 2);
    assert!(first.shared.iter().all(|s| !s.solids.is_empty()));
    let again = loaded::place(&env.snapshot(), &component).unwrap();
    let ids = |p: &loaded::Placed| p.shared.iter().map(|s| s.id).collect::<Vec<_>>();
    assert_eq!(
        ids(&first),
        ids(&again),
        "identifiers are stable across regeneration"
    );
    assert_ne!(first.shared[0].id, first.shared[1].id);
    assert_ne!(first.shared[0].id, component.id);
    // The second copy is 2 m beyond the first.
    let x = |s: &loaded::PlacedShared| s.solids[0].solid.bounding_box().min()[0];
    assert!((x(&first.shared[1]) - x(&first.shared[0]) - 2.0).abs() < 1e-9);
    let _ = id;
    // A profile family is not placed as a component.
    assert_eq!(code(place("Strip: 1400x800")), "ab.family.invalid");
}

#[test]
fn a_hosted_door_cuts_its_wall_and_refuses_what_a_built_in_insert_refuses() {
    use archbuilder_model::family::fixtures::hosted_door;
    use archbuilder_model::types::{FAMILY_INSERT, WALL};
    let env = Env::new("hosted", true);
    env.load(
        &env.package(
            "h.archbfam",
            &[(&hosted_door(), "1.0.0"), (&table(), "1.0.0")],
        ),
        json!({}),
    )
    .unwrap();
    let wall = env
        .run(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [6.0, 0.0], "height": 3.0, "thickness": 0.2}),
        )
        .unwrap()
        .value["wall"]
        .as_str()
        .unwrap()
        .to_owned();
    let volume_of_wall = |env: &Env| {
        let s = env.snapshot();
        let o = spatial::objects(&s, WALL).remove(0);
        archbuilder_model::insert::wall_solid(&s, &o)
            .unwrap()
            .unwrap()
            .volume()
    };
    let before = volume_of_wall(&env);
    let place = |distance: f64, ty: &str| {
        env.run(
            "archbuilder.family_insert.place",
            json!({"host": wall, "type": ty, "distance": distance}),
        )
    };
    place(3.0, "Wall door: 0900x2100").unwrap();
    assert_eq!(env.count(FAMILY_INSERT), 1);
    // Host volume net of the void: the opening is 0.9 x 2.1 through the 0.2 thick wall.
    assert!((volume_of_wall(&env) - (before - 0.9 * 2.1 * 0.2)).abs() < 1e-9);
    // The refusals of built-in inserts: too close to the end, overlapping another opening.
    assert_eq!(
        code(place(0.0, "Wall door: 0900x2100")),
        "ab.model.insert_outside"
    );
    assert_eq!(
        code(place(3.5, "Wall door: 1000x2100")),
        "ab.model.insert_overlap"
    );
    // A family that is not hosted, and a host of the wrong kind.
    assert_eq!(code(place(1.0, "Table: 1400x800")), "ab.family.invalid");
    let storey = spatial::objects(&env.snapshot(), archbuilder_model::types::STOREY)[0].id;
    let wrong = env.run(
        "archbuilder.family_insert.place",
        json!({"host": storey.to_string(), "type": "Wall door: 0900x2100", "distance": 1.0}),
    );
    assert_eq!(code(wrong), "ab.family.invalid");
    assert_eq!(env.count(FAMILY_INSERT), 1);
    env.undo();
    assert_eq!(env.count(FAMILY_INSERT), 0);
    assert!(
        (volume_of_wall(&env) - before).abs() < 1e-9,
        "one undo restores the wall"
    );
}

#[test]
fn a_hosted_instance_slides_along_its_wall_flips_and_copies_and_undoes_in_one_step() {
    use arc_base::ObjectId;
    use archbuilder_model::family::fixtures::hosted_door;
    use archbuilder_model::family::loaded::FamilyInsert;
    use archbuilder_model::types::{FAMILY_INSERT, WALL};
    let env = Env::new("hosted-slide", true);
    env.load(
        &env.package("h.archbfam", &[(&hosted_door(), "1.0.0")]),
        json!({}),
    )
    .unwrap();
    let wall = env
        .run(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [6.0, 0.0], "height": 3.0, "thickness": 0.2}),
        )
        .unwrap()
        .value["wall"]
        .as_str()
        .unwrap()
        .to_owned();
    let placed = env
        .run(
            "archbuilder.family_insert.place",
            json!({"host": wall, "type": "Wall door: 0900x2100", "distance": 3.0}),
        )
        .unwrap()
        .value["insert"]
        .as_str()
        .unwrap()
        .to_owned();
    let id: ObjectId = placed.parse().unwrap();
    let read = |env: &Env| {
        let s = env.snapshot();
        let object = spatial::get(&s, id).unwrap();
        FamilyInsert::of(&object).unwrap()
    };
    let on_selection = |env: &Env, command: &str, params: Json| {
        let invocation = Invocation {
            seed: SEEDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            selection: vec![id],
            ..Invocation::new(command, params)
        };
        Invoker::new(&env.registry).invoke(&env.document, &env.session, invocation)
    };
    // Moved along the wall: the displacement is projected onto the baseline.
    on_selection(
        &env,
        "archbuilder.modify.move",
        json!({"displacement": [1.0, 0.5]}),
    )
    .unwrap();
    assert!((read(&env).distance - 4.0).abs() < 1e-9);
    assert_eq!(
        read(&env).host,
        Some(spatial::objects(&env.snapshot(), WALL)[0].id)
    );
    env.undo();
    assert!((read(&env).distance - 3.0).abs() < 1e-9);
    // Flipping the hand and the facing: one undo step each.
    env.run(
        "archbuilder.insert.flip_hand",
        json!({"inserts": [id.to_string()]}),
    )
    .unwrap();
    assert_eq!(read(&env).hand, archbuilder_model::insert::Hand::Right);
    env.run(
        "archbuilder.insert.flip_facing",
        json!({"inserts": [id.to_string()]}),
    )
    .unwrap();
    assert_eq!(
        read(&env).facing,
        archbuilder_model::insert::Facing::Interior
    );
    env.undo();
    env.undo();
    assert_eq!(read(&env).hand, archbuilder_model::insert::Hand::Left);
    // A copy keeps the mark and the family type, at the distance the copy gives.
    let made = on_selection(
        &env,
        "archbuilder.modify.copy",
        json!({"displacement": [2.0, 0.0]}),
    )
    .unwrap();
    assert_eq!(env.count(FAMILY_INSERT), 2);
    let copy: ObjectId = made.value["created"][0].as_str().unwrap().parse().unwrap();
    let copied = FamilyInsert::of(&spatial::get(&env.snapshot(), copy).unwrap()).unwrap();
    assert_eq!(copied.family_type, read(&env).family_type);
    assert!((copied.distance - 5.0).abs() < 1e-9);
}

#[test]
fn a_placed_instance_takes_new_values_and_placement_in_one_undo_step() {
    use arc_base::ObjectId;
    use archbuilder_model::family::loaded::Component;
    let env = Env::new("instance-set", true);
    env.load(
        &env.package("t.archbfam", &[(&table(), "1.0.0")]),
        json!({}),
    )
    .unwrap();
    let id = env
        .run(
            "archbuilder.component.place",
            json!({"type": "1800x900", "point": [1.0, 2.0], "values": {"LegSize": 0.08}}),
        )
        .unwrap()
        .value["component"]
        .as_str()
        .unwrap()
        .to_owned();
    let id: ObjectId = id.parse().unwrap();
    let read = |env: &Env| {
        let s = env.snapshot();
        Component::of(&spatial::get(&s, id).unwrap()).unwrap()
    };
    // A value and a placement change together, as one command.
    env.run(
        "archbuilder.instance.set",
        json!({"object": id.to_string(), "point": [3.0, 4.0], "name": "T2",
               "values": {"LegSize": 0.05}}),
    )
    .unwrap();
    let c = read(&env);
    assert_eq!((c.origin, c.name.as_str()), ([3.0, 4.0], "T2"));
    assert_eq!(c.values.len(), 1);
    assert!(
        (volume(&env, &id.to_string()) - (1.8 * 0.9 * 0.04 + 4.0 * 0.05 * 0.05 * 0.71)).abs()
            < 1e-9
    );
    // The property grid's edit of one value, then the value cleared back to the type's.
    env.run(
        "archbuilder.property.set",
        json!({"objects": [id.to_string()], "field": "value.LegSize", "value": 0.06}),
    )
    .unwrap();
    assert!(
        (volume(&env, &id.to_string()) - (1.8 * 0.9 * 0.04 + 4.0 * 0.06 * 0.06 * 0.71)).abs()
            < 1e-9
    );
    env.run(
        "archbuilder.property.set",
        json!({"objects": [id.to_string()], "field": "value.LegSize", "value": ""}),
    )
    .unwrap();
    assert!(read(&env).values.is_empty());
    // A value that is not instance-scoped, or a key the family does not have, is refused.
    assert!(
        env.run(
            "archbuilder.instance.set",
            json!({"object": id.to_string(), "values": {"Nope": 1.0}}),
        )
        .is_err()
    );
    // Each command is one undo step: three steps back to the placement.
    env.undo();
    env.undo();
    env.undo();
    let c = read(&env);
    assert_eq!((c.origin, c.name.as_str()), ([1.0, 2.0], ""));
    assert!(c.values.contains_key("LegSize"));
}
