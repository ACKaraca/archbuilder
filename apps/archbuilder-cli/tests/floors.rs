//! `inspect` lists the floors of a saved file with their storey, area, thickness, volume and the
//! walls their edges follow.

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
fn inspect_lists_a_floor_by_interior_faces_with_its_area_volume_and_walls() {
    let dir = std::env::temp_dir().join(format!("archbuilder-cli-floors-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("house.archb");
    let (file, dir_text) = (file.to_str().unwrap(), dir.to_str().unwrap());
    let corners = [[0.0, 0.0], [0.0, 6.0], [10.0, 6.0], [10.0, 0.0]];
    let mut steps = vec![(true, "archbuilder.project.new", json!({}))];
    for k in 0..4 {
        steps.push((
            false,
            "archbuilder.wall.create",
            json!({"start": corners[k], "end": corners[(k + 1) % 4], "height": 3.0}),
        ));
    }
    steps.push((
        false,
        "archbuilder.floor.create",
        json!({"picks": [[5.0, 3.0]]}),
    ));
    steps.push((
        false,
        "archbuilder.floor.create",
        json!({"points": [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]],
               "holes": [[[1.0, 1.0], [2.0, 1.0], [2.0, 2.0], [1.0, 2.0]]],
               "thickness": 0.3}),
    ));
    for (create, command, params) in steps {
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
        assert_eq!(cli(&args).0, Exit::Success, "{command}");
    }
    let (exit, dump) = cli(&["inspect", file]);
    assert_eq!(exit, Exit::Success);
    let floors = dump["floors"].as_array().unwrap();
    assert_eq!(floors.len(), 2);
    let house = floors.iter().find(|f| f["name"] == "Slab 1").unwrap();
    assert_eq!(house["storey"], "Ground Floor");
    assert!((house["area"].as_f64().unwrap() - 9.8 * 5.8).abs() < 1e-9);
    assert!((house["thickness"].as_f64().unwrap() - 0.2).abs() < 1e-12);
    assert!((house["volume"].as_f64().unwrap() - 9.8 * 5.8 * 0.2).abs() < 1e-9);
    assert_eq!(house["walls"].as_array().unwrap().len(), 4);
    let sketched = floors.iter().find(|f| f["name"] == "Slab 2").unwrap();
    assert!((sketched["area"].as_f64().unwrap() - 15.0).abs() < 1e-9);
    assert!((sketched["volume"].as_f64().unwrap() - 4.5).abs() < 1e-9);
    assert!(sketched["walls"].as_array().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
