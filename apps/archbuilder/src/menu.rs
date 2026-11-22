//! The context menu of the viewport, by state. A secondary click opens it (the shell shows it
//! through [`arc_app::shell::Interaction::context_menu`]); nothing is done silently any more
//! on the secondary button: during a command the first entry is Enter, so a secondary click is
//! two steps (open, choose) where it used to be one.
//!
//! - Running command: Enter (finish), Cancel, and Undo point when the command has an answer to
//!   take back.
//! - Idle, nothing selected: Repeat last command, Paste (with something on the clipboard), New
//!   stair, Select all, Zoom extents, Unhide all.
//! - Idle with a selection: Delete, Move, Copy, Rotate, Mirror; Copy to clipboard; Hide, Isolate,
//!   Hide kind, Isolate kind, Unhide all (while something is hidden); Select all of this kind, and
//!   when the selection holds several kinds one Keep only entry per kind; Create similar, Match
//!   type and Edit type when it holds walls, floors, doors or windows; Properties.

use arc_app::shell::ContextEntry;
use arc_ui::{Intent, LineAction};
use archbuilder_commands::modify::{
    CLIPBOARD_PASTE, MODIFY_COPY, MODIFY_MIRROR, MODIFY_MOVE, MODIFY_ROTATE,
};
use archbuilder_commands::wall::OBJECT_DELETE;
use archbuilder_model::types::{DOOR, FLOOR, ROOM, STAIR, WALL, WINDOW};

use crate::shortcuts::ui;

/// What the menu depends on.
#[derive(Debug, Clone, Default)]
pub struct State {
    /// A command is running (a tool or a modify command).
    pub running: bool,
    /// The running command has an answer that Undo point takes back.
    pub can_back: bool,
    /// The number of selected objects.
    pub selected: usize,
    /// The kinds in the selection, in the order of [`crate::visibility::KINDS`].
    pub kinds: Vec<&'static str>,
    /// The clipboard holds something.
    pub clipboard: bool,
    /// There is a command to repeat.
    pub has_last: bool,
    /// Objects are hidden or isolated.
    pub temporary: bool,
}

/// A message of the catalog, named by identifier.
struct Label {
    message_id: &'static str,
}

const CANCEL: Label = Label {
    message_id: "ab-menu-cancel",
};
const COPY: Label = Label {
    message_id: "ab-menu-copy",
};
const COPY_CLIPBOARD: Label = Label {
    message_id: "ab-menu-copy-clipboard",
};
const CUT: Label = Label {
    message_id: "ab-menu-cut",
};
const DELETE: Label = Label {
    message_id: "ab-menu-delete",
};
const ENTER: Label = Label {
    message_id: "ab-menu-enter",
};
const HIDE: Label = Label {
    message_id: "ab-menu-hide",
};
const HIDE_KIND: Label = Label {
    message_id: "ab-menu-hide-kind",
};
const ISOLATE: Label = Label {
    message_id: "ab-menu-isolate",
};
const ISOLATE_KIND: Label = Label {
    message_id: "ab-menu-isolate-kind",
};
const KEEP_DOORS: Label = Label {
    message_id: "ab-menu-keep-doors",
};
const KEEP_FLOORS: Label = Label {
    message_id: "ab-menu-keep-floors",
};
const KEEP_ROOMS: Label = Label {
    message_id: "ab-menu-keep-rooms",
};
const KEEP_STAIRS: Label = Label {
    message_id: "ab-menu-keep-stairs",
};
const NEW_STAIR: Label = Label {
    message_id: "ab-menu-new-stair",
};
const KEEP_WALLS: Label = Label {
    message_id: "ab-menu-keep-walls",
};
const KEEP_WINDOWS: Label = Label {
    message_id: "ab-menu-keep-windows",
};
const MIRROR: Label = Label {
    message_id: "ab-menu-mirror",
};
const SELECT_TYPE: Label = Label {
    message_id: "ab-menu-select-type",
};
const CREATE_SIMILAR: Label = Label {
    message_id: "ab-menu-create-similar",
};
const MATCH_TYPE: Label = Label {
    message_id: "ab-menu-match-type",
};
const EDIT_TYPE: Label = Label {
    message_id: "ab-menu-edit-type",
};
const MOVE: Label = Label {
    message_id: "ab-menu-move",
};
const PASTE: Label = Label {
    message_id: "ab-menu-paste",
};
const PROPERTIES_LABEL: Label = Label {
    message_id: "ab-menu-properties",
};
const REPEAT: Label = Label {
    message_id: "ab-menu-repeat",
};
const ROTATE: Label = Label {
    message_id: "ab-menu-rotate",
};
const SELECT_ALL: Label = Label {
    message_id: "ab-menu-select-all",
};
const SELECT_KIND: Label = Label {
    message_id: "ab-menu-select-kind",
};
const UNDO_POINT: Label = Label {
    message_id: "ab-menu-undo-point",
};
const UNHIDE_ALL: Label = Label {
    message_id: "ab-menu-unhide-all",
};
const ZOOM_EXTENTS: Label = Label {
    message_id: "ab-menu-zoom-extents",
};

/// The message identifier of a kind's plural name in a Keep only entry.
fn keep_label(kind: &str) -> &'static str {
    match kind {
        WALL => KEEP_WALLS.message_id,
        DOOR => KEEP_DOORS.message_id,
        WINDOW => KEEP_WINDOWS.message_id,
        FLOOR => KEEP_FLOORS.message_id,
        ROOM => KEEP_ROOMS.message_id,
        STAIR => KEEP_STAIRS.message_id,
        _ => KEEP_WALLS.message_id,
    }
}

/// The entries for `state`, in order.
#[must_use]
pub fn entries(state: &State) -> Vec<ContextEntry> {
    if state.running {
        let mut out = vec![
            ContextEntry::new(ENTER.message_id, LineAction::Submit(String::new())),
            ContextEntry::new(CANCEL.message_id, LineAction::Cancel),
        ];
        if state.can_back {
            out.push(ContextEntry::new(UNDO_POINT.message_id, LineAction::Back));
        }
        return out;
    }
    let unhide = ContextEntry::command(UNHIDE_ALL.message_id, ui::RESET_VISIBILITY);
    let unhide = if state.temporary {
        unhide
    } else {
        unhide.disabled()
    };
    if state.selected == 0 {
        let repeat = ContextEntry::command(REPEAT.message_id, ui::REPEAT);
        let mut out = vec![if state.has_last {
            repeat
        } else {
            repeat.disabled()
        }];
        if state.clipboard {
            out.push(ContextEntry::command(PASTE.message_id, CLIPBOARD_PASTE));
        }
        out.extend([
            ContextEntry::command(
                NEW_STAIR.message_id,
                archbuilder_commands::stair::STAIR_CREATE,
            ),
            ContextEntry::command(SELECT_ALL.message_id, ui::SELECT_ALL),
            ContextEntry::command(ZOOM_EXTENTS.message_id, ui::ZOOM_EXTENTS),
            unhide,
        ]);
        return out;
    }
    let mut out = vec![
        ContextEntry::command(DELETE.message_id, OBJECT_DELETE),
        ContextEntry::command(MOVE.message_id, MODIFY_MOVE),
        ContextEntry::command(COPY.message_id, MODIFY_COPY),
        ContextEntry::command(ROTATE.message_id, MODIFY_ROTATE),
        ContextEntry::command(MIRROR.message_id, MODIFY_MIRROR),
        ContextEntry::separator(),
        ContextEntry::command(COPY_CLIPBOARD.message_id, ui::COPY),
        ContextEntry::command(CUT.message_id, ui::CUT),
        ContextEntry::command(HIDE.message_id, ui::HIDE),
        ContextEntry::command(ISOLATE.message_id, ui::ISOLATE),
        ContextEntry::command(HIDE_KIND.message_id, ui::HIDE_KIND),
        ContextEntry::command(ISOLATE_KIND.message_id, ui::ISOLATE_KIND),
        unhide,
        ContextEntry::separator(),
        ContextEntry::command(SELECT_KIND.message_id, ui::SELECT_KIND),
    ];
    if state
        .kinds
        .iter()
        .any(|k| matches!(*k, WALL | FLOOR | DOOR | WINDOW))
    {
        out.push(ContextEntry::command(
            SELECT_TYPE.message_id,
            ui::SELECT_TYPE,
        ));
    }
    if state.kinds.len() > 1 {
        out.extend(state.kinds.iter().map(|k| {
            ContextEntry::new(
                keep_label(k),
                LineAction::Invoke(Intent::new(ui::SELECT_ONLY).with("kind", *k)),
            )
        }));
    }
    // Walls, floors, doors and windows have types.
    if state
        .kinds
        .iter()
        .any(|k| matches!(*k, WALL | FLOOR | DOOR | WINDOW))
    {
        out.extend([
            ContextEntry::separator(),
            ContextEntry::command(CREATE_SIMILAR.message_id, ui::CREATE_SIMILAR),
            ContextEntry::command(MATCH_TYPE.message_id, ui::MATCH_TYPE),
            ContextEntry::command(EDIT_TYPE.message_id, ui::EDIT_TYPE),
        ]);
    }
    out.extend([
        ContextEntry::separator(),
        ContextEntry::command(PROPERTIES_LABEL.message_id, PROPERTIES),
    ]);
    out
}

/// The interface command that brings the property grid to the front.
pub const PROPERTIES: &str = "archbuilder.view.properties";

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(state: &State) -> Vec<String> {
        entries(state)
            .into_iter()
            .filter(|e| !e.separator)
            .map(|e| e.label)
            .collect()
    }

    #[test]
    fn a_running_command_offers_enter_first_then_cancel_and_undo_point_when_there_is_one() {
        let s = State {
            running: true,
            selected: 3,
            ..State::default()
        };
        assert_eq!(labels(&s), ["ab-menu-enter", "ab-menu-cancel"]);
        let e = entries(&s);
        assert_eq!(e[0].action, LineAction::Submit(String::new()));
        assert_eq!(e[1].action, LineAction::Cancel);
        let s = State {
            can_back: true,
            ..s
        };
        assert_eq!(
            labels(&s),
            ["ab-menu-enter", "ab-menu-cancel", "ab-menu-undo-point"]
        );
        assert_eq!(entries(&s)[2].action, LineAction::Back);
    }

    #[test]
    fn idle_with_nothing_selected_offers_repeat_select_all_zoom_and_unhide() {
        let s = State::default();
        assert_eq!(
            labels(&s),
            [
                "ab-menu-repeat",
                "ab-menu-new-stair",
                "ab-menu-select-all",
                "ab-menu-zoom-extents",
                "ab-menu-unhide-all"
            ]
        );
        // Repeat and Unhide all are disabled while there is nothing to repeat or show again.
        let e = entries(&s);
        assert!(!e[0].enabled && !e[4].enabled && e[1].enabled && e[2].enabled && e[3].enabled);
        let s = State {
            has_last: true,
            temporary: true,
            clipboard: true,
            ..s
        };
        let e = entries(&s);
        assert!(e.iter().all(|e| e.enabled));
        assert!(labels(&s).contains(&"ab-menu-paste".to_owned()));
    }

    #[test]
    fn a_selection_offers_the_modify_commands_hide_isolate_kind_selection_and_properties() {
        let one = State {
            selected: 2,
            kinds: vec![WALL],
            ..State::default()
        };
        let names = labels(&one);
        for want in [
            "ab-menu-delete",
            "ab-menu-move",
            "ab-menu-copy",
            "ab-menu-rotate",
            "ab-menu-mirror",
            "ab-menu-hide",
            "ab-menu-isolate",
            "ab-menu-select-kind",
            "ab-menu-select-type",
            "ab-menu-create-similar",
            "ab-menu-match-type",
            "ab-menu-edit-type",
            "ab-menu-properties",
        ] {
            assert!(names.contains(&want.to_owned()), "{want}");
        }
        // Keep only entries appear only when the selection holds several kinds.
        assert!(!names.iter().any(|n| n.starts_with("ab-menu-keep")));
        let mixed = State {
            kinds: vec![WALL, DOOR],
            ..one
        };
        let e = entries(&mixed);
        let keep: Vec<_> = e
            .iter()
            .filter(|e| e.label.starts_with("ab-menu-keep"))
            .collect();
        assert_eq!(keep.len(), 2);
        assert_eq!(
            keep[1].action,
            LineAction::Invoke(Intent::new(ui::SELECT_ONLY).with("kind", DOOR))
        );
    }
}
