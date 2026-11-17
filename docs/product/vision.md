# Vision

This document states why ArchBuilder exists, whom it serves, the principles that decide trade-offs,
what is in and out of scope, and what distinguishes it.

## Problem and purpose

A building is designed by several disciplines that each need a precise, data-rich model and a full
set of drawings derived from it. The tools that do this well tie users to one operating system, one
vendor's file format and, increasingly, one vendor's cloud. Open exchange is treated as an export
step, large models regenerate on a single core, and documentation detail is either forced into the
model or redrawn by hand without any record of where it came from.

ArchBuilder is a building information modeling application for architecture, structure and building
services. One semantic model per document holds the building; plans, sections, elevations, 3D views,
schedules, sheets and exchange files are derived from it and can be rebuilt at any time. It runs
natively on Windows, macOS and Linux, has browser and tablet builds of the same code for concept
massing, viewing and markup, works fully offline without an account, stores documents in a
documented open package, and treats IFC as a first-class representation of its own data.

## Primary users and deliverables

| User | Deliverables |
|---|---|
| Architects and architectural technologists | Design models, area and program reports, construction document sets (plans, sections, elevations, details, schedules, sheets) in PDF and DWG |
| Interior designers | Room layouts, finish schedules, furniture plans, presentation views |
| Structural engineers and detailers | Physical and analytical models, framing and foundation drawings, reinforcement and steel fabrication output |
| Building-services engineers | Duct, pipe and electrical systems, sizing reports, coordinated services models and drawings |
| BIM coordinators and managers | Federated, checked IFC models, IDS validation results, BCF issue sets, model health reports |
| Owners and facility teams | Handover data in IFC and COBie with classification and properties |
| Computational designers and developers | Node graphs, scripts, plug-ins and headless checks in continuous integration |

## Product principles

1. **One semantic authority.** The document's model is the only source of truth. Views, schedules,
   sheets, render scenes and exports are derived and never edited as independent copies of model
   data.
2. **Typed commands only.** Every change, whether from the interface, a script, a node graph, a
   plug-in or an assistant, is a validated, undoable command inside a transaction.
3. **Three editing contexts.** Linked model views edit the model; view detail layers add drawing
   detail without touching the model; detached drawings are independent drawings that record their
   source and never write back.
4. **Declarative families.** Families are typed parameters, constraints and geometry recipes
   evaluated by a bounded expression language, not arbitrary programs.
5. **Exchange that tells the truth.** Imports preserve what they cannot model and report every loss;
   exports never fall back silently to generic proxy elements; an independent reader validates them.
6. **Housing first.** The first acceptance scenario is multi-building, multi-storey housing; every
   milestone is proven on it before broader building types.
7. **Offline and portable.** No feature requires a network, an account or a licence server.

## Scope

In scope: datum and coordinates; walls, curtain systems, openings, floors, roofs, ceilings and
circulation; rooms, areas and site; families and parameters; views, annotation, detached drawings,
schedules and sheets; phases, design options, groups and assemblies; links and coordination;
collaboration through Arc Hub; visualization through RenderBuilder; automation and extensibility;
IFC-grade interoperability; structural modeling, reinforcement, steel detailing and the analytical
model; HVAC, piping and electrical systems with fabrication detailing; concept massing and early
performance analysis; exchange with the native project and family files of other applications
through connectors, licensed bridges and open readers; concept massing, viewing and markup in the
browser and on tablets; and linear static solving of simple structural cases through open solver
processes. The capability areas are listed in
[capabilities/README.md](capabilities/README.md).

## Non-goals

| Non-goal | Reason |
|---|---|
| Reverse engineering undocumented file formats or binaries, and running the compiled or protected scripts of other applications | Excluded for legal reasons by the suite's clean-room policy; native project and family files are exchanged through connectors that use the other application's public API ([30](capabilities/30-application-connectors.md)) |
| A built-in path tracer | Photoreal output comes from RenderBuilder through the Arc Scene contract, which keeps one renderer for the suite |
| Detailed civil design (alignments, corridors, grading optimization, networks) | Covered by CivilBuilder; ArchBuilder exchanges terrain and context with it |
| Federated clash management across many models, 4D sequencing and 5D cost review | Covered by Arc Review; ArchBuilder provides in-document interference checks and BCF |
| Native finite-element kernels, nonlinear and dynamic analysis, and code design checks | Linear static solving of simple cases runs in an open solver process (AB-ANALYT-051); other analyses and code design checks stay with analysis software, and results are imported and displayed |
| Embedded scripting interpreters and native plug-ins | Scripts run out of process through the automation protocol; plug-ins are sandboxed WebAssembly |
| Mandatory cloud services | Arc Hub is optional and self-hostable; nothing in the product depends on a hosted service |
| Full construction-documentation authoring on tablets and in the browser | Tablet and browser builds cover concept massing, viewing, markup and light edits ([31](capabilities/31-web-and-tablet.md)); sheets, schedules and detailed authoring stay on the desktop build |
| Certifying compliance with building regulations | Checks report against stated rules and sources; certification remains with the responsible professional |

## Positioning

ArchBuilder covers the professional scope of building information modeling and documentation
addressed today by tools such as Autodesk Revit, Graphisoft Archicad, Vectorworks Architect,
Nemetschek Allplan, Trimble Tekla Structures and Autodesk Advance Steel for structure and steel, and
early-stage design tools such as Autodesk FormIt, alongside open-source work such as Bonsai and the
FreeCAD BIM workbench. It is an independent, open-source implementation built on the Arc foundation.

## Differentiators

| Capability the product has | Where specified |
|---|---|
| IFC 4.3 semantics as the model's exchange vocabulary, with IDS validation, BCF and data dictionary lookup | [19](capabilities/19-interoperability.md), [15](capabilities/15-links-and-coordination.md), [09](capabilities/09-parameters-and-constraints.md) |
| Loss-reported round trips with preserved foreign data and stable identity maps | AB-IO-017, AB-IO-018, AB-IO-008 |
| Repeated round trips with native project files of other applications, with identifiers independent of their worksharing and change detection on re-import | AB-CONNECT-010, AB-CONNECT-011, AB-CONNECT-012 |
| Concept massing by touch and pen, on-site review and offline use on tablets, and the same document in the browser | AB-TOUCH-004, AB-TOUCH-012, AB-TOUCH-014, AB-TOUCH-017 |
| Multi-threaded, revision-tagged regeneration of geometry, views and schedules | AB-VIEW-018, [architecture overview](../architecture/overview.md) |
| Element-level model comparison, a canonical text form for diff tools, and branch and merge through Arc Hub | AB-COLLAB-004, AB-COLLAB-005, AB-COLLAB-007 |
| One constraint solver for project constraints, family sketches and profiles | AB-PARAM-020, AB-FAM-008 |
| One executable with identical features on Windows, macOS and Linux | [architecture overview](../architecture/overview.md) |
| No version lock-in: every older document opens; newer minor formats open read-only | AB-IO-001, AB-IO-002 |
| The DraftBuilder drafting engine for details, drafting views, detached drawings and sheets | AB-ANNO-022, AB-ANNO-023, [13](capabilities/13-sheets-and-publishing.md) |
| Built-in node graphs for parametric design, family geometry and rules | AB-AUTO-009, AB-FAM-022, AB-AUTO-012 |
| Headless model checking and publishing in continuous integration | AB-AUTO-014, AB-SHEET-015 |
| Family-free opening recipes and family test cases before publishing | AB-OPEN-009, AB-FAM-010 |
| Versioned family libraries that never change a project silently | AB-FAM-029, AB-FAM-030 |
| Detached drawings that keep their source revision and orphan-aware annotations that never guess | AB-ANNO-020, AB-ANNO-024 |
| Cross-storey group placement for repeated housing units | AB-PHASE-011 |
| Spreadsheet round trips keyed by stable element identifiers | AB-SCHED-016 |
| Space program comparison and design option comparison by metrics | AB-SPACE-013, AB-PHASE-009 |
| Headroom and travel-path checks computed from actual geometry | AB-CIRC-007, AB-CIRC-016 |
| Offline-first collaboration with conflict preservation that never discards work | AB-COLLAB-001, AB-COLLAB-013 |
