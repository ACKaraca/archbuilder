# Annotation, detailing and detached drawings

This area specifies dimensions, text, tags, keynotes and symbols, the detail layer each model view
owns, drafting views, and detached drawings that never write back to the model. Drafting geometry
uses the same drafting engine as DraftBuilder. Anchoring and orphan rules are described in
[views-and-documentation.md](../../architecture/views-and-documentation.md).

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Dimension | Kind, references, style, overrides | Owned by a view's detail layer; values read from the model |
| Spot annotation | Kind (elevation, coordinate, slope), anchor, origin choice | Owned by a detail layer |
| Text note | Rich text, style, leaders, width | Owned by a detail layer or drafting view |
| Tag | Tag family type, tagged element, leader, orientation | Owned by a detail layer; reads element parameters |
| Keynote table | Hierarchical keys and texts | Document resource; read by keynote tags and legends |
| Detail item | Drafting entity, detail component, filled or masking region, repeating detail | Owned by a detail layer or drafting view |
| Anchor | Kind (element role, persistent face or edge reference, datum, view coordinate), state (bound, orphaned) | Attached to every annotation |
| Drafting view | Drafting document (arc-draft), scale | Model-independent |
| Detached drawing | Drafting document, source view ID, source revision, copied detail IDs | Never writes to the model |

## Dimensions

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ANNO-001 | Dimension kinds | Creates aligned, linear, angular, radial, diameter and arc-length dimensions, as chains, baseline sets or ordinate sets. | T1 | 2 |
| AB-ANNO-002 | Dimension references | Dimensions to faces, centerlines, core faces, grids, levels and reference planes. A wall dimension command dimensions a wall with its openings and intersecting walls in one chain using a chosen reference rule. | T1 | 2 |
| AB-ANNO-003 | Dimension values and overrides | Displays values measured from the model, with prefix, suffix, text above and below and alternate units. Replacement text is visibly marked as overridden and never changes geometry. | T1 | 2 |
| AB-ANNO-004 | Equality display | Shows an equality-constrained chain as equal marks or as values, per dimension. | T2 | 2 |
| AB-ANNO-005 | Spot annotations | Places spot elevations, spot coordinates and spot slopes that read the model at the anchor, relative to the project base point, survey point or a level. | T1 | 2 |
| AB-ANNO-006 | Dimension styles | Defines terminators, text style, witness line gap and extension, units and precision per dimension type. | T1 | 2 |

## Text, tags and symbols

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ANNO-007 | Text notes | Creates rich text (bold, italic, underline, lists, subscript, superscript) with straight or arc leaders, wrap width and text types, with spelling check and find and replace across the document. | T1 | 2 |
| AB-ANNO-008 | Tags | Tags elements by category, by multi-category tag or by material, with leaders and orientation, optionally on placement. Tags read live parameter values and can edit them in place. | T1 | 2 |
| AB-ANNO-009 | Bulk tagging | Tags every element of chosen categories visible in a view that has no tag yet, placing tags at the family's default offset and skipping elements already tagged. | T2 | 2 |
| AB-ANNO-010 | Collision-free placement | Arranges tags and labels in a view to avoid overlaps with each other and with model lines, following leader rules, as a command that previews before applying. | T3 | 3 |
| AB-ANNO-011 | Keynotes | Reads keynote tables in an open text format, places element, material and user keynotes, and generates keynote legends listing only keys used in a view or sheet. Reloading the table updates every keynote. | T2 | 2 |
| AB-ANNO-012 | Symbols | Places north arrows, scale bars, break lines and generic annotation symbols; a north arrow follows the view's true-north setting. | T1 | 2 |
| AB-ANNO-013 | Revision clouds | Draws revision clouds assigned to a revision, with revision tags; clouds feed sheet revision tables. | T1 | 2 |
| AB-ANNO-014 | View markers | Displays section, elevation and callout markers with the sheet and detail number of the referenced view, updated when the view is placed or moved on sheets. | T1 | 2 |
| AB-ANNO-015 | Match lines and view references | Places match lines and view reference annotations that display the sheet and detail of the adjacent view. | T2 | 2 |

## View detail layers

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ANNO-016 | Detail layer | Gives every model view a detail layer holding detail lines, arcs, splines, filled and masking regions, detail components, repeating details, insulation patterns, text and dimensions. Detail items are stored separately from the model, appear only in that view and never change model elements. | T1 | 2 |
| AB-ANNO-017 | Detail components and repeating details | Places 2D detail component families and repeats a component along a path at a spacing, adjusting the count when the path changes. | T1 | 2 |
| AB-ANNO-018 | Detail groups | Groups detail items into reusable detail groups; an attached detail group follows a model group's instances in views where it is shown. | T2 | 2 |
| AB-ANNO-019 | Annotation anchoring | Anchors every annotation to an element role, a persistent face or edge reference, a datum or a view coordinate. Anchored annotations follow model changes in their view. | T1 | 2 |
| AB-ANNO-020 | Orphan handling | Marks an annotation as orphaned when its anchor no longer resolves, keeps its last position, shows the orphan state and stops presenting its measured value as current. Reattachment is an explicit command; nearby references are never guessed. | T1 | 2 |
| AB-ANNO-021 | Legends | Creates legend views showing symbolic representations of chosen types at a scale with text; a legend can be placed on several sheets. | T1 | 2 |

## Drafting views and detached drawings

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-ANNO-022 | Drafting views | Creates model-independent 2D views with the drafting engine's full entity set: lines, polylines, arcs, splines, hatches, text, dimensions, blocks with attributes, layers and line types. | T1 | 2 |
| AB-ANNO-023 | Detached drawings | Creates a drafting drawing from a model view's current projection and selected detail items, with new identities, the source view and the source revision recorded. Later model changes never alter it, and edits to it never write to the model. | T1 | 2 |
| AB-ANNO-024 | Source change indication | Shows when a detached drawing's source view has changed since its recorded revision and offers a side-by-side comparison. Refreshing creates a new detached drawing; the existing one is never overwritten. | T2 | 2 |
| AB-ANNO-025 | Detail library | Saves drafting views, detail groups and detail components to packages and inserts them into other documents with conflict handling for styles and blocks. | T2 | 2 |
| AB-ANNO-026 | Drafting to model conversion | Creates walls, floors or grids from selected drafting lines as an explicit command that leaves the drafting entities untouched. | T3 | 3 |
