//! The error codes owned by `archbuilder-commands`. Messages are in
//! `locales/en-US/archbuilder-commands.ftl`.

/// The error codes owned by `archbuilder-commands`.
pub mod codes {
    use arc_base::{Category, ErrorCode};

    /// A new project is made in a document that already has one.
    pub const NOT_EMPTY: ErrorCode = ErrorCode {
        code: "ab.cmd.document_not_empty",
        category: Category::Conflict,
        message_id: "ab-cmd-document-not-empty",
    };
    /// The object is not of the kind the command works on.
    pub const KIND_MISMATCH: ErrorCode = ErrorCode {
        code: "ab.cmd.kind_mismatch",
        category: Category::InvalidInput,
        message_id: "ab-cmd-kind-mismatch",
    };
    /// The document has no building to put a storey in.
    pub const NO_BUILDING: ErrorCode = ErrorCode {
        code: "ab.cmd.no_building",
        category: Category::NotFound,
        message_id: "ab-cmd-no-building",
    };
    /// The command names no object and the selection holds none it can use.
    pub const NOTHING_SELECTED: ErrorCode = ErrorCode {
        code: "ab.cmd.nothing_selected",
        category: Category::InvalidInput,
        message_id: "ab-cmd-nothing-selected",
    };
    /// The object has no such field, or the field cannot be edited.
    pub const FIELD_UNKNOWN: ErrorCode = ErrorCode {
        code: "ab.cmd.field_unknown",
        category: Category::InvalidInput,
        message_id: "ab-cmd-field-unknown",
    };
    /// The value does not fit the field.
    pub const VALUE_INVALID: ErrorCode = ErrorCode {
        code: "ab.cmd.value_invalid",
        category: Category::InvalidInput,
        message_id: "ab-cmd-value-invalid",
    };
    /// The last storey of a building is deleted.
    pub const LAST_STOREY: ErrorCode = ErrorCode {
        code: "ab.cmd.last_storey",
        category: Category::Conflict,
        message_id: "ab-cmd-last-storey",
    };
    /// A storey that walls stand on is deleted. The walls are not deleted with it.
    pub const STOREY_HAS_WALLS: ErrorCode = ErrorCode {
        code: "ab.cmd.storey_has_walls",
        category: Category::Conflict,
        message_id: "ab-cmd-storey-has-walls",
    };
    /// The end point of a wall is the start point.
    pub const WALL_END_SAME: ErrorCode = ErrorCode {
        code: "ab.cmd.wall_end_same",
        category: Category::InvalidInput,
        message_id: "ab-cmd-wall-end-same",
    };
    /// A door or window is placed and no wall is named, picked or selected.
    pub const NO_WALL_SELECTED: ErrorCode = ErrorCode {
        code: "ab.cmd.no_wall_selected",
        category: Category::InvalidInput,
        message_id: "ab-cmd-no-wall-selected",
    };
    /// An insert command names no door or window and the selection holds none.
    pub const NO_INSERT_SELECTED: ErrorCode = ErrorCode {
        code: "ab.cmd.no_insert_selected",
        category: Category::InvalidInput,
        message_id: "ab-cmd-no-insert-selected",
    };
    /// A click or point for a room is in no closed loop of walls.
    pub const ROOM_NO_REGION: ErrorCode = ErrorCode {
        code: "ab.cmd.room_no_region",
        category: Category::InvalidInput,
        message_id: "ab-cmd-room-no-region",
    };
    /// The region of a click or point already has a room.
    pub const ROOM_REGION_TAKEN: ErrorCode = ErrorCode {
        code: "ab.cmd.room_region_taken",
        category: Category::Conflict,
        message_id: "ab-cmd-room-region-taken",
    };
    /// No enclosed region of the storey is without a room.
    pub const ROOM_NONE_MISSING: ErrorCode = ErrorCode {
        code: "ab.cmd.room_none_missing",
        category: Category::Conflict,
        message_id: "ab-cmd-room-none-missing",
    };
    /// A region is enclosed by walls but its boundary cannot be resolved, so it gets no room.
    pub const ROOM_REGION_UNRESOLVED: ErrorCode = ErrorCode {
        code: "ab.cmd.room_region_unresolved",
        category: Category::Conflict,
        message_id: "ab-cmd-room-region-unresolved",
    };
    /// A point lies in a region smaller than the least room area: a sliver between walls.
    pub const ROOM_REGION_SLIVER: ErrorCode = ErrorCode {
        code: "ab.cmd.room_region_sliver",
        category: Category::Conflict,
        message_id: "ab-cmd-room-region-sliver",
    };
    /// Every region without a room is smaller than the least room area.
    pub const ROOM_ONLY_SLIVERS: ErrorCode = ErrorCode {
        code: "ab.cmd.room_only_slivers",
        category: Category::Conflict,
        message_id: "ab-cmd-room-only-slivers",
    };
    /// The walls or the click do not make a closed outline for a floor.
    pub const FLOOR_NO_LOOP: ErrorCode = ErrorCode {
        code: "ab.cmd.floor_no_loop",
        category: Category::InvalidInput,
        message_id: "ab-cmd-floor-no-loop",
    };
    /// Two walls that follow each other in a floor outline are parallel and do not meet.
    pub const FLOOR_WALLS_PARALLEL: ErrorCode = ErrorCode {
        code: "ab.cmd.floor_walls_parallel",
        category: Category::InvalidInput,
        message_id: "ab-cmd-floor-walls-parallel",
    };
    /// A hole is added and no floor is named or selected.
    pub const NO_FLOOR_SELECTED: ErrorCode = ErrorCode {
        code: "ab.cmd.no_floor_selected",
        category: Category::InvalidInput,
        message_id: "ab-cmd-no-floor-selected",
    };
    /// A floor has too many loops or points.
    pub const FLOOR_TOO_BIG: ErrorCode = ErrorCode {
        code: "ab.cmd.floor_too_big",
        category: Category::InvalidInput,
        message_id: "ab-cmd-floor-too-big",
    };
    /// A modify command names only objects it cannot move (storeys, levels).
    pub const NOT_MOVABLE: ErrorCode = ErrorCode {
        code: "ab.cmd.not_movable",
        category: Category::InvalidInput,
        message_id: "ab-cmd-not-movable",
    };
    /// A door or window alone is rotated or mirrored.
    pub const INSERT_ALONE: ErrorCode = ErrorCode {
        code: "ab.cmd.insert_alone",
        category: Category::InvalidInput,
        message_id: "ab-cmd-insert-alone",
    };
    /// The two points of a mirror line coincide.
    pub const MIRROR_AXIS: ErrorCode = ErrorCode {
        code: "ab.cmd.mirror_axis",
        category: Category::InvalidInput,
        message_id: "ab-cmd-mirror-axis",
    };
    /// A point or value is typed while a modify command still waits for its objects.
    pub const SELECT_BEFORE_POINTS: ErrorCode = ErrorCode {
        code: "ab.cmd.select_before_points",
        category: Category::InvalidInput,
        message_id: "ab-cmd-select-before-points",
    };
    /// The clipboard holds nothing to paste.
    pub const CLIPBOARD_EMPTY: ErrorCode = ErrorCode {
        code: "ab.cmd.clipboard_empty",
        category: Category::InvalidInput,
        message_id: "ab-cmd-clipboard-empty",
    };
    /// Paste to storey needs a storey, and none is selected.
    pub const STOREY_NOT_SELECTED: ErrorCode = ErrorCode {
        code: "ab.cmd.storey_not_selected",
        category: Category::InvalidInput,
        message_id: "ab-cmd-storey-not-selected",
    };
    /// A shortcut file is not an exported table or names something that cannot be bound.
    pub const SHORTCUTS_INVALID: ErrorCode = ErrorCode {
        code: "ab.cmd.shortcuts_invalid",
        category: Category::InvalidInput,
        message_id: "ab-cmd-shortcuts-invalid",
    };
    /// An identifier typed to select objects names no object.
    pub const OBJECT_NOT_FOUND: ErrorCode = ErrorCode {
        code: "ab.cmd.object_not_found",
        category: Category::InvalidInput,
        message_id: "ab-cmd-object-not-found",
    };
}
