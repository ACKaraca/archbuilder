# Views and graphics

This area specifies model views (plans, elevations, sections, callouts, 3D), their visibility and
graphic controls, view templates, graphic standards and materials, navigation, and the model
navigator. How views are generated from the model is described in
[views-and-documentation.md](../../architecture/views-and-documentation.md).

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Model view | Kind, name, scale, detail level, visual style, discipline, phase, crop, template, parent view | Projects the model; owns one detail layer |
| Plan range | Top, cut plane, bottom, view depth | Belongs to a plan view or plan region |
| Vertical plane | Start, end, jogs, depth, far clip | Belongs to a section or elevation |
| Camera | Eye, target, up, projection, field of view, section volume | Belongs to a 3D view |
| View template | Name, controlled properties, values | Bound by views |
| Category override | Category or subcategory, visibility, line and pattern overrides, transparency, halftone | Stored per view or template |
| View filter | Name, categories, rule tree | Applied to views with an override |
| Object style | Category, line weights (projection, cut), color, line pattern, material | Document graphic standard |
| Material | Name, graphics, appearance, physical, thermal, identity, carbon data | Referenced by layers, families and faces |

## View kinds

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-VIEW-001 | Plan views | Creates floor plans and reflected ceiling plans per level, controlled by a plan range (top, cut plane, bottom, view depth). Plan regions give part of a plan its own range, and an underlay shows another level halftoned. | T1 | 2 |
| AB-VIEW-002 | Elevations | Places exterior and interior elevation markers with up to four directions; each creates an elevation view with crop and depth. | T1 | 2 |
| AB-VIEW-003 | Sections | Creates straight or jogged sections with depth and far clip, with head, tail and segment symbols controlled by type. | T1 | 2 |
| AB-VIEW-004 | Callouts | Creates rectangular or sketched callouts that generate an enlarged view with its own scale and detail level, or reference an existing view. | T1 | 2 |
| AB-VIEW-005 | 3D views | Creates orthographic and perspective 3D views with a section volume, orientation to a view or face, and saved cameras. Dragging a section volume face updates the cut in real time. | T1 | 2 |
| AB-VIEW-006 | View scale | Sets a standard or custom scale per view; annotation sizes are defined in paper millimeters and follow the scale. | T1 | 2 |
| AB-VIEW-007 | Partial child views | Splits a large view into child views that share the parent's settings and detail layer; each child has its own crop. Match lines and view references connect them. | T2 | 2 |
| AB-VIEW-008 | View duplication | Duplicates a view as model-only, with its detail layer, or as a child view. | T1 | 2 |

## Visibility and graphic control

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-VIEW-009 | Category visibility and overrides | Sets, per view and per category or subcategory, visibility, projection and cut line style, surface and cut pattern, transparency, halftone and detail level. Hidden categories stay in the model, quantities and exports. | T1 | 2 |
| AB-VIEW-010 | Rule-based view filters | Applies filters made of categories and parameter rules combined with nested AND and OR groups to change visibility or graphics; filter order sets precedence. | T1 | 2 |
| AB-VIEW-011 | View templates | Binds views to templates that control chosen properties (scale, detail level, visibility, filters, range, phase, discipline, visual style). Controlled properties are read-only on the view until the binding is removed, which keeps current values. | T1 | 2 |
| AB-VIEW-012 | Detail level | Sets low, medium or high detail per view; families show the geometry their visibility conditions allow and layered elements show layers at medium and high. | T1 | 2 |
| AB-VIEW-013 | Visual styles | Provides wireframe, hidden line, shaded, consistent colors and textured styles with options for silhouettes, edges, shadows, ambient occlusion, transparency and background. | T1 | 2 |
| AB-VIEW-014 | Discipline display | Sets a discipline per view that halftones or hides elements of other disciplines. | T2 | 2 |
| AB-VIEW-015 | Temporary hide and isolate | Hides or isolates elements or categories temporarily without changing the view; the state can be made permanent, and a reveal mode shows hidden elements highlighted for unhiding. | T1 | 2 |
| AB-VIEW-016 | Crop and clip | Crops a view by a rectangle or a sketched boundary, crops annotations separately, and clips depth with a far clip plane. | T1 | 2 |
| AB-VIEW-017 | Displaced elements | Moves elements visually in one view, with optional displacement path lines, without changing the model, for exploded presentations. | T3 | 3 |
| AB-VIEW-018 | Background regeneration | Regenerates views on worker threads after a change and shows a progress state; a view never presents a result computed from an older revision as current. | T1 | 2 |
| AB-VIEW-019 | Projection quality | Produces cut regions, visible and hidden edges and cut patterns exactly for planar and analytic surfaces; tessellated curved surfaces carry a stated chord tolerance in the drawing metadata. | T1 | 2 |

## Graphic standards and materials

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-VIEW-020 | Object styles | Sets per category and subcategory line weights for projection and cut, color, line pattern and default material. | T1 | 2 |
| AB-VIEW-021 | Line weights, patterns and fills | Defines line weight tables by view scale, line patterns, and fill patterns as drafting patterns (fixed paper size) or model patterns (anchored to geometry). | T1 | 2 |
| AB-VIEW-022 | Materials | Defines materials with graphics (shading color, surface and cut patterns), appearance on the Arc Scene material model, physical and thermal properties, identity data and carbon factors, in a document library and shareable libraries. | T1 | 2 |
| AB-VIEW-023 | Face painting | Applies a material to an element face or to a region of a split face without changing geometry or the element's type. | T2 | 2 |

## Navigation and organization

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-VIEW-024 | Model navigator | Lists views, schedules, sheets, families, groups and links in a tree organized by configurable rules (by kind, level, discipline, phase or parameter), with search. | T1 | 2 |
| AB-VIEW-025 | Viewport navigation | Orbits, pans and zooms with mouse, trackpad and keyboard, with an orientation cube, walk and fly modes, zoom to selection or extents, and saved viewpoints. | T1 | 2 |
| AB-VIEW-026 | Multiple windows | Shows several views at once in tabs, tiles or separate windows, with selection highlighted in all views. | T1 | 2 |
