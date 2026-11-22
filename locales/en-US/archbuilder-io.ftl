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
ab-io-loss-stair-not-written = A stair is not in the IFC file: stairs, flights and railings cannot be written yet. The shaft opening it cuts in the floor above is in the file.
