# Assistance

This area specifies what the optional assistant does in ArchBuilder: answering questions about the
model, proposing documentation, checking against rules, generating early design options and
drafting data, always as previewed commands checked by the product's own engines. The suite rules
for levels, grounding, data handling, provenance and evaluation are in
[assistance](https://github.com/ACKaraca/arc-suite/blob/main/docs/architecture/assistance.md); the
panel itself is [AB-AUTO-015](18-automation-and-extensibility.md) and the protocol endpoint is
[AB-AUTO-006](18-automation-and-extensibility.md). No capability in this file is required for any
workflow; each has a manual and scripted path.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Assistance request | Text, level, scope (selection, storeys, views, documents), protected elements, base revision | Produces answers or proposals |
| Proposal | Items, invocations, change summary, checker findings, citations, assumptions, status | Bound to a base revision; applied as one transaction group |
| Design brief | Site, envelope limits, program lines (use, count, area range), adjacency requirements, locks, units | Input of the layout and massing option engines |
| Layout candidate | Room polygons per storey, measured areas, violations, assumptions, status (feasible, unknown, invalid), engine version | Produced for a design brief; previewed in temporary views; converted by proposal |
| Rule draft | Rules in the project's rule-set form, source clause citations, test results | Saved as a rule set (AB-AUTO-013) after review |
| Provenance mark | Origin, proposal, revision | On elements created or changed by an applied proposal until reviewed |

## Questions and guidance

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ASSIST-001 | Model questions | The user asks about the model ("which apartments on level 3 exceed 95 m² net", "doors without a fire rating in the cores"); the assistant answers from object and schedule queries, lists the matching elements as citations that select them, and offers the result as a selection set. Counts and areas come from queries and quantity rules, never from the model's own arithmetic. | T2 | 3 |
| AB-ASSIST-002 | Schedule and quantity questions | Answers about quantities, areas and counts cite the schedule or quantity rule (AB-SCHED-012) and the revision they were computed from; when no schedule answers the question, the assistant proposes a schedule definition that the user previews and adds. | T2 | 3 |
| AB-ASSIST-003 | Command and concept guidance | The assistant explains how to perform a task with the product's commands, cites help pages, and opens the relevant command with parameters filled for the user to complete; it explains concepts such as types and instances and the three editing contexts in the interface language. | T2 | 3 |
| AB-ASSIST-004 | Model health review | On request or before publishing, the assistant runs the warnings register and integrity audit (AB-DATUM-031, AB-DATUM-033), groups findings by cause (duplicate instances, overlapping walls, unjoined walls, unbounded rooms, orphaned annotation), explains each group with its elements, and proposes fixes per group; the audit runs again on the previewed result. | T2 | 3 |
| AB-ASSIST-005 | Option and version narratives | For two design options (AB-PHASE-009) or two versions (AB-COLLAB-004), the assistant writes a summary of what differs (elements added, removed and changed, area and count deltas from schedules) with each statement cited to elements or schedule rows. | T3 | 3 |

## Editing by intent

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ASSIST-006 | Bulk property edits | A request such as "set the doors of the stair cores to EI60 and self-closing" becomes a proposal of parameter edits on the elements in scope; instance or type edits are shown separately, type edits list every affected instance, and values pass the parameter types and rule sets before preview. | T3 | 3 |
| AB-ASSIST-007 | Element edits by intent | Requests to place, move, align or change elements ("set the windows on this storey to 120 cm wide, leave the stair untouched") become proposals of the product's modeling commands within the stated scope; protected elements and elements outside the scope are never changed, and joins and hosted openings are shown in the preview. | T3 | 3 |
| AB-ASSIST-008 | Naming, numbering and classification | The assistant proposes names, numbers (AB-PARAM-015) and classification codes (AB-PARAM-016) for types and elements following the project's naming standard and classification system; the data dictionary lookup (AB-PARAM-017) validates codes and the naming rules check the result. | T3 | 3 |
| AB-ASSIST-009 | Script and graph drafting | The assistant drafts a Python script or a node graph for a repetitive task, shown as source or as a graph with an explanation; it runs only when the user starts it, after the static check, under the script's own permissions. | T3 | 3 |

## Documentation proposals

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ASSIST-010 | Tagging and dimensioning | For chosen views the assistant proposes tags (AB-ANNO-009) and dimension strings for walls, openings and grids following the project's dimensioning conventions; collision-free placement (AB-ANNO-010) positions them, and the anchoring and overlap checks run on the preview. | T3 | 3 |
| AB-ASSIST-011 | Views and sheets | From a request such as "plans, sections and a door schedule for building B at 1:100", the assistant proposes views from templates, sheets with viewports (AB-SHEET-007), numbering by the project's sheet standard and title-block values; sheet-fit and numbering checks report problems before apply. | T3 | 3 |
| AB-ASSIST-012 | Schedule setup | The assistant proposes schedule definitions (fields, filters, grouping, totals) from a description and the project's schedule templates, previewed as the resulting table. | T3 | 3 |
| AB-ASSIST-013 | Specification draft | From the types and materials in the model and the project's specification template, the assistant drafts specification sections as a text document in which every clause links to the types it covers; a consistency check lists types without a section and sections without a type, and keynotes (AB-ANNO-011) map to the sections. | T3 | 4 |

## Checking and coordination

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ASSIST-014 | Rule drafting from requirements | From a requirement the user writes or a clause from an attached document, the assistant drafts rules in the project's rule-set form (AB-AUTO-013) or as an IDS specification, each rule citing its source clause; the draft runs against the model and a test fixture and is saved only by the user. | T3 | 3 |
| AB-ASSIST-015 | Findings explained | After a rule set or IDS validation runs (AB-AUTO-013, AB-IO-009), the assistant explains each finding with the rule, the clause it encodes and the violating elements, groups findings with one cause, and proposes fixes that the same rules check again; it never states that the design complies. | T3 | 4 |
| AB-ASSIST-016 | Clash triage | For interference results (AB-LINK-012), the assistant groups clashes by likely cause (one element against many, a type against a type, a level offset), proposes a responsible party and priority per group from the project's rules, and drafts BCF issues (AB-LINK-013) with viewpoints for the user to issue; the interference check runs again on any proposed fix. | T3 | 3 |
| AB-ASSIST-017 | Exchange readiness | Before an IFC exchange the assistant runs export self-validation, IDS validation and the loss report (AB-IO-019, AB-IO-009, AB-IO-017), explains every loss and missing value, and proposes mapping configuration changes (AB-IO-006) and property values; the export validation runs again on the preview. | T3 | 3 |
| AB-ASSIST-018 | Import cleanup | After an IFC, DWG or DXF import the assistant explains the loss report and proposes the conversion of generic or preserved elements to native kinds where the import rules allow it (AB-IO-007), listing what each conversion keeps and drops. | T3 | 3 |
| AB-ASSIST-019 | Accessibility and egress pre-check | The assistant sets up and runs travel-path, headroom and occupant-load checks (AB-CIRC-007, AB-CIRC-016, AB-SPACE-009) against the rule data of the project, and explains the findings with the paths and rooms involved; results are a pre-check, never a compliance statement. | T3 | 4 |

## Early design

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ASSIST-020 | Layout option engine | A deterministic engine takes a design brief (storey envelopes, fixed cores and obstacles, program lines with counts and hard area ranges, required adjacencies, circulation allowance, locked rooms) and returns up to a set number of room layout candidates within a time limit, each with measured areas, violations and a status: feasible, unknown when the search ended without a result, or infeasible only when the total area provably does not fit. It runs without an assistant, headless and in the application. | T3 | 4 |
| AB-ASSIST-021 | Brief to program | The assistant turns a written brief or an attached room schedule into a design brief: program lines, area ranges, adjacencies and preferences, with hard requirements and preferences kept separate and every line citing the brief text; the user edits the brief before any option is generated. | T3 | 4 |
| AB-ASSIST-022 | Layout candidate review | Candidates open in temporary plan and 3D views that never write to the model; the user compares, pins, rejects or locks a room in place for the next run, and rejected layouts are excluded from later runs. Candidates bound to an older revision or brief are marked stale and cannot be applied. | T3 | 4 |
| AB-ASSIST-023 | Candidate to construction | An accepted candidate becomes a proposal of native rooms, walls of the project's chosen types, doors between required adjacencies and floors; room boundaries are recomputed from the resulting walls, and a candidate whose measured areas or adjacencies no longer hold after wall thickness is reported with the differences before apply. | T3 | 5 |
| AB-ASSIST-024 | Massing options from a brief | From a site with property lines, setbacks and height envelopes (AB-SITE-007, AB-SITE-008) and a program, the assistant proposes massing options as mass commands with mass floors (AB-MASS-036 to AB-MASS-045); envelope containment and program comparison (AB-MASS-044) check every option, and options are stored as design options (AB-PHASE-006). | T3 | 4 |
| AB-ASSIST-025 | Repeated unit placement | The assistant proposes placements of the project's verified apartment and core groups across storeys and buildings to meet a unit mix, with shaft alignment between storeys and locked placements kept; overlap, room-area and alignment checks run on the full resulting model before apply. | T3 | 4 |
| AB-ASSIST-026 | Energy and daylight pre-check | The assistant prepares the analysis model (AB-PERF-001, AB-PERF-005), runs energy and daylight analyses with the project's weather and material data, explains the results, and proposes variants (glazing ratio, shading depth) as design options whose metrics come from re-running the same analyses. | T3 | 4 |

## Content and data

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ASSIST-027 | Family from a description | From a description ("a 900 by 2100 single-leaf door with a 50 mm frame and a vision panel"), the assistant drafts a declarative family with reference planes, parameters, forms and types; the family's parameter test cases (AB-FAM-010) and geometry validity checks run before the user opens it in the family editor. Simple extruded and swept components are in scope; complex freeform components are not. | T3 | 3 |
| AB-ASSIST-028 | Family from a data sheet | From an attached manufacturer data sheet the assistant drafts a family's type table (AB-FAM-019), dimensions, properties and classification, citing the page and table of every value; values the assistant cannot read with certainty are left empty and flagged for the user. | T3 | 4 |
| AB-ASSIST-029 | Cost and carbon estimate | The assistant maps model materials and types to the project's cost items (AB-SCHED-015) and carbon coefficients (AB-SCHED-014, AB-PERF-025), proposing mappings for unmapped items as assumptions with their source; quantities come from the quantity rules, totals from the estimate engine, and the report lists every assumption and unmapped item. | T3 | 4 |

## Existing buildings

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ASSIST-030 | Drawing to model | On a linked DWG, DXF or vector PDF plan (AB-LINK-005, AB-LINK-007), the assistant proposes walls, openings, columns and rooms from the linework through drafting-to-model conversion (AB-ANNO-026), with wall types chosen from thickness and hatching and every proposed element traced to its source lines; room closure and join checks run on the preview. | T3 | 4 |
| AB-ASSIST-031 | Scan and photograph to model | From a point cloud (AB-LINK-006), a raster scan of a drawing or site photographs, the assistant proposes walls, floors and openings for existing conditions with deviation from the source shown per element. | T3 | 5 |

## Building systems

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ASSIST-032 | Services route options | Between chosen equipment and terminals, the assistant proposes duct, pipe or cable-containment routes that the product's sizing and interference checks evaluate, presented as options with lengths, fitting counts and clashes. | T3 | 5 |
| AB-ASSIST-033 | Structural layout options | From a grid, storeys and spans, the assistant proposes column and beam layouts from the project's member types, checked by the analytical model generation and checks (AB-ANALYT-001, AB-ANALYT-009) and exchanged for analysis by the user; member sizes are never presented as designed. | T3 | 5 |

## Review and provenance

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ASSIST-034 | Assisted content review | Elements carrying a provenance mark are shown with a distinct overlay and filter; the user reviews them in a list and clears the mark, and publishing a sheet set or exporting IFC reports any unreviewed elements on the published sheets or in the export before it proceeds. | T3 | 3 |

## Readiness

Readiness classes are defined in
[assistance](https://github.com/ACKaraca/arc-suite/blob/main/docs/architecture/assistance.md#readiness-classes).

| Class | ID | Checked by | Depends on |
|---|---|---|---|
| R1 | AB-ASSIST-001 | Object queries, schedules | AB-AUTO-006, AB-AUTO-015, AB-SCHED-001 |
| R1 | AB-ASSIST-002 | Quantity rules | AB-SCHED-012, AB-SCHED-013 |
| R1 | AB-ASSIST-003 | Command schemas | Command catalog and help in both languages |
| R1 | AB-ASSIST-004 | Warnings register, integrity audit | AB-DATUM-031, AB-DATUM-033 |
| R1 | AB-ASSIST-005 | Option comparison, version comparison | AB-PHASE-009, AB-COLLAB-004 |
| R1 | AB-ASSIST-006 | Parameter types, rule sets | AB-PARAM-001, AB-AUTO-013 |
| R1 | AB-ASSIST-007 | Joins, hosting, warnings register | Modeling commands of the element kinds |
| R1 | AB-ASSIST-008 | Data dictionary lookup, naming rules | AB-PARAM-015 to AB-PARAM-017 |
| R1 | AB-ASSIST-009 | Static check of scripts and graphs | AB-AUTO-004, AB-AUTO-009 |
| R1 | AB-ASSIST-010 | Anchoring and overlap checks | AB-ANNO-009, AB-ANNO-010 |
| R1 | AB-ASSIST-011 | Sheet fit and numbering checks | AB-SHEET-007, view templates |
| R1 | AB-ASSIST-012 | Schedule preview | AB-SCHED-001 |
| R1 | AB-ASSIST-013 | Type and section consistency check | AB-ANNO-011, a project specification template |
| R1 | AB-ASSIST-014 | Rule-set test run | AB-AUTO-013, AB-IO-009 |
| R2 | AB-ASSIST-015 | Rule sets and IDS validation | Encoded rule data for the project's codes and standards |
| R1 | AB-ASSIST-016 | Interference check | AB-LINK-012, AB-LINK-013 |
| R1 | AB-ASSIST-017 | Export self-validation, IDS, loss report | AB-IO-006, AB-IO-009, AB-IO-017, AB-IO-019 |
| R1 | AB-ASSIST-018 | Import validation, loss report | AB-IO-007, AB-IO-017 |
| R2 | AB-ASSIST-019 | Travel-path and occupant-load checks | AB-CIRC-007, AB-CIRC-016, AB-SPACE-009, rule data |
| R2 | AB-ASSIST-020 | Its own acceptance check of candidates | Room boundaries, area rules (AB-SPACE-002, AB-SPACE-011) |
| R1 | AB-ASSIST-021 | Brief validation (ranges, conflicting locks) | AB-ASSIST-020 |
| R2 | AB-ASSIST-022 | Candidate acceptance check | AB-ASSIST-020 |
| R2 | AB-ASSIST-023 | Room boundary recomputation, joins, program comparison | AB-ASSIST-020, AB-SPACE-013, wall and door commands |
| R2 | AB-ASSIST-024 | Envelope containment, program comparison | AB-SITE-008, AB-MASS-036 to AB-MASS-045, AB-PHASE-006 |
| R2 | AB-ASSIST-025 | Overlap, room-area and shaft alignment checks | AB-PHASE-011, a verified group library |
| R2 | AB-ASSIST-026 | Energy and daylight analyses | AB-PERF-017, AB-PERF-034, bundled or user weather and material data |
| R1 | AB-ASSIST-027 | Family parameter test cases, geometry validity | AB-FAM-004 to AB-FAM-010, canonical text form |
| R1 | AB-ASSIST-028 | Type table validation, user confirmation of flagged values | AB-FAM-019 |
| R2 | AB-ASSIST-029 | Quantity rules, estimate engine | AB-SCHED-014, AB-SCHED-015, AB-PERF-025, cost and carbon data |
| R2 | AB-ASSIST-030 | Room closure, joins | AB-ANNO-026, AB-LINK-007 |
| R3 | AB-ASSIST-031 | Deviation from source | AB-LINK-006, segmentation of point clouds |
| R3 | AB-ASSIST-032 | Sizing, interference check | AB-HVAC-041, pipe sizing, AB-LINK-012 |
| R3 | AB-ASSIST-033 | Analytical model checks | AB-ANALYT-001, AB-ANALYT-009 |
| R1 | AB-ASSIST-034 | Provenance query | Provenance marks in `arc-core` |

## Evaluation

The assistance task suite is built from the [reference workflows](../workflows.md) on the housing
fixtures: WF-06 (AB-ASSIST-001, AB-ASSIST-002, AB-ASSIST-020 to AB-ASSIST-025), WF-07
(AB-ASSIST-010 to AB-ASSIST-012), WF-08 (AB-ASSIST-002, AB-ASSIST-029), WF-10 (AB-ASSIST-030),
WF-11 (AB-ASSIST-005), WF-12 (AB-ASSIST-016), WF-13 (AB-ASSIST-014, AB-ASSIST-015, AB-ASSIST-017)
and WF-04 (AB-ASSIST-027). Each task's acceptance check uses the checkers named above, and the
thresholds are recorded with the suite ([OQ-15, OQ-25](../open-questions.md)).
