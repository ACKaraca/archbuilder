# Messages owned by archbuilder-io: the export command, its errors and the loss reasons of the
# IFC export (crates/archbuilder-io/src).

ab-io-command-export-ifc-label = IFC…
ab-io-command-export-ifc-description = Writes the document as an IFC4 file to a path, with a loss report.
ab-io-no-structure = The document has no project, site and building to export.
ab-io-loss-object-not-exported = An object could not be read and is not in the IFC file.
ab-io-loss-body-not-exported = An element is in the IFC file without its shape.
ab-io-loss-quantity-not-exported = A quantity of an element is left out of the IFC file.
ab-io-loss-window-operation = A casement, awning, hopper, pivoting or folding window is written as a single or double panel: IFC4 has no operation on the window itself, so its operation, hand and facing are not in the file.
ab-io-path-required = No file name was given. Type archbuilder.file.export_ifc path=<file> in the command line.
ab-io-loss-room-unenclosed = A room is not in the IFC file: no closed loop of walls encloses it, so it has no region to write as a space.
ab-io-loss-room-redundant = A room is not in the IFC file: another room already holds its region, and each region is written as one space.
ab-io-command-export-shortcuts-label = Shortcuts…
ab-io-command-export-shortcuts-description = Writes the typed command shortcut table to a file, so it can be shared or restored.
ab-io-command-import-shortcuts-label = Shortcuts…
ab-io-command-import-shortcuts-description = Reads a typed command shortcut table from a file and uses it.
ab-io-loss-arc-faceted = An arc edge or an arched top is written as straight chords within two millimeters: IFC bodies here are faceted.
ab-io-loss-cut-not-applied = A shaft or vertical opening could not be cut out of the body of the element it passes through: the body is written whole and the opening is in the file as a void of the element.
ab-io-validation-unreadable = The IFC file was written but could not be read back, so it is incomplete.
ab-io-validation-missing = An object of the model is not in the IFC file under its identifier, so the file is incomplete.
ab-io-validation-duplicate-id = Two entities of the IFC file have the same global identifier, so the file is incomplete.
ab-io-validation-count = The IFC file holds { $found } elements but the export counted { $expected }, so the file is incomplete.
ab-io-loss-floor-edge-as-covering = IFC4 has no element for the edge of a slab: a floor edge is written as a covering of the type molding with its swept body.
ab-io-loss-floor-edge-orphan = A floor edge names edges that its floor no longer has: those edges are not in the file.
ab-io-loss-georeference-not-written = The geodetic reference of the project is not in the IFC file: the coordinate system and the position of the survey point cannot be written yet.
ab-io-loss-datum-not-written = A reference plane or work plane is not in the IFC file: it is a drawing aid, not a building element.
ab-io-loss-selection-set-not-written = A selection set is not in the IFC file: it is a list of elements, and groups cannot be written yet. Its elements are in the file.
ab-io-loss-grid-one-direction = A grid line is not in the IFC file: an IFC grid needs lines in two directions, and the grid lines of this building run in one.
ab-io-loss-grid-multi-segment = A grid line of several segments is not in the IFC file: an IFC grid axis can be written only as one line or one arc so far, and shortening it to its ends would misplace it.
ab-io-loss-curtain-panel-filled = A curtain wall has { $count } doors or windows as panels: each is written as a door or window that fills an opening in the curtain wall, not as a part of it.
ab-io-loss-elevator-not-written = An elevator is not in the IFC file: transport elements cannot be written yet. The openings its shaft cuts in the floors are in the file.
ab-io-loss-dormer-as-parts = IFC4 has no element for a dormer: it is written as the opening in its roof, a wall for each of its front and cheek walls and a roof for its own small roof.
ab-io-loss-fascia-as-covering = IFC4 has no element for a fascia: it is written as a covering of the type molding with its swept body.
ab-io-loss-gutter-not-written = A gutter is not in the IFC file: it belongs to a pipe segment of the type gutter, which cannot be written yet.
ab-io-loss-roof-edge-orphan = A roof edge element names edges that its roof no longer has or that define no slope: those edges are not in the file.
ab-io-loss-project-info-not-written = The project information could not be added to the IFC file: its long name, phase, addresses, actors and building properties are left out.
ab-io-loss-issue-date-not-written = The issue date of the project is not in the IFC file: IFC4 has no attribute for it.
ab-io-command-new-from-template-label = New From Template
ab-io-command-new-from-template-description = Starts the project of the open document from a template, an ordinary document: its project settings and information, site, building, storeys, levels, element types and plan views are copied under new identifiers, replacing the structure the document has. The template is not changed. The document must have no elements.
ab-io-command-transfer-standards-label = Transfer Standards
ab-io-command-transfer-standards-description = Copies element types and the display unit settings from another document file. A type whose name the document already has is kept, overwritten (when nothing uses it) or copied under a free name, per type; the result lists what was copied and what was left out.
