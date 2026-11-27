# Messages of archbuilder-commands: its error codes and the labels of its commands
# (crates/archbuilder-commands/src/error.rs and structure.rs).

ab-cmd-document-not-empty = A project cannot be created in a document that already has one.
ab-cmd-kind-mismatch = The object is not the kind of object this command works on.
ab-cmd-no-building = The document has no building to put a storey in.
ab-cmd-nothing-selected = Select objects first.
ab-cmd-field-unknown = The field "{ $field }" cannot be edited.
ab-cmd-value-invalid = The value does not fit the field.
ab-cmd-last-storey = The last storey of a building cannot be deleted. A building keeps at least one storey.

ab-cmd-project-new-label = New Project
ab-cmd-project-new-description = Creates a project with a site, a building and three storeys with their levels.
ab-cmd-storey-add-label = Add Storey
ab-cmd-storey-add-description = Adds a storey and its level to a building, above the highest level unless an elevation is given.
ab-cmd-storey-rename-label = Rename Storey
ab-cmd-storey-rename-description = Renames a storey and its level.
ab-cmd-storey-delete-label = Delete Storey
ab-cmd-storey-delete-description = Deletes a storey and its level.
ab-cmd-building-rename-label = Rename Building
ab-cmd-building-rename-description = Renames a building.
ab-cmd-level-set-elevation-label = Set Level Elevation
ab-cmd-level-set-elevation-description = Sets the elevation of a level, or of the level of a storey.
ab-cmd-object-rename-label = Rename
ab-cmd-object-rename-description = Renames a project, site, building, storey or level.
ab-cmd-object-select-label = Select
ab-cmd-object-select-description = Selects objects; the outline of a level selects its storey.
ab-cmd-property-set-label = Set Property
ab-cmd-property-set-description = Sets a field of objects of the spatial structure; the command of the property grid.

ab-cmd-storey-has-walls = The storey cannot be deleted because walls or floors stand on it (count: { $count }). Delete them first.
ab-cmd-wall-end-same = The end point is the same as the start point. A wall needs a length.
ab-cmd-wall-create-label = Wall
ab-cmd-wall-create-description = Places a wall on a storey from a baseline of two points, with a thickness, a height above the storey's level and a location line.
ab-cmd-wall-delete-label = Delete Wall
ab-cmd-wall-delete-description = Deletes walls, the selected ones unless walls are named.
ab-cmd-wall-flip-label = Flip Wall
ab-cmd-wall-flip-description = Flips walls: the baseline is reversed, so the exterior side swaps; the location line stays.
ab-cmd-wall-move-end-label = Move Wall End
ab-cmd-wall-move-end-description = Moves one end of a wall to a point; the other end stays and doors and windows keep their place along the wall, or the move is refused naming them.
ab-cmd-object-delete-label = Delete
ab-cmd-object-delete-description = Deletes the selected walls, or the selected storey when no wall is selected.

ab-prompt-wall-start = Wall on { $storey }: specify the start point (thickness { $thickness }, height { $height }, line { $location ->
        [exterior] exterior face
        [interior] interior face
        [core_center] core center
        [core_exterior] core exterior face
        [core_interior] core interior face
       *[other] center
    })
ab-prompt-wall-end = Wall on { $storey }: specify the next point, or press Enter to finish (thickness { $thickness }, height { $height }, line { $location ->
        [exterior] exterior face
        [interior] interior face
        [core_center] core center
        [core_exterior] core exterior face
        [core_interior] core interior face
       *[other] center
    })
ab-prompt-wall-thickness = Wall thickness (now { $thickness })
ab-prompt-wall-height = Wall height (now { $height })
ab-prompt-wall-location = Location line
ab-keyword-wall-thickness = Thickness
ab-keyword-wall-height = Height
ab-keyword-wall-location = Location line
ab-keyword-wall-flip = Flip
ab-keyword-wall-close = Close
ab-keyword-wall-center = Center
ab-keyword-wall-exterior = Exterior face
ab-keyword-wall-interior = Interior face
ab-cmd-wall-join-style-label = Wall Join Style
ab-cmd-wall-join-style-description = Sets how one or both ends of walls join the walls they meet: automatic, butt, square or not joined.
ab-cmd-wall-disallow-join-label = Disallow Wall Join
ab-cmd-wall-disallow-join-description = Keeps wall ends square and out of any join; the walls may overlap there.
ab-cmd-wall-allow-join-label = Allow Wall Join
ab-cmd-wall-allow-join-description = Puts wall ends back into automatic joins.
ab-cmd-no-wall-selected = Point at a wall or select one first.
ab-cmd-no-insert-selected = Select a door or a window first.
ab-cmd-door-place-label = Door
ab-cmd-door-place-description = Places a door in a wall at a distance along it, with a width, a height, a hand and a facing.
ab-cmd-window-place-label = Window
ab-cmd-window-place-description = Places a window in a wall at a distance along it, with a width, a height, a sill height, a hand and a facing.
ab-cmd-insert-flip-hand-label = Flip Hand
ab-cmd-insert-flip-hand-description = Flips the hand of doors and windows: the hinge moves to the other side.
ab-cmd-insert-flip-facing-label = Flip Facing
ab-cmd-insert-flip-facing-description = Flips the facing of doors and windows: the leaf swings to the other side of the wall.
ab-prompt-door-place = Door on { $wall }: click a wall or type a distance ({ $width } x { $height })
ab-prompt-window-place = Window on { $wall }: click a wall or type a distance ({ $width } x { $height })
ab-prompt-insert-width = Width (now { $width })
ab-prompt-insert-height = Height (now { $height })
ab-prompt-insert-sill = Sill height (now { $sill })
ab-keyword-insert-width = Width
ab-keyword-insert-height = Height
ab-keyword-insert-sill = Sill height
ab-keyword-insert-hand = Flip hand
ab-keyword-insert-facing = Flip facing
ab-cmd-floor-arcs-unsupported = The floor { $name } has arc edges, which this command cannot keep. Make its edges straight, or edit its boundary instead.
ab-cmd-floor-no-loop = These walls do not make a closed outline. Pick walls that meet end to end around the area, or click inside a closed loop of walls.
ab-cmd-floor-walls-parallel = Two walls that follow each other in the outline are parallel and do not meet. Pick walls that meet at corners.
ab-cmd-no-floor-selected = No floor is named or selected. Select a floor first.
ab-cmd-floor-too-big = A floor takes at most { $loops } loops and { $points } points in all.
ab-cmd-floor-create-label = Floor
ab-cmd-floor-create-description = Creates a floor slab on a storey, from picked walls (edges follow the wall faces) or from a sketched boundary, with a thickness and an offset of its top from the storey's level.
ab-cmd-floor-edit-boundary-label = Edit boundary
ab-cmd-floor-edit-boundary-description = Replaces the outline and the openings of a floor, with arcs and the walls its edges follow, in one step.
ab-cmd-floor-add-hole-label = Floor hole
ab-cmd-floor-add-hole-description = Cuts a hole in a floor from a sketched closed boundary.
ab-prompt-floor-pick = Floor on { $storey }: click a wall, or click inside a closed loop of walls ({ $face ->
        [exterior] exterior face
       *[interior] interior face
    }, thickness { $thickness }, offset { $offset }). { $count } wall(s) picked; press Enter to finish.
ab-prompt-floor-start = Floor on { $storey }: specify the first point of the boundary (thickness { $thickness }, offset { $offset }).
ab-prompt-floor-next = Floor on { $storey }: specify the next point, or press Enter or choose Close to finish ({ $count } points, thickness { $thickness }, offset { $offset }).
ab-prompt-floor-hole-start = Hole in { $floor }: specify the first point of the hole.
ab-prompt-floor-hole-next = Hole in { $floor }: specify the next point, or press Enter or choose Close to finish ({ $count } points).
ab-prompt-floor-thickness = Floor thickness (now { $thickness })
ab-prompt-floor-offset = Top offset from the level (now { $offset })
ab-keyword-floor-thickness = Thickness
ab-keyword-floor-offset = Level offset
ab-keyword-floor-interior = Interior face
ab-keyword-floor-exterior = Exterior face
ab-keyword-floor-sketch = Sketch
ab-keyword-floor-walls = Pick walls
ab-keyword-floor-done = Done
ab-keyword-floor-close = Close
ab-keyword-floor-undo = Undo point

ab-cmd-not-movable = Storeys and levels cannot be moved, copied, rotated or mirrored.
ab-cmd-insert-alone = A door or window alone can only slide along its wall. Turn or mirror its wall with it.
ab-cmd-mirror-axis = The two points of the mirror line must differ.
ab-cmd-modify-move-label = Move
ab-cmd-modify-move-description = Moves the selected walls, floors, doors and windows; doors and windows follow their wall, and one alone slides along its wall.
ab-cmd-modify-copy-label = Copy
ab-cmd-modify-copy-description = Copies the selected walls (with their doors and windows) and floors by a displacement; copies get new names.
ab-cmd-modify-rotate-label = Rotate
ab-cmd-modify-rotate-description = Turns the selected objects about a base point by an angle, or from a reference angle to a new one.
ab-cmd-modify-mirror-label = Mirror
ab-cmd-modify-mirror-description = Mirrors the selected objects across a line, keeping or erasing the originals; mirrored doors and windows swap hand.
ab-prompt-modify-base = Specify base point or [Displacement]:
ab-prompt-modify-second-move = Specify second point or type a distance:
ab-prompt-modify-second-copy = Specify second point (Enter to finish):
ab-prompt-modify-displacement = Specify displacement (for example 12,0):
ab-prompt-rotate-base = Specify base point:
ab-prompt-rotate-angle = Specify rotation angle or [Reference]:
ab-prompt-rotate-reference = Specify reference angle:
ab-prompt-rotate-new = Specify new angle:
ab-prompt-mirror-first = Specify first point of mirror line:
ab-prompt-mirror-second = Specify second point of mirror line:
ab-prompt-mirror-erase = Erase the original objects? [Yes/No] <No>:
ab-keyword-modify-displacement = Displacement
ab-keyword-rotate-reference = Reference
ab-keyword-yes = Yes
ab-keyword-no = No
ab-prompt-modify-select = Select objects (click, window left to right, crossing right to left), then press Enter:
ab-cmd-select-before-points = Select the objects first (click or drag a window), then press Enter, before giving points.

ab-cmd-room-no-region = This point is not inside a closed loop of walls. Click inside walls that enclose an area.
ab-cmd-room-region-taken = This region already has a room.
ab-cmd-room-none-missing = Every enclosed region of this storey already has a room.
ab-cmd-room-region-unresolved = { $count } region(s) enclosed by walls have a boundary that cannot be resolved, so they have no room. Check the walls around them.
ab-cmd-room-region-sliver = This region has { $area } m², less than the least room area of { $least } m²: it is a sliver between walls and gets no room.
ab-cmd-room-only-slivers = Every region without a room is smaller than the least room area of { $least } m² ({ $count } slivers): no room was placed.
ab-cmd-room-place-label = Room
ab-cmd-room-place-description = Places a room on a storey at a point inside a region closed by walls. The boundary, area, perimeter and volume follow the walls; the room gets the next number of the storey.
ab-cmd-room-place-all-label = Place all rooms
ab-cmd-room-place-all-description = Places a room in every enclosed region of a storey that has none.
ab-prompt-room-place = Room on { $storey }: click inside a region closed by walls, Escape to finish.
ab-cmd-clipboard-paste-label = Paste
ab-cmd-clipboard-paste-description = Pastes copies of the clipboard objects (walls with their doors and windows, floors and rooms) at an insertion point or in place.
ab-cmd-clipboard-paste-storey-label = Paste to Storey
ab-cmd-clipboard-paste-storey-description = Pastes copies of the clipboard objects into another storey at the same plan position; elevations follow the storey.
ab-prompt-paste-point = Specify insertion point (Enter to paste in place):
ab-cmd-clipboard-empty = Nothing has been copied yet. Select walls, floors, doors, windows or rooms and copy them first.
ab-cmd-storey-not-selected = Select the target storey first (for example in the outliner), then paste to storey.
ab-cmd-shortcuts-invalid = The file is not a shortcut table, or it binds a key that is not two letters or a command that does not exist.
ab-cmd-object-not-found = No object has the id { $id }.
ab-cmd-stair-create-label = Stair
ab-cmd-stair-create-description = Creates a stair from a storey's level to the level above: straight, or with a landing and an L or U turn, with a width, a tread depth and a highest riser; the riser count and height follow from the levels.
ab-prompt-stair-start = Stair on { $storey } ({ $shape ->
    [l_turn] L turn to the { $turn ->
        [right] right
       *[left] left
    }
    [u_turn] U turn to the { $turn ->
        [right] right
       *[left] left
    }
   *[straight] straight
}, { $risers } risers of { $riser }, width { $width }, tread { $tread }): specify the start, the middle of the first riser, or an option.
ab-prompt-stair-direction = Stair ({ $risers } risers of { $riser }, width { $width }, tread { $tread }): specify the direction of the first run, or an option.
ab-prompt-stair-turn = Turn to which side, seen walking up?
ab-prompt-stair-width = Stair width (now { $width })
ab-prompt-stair-tread = Tread depth (now { $tread })
ab-keyword-stair-straight = Straight
ab-keyword-stair-l-turn = L turn
ab-keyword-stair-u-turn = U turn
ab-keyword-stair-left = Left
ab-keyword-stair-right = Right
ab-keyword-stair-width = Width
ab-keyword-stair-tread = Tread
ab-prompt-stair-turn-again = "{ $typed }" is not a side. Answer Left (l) or Right (r), or press Enter for the side shown.

ab-cmd-field-from-type = { $field } comes from the type. Edit the type, or choose another type for the selected objects.
ab-cmd-type-unknown = No type is named "{ $name }". Choose one of the types of this kind.
ab-cmd-type-in-use = The type "{ $name }" is used by { $count ->
        [one] one object
       *[other] { $count } objects
    }. Choose a type to replace it with, then delete it.
ab-cmd-type-last = "{ $name }" is the last type of its kind and cannot be deleted. Duplicate it first, or edit it.
ab-cmd-type-kind = The objects are not of the kind of that type. A wall takes a wall type, a floor a floor type, a door a door type and a window a window type.
ab-cmd-type-set-label = Edit Type
ab-cmd-type-set-description = Changes a type's name, layers, core, flags or size; every object of the type changes with it in one step.
ab-cmd-type-duplicate-label = Duplicate Type
ab-cmd-type-duplicate-description = Makes a copy of a type under a new name.
ab-cmd-type-delete-label = Delete Type
ab-cmd-type-delete-description = Deletes a type; the objects that use it get the replacement type.
ab-cmd-type-assign-label = Change Type
ab-cmd-type-assign-description = Gives the selected walls, floors, doors or windows another type of their kind.
ab-cmd-type-match-label = Match Type
ab-cmd-type-match-description = Gives the selected objects the type of a source object of the same kind.
ab-cmd-type-upgrade-label = Add Types
ab-cmd-type-upgrade-description = Gives every wall, floor, door and window without a type a type that matches its size, made when none matches.

ab-type-wall-generic = Generic 200
ab-type-wall-cavity = Exterior masonry cavity
ab-type-wall-partition = Interior partition 100
ab-type-floor-generic = Generic 200
ab-type-floor-slab = Concrete slab with screed and finish
ab-type-door-single = Single 900 × 2100
ab-type-door-double = Double 1500 × 2100
ab-type-window-large = Window 1200 × 1500
ab-type-window-small = Window 600 × 600
ab-type-wall-generated = Generic { $mm }
ab-type-floor-generated = Generic { $mm }
ab-type-roof-generic = Generic roof 300
ab-type-roof-rafters = Timber rafters with insulation and tiles
ab-type-roof-generated = Roof { $mm }
ab-material-roof-tiles = Roof tiles
ab-material-battens = Timber battens
ab-material-roof-underlay = Roof underlay
ab-material-rafters-insulation = Timber rafters with insulation
ab-type-door-generated = Door { $width } × { $height }
ab-type-window-generated = Window { $width } × { $height }
ab-type-copy-name = { $name } ({ $n })
ab-material-generic = Generic
ab-material-brick = Brick
ab-material-air = Air gap
ab-material-insulation = Insulation
ab-material-vapor-barrier = Vapor barrier
ab-material-concrete-block = Concrete block
ab-material-plaster = Plaster
ab-material-gypsum-board = Gypsum board
ab-material-steel-stud = Steel studs
ab-material-floor-finish = Floor finish
ab-material-screed = Screed
ab-material-separation-layer = Separation layer
ab-material-concrete = Concrete
ab-keyword-wall-core-center = Core center
ab-keyword-wall-core-exterior = Core exterior face
ab-keyword-wall-core-interior = Core interior face
ab-type-door-sliding = Sliding 1800 × 2100
ab-type-window-sliding = Sliding window 1800 × 1200

## Align, offset, trim, split, merge, arrays, pin

ab-cmd-modify-align-label = Align
ab-cmd-modify-align-description = Turns and moves an element so the picked line of it lies on a reference line: a wall face or center line, or a floor edge. Walls keep their joins and their doors and windows follow.
ab-cmd-modify-offset-label = Offset
ab-cmd-modify-offset-description = Moves or copies the selected walls and floor boundaries across by a distance, toward the side of the picked point. Walls joined end to end move as a chain.
ab-cmd-modify-trim-label = Trim to corner
ab-cmd-modify-trim-description = Trims or extends two walls so they meet in a clean corner; the picked part of each stays.
ab-cmd-modify-trim-to-label = Trim to boundary
ab-cmd-modify-trim-to-description = Trims or extends one wall at a time to a picked boundary line; the picked part stays.
ab-cmd-modify-trim-many-label = Trim several to boundary
ab-cmd-modify-trim-many-description = Trims or extends several walls to a picked boundary line at once; Enter ends the picks.
ab-cmd-wall-split-label = Split wall
ab-cmd-wall-split-description = Splits a wall in two at a point, or at a distance from its nearest end, with an optional gap. Doors and windows go to the piece that holds them.
ab-cmd-wall-merge-label = Merge walls
ab-cmd-wall-merge-description = Merges two collinear walls that touch end to end, of the same type and height, into one; their doors and windows move with them.
ab-cmd-modify-array-label = Linear array
ab-cmd-modify-array-description = Makes independent copies of the selection in a row, by a spacing or by the distance to the last copy.
ab-cmd-modify-array-radial-label = Radial array
ab-cmd-modify-array-radial-description = Makes independent copies of the selection around a center, over an angle.
ab-cmd-element-pin-label = Pin
ab-cmd-element-pin-description = Pins the selected elements: commands that would move, reshape or delete them refuse.
ab-cmd-element-unpin-label = Unpin
ab-cmd-element-unpin-description = Unpins the selected elements.

ab-prompt-align-reference = Pick the reference line (a wall face or center line, or a floor edge):
ab-prompt-align-target = Pick the line to align to it:
ab-prompt-offset-distance = Type the offset distance, or pick the point to offset to, or [Copy/Move]:
ab-prompt-offset-side = Pick the side to offset toward:
ab-keyword-offset-copy = Copy
ab-keyword-offset-move = Move
ab-prompt-trim-first = Pick the first wall on the part to keep:
ab-prompt-trim-other = Pick the second wall on the part to keep:
ab-prompt-trim-boundary = Pick the boundary line:
ab-prompt-trim-target = Pick the wall to trim or extend, on the part to keep:
ab-prompt-trim-targets = Pick the walls to trim or extend, on the part to keep; Enter when done:
ab-keyword-done = Done
ab-prompt-split-point = Pick the point on a wall to split it, or type a distance from its end, or [Gap]:
ab-prompt-split-end = Pick the wall near the end to measure from:
ab-prompt-split-gap = Type the width of the gap between the pieces:
ab-keyword-split-gap = Gap
ab-prompt-merge-first = Pick the wall that stays:
ab-prompt-merge-other = Pick the wall to merge into it:
ab-prompt-array-count = Type the number of items, the original included:
ab-prompt-array-base = Pick the base point:
ab-prompt-array-second = Pick where the second item goes, or [Last]:
ab-prompt-array-last = Pick where the last item goes, or [Spacing]:
ab-keyword-array-last = Last
ab-keyword-array-spacing = Spacing
ab-prompt-array-center = Pick the center:
ab-prompt-array-angle = Type the angle the items cover, or pick a direction (Enter for a full turn):

ab-cmd-no-wall-at = No wall at that point. Pick on a wall.
ab-cmd-no-line-at = No wall face, wall center line or floor edge at that point. Pick closer to one.
ab-cmd-parallel = The two lines are parallel and never meet.
ab-cmd-same-wall = The same wall was picked twice. Pick another one.
ab-cmd-split-end = Splitting "{ $name }" there would leave a piece too short. Pick farther from its ends.
ab-cmd-split-insert = "{ $name }" lies across the split point of "{ $wall }". Split beside it, or move it first.
ab-cmd-merge-refused = "{ $name }" and "{ $other }" cannot merge: they must be collinear, touch end to end and have the same type, height, base, top and location line.
ab-cmd-array-insert = "{ $name }" can only be arrayed along its own wall.
ab-cmd-nothing-to-offset = Select walls or floors to offset, and pick a side away from their lines.
ab-cmd-roof-create-label = Roof
ab-cmd-roof-create-description = Creates a roof by footprint on a storey, from picked walls (edges follow the wall faces) or from a sketched outline. Each edge either slopes at its own angle or is a gable, and has an overhang.
ab-cmd-roof-set-edge-label = Roof edges
ab-cmd-roof-set-edge-description = Sets whether roof edges slope, their slope and their overhang.
ab-cmd-roof-move-vertex-label = Move roof point
ab-cmd-roof-move-vertex-description = Moves a point of a roof's footprint; the two edges at it stop following walls.
ab-cmd-roof-move-edge-label = Move roof edge
ab-cmd-roof-move-edge-description = Moves an edge of a roof's footprint outward or inward; an edge that follows a wall keeps following it at a new offset.
ab-cmd-wall-attach-top-label = Attach top
ab-cmd-wall-attach-top-description = Attaches the top of the selected walls to the roof or floor above them, so the top follows its underside, including gable ends and hips.
ab-cmd-wall-detach-top-label = Detach top
ab-cmd-wall-detach-top-description = Detaches the top of the selected walls from a roof or floor; each wall gets back the top it had before.
ab-cmd-no-roof-selected = No roof is named or selected. Select a roof first.
ab-cmd-roof-edge-index = The roof has no edge or point with that number.
ab-cmd-no-attach-target = No roof or floor is above the wall "{ $name }". Place a roof over it, or name the roof or floor to attach to.
ab-cmd-attach-target-kind = A wall top can attach only to a roof or a floor.
ab-prompt-roof-pick = Roof on { $storey }: click the walls in order around the area, or click inside a closed loop of walls, then choose Done (slope { $slope }, overhang { $overhang }).
ab-prompt-roof-start = Roof on { $storey }: specify the first point of the footprint (slope { $slope }, overhang { $overhang }).
ab-prompt-roof-next = Roof on { $storey }: specify the next point, or press Enter or choose Close to finish ({ $count } points, slope { $slope }, overhang { $overhang }).
ab-prompt-roof-slope = Slope of the edges (now { $slope }): an angle (30°), a percentage (58%) or a rise over a run (1:2); a bare number is in the project's slope unit
ab-prompt-roof-overhang = Overhang (now { $overhang })
ab-prompt-roof-base = Height of the eaves above the level
ab-keyword-roof-slope = Slope
ab-keyword-roof-overhang = Overhang
ab-keyword-roof-base = Eaves height
ab-cmd-slope-invalid = That slope cannot be used. Type a slope from { $low } to { $high }: an angle (30°), a percentage (58%) or a rise over a run (1:2).
ab-cmd-ceiling-create-label = Ceiling
ab-cmd-ceiling-create-description = Creates a ceiling on a storey from a click inside a room (its boundary is the room's region at the wall faces), from picked walls or from a sketched boundary with arcs, at a height above the storey's level and with an optional slope arrow.
ab-cmd-ceiling-slope-label = Ceiling slope
ab-cmd-ceiling-slope-description = Gives a ceiling a slope arrow: a start line, a direction, and a slope or an end height. A flat ceiling has none.
ab-cmd-shaft-create-label = Shaft
ab-cmd-shaft-create-description = Cuts a sketched footprint through every floor, roof and ceiling between a base and a top, each a level with an offset.
ab-cmd-vertical-opening-create-label = Vertical opening
ab-cmd-vertical-opening-create-description = Cuts a sketched opening in one floor, roof or ceiling, which hosts it.
ab-cmd-opening-place-label = Wall opening
ab-cmd-opening-place-description = Cuts a rectangular or arched opening in a wall at a distance along it, with a width, a height, a sill and a rise, without a door or window.
ab-cmd-no-ceiling-selected = No ceiling is named or selected. Select a ceiling first.
ab-cmd-no-host-selected = No floor, roof or ceiling is named or selected to host the opening. Select one first.
ab-prompt-ceiling-pick = Ceiling on { $storey }: click inside a room or a closed loop of walls, or on walls ({ $face ->
        [exterior] exterior face
       *[interior] interior face
    }, height { $offset }, thickness { $thickness }). { $count } wall(s) picked; press Enter to finish.
ab-prompt-ceiling-start = Ceiling on { $storey }: specify the first point of the boundary (height { $offset }, thickness { $thickness }).
ab-prompt-ceiling-next = Ceiling on { $storey }: specify the next point, or press Enter or choose Close to finish ({ $count } points, height { $offset }).
ab-prompt-ceiling-height = Height of the underside above the level (now { $offset })
ab-prompt-shaft-start = Shaft from { $storey }: specify the first point of the footprint.
ab-prompt-shaft-next = Shaft from { $storey }: specify the next point, or press Enter or choose Close to finish ({ $count } points).
ab-prompt-cut-start = Opening in { $floor }: specify the first point of the cut.
ab-prompt-cut-next = Opening in { $floor }: specify the next point, or press Enter or choose Close to finish ({ $count } points).
ab-prompt-opening-place = Opening in { $wall }: click a wall or type a distance ({ $width } x { $height })
ab-prompt-opening-rise = Rise of the arch (0 for a rectangle)
ab-keyword-opening-rise = Arch rise
ab-type-ceiling-generic = Generic ceiling 20
ab-type-ceiling-grid = Suspended grid 600 x 600
ab-type-ceiling-generated = Ceiling { $mm }
ab-material-ceiling-board = Ceiling board
ab-material-ceiling-tile = Ceiling tile
ab-prompt-arc-mid = Arc: specify a point on the arc.
ab-prompt-arc-end = Arc: specify the end point of the arc.
ab-keyword-arc = Arc
ab-prompt-shaft-base = Base level (name)
ab-prompt-shaft-top = Top level (name)
ab-prompt-shaft-offset = Top offset from the top level
ab-keyword-shaft-base = Base level
ab-keyword-shaft-top = Top level
ab-keyword-shaft-offset = Top offset

ab-cmd-last-view = The last plan view of a storey cannot be deleted. A storey keeps at least one view.
ab-cmd-view-duplicate-label = Duplicate Plan View
ab-cmd-view-duplicate-description = Copies a plan view with its range, scale, detail level, category flags and hidden elements, under a new name.
ab-cmd-view-rename-label = Rename Plan View
ab-cmd-view-rename-description = Renames a plan view; the name must be unused by the other views of its storey.
ab-cmd-view-delete-label = Delete Plan View
ab-cmd-view-delete-description = Deletes a plan view. The last view of a storey stays.
ab-cmd-view-hide-label = Hide in View
ab-cmd-view-hide-description = Hides elements in one plan view only; the model and the other views are unchanged.
ab-cmd-view-show-label = Show in View
ab-cmd-view-show-description = Shows elements hidden in a plan view again, or all of them when none is named.
ab-cmd-view-defaults-label = Make Default Plan Views
ab-cmd-view-defaults-description = Gives every storey without a plan view its default view.
ab-cmd-view-set-label = Change View Setting
ab-cmd-view-set-description = Sets one setting of a plan view; for a storey without a view, makes its view first.

ab-cmd-set-create-label = Create Selection Set
ab-cmd-set-create-description = Saves a named selection set in the document: the given elements, else the selected ones.
ab-cmd-set-update-label = Change Selection Set
ab-cmd-set-update-description = Adds elements to a selection set or removes members from it.
ab-cmd-set-rename-label = Rename Selection Set
ab-cmd-set-rename-description = Renames a selection set; the name must be unused by the other sets.
ab-cmd-set-delete-label = Delete Selection Set
ab-cmd-set-delete-description = Deletes a selection set; its elements stay in the model.
ab-cmd-set-select-label = Select Set Members
ab-cmd-set-select-description = Selects the members of a selection set, replacing the selection or joining it.
ab-cmd-statistics-label = Model Statistics
ab-cmd-statistics-description = Reports element counts by kind, type and storey, and the number of views, selection sets and element types.
ab-cmd-floor-slope-label = Floor slope
ab-cmd-floor-slope-description = Gives a floor a slope arrow: a start line, a direction, and a slope or an end height. A level floor has none.
ab-cmd-floor-slope-edge-label = Floor slope from an edge
ab-cmd-floor-slope-edge-description = Slopes a floor away from one of its boundary edges, which stays at the top offset.
ab-cmd-floor-slope-points-label = Floor slope by three points
ab-cmd-floor-slope-points-description = Slopes a floor so that its top passes through three given elevations.
ab-cmd-floor-edge-create-label = Floor edge
ab-cmd-floor-edge-create-description = Sweeps a profile along selected edges of a floor (slab edge, curb, nosing), mitered at corners and following the floor when its edges change.
ab-cmd-floor-edge-edit-label = Edit floor edge
ab-cmd-floor-edge-edit-description = Changes the profile or the edges of a floor edge.
ab-cmd-roof-extrusion-create-label = Roof by extrusion
ab-cmd-roof-extrusion-create-description = Creates a roof by extruding an open profile sketched in a vertical plane between a start and an end distance, with the layers of its type offset normal to the profile.
ab-cmd-roof-extrusion-edit-label = Edit roof by extrusion
ab-cmd-roof-extrusion-edit-description = Changes the plane, the profile or the extrusion distances of a roof by extrusion.
ab-cmd-railing-create-label = Railing
ab-cmd-railing-create-description = Creates a railing from a sketched path of lines and arcs, or hosted on a stair or ramp so that it follows its host. The type gives the height, top rail, balusters and posts; extensions at the ends and a continuing rail are set on the railing.
ab-cmd-railing-type-define-label = Railing Type
ab-cmd-railing-type-define-description = Creates a railing type, or edits the one named: height, top rail and baluster profiles, baluster spacing, post size, handrail height and corner join.
ab-cmd-ramp-create-label = Ramp
ab-cmd-ramp-create-description = Creates a straight ramp from a foot and the end of its run, with landings, a width, a thickness and a form. The slope follows from the rise and the length; a slope over the largest of its type is a warning. Railings are made on the chosen sides, and a ramp that arrives at the level above cuts an opening in its floor.
ab-cmd-ramp-type-define-label = Ramp Type
ab-cmd-ramp-type-define-description = Creates a ramp type, or edits the one named: the largest slope and the thickness.
ab-prompt-railing-first = Railing: specify the first point of the path.
ab-prompt-railing-next = Railing ({ $count } points): specify the next point, or press Enter to finish.
ab-keyword-railing-done = Done
ab-prompt-ramp-start = Ramp: specify the foot of the sloped run.
ab-prompt-ramp-end = Ramp: specify the end of the sloped run.
ab-cmd-no-work-plane = A work plane needs a level, a grid, a reference plane, a wall, a floor or a ceiling (name it or select it), or three points that are not on one line.
ab-cmd-no-line = Give the start and end points, or a wall whose baseline the line takes.
ab-cmd-grid-create-label = Grid
ab-cmd-grid-create-description = Draws a grid line, straight, an arc or several segments through a list of points, from two points, a wall's baseline or a chain of walls. The label follows the last one: 1, 2, 3 or A, B, C or A.1, A.2.
ab-cmd-reference-plane-create-label = Reference plane
ab-cmd-reference-plane-create-description = Makes a named vertical reference plane, bounded or not, with a strength of strong, weak or none that decides whether snapping finds it.
ab-cmd-work-plane-set-label = Work plane
ab-cmd-work-plane-set-description = Makes a level, grid, reference plane, a face of a wall, floor or ceiling, or three points the active work plane, and displays it.
ab-cmd-work-plane-show-label = Show work plane
ab-cmd-work-plane-show-description = Shows or hides the active work plane.
ab-cmd-datum-set-extent-label = Datum extent
ab-cmd-datum-set-extent-description = Sets the levels a grid reaches between and whether each end shows its head.
ab-cmd-datum-set-view-extent-label = Grid extent in views
ab-cmd-datum-set-view-extent-description = Pulls grid ends in or pushes them out and shows or hides their heads in chosen plan views only, or copies those overrides from one view to others; the grid itself does not change.
ab-prompt-grid-start = Grid: specify the start point.
ab-prompt-grid-end = Grid: specify the end point.
ab-prompt-reference-plane-start = Reference plane: specify the first point.
ab-prompt-reference-plane-end = Reference plane: specify the second point.
ab-type-wall-curtain = Curtain Wall
ab-cmd-curtain-type-label = Curtain Wall Type
ab-cmd-curtain-type-description = Makes or edits a curtain wall type: the grid rules of both directions, the system panel and the interior, border and corner mullions; every curtain wall of the type changes with it.
ab-cmd-curtain-grid-add-label = Add Grid Line
ab-cmd-curtain-grid-add-description = Adds a vertical or horizontal grid line to a curtain wall, across the whole wall or along one segment.
ab-cmd-curtain-grid-remove-label = Remove Grid Segment
ab-cmd-curtain-grid-remove-description = Removes a grid line of a curtain wall, or one segment of it; the cells next to it merge.
ab-cmd-curtain-grid-move-label = Move Grid Line
ab-cmd-curtain-grid-move-description = Moves a grid line of a curtain wall, or pins it where it is so that it stays when the grid rule changes.
ab-cmd-curtain-panel-label = Set Panel
ab-cmd-curtain-panel-description = Gives one cell of a curtain wall a glazed, solid or empty panel, or a door or window, or clears the choice.
ab-cmd-curtain-schedule-label = Curtain Wall Schedule
ab-cmd-curtain-schedule-description = Lists the panels (key, kind, type, size, area, whether overridden) and the mullions (key, role, profile, size, length) of one or every curtain wall, for schedules and exports; derived, never stored.
ab-cmd-curtain-wall-label = Curtain Wall
ab-cmd-curtain-wall-description = Places a curtain wall on a storey from a baseline of two points, with the first curtain wall type of the document unless a type is given.

## Prompts of the floor slope, roof by extrusion and selection set commands
ab-prompt-floor-slope-from = Floor slope: specify the start of the arrow (the floor keeps its top offset along the line square to the arrow there).
ab-prompt-floor-slope-to = Floor slope: specify the end of the arrow (the direction the floor rises toward).
ab-prompt-floor-slope-value = Floor slope: type the slope: an angle (2°), a percentage (2%) or a rise over a run (1:50); a bare number is in the project's slope unit.
ab-prompt-roof-extrusion-plane-start = Roof by extrusion: specify the first point of the vertical plane the profile is drawn in.
ab-prompt-roof-extrusion-plane-end = Roof by extrusion: specify a second point of the plane, along its horizontal axis.
ab-prompt-roof-extrusion-profile = Roof by extrusion: type the next point of the profile as u,z (along the plane's axis, height above the level), or press Enter to finish ({ $count } points).
ab-keyword-roof-extrusion-done = Done
ab-prompt-roof-extrusion-end = Roof by extrusion: type the depth of the extrusion from the plane (negative goes to the other side).
ab-prompt-set-name = Name of the new selection set (the selected elements become its members):
ab-prompt-set-select = Name of the selection set to select ({ $count } sets; Enter takes the first):
ab-cmd-set-unknown = No selection set has that name. Save a selection set first, or type the name of one.
ab-cmd-coordinates-set-label = Project Location
ab-cmd-coordinates-set-description = Sets the project base point, the survey point, true north and the geodetic reference (coordinate system, eastings, northings, height, scale). Elements never move; only the coordinates reported for a point change. Without values it selects the project, whose properties list the same fields.
ab-cmd-coordinates-query-label = Coordinates
ab-cmd-coordinates-query-description = Reports a point in internal, project, survey and map coordinates.
ab-prompt-coordinates-point = Coordinates: pick a point.
ab-cmd-coordinates-recenter-label = Recenter Model
ab-cmd-coordinates-recenter-description = Moves the whole model, with the project base point and the survey point, so that the survey point lies on the internal origin. Map coordinates of every element stay the same.
ab-cmd-project-set-units-label = Display Units
ab-cmd-project-set-units-description = Sets the display unit, the decimals and the rounding step of lengths, areas, volumes, angles and slopes. Stored values stay in SI units; only how values are shown and how a typed number is read changes. Without values it selects the project, whose properties list the same fields.
ab-cmd-project-set-info-label = Project Information
ab-cmd-project-set-info-description = Sets the project information: name, number, client, address, status, issue date, author, organization, description, long name, and the occupancy type, year and method of construction of the building. An empty text clears a field. The IFC export writes these values. Without values it selects the project, whose properties list the same fields.
