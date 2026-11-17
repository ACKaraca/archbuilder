# Links and coordination

This area specifies references to other documents and files (Arc documents, IFC models, drawings,
point clouds, underlays), coordination between disciplines through monitored elements and
interference checks, and issue exchange with BCF. Federated clash detection across many models is
an Arc Review function; this area covers what is needed inside the authoring tool.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Link | Source path or Hub reference, format, content hash, positioning, reference kind (attachment or overlay), loaded state | Instance placements in this document |
| Link instance | Link, named position or transform, display settings, phase mapping | Displayed in views |
| Monitored element | Local element, source link and element ID, last accepted source state | Raises coordination items |
| Coordination item | Monitored element, change kind, state (open, accepted, rejected, postponed), comment | Reviewed by users |
| Interference result | Element pair, intersection volume or clearance, state | Derived from a check run |
| Issue | BCF topic, viewpoints, element references, comments, status | Local and Hub-synchronized |

## Links

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-LINK-001 | Arc document links | Links `.archb` documents by origin, shared position or center, with relative or absolute paths, reload, unload and reload-from. Changed sources are detected by content hash and offered for reload. | T1 | 3 |
| AB-LINK-002 | Nested links | Treats links inside linked documents as attachments (shown in the host) or overlays (not shown), per link. | T2 | 3 |
| AB-LINK-003 | Link display | Displays each link per view as in the host view, as in a named view of the linked document, or with custom category overrides. | T1 | 3 |
| AB-LINK-004 | IFC reference models | Links IFC files as read-only reference models with spatial structure, properties and selectable elements, optionally room-bounding. Refresh compares the new file's hash, keeps the previous version for comparison and never alters converted native elements. | T1 | 3 |
| AB-LINK-005 | Drawing links | Links DWG and DXF drawings with layer visibility, unit and positioning control, to one view or to all views. | T1 | 2 |
| AB-LINK-006 | Point cloud links | Links LAS, LAZ and E57 point clouds positioned by coordinates, cropped by section volumes, displayed by color, intensity or elevation, with snapping to points. | T2 | 3 |
| AB-LINK-007 | PDF and image underlays | Places PDF pages and raster images as underlays with page choice and two-point scale calibration. | T1 | 2 |
| AB-LINK-008 | Mesh context links | Links glTF and OBJ models as context with transforms and units. | T2 | 3 |
| AB-LINK-009 | Binding a link | Converts a linked Arc document into native elements and groups with new identities, an identity map to the source and a report of unconverted content. | T2 | 3 |
| AB-LINK-010 | Annotating linked elements | Dimensions and tags elements of linked documents, anchored to linked element IDs; an annotation becomes orphaned when its linked element disappears. | T2 | 3 |

## Coordination

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-LINK-011 | Monitored elements | Copies levels, grids, columns, walls and floors from a link, or monitors existing local elements against linked ones. When a source changes, a coordination item lists the change for the user to accept (apply to the local element), reject, postpone or comment. | T2 | 3 |
| AB-LINK-012 | Interference check | Tests selected categories in this document and its links for intersections or clearance violations, lists element pairs with location, and exports results as BCF issues. | T2 | 3 |
| AB-LINK-013 | BCF issues | Creates issues with viewpoints, element references and comments; imports and exports BCF files and synchronizes through the BCF API of an Arc Hub. Opening an issue restores its viewpoint, section and selection. | T1 | 3 |
| AB-LINK-014 | Send to Arc Review | Sends the current federation (this document, its links and their positions) to Arc Review for clash detection and review, with element identities preserved for issues coming back. | T2 | 3 |
