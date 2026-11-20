# Families and family editor

This area specifies the kinds of families, the family editor, geometry recipes, visibility rules,
nesting, annotation families, and versioned library packages. The definition model, the expression
language and package format are described in
[families.md](../../architecture/families.md); parameters are specified in
[09-parameters-and-constraints.md](09-parameters-and-constraints.md).

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Family definition | Stable ID, version, category, hosting kind, behavior flags, parameters, types, reference planes, geometry recipes, symbolic geometry, nested families | Pinned by every document that uses it; published in library packages |
| Family type | Name, parameter values | Belongs to one family definition; referenced by instances |
| Geometry recipe | Form kind (extrusion, blend, revolve, sweep, swept blend), solid or void, sketch, constraints, visibility condition, subcategory, material slot | Evaluated per type and instance |
| Visibility condition | View kinds, detail levels, boolean expression | Attached to a recipe or symbolic element |
| Nested family use | Nested family reference, parameter associations, shared flag | Places one family inside another |
| Connector | Domain, flow direction, system classification, size parameters, position | Element of a family definition read by service and structural areas |
| Library package | Manifest (name, version, license, source, checksum, dependencies), definitions, assets | Source of family definitions |

## Family kinds

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-FAM-001 | System families | Provides built-in element kinds (walls, floors, roofs, ceilings, stairs, ramps, railings, curtain systems, levels, grids, terrain, annotation styles) whose types live in documents and are copied between documents. System family behavior is fixed; their types are fully parameterized. | T1 | 2 |
| AB-FAM-002 | Loadable families | Defines families with category, hosting kind, typed parameters, types, geometry recipes and symbolic representations, authored in the family editor. A definition is stored in every document that uses it and in library packages. | T1 | 2 |
| AB-FAM-003 | In-place elements | Authors a one-off element in project context with the family editor's tools, assigned a category. It can be extracted later as a loadable family, which replaces the in-place element with an instance. | T2 | 2 |
| AB-FAM-004 | Family templates | Starts a definition from a template per category and hosting kind that provides reference planes, default parameters and representative host geometry for testing. | T1 | 2 |

## Family editor

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-FAM-005 | Family editing context | Opens a family definition as its own editing context with its own undo history, from a library or from a document's instance. Committing loads the new version into the originating document and reports affected instances. | T1 | 2 |
| AB-FAM-006 | Family reference planes | Uses named reference planes and lines with strength and orientation roles (left, right, front, back, top, bottom, center, origin). Roles define the insertion origin and the references offered for dimensions and alignment in projects. | T1 | 2 |
| AB-FAM-007 | Solid and void forms | Creates extrusions, blends, revolves, sweeps along sketched or picked paths, and swept blends, as solids or voids. Voids subtract from the family's solids and, when the family is set to cut its host, from the host. | T1 | 2 |
| AB-FAM-008 | Sketch constraints | Solves dimensional and geometric constraints in family sketches with the same solver as project constraints, and shows each sketch as under-, fully or over-constrained. Over-constrained edits are rejected with the conflicting constraints listed. | T1 | 2 |
| AB-FAM-009 | Parameter-driven geometry | Labels dimensions with parameters, adds equality constraints and locks alignments, so geometry follows parameter values. | T1 | 2 |
| AB-FAM-010 | Parameter test cases | Stores named parameter sets (each type plus edge cases) and evaluates the family against all of them on demand and before publishing. Unsolvable sketches, invalid solids and failed expressions are reported per test case. | T1 | 2 |
| AB-FAM-011 | Visibility conditions | Shows each form or symbolic element only in chosen view kinds (plan, ceiling plan, front and back, left and right, 3D), at chosen detail levels (low, medium, high) and when a boolean expression is true. | T1 | 2 |
| AB-FAM-012 | Symbolic and model lines | Adds 2D symbolic lines that appear only in views parallel to their plane, 3D model lines, filled regions and masking regions for plan and elevation symbols. | T1 | 2 |
| AB-FAM-013 | Subcategories | Assigns forms and lines to subcategories of the family's category so views control their graphics independently. | T1 | 2 |
| AB-FAM-014 | Material slots | Binds forms to material slots driven by material parameters, so types and instances choose materials without editing geometry. | T1 | 2 |
| AB-FAM-015 | Nested families | Nests families inside a family and associates nested parameters with host parameters. A shared nested family is independently selectable, scheduled and tagged in projects. | T1 | 2 |
| AB-FAM-016 | Swappable components | Defines family-type parameters that choose among nested family types, so a type swaps a component without new geometry. | T2 | 2 |
| AB-FAM-017 | Arrays in families | Creates linear and radial arrays of forms or nested families with count and spacing driven by parameters. | T2 | 2 |
| AB-FAM-018 | Lookup tables | Embeds tables (CSV) in a family and reads values with a lookup function in expressions, with unit-checked columns and a fallback value. | T2 | 2 |
| AB-FAM-019 | Type tables | Lists types and their parameter values in a table stored with the family or beside it in a package; loading the family lets the user pick a subset of types. Unknown columns and values failing unit checks are reported. | T2 | 2 |
| AB-FAM-020 | Connectors | Places connectors with domain (duct, pipe, electrical, cable containment, structural), flow direction, system classification and size parameters, read by the service and structural areas. | T2 | 4 |
| AB-FAM-021 | Hosting and behavior flags | Sets hosting kind, cut-host-with-voids, always vertical, work-plane based, shared, room calculation point and plan representation behavior per family. | T1 | 2 |
| AB-FAM-022 | Node-graph geometry recipes | Lets a recipe include a node graph restricted to pure geometry nodes, evaluated under the same step and depth limits as expressions, for forms that sketches cannot express. | T3 | 3 |
| AB-FAM-023 | Adaptive point families | Defines families placed by picking a fixed number of points, with geometry rigged to those points. | T3 | 4 |
| AB-FAM-034 | Mesh-based families | Creates a loadable family from a glTF or OBJ mesh with its source, license, units and up axis recorded, a category and an optional plan symbol. Meshes that reference external resources are refused and element and triangle counts are checked before loading; a mesh that is not a closed volume reports no volume, and exports write it as tessellated geometry under the family's IFC assignment (AB-FAM-024). | T2 | 3 |
| AB-FAM-024 | IFC classification per family | Assigns an IFC entity and predefined type per family and per type, defaulting from the category. Exports use this assignment and never substitute a proxy silently. | T1 | 2 |

## Annotation, profile and detail families

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-FAM-025 | Annotation families | Authors tags, symbols, title blocks, view titles and section, elevation and callout heads, with label fields bound to parameters and formatted with units, prefixes and suffixes. | T1 | 2 |
| AB-FAM-026 | Multi-category tags | Lets one tag family serve several categories by reading library parameters common to them; it can be the default tag for those categories. | T2 | 2 |
| AB-FAM-027 | Profile families | Authors closed or open 2D profiles used by sweeps, mullions, railings, edges and bands. | T1 | 2 |
| AB-FAM-028 | Detail component families | Authors 2D components for detail layers and drafting views, including repeating components laid along a path. | T1 | 2 |

## Libraries and versions

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-FAM-029 | Family versions | Gives every definition a stable identifier and a version. A document pins the version it uses; a newer version in a library never changes a document until the user updates it. | T1 | 2 |
| AB-FAM-030 | Load and update | Loads families from packages or other documents, and updates a pinned definition to a newer version with a preview of affected instances, removed or renamed parameters, and values that will change. The update is one undoable command. | T1 | 2 |
| AB-FAM-031 | Library packages | Publishes and reads packages that carry a manifest (name, version, license, source, checksum, dependencies), family definitions and assets. Packages with failed checksums or missing dependencies are refused with a report. | T1 | 2 |
| AB-FAM-032 | Library search | Searches configured libraries by name, category, parameter value and classification, with previews, without loading the definition into the document. | T2 | 2 |
| AB-FAM-033 | Family extraction | Saves families from a document into a package with the version and source recorded. | T1 | 2 |
