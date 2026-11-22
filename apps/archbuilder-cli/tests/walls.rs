//! `inspect` lists the walls of a saved file with their storey, length, thickness, height and the
//! volume of the derived solid.

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
fn inspect_lists_each_wall_with_its_storey_and_the_volume_of_its_solid() {
    let dir = std::env::temp_dir().join(format!("archbuilder-cli-walls-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("walls.archb");
    let (file, dir_text) = (file.to_str().unwrap(), dir.to_str().unwrap());
    for (create, command, params) in [
        (true, "archbuilder.project.new", json!({})),
        (
            false,
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [4.0, 3.0], "thickness": 0.25,
                   "location_line": "exterior"}),
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
    let (exit, dump) = cli(&["inspect", file]);
    assert_eq!(exit, Exit::Success);
    let walls = dump["walls"].as_array().unwrap();
    assert_eq!(walls.len(), 1);
    let wall = &walls[0];
    assert_eq!(wall["storey"], "Ground Floor");
    assert_eq!(wall["name"], "Wall 1");
    assert_eq!(wall["length"].as_f64(), Some(5.0));
    assert_eq!(wall["thickness"].as_f64(), Some(0.25));
    assert_eq!(wall["height"].as_f64(), Some(3.0));
    assert!((wall["volume"].as_f64().unwrap() - 5.0 * 0.25 * 3.0).abs() < 1e-9);

    // The wall's top follows the level above: raising it makes the dump show the taller wall.
    let floor_1 = dump["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["type_key"] == "archbuilder.storey" && o["values"]["name"] == "Floor 1")
        .unwrap()["id"]
        .clone();
    let params = json!({"objects": [floor_1], "field": "elevation", "value": "4 m"}).to_string();
    let args = [
        "run",
        file,
        "--save",
        "--allow-edit",
        "--write-root",
        dir_text,
        "--command",
        "archbuilder.property.set",
        "--params",
        &params,
    ];
    assert_eq!(cli(&args).0, Exit::Success);
    let (_, dump) = cli(&["inspect", file]);
    let wall = &dump["walls"].as_array().unwrap()[0];
    assert_eq!(wall["height"].as_f64(), Some(4.0));
    assert!((wall["volume"].as_f64().unwrap() - 5.0 * 0.25 * 4.0).abs() < 1e-9);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn inspect_counts_the_corners_of_a_closed_rectangle_once() {
    let dir = std::env::temp_dir().join(format!("archbuilder-cli-joins-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("house.archb");
    let (file, dir_text) = (file.to_str().unwrap(), dir.to_str().unwrap());
    let corners = [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]];
    for k in 0..4 {
        let params = json!({"start": corners[k], "end": corners[(k + 1) % 4],
                            "height": 3.0})
        .to_string();
        let mut args = vec![
            "run",
            file,
            "--save",
            "--allow-edit",
            "--write-root",
            dir_text,
        ];
        if k == 0 {
            args.extend([
                "--create",
                "--command",
                "archbuilder.project.new",
                "--params",
                "{}",
            ]);
            assert_eq!(cli(&args).0, Exit::Success);
            args.truncate(6);
        }
        args.extend(["--command", "archbuilder.wall.create", "--params", &params]);
        assert_eq!(cli(&args).0, Exit::Success);
    }
    let (_, dump) = cli(&["inspect", file]);
    let walls = dump["walls"].as_array().unwrap();
    assert_eq!(walls.len(), 4);
    let total: f64 = walls.iter().map(|w| w["volume"].as_f64().unwrap()).sum();
    assert!(
        (total - (10.2 * 6.2 - 9.8 * 5.8) * 3.0).abs() < 1e-9,
        "{total}"
    );
    assert_eq!(walls[0]["start_join"], "auto");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn inspect_lists_doors_and_windows_and_the_wall_volume_without_their_openings() {
    let dir = std::env::temp_dir().join(format!("archbuilder-cli-inserts-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("inserts.archb");
    let (file, dir_text) = (file.to_str().unwrap(), dir.to_str().unwrap());
    let run = |create: bool, command: &str, params: Value| {
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
    run(true, "archbuilder.project.new", json!({}));
    run(
        false,
        "archbuilder.wall.create",
        json!({"start": [0.0, 0.0], "end": [10.0, 0.0], "height": 3.0}),
    );
    let (_, dump) = cli(&["inspect", file]);
    let wall = dump["walls"][0]["id"].clone();
    run(
        false,
        "archbuilder.door.place",
        json!({"wall": wall, "distance": 2.0}),
    );
    run(
        false,
        "archbuilder.window.place",
        json!({"wall": wall, "distance": 6.0, "width": 1.5, "flip_hand": true}),
    );
    let (exit, dump) = cli(&["inspect", file]);
    assert_eq!(exit, Exit::Success);
    let door = &dump["doors"][0];
    assert_eq!(
        (door["wall"].as_str(), door["name"].as_str()),
        (Some("Wall 1"), Some("Door 1"))
    );
    assert_eq!(door["distance"].as_f64(), Some(2.0));
    assert_eq!(door["width"].as_f64(), Some(0.9));
    assert_eq!(door["height"].as_f64(), Some(2.1));
    assert_eq!(door["sill"].as_f64(), Some(0.0));
    let window = &dump["windows"][0];
    assert_eq!(window["sill"].as_f64(), Some(0.9));
    assert_eq!(window["width"].as_f64(), Some(1.5));
    assert_eq!(window["hand"], "right");
    let wall = &dump["walls"][0];
    assert_eq!(wall["openings"].as_u64(), Some(2));
    // 10 x 0.2 x 3, minus 0.9 x 2.1 x 0.2 and 1.5 x 1.5 x 0.2 (the window type's height).
    assert!((wall["gross_volume"].as_f64().unwrap() - 6.0).abs() < 1e-9);
    let net = 6.0 - 0.9 * 2.1 * 0.2 - 1.5 * 1.5 * 0.2;
    assert!((wall["volume"].as_f64().unwrap() - net).abs() < 1e-9);
    let _ = std::fs::remove_dir_all(&dir);
}
