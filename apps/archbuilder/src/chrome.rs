//! The command tabs of the ArchBuilder window: which commands the buttons start, how they are
//! grouped, the contextual selection tab, the quick access buttons, the icon set and the status
//! texts. Each button submits the identifier of a command (or a typed word of the command line)
//! as if the user typed it; the shell draws the tabs and the status bar.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use arc_app::application::TabId;
use arc_app::commands;
use arc_app::settings::SettingValue;
use arc_app::shell::{
    ButtonSize, CommandGroup, CommandTab, ContextProbe, StatusProbe, StatusToggle, ToolItem,
};
use arc_base::error::Value;
use arc_ui::Intent;
use archbuilder_commands::datum::{GRID_CREATE, REFERENCE_PLANE_CREATE, WORK_PLANE_SET};
use archbuilder_commands::edit;
use archbuilder_commands::floor::FLOOR_CREATE;
use archbuilder_commands::insert::{
    DOOR_PLACE, INSERT_FLIP_FACING, INSERT_FLIP_HAND, WINDOW_PLACE,
};
use archbuilder_commands::modify::{MODIFY_COPY, MODIFY_MIRROR, MODIFY_MOVE, MODIFY_ROTATE};
use archbuilder_commands::roof::{WALL_ATTACH_TOP, WALL_DETACH_TOP};
use archbuilder_commands::room::{ROOM_PLACE, ROOM_PLACE_ALL};
use archbuilder_commands::stair::STAIR_CREATE;
use archbuilder_commands::structure::{STOREY_ADD, STOREY_DELETE};
use archbuilder_commands::wall::{OBJECT_DELETE, WALL_CREATE};

/// A message of the application.
struct Message {
    message_id: &'static str,
}

/// The words of the command line that the buttons of the application start besides registered
/// commands (see `habits::typed_word`).
pub const TYPED_WORDS: &[&str] = &[
    "hide",
    "isolate",
    "unhide",
    "createsimilar",
    "matchtype",
    "edittype",
    "warnings",
    "plan",
    "3d",
];

/// The most tabs whose selection is remembered.
const MAX_TABS: usize = 256;

/// How many objects each tab has selected and the name of its working storey, as the controller
/// of the tab last saw them (the probes of the shell get no selection).
static SEEN: Mutex<BTreeMap<TabId, (usize, Option<String>)>> = Mutex::new(BTreeMap::new());

/// Records what `tab` has selected and which storey it works on, for the contextual tab and the
/// status text.
pub fn publish(tab: TabId, selected: usize, storey: Option<String>) {
    let mut seen = SEEN.lock().unwrap_or_else(PoisonError::into_inner);
    if seen.len() >= MAX_TABS && !seen.contains_key(&tab) {
        seen.clear();
    }
    seen.insert(tab, (selected, storey));
}

/// The number of objects `tab` has selected.
#[must_use]
pub fn selected(tab: TabId) -> usize {
    SEEN.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&tab)
        .map_or(0, |(n, _)| *n)
}

/// An icon button with a label, an optional tooltip and the line it submits.
fn icon_button(
    label: Message,
    hint: Option<Message>,
    line: &str,
    icon: &str,
    size: ButtonSize,
) -> ToolItem {
    ToolItem::IconButton {
        label: label.message_id.to_owned(),
        hint: hint.map(|h| h.message_id.to_owned()),
        line: line.to_owned(),
        icon: icon.to_owned(),
        size,
    }
}

fn group(label: Message, items: Vec<ToolItem>) -> CommandGroup {
    CommandGroup {
        label: label.message_id.to_owned(),
        items,
    }
}

/// The command tabs: Architecture, Modify, View, Manage and Exchange, and the Selection tab that
/// shows while the active document has a selection.
#[must_use]
#[allow(clippy::too_many_lines, reason = "one entry per button")]
pub fn command_tabs() -> Vec<CommandTab> {
    vec![
        CommandTab {
            label: Message {
                message_id: "ab-tab-architecture",
            }
            .message_id
            .to_owned(),
            groups: vec![
                group(
                    Message {
                        message_id: "ab-group-build",
                    },
                    vec![
                        icon_button(
                            Message {
                                message_id: "ab-tool-wall",
                            },
                            Some(Message {
                                message_id: "ab-tool-wall-hint",
                            }),
                            WALL_CREATE,
                            "wall",
                            ButtonSize::Large,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-door",
                            },
                            Some(Message {
                                message_id: "ab-tool-door-hint",
                            }),
                            DOOR_PLACE,
                            "door",
                            ButtonSize::Large,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-window",
                            },
                            Some(Message {
                                message_id: "ab-tool-window-hint",
                            }),
                            WINDOW_PLACE,
                            "window",
                            ButtonSize::Large,
                        ),
                    ],
                ),
                group(
                    Message {
                        message_id: "ab-group-floors-roofs",
                    },
                    vec![
                        icon_button(
                            Message {
                                message_id: "ab-tool-floor",
                            },
                            Some(Message {
                                message_id: "ab-tool-floor-hint",
                            }),
                            FLOOR_CREATE,
                            "floor",
                            ButtonSize::Large,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-roof",
                            },
                            Some(Message {
                                message_id: "ab-tool-roof-hint",
                            }),
                            archbuilder_commands::roof::ROOF_CREATE,
                            "roof",
                            ButtonSize::Large,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-ceiling",
                            },
                            Some(Message {
                                message_id: "ab-tool-ceiling-hint",
                            }),
                            archbuilder_commands::ceiling::CEILING_CREATE,
                            "ceiling",
                            ButtonSize::Large,
                        ),
                    ],
                ),
                group(
                    Message {
                        message_id: "ab-group-openings",
                    },
                    vec![
                        icon_button(
                            Message {
                                message_id: "ab-tool-opening",
                            },
                            Some(Message {
                                message_id: "ab-tool-opening-hint",
                            }),
                            archbuilder_commands::insert::OPENING_PLACE,
                            "opening-wall",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-shaft",
                            },
                            Some(Message {
                                message_id: "ab-tool-shaft-hint",
                            }),
                            archbuilder_commands::opening::SHAFT_CREATE,
                            "opening-shaft",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-cut",
                            },
                            Some(Message {
                                message_id: "ab-tool-cut-hint",
                            }),
                            archbuilder_commands::opening::VERTICAL_OPENING_CREATE,
                            "opening-vertical",
                            ButtonSize::Small,
                        ),
                    ],
                ),
                group(
                    Message {
                        message_id: "ab-group-circulation",
                    },
                    vec![icon_button(
                        Message {
                            message_id: "ab-tool-stair",
                        },
                        Some(Message {
                            message_id: "ab-tool-stair-hint",
                        }),
                        STAIR_CREATE,
                        "stair",
                        ButtonSize::Large,
                    )],
                ),
                group(
                    Message {
                        message_id: "ab-group-rooms",
                    },
                    vec![
                        icon_button(
                            Message {
                                message_id: "ab-tool-room",
                            },
                            Some(Message {
                                message_id: "ab-tool-room-hint",
                            }),
                            ROOM_PLACE,
                            "room",
                            ButtonSize::Large,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-room-all",
                            },
                            Some(Message {
                                message_id: "ab-tool-room-all-hint",
                            }),
                            ROOM_PLACE_ALL,
                            "room-all",
                            ButtonSize::Small,
                        ),
                    ],
                ),
                group(
                    Message {
                        message_id: "ab-group-datum",
                    },
                    vec![
                        icon_button(
                            Message {
                                message_id: "ab-tool-grid",
                            },
                            Some(Message {
                                message_id: "ab-tool-grid-hint",
                            }),
                            GRID_CREATE,
                            "grid",
                            ButtonSize::Large,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-reference-plane",
                            },
                            Some(Message {
                                message_id: "ab-tool-reference-plane-hint",
                            }),
                            REFERENCE_PLANE_CREATE,
                            "reference-plane",
                            ButtonSize::Large,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-work-plane",
                            },
                            Some(Message {
                                message_id: "ab-tool-work-plane-hint",
                            }),
                            WORK_PLANE_SET,
                            "work-plane",
                            ButtonSize::Large,
                        ),
                    ],
                ),
                group(
                    Message {
                        message_id: "ab-group-type",
                    },
                    vec![crate::tool_type::toolbar_item()],
                ),
            ],
            context: None,
        },
        CommandTab {
            label: Message {
                message_id: "ab-tab-modify",
            }
            .message_id
            .to_owned(),
            groups: vec![
                group(
                    Message {
                        message_id: "ab-group-transform",
                    },
                    vec![
                        icon_button(
                            Message {
                                message_id: "ab-tool-move",
                            },
                            Some(Message {
                                message_id: "ab-tool-move-hint",
                            }),
                            MODIFY_MOVE,
                            "move",
                            ButtonSize::Large,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-copy",
                            },
                            Some(Message {
                                message_id: "ab-tool-copy-hint",
                            }),
                            MODIFY_COPY,
                            "copy",
                            ButtonSize::Large,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-rotate",
                            },
                            Some(Message {
                                message_id: "ab-tool-rotate-hint",
                            }),
                            MODIFY_ROTATE,
                            "rotate",
                            ButtonSize::Large,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-mirror",
                            },
                            Some(Message {
                                message_id: "ab-tool-mirror-hint",
                            }),
                            MODIFY_MIRROR,
                            "mirror",
                            ButtonSize::Large,
                        ),
                    ],
                ),
                group(
                    Message {
                        message_id: "ab-group-edit",
                    },
                    vec![
                        icon_button(
                            Message {
                                message_id: "ab-tool-align",
                            },
                            Some(Message {
                                message_id: "ab-tool-align-hint",
                            }),
                            edit::MODIFY_ALIGN,
                            "align",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-offset",
                            },
                            Some(Message {
                                message_id: "ab-tool-offset-hint",
                            }),
                            edit::MODIFY_OFFSET,
                            "offset",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-trim",
                            },
                            Some(Message {
                                message_id: "ab-tool-trim-hint",
                            }),
                            edit::MODIFY_TRIM,
                            "trim",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-trim-to",
                            },
                            Some(Message {
                                message_id: "ab-tool-trim-to-hint",
                            }),
                            edit::MODIFY_TRIM_TO,
                            "trim-boundary",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-split",
                            },
                            Some(Message {
                                message_id: "ab-tool-split-hint",
                            }),
                            edit::WALL_SPLIT,
                            "split",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-merge",
                            },
                            Some(Message {
                                message_id: "ab-tool-merge-hint",
                            }),
                            edit::WALL_MERGE,
                            "merge",
                            ButtonSize::Small,
                        ),
                    ],
                ),
                group(
                    Message {
                        message_id: "ab-group-arrays",
                    },
                    vec![
                        icon_button(
                            Message {
                                message_id: "ab-tool-array",
                            },
                            Some(Message {
                                message_id: "ab-tool-array-hint",
                            }),
                            edit::MODIFY_ARRAY,
                            "array",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-array-radial",
                            },
                            Some(Message {
                                message_id: "ab-tool-array-radial-hint",
                            }),
                            edit::MODIFY_ARRAY_RADIAL,
                            "array-radial",
                            ButtonSize::Small,
                        ),
                    ],
                ),
                group(
                    Message {
                        message_id: "ab-group-element",
                    },
                    vec![
                        icon_button(
                            Message {
                                message_id: "ab-tool-pin",
                            },
                            Some(Message {
                                message_id: "ab-tool-pin-hint",
                            }),
                            edit::ELEMENT_PIN,
                            "pin",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-unpin",
                            },
                            Some(Message {
                                message_id: "ab-tool-unpin-hint",
                            }),
                            edit::ELEMENT_UNPIN,
                            "unpin",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-menu-delete",
                            },
                            None,
                            OBJECT_DELETE,
                            "delete",
                            ButtonSize::Small,
                        ),
                    ],
                ),
                group(
                    Message {
                        message_id: "ab-group-types",
                    },
                    vec![
                        icon_button(
                            Message {
                                message_id: "ab-menu-match-type",
                            },
                            None,
                            "matchtype",
                            "match-type",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-menu-create-similar",
                            },
                            None,
                            "createsimilar",
                            "create-similar",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-menu-edit-type",
                            },
                            None,
                            "edittype",
                            "types",
                            ButtonSize::Small,
                        ),
                    ],
                ),
            ],
            context: None,
        },
        CommandTab {
            label: Message {
                message_id: "ab-tab-view",
            }
            .message_id
            .to_owned(),
            groups: vec![
                group(
                    Message {
                        message_id: "ab-group-navigate",
                    },
                    vec![
                        ToolItem::ZoomExtentsIcon {
                            label: Message {
                                message_id: "ab-tool-zoom-extents",
                            }
                            .message_id
                            .to_owned(),
                            hint: None,
                            icon: "zoom-extents".to_owned(),
                            size: ButtonSize::Small,
                        },
                        icon_button(
                            Message {
                                message_id: "ab-tool-plan",
                            },
                            Some(Message {
                                message_id: "ab-tool-plan-hint",
                            }),
                            "plan",
                            "plan-view",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-view-3d",
                            },
                            Some(Message {
                                message_id: "ab-tool-view-3d-hint",
                            }),
                            "3d",
                            "view-3d",
                            ButtonSize::Small,
                        ),
                    ],
                ),
                group(
                    Message {
                        message_id: "ab-group-visibility",
                    },
                    vec![
                        icon_button(
                            Message {
                                message_id: "ab-menu-hide",
                            },
                            None,
                            "hide",
                            "hide",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-menu-isolate",
                            },
                            None,
                            "isolate",
                            "isolate",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-menu-unhide-all",
                            },
                            None,
                            "unhide",
                            "reveal",
                            ButtonSize::Small,
                        ),
                    ],
                ),
            ],
            context: None,
        },
        CommandTab {
            label: Message {
                message_id: "ab-tab-manage",
            }
            .message_id
            .to_owned(),
            groups: vec![
                group(
                    Message {
                        message_id: "ab-group-storeys",
                    },
                    vec![
                        icon_button(
                            Message {
                                message_id: "ab-tool-add-storey",
                            },
                            Some(Message {
                                message_id: "ab-tool-add-storey-hint",
                            }),
                            STOREY_ADD,
                            "storey-add",
                            ButtonSize::Large,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-delete-storey",
                            },
                            Some(Message {
                                message_id: "ab-tool-delete-storey-hint",
                            }),
                            STOREY_DELETE,
                            "storey-delete",
                            ButtonSize::Large,
                        ),
                    ],
                ),
                group(
                    Message {
                        message_id: "ab-group-types",
                    },
                    vec![
                        icon_button(
                            Message {
                                message_id: "ab-menu-edit-type",
                            },
                            None,
                            "edittype",
                            "types",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-menu-create-similar",
                            },
                            None,
                            "createsimilar",
                            "create-similar",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-menu-match-type",
                            },
                            None,
                            "matchtype",
                            "match-type",
                            ButtonSize::Small,
                        ),
                    ],
                ),
                group(
                    Message {
                        message_id: "ab-group-checks",
                    },
                    vec![icon_button(
                        Message {
                            message_id: "ab-panel-warnings",
                        },
                        None,
                        "warnings",
                        "warnings",
                        ButtonSize::Large,
                    )],
                ),
                group(
                    Message {
                        message_id: "ab-group-shortcuts",
                    },
                    vec![
                        icon_button(
                            Message {
                                message_id: "ab-tool-shortcuts-export",
                            },
                            None,
                            archbuilder_io::command::EXPORT_SHORTCUTS,
                            "shortcuts",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-shortcuts-import",
                            },
                            None,
                            archbuilder_io::command::IMPORT_SHORTCUTS,
                            "shortcuts",
                            ButtonSize::Small,
                        ),
                    ],
                ),
            ],
            context: None,
        },
        CommandTab {
            label: Message {
                message_id: "ab-tab-exchange",
            }
            .message_id
            .to_owned(),
            groups: vec![group(
                Message {
                    message_id: "ab-group-ifc",
                },
                vec![icon_button(
                    Message {
                        message_id: "ab-tool-export-ifc",
                    },
                    None,
                    archbuilder_io::command::EXPORT_IFC,
                    "export-ifc",
                    ButtonSize::Large,
                )],
            )],
            context: None,
        },
        CommandTab {
            label: Message {
                message_id: "ab-tab-selection",
            }
            .message_id
            .to_owned(),
            groups: vec![
                group(
                    Message {
                        message_id: "ab-group-transform",
                    },
                    vec![
                        icon_button(
                            Message {
                                message_id: "ab-tool-move",
                            },
                            Some(Message {
                                message_id: "ab-tool-move-hint",
                            }),
                            MODIFY_MOVE,
                            "move",
                            ButtonSize::Large,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-copy",
                            },
                            Some(Message {
                                message_id: "ab-tool-copy-hint",
                            }),
                            MODIFY_COPY,
                            "copy",
                            ButtonSize::Large,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-rotate",
                            },
                            Some(Message {
                                message_id: "ab-tool-rotate-hint",
                            }),
                            MODIFY_ROTATE,
                            "rotate",
                            ButtonSize::Large,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-mirror",
                            },
                            Some(Message {
                                message_id: "ab-tool-mirror-hint",
                            }),
                            MODIFY_MIRROR,
                            "mirror",
                            ButtonSize::Large,
                        ),
                    ],
                ),
                group(
                    Message {
                        message_id: "ab-group-edit",
                    },
                    vec![
                        icon_button(
                            Message {
                                message_id: "ab-tool-align",
                            },
                            Some(Message {
                                message_id: "ab-tool-align-hint",
                            }),
                            edit::MODIFY_ALIGN,
                            "align",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-offset",
                            },
                            Some(Message {
                                message_id: "ab-tool-offset-hint",
                            }),
                            edit::MODIFY_OFFSET,
                            "offset",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-trim",
                            },
                            Some(Message {
                                message_id: "ab-tool-trim-hint",
                            }),
                            edit::MODIFY_TRIM,
                            "trim",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-trim-to",
                            },
                            Some(Message {
                                message_id: "ab-tool-trim-to-hint",
                            }),
                            edit::MODIFY_TRIM_TO,
                            "trim-boundary",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-split",
                            },
                            Some(Message {
                                message_id: "ab-tool-split-hint",
                            }),
                            edit::WALL_SPLIT,
                            "split",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-merge",
                            },
                            Some(Message {
                                message_id: "ab-tool-merge-hint",
                            }),
                            edit::WALL_MERGE,
                            "merge",
                            ButtonSize::Small,
                        ),
                    ],
                ),
                group(
                    Message {
                        message_id: "ab-group-arrays",
                    },
                    vec![
                        icon_button(
                            Message {
                                message_id: "ab-tool-array",
                            },
                            Some(Message {
                                message_id: "ab-tool-array-hint",
                            }),
                            edit::MODIFY_ARRAY,
                            "array",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-array-radial",
                            },
                            Some(Message {
                                message_id: "ab-tool-array-radial-hint",
                            }),
                            edit::MODIFY_ARRAY_RADIAL,
                            "array-radial",
                            ButtonSize::Small,
                        ),
                    ],
                ),
                group(
                    Message {
                        message_id: "ab-group-element",
                    },
                    vec![
                        icon_button(
                            Message {
                                message_id: "ab-tool-pin",
                            },
                            Some(Message {
                                message_id: "ab-tool-pin-hint",
                            }),
                            edit::ELEMENT_PIN,
                            "pin",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-unpin",
                            },
                            Some(Message {
                                message_id: "ab-tool-unpin-hint",
                            }),
                            edit::ELEMENT_UNPIN,
                            "unpin",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-menu-delete",
                            },
                            None,
                            OBJECT_DELETE,
                            "delete",
                            ButtonSize::Small,
                        ),
                    ],
                ),
                group(
                    Message {
                        message_id: "ab-group-hosted",
                    },
                    vec![
                        icon_button(
                            Message {
                                message_id: "ab-tool-flip-hand",
                            },
                            None,
                            INSERT_FLIP_HAND,
                            "move",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-flip-facing",
                            },
                            None,
                            INSERT_FLIP_FACING,
                            "mirror",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-attach-top",
                            },
                            None,
                            WALL_ATTACH_TOP,
                            "roof",
                            ButtonSize::Small,
                        ),
                        icon_button(
                            Message {
                                message_id: "ab-tool-detach-top",
                            },
                            None,
                            WALL_DETACH_TOP,
                            "wall",
                            ButtonSize::Small,
                        ),
                    ],
                ),
                group(
                    Message {
                        message_id: "ab-group-type",
                    },
                    vec![crate::tool_type::toolbar_item()],
                ),
            ],
            context: Some(ContextProbe(Arc::new(|_, tab| selected(tab) > 0))),
        },
    ]
}

/// The buttons of the quick access row: file commands, undo and redo.
#[must_use]
pub fn quick_access() -> Vec<ToolItem> {
    let q = |label: Message, hint: Message, line: &str, icon: &str| {
        icon_button(label, Some(hint), line, icon, ButtonSize::Small)
    };
    vec![
        q(
            Message {
                message_id: "ab-tool-new",
            },
            Message {
                message_id: "ab-tool-new-hint",
            },
            commands::FILE_NEW,
            "new",
        ),
        q(
            Message {
                message_id: "ab-tool-open",
            },
            Message {
                message_id: "ab-tool-open-hint",
            },
            commands::FILE_OPEN,
            "open",
        ),
        q(
            Message {
                message_id: "ab-tool-save",
            },
            Message {
                message_id: "ab-tool-save-hint",
            },
            commands::FILE_SAVE,
            "save",
        ),
        q(
            Message {
                message_id: "ab-tool-save-as",
            },
            Message {
                message_id: "ab-tool-save-as-hint",
            },
            commands::FILE_SAVE_AS,
            "save-as",
        ),
        q(
            Message {
                message_id: "ab-tool-undo",
            },
            Message {
                message_id: "ab-tool-undo-hint",
            },
            commands::EDIT_UNDO,
            "undo",
        ),
        q(
            Message {
                message_id: "ab-tool-redo",
            },
            Message {
                message_id: "ab-tool-redo-hint",
            },
            commands::EDIT_REDO,
            "redo",
        ),
    ]
}

/// The icons of the application, by name: the SVG sources of `apps/archbuilder/icons`.
#[must_use]
pub fn icons() -> Vec<(String, String)> {
    macro_rules! icons {
        ($($name:literal),* $(,)?) => {
            vec![$(($name.to_owned(), include_str!(concat!("../icons/", $name, ".svg")).to_owned())),*]
        };
    }
    icons![
        "new",
        "open",
        "save",
        "save-as",
        "undo",
        "redo",
        "wall",
        "door",
        "window",
        "floor",
        "roof",
        "ceiling",
        "opening-wall",
        "opening-shaft",
        "opening-vertical",
        "stair",
        "grid",
        "reference-plane",
        "work-plane",
        "room",
        "room-all",
        "storey-add",
        "storey-delete",
        "select",
        "move",
        "copy",
        "rotate",
        "mirror",
        "align",
        "offset",
        "trim",
        "trim-boundary",
        "split",
        "merge",
        "array",
        "array-radial",
        "pin",
        "unpin",
        "delete",
        "match-type",
        "create-similar",
        "plan-view",
        "view-3d",
        "zoom-extents",
        "hide",
        "isolate",
        "reveal",
        "types",
        "settings",
        "warnings",
        "shortcuts",
        "properties",
        "outliner",
        "export-ifc",
    ]
}

/// The status texts: the storey the active tab works on and how many objects it has selected.
#[must_use]
pub fn status_text() -> StatusProbe {
    StatusProbe(Arc::new(|_, tab, localize| {
        let seen = SEEN
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&tab)
            .cloned();
        let Some((selected, storey)) = seen else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if let Some(name) = storey {
            out.push(arc_ui::resolve(
                localize,
                Message {
                    message_id: "ab-status-storey",
                }
                .message_id,
                &[("name", Value::Text(name))],
            ));
        }
        if selected > 0 {
            let count = i64::try_from(selected).unwrap_or(i64::MAX);
            out.push(arc_ui::resolve(
                localize,
                Message {
                    message_id: "ab-status-selected",
                }
                .message_id,
                &[("count", Value::Int(count))],
            ));
        }
        out
    }))
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
