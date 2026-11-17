# Floors, roofs and ceilings

This area specifies horizontal and sloped enclosure: sketch-based floors with layered types and
shape editing, roofs by footprint and by extrusion with their edge elements and dormers, and
ceilings. Structural floors and slabs extend these objects in
[20-structural-modeling.md](20-structural-modeling.md).

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Floor | Boundary profile with holes, level and offset, slope definition, shape points, room-bounding flag, structural flag | Instance of a floor type; hosts inserts and edges; joins walls |
| Floor type, roof type, ceiling type | Ordered layers with variable layer, function, rafter cut (roofs), pattern (ceilings) | System family types |
| Boundary profile | Rings of line and arc edges with stable edge IDs; edges may be associated to walls | Owned by a floor, roof, ceiling or opening |
| Roof | Footprint or extrusion profile, per-edge slope and overhang, base level and offset, cutoff | Instance of a roof type; owns face cells; hosts edges, dormers and inserts |
| Roof face cell | Plane, domain polygon, layer offsets | Derived from the roof solve |
| Edge element | Profile, host edges, miter at corners | Fascia, gutter, soffit, floor edge or sweep along host edges |
| Dormer | Host roof, local placement, width, depth, pitch, owned walls and opening | Reconciles its roof, walls and host cut as one assembly |
| Ceiling | Boundary profile, level and offset, slope, pattern origin and angle | Instance of a ceiling type; hosts fixtures |

## Floors

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-SLAB-001 | Floor by sketch or walls | Creates a floor from a sketched boundary of lines and arcs with holes, or by picking walls with offset to their core or finish face, on a level with offset. Self-intersecting rings, overlapping holes and edges shorter than the model tolerance are rejected before commit. | T1 | 2 |
| AB-SLAB-002 | Associative boundaries | Keeps boundary edges created by picking walls associated with those walls, so the floor follows when a wall moves or changes thickness. Deleting the wall keeps the edge at its last position and reports the lost association. | T1 | 2 |
| AB-SLAB-003 | Layered floor types | Defines floor types as layers with function, material and thickness, including one variable layer that absorbs slope while the other layers keep constant thickness. | T1 | 2 |
| AB-SLAB-004 | Floor slope | Slopes a floor by a slope arrow, by a sloped boundary edge, or by elevations at three points. | T1 | 2 |
| AB-SLAB-005 | Shape editing | Adds points and split lines on a floor top and sets their elevations, producing drained or warped surfaces. With a variable layer the bottom stays flat; without one the whole assembly follows the shape. | T2 | 2 |
| AB-SLAB-006 | Floor edges | Sweeps a profile along selected floor edges for slab edges, curbs and nosings, mitered at corners and following edge changes. | T2 | 2 |
| AB-SLAB-007 | Floor and wall joins | Resolves floor and wall intersections by layer priority so cores join cores, and lets walls attach their base or top to floor surfaces. Join order between a floor and a wall can be switched. | T1 | 2 |

## Roofs

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-SLAB-008 | Roof by footprint | Creates a roof from a closed footprint where each edge either defines slope (with its own pitch) or does not. Edge settings produce gable, hip, shed and mansard forms; the solver computes ridges, hips and valleys and produces closed face cells covering the footprint without overlap. | T1 | 2 |
| AB-SLAB-009 | Overhangs and wall alignment | Sets overhang per edge, with the option to measure the footprint from wall cores. Picking walls for the footprint keeps edges associated with those walls. | T1 | 2 |
| AB-SLAB-010 | Complex footprints | Solves concave footprints, unequal pitches and holes, where a hole is either a plain aperture or a courtyard with its own inner eaves and slopes. A configuration with no valid continuous roof is rejected with the location of the conflict, never with invented vertical faces. | T1 | 2 |
| AB-SLAB-011 | Roof by extrusion | Creates a roof by extruding an open profile sketched in a vertical plane between start and end distances, with layers offset normal to the profile. | T1 | 2 |
| AB-SLAB-012 | Roof types and rafter cut | Defines roof types as layers and sets the rafter cut at eaves (plumb, square, two-cut) and fascia depth, which shape the eave edge of every roof of that type. | T2 | 2 |
| AB-SLAB-013 | Roof edge elements | Places fascias, gutters, soffits and roof sweeps along selected roof edges from profiles, mitered at corners, and regenerates them when edges move. | T2 | 2 |
| AB-SLAB-014 | Roof joins and cuts | Extends one roof to meet another roof's face and joins them, or cuts one roof with another, and keeps the relationship on later edits. | T2 | 2 |
| AB-SLAB-015 | Dormers | Creates a dormer assembly (dormer roof, cheek and front walls, host roof cut) from width, depth, pitch and placement on a host roof. The assembly regenerates as one unit; editing a member directly is redirected to the dormer, and deleting the host deletes the dormer. | T2 | 2 |
| AB-SLAB-016 | Glazed roofs | Gives a roof type a curtain grid with panels and mullions on each face, reusing the grid rules of AB-WALL-020. | T2 | 2 |
| AB-SLAB-017 | Roofs from faces | Creates roofs from faces of mass or generic elements and keeps them associated until the association is broken. | T3 | 4 |

## Ceilings

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-SLAB-018 | Ceiling by room or sketch | Creates a ceiling by picking inside a region enclosed by walls at the ceiling height or from a sketched boundary, on a level with offset. | T1 | 2 |
| AB-SLAB-019 | Ceiling types and grid patterns | Defines ceiling types as layers or a single thickness, with a model pattern for grid tiles. The pattern origin and angle are editable per ceiling and stay fixed when the boundary changes. | T1 | 2 |
| AB-SLAB-020 | Sloped ceilings | Slopes a ceiling by a slope arrow or a sloped boundary edge. | T2 | 2 |
