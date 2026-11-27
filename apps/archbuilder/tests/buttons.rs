//! The buttons of commands that need more than a click, through the windowed shell: floor slope
//! and roof by extrusion ask for their points, floor edge works on the selected floor, the
//! selection set buttons ask for a name, and statistics says its report. Each button submits its
//! command's identifier as a line, as the command tabs do.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;

use arc_ui::egui;
use archbuilder::shortcuts::ui;
use archbuilder_commands::floor::FLOOR_CREATE;
use archbuilder_commands::floor_edge::FLOOR_EDGE_CREATE;
use archbuilder_commands::floor_slope::FLOOR_SET_SLOPE;
use archbuilder_commands::roof_extrusion::ROOF_EXTRUSION_CREATE;
use archbuilder_commands::selection::{SET_CREATE, SET_SELECT};
use archbuilder_commands::stats::STATISTICS;
use archbuilder_model::spatial;
use archbuilder_model::types::{FLOOR, FLOOR_EDGE, ROOF_EXTRUSION, SELECTION_SET};
use common::Ui;
use serde_json::json;

/// A rectangle of walls and a selected floor inside it.
fn with_floor(name: &str) -> Ui {
    let mut ui = Ui::new(name);
    ui.draw_walls();
    ui.invoke(
        FLOOR_CREATE,
        json!({"points": [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]]}),
    );
    assert_eq!(ui.count(FLOOR), 1);
    // The walls and the floor: the commands take the floor of the selection.
    ui.menu(ui::SELECT_ALL);
    assert_eq!(ui.selected(), 5);
    ui
}

fn floor_slope(ui: &Ui) -> Option<f64> {
    let tab = ui.app.active().unwrap();
    let snapshot = ui.app.tab(tab).unwrap().document().snapshot();
    let floor = spatial::objects(&snapshot, FLOOR)[0].clone();
    archbuilder_model::ceiling::Arrow::read(&floor.values, archbuilder_model::types::floor::ARROW)
        .and_then(|a| match a.rise {
            archbuilder_model::ceiling::Rise::Angle(v) => Some(v),
            archbuilder_model::ceiling::Rise::EndHeight(_) => None,
        })
}

#[test]
fn the_floor_slope_button_asks_for_the_arrow_and_the_slope_of_the_selected_floor() {
    let mut ui = with_floor("slope");
    ui.line(FLOOR_SET_SLOPE);
    assert!(
        ui.prompt().contains("start of the arrow"),
        "{}",
        ui.prompt()
    );
    ui.line("0,1.5");
    assert!(ui.prompt().contains("end of the arrow"), "{}", ui.prompt());
    ui.line("4,1.5");
    // A slope that cannot be read is refused and the prompt stays.
    ui.line("steep");
    assert!(ui.labels().contains("cannot be used"), "{}", ui.labels());
    ui.line("2%");
    let slope = floor_slope(&ui).unwrap();
    assert!((slope - 0.02_f64.atan()).abs() < 1e-9, "{slope}");
    // The command ran once: the line is idle again.
    assert!(ui.prompt().starts_with("Command"), "{}", ui.prompt());
    // Without a floor selected the button says so.
    let mut bare = Ui::new("slope-bare");
    bare.line(FLOOR_SET_SLOPE);
    assert!(bare.labels().contains("No floor is named or selected"));
}

#[test]
fn the_floor_edge_button_runs_on_the_selected_floor() {
    let mut ui = with_floor("edge");
    ui.line(FLOOR_EDGE_CREATE);
    assert_eq!(ui.count(FLOOR_EDGE), 1);
}

#[test]
fn the_roof_by_extrusion_button_asks_for_the_plane_the_typed_profile_and_the_depth() {
    let mut ui = Ui::new("roof-extrusion");
    ui.draw_walls();
    ui.line(ROOF_EXTRUSION_CREATE);
    assert!(ui.prompt().contains("first point of the vertical plane"));
    ui.line("0,0");
    ui.line("4,0");
    assert!(ui.prompt().contains("as u,z"), "{}", ui.prompt());
    // A click in plan is no profile point.
    ui.click_viewport();
    assert!(ui.prompt().contains("(0 points)"), "{}", ui.prompt());
    for point in ["-0.5,3", "2,4.5", "4.5,3"] {
        ui.line(point);
    }
    ui.key(egui::Key::Enter);
    assert!(ui.prompt().contains("depth"), "{}", ui.prompt());
    ui.line("3");
    assert_eq!(ui.count(ROOF_EXTRUSION), 1);
    assert!(ui.prompt().starts_with("Command"), "{}", ui.prompt());
}

#[test]
fn the_set_buttons_save_the_selection_under_a_name_and_select_it_again() {
    let mut ui = Ui::new("sets");
    ui.draw_walls();
    // Without a set, the select button says so.
    ui.line(SET_SELECT);
    assert!(ui.labels().contains("No selection set has that name"));
    ui.menu(ui::SELECT_ALL);
    assert_eq!(ui.selected(), 4);
    ui.line(SET_CREATE);
    assert!(ui.labels().contains("Name of the new selection set"));
    ui.line("Outer walls");
    assert_eq!(ui.count(SELECTION_SET), 1);
    ui.key(egui::Key::Escape);
    ui.key(egui::Key::Escape);
    assert_eq!(ui.selected(), 0);
    ui.line(SET_SELECT);
    assert!(ui.labels().contains("(1 sets"), "{}", ui.labels());
    ui.line("nothing like it");
    assert!(ui.labels().contains("No selection set has that name"));
    ui.line("outer WALLS");
    assert_eq!(ui.selected(), 4);
}

#[test]
fn the_statistics_button_says_the_report() {
    let mut ui = Ui::new("statistics");
    ui.draw_walls();
    ui.line(STATISTICS);
    let text = ui.labels();
    assert!(
        text.contains("Model statistics: 4 elements (Wall 4)"),
        "{text}"
    );
}
