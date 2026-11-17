# Piping and plumbing

This area specifies piped systems: hydronic, domestic water, sanitary and storm drainage, and fire protection. It covers pipe systems, pipe types and routing preferences, fittings and accessories, slopes, plumbing fixtures, sizing and flow calculation, sprinkler layout, placeholders, and layout generation. Connectors define the logical systems, and calculations read the modeled network.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Pipe segment | Material, nominal and inside diameter, route, elevation, slope, system, insulation | Connects through connectors to fittings, accessories, fixtures, equipment, and segments; belongs to a pipe system |
| Pipe type | Material, size table, roughness, joint kind, routing rules, preferred fittings | Referenced by segments; defines fitting choices per joint kind |
| Pipe fitting | Kind (elbow, tee, wye, cross, transition, union, cap), dimensions, loss coefficient, connectors | Placed automatically or by hand; belongs to a pipe system |
| Pipe accessory | Kind (valve, strainer, meter, backflow preventer, trap, cleanout, vent), connectors, loss data | Inserted in a segment |
| Plumbing fixture | Kind, connectors for supply and drain, fixture units, flow rate, supply pressure demand | Placed in a space; connects to supply and drain networks |
| Pipe system | Class (hydronic supply and return, domestic cold and hot water, sanitary, storm, vent, fire protection, other), name, source, served elements | Groups connected elements; read by sizing and inspection |
| Sprinkler | Coverage area, K-factor, flow and pressure demand, orientation | Hosted on ceiling or pipe; belongs to a fire protection system and a hazard area |
| Hazard area | Hazard class, design density, design area | Groups sprinklers for demand calculation |
| Slope rule | Minimum fall, direction, applicability by pipe size and system class | Applied to drainage segments |
| Pipe placeholder | Route, nominal size, system | Stands for deferred routing; converts to pipes and fittings |
| Calculation report | Method, inputs, results per segment and system | Derived from a system and its design flows |

## Pipe types and settings

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PIPE-001 | Pipe types and size tables | Defines pipe types with material, size table (nominal size, inside and outside diameter, wall thickness), roughness, and joint kind. Size tables are open data per standard and material. | T1 | 4 |
| AB-PIPE-002 | Routing preferences | Specifies preferred fittings for elbows, tees, wyes, crosses, transitions, and taps per pipe type and size range, with angle and minimum length rules. Changing a rule regenerates fittings. | T1 | 4 |
| AB-PIPE-003 | System class settings | Defines pipe system classes with graphics, abbreviation, flow direction, and default pipe type, including hydronic, domestic water, sanitary, storm, vent, and fire protection. | T1 | 4 |
| AB-PIPE-004 | Fluid properties | Stores fluid type, temperature, density, viscosity, and glycol percentage for each system class, used in flow and pressure calculation. | T1 | 4 |
| AB-PIPE-005 | Design settings | Stores design velocity limits, friction limits, safety factors, and diversity values per system class and space type. | T2 | 4 |

## Pipe routing

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PIPE-006 | Pipe drawing | Draws pipes by picking points with size, elevation, offset, and justification, connecting to equipment, fixtures, and other pipes. Fittings are placed as the pipe is drawn. | T1 | 4 |
| AB-PIPE-007 | Pipe editing | Edits size, elevation, slope, and alignment, with connected elements following. The command breaks pipes, aligns elbows, and swaps fittings by type. | T1 | 4 |
| AB-PIPE-008 | Parallel pipes | Draws parallel pipe runs with spacing and generates matching fittings at bends and offsets. | T2 | 4 |
| AB-PIPE-009 | Flexible pipes | Draws flexible pipes between two connectors with minimum bend radius and maximum length, and reports violations. | T2 | 4 |
| AB-PIPE-010 | Vertical risers | Creates risers across levels with offsets and fittings, and keeps them aligned when a level moves. | T1 | 4 |
| AB-PIPE-011 | Offsets and elevation changes | Creates offsets, drops, and rises from a pick with an angle and a distance, with fittings chosen from routing preferences. | T1 | 4 |
| AB-PIPE-012 | Pipe penetrations | Generates openings and sleeves where pipes pass through floors, walls, and slabs, with clearances, and lists them for architecture and structure to approve. | T2 | 4 |
| AB-PIPE-013 | Pipe conversions | Converts pipe types in selected runs and regenerates fittings and sizes from the new type, reporting joints that have no matching fitting. | T2 | 4 |

## Fittings and accessories

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PIPE-014 | Pipe fitting families | Provides fittings with connectors, dimensions, and loss coefficient tables as loadable families. | T1 | 4 |
| AB-PIPE-015 | Pipe accessories | Inserts valves, strainers, meters, backflow preventers, traps, and cleanouts into a pipe, splitting the segment, and carries loss and flow coefficient data. | T1 | 4 |
| AB-PIPE-016 | Fitting loss data | Stores loss coefficients as constants, tables, or equivalent lengths per fitting kind and uses them in pressure calculations. | T1 | 4 |
| AB-PIPE-017 | Fitting reset and rotation | Rotates fittings, resets a fitting to its routing preference, and reports fittings that no longer match a connected pipe. | T2 | 4 |

## Slopes

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PIPE-018 | Pipe slope | Sets slope on a pipe as a ratio, angle, or percentage and a direction up or down, with connected elements following. | T1 | 4 |
| AB-PIPE-019 | Drainage fall rules | Applies minimum fall by pipe size and system class, and reports pipes below the minimum with the shortfall. | T1 | 4 |
| AB-PIPE-020 | Slope propagation | Applies a slope to a run from a start invert to an end invert and recomputes intermediate elevations of every segment and fitting. | T1 | 4 |
| AB-PIPE-021 | Invert elevations | Reports and edits invert elevations at the ends of pipes and annotates them in views and sections. | T2 | 4 |

## Plumbing fixtures and equipment

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PIPE-022 | Fixture placement | Places plumbing fixtures on floors and walls with supply and drain connectors, in a space, with fixture units and flow rate from the family. | T1 | 4 |
| AB-PIPE-023 | Fixture unit tables | Stores fixture unit values per fixture kind and standard for water supply and drainage, and sums them by branch. Tables are open data. | T1 | 4 |
| AB-PIPE-024 | Fixture counts by occupancy | Computes required fixture counts from occupancy and a standard table and compares them with placed fixtures. | T3 | 5 |
| AB-PIPE-025 | Plumbing equipment | Places pumps, tanks, heaters, boilers, and heat exchangers with connectors, capacities, and clearance zones. | T1 | 4 |
| AB-PIPE-026 | Equipment connections | Connects fixtures and equipment to a system by selecting the connector and a target pipe, creating the branch and fittings. | T1 | 4 |

## Pipe systems

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PIPE-027 | System creation | Creates a pipe system of a chosen class from selected fixtures, equipment, and pipes, names it, and assigns a source. Elements are added to the system as pipes connect to them. | T1 | 4 |
| AB-PIPE-028 | System membership | Adds and removes elements, merges and splits systems, and lists connected elements with no system. | T1 | 4 |
| AB-PIPE-029 | System browser | Shows systems as a tree of equipment, branches, and endpoints with flow, size, and warnings, and selects or isolates elements in the viewport. | T1 | 4 |
| AB-PIPE-030 | System checks | Checks connectivity, flow direction, size continuity, slope conflicts, and missing connections, listing findings with fix commands. Checks run headless. | T1 | 4 |
| AB-PIPE-031 | System colors | Colors pipes by system through rules and shows class and name in tags. | T1 | 4 |

## Sizing and flow calculation

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PIPE-032 | Supply sizing by flow | Sizes supply and hydronic pipes from design flow by velocity and friction limits and applies the sizes to selected branches, with a preview before applying. | T1 | 4 |
| AB-PIPE-033 | Design flow from fixture units | Converts accumulated fixture units to design flow using a demand curve from the selected standard, and applies it to each supply segment. | T1 | 4 |
| AB-PIPE-034 | Pressure loss calculation | Computes friction loss per segment from fluid properties, diameter, and roughness, and adds fitting and accessory losses along each path. The critical path and total loss are reported. | T1 | 4 |
| AB-PIPE-035 | Drainage sizing | Sizes drain and vent pipes from accumulated drainage fixture units, slope, and stack rules from the selected standard, and reports undersized segments. | T1 | 4 |
| AB-PIPE-036 | Stormwater sizing | Sizes storm pipes from catchment area, rainfall intensity, and slope using a documented method, and reports capacity per segment. | T2 | 4 |
| AB-PIPE-037 | Hydronic balance | Computes pressure differences between branches and the setting of balancing valves at design flow, and reports the pump duty. | T2 | 4 |
| AB-PIPE-038 | Pump duty | Computes pump flow and head from the critical path and compares them with equipment selection data. | T2 | 4 |
| AB-PIPE-039 | Pipe sizing report | Presents sizing and loss results in a table and an overlay on the model, with export to CSV and a printable report. | T1 | 4 |
| AB-PIPE-040 | Domestic hot water circulation | Computes heat loss and circulation flow for hot water loops from insulation and temperatures, and checks return temperature at the farthest fixture. | T3 | 5 |

## Fire protection

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PIPE-041 | Sprinkler placement | Places sprinklers on ceilings and pipes with orientation, K-factor, and coverage area, assigned to a hazard area. | T1 | 4 |
| AB-PIPE-042 | Sprinkler layout | Distributes sprinklers in a space by spacing and distance-to-wall rules from the hazard class and reports coverage gaps. | T2 | 4 |
| AB-PIPE-043 | Hazard areas | Defines hazard areas with class, design density, and design area, and groups the sprinklers they contain. | T2 | 4 |
| AB-PIPE-044 | Hydraulic calculation | Computes flow and pressure at each sprinkler and pipe along the demand path using a friction formula and fitting equivalent lengths, and reports the demand at the source. | T3 | 5 |
| AB-PIPE-045 | Fire protection pipe sizing | Sizes fire protection pipes from schedules or hydraulic calculation, with velocity limits, and reports undersized segments. | T3 | 5 |
| AB-PIPE-046 | Standpipes and hydrants | Places standpipes, hose connections, and hydrants with flow demand and connects them to a fire protection system. | T3 | 5 |

## Placeholders and layout generation

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PIPE-047 | Pipe placeholders | Draws placeholder runs of nominal size for deferred detailing, which convert to pipes and fittings on request. A placeholder carries its system and counts in quantities as a run. | T2 | 4 |
| AB-PIPE-048 | Layout proposals | Proposes pipe routes between selected equipment and endpoints with several path patterns, shows previews with length and estimated loss, and applies the chosen route as pipes and fittings. | T3 | 5 |
| AB-PIPE-049 | Layout constraints | Respects clearance to structure, minimum heights, preferred corridors, and zones to avoid when proposing routes. | T3 | 5 |
| AB-PIPE-050 | Graph-driven pipework | Reads a node graph that outputs route curves and sizes and creates pipes from them, retaining the link to the graph. | T3 | 5 |

## Insulation, documentation, and exchange

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PIPE-051 | Pipe insulation | Adds insulation of type and thickness to pipes and fittings by selection, system, or default on the pipe type, counts it in quantities, and draws it in views. | T1 | 4 |
| AB-PIPE-052 | Pipe tags and symbols | Tags pipes with size, material, slope, and system, shows rise and drop symbols, and places flow arrows. Tags update with the pipe. | T1 | 4 |
| AB-PIPE-053 | Pipe schedules | Lists pipes, fittings, accessories, fixtures, and equipment with length, size, and count groups, and writes edits to the elements. | T1 | 4 |
| AB-PIPE-054 | Piping IFC mapping | Maps pipes, fittings, accessories, fixtures, equipment, sprinklers, and systems to IFC distribution entities and ports with property sets, and reports unmapped data. | T1 | 4 |
| AB-PIPE-055 | Piping content authoring | Authors fitting, accessory, fixture, and equipment families with connectors, flow data, and lookup tables in the family editor. | T1 | 4 |
