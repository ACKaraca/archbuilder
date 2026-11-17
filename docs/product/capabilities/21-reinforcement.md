# Reinforcement

This area specifies reinforcing steel in concrete and masonry hosts: individual bars and their shapes, sets and distributions, area and path reinforcement, fabric, couplers, cover, host constraints, numbering, bending schedules, free-form reinforcement, and visibility for detailing. Reinforcement is real, countable, weighable geometry that follows its host, and it exports to IFC reinforcement entities and to bar bending schedule formats.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Rebar bar | Shape, bar diameter, steel grade, segment lengths, bend radii, hooks, total length, mass, number mark | Belongs to a host element; constrained to host faces; may belong to a rebar set; scheduled in a bending schedule |
| Rebar shape | Code, parameter list, segment topology, bend rules, picture | Referenced by bars; stored in the shape library |
| Rebar set | Layout rule, bar count, spacing ranges, distribution path and plane, first and last bar options | Owns generated bar instances; references a host and a bar shape |
| Area reinforcement | Boundary region, major direction, layers (top, bottom, each way), bar types, spacing, hooks | Hosted on a slab or wall; owns generated sets; honors openings |
| Path reinforcement | Path curve, side, bar types, spacing, lap rules | Hosted on a floor or wall; owns generated sets |
| Reinforcement fabric | Sheet size, wire diameters, wire spacings, overlap, layers | Hosted on a slab or wall; scheduled by sheet |
| Coupler | Type, size, position, connected bars | Joins two bars; hosted on the host element |
| Cover rule | Distance per face, exposure class, host reference | Belongs to a host element or a host face; read by bars |
| Free-form reinforcement | Host solid, guide surfaces or curves, spacing, bar type | Generated across curved or morphed hosts; owns generated bars |
| Bending schedule | Included bars, grouping, columns, totals, shape images | Reads bars and fabric; exports to files |
| Numbering rule | Filter, grouping parameters, number format, target parameter | Writes bar marks and element marks |
| Reinforcement settings | Cover defaults, bend rule tables, detailing standard, display options | Document-level; read by all reinforcement tools |

## Settings and libraries

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-REBAR-001 | Reinforcement settings | Stores the detailing standard, default cover per exposure class, minimum bend diameters per bar size, hook types, and display options. Changing a setting revalidates all bars against the new rules. | T1 | 4 |
| AB-REBAR-002 | Bar type library | Defines bar types by diameter, grade, surface, nominal mass per length, and standard designation. Libraries are open tables selectable by region. | T1 | 4 |
| AB-REBAR-003 | Rebar shape library | Provides a library of standard shapes with parameters for each segment and bend, picture, and a shape code in the selected detailing standard. Users add custom shapes by sketching a bar path and naming its segments. | T1 | 4 |
| AB-REBAR-004 | Bend and hook rules | Computes bend diameters, hook lengths, and developed lengths from bar size and the active rule table. A bar violating a minimum bend diameter is flagged and cannot be scheduled until fixed or overridden with a recorded reason. | T1 | 4 |

## Bars

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-REBAR-005 | Single bar placement | Places one bar of a chosen shape in a host by picking a work plane and sketch points, or by choosing a preset such as straight, stirrup, or U-bar. The bar is attached to the host and keeps cover when the host changes. | T1 | 4 |
| AB-REBAR-006 | Bar by sketch | Creates a bar by sketching its centerline on a work plane in the host. The command derives segments, bends, and hooks, and offers a matching shape from the library. | T1 | 4 |
| AB-REBAR-007 | Shape parameter editing | Edits segment lengths and angles of a bar through the shape parameters, and updates length, mass, and the schedule entry. Edits that break the shape topology are rejected with a reason. | T1 | 4 |
| AB-REBAR-008 | Hooks and end treatments | Applies hooks, bends, and straight extensions at bar ends by type and orientation, and reports the resulting length. | T1 | 4 |
| AB-REBAR-009 | Bar length and mass | Computes the cut length and mass of each bar from geometry, bend deductions, and nominal mass per length. Mass is available on every bar for schedules and carbon calculations. | T1 | 4 |
| AB-REBAR-010 | Rebar cages | Generates a complete cage for a column, beam, wall end, pile, or footing from a template with longitudinal bars, stirrups, and ties. Cage parameters stay editable, and the cage regenerates when the host dimensions change. | T2 | 4 |
| AB-REBAR-011 | Bar library from family | Loads bars and cages defined in family files as reusable reinforcement types, with parameters exposed on the instance. | T3 | 5 |

## Sets and distributions

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-REBAR-012 | Layout rules | Distributes a bar as a set by one of these rules: single, fixed number, maximum spacing, spacing with number, minimum clear spacing. The set regenerates when the host or the distribution path changes. | T1 | 4 |
| AB-REBAR-013 | Varied spacing | Defines a set with several spacing ranges, specified as number of spaces at a spacing, length at a spacing, or percentage of the set length. The total must equal the set length, and the command reports any mismatch. | T2 | 4 |
| AB-REBAR-014 | Distribution path and plane | Sets the direction and plane of a distribution, including curved paths along a host edge, and the offset of the first and last bar. | T1 | 4 |
| AB-REBAR-015 | Set visibility per bar | Shows a set as first, last, middle, every nth, selected, or all bars, per view. Hidden bars remain in the model and in schedules. | T1 | 4 |
| AB-REBAR-016 | Set editing | Edits one bar of a set to make it an individual bar, or restores it to the set. Quantities update immediately. | T2 | 4 |

## Area and path reinforcement

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-REBAR-017 | Area reinforcement | Fills a slab or wall region with bar layers in a major and minor direction, top and bottom, with bar type, spacing, and hooks per layer. Openings in the host are honored by clipping or by trim bars. | T1 | 4 |
| AB-REBAR-018 | Area reinforcement boundary | Defines the region from the host face, a sketched boundary, or a selected portion of the host, and updates the layers when the boundary is edited. | T1 | 4 |
| AB-REBAR-019 | Path reinforcement | Places bars along a path on a floor or wall, such as edge bars and additional top bars, with side, spacing, and lap rules. | T1 | 4 |
| AB-REBAR-020 | Reinforcement symbols and tags | Annotates area and path reinforcement with a symbol and a text that states bar type and spacing. The text updates when the reinforcement changes. | T1 | 4 |
| AB-REBAR-021 | Laps and splices | Computes lap lengths from bar size, grade, and concrete class using the detailing standard, and places lap segments on adjacent bars or sets. The lap rules are open tables. | T2 | 4 |

## Fabric and couplers

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-REBAR-022 | Reinforcement fabric | Lays fabric sheets over a slab or wall region by sheet type, overlap, and orientation, and counts sheets and wire length. Sheets honor openings and edges. | T2 | 4 |
| AB-REBAR-023 | Fabric types | Defines fabric types by sheet dimensions, wire diameters, and spacings in each direction, with nominal mass per area. Fabric appears in schedules as sheets with weight. | T2 | 4 |
| AB-REBAR-024 | Couplers | Places mechanical couplers at the ends of two bars with type and size, moves the bars to the coupler position, and lists couplers in schedules. | T2 | 4 |

## Cover and host constraints

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-REBAR-025 | Cover per host face | Assigns concrete cover to host elements and to individual host faces, referenced from a cover rule. Bars placed against a face keep that cover distance. | T1 | 4 |
| AB-REBAR-026 | Cover checks | Reports bars whose clear distance to a host face is below the cover rule and highlights them in the view. The check runs on demand and in headless model checking. | T1 | 4 |
| AB-REBAR-027 | Host face constraints | Constrains bar segments to host faces so that bars follow the host when its dimensions change. A constraint that cannot hold is released and reported. | T1 | 4 |
| AB-REBAR-028 | Reinforcement in joined hosts | Treats geometry-joined concrete elements as one host region for cover and clipping, and allows bars to cross the join. | T2 | 4 |
| AB-REBAR-029 | Reinforcement for precast pieces | Hosts reinforcement on each precast piece separately, with a layout copied to identical pieces. Mass and counts roll up per piece type. | T2 | 5 |

## Free-form reinforcement

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-REBAR-030 | Free-form bars by guide curve | Generates bars along guide curves across a curved host, such as a shell, arch, or tunnel lining, at a spacing along the host surface. | T2 | 5 |
| AB-REBAR-031 | Morphed reinforcement | Deforms a regular reinforcement layout to follow a curved or tapering host solid while keeping the bar count and spacing rule. | T3 | 5 |
| AB-REBAR-032 | Transverse bars on free-form hosts | Places transverse bars across free-form layouts at a spacing along the guide path, with cover from the host surface. | T3 | 5 |
| AB-REBAR-033 | Reinforcement from graph | Generates bar geometry from a node graph that outputs curves and bar types and keeps the bars tied to graph outputs. | T3 | 5 |

## Numbering

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-REBAR-034 | Bar numbering | Assigns marks to bars so that bars of identical shape, size, and dimensions share one mark. The tolerance for identical bars is a setting. | T1 | 4 |
| AB-REBAR-035 | Rule-based numbering | Numbers elements and bars by rules that select with a filter, group by parameter values, and write a formatted number to a chosen parameter. The numbers refresh when the driving parameters change. | T2 | 4 |
| AB-REBAR-036 | Mark locking and reservation | Locks chosen marks so renumbering preserves them, and reserves number ranges per host or per zone. | T2 | 4 |
| AB-REBAR-037 | Numbering report | Lists unnumbered bars, duplicate marks on non-identical bars, and marks changed since the last numbering run. | T2 | 4 |

## Schedules and bending schedules

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-REBAR-038 | Bending schedule | Produces a bar bending schedule with mark, shape code, shape picture, bar size, count, segment lengths, total length, and mass. Rows group by mark, host, level, or zone as selected. | T1 | 4 |
| AB-REBAR-039 | Schedule standards | Formats the schedule according to a selected detailing standard, including shape code conventions and dimension letters. The format definition is an open template. | T2 | 4 |
| AB-REBAR-040 | Reinforcement quantities | Reports total reinforcement mass and mass per cubic meter of concrete by host, level, and element type. Values feed carbon calculations. | T1 | 4 |
| AB-REBAR-041 | Schedule export | Exports bending schedules and quantities to CSV and to a machine-readable bar list format, and to IFC reinforcement entities with mass and shape data. | T1 | 4 |
| AB-REBAR-042 | Fabrication bar list output | Writes a bar list for bending machines in a documented open format, with one record per mark and its segment geometry. | T3 | 5 |

## Visibility and detailing

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-REBAR-043 | Rebar view visibility | Shows reinforcement per view as unobscured, as solid, or hidden, per category and per host, with a cut-aware display in sections. | T1 | 4 |
| AB-REBAR-044 | Reinforcement sections | Creates section and detail views of a host with its reinforcement, sized to show bar cross-sections and stirrups at a chosen scale. | T1 | 4 |
| AB-REBAR-045 | Reinforcement tags and dimensions | Tags bars and sets with mark, count, size, and spacing, and dimensions bar segments and spacing in views. Tag text updates with the bars. | T1 | 4 |
| AB-REBAR-046 | Detail lines from reinforcement | Generates two-dimensional detail representation of a set, drawing the first and last bar and a distribution line, from the three-dimensional set. | T2 | 4 |

## Checks and analysis

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-REBAR-047 | Reinforcement clash detection | Detects bars that intersect other bars, embedded items, or openings beyond a tolerance, and reports them with positions. | T2 | 4 |
| AB-REBAR-048 | Minimum and maximum spacing checks | Checks bar spacing against minimum clear spacing and maximum spacing from the detailing standard and lists violations by host. | T2 | 4 |
| AB-REBAR-049 | Embodied carbon of reinforcement | Computes carbon from bar mass and per-material coefficients, and reports it per host and per level alongside concrete carbon. | T2 | 4 |
| AB-REBAR-050 | Reinforcement change comparison | Compares two revisions and lists bars added, removed, or changed in mark, size, or length per host. | T3 | 5 |
