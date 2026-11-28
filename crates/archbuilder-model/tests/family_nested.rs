//! Family arrays, lookup tables, nested uses and profile families (ADR 0009, units 10 and 11):
//! counts at the limits, nesting cycle and depth, derived identifiers, family-type parameters and
//! the canonical text round trip of every new part.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use arc_base::ObjectId;
use arc_base::id::Identifier;
use archbuilder_model::expr::{DataType, Evaluated, Stored};
use archbuilder_model::family::def::{
    Anchor, ArrayDef, ArrayKind, Axis, Category, Definition, NestedUse, ParamDef, ProfileDef, Role,
    Scope, Version, limits,
};
use archbuilder_model::family::fixtures::{cabinet, fm, len, table, vx};
use archbuilder_model::family::table::{ColumnDef, TableDef};
use archbuilder_model::family::{Evaluation, Inputs, evaluate};

const V1: Version = Version {
    major: 1,
    minor: 0,
    patch: 0,
};

type Library = HashMap<ObjectId, Arc<Definition>>;

fn run(def: &Definition, library: &Library, ty: Option<&str>) -> arc_base::Result<Evaluation> {
    let instance = BTreeMap::new();
    let children = |id: ObjectId, _v: Version| library.get(&id).cloned();
    evaluate(
        def,
        &Inputs {
            type_name: ty,
            instance: &instance,
            variables: &|_| None,
            children: &children,
        },
    )
}

fn code<T>(r: arc_base::Result<T>) -> &'static str {
    match r {
        Err(e) => e.code(),
        Ok(_) => panic!("expected an error"),
    }
}

fn array(key: &str, target: &str, kind: ArrayKind, count: Stored) -> ArrayDef {
    ArrayDef {
        key: key.into(),
        target: target.into(),
        kind,
        count,
        spacing: Some(len(1.0)),
        axis: Axis::X,
        angle: None,
        center: [None, None],
    }
}

fn use_of(key: &str, family: &Definition, shared: bool) -> NestedUse {
    NestedUse {
        key: key.into(),
        family: family.header.id,
        version: V1,
        type_name: None,
        type_param: None,
        shared,
        at: [Anchor::default(), Anchor::default(), Anchor::default()],
        rotation: None,
        params: BTreeMap::new(),
    }
}

fn with_id(mut d: Definition, n: u128) -> Definition {
    d.header.id = ObjectId::from_u128(n);
    d.tests.clear();
    d
}

fn solo() -> Definition {
    // The cabinet without its void: one solid box of 0.6 x 0.6 x 0.9.
    let mut d = with_id(cabinet(), 0x900);
    d.forms.retain(|f| f.key == "carcass");
    d.profiles.retain(|p| p.key == "body_outline");
    d
}

fn volume_of_one() -> f64 {
    0.6 * 0.6 * 0.9
}

fn round_trips(d: &Definition) {
    let text = d.to_text().unwrap();
    let back = Definition::from_text(&text).unwrap();
    assert_eq!(back.to_text().unwrap(), text);
}

#[test]
fn linear_and_radial_arrays_repeat_a_form() {
    let mut d = solo();
    d.arrays = vec![array("row", "carcass", ArrayKind::Linear, len(3.0))];
    let ev = run(&d, &Library::new(), None).unwrap();
    assert_eq!(ev.solids.len(), 3);
    assert!((ev.volume - 3.0 * volume_of_one()).abs() < 1e-9);
    let keys: Vec<&str> = ev.solids.iter().map(|s| s.form.as_str()).collect();
    assert!(keys.contains(&"carcass") && keys.contains(&"carcass#2"));
    // The third copy is 2 m to the right.
    let far = ev.solids.iter().find(|s| s.form == "carcass#2").unwrap();
    assert!((far.solid.bounding_box().min()[0] - 2.0).abs() < 1e-9);
    round_trips(&d);

    let mut r = array("ring", "carcass", ArrayKind::Radial, len(4.0));
    r.spacing = None;
    r.center = [Some(len(3.0)), Some(len(0.0))];
    d.arrays = vec![r];
    let ev = run(&d, &Library::new(), None).unwrap();
    assert_eq!(ev.solids.len(), 4);
    assert!((ev.volume - 4.0 * volume_of_one()).abs() < 1e-6);
    // A quarter turn about (3, 0) moves the box at the origin to the other side of the center.
    let turned = ev.solids.iter().find(|s| s.form == "carcass#2").unwrap();
    assert!(turned.solid.bounding_box().min()[0] > 5.0);
    round_trips(&d);
}

#[test]
fn array_counts_hold_at_the_limits() {
    let mut d = solo();
    d.arrays = vec![array("row", "carcass", ArrayKind::Linear, len(1.0))];
    assert_eq!(run(&d, &Library::new(), None).unwrap().solids.len(), 1);
    // The most placed children is allowed; one more is refused, as is a count of zero.
    let mut d = with_id(solo(), 0x901);
    d.arrays = vec![array(
        "row",
        "carcass",
        ArrayKind::Linear,
        len(limits::PLACED_CHILDREN as f64),
    )];
    d.validate().unwrap();
    d.arrays[0].spacing = Some(len(0.7));
    assert_eq!(
        run(&d, &Library::new(), None).unwrap().solids.len(),
        limits::PLACED_CHILDREN
    );
    d.arrays[0].count = len(limits::PLACED_CHILDREN as f64 + 1.0);
    assert_eq!(code(d.validate()), "ab.family.limit_exceeded");
    d.arrays[0].count = fm("10000 + 1");
    d.arrays[0].count =
        Stored::Formula(archbuilder_model::expr::Formula::new("10000 + 1", None).unwrap());
    assert_eq!(
        code(run(&d, &Library::new(), None)),
        "ab.family.limit_exceeded"
    );
    d.arrays[0].count = len(0.0);
    assert_eq!(code(d.validate()), "ab.family.limit_exceeded");
}

#[test]
fn nested_children_join_the_body_or_stay_shared_with_derived_ids() {
    let child = Arc::new(table());
    let library: Library = HashMap::from([(child.header.id, Arc::clone(&child))]);
    let mut parent = with_id(solo(), 0x902);
    let mut inner = use_of("tbl", &child, false);
    inner.at[0] = Anchor {
        plane: None,
        offset: Some(len(2.0)),
    };
    inner.type_name = Some("1400x800".into());
    inner.params.insert("LegSize".into(), fm("0.05"));
    parent.nested = vec![inner];
    parent.validate().unwrap();
    let alone = run(&with_id(solo(), 0x903), &Library::new(), None)
        .unwrap()
        .volume;
    let ev = run(&parent, &library, None).unwrap();
    // The child's solids are part of the body, moved 2 m; its leg size comes from the parent.
    let leg = 0.05 * 0.05 * (0.75 - 0.04);
    let top = 1.4 * 0.8 * 0.04;
    assert!((ev.volume - (alone + top + 4.0 * leg)).abs() < 1e-9);
    assert!(ev.shared.is_empty());
    assert!(ev.solids.iter().any(|s| s.form.starts_with("tbl/")));
    round_trips(&parent);

    // Shared: its own element, two copies of an array, ids from parent and key alone.
    parent.nested[0].shared = true;
    parent.arrays = vec![array("pair", "tbl", ArrayKind::Linear, len(2.0))];
    let ev = run(&parent, &library, None).unwrap();
    assert!(
        (ev.volume - alone).abs() < 1e-9,
        "shared children are not in the body"
    );
    let keys: Vec<&str> = ev.shared.iter().map(|s| s.key.as_str()).collect();
    assert_eq!(keys, ["tbl#0", "tbl#1"]);
    assert!((ev.shared[1].matrix[0][3] - 3.0).abs() < 1e-12);
    let parent_id = ObjectId::from_u128(0xAB);
    let id = |k: &str| archbuilder_model::family::nested_id(parent_id, k);
    assert_eq!(id("tbl#0"), id("tbl#0"));
    assert_ne!(id("tbl#0"), id("tbl#1"));
    assert_ne!(id("tbl#0"), parent_id.as_u128());
    assert_eq!((id("tbl#0") >> 76) & 0xF, 8, "a version 8 identifier");
    round_trips(&parent);
}

#[test]
fn a_family_type_parameter_swaps_the_nested_type() {
    let child = Arc::new(table());
    let library: Library = HashMap::from([(child.header.id, Arc::clone(&child))]);
    let mut parent = with_id(solo(), 0x904);
    parent.parameters.push(ParamDef {
        id: ObjectId::from_u128(0x9040),
        key: "Top".into(),
        name: "Top".into(),
        data: DataType::FamilyType,
        scope: Scope::Type,
        role: Role::Driving,
        group: String::new(),
        order: 0,
        default: Some(Stored::Literal(Evaluated::Text("1400x800".into()))),
        minimum: None,
        maximum: None,
        allowed: Vec::new(),
    });
    let mut inner = use_of("tbl", &child, false);
    inner.type_param = Some("Top".into());
    parent.nested = vec![inner];
    parent.validate().unwrap();
    let small = run(&parent, &library, None).unwrap().volume;
    parent.parameters.last_mut().unwrap().default =
        Some(Stored::Literal(Evaluated::Text("1800x900".into())));
    let big = run(&parent, &library, None).unwrap().volume;
    assert!(
        big > small + 0.01,
        "another type, other geometry without new geometry"
    );
    parent.parameters.last_mut().unwrap().default =
        Some(Stored::Literal(Evaluated::Text("nope".into())));
    assert_eq!(code(run(&parent, &library, None)), "ab.family.type_unknown");
    // A type parameter must be of the family-type kind.
    parent.nested[0].type_param = Some("Missing".into());
    assert_eq!(code(parent.validate()), "ab.family.invalid");
}

#[test]
fn nesting_cycles_and_depth_are_refused() {
    // a -> b -> a
    let mut a = with_id(solo(), 0xA01);
    let mut b = with_id(solo(), 0xA02);
    a.nested = vec![use_of("b", &b, false)];
    b.nested = vec![use_of("a", &a, false)];
    let library: Library = HashMap::from([
        (a.header.id, Arc::new(a.clone())),
        (b.header.id, Arc::new(b.clone())),
    ]);
    assert_eq!(code(run(&a, &library, None)), "ab.family.invalid");
    // A family that nests itself is invalid on its own.
    let mut me = with_id(solo(), 0xA03);
    me.nested = vec![use_of("me", &me.clone(), false)];
    assert_eq!(code(me.validate()), "ab.family.invalid");
    // A chain of eight uses (nine families) is the limit; one more is refused.
    let chain = |n: usize| -> (Definition, Library) {
        let mut defs: Vec<Definition> = (0..=n)
            .map(|i| with_id(solo(), 0xB00 + i as u128))
            .collect();
        for i in (0..n).rev() {
            let next = defs[i + 1].clone();
            defs[i].nested = vec![use_of("c", &next, false)];
        }
        let lib = defs
            .iter()
            .map(|d| (d.header.id, Arc::new(d.clone())))
            .collect();
        (defs.remove(0), lib)
    };
    let (root, lib) = chain(limits::NESTING_DEPTH - 1);
    assert!(run(&root, &lib, None).is_ok());
    let (root, lib) = chain(limits::NESTING_DEPTH);
    assert_eq!(code(run(&root, &lib, None)), "ab.family.limit_exceeded");
}

#[test]
fn lookup_drives_geometry_and_needs_function_set_1_1() {
    let mut d = with_id(solo(), 0xC01);
    d.tables = vec![TableDef {
        name: "widths".into(),
        keys: 1,
        columns: vec![
            ColumnDef {
                name: "code".into(),
                data: DataType::Text,
            },
            ColumnDef {
                name: "w".into(),
                data: DataType::Length,
            },
        ],
        rows: vec![
            vec![Evaluated::Text("A".into()), Evaluated::Number(0.5)],
            vec![Evaluated::Text("B".into()), Evaluated::Number(0.8)],
        ],
    }];
    for p in &mut d.parameters {
        if p.key == "Width" {
            p.default = Some(fm("lookup(\"widths\", \"w\", \"B\", 0.1 m)"));
        }
    }
    // Declared as function set 1.0 the definition may not call lookup.
    d.header.functions = (1, 0);
    assert_eq!(code(d.validate()), "ab.family.invalid");
    d.header.functions = (1, 1);
    d.validate().unwrap();
    let ev = run(&d, &Library::new(), None).unwrap();
    assert!((ev.volume - 0.8 * 0.6 * 0.9).abs() < 1e-9);
    round_trips(&d);
    // The fallback applies when no row matches.
    for p in &mut d.parameters {
        if p.key == "Width" {
            p.default = Some(fm("lookup(\"widths\", \"w\", \"Z\", 0.3 m)"));
        }
    }
    let ev = run(&d, &Library::new(), None).unwrap();
    assert!((ev.volume - 0.3 * 0.6 * 0.9).abs() < 1e-9);
    // A newer function set than this build has is refused on load.
    d.header.functions = (1, 2);
    assert_eq!(code(d.validate()), "ab.family.functions_newer");
}

#[test]
fn a_profile_family_evaluates_open_and_closed_profiles() {
    let mut d = with_id(solo(), 0xD01);
    d.header.category = Category::Profile;
    d.forms.clear();
    d.arrays.clear();
    d.profiles = vec![
        ProfileDef {
            key: "strip".into(),
            axis: Axis::Y,
            open: true,
            vertices: vec![
                vx("origin_z", None, "left", None),
                vx("top", None, "left", None),
                vx("top", None, "right", None),
            ],
        },
        ProfileDef {
            key: "frame".into(),
            axis: Axis::Y,
            open: false,
            vertices: vec![
                vx("origin_z", None, "left", None),
                vx("top", None, "left", None),
                vx("top", None, "right", None),
            ],
        },
    ];
    d.validate().unwrap();
    let ev = run(&d, &Library::new(), None).unwrap();
    assert_eq!(ev.profiles.len(), 2);
    assert!(ev.profiles.iter().any(|p| p.open && p.points.len() == 3));
    assert!(ev.solids.is_empty());
    round_trips(&d);
    // An open profile cannot be extruded.
    let mut e = solo();
    e.profiles[0].open = true;
    assert_eq!(code(e.validate()), "ab.family.invalid");
}
