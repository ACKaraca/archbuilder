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
| Operators | `+ - * / ^`, comparison, `and`, `or`, `not`, conditional `if(condition, a, b)` |
| Functions | Rounding (`round`, `roundup`, `rounddown` with optional increment), `abs`, `sqrt`, `min`, `max`, trigonometric and inverse trigonometric, `exp`, `ln`, `log`, `pi`, text `concat`, `format` with unit and precision, `lookup(table, column, keys..., fallback)`, `count` and `sum` over nested arrays |
| Units | Literals carry units (`900 mm`, `15 deg`); every operation is dimensionally checked; dimensionless results of trigonometric arguments require angles |
| References | Parameters of the same family or its host (for nested associations), document variables, and in schedules the fields of the row |
| Excluded | Loops, recursion, user-defined functions, assignments, file, network, clock or random access |
| Limits | Maximum nesting depth and maximum evaluation steps per expression and per family evaluation; exceeding a limit is an evaluation error, not a hang (values settled by OQ-01) |
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
  element aggregated by the parent.
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
