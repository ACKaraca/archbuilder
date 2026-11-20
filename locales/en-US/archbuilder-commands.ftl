# Messages of archbuilder-commands: its error codes and the labels of its commands
# (crates/archbuilder-commands/src/error.rs and structure.rs).

ab-cmd-document-not-empty = A project cannot be created in a document that already has one.
ab-cmd-kind-mismatch = The object is not the kind of object this command works on.
ab-cmd-no-building = The document has no building to put a storey in.
ab-cmd-nothing-selected = Select a storey or a wall first.
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

ab-cmd-storey-has-walls = The storey cannot be deleted because walls stand on it (count: { $count }). Delete the walls first.
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
