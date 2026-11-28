//! Family packages (ADR 0009, unit 6): `family publish` and `family check`.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::ffi::OsString;
use std::sync::Arc;

use arc_command::FixedClock;
use arc_command::headless::Exit;
use arc_doc::package::{LockOwner, Mode, Package};
use archbuilder_model::family::def::Version;
use archbuilder_model::family::fixtures::{cabinet, door, table};
use serde_json::Value;

fn owner() -> LockOwner {
    LockOwner {
        user: "test".into(),
        host: "test".into(),
        application: "archbuilder-cli tests".into(),
        time_unix_ms: 0,
    }
}

fn cli(args: &[&str]) -> (Exit, Value) {
    let product = archbuilder_cli::product(Arc::new(FixedClock(0)), owner()).unwrap();
    let args: Vec<OsString> = args.iter().map(OsString::from).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let exit = archbuilder_cli::run(args, &product, &b""[..], &mut out, &mut err);
    let text = if out.is_empty() { err } else { out };
    (
        exit,
        serde_json::from_slice(text.split(|b| *b == b'\n').next().unwrap()).unwrap_or(Value::Null),
    )
}

fn dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("archbuilder-fam-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write(d: &std::path::Path) -> Vec<String> {
    ["table", "door", "cabinet"]
        .into_iter()
        .zip([table(), door(), cabinet()])
        .map(|(n, def)| {
            let f = d.join(format!("{n}.txt"));
            std::fs::write(&f, def.to_text().unwrap()).unwrap();
            f.to_str().unwrap().to_owned()
        })
        .collect()
}

fn publish(d: &std::path::Path, extra: &[&str]) -> (Exit, Value, String) {
    let files = write(d);
    let out = d.join("furniture.archbfam").to_str().unwrap().to_owned();
    let entries: Vec<String> = files.iter().map(|f| format!("{f}=1.0.0")).collect();
    let mut args = vec![
        "family",
        "publish",
        "--out",
        &out,
        "--name",
        "furniture",
        "--version",
        "1.0.0",
        "--license",
        "Apache-2.0",
    ];
    for e in &entries {
        args.extend(["--family", e]);
    }
    args.extend(extra);
    let (exit, line) = cli(&args);
    (exit, line, out)
}

#[test]
fn several_definitions_publish_into_one_package_and_check_clean() {
    let d = dir("ok");
    let (exit, line, out) = publish(&d, &[]);
    assert_eq!(exit, Exit::Success, "{line}");
    let (exit, report) = cli(&["family", "check", &out]);
    assert_eq!(exit, Exit::Success, "{report}");
    assert_eq!(report["ok"], true);
    assert_eq!(report["package"], "furniture 1.0.0");
    assert_eq!(report["families"].as_array().unwrap().len(), 3);
    // The published text is the text unit 4 writes, and its checksum is that text's.
    let package = Package::open(
        std::path::Path::new(&out),
        Mode::ReadOnly,
        &arc_base::Limits::DEFAULT,
    )
    .unwrap();
    let product = archbuilder_cli::product(Arc::new(FixedClock(0)), owner()).unwrap();
    let doc = package
        .open_document(
            Arc::clone(&product.types),
            Arc::clone(&product.evaluators),
            &arc_doc::LoadOptions::default(),
            arc_doc::DocumentConfig::default(),
        )
        .unwrap();
    let contents = archbuilder_model::family::package::read(&doc.snapshot());
    let mut expect = table();
    expect.header.version = Version {
        major: 1,
        minor: 0,
        patch: 0,
    };
    let got = contents
        .definitions
        .iter()
        .find(|p| p.name == "Table")
        .unwrap();
    assert_eq!(got.text, expect.to_text().unwrap());
    assert_eq!(got.checksum, archbuilder_model::family::checksum(&got.text));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_tampered_checksum_or_text_is_refused() {
    let d = dir("tamper");
    let (_, _, out) = publish(&d, &[]);
    for field in ["checksum", "text"] {
        let file = d.join(format!("tampered-{field}.archbfam"));
        std::fs::copy(&out, &file).unwrap();
        let product = archbuilder_cli::product(Arc::new(FixedClock(0)), owner()).unwrap();
        let mut package =
            Package::open(&file, Mode::Edit(&owner()), &arc_base::Limits::DEFAULT).unwrap();
        let doc = package
            .open_document(
                Arc::clone(&product.types),
                Arc::clone(&product.evaluators),
                &arc_doc::LoadOptions::default(),
                arc_doc::DocumentConfig::default(),
            )
            .unwrap();
        let record =
            archbuilder_model::spatial::objects(&doc.snapshot(), "archbuilder.fam.definition")[0]
                .id;
        let (name, value) = if field == "checksum" {
            ("checksum", "0".repeat(64))
        } else {
            (
                "text",
                archbuilder_model::spatial::get(&doc.snapshot(), record).map_or_else(
                    String::new,
                    |o| {
                        format!(
                            "{}\n",
                            archbuilder_model::types::text(&o.values, 5)
                                .unwrap_or_default()
                                .replacen("Table", "Tablf", 1)
                        )
                    },
                ),
            )
        };
        doc.submit(move |txn| {
            txn.modify(
                record,
                arc_doc::FieldPath::new(name),
                Some(arc_doc::Value::String(value)),
            )
        })
        .wait()
        .unwrap();
        package
            .save_document(&doc, &[], None, &mut |_| Ok(()))
            .unwrap();
        drop(package);
        let (exit, report) = cli(&["family", "check", file.to_str().unwrap()]);
        assert_eq!(exit, Exit::Invalid, "{field}: {report}");
        assert_eq!(report["ok"], false);
        assert_eq!(
            report["refusals"][0]["code"], "ab.family.checksum_mismatch",
            "{field}"
        );
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_missing_dependency_is_refused_until_the_package_is_available() {
    let d = dir("deps");
    let (exit, line, out) = publish(&d, &["--depends", "base>=1.0.0"]);
    assert_eq!(exit, Exit::Success, "{line}");
    let (exit, report) = cli(&["family", "check", &out]);
    assert_eq!(exit, Exit::Invalid);
    assert_eq!(
        report["refusals"][0]["code"],
        "ab.family.dependency_missing"
    );
    assert_eq!(report["refusals"][0]["arguments"]["name"], "base");
    let (exit, _) = cli(&["family", "check", &out, "--available", "base@0.9.0"]);
    assert_eq!(exit, Exit::Invalid, "too old");
    let (exit, report) = cli(&["family", "check", &out, "--available", "base@1.2.0"]);
    assert_eq!(exit, Exit::Success, "{report}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn publishing_refuses_a_failing_test_case_a_bad_version_and_an_existing_file() {
    let d = dir("refuse");
    // A test case whose expected volume is wrong.
    let mut bad = table();
    bad.tests[0].volume = Some(9.0);
    let f = d.join("bad.txt");
    std::fs::write(&f, bad.to_text().unwrap()).unwrap();
    let out = d.join("bad.archbfam");
    let entry = format!("{}=1.0.0", f.to_str().unwrap());
    let args = [
        "family",
        "publish",
        "--out",
        out.to_str().unwrap(),
        "--name",
        "n",
        "--version",
        "1.0.0",
        "--family",
        &entry,
    ];
    let (exit, err) = cli(&args);
    assert_ne!(exit, Exit::Success);
    assert_eq!(
        err["error"]["data"]["code"], "ab.family.test_failed",
        "{err}"
    );
    assert!(!out.exists(), "no partial package");
    // A malformed version.
    let entry = format!("{}=one", f.to_str().unwrap());
    let args = [
        "family",
        "publish",
        "--out",
        out.to_str().unwrap(),
        "--name",
        "n",
        "--version",
        "1.0.0",
        "--family",
        &entry,
    ];
    assert_ne!(cli(&args).0, Exit::Success);
    // An existing output is not overwritten.
    let (_, _, made) = publish(&d, &[]);
    let (exit, _, _) = publish(&d, &[]);
    assert_ne!(exit, Exit::Success);
    assert!(std::path::Path::new(&made).exists());
    let _ = std::fs::remove_dir_all(&d);
}
