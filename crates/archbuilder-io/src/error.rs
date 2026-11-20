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
    /// The export command was started without a path and the shell has no file dialog for it.
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
    /// A casement window: IFC4 has no operation attribute on the window itself.
    pub const WINDOW_OPERATION: Reason = Reason {
        code: "ab.io.ifc.window_operation_approximated",
        outcome: Outcome::Approximated,
        severity: Severity::Warning,
        message_id: "ab-io-loss-window-operation",
    };
}
