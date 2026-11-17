# Fabrication detailing

This area specifies fabrication-level duct, pipe, and containment parts: services and part catalogues, conversion of design-intent systems into manufacturable parts, hangers and supports, spooling, and fabrication outputs. Fabrication parts are real parts with lengths, joints, and manufacturing data, and they stay associated with the design-intent system they came from.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Fabrication service | Name, discipline (duct, pipe, containment), material, specification, part rules, default catalog | Selects catalogs and rules for parts; assigned to systems |
| Part catalog | Part families by service, size tables, joint types, manufacturing rules, stock lengths | Referenced by services and fabrication parts |
| Fabrication part | Catalog entry, size, length, connectors, joints, material, mark, status | Belongs to a service and a system; connects to other parts; carries hangers; belongs to a spool |
| Joint type | Kind (flange, weld, socket, clip, press, thread), gasket and fastener data, gap | Connects two parts; read by outputs |
| Hanger | Kind, rod and support dimensions, load rating, attachment point, host | Supports one or more parts; attached to structure; has a schedule entry |
| Support rule set | Spacing by size and material, load limits, preferred hanger kinds, clearance | Applied to services when placing hangers |
| Spool | Name, member parts, break points, mass, mark, status | Groups parts for prefabrication; has a spool sheet |
| Conversion record | Source design element, resulting fabrication parts, rules applied | Links design intent to fabrication parts |
| Fabrication output | Kind (list, part file, spool sheet, machine file), configuration | Reads parts and spools; writes files |

## Services and catalogues

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-FAB-001 | Fabrication services | Defines services by discipline, material, and specification, each linking a catalog, part selection rules, and default joints. A system can be assigned to one service. | T1 | 4 |
| AB-FAB-002 | Part catalogs | Stores catalogs of parts with size tables, connectors, joint types, stock lengths, and manufacturing rules, in an open documented table format. Catalogs can be loaded per project. | T1 | 4 |
| AB-FAB-003 | Catalog import | Imports manufacturer catalogs from documented exchange files and reports entries that cannot be mapped to a part kind. | T2 | 4 |
| AB-FAB-004 | Stock lengths and cut rules | Defines stock lengths and cutting allowances per part kind and uses them to split runs and to compute offcuts. | T2 | 4 |
| AB-FAB-005 | Fabrication settings | Stores per-document settings for part marks, labeling, display of joints, and default tolerances. | T1 | 4 |
| AB-FAB-006 | Catalog authoring | Authors parts as parametric definitions with dimensions, connectors, and joint types in the family editor and adds them to a catalog. | T2 | 4 |

## Conversion from design intent

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-FAB-007 | Convert design elements | Converts selected design-intent ducts, pipes, fittings, and containment into fabrication parts of an assigned service, choosing parts by size and rules. The command previews the parts and any elements it could not convert. | T1 | 4 |
| AB-FAB-008 | Conversion mapping | Maps design fittings and accessories to catalog parts by kind and size, with user overrides stored in a mapping table. | T1 | 4 |
| AB-FAB-009 | Conversion by system | Converts an entire system or a level of a system in one command, retaining flow and size data and tagging the resulting parts with the system name. | T1 | 4 |
| AB-FAB-010 | Source association | Keeps each fabrication part linked to the design element it came from so changes to the design can be reconciled. | T2 | 4 |
| AB-FAB-011 | Design change reconciliation | Compares design intent with fabrication parts after the design changes and lists parts needing update, with a command to regenerate them while preserving manual part edits. | T2 | 4 |
| AB-FAB-012 | Conversion report | Lists converted elements, parts created, and conversions that failed, with the reason for each failure. | T1 | 4 |

## Routing with fabrication parts

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-FAB-013 | Fabrication routing | Draws runs directly with fabrication parts from a service, inserting catalog fittings and respecting minimum straight lengths and joint spacing. | T1 | 4 |
| AB-FAB-014 | Fabrication part placement | Places individual parts such as fittings, valves, dampers, and accessories from a catalog with connectors that snap to existing parts. | T1 | 4 |
| AB-FAB-015 | Part properties | Shows and edits size, length, material, specification, and manufacturing attributes of a part, with changes propagating to connected parts through connectors. | T1 | 4 |
| AB-FAB-016 | Part alignment and rotation | Aligns parts to others, rotates them about connectors, and reports misalignment beyond a tolerance. | T2 | 4 |
| AB-FAB-017 | Fabrication insulation and lining | Adds insulation and lining to parts with type and thickness and includes them in outputs and lists. | T2 | 4 |

## Part editing

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-FAB-018 | Part join and split | Joins adjacent straight parts into one part within stock length, and splits parts at a point or at stock length, keeping connectors and marks consistent. | T1 | 4 |
| AB-FAB-019 | Length editing | Edits the length of a part from either end, with connected parts moving or adjusting, and warns when a length falls outside the catalog range. | T1 | 4 |
| AB-FAB-020 | Part swap | Replaces a part with another from the catalog of the same kind and checks connector compatibility. | T2 | 4 |
| AB-FAB-021 | Break into parts | Breaks a continuous run into stock-length parts with joints at the chosen spacing and checks the result against joint and hanger positions. | T2 | 4 |
| AB-FAB-022 | Connection repair | Reports disconnected or mismatched part connectors and offers to insert a transition, a coupling, or a corrected fitting. | T2 | 4 |

## Hangers and supports

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-FAB-023 | Hanger placement | Places hangers on parts by pick or by rule, attached to a structural host element, with rod length computed from the distance to the host. | T1 | 4 |
| AB-FAB-024 | Hanger rule sets | Defines spacing and kind of hangers by service, size, and material, and places hangers along runs by rule, avoiding joints and fittings by a clearance. | T1 | 4 |
| AB-FAB-025 | Multi-service supports | Places trapeze and rack supports carrying several parts, with member sizes determined by loaded width and load. | T2 | 4 |
| AB-FAB-026 | Support load estimate | Computes load per hanger from part mass, contents, and insulation and checks it against the hanger rating. | T2 | 4 |
| AB-FAB-027 | Host attachment | Checks that every hanger attaches to a valid structural host in range and reports unattached hangers. | T1 | 4 |
| AB-FAB-028 | Hanger edits | Moves, reorients, and resizes hangers with the supported parts following the new positions, and regenerates rod lengths. | T1 | 4 |
| AB-FAB-029 | Hanger schedules | Lists hangers by kind, rod length, and load, with counts for procurement. | T1 | 4 |

## Spooling

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-FAB-030 | Spool creation | Groups connected parts into spools by selection, by system and level, or by automatic rules on maximum length, mass, and part count. Each spool receives a mark. | T1 | 4 |
| AB-FAB-031 | Spool break points | Edits spool boundaries by moving break points to joints, and checks that every break falls at a field-joint type. | T2 | 4 |
| AB-FAB-032 | Spool sheets | Generates a sheet per spool with isometric and orthographic views, part marks, joint labels, dimensions, and a parts list, using the shared drafting engine. | T1 | 4 |
| AB-FAB-033 | Spool status | Tracks status per spool (planned, released, fabricated, delivered, installed) and shows status as colors in views and schedules. | T2 | 4 |
| AB-FAB-034 | Spool numbering | Numbers spools and parts by series and system, keeps existing marks through edits, and reports duplicates. | T1 | 4 |
| AB-FAB-035 | Spool change tracking | Marks a spool as changed when its parts change after release, and lists the changes so the sheet and output can be reissued. | T2 | 4 |

## Fabrication outputs

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-FAB-036 | Part lists | Produces bills of materials by service, system, spool, and level with part kind, size, length, count, mass, and area. | T1 | 4 |
| AB-FAB-037 | Cut lists | Produces cut lists for stock lengths with quantities of pieces and offcuts per service. | T2 | 4 |
| AB-FAB-038 | Piping component exchange | Writes piping components and isometric data to a documented neutral piping exchange format and reads it back for comparison. | T2 | 4 |
| AB-FAB-039 | Duct flat patterns | Produces flat pattern outlines of duct parts with seams and notches for sheet-metal cutting, as DXF. | T2 | 4 |
| AB-FAB-040 | Machine file output | Writes part geometry and data to documented open formats for duct and pipe fabrication equipment, using output configurations per machine. | T3 | 5 |
| AB-FAB-041 | Joint and fastener lists | Lists flanges, gaskets, fasteners, and welds required by joint type with counts per spool. | T2 | 4 |
| AB-FAB-042 | Fabrication IFC export | Exports fabrication parts as IFC distribution segments and fittings with joint data and marks, and reports unmapped data. | T1 | 4 |
| AB-FAB-043 | Delivery and installation lists | Lists spools and loose parts by delivery batch, level, and installation sequence with mass for logistics. | T2 | 4 |

## Checks and coordination

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-FAB-044 | Fabrication clash check | Detects clashes between fabrication parts, hangers, structure, and reference elements with clearance and insulation allowances, and lists them with positions. | T1 | 4 |
| AB-FAB-045 | Manufacturability check | Checks minimum straight lengths, joint access clearance, maximum spool dimensions, and unsupported lengths against the service rules. Checks run headless. | T2 | 4 |
| AB-FAB-046 | Penetration coordination | Confirms that each part passing through a wall or floor has an approved opening with sufficient clearance and lists those without. | T2 | 4 |
