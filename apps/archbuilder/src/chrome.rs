//! The toolbar strip of the ArchBuilder window: which commands the buttons start. Each button
//! submits the identifier of a command to the command line, as if the user typed it; the shell draws
//! the strip and the status bar.

use arc_app::commands;
use arc_app::settings::SettingValue;
use arc_app::shell::{StatusToggle, ToolItem};
use arc_ui::Intent;
use archbuilder_commands::floor::FLOOR_CREATE;
use archbuilder_commands::insert::{DOOR_PLACE, WINDOW_PLACE};
use archbuilder_commands::modify::{MODIFY_COPY, MODIFY_MIRROR, MODIFY_MOVE, MODIFY_ROTATE};
use archbuilder_commands::room::{ROOM_PLACE, ROOM_PLACE_ALL};
use archbuilder_commands::stair::STAIR_CREATE;
use archbuilder_commands::structure::{STOREY_ADD, STOREY_DELETE};
use archbuilder_commands::wall::WALL_CREATE;

/// A message of the application.
struct Message {
    message_id: &'static str,
}

/// A button with a label and a tooltip message, starting `line`.
fn button(label: Message, hint: Message, line: &str) -> ToolItem {
    ToolItem::Button {
        label: label.message_id.to_owned(),
        hint: Some(hint.message_id.to_owned()),
        line: line.to_owned(),
    }
}

/// The toolbar: the file commands, undo, redo and zoom first (so a wrap never moves them), the storey commands, the wall, door and window tools, undo and redo, and zoom to
/// the extents.
#[must_use]
#[allow(clippy::too_many_lines, reason = "one entry per toolbar button")]
pub fn toolbar() -> Vec<ToolItem> {
    vec![
        button(
            Message {
                message_id: "ab-tool-new",
            },
            Message {
                message_id: "ab-tool-new-hint",
            },
            commands::FILE_NEW,
        ),
        button(
            Message {
                message_id: "ab-tool-open",
            },
            Message {
                message_id: "ab-tool-open-hint",
            },
            commands::FILE_OPEN,
        ),
        button(
            Message {
                message_id: "ab-tool-save",
            },
            Message {
                message_id: "ab-tool-save-hint",
            },
            commands::FILE_SAVE,
        ),
        button(
            Message {
                message_id: "ab-tool-save-as",
            },
            Message {
                message_id: "ab-tool-save-as-hint",
            },
            commands::FILE_SAVE_AS,
        ),
        ToolItem::Separator,
        button(
            Message {
                message_id: "ab-tool-undo",
            },
            Message {
                message_id: "ab-tool-undo-hint",
            },
            commands::EDIT_UNDO,
        ),
        button(
            Message {
                message_id: "ab-tool-redo",
            },
            Message {
                message_id: "ab-tool-redo-hint",
            },
            commands::EDIT_REDO,
        ),
        ToolItem::Separator,
        ToolItem::ZoomExtents {
            label: Message {
                message_id: "ab-tool-zoom-extents",
            }
            .message_id
            .to_owned(),
        },
        ToolItem::Separator,
        button(
            Message {
                message_id: "ab-tool-add-storey",
            },
            Message {
                message_id: "ab-tool-add-storey-hint",
            },
            STOREY_ADD,
        ),
        button(
            Message {
                message_id: "ab-tool-delete-storey",
            },
            Message {
                message_id: "ab-tool-delete-storey-hint",
            },
            STOREY_DELETE,
        ),
        ToolItem::Separator,
        button(
            Message {
                message_id: "ab-tool-wall",
            },
            Message {
                message_id: "ab-tool-wall-hint",
            },
            WALL_CREATE,
        ),
        button(
            Message {
                message_id: "ab-tool-door",
            },
            Message {
                message_id: "ab-tool-door-hint",
            },
            DOOR_PLACE,
        ),
        button(
            Message {
                message_id: "ab-tool-window",
            },
            Message {
                message_id: "ab-tool-window-hint",
            },
            WINDOW_PLACE,
        ),
        button(
            Message {
                message_id: "ab-tool-floor",
            },
            Message {
                message_id: "ab-tool-floor-hint",
            },
            FLOOR_CREATE,
        ),
        button(
            Message {
                message_id: "ab-tool-room",
            },
            Message {
                message_id: "ab-tool-room-hint",
            },
            ROOM_PLACE,
        ),
        button(
            Message {
                message_id: "ab-tool-room-all",
            },
            Message {
                message_id: "ab-tool-room-all-hint",
            },
            ROOM_PLACE_ALL,
        ),
        button(
            Message {
                message_id: "ab-tool-stair",
            },
            Message {
                message_id: "ab-tool-stair-hint",
            },
            STAIR_CREATE,
        ),
        ToolItem::Separator,
        button(
            Message {
                message_id: "ab-tool-move",
            },
            Message {
                message_id: "ab-tool-move-hint",
            },
            MODIFY_MOVE,
        ),
        button(
            Message {
                message_id: "ab-tool-copy",
            },
            Message {
                message_id: "ab-tool-copy-hint",
            },
            MODIFY_COPY,
        ),
        button(
            Message {
                message_id: "ab-tool-rotate",
            },
            Message {
                message_id: "ab-tool-rotate-hint",
            },
            MODIFY_ROTATE,
        ),
        button(
            Message {
                message_id: "ab-tool-mirror",
            },
            Message {
                message_id: "ab-tool-mirror-hint",
            },
            MODIFY_MIRROR,
        ),
    ]
}

const SNAP_LABEL: Message = Message {
    message_id: "ab-snap-label",
};
const SNAP_HINT: Message = Message {
    message_id: "ab-snap-hint",
};

/// The status bar toggles: snapping to points (a user setting, so it stays across sessions), which
/// F3 also flips. The toggle reaches the controller as the interface command
/// `archbuilder.view.snap`.
#[must_use]
pub fn status() -> Vec<StatusToggle> {
    vec![StatusToggle {
        label: SNAP_LABEL.message_id.to_owned(),
        hint: Some(SNAP_HINT.message_id.to_owned()),
        state: std::sync::Arc::new(|app, _| {
            Some(!matches!(
                app.settings()
                    .get(crate::interaction::SNAP_SETTING)
                    .map(|e| e.value),
                Ok(SettingValue::Bool(false))
            ))
        }),
        set: std::sync::Arc::new(|_, _, on| {
            Some(Intent::new(crate::shortcuts::ui::SNAP).with("on", on))
        }),
        key: Some(arc_ui::egui::Key::F3),
    }]
}
