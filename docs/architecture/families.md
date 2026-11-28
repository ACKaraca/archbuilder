# Families

This document defines the family definition model: typed parameters, the bounded expression
language, geometry recipes, nested and shared families, versioning and library packages. Family
capabilities are specified in
[08-families-and-family-editor.md](../product/capabilities/08-families-and-family-editor.md).

## Design rule

A family is data, not a program. A definition declares parameters, constraints, geometry recipes and
visibility conditions; the family evaluator in `archbuilder-model` turns a definition plus parameter
values into solids, symbolic geometry and reported values. No family carries executable code, so a
family from any source can be loaded, evaluated, diffed and validated without trusting its author
(see [ADR 0002](../adr/0002-declarative-families.md)).

## Definition model

| Part | Content |
|---|---|
| Header | Stable family ID, version (major.minor.patch), name, category, subcategories, hosting kind, behavior flags, IFC entity and predefined type, template origin |
| Parameters | Parameter definitions (see below) in display order and groups |
| Types | Named parameter value sets |
| Reference planes and lines | Named, with strength and orientation roles; one marks the insertion origin per axis |
| Sketches | Planar sketches of lines, arcs, circles, ellipses and splines with dimensional and geometric constraints |
| Geometry recipes | Forms built from sketches (see recipes), each solid or void, with subcategory, material slot and visibility condition |
| Symbolic geometry | 2D lines and regions per view orientation, with visibility conditions |
| Nested uses | References to other family definitions with placement, parameter associations and shared flag |
| Connectors | Typed connection points with domain, direction, classification and size parameters |
| Tables | Embedded lookup tables (CSV with typed header) |
| Test cases | Named parameter sets evaluated before publishing |
| Provenance | Author, license, source package, checksum of the published definition |

The definition is stored as a typed record in each document that uses it and in packages in a
documented, line-oriented text serialization so that two versions can be compared with ordinary
diff tools.

## Typed parameters

| Field | Meaning |
|---|---|
| `id` | Stable identifier; a library parameter keeps its library identifier |
| `name` | Display name, unique within the family |
| `data_type` | One of the types of AB-PARAM-002, including quantity kind for measures |
| `scope` | Type or instance |
| `source` | Family-local, library (with library ID), or built-in |
| `value` | Literal or expression |
| `role` | Driving, reporting (measured, read-only) or derived (expression output) |
| `rule` | Optional minimum, maximum, allowed values, required |
| `group`, `order` | Presentation |

Rules:

- Instance parameters may override type defaults only if the family marks them as instance scope.
- A parameter used by a dimension label drives that dimension; a reporting parameter reads one.
- Family-type parameters hold a reference to a nested family type and are restricted to families of
  a declared category.

## Bounded expression language

Expressions appear in parameter values, document variables, schedule calculated fields, view filter
values and numbering formats. One implementation in `arc-solve` serves all of them.

| Aspect | Rule |
|---|---|
| Values | Typed quantities with units, numbers, integers, booleans, text, dates, references to family types |
| Operators | `+ - * / ^`, comparisons that do not chain, `&&`, `||`, `!`, conditional `if(condition, a, b)` evaluating only the chosen branch; `,` or `;` between arguments |
| Functions | Version 1, exactly: `abs`, `min`, `max`, `sqrt`, `pow`; `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`; `round`, `floor`, `ceil`, each with an optional increment; `if`; `len`, `sum`, `first`, `last`, `concat`, `format`; and `lookup(table, column, key, ..., fallback)` over a family's embedded tables. A call of any other function is refused at its name. The set has a version recorded in every definition and document ([ADR 0009](../adr/0009-parameters-expressions-and-family-documents.md)) |
| Units | Literals carry units (`900 mm`, `15 deg`); every operation is dimensionally checked; dimensionless results of trigonometric arguments require angles |
| References | Formula identifiers (ASCII, unique among the names a formula can see) of parameters of the same family or its host (for nested associations), document variables, and in schedules the fields of the row; renaming an identifier rewrites every formula that uses it |
| Excluded | Loops, recursion, user-defined functions, assignments, file, network, clock or random access |
| Limits | Per formula: 2,048 bytes of text and of every text value, depth 32, 1,000 list items and call arguments, 10,000 steps, 65,536 bytes of created values; 1,000 parameters per family definition and 1,000 variables per document; per family instance 1,000,000 steps, nesting depth 8 and 10,000 placed children; exceeding a limit is an evaluation error, not a hang |
| Cycles | The parameter dependency graph must be acyclic; a cycle is rejected when the expression is entered |
| Errors | Errors carry the parameter, the sub-expression span and the reason; a family instance with an evaluation error keeps its last valid geometry and raises a warning |

Determinism: evaluation results are bit-identical across platforms for the same inputs; functions
with platform-dependent floating behavior are implemented in Rust without relying on platform math
libraries where results could differ.

## Geometry recipes

| Recipe | Inputs | Output |
|---|---|---|
| Extrusion | Closed profile sketch, start and end distances | Prism |
| Blend | Two closed profiles on parallel planes, vertex mapping | Ruled solid |
| Revolve | Closed profile, axis, start and end angles | Solid of revolution |
| Sweep | Closed profile, path (sketched or picked edges) | Swept solid |
| Swept blend | Two profiles, path | Swept ruled solid |
| Array | Recipe or nested use, count, spacing or extent | Repeated results |
| Node-graph recipe | Restricted geometry graph with declared inputs | Solid or curves |

Evaluation order: resolve parameter values; evaluate expressions; solve each sketch with
`arc-solve`; build solids (planar prisms through `arc-geom`, curved forms through `arc-brep`); apply
family voids to family solids; record host-cutting voids separately for the host; produce symbolic
geometry per view orientation; tag every face with a stable role for persistent references and
dimensions in projects.

Failure handling: an unsolvable sketch, a non-manifold or self-intersecting solid, or a void that
removes all material is an evaluation error reported with the recipe name. Test cases (AB-FAM-010)
run the same evaluation for every stored parameter set before a version is published.

## Nested and shared families

- A nested use places a child definition at a position defined relative to the parent's reference
  planes, with parameter associations (child parameter takes the value of a parent parameter or
  expression).
- A non-shared nested child is part of its parent's geometry: it is not selectable, scheduled or
  exported separately.
- A shared nested child becomes its own element in the project, with its own identifier derived
  from the parent identifier and the nested-use key, its own tags and schedule rows, and its own IFC
  element aggregated by the parent. The IFC export does not write shared children yet; it lists them in
  the loss report (`ab.io.ifc.family_shared_not_written`).
- A nested definition is pinned by version inside the parent. Updating the child in a document
  updates parents that use the same child version family-wide only through the explicit update
  command, which lists every affected parent.
- Nesting depth is bounded; cyclic nesting is rejected.

## Versioning

- A family ID is stable for the life of a family. A version is `major.minor.patch`:
  - patch: geometry or metadata change with no parameter changes;
  - minor: parameters added, nothing removed or retyped;
  - major: parameters removed, renamed or retyped, or hosting changed.
- A document pins one version per family ID. Loading a different version is an update command with
  a preview that lists affected instances, parameter mappings (by parameter ID), values that will
  change and instances that will fail evaluation. Major updates require an explicit parameter
  mapping for removed or retyped parameters.
- Two definitions with the same ID and version but different checksums are a conflict and are
  never merged silently.

## Library packages

A package is a single file (provisional extension `.archbfam`) in the Arc document package format
containing:

| Part | Content |
|---|---|
| Manifest | Package name, version, description, license (SPDX), source, author, minimum product version, dependencies on other packages by name and version range |
| Definitions | Family definitions with their checksums |
| Assets | Preview images, lookup tables, type tables, photometric files, textures; content-addressed |
| Index | Category, classification and parameter values for search without loading definitions |

Rules: checksums are verified on load; a package with unresolved dependencies or failed checksums is
refused with a report; license and source travel with every definition into documents and are
reported by model statistics; packages are read-only once published, and a change is a new package
version.

## Canonical text and packages

The canonical text of a definition is line oriented: a first line `archbfam-text 1 functions 1.0` (1.1 when a formula calls `lookup`),
then one line per part (`definition`, `parameter`, `type` with its `value` lines, `plane`,
`profile` (with `open=true` for an open polyline) and `symbol` with their `vertex` lines, `form` (a host-cutting void carries `cut_host=true`), `table` with its `column` and `row` lines, `nested` with its `nparam` lines, `array`, `test` with its `set` lines), ordered by
kind and then by key, with `name=value` fields in schema order; a value is a bare word or a quoted
string, and a stored value `x` is `x=<number>`, `x=true`, `x="text"` or `x_f="formula"` with an
optional `x_u=<unit>`. Numbers are in the shortest form that reads back the same, lengths in meters.
The reader is bounded (4 MiB, 100,000 lines, 4,096 bytes a line, 2,048 bytes a string, 1,000
parameters, 5,000 types), refuses an unknown field, escape, function or a function set newer than
the build's, and text read and written again is byte-identical. The checksum is the BLAKE3 hash of
the text.

M1 definitions hold extrusions and prism voids between reference planes (or by a signed depth),
profiles whose vertices are anchored to reference planes with optional formula offsets and bulges,
visibility by detail level, view orientation and a boolean formula, symbolic lines, regions and
masks per view orientation, reporting parameters that read `solid_volume`, `size_x`, `size_y` and
`size_z`, and test cases with expected volume, solid count or the form a failure must name. An
evaluation gives the closed solids of each form (a void is subtracted from the solids it meets and
must pass through two parallel faces), a role for every face (`family.form[<key>].start`, `end`,
`side.<loop>.<curve>.<facet>`, `void[<void>].<curve>.<facet>`), and the plan symbols; a failure
names the form. The budget of one evaluation counts the bytes of the formulas it runs.

Tables, arrays and nested uses: a `table` has typed columns (the first `keys` are keys) and
`lookup("table", "column", key, ..., fallback)` returns the column of the first row whose keys
match, else the fallback, which must have the column's dimension. An `array` repeats a form or a
nested use linearly (count, spacing, axis) or radially (count, angle, center about the vertical
axis); copy `i` of form `f` is `f#i`. A `nested` use pins a child family and version, a type by
name or by a family-type parameter, a placement on reference planes and the child parameters that
take formulas of the parent. A non-shared child's solids join the parent's body; a shared child is
kept apart with the identifier `nested_id(parent, "key")` (`"key#i"` for a copy), a version 8
identifier that is the same after every regeneration. A cycle among nested families, a nesting
deeper than 8 and more than 10,000 placed children are refused. A profile family
(category `profile`) evaluates its open and closed profiles and is not placed as a component.

A hosted family has a hosting kind; its void forms marked `cut_host` do not subtract from the
family's own solids but give the opening it cuts in a wall (the box of the voids). The record
`archbuilder.family_insert` (owned by the host wall) holds the family type, the distance along
the baseline, the height above the base, the hand and the facing; `archbuilder.family_insert.place`
refuses a family that is not hosted, a door or window family that is not hosted by a wall, a host
of another kind than the family's hosting, hosts other than walls (not yet), and what a built-in
insert refuses (outside the clear span, over another opening, too tall).

A `.archbfam` file is an Arc document package with a manifest record (`archbuilder.fam.manifest`)
and one record per published version (`archbuilder.fam.definition`: family identifier, version,
name, category, checksum and the canonical text). Publishing validates the definition, runs every
test case, and refuses a version that is not newer or smaller than the change needs (a removed,
retyped or renamed parameter or a hosting change is major, an added one minor). Reading verifies
the checksum, the text, the function set, the test cases and the dependencies (`name>=version`
against the packages available) and refuses with a report. `archbuilder-cli family check` and
`family publish` run these; the commands are `archbuilder.family.manifest`, `publish` and `check`.
Definitions are stored in a package as their canonical text; per-part records for the family
editing context come with that context.

