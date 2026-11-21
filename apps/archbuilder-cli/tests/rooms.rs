//! `inspect` lists rooms of a saved file (save and reopen) with their state, area, perimeter and
//! volume, and `inspect --check` reports the room warnings.

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
fn inspect_lists_rooms_after_reopen_and_check_reports_the_states() {
    let dir = std::env::temp_dir().join(format!("archbuilder-cli-rooms-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("house.archb");
    let (file, dir_text) = (file.to_str().unwrap(), dir.to_str().unwrap());
    let c = [[0.0, 0.0], [0.0, 6.0], [10.0, 6.0], [10.0, 0.0]];
    let mut steps = vec![(true, "archbuilder.project.new", json!({}))];
    for k in 0..4 {
        steps.push((
            false,
            "archbuilder.wall.create",
            json!({"start": c[k], "end": c[(k + 1) % 4]}),
        ));
    }
    steps.push((
        false,
        "archbuilder.wall.create",
        json!({"start": [5.0, 0.0], "end": [5.0, 6.0]}),
    ));
    steps.push((false, "archbuilder.room.place_all", json!({})));
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
    for (create, command, params) in &steps {
        run(*create, command, params);
    }
    let (exit, dump) = cli(&["inspect", file, "--check"]);
    assert_eq!(exit, Exit::Success);
    let rooms = dump["rooms"].as_array().unwrap();
    assert_eq!(rooms.len(), 2);
    for room in rooms {
        assert_eq!(room["state"], "enclosed");
        assert!((room["area"].as_f64().unwrap() - 27.84).abs() < 1e-9);
        assert!((room["perimeter"].as_f64().unwrap() - 21.2).abs() < 1e-9);
        assert!((room["volume"].as_f64().unwrap() - 83.52).abs() < 1e-9);
    }
    let numbers: Vec<_> = rooms
        .iter()
        .map(|r| r["number"].as_str().unwrap())
        .collect();
    assert!(numbers.contains(&"101") && numbers.contains(&"102"));
    // Deleting the partition makes both rooms redundant: a warning, not an error.
    let partition = dump["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["values"]["start_x"] == 5.0 && o["values"]["end_x"] == 5.0)
        .unwrap()["id"]
        .clone();
    run(
        false,
        "archbuilder.wall.delete",
        &json!({"walls": [partition]}),
    );
    let (exit, dump) = cli(&["inspect", file, "--check"]);
    assert_eq!(exit, Exit::Success);
    let states: Vec<_> = dump["rooms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["number"].as_str().unwrap(), r["state"].as_str().unwrap()))
        .collect();
    assert!(states.contains(&("101", "enclosed")) && states.contains(&("102", "redundant")));
    let codes: Vec<_> = dump["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|f| f["code"].as_str())
        .filter(|c| c.contains("room"))
        .collect();
    assert_eq!(codes, ["ab.model.room_redundant"]);
    let _ = std::fs::remove_dir_all(&dir);
}
