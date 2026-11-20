# Doors, windows and openings

This area specifies hosted inserts, family-free opening recipes, openings that are not families,
and freestanding components. Inserts depend on hosts from
[02-walls-and-curtain-systems.md](02-walls-and-curtain-systems.md) and
[04-floors-roofs-and-ceilings.md](04-floors-roofs-and-ceilings.md); their definitions come from
[08-families-and-family-editor.md](08-families-and-family-editor.md).

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Insert | Family type, host ID, host-local position, sill or head height, hand and facing flags, room calculation point | Hosted by a wall, floor, ceiling, roof or face; cuts its host |
| Opening recipe | Operation kind, frame, panel and glazing dimensions, materials | Built-in definition used by door and window types without a family |
| Opening | Kind (wall, shaft, face, vertical, dormer), profile, base and top constraints | Cuts one or more hosts |
| Component | Family type, level and offset or work plane, mirror state | Freestanding; may be face-based or line-based |
| Host cut | Void solid, host layers affected, wrapping result | Derived from the insert or opening |
| Clearance zone | Source (family declaration, door or window operation, user placement), shape, height | Diagnostic; checked against elements and other zones, never part of quantities or exports |

## Hosted inserts

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-OPEN-001 | Hosted placement | Places a loadable family according to its hosting kind (wall, floor, ceiling, roof, face, work plane, level, line) with snapping to host faces and live dimensions to nearby inserts and wall ends. Keys rotate and flip during placement; an option tags each insert on placement. | T1 | 2 |
| AB-OPEN-002 | Door and window parameters | Exposes width, height, sill or head height, frame, panel and glazing materials, operation and any authored family parameters at type or instance level as the family defines. Rough opening width and height are derived and exposed for schedules. | T1 | 2 |
| AB-OPEN-003 | Host cutting | Cuts host layers with the insert's void and wraps finish layers per the host and family rules. When host thickness changes, the insert keeps its position relative to the host reference face named by the family. | T1 | 2 |
| AB-OPEN-004 | Hand and facing | Flips hand and facing with in-view controls or commands. Mirrored copies keep correct left and right hand values for schedules and IFC operation types. | T1 | 2 |
| AB-OPEN-005 | Host change and fit validation | Moves an insert to another host in one command. Placement or movement that leaves the insert outside its host, or overlapping another insert at the same height, is rejected with the conflicting element named; inserts at separate heights may overlap in plan. | T1 | 2 |
| AB-OPEN-006 | Host deletion | Deletes hosted inserts together with their host in one transaction and lists them before the interactive command completes. The programmatic command returns the same list without prompting. | T1 | 2 |
| AB-OPEN-007 | Plan and elevation symbols | Draws swing arcs, sliding arrows, sill lines and opening direction triangles from the family's symbolic geometry per view kind and detail level. | T1 | 2 |
| AB-OPEN-008 | Room relations | Computes from-room and to-room for each door and the room for each window per phase from the family's room calculation point. Flipping the door swaps the relation; schedules and tags update. | T1 | 2 |
| AB-OPEN-009 | Opening recipes | Provides built-in parametric door and window recipes (single swing, double swing, sliding, folding, fixed, casement, awning, hopper, pivot) that generate frame, panel, glazing, symbols, quantities and IFC representations from typed dimensions without authoring a family. A recipe type can be converted to an editable family. | T1 | 2 |
| AB-OPEN-010 | Opening clearance zones | Generates optional swing and operation clearance volumes from door and window geometry and reports collisions with other elements. Clearance volumes are diagnostic and excluded from quantities and exports. | T3 | 3 |

## Openings

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-OPEN-011 | Wall openings | Cuts rectangular or arched openings in straight and curved walls without a family, sized by width, height and sill, editable by grips and live dimensions. | T1 | 2 |
| AB-OPEN-012 | Shaft openings | Cuts a sketched vertical opening through every floor, roof and ceiling between a base and a top constraint, with symbolic cross lines in plan. Adding a floor inside the range cuts it automatically. | T1 | 2 |
| AB-OPEN-013 | Face and vertical openings | Cuts an opening normal to a picked face, or a vertical opening through a sloped floor or roof, from a sketched profile. | T2 | 2 |
| AB-OPEN-014 | Dormer openings | Cuts a roof by the outline formed where selected walls and a dormer roof meet it, and updates when those elements move. | T2 | 2 |

## Components

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-OPEN-015 | Freestanding components | Places furniture, casework, equipment, fixtures, planting and entourage on a level with offset or on a work plane, with mirror, rotate and align. Mirroring reflects geometry without changing the family definition. | T1 | 2 |
| AB-OPEN-016 | Line-based components | Places components defined along a picked or drawn line, such as counters and rails, whose length follows the line. | T2 | 2 |
| AB-OPEN-017 | Face-based components | Places components on any face, including faces of linked documents, and keeps them on the face when it moves. A component on a linked face reports when the face no longer exists. | T2 | 3 |
| AB-OPEN-018 | Bounding alignment | Aligns a component to wall faces or other components using its actual geometry bounds in the storey's axes, moving only that component. | T2 | 2 |
| AB-OPEN-019 | Component clearance zones | Generates the use zones that component families declare (the space in front of fixtures, appliances and cabinets, a wheelchair turning circle) and user zones placed in plans, and reports where a zone overlaps an element or another zone. A zone that meets only mesh geometry without a closed volume is reported as uncertain, never as clear. Zones are diagnostic and excluded from quantities and exports. | T3 | 3 |
