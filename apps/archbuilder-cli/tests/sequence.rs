//! The headless sequence a script or a pipeline runs: create a project, add a storey, change a
//! level and rename a storey, each a `run --save` on the package file, and read the structure back
//! from the saved file with `inspect`. A refused change leaves the file as it was.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::ffi::OsString;
use std::path::Path;
use std::sync::Arc;

use arc_command::FixedClock;
use arc_command::headless::Exit;
use arc_doc::package::LockOwner;
use serde_json::{Value, json};

fn cli(args: &[&str]) -> (Exit, Value) {
    let owner = LockOwner {
        user: "test".into(),
        host: "test".into(),
        application: "archbuilder-cli tests".into(),
        time_unix_ms: 0,
    };
    let product = archbuilder_cli::product(Arc::new(FixedClock(0)), owner).unwrap();
    let args: Vec<OsString> = args.iter().map(OsString::from).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let exit = archbuilder_cli::run(args, &product, &b""[..], &mut out, &mut err);
    let text = if out.is_empty() { err } else { out };
    (
        exit,
        serde_json::from_slice(text.split(|b| *b == b'\n').next().unwrap()).unwrap_or(Value::Null),
    )
}

fn run(file: &Path, dir: &Path, create: bool, command: &str, params: &Value) -> (Exit, Value) {
    let (file, dir, params) = (
        file.to_str().unwrap(),
        dir.to_str().unwrap(),
        params.to_string(),
    );
    let mut args = vec!["run", file, "--save", "--allow-edit", "--write-root", dir];
    if create {
        args.push("--create");
    }
    args.extend(["--command", command, "--params", &params]);
    cli(&args)
}

/// The names of the storeys with their level's elevation, bottom up, and the project name.
fn structure(file: &Path) -> (String, Vec<(String, f64)>) {
    let (exit, inspected) = cli(&["inspect", file.to_str().unwrap()]);
    assert_eq!(exit, Exit::Success);
    let objects = inspected["objects"].as_array().unwrap();
    let of = |key: &'static str| objects.iter().filter(move |o| o["type_key"] == key);
    let project = of("archbuilder.project").next().unwrap()["values"]["name"].as_str();
    let mut storeys: Vec<(String, f64)> = of("archbuilder.storey")
        .map(|s| {
            let level = of("archbuilder.level")
                .find(|l| l["id"] == s["values"]["level"])
                .unwrap();
            (
                s["values"]["name"].as_str().unwrap().to_owned(),
                level["values"]["elevation"].as_f64().unwrap(),
            )
        })
        .collect();
    storeys.sort_by(|a, b| a.1.total_cmp(&b.1));
    (project.unwrap().to_owned(), storeys)
}

#[test]
fn a_project_is_created_changed_saved_and_read_back_through_the_cli() {
    let dir = std::env::temp_dir().join(format!("archbuilder-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("house.archb");

    let (exit, made) = run(
        &file,
        &dir,
        true,
        "archbuilder.project.new",
        &json!({"name": "House"}),
    );
    assert_eq!(exit, Exit::Success);
    assert_eq!(made["value"]["storeys"].as_array().unwrap().len(), 3);

    let (exit, added) = run(
        &file,
        &dir,
        false,
        "archbuilder.storey.add",
        &json!({"name": "Roof", "elevation": "10.5 m"}),
    );
    assert_eq!(exit, Exit::Success);
    let roof = added["value"]["storey"].as_str().unwrap();
    let floor2 = made["value"]["storeys"][2].as_str().unwrap();

    let (exit, _) = run(
        &file,
        &dir,
        false,
        "archbuilder.level.set_elevation",
        &json!({"level": roof, "elevation": "11000 mm"}),
    );
    assert_eq!(exit, Exit::Success);
    let (exit, _) = run(
        &file,
        &dir,
        false,
        "archbuilder.storey.rename",
        &json!({"storey": floor2, "name": "Top Floor"}),
    );
    assert_eq!(exit, Exit::Success);

    let expected = (
        "House".to_owned(),
        vec![
            ("Ground Floor".to_owned(), 0.0),
            ("Floor 1".to_owned(), 3.0),
            ("Top Floor".to_owned(), 6.0),
            ("Roof".to_owned(), 11.0),
        ],
    );
    assert_eq!(structure(&file), expected);

    // A refused change reports a conflict and leaves the saved file as it was.
    let (exit, refused) = run(
        &file,
        &dir,
        false,
        "archbuilder.storey.add",
        &json!({"name": "roof"}),
    );
    assert_eq!(exit, Exit::Failed);
    assert_eq!(refused["error"]["data"]["code"], "ab.model.name_taken");
    assert_eq!(structure(&file), expected);
    let _ = std::fs::remove_dir_all(&dir);
}
