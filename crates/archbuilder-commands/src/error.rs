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
    /// A selection holds an element a group cannot take in (only walls with their openings,
    /// components and rooms are taken).
    pub const GROUP_KIND_UNSUPPORTED: ErrorCode = ErrorCode {
        code: "ab.cmd.group_kind_unsupported",
        category: Category::InvalidInput,
        message_id: "ab-cmd-group-kind-unsupported",
    };
    /// A group definition has instances; they are separated or deleted first.
    pub const GROUP_IN_USE: ErrorCode = ErrorCode {
        code: "ab.cmd.group_in_use",
        category: Category::Conflict,
        message_id: "ab-cmd-group-in-use",
    };
    /// A generated element of an instance was changed directly; a regeneration would overwrite it.
    /// Edit the group instead, or separate the instance.
    pub const GROUP_EDITED: ErrorCode = ErrorCode {
        code: "ab.cmd.group_edited",
        category: Category::Conflict,
        message_id: "ab-cmd-group-edited",
    };
    /// A storey mapped by a group instance cannot be removed until the instance is separated.
    pub const GROUP_LEVEL_MAPPED: ErrorCode = ErrorCode {
        code: "ab.cmd.group_level_mapped",
        category: Category::Conflict,
        message_id: "ab-cmd-group-level-mapped",
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
    /// An operation that would cut or offset the outline of a floor with arc edges, which it
    /// cannot keep.
    pub const FLOOR_ARCS_UNSUPPORTED: ErrorCode = ErrorCode {
        code: "ab.cmd.floor_arcs_unsupported",
        category: Category::InvalidInput,
        message_id: "ab-cmd-floor-arcs-unsupported",
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
    /// A field of an instance comes from its type and is not edited on the instance.
    pub const FIELD_FROM_TYPE: ErrorCode = ErrorCode {
        code: "ab.cmd.field_from_type",
        category: Category::InvalidInput,
        message_id: "ab-cmd-field-from-type",
    };
    /// No type of the kind has the name or identifier given.
    pub const TYPE_UNKNOWN: ErrorCode = ErrorCode {
        code: "ab.cmd.type_unknown",
        category: Category::InvalidInput,
        message_id: "ab-cmd-type-unknown",
    };
    /// A type is still used by instances and no replacement was given.
    pub const TYPE_IN_USE: ErrorCode = ErrorCode {
        code: "ab.cmd.type_in_use",
        category: Category::Conflict,
        message_id: "ab-cmd-type-in-use",
    };
    /// The file is not a template a project can start from: it holds no project, site, building
    /// and storey, or cannot be read as a document.
    pub const TEMPLATE_INVALID: ErrorCode = ErrorCode {
        code: "ab.cmd.template_invalid",
        category: Category::InvalidInput,
        message_id: "ab-cmd-template-invalid",
    };
    /// A project is started from a template in a document that already has elements.
    pub const TEMPLATE_TARGET_HAS_ELEMENTS: ErrorCode = ErrorCode {
        code: "ab.cmd.template_target_has_elements",
        category: Category::Conflict,
        message_id: "ab-cmd-template-target-has-elements",
    };
    /// The last type of a kind cannot be deleted.
    pub const TYPE_LAST: ErrorCode = ErrorCode {
        code: "ab.cmd.type_last",
        category: Category::Conflict,
        message_id: "ab-cmd-type-last",
    };
    /// None of the objects is of the kind of the type.
    pub const TYPE_KIND: ErrorCode = ErrorCode {
        code: "ab.cmd.type_kind",
        category: Category::InvalidInput,
        message_id: "ab-cmd-type-kind",
    };
    /// No wall lies at the picked point.
    pub const NO_WALL_AT: ErrorCode = ErrorCode {
        code: "ab.cmd.no_wall_at",
        category: Category::NotFound,
        message_id: "ab-cmd-no-wall-at",
    };
    /// The command is for straight walls and the wall is an arc.
    pub const ARC_WALL_UNSUPPORTED: ErrorCode = ErrorCode {
        code: "ab.cmd.arc_wall_unsupported",
        category: Category::InvalidInput,
        message_id: "ab-cmd-arc-wall-unsupported",
    };
    /// No wall face, wall center line or floor edge lies at the picked point.
    pub const NO_LINE_AT: ErrorCode = ErrorCode {
        code: "ab.cmd.no_line_at",
        category: Category::NotFound,
        message_id: "ab-cmd-no-line-at",
    };
    /// The two lines are parallel and never meet.
    pub const PARALLEL: ErrorCode = ErrorCode {
        code: "ab.cmd.parallel",
        category: Category::InvalidInput,
        message_id: "ab-cmd-parallel",
    };
    /// The same wall was picked twice.
    pub const SAME_WALL: ErrorCode = ErrorCode {
        code: "ab.cmd.same_wall",
        category: Category::InvalidInput,
        message_id: "ab-cmd-same-wall",
    };
    /// A split would leave a piece shorter than the shortest wall.
    pub const SPLIT_END: ErrorCode = ErrorCode {
        code: "ab.cmd.split_end",
        category: Category::InvalidInput,
        message_id: "ab-cmd-split-end",
    };
    /// A door or window lies across the split point.
    pub const SPLIT_INSERT: ErrorCode = ErrorCode {
        code: "ab.cmd.split_insert",
        category: Category::Conflict,
        message_id: "ab-cmd-split-insert",
    };
    /// Two walls are not collinear, do not touch end to end, or differ in type, height, base, top or location line.
    pub const MERGE_REFUSED: ErrorCode = ErrorCode {
        code: "ab.cmd.merge_refused",
        category: Category::Conflict,
        message_id: "ab-cmd-merge-refused",
    };
    /// A door or window can only be arrayed along its own wall.
    pub const ARRAY_INSERT: ErrorCode = ErrorCode {
        code: "ab.cmd.array_insert",
        category: Category::InvalidInput,
        message_id: "ab-cmd-array-insert",
    };
    /// The selection holds no wall or floor to offset, or the side point lies on its line.
    pub const NOTHING_TO_OFFSET: ErrorCode = ErrorCode {
        code: "ab.cmd.nothing_to_offset",
        category: Category::InvalidInput,
        message_id: "ab-cmd-nothing-to-offset",
    };
    /// The command names no roof and none is selected.
    pub const NO_ROOF_SELECTED: ErrorCode = ErrorCode {
        code: "ab.cmd.no_roof_selected",
        category: Category::InvalidInput,
        message_id: "ab-cmd-no-roof-selected",
    };
    /// A roof has no edge or vertex with the given number.
    pub const ROOF_EDGE_INDEX: ErrorCode = ErrorCode {
        code: "ab.cmd.roof_edge_index",
        category: Category::InvalidInput,
        message_id: "ab-cmd-roof-edge-index",
    };
    /// No roof or floor is above a wall to attach its top to.
    pub const NO_ATTACH_TARGET: ErrorCode = ErrorCode {
        code: "ab.cmd.no_attach_target",
        category: Category::NotFound,
        message_id: "ab-cmd-no-attach-target",
    };
    /// A wall top can attach only to a roof or a floor.
    pub const ATTACH_TARGET_KIND: ErrorCode = ErrorCode {
        code: "ab.cmd.attach_target_kind",
        category: Category::InvalidInput,
        message_id: "ab-cmd-attach-target-kind",
    };
    /// A slope cannot be read or is outside the range of a roof edge.
    pub const SLOPE_INVALID: ErrorCode = ErrorCode {
        code: "ab.cmd.slope_invalid",
        category: Category::InvalidInput,
        message_id: "ab-cmd-slope-invalid",
    };
    /// No ceiling is named or selected.
    pub const NO_CEILING_SELECTED: ErrorCode = ErrorCode {
        code: "ab.cmd.no_ceiling_selected",
        category: Category::InvalidInput,
        message_id: "ab-cmd-no-ceiling-selected",
    };
    /// A work plane needs a level, a grid, a reference plane or three points that make a plane.
    pub const NO_WORK_PLANE: ErrorCode = ErrorCode {
        code: "ab.cmd.no_work_plane",
        category: Category::InvalidInput,
        message_id: "ab-cmd-no-work-plane",
    };
    /// A grid or reference plane needs two points, or a wall to take its line from.
    pub const NO_LINE: ErrorCode = ErrorCode {
        code: "ab.cmd.no_line",
        category: Category::InvalidInput,
        message_id: "ab-cmd-no-line",
    };
    /// No floor, roof or ceiling is named or selected to host a vertical opening.
    pub const NO_HOST_SELECTED: ErrorCode = ErrorCode {
        code: "ab.cmd.no_host_selected",
        category: Category::InvalidInput,
        message_id: "ab-cmd-no-host-selected",
    };
    /// The last plan view of a storey is deleted.
    pub const LAST_VIEW: ErrorCode = ErrorCode {
        code: "ab.cmd.last_view",
        category: Category::Conflict,
        message_id: "ab-cmd-last-view",
    };
    /// No selection set has the given name, or the document has none.
    pub const SET_UNKNOWN: ErrorCode = ErrorCode {
        code: "ab.cmd.set_unknown",
        category: Category::NotFound,
        message_id: "ab-cmd-set-unknown",
    };
}
