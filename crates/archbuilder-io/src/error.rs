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
    /// A mesh export of glTF asked for another unit than meters.
    pub const MESH_UNIT_UNSUPPORTED: ErrorCode = ErrorCode {
        code: "ab.io.mesh_unit_unsupported",
        category: Category::InvalidInput,
        message_id: "ab-io-mesh-unit-unsupported",
    };
    /// A mesh export has no element with a body to write.
    pub const MESH_EMPTY: ErrorCode = ErrorCode {
        code: "ab.io.mesh_empty",
        category: Category::NotFound,
        message_id: "ab-io-mesh-empty",
    };
    /// The mesh format is neither named nor given by the file extension.
    pub const MESH_FORMAT_UNKNOWN: ErrorCode = ErrorCode {
        code: "ab.io.mesh_format_unknown",
        category: Category::InvalidInput,
        message_id: "ab-io-mesh-format-unknown",
    };
    /// The file given to the import is not an IFC file the reader can read.
    pub const IMPORT_UNREADABLE: ErrorCode = ErrorCode {
        code: "ab.io.import_unreadable",
        category: Category::InvalidInput,
        message_id: "ab-io-import-unreadable",
    };
    /// The build has no schema tables for the schema of the file: the import cannot read its
    /// objects.
    pub const IMPORT_TABLES_MISSING: ErrorCode = ErrorCode {
        code: "ab.io.import_tables_missing",
        category: Category::Unsupported,
        message_id: "ab-io-import-tables-missing",
    };
    /// The mesh export of the selection was asked for with nothing selected.
    pub const MESH_SELECTION_EMPTY: ErrorCode = ErrorCode {
        code: "ab.io.mesh_selection_empty",
        category: Category::InvalidInput,
        message_id: "ab-io-mesh-selection-empty",
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
    /// A family instance whose family names no IFC entity the export writes (a generic model or
    /// specialty equipment without one, or an unknown entity): it is left out and named here with
    /// the entity, instead of becoming a proxy.
    pub const FAMILY_NOT_WRITTEN: Reason = Reason {
        code: "ab.io.ifc.family_not_written",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-family-not-written",
    };
    /// The shared nested children of a family instance: they are elements of their own, and the
    /// export does not write them yet (neither as elements nor in the parent's body).
    pub const FAMILY_SHARED_NOT_WRITTEN: Reason = Reason {
        code: "ab.io.ifc.family_shared_not_written",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-family-shared-not-written",
    };
    /// Parameters of a type or an instance IFC cannot hold (family type) are written as text.
    pub const FAMILY_PARAMETER_AS_TEXT: Reason = Reason {
        code: "ab.io.ifc.family_parameter_as_text",
        outcome: Outcome::Approximated,
        severity: Severity::Info,
        message_id: "ab-io-loss-family-parameter-as-text",
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
    /// A roof edge element that names edges its roof no longer has or that define no slope: those
    /// edges are not written.
    pub const ROOF_EDGE_ORPHAN: Reason = Reason {
        code: "ab.io.ifc.roof_edge_orphan",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-roof-edge-orphan",
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
    /// Members of a selection set that are not in the IFC file (an element that was not exported):
    /// the group is written with the members that are.
    pub const SET_MEMBER_NOT_WRITTEN: Reason = Reason {
        code: "ab.io.ifc.selection_set_member_not_written",
        outcome: Outcome::Dropped,
        severity: Severity::Info,
        message_id: "ab-io-loss-selection-set-member-not-written",
    };
    /// The exported file does not conform to the IFC schema it declares: a finding of the
    /// conformance check (levels 1 to 3 of the reader of the foundation), which runs only when the
    /// schema tables are present in the build.
    pub const VALIDATION_CONFORMANCE: Reason = Reason {
        code: "ab.io.ifc.validation_conformance",
        outcome: Outcome::Failed,
        severity: Severity::Error,
        message_id: "ab-io-validation-conformance",
    };
    /// The grid lines of a building are all in one direction: `IfcGrid` needs a U and a V family
    /// of axes, so no grid is written and each line is named.
    pub const GRID_ONE_DIRECTION: Reason = Reason {
        code: "ab.io.ifc.grid_one_direction",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-grid-one-direction",
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

    /// The parameters, types, layers and quantities of the elements: a mesh holds triangles.
    pub const MESH_PARAMETERS_NOT_CARRIED: Reason = Reason {
        code: "ab.io.mesh.parameters_not_carried",
        outcome: Outcome::Dropped,
        severity: Severity::Info,
        message_id: "ab-io-loss-mesh-parameters",
    };
    /// Hosting, attachments, joins and room relations: a mesh holds no relations.
    pub const MESH_RELATIONS_NOT_CARRIED: Reason = Reason {
        code: "ab.io.mesh.relations_not_carried",
        outcome: Outcome::Dropped,
        severity: Severity::Info,
        message_id: "ab-io-loss-mesh-relations",
    };
    /// Objects without a body: rooms, grids, reference and work planes, selection sets.
    pub const MESH_OBJECT_NOT_EXPORTED: Reason = Reason {
        code: "ab.io.mesh.object_not_exported",
        outcome: Outcome::Dropped,
        severity: Severity::Info,
        message_id: "ab-io-loss-mesh-object",
    };
    /// A family instance hosted by a wall: the mesh export does not derive its body yet (its
    /// opening is in the wall).
    pub const MESH_HOSTED_FAMILY_NOT_EXPORTED: Reason = Reason {
        code: "ab.io.mesh.hosted_family_not_exported",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-mesh-hosted-family",
    };
    /// An element whose solid could not be derived or meshed.
    pub const MESH_BODY_FAILED: Reason = Reason {
        code: "ab.io.mesh.body_failed",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-mesh-body-failed",
    };
    /// Curved edges and surfaces are written as triangles.
    pub const MESH_CURVES_FACETED: Reason = Reason {
        code: "ab.io.mesh.curves_faceted",
        outcome: Outcome::Approximated,
        severity: Severity::Info,
        message_id: "ab-io-loss-mesh-curves",
    };
    /// OBJ and STL record no unit.
    pub const MESH_UNIT_NOT_RECORDED: Reason = Reason {
        code: "ab.io.mesh.unit_not_recorded",
        outcome: Outcome::Dropped,
        severity: Severity::Info,
        message_id: "ab-io-loss-mesh-unit",
    };

    /// A product without a native counterpart, kept as a foreign element with its class, data and mesh.
    pub const IMPORT_PRESERVED: Reason = Reason {
        code: "ab.io.ifc.import_preserved",
        outcome: Outcome::Preserved,
        severity: Severity::Info,
        message_id: "ab-io-loss-import-preserved",
    };

    /// A foreign element whose geometry the file did not give or the reader did not evaluate.
    pub const IMPORT_NO_GEOMETRY: Reason = Reason {
        code: "ab.io.ifc.import_no_geometry",
        outcome: Outcome::Approximated,
        severity: Severity::Warning,
        message_id: "ab-io-loss-import-no-geometry",
    };

    /// A product whose body is not the shape its native kind has, kept as a foreign element.
    pub const IMPORT_SHAPE_NOT_RECOGNIZED: Reason = Reason {
        code: "ab.io.ifc.import_shape_not_recognized",
        outcome: Outcome::Approximated,
        severity: Severity::Warning,
        message_id: "ab-io-loss-import-shape-not-recognized",
    };

    /// A flat roof slab made a floor, because a roof of the model has sloped edges.
    pub const IMPORT_ROOF_AS_FLOOR: Reason = Reason {
        code: "ab.io.ifc.import_roof_as_floor",
        outcome: Outcome::Approximated,
        severity: Severity::Warning,
        message_id: "ab-io-loss-import-roof-as-floor",
    };

    /// The layers of an element do not add up to its body; the type has one layer of the body's thickness.
    pub const IMPORT_LAYERS_MISMATCH: Reason = Reason {
        code: "ab.io.ifc.import_layers_mismatch",
        outcome: Outcome::Approximated,
        severity: Severity::Warning,
        message_id: "ab-io-loss-import-layers-mismatch",
    };

    /// The swing side of a door or window is not derived from the file.
    pub const IMPORT_SWING_ASSUMED: Reason = Reason {
        code: "ab.io.ifc.import_swing_assumed",
        outcome: Outcome::Approximated,
        severity: Severity::Info,
        message_id: "ab-io-loss-import-swing-assumed",
    };

    /// A door or window whose wall was not mapped, kept as a foreign element.
    pub const IMPORT_INSERT_UNHOSTED: Reason = Reason {
        code: "ab.io.ifc.import_insert_unhosted",
        outcome: Outcome::Approximated,
        severity: Severity::Warning,
        message_id: "ab-io-loss-import-insert-unhosted",
    };

    /// An opening element without a filling; the wall keeps no opening.
    pub const IMPORT_OPENING_UNFILLED: Reason = Reason {
        code: "ab.io.ifc.import_opening_unfilled",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-import-opening-unfilled",
    };

    /// A product that is not an element and has no native counterpart.
    pub const IMPORT_NOT_CARRIED: Reason = Reason {
        code: "ab.io.ifc.import_not_carried",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-import-not-carried",
    };

    /// A type object that no product uses.
    pub const IMPORT_TYPE_UNUSED: Reason = Reason {
        code: "ab.io.ifc.import_type_unused",
        outcome: Outcome::Dropped,
        severity: Severity::Info,
        message_id: "ab-io-loss-import-type-unused",
    };

    /// The reference latitude and longitude of a site.
    pub const IMPORT_SITE_DATA: Reason = Reason {
        code: "ab.io.ifc.import_site_data",
        outcome: Outcome::Dropped,
        severity: Severity::Info,
        message_id: "ab-io-loss-import-site-data",
    };

    /// A part of an aggregate imported as an element of its own.
    pub const IMPORT_AGGREGATION_FLATTENED: Reason = Reason {
        code: "ab.io.ifc.import_aggregation_flattened",
        outcome: Outcome::Approximated,
        severity: Severity::Info,
        message_id: "ab-io-loss-import-aggregation-flattened",
    };

    /// A product with no storey of its own, placed in the lowest storey of its building.
    pub const IMPORT_STOREY_ASSUMED: Reason = Reason {
        code: "ab.io.ifc.import_storey_assumed",
        outcome: Outcome::Approximated,
        severity: Severity::Info,
        message_id: "ab-io-loss-import-storey-assumed",
    };
    /// A building beyond the first of the file, merged into the document's building.
    pub const IMPORT_BUILDING_MERGED: Reason = Reason {
        code: "ab.io.ifc.import_building_merged",
        outcome: Outcome::Approximated,
        severity: Severity::Warning,
        message_id: "ab-io-loss-import-building-merged",
    };
    /// The body of a door or window is the model's own frame and leaf, not the source's.
    pub const IMPORT_INSERT_REGENERATED: Reason = Reason {
        code: "ab.io.ifc.import_insert_regenerated",
        outcome: Outcome::Approximated,
        severity: Severity::Info,
        message_id: "ab-io-loss-import-insert-regenerated",
    };
    /// A grid of the file: the model's grid lines are separate objects and the export writes one
    /// grid for the building, so the `GlobalId` of the file's grid is not kept.
    pub const IMPORT_GRID_IDENTITY: Reason = Reason {
        code: "ab.io.ifc.import_grid_identity",
        outcome: Outcome::Approximated,
        severity: Severity::Info,
        message_id: "ab-io-loss-import-grid-identity",
    };
    /// A product the document already has, matched by its identifier and not imported again.
    pub const IMPORT_MATCHED: Reason = Reason {
        code: "ab.io.ifc.import_matched",
        outcome: Outcome::Preserved,
        severity: Severity::Info,
        message_id: "ab-io-loss-import-matched",
    };

    /// An imported room that no loop of walls encloses, written as a space without a body.
    pub const ROOM_BODY_MISSING: Reason = Reason {
        code: "ab.io.ifc.room_body_missing",
        outcome: Outcome::Approximated,
        severity: Severity::Warning,
        message_id: "ab-io-loss-room-body-missing",
    };
    /// A property of an imported object the writer has no value for (a list, bounded or table value).
    pub const FOREIGN_PROPERTY_NOT_WRITTEN: Reason = Reason {
        code: "ab.io.ifc.foreign_property_not_written",
        outcome: Outcome::Dropped,
        severity: Severity::Warning,
        message_id: "ab-io-loss-foreign-property-not-written",
    };

    /// A property of an imported object written with a type the writer has instead of its source type.
    pub const FOREIGN_PROPERTY_RETYPED: Reason = Reason {
        code: "ab.io.ifc.foreign_property_retyped",
        outcome: Outcome::Approximated,
        severity: Severity::Info,
        message_id: "ab-io-loss-foreign-property-retyped",
    };

    /// A foreign element whose class the output schema does not have, written as a building element proxy.
    pub const FOREIGN_CLASS_CHANGED: Reason = Reason {
        code: "ab.io.ifc.foreign_class_changed",
        outcome: Outcome::Approximated,
        severity: Severity::Warning,
        message_id: "ab-io-loss-foreign-class-changed",
    };
}

pub use arc_io_common::loss::Reason;
