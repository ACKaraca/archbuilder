//! A typed shortcut and the Enter that follows, through the windowed shell: Enter only confirms
//! a shortcut when it is the next input and the started prompt has no use for an empty answer.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    reason = "tests; pixel positions fit f32"
)]

use std::collections::BTreeMap;
use std::sync::Arc;

use arc_app::application::{AppConfig, Application};
use arc_app::recent::RecentFiles;
use arc_app::shell::{FrameInput, Shell};
use arc_base::{IdGenerator, Limits};
use arc_doc::package::LockOwner;
use arc_ui::Intent;
use arc_ui::egui::{self, accesskit};
use archbuilder::shortcuts::ui;
use archbuilder_model::spatial;
use archbuilder_model::types::WALL;

struct Ui {
    app: Application,
    shell: Shell,
    ctx: egui::Context,
    localizer: arc_app::locale::Localizer,
    _dir: TempDir,
}

struct TempDir(std::path::PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

impl Ui {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("archbuilder-enter-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let owner = LockOwner {
            user: "test".into(),
            host: "test".into(),
            application: "archbuilder tests".into(),
            time_unix_ms: 0,
        };
        let product = archbuilder::product(Arc::new(arc_command::FixedClock(0)), owner).unwrap();
        let mut next = 90;
        let mut app = Application::new(
            product,
            AppConfig {
                data_dir: dir.clone(),
                limits: Limits::DEFAULT,
                id_generators: Box::new(move || {
                    next += 1;
                    IdGenerator::seeded(next)
                }),
                recent: RecentFiles::new(4),
            },
        );
        app.new_document_in("en").unwrap();
        let shell = Shell::new(&app, archbuilder::config(1).unwrap(), None);
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let localizer = archbuilder::localizer(&["en-US".to_owned()]).unwrap();
        let mut ui = Self {
            app,
            shell,
            ctx,
            localizer,
            _dir: TempDir(dir),
        };
        ui.frame(Vec::new(), Vec::new());
        ui.frame(Vec::new(), Vec::new());
        ui
    }

    fn frame(&mut self, events: Vec<egui::Event>, menu: Vec<Intent>) -> Vec<accesskit::Node> {
        let textures = BTreeMap::new();
        let mut out = self.shell.frame(
            &mut self.app,
            &self.ctx,
            FrameInput {
                raw: egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1280.0, 800.0),
                    )),
                    events,
                    ..Default::default()
                },
                time: 1.0,
                localize: &self.localizer,
                language: "en",
                textures: &textures,
                menu_intents: menu,
                native_menu: false,
            },
        );
        out.full.textures_delta.clear();
        out.full
            .platform_output
            .accesskit_update
            .map(|u| u.nodes.into_iter().map(|(_, n)| n).collect())
            .unwrap_or_default()
    }

    /// Focuses the command line and types `text` into it, without Enter.
    fn type_text(&mut self, text: &str) {
        let nodes = self.frame(Vec::new(), Vec::new());
        let input = nodes
            .iter()
            .filter(|n| n.role() == accesskit::Role::TextInput)
            .max_by(|a, b| {
                let y = |n: &accesskit::Node| n.bounds().map_or(0.0, |b| b.y0);
                y(a).total_cmp(&y(b))
            })
            .unwrap();
        self.frame(click_at(center(input)), Vec::new());
        self.frame(vec![egui::Event::Text(text.into())], Vec::new());
        self.frame(Vec::new(), Vec::new());
        self.frame(Vec::new(), Vec::new());
    }

    fn key(&mut self, key: egui::Key) {
        let press = |pressed| egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        self.frame(vec![press(true), press(false)], Vec::new());
        self.frame(Vec::new(), Vec::new());
    }

    fn line(&mut self, text: &str) {
        self.type_text(text);
        self.key(egui::Key::Enter);
    }

    fn menu(&mut self, command: &str) {
        self.frame(Vec::new(), vec![Intent::new(command)]);
        self.frame(Vec::new(), Vec::new());
    }

    /// A primary click in the middle of the viewport.
    fn click_viewport(&mut self) {
        self.frame(click_at(egui::pos2(300.0, 420.0)), Vec::new());
        self.frame(Vec::new(), Vec::new());
    }

    fn walls(&self) -> usize {
        let tab = self.app.active().unwrap();
        let snapshot = self.app.tab(tab).unwrap().document().snapshot();
        spatial::objects(&snapshot, WALL).len()
    }

    /// The label beside the command line's input: the prompt asked, else `Command`.
    fn prompt(&mut self) -> String {
        let nodes = self.frame(Vec::new(), Vec::new());
        let input = nodes
            .iter()
            .filter(|n| n.role() == accesskit::Role::TextInput)
            .filter_map(|n| n.bounds())
            .max_by(|a, b| a.y0.total_cmp(&b.y0))
            .unwrap();
        nodes
            .iter()
            .filter(|n| n.role() == accesskit::Role::Label)
            .filter(|n| {
                n.bounds().is_some_and(|b| {
                    b.x1 <= input.x0 + 1.0 && b.y0 >= input.y0 - 8.0 && b.y0 <= input.y1
                })
            })
            .filter_map(|n| n.label().or_else(|| n.value()))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// A rectangle of four walls, typed, ending the chain with Escape.
    fn draw_walls(&mut self) {
        self.line("wall");
        for point in ["0,0", "4,0", "4,3", "0,3", "0,0"] {
            self.line(point);
        }
        self.key(egui::Key::Escape);
        assert_eq!(self.walls(), 4);
    }
}

fn center(node: &accesskit::Node) -> egui::Pos2 {
    let b = node.bounds().unwrap();
    egui::pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
}

fn click_at(at: egui::Pos2) -> Vec<egui::Event> {
    let button = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    vec![egui::Event::PointerMoved(at), button(true), button(false)]
}

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
fn enter_right_after_a_typed_instant_command_repeats_the_last_command() {
    let mut ui = Ui::new("ze");
    ui.draw_walls();
    // The wall command is the last one: ZE is done at once, so Enter repeats the wall command.
    ui.type_text("ze");
    ui.key(egui::Key::Enter);
    let prompt = ui.prompt();
    assert!(prompt.contains("specify the start point"), "{prompt}");
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
