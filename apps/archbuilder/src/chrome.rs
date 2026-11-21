//! The toolbar strip of the ArchBuilder window: which commands the buttons start. Each button
//! submits the identifier of a command to the command line, as if the user typed it; the shell draws
//! the strip and the status bar.

use arc_app::commands;
use arc_app::shell::ToolItem;
use archbuilder_commands::floor::FLOOR_CREATE;
use archbuilder_commands::insert::{DOOR_PLACE, WINDOW_PLACE};
use archbuilder_commands::modify::{MODIFY_COPY, MODIFY_MIRROR, MODIFY_MOVE, MODIFY_ROTATE};
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

/// The toolbar: the file commands, the storey commands, the wall, door and window tools, undo and redo, and zoom to
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
    ]
}
