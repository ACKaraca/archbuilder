# Messages owned by archbuilder-io: the export command, its errors and the loss reasons of the
# IFC export (crates/archbuilder-io/src).

ab-io-command-export-ifc-label = IFC…
ab-io-command-export-ifc-description = Writes the document as an IFC4 file to a path, with a loss report.
ab-io-no-structure = The document has no project, site and building to export.
ab-io-loss-object-not-exported = An object could not be read and is not in the IFC file.
ab-io-loss-body-not-exported = An element is in the IFC file without its shape.
ab-io-loss-quantity-not-exported = A quantity of an element is left out of the IFC file.
ab-io-loss-window-operation = A casement window is written as a single panel: IFC4 has no operation on the window itself, so its operation, hand and facing are not in the file.
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
ab-io-loss-curtain-panel-filled = A curtain wall has { $count } doors or windows as panels: each is written as a door or window that fills an opening in the curtain wall, not as a part of it.
