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
    /// The ends of two walls lie close together but are not joined (a warning).
    pub const WALL_CORNER_OPEN: ErrorCode = ErrorCode {
        code: "ab.model.wall_corner_open",
        category: Category::Conflict,
        message_id: "ab-model-wall-corner-open",
    };
    /// An edge of a floor follows a wall that does not bound it any more (a warning).
    pub const FLOOR_EDGE_UNSUPPORTED: ErrorCode = ErrorCode {
        code: "ab.model.floor_edge_unsupported",
        category: Category::Conflict,
        message_id: "ab-model-floor-edge-unsupported",
    };
    /// A room's point is in no closed loop of walls (a warning).
    pub const ROOM_UNENCLOSED: ErrorCode = ErrorCode {
        code: "ab.model.room_unenclosed",
        category: Category::Conflict,
        message_id: "ab-model-room-unenclosed",
    };
    /// Two rooms are in one region (a warning).
    pub const ROOM_REDUNDANT: ErrorCode = ErrorCode {
        code: "ab.model.room_redundant",
        category: Category::Conflict,
        message_id: "ab-model-room-redundant",
    };
    /// Two rooms of a storey have the same number (a warning).
    pub const ROOM_NUMBER_DUPLICATE: ErrorCode = ErrorCode {
        code: "ab.model.room_number_duplicate",
        category: Category::Conflict,
        message_id: "ab-model-room-number-duplicate",
    };
    /// A floor's boundary is refused: malformed.
    pub const FLOOR_MALFORMED: ErrorCode = ErrorCode {
        code: "ab.model.floor_malformed",
        category: Category::InvalidInput,
        message_id: "ab-model-floor-malformed",
    };
    /// A floor's boundary is refused: edge short.
    pub const FLOOR_EDGE_SHORT: ErrorCode = ErrorCode {
        code: "ab.model.floor_edge_short",
        category: Category::InvalidInput,
        message_id: "ab-model-floor-edge-short",
    };
    /// A floor's boundary is refused: self intersecting.
    pub const FLOOR_SELF_INTERSECTING: ErrorCode = ErrorCode {
        code: "ab.model.floor_self_intersecting",
        category: Category::InvalidInput,
        message_id: "ab-model-floor-self-intersecting",
    };
    /// A floor's boundary is refused: area.
    pub const FLOOR_AREA: ErrorCode = ErrorCode {
        code: "ab.model.floor_area",
        category: Category::InvalidInput,
        message_id: "ab-model-floor-area",
    };
    /// A floor's boundary is refused: hole outside.
    pub const FLOOR_HOLE_OUTSIDE: ErrorCode = ErrorCode {
        code: "ab.model.floor_hole_outside",
        category: Category::InvalidInput,
        message_id: "ab-model-floor-hole-outside",
    };
    /// A floor's boundary is refused: holes overlap.
    pub const FLOOR_HOLES_OVERLAP: ErrorCode = ErrorCode {
        code: "ab.model.floor_holes_overlap",
        category: Category::InvalidInput,
        message_id: "ab-model-floor-holes-overlap",
    };
    /// Two walls of a storey lie on top of each other over a stretch: a warning. Rooms and floors
    /// treat them as one wall.
    pub const WALL_OVERLAP: ErrorCode = ErrorCode {
        code: "ab.model.wall_overlap",
        category: Category::Conflict,
        message_id: "ab-model-wall-overlap",
    };
    /// A stair has no level above its base to arrive at.
    pub const STAIR_NO_TOP: ErrorCode = ErrorCode {
        code: "ab.model.stair_no_top",
        category: Category::Conflict,
        message_id: "ab-model-stair-no-top",
    };
    /// A stair's height and highest riser give too few or too many risers.
    pub const STAIR_RISERS: ErrorCode = ErrorCode {
        code: "ab.model.stair_risers",
        category: Category::InvalidInput,
        message_id: "ab-model-stair-risers",
    };
    /// A stair's record cannot be read.
    pub const STAIR_MALFORMED: ErrorCode = ErrorCode {
        code: "ab.model.stair_malformed",
        category: Category::InvalidInput,
        message_id: "ab-model-stair-malformed",
    };
    /// A stair's stride `2R + T` lies outside the comfortable range: a warning.
    pub const STAIR_STRIDE: ErrorCode = ErrorCode {
        code: "ab.model.stair_stride",
        category: Category::InvalidInput,
        message_id: "ab-model-stair-stride",
    };
    /// A stair's shaft opening reaches a floor but cannot be cut out of it: a warning.
    pub const STAIR_OPENING_UNCUT: ErrorCode = ErrorCode {
        code: "ab.model.stair_opening_uncut",
        category: Category::Conflict,
        message_id: "ab-model-stair-opening-uncut",
    };
    /// Two stairs of one storey overlap in plan: a warning.
    pub const STAIR_OVERLAP: ErrorCode = ErrorCode {
        code: "ab.model.stair_overlap",
        category: Category::Conflict,
        message_id: "ab-model-stair-overlap",
    };
    /// A type has no layers or more than [`crate::element_type::MAX_LAYERS`].
    pub const TYPE_LAYER_COUNT: ErrorCode = ErrorCode {
        code: "ab.model.type_layer_count",
        category: Category::InvalidInput,
        message_id: "ab-model-type-layer-count",
    };
    /// A layer other than a membrane is not thicker than zero, or is too thick.
    pub const TYPE_LAYER_THICKNESS: ErrorCode = ErrorCode {
        code: "ab.model.type_layer_thickness",
        category: Category::InvalidInput,
        message_id: "ab-model-type-layer-thickness",
    };
    /// A membrane layer has a thickness.
    pub const TYPE_MEMBRANE_THICKNESS: ErrorCode = ErrorCode {
        code: "ab.model.type_membrane_thickness",
        category: Category::InvalidInput,
        message_id: "ab-model-type-membrane-thickness",
    };
    /// The core of a type holds no layer or lies outside the layers.
    pub const TYPE_CORE: ErrorCode = ErrorCode {
        code: "ab.model.type_core",
        category: Category::InvalidInput,
        message_id: "ab-model-type-core",
    };
    /// The total thickness of a type is outside the range of a wall or floor thickness.
    pub const TYPE_TOTAL: ErrorCode = ErrorCode {
        code: "ab.model.type_total",
        category: Category::InvalidInput,
        message_id: "ab-model-type-total",
    };
    /// A value an instance takes from its type differs from the type.
    pub const TYPE_MISMATCH: ErrorCode = ErrorCode {
        code: "ab.model.type_mismatch",
        category: Category::Conflict,
        message_id: "ab-model-type-mismatch",
    };
    /// A door or window type has an operation its kind does not allow.
    pub const TYPE_OPERATION: ErrorCode = ErrorCode {
        code: "ab.model.type_operation",
        category: Category::InvalidInput,
        message_id: "ab-model-type-operation",
    };
    /// A command would move, reshape or delete a pinned element.
    pub const PINNED: ErrorCode = ErrorCode {
        code: "ab.model.pinned",
        category: Category::Conflict,
        message_id: "ab-model-pinned",
    };
    /// A roof record cannot be read.
    pub const ROOF_MALFORMED: ErrorCode = ErrorCode {
        code: "ab.model.roof_malformed",
        category: Category::InvalidInput,
        message_id: "ab-model-roof-malformed",
    };
    /// A roof's footprint is not a simple loop of edges at least a millimeter long.
    pub const ROOF_FOOTPRINT: ErrorCode = ErrorCode {
        code: "ab.model.roof_footprint",
        category: Category::InvalidInput,
        message_id: "ab-model-roof-footprint",
    };
    /// A roof's outline (the footprint with its overhangs) crosses or touches itself.
    pub const ROOF_OUTLINE: ErrorCode = ErrorCode {
        code: "ab.model.roof_outline",
        category: Category::InvalidInput,
        message_id: "ab-model-roof-outline",
    };
    /// No edge of a roof defines a slope.
    pub const ROOF_NO_SLOPE: ErrorCode = ErrorCode {
        code: "ab.model.roof_no_slope",
        category: Category::InvalidInput,
        message_id: "ab-model-roof-no-slope",
    };
    /// Two neighboring edges of a roof lie on one line with different slopes.
    pub const ROOF_COLLINEAR_SLOPES: ErrorCode = ErrorCode {
        code: "ab.model.roof_collinear_slopes",
        category: Category::InvalidInput,
        message_id: "ab-model-roof-collinear-slopes",
    };
    /// No continuous roof surface covers a part of the outline.
    pub const ROOF_UNCLOSED: ErrorCode = ErrorCode {
        code: "ab.model.roof_unclosed",
        category: Category::InvalidInput,
        message_id: "ab-model-roof-unclosed",
    };
    /// The surface of a roof could not be computed reliably.
    pub const ROOF_DEGENERATE: ErrorCode = ErrorCode {
        code: "ab.model.roof_degenerate",
        category: Category::FailedOperation,
        message_id: "ab-model-roof-degenerate",
    };
    /// A wall attached to a roof or floor cannot follow its underside and keeps its own top: a warning.
    pub const WALL_ATTACH_FAILED: ErrorCode = ErrorCode {
        code: "ab.model.wall_attach_failed",
        category: Category::Conflict,
        message_id: "ab-model-wall-attach-failed",
    };
    /// A wall is attached to something that is not a roof or a floor of its building.
    pub const WALL_ATTACH_TARGET: ErrorCode = ErrorCode {
        code: "ab.model.wall_attach_target",
        category: Category::InvalidInput,
        message_id: "ab-model-wall-attach-target",
    };
}
