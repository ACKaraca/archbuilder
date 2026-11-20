# Capabilities

This index lists the capability areas of ArchBuilder and defines how capabilities are identified,
tiered and planned. Each area file specifies observable behavior that implementation and tests
follow.

## Legend

**Identifier.** `AB-<AREA>-<NNN>`, where `AB` is the ArchBuilder prefix, `AREA` is the area token
listed below and `NNN` numbers capabilities within an area from `001`. Identifiers are permanent: a
withdrawn capability keeps its number and is marked withdrawn, and a number is never reused.

**Tier.**

| Tier | Meaning |
|---|---|
| T1 Core | Required for first production use |
| T2 Professional | Required for parity with established professional tools |
| T3 Advanced | Specialized or differentiating |

**Phase.** The suite phase in which the capability is planned, as defined in the
suite roadmap in [`arc-suite`](https://github.com/ACKaraca/arc-suite): 2 building design
and collaboration, 3 engineering breadth, 4 visualization and content, 5 depth and scale. The
product milestones that group these phases are in [the roadmap](../../roadmap.md).

**Columns.** Every area file has the columns `ID | Capability | Behavior | Tier | Phase`. The
Behavior cell states inputs, result, what updates automatically and limits, precisely enough to
implement and test.

## Areas

| File | Area token | Scope | Rows |
|---|---|---|---|
| [01-project-setup-and-datum.md](01-project-setup-and-datum.md) | `DATUM` | Templates, spatial structure, units, levels, grids, reference planes, coordinates, modify and selection tools, model health | 34 |
| [02-walls-and-curtain-systems.md](02-walls-and-curtain-systems.md) | `WALL` | Layered, stacked and lining walls, joins, profiles, bands, curtain walls and systems | 27 |
| [03-doors-windows-and-openings.md](03-doors-windows-and-openings.md) | `OPEN` | Hosted inserts, opening recipes, openings, freestanding components | 18 |
| [04-floors-roofs-and-ceilings.md](04-floors-roofs-and-ceilings.md) | `SLAB` | Floors, roofs, roof edges, dormers, ceilings | 20 |
| [05-circulation.md](05-circulation.md) | `CIRC` | Stairs, ramps, railings, elevators, moving walks, headroom and travel paths | 16 |
| [06-rooms-areas-and-spaces.md](06-rooms-areas-and-spaces.md) | `SPACE` | Rooms, area schemes, space programs, color schemes, architectural zones | 15 |
| [07-site-and-context.md](07-site-and-context.md) | `SITE` | Terrain, pads, cut and fill, property lines, site components, GIS and civil context | 13 |
| [08-families-and-family-editor.md](08-families-and-family-editor.md) | `FAM` | Family kinds, family editor, geometry recipes, nesting, annotation families, libraries and versions | 33 |
| [09-parameters-and-constraints.md](09-parameters-and-constraints.md) | `PARAM` | Typed parameters, expression language, properties, numbering, classification, constraints | 23 |
| [10-views-and-graphics.md](10-views-and-graphics.md) | `VIEW` | View kinds, visibility and graphics, templates, graphic standards, materials, navigation | 26 |
| [11-annotation-detailing-and-drawings.md](11-annotation-detailing-and-drawings.md) | `ANNO` | Dimensions, text, tags, keynotes, detail layers, drafting views, detached drawings | 26 |
| [12-schedules-and-quantities.md](12-schedules-and-quantities.md) | `SCHED` | Schedules, quantity rules, material takeoff, carbon, cost, spreadsheet exchange | 16 |
| [13-sheets-and-publishing.md](13-sheets-and-publishing.md) | `SHEET` | Sheets, viewports, revisions, sheet sets, PDF, DWG, DXF and Hub publishing | 16 |
| [14-phases-options-groups-and-assemblies.md](14-phases-options-groups-and-assemblies.md) | `PHASE` | Phases, design options, model groups, assemblies, parts | 15 |
| [15-links-and-coordination.md](15-links-and-coordination.md) | `LINK` | Links, underlays, point clouds, monitored elements, interference checks, BCF | 14 |
| [16-collaboration.md](16-collaboration.md) | `COLLAB` | Hub versions, locks, comparison, packages, branches, multi-user editing | 16 |
| [17-visualization.md](17-visualization.md) | `VIS` | Viewport, sun and shadows, cameras, walkthroughs, rendering through RenderBuilder | 12 |
| [18-automation-and-extensibility.md](18-automation-and-extensibility.md) | `AUTO` | Commands, automation protocol, CLI, scripting, node graphs, plug-ins, model checking, assistant | 15 |
| [19-interoperability.md](19-interoperability.md) | `IO` | Native document, IFC, IDS, COBie, DWG and DXF, meshes, OpenUSD, STEP, exchange package, loss reports | 20 |
| [20-structural-modeling.md](20-structural-modeling.md) | `STRUCT` | Physical structural model: columns, framing, foundations, structural walls and floors | 65 |
| [21-reinforcement.md](21-reinforcement.md) | `REBAR` | Reinforcement modeling, schedules and detailing | 50 |
| [22-steel-detailing.md](22-steel-detailing.md) | `STEEL` | Steel parts, connections, numbering, shop drawings and fabrication output | 96 |
| [23-analytical-model.md](23-analytical-model.md) | `ANALYT` | Analytical members and panels, loads, boundary conditions, analysis exchange | 59 |
| [24-hvac.md](24-hvac.md) | `HVAC` | Service spaces and zones, loads, duct systems, sizing | 65 |
| [25-piping-and-plumbing.md](25-piping-and-plumbing.md) | `PIPE` | Pipe systems, plumbing fixtures, sizing, fire protection | 55 |
| [26-electrical.md](26-electrical.md) | `ELEC` | Circuits, panels, devices, cable containment, lighting | 67 |
| [27-fabrication-detailing.md](27-fabrication-detailing.md) | `FAB` | Fabrication parts for building services, hangers, fabrication output | 46 |
| [28-concept-massing.md](28-concept-massing.md) | `MASS` | Concept massing, mass floors, divided surfaces, early design | 76 |
| [29-building-performance-analysis.md](29-building-performance-analysis.md) | `PERF` | Solar, energy, daylight, wind and carbon analysis | 58 |
| [30-application-connectors.md](30-application-connectors.md) | `CONNECT` | Connectors for native project and family files of other applications, exchange package, round trips, change detection, licensed bridges, open readers | 20 |
| [31-web-and-tablet.md](31-web-and-tablet.md) | `TOUCH` | Browser and tablet builds, touch and pen massing, viewing and markup, on-site review, offline documents, Hub synchronization | 18 |
| [32-assistance.md](32-assistance.md) | `ASSIST` | Optional assistance: model questions, editing by intent, documentation proposals, rule drafting and findings, clash triage, layout and massing options, content and data drafting, existing buildings, review of assisted content | 34 |

## Capabilities and issues

Status is tracked in GitHub issues and the project board, never in these tables. An issue names the
capability identifiers it implements in its *Scope* section and carries the `area: <token>` label of
the area token above (lower case). An epic covers a range of identifiers in one area; a feature or
bug names individual identifiers. A pull request that changes specified behavior updates the
affected rows in the same pull request. A capability is complete when every issue that names it is
*Done* with verified acceptance criteria.
