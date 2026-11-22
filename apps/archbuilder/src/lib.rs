//! The ArchBuilder desktop application (wiring only): the ArchBuilder type set and commands in the
//! windowed shell of `arc-app`. The outliner shows the project, site, building, storeys and the
//! walls of each storey, the property grid edits names, elevations and the fields of a wall
//! through the commands, and the viewport draws each level as a horizontal reference plane, with
//! its name and elevation as a label that faces the screen, and each wall as a shaded solid. The
//! wall tool ([`wall_tool`], [`interaction`]) places chains of walls with the pointer or typed
//! coordinates, the floor tool ([`floor_tool`]) makes floors from picked walls or a sketch, and the insert tool ([`insert_tool`]) places doors and windows in walls. The binary builds this product and starts the event loop; tests drive the same
//! product headless.

use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use arc_app::product::{
    Entry, Extractor, FileType, PathDialog, PathFilter, PathMode, Product, ProductBuilder,
};
use arc_app::settings::{Scope, Setting, SettingType, SettingValue};
use arc_app::shell::{
    self, CommandLinePanel, InitialView, OverlayProbe, ShellConfig, ViewportPanel, WorkPlaneProbe,
};
use arc_base::{Error, Result};
use arc_command::Clock;
use arc_doc::package::LockOwner;
use arc_gpu::extract;
use arc_ui::{Axis, Binding, Dock, DockLayout, Menu, MenuItem};
use archbuilder_commands::floor::{FLOOR_ADD_HOLE, FLOOR_CREATE};
use archbuilder_commands::insert::{
    DOOR_PLACE, INSERT_FLIP_FACING, INSERT_FLIP_HAND, WINDOW_PLACE,
};
use archbuilder_commands::modify::{
    CLIPBOARD_PASTE, CLIPBOARD_PASTE_TO_STOREY, MODIFY_COPY, MODIFY_MIRROR, MODIFY_MOVE,
    MODIFY_ROTATE,
};
use archbuilder_commands::room::{ROOM_PLACE, ROOM_PLACE_ALL};
use archbuilder_commands::structure::{OBJECT_SELECT, PROJECT_NEW, STOREY_ADD, STOREY_DELETE};
use archbuilder_commands::wall::{OBJECT_DELETE, WALL_CREATE};
use archbuilder_model::types::{DOOR, FLOOR, LEVEL, WALL, WINDOW};

pub mod chrome;
pub mod clipboard;
pub mod direct;
pub mod extract_floor;
pub mod extract_insert;
pub mod extract_level;
pub mod extract_wall;
pub mod floor_tool;
pub mod insert_tool;
pub mod interaction;
pub mod labels;
pub mod menu;
pub mod modify_tool;
pub mod outliner;
pub mod picker;
pub mod properties;
pub mod room_tool;
pub mod room_view;
pub mod selection;
pub mod shortcuts;
pub mod snap;
pub mod visibility;
pub mod wall_tool;

/// The viewport panel.
pub const VIEWPORT: &str = "archbuilder.viewport";
/// The property grid.
pub const PROPERTIES: &str = "archbuilder.properties";
/// The outliner.
pub const OUTLINER: &str = "archbuilder.outliner";
/// The command line.
pub const COMMAND_LINE: &str = "archbuilder.command_line";
/// The settings panel.
pub const SETTINGS: &str = "archbuilder.settings";

/// A message of the application.
struct Message {
    message_id: &'static str,
}

const TITLE: Message = Message {
    message_id: "ab-title",
};
const MENU_STRUCTURE: Message = Message {
    message_id: "ab-menu-structure",
};
const MENU_MODIFY: Message = Message {
    message_id: "ab-menu-modify",
};
const SHORTCUTS_EXPORT_TITLE: Message = Message {
    message_id: "ab-shortcuts-export-title",
};
const SHORTCUTS_IMPORT_TITLE: Message = Message {
    message_id: "ab-shortcuts-import-title",
};
const MENU_EXPORT: Message = Message {
    message_id: "ab-menu-export",
};
const MENU_IMPORT: Message = Message {
    message_id: "ab-menu-import",
};

fn entry(id: &str, label: Message, make: fn() -> Box<dyn shell::Panel>) -> Entry {
    Entry {
        id: id.into(),
        message_id: label.message_id.into(),
        factory: Arc::new(make),
    }
}

const EXPORT_IFC_TITLE: Message = Message {
    message_id: "ab-export-ifc-title",
};

/// The save dialog of File > Export > IFC.
fn export_ifc_dialog() -> PathDialog {
    PathDialog {
        mode: PathMode::Save,
        title: EXPORT_IFC_TITLE.message_id.into(),
        filters: vec![PathFilter {
            message_id: "ab-export-ifc-filter".into(),
            extensions: vec!["ifc".into()],
        }],
        default_name: Some("{document}.ifc".into()),
    }
}

/// The dialog of the shortcut file commands: a JSON file.
fn shortcuts_dialog(mode: PathMode, title: &str) -> PathDialog {
    PathDialog {
        mode,
        title: title.into(),
        filters: vec![PathFilter {
            message_id: "ab-shortcuts-filter".into(),
            extensions: vec!["json".into()],
        }],
        default_name: Some("archbuilder-shortcuts.json".into()),
    }
}

/// The settings of the application: the typed shortcuts, snapping and the fine grid step.
fn register_settings(builder: &mut ProductBuilder) -> Result<()> {
    builder.setting(Setting {
        key: shortcuts::SETTING,
        ty: SettingType::Text,
        default: SettingValue::Text(String::new()),
        scope: Scope::User,
        message_id: "ab-setting-shortcuts",
    })?;
    builder.setting(Setting {
        key: interaction::SNAP_SETTING,
        ty: SettingType::Bool,
        default: SettingValue::Bool(true),
        scope: Scope::User,
        message_id: "ab-setting-snap-enabled",
    })?;
    builder.setting(Setting {
        key: interaction::GRID_SETTING,
        ty: SettingType::Int { min: 1, max: 1000 },
        default: SettingValue::Int(10),
        scope: Scope::User,
        message_id: "ab-setting-snap-grid",
    })
}

/// The product of the application: the type set and commands of `archbuilder-commands`, the shell
/// commands and settings, the `archb` package, four panels and the level extractor.
///
/// # Errors
/// The registries' errors; none occur for this set.
#[allow(clippy::too_many_lines, reason = "one registration after another")]
pub fn product(clock: Arc<dyn Clock>, owner: LockOwner) -> Result<Product> {
    let mut builder = ProductBuilder::new("archbuilder", env!("CARGO_PKG_VERSION"))
        .with_types(archbuilder_commands::types()?)
        .with_commands(archbuilder_commands::registry()?);
    arc_app::commands::register(builder.commands_mut())?;
    archbuilder_io::register(builder.commands_mut())?;
    let seed = Arc::new(AtomicU64::new(clock.now_unix_millis()));
    builder.interaction(move || Box::new(interaction::WallInteraction::new(&seed)));
    // A wall's solid derives from the elevation of its storey's level: a change of the level or
    // the storey extracts the storey's walls again.
    builder.extraction_dependents(|snapshot, changes| {
        let mut out = extract_wall::dependents(snapshot, changes);
        out.extend(extract_floor::dependents(snapshot, changes));
        out
    });
    shell::prefs::register_settings(&mut builder)?;
    register_settings(&mut builder)?;
    // File > Export > IFC asks for the file in a save dialog that starts with the document's name.
    builder.path_dialog(archbuilder_io::command::EXPORT_IFC, export_ifc_dialog());
    // The shortcut table goes to and comes from a small JSON file.
    builder.path_dialog(
        archbuilder_io::command::EXPORT_SHORTCUTS,
        shortcuts_dialog(PathMode::Save, SHORTCUTS_EXPORT_TITLE.message_id),
    );
    builder.path_dialog(
        archbuilder_io::command::IMPORT_SHORTCUTS,
        shortcuts_dialog(PathMode::Open, SHORTCUTS_IMPORT_TITLE.message_id),
    );
    builder.package_extension("archb");
    // A new document is created with its project in one step: the starting content is no undo
    // step, and the document saves like any other.
    builder.new_document_command(PROJECT_NEW);
    builder.file_type(FileType {
        extension: "archb".into(),
        message_id: "ab-file-archb".into(),
        open: true,
        save: true,
    })?;
    let panels = [
        entry(
            VIEWPORT,
            Message {
                message_id: "ab-panel-viewport",
            },
            || {
                // A click in the viewport selects through the product's command, so a level
                // outline selects its storey.
                let mut viewport = ViewportPanel::new();
                viewport.commands.select = OBJECT_SELECT.into();
                shell::panel(viewport)
            },
        ),
        entry(
            PROPERTIES,
            Message {
                message_id: "ab-panel-properties",
            },
            || shell::panel(properties::PropertiesPanel::default()),
        ),
        entry(
            OUTLINER,
            Message {
                message_id: "ab-panel-outliner",
            },
            || shell::panel(outliner::OutlinerPanel::default()),
        ),
        entry(
            SETTINGS,
            Message {
                message_id: "ab-panel-settings",
            },
            || shell::panel(shell::SettingsPanel::default()),
        ),
        entry(
            COMMAND_LINE,
            Message {
                message_id: "ab-panel-command-line",
            },
            || shell::panel(CommandLinePanel),
        ),
    ];
    for panel in panels {
        builder.panel(panel)?;
    }
    builder.extractor(Extractor {
        id: "archbuilder.level".into(),
        types: vec![LEVEL.to_owned()],
        factory: Arc::new(|| -> extract::Extractor {
            shell::extractor(|_, object, cancel| extract_level::extract_level(object, cancel))
        }),
    })?;
    builder.extractor(Extractor {
        id: "archbuilder.wall".into(),
        types: vec![WALL.to_owned()],
        factory: Arc::new(|| -> extract::Extractor {
            shell::extractor(|snapshot, object, cancel| {
                extract_wall::extract_wall(snapshot, object, cancel)
            })
        }),
    })?;
    builder.extractor(Extractor {
        id: "archbuilder.floor".into(),
        types: vec![FLOOR.to_owned()],
        factory: Arc::new(|| -> extract::Extractor {
            shell::extractor(|snapshot, object, cancel| {
                extract_floor::extract_floor(snapshot, object, cancel)
            })
        }),
    })?;
    builder.extractor(Extractor {
        id: "archbuilder.insert".into(),
        types: vec![DOOR.to_owned(), WINDOW.to_owned()],
        factory: Arc::new(|| -> extract::Extractor {
            shell::extractor(|snapshot, object, cancel| {
                extract_insert::extract_insert(snapshot, object, cancel)
            })
        }),
    })?;
    builder.build(clock, owner)
}

fn tabs(panels: &[&str]) -> Dock {
    Dock::Tabs {
        tabs: panels.iter().map(|p| (*p).to_owned()).collect(),
        active: 0,
    }
}

/// The menu bar: the shell's File and Edit menus, with an Export submenu in File, and the Structure
/// menu with the storey commands.
fn menu() -> Result<Menu> {
    let mut items = shell::default_menu()?.items().to_vec();
    // After Save As: File > Export > IFC.
    if let Some(MenuItem::Submenu { items: file, .. }) = items.first_mut() {
        let at = file
            .iter()
            .position(|i| matches!(i, MenuItem::Command { command } if command == arc_app::commands::FILE_SAVE_AS))
            .map_or(file.len(), |k| k + 1);
        file.insert(
            at,
            MenuItem::Submenu {
                label: MENU_EXPORT.message_id.into(),
                items: [
                    archbuilder_io::command::EXPORT_IFC,
                    archbuilder_io::command::EXPORT_SHORTCUTS,
                ]
                .into_iter()
                .map(|c| MenuItem::Command { command: c.into() })
                .collect(),
            },
        );
        file.insert(
            at + 1,
            MenuItem::Submenu {
                label: MENU_IMPORT.message_id.into(),
                items: vec![MenuItem::Command {
                    command: archbuilder_io::command::IMPORT_SHORTCUTS.into(),
                }],
            },
        );
    }
    items.push(MenuItem::Submenu {
        label: MENU_STRUCTURE.message_id.into(),
        items: [
            STOREY_ADD,
            STOREY_DELETE,
            WALL_CREATE,
            DOOR_PLACE,
            WINDOW_PLACE,
            FLOOR_CREATE,
            FLOOR_ADD_HOLE,
            ROOM_PLACE,
            ROOM_PLACE_ALL,
            INSERT_FLIP_HAND,
            INSERT_FLIP_FACING,
        ]
        .into_iter()
        .map(|c| MenuItem::Command { command: c.into() })
        .collect(),
    });
    items.push(MenuItem::Submenu {
        label: MENU_MODIFY.message_id.into(),
        items: [
            MODIFY_MOVE,
            MODIFY_COPY,
            MODIFY_ROTATE,
            MODIFY_MIRROR,
            CLIPBOARD_PASTE,
            CLIPBOARD_PASTE_TO_STOREY,
            OBJECT_DELETE,
        ]
        .into_iter()
        .map(|c| MenuItem::Command { command: c.into() })
        .collect(),
    });
    Menu::new(items)
}

/// The shell configuration: the viewport in the middle, the outliner above the property grid on
/// the right, the command line below; the menu bar and the toolbar; key bindings for the file
/// commands and Delete for the storey; the level labels and the starting view. `seed` seeds the invocations (see [`ShellConfig::seed`]).
///
/// # Errors
/// None for this layout and menu; their validation errors otherwise.
pub fn config(seed: u64) -> Result<ShellConfig> {
    let right = Dock::Split {
        axis: Axis::Vertical,
        fraction: 0.55,
        first: Box::new(tabs(&[OUTLINER])),
        second: Box::new(tabs(&[PROPERTIES, SETTINGS])),
    };
    let top = Dock::Split {
        axis: Axis::Horizontal,
        fraction: 0.72,
        first: Box::new(tabs(&[VIEWPORT])),
        second: Box::new(right),
    };
    let root = Dock::Split {
        axis: Axis::Vertical,
        fraction: 0.78,
        first: Box::new(top),
        second: Box::new(tabs(&[COMMAND_LINE])),
    };
    let binding = |keys: &str, command: &str| Binding {
        keys: keys.into(),
        command: command.into(),
    };
    Ok(ShellConfig {
        layout: DockLayout::new(root, Vec::new())?,
        menu: menu()?,
        bindings: vec![
            binding("Ctrl+N", arc_app::commands::FILE_NEW),
            binding("Ctrl+O", arc_app::commands::FILE_OPEN),
            binding("Ctrl+S", arc_app::commands::FILE_SAVE),
            binding("Ctrl+Shift+S", arc_app::commands::FILE_SAVE_AS),
            binding("Ctrl+W", arc_app::commands::FILE_CLOSE),
            binding("Ctrl+Q", arc_app::commands::APP_QUIT),
            // Ctrl+Z and Ctrl+Y are the shell's: the command line handles them once.
            // Delete removes the selected walls, else the selected storey.
            binding("Delete", OBJECT_DELETE),
        ],
        appearance: arc_ui::Appearance {
            theme: arc_ui::ThemeId::Dark,
            ..arc_ui::Appearance::default()
        },
        display: None,
        // The rooms' fills and tags follow the level labels; the shared room view learns the
        // selection and the active storey from the work-plane probe.
        overlays: Some(OverlayProbe(Arc::new(|app, tab, language| {
            let mut out = labels::overlays(app, tab, language);
            if let Ok(t) = app.tab(tab) {
                out.extend(room_view::shared().overlays(&t.document().snapshot(), language));
            }
            out
        }))),
        work_plane: Some(WorkPlaneProbe(Arc::new(interaction::work_plane))),
        // Seen from the front and the right, from above: the levels are told apart at once.
        initial_view: Some(InitialView {
            toward_camera: [1.0, -1.4, 0.9],
        }),
        toolbar: chrome::toolbar(),
        status: chrome::status(),
        seed,
    })
}

/// The languages the operating system asks for, in the order the environment names them.
#[must_use]
pub fn os_locales() -> Vec<String> {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|n| std::env::var(n).ok())
        .collect()
}

/// The message catalogs the application formats with, for the requested languages: the shell's
/// ([`arc_app::locale::shell_catalogs`]) with the English and Turkish messages of the application
/// and its crates appended.
///
/// # Errors
/// `app.locale.catalog_invalid` when a catalog does not parse.
pub fn localizer(requested: &[String]) -> Result<arc_app::locale::Localizer> {
    let mut catalogs = arc_app::locale::shell_catalogs()?;
    let english = [
        include_str!("../../../locales/en-US/archbuilder.ftl"),
        include_str!("../../../locales/en-US/archbuilder-commands.ftl"),
        include_str!("../../../locales/en-US/archbuilder-io.ftl"),
        include_str!("../../../locales/en-US/archbuilder-model.ftl"),
    ]
    .concat();
    catalogs.append("en-US", &english)?;
    let turkish = [
        include_str!("../../../locales/tr-TR/archbuilder.ftl"),
        include_str!("../../../locales/tr-TR/archbuilder-commands.ftl"),
        include_str!("../../../locales/tr-TR/archbuilder-io.ftl"),
        include_str!("../../../locales/tr-TR/archbuilder-model.ftl"),
    ]
    .concat();
    catalogs.append("tr-TR", &turkish)?;
    arc_app::locale::Localizer::new(catalogs, requested)
}

/// The product part of the window title. The shell puts the active document's name and its
/// modified mark before it (`*house.archb - ArchBuilder`).
#[must_use]
pub fn title(localizer: &arc_app::locale::Localizer) -> String {
    localizer.format(TITLE.message_id, &[]).text
}

/// The error of a start-up step, for the report on standard error.
#[must_use]
pub fn describe(error: &Error, localizer: Option<&arc_app::locale::Localizer>) -> String {
    match localizer {
        Some(l) => format!("{}: {}", error.code(), l.format_error(error).text),
        None => error.code().to_owned(),
    }
}
