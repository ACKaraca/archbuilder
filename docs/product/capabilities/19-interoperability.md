# Interoperability

This area specifies the native document and exchange with other formats. Fidelity levels, mapping
principles, validation and deliberately unsupported formats are described in
[interoperability.md](../../architecture/interoperability.md). Publishing of sheets is specified in
[13-sheets-and-publishing.md](13-sheets-and-publishing.md) and references in
[15-links-and-coordination.md](15-links-and-coordination.md).

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Native document | Package schema version, object tables, journal, assets, preserved foreign data | One `.archb` file |
| Mapping table | Source category or type, target entity, predefined type, property set mapping | Saved and shared as files |
| Loss report | Preserved, approximated, unsupported items, identity map, validation evidence | Produced by every import and export |
| Preserved foreign data | Source format, original payload, owning element or document | Kept for round trip |
| Identity map | Native ID to IFC GlobalId and other source identifiers | Persistent per document |

## Native document

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-IO-001 | Native document | Saves and opens `.archb` documents in the Arc document package with atomic saves. A document written by any released version opens in every later version. | T1 | 2 |
| AB-IO-002 | Newer-format reading | Opens a document written by a newer minor format version read-only, showing the content it understands and listing what it cannot display; a newer major format is refused without touching the file. | T2 | 2 |

## IFC

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-IO-003 | IFC 4.3 export | Exports spatial structure, elements, types, materials and layer sets, openings with void and fill relations, property sets, quantity sets, classifications and groups to IFC 4.3, with GlobalIds that stay stable across exports. | T1 | 2 |
| AB-IO-004 | IFC 4 and IFC 2x3 export | Exports to IFC 4 and IFC 2x3 for older consumers, reporting content those schemas cannot carry. | T1 | 2 |
| AB-IO-005 | Export view choice | Exports for reference use (tessellated geometry) or for design transfer (parametric and swept geometry where the schema allows), chosen per export. | T2 | 2 |
| AB-IO-006 | IFC mapping configuration | Edits mapping tables from categories, families and types to IFC entities and predefined types, and from parameters to property sets; mappings are saved as files and reused. Elements without a mapping are reported, never exported as proxies silently. | T1 | 2 |
| AB-IO-007 | IFC import to native elements | Converts supported IFC entities into native elements and types with properties and relations. Unsupported content stays as preserved foreign data and read-only reference elements, the original file is kept, and a loss report lists every conversion decision. | T1 | 2 |
| AB-IO-008 | IFC round-trip identity | Re-importing an IFC file exported from the same document matches elements by GlobalId and updates them in place instead of duplicating them. | T2 | 3 |
| AB-IO-009 | IDS validation | Validates the model or an IFC export against IDS files and reports results per specification and element, in the application and from the CLI. | T1 | 3 |
| AB-IO-010 | COBie export | Writes facility handover data in the COBie spreadsheet format from model data, with a report of missing required fields. | T2 | 3 |

## Other formats

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-IO-011 | DWG and DXF import | Imports DWG and DXF drawings into drafting views as native drafting entities with layers, blocks, text and dimensions, keeping unsupported entities as preserved foreign data. | T1 | 2 |
| AB-IO-012 | 3D DWG and DXF export | Exports 3D views as DWG or DXF with elements as meshes or solids per element and layer mapping by category. | T2 | 3 |
| AB-IO-013 | Mesh export | Exports glTF, OBJ and STL from 3D views or selections with materials, units and per-element node names. | T1 | 2 |
| AB-IO-014 | OpenUSD export | Exports the model as an OpenUSD stage with materials aligned to the Arc Scene contract. | T2 | 4 |
| AB-IO-015 | STEP exchange | Imports STEP solids as family geometry or generic elements and exports selected elements as STEP solids. | T2 | 3 |
| AB-IO-016 | Energy model export | Exports gbXML from rooms and their space boundaries for energy analysis tools. | T2 | 4 |

## Exchange integrity

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-IO-017 | Loss reports | Produces a structured report for every import and export: what was preserved, approximated and unsupported, the identity map, the engine version and validation results. | T1 | 2 |
| AB-IO-018 | Preserved foreign data | Keeps content an importer cannot model with its owning element or the document and writes it back when exporting to the same format. | T1 | 2 |
| AB-IO-019 | Export self-validation | Validates each export against its schema rules before reporting success; an export that fails validation is reported as failed and its file is marked incomplete. | T1 | 2 |
