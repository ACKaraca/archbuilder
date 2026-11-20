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
}
