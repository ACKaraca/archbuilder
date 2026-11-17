# Application connectors

This area specifies exchange with the native BIM project and family files of other applications. The
exchange runs through connectors, small add-ins that execute inside the user's own licensed copy of
the other application, use only that application's official public API, and read or write the Arc
exchange package, an open container specified in
[`interchange.md`](https://github.com/ACKaraca/arc-core/blob/main/docs/architecture/interchange.md)
in `arc-core`. The decision is
[suite ADR-0017](https://github.com/ACKaraca/arc-suite/blob/main/docs/adr/0017-exchange-connectors-and-licensed-bridges.md).
Connectors live in a separate repository and are not part of the ArchBuilder desktop application.
Reverse engineering undocumented formats or binaries stays excluded. Mapping principles, fidelity
levels and loss reporting are in [interoperability.md](../../architecture/interoperability.md).

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Exchange package | Schema version, entries (elements, types, families, datums, views, sheets, materials, phases), exchange identifiers, content hashes, source application and version | Written by a connector or by ArchBuilder; read by the other side |
| Exchange identifier | Stable identifier assigned by the exchange, independent of the host's session, worksharing and element identifiers | Recorded on the element in this document and on the element in the other application |
| Exchange link | Package location, mapping table, last exchanged revision, per-entry hashes | Persistent per document; drives change detection |
| Change set of an exchange | Added, removed, modified, moved and conflicting entries | Derived by comparing a package with the last exchanged state |
| Fidelity declaration | Element kind, direction, level (view, data, edit) | Published by a connector; shown before an exchange |
| Licensed bridge | User-installed conversion library, separate process, version | Called through a byte-level interface; never distributed with Arc |
| Open reader format | Format, reader crate, supported entities, fidelity | Read natively behind an `arc-io-*` crate |

## Connector exchange

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-CONNECT-001 | Exchange package import | Reads an Arc exchange package produced by a connector and converts its entries into native elements, types, families, datums, views, sheets, materials and phases through the mapping table. The package is kept as a document asset and a loss report lists every conversion decision. | T3 | 5 |
| AB-CONNECT-002 | Exchange package export | Writes selected elements, types, families, datums, views, sheets, materials and phases to an exchange package for a connector to apply in the other application, with a loss report for content the package schema or the declared fidelity cannot carry. | T3 | 5 |
| AB-CONNECT-003 | Elements and types | Exchanges elements with category, type, instance parameters, host relationships, level association, geometry and material assignments in both directions. Geometry the other side cannot represent parametrically travels as exact solids or meshes with a recorded tolerance. | T3 | 5 |
| AB-CONNECT-004 | Families with parameters | Exchanges loadable and system families with their types and typed parameters. Where the host exposes a declarative definition, the family is rebuilt as a native family; otherwise the types arrive as generic types with solid geometry, flagged as non-parametric in the loss report. | T3 | 5 |
| AB-CONNECT-005 | Parameters and units | Carries parameter definitions with names, data types, units, grouping and values, converts units at the exchange boundary, and maps shared definitions to library parameters by their stable identifier. | T3 | 5 |
| AB-CONNECT-006 | Levels and grids | Exchanges levels, grids and reference planes with elevations, extents and names, and maps levels to storeys. A mismatch between the two sides' datums is listed before any element moves. | T3 | 5 |
| AB-CONNECT-007 | Views and sheets | Exchanges view definitions (plan, section, elevation, 3D, schedule), sheets, viewport placement and titles. Views arrive as native model views with their ranges and graphic settings where mappable, and as view-only content otherwise. | T3 | 5 |
| AB-CONNECT-008 | Materials | Exchanges materials with names, appearance, physical and thermal properties and classification, and links them to the layers and element types that use them. | T3 | 5 |
| AB-CONNECT-009 | Phases | Exchanges phases, phase assignment of elements and phase filters so that existing and demolished states survive in both directions. | T3 | 5 |
| AB-CONNECT-010 | Worksharing-independent identifiers | Assigns each exchanged element an exchange identifier that does not depend on the host's session, worksharing or file-copy identifiers, stores it on both sides, and matches elements by it on every later exchange. Copying a file on either side does not break the match. | T3 | 5 |
| AB-CONNECT-011 | Repeated round trips | Exports to the other application and re-imports the result repeatedly without duplicating elements, without changing identifiers and without losing parameters that only one side understands, which are preserved as foreign data and written back. | T3 | 5 |
| AB-CONNECT-012 | Change detection on re-import | Compares a new package with the last exchanged state by content hash and lists added, removed, modified and moved entries with the changed parameters and geometry, shows the list for review, and applies the selection in one transaction. | T3 | 5 |
| AB-CONNECT-013 | Two-sided edits | Detects entries edited on both sides since the last exchange, keeps both states, and presents the conflict for a decision per entry. Nothing is overwritten silently. | T3 | 5 |
| AB-CONNECT-014 | Exchange mapping tables | Edits and stores tables that map categories, families, types, parameters, materials and levels between the other application and the model, with validation of unmapped entries. Tables are saved as files, kept in the document and reused. Complements AB-IO-006. | T3 | 5 |
| AB-CONNECT-015 | Fidelity declarations | Shows, per element kind and direction, whether a connector exchanges at view, data or edit fidelity, before the exchange runs, and records the level used in the loss report. | T3 | 5 |
| AB-CONNECT-016 | Connector loss report | Extends the loss report of AB-IO-017 with items the connector could not read or write, host-side warnings, the connector and host versions, and the identity map between exchange identifiers and both sides' identifiers. | T3 | 5 |
| AB-CONNECT-017 | Package validation | Validates an exchange package against its schema, bounds recursion, entry counts and allocation, and rejects a package with a newer major schema without touching the document. | T3 | 5 |
| AB-CONNECT-018 | Licensed bridge | Calls a conversion library that the user has installed and licensed, as a separate process through a byte-level interface, for formats that a vendor or consortium licenses. The library is never distributed with ArchBuilder, and no core workflow requires it. | T3 | 5 |
| AB-CONNECT-019 | Open readers | Reads 3DM files and other formats that have a permissively licensed open reader or a published specification natively, behind an `arc-io-*` crate, into generic elements, meshes and exact solids at view or data fidelity, with a loss report and the original file kept. | T3 | 5 |
| AB-CONNECT-020 | Headless exchange | Imports, exports, compares and reports on exchange packages from the command line and the automation protocol, with a machine-readable loss report and a change list, so exchanges can run in continuous integration or Hub jobs. | T3 | 5 |
