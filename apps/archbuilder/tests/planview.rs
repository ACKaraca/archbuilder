//! Plan views in the application, through the windowed shell without a window: a document saved
//! before views existed opens unmodified; a plan view opens straight down and gives the 3D camera
//! back; temporary hide and isolate filter its drawing without touching the document, and only
//! Hide in view is a document edit.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    reason = "tests; pixel positions fit f32"
)]

use std::collections::BTreeMap;
use std::sync::Arc;

use arc_app::shell::{FrameInput, Shell};
use arc_base::ObjectId;
use arc_ui::Intent;
use arc_ui::egui;
use archbuilder::plan_state;
use archbuilder::shortcuts::ui;
use archbuilder_model::types::WALL;

use arc_app::application::{AppConfig, Application};
use arc_app::recent::RecentFiles;
use arc_base::{IdGenerator, Limits};
use arc_command::{FixedClock, Invocation, Invoker, Permission, Session, Surface};
use arc_doc::package::LockOwner;
use archbuilder_model::spatial;
use archbuilder_model::types::PLAN_VIEW;

fn app(dir: &std::path::Path, seed: u64) -> Application {
    let owner = LockOwner {
        user: "test".into(),
        host: "test".into(),
        application: "archbuilder tests".into(),
        time_unix_ms: 0,
    };
    let product = archbuilder::product(Arc::new(FixedClock(0)), owner).unwrap();
    let mut next = seed;
    Application::new(
        product,
        AppConfig {
            data_dir: dir.to_path_buf(),
            limits: Limits::DEFAULT,
            id_generators: Box::new(move || {
                next += 1;
                IdGenerator::seeded(next)
            }),
            recent: RecentFiles::new(4),
        },
    )
}

struct Ui {
    app: Application,
    shell: Shell,
    ctx: egui::Context,
    localizer: arc_app::locale::Localizer,
}

impl Ui {
    fn new(app: Application) -> Self {
        let shell = Shell::new(&app, archbuilder::config(1).unwrap(), None);
        let mut ui = Self {
            app,
            shell,
            ctx: egui::Context::default(),
            localizer: archbuilder::localizer(&["en-US".to_owned()]).unwrap(),
        };
        for _ in 0..3 {
            ui.frame(Vec::new());
        }
        ui
    }

    fn frame(&mut self, menu: Vec<Intent>) {
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
    }

    fn menu(&mut self, intent: Intent) {
        self.frame(vec![intent]);
        self.frame(Vec::new());
    }

    fn select(&mut self, ids: &[ObjectId]) {
        self.menu(Intent::new(arc_app::commands::SELECT_SET).with(
            "objects",
            ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ));
    }

    fn tab(&self) -> arc_app::application::TabId {
        self.app.active().unwrap()
    }

    fn snapshot(&self) -> arc_doc::Snapshot {
        self.app.tab(self.tab()).unwrap().document().snapshot()
    }

    fn camera(&self) -> arc_geom::view::ViewFrame {
        *self.shell.view(self.tab()).unwrap().camera()
    }
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "archbuilder-planview-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_file_saved_before_views_existed_opens_unmodified_and_its_default_views_first_edit_is_one_undo_step()
 {
    let dir = temp_dir("old");
    let mut first = app(&dir, 0);
    let tab = first.new_document().unwrap();
    let doc = first.tab(tab).unwrap().document();
    // A new document has its views and no undo step.
    assert_eq!(spatial::objects(&doc.snapshot(), PLAN_VIEW).len(), 3);
    assert!(!first.undo(tab).unwrap());
    // Make the file an old one: no views.
    let ids: Vec<_> = spatial::objects(&doc.snapshot(), PLAN_VIEW)
        .iter()
        .map(|v| v.id)
        .collect();
    doc.submit(move |txn| {
        for id in ids {
            txn.delete(id)?;
        }
        Ok(())
    })
    .wait()
    .unwrap();
    // One wall, so the default view has something to draw.
    let core = first.product().headless();
    let session = Session::new(Surface::Ui, [Permission::Read, Permission::Edit]);
    Invoker::new(&core.registry)
        .invoke(
            doc,
            &session,
            Invocation {
                seed: 2,
                ..Invocation::new(
                    "archbuilder.wall.create",
                    serde_json::json!({"start": [0.0, 0.0], "end": [4.0, 0.0]}),
                )
            },
        )
        .unwrap();
    let file = dir.join("old.archb");
    first.save_as(tab, &file).unwrap();
    first
        .close(tab, |_| arc_app::application::CloseAnswer::Discard)
        .unwrap();

    let mut second = app(&dir, 100);
    second.open(&file).unwrap();
    let mut ui = Ui::new(second);
    for _ in 0..5 {
        ui.frame(Vec::new());
    }
    assert!(spatial::objects(&ui.snapshot(), PLAN_VIEW).is_empty());
    assert!(!ui.app.tab(ui.tab()).unwrap().is_dirty());
    // Opening a storey's plan shows its default view, which the document does not hold: no
    // record, no undo step, no modified mark, and it draws.
    let snapshot = ui.snapshot();
    let ground = spatial::storeys(&snapshot, spatial::buildings(&snapshot)[0].id)[0]
        .storey
        .id;
    let revision = snapshot.revision();
    ui.menu(Intent::new(ui::OPEN_VIEW).with("storey", ground.to_string()));
    assert!(archbuilder_model::view::of_storey(&ui.snapshot(), ground).is_empty());
    assert_eq!(ui.snapshot().revision(), revision);
    assert!(!ui.app.tab(ui.tab()).unwrap().is_dirty());
    assert_eq!(plan_state::current(ui.tab()), Some(ground));
    let storey = spatial::get(&ui.snapshot(), ground).unwrap();
    let drawn =
        archbuilder::extract_view::extract_view(&ui.snapshot(), &storey, &arc_base::Cancel::new())
            .unwrap();
    assert!(!drawn.items.is_empty());

    // The first edit of its settings (what the property grid sends for it) makes the view, in one
    // undo step; the tab shows that view.
    ui.menu(
        Intent::new(archbuilder_commands::view::VIEW_SET)
            .with("view", ground.to_string())
            .with("field", "cut")
            .with("value", 1.6),
    );
    let views = archbuilder_model::view::of_storey(&ui.snapshot(), ground);
    assert_eq!(views.len(), 1);
    assert_eq!(
        plan_state::current_in(ui.tab(), &ui.snapshot()),
        Some(views[0].id)
    );
    assert!(ui.app.tab(ui.tab()).unwrap().is_dirty());
    // Undo: the default view again, nothing held, and the file is not modified.
    ui.menu(Intent::new(arc_app::commands::EDIT_UNDO));
    assert!(archbuilder_model::view::of_storey(&ui.snapshot(), ground).is_empty());
    assert_eq!(
        plan_state::current_in(ui.tab(), &ui.snapshot()),
        Some(ground)
    );
    assert!(!ui.app.tab(ui.tab()).unwrap().is_dirty());
    plan_state::set(ui.tab(), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_plan_view_opens_straight_down_hides_temporarily_and_gives_the_3d_camera_back() {
    let dir = temp_dir("hide");
    let mut app = app(&dir, 40);
    let tab = app.new_document().unwrap();
    let document = app.tab(tab).unwrap().document();
    let core = app.product().headless();
    let session = Session::new(Surface::Ui, [Permission::Read, Permission::Edit]);
    let corners = [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]];
    for k in 0..4 {
        Invoker::new(&core.registry)
            .invoke(
                document,
                &session,
                Invocation {
                    seed: 3,
                    ..Invocation::new(
                        "archbuilder.wall.create",
                        serde_json::json!({"start": corners[k], "end": corners[(k + 1) % 4]}),
                    )
                },
            )
            .unwrap();
    }
    let mut ui = Ui::new(app);
    let snapshot = ui.snapshot();
    let walls: Vec<ObjectId> = spatial::objects(&snapshot, WALL)
        .iter()
        .map(|w| w.id)
        .collect();
    let ground = spatial::get(&snapshot, walls[0])
        .unwrap()
        .header
        .owner
        .unwrap();
    let view = archbuilder_model::view::of_storey(&snapshot, ground)[0].id;
    let camera_3d = ui.camera();
    // A view no tab shows is not drawn: its extraction is empty until it opens.
    let drawing = |ui: &Ui| {
        let object = spatial::get(&ui.snapshot(), view).unwrap();
        archbuilder::extract_view::extract_view(&ui.snapshot(), &object, &arc_base::Cancel::new())
            .unwrap()
            .items
            .len()
    };
    assert_eq!(drawing(&ui), 0);

    ui.menu(Intent::new(ui::OPEN_VIEW).with("view", view.to_string()));
    assert!(drawing(&ui) > 0);
    assert_eq!(plan_state::current(ui.tab()), Some(view));
    let plan = ui.camera();
    assert!((plan.forward() - arc_base::math::Vector3::new(0.0, 0.0, -1.0)).norm() < 1e-9);
    // Framed on the storey's walls: the 10 m by 6 m house, not far beyond.
    let arc_geom::view::Projection::Orthographic { height } = plan.projection() else {
        panic!("not orthographic");
    };
    assert!((6.0..15.0).contains(&height), "{height}");

    // Hide, isolate and hide by kind are temporary: no document change, the drawing's filter.
    let revision = ui.snapshot().revision();
    ui.select(&walls[..1]);
    ui.menu(Intent::new(ui::HIDE));
    assert_eq!(ui.snapshot().revision(), revision);
    assert!(plan_state::filter(view).hidden.contains(&walls[0]));
    ui.select(&walls[1..2]);
    ui.menu(Intent::new(ui::ISOLATE));
    assert_eq!(ui.snapshot().revision(), revision);
    assert_eq!(
        plan_state::filter(view).only,
        Some([walls[1]].into_iter().collect())
    );
    ui.menu(Intent::new(ui::RESET_VISIBILITY));
    ui.select(&walls[2..3]);
    ui.menu(Intent::new(ui::HIDE_KIND));
    assert_eq!(ui.snapshot().revision(), revision);
    assert!(
        walls
            .iter()
            .all(|w| plan_state::filter(view).hidden.contains(w))
    );
    ui.menu(Intent::new(ui::RESET_VISIBILITY));
    assert!(plan_state::filter(view).is_empty());

    // Hide in view is the view's own list: a document edit, which Unhide all leaves alone.
    ui.select(&walls[3..4]);
    ui.menu(Intent::new(ui::HIDE_IN_VIEW));
    assert_ne!(ui.snapshot().revision(), revision);
    let hidden = |ui: &Ui| {
        archbuilder_model::view::PlanView::from_values(
            &spatial::get(&ui.snapshot(), view).unwrap().values,
        )
        .unwrap()
        .hidden
    };
    assert_eq!(hidden(&ui), [walls[3]]);
    ui.menu(Intent::new(ui::RESET_VISIBILITY));
    assert_eq!(hidden(&ui), [walls[3]]);

    // The 3D view gets its camera back.
    ui.menu(Intent::new(ui::OPEN_VIEW).with("view", "3d"));
    assert_eq!(plan_state::current(ui.tab()), None);
    assert_eq!(ui.camera(), camera_3d);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_view_an_undo_removes_while_it_is_shown_gives_the_3d_view_and_camera_back() {
    let dir = temp_dir("undo");
    let mut app = app(&dir, 70);
    let tab = app.new_document().unwrap();
    // A wall, so the plan's framing differs from the 3D camera.
    let core = app.product().headless();
    let session = Session::new(Surface::Ui, [Permission::Read, Permission::Edit]);
    Invoker::new(&core.registry)
        .invoke(
            app.tab(tab).unwrap().document(),
            &session,
            Invocation {
                seed: 4,
                ..Invocation::new(
                    "archbuilder.wall.create",
                    serde_json::json!({"start": [0.0, 0.0], "end": [6.0, 0.0]}),
                )
            },
        )
        .unwrap();
    let mut ui = Ui::new(app);
    let snapshot = ui.snapshot();
    let ground = spatial::storeys(&snapshot, spatial::buildings(&snapshot)[0].id)[0]
        .storey
        .id;
    let view = archbuilder_model::view::of_storey(&snapshot, ground)[0].id;
    let camera_3d = ui.camera();
    ui.menu(Intent::new("archbuilder.view.duplicate").with("view", view.to_string()));
    let copy = archbuilder_model::view::of_storey(&ui.snapshot(), ground)
        .into_iter()
        .map(|v| v.id)
        .find(|id| *id != view)
        .unwrap();
    ui.menu(Intent::new(ui::OPEN_VIEW).with("view", copy.to_string()));
    assert_eq!(plan_state::current(ui.tab()), Some(copy));
    assert_ne!(ui.camera(), camera_3d);
    ui.menu(Intent::new(arc_app::commands::EDIT_UNDO));
    ui.frame(Vec::new());
    assert!(spatial::get(&ui.snapshot(), copy).is_none());
    assert_eq!(plan_state::current(ui.tab()), None);
    assert_eq!(ui.camera(), camera_3d);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Builds the two-storey house of the manual checks and saves it where `ARCHBUILDER_SCENE` says:
/// `ARCHBUILDER_SCENE=/tmp/house.archb cargo test -p archbuilder --test planview -- --ignored`.
#[test]
#[ignore = "builds a scene for manual checks"]
fn builds_the_two_storey_house() {
    let Ok(path) = std::env::var("ARCHBUILDER_SCENE") else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("archbuilder-scene-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut app = app(&dir, 7);
    let tab = app.new_document().unwrap();
    let document = app.tab(tab).unwrap().document();
    let core = app.product().headless();
    let session = Session::new(Surface::Ui, [Permission::Read, Permission::Edit]);
    let run = |command: &str, params: serde_json::Value| {
        Invoker::new(&core.registry)
            .invoke(
                document,
                &session,
                Invocation {
                    seed: 5,
                    ..Invocation::new(command, params)
                },
            )
            .unwrap()
            .value
    };
    let snapshot = document.snapshot();
    let storeys = spatial::storeys(&snapshot, spatial::buildings(&snapshot)[0].id);
    let (ground, upper) = (
        storeys[0].storey.id.to_string(),
        storeys[1].storey.id.to_string(),
    );
    run(
        "archbuilder.storey.delete",
        serde_json::json!({"storey": storeys[2].storey.id.to_string()}),
    );
    let mut ids = Vec::new();
    for storey in [&ground, &upper] {
        let c = [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]];
        let mut walls = Vec::new();
        for k in 0..4 {
            let made = run(
                "archbuilder.wall.create",
                serde_json::json!({"start": c[k], "end": c[(k + 1) % 4], "height": 3.0,
                    "storey": storey, "type": "Exterior masonry cavity"}),
            );
            walls.push(made["wall"].as_str().unwrap().to_owned());
        }
        let part = run(
            "archbuilder.wall.create",
            serde_json::json!({"start": [5.0, 0.0], "end": [5.0, 6.0], "height": 3.0, "storey": storey}),
        );
        walls.push(part["wall"].as_str().unwrap().to_owned());
        ids.push(walls);
    }
    for (k, walls) in ids.iter().enumerate() {
        if k == 0 {
            run(
                "archbuilder.door.place",
                serde_json::json!({"wall": walls[0], "distance": 2.0}),
            );
            run(
                "archbuilder.door.place",
                serde_json::json!({"wall": walls[4], "distance": 3.0}),
            );
        }
        run(
            "archbuilder.window.place",
            serde_json::json!({"wall": walls[0], "distance": 7.5}),
        );
        run(
            "archbuilder.window.place",
            serde_json::json!({"wall": walls[2], "distance": 2.5}),
        );
        run(
            "archbuilder.window.place",
            serde_json::json!({"wall": walls[2], "distance": 7.5}),
        );
    }
    run(
        "archbuilder.stair.create",
        serde_json::json!({"start": [1.0, 4.5], "to": [4.0, 4.5], "storey": ground}),
    );
    for storey in [&ground, &upper] {
        run(
            "archbuilder.floor.create",
            serde_json::json!({"storey": storey, "points": [[-0.3, -0.3], [10.3, -0.3], [10.3, 6.3], [-0.3, 6.3]]}),
        );
        run(
            "archbuilder.room.place_all",
            serde_json::json!({"storey": storey}),
        );
    }
    run(
        "archbuilder.roof.create",
        serde_json::json!({"picks": [[7.5, 3.0]], "storey": upper, "base_offset": 3.0}),
    );
    // A double-height wall of the ground storey beside the house, seen in the upper plan too.
    run(
        "archbuilder.wall.create",
        serde_json::json!({"start": [-2.0, 0.0], "end": [-2.0, 6.0], "height": 6.0, "storey": ground}),
    );
    // A section across both rooms, looking west, and the four elevations.
    run(
        "archbuilder.view.create_section",
        serde_json::json!({"start": [7.5, -3.0], "end": [7.5, 9.0]}),
    );
    run("archbuilder.view.create_elevations", serde_json::json!({}));
    app.save_as(tab, std::path::Path::new(&path)).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_section_opens_facing_its_drawing_and_a_saved_3d_view_keeps_its_camera_and_style() {
    let dir = temp_dir("section");
    let mut app = app(&dir, 60);
    let tab = app.new_document().unwrap();
    let document = app.tab(tab).unwrap().document();
    let core = app.product().headless();
    let session = Session::new(Surface::Ui, [Permission::Read, Permission::Edit]);
    let run = |command: &str, params: serde_json::Value| {
        Invoker::new(&core.registry)
            .invoke(
                document,
                &session,
                Invocation {
                    seed: 9,
                    ..Invocation::new(command, params)
                },
            )
            .unwrap()
            .value
    };
    let corners = [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]];
    for k in 0..4 {
        run(
            "archbuilder.wall.create",
            serde_json::json!({"start": corners[k], "end": corners[(k + 1) % 4]}),
        );
    }
    let id = |v: serde_json::Value| v["view"].as_str().unwrap().parse::<ObjectId>().unwrap();
    let section = id(run(
        "archbuilder.view.create_section",
        serde_json::json!({"start": [5.0, -2.0], "end": [5.0, 8.0]}),
    ));
    let saved = id(run(
        "archbuilder.view.create_3d",
        serde_json::json!({"eye": [20.0, -20.0, 12.0], "target": [5.0, 3.0, 1.0]}),
    ));
    let mut ui = Ui::new(app);
    let camera_3d = ui.camera();
    let drawing = |ui: &Ui| {
        let object = spatial::get(&ui.snapshot(), section).unwrap();
        archbuilder::extract_model_view::extract(&ui.snapshot(), &object, &arc_base::Cancel::new())
            .unwrap()
            .items
            .len()
    };
    assert_eq!(drawing(&ui), 0);

    // The section looks west, level, at its drawing, which is built once it is open.
    ui.menu(Intent::new(ui::OPEN_VIEW).with("view", section.to_string()));
    assert_eq!(plan_state::current(ui.tab()), Some(section));
    assert!(drawing(&ui) > 0);
    let facing = ui.camera();
    assert!((facing.forward() - arc_base::math::Vector3::new(-1.0, 0.0, 0.0)).norm() < 1e-9);
    assert!(matches!(
        facing.projection(),
        arc_geom::view::Projection::Orthographic { .. }
    ));

    // A saved 3D view opens with its camera; a visual style is its own, as an undo step.
    ui.menu(Intent::new(ui::OPEN_VIEW).with("view", saved.to_string()));
    let eye = ui.camera().eye();
    assert!((eye - arc_base::math::Point3::new(20.0, -20.0, 12.0)).norm() < 1e-6);
    let revision = ui.snapshot().revision();
    ui.menu(Intent::new(ui::VIEW_STYLE).with("style", "hidden_line"));
    assert_ne!(ui.snapshot().revision(), revision);
    let style = archbuilder_model::model_view::settings(&ui.snapshot(), saved)
        .unwrap()
        .style;
    assert_eq!(
        style,
        archbuilder_model::model_view::VisualStyle::HiddenLine
    );

    // The default 3D view gets its camera back.
    ui.menu(Intent::new(ui::OPEN_VIEW).with("view", "3d"));
    assert_eq!(plan_state::current(ui.tab()), None);
    assert_eq!(ui.camera(), camera_3d);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_plan_and_3d_words_open_the_plan_of_the_active_storey_and_the_3d_view() {
    let dir = temp_dir("words");
    let mut app = app(&dir, 90);
    let tab = app.new_document().unwrap();
    let document = app.tab(tab).unwrap().document();
    let core = app.product().headless();
    let session = Session::new(Surface::Ui, [Permission::Read, Permission::Edit]);
    Invoker::new(&core.registry)
        .invoke(
            document,
            &session,
            Invocation {
                seed: 3,
                ..Invocation::new(
                    "archbuilder.wall.create",
                    serde_json::json!({"start": [0.0, 0.0], "end": [5.0, 0.0]}),
                )
            },
        )
        .unwrap();
    let mut ui = Ui::new(app);
    let snapshot = ui.snapshot();
    let wall = spatial::objects(&snapshot, WALL)[0].id;
    let ground = spatial::get(&snapshot, wall).unwrap().header.owner.unwrap();
    let view = archbuilder_model::view::of_storey(&snapshot, ground)[0].id;

    ui.select(&[wall]);
    ui.menu(Intent::new(ui::OPEN_VIEW).with("view", "plan"));
    assert_eq!(plan_state::current(ui.tab()), Some(view));
    // Asked again in the plan view, it stays on the view's storey.
    ui.menu(Intent::new(ui::OPEN_VIEW).with("view", "plan"));
    assert_eq!(plan_state::current(ui.tab()), Some(view));
    ui.menu(Intent::new(ui::OPEN_VIEW).with("view", "3d"));
    assert_eq!(plan_state::current(ui.tab()), None);
    let _ = std::fs::remove_dir_all(&dir);
}
