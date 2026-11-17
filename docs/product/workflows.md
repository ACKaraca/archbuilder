# Reference workflows

These workflows are end-to-end scenarios used as acceptance tests for milestones. Each is written
in ArchBuilder's vocabulary and ends with observable acceptance results that reference capability
identifiers.

The reference fixtures are synthetic documents created with the product's own commands:

- **Housing fixture A:** two buildings on one site, each with eight storeys of different
  floor-to-floor heights and a basement, two vertical cores per building, repeated apartment units,
  pitched and flat roofs, curved and straight walls.
- **Housing fixture B:** a two-storey duplex with a stair, one curtain wall, mirrored doors and a
  sloped site, small enough to inspect by hand.

## WF-01 Set up a housing project

1. Create a project from the housing template (AB-DATUM-001) and fill in project information
   (AB-DATUM-002).
2. Set display units to millimeters and square meters (AB-DATUM-004).
3. Record the coordinate reference system, survey point map coordinates and true north
   (AB-DATUM-014, AB-DATUM-015, AB-DATUM-016).
4. Create two buildings on one site (AB-DATUM-003) and, per building, levels with storey flags and
   plan views (AB-DATUM-007).
5. Draw the structural grid with numeric and alphabetic labels and pin it (AB-DATUM-009,
   AB-DATUM-022).

**Acceptance**

- The spatial tree shows one site, two buildings and their storeys with the elevations entered.
- A coordinate query at a grid intersection reports internal, project, survey and map coordinates
  consistent with the entered georeference (AB-DATUM-018).
- An IFC 4.3 export of the empty project contains the spatial structure and map conversion read back
  by an independent reader (AB-IO-003).

## WF-02 Model the shell of a multi-storey block

1. Define exterior and party wall types with layers and core boundaries (AB-WALL-003, AB-WALL-004).
2. Place exterior walls by finish face and interior walls by centerline (AB-WALL-001, AB-WALL-002),
   with top constraints to the level above (AB-WALL-005).
3. Create floors by picking walls (AB-SLAB-001, AB-SLAB-002) and a hip roof with a courtyard
   (AB-SLAB-008, AB-SLAB-010); bind top-storey walls to the roof as parapets (AB-WALL-006).
4. Place doors and windows from families and opening recipes (AB-OPEN-001, AB-OPEN-009).
5. Change a level elevation and the exterior wall type thickness.

**Acceptance**

- Every wall bound to the moved level changes height in the same transaction; walls, floors and
  hosted inserts remain joined and cut correctly (AB-DATUM-007, AB-WALL-007, AB-OPEN-003).
- The thicker wall grows away from its location line and inserts keep their reference face
  (AB-WALL-002, AB-OPEN-003).
- One undo restores both the elevation and all dependent geometry (AB-DATUM-030).
- The warnings register is empty, or lists only intended conditions (AB-DATUM-031).

## WF-03 Design a curtain wall facade

1. Create a curtain wall type with maximum-spacing vertical and fixed-distance horizontal grids
   (AB-WALL-020) and mullion types including corner mullions (AB-WALL-022).
2. Place it inside a basic wall so it embeds (AB-WALL-024).
3. Replace a ground-row panel with a door family and override two panels to solid (AB-WALL-021).
4. Add and remove grid segments (AB-WALL-023), then lengthen the curtain wall.

**Acceptance**

- Overridden panels keep their cell identity and type after the length change.
- The host wall's opening follows the curtain wall.
- A schedule lists panels by type with width, height and area (AB-WALL-027).

## WF-04 Author and load a door family

1. Start from the wall-hosted door template (AB-FAM-004) in a family editing context (AB-FAM-005).
2. Add reference planes and label width and height dimensions with parameters (AB-FAM-006,
   AB-FAM-009); add a frame depth expression (AB-PARAM-010).
3. Model frame and leaf with solids and a host-cutting void (AB-FAM-007, AB-FAM-021); add plan swing
   symbols visible only in plan (AB-FAM-011, AB-FAM-012).
4. Bind material slots and subcategories (AB-FAM-013, AB-FAM-014).
5. Define test cases for minimum, typical and maximum sizes and run them (AB-FAM-010).
6. Publish to a library package and load it into the project (AB-FAM-031, AB-FAM-030).

**Acceptance**

- All test cases pass; a deliberately conflicting constraint is reported with the constraints
  involved (AB-FAM-008).
- Placed doors cut walls of different thicknesses and show swing symbols in plan only.
- Publishing a new version does not change the project until the update is previewed and applied
  (AB-FAM-029).
- The door exports as an IFC door with type, operation and void and fill relations (AB-FAM-024,
  AB-IO-003).

## WF-05 Vertical circulation through all storeys

1. Create a U-shaped stair by components between the basement and ground level (AB-CIRC-001) with
   type rules (AB-CIRC-002).
2. Repeat it to the top storey (AB-CIRC-005) and let it own its shaft opening (AB-CIRC-008).
3. Accept the automatic railings and add wall-return extensions (AB-CIRC-010, AB-CIRC-012).
4. Add an elevator serving every level except the roof (AB-CIRC-014) and a ramp at the entrance
   (AB-CIRC-009).
5. Run the headroom check (AB-CIRC-007).

**Acceptance**

- Each repeated stair has a riser count matching its own floor-to-floor height and no rule
  violations, or a warning naming the violated rule.
- Shaft openings cut every floor the stair passes; moving the source stair moves every member and
  opening.
- Plans show cut lines, dashed upper portions and walking lines on both connected levels
  (AB-CIRC-006).
- The headroom report lists the minimum clearance and its location per run.

## WF-06 Rooms, areas and repeated apartments

1. Model one apartment with walls, doors, furniture (AB-OPEN-015) and rooms (AB-SPACE-001,
   AB-SPACE-005).
2. Group the apartment (AB-PHASE-010) and place it on every typical storey by level mapping
   (AB-PHASE-011), with room numbers as permitted exceptions (AB-PHASE-012).
3. Create gross and net area plans (AB-SPACE-010) and a color scheme by department (AB-SPACE-014).
4. Load the space program and compare (AB-SPACE-013).
5. Edit the group definition by moving one interior wall.

**Acceptance**

- Every instance updates; room numbers set as exceptions are unchanged.
- Room areas update in all instances and no room reports a stale area (AB-SPACE-004).
- The area report gives totals per level and the net-to-gross ratio (AB-SPACE-012).
- The program comparison lists deviations by department.

## WF-07 Produce a construction document set

1. Create plan, section, elevation and callout views (AB-VIEW-001 to AB-VIEW-004) bound to view
   templates (AB-VIEW-011) with rule-based filters for fire-rated walls (AB-VIEW-010).
2. Dimension and tag (AB-ANNO-002, AB-ANNO-009), add keynotes (AB-ANNO-011) and text.
3. Create sheets with title blocks (AB-SHEET-001), place and align viewports (AB-SHEET-002,
   AB-SHEET-003), and place door and room schedules (AB-SHEET-006).
4. Add revision clouds and issue a revision (AB-ANNO-013, AB-SHEET-008).
5. Publish the sheet set to PDF and DWG (AB-SHEET-009, AB-SHEET-011, AB-SHEET-012).

**Acceptance**

- Section markers show the correct sheet and detail numbers after viewports move (AB-ANNO-014,
  AB-SHEET-005).
- The PDF measures true scale on a 100 mm reference line, has bookmarks per sheet and embedded
  fonts, and includes Turkish characters correctly.
- The DWG files open in an independent reader with mapped layers.
- All sheets come from one recorded document revision (AB-SHEET-010).

## WF-08 Schedules, takeoff and spreadsheet round trip

1. Create door, window and room finish schedules with calculated fields (AB-SCHED-001,
   AB-SCHED-002) and conditional formatting for missing fire ratings (AB-SCHED-004).
2. Create a material takeoff by layer (AB-SCHED-013) with carbon factors on materials
   (AB-SCHED-014).
3. Export the door schedule to XLSX, edit marks and fire ratings externally, and import it back
   (AB-SCHED-016).
4. Edit a door width in the schedule (AB-SCHED-003).

**Acceptance**

- The import preview lists each changed value and rejects edits to computed cells; applying it is
  one undoable command.
- The width edit changes the door in every view.
- Takeoff volumes match the quantity rules used for IFC quantity sets (AB-SCHED-012).

## WF-09 Details in the three editing contexts

1. In a wall section callout, add detail components, a repeating brick detail and dimensions to the
   view's detail layer (AB-ANNO-016, AB-ANNO-017).
2. Detach the callout into a detached drawing and refine it with drafting tools (AB-ANNO-023).
3. Change the wall type's insulation thickness in the model.
4. Delete a window that a dimension in the detail layer referenced.

**Acceptance**

- The model view's projection and anchored detail items follow the new thickness (AB-ANNO-019).
- The detached drawing is unchanged and shows that its source changed since its recorded revision
  (AB-ANNO-024).
- The dimension to the deleted window is shown as orphaned and is not rebound to another element
  (AB-ANNO-020).
- All three contexts can be placed on one sheet.

## WF-10 Renovation of an existing building

1. Link a point cloud of the existing building (AB-LINK-006) and model existing walls and floors in
   the existing phase (AB-PHASE-001).
2. Demolish selected walls in the new construction phase (AB-PHASE-003).
3. Model new work and set display rules for existing, demolition and new plans (AB-PHASE-002).
4. Schedule demolished walls by phase (AB-SCHED-010).

**Acceptance**

- Demolition plans show demolished elements with their override and new elements hidden; new plans
  show existing and new only.
- Rooms in the new phase are bounded only by elements that exist in that phase (AB-PHASE-004).
- Hosted windows in demolished walls are demolished with them.

## WF-11 Compare design options

1. Create an option set for the ground-floor layout with two options (AB-PHASE-006, AB-PHASE-007).
2. Generate a third option with a node graph that varies apartment widths (AB-AUTO-009).
3. Compare the options by area, unit count and carbon (AB-PHASE-009).
4. Accept the chosen option (AB-PHASE-008).

**Acceptance**

- Views show the option chosen per view; the main model never joins to a hidden option.
- The comparison table matches schedules filtered by option.
- After acceptance only the chosen option's elements remain, merged into the main model, and the
  operation can be undone.

## WF-12 Coordinate with consultants

1. Link the structural consultant's IFC file as a reference model (AB-LINK-004) and a services DWG
   (AB-LINK-005).
2. Monitor grids and levels from the linked structural model (AB-LINK-011).
3. Run an interference check between beams in the reference model and ceilings in this document
   (AB-LINK-012) and export results as BCF (AB-LINK-013).
4. Receive an updated IFC file and refresh the reference.

**Acceptance**

- The refresh reports changed elements and raises coordination items for moved grids.
- Accepting an item moves the local grid; rejecting it records the decision with a comment.
- BCF issues opened in Arc Review restore the same viewpoint and elements.

## WF-13 Deliver checked IFC from continuous integration

1. Configure IFC mapping and property sets for the client's requirements (AB-IO-006,
   AB-PARAM-018) and classifications (AB-PARAM-016).
2. Load the client's IDS file and validate in the application (AB-IO-009).
3. Run the CLI in a CI job that validates the document against the IDS and rule sets, exports IFC
   4.3 and publishes PDFs (AB-AUTO-003, AB-AUTO-014, AB-SHEET-015).
4. Re-import the exported IFC into a copy of the document (AB-IO-008).

**Acceptance**

- The CI job fails with a JUnit-style report when a required property is missing and passes after
  the fix.
- The IFC file passes the independent reader's schema validation; no element is exported as a proxy
  without being listed in the loss report (AB-IO-017, AB-IO-019).
- Re-import matches every element by GlobalId and creates no duplicates.

## WF-14 Share, compare and merge through Arc Hub

1. Save the project to an Arc Hub project and check it out (AB-COLLAB-001, AB-COLLAB-002).
2. Work offline, save two versions locally, reconnect and upload.
3. Compare the latest version with the previous one (AB-COLLAB-004) and write both as canonical
   text for review (AB-COLLAB-005).
4. Create a branch for a facade study, edit on both lines, and merge (AB-COLLAB-007).

**Acceptance**

- Versions created offline appear in Hub in order with their comments (AB-COLLAB-003).
- The comparison lists added, removed and modified elements with changed parameters; the text diff
  shows the same elements.
- Merging edits to different elements completes automatically; an edit to the same element on both
  lines is presented as a conflict with both states kept (AB-COLLAB-013).

## WF-15 Round trip with a native project file of another application

1. Export housing fixture A to an exchange package for a connector (AB-CONNECT-002) and apply it in
   the other application.
2. Edit wall types and room names there and move one storey's grids; edit a different wall type
   here.
3. Re-import the package (AB-CONNECT-001) and review the change list (AB-CONNECT-012).
4. Repeat the round trip twice more (AB-CONNECT-011).

**Acceptance**

- No element is duplicated across three round trips and no element identifier changes
  (AB-CONNECT-010).
- The change list contains exactly the edited elements; an element edited on both sides is a
  conflict with both states kept (AB-CONNECT-013).
- Parameters that only the other application understands are written back unchanged (AB-IO-018).
- The loss report names every unmapped family and the fidelity level used (AB-CONNECT-015,
  AB-CONNECT-016).

## WF-16 Massing and review on a tablet

1. Open the site document on a tablet, sketch a massing option with the pen and push and pull its
   faces (AB-TOUCH-004, AB-TOUCH-005).
2. Go offline, walk the site with the model aligned to the device position, and record two issues
   with photos and pen markup (AB-TOUCH-012, AB-TOUCH-011).
3. Reconnect and synchronize through Arc Hub (AB-TOUCH-016); open the document in the browser build
   (AB-TOUCH-017).

**Acceptance**

- The mass and its areas on the tablet equal those of the desktop build for the same commands
  (AB-TOUCH-001).
- Everything done offline is present after synchronization; a conflicting edit keeps both states
  (AB-TOUCH-016).
- The issues open in the desktop build with viewpoints, photos and markup (AB-TOUCH-011).
