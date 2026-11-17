# HVAC

This area specifies heating, ventilation, and air conditioning design: spaces and zones for services, heating and cooling loads, duct systems with fittings and accessories, air terminals, mechanical equipment, sizing and pressure-loss calculation, system inspection, insulation and lining, and schematic representation. Connectors on equipment and parts define the logical systems, and calculations read the same model that is drawn.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Service space | Name, number, occupancy type, volume, boundary conditions, design conditions, ventilation and load values | Occupies an architectural room or a free volume; belongs to a zone; receives air terminals; read by load calculation |
| Zone | Name, thermostat settings, served spaces, schedule | Groups service spaces; read by load calculation and system assignment |
| Duct segment | Shape (rectangular, round, oval), size, material, route, elevation, slope, system, insulation, lining | Connects through connectors to fittings, terminals, equipment, and other segments; belongs to a duct system |
| Duct type | Shape, material, roughness, routing rules, fitting selection, insulation and lining defaults | Referenced by segments; stores preferred fittings per joint kind |
| Duct fitting | Kind (elbow, tee, cross, transition, union, cap), dimensions, loss coefficient, connectors | Placed by routing or manually; belongs to a duct system |
| Duct accessory | Kind (damper, fire damper, silencer, filter, volume control, access door), connectors, loss data | Inserted in a duct segment |
| Air terminal | Kind (diffuser, grille, register), size, flow, throw and noise data, connector | Hosted on a ceiling, wall, or duct; assigned to a space; belongs to a system |
| Mechanical equipment | Kind (air handler, fan, boiler, chiller, heat pump, coil, unit), capacities, connectors, clearances | Has connectors for air, water, and power; belongs to systems |
| Connector | Domain, flow direction, system class, shape, size, flow configuration | Belongs to an element; links to another connector |
| Duct system | Class (supply, return, exhaust, outside air), name, source equipment, served terminals, design flow | Groups connected elements; read by sizing and inspection |
| Insulation or lining | Material, thickness, kind (external insulation, internal lining) | Attached to segments and fittings; counted in quantities |
| Calculation report | Method, inputs, results per element and system | Derived from a system or a space set |

## Spaces and zones

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-HVAC-001 | Service space creation | Creates a service space in each room of the architectural model, or in a picked closed volume, and keeps its boundary and height in step with the room. A space can also be unplaced and drawn as a free volume. | T1 | 4 |
| AB-HVAC-002 | Space data | Stores occupancy type, number of people, design temperature and humidity, ventilation rate, equipment and lighting gains, and infiltration on each space, defaulting from a space type table. | T1 | 4 |
| AB-HVAC-003 | Space boundaries | Derives space boundaries and boundary conditions (exterior, interior, ground, adiabatic) from the linked architectural model, and exposes each boundary with area, orientation, construction, and openings. | T1 | 4 |
| AB-HVAC-004 | Space separators | Subdivides or limits a space with separation lines for open areas and keeps the resulting areas and volumes consistent. | T2 | 4 |
| AB-HVAC-005 | Zones | Groups spaces into zones with a thermostat setting and schedule, and shows zone membership in the system browser and schedules. | T1 | 4 |
| AB-HVAC-006 | Space and zone schedules | Lists spaces and zones with area, volume, loads, airflow, and occupancy, and writes edited values back to the objects. | T1 | 4 |
| AB-HVAC-007 | Color schemes for spaces | Colors spaces in a view by a chosen parameter, such as zone, load, or airflow, with a legend. | T2 | 4 |
| AB-HVAC-008 | Space reconciliation | Reports spaces that no longer match their rooms in area, volume, or boundary, and offers an update command. | T2 | 4 |

## Heating and cooling loads

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-HVAC-009 | Load calculation | Computes peak heating and cooling loads per space and zone from boundary constructions, internal gains, ventilation, and design weather, using a documented calculation method. The method and its inputs are stored with the report. | T1 | 4 |
| AB-HVAC-010 | Calculation methods | Provides steady-state heat loss per a published European standard and peak cooling load per a published radiant-time or heat-balance method. Method parameters are open tables. | T2 | 4 |
| AB-HVAC-011 | Design weather data | Reads design weather values for the project location from open climate data files and allows manual design conditions. | T1 | 4 |
| AB-HVAC-012 | Construction thermal data | Reads thermal resistance and heat capacity of construction layers from material definitions and reports missing data per boundary. | T1 | 4 |
| AB-HVAC-013 | Load reports | Presents loads by space, zone, system, and building with components (transmission, ventilation, internal gains, solar), and exports to CSV and a printable report. | T1 | 4 |
| AB-HVAC-014 | Load results to design flows | Writes calculated supply airflow and heating and cooling demand to spaces and from there to air terminals and equipment selection data. | T1 | 4 |
| AB-HVAC-015 | Load calculation exchange | Exports the load model to gbXML and to the documented analysis exchange format, and imports results from external load programs by space identifier. | T2 | 4 |
| AB-HVAC-016 | Load recalculation tracking | Marks load results as outdated when constructions, spaces, or conditions change and lists the changed inputs. | T2 | 4 |

## Duct modeling

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-HVAC-017 | Duct drawing | Draws rectangular, round, and oval ducts by picking points with size, elevation, offset, and justification. Ducts connect to equipment and terminals and carry a system. | T1 | 4 |
| AB-HVAC-018 | Duct types and routing preferences | Defines duct types that select fittings for elbows, tees, crosses, transitions, and taps, with minimum lengths and angle rules. Changing the type regenerates fittings. | T1 | 4 |
| AB-HVAC-019 | Automatic fittings | Inserts fittings at changes of direction, size, and branching as ducts are drawn or edited, and keeps fitting dimensions and losses current. | T1 | 4 |
| AB-HVAC-020 | Placeholders | Draws duct and fitting placeholders for deferred routing detail, and converts them to real ducts and fittings on request. | T2 | 4 |
| AB-HVAC-021 | Flexible ducts | Draws flexible ducts as editable splines between two connectors with a maximum length and bend radius, and reports violations. | T1 | 4 |
| AB-HVAC-022 | Duct editing | Edits duct size, elevation, slope, and alignment with connected elements following, breaks ducts, aligns elbows, and swaps fitting types. | T1 | 4 |
| AB-HVAC-023 | Parallel duct runs | Draws parallel ducts with a spacing and generates matching fittings at bends. | T2 | 4 |
| AB-HVAC-024 | Duct conversions | Converts between duct shapes and between sizes through transitions, and restores fittings that remain connected. | T2 | 4 |
| AB-HVAC-025 | Duct shafts and penetrations | Generates openings in floors, walls, and slabs where ducts pass, with clearances, for architecture and structure to approve. | T2 | 4 |

## Fittings and accessories

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-HVAC-026 | Duct fitting families | Provides fitting families with connectors, dimensions, and loss coefficient tables, authored in the family editor. | T1 | 4 |
| AB-HVAC-027 | Duct accessories | Inserts dampers, fire dampers, silencers, filters, volume controls, and access doors in a duct, splitting the segment and carrying pressure loss data. | T1 | 4 |
| AB-HVAC-028 | Fitting loss data | Stores loss coefficients as constants, tables, or formulas per fitting kind and flow ratio, and uses them in pressure calculations. | T1 | 4 |
| AB-HVAC-029 | Fire damper rules | Places fire dampers automatically where ducts cross fire-rated walls and floors, and reports crossings without a damper. | T3 | 5 |

## Air terminals and equipment

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-HVAC-030 | Air terminal placement | Places air terminals on ceilings, walls, and ducts, assigns them to a space, and sets design flow. The terminal inherits flow from the space airflow distribution. | T1 | 4 |
| AB-HVAC-031 | Terminal layout | Distributes terminals in a space on a grid or along a line by count or spacing, aligned to a ceiling pattern. | T2 | 4 |
| AB-HVAC-032 | Terminal performance data | Reads throw, pressure drop, and sound data per size and flow from terminal families and reports space noise and throw coverage. | T3 | 5 |
| AB-HVAC-033 | Mechanical equipment | Places equipment families with air, water, and power connectors, clearance zones, and capacity data, and lists them in equipment schedules. | T1 | 4 |
| AB-HVAC-034 | Equipment clearances | Shows required service and clearance volumes around equipment and reports clashes with other elements. | T2 | 4 |
| AB-HVAC-035 | Equipment selection data | Stores selection parameters (airflow, pressure, heating and cooling capacity) on equipment and compares them to calculated demand. | T2 | 4 |

## Connectors and systems

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-HVAC-036 | Connectors | Defines connectors by domain, flow direction, system class, shape, size, and flow configuration. Connectors snap together, check compatibility, and report why a connection is refused. | T1 | 4 |
| AB-HVAC-037 | Duct systems | Creates systems of class supply, return, exhaust, or outside air by selecting terminals and equipment, naming the system and assigning a source. Elements are added automatically as ducts connect. | T1 | 4 |
| AB-HVAC-038 | System membership | Adds and removes elements from a system, merges and splits systems, and reports elements that are connected but not assigned. | T1 | 4 |
| AB-HVAC-039 | Disconnected element report | Lists elements with open connectors, unassigned systems, or flow direction conflicts and zooms to each. | T1 | 4 |
| AB-HVAC-040 | System colors and overrides | Colors elements in views by system through rules, and shows system class and name in tags. | T1 | 4 |

## Sizing and pressure loss

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-HVAC-041 | Duct sizing methods | Sizes ducts by velocity, equal friction, or static regain with a limit on velocity, friction rate, and aspect ratio, and applies the sizes to selected branches. Sizing can preview changes before applying. | T1 | 4 |
| AB-HVAC-042 | Pressure loss calculation | Computes pressure loss for each duct segment, fitting, and accessory along the paths of a system and reports the critical path with total loss. | T1 | 4 |
| AB-HVAC-043 | Flow propagation | Accumulates design airflow from terminals through the network to equipment and reports flow at each segment, with checks for imbalance. | T1 | 4 |
| AB-HVAC-044 | Balance report | Reports pressure difference between branches and the dampers needed for balancing at design flow. | T2 | 4 |
| AB-HVAC-045 | Velocity and noise limits | Checks duct velocity against limits per space type and reports segments that exceed limits. | T2 | 4 |
| AB-HVAC-046 | Fan duty | Computes fan duty (flow and total pressure) from the critical path and compares it with equipment selection data. | T2 | 4 |
| AB-HVAC-047 | System sizing report | Presents sizing and loss results as a table and a graphical overlay on the model, and exports them to CSV. | T1 | 4 |

## System browser and inspection

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-HVAC-048 | System browser | Shows systems as a tree of equipment, branches, and terminals with flow, size, and warnings, and selects or isolates elements in the viewport. | T1 | 4 |
| AB-HVAC-049 | System inspection | Shows flow and direction through a selected system with arrows, values on segments, and the path from a chosen terminal to the source. | T1 | 4 |
| AB-HVAC-050 | System checks | Checks connectivity, flow direction, size continuity, and missing equipment for each system, and lists findings with fix commands. Checks run headless. | T1 | 4 |
| AB-HVAC-051 | System isolation | Isolates one or more systems in a view with other systems dimmed, and saves the isolation as a view state. | T2 | 4 |

## Insulation and lining

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-HVAC-052 | Duct insulation | Adds external insulation of type and thickness to ducts and fittings by selection, by system, or by default on the duct type. Insulation is counted in quantities and drawn in views. | T1 | 4 |
| AB-HVAC-053 | Duct lining | Adds internal lining with type and thickness, reduces the net free area for sizing, and counts lining area. | T2 | 4 |
| AB-HVAC-054 | Insulation editing | Changes, hides, or removes insulation on selected elements and lists elements whose insulation is missing against the default. | T2 | 4 |
| AB-HVAC-055 | Insulation schedules | Totals insulation area and volume by system, type, and thickness. | T1 | 4 |

## Schematic and documentation

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-HVAC-056 | Single-line representation | Draws ducts as single lines with symbols at coarse detail and as double lines at fine detail, per view, without altering the model. | T1 | 4 |
| AB-HVAC-057 | Schematic diagrams | Generates a schematic of a system from model connectivity, with symbols, flow values, and sizes, laid out on a sheet and updated on demand. | T3 | 5 |
| AB-HVAC-058 | Duct tags | Tags ducts with size, elevation, flow, and system. Tags update when the duct changes and show rise and drop symbols for vertical runs. | T1 | 4 |
| AB-HVAC-059 | Line gaps at crossings | Draws gaps in lines where a duct passes over another service in plan, with the gap size set per view. | T2 | 4 |
| AB-HVAC-060 | Equipment and duct schedules | Lists equipment, terminals, accessories, and duct quantities with length, area, and size groups, and writes edits to the elements. | T1 | 4 |

## Automated layout

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-HVAC-061 | Layout proposals | Proposes duct routes between selected equipment and terminals with several path patterns, shows each as a preview with length and estimated pressure loss, and applies the chosen one as ducts and fittings. | T3 | 5 |
| AB-HVAC-062 | Layout constraints | Respects zones that must stay clear, clearance to structure, minimum heights, and preferred corridors when proposing routes. | T3 | 5 |
| AB-HVAC-063 | Graph-driven routing | Reads a node graph that outputs route curves and sizes and creates ducts from them, retaining the link to the graph. | T3 | 5 |

## Exchange and content

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-HVAC-064 | HVAC IFC mapping | Maps ducts, fittings, accessories, terminals, equipment, spaces, zones, and systems to IFC distribution entities and ports, with property sets and system membership, and reports unmapped data. | T1 | 4 |
| AB-HVAC-065 | HVAC content authoring | Authors fitting, terminal, and equipment families with connectors, parameters, and lookup tables in the family editor, and tests them by flexing parameters. | T1 | 4 |
