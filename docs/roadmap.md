# Roadmap

This roadmap groups ArchBuilder capabilities into milestones mapped onto the suite phases. Milestones
have exit criteria stated as acceptance scenarios from [workflows.md](product/workflows.md); they
have no dates. Tier 3 capabilities listed in a phase may move to a later milestone without changing
that milestone's exit criteria.

Assistance rows ([32-assistance.md](product/capabilities/32-assistance.md)) never gate a milestone:
each is offered when its task suite passes, and every exit criterion is met with no assistant
configured ([suite assistance track](https://github.com/ACKaraca/arc-suite/blob/main/docs/roadmap.md#assistance-track)).

| Milestone | Suite phase | Theme |
|---|---|---|
| M1 | 2 | Model core |
| M2 | 2 | Documentation and exchange |
| M3 | 3 | Coordination, depth and site |
| M4 | 4 | Structure, building services, massing and visualization |
| M5 | 5 | Multi-user editing, fabrication depth, application exchange, web and tablet, and assistance |

## M1 Model core (phase 2)

**Goal.** Model the shell of a multi-storey, multi-building housing project with families, save it,
and see it in plans and 3D.

**Scope.**
- AB-DATUM-001 to AB-DATUM-012, AB-DATUM-014 to AB-DATUM-016, AB-DATUM-018 to AB-DATUM-037
- AB-WALL-001 to AB-WALL-012, AB-WALL-016 to AB-WALL-025, AB-WALL-027
- AB-OPEN-001 to AB-OPEN-009, AB-OPEN-011 to AB-OPEN-016, AB-OPEN-018
- AB-SLAB-001 to AB-SLAB-016, AB-SLAB-018 to AB-SLAB-020
- AB-CIRC-001 to AB-CIRC-014
- AB-FAM-001 to AB-FAM-019, AB-FAM-021, AB-FAM-024, AB-FAM-027, AB-FAM-029 to AB-FAM-033
- AB-PARAM-001 to AB-PARAM-016, AB-PARAM-018 to AB-PARAM-023
- AB-VIEW-001, AB-VIEW-005, AB-VIEW-006, AB-VIEW-012, AB-VIEW-013, AB-VIEW-018 to AB-VIEW-022,
  AB-VIEW-024 to AB-VIEW-026
- AB-PHASE-010 to AB-PHASE-013
- AB-VIS-001, AB-VIS-003
- AB-AUTO-001 to AB-AUTO-003, AB-AUTO-007, AB-AUTO-008
- AB-IO-001, AB-IO-002, AB-IO-013, AB-IO-017 to AB-IO-019

**Dependencies.** `arc-core` phase 0 deliverables: `arc-base`, `arc-geom`, `arc-solve`, `arc-doc`
(transactions, journal, dependency graph, package), `arc-command`, `arc-gpu`, `arc-ui`, `arc-app`;
`arc-brep` for curved family forms; `arc-view` plan projection. No dependency on other Arc products.

**Exit criteria.**
- WF-01, WF-02, WF-04 and WF-05 pass on housing fixtures A and B.
- WF-06 passes for group placement and room computation (area plans may follow in M2).
- The interactive edit and level-change performance targets of the
  [architecture overview](architecture/overview.md) are met on fixture A.
- Fixture A saved by one build opens unchanged in the next build and after a simulated crash.

## M2 Documentation and exchange (phase 2)

**Goal.** Produce a complete construction document set and checked IFC from the housing project.

**Scope.**
- AB-VIEW-002 to AB-VIEW-004, AB-VIEW-007 to AB-VIEW-011, AB-VIEW-014 to AB-VIEW-016,
  AB-VIEW-023
- AB-ANNO-001 to AB-ANNO-009, AB-ANNO-011 to AB-ANNO-025
- AB-SCHED-001 to AB-SCHED-008, AB-SCHED-012, AB-SCHED-013, AB-SCHED-016
- AB-SHEET-001 to AB-SHEET-015
- AB-SPACE-001 to AB-SPACE-007, AB-SPACE-010 to AB-SPACE-012, AB-SPACE-014
- AB-FAM-025, AB-FAM-026, AB-FAM-028
- AB-LINK-005, AB-LINK-007
- AB-VIS-002, AB-VIS-004, AB-VIS-005
- AB-IO-003 to AB-IO-007, AB-IO-011

**Dependencies.** `arc-draft` (detail layers, drafting views, sheets, plot), `arc-view` sections and
hidden lines, `arc-io-ifc`, `arc-io-dxf`, `arc-io-dwg` (read in phase 1, write in phase 2),
`arc-io-pdf`, `arc-io-mesh`. DraftBuilder's 2D core defines the shared drafting behavior.

**Exit criteria.**
- WF-03, WF-06, WF-07, WF-08 and WF-09 pass.
- IFC 4.3 export of fixture A passes independent validation and contains no unreported proxy; the
  same export re-imported into an empty document reproduces the spatial structure, element counts
  by entity, types and quantities.
- PDF and DWG sets of fixture A pass the validation rules in
  [interoperability.md](architecture/interoperability.md).
- **First production release** of ArchBuilder for architectural use.

## M3 Coordination, depth and site (phase 3)

**Goal.** Work with consultants and existing buildings, manage alternatives and phases, model the
site, and collaborate through Arc Hub at file level.

**Scope.**
- AB-DATUM-013, AB-DATUM-017
- AB-WALL-013, AB-WALL-014, AB-WALL-026
- AB-OPEN-010, AB-OPEN-017, AB-OPEN-019, AB-CIRC-015, AB-CIRC-016
- AB-SPACE-008, AB-SPACE-009, AB-SPACE-013, AB-SPACE-015 to AB-SPACE-019
- AB-SITE-001 to AB-SITE-013
- AB-FAM-022, AB-FAM-034
- AB-PARAM-017
- AB-VIEW-017, AB-ANNO-010, AB-ANNO-026
- AB-SCHED-009 to AB-SCHED-011, AB-SCHED-014, AB-SCHED-015, AB-SCHED-017
- AB-SHEET-016 to AB-SHEET-018
- AB-PHASE-001 to AB-PHASE-009, AB-PHASE-014 to AB-PHASE-016
- AB-LINK-001 to AB-LINK-004, AB-LINK-006, AB-LINK-008 to AB-LINK-014
- AB-COLLAB-001 to AB-COLLAB-006, AB-COLLAB-017, AB-COLLAB-018
- AB-VIS-006
- AB-AUTO-004 to AB-AUTO-006, AB-AUTO-009, AB-AUTO-011 to AB-AUTO-015
- AB-ASSIST-001 to AB-ASSIST-012, AB-ASSIST-014, AB-ASSIST-016 to AB-ASSIST-018, AB-ASSIST-027,
  AB-ASSIST-034: read-only questions, editing by intent, documentation proposals, rule drafting,
  clash triage and exchange checks
- AB-IO-008 to AB-IO-010, AB-IO-012, AB-IO-015

**Dependencies.** `arc-hub-api` and Arc Hub Documents module (phase 2 of the suite); `arc-nodes`;
`arc-io-bcf`, `arc-io-pointcloud`, `arc-io-geo`, `arc-io-step`; the suite assistant host in
`arc-app` for the assistance rows; Arc Review for federated review of
BCF issues (runtime only); CivilBuilder surfaces through LandXML and IFC 4.3 (runtime only).

**Exit criteria.**
- WF-10, WF-11, WF-12, WF-13 and the file-level parts of WF-14 (steps 1 to 3) pass.
- A CI job runs IDS validation, rule sets and publishing on fixture A headlessly.

## M4 Structure, building services, massing and visualization (phase 4)

**Goal.** Add the structural and building-services disciplines, concept massing with early analysis,
and photoreal visualization.

**Scope.**
- [20-structural-modeling.md](product/capabilities/20-structural-modeling.md),
  [21-reinforcement.md](product/capabilities/21-reinforcement.md),
  [23-analytical-model.md](product/capabilities/23-analytical-model.md),
  [24-hvac.md](product/capabilities/24-hvac.md),
  [25-piping-and-plumbing.md](product/capabilities/25-piping-and-plumbing.md),
  [26-electrical.md](product/capabilities/26-electrical.md),
  [28-concept-massing.md](product/capabilities/28-concept-massing.md) and
  [29-building-performance-analysis.md](product/capabilities/29-building-performance-analysis.md):
  rows planned for phase 4
- AB-WALL-015, AB-SLAB-017, AB-FAM-020, AB-FAM-023
- AB-VIS-007 to AB-VIS-012
- AB-AUTO-010
- AB-ASSIST-013, AB-ASSIST-015, AB-ASSIST-019 to AB-ASSIST-022, AB-ASSIST-024 to AB-ASSIST-026,
  AB-ASSIST-028 to AB-ASSIST-030: the layout option engine, brief and massing options, findings
  explained, analysis pre-checks, data sheets, estimates and drawing-to-model proposals
- AB-IO-014, AB-IO-016

**Dependencies.** RenderBuilder through the Arc Scene contract (`arc-scene`); MeshBuilder live link
(runtime only); `arc-io-usd`; structural analysis exchange formats.

**Exit criteria.**
- The housing fixture gains a concrete frame with foundations and a services model with ventilation,
  domestic water and power; structural and services elements export to IFC 4.3 with systems and
  connections and pass independent validation.
- The analytical model of the frame is exchanged with an analysis tool and results are displayed.
- A massing study generates storeys and walls by face association and reports areas and solar
  exposure.
- A photoreal still and a solar study of fixture A are produced through RenderBuilder.

## M5 Multi-user editing, fabrication depth, application exchange, web and tablet, and assistance (phase 5)

**Goal.** Element-level multi-user editing and branch and merge through Arc Hub, fabrication-level
steel and services output, exchange with the native project files of other applications through
connectors, browser and tablet builds for massing and review, and optional assistant workflows.

**Scope.**
- AB-COLLAB-007 to AB-COLLAB-016
- [22-steel-detailing.md](product/capabilities/22-steel-detailing.md) and
  [27-fabrication-detailing.md](product/capabilities/27-fabrication-detailing.md), and rows of the
  other reserved files planned for phase 5
- AB-ASSIST-023, AB-ASSIST-031 to AB-ASSIST-033: candidate to construction, scan to model, services
  and structural layout options
- [30-application-connectors.md](product/capabilities/30-application-connectors.md) and AB-IO-020:
  connectors, exchange package, round trips, licensed bridges and open readers
- [31-web-and-tablet.md](product/capabilities/31-web-and-tablet.md): browser and tablet builds,
  touch and pen massing, on-site review, offline documents

**Dependencies.** Arc Hub design collaboration module with change-set exchange; `arc-doc`
change-set support; the Arc exchange package specification in
`arc-core` and the `arc-connectors` repository (runtime only; connectors are not linked into
ArchBuilder); WebAssembly, iOS and Android targets of `arc-core`, `arc-gpu` and `arc-ui`; a
user-installed open solver for AB-ANALYT-051.

**Exit criteria.**
- WF-14 passes in full, including branch merge with conflict preservation.
- Several users edit fixture A concurrently with element borrowing, and no synchronization discards
  work.
- A steel stair and frame produce shop drawings and fabrication files that pass independent readers.
- WF-15 passes: a housing fixture round-trips through a connector three times without duplicates, and
  a re-import lists exactly the elements edited on each side.
- WF-16 passes: a mass is sketched with a pen on a tablet, reviewed on site offline and synchronized
  through Arc Hub, and the same document opens in the browser build.
