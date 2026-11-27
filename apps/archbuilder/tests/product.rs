//! The application product, headless: a new document starts with its project (no undo step), saves
//! through the application's own path and opens again with the same structure; each level draws
//! its outline and has a label with its name and elevation.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::sync::Arc;

use arc_app::application::{AppConfig, Application};
use arc_app::recent::RecentFiles;
use arc_base::{Cancel, IdGenerator, Limits};
use arc_command::FixedClock;
use arc_doc::package::LockOwner;
use arc_gpu::extract::Extractor;
use archbuilder_model::spatial;
use archbuilder_model::types::{LEVEL, text};

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

/// The names of the project and of the storeys, bottom up, with each level's elevation.
fn structure(app: &Application, tab: arc_app::application::TabId) -> (String, Vec<(String, f64)>) {
    let snapshot = app.tab(tab).unwrap().document().snapshot();
    let project = spatial::project(&snapshot).unwrap();
    let building = spatial::buildings(&snapshot)[0].id;
    let storeys = spatial::storeys(&snapshot, building)
        .iter()
        .map(|s| (text(&s.storey.values, 0).unwrap().to_owned(), s.elevation()))
        .collect();
    (text(&project.values, 0).unwrap().to_owned(), storeys)
}

#[test]
fn a_new_document_starts_with_its_project_no_undo_step_and_saves_and_opens_again() {
    let dir = std::env::temp_dir().join(format!("archbuilder-app-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut first = app(&dir, 0);
    let tab = first.new_document_in("tr").unwrap();
    let before = structure(&first, tab);
    assert_eq!(before.0, "Adsız Proje", "named in the interface language");
    assert_eq!(before.1.len(), 3);
    // The project is the document's starting content: clean, and nothing for undo to remove.
    assert!(!first.tab(tab).unwrap().is_dirty());
    assert!(!first.undo(tab).unwrap());
    assert_eq!(structure(&first, tab), before);
    let file = dir.join("house.archb");
    first.save_as(tab, &file).unwrap();
    assert!(!first.tab(tab).unwrap().is_dirty());
    first
        .close(tab, |_| arc_app::application::CloseAnswer::Discard)
        .unwrap();

    let mut second = app(&dir, 100);
    let reopened = second.open(&file).unwrap();
    assert_eq!(structure(&second, reopened), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn each_level_draws_an_outline_and_labels_carry_name_and_elevation_in_the_unit_and_language() {
    let dir = std::env::temp_dir().join(format!("archbuilder-draw-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut app = app(&dir, 200);
    let tab = app.new_document().unwrap();
    let product = app.product();
    let extractor = *product.extractors()[0]
        .factory
        .create()
        .downcast::<Extractor>()
        .unwrap();
    let snapshot = app.tab(tab).unwrap().document().snapshot();
    let levels = spatial::objects(&snapshot, LEVEL);
    assert_eq!(levels.len(), 3);
    for level in &levels {
        let extraction = extractor(&snapshot, level, &Cancel::new()).unwrap();
        assert_eq!(
            extraction.items.len(),
            1,
            "the outline; the label faces the screen"
        );
    }
    use archbuilder_model::units::LengthUnit;
    let floor1 = levels
        .iter()
        .find(|l| text(&l.values, 0) == Some("Floor 1"))
        .unwrap();
    assert_eq!(
        archbuilder::labels::label(floor1, LengthUnit::Meter, "en"),
        "Floor 1  3 m"
    );
    assert_eq!(
        archbuilder::labels::label(floor1, LengthUnit::Millimeter, "tr"),
        "Floor 1  3000 mm"
    );
    // Seen from above every level projects to one place: the labels come highest first.
    let overlays = archbuilder::labels::of_snapshot(&snapshot, "tr");
    assert_eq!(overlays.len(), 3);
    let _ = std::fs::remove_dir_all(&dir);
}

use arc_app::shell::ToolItem;

/// Every item of the command tabs and of the quick access row.
fn all_items() -> Vec<ToolItem> {
    let mut items = archbuilder::chrome::quick_access();
    for tab in archbuilder::chrome::command_tabs() {
        items.extend(tab.groups.into_iter().flat_map(|g| g.items));
    }
    items
}

#[test]
fn every_label_of_the_toolbar_and_the_commands_is_in_both_catalogs() {
    let mut ids: Vec<String> = Vec::new();
    for item in all_items() {
        match item {
            ToolItem::Button { label, hint, .. } | ToolItem::IconButton { label, hint, .. } => {
                ids.push(label);
                ids.extend(hint);
            }
            ToolItem::ZoomExtents { label } => ids.push(label),
            ToolItem::ZoomExtentsIcon { label, hint, .. } => {
                ids.push(label);
                ids.extend(hint);
            }
            _ => {}
        }
    }
    for tab in archbuilder::chrome::command_tabs() {
        ids.push(tab.label);
        ids.extend(tab.groups.into_iter().map(|g| g.label));
    }
    ids.extend(
        [
            "ab-status-storey",
            "ab-status-selected",
            "ab-panel-outliner",
        ]
        .map(String::from),
    );
    let mut registry = archbuilder_commands::registry().unwrap();
    archbuilder_io::register(&mut registry).unwrap();
    for spec in registry.iter() {
        ids.push(spec.label().to_owned());
        ids.push(spec.description().to_owned());
    }
    ids.extend(
        [
            "ab-menu-export",
            "ab-notice-ifc-exported",
            "ab-notice-ifc-loss",
        ]
        .map(String::from),
    );
    for language in ["en-US", "tr-TR"] {
        let localizer = archbuilder::localizer(&[language.to_owned()]).unwrap();
        for id in &ids {
            let text = localizer.format(id, &[]);
            assert_eq!(
                text.locale.as_ref().map(ToString::to_string).as_deref(),
                Some(language),
                "{id} in {language}"
            );
        }
    }
}

/// File > Export > IFC follows Save As, and the command it names is one of the product's.
#[test]
fn the_file_menu_has_export_ifc_after_save_as() {
    use arc_ui::MenuItem;
    let config = archbuilder::config(0).unwrap();
    let Some(MenuItem::Submenu { items, .. }) = config.menu.items().first() else {
        panic!("no File menu");
    };
    let save_as = items
        .iter()
        .position(|i| matches!(i, MenuItem::Command { command } if command == arc_app::commands::FILE_SAVE_AS))
        .unwrap();
    let MenuItem::Submenu {
        label,
        items: export,
    } = &items[save_as + 1]
    else {
        panic!("no Export submenu");
    };
    assert_eq!(label, "ab-menu-export");
    assert_eq!(
        export,
        &[
            MenuItem::Command {
                command: "archbuilder.file.export_ifc".into()
            },
            MenuItem::Command {
                command: "archbuilder.file.export_mesh".into()
            },
            MenuItem::Command {
                command: "archbuilder.shortcuts.export".into()
            }
        ]
    );
    // The shortcut table comes back through File > Import.
    let MenuItem::Submenu {
        label,
        items: import,
    } = &items[save_as + 2]
    else {
        panic!("no Import submenu");
    };
    assert_eq!(label, "ab-menu-import");
    assert_eq!(
        import,
        &[MenuItem::Command {
            command: "archbuilder.shortcuts.import".into()
        }]
    );
    let owner = LockOwner {
        user: "t".into(),
        host: "t".into(),
        application: "t".into(),
        time_unix_ms: 0,
    };
    let product = archbuilder::product(Arc::new(FixedClock(0)), owner).unwrap();
    assert!(
        product
            .headless()
            .registry
            .get("archbuilder.file.export_ifc")
            .is_some()
    );
    // Without a path the command asks for one in a save dialog for IFC files.
    let dialog = product.path_dialog("archbuilder.file.export_ifc").unwrap();
    assert_eq!(dialog.mode, arc_app::product::PathMode::Save);
    assert_eq!(dialog.filters[0].extensions, ["ifc"]);
    assert_eq!(dialog.default_name.as_deref(), Some("{document}.ifc"));
    // Export mesh asks in a save dialog whose extension names the format.
    let dialog = product.path_dialog("archbuilder.file.export_mesh").unwrap();
    let extensions: Vec<_> = dialog.filters.iter().flat_map(|f| &f.extensions).collect();
    assert_eq!(extensions, ["obj", "stl", "gltf", "glb"]);
    assert!(
        product
            .headless()
            .registry
            .get("archbuilder.file.export_mesh")
            .is_some()
    );
}

#[test]
fn every_line_of_the_command_tabs_names_a_command_and_every_icon_exists() {
    let mut registry = archbuilder_commands::registry().unwrap();
    archbuilder_io::register(&mut registry).unwrap();
    arc_app::commands::register(&mut registry).unwrap();
    let icons = archbuilder::chrome::icons();
    arc_app::shell::check_icons(&icons).unwrap();
    let mut lines = Vec::new();
    for item in all_items() {
        match item {
            ToolItem::IconButton { line, icon, .. } => {
                assert!(
                    registry.get(&line).is_some()
                        || archbuilder::chrome::TYPED_WORDS.contains(&line.as_str()),
                    "{line} is no command"
                );
                assert!(icons.iter().any(|(n, _)| *n == icon), "no icon {icon}");
                lines.push(line);
            }
            ToolItem::ZoomExtentsIcon { icon, .. } => {
                assert!(icons.iter().any(|(n, _)| *n == icon), "no icon {icon}");
            }
            ToolItem::Button { .. } => panic!("a text button among the command tabs"),
            _ => {}
        }
    }
    // Everything the flat toolbar offered stays reachable.
    use archbuilder_commands::{edit, modify};
    let old = [
        arc_app::commands::FILE_NEW,
        arc_app::commands::FILE_OPEN,
        arc_app::commands::FILE_SAVE,
        arc_app::commands::FILE_SAVE_AS,
        arc_app::commands::EDIT_UNDO,
        arc_app::commands::EDIT_REDO,
        archbuilder_commands::structure::STOREY_ADD,
        archbuilder_commands::structure::STOREY_DELETE,
        archbuilder_commands::wall::WALL_CREATE,
        archbuilder_commands::insert::DOOR_PLACE,
        archbuilder_commands::insert::WINDOW_PLACE,
        archbuilder_commands::floor::FLOOR_CREATE,
        archbuilder_commands::room::ROOM_PLACE,
        archbuilder_commands::room::ROOM_PLACE_ALL,
        archbuilder_commands::stair::STAIR_CREATE,
        archbuilder_commands::roof::ROOF_CREATE,
        archbuilder_commands::ceiling::CEILING_CREATE,
        archbuilder_commands::insert::OPENING_PLACE,
        archbuilder_commands::opening::SHAFT_CREATE,
        archbuilder_commands::opening::VERTICAL_OPENING_CREATE,
        modify::MODIFY_MOVE,
        modify::MODIFY_COPY,
        modify::MODIFY_ROTATE,
        modify::MODIFY_MIRROR,
        edit::MODIFY_ALIGN,
        edit::MODIFY_OFFSET,
        edit::MODIFY_TRIM,
        edit::MODIFY_TRIM_TO,
        edit::WALL_SPLIT,
        edit::WALL_MERGE,
        edit::MODIFY_ARRAY,
        edit::MODIFY_ARRAY_RADIAL,
        edit::ELEMENT_PIN,
        edit::ELEMENT_UNPIN,
        archbuilder_commands::wall::OBJECT_DELETE,
        archbuilder_io::command::EXPORT_IFC,
    ];
    for line in old {
        assert!(lines.iter().any(|l| l == line), "{line} unreachable");
    }
    let items = all_items();
    assert!(
        items
            .iter()
            .any(|i| matches!(i, ToolItem::ZoomExtentsIcon { .. }))
    );
    assert!(items.iter().any(|i| matches!(i, ToolItem::Choice { .. })));
}

#[test]
fn the_selection_tab_shows_only_with_a_selection() {
    let dir = std::env::temp_dir().join(format!("archbuilder-tabs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut app = app(&dir, 0);
    let tab = app.new_document_in("en").unwrap();
    let selection_tab = archbuilder::chrome::command_tabs()
        .into_iter()
        .find(|t| t.context.is_some())
        .unwrap();
    let shown = |app: &Application| (selection_tab.context.as_ref().unwrap().0)(app, tab);
    // The controller reports the selection through the work-plane probe, each frame.
    let _ = archbuilder::interaction::work_plane(&app, tab, &[]);
    assert!(!shown(&app));
    let storey = {
        let snapshot = app.tab(tab).unwrap().document().snapshot();
        spatial::objects(&snapshot, archbuilder_model::types::STOREY)[0].id
    };
    let _ = archbuilder::interaction::work_plane(&app, tab, &[storey]);
    assert!(shown(&app));
    let _ = archbuilder::interaction::work_plane(&app, tab, &[]);
    assert!(!shown(&app));
    let _ = std::fs::remove_dir_all(&dir);
}
