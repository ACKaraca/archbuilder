//! `export-ifc` writes the document of a file as IFC4 and prints the loss report; the command
//! `archbuilder.file.export_ifc` of `run` writes the same bytes inside its write root.

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
fn export_ifc_writes_the_file_and_prints_the_report() {
    let dir = std::env::temp_dir().join(format!("archbuilder-cli-ifc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("a")).unwrap();
    std::fs::create_dir_all(dir.join("b")).unwrap();
    let file = dir.join("house.archb");
    let (file, dir_text) = (file.to_str().unwrap(), dir.to_str().unwrap());
    for (create, command, params) in [
        (true, "archbuilder.project.new", json!({})),
        (
            false,
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [6.0, 0.0], "name": "South"}),
        ),
        (
            false,
            "archbuilder.window.place",
            json!({"wall": null, "distance": 3.0}),
        ),
    ] {
        let mut params = params;
        if command == "archbuilder.window.place" {
            let (_, dump) = cli(&["inspect", file]);
            params["wall"] = dump["walls"][0]["id"].clone();
        }
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
        assert_eq!(cli(&args).0, Exit::Success);
    }

    let out = dir.join("a").join("house.ifc");
    let (exit, report) = cli(&[
        "export-ifc",
        file,
        out.to_str().unwrap(),
        "--timestamp",
        "1970-01-01T00:00:00",
    ]);
    assert_eq!(exit, Exit::Success);
    assert_eq!(report["elements"], 2);
    assert_eq!(report["summary"]["dropped"], 0);
    let bytes = std::fs::read(&out).unwrap();
    assert_eq!(report["bytes"], bytes.len());
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(text.contains("FILE_NAME('house.ifc','1970-01-01T00:00:00'"));
    assert!(text.contains("IFCWALL(") && text.contains("'South'"));
    assert!(text.contains("IFCWINDOW("));

    // The command writes the same bytes inside its write root; outside it is refused.
    let command_out = dir.join("b").join("house.ifc");
    let params = json!({"path": command_out.to_str().unwrap()}).to_string();
    let (exit, result) = cli(&[
        "run",
        file,
        "--allow-edit",
        "--write-root",
        dir_text,
        "--command",
        "archbuilder.file.export_ifc",
        "--params",
        &params,
    ]);
    assert_eq!(exit, Exit::Success, "{result}");
    assert_eq!(std::fs::read(&command_out).unwrap(), bytes);

    // Usage errors and a missing file are reported with an exit code.
    assert_eq!(cli(&["export-ifc", file]).0, Exit::Usage);
    assert_eq!(
        cli(&["export-ifc", file, "x.ifc", "--bogus"]).0,
        Exit::Usage
    );
    let missing = dir.join("none.archb");
    assert_ne!(
        cli(&[
            "export-ifc",
            missing.to_str().unwrap(),
            out.to_str().unwrap()
        ])
        .0,
        Exit::Success
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn export_mesh_writes_each_format_and_prints_the_report() {
    let dir = std::env::temp_dir().join(format!("archbuilder-cli-mesh-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("house.archb");
    let (file, dir_text) = (file.to_str().unwrap(), dir.to_str().unwrap());
    for (create, command, params) in [
        (true, "archbuilder.project.new", json!({})),
        (
            false,
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [6.0, 0.0], "name": "South"}),
        ),
    ] {
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
        assert_eq!(cli(&args).0, Exit::Success);
    }
    let obj = dir.join("house.obj");
    let (exit, report) = cli(&["export-mesh", file, obj.to_str().unwrap(), "--unit", "mm"]);
    assert_eq!(exit, Exit::Success, "{report}");
    assert_eq!(report["elements"], 1);
    assert!(report["triangles"].as_u64().unwrap() >= 12);
    assert!((report["bounds"][1][0].as_f64().unwrap() - 6000.0).abs() < 1e-6);
    let text = std::fs::read_to_string(&obj).unwrap();
    assert!(text.contains("mtllib house.mtl") && text.contains("o South ["));
    assert!(dir.join("house.mtl").is_file());
    for name in ["house.stl", "house.glb", "house.gltf"] {
        let out = dir.join(name);
        assert_eq!(
            cli(&["export-mesh", file, out.to_str().unwrap()]).0,
            Exit::Success
        );
        assert!(out.is_file());
    }
    assert_eq!(cli(&["export-mesh", file, "x.bin"]).0, Exit::Usage);
    let _ = std::fs::remove_dir_all(&dir);
}
