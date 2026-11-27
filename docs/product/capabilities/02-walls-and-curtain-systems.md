# Walls and curtain systems

This area specifies vertical enclosure: layered walls, their constraints, joins, profiles and
attached bands, stacked walls, and curtain walls and curtain systems with grids, panels and mullions.
Walls host the openings of [03-doors-windows-and-openings.md](03-doors-windows-and-openings.md) and
bound the rooms of [06-rooms-areas-and-spaces.md](06-rooms-areas-and-spaces.md).

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Wall | Location curve (line or arc), location line, base and top constraints, profile override, cross-section kind, flip state, function, structural usage, room-bounding flag | Instance of a wall type; hosts inserts and bands; joins other walls; attaches to floors, roofs, ceilings and planes |
| Wall type | Ordered layers, core boundary, wrapping rules, attached bands, function | System family type; referenced by walls and stacked wall types |
| Layer | Function, material, thickness, variable flag, wrap flag | Belongs to a wall, floor, roof or ceiling type |
| Wall join | Participants, join style, priority, end disallow flags | Derived at regeneration; editable style and order |
| Wall band | Profile, face, height reference, offset, setback | Belongs to a wall type or a wall instance |
| Stacked wall type | Segment list (wall type, height, one variable segment), offsets | Composes wall types |
| Curtain wall | Location curve, constraints, grid rules, cell map | Instance of a curtain wall type; owns grid lines, panels and mullions |
| Curtain grid line | Direction, rule-placed or manual, segments present | Owned by a curtain wall or curtain system |
| Curtain panel | Cell key, panel type, override flag | Fills one cell; may be a door or window family |
| Mullion | Profile type, position (interior, border, corner), segment | Sits on a grid line segment |

## Basic walls

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-WALL-001 | Wall placement | Places walls by line, arc, chain, rectangle, polygon, circle, picked lines or picked faces, with height and constraints from the tool settings. A key flips the wall side about the location line during placement. | T1 | 2 |
| AB-WALL-002 | Location line | Positions a wall by its centerline, core centerline, exterior or interior finish face, or exterior or interior core face. Changing the location line keeps the wall's physical position; thickness changes grow away from the location line. | T1 | 2 |
| AB-WALL-003 | Layered wall types | Defines a wall type as ordered layers, each with function (structure, substrate, insulation, air gap, membrane, finish), material and thickness, and a core boundary separating core from finish layers. Total thickness is derived; membranes have zero thickness. | T1 | 2 |
| AB-WALL-004 | Assembly editor | Edits layers with a live section preview showing materials, core boundary and wrapping. Invalid assemblies (no structure layer where the function requires one, negative or zero total thickness) are rejected with a reason. | T1 | 2 |
| AB-WALL-005 | Vertical constraints and attachment | Applies base and top constraints per AB-DATUM-008 and attaches the top or base to floors, roofs, ceilings or reference planes so the wall follows the target surface, including sloped and multi-plane roofs. Detaching restores the top constraint the wall had before it was attached. | T1 | 2 |
| AB-WALL-006 | Roof-bound walls | Lets a wall stop at the underside of a roof or rise through it as a parapet with a height above the roof surface. The wall top is recomputed from the actual roof surface whenever the roof, its type or the wall moves; no derived height is stored. | T1 | 2 |
| AB-WALL-007 | Wall joins | Joins walls automatically at ends and T intersections with butt, miter or square-off style, and cleans layers by function priority so cores meet cores and finishes wrap. Joins can be disallowed per end and join order can be switched. | T1 | 2 |
| AB-WALL-008 | Layer wrapping | Wraps finish layers at free ends and around inserts as exterior, interior, both or none, set per type and overridable per insert family. | T2 | 2 |
| AB-WALL-009 | Elevation profile editing | Replaces a wall's rectangular elevation outline with a sketched profile, including notches and sloped tops, and restores the rectangle on reset. Hosted inserts that fall outside the new profile are reported and the edit is rejected. | T1 | 2 |
| AB-WALL-010 | Slanted and tapered walls | Sets a wall cross-section to vertical, slanted (single angle) or tapered (separate exterior and interior angles). Layers follow the slant; inserts remain normal to the wall's location surface. | T2 | 2 |
| AB-WALL-011 | Wall bands | Adds profile-based horizontal or vertical bands (projecting or recessed) to wall faces by type or per instance, measured from base or top. Bands follow wall length, stop at inserts and join across joined walls of the same band definition. | T2 | 2 |
| AB-WALL-012 | Stacked walls | Composes a wall type from basic wall types stacked at fixed heights with one variable segment. The stack behaves as one wall for placement, joins and hosting, and displays each segment's layers in section. | T2 | 2 |
| AB-WALL-013 | Lining walls | Places a wall against a host wall on a chosen side with an optional gap. The lining follows the host's length, location and height changes and is reported if the host is deleted. | T2 | 3 |
| AB-WALL-014 | Walls from room boundaries | Creates walls along the boundary of selected rooms or picked boundary segments, inside or outside the boundary, with the room-bounding flag set at creation. | T2 | 3 |
| AB-WALL-015 | Walls from faces | Creates walls from planar or ruled faces of mass or generic elements and keeps them associated so they update when the face changes; breaking the association freezes the wall. | T3 | 4 |
| AB-WALL-016 | Function and usage flags | Records wall function (interior, exterior, foundation, retaining, soffit, shaft), room-bounding flag and structural usage (non-bearing, bearing, shear, combined). Function drives default IFC predefined type; structural usage feeds the analytical model. | T1 | 2 |
| AB-WALL-017 | Orientation flip | Swaps exterior and interior sides of a wall about its location line, reversing layer order. Inserts keep their facing relative to the room or exterior as their family defines. | T1 | 2 |
| AB-WALL-018 | Split and merge | Splits a wall at a point or with a gap and merges collinear or concentric walls of the same type and constraints into one, preserving hosted inserts and their identities. | T2 | 2 |
| AB-WALL-019 | Layer display by detail level | Draws walls as a single outline with optional solid fill at low detail and with individual layers, cut patterns and core boundary at medium and high detail, using material graphics. | T1 | 2 |

## Curtain walls and curtain systems

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-WALL-020 | Curtain wall grid rules | Lays out horizontal and vertical grid lines by fixed distance, fixed number, maximum spacing or minimum spacing, with justification, angle and offset. Rule changes regenerate grid lines; manually unpinned lines keep their position. | T1 | 2 |
| AB-WALL-021 | Curtain panels | Fills each cell with a panel type: glazed, solid, empty, a panel family, or a door or window family. A per-cell override keeps the cell's identity when grid rules change, as long as the cell survives. | T1 | 2 |
| AB-WALL-022 | Mullions | Places profile-based mullions on interior, border and corner positions, with per-type join priority (horizontal or vertical continuous) and corner mullion types for angled joins between curtain walls. | T1 | 2 |
| AB-WALL-023 | Grid editing | Adds a grid line across all segments, one segment or all except picked segments, removes individual segments, and unpins a line from its rule. Panels and mullions on merged or split cells are reconciled in the same transaction. | T1 | 2 |
| AB-WALL-024 | Embedded curtain walls | Cuts an opening in a basic wall automatically when a curtain wall is placed inside it, and updates the opening when either wall moves. | T2 | 2 |
| AB-WALL-025 | Curved curtain walls | Places curtain walls on arcs with planar panels between vertical grid lines, reporting chord deviation per panel. | T2 | 2 |
| AB-WALL-026 | Curtain systems on faces | Applies curtain grids, panels and mullions to arbitrary planar or ruled faces of mass or generic elements with a UV grid, and regenerates when the face changes. | T2 | 3 |
| AB-WALL-027 | Panel and mullion counts | Exposes per-panel width, height, area and type and per-mullion length to schedules and IFC, with cell keys that stay stable across regeneration. | T1 | 2 |

### Behavior notes: curtain grids

- **Grid angle (AB-WALL-020).** Each grid rule of a curtain type has an angle of at most 60 degrees
  either way, counter-clockwise in the elevation seen with the start of the baseline on the left.
  A turned family leans its lines about the middle of the wall; a line's position is where it
  crosses the middle, and fixed distances and spacings are measured along the baseline (vertical
  lines) or the height (horizontal lines). Cells and panels of a turned grid are convex polygons
  cut to the wall, listed with their corners; width and height of such a panel are those of its
  bounding box and its area is the polygon's. Leaning bars are cut square to their axis. A door or
  window override needs a rectangular cell: in a turned cell the panel is drawn as a plain one.
  Added lines lean with their family. The two families must cross at a clear angle; a rule pair
  that makes them nearly parallel is refused. Older documents carry no angle and stay straight.
- **Justification and offset (AB-WALL-020).** A fixed distance counts from the start, the middle
  or the end of the baseline or the height, shifted by the offset. The other three rules divide
  the stretch the offset leaves free (from the offset to the end for a start justification, up to
  the end less the offset for an end justification, between the two for a center one) into equal
  cells, and a line stands at each end of that stretch that does not reach a border. Their value
  is the number of interior lines of the stretch or the spacing measured over it. Without an
  offset nothing changes.
- **Split and merge with grid edits (AB-WALL-018, AB-WALL-023).** A curtain wall that holds grid
  edits or panel overrides can be split or merged. Edits are carried to the wall they belong to
  only where that is unambiguous: an added line stays where it is, a segment is cut to the part
  it lies in, and a move or removal of a rule's line, or a panel override, is kept only when the
  new wall has the same line or cell, under the same key and in the same place. The first line of
  a merged wall's second part gets a fresh key past the first part's. What cannot be kept without
  guessing is left out and listed in the command result as `unkept`, never rebound to a neighbor.
  Both commands are one undo step.
- **Pin (AB-DATUM-022).** A pinned curtain wall still takes grid edits and panel overrides: they
  divide and fill the wall but never move or reshape it, like the finishes of a pinned room.
  Moving, splitting, merging or deleting it is refused, naming the wall.
- **Schedule rows (AB-WALL-027).** `archbuilder.curtain.schedule` (read-only, not an undo step)
  lists, for one curtain wall or all of them, a row per panel (cell key, kind, door or window type
  name, material, thickness, clear width, height and area, whether overridden, whether the cell
  is cut by a turned grid) and per mullion (key, role, upright or not, profile, width, depth,
  length along its axis), with the wall's total panel area and mullion length. The rows are
  derived from the wall, and `archbuilder-cli inspect` prints the same rows. A schedule view
  built on them follows the keys, which stay the same when the grid regenerates.
