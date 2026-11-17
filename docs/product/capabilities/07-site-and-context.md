# Site and context

This area specifies the building site: terrain solids, pads and graded regions, cut and fill, property
lines and setbacks, site components, parking, and context from GIS and civil data. Detailed civil
design (alignments, corridors, grading optimization) belongs to CivilBuilder; the two products
exchange terrain and context through LandXML and IFC 4.3.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Terrain solid | Point set or TIN, boundary, layered type, phase | Instance of a terrain type; owns subdivision regions and pads |
| Subdivision region | Boundary, material | Owned by a terrain solid |
| Pad | Boundary, elevation, layered type | Depresses a terrain solid |
| Property line | Segments by bearing and distance or sketch, closure | Owns setback definitions |
| Setback envelope | Offsets per segment, height planes | Derived volume from a property line |
| Site component | Family type, position, size parameters | Placed on terrain or level |
| Context element | Source (GIS layer, mesh, IFC), footprint, height | Excluded from quantities by default |

## Terrain

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-SITE-001 | Terrain solids | Creates a layered terrain solid from picked points, a point file (CSV), contour polylines from a DXF or DWG drawing, a LandXML surface, or a TIN from an IFC 4.3 file. Point counts above the document's resource limit are rejected with a simplification offer, not truncated. | T1 | 3 |
| AB-SITE-002 | Terrain editing | Adds, moves and deletes points, simplifies the surface to a tolerance, splits and merges terrain solids, and defines subdivision regions with their own material such as paving or lawn. | T1 | 3 |
| AB-SITE-003 | Pads and graded regions | Depresses the terrain under a building pad with its own layers and elevation, and creates a graded region that replaces part of the existing terrain with a new surface while keeping the existing surface for comparison. | T2 | 3 |
| AB-SITE-004 | Cut and fill | Computes cut and fill volumes between an existing and a proposed terrain (by phase or by pair of solids), per region and in total, and exposes them to schedules. | T2 | 3 |
| AB-SITE-005 | Excavation by building elements | Cuts the terrain solid with foundations, basements and pads, and reports excavated volume per cutting element. | T2 | 3 |
| AB-SITE-006 | Site display | Draws contours at primary and secondary intervals with labels, and fills terrain below grade in sections with a configurable pattern and base elevation. | T1 | 3 |

## Boundaries and components

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-SITE-007 | Property lines | Creates property lines from a bearing and distance table or by sketch, computes enclosed area and reports the closure error. | T1 | 3 |
| AB-SITE-008 | Setbacks and height envelopes | Offsets property segments by setback distances and adds height planes to form an allowable building envelope; elements outside the envelope are reported as warnings. | T3 | 3 |
| AB-SITE-009 | Site components | Places trees, planting, vehicles, people, lighting and street furniture on terrain or levels, with height and spread parameters for planting. | T1 | 3 |
| AB-SITE-010 | Parking layout | Arrays parking stalls along a path by stall width, depth, angle and aisle rules, and reports stall counts by type. | T2 | 3 |

## Context

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-SITE-011 | GIS underlays | Displays CRS-aware vector and raster GIS data positioned by the document's georeference, without converting it into model elements. | T2 | 3 |
| AB-SITE-012 | Context buildings | Generates context buildings from GIS footprints with height attributes, or imports them as meshes, as context elements that are excluded from quantities and optionally from exports. | T2 | 3 |
| AB-SITE-013 | Civil terrain exchange | Imports and refreshes terrain and site surfaces published by a civil design document through LandXML or IFC 4.3, keeping a source link and reporting what changed on refresh. | T2 | 3 |
