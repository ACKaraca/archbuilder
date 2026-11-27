//! The error codes owned by `archbuilder-io`. Messages are in `locales/en-US/archbuilder-io.ftl`.

/// The error codes owned by `archbuilder-io`.
pub mod codes {
    use arc_base::{Category, ErrorCode};

    /// The document has no project, site and building to export.
    pub const NO_STRUCTURE: ErrorCode = ErrorCode {
        code: "ab.io.no_structure",
        category: Category::NotFound,
        message_id: "ab-io-no-structure",
    };
    /// The export command was given an empty path.
    pub const PATH_REQUIRED: ErrorCode = ErrorCode {
        code: "ab.io.path_required",
        category: Category::InvalidInput,
        message_id: "ab-io-path-required",
    };
}

/// The loss reasons of the IFC export.
pub mod reasons {
    use arc_io_common::loss::{Outcome, Reason, Severity};

    /// An object that was not exported at all.
    pub const NOT_EXPORTED: Reason = Reason {
        code: "ab.io.ifc.object_not_exported",
        outcome: Outcome::Dropped,
        severity: Severity::Error,
        message_id: "ab-io-loss-object-not-exported",
    };
    /// An element written without its body.
    pub const BODY_NOT_EXPORTED: Reason = Reason {
        code: "ab.io.ifc.body_not_exported",
        outcome: Outcome::Dropped,
        severity: Severity::Error,
        message_id: "ab-io-loss-body-not-exported",
    };
    /// A quantity left out of a quantity set.
    pub const QUANTITY_NOT_EXPORTED: Reason = Reason {
        code: "ab.io.ifc.quantity_not_exported",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-quantity-not-exported",
    };
    /// A room that no closed loop of walls encloses: it has no region to write as a space.
    pub const ROOM_UNENCLOSED: Reason = Reason {
        code: "ab.io.ifc.room_unenclosed",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-room-unenclosed",
    };
    /// A room whose region another room already holds: one space for each region.
    pub const ROOM_REDUNDANT: Reason = Reason {
        code: "ab.io.ifc.room_redundant",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-room-redundant",
    };
    /// A casement window: IFC4 has no operation attribute on the window itself.
    pub const WINDOW_OPERATION: Reason = Reason {
        code: "ab.io.ifc.window_operation_approximated",
        outcome: Outcome::Approximated,
        severity: Severity::Warning,
        message_id: "ab-io-loss-window-operation",
    };
    /// A floor edge element: IFC4 has no element for an edge of a slab, so it is written as a
    /// molding covering that carries its swept body.
    pub const FLOOR_EDGE_AS_COVERING: Reason = Reason {
        code: "ab.io.ifc.floor_edge_as_covering",
        outcome: Outcome::Approximated,
        severity: Severity::Info,
        message_id: "ab-io-loss-floor-edge-as-covering",
    };
    /// A floor edge element that names edges its floor no longer has: those edges are not written.
    pub const FLOOR_EDGE_ORPHAN: Reason = Reason {
        code: "ab.io.ifc.floor_edge_orphan",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-floor-edge-orphan",
    };
    /// The geodetic reference of the project (coordinate system, survey point position, true
    /// north): the IFC writer of the foundation has no map conversion or projected coordinate
    /// system entity yet, so it is left out.
    pub const GEOREFERENCE_NOT_WRITTEN: Reason = Reason {
        code: "ab.io.ifc.georeference_not_written",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-georeference-not-written",
    };
    /// An elevator: the writer of the pinned foundation has no transport element entity yet
    /// (`IfcTransportElement` of the predefined type `ELEVATOR`), so the elevator is left out; the
    /// openings its shaft cuts in the floors are written.
    pub const ELEVATOR_NOT_WRITTEN: Reason = Reason {
        code: "ab.io.ifc.elevator_not_written",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-elevator-not-written",
    };
    /// A dormer: IFC4 has no dormer element, so it is written as the opening in its roof, a wall
    /// for each of its front and cheek walls and a roof for its own small roof.
    pub const DORMER_AS_PARTS: Reason = Reason {
        code: "ab.io.ifc.dormer_as_parts",
        outcome: Outcome::Approximated,
        severity: Severity::Info,
        message_id: "ab-io-loss-dormer-as-parts",
    };
    /// A fascia: IFC4 has no fascia, so it is written as a molding covering that carries its
    /// swept body.
    pub const FASCIA_AS_COVERING: Reason = Reason {
        code: "ab.io.ifc.fascia_as_covering",
        outcome: Outcome::Approximated,
        severity: Severity::Info,
        message_id: "ab-io-loss-fascia-as-covering",
    };
    /// A gutter: IFC4 maps it to a pipe segment of the predefined type `GUTTER`, which the writer
    /// of the pinned foundation does not have yet, so the gutter is left out.
    pub const GUTTER_NOT_WRITTEN: Reason = Reason {
        code: "ab.io.ifc.gutter_not_written",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-gutter-not-written",
    };
    /// A roof edge element that names edges its roof no longer has or that define no slope: those
    /// edges are not written.
    pub const ROOF_EDGE_ORPHAN: Reason = Reason {
        code: "ab.io.ifc.roof_edge_orphan",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-roof-edge-orphan",
    };
    /// The project information could not be added to the IFC file: the file is not the shape the
    /// writer produces, so the project's long name, phase, addresses, actors and building
    /// properties are left out.
    pub const PROJECT_INFO_NOT_WRITTEN: Reason = Reason {
        code: "ab.io.ifc.project_info_not_written",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-project-info-not-written",
    };
    /// The issue date of the project: IFC4 has no attribute for it, so it is left out.
    pub const ISSUE_DATE_NOT_WRITTEN: Reason = Reason {
        code: "ab.io.ifc.issue_date_not_written",
        outcome: Outcome::Dropped,
        severity: Severity::Info,
        message_id: "ab-io-loss-issue-date-not-written",
    };
    /// A reference plane or work plane: a drawing aid with no IFC building element, left out.
    pub const DATUM_NOT_WRITTEN: Reason = Reason {
        code: "ab.io.ifc.datum_not_written",
        outcome: Outcome::Dropped,
        severity: Severity::Info,
        message_id: "ab-io-loss-datum-not-written",
    };
    /// A selection set: a list of elements that could become an `IfcGroup`, which the writer of the
    /// foundation cannot write yet, so it is left out.
    pub const SET_NOT_WRITTEN: Reason = Reason {
        code: "ab.io.ifc.selection_set_not_written",
        outcome: Outcome::Dropped,
        severity: Severity::Info,
        message_id: "ab-io-loss-selection-set-not-written",
    };
    /// The grid lines of a building are all in one direction: `IfcGrid` needs a U and a V family
    /// of axes, so no grid is written and each line is named.
    pub const GRID_ONE_DIRECTION: Reason = Reason {
        code: "ab.io.ifc.grid_one_direction",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-grid-one-direction",
    };
    /// A grid line of several segments: the writer's grid axis is one line or one arc, so the line
    /// is named rather than shortened to its ends.
    pub const GRID_MULTI_SEGMENT: Reason = Reason {
        code: "ab.io.ifc.grid_multi_segment",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-grid-multi-segment",
    };
    /// A curtain wall with doors or windows as panels: IFC4 aggregates them in the curtain wall,
    /// which the writer of the foundation does not do, so each fills an opening in the wall.
    pub const CURTAIN_PANEL_FILLED: Reason = Reason {
        code: "ab.io.ifc.curtain_panel_filled",
        outcome: Outcome::Approximated,
        severity: Severity::Info,
        message_id: "ab-io-loss-curtain-panel-filled",
    };
    /// An arc edge of a boundary or an arched opening, written as chords within the model's
    /// flattening tolerance: IFC bodies here are faceted.
    pub const ARC_FACETED: Reason = Reason {
        code: "ab.io.ifc.arc_faceted",
        outcome: Outcome::Approximated,
        severity: Severity::Info,
        message_id: "ab-io-loss-arc-faceted",
    };
    /// A shaft or vertical opening whose cut could not be applied to the body of the element it
    /// passes through: the element's body is written without it, the opening is still in the file.
    pub const CUT_NOT_APPLIED: Reason = Reason {
        code: "ab.io.ifc.cut_not_applied",
        outcome: Outcome::Approximated,
        severity: Severity::Warning,
        message_id: "ab-io-loss-cut-not-applied",
    };
    /// The exported file could not be read back by the independent reader.
    pub const VALIDATION_UNREADABLE: Reason = Reason {
        code: "ab.io.ifc.validation_unreadable",
        outcome: Outcome::Failed,
        severity: Severity::Error,
        message_id: "ab-io-validation-unreadable",
    };
    /// An object of the model is not in the exported file.
    pub const VALIDATION_MISSING: Reason = Reason {
        code: "ab.io.ifc.validation_missing",
        outcome: Outcome::Failed,
        severity: Severity::Error,
        message_id: "ab-io-validation-missing",
    };
    /// Two entities of the exported file share a `GlobalId`.
    pub const VALIDATION_DUPLICATE_ID: Reason = Reason {
        code: "ab.io.ifc.validation_duplicate_id",
        outcome: Outcome::Failed,
        severity: Severity::Error,
        message_id: "ab-io-validation-duplicate-id",
    };
    /// The exported file holds a number of products other than the export counted.
    pub const VALIDATION_COUNT: Reason = Reason {
        code: "ab.io.ifc.validation_count",
        outcome: Outcome::Failed,
        severity: Severity::Error,
        message_id: "ab-io-validation-count",
    };
}
