# Rooms, areas and spaces

This area specifies architectural rooms, area schemes and area plans, space programs, color fill
schemes and architectural zones. Service spaces and thermal zones used for building-services design
are specified in [24-hvac.md](24-hvac.md); they read their boundaries from the rooms defined here.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Room | Seed point or closed profile, level, base offset, upper limit and offset, name, number, department, occupancy, finishes, phase | Bounded by room-bounding elements and separation lines; computed area, perimeter, volume |
| Room separation line | Curve, level | Adds a virtual boundary |
| Room boundary rule | Position (finish face, wall center, core face, core center), volume computation flag | Document setting |
| Area scheme | Name, area types, boundary rules | Owns area plan views per level |
| Area | Seed point, area type, name | Bounded by area boundary lines in one area plan |
| Area boundary line | Curve, picked-from element (optional) | Belongs to one area plan |
| Space program | Rows of name, count, target area range, department | Compared against rooms |
| Color scheme | Parameter, value or range rules, colors, legend style | Applied per view; drives a legend element |
| Zone | Kind (fire compartment, acoustic, security, user kind), members | Groups rooms; one zone per kind per room |

## Rooms

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-SPACE-001 | Room placement | Places a room by picking inside a region enclosed by room-bounding elements at a level, or places rooms in every enclosed region of a level in one command. Area, perimeter and volume are computed from current boundaries, never stored. | T1 | 2 |
| AB-SPACE-002 | Room boundaries | Derives boundaries from elements whose room-bounding flag is set, including elements of linked documents when the link permits, and from room separation lines. The boundary position follows the document's boundary rule. | T1 | 2 |
| AB-SPACE-003 | Room height and volume | Limits each room by an upper level and offset and a base offset. When volume computation is on, volume uses the actual bounding surfaces, including sloped ceilings and roofs. | T1 | 2 |
| AB-SPACE-004 | Room states | Reports unplaced, unenclosed, redundant (two rooms in one region) and overlapping rooms as warnings. A room in an invalid state keeps its identity and data but reports no area or volume until it is valid again. | T1 | 2 |
| AB-SPACE-005 | Room data | Stores name, number, department, occupancy and floor, wall, ceiling and base finishes, plus any project or library parameters bound to rooms. Duplicate numbers on one level are reported. | T1 | 2 |
| AB-SPACE-006 | Sequential renumbering | Renumbers rooms in the order a picked path crosses them, with prefix, start value and increment. Rule-based numbering per AB-PARAM-015 is also available for rooms. | T2 | 2 |
| AB-SPACE-007 | Room solids | Provides each room as a closed solid for 3D display, clash and clearance checks, quantities and IFC space geometry. | T1 | 2 |
| AB-SPACE-008 | Finish quantities by room | Computes finish areas per room from the faces of bounding walls, floors and ceilings that face the room, net of openings, and exposes them to schedules. | T3 | 3 |
| AB-SPACE-009 | Occupant load | Computes an occupant count per room from its area and an occupancy load factor table stored in the document. | T3 | 3 |

## Areas and programs

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-SPACE-010 | Area schemes and area plans | Defines named area schemes (gross, rentable, net, user schemes), each with area plan views per level. Area boundaries are created automatically from walls or drawn, and areas carry an area type. | T1 | 2 |
| AB-SPACE-011 | Area boundary rules | Applies boundary rules per area type, such as measuring to the exterior face, the wall center or the interior face, when boundaries are generated from walls. | T2 | 2 |
| AB-SPACE-012 | Area reports | Reports area totals per scheme, level and area type, and the ratio between two schemes (for example net to gross). | T2 | 2 |
| AB-SPACE-013 | Space program comparison | Stores a space program and compares it with placed rooms by name or department, reporting count and area deviations; views and schedules can highlight rooms outside their target range. | T3 | 3 |

## Color schemes and zones

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-SPACE-014 | Color schemes | Fills rooms or areas in a view by a parameter's values or by value ranges. A legend element lists the colors in use and updates when values change. | T1 | 2 |
| AB-SPACE-015 | Architectural zones | Groups rooms into zones of a typed kind (fire compartment, acoustic zone, security zone, user kind). A room belongs to at most one zone of each kind; zone area and boundary are derived from its members and exported as IFC zones. | T2 | 3 |
