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
    /// A stair: the IFC writer of the foundation has no stair, stair flight or railing entity
    /// yet, so the stair is left out (the shaft opening it cuts in the floors is written).
    pub const STAIR_NOT_WRITTEN: Reason = Reason {
        code: "ab.io.ifc.stair_not_written",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-stair-not-written",
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
    /// A railing: the writer of the pinned foundation has no railing entity yet.
    pub const RAILING_NOT_WRITTEN: Reason = Reason {
        code: "ab.io.ifc.railing_not_written",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-railing-not-written",
    };
    /// A ramp: the writer of the pinned foundation has no ramp or ramp flight entity yet, so the
    /// ramp is left out (the opening it cuts in the floor above is written).
    pub const RAMP_NOT_WRITTEN: Reason = Reason {
        code: "ab.io.ifc.ramp_not_written",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-ramp-not-written",
    };
    /// A grid: the IFC writer of the foundation has no grid or grid axis entity yet, so the grid
    /// is left out rather than written as a proxy.
    pub const GRID_NOT_WRITTEN: Reason = Reason {
        code: "ab.io.ifc.grid_not_written",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-grid-not-written",
    };
    /// A reference plane or work plane: drawing aids with no IFC entity, left out.
    pub const DATUM_NOT_WRITTEN: Reason = Reason {
        code: "ab.io.ifc.datum_not_written",
        outcome: Outcome::Dropped,
        severity: Severity::Info,
        message_id: "ab-io-loss-datum-not-written",
    };
    /// A curtain wall: the IFC writer of the foundation has no curtain wall, plate or member
    /// entity yet, so the wall, its panels and its mullions are left out. The count is the
    /// number of entities of that kind the wall would write.
    pub const CURTAIN_NOT_WRITTEN: Reason = Reason {
        code: "ab.io.ifc.curtain_not_written",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-curtain-not-written",
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
