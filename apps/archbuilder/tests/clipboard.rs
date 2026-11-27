//! Copy and paste of the newer element kinds through the windowed shell: railings, ramps (with
//! their railings), grids, reference planes and roofs by extrusion are copied by Ctrl+C and pasted
//! in place, each one more.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;

use arc_ui::egui;
use archbuilder::shortcuts::ui;
use archbuilder_model::types::{GRID, RAILING, RAMP, REFERENCE_PLANE, ROOF_EXTRUSION};
use common::Ui;
use serde_json::json;

#[test]
fn every_kind_is_copied_and_pasted_in_place() {
    let mut ui = Ui::new("clip-kinds");

    ui.invoke(
        "archbuilder.railing.create",
        json!({"points": [[0.0, 0.0], [3.0, 0.0]]}),
    );
    ui.invoke(
        "archbuilder.ramp.create",
        json!({"start": [0.0, 4.0], "to": [6.0, 4.0]}),
    );
    ui.invoke(
        "archbuilder.grid.create",
        json!({"start": [-2.0, -2.0], "end": [-2.0, 8.0]}),
    );
    ui.invoke(
        "archbuilder.reference_plane.create",
        json!({"start": [0.0, -2.0], "end": [8.0, -2.0]}),
    );
    ui.invoke(
        "archbuilder.roof_extrusion.create",
        json!({"plane": [[0.0, 0.0], [4.0, 0.0]], "profile": [[0.0, 3.0], [4.0, 4.0]], "end": 3.0}),
    );
    let kinds = [RAILING, RAMP, GRID, REFERENCE_PLANE, ROOF_EXTRUSION];
    let before: Vec<usize> = kinds.iter().map(|k| ui.count(k)).collect();
    assert_eq!(before, [3, 1, 1, 1, 1]);
    ui.menu(ui::SELECT_ALL);
    ui.menu(ui::COPY);
    ui.type_text("ps");
    ui.key(egui::Key::Enter);
    let after: Vec<usize> = kinds.iter().map(|k| ui.count(k)).collect();
    assert_eq!(after, [6, 2, 2, 2, 2], "{}", ui.labels());
}
