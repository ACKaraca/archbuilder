# Structural modeling

This area specifies the physical structural model: columns, framing, beam systems, trusses, braces, structural walls and floors, foundations, structural openings, joins and cutbacks, and the settings that control how structure is represented. Physical elements carry material, section, and usage data that the reinforcement, steel detailing, and analytical areas read, and every element maps to IFC structural entities with loss reporting.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Structural column | Section, material, base and top level with offsets, axis (vertical or slanted), rotation, usage | Hosted on a base level; attaches to floors, roofs, beams, or planes; has one analytical member; hosts reinforcement and connections; sits on a foundation |
| Framing member | Kind (beam, brace, chord, web, purlin), section, material, start and end points, justification, end offsets, cutbacks, camber | Supported by columns, walls, or other members; belongs to at most one beam system or truss; has one analytical member; hosts connections |
| Beam system | Boundary region, direction, spacing rule, member type, supports | Owns generated framing members; references supporting elements; regenerates when boundary or supports change |
| Truss | Truss type, span, depth, chord and web section assignments, support points | Owns chord and web framing members; references supports; exposes members as framing |
| Structural wall | Layered type with structural core, usage, location line, base and top constraints, height | Hosts openings, doors, and reinforcement; joins other walls and floors; has analytical panel |
| Structural floor | Layered type, boundary, slope, thickness, usage, span direction, deck profile | Hosts openings and reinforcement; supported by walls, framing, and columns; has analytical panel |
| Foundation element | Kind (isolated footing, strip footing, raft, pile, pile cap, grade beam), geometry, bearing elevation | Supports columns and walls; cuts site terrain; hosts reinforcement; has analytical member or panel |
| Structural opening | Shape, size, host, position, direction | Cuts a wall, floor, or framing member; may span multiple levels; feeds reinforcement trim data |
| Section profile | Shape definition, dimensions, standard designation, computed section properties | Referenced by columns, framing members, and families; belongs to a profile library |
| Structural material | Class, grade, density, strength and stiffness values, carbon coefficients | Referenced by elements, profiles, analytical members, schedules, and carbon calculations |
| Join record | Participating elements, join type, priority order, cutback result | Derived by regeneration; references the elements it resolves |
| Structural settings | Representation rules, tolerances, defaults per usage | Document-level; read by all structural elements and by analytical generation |

## Settings, materials, and sections

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-STRUCT-001 | Structural settings | Stores document-level structural settings: symbolic representation rules, default material per usage, join tolerance, analytical tolerances, and default cover. A change to a setting regenerates every element that reads it. | T1 | 4 |
| AB-STRUCT-002 | Structural material definitions | Defines materials with a class (concrete, steel, timber, masonry, aluminum, precast, composite, other), grade, density, strength and stiffness values, thermal values, and carbon coefficients. Elements, profiles, analytical members, schedules, and carbon calculations reference the same definition. | T1 | 4 |
| AB-STRUCT-003 | Standard profile library | Provides profile libraries keyed by national or regional standard, covering I and H sections, channels, angles, tees, rectangular and circular hollow sections, flats, and bars. Library data is stored as open, editable tables; choosing a profile sets dimensions and section properties on the element. | T1 | 4 |
| AB-STRUCT-004 | Custom section profiles | Creates a profile from a constrained sketch, including built-up, welded plate, tapered, and cellular shapes, using the same constraint solver as the family editor. The profile is saved to a library and reused by any member. | T2 | 4 |
| AB-STRUCT-005 | Section property computation | Computes area, centroid, second moments, section moduli, torsion constant, and shear center for any closed profile region and writes them to the profile. The user may override a value; an override is marked and survives regeneration. | T1 | 4 |
| AB-STRUCT-006 | Structural usage | Assigns usage (bearing, non-bearing, shear, combined, gravity, lateral system) to walls, floors, columns, and members. Usage controls analytical generation, schedule filters, and the IFC predefined type. | T1 | 4 |
| AB-STRUCT-007 | Structural families | Authors structural column, framing, and foundation families in the family editor with typed parameters, a section parameter, and a reference axis. Families load into documents as loadable content and appear in type pickers. | T1 | 4 |
| AB-STRUCT-008 | Type catalog loading | Loads a chosen subset of a family's types from a tabular catalog file with unit-aware values. The loader reports unknown columns and rejects values that fail unit parsing. | T2 | 4 |
| AB-STRUCT-009 | IFC mapping of structure | Maps each structural class to an IFC entity and predefined type, with material profile sets, base quantities, and property sets. Export and import report every element or property that cannot be mapped. | T1 | 4 |
| AB-STRUCT-010 | Structural material takeoff | Computes volume, mass, and count of elements by material, section, and level, net of joins and openings. The result feeds schedules, carbon calculations, and the steel and reinforcement lists. | T1 | 4 |

## Columns

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-STRUCT-011 | Vertical column placement | Places a column by base level, top level, and offsets, at a picked point, at grid intersections, or at every selected grid intersection in one command. The column stays bound to its levels. | T1 | 4 |
| AB-STRUCT-012 | Slanted column | Places a column between two three-dimensional points or by angle from vertical. The section orientation follows the axis, and the analytical member follows the axis. | T1 | 4 |
| AB-STRUCT-013 | Column attachment | Attaches the top or base of a column to a floor, roof, beam, or plane so that the column length follows the target. Detaching freezes the current height. | T1 | 4 |
| AB-STRUCT-014 | Level-driven columns | Re-lays columns when a base or top level moves. A column set to unconnected height keeps its height instead. | T1 | 4 |
| AB-STRUCT-015 | Column justification and rotation | Sets section rotation, offset from grid, and justification of the section against the axis. Changes update the analytical member and the join results. | T1 | 4 |
| AB-STRUCT-016 | Stacked column splices | Splits a tall column at chosen levels into segments with lap or splice rules, and keeps the segments linked as one logical column for schedules and analysis. | T2 | 4 |
| AB-STRUCT-017 | Columns from architectural columns | Creates structural columns at the positions of columns in a linked architectural model, copying position and size, and optionally monitors the source for movement. | T2 | 4 |
| AB-STRUCT-018 | Column grid generation | Creates columns for every grid intersection inside a picked region and removes those that coincide with existing columns. A node-graph input can supply a different layout. | T2 | 4 |

## Framing members

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-STRUCT-019 | Beam placement | Places beams by two points, as chains, along grid lines, or between picked columns and walls. The member snaps to supports and records them as references. | T1 | 4 |
| AB-STRUCT-020 | Framing justification and offsets | Sets vertical and horizontal justification, start and end offsets, rotation, and cut length. The analytical member keeps the work-line position while the solid follows the justification. | T1 | 4 |
| AB-STRUCT-021 | Framing end extensions | Extends or shortens a member end by a distance or to a picked face, independently of the analytical line. Quantities use the cut length. | T2 | 4 |
| AB-STRUCT-022 | Beam systems | Fills a boundary region with parallel beams by spacing rule: fixed distance, fixed count, maximum spacing, or clear spacing. The generated members are real framing members with their own tags and schedules. | T1 | 4 |
| AB-STRUCT-023 | Beam system regeneration | Regenerates the members of a beam system when the boundary, direction, or supports change. A member edited by hand is excluded from regeneration and flagged. | T1 | 4 |
| AB-STRUCT-024 | Braces | Places diagonal members between nodes on columns and beams, and generates single, X, chevron, and K patterns across a bay. Braces keep their end references when the frame moves. | T1 | 4 |
| AB-STRUCT-025 | Parametric trusses | Generates a truss between two supports from a type (parallel chord, pitched, Pratt, Warren, Howe, or custom family) with depth, panel count, and chord and web sections. Members are exposed as framing members for tagging and scheduling. | T2 | 4 |
| AB-STRUCT-026 | Truss editing | Edits individual truss members, swaps sections per chord or web group, and keeps the truss type's topology when span or depth changes. Members changed by hand are marked and retained on regeneration. | T2 | 4 |
| AB-STRUCT-027 | Curved and tapered members | Creates framing members that are curved along an arc or spline, polyline-bent, or tapered between two sections. Section properties and analytical sections are computed from the varying profile. | T2 | 4 |
| AB-STRUCT-028 | Member split and merge | Splits a member at a point or at intersections, and merges collinear members of equal section. Marks, analytical members, and connections are redistributed to the resulting members. | T1 | 4 |
| AB-STRUCT-029 | Frame generation by grid | Generates columns and beams for a selected grid and level range using chosen types, with edge and interior rules. The command previews the result before applying it. | T2 | 4 |
| AB-STRUCT-030 | Graph-driven framing | Reads a node graph that outputs member lines, sections, and types and creates or updates framing members from it. Members are tied to graph outputs and update when inputs change. | T3 | 5 |
| AB-STRUCT-031 | Camber and pre-set | Records camber on a framing member as a parameter that appears on schedules and fabrication outputs but does not alter the analytical line. | T2 | 4 |
| AB-STRUCT-032 | Framing annotation | Tags framing members with section, mark, and start and end elevations, and shows span direction on floors and decks. Tags update when the member or section changes. | T1 | 4 |

## Walls and floors

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-STRUCT-033 | Structural walls | Places layered walls whose core layer is structural, with usage, location line, and base and top constraints. Walls generate analytical panels and expose faces for reinforcement. | T1 | 4 |
| AB-STRUCT-034 | Shear wall and core grouping | Groups connected structural walls into a named core or lateral-system component, with combined area and inertia reporting. Group membership updates when walls are added or removed. | T3 | 5 |
| AB-STRUCT-035 | Structural floors | Creates slabs from a closed boundary with layered type, level offset, slope, and usage. Slopes are set by slope arrow, edge heights, or point heights, and the analytical panel follows. | T1 | 4 |
| AB-STRUCT-036 | Span direction | Sets the span direction of a floor or deck and shows a direction symbol in plan. Deck-direction changes update the analytical load distribution. | T1 | 4 |
| AB-STRUCT-037 | Metal deck and composite floors | Defines floors with a profiled deck, topping thickness, and shear connector layout. The deck profile and direction appear in sections, and quantities separate deck, topping, and connectors. | T2 | 4 |
| AB-STRUCT-038 | Slab thickenings and drop panels | Adds local thickening, drop panels, and edge beams to a slab through sketched regions with their own depth. The slab geometry and analytical thickness update. | T2 | 4 |
| AB-STRUCT-039 | Floor and wall edge profiles | Hosts edge profiles on slab edges and wall tops, such as upstands, curbs, and fascias, as profile sweeps that follow the host. | T2 | 4 |
| AB-STRUCT-040 | Ribbed and voided slabs | Generates ribbed, waffle, and voided slabs from rib and void parameters over a region. Quantities report concrete net of voids. | T3 | 5 |
| AB-STRUCT-041 | Precast subdivision | Subdivides walls and slabs into precast pieces by divisions, grids, or sketch lines, gives each piece a mark, and keeps pieces linked to the source element. Identical pieces share a type mark. | T2 | 5 |

## Foundations

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-STRUCT-042 | Isolated footings | Places a footing under a column with plan size, depth, and bearing elevation, and keeps it centered on the column when the column moves. The footing carries a base level and cuts terrain it penetrates. | T1 | 4 |
| AB-STRUCT-043 | Strip footings | Creates continuous footings under walls with width and depth that follow the wall and update when the wall is edited. | T1 | 4 |
| AB-STRUCT-044 | Raft foundations | Creates a foundation slab from a boundary with thickness, steps, and thickened zones. Analytical panels and reinforcement hosts update with the boundary. | T1 | 4 |
| AB-STRUCT-045 | Piles and pile caps | Places piles singly or in groups by pattern, with length, diameter, and cutoff elevation, and generates a pile cap sized to the group. | T2 | 4 |
| AB-STRUCT-046 | Grade and tie beams | Places beams at foundation level that are classified as foundation elements, tie columns and footings, and appear in foundation schedules. | T1 | 4 |
| AB-STRUCT-047 | Retaining walls | Creates retaining walls with a stem, base slab, and optional key as a single parametric element, retaining height, and fill side. Terrain is cut on the excavation side. | T2 | 4 |
| AB-STRUCT-048 | Excavation relationship | Cuts the site terrain where foundations penetrate it and reports excavation volume per foundation element. The cut updates as footings change. | T2 | 4 |

## Structural openings

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-STRUCT-049 | Wall and floor openings | Cuts rectangular, circular, or profile-shaped openings in walls and floors by host face, vertically through a floor, or across several levels. Openings move with the host and are listed in an opening schedule. | T1 | 4 |
| AB-STRUCT-050 | Openings in framing members | Cuts web and flange openings in framing members by shape, size, and position along the member. Openings expose their extents so that analysis and steel detailing can read them. | T2 | 4 |
| AB-STRUCT-051 | Opening trim data | Exposes the edges and trim extents of each opening so that reinforcement tools and detailing can place trim bars and plates. | T2 | 4 |

## Joins and cutbacks

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-STRUCT-052 | Framing join resolution | Resolves where columns, beams, and walls meet by a priority table (for example column over beam, primary beam over secondary beam). The lower-priority solid is cut back, and the result updates when either element moves. | T1 | 4 |
| AB-STRUCT-053 | End cuts | Applies mitre, cope, notch, and skew end cuts to framing members as parameters of the member end. The cuts are visible in sections and carried into steel detailing. | T2 | 4 |
| AB-STRUCT-054 | Geometry join and cut | Joins overlapping structural solids so shared faces and edges are hidden, and cuts one element with another. The user can switch which element wins the shared volume. | T1 | 4 |
| AB-STRUCT-055 | Per-end join control | Allows or disallows joining at each end of a member or wall, so a connection that needs a gap is not resolved automatically. | T2 | 4 |
| AB-STRUCT-056 | Join-aware quantities | Counts overlapping concrete volume once and reports it against the winning element. Quantities in schedules and takeoffs use the resolved geometry. | T1 | 4 |
| AB-STRUCT-057 | Background join regeneration | Computes join results on worker threads and applies a result only if it matches the current document revision. Editing is never blocked while joins recompute. | T2 | 4 |

## Representation and documentation

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-STRUCT-058 | Symbolic representation | Draws columns, beams, and braces as lines with symbols at coarse detail and as solids at fine detail, controlled by structural settings and view detail level. Switching detail level does not alter the model. | T1 | 4 |
| AB-STRUCT-059 | Structural plan views | Provides plan views with a view range, an underlay of the level below, and a discipline filter that shows structure in full and other disciplines dimmed. | T1 | 4 |
| AB-STRUCT-060 | Framing elevations | Produces elevation views of braced frames with members shown by section and mark, and brace and connection symbols. | T2 | 4 |
| AB-STRUCT-061 | Structural schedules | Provides schedules for columns, framing, foundations, openings, and materials, including a graphical column schedule that groups columns by section along levels. Schedule edits write to the elements. | T1 | 4 |

## Checks and analysis aids

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-STRUCT-062 | Structural model checks | Runs rule-based checks for members without sections, unsupported member ends, columns without foundations, duplicate or overlapping members, and floating elements. Checks run headless and report to the command line and the issue list. | T2 | 4 |
| AB-STRUCT-063 | Load path trace | Traces from a selected element downward through the supports it references to the foundation and highlights the chain. A broken chain is reported with the first unsupported element. | T3 | 5 |
| AB-STRUCT-064 | Structural change comparison | Compares two revisions of the structural model and lists added, removed, and changed members by section, material, length, and position. The report is text-diffable and exportable. | T3 | 5 |
| AB-STRUCT-065 | Structural parameter set manager | Edits shared structural parameters across selected families and instances in one transaction with a preview of the affected elements. | T3 | 5 |
