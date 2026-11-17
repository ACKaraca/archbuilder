# Glossary

The vocabulary used across ArchBuilder documents, one entry per term.

| Term | Meaning |
|---|---|
| Anchor | The model or view reference an annotation is attached to: element role, persistent face or edge reference, datum, linked element, view coordinate or drafting entity |
| Area scheme | A named way of measuring areas (gross, rentable, net or user-defined) with its own area plans |
| Arc document package | The suite's single-file SQLite document format; ArchBuilder documents use the `.archb` extension |
| Arc Scene | The suite contract for renderable scenes exchanged with RenderBuilder |
| Assembly | A named collection of elements treated as one deliverable unit with its own views and schedules |
| Attachment | A wall's top or base following the surface of a floor, roof, ceiling or plane |
| Bounded expression language | The typed, unit-checked formula language for parameters and schedule fields, with no loops or external access |
| Canonical text form | A deterministic text serialization of a document's semantic records for comparison and review |
| Category | The fixed classification of an element (wall, door, room) that sets its behavior, graphics and default IFC entity |
| Change set | The transactions of one user exchanged during multi-user editing |
| Child view | A partial view sharing its parent's settings and detail layer, with its own crop |
| Coordination item | A recorded change of a monitored linked element awaiting a decision |
| Core boundary | The division in a layered type between structural core layers and finish layers |
| Curtain wall | A wall made of a grid of panels and mullions |
| Datum | A level, grid or reference plane |
| Datum extent volume | A named box limiting where assigned datums appear |
| Design option | An alternative set of elements inside one document, one of an option set |
| Detached drawing | An independent drafting drawing created from a view's projection, recording its source and never writing back |
| Detail layer | The drafting content owned by one model view, stored separately from the model |
| Detail level | Low, medium or high graphic detail of a view |
| Display rule | The rule that decides how elements of each phase status appear in a view |
| Document variable | A project-wide named value that drives parameters and dimensions |
| Dormer | An assembly of a small roof, its walls and the cut in a host roof |
| Drafting view | A model-independent 2D view drawn with the drafting engine |
| Element | A record in the model with an identity, such as a wall, door, room or view |
| Family | A definition of an element kind with typed parameters, types and geometry recipes |
| Family test case | A stored parameter set a family is evaluated against before publishing |
| Geometry recipe | A declarative form (extrusion, blend, revolve, sweep, swept blend, array or node graph) in a family |
| Group | A reusable set of elements whose instances share content |
| Host | The element an insert depends on and cuts, such as a wall for a door |
| Identity map | The record of correspondence between native identifiers and identifiers in other formats |
| Insert | A hosted family instance such as a door or window |
| Layered type | A wall, floor, roof or ceiling type made of ordered layers with function, material and thickness |
| Level | A named horizontal datum at an elevation; a storey level creates a storey |
| Library package | A versioned file of family definitions with a manifest, license and checksums |
| Library parameter | A parameter definition with a stable identifier shared across documents |
| Link | A reference to another document or file displayed in this document without copying it |
| Live dimension | A transient dimension shown on selection that moves the element when edited |
| Location line | The line of a wall that stays fixed when its thickness changes |
| Loss report | The structured record of what an import or export preserved, approximated and could not carry |
| Model view | A plan, section, elevation, callout or 3D view projected from the model |
| Monitored element | A local element compared against a linked element for changes |
| Opening recipe | A built-in parametric door or window definition usable without a family |
| Orphaned annotation | An annotation whose anchor no longer resolves; it keeps its position and shows no trusted value |
| Part | A piece of a layered element split per layer for scheduling and fabrication |
| Persistent reference | A reference to a face or edge by element, generator face role and source edge, stable across edits |
| Phase | A stage of construction; elements are created and optionally demolished in phases |
| Plan range | The top, cut plane, bottom and depth that control what a plan shows |
| Preserved foreign data | Imported content that has no native counterpart, kept for round trip |
| Reference plane | A named construction plane used for placement, constraints and dimensions |
| Room | A space bounded by room-bounding elements with computed area, perimeter and volume |
| Room-bounding | The flag that makes an element part of room and area boundaries |
| Rule set | A set of model checks (IDS, parameter, naming, geometric) run in the application or headless |
| Schedule | A live table of model data that edits elements when cells change |
| Section volume | A box that clips a 3D view |
| Sheet | A drafting layout in paper units with viewports, title block and revisions |
| Storey | A building level that contains elements, mapped to an IFC building storey |
| System family | A built-in element kind whose types live in documents |
| Type | A named set of parameter values shared by instances |
| View template | A named set of view settings that controls bound views |
| Viewport | The placement of a view on a sheet |
| Work area | A named partition of a shared document for loading, visibility and ownership |
| Work plane | The plane on which placement and sketching happen |
