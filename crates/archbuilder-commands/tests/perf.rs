//! Edit cost on a generated model of 220 walls, 100 rooms and one slab, in the work an edit asks
//! for: the commit, then what the window draws again (the layers of the walls, the slab, the plan
//! lines). Run in release:
//! `cargo test --release -p archbuilder-commands --test perf -- --ignored --nocapture`.

#![allow(
    clippy::unwrap_used,
    clippy::print_stdout,
    clippy::too_many_lines,
    reason = "a measurement"
)]

use std::sync::Arc;
use std::time::Instant;

use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::types::{FLOOR, ROOM, STOREY, WALL};
use archbuilder_model::{cleanup, spatial};
use serde_json::json;

/// What the window draws after a commit: every wall (the walls of the changed storey), every slab
/// and the plan lines of every storey.
fn redraw(snapshot: &Snapshot, walls: bool) -> usize {
    let mut n = 0;
    if walls {
        for w in spatial::objects(snapshot, WALL) {
            n += cleanup::wall_pieces(snapshot, &w).unwrap().unwrap().len();
        }
        for f in spatial::objects(snapshot, FLOOR) {
            n += usize::from(cleanup::floor_solid(snapshot, &f).unwrap().is_ok());
        }
    }
    for s in spatial::objects(snapshot, STOREY) {
        n += cleanup::plan_lines(snapshot, s.id, 1.0).len();
    }
    n
}

#[test]
#[ignore = "a measurement"]
fn the_cost_of_a_wall_move_and_of_a_room_rename() {
    let document = Document::open(
        Snapshot::empty(Arc::new(archbuilder_commands::types().unwrap())),
        Arc::new(Evaluators::new()),
        DocumentConfig {
            workers: 0,
            ..DocumentConfig::default()
        },
    )
    .unwrap();
    archbuilder_commands::install(&document);
    let session = Session::new(Surface::CommandLine, [Permission::Read, Permission::Edit]);
    let registry = archbuilder_commands::registry().unwrap();
    let run = |c: &str, p: serde_json::Value| {
        Invoker::new(&registry)
            .invoke(
                &document,
                &session,
                Invocation {
                    seed: 99,
                    ..Invocation::new(c, p)
                },
            )
            .unwrap()
            .value
    };
    run("archbuilder.project.new", json!({}));
    let t = Instant::now();
    for row in 0..=10 {
        for k in 0..10 {
            let (y, x) = (f64::from(row) * 3.0, f64::from(k) * 3.0);
            run(
                "archbuilder.wall.create",
                json!({"start": [x, y], "end": [x + 3.0, y], "type": "Exterior masonry cavity"}),
            );
            run(
                "archbuilder.wall.create",
                json!({"start": [y, x], "end": [y, x + 3.0], "type": "Exterior masonry cavity"}),
            );
        }
    }
    let snapshot = document.snapshot();
    let upper = spatial::storeys(&snapshot, spatial::buildings(&snapshot)[0].id)[1]
        .storey
        .id;
    run(
        "archbuilder.floor.create",
        json!({"storey": upper, "points": [[0.0, 0.0], [30.0, 0.0], [30.0, 30.0], [0.0, 30.0]]}),
    );
    run("archbuilder.room.place_all", json!({}));
    println!("built in {:?}", t.elapsed());
    let snapshot = document.snapshot();
    println!(
        "{} walls, {} rooms, {} slabs",
        spatial::objects(&snapshot, WALL).len(),
        spatial::objects(&snapshot, ROOM).len(),
        spatial::objects(&snapshot, FLOOR).len()
    );
    let t = Instant::now();
    redraw(&snapshot, true);
    println!("first draw {:?}", t.elapsed());
    // Move one wall: the commit and what is drawn again.
    let wall = spatial::objects(&snapshot, WALL)[7].id;
    let t = Instant::now();
    run(
        "archbuilder.modify.move",
        json!({"objects": [wall], "displacement": [0.0, 0.4]}),
    );
    let committed = t.elapsed();
    let after = document.snapshot();
    let storey = spatial::get(&after, wall).unwrap().header.owner.unwrap();
    let t = Instant::now();
    let _ = cleanup::storey_layers(&after, storey);
    println!("  layout of the storey {:?}", t.elapsed());
    let t = Instant::now();
    let _ = cleanup::plan_lines(&after, storey, 1.0);
    println!("  plan lines {:?}", t.elapsed());
    let t = Instant::now();
    for w in spatial::objects(&after, WALL) {
        cleanup::wall_pieces(&after, &w).unwrap().unwrap();
    }
    println!("  pieces of the walls {:?}", t.elapsed());
    let t = Instant::now();
    redraw(&after, true);
    println!(
        "wall move: commit {committed:?}, redraw (again, warm) {:?}",
        t.elapsed()
    );
    // Rename a room: it touches no wall, so no wall is drawn again, only the plan overlays.
    let room = spatial::objects(&after, ROOM)[3].id;
    let t = Instant::now();
    run(
        "archbuilder.property.set",
        json!({"objects": [room], "field": "name", "value": "Study"}),
    );
    let committed = t.elapsed();
    let after = document.snapshot();
    let t = Instant::now();
    redraw(&after, false);
    println!(
        "room rename: commit {committed:?}, redraw {:?}",
        t.elapsed()
    );
    // The plan view of the storey the walls stand on: the drawing of 220 walls, then what a wall
    // move and a room rename build again.
    let view = |snapshot: &Snapshot| {
        let storey = spatial::get(snapshot, wall).unwrap().header.owner.unwrap();
        archbuilder_model::view::of_storey(snapshot, storey).remove(0)
    };
    let cancel = arc_base::Cancel::new();
    let t = Instant::now();
    let built = archbuilder_model::plan::build(&after, &view(&after), &Default::default(), &cancel)
        .unwrap();
    println!(
        "plan view first build {:?} ({} primitives, {} diagnostics)",
        t.elapsed(),
        built.geometry.list.primitives.len(),
        built.geometry.diagnostics.len()
    );
    run(
        "archbuilder.modify.move",
        json!({"objects": [wall], "displacement": [0.0, -0.4]}),
    );
    let moved = document.snapshot();
    let t = Instant::now();
    let _ = archbuilder_model::plan::build(&moved, &view(&moved), &Default::default(), &cancel)
        .unwrap();
    println!("plan view after a wall move {:?}", t.elapsed());
    run(
        "archbuilder.property.set",
        json!({"objects": [room], "field": "name", "value": "Library"}),
    );
    let renamed = document.snapshot();
    let t = Instant::now();
    let _ = archbuilder_model::plan::build(&renamed, &view(&renamed), &Default::default(), &cancel)
        .unwrap();
    println!("plan view after a room rename {:?}", t.elapsed());
}
