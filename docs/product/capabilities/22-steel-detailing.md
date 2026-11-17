# Steel detailing

This area specifies fabrication-level steel modeling on top of the structural model: plates, bolts, anchors, welds, cuts and copes, a parametric connection library with user-authored connections, stair, railing, and ladder generators, cladding and miscellaneous steel, model checking, numbering, general arrangement and shop drawing generation, material lists, and NC and DXF fabrication output. Detailed parts are derived from and stay associated with the physical framing, so a change to a member regenerates its connections, marks, and drawings.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Steel part | Kind (member, plate, grating, special part), profile or outline, thickness, grade, coating, length, mass, part mark | Belongs to an assembly; derives from a framing member or stands alone; carries features |
| Plate | Outline, thickness, grade, bend lines, corner treatments, holes | Part of a connection or a standalone part; carries welds and bolt holes |
| Fastener group | Kind (bolt, anchor, stud), standard, grade, diameter, pattern, hole type, shop or site flag | Joins parts; belongs to a connection; generates holes in joined parts |
| Weld | Type, size, length, side, shop or site flag, process, symbol | Joins two parts; belongs to a connection or stands alone |
| Part feature | Kind (cut, cope, notch, shortening, corner finish, hole), geometry | Owned by one part; may be created by a connection |
| Connection | Library definition, parameters, primary and secondary parts, state (valid, failed, overridden) | Owns generated plates, fasteners, welds, and features; regenerates with parents |
| Connection definition | Name, category, parameter schema, applicability rules, generation logic (node graph or built-in) | Referenced by connections; stored in a library |
| Macro object | Kind (stair, railing, ladder, cage, platform, cladding system, purlin system), path or points, parameters | Owns generated parts and assemblies; regenerates from its parameters |
| Assembly | Main part, member parts, marks, sub-assemblies, status | Owns parts; target of assembly numbering and shop drawings |
| Numbering scheme | Series prefixes, start values, identity tolerance, lock flags | Writes part and assembly marks |
| Drawing style | View rules, dimension rules, label rules, part visibility, sheet format | Referenced by drawing definitions; stored in a style library |
| Drawing definition | Kind (general arrangement, single part, assembly), source selection, style, status | Produces a derived drawing; tracks the model revision it was built from |
| Fabrication output | Kind (NC, DXF, list), configuration, target parts | Reads numbered parts; writes files |

## Settings and catalogs

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-STEEL-001 | Detailing settings | Stores project-level steel settings: detailing standard, profile and fastener catalogs, tolerance values, numbering scheme, output configurations, and drawing styles. Settings can be saved as templates and applied to new documents. | T1 | 4 |
| AB-STEEL-002 | Fastener catalog | Provides catalogs of bolts, nuts, washers, anchors, and studs by standard, grade, and diameter, with hole sizes, edge and pitch minimums, and mass. Catalogs are open tables extended by users. | T1 | 4 |
| AB-STEEL-003 | Weld catalog and symbols | Defines weld types, sizes, processes, and symbols per standard, and draws weld symbols in views and drawings. | T1 | 4 |
| AB-STEEL-004 | Steel grade and coating attributes | Records grade and coating on parts, with surface area for coating quantities. The attributes carry into lists, drawings, and output files. | T1 | 4 |
| AB-STEEL-005 | Detail level conversion | Converts a physical framing member to a detailed steel part with the same profile, position, and marks, while keeping the association so that edits to the framing member update the part. | T1 | 4 |

## Parts, plates, and members

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-STEEL-006 | Detailed steel members | Places straight, curved, polyline, tapered, haunched, and folded members with a profile from the catalog, justification, and end treatments. They are also reachable from framing members. | T1 | 4 |
| AB-STEEL-007 | Plates | Creates rectangular or shaped plates from points or an outline with thickness and grade, with corner chamfers and radii and edge preparation. | T1 | 4 |
| AB-STEEL-008 | Folded plates | Creates plates with bend lines, bend radius, and angle, and produces a flat pattern with bend marks for output. | T2 | 4 |
| AB-STEEL-009 | Grating and decking | Places grating and floor plate panels with bar spacing, loading direction, and cut-outs, and counts panels and cut sizes. | T2 | 4 |
| AB-STEEL-010 | Special parts | Imports user-defined parts from solid geometry as named, parametric special parts with mass, grade, and a symbol for drawings. | T2 | 4 |
| AB-STEEL-011 | Stiffeners | Creates web and flange stiffeners in chosen bays of a member with chamfers, clearances, and weld preparation. They update with the member section. | T1 | 4 |
| AB-STEEL-012 | Cambered and pre-set members | Models camber on a member as geometry for fabrication, and carries the camber to drawings and NC output. | T3 | 5 |
| AB-STEEL-013 | Concrete references | Brings concrete elements into the detailing context as references for anchors, embeds, and base plates, without making them detailing parts. | T1 | 4 |
| AB-STEEL-014 | Array, mirror, and copy | Copies, arrays, and mirrors parts and connections while preserving parametric definitions and regenerating features for the new positions. | T1 | 4 |

## Fasteners and welds

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-STEEL-015 | Bolt groups | Places bolts by rectangular, circular, or custom pattern with standard, grade, and diameter. The command creates matching holes in all joined parts and applies edge and pitch checks. | T1 | 4 |
| AB-STEEL-016 | Anchor groups | Places anchors on a base plate or embed with embedment length, projection, and washers, and checks edge distances against the concrete reference. | T1 | 4 |
| AB-STEEL-017 | Shear studs | Places shear connectors on a member flange or plate by layout, with height and diameter, and counts them for composite floors. | T2 | 4 |
| AB-STEEL-018 | Holes and slots | Creates round, oversized, and slotted holes by position and pattern in plates and members, with a hole tolerance setting that carries into output. | T1 | 4 |
| AB-STEEL-019 | Welds | Creates fillet, butt, and plug welds between parts with size, length, side, and process. Welds carry a shop or site flag and show symbols in views and drawings. | T1 | 4 |
| AB-STEEL-020 | Weld lists | Totals weld length and volume by type and size and lists welds by mark, part, and shop or site location. | T2 | 4 |
| AB-STEEL-021 | Fastener lists | Counts fasteners by standard, grade, diameter, and length, including washers and nuts, and groups them by assembly or zone. | T1 | 4 |

## Cuts, copes, and end treatments

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-STEEL-022 | Cuts and openings in parts | Cuts rectangular, circular, or profile-shaped openings in members and plates by position and size, and records them as part features. | T1 | 4 |
| AB-STEEL-023 | Copes and notches | Creates flange and web copes and notches at member ends with depth, length, and radius, and updates them when the supporting member changes. | T1 | 4 |
| AB-STEEL-024 | Shortening and mitre | Shortens members by a clearance, a distance, or a mitre angle against another part, with the result tracked as a feature. | T1 | 4 |
| AB-STEEL-025 | Corner finishes | Finishes plate corners with chamfer, round, or notch, individually or for all corners at once. | T1 | 4 |
| AB-STEEL-026 | Trim and extend by intersection | Trims or extends a member or plate to another part or plane and keeps the relationship so that the length follows the target. | T1 | 4 |

## Parametric connection library

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-STEEL-027 | Connection library browser | Lists connection definitions by category (base plate, beam to column, beam to beam, shear plate, end plate, moment, brace gusset, truss node, splice, purlin, support bracket) with search and a preview of applicability. | T1 | 4 |
| AB-STEEL-028 | Connection placement | Applies a connection to a primary part and one or more secondary parts after checking applicability, and shows a parameter form with a live preview. The command creates plates, fasteners, welds, and features as one connection object. | T1 | 4 |
| AB-STEEL-029 | Connection regeneration | Regenerates a connection when a parent part changes profile, length, angle, or position. A connection that cannot resolve is marked failed with a reason and keeps its last valid parts hidden until fixed. | T1 | 4 |
| AB-STEEL-030 | Shear and simple connections | Provides fin plate, clip angle, double angle, and shear end plate connections for beam-to-column and beam-to-beam joints with parameters for plate size, bolt rows, edge distances, and welds. | T1 | 4 |
| AB-STEEL-031 | Moment connections | Provides extended end plate, haunched, and flange-plated moment connections with stiffener options, bolt layouts, and weld preparation. | T1 | 4 |
| AB-STEEL-032 | Base plate connections | Provides base plates with anchor layouts, stiffeners, grout thickness, and shear keys for columns on foundations or concrete references. | T1 | 4 |
| AB-STEEL-033 | Brace and gusset connections | Provides gusset plate connections for diagonals to columns and beams with bolted and welded variants and gusset geometry derived from member work points. | T1 | 4 |
| AB-STEEL-034 | Truss node connections | Provides connections for hollow-section and angle truss nodes with gap and overlap rules, gusset options, and cut geometry. | T2 | 4 |
| AB-STEEL-035 | Splice connections | Provides beam and column splices with cover plates or end plates, with bolt groups on each side of the splice. | T2 | 4 |
| AB-STEEL-036 | Purlin and rail connections | Provides cleats and sleeve connections between purlins or rails and primary members, with spacing taken from the purlin system. | T2 | 4 |
| AB-STEEL-037 | Support bracket connections | Provides brackets and seats for stair stringers, railing posts, and equipment supports. | T2 | 4 |
| AB-STEEL-038 | Standard variants | Provides each connection in variants for regional standards and fastener types, selected by the document's detailing standard. | T2 | 4 |
| AB-STEEL-039 | Batch connection application | Applies one connection definition and parameter set to all matching joints in a selection, and lists joints where it could not apply. | T1 | 4 |
| AB-STEEL-040 | Connection templates | Saves a parameter set of a connection as a named template and applies it to other joints, with rules that choose a template by joint conditions. | T2 | 4 |
| AB-STEEL-041 | Connection override and conversion | Converts a connection to editable independent parts, or marks it as manually overridden so regeneration no longer changes it. The override is visible in lists and checks. | T2 | 4 |
| AB-STEEL-042 | Connection checks | Verifies bolt edge, end, and pitch distances, weld sizes, clearances, and plate sizes against the standard, and flags the connection. | T2 | 4 |
| AB-STEEL-043 | Connection capacity checks | Computes joint capacities for supported joint types from published design formulas and reports the utilization against analytical forces. The formulas are open and documented per connection definition. | T3 | 5 |

## Custom connections

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-STEEL-044 | Custom connection authoring | Authors a connection definition as a node graph with typed inputs (parts, parameters), geometry and fastener nodes, and outputs. The definition is saved to a library and appears in the connection browser. | T2 | 4 |
| AB-STEEL-045 | Connection capture | Converts a manually modeled arrangement of plates, bolts, and welds around a joint into a parametric definition, by choosing which dimensions become parameters. | T3 | 5 |
| AB-STEEL-046 | Definition validation | Tests a connection definition against sample joints with different angles and profiles, and reports parameter ranges where generation fails. | T3 | 5 |
| AB-STEEL-047 | Definition sharing | Packages a connection definition with its parameter schema and test cases as a signed package that other users can install, with the host assigning trust. | T3 | 5 |

## Stair, railing, and ladder generators

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-STEEL-048 | Steel stairs | Generates straight, spiral, and winder stairs from a path or start and end points with width, rise, tread, stringer style, and landing. The stair regenerates when path or height changes. | T1 | 4 |
| AB-STEEL-049 | Stair parts | Produces stringers, treads, nosings, landings, and brackets as individual parts and assemblies with marks, ready for lists and drawings. | T1 | 4 |
| AB-STEEL-050 | Railings | Generates railings along a path with posts, rails, and infill by spacing rules, with corner, end, and return treatments. | T1 | 4 |
| AB-STEEL-051 | Ladders and cages | Generates ladders with rungs, stiles, and wall brackets, and safety cages with hoop spacing, to the selected standard. | T2 | 4 |
| AB-STEEL-052 | Platforms and walkways | Generates platforms and walkways with grating or plate, kick plates, and support beams from an outline and elevation. | T2 | 4 |

## Cladding and miscellaneous steel

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-STEEL-053 | Purlin and rail systems | Generates purlins and wall rails between primary frames with spacing, lap or sleeve rules, and sag rods, and updates when the frame changes. | T2 | 4 |
| AB-STEEL-054 | Portal frame generator | Generates portal frames from span, eaves height, pitch, and bay spacing with columns, rafters, haunches, and bracing bays. | T2 | 4 |
| AB-STEEL-055 | Miscellaneous steel generators | Generates lintels, embeds, equipment supports, and base brackets from parameter sets, with parts, marks, and lists. | T2 | 4 |
| AB-STEEL-056 | Special structures | Generates lattice towers and masts from height, taper, and member rules. | T3 | 5 |

## Model checking

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-STEEL-057 | Detailing model check | Checks for duplicate parts, overlapping parts, zero-length parts, unrealistic geometry, and parts without grade or profile. Results list the object and a reason, and selecting a row zooms to it. | T1 | 4 |
| AB-STEEL-058 | Clash check on steel | Detects intersections between steel parts and against reference elements with a tolerance and reports pairs. Clashes inside a connection are ignored by rule. | T1 | 4 |
| AB-STEEL-059 | Property-based selection | Selects parts by any attribute with logical filters and saves selections as named sets used by numbering, drawings, and output. | T1 | 4 |
| AB-STEEL-060 | Weight and center of gravity | Reports mass and center of gravity for parts, assemblies, and any selection. | T1 | 4 |
| AB-STEEL-061 | Headless model check | Runs the detailing checks from the command line and in continuous integration, with a machine-readable report. | T2 | 4 |

## Numbering

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-STEEL-062 | Part and assembly numbering | Assigns part marks and assembly marks so that identical parts and assemblies share one mark. Identity is decided by geometry, holes, welds, features, and attributes within configured tolerances. | T1 | 4 |
| AB-STEEL-063 | Numbering series | Defines series by prefix, start value, and part category (beams, columns, plates, bracing), and by zone or phase. | T1 | 4 |
| AB-STEEL-064 | Number retention | Keeps existing marks when parts are edited unless they are no longer identical, and reports marks that changed. | T1 | 4 |
| AB-STEEL-065 | Locked marks | Locks marks against renumbering. A locked mark that conflicts with a non-identical part is reported. | T2 | 4 |
| AB-STEEL-066 | Numbering status | Lists unnumbered, modified, and duplicate-marked parts, and shows marks in the model for review. | T1 | 4 |
| AB-STEEL-067 | Selection by mark | Finds all parts or assemblies with a mark and selects them. | T1 | 4 |

## Drawings

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-STEEL-068 | General arrangement drawings | Generates plan, elevation, and section drawings of selected zones with grids, levels, member marks, and dimensions from a drawing style. | T1 | 4 |
| AB-STEEL-069 | Single-part drawings | Generates fabrication drawings of a single part with views, holes, welds, cuts, and dimensions, one drawing per unique mark. | T1 | 4 |
| AB-STEEL-070 | Assembly drawings | Generates shop drawings of assemblies with main and detail views, sections, part marks, weld symbols, and a parts list. | T1 | 4 |
| AB-STEEL-071 | Drawing styles | Defines styles that control which views are created, orientation, scale, dimensioning rules, label content, and part visibility. Styles inherit from a parent style and are stored as open definitions. | T1 | 4 |
| AB-STEEL-072 | Style selection rules | Chooses a drawing style for each assembly by ordered rules on type, profile, and attributes. The first matching rule wins. | T2 | 4 |
| AB-STEEL-073 | Automatic dimensioning | Dimensions part lengths, hole positions, weld locations, and overall sizes according to the style, avoiding overlaps. | T1 | 4 |
| AB-STEEL-074 | Label and text templates | Defines labels, mark bubbles, and text blocks with placeholders for attributes, which resolve from the model when the drawing is built or updated. | T1 | 4 |
| AB-STEEL-075 | Sheet formats and title blocks | Places drawings on sheets with title blocks, scales, and revision fields, using the same sheet and drafting engine as the rest of the document. | T1 | 4 |
| AB-STEEL-076 | Drawing process | Defines an ordered process that creates views, applies a style, dimensions, merges views, and places sheets. A process can run on a selection or on all assemblies. | T2 | 4 |
| AB-STEEL-077 | Drawing update with manual edits kept | Rebuilds a drawing from the current model and keeps manual dimensions, text, and symbols attached to model features. Manual items whose target no longer exists are listed for review instead of silently removed. | T1 | 4 |
| AB-STEEL-078 | Drawing register | Lists all drawings with source assemblies, status, model revision built from, and state (current, outdated, source deleted). The register creates, updates, and deletes drawings in batches. | T1 | 4 |
| AB-STEEL-079 | Batch drawing creation | Creates all drawings for a selection or a zone in one command on worker threads, with progress and cancellation. | T2 | 4 |
| AB-STEEL-080 | Drawing export | Exports drawings to DXF, DWG, and PDF, singly or in batches, with sheet naming rules. | T1 | 4 |

## Material lists

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-STEEL-081 | Bill of materials | Produces material lists by part and by assembly with mark, profile, grade, length, count, mass, and coating, from list templates. | T1 | 4 |
| AB-STEEL-082 | Structured assembly list | Produces a list that shows each assembly with its child parts and sub-assemblies, with counts and mass roll-ups. | T2 | 4 |
| AB-STEEL-083 | Totals by profile and grade | Totals mass and length by profile and grade, and area by coating system. | T1 | 4 |
| AB-STEEL-084 | Erection and transport lists | Lists assemblies by phase, zone, and shipment with mass for erection and transport planning. | T2 | 4 |
| AB-STEEL-085 | List export | Exports lists to CSV, spreadsheet-ready text, and XML with configurable columns. | T1 | 4 |

## Fabrication output

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-STEEL-086 | NC output | Writes DSTV NC files per numbered part with profile, holes, cuts, notches, and markings, from a configuration that sets header values and hole-group handling. | T1 | 4 |
| AB-STEEL-087 | DXF plate output | Writes DXF profiles of plates and flat patterns with marks and scribing layers, ready for nesting. | T1 | 4 |
| AB-STEEL-088 | Output configuration | Defines per-machine output settings: header tokens, tolerances, marking text, and layer mapping. Configuration files are open and versioned. | T2 | 4 |
| AB-STEEL-089 | Marking and signature text | Adds marking text and part signatures to NC and DXF output, with position rules relative to part geometry. | T2 | 4 |
| AB-STEEL-090 | NC preview | Shows a preview of an NC file against the part geometry so that holes, cuts, and markings can be reviewed before release. | T2 | 4 |
| AB-STEEL-091 | Neutral steel exchange | Exports and imports steel models through IFC, CIS/2, and SDNF with identifiers retained, and reports data that cannot be mapped. | T2 | 4 |
| AB-STEEL-092 | Nesting hand-off | Writes plate and profile lists with quantities and grades in a documented open format for nesting and planning software. | T3 | 5 |

## Revisions and collaboration

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-STEEL-093 | Revision tracking | Compares the current model with an issued revision, lists changed, added, and removed parts, and marks affected drawings and output as outdated. | T2 | 4 |
| AB-STEEL-094 | Revision marks on drawings | Places revision clouds and revision table entries on drawings for changed parts, using the revision data of the document. | T2 | 4 |
| AB-STEEL-095 | Work areas for steel | Assigns zones of the detailing model to users, who borrow parts in their zone through Hub. Parts outside the zone are read-only. | T3 | 5 |
| AB-STEEL-096 | Marks exchange with the physical model | Writes assembly marks and part marks back to the physical structural elements, so engineering views and schedules show the same marks. | T1 | 4 |
