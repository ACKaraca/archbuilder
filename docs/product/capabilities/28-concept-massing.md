# Concept massing

This area specifies early-design modeling: direct solid sketch modeling, mass forms and mass floors, area and volume program analysis, groups and levels in concept models, conversion of mass faces to building elements, solar and daylight studies on masses, site context, and generative layout studies. Masses are lightweight solids that can be sketched, compared, and analyzed quickly, and they convert into associated building elements when the design advances.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Mass | Solid or void body, name, layer, material assignment, program type, computed volume and surface area | Contains faces and edges; cut by mass floors; owner of converted elements; belongs to a group |
| Mass group | Name, member masses and groups, instance transform | Defines reusable content; instanced in the concept model; editable in a nested edit context |
| Mass instance | Group definition, transform, per-instance overrides | Shares its definition with other instances |
| Mass level | Name, elevation, height to next level | Slices masses into mass floors; may map to document levels |
| Mass floor | Source mass, level, boundary, gross area | Derived from a mass at a level; owns program areas |
| Program area | Program type, target area, actual area, boundary, color | Subdivides a mass floor; read by program tables |
| Program type | Name, color, target gross area, efficiency target | Assigned to program areas; summarized per mass and level |
| Mass face conversion | Source face, target kind (wall, floor, roof, curtain system), element reference, state | Links a mass face to a generated element; flags when the face changes |
| Site context | Terrain, neighboring buildings, boundary, location and north | Provides context for studies; imported or authored; can convert to editable elements |
| Study definition | Kind (sun, shadow, radiation, daylight, wind estimate), time range, targets, parameters | Reads masses and context; produces a result set |
| Study result | Values per face or sample point, legend, source revision | Derived; displayed as overlays; stale when sources change |
| Generative study | Inputs, constraints, goals, generated options, ranking | Reads site and program; produces mass options |

## Sketch and solid modeling

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-MASS-001 | Sketch tools | Draws lines, polylines, arcs, circles, rectangles, and polygons on the ground plane, on a face, or on a work plane. Closed loops form faces automatically. | T1 | 3 |
| AB-MASS-002 | Push and pull | Moves a face along its normal by dragging or by a typed value to add or remove volume, and merges the result into the body. A face that cannot move without breaking the solid reports why. | T1 | 3 |
| AB-MASS-003 | Face split and merge | Splits a face by drawing across it and merges coplanar faces when an edge is removed, keeping the body valid. | T1 | 3 |
| AB-MASS-004 | Inference | Snaps to endpoints, midpoints, centers, axes, perpendicular and parallel directions, tangents, and alignments with other geometry, with a visible indicator of each active inference. | T1 | 3 |
| AB-MASS-005 | Typed value entry | Accepts a typed numeric value with units and expressions during any drawing or modifying drag, and applies it on confirmation. | T1 | 3 |
| AB-MASS-006 | Move, copy, rotate, scale | Transforms selected geometry with multiple copies and array-by-typing, and scales uniformly or by axis with a reference. | T1 | 3 |
| AB-MASS-007 | Mirror | Reflects selected geometry or instances across a picked plane, with an option to keep the original. | T1 | 3 |
| AB-MASS-008 | Arrays | Creates linear and radial arrays of geometry or group instances by count and spacing. | T1 | 3 |
| AB-MASS-009 | Offset | Offsets edges and faces to create walls, slabs, and setbacks by a typed distance, in either direction. | T1 | 3 |
| AB-MASS-010 | Sweep along path | Sweeps a profile along a path, curved or straight, to create moldings and curved bodies. | T2 | 3 |
| AB-MASS-011 | Loft and revolve | Creates bodies by lofting between profiles and by revolving a profile about an axis. | T2 | 3 |
| AB-MASS-012 | Boolean operations | Combines solid bodies by union, subtraction, and intersection, with the result kept as one mass or as separate masses. | T1 | 3 |
| AB-MASS-013 | Slice | Cuts a body by a plane, face, or curve to split a mass into parts. | T1 | 3 |
| AB-MASS-014 | Edge rounding | Rounds or chamfers selected edges by a typed radius or distance. | T2 | 3 |
| AB-MASS-015 | Primitive shapes | Creates boxes, cylinders, spheres, cones, and wedges by drag or by typed size. | T1 | 3 |
| AB-MASS-016 | Surface and mesh bodies | Holds open surfaces and imported meshes as limited-edit bodies with an indicator that they are not solid, and offers a repair command to close them. | T2 | 3 |
| AB-MASS-017 | Direction locks | Locks drawing direction to an axis or to inferred directions by key, and shows the lock in the viewport. | T1 | 3 |
| AB-MASS-018 | Cleanup | Removes duplicate and hidden edges, merges coplanar faces, and heals small gaps, and reports every change made. | T1 | 3 |
| AB-MASS-019 | Align and distribute | Aligns and distributes selected objects relative to a chosen reference. | T2 | 3 |
| AB-MASS-020 | Sections and clipping | Creates live section planes with saved states for presentation and analysis, with a section box for the whole model. | T1 | 3 |

## Groups, instances, and organization

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-MASS-021 | Make group | Converts a selection into a group and enters a nested edit context with a path indicator, greying everything outside the group. | T1 | 3 |
| AB-MASS-022 | Group instances | Places instances that share one definition, so editing the definition updates every instance. Per-instance transform and overrides stay local. | T1 | 3 |
| AB-MASS-023 | Make unique | Detaches an instance into a new definition with a name, leaving other instances unchanged. | T1 | 3 |
| AB-MASS-024 | Nested groups | Nests groups to any depth with the edit-context stack shown, and warns on circular references. | T1 | 3 |
| AB-MASS-025 | Layers | Assigns objects to layers that control visibility, and stores layer state in saved views. | T1 | 3 |
| AB-MASS-026 | Properties and metadata | Shows name, identifier, layer, material, area, and volume of a selection, and stores user-defined key-value data on masses and groups. The data carries into converted elements. | T1 | 3 |
| AB-MASS-027 | Lock and hide | Locks objects against editing, and hides or isolates selections temporarily or per saved view. | T1 | 3 |
| AB-MASS-028 | Selection filters | Selects by type (face, edge, body, group), by material, by layer, by program type, and by shared definition. | T1 | 3 |

## Mass forms and parametric masses

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-MASS-029 | Mass from profiles | Creates mass forms from closed or open profiles by extrusion, revolve, sweep, blend, and swept blend, with the profiles kept editable. | T1 | 3 |
| AB-MASS-030 | Void forms | Creates void masses that subtract from solid masses in the same group, with the void retained for later edits. | T1 | 3 |
| AB-MASS-031 | Direct face manipulation | Moves, rotates, and scales faces, edges, and vertices of a mass form with a gizmo and typed values, keeping the form valid. | T1 | 3 |
| AB-MASS-032 | Parametric mass families | Authors mass families with named parameters, reference lines, and constraints in the family editor, and places them as instances with type and instance parameters. | T2 | 3 |
| AB-MASS-033 | Divided surfaces | Divides a mass face into a grid by counts or distances and places pattern-based panels, with rectangle, triangle, rhombus, and custom patterns. | T2 | 4 |
| AB-MASS-034 | Pattern components | Authors panel components that attach to the points of a divided surface, and repeats them along paths and surfaces. | T2 | 4 |
| AB-MASS-035 | Graph-driven masses | Reads a node graph that outputs curves and solids and creates mass forms from it, retaining the link so that input changes update the forms. | T3 | 5 |

## Mass levels and floors

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-MASS-036 | Mass levels | Defines levels with elevations and floor-to-floor heights, inserts levels with constant or varied heights, and shows one level or a range per view. | T1 | 3 |
| AB-MASS-037 | Mass floors | Slices masses at levels to produce a floor boundary per mass and level with gross floor area, updating when the mass changes. | T1 | 3 |
| AB-MASS-038 | Mass floor schedules | Lists gross floor area per level, per mass, and in total, and writes the totals to the program table. | T1 | 3 |
| AB-MASS-039 | Level mapping | Maps mass levels to document levels so that conversion to building elements creates or reuses the matching levels. | T1 | 3 |
| AB-MASS-040 | Mass floor exclusions | Excludes regions such as cores and shafts from gross floor area through subtraction boundaries, with the exclusion kept per level. | T2 | 3 |

## Area, volume, and program analysis

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-MASS-041 | Mass quantities | Computes volume, surface area, and gross floor area per mass, per group, and in total, and updates on edit. | T1 | 3 |
| AB-MASS-042 | Program types | Defines program types with name, color, target gross area, and efficiency target, and assigns them to mass floor areas. | T1 | 3 |
| AB-MASS-043 | Program areas | Draws net-area boundaries inside mass floors, tagged with a program type, with area computed from the boundary. | T1 | 3 |
| AB-MASS-044 | Program comparison | Compares target and actual areas per program type and per level, with efficiency ratios and totals, in a table and a bar view. | T1 | 3 |
| AB-MASS-045 | Gross to net factors | Applies editable gross-to-net ratios to compute net area from gross when areas are not drawn. | T2 | 3 |
| AB-MASS-046 | Program color overlay | Colors mass floors and areas by program type in views with a legend, and exports the legend as drafting content. | T1 | 3 |
| AB-MASS-047 | Unit and count metrics | Computes unit counts and parking counts from area and unit-size rules stored on program types. | T3 | 5 |
| AB-MASS-048 | Scenario comparison | Stores several mass options as named scenarios and compares their metrics side by side, including area, volume, program fit, and study results. | T2 | 4 |

## Conversion to building elements

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-MASS-049 | Walls from mass faces | Creates walls on selected vertical mass faces with a chosen type and level mapping, associated with the source faces. | T1 | 3 |
| AB-MASS-050 | Floors and roofs from mass faces | Creates floors from mass floor boundaries and roofs from top faces, with type and slope assigned and the association kept. | T1 | 3 |
| AB-MASS-051 | Curtain systems from mass faces | Creates curtain systems on mass faces with grid divisions from the face geometry, associated with the source. | T2 | 3 |
| AB-MASS-052 | Association status | Flags converted elements whose source mass face changed and offers an update command that regenerates them while keeping manual edits where possible. | T1 | 3 |
| AB-MASS-053 | Conversion of whole masses | Converts an entire mass to walls, floors, and roofs in one command using a conversion set, with a preview and a report of faces left unconverted. | T2 | 3 |
| AB-MASS-054 | Program to rooms | Creates rooms from program areas with names, numbers, and program data, and keeps program targets for comparison. | T2 | 3 |
| AB-MASS-055 | Mass visibility after conversion | Hides masses once converted without deleting them, and retains the mass for later updates. | T1 | 3 |

## Solar and daylight studies on masses

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-MASS-056 | Sun and shadow study | Displays shadows for a location, date, and time range, with a single-moment view and an animated range, and saves each setting in a saved view. | T1 | 4 |
| AB-MASS-057 | Solar radiation analysis | Computes incident radiation per face over a chosen period from climate data and context shading, and shows it as a color map with a legend and limits. | T2 | 4 |
| AB-MASS-058 | Direct sun hours | Computes hours of direct sun per face or sample point over a period and shows them as a color map. | T2 | 4 |
| AB-MASS-059 | Facade daylight potential | Estimates daylight availability on facades from sky models and context, and shows it as a map for glazing decisions. | T3 | 4 |
| AB-MASS-060 | Study results management | Stores each study result with its parameters and source revision, shows stale results as outdated, and compares results between scenarios. | T2 | 4 |
| AB-MASS-061 | Glazing ratio exploration | Adjusts glazing ratio and shading by face or orientation as parameters and shows the effect on radiation and energy indicators. | T3 | 4 |

## Site context

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-MASS-062 | Location and north | Sets the project location by address or coordinates, with time zone and true north, and uses them in all studies. | T1 | 3 |
| AB-MASS-063 | Terrain context | Imports terrain from survey points, contour data, and open elevation formats as a context surface, with bounds and resolution limits stated at import. | T1 | 3 |
| AB-MASS-064 | Neighboring buildings | Imports surrounding building volumes from open geospatial data as context masses that are shaded differently and excluded from program totals. | T2 | 3 |
| AB-MASS-065 | Image and drawing underlays | Imports scaled images and drawings as underlays for tracing and checks scale against a known distance. | T1 | 3 |
| AB-MASS-066 | Site boundary and setbacks | Defines a site boundary with setback offsets, and checks masses against setbacks and height limits, reporting the exceeding faces. | T2 | 3 |
| AB-MASS-067 | Context conversion | Converts context terrain and buildings into editable elements when they become part of the design. | T3 | 4 |

## Generative layout studies

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-MASS-068 | Parametric massing generators | Generates mass options from a site, setbacks, height limits, and a program using a node graph, with inputs exposed as sliders. | T3 | 5 |
| AB-MASS-069 | Goal-based option search | Searches generated options against goals such as program fit, sun exposure, and area efficiency, and ranks results with the metrics visible. | T3 | 5 |
| AB-MASS-070 | Option results exploration | Browses generated options in a gallery with metric filters, and applies a chosen option as a scenario. | T3 | 5 |
| AB-MASS-071 | Space layout generation | Proposes room layouts inside a floor boundary from a room list with area targets and adjacency rules, and applies a chosen layout as rooms. | T3 | 5 |

## Import, export, and presentation

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-MASS-072 | Mass exchange | Imports and exports masses through DXF, DWG, OBJ, glTF, STL, STEP, and IFC with a loss report that names geometry that could not be kept as solid. | T1 | 3 |
| AB-MASS-073 | Material assignment | Applies materials to faces and instances, with optional thermal type for studies, and carries them into converted elements. | T1 | 3 |
| AB-MASS-074 | Saved views | Saves camera, section, layer, visibility, sun, and display style as named views that can be recalled and switched with a transition. | T1 | 3 |
| AB-MASS-075 | Visual styles | Displays masses shaded, wireframe, or hidden line, with shadows, edges, and background options. | T1 | 3 |
| AB-MASS-076 | Image export | Exports the current view at a chosen resolution to PNG and other raster formats, with annotation optional. | T1 | 3 |
