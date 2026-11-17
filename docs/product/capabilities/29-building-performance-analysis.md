# Building performance analysis

This area specifies performance analysis of building models: generation of energy models from masses, rooms, and spaces, thermal properties, energy and carbon analysis, daylighting, wind and microclimate studies, system analysis reports, exchange with analysis formats, and results visualization. Analysis runs locally on the user's machine by default through isolated open engines, and every result records the model revision and the inputs it was computed from.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Analysis model | Source (mass, rooms, spaces), thermal zones, surfaces, openings, shading, ground plane, location | Derived from the building model; read by energy and daylight analyses; exchanged in gbXML |
| Analytical surface | Kind (wall, roof, floor, window, door, shade), area, orientation, tilt, construction, boundary condition | Belongs to an analysis model; linked to a building element or mass face |
| Thermal zone | Name, spaces, schedule set, setpoints, ventilation, internal gains | Groups service spaces; belongs to an analysis model |
| Thermal construction | Layers with conductivity, density, specific heat, thickness; computed U-value and heat capacity | Referenced by building element types and analytical surfaces |
| Glazing definition | U-value, solar heat gain coefficient, visible transmittance, frame data | Referenced by window types and analytical surfaces |
| Schedule set | Occupancy, lighting, equipment, and setpoint profiles by day type | Assigned to zones and space types |
| Weather data | Location, hourly dry-bulb, humidity, radiation, wind, and sky data | Read by energy, solar, and daylight analyses |
| Energy analysis run | Engine, settings, weather, model revision, results | Produces energy results; stored with inputs |
| Carbon assessment | Boundary (life cycle modules), material coefficients, quantities, operational energy, results | Reads quantities and energy results; produces carbon results |
| Daylight analysis | Metric, sensor grid, sky model, thresholds, results | Reads spaces, openings, and surfaces |
| Wind study | Domain, wind direction set, terrain roughness, engine, results | Reads masses and context; produces pressure and velocity fields |
| System analysis report | System, method, results per element | Derived from a building services system |
| Result set | Source revision, quantities, units, display state | Referenced by overlays, tables, and comparisons |
| Design alternative | Name, model state, results summary | Compared against other alternatives |

## Analysis model generation

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PERF-001 | Energy model from rooms and spaces | Generates analytical surfaces and thermal zones from spaces and their bounding elements, with each surface linked to the source element. Gaps and overlaps in the boundary are reported with positions. | T1 | 4 |
| AB-PERF-002 | Energy model from masses | Generates a simplified analysis model from mass forms and mass floors using conceptual construction and glazing assumptions by building type. | T1 | 4 |
| AB-PERF-003 | Surface classification | Classifies surfaces as exterior, interior, ground, adiabatic, or air-connected from adjacency, and lets the user override each classification with the override flagged. | T1 | 4 |
| AB-PERF-004 | Openings and shading | Maps windows, doors, and skylights to analytical openings, and maps overhangs, fins, and neighboring buildings to shading surfaces. | T1 | 4 |
| AB-PERF-005 | Model checking for analysis | Checks the analysis model for unclosed zones, missing constructions, zero-area surfaces, intersecting surfaces, and orphaned openings, and lists each with a fix command. Checks run headless. | T1 | 4 |
| AB-PERF-006 | Analysis model editing | Edits analytical surfaces, zones, and boundary conditions without changing the building model, and keeps those edits through regeneration. | T2 | 4 |
| AB-PERF-007 | Ground plane and underground | Defines the ground plane and the ground contact treatment of underground surfaces with soil conductivity and ground temperature profiles. | T2 | 4 |
| AB-PERF-008 | Shading from context | Includes neighboring buildings and terrain from the site context as shading surfaces, with a distance limit for inclusion. | T1 | 4 |

## Thermal properties

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PERF-009 | Material thermal properties | Stores conductivity, density, specific heat, and emissivity on material definitions, and reports materials missing values. | T1 | 4 |
| AB-PERF-010 | Construction thermal calculation | Computes thermal resistance, U-value, and heat capacity of a layered construction from its layers and surface resistances, with thermal bridging adjustments by value. | T1 | 4 |
| AB-PERF-011 | Glazing properties | Defines glazing and frame properties for window types, and computes the overall window value from glass and frame areas. | T1 | 4 |
| AB-PERF-012 | Thermal property library | Provides open libraries of common materials, constructions, and glazings by region, extended by users and imported from open data files. | T2 | 4 |
| AB-PERF-013 | Thermal property overrides | Overrides thermal data on an element type or an analytical surface, with the override visible in reports and reversible. | T2 | 4 |
| AB-PERF-014 | Airtightness and infiltration | Sets airtightness by building or zone and converts it to infiltration rates for analysis. | T2 | 4 |

## Energy analysis

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PERF-015 | Schedules and internal gains | Defines occupancy, lighting, equipment, and setpoint schedules by day type and assigns them to zones and space types. | T1 | 4 |
| AB-PERF-016 | Weather data | Reads weather data from open climate files by location, and shows the data source and period in the report. | T1 | 4 |
| AB-PERF-017 | Energy simulation | Runs an hourly energy simulation of the analysis model through an open simulation engine in a separate process, with progress and cancellation. The model is never blocked during the run. | T1 | 4 |
| AB-PERF-018 | Idealized systems | Runs simulations with idealized heating and cooling systems to obtain loads independent of system choice. | T1 | 4 |
| AB-PERF-019 | Energy results | Reports annual energy by end use and fuel, peak loads, energy use intensity, and monthly profiles per zone and building. | T1 | 4 |
| AB-PERF-020 | Conceptual energy comparison | Runs fast conceptual energy estimates on masses and compares alternatives by form, orientation, glazing ratio, and shading. | T2 | 4 |
| AB-PERF-021 | Parametric energy studies | Varies chosen parameters over a range and runs the resulting simulations in parallel, showing the effect on energy indicators. | T3 | 5 |
| AB-PERF-022 | Building systems definitions | Assigns system templates such as ideal air, variable air volume, fan coil, and heat pump with efficiencies, for simulation with real systems. | T2 | 4 |
| AB-PERF-023 | Renewable energy | Estimates on-site generation from photovoltaic area, tilt, and orientation using radiation data and shading from context. | T3 | 5 |
| AB-PERF-024 | Compliance reports | Produces compliance reports against configurable thresholds for envelope values, energy use, and lighting power, with the rule source stated. | T3 | 5 |

## Carbon analysis

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PERF-025 | Material carbon coefficients | Stores embodied carbon coefficients on material definitions by life cycle module with source, unit, and validity, and lists materials without data. | T1 | 4 |
| AB-PERF-026 | Embodied carbon calculation | Multiplies model quantities by coefficients per element, material, and module, and reports totals per element type, level, and building. Concrete and reinforcement carbon are reported separately. | T1 | 4 |
| AB-PERF-027 | Life cycle boundary | Selects the life cycle modules included in the assessment and states them in every report. | T2 | 4 |
| AB-PERF-028 | Operational carbon | Converts simulated energy by fuel to operational carbon with grid factors that may vary by year. | T2 | 4 |
| AB-PERF-029 | Total carbon report | Combines embodied and operational carbon over a stated study period and shows the split by element type and by phase. | T2 | 4 |
| AB-PERF-030 | Carbon alternatives | Compares carbon results of design alternatives and shows differences by element type. | T2 | 4 |
| AB-PERF-031 | Carbon data import | Imports carbon coefficient data from open and published product datasets and matches them to material definitions with reviewable mappings. | T3 | 5 |
| AB-PERF-032 | Carbon data quality flags | Marks results that depend on missing, default, or generic coefficients, and reports the share of the total they cover. | T2 | 4 |

## Daylighting

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PERF-033 | Sensor grids | Generates sensor grids on work planes in spaces with spacing and offset, and edits points. | T1 | 4 |
| AB-PERF-034 | Daylight factor | Computes daylight factor at sensor points under an overcast sky and shows average, minimum, and evenness ratio per space. | T1 | 4 |
| AB-PERF-035 | Climate-based daylight metrics | Computes spatial daylight autonomy and annual sunlight exposure from weather data and thresholds, with results per space and per point. | T2 | 4 |
| AB-PERF-036 | Illuminance rendering | Renders illuminance on surfaces through the rendering service in a separate process, using the scene contract for the model, for a chosen sky and time. | T2 | 4 |
| AB-PERF-037 | View and glare metrics | Computes view-out indicators and glare probability from chosen viewpoints. | T3 | 5 |
| AB-PERF-038 | Daylight results display | Shows results as color maps on work planes, with threshold lines and tables per space. | T1 | 4 |

## Wind and microclimate

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PERF-039 | Wind estimate | Estimates wind speed and pressure around masses from wind direction and terrain roughness with a fast analytical method, with its limits stated in the result. | T2 | 4 |
| AB-PERF-040 | Wind simulation | Runs a computational flow simulation of the masses and context through an isolated open solver, for a set of wind directions, and returns velocity and pressure fields. | T3 | 5 |
| AB-PERF-041 | Pedestrian comfort | Evaluates pedestrian-level wind speed against comfort criteria and shows zones that exceed them. | T3 | 5 |
| AB-PERF-042 | Wind loads for structure | Transfers surface pressures from wind results to cladding panels in the analytical model for structural load cases. | T3 | 5 |
| AB-PERF-043 | Streamlines and sections | Displays streamlines, velocity slices, and pressure on surfaces for a selected direction. | T3 | 5 |
| AB-PERF-044 | Microclimate maps | Computes solar exposure, shade, and sky view factor at ground level around the building and displays them as maps. | T3 | 5 |

## System analysis reports

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PERF-045 | System analysis report | Compiles loads, flows, sizes, and pressure losses of a duct, pipe, or electrical system into one report with the method and inputs, linked to elements. | T1 | 4 |
| AB-PERF-046 | Building services summary | Summarizes heating and cooling demand, airflow, water flow, and electrical load per zone, system, and building. | T2 | 4 |
| AB-PERF-047 | Report templates | Defines report templates with sections, tables, and charts, and generates the report on demand or in headless runs. | T2 | 4 |

## Exchange

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PERF-048 | gbXML export | Writes the analysis model as gbXML with spaces, surfaces, openings, constructions, and schedules, and reports data that cannot be represented. | T1 | 4 |
| AB-PERF-049 | gbXML import | Reads gbXML models and results into analysis models and maps them to spaces by identifier. | T2 | 4 |
| AB-PERF-050 | IFC space boundaries | Exports and reads second-level space boundaries in IFC with constructions and properties, and reports boundaries that cannot be derived. | T2 | 4 |
| AB-PERF-051 | Simulation input export | Writes the input of the open simulation engine so that users can run it elsewhere, with the mapping from model elements to engine objects stored. | T2 | 4 |
| AB-PERF-052 | Result import | Imports results from external analysis programs through the documented exchange format and displays them like native results. | T2 | 4 |

## Results visualization and comparison

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PERF-053 | Result overlays | Displays results as colored faces, grids, or sample points in plan, section, and three-dimensional views, with legend, limits, and units. | T1 | 4 |
| AB-PERF-054 | Result tables and charts | Presents results as sortable tables and charts that export to CSV and image files. | T1 | 4 |
| AB-PERF-055 | Alternatives comparison | Stores design alternatives with their results and compares them across energy, carbon, daylight, and cost indicators in one view. | T2 | 4 |
| AB-PERF-056 | Result provenance and staleness | Stores the model revision, engine, and settings with each result, and marks a result outdated in every view when the model has changed. | T1 | 4 |
| AB-PERF-057 | Saved result views | Saves result display settings with a view and applies them to new runs. | T2 | 4 |
| AB-PERF-058 | Headless analysis runs | Runs analysis models and result exports from the command line and from Hub jobs with machine-readable output. | T2 | 5 |
