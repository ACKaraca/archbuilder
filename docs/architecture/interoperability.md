# Interoperability

This document lists every exchange format by direction and fidelity, the mapping principles, how
exports are validated, how losses are reported, and which formats are deliberately unsupported.
Fidelity levels follow the suite policy: **view** (geometry and appearance), **data** (semantics,
properties, relationships) and **edit** (native, parametric editing after import).

## Formats

| Format | Direction | Fidelity | Notes | Capabilities |
|---|---|---|---|---|
| `.archb` (Arc document package) | Read, write | Edit | Native; every older version opens; newer minor read-only | AB-IO-001, AB-IO-002 |
| `.archbfam` (family package) | Read, write | Edit | Family libraries; see [families.md](families.md) | AB-FAM-031 |
| IFC 4.3 | Write | Data | Reference and design transfer exports | AB-IO-003, AB-IO-005 |
| IFC 4.3, IFC 4, IFC 2x3 | Read | Edit for supported entities, data for the rest | Conversion to native elements or reference links | AB-IO-007, AB-LINK-004 |
| IFC 4, IFC 2x3 | Write | Data | For older consumers, with reported losses | AB-IO-004 |
| IDS | Read | Data | Validation specifications | AB-IO-009 |
| BCF 3.0 (file and API) | Read, write | Data | Issues with viewpoints | AB-LINK-013 |
| COBie (spreadsheet) | Write | Data | Handover | AB-IO-010 |
| DWG, DXF | Read | Edit (drafting) | Drafting views and links | AB-IO-011, AB-LINK-005 |
| DWG, DXF | Write | View (sheets, views); view (3D) | Layer mapping tables | AB-SHEET-012, AB-IO-012 |
| PDF | Write | View | Vector sheets at true scale | AB-SHEET-011 |
| PDF, PNG, JPEG | Read | View | Underlays | AB-LINK-007 |
| PNG, JPEG, TIFF | Write | View | Image export | AB-SHEET-013 |
| glTF, OBJ, STL | Write | View | Meshes with materials and element node names (`archbuilder.file.export_mesh`) | AB-IO-013 |
| glTF, OBJ | Read | View | Context links and generic elements | AB-LINK-008 |
| OpenUSD | Write | View with element metadata | Aligned with the Arc Scene contract | AB-IO-014 |
| Arc Scene | Write (stream) | View | To RenderBuilder | AB-VIS-009 |
| STEP | Read, write | Edit for family geometry (exact solids); view for elements | Through `arc-brep` | AB-IO-015 |
| LAS, LAZ, E57 | Read | View | Point clouds | AB-LINK-006 |
| LandXML | Read | Data | Terrain surfaces | AB-SITE-001, AB-SITE-013 |
| CSV (points) | Read | Data | Terrain points | AB-SITE-001 |
| GIS vector and raster (via `arc-io-geo`) | Read | View | Underlays and context | AB-SITE-011, AB-SITE-012 |
| gbXML | Write | Data | Energy model | AB-IO-016 |
| CSV, XLSX | Read, write | Data | Schedules by stable ID | AB-SCHED-016 |
| Parameter library, keynote table, classification table | Read | Data | Open text formats | AB-PARAM-004, AB-ANNO-011, AB-PARAM-016 |
| Arc exchange package | Read, write | Data; edit where the connector declares it | Open container used by connectors in other applications | AB-IO-020, AB-CONNECT-001, AB-CONNECT-002 |
| 3DM and other formats with a permissively licensed open reader or a published specification | Read | View or data, per format | Native readers behind `arc-io-*` crates | AB-CONNECT-019 |
| Formats converted by a user-licensed library | Read, write | Per library | Separate process, never distributed with Arc | AB-CONNECT-018 |

Structural analysis exchange and steel fabrication formats are listed in the capability files [23-analytical-model.md](../product/capabilities/23-analytical-model.md) and
[22-steel-detailing.md](../product/capabilities/22-steel-detailing.md) and follow the same principles.

## Exchange with other applications

Native project and family files that have no published specification are exchanged by connectors:
add-ins that run inside the user's own licensed copy of the other application, use only its official
public API, and read or write the Arc exchange package (suite
[ADR-0017](https://github.com/ACKaraca/arc-suite/blob/main/docs/adr/0017-exchange-connectors-and-licensed-bridges.md)).
The package is specified in
[`interchange.md`](https://github.com/ACKaraca/arc-core/blob/main/docs/architecture/interchange.md)
in `arc-core`. The mapping principles below apply unchanged, with these additions.

1. **Exchange identifiers.** Each exchanged element carries an exchange identifier that does not
   depend on the other application's session, worksharing or file-copy identifiers; it is recorded
   in the identity map next to the GlobalId (AB-CONNECT-010, [ADR 0007](../adr/0007-exchange-identifiers.md)).
2. **Change detection.** A re-import compares per-entry content hashes with the last exchanged state
   and applies added, removed and modified entries after review; entries edited on both sides keep
   both states (AB-CONNECT-012, AB-CONNECT-013).
3. **Fidelity is declared.** A connector states the level (view, data, edit) per element kind and
   direction before an exchange runs (AB-CONNECT-015). Parameters and content that only one side
   understands are preserved as foreign data and written back.
4. **Mapping tables.** Categories, families, types, parameters, materials and levels are mapped by
   editable tables stored in the document (AB-CONNECT-014).
5. **Licensed bridges and open readers.** A conversion library licensed to the user runs as a
   separate process and is never required for a core workflow; formats with an open reader are read
   natively.
6. **Still excluded.** Reverse engineering undocumented formats or binaries and running the compiled
   or protected scripts of other applications are not provided, for legal reasons.

## Mapping principles

1. **The product owns the mapping.** `arc-io-*` crates produce neutral models of each format;
   `archbuilder-io` maps between them and the domain model. Mapping tables are data, editable by
   users and versioned with the document (AB-IO-006).
2. **IFC is the exchange vocabulary of the model.** Every category, system family and element kind
   has a default IFC entity and predefined type. Spatial structure maps to project, site, building
   and storey; walls, slabs, roofs, coverings, stairs, ramps, railings, doors, windows, columns,
   beams, furnishings and transport elements map to their native entities with type objects and
   type relations; layered types map to material layer sets with usage; openings map to opening
   elements with void and fill relations; rooms map to spaces; zones and groups to their IFC
   counterparts; quantities to element quantity sets computed by the same quantity rules as
   schedules.
3. **No silent proxies.** An element without a specific mapping is reported in the loss report. A
   generic proxy entity is used only when the user's mapping says so or when the element is a
   generic element by category, and every such use is listed.
4. **Stable identity.** GlobalIds derive from element identifiers and are recorded in the identity
   map; a re-export produces the same GlobalIds, and re-import updates elements in place (AB-IO-008).
5. **Preserve and report on import.** Imports keep the original file as a document asset, convert
   what has a native counterpart, keep everything else as preserved foreign data attached to the
   element or document, and produce a loss report. Converted elements record source identifiers.
6. **Geometry is honest.** Swept and extruded geometry is exported parametrically where the schema
   allows in design transfer exports; otherwise as exact boundary or tessellated geometry with the
   tessellation tolerance recorded. Curves are never silently straightened beyond the stated
   tolerance.
7. **One revision per export.** Exports run against a frozen revision on a worker.
8. **Coordinates are explicit.** Every export names its coordinate basis (internal, project or
   survey and map); IFC exports write map conversion and projected CRS from the coordinate record.

### IFC4 export as implemented

| Model kind | IFC4 | Notes |
|---|---|---|
| Stair | `IfcStair` aggregating `IfcStairFlight`, `IfcSlab` `LANDING`, `IfcRailing` | Predefined type from the shape; `Qto_StairFlightBaseQuantities` |
| Ramp | `IfcRamp` aggregating `IfcRampFlight`, `IfcSlab` `LANDING`, `IfcRailing` | `Qto_RampFlightBaseQuantities` |
| Railing | `IfcRailing` (`GUARDRAIL` on a stair or ramp, else `BALUSTRADE`) | `Qto_RailingBaseQuantities` |
| Curtain wall | `IfcCurtainWall` aggregating `IfcPlate` (`CURTAIN_PANEL`) and `IfcMember` (`MULLION`), types `IfcCurtainWallType`, `IfcPlateType`, `IfcMemberType` | `Pset_CurtainWallCommon`, `Pset_PlateCommon`, `Pset_MemberCommon`, `Qto_CurtainWallQuantities`, `Qto_PlateBaseQuantities`, `Qto_MemberBaseQuantities`; door and window panels fill openings in the wall (reported as approximated) |
| Grid | one `IfcGrid` per building with U and V `IfcGridAxis` | Labels are the axis tags |
| Reference plane, work plane, selection set | not exported | Reported (`IFCANNOTATION`, `IFCGROUP`) |

Standard property sets are written only with the value types the writer supports; a property whose
definition asks for a count, positive length, plane angle or identifier is left out.

## Validation

| Format | Validation in CI |
|---|---|
| IFC | Schema and rule validation by an independent IFC toolkit; IDS checks; re-import by GlobalId; object and relationship counts against fixture expectations |
| DWG, DXF | Opened by an independent reader; entity counts, layers, blocks, text and dimension values checked |
| PDF | Page sizes, embedded fonts, text extraction including Turkish characters, measured scale on a reference line |
| glTF | Validated by a glTF validator; bounds, units and node names checked |
| BCF | Round trip through an independent BCF library; viewpoints compared |
| COBie | Required sheets and columns present; values cross-checked with the model |
| Exchange package | Schema validation by an independent reader; export, re-import and change detection reproduce the identity map and report no change on an unchanged model |

Exports also self-validate before reporting success (AB-IO-019). Fixtures are synthetic documents
built by scripts, held in the suite's corpus repositories with owner, license and checksum; no vendor
sample files are used. A format profile is not declared complete until its independent validation
passes on the housing fixtures.

## Loss reporting

Every import and export returns a structured report (AB-IO-017):

| Field | Content |
|---|---|
| Source and target | Format, schema version, file checksums, document revision |
| Engine | Product and crate versions |
| Status | Success, partial, failed, cancelled |
| Preserved | Counts and lists of mapped items |
| Approximated | Items changed in representation (tessellated, flattened, simplified) with tolerance |
| Unsupported | Items not carried, with reason |
| Identity map | Native identifiers to target or source identifiers |
| Validation | Results of self-validation and, in CI, independent validation |

Status is *success* only when everything the format profile requires was preserved. Reports are
shown in the interface, written as JSON by the CLI, and stored with the document for imports.

## Deliberately unsupported

| Format or exchange | Reason | How it is exchanged instead |
|---|---|---|
| Native project and family files read by reverse engineering or by decoding unpublished binaries | Excluded for legal reasons | Connectors inside the other application, IFC 4.3, DWG for drawings, STEP or glTF for geometry |
| DGN versions without a published specification or open reader | No permissively licensed implementation | DWG or DXF, IFC |
| SKP | No published specification under a compatible license | OBJ, glTF, STEP, IFC, or a connector |
| NWC, NWD and similar proprietary coordination caches | Proprietary | IFC and BCF; federation in Arc Review |
| Proprietary fabrication databases | Proprietary | Open fabrication formats listed in the reserved fabrication and steel files |
