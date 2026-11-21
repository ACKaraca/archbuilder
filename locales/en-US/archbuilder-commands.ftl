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
ab-cmd-object-delete-label = Delete
ab-cmd-object-delete-description = Deletes the selected walls, or the selected storey when no wall is selected.

ab-prompt-wall-start = Wall on { $storey }: specify the start point (thickness { $thickness }, height { $height }, line { $location ->
        [exterior] exterior face
        [interior] interior face
       *[other] center
    })
ab-prompt-wall-end = Wall on { $storey }: specify the next point, or press Enter to finish (thickness { $thickness }, height { $height }, line { $location ->
        [exterior] exterior face
        [interior] interior face
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
ab-cmd-floor-no-loop = These walls do not make a closed outline. Pick walls that meet end to end around the area, or click inside a closed loop of walls.
ab-cmd-floor-walls-parallel = Two walls that follow each other in the outline are parallel and do not meet. Pick walls that meet at corners.
ab-cmd-no-floor-selected = No floor is named or selected. Select a floor first.
ab-cmd-floor-too-big = A floor takes at most { $loops } loops and { $points } points in all.
ab-cmd-floor-create-label = Floor
ab-cmd-floor-create-description = Creates a floor slab on a storey, from picked walls (edges follow the wall faces) or from a sketched boundary, with a thickness and an offset of its top from the storey's level.
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
