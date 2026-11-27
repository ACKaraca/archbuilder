//! A typed shortcut and the Enter that follows, through the windowed shell: Enter only confirms
//! a shortcut when it is the next input and the started prompt has no use for an empty answer.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    reason = "tests; pixel positions fit f32"
)]

mod common;

use arc_ui::egui;
use archbuilder::shortcuts::ui;
use archbuilder_model::spatial;
use archbuilder_model::types::WALL;
use common::Ui;

const MATCHING: &str = "Match type:";

#[test]
fn enter_right_after_a_typed_match_type_keeps_it_and_after_a_click_ends_it() {
    let mut ui = Ui::new("mt");
    ui.draw_walls();
    ui.type_text("mt");
    assert!(ui.prompt().contains(MATCHING), "{}", ui.prompt());
    ui.key(egui::Key::Enter);
    assert!(
        ui.prompt().contains(MATCHING),
        "Enter after MT ended match type"
    );
    // A click comes between: Enter then finishes.
    ui.click_viewport();
    assert!(ui.prompt().contains(MATCHING), "{}", ui.prompt());
    ui.key(egui::Key::Enter);
    assert!(
        !ui.prompt().contains(MATCHING),
        "Enter after a click did not end match type"
    );
}

#[test]
fn enter_right_after_a_typed_paste_pastes_in_place() {
    let mut ui = Ui::new("ps");
    ui.draw_walls();
    ui.menu(ui::SELECT_ALL);
    ui.menu(ui::COPY);
    ui.type_text("ps");
    ui.key(egui::Key::Enter);
    assert_eq!(ui.walls(), 8);
    // Nothing of the paste is left over: Escape, then match type with Enter in the very next
    // frame (decided while the line still looked idle, it is not a repeat of the paste), and a
    // click paste nothing.
    ui.key(egui::Key::Escape);
    ui.type_text("");
    ui.frame(vec![egui::Event::Text("mt".into())], Vec::new());
    ui.key(egui::Key::Enter);
    assert!(ui.prompt().contains(MATCHING), "{}", ui.prompt());
    ui.click_viewport();
    assert_eq!(ui.walls(), 8);
}

#[test]
fn enter_right_after_a_typed_instant_command_is_dropped_and_the_next_one_repeats() {
    let mut ui = Ui::new("ze");
    ui.draw_walls();
    // ZE is done at once: the Enter that confirms it is dropped, it does not restart the wall
    // command; a later Enter repeats the last command.
    ui.type_text("ze");
    ui.key(egui::Key::Enter);
    let prompt = ui.prompt();
    assert!(!prompt.contains("specify the start point"), "{prompt}");
    ui.key(egui::Key::Enter);
    let prompt = ui.prompt();
    assert!(prompt.contains("specify the start point"), "{prompt}");
}

#[test]
fn enter_right_after_a_typed_pin_pins_without_restarting_the_last_tool() {
    let mut ui = Ui::new("pn");
    ui.draw_walls();
    ui.menu(ui::SELECT_ALL);
    ui.type_text("pn");
    ui.key(egui::Key::Enter);
    let prompt = ui.prompt();
    assert!(!prompt.contains("specify"), "{prompt}");
    let tab = ui.app.active().unwrap();
    let snapshot = ui.app.tab(tab).unwrap().document().snapshot();
    assert!(
        spatial::objects(&snapshot, WALL)
            .iter()
            .all(|w| archbuilder_model::types::is_pinned(WALL, &w.values))
    );
}

#[test]
fn a_command_started_during_match_type_ends_it() {
    let mut ui = Ui::new("mt-wall");
    ui.draw_walls();
    ui.type_text("mt");
    ui.key(egui::Key::Enter);
    ui.line(archbuilder_commands::wall::WALL_CREATE);
    assert!(
        ui.prompt().contains("specify the start point"),
        "{}",
        ui.prompt()
    );
    ui.key(egui::Key::Escape);
    assert!(!ui.prompt().contains("specify"), "{}", ui.prompt());
}

#[test]
fn a_refused_modify_step_says_why_at_once() {
    let mut ui = Ui::new("refuse");
    ui.draw_walls();
    // A split across nothing: the pick misses every wall.
    ui.line("split");
    ui.line("20,20");
    let text = ui.labels();
    assert!(text.contains("No wall at that point"), "{text}");
    // A clicked pick that misses says so once.
    ui.click_viewport();
    let text = ui.labels();
    assert_eq!(text.matches("No wall at that point").count(), 2, "{text}");
    ui.key(egui::Key::Escape);
    // A pinned wall refuses a move, and says so before anything else is typed.
    ui.menu(ui::SELECT_ALL);
    ui.line("pin");
    ui.line("m");
    ui.line("0,0");
    ui.line("1,0");
    let text = ui.labels();
    assert!(text.contains("is pinned"), "{text}");
}
