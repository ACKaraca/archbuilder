//! The error codes owned by `archbuilder-model`. Messages are in
//! `locales/en-US/archbuilder-model.ftl`.

/// The error codes owned by `archbuilder-model`.
pub mod codes {
    use arc_base::{Category, ErrorCode};

    /// A name is empty or only white space.
    pub const NAME_EMPTY: ErrorCode = ErrorCode {
        code: "ab.model.name_empty",
        category: Category::InvalidInput,
        message_id: "ab-model-name-empty",
    };
    /// Another object of the same kind in the same scope has the name, compared by
    /// [`crate::names::fold`] (case folded, `i`, `ı`, `İ` and `I` alike, surrounding white space
    /// removed).
    pub const NAME_TAKEN: ErrorCode = ErrorCode {
        code: "ab.model.name_taken",
        category: Category::Conflict,
        message_id: "ab-model-name-taken",
    };
    /// An object is not contained in an object of the kind its type belongs to.
    pub const PARENT_INVALID: ErrorCode = ErrorCode {
        code: "ab.model.parent_invalid",
        category: Category::Conflict,
        message_id: "ab-model-parent-invalid",
    };
    /// A document holds more than one project.
    pub const PROJECT_NOT_UNIQUE: ErrorCode = ErrorCode {
        code: "ab.model.project_not_unique",
        category: Category::Conflict,
        message_id: "ab-model-project-not-unique",
    };
    /// A wall's baseline is shorter than [`crate::types::MIN_WALL_LENGTH`].
    pub const WALL_TOO_SHORT: ErrorCode = ErrorCode {
        code: "ab.model.wall_too_short",
        category: Category::InvalidInput,
        message_id: "ab-model-wall-too-short",
    };
    /// The level a wall's top refers to is not above the wall's base.
    pub const WALL_TOP_INVALID: ErrorCode = ErrorCode {
        code: "ab.model.wall_top_invalid",
        category: Category::InvalidInput,
        message_id: "ab-model-wall-top-invalid",
    };
    /// A level moved to or below the base of walls whose top follows it.
    pub const LEVEL_BELOW_WALLS: ErrorCode = ErrorCode {
        code: "ab.model.level_below_walls",
        category: Category::Conflict,
        message_id: "ab-model-level-below-walls",
    };
    /// A storey refers to a level of another building, to a level that is not flagged as a
    /// storey level, or to a level that another storey already uses.
    pub const LEVEL_INVALID: ErrorCode = ErrorCode {
        code: "ab.model.level_invalid",
        category: Category::Conflict,
        message_id: "ab-model-level-invalid",
    };
    /// A door or window does not lie inside the clear length of its wall.
    pub const INSERT_OUTSIDE: ErrorCode = ErrorCode {
        code: "ab.model.insert_outside",
        category: Category::Conflict,
        message_id: "ab-model-insert-outside",
    };
    /// A door or window reaches the top of its wall or above it.
    pub const INSERT_TOO_TALL: ErrorCode = ErrorCode {
        code: "ab.model.insert_too_tall",
        category: Category::Conflict,
        message_id: "ab-model-insert-too-tall",
    };
    /// A door or window overlaps another one in the same wall.
    pub const INSERT_OVERLAP: ErrorCode = ErrorCode {
        code: "ab.model.insert_overlap",
        category: Category::Conflict,
        message_id: "ab-model-insert-overlap",
    };
    /// A door or window has an operation its kind does not have.
    pub const INSERT_OPERATION: ErrorCode = ErrorCode {
        code: "ab.model.insert_operation",
        category: Category::InvalidInput,
        message_id: "ab-model-insert-operation",
    };
    /// The frame of a door or window leaves no clear opening.
    pub const INSERT_FRAME: ErrorCode = ErrorCode {
        code: "ab.model.insert_frame",
        category: Category::InvalidInput,
        message_id: "ab-model-insert-frame",
    };
    /// A door or window would open into the body of another wall that meets or crosses its wall.
    pub const INSERT_MEETS_WALL: ErrorCode = ErrorCode {
        code: "ab.model.insert_meets_wall",
        category: Category::Conflict,
        message_id: "ab-model-insert-meets-wall",
    };
}
