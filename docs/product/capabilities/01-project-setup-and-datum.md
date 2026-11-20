# Project setup and datum

This area specifies how a project starts and how every element is positioned: templates, project
information, the spatial structure, display units, levels, grids, reference planes, work planes, the
coordinate framework, the general modify and selection toolset, precision input, and model health
tools. Every other area places its elements against the datums and coordinates defined here.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Project | Name, number, client, address, status, issue date, user fields, template origin | Root of the spatial structure; owns settings, phases, coordinate record |
| Site, building, storey | Name, long name, elevation, storey flag | Site contains buildings; building contains storeys; every element is contained in one storey or the site |
| Level | Name, elevation, elevation base (project or survey), storey flag, head type | Owns plan views when requested; elements bind base and top constraints to it |
| Grid | Name, curve (line, arc, multi-segment), head visibility per end, extents | Elements and constraints bind to it; views clip it by extents or extent volumes |
| Reference plane | Name, plane, bounded or infinite, reference strength | Defines work planes; constraint and dimension target |
| Work plane | Origin, normal, source (level, grid, reference plane, face, three points) | Stored on every sketch and plane-based element |
| Coordinate record | Internal origin, project base point, survey point, true north, CRS, map conversion | One per document; read by views, annotations, links and exports |
| Datum extent volume | Name, box | Limits where assigned datums display |
| Selection set | Name, static member list or rule | Evaluated on use |
| Warning | Rule, severity, affected element IDs, first revision seen | Derived; cleared when the condition no longer holds |

## Project and templates

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-DATUM-001 | Project from template | Creates a document from a template document that carries levels, view templates, sheet formats, schedules, families, parameter definitions, phases, materials and graphic standards. A template is an ordinary `.archb` document marked as a template; creating a project copies its content under a new document identity and leaves the template unchanged. | T1 | 2 |
| AB-DATUM-002 | Project information | Stores named project fields (name, number, client, address, status, issue date) plus user-defined fields. Title blocks, schedules and the IFC project entity read these values live; editing a field updates every sheet that shows it. | T1 | 2 |
| AB-DATUM-003 | Spatial structure | Maintains the hierarchy project, site, building, storey, with any number of buildings and an independent storey set per building. Every physical element is contained in exactly one storey or the site; moving an element between storeys keeps its world position unless the user asks to keep its relative offset. | T1 | 2 |
| AB-DATUM-004 | Display units | Sets, per quantity kind (length, area, volume, angle, slope, and the discipline kinds used by structure and services), the display unit, precision, rounding and symbol. Stored values remain SI; changing display units never changes a stored value, only how it is shown and typed. | T1 | 2 |
| AB-DATUM-005 | Standards transfer | Copies selected types, styles, view templates, parameter definitions, materials and settings from another open document. Name conflicts are resolved per item by keep, overwrite or rename, and the command returns a report of what was copied and what was skipped. | T2 | 2 |
| AB-DATUM-006 | Purge unused content | Lists unused families, types, materials, styles, filters and view templates with the reason each is considered unused, and removes the selected items in one undoable command. Items referenced only by other unused items are removed together; items referenced by anything in use are never offered. | T2 | 2 |

## Levels, grids and reference planes

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-DATUM-007 | Levels | Creates named horizontal datums in an elevation or section view, by elevation value, or by copying and arraying with automatic name sequences. A level flagged as a storey creates the storey record and optionally its plan views; changing a level elevation regenerates every element bound to it in one transaction. | T1 | 2 |
| AB-DATUM-008 | Level-bound placement | Stores, for level-based elements, a base level with offset and either a top level with offset or an unconnected height. Switching an element from a top level to an unconnected height keeps its current height and breaks the dependency on the top level. | T1 | 2 |
| AB-DATUM-009 | Grids | Creates straight, arc and multi-segment grid lines, drawn or picked from existing curves. Labels increment automatically in numeric or alphabetic sequences, including sub-grid labels such as `A.1`, and duplicate labels within a building are reported. | T1 | 2 |
| AB-DATUM-010 | Datum display extents | Gives each grid and level a model extent and, per view, an optional view-specific extent and head visibility at each end. An extent adjustment can be propagated to selected parallel views in one command. | T2 | 2 |
| AB-DATUM-037 | Level removal | Lists, before a level is deleted, every element, view, datum and constraint bound to it, and lets the user delete them with the level or rebind them to another level of the same building while keeping their world elevations. Either choice is one transaction; nothing remains bound to a missing level. | T1 | 2 |
| AB-DATUM-011 | Reference planes | Creates named bounded or unbounded planes with a reference strength (strong, weak, none) that controls snapping and dimension priority. Reference planes act as work planes and constraint targets in projects and families. | T1 | 2 |
| AB-DATUM-012 | Work planes | Sets the active plane for placement and sketching from a level, grid, named reference plane, element face or three picked points, and displays it on request. Every sketch and plane-based element stores its work plane, and moving that datum moves the element. | T1 | 2 |
| AB-DATUM-013 | Datum extent volumes | Defines named boxes that limit where assigned grids, levels and reference planes appear. A view assigned to a volume shows only the datums that intersect it, with extents clipped to the box. | T2 | 3 |

## Coordinates and georeference

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-DATUM-014 | Origins and base points | Keeps three points per document: a fixed internal origin, a movable project base point that defines project coordinates, and a survey point that carries the geodetic position. Moving the project base point changes reported coordinates, never element positions. | T1 | 2 |
| AB-DATUM-015 | Geodetic reference | Stores a coordinate reference system (EPSG code or WKT), map coordinates and elevation of the survey point, true north angle and scale factor. Exports write this record to IFC map conversion and projected CRS entities and to the georeference fields of other formats that carry one. | T1 | 2 |
| AB-DATUM-016 | True north and project north | Lets each plan view display in project north or true north. Rotating true north changes orientation-dependent results (sun, exports, true-north views) and never moves elements. | T1 | 2 |
| AB-DATUM-017 | Shared positioning | Acquires the position of a linked document or publishes this document's position to a link, and keeps named positions so one building can be placed several times on a site. The relationship is stored on both sides and reported when the two records disagree. | T2 | 3 |
| AB-DATUM-018 | Coordinate query | Reports the internal, project, survey and map coordinates and the elevation of a picked point in one readout, in display units. The same query is available as a command for scripts. | T2 | 2 |
| AB-DATUM-019 | Distant-geometry protection | Warns when geometry is created or imported farther from the internal origin than the document's precision distance and offers to place it relative to the survey point instead. Model coordinates are double precision and viewports render relative to the camera, so georeferenced projects keep sub-millimeter display precision. | T1 | 2 |

## Modify and precision input

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-DATUM-020 | Modify toolset | Provides move, copy, rotate, mirror (pick or draw axis), scale where the element permits, align, offset, trim and extend (corner, single, multiple), split with optional gap, and delete. Each invocation is one undoable command; hosted and attached elements follow their hosts. | T1 | 2 |
| AB-DATUM-021 | Linear and radial arrays | Creates linear and radial arrays by count and spacing or by count and total extent. An associated array is stored as a group with editable count and spacing; an unassociated array produces independent copies. | T1 | 2 |
| AB-DATUM-022 | Pin | Marks elements as pinned so that modify commands reject them with a message naming the pinned element. Datums and links can be pinned on creation by a document setting. | T1 | 2 |
| AB-DATUM-023 | Snapping | Snaps to endpoint, midpoint, center, intersection, perpendicular, tangent, nearest, quadrant, work-plane grid increment, and extension and alignment lines. A key cycles overlapping candidates; snap distance is measured in screen pixels and is independent of geometric tolerance. | T1 | 2 |
| AB-DATUM-024 | Typed values during placement | Accepts a typed length, angle or expression with units while drawing or dragging and applies it to the current segment or displacement. Expressions are evaluated by the same unit-aware engine as parameters. | T1 | 2 |
| AB-DATUM-025 | Live dimensions | Shows transient dimensions from a selected element to the nearest references; editing a value moves or resizes the element in one command. Reference preference (centerline, faces, core faces) is a user setting, and a live dimension can be made permanent. | T1 | 2 |
| AB-DATUM-035 | Numeric entry | Accepts numbers in every numeric field (properties, schedules, view and sheet settings, dialogs and placement input) through one parser that takes the interface locale's decimal and grouping separators, unit symbols and expressions. An empty, invalid or out-of-range entry is rejected with a message and the previous value is restored; it is never stored as zero. | T1 | 2 |
| AB-DATUM-026 | Selection | Selects by click, enclosing window, crossing window, add and remove modifiers, candidate cycling through overlapping, nested and linked elements, all instances of a type in the view or project, previous selection, category filter and element ID. Selection is transient state and is not recorded in history. | T1 | 2 |
| AB-DATUM-027 | Selection sets | Saves named selection sets in the document, either as a static member list or as a rule (category and parameter conditions) evaluated each time the set is used. Deleted members are dropped from static sets and reported. | T2 | 2 |
| AB-DATUM-028 | Aligned paste | Pastes copied elements at the same position, onto selected levels, or into selected views. Pasted elements rebind to the target levels and hosts and report any element that found no valid host. | T1 | 2 |
| AB-DATUM-029 | Create similar and match type | Starts placement with the type of a selected element, or applies the type of a source element to selected targets of the same category. | T1 | 2 |
| AB-DATUM-030 | Undo and redo | Keeps a per-document history of named transactions, each labeled with its source (user, script, plug-in, node graph, assistant). Undo and redo apply whole transactions; an interactive drag is one transaction on release. | T1 | 2 |

## Model health

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-DATUM-031 | Warnings register | Records every rule violation (overlapping walls, unenclosed room, duplicate mark, unsatisfied constraint, failed join, orphaned annotation, invalid host) as a warning with severity, affected element IDs and the revision in which it first appeared. Warnings can be grouped, navigated, filtered and exported, and clear automatically when the condition no longer holds. | T1 | 2 |
| AB-DATUM-032 | Model statistics | Reports element counts by category, family and type, stored size by table, the largest families, view counts, and measured regeneration time per element kind. The same report is available from the command line. | T2 | 2 |
| AB-DATUM-033 | Integrity audit | Checks references, schemas and journal consistency when a document is opened with audit or on demand. Repairs run only after confirmation, on a copy of the original, and produce a report of every change. | T1 | 2 |
| AB-DATUM-034 | Crash recovery | Offers the recovered state after an abnormal exit, built from the recovery journal, alongside the last explicit save. The user chooses which to open; neither is discarded until the user decides. | T1 | 2 |
| AB-DATUM-036 | Save integrity | Reports a document as saved only after the atomic write has completed and been read back; until then the unsaved state stays visible. A failed save (full disk, lost drive, denied permission) keeps the document open and editable, leaves the last saved file unchanged and offers saving to another location. A document already open for writing in another process opens read-only, naming that process, with an option to open a copy. | T1 | 2 |
