# Web and tablet

This area specifies the browser build and the tablet builds of ArchBuilder: concept massing with
touch and pen, viewing and marking up models, on-site review, offline documents and synchronization
through Arc Hub. They are targets of the same Rust code compiled to WebAssembly and to iOS and
Android with the same UI kit, not separate applications; the decision is
[suite ADR-0018](https://github.com/ACKaraca/arc-suite/blob/main/docs/adr/0018-web-and-mobile-targets.md).
Desktop applications remain Rust-only with no web view. Massing behavior is specified in
[28-concept-massing.md](28-concept-massing.md) and Hub behavior in
[16-collaboration.md](16-collaboration.md); this area adds the input model, the device targets and
the on-site workflows.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Target profile | Platform (browser, tablet), input modes, memory budget, command availability | Decides which commands and views are offered |
| Gesture binding | Gesture, tool, modifier state, typed-value prompt | Maps touch and pen input to the same commands as mouse and keyboard |
| Offline selection | Document, work areas, versions, size | Held on the device with its journal |
| Site alignment | Device position, heading, survey point, accuracy | Places the model for on-site review |
| Field markup | Pen strokes, text, photo, viewpoint, author | Stored as an issue or a markup in the document |

## Targets and input

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-TOUCH-001 | Browser and tablet builds | Builds the product's crates for the browser (WebAssembly) and for tablets with the same document engine, commands and UI kit. A document opened on any target is the same `.archb` document, and a target never changes the meaning of a command. | T3 | 5 |
| AB-TOUCH-002 | Target feature declaration | Lists for each target the commands and views it offers, disables the others with the reason shown, and never produces results that differ from the desktop build for the same command. | T3 | 5 |
| AB-TOUCH-003 | Touch navigation | Orbits, pans and zooms with one- and two-finger gestures, selects and multi-selects by tap and lasso, and rejects palm contact while a pen is in use. Every gesture has an equivalent pointer and keyboard action. | T3 | 5 |
| AB-TOUCH-004 | Touch and pen massing | Creates and edits masses by touch and pen: draws profiles on the ground plane or a face, pushes and pulls faces, moves, rotates and scales masses and groups, and applies typed values through an on-screen entry that accepts units and expressions. Uses the commands of [28-concept-massing.md](28-concept-massing.md). | T3 | 5 |
| AB-TOUCH-005 | Pen stroke recognition | Turns pen strokes into lines, arcs, rectangles, circles and polygons with a preview before commit, and keeps the stroke as drawn when the user declines the recognized shape. | T3 | 5 |
| AB-TOUCH-006 | Touch-sized snapping | Enlarges snap targets and shows each active inference as a visible indicator, with a magnifier for precise placement under the finger. | T3 | 5 |
| AB-TOUCH-007 | Live program metrics | Shows mass floor areas, program totals and target deviation while a mass is being edited, updated as the gesture proceeds. | T3 | 5 |
| AB-TOUCH-008 | Light model edits | Edits element parameters, types and selection sets, moves and copies elements, and deletes elements through commands on tablet and in the browser. Commands that need long-running generation, such as roof solving or full regeneration of large views, stay on the desktop build. | T3 | 5 |

## Viewing, markup and on-site review

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-TOUCH-009 | Model viewing | Opens a document in 3D, plan, section and elevation views, isolates storeys and categories, shows properties of the selected element, measures distances and areas, and applies saved views and sections. | T3 | 5 |
| AB-TOUCH-010 | Pen markup | Marks up views, sheets and 3D viewpoints with pen strokes, text and shapes that are stored as markups or issues and never change the model. | T3 | 5 |
| AB-TOUCH-011 | Field issues | Creates an issue from the current viewpoint with a photo, a location, pen markup and an assignee, in the BCF form shared with [15-links-and-coordination.md](15-links-and-coordination.md). | T3 | 5 |
| AB-TOUCH-012 | On-site alignment | Aligns the model with the device's position and heading using the document's survey point and shows the model relative to the device, with the position accuracy displayed. Location data stays on the device unless the user shares it. | T3 | 5 |
| AB-TOUCH-013 | Review sessions | Steps through saved views and issues in a review list, records decisions and comments, and marks each item reviewed, working with or without a connection. | T3 | 5 |

## Offline use and synchronization

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-TOUCH-014 | Offline documents | Keeps chosen documents, work areas and versions on the device with their journals, so every capability offered by the target works without a connection. | T3 | 5 |
| AB-TOUCH-015 | Partial loading on devices | Opens only the chosen work areas, buildings or storeys within the target's memory budget, with level-of-detail display for the rest, using the mechanism of AB-COLLAB-011. | T3 | 5 |
| AB-TOUCH-016 | Hub synchronization | Sends local change sets to Arc Hub and receives others' through the mechanism of AB-COLLAB-012. A conflict keeps both states for comparison and is never resolved by discarding work. | T3 | 5 |
| AB-TOUCH-017 | Open in the browser | Opens a document from a Hub project in the browser build without installation, with the document held in browser storage private to the origin and no content sent anywhere except to the Hub the user opened it from. | T3 | 5 |
| AB-TOUCH-018 | Accessible touch interface | Provides accessible names for every control, scalable interface size, and operation by touch, pen, pointer and keyboard on every target. | T3 | 5 |
