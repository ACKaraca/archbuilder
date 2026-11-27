//! `inspect` lists the document variables and project parameters of a saved file, the same text
//! on every run.

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
fn inspect_lists_variables_with_the_same_text_every_time() {
    let dir = std::env::temp_dir().join(format!("archbuilder-cli-vars-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("vars.archb");
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
        let (exit, _) = cli(&args);
        assert_eq!(exit, Exit::Success, "{command}");
    };
    run(true, "archbuilder.project.new", &json!({}));
    run(
        false,
        "archbuilder.variable.create",
        &json!({"name": "Module", "data_type": "length", "value": 1.2}),
    );
    run(
        false,
        "archbuilder.variable.create",
        &json!({"name": "Bay", "data_type": "length", "formula": "Module * 3"}),
    );
    let (exit, dump) = cli(&["inspect", file]);
    assert_eq!(exit, Exit::Success);
    let bay = &dump["variables"][0];
    assert_eq!(bay["identifier"], "Bay");
    assert_eq!(bay["formula"], "Module * 3");
    assert!((bay["value"].as_f64().unwrap() - 3.6).abs() < 1e-9);
    run(
        false,
        "archbuilder.wall.create",
        &json!({"start": [0.0, 0.0], "end": [4.0, 0.0]}),
    );
    run(
        false,
        "archbuilder.parameter.create",
        &json!({"name": "Mark", "data_type": "length", "categories": ["archbuilder.wall"],
                "formula": "Bay / 2"}),
    );
    let (_, dump) = cli(&["inspect", file]);
    assert_eq!(dump["parameters"]["parameters"][0]["identifier"], "Mark");
    let value = &dump["parameters"]["values"][0];
    assert_eq!(value["source"], "default");
    assert!((value["value"].as_f64().unwrap() - 1.8).abs() < 1e-9);
    let (_, again) = cli(&["inspect", file]);
    assert_eq!(dump.to_string(), again.to_string());
    let _ = std::fs::remove_dir_all(&dir);
}
