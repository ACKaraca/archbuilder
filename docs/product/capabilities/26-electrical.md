# Electrical

This area specifies electrical design: equipment and devices, circuits, panels and panel schedules, load classification and demand factors, wire sizing, cable tray and conduit, lighting fixtures and lighting calculation, single-line diagrams, and low-voltage and data systems. Circuits and loads are model data, so schedules, calculations, and diagrams read the same objects that are drawn.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Distribution system | Name, voltage, phases, wires, frequency | Assigned to panels, transformers, and circuits; read by wire sizing |
| Electrical equipment | Kind (panel, switchboard, transformer, switchgear, generator, UPS), rating, distribution systems, connectors | Feeds circuits and downstream equipment; has a schedule |
| Electrical device | Kind (socket, switch, sensor, junction box, data outlet), load, connector, mounting height | Hosted on walls, ceilings, or floors; belongs to circuits |
| Lighting fixture | Photometric data, lamp wattage, mounting, light loss factor, connector | Hosted on ceilings or walls; belongs to lighting circuits and switch systems; read by lighting calculation |
| Circuit | Class (power, lighting, switch, data, communication, fire alarm), panel, circuit number, connected devices, load, voltage, wire and protective device | Belongs to a panel; owns a path; read by panel schedules and wire sizing |
| Circuit path | Route curve, wire type, conduit or tray assignment, length | Belongs to a circuit; physical or schematic |
| Panel schedule | Template, panel, circuits, spares, spaces, load summary | Derived from circuits; one per panel |
| Load classification | Name, demand factor rule, connected and demand totals | Assigned to loads; read by load summaries |
| Demand factor rule | Kind (constant, by load range, by count), thresholds, factors | Applied to a load classification |
| Wire type | Material, insulation, temperature rating, size table, ampacity data | Referenced by circuits and conductors |
| Cable tray | Width, height, material, route, fill | Connects through fittings; carries circuits; belongs to a containment run |
| Conduit | Diameter, material, route, fill | Connects through fittings; carries circuits; belongs to a containment run |
| Containment fitting | Kind (bend, tee, cross, reducer, riser), dimensions, connectors | Joins containment segments |
| Single-line diagram | Symbols, connections, ratings, layout | Derived from equipment and circuits; placed on a sheet |
| Low-voltage system | Class (fire alarm, data, security, nurse call, telephone), devices, cable paths | Groups devices and cabling by function |

## Settings and distribution

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ELEC-001 | Electrical settings | Stores document-level electrical settings: wiring display, circuit numbering rules, voltage tolerance, demand factor defaults, and calculation standard. A change revalidates affected circuits. | T1 | 4 |
| AB-ELEC-002 | Distribution systems | Defines distribution systems by voltage, phases, and wires, assigns them to equipment, and restricts circuits to compatible systems. | T1 | 4 |
| AB-ELEC-003 | Voltage definitions | Defines line-to-line and line-to-neutral voltages with a tolerance range, and maps device voltage ratings to systems. | T1 | 4 |
| AB-ELEC-004 | Wire types and size tables | Defines wire types by material, insulation, temperature rating, size table, and ampacity table in open data files. | T1 | 4 |
| AB-ELEC-005 | Calculation standards | Selects a published wiring standard whose tables and rules drive ampacity, derating, and voltage drop. Standards are open tables, extensible by users. | T2 | 4 |

## Equipment and devices

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ELEC-006 | Electrical equipment placement | Places panels, switchboards, transformers, switchgear, generators, and UPS units with ratings, distribution systems, connectors, and clearance zones. | T1 | 4 |
| AB-ELEC-007 | Equipment hierarchy | Defines supply relationships between equipment (transformer to switchboard to panel) and shows the feed in schedules and diagrams. | T1 | 4 |
| AB-ELEC-008 | Device placement | Places sockets, switches, sensors, and junction boxes on walls, floors, and ceilings by face, with mounting height and orientation, and carries load and voltage data. | T1 | 4 |
| AB-ELEC-009 | Device layout by rule | Places devices along walls or in rooms by spacing and count rules, aligned to room edges and openings. | T2 | 4 |
| AB-ELEC-010 | Electrical connectors | Defines connectors by system type, voltage, phases, poles, and load, which determine circuit compatibility and show the reason for refused connections. | T1 | 4 |
| AB-ELEC-011 | Equipment clearances | Shows working clearance volumes around electrical equipment per standard and reports clashes with other elements. | T2 | 4 |
| AB-ELEC-012 | Electrical content authoring | Authors device, fixture, and equipment families with connectors, load parameters, and symbolic representation. | T1 | 4 |

## Circuits

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ELEC-013 | Circuit creation | Creates a circuit from selected devices, fixtures, and equipment with a class and a panel. The command checks voltage, phases, and poles, and reports any incompatible element. | T1 | 4 |
| AB-ELEC-014 | Circuit assignment | Assigns circuits to panels and slots, moves circuits between panels, and shows the effect on load balance. | T1 | 4 |
| AB-ELEC-015 | Circuit naming and numbering | Names circuits by panel and slot with configurable formats and numbers new circuits in the next free slot, keeping multi-pole circuits aligned. | T1 | 4 |
| AB-ELEC-016 | Circuit path editing | Edits the path of a circuit as a route between its devices and panel, as automatic shortest route, as a manual path, or as schematic wiring. Length updates the voltage drop. | T1 | 4 |
| AB-ELEC-017 | Schematic wiring | Draws wires between devices as arcs, chamfers, or splines with tick marks for conductors, and keeps the wires attached to devices. | T2 | 4 |
| AB-ELEC-018 | Circuit protective devices | Assigns breakers or fuses with rating and trip type to circuits, and checks the rating against load and wire ampacity. | T1 | 4 |
| AB-ELEC-019 | Circuit load summary | Totals connected and demand load per circuit and shows power factor and current. | T1 | 4 |
| AB-ELEC-020 | Circuit checks | Reports overloaded circuits, mixed voltage elements, elements on no circuit, and circuits with no panel. Checks run headless. | T1 | 4 |
| AB-ELEC-021 | Spare and space slots | Reserves spare and space slots in panels and keeps them through renumbering. | T1 | 4 |
| AB-ELEC-022 | Switch systems | Links switches to lighting fixtures with switch legs and shows the control relation in views and schedules. | T2 | 4 |

## Panels and panel schedules

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ELEC-023 | Panel schedule templates | Defines schedule templates for branch panels, switchboards, and data panels with column sets, slot layout, and summary sections. | T1 | 4 |
| AB-ELEC-024 | Panel schedules | Generates one schedule per panel listing circuits by slot with load, protective device, wire, and description. Edits to circuit data in the schedule write to the circuits. | T1 | 4 |
| AB-ELEC-025 | Panel load summary | Totals connected and demand load per phase and per load classification, and states the balance between phases. | T1 | 4 |
| AB-ELEC-026 | Switchboard schedules | Generates schedules for switchboards with feeders, ratings, and outgoing ways. | T2 | 4 |
| AB-ELEC-027 | Panel schedule on sheets | Places panel schedules on sheets, one panel per sheet or several per sheet, and updates them on circuit changes. | T1 | 4 |
| AB-ELEC-028 | Phase balancing | Suggests circuit moves between phases to reduce imbalance, and applies the chosen moves as one transaction. | T3 | 5 |
| AB-ELEC-029 | Equipment schedules | Lists electrical equipment and devices with rating, location, and feed, and writes edits to the elements. | T1 | 4 |

## Loads and demand

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ELEC-030 | Load classifications | Defines load classifications such as lighting, receptacle, motor, HVAC, and kitchen, and assigns them to devices and circuits. | T1 | 4 |
| AB-ELEC-031 | Demand factors | Applies demand factors per classification by constant, by load range, or by count, and computes demand load per panel and per system. | T1 | 4 |
| AB-ELEC-032 | Connected and demand load | Reports connected, demand, and estimated load at each level of the distribution hierarchy, from device to incoming supply. | T1 | 4 |
| AB-ELEC-033 | Loads from mechanical equipment | Reads electrical load from mechanical and plumbing equipment connectors and adds them to circuits. | T2 | 4 |
| AB-ELEC-034 | Load growth and spare capacity | Applies growth factors and reports spare capacity per panel and per transformer. | T2 | 4 |
| AB-ELEC-035 | Short-circuit estimate | Estimates prospective fault current at panels from supply data, transformer impedance, and cable lengths, and compares it with equipment ratings. | T3 | 5 |

## Wire sizing

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ELEC-036 | Wire sizing by ampacity | Sizes conductors from circuit current, protective device, installation method, and derating factors, and applies results to circuits. | T1 | 4 |
| AB-ELEC-037 | Voltage drop calculation | Computes voltage drop along each circuit path from length, wire size, and load, and flags circuits above the limit. | T1 | 4 |
| AB-ELEC-038 | Grouping and derating | Applies grouping, ambient temperature, and installation correction factors from the standard tables, with each factor recorded in the report. | T2 | 4 |
| AB-ELEC-039 | Protective conductor sizing | Sizes protective conductors from fault level and protective device characteristics. | T3 | 5 |
| AB-ELEC-040 | Wire size report | Presents results as a table and overlay with export to CSV and a printable report, and marks results as outdated when inputs change. | T1 | 4 |

## Cable tray and conduit

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ELEC-041 | Cable tray runs | Draws cable tray with width, height, elevation, and justification and generates fittings at bends, tees, crosses, and reducers from routing preferences. | T1 | 4 |
| AB-ELEC-042 | Conduit runs | Draws conduit with diameter and elevation and generates fittings at bends, couplings, and junctions, including parallel conduit groups with a spacing. | T1 | 4 |
| AB-ELEC-043 | Containment types and routing preferences | Defines tray and conduit types with materials, sizes, and preferred fittings, and regenerates fittings when a rule changes. | T1 | 4 |
| AB-ELEC-044 | Containment placeholders | Draws placeholder runs for deferred detailing and converts them to real containment with fittings. | T2 | 4 |
| AB-ELEC-045 | Containment fill | Computes the fill of a tray or conduit from the cables it carries and reports runs above the limit. | T2 | 4 |
| AB-ELEC-046 | Circuit assignment to containment | Assigns circuits to tray and conduit runs and derives circuit path length from the route. | T2 | 4 |
| AB-ELEC-047 | Containment sizing | Proposes tray width or conduit diameter from assigned cables and fill limits, with a preview before applying. | T2 | 4 |
| AB-ELEC-048 | Containment penetrations | Generates openings where containment passes through floors and walls, with fire-stopping data, and lists them for approval. | T2 | 4 |
| AB-ELEC-049 | Containment annotations | Tags runs with size, elevation, and circuits carried, and shows rise and drop symbols. | T1 | 4 |

## Lighting

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ELEC-050 | Lighting fixture placement | Places fixtures on ceilings, walls, and faces with photometric data, lamp output, wattage, and mounting height, connected to lighting circuits. | T1 | 4 |
| AB-ELEC-051 | Photometric data import | Reads fixture photometric data from IES and EULUMDAT files and shows the distribution, flux, and efficacy on the fixture. | T1 | 4 |
| AB-ELEC-052 | Fixture layout | Lays out fixtures in a room by grid, count, or target illuminance, aligned to ceiling patterns and obstructions. | T2 | 4 |
| AB-ELEC-053 | Illuminance calculation | Computes average and point illuminance on a work plane in each space from fixtures, room reflectances, and a maintenance factor, by a lumen method and a point-by-point method. | T1 | 4 |
| AB-ELEC-054 | Lighting results | Shows illuminance as a color grid and values on the work plane, with an evenness ratio and a comparison to the target for the space type. | T1 | 4 |
| AB-ELEC-055 | Lighting power density | Computes installed lighting power per area per space and compares it with the allowance for the space type. | T2 | 4 |
| AB-ELEC-056 | Emergency lighting | Marks emergency fixtures and checks escape route illuminance from a route definition. | T3 | 5 |
| AB-ELEC-057 | Lighting calculation exchange | Exports the lighting model to open photometric exchange formats and imports results from external lighting programs by space identifier. | T3 | 5 |

## Single-line diagrams

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ELEC-058 | Single-line generation | Generates a single-line diagram from the distribution hierarchy, with symbols, ratings, feeder sizes, and loads, laid out on a sheet. The diagram is derived from model data and not drawn by hand. | T2 | 4 |
| AB-ELEC-059 | Diagram update | Updates a generated diagram when equipment, circuits, or ratings change, and preserves manual layout positions and annotations. | T2 | 4 |
| AB-ELEC-060 | Diagram symbols | Provides a symbol library for protection, switching, transformation, and metering, editable as drafting content. | T2 | 4 |
| AB-ELEC-061 | Diagram checks | Reports equipment missing a supply, feeders below the protective device rating, and loops in the hierarchy. | T2 | 4 |

## Low-voltage and data systems

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ELEC-062 | Low-voltage device systems | Defines systems for fire alarm, data, security, nurse call, and telephone with devices, cabling, and per-system schedules. | T1 | 4 |
| AB-ELEC-063 | Data circuits and cabling | Creates data circuits between outlets and patch panels with cable type, length, and labeling, and lists cables in schedules. | T2 | 4 |
| AB-ELEC-064 | Fire alarm layout | Places detectors and sounders with coverage radius, checks spacing against a configurable rule table, and groups devices into loops. | T3 | 5 |
| AB-ELEC-065 | Rack and cabinet layout | Models racks with unit-based equipment layout, capacity, and power and cooling load totals. | T3 | 5 |
| AB-ELEC-066 | Low-voltage schedules | Lists devices, loops, and cables with counts, lengths, and addresses, and writes edits to the elements. | T2 | 4 |

## Exchange

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ELEC-067 | Electrical IFC mapping | Maps equipment, devices, fixtures, containment, circuits, and systems to IFC electrical entities and ports with property sets, and reports unmapped data. | T1 | 4 |
