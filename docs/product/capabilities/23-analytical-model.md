# Analytical model

This area specifies the structural analytical model: members and panels derived from physical elements, nodes, boundary conditions, loads, load cases and combinations, association rules, consistency checks, exchange with analysis software, and the display of results. The analytical model is a separate, editable layer that stays associated with the physical model and exports to open exchange formats.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Analytical member | Axis curve, section, material, end releases, rigid offsets, orientation, status | Associated with one physical column, framing member, or foundation element; connects to nodes; carries loads |
| Analytical panel | Boundary loops, thickness, material, orientation, local axes, openings, support edges | Associated with a physical wall, floor, or foundation slab; connects to nodes along edges; carries loads |
| Analytical node | Position, merged members and panel edges, support, pinned flag | Joins members and panels; carries a boundary condition |
| Boundary condition | Kind (point, line, area), translational and rotational restraints or spring stiffnesses, local axes | Attached to a node, edge, or panel; may follow a foundation element |
| Load | Kind (point, line, area, temperature, self-weight), magnitude, direction, distribution, nature | Belongs to a load case; hosted on a member, panel, node, or physical element |
| Load case | Name, nature (dead, live, wind, snow, seismic, temperature, other), self-weight factor | Owns loads; participates in combinations |
| Load combination | Name, kind (ultimate, serviceability, accidental), case factors | References load cases; read by analysis exchange |
| Analytical association | Physical element, analytical element, sync mode, pinned flag | Links the two models; stores the last synchronized revision |
| Analysis exchange job | Format, target, scope, mapping table, loss report | Reads the analytical model; writes files; imports results |
| Analysis result set | Source, result kind, values per member, node, and panel, load case or combination | Referenced by display styles and size updates |
| Result display style | Quantity, scale, colors, limits, labels | Applied per view to an analysis result set |

## Analytical members

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ANALYT-001 | Member generation | Creates an analytical member for each structural column, framing member, brace, and foundation beam that has analysis enabled. The member takes section, material, and axis from the physical element. | T1 | 4 |
| AB-ANALYT-002 | Analysis enable and disable | Enables or disables analytical generation per element and by structural usage. A disabled element keeps no analytical member and is listed in the analytical report. | T1 | 4 |
| AB-ANALYT-003 | End releases | Sets pinned, fixed, and partial releases for each translation and rotation at each member end. Releases appear as symbols in the analytical view. | T1 | 4 |
| AB-ANALYT-004 | Rigid offsets | Sets rigid end zones that model the finite size of joints, either computed from connecting elements or specified by value. | T2 | 4 |
| AB-ANALYT-005 | Local axes and orientation | Shows and edits member local axes and the section orientation angle so that the analysis section matches the physical element. | T1 | 4 |
| AB-ANALYT-006 | Member subdivision | Divides long analytical members at nodes created by intersecting members and at user-specified points, while the physical element stays one element. | T2 | 4 |
| AB-ANALYT-007 | Analytical section assignment | Assigns an analysis section to a member independently of its physical section, for cases where analysis uses a simplified or cracked section. The override is flagged. | T2 | 4 |
| AB-ANALYT-008 | Analytical position adjustment | Moves an analytical member away from the physical position and records the adjustment so that synchronization does not undo it. | T2 | 4 |

## Analytical panels

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ANALYT-009 | Panel generation | Creates an analytical panel for each structural wall, floor, and foundation slab with analysis enabled, with a mid-surface boundary and thickness. | T1 | 4 |
| AB-ANALYT-010 | Thickness from structural layers | Derives panel thickness and material from the structural layers of the physical element, excluding non-structural layers such as finishes and blinding. | T1 | 4 |
| AB-ANALYT-011 | Panel splitting | Splits a panel at grids, levels, sketched lines, or edges of other panels so that meshes and loads follow the structural layout. | T2 | 4 |
| AB-ANALYT-012 | Panel openings | Carries openings from the physical element into the panel, with a size threshold below which openings are ignored. | T1 | 4 |
| AB-ANALYT-013 | Panel local axes | Shows and edits panel local axes and orientation, and sets the reference surface used for eccentricity. | T1 | 4 |
| AB-ANALYT-014 | Panel edge connectivity | Connects panel edges to members and other panels, and lists edges that lie within tolerance but are not connected. | T1 | 4 |
| AB-ANALYT-015 | Cladding panels for wind | Defines non-structural cladding panels that carry only wind loads and distribute them to supporting analytical elements. | T3 | 5 |

## Nodes and connectivity

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ANALYT-016 | Node generation | Creates nodes at member ends and panel corners, merging nodes within the connectivity tolerance and keeping the original positions for reference. | T1 | 4 |
| AB-ANALYT-017 | Node editing | Moves, merges, and splits nodes. A manually edited node is flagged and kept stable through regeneration. | T2 | 4 |
| AB-ANALYT-018 | Pinned analytical position | Pins an analytical node or member so its position does not follow physical changes until released. | T2 | 4 |
| AB-ANALYT-019 | Unhosted nodes | Marks nodes as independent of physical connectivity so they influence analysis only and do not change geometry. | T3 | 5 |
| AB-ANALYT-020 | Connectivity tolerances | Sets tolerances for node merging, member-to-panel snapping, and column-to-beam alignment, with separate values per discipline setting. | T1 | 4 |

## Boundary conditions

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ANALYT-021 | Point supports | Applies fixed, pinned, roller, and spring supports at nodes, with restraints on each translation and rotation and a local axis choice. | T1 | 4 |
| AB-ANALYT-022 | Line and area supports | Applies line supports along panel edges and member axes and area supports on panels, with stiffness per unit length or area. | T1 | 4 |
| AB-ANALYT-023 | Supports from foundations | Derives supports from foundation elements, with fixed, pinned, or spring supports from soil stiffness values assigned to footings and rafts. | T2 | 4 |
| AB-ANALYT-024 | Support display | Shows supports with symbols that follow the restraint pattern in the analytical view and plan views. | T1 | 4 |

## Loads

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ANALYT-025 | Point, line, and area loads | Creates point loads on nodes and members, line loads on members and edges, and area loads on panels, each with magnitude, direction, and load case. | T1 | 4 |
| AB-ANALYT-026 | Hosted loads | Hosts loads on physical elements, such as an area load on a floor or a line load on a beam, and updates the load extents when the host changes. | T1 | 4 |
| AB-ANALYT-027 | Self-weight | Computes self-weight from material density and geometry, with a factor per load case. | T1 | 4 |
| AB-ANALYT-028 | Load distribution | Distributes area loads on floors with a one-way or two-way rule to supporting beams and walls, with the rule recorded on the load. | T2 | 4 |
| AB-ANALYT-029 | Load definitions from usage | Assigns standard imposed loads by space occupancy type from a load table, and writes them as area loads on floors in those spaces. | T2 | 4 |
| AB-ANALYT-030 | Temperature and settlement loads | Defines uniform and gradient temperature loads and imposed support displacements by load case. | T2 | 4 |
| AB-ANALYT-031 | Wind loads on enclosure | Generates wind pressures on façade and roof panels from a wind speed, terrain category, and pressure coefficients, distributed over several panels. | T3 | 5 |
| AB-ANALYT-032 | Snow and seismic load tools | Generates snow loads on roofs and equivalent seismic masses and forces by story from code parameters. Parameter tables are open data. | T3 | 5 |

## Load cases and combinations

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ANALYT-033 | Load cases | Defines load cases with a name, nature, and self-weight factor. A case can be activated per view for display of its loads. | T1 | 4 |
| AB-ANALYT-034 | Load combinations | Defines combinations as factored sums of cases, and generates standard ultimate and serviceability combinations from a code table. | T1 | 4 |
| AB-ANALYT-035 | Combination tables | Edits combination factors in a table with validation of case natures and exclusivity, and exports the table to the analysis exchange. | T2 | 4 |
| AB-ANALYT-036 | Load schedule | Lists loads by case, element, and magnitude, and totals applied load by case and level. | T1 | 4 |

## Association with the physical model

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ANALYT-037 | Automatic synchronization | Regenerates the analytical model after physical edits by default, applying updates incrementally and in the background. Synchronization respects pinned and manually edited items. | T1 | 4 |
| AB-ANALYT-038 | Association status | Shows for each analytical element its physical element and sync state (in sync, pinned, manually edited, orphaned). Orphans are listed and can be removed. | T1 | 4 |
| AB-ANALYT-039 | Manual synchronization mode | Switches a document or a selection to manual synchronization and lists pending differences before applying them in one transaction. | T2 | 4 |
| AB-ANALYT-040 | Reassociation | Re-associates an analytical element with a different physical element after a physical element is replaced, keeping loads and supports. | T2 | 4 |
| AB-ANALYT-041 | Analytical-only elements | Creates analytical members and panels with no physical counterpart, for modeling idealizations or unbuilt elements. | T2 | 4 |
| AB-ANALYT-042 | Selection and isolation | Selects the physical element from the analytical one and the reverse, and isolates analytical elements per view and discipline. | T1 | 4 |

## Consistency checks

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ANALYT-043 | Consistency check | Reports duplicate nodes, unconnected members and panels, members shorter than a limit, panels with gaps, and elements with missing sections or materials. Each finding names the element and offers a fix command. | T1 | 4 |
| AB-ANALYT-044 | Stability check | Reports instability candidates such as free rotations, missing supports, and mechanisms from a rank test of the support and release pattern. | T2 | 4 |
| AB-ANALYT-045 | Load completeness check | Reports load cases with no loads, loads with no host, and combinations that reference missing cases. | T2 | 4 |
| AB-ANALYT-046 | Headless analytical checks | Runs the consistency, stability, and load checks from the command line with a machine-readable report. | T2 | 4 |

## Exchange with analysis software

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ANALYT-047 | IFC structural analysis export | Exports the analytical model as an IFC structural analysis view with members, surfaces, connections, supports, actions, load groups, and combinations, and reports unmapped data. | T1 | 4 |
| AB-ANALYT-048 | Analysis exchange import | Imports analysis models and result sets from the same open exchange format and maps them to existing analytical elements by identifier, or lists unmatched elements. | T2 | 4 |
| AB-ANALYT-049 | Documented exchange format | Reads and writes a documented JSON-based analysis exchange format with members, panels, nodes, supports, loads, and results, usable from scripts and external solvers. | T1 | 4 |
| AB-ANALYT-050 | Section updates from analysis | Applies section sizes from imported results to physical elements in one command with a preview of changed elements and the resulting quantity change. | T2 | 4 |
| AB-ANALYT-051 | Open solver bridge | Runs a linear static analysis in an open finite element solver through an isolated process, with the model written to and results read from the exchange format. | T3 | 5 |
| AB-ANALYT-052 | Exchange mapping tables | Maps sections, materials, and load case names between the model and an analysis program through editable tables stored in the document. | T2 | 4 |

## Results display

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ANALYT-053 | Member diagrams | Displays axial force, shear, moment, torsion, and deflection diagrams on members for a chosen case or combination, with scale and label controls. | T1 | 4 |
| AB-ANALYT-054 | Panel contours | Displays contours of stresses, forces per unit length, and displacements on panels with a legend and limit controls. | T2 | 4 |
| AB-ANALYT-055 | Support reactions | Displays reaction forces and moments at supports and tabulates them by case and combination. | T1 | 4 |
| AB-ANALYT-056 | Utilization display | Colors members by utilization ratio from imported results, with a threshold and a list of members above one. | T2 | 4 |
| AB-ANALYT-057 | Results tables | Lists result values per element, node, case, and combination in sortable tables that export to CSV. | T1 | 4 |
| AB-ANALYT-058 | Result display styles | Saves display styles (quantity, scale, colors, limits) and applies them per view, so one model can show several result views at once. | T2 | 4 |
| AB-ANALYT-059 | Result staleness | Marks a result set as outdated when the analytical model has changed since the results were produced and shows that state in every view using it. | T1 | 4 |
