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
    /// A group instance names a definition that is not one, or the definition is gone.
    pub const GROUP_DEFINITION_MISSING: ErrorCode = ErrorCode {
        code: "ab.model.group_definition_missing",
        category: Category::Conflict,
        message_id: "ab-model-group-definition-missing",
    };
    /// A group instance does not map each level of its definition to a storey of one building.
    pub const GROUP_LEVELS_UNMAPPED: ErrorCode = ErrorCode {
        code: "ab.model.group_levels_unmapped",
        category: Category::Conflict,
        message_id: "ab-model-group-levels-unmapped",
    };
    /// A group instance excludes a member its definition does not have.
    pub const GROUP_MEMBER_UNKNOWN: ErrorCode = ErrorCode {
        code: "ab.model.group_member_unknown",
        category: Category::Conflict,
        message_id: "ab-model-group-member-unknown",
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
    /// A project information field has a value it cannot hold: an issue date that is no calendar
    /// date `YYYY-MM-DD`, or a year of construction that is not four digits. The `field` argument
    /// names it.
    pub const PROJECT_INFO_INVALID: ErrorCode = ErrorCode {
        code: "ab.model.project_info_invalid",
        category: Category::InvalidInput,
        message_id: "ab-model-project-info-invalid",
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
    /// The sketched outline of a wall opening crosses itself, has no area or leaves the
    /// opening's width and height.
    pub const INSERT_PROFILE: ErrorCode = ErrorCode {
        code: "ab.model.insert_profile",
        category: Category::InvalidInput,
        message_id: "ab-model-insert-profile",
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
    /// A ramp or railing finding (see the message `ab-model-ramp-no-top`).
    pub const RAMP_NO_TOP: ErrorCode = ErrorCode {
        code: "ab.model.ramp_no_top",
        category: Category::Conflict,
        message_id: "ab-model-ramp-no-top",
    };
    /// A ramp or railing finding (see the message `ab-model-ramp-malformed`).
    pub const RAMP_MALFORMED: ErrorCode = ErrorCode {
        code: "ab.model.ramp_malformed",
        category: Category::InvalidInput,
        message_id: "ab-model-ramp-malformed",
    };
    /// A ramp or railing finding (see the message `ab-model-ramp-slope`).
    pub const RAMP_SLOPE: ErrorCode = ErrorCode {
        code: "ab.model.ramp_slope",
        category: Category::InvalidInput,
        message_id: "ab-model-ramp-slope",
    };
    /// A ramp or railing finding (see the message `ab-model-ramp-top`).
    pub const RAMP_TOP: ErrorCode = ErrorCode {
        code: "ab.model.ramp_top",
        category: Category::InvalidInput,
        message_id: "ab-model-ramp-top",
    };
    /// A ramp or railing finding (see the message `ab-model-railing-malformed`).
    pub const RAILING_MALFORMED: ErrorCode = ErrorCode {
        code: "ab.model.railing_malformed",
        category: Category::InvalidInput,
        message_id: "ab-model-railing-malformed",
    };
    /// A ramp or railing finding (see the message `ab-model-railing-balusters`).
    pub const RAILING_BALUSTERS: ErrorCode = ErrorCode {
        code: "ab.model.railing_balusters",
        category: Category::InvalidInput,
        message_id: "ab-model-railing-balusters",
    };
    /// A ramp or railing finding (see the message `ab-model-railing-gap`).
    pub const RAILING_GAP: ErrorCode = ErrorCode {
        code: "ab.model.railing_gap",
        category: Category::InvalidInput,
        message_id: "ab-model-railing-gap",
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
    /// A curtain type has a grid rule that cannot lay lines out.
    pub const CURTAIN_RULE: ErrorCode = ErrorCode {
        code: "ab.model.curtain_rule",
        category: Category::InvalidInput,
        message_id: "ab-model-curtain-rule",
    };
    /// A curtain type's system panel is a door or window.
    pub const CURTAIN_PANEL: ErrorCode = ErrorCode {
        code: "ab.model.curtain_panel",
        category: Category::InvalidInput,
        message_id: "ab-model-curtain-panel",
    };
    /// A curtain type has a mullion or panel without a usable size.
    pub const CURTAIN_MULLION: ErrorCode = ErrorCode {
        code: "ab.model.curtain_mullion",
        category: Category::InvalidInput,
        message_id: "ab-model-curtain-mullion",
    };
    /// A curtain wall holds more edits or overrides than allowed.
    pub const CURTAIN_LIMIT: ErrorCode = ErrorCode {
        code: "ab.model.curtain_limit",
        category: Category::InvalidInput,
        message_id: "ab-model-curtain-limit",
    };
    /// A door, window or opening is hosted by a curtain wall.
    pub const CURTAIN_INSERT: ErrorCode = ErrorCode {
        code: "ab.model.curtain_insert",
        category: Category::Conflict,
        message_id: "ab-model-curtain-insert",
    };
    /// Removed grid segments leave a cell that is not a rectangle.
    pub const CURTAIN_CELL_SHAPE: ErrorCode = ErrorCode {
        code: "ab.model.curtain_cell_shape",
        category: Category::Conflict,
        message_id: "ab-model-curtain-cell-shape",
    };
    /// The wall is not a curtain wall.
    pub const CURTAIN_NOT_CURTAIN: ErrorCode = ErrorCode {
        code: "ab.model.curtain_not_curtain",
        category: Category::InvalidInput,
        message_id: "ab-model-curtain-not-curtain",
    };
    /// A grid line, cell or type a curtain command names does not exist.
    pub const CURTAIN_UNKNOWN: ErrorCode = ErrorCode {
        code: "ab.model.curtain_unknown",
        category: Category::NotFound,
        message_id: "ab-model-curtain-unknown",
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
    /// Two neighboring eaves of a roof cannot both be cut level at their own heights.
    pub const ROOF_CUT_CONFLICT: ErrorCode = ErrorCode {
        code: "ab.model.roof_cut_conflict",
        category: Category::Conflict,
        message_id: "ab-model-roof-cut-conflict",
    };
    /// A wall attached to a roof or floor cannot follow its underside and keeps its own top: a warning.
    pub const WALL_ATTACH_FAILED: ErrorCode = ErrorCode {
        code: "ab.model.wall_attach_failed",
        category: Category::Conflict,
        message_id: "ab-model-wall-attach-failed",
    };
    /// The shape numbers of a wall (arc, lean, profile, bands) cannot make a solid.
    pub const WALL_SHAPE_INVALID: ErrorCode = ErrorCode {
        code: "ab.model.wall_shape_invalid",
        category: Category::InvalidInput,
        message_id: "ab-model-wall-shape-invalid",
    };
    /// A wall's profile has an opening or an outline that does not fit.
    pub const WALL_PROFILE_INVALID: ErrorCode = ErrorCode {
        code: "ab.model.wall_profile_invalid",
        category: Category::InvalidInput,
        message_id: "ab-model-wall-profile-invalid",
    };
    /// Doors or windows of a wall fall outside the profile it was given.
    pub const WALL_PROFILE_INSERTS: ErrorCode = ErrorCode {
        code: "ab.model.wall_profile_inserts",
        category: Category::Conflict,
        message_id: "ab-model-wall-profile-inserts",
    };
    /// Two shapes of a wall cannot be combined.
    pub const WALL_SHAPE_COMBINATION: ErrorCode = ErrorCode {
        code: "ab.model.wall_shape_combination",
        category: Category::InvalidInput,
        message_id: "ab-model-wall-shape-combination",
    };
    /// A stacked wall type is not usable.
    pub const WALL_STACK_INVALID: ErrorCode = ErrorCode {
        code: "ab.model.wall_stack_invalid",
        category: Category::InvalidInput,
        message_id: "ab-model-wall-stack-invalid",
    };
    /// A band of a wall does not fit it.
    pub const WALL_BAND_INVALID: ErrorCode = ErrorCode {
        code: "ab.model.wall_band_invalid",
        category: Category::InvalidInput,
        message_id: "ab-model-wall-band-invalid",
    };
    /// A wall is attached to something that is not a roof or a floor of its building.
    pub const WALL_ATTACH_TARGET: ErrorCode = ErrorCode {
        code: "ab.model.wall_attach_target",
        category: Category::InvalidInput,
        message_id: "ab-model-wall-attach-target",
    };
    /// A ceiling has no readable boundary.
    pub const CEILING_MALFORMED: ErrorCode = ErrorCode {
        code: "ab.model.ceiling_malformed",
        category: Category::InvalidInput,
        message_id: "ab-model-ceiling-malformed",
    };
    /// A ceiling's boundary is refused: a loop crosses itself, an edge is too short, or an area is out of range.
    pub const CEILING_BOUNDARY: ErrorCode = ErrorCode {
        code: "ab.model.ceiling_boundary",
        category: Category::InvalidInput,
        message_id: "ab-model-ceiling-boundary",
    };
    /// An edge of a ceiling follows a wall that does not bound it any more (a warning).
    pub const CEILING_EDGE_UNSUPPORTED: ErrorCode = ErrorCode {
        code: "ab.model.ceiling_edge_unsupported",
        category: Category::Conflict,
        message_id: "ab-model-ceiling-edge-unsupported",
    };
    /// The footprint of a shaft or vertical opening is refused.
    pub const OPENING_FOOTPRINT: ErrorCode = ErrorCode {
        code: "ab.model.opening_footprint",
        category: Category::InvalidInput,
        message_id: "ab-model-opening-footprint",
    };
    /// A vertical opening lies outside the element that hosts it (a warning).
    pub const OPENING_ABOVE_HOST: ErrorCode = ErrorCode {
        code: "ab.model.opening_above_host",
        category: Category::Conflict,
        message_id: "ab-model-opening-outside-host",
    };
    /// The profile of a floor edge is not a simple loop, or the floor edge names no edge.
    pub const FLOOR_EDGE_PROFILE: ErrorCode = ErrorCode {
        code: "ab.model.floor_edge_profile",
        category: Category::InvalidInput,
        message_id: "ab-model-floor-edge-profile",
    };
    /// A floor edge names an edge its floor no longer has (a warning).
    pub const FLOOR_EDGE_ORPHAN: ErrorCode = ErrorCode {
        code: "ab.model.floor_edge_orphan",
        category: Category::Conflict,
        message_id: "ab-model-floor-edge-orphan",
    };
    /// An elevator's sizes are out of range, or its door is wider than the shaft.
    pub const ELEVATOR_INVALID: ErrorCode = ErrorCode {
        code: "ab.model.elevator_invalid",
        category: Category::InvalidInput,
        message_id: "ab-model-elevator-invalid",
    };
    /// A dormer's sizes are out of range.
    pub const DORMER_INVALID: ErrorCode = ErrorCode {
        code: "ab.model.dormer_invalid",
        category: Category::InvalidInput,
        message_id: "ab-model-dormer-invalid",
    };
    /// The profile of a roof edge element is not a simple loop, or it names no edge.
    pub const ROOF_EDGE_PROFILE: ErrorCode = ErrorCode {
        code: "ab.model.roof_edge_profile",
        category: Category::InvalidInput,
        message_id: "ab-model-roof-edge-profile",
    };
    /// A roof edge element names an edge its roof no longer has, or one without a slope (a
    /// warning).
    pub const ROOF_EDGE_ORPHAN: ErrorCode = ErrorCode {
        code: "ab.model.roof_edge_orphan",
        category: Category::Conflict,
        message_id: "ab-model-roof-edge-orphan",
    };
    /// A stair has less clear height above its treads or landings than its minimum (a warning).
    pub const STAIR_HEADROOM: ErrorCode = ErrorCode {
        code: "ab.model.stair_headroom",
        category: Category::Conflict,
        message_id: "ab-model-stair-headroom",
    };
    /// The profile of a roof by extrusion is unreadable or has too few points, a segment of no
    /// length or a point out of range.
    pub const ROOF_EXTRUSION_PROFILE: ErrorCode = ErrorCode {
        code: "ab.model.roof_extrusion_profile",
        category: Category::InvalidInput,
        message_id: "ab-model-roof-extrusion-profile",
    };
    /// The thickness of a roof by extrusion makes its section cross itself.
    pub const ROOF_EXTRUSION_SECTION: ErrorCode = ErrorCode {
        code: "ab.model.roof_extrusion_section",
        category: Category::InvalidInput,
        message_id: "ab-model-roof-extrusion-section",
    };
    /// The extrusion of a roof is too short or too long.
    pub const ROOF_EXTRUSION_SPAN: ErrorCode = ErrorCode {
        code: "ab.model.roof_extrusion_span",
        category: Category::InvalidInput,
        message_id: "ab-model-roof-extrusion-span",
    };
    /// A floor's slope is steeper than the limit.
    pub const FLOOR_SLOPE: ErrorCode = ErrorCode {
        code: "ab.model.floor_slope",
        category: Category::InvalidInput,
        message_id: "ab-model-floor-slope",
    };
    /// A ceiling's slope is steeper than the limit.
    pub const CEILING_SLOPE: ErrorCode = ErrorCode {
        code: "ab.model.ceiling_slope",
        category: Category::InvalidInput,
        message_id: "ab-model-ceiling-slope",
    };
    /// A ceiling's underside leaves the range from its storey's level to the level above.
    pub const CEILING_RANGE: ErrorCode = ErrorCode {
        code: "ab.model.ceiling_range",
        category: Category::InvalidInput,
        message_id: "ab-model-ceiling-range",
    };
    /// A ceiling's underside passes through the slab or roof above it.
    pub const CEILING_RANGE_ELEMENT: ErrorCode = ErrorCode {
        code: "ab.model.ceiling_range_element",
        category: Category::InvalidInput,
        message_id: "ab-model-ceiling-range-element",
    };
    /// A grid or reference plane is shorter than a millimeter, or a work plane has no plane.
    pub const DATUM_DEGENERATE: ErrorCode = ErrorCode {
        code: "ab.model.datum_degenerate",
        category: Category::InvalidInput,
        message_id: "ab-model-datum-degenerate",
    };
    /// Several grids of a building have the same label (a warning).
    pub const DATUM_LABEL_DUPLICATE: ErrorCode = ErrorCode {
        code: "ab.model.datum_label_duplicate",
        category: Category::Conflict,
        message_id: "ab-model-datum-label-duplicate",
    };
    /// Geometry lies farther from the internal origin than the document's precision distance (a
    /// warning).
    pub const LOCATION_DISTANT: ErrorCode = ErrorCode {
        code: "ab.model.location_distant",
        category: Category::Conflict,
        message_id: "ab-model-location-distant",
    };
    /// A plan view's range is out of order: view depth, bottom, cut and top must rise.
    pub const VIEW_RANGE: ErrorCode = ErrorCode {
        code: "ab.model.view_range",
        category: Category::InvalidInput,
        message_id: "ab-model-view-range",
    };
    /// A formula identifier is not an ASCII identifier, or is `true`, `false`, a function or a
    /// unit.
    pub const PARAM_IDENTIFIER_INVALID: ErrorCode = ErrorCode {
        code: "ab.model.param_identifier_invalid",
        category: Category::InvalidInput,
        message_id: "ab-model-param-identifier-invalid",
    };
    /// Another parameter or document variable has the formula identifier.
    pub const PARAM_IDENTIFIER_TAKEN: ErrorCode = ErrorCode {
        code: "ab.model.param_identifier_taken",
        category: Category::Conflict,
        message_id: "ab-model-param-identifier-taken",
    };
    /// A value does not fit its data type: a literal of another kind, a formula where only
    /// literals are allowed, an integer that is not whole.
    pub const PARAM_VALUE_INVALID: ErrorCode = ErrorCode {
        code: "ab.model.param_value_invalid",
        category: Category::InvalidInput,
        message_id: "ab-model-param-value-invalid",
    };
    /// Formulas read each other in a cycle; the argument `cycle` names it.
    pub const PARAM_CYCLE: ErrorCode = ErrorCode {
        code: "ab.model.param_cycle",
        category: Category::Conflict,
        message_id: "ab-model-param-cycle",
    };
    /// A parameter value breaks its rule (a refusal for a reject rule, a warning otherwise).
    pub const PARAM_RULE: ErrorCode = ErrorCode {
        code: "ab.model.param_rule",
        category: Category::Conflict,
        message_id: "ab-model-param-rule",
    };
    /// A document would hold more document variables or project parameters than the limit.
    pub const PARAM_TOO_MANY: ErrorCode = ErrorCode {
        code: "ab.model.param_too_many",
        category: Category::LimitExceeded,
        message_id: "ab-model-param-too-many",
    };
    /// A parameter is bound to something that is not a category of physical elements, or a
    /// value is set on an object the parameter is not bound to in that scope.
    pub const PARAM_CATEGORY: ErrorCode = ErrorCode {
        code: "ab.model.param_category",
        category: Category::InvalidInput,
        message_id: "ab-model-param-category",
    };
    /// A family definition is invalid; the arguments `part` and `reason` name what.
    pub const FAMILY_INVALID: ErrorCode = ErrorCode {
        code: "ab.family.invalid",
        category: Category::InvalidInput,
        message_id: "ab-family-invalid",
    };
    /// A family definition, text or evaluation passes a limit; `limit` names it.
    pub const FAMILY_LIMIT: ErrorCode = ErrorCode {
        code: "ab.family.limit_exceeded",
        category: Category::LimitExceeded,
        message_id: "ab-family-limit-exceeded",
    };
    /// A canonical family text is malformed; `line` and `reason` say where and why.
    pub const FAMILY_TEXT: ErrorCode = ErrorCode {
        code: "ab.family.text_invalid",
        category: Category::InvalidInput,
        message_id: "ab-family-text-invalid",
    };
    /// A form of a family cannot be built; `form` names it and `reason` the cause.
    pub const FAMILY_FORM: ErrorCode = ErrorCode {
        code: "ab.family.form_failed",
        category: Category::InvalidInput,
        message_id: "ab-family-form-failed",
    };
    /// A reference plane of a family does not evaluate to a position; `plane` names it.
    pub const FAMILY_PLANE: ErrorCode = ErrorCode {
        code: "ab.family.plane_invalid",
        category: Category::InvalidInput,
        message_id: "ab-family-plane-invalid",
    };
    /// The family has no type of that name.
    pub const FAMILY_TYPE_UNKNOWN: ErrorCode = ErrorCode {
        code: "ab.family.type_unknown",
        category: Category::NotFound,
        message_id: "ab-family-type-unknown",
    };
    /// A test case of a family failed; `test` and `reason` say which and why.
    pub const FAMILY_TEST: ErrorCode = ErrorCode {
        code: "ab.family.test_failed",
        category: Category::Conflict,
        message_id: "ab-family-test-failed",
    };
    /// The family needs a newer function set than this build has.
    pub const FAMILY_FUNCTIONS: ErrorCode = ErrorCode {
        code: "ab.family.functions_newer",
        category: Category::Conflict,
        message_id: "ab-family-functions-newer",
    };
    /// The checksum of a family definition does not match its text.
    pub const FAMILY_CHECKSUM: ErrorCode = ErrorCode {
        code: "ab.family.checksum_mismatch",
        category: Category::Conflict,
        message_id: "ab-family-checksum-mismatch",
    };
    /// A package depends on a package that is not available.
    pub const FAMILY_DEPENDENCY: ErrorCode = ErrorCode {
        code: "ab.family.dependency_missing",
        category: Category::Conflict,
        message_id: "ab-family-dependency-missing",
    };
    /// A family package is malformed or incomplete.
    pub const FAMILY_PACKAGE: ErrorCode = ErrorCode {
        code: "ab.family.package_invalid",
        category: Category::InvalidInput,
        message_id: "ab-family-package-invalid",
    };
    /// The family and version are loaded already with another checksum.
    pub const FAMILY_CONFLICT: ErrorCode = ErrorCode {
        code: "ab.family.family_conflict",
        category: Category::Conflict,
        message_id: "ab-family-family-conflict",
    };
    /// A major update needs a value mapping for parameters that were removed or retyped.
    pub const FAMILY_MAPPING: ErrorCode = ErrorCode {
        code: "ab.family.mapping_required",
        category: Category::Conflict,
        message_id: "ab-family-mapping-required",
    };
    /// The version is not newer than the loaded one, or is larger than the step chosen.
    pub const FAMILY_VERSION: ErrorCode = ErrorCode {
        code: "ab.family.version_invalid",
        category: Category::InvalidInput,
        message_id: "ab-family-version-invalid",
    };
    /// A family instance would not evaluate.
    pub const FAMILY_EVALUATION: ErrorCode = ErrorCode {
        code: "ab.family.evaluation_failed",
        category: Category::Conflict,
        message_id: "ab-family-evaluation-failed",
    };
}
