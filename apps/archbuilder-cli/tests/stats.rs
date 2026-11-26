//! `inspect` prints the model statistics and the selection sets of a saved file.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::ffi::OsString;
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

#[test]
fn inspect_prints_statistics_and_selection_sets_after_reopen() {
    let dir = std::env::temp_dir().join(format!("archbuilder-cli-stats-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("house.archb");
    let (file, dir_text) = (file.to_str().unwrap(), dir.to_str().unwrap());
    let run = |create: bool, command: &str, params: &Value| {
        let params = params.to_string();
        let mut args = vec![
            "run",
            file,
            "--save",
            "--allow-edit",
            "--write-root",
            dir_text,
        ];
        if create {
            args.push("--create");
        }
        args.extend(["--command", command, "--params", &params]);
        let (exit, out) = cli(&args);
        assert_eq!(exit, Exit::Success, "{command}");
        out
    };
    run(true, "archbuilder.project.new", &json!({}));
    let wall = run(
        false,
        "archbuilder.wall.create",
        &json!({"start": [0.0, 0.0], "end": [4.0, 0.0]}),
    );
    let wall = wall["value"]["wall"]
        .as_str()
        .or_else(|| wall["outcome"]["value"]["wall"].as_str())
        .unwrap()
        .to_owned();
    run(
        false,
        "archbuilder.set.create",
        &json!({"name": "Exterior", "objects": [wall]}),
    );
    let (exit, dump) = cli(&["inspect", file]);
    assert_eq!(exit, Exit::Success);
    assert_eq!(dump["statistics"]["elements"], 1);
    assert_eq!(dump["statistics"]["selection_sets"], 1);
    assert_eq!(dump["selection_sets"][0]["name"], "Exterior");
    assert_eq!(dump["selection_sets"][0]["members"][0], wall);
    let _ = std::fs::remove_dir_all(&dir);
}
