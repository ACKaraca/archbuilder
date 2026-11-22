# Messages owned by the ArchBuilder application: panel titles, the file type, the window title,
# the toolbar and the property grid (apps/archbuilder/src).

ab-title = ArchBuilder
ab-file-archb = ArchBuilder project
ab-panel-viewport = Viewport
ab-panel-properties = Properties
ab-panel-outliner = Outliner
ab-panel-command-line = Command line
ab-menu-structure = Structure
ab-menu-export = Export

ab-tool-new = New
ab-tool-new-hint = Create a new project
ab-tool-open = Open
ab-tool-open-hint = Open a project file
ab-tool-save = Save
ab-tool-save-hint = Save the project
ab-tool-save-as = Save As
ab-tool-save-as-hint = Save the project under another name
ab-tool-add-storey = Add Storey
ab-tool-add-storey-hint = Add a storey above the highest one
ab-tool-delete-storey = Delete Storey
ab-tool-delete-storey-hint = Delete the selected storey and its level
ab-tool-wall = Wall
ab-tool-wall-hint = Draw walls on the selected storey, or the lowest one, with the pointer or typed coordinates
ab-tool-undo = Undo
ab-tool-undo-hint = Undo the last change
ab-tool-redo = Redo
ab-tool-redo-hint = Redo the change that was undone
ab-tool-zoom-extents = Zoom Extents

ab-prop-empty = Select an item in the outliner to see its properties.
ab-prop-not-editable = Base elevations and elevation references are stored for later use and change nothing yet, so they cannot be edited.
ab-kind-project = Project
ab-kind-site = Site
ab-kind-building = Building
ab-kind-storey = Storey
ab-kind-level = Level
ab-kind-wall = Wall
ab-prop-name = Name
ab-prop-number = Number
ab-prop-client = Client
ab-prop-address = Address
ab-prop-length-unit = Length unit
ab-prop-template = Template
ab-prop-elevation = Elevation
ab-prop-elevation-base = Elevation reference
ab-prop-storey = Storey level
ab-prop-level = Level
ab-prop-start-x = Start X
ab-prop-start-y = Start Y
ab-prop-end-x = End X
ab-prop-end-y = End Y
ab-prop-thickness = Thickness
ab-prop-height = Height
ab-prop-base-offset = Base offset
ab-prop-location-line = Location line
ab-prop-top-constraint = Top level
ab-top-unconnected = Unconnected
ab-prop-wall-storey = Storey
ab-loc-center = Center
ab-loc-exterior = Exterior face
ab-loc-interior = Interior face
ab-unit-meter = Meters
ab-unit-millimeter = Millimeters
ab-unit-centimeter = Centimeters
ab-unit-foot = Feet
ab-unit-inch = Inches
ab-base-project = Project origin
ab-base-survey = Survey point
ab-aspect-identity = Identity
ab-aspect-geometry = Position
ab-aspect-units = Units
ab-aspect-data = Relations

ab-outliner-no-project = This document has no project.
ab-outliner-create-project = Create Project
ab-prop-start-join = Start join
ab-prop-end-join = End join
ab-prop-wall-volume = Volume
ab-join-auto = Automatic
ab-join-butt = Butt
ab-join-square = Square
ab-join-disallowed = Not joined
ab-tool-door = Door
ab-tool-door-hint = Place doors in walls: click a wall, or select a wall and type the distance along it
ab-tool-window = Window
ab-tool-window-hint = Place windows in walls: click a wall, or select a wall and type the distance along it
ab-kind-door = Door
ab-kind-window = Window
ab-prop-distance = Distance along wall
ab-prop-width = Width
ab-prop-sill = Sill height
ab-prop-frame-width = Frame
ab-prop-frame-depth = Frame depth
ab-prop-operation = Operation
ab-prop-hand = Hand
ab-prop-facing = Opens to
ab-prop-insert-wall = Wall
ab-op-single-swing = Single swing
ab-op-fixed = Fixed
ab-op-casement = Casement
ab-hand-left = Left
ab-hand-right = Right
ab-notice-inserts-deleted = { $count ->
    [one] The wall went with 1 door or window in it, in one step. Undo brings it back.
   *[other] The wall went with { $count } doors and windows in it, in one step. Undo brings them all back.
}
ab-notice-floor-edges-frozen = { $count ->
    [one] 1 floor edge lost its wall and stays where it was. Undo brings the wall and the association back.
   *[other] { $count } floor edges lost their wall and stay where they were. Undo brings the wall and the association back.
}
ab-tool-floor = Floor
ab-tool-floor-hint = Create a floor by picking walls or sketching its boundary on the selected storey
ab-kind-floor = Floor
ab-prop-top-offset = Level offset
ab-prop-floor-area = Area
ab-prop-floor-volume = Volume
ab-prop-floor-walls = Associated walls
ab-prop-floor-storey = Storey
ab-notice-ifc-exported = Exported { $name } ({ $count } elements)
ab-notice-ifc-loss = { $count ->
    [one] The loss report lists 1 item that is missing from the file or only approximated.
   *[other] The loss report lists { $count } items that are missing from the file or only approximated.
}
ab-export-ifc-title = Export IFC
ab-export-ifc-filter = IFC files (*.ifc)

ab-notice-modify-dissociated = { $count ->
    [one] One floor edge no longer follows a wall and stays where it is.
   *[other] { $count } floor edges no longer follow a wall and stay where they are.
}

ab-menu-modify = Modify
ab-tool-move = Move
ab-tool-move-hint = Move the selected objects (M); select first, or pick the command and then select and press Enter
ab-tool-copy = Copy
ab-tool-copy-hint = Copy the selected objects, as many times as you like (CO); Enter ends
ab-tool-rotate = Rotate
ab-tool-rotate-hint = Rotate the selected objects about a base point (RO)
ab-tool-mirror = Mirror
ab-tool-mirror-hint = Mirror the selected objects across a line (MI)
ab-prop-objects = { $count } objects
ab-notice-modify-unjoined = { $names } could not be reconnected to the moved wall and stay open.
ab-notice-modify-meets = "{ $name }" now opens into the wall "{ $other }".
ab-notice-picked-point = Point: { $x }, { $y }

ab-tool-room = Room
ab-tool-room-hint = Place rooms by clicking inside regions closed by walls
ab-tool-room-all = All rooms
ab-tool-room-all-hint = Place a room in every enclosed region of the storey that has none
ab-kind-room = Room
ab-prop-department = Department
ab-prop-floor-finish = Floor finish
ab-prop-wall-finish = Wall finish
ab-prop-ceiling-finish = Ceiling finish
ab-prop-base-finish = Base finish
ab-prop-upper-constraint = Upper limit
ab-prop-upper-offset = Upper offset
ab-prop-point-x = Point X
ab-prop-point-y = Point Y
ab-prop-room-area = Area
ab-prop-room-perimeter = Perimeter
ab-prop-room-volume = Volume
ab-prop-room-state = State
ab-room-state-enclosed = Enclosed
ab-room-state-unenclosed = Not enclosed
ab-room-state-redundant = Redundant
ab-room-upper-default = Level above (3 m if none)
ab-room-tag-unenclosed = (not enclosed)
ab-room-tag-redundant = (redundant)
ab-room-tags-hidden = { $count } room tags not shown — zoom in

ab-dim-edit-prompt = Set dimension
ab-dim-edit-label = Length
ab-dim-edit-invalid = That is not a length. Type a number, with a unit if you like.
ab-menu-enter = Enter
ab-menu-cancel = Cancel
ab-menu-undo-point = Undo last point
ab-menu-repeat = Repeat last command
ab-menu-paste = Paste
ab-menu-select-all = Select all
ab-menu-zoom-extents = Zoom extents
ab-menu-unhide-all = Unhide all
ab-menu-delete = Delete
ab-menu-move = Move
ab-menu-copy = Copy
ab-menu-rotate = Rotate
ab-menu-mirror = Mirror
ab-menu-copy-clipboard = Copy to clipboard
ab-menu-hide = Hide
ab-menu-isolate = Isolate
ab-menu-hide-kind = Hide this kind
ab-menu-isolate-kind = Isolate this kind
ab-menu-select-kind = Select all of this kind
ab-menu-keep-walls = Keep only walls
ab-menu-keep-doors = Keep only doors
ab-menu-keep-windows = Keep only windows
ab-menu-keep-floors = Keep only floors
ab-menu-keep-rooms = Keep only rooms
ab-menu-properties = Properties
ab-banner-hidden = Hidden: { $count }
ab-banner-isolated = Isolated: { $count }
ab-banner-reset = right-click, Unhide all, to end
ab-snap-label = Snap
ab-snap-hint = Snaps to end points, midpoints, perpendiculars and centers (F3)
ab-setting-shortcuts = Typed command shortcuts: XX=command pairs separated by ;
ab-setting-snap-enabled = Snap to points while drawing
ab-setting-snap-grid = Snap grid step in millimeters
ab-panel-settings = Settings
ab-notice-copied = { $count ->
    [one] Copied 1 object to the clipboard.
   *[other] Copied { $count } objects to the clipboard.
}
ab-notice-pasted-storey = Pasted into the storey.
ab-notice-shortcuts-exported = Shortcuts written to { $path }.
ab-notice-shortcuts-imported = Imported { $count } shortcuts.
ab-notice-selected-id = Selected { $count } objects by identifier.
ab-notice-snap = Snap is { $state }.
ab-rename-room = Room name
ab-menu-import = Import
ab-shortcuts-export-title = Export shortcuts
ab-shortcuts-import-title = Import shortcuts
ab-shortcuts-filter = Shortcut files (*.json)
ab-state-on = on
ab-state-off = off
ab-menu-cut = Cut
ab-notice-paste-on-top = Warning: the pasted objects lie on top of their originals, so their doors and windows and rooms overlap them.
ab-kind-stair = Stair
ab-prop-stair-shape = Shape
ab-prop-stair-turn = Turns to
ab-prop-stair-tread = Tread depth
ab-prop-stair-max-riser = Highest riser
ab-prop-stair-waist = Waist thickness
ab-prop-stair-railing-left = Railing on the left
ab-prop-stair-railing-right = Railing on the right
ab-prop-stair-direction = Direction
ab-prop-stair-risers = Risers
ab-prop-stair-riser = Riser height
ab-prop-stair-runs = Run lengths
ab-prop-stair-stride = Stride (2R + T)
ab-prop-stair-volume = Volume
ab-prop-stair-storey = Storey
ab-stair-shape-straight = Straight
ab-stair-shape-l-turn = L turn
ab-stair-shape-u-turn = U turn
ab-stair-top-default = Level above
ab-stair-up = UP
ab-tool-stair = Stair
ab-tool-stair-hint = Create a stair from this storey up to the level above (ST)
ab-menu-keep-stairs = Keep only stairs
ab-menu-new-stair = New stair
ab-notice-room-slivers = { $count ->
    [one] 1 region smaller than { $least } m² got no room: it is a sliver between walls.
   *[other] { $count } regions smaller than { $least } m² got no room: they are slivers between walls.
}
ab-stair-down = DN

ab-tool-type-hint = Type: the type the running tool places next, or the type of the selected walls, floors, doors or windows
ab-panel-types = Types
ab-prop-type = Type
ab-op-double-swing = Double leaf
ab-op-sliding = Sliding
ab-loc-core-center = Core center
ab-loc-core-exterior = Core exterior face
ab-loc-core-interior = Core interior face
ab-menu-create-similar = Create Similar
ab-menu-match-type = Match Type
ab-menu-edit-type = Edit Type
ab-notice-match-source = Match type: click the object whose type the others should get. Escape ends.
ab-notice-match-targets = Match type: click objects to give them the type "{ $type }". Escape ends.
ab-notice-no-typed = That object has no type: click a wall, floor, door or window.
ab-notice-types-added = { $count ->
        [one] One object of this older document got a type
       *[other] { $count } objects of this older document got types
    } matching their size. Saving keeps them.
ab-layer-structure = Structure
ab-layer-substrate = Substrate
ab-layer-thermal = Thermal or air
ab-layer-membrane = Membrane
ab-layer-finish = Finish
ab-types-kind-wall = Wall types
ab-types-kind-floor = Floor types
ab-types-kind-door = Door types
ab-types-kind-window = Window types
ab-types-type = Type
ab-types-name = Name
ab-types-rename = Rename
ab-types-duplicate = Duplicate
ab-types-delete = Delete
ab-types-replace-with = Replace with
ab-types-in-use = { $count ->
        [0] Not used
        [one] Used by one object
       *[other] Used by { $count } objects
    }
ab-types-exterior = Exterior
ab-types-interior = Interior
ab-types-load-bearing = Load-bearing
ab-types-room-bounding = Bounds rooms
ab-types-layers = Layers
ab-types-core = Core
ab-types-function = Function
ab-types-material = Material
ab-types-thickness-mm = Thickness (mm)
ab-types-add-layer = Add layer
ab-types-total = Total thickness: { $mm } mm
ab-types-apply = Apply
ab-types-revert = Revert
ab-types-width-mm = Width (mm)
ab-types-height-mm = Height (mm)
ab-types-frame-width-mm = Frame width (mm)
ab-types-frame-depth-mm = Frame depth (mm)
ab-types-operation = Operation
ab-types-sill-mm = Default sill height (mm)
ab-types-number-invalid = { $layer ->
        [0] Type each size as a number of millimeters, 0 or more.
       *[other] Type the thickness of layer { $layer } as a number of millimeters.
    }
ab-types-core-gap = The core layers must follow one another: mark one run of layers as the core.
ab-types-exterior-side = Exterior side on the left
ab-types-top-side = Top on the left
ab-types-empty = The document has no type of this kind.
ab-menu-select-type = Select all of this type
ab-notice-types-resynced = { $count ->
        [one] One object disagreed with its type in this file and took its type's values again
       *[other] { $count } objects disagreed with their types in this file and took their types' values again
    }. Saving keeps the correction.

## Editing tools, pin and the warnings panel

ab-menu-align = Align
ab-menu-trim = Trim to corner
ab-menu-split = Split wall
ab-menu-offset = Offset
ab-menu-array = Array
ab-menu-pin = Pin
ab-menu-unpin = Unpin
ab-tool-align = Align
ab-tool-align-hint = Align an element's face or edge to a reference line (AL)
ab-tool-offset = Offset
ab-tool-offset-hint = Move or copy the selected walls and floors across by a distance (OF)
ab-tool-trim = Trim
ab-tool-trim-hint = Trim or extend two walls to a corner (TR)
ab-tool-trim-to = Trim to boundary
ab-tool-trim-to-hint = Trim or extend walls to a boundary line, one at a time (TB); several at once: TM
ab-tool-split = Split
ab-tool-split-hint = Split a wall at a point, optionally with a gap (SL)
ab-tool-merge = Merge
ab-tool-merge-hint = Merge two collinear walls that touch into one (MG)
ab-tool-array = Array
ab-tool-array-hint = Copy the selection in a row (AR)
ab-tool-array-radial = Radial array
ab-tool-array-radial-hint = Copy the selection around a center (RA)
ab-tool-pin = Pin
ab-tool-pin-hint = Pin the selected elements so commands cannot move or delete them (PN)
ab-tool-unpin = Unpin
ab-tool-unpin-hint = Unpin the selected elements (UP)
ab-prop-pinned = Pinned
ab-panel-warnings = Warnings
ab-warnings-count = { $count ->
    [0] No warnings
    [one] 1 warning
   *[other] { $count } warnings
}
ab-warnings-none = Nothing to show.
ab-warnings-hint = Double-click a row to select its elements and zoom to them.
ab-warnings-kind-all = All kinds
ab-warnings-kind-stair = Stairs
ab-warnings-kind-room = Rooms
ab-warnings-kind-wall = Walls
ab-warnings-kind-floor = Floors
ab-warnings-kind-insert = Doors and windows
ab-warnings-kind-type = Types
ab-warnings-severity-error = Error
ab-warnings-severity-warning = Warning
ab-warnings-count-filtered = { $count ->
    [one] { $shown } of 1 warning
   *[other] { $shown } of { $count } warnings
}
