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
}
