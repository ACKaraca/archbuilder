//! `inspect` lists the stairs of a saved file (save and reopen) with riser count and height, run
//! lengths and volume, the floor above with its shaft opening taken out, and `inspect --check`
//! reports walls on top of each other.

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
fn inspect_lists_stairs_after_reopen_with_the_opening_in_the_floor_above() {
    let dir = std::env::temp_dir().join(format!("archbuilder-cli-stairs-{}", std::process::id()));
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
        assert_eq!(cli(&args).0, Exit::Success, "{command}");
    };
    run(true, "archbuilder.project.new", &json!({}));
    let c = [[0.0, 0.0], [0.0, 6.0], [10.0, 6.0], [10.0, 0.0]];
    for k in 0..4 {
        run(
            false,
            "archbuilder.wall.create",
            &json!({"start": c[k], "end": c[(k + 1) % 4]}),
        );
    }
    // The left wall drawn a second time: a warning of the check.
    run(
        false,
        "archbuilder.wall.create",
        &json!({"start": [0.0, 0.0], "end": [0.0, 6.0]}),
    );
    let (_, dump) = cli(&["inspect", file]);
    let floor1 = dump["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["type_key"] == "archbuilder.storey" && o["values"]["name"] == "Floor 1")
        .unwrap()["id"]
        .clone();
    run(
        false,
        "archbuilder.floor.create",
        &json!({"storey": floor1, "points": [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]]}),
    );
    run(
        false,
        "archbuilder.stair.create",
        &json!({"start": [2.0, 1.0], "to": [3.0, 1.0], "shape": "l_turn", "turn": "left"}),
    );
    let (exit, dump) = cli(&["inspect", file, "--check"]);
    assert_eq!(exit, Exit::Success);
    let stairs = dump["stairs"].as_array().unwrap();
    assert_eq!(stairs.len(), 1);
    let s = &stairs[0];
    assert_eq!(s["risers"], 17);
    assert!((s["riser"].as_f64().unwrap() - 3.0 / 17.0).abs() < 1e-12);
    assert!((s["runs"][0].as_f64().unwrap() - 2.24).abs() < 1e-12);
    assert!((s["runs"][1].as_f64().unwrap() - 1.96).abs() < 1e-12);
    assert_eq!(s["stride_ok"], true);
    assert!(s["volume"].as_f64().unwrap() > 0.5);
    // The L opening: (2.24 + 1 + 0.1) x 1.1 for the first run and the landing, plus the second
    // run above the landing's clearance, (1.96 - 0.05) x 1.1, up to its arrival edge.
    let opening = (2.24 + 1.0 + 0.1) * 1.1 + (1.96 - 0.05) * 1.1;
    let floors = dump["floors"].as_array().unwrap();
    let area = floors[0]["area"].as_f64().unwrap();
    assert!((area - (60.0 - opening)).abs() < 1e-9, "{area}");
    let codes: Vec<_> = dump["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|f| f["code"].as_str())
        .collect();
    assert!(codes.contains(&"ab.model.wall_overlap"), "{codes:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
