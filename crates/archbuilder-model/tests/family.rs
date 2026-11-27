//! Family definitions (ADR 0009, units 4 and 5): canonical text round trip, hostile input, and the
//! evaluator on the table, door and cabinet fixtures with volumes computed by hand.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::collections::BTreeMap;

use archbuilder_model::family::def::{Detail, ViewKind, limits};
use archbuilder_model::family::fixtures::{cabinet, door, fm, len, table};
use archbuilder_model::family::{Definition, checksum};

fn fixtures() -> [(&'static str, Definition); 3] {
    [("table", table()), ("door", door()), ("cabinet", cabinet())]
}

fn code<T>(r: arc_base::Result<T>) -> &'static str {
    match r {
        Err(e) => e.code(),
        Ok(_) => panic!("expected an error"),
    }
}

#[test]
fn fixtures_validate_and_their_tests_pass() {
    for (name, d) in fixtures() {
        d.validate().unwrap_or_else(|e| panic!("{name}: {e:?}"));
        d.run_tests().unwrap_or_else(|e| panic!("{name}: {e:?}"));
    }
}

#[test]
fn canonical_text_round_trips_byte_identically_and_matches_the_golden_files() {
    for (name, d) in fixtures() {
        let text = d.to_text().unwrap();
        let back = Definition::from_text(&text).unwrap();
        assert_eq!(back.to_text().unwrap(), text, "{name}");
        assert_eq!(checksum(&text), back.checksum().unwrap());
        let path = format!(
            "{}/tests/fixtures/families/{name}.txt",
            env!("CARGO_MANIFEST_DIR")
        );
        if std::env::var_os("ARCHB_WRITE_GOLDEN").is_some() {
            std::fs::write(&path, &text).unwrap();
        }
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            text,
            "{name}: golden file"
        );
    }
}

#[test]
fn hostile_text_is_refused_within_the_limits() {
    let good = table().to_text().unwrap();
    // Too large, too many lines, a line too long, bad escapes, unknown kinds and fields.
    assert_eq!(
        code(Definition::from_text(&"x".repeat(limits::TEXT_BYTES + 1))),
        "ab.family.limit_exceeded"
    );
    let lines = format!("{good}{}", "vertex\n".repeat(limits::LINES));
    assert_eq!(
        code(Definition::from_text(&lines)),
        "ab.family.limit_exceeded"
    );
    let long = good.replacen(
        "source=\"ArchBuilder fixtures\"",
        &format!("source=\"{}\"", "a".repeat(limits::LINE_BYTES)),
        1,
    );
    assert_eq!(
        code(Definition::from_text(&long)),
        "ab.family.limit_exceeded"
    );
    for bad in [
        String::new(),
        "archbfam-text 2 functions 1.0\n".to_owned(),
        good.replacen("category=furniture", "category=nothing", 1),
        good.replacen("name=\"Table\"", "name=\"Ta\\qble\"", 1),
        good.replacen("name=\"Table\"", "name=\"Table", 1),
        format!("{good}unknown a=1\n"),
        good.replacen(" strong=true", " strong=true extra=1", 1),
        good.replacen("offset_f=\"Width\"", "offset_f=\"Width +\"", 1),
        good.replace("functions 1.0", "functions 9.0"),
    ] {
        assert!(Definition::from_text(&bad).is_err(), "{bad}");
    }
    // A formula past the formula limits.
    let deep = format!("{}1{}", "(".repeat(64), ")".repeat(64));
    let text = good.replacen("offset_f=\"Width\"", &format!("offset_f=\"{deep}\""), 1);
    assert!(Definition::from_text(&text).is_err());
    // A function outside the version 1 set.
    let text = good.replacen("offset_f=\"Width\"", "offset_f=\"exp(Width)\"", 1);
    assert!(Definition::from_text(&text).is_err());
}

#[test]
fn validation_refuses_what_cannot_evaluate() {
    let mut d = table();
    d.forms[0].profile = "missing".into();
    assert_eq!(code(d.validate()), "ab.family.invalid");
    let mut d = table();
    d.planes[1].axis = archbuilder_model::family::def::Axis::Y;
    assert_eq!(code(d.validate()), "ab.family.invalid");
    // A driving value may not read a reporting parameter.
    let mut d = table();
    d.planes[0].offset = fm("Volume");
    assert_eq!(code(d.validate()), "ab.family.invalid");
    // A type formula may not read an instance parameter.
    let mut d = table();
    d.parameters[0].default = Some(fm("LegSize"));
    assert_eq!(code(d.validate()), "ab.family.invalid");
    // A cycle among parameters is named.
    let mut d = table();
    d.parameters[0].default = Some(fm("Depth"));
    d.parameters[1].default = Some(fm("Width"));
    assert_eq!(code(d.validate()), "ab.model.param_cycle");
    // Too many parameters.
    let mut d = table();
    let one = d.parameters[0].clone();
    for i in 0..=limits::PARAMETERS {
        let mut p = one.clone();
        p.key = format!("p{i}");
        d.parameters.push(p);
    }
    assert_eq!(code(d.validate()), "ab.family.limit_exceeded");
}

fn vol(d: &Definition, ty: &str, set: &[(&str, f64)]) -> f64 {
    let set: BTreeMap<_, _> = set
        .iter()
        .map(|(k, v)| ((*k).to_owned(), len(*v)))
        .collect();
    d.evaluate_with(Some(ty), &set).unwrap().volume
}

#[test]
fn volumes_match_hand_computed_values_for_every_type() {
    let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
    let t = table();
    assert!(close(
        vol(&t, "1400x800", &[]),
        1.4 * 0.8 * 0.04 + 4.0 * 0.06 * 0.06 * 0.71
    ));
    assert!(close(
        vol(&t, "1800x900", &[("LegSize", 0.08)]),
        1.8 * 0.9 * 0.04 + 4.0 * 0.08 * 0.08 * 0.71
    ));
    let d = door();
    let frame = 0.1 * (0.9 * 2.1 - 0.8 * 2.05);
    assert!(close(vol(&d, "0900x2100", &[]), frame + 0.8 * 2.05 * 0.04));
    let c = cabinet();
    for (ty, w) in [("600", 0.6), ("800", 0.8), ("1200", 1.2)] {
        assert!(
            close(vol(&c, ty, &[]), w * 0.6 * 0.9 - 0.8 * w * 0.6 * 0.1),
            "{ty}"
        );
    }
}

#[test]
fn reporting_values_read_the_evaluated_geometry() {
    let ev = cabinet().evaluate(Some("800"), &BTreeMap::new()).unwrap();
    let archbuilder_model::expr::Evaluated::Number(v) = ev.values["Volume"] else {
        panic!("not a number")
    };
    assert!((v - ev.volume).abs() < 1e-12);
    assert!((ev.size[0] - 0.8).abs() < 1e-9 && (ev.size[2] - 0.9).abs() < 1e-9);
}

#[test]
fn failures_name_the_form_and_edge_cases_are_refused() {
    let named = |d: &Definition, ty: &str, key: &str, value: f64, form: &str| {
        let set = BTreeMap::from([(key.to_owned(), len(value))]);
        let e = d.evaluate_with(Some(ty), &set).unwrap_err();
        assert!(
            matches!(e.arguments().get("form"), Some(arc_base::error::Value::Text(f)) if f == form),
            "{key}={value}: {e:?}"
        );
    };
    named(&table(), "1400x800", "Width", 0.0, "top");
    named(&door(), "0900x2100", "FrameWidth", 0.5, "frame");
    named(&cabinet(), "600", "Width", 0.0, "carcass");
    // A void that does not pass through the body is named too.
    let mut c = cabinet();
    c.planes
        .iter_mut()
        .find(|p| p.key == "through_back")
        .unwrap()
        .offset = fm("Depth / 2");
    let e = c.evaluate(Some("600"), &BTreeMap::new()).unwrap_err();
    assert!(
        matches!(e.arguments().get("form"), Some(arc_base::error::Value::Text(f)) if f == "notch")
    );
    // An unknown type.
    assert_eq!(
        code(table().evaluate(Some("none"), &BTreeMap::new())),
        "ab.family.type_unknown"
    );
}

#[test]
fn visibility_symbols_and_face_roles() {
    let d = door();
    let ev = d.evaluate(Some("0900x2100"), &BTreeMap::new()).unwrap();
    let swing = ev.symbols.iter().find(|s| s.key == "swing").unwrap();
    assert!(swing.shown.at(Detail::High, ViewKind::Plan));
    assert!(!swing.shown.at(Detail::Low, ViewKind::Plan));
    assert!(!swing.shown.at(Detail::High, ViewKind::Front));
    // The arc is faceted: more than its two ends, and every point is the leaf's length from the hinge.
    assert!(swing.points.len() > 4);
    // The condition: the swing hides when ShowSwing is false.
    let off = BTreeMap::from([(
        "ShowSwing".to_owned(),
        archbuilder_model::expr::Stored::Literal(archbuilder_model::expr::Evaluated::Bool(false)),
    )]);
    let ev = d.evaluate(Some("0900x2100"), &off).unwrap();
    assert!(
        !ev.symbols
            .iter()
            .find(|s| s.key == "swing")
            .unwrap()
            .shown
            .at(Detail::High, ViewKind::Plan)
    );
    // Roles are stable names of the form and its face.
    let t = table().evaluate(None, &BTreeMap::new()).unwrap();
    let top = t.solids.iter().find(|s| s.form == "top").unwrap();
    assert!(top.roles.contains(&"family.form[top].end".to_owned()));
    assert!(top.roles.contains(&"family.form[top].start".to_owned()));
    assert!(
        top.roles
            .contains(&"family.form[top].side.0.0.0".to_owned())
    );
    let c = cabinet().evaluate(None, &BTreeMap::new()).unwrap();
    assert!(
        c.solids[0]
            .roles
            .iter()
            .any(|r| r.starts_with("family.form[carcass].void[notch]."))
    );
}

#[test]
fn the_evaluation_budget_bounds_a_hostile_definition() {
    let mut d = table();
    let big = format!("{}0", "1 + ".repeat(500));
    for i in 0..600 {
        let mut p = d.parameters[0].clone();
        p.id = arc_base::id::Identifier::from_u128(10_000 + i);
        p.key = format!("q{i}");
        p.default = Some(fm(&big));
        d.parameters.push(p);
    }
    let e = d.evaluate(None, &BTreeMap::new()).unwrap_err();
    assert_eq!(e.code(), "ab.family.limit_exceeded");
}

#[test]
fn publishing_checks_the_version_step_and_runs_the_tests() {
    use archbuilder_model::family::Version;
    use archbuilder_model::family::package::publish;
    let v = |a, b, c| Version {
        major: a,
        minor: b,
        patch: c,
    };
    let old = publish(&table(), None, v(1, 0, 0)).unwrap();
    let old = old.definition().unwrap();
    // The same version, or an older one, is not newer.
    assert_eq!(
        code(publish(&table(), Some(&old), v(1, 0, 0))),
        "ab.family.version_invalid"
    );
    // A parameter added needs a minor step.
    let mut added = table();
    let mut p = added.parameters[0].clone();
    p.id = arc_base::id::Identifier::from_u128(0x99);
    p.key = "Extra".into();
    p.name = "Extra".into();
    added.parameters.push(p);
    assert_eq!(
        code(publish(&added, Some(&old), v(1, 0, 1))),
        "ab.family.version_invalid"
    );
    assert!(publish(&added, Some(&old), v(1, 1, 0)).is_ok());
    // A parameter removed needs a major step.
    let mut removed = table();
    removed.parameters.retain(|p| p.key != "LegSize");
    removed.tests.clear();
    removed.forms.clear();
    removed.profiles.clear();
    assert_eq!(
        code(publish(&removed, Some(&old), v(1, 5, 0))),
        "ab.family.version_invalid"
    );
    // A failing test case refuses the publication.
    let mut failing = table();
    failing.tests[0].volume = Some(1.0);
    assert_eq!(
        code(publish(&failing, None, v(1, 0, 0))),
        "ab.family.test_failed"
    );
}
