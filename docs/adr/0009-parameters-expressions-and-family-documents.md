# 0009 Parameters, expressions and family documents

Status: Accepted

## Context

[ADR 0002](0002-declarative-families.md) decides that families are declarative data evaluated by
the bounded expression language, and [families.md](../architecture/families.md) sketches the
definition model. Before any of it is built, several questions need one answer each, because the
answers change the `.archb` format, create the `.archbfam` format, fix the expression function set
and set IFC defaults, all of which need the maintainer's approval:

- Where parameters, formulas and document variables live in a document, and what is authored and
  what is derived.
- How the product uses `arc-solve`. At the pinned `arc-core` revision, `arc-solve` provides the
  lexer, parser and evaluator with units, the closed built-in function set, host functions,
  name lookup and evaluation limits (CORE-SOLVE-016 to CORE-SOLVE-018, CORE-SOLVE-021,
  CORE-SOLVE-022). Parameter scopes with dependency order (CORE-SOLVE-020), typed host tables
  (CORE-SOLVE-019) and the geometric constraint solver (CORE-SOLVE-001 to CORE-SOLVE-015) are
  specified but not implemented. The function list in [families.md](../architecture/families.md)
  differs from the implemented one (`and`/`or`/`not`, `roundup`, `exp`, `ln`, `log`, `count` are
  not in `arc-solve`; `&&`/`||`/`!`, `floor`, `ceil`, `len`, `first`, `last` are). Its names are
  ASCII identifiers only. Its trigonometric functions call the standard library, whose last bit may
  differ between platforms, which [families.md](../architecture/families.md) does not allow.
- Which forms can be built. `arc-geom` provides planar extrusion of regions with arcs faceted at a
  chord (`extrude`, `extrude_between`, `prism`), clipping and sectioning of polyhedra by planes, and
  subtraction of a prism that passes through two parallel faces (`subtract_prism`). It has no
  general solid boolean, revolve, sweep or blend; `arc-mesh` has no boolean yet; `arc-brep`, which
  [ADR 0004](0004-element-geometry-tiers.md) assigns to curved family forms, is a specification.
- How families relate to the element types that exist: project-owned type records
  (`archbuilder.wall_type`, `archbuilder.door_type` and the others) with the commands
  `archbuilder.type.set`, `duplicate`, `delete`, `assign`, `match` and `upgrade`, and built-in
  door and window kinds that copy their type's values onto each instance
  ([domain-model.md](../architecture/domain-model.md#the-element-types-of-the-first-slice)).
  Families must extend this, not replace it.
- How the step and depth limits of OQ-01 are set.

## Decision

### 1. Parameters

**Kinds of parameter definition.**

| Kind | Record | Owner | Scope |
|---|---|---|---|
| Built-in | None; fixed per category in code | Product | Type or instance; computed ones are read-only |
| Project parameter | `archbuilder.parameter` | Project | Type or instance, bound to categories |
| Library parameter | `archbuilder.parameter` with a library identifier | Project (copied from a parameter library file) | As a project parameter; keeps its library identifier |
| Family parameter | A part of the family definition | Family definition | Type or instance |
| Document variable | `archbuilder.variable` | Project | Document-wide, one value |

A parameter definition carries a stable identifier (128-bit, never changed by renaming), a display
name, a formula identifier (below), a data type, the dimension for quantity types, the display unit
of its formulas, a scope, a group and an order, and an optional rule (minimum, maximum, allowed
values, required; warn or reject). A project parameter also lists its categories. Data types for
the first release: length, area, volume, angle, slope, number, integer, boolean, text, enumeration,
material, family type. The discipline quantity kinds, dates, URLs, images and multiline text of
AB-PARAM-002 follow without a format change, because the data type is an enumeration whose new
variants are a minor schema version.

**Formula identifiers.** `arc-solve` names are ASCII identifiers, while display names are any text
(`Genişlik`). Every parameter and variable therefore has a formula identifier: ASCII, unique among
the names a formula can see, proposed from the display name by transliteration and editable.
Formulas are stored with identifiers; the editor shows display names beside them. Renaming an
identifier rewrites every formula that uses it in the same transaction, token by token from the
parsed spans, never by text search.

**Values.** A parameter value is a literal or a formula. A literal quantity is stored in SI units.
A formula is stored as its source text with `.` as the decimal separator (localized input is read
with `Expr::parse_localized` and stored in the canonical form) together with the unit that bare
numbers take (the field's display unit when it was entered), so changing the project's display
units never changes what a stored formula means. The parsed tree, the referenced names and the
result are derived and never stored.

**Where values are stored.**

- A type's values: in the type record. For a family type, a keyed list `values` of
  (parameter identifier, literal or formula).
- An instance's values: in the `parameters` field shared by physical elements
  ([domain-model.md](../architecture/domain-model.md#physical-elements)), a keyed list of
  (parameter identifier, literal or formula). Only values that differ from the default are stored.
- A document variable's value: in its record.
- Project parameters use these product fields, not `arc-doc` property sets, because values must
  hold formulas and references (material, family type), which property set values cannot, and
  because bindings follow categories, which do not map one to one onto type keys. `arc-doc`
  property sets stay available for preserved IFC property sets on import.

**Precedence and scope.** An instance reads, in order: its own instance values, its type's values,
the definition defaults. A type formula reads type parameters, document variables and built-in
type values; it never reads an instance parameter. An instance formula additionally reads the
instance's parameters, built-in instance values (for example the host's thickness for a hosted
family) and, in a nested family, the values its parent passes. A document variable reads other
document variables only. A parameter driven by a formula is read-only where it is shown; it is
edited by changing the formula.

**Evaluation order and dependencies.** Each scope (a document's variables, a family type, a family
instance) is a dependency graph whose nodes are parameters and whose edges are the names each
formula references. Evaluation is in topological order, ties broken by parameter identifier, so the
order is deterministic. A cycle is rejected when the formula that closes it is entered, with the
cycle listed by display name. Reporting parameters (values measured from evaluated geometry) are
evaluated after geometry; a driving value (a reference plane offset, a form input, a visibility
condition, a nested association) may not depend on a reporting parameter, so the graph cannot
loop through geometry. Between records, the `arc-doc` dependency graph carries the edges: a
variable to the elements and types that read it, a type to its instances, a family definition to
its types. Changing a value regenerates its dependents in one transaction (AB-PARAM-005).

**Validity.** A command that would leave a type or instance whose values break a reject rule, whose
formulas fail, or whose family evaluation fails is refused with the parameter, the span and the
reason, and the document is unchanged (the family invariant in
[domain-model.md](../architecture/domain-model.md#invariants)). A change that affects many elements
(a variable, a type, a family update) lists every element that would fail and is refused unless the
user removes the cause. Only a document opened with values that no longer evaluate (for example a
file written by a newer build) keeps its last derived geometry where a cache exists and raises a
warning per element.

**Locks.** Parameter locks are rule-based: a value written by a numbering template and locked
(AB-PARAM-015) is a flag on the stored value. Alignment and dimension locks between project
elements (AB-PARAM-019, AB-PARAM-020) are constraints and are not decided here. Inside a family,
alignment holds by construction (section 3).

### 2. Expression language

**Engine.** `arc-solve` is added to `[workspace.dependencies]` at the pinned `arc-core` revision
and to `third_party` in `xtask/layers.toml`; only `archbuilder-model` depends on it directly. No
other new crate or third-party dependency is needed. The product module `archbuilder-model::expr`
owns the formula value, parsing with the product's limits, reference extraction from the tree,
identifier rewriting, and the name lookup that maps scopes onto `arc_solve::Names`. When
CORE-SOLVE-020 lands in `arc-core`, the product's ordering moves onto it without a format change.

**Syntax.** The grammar of `arc-solve` as implemented, unchanged: `&&`, `||` and `!` for logic,
`if(c, a, b)` with only the chosen branch evaluated, comparisons that do not chain, unit literals
(`900 mm`, `15 deg`, `8' 6"`), text in double quotes, `,` or `;` between arguments. No loops,
recursion, user functions, assignments, file, network, clock or random access, as in ADR 0002.

**Function set, version 1.** Exactly these, and nothing else is callable from a family or a
parameter:

| Group | Functions | Source |
|---|---|---|
| Arithmetic | `abs`, `min`, `max`, `sqrt`, `pow` | `arc-solve` built-in |
| Trigonometric | `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2` | `arc-solve` built-in |
| Rounding | `round`, `floor`, `ceil`, each with an optional increment | `arc-solve` built-in |
| Conditional | `if` | `arc-solve` built-in |
| Lists and text | `len`, `sum`, `first`, `last`, `concat`, `format` | `arc-solve` built-in |
| Tables | `lookup(table, column, key, ..., fallback)` | Product host function (AB-FAM-018) |

`lookup` reads an embedded table of the family by name; each column has a declared dimension or is
text; keys compare exactly; the fallback is returned when no row matches and must have the
column's dimension. On acceptance, the function row and operator row of
[families.md](../architecture/families.md) are replaced by this table: `exp`, `ln` and `log` are
not in version 1 (no reference family needs them), `roundup` and `rounddown` are `ceil` and `floor`
with an increment, `count` is `len`, and `and`, `or`, `not` are `&&`, `||`, `!`.

The function set has a version, recorded in every family definition and in the document. A
definition that needs a newer version is refused on load with a report. Adding a function is a
minor version; changing or removing one is a major version and a format change.

**Values in formulas.** Quantities with dimensions, bare numbers read by the stored unit, truth
values, texts and lists. An integer parameter takes a dimensionless result that is a whole number.
An enumeration takes a text naming one of its values. A family-type parameter takes a text naming a
type of the declared family, resolved after evaluation; an unknown name is an evaluation error.
Material, image and URL parameters are literals only in version 1.

**Limits.** These close the value part of OQ-01; exceeding one is `solve.expr.limit_exceeded`
(per formula) or `ab.family.limit_exceeded` (per evaluation), never a hang:

| Limit | Value | Applies to |
|---|---|---|
| `text_bytes` | 2,048 | One formula's text and every text value |
| `depth` | 32 | Expression tree and list nesting |
| `list_length` | 1,000 | Lists and call arguments |
| `steps` | 10,000 | One formula |
| `value_bytes` | 65,536 | Values one formula creates |
| Parameters | 1,000 | One family definition; also variables per document |
| Steps per evaluation | 1,000,000 | One family instance, nested uses included |
| Nesting depth | 8 | Nested family uses |
| Placed children | 10,000 | Nested uses and array copies in one instance |
| Lookup table | 10,000 rows, 64 columns, 1 MiB | One embedded table |
| Types | 5,000 | One family definition or type table |

**Units.** Every operation is dimension-checked by `arc-solve`; a result is checked against the
parameter's dimension with `evaluate_as`, so a length assigned to an angle is refused with the
whole expression's span (AB-PARAM-011).

**Errors.** A formula error is reported with the `arc-solve` code, the parameter's display name,
the byte span of the sub-expression mapped back to the text as displayed, and, for unknown names,
the closest known names once CORE-SOLVE-023 exists. Family evaluation errors use `ab.family.*`
codes naming the form, reference plane or nested use. Messages are catalog entries in
`locales/*/archbuilder-model.ftl`.

**Determinism.** Everything except the trigonometric functions and fractional powers is
bit-identical across platforms today. Until `arc-solve` implements those in Rust without the
standard library's platform functions (requested of `arc-core`), family test cases compare
geometry within the model tolerance and no stored value or checksum depends on a trigonometric
result, because results are derived and never stored.

### 3. Family documents

**Content of a definition.**

| Part | First release | Later |
|---|---|---|
| Header | Family identifier (128-bit, stable), version `major.minor.patch`, name, category, hosting kind (none, wall, floor, ceiling, roof, face, work plane), behavior flags (AB-FAM-021), IFC entity and predefined type, function set version, source, license | Template origin |
| Parameters | As in section 1, with groups and order | Library parameters by identifier |
| Types | Named value sets; optional type table | |
| Reference planes | Named planes, each parallel to one of the three origin planes at an offset that is a literal or formula, with a strength and an orientation role (left, right, front, back, top, bottom, center); the origin planes mark the insertion point | Planes at an angle |
| Profiles | Closed loops of lines and arcs whose vertices lie on intersections of reference planes, optionally offset by a formula, or are fixed points; an arc is given by its ends and a bulge formula | Sketches solved by the constraint solver |
| Forms | Extrusion between two reference planes or by a formula depth; solid or void; material slot, subcategory, visibility | Blend, revolve, sweep, swept blend, node graphs |
| Voids | A prism void through family solids; a host-cutting opening profile for hosted families | General voids |
| Symbolic geometry | Lines, arcs, filled and masking regions per view orientation (plan, front, back, left, right), with visibility | |
| Visibility | View orientations, detail levels (low, medium, high) and a boolean formula, per form and symbolic element | Per view kind beyond orientation |
| Nested uses | A pinned child definition and type (or a family-type parameter), placement on reference planes, parameter associations, shared flag | |
| Arrays | Linear and radial copies of a form or nested use, count and spacing by formula | |
| Tables | Embedded lookup tables with typed headers | |
| Test cases | Named value sets evaluated before publishing (AB-FAM-010) | |
| Connectors | | Phase 4 (AB-FAM-020) |

Profiles anchored to reference planes give dimension-driven geometry (AB-FAM-009) without a
constraint solver: a labeled dimension is a reference plane offset formula, an alignment holds
because the vertex is defined on the plane, and an equality is two offsets that read one parameter.
When the `arc-solve` constraint solver exists, constrained sketches become a second profile kind;
anchored profiles stay valid.

**Forms by geometry tier.** Extrusions and prism voids use `arc-geom` (planar faces exact, arcs
faceted at the chord tolerance recorded in outputs). A family's solids are a list of closed solids,
not a union, as the door and window solids are today, so no general boolean is needed. Blend,
revolve, sweep and swept blend wait for `arc-brep`
([ADR 0004](0004-element-geometry-tiers.md)); whether faceted versions are offered earlier is an
open question below. Every evaluated face carries a stable role (`family.form[<form key>].<face>`)
for persistent references.

**Storage of `.archbfam`.** A `.archbfam` file is an Arc document package
([package format](https://github.com/ACKaraca/arc-core/blob/main/docs/architecture/document-package.md)) like `.archb`,
holding one or more definitions:

- Each definition in authoring form as records (`archbuilder.fam.definition`, `.parameter`,
  `.reference_plane`, `.profile`, `.form`, `.symbol`, `.nested_use`, `.table`, `.test_case`,
  `.type`), so the family editing context (AB-FAM-005) gets transactions, undo, the journal,
  validation and recovery from `arc-doc` unchanged.
- A manifest record (name, version, description, SPDX license, source, author, minimum product
  version, package dependencies) and the search index of
  [families.md](../architecture/families.md#library-packages).
- For each published version, the definition's canonical text (below) as an asset. The asset hash
  (BLAKE3 of the text) is the definition's checksum.

Reasons: one reader, one recovery model and one `inspect` for both file kinds; atomic saves and
partial loading; content-addressed assets for previews, tables and published texts. The
line-oriented text that [families.md](../architecture/families.md) calls for is the canonical text
of a definition, not the native file.

**Canonical text of a definition.** Published definitions are serialized by the rules of
[ADR 0006](0006-canonical-text-form.md): one record per part, parts ordered by kind and then by key,
fields in schema order, numbers in shortest round-trip form, formulas as stored, derived data
excluded, and a first line naming the text format version and the function set version. Reading the
text back yields records that serialize to the same bytes; this round trip is a test for every
fixture family. The project's own canonical text prints a loaded definition as its header and
checksum, and the CLI can expand the body.

**Versions.** The rules of [families.md](../architecture/families.md#versioning) stand: patch,
minor and major by parameter changes; a document pins one version per family identifier; same
identifier and version with a different checksum is a conflict. Publishing assigns the version,
runs all test cases, and refuses when a case fails or when the change is larger than the version
step chosen (a removed parameter in a minor version).

**Loading into a project.** Loading copies; a document never references a library file and opens
on its own.

| Record in `.archb` | Content |
|---|---|
| `archbuilder.family` | Family identifier, version, checksum, name, category, hosting, IFC assignment, source, license, the canonical text as an asset; owned by the project |
| `archbuilder.family_type` | Owning family, name, keyed `values`; a new type kind beside the existing ones |
| `archbuilder.component` | A freestanding instance: family type, storey, level offset, placement (origin, rotation, mirror), work plane, `parameters` (AB-OPEN-015) |
| `archbuilder.family_insert` | A hosted instance: family type, host, host-local position, hand, facing, `parameters`; the placement rules of doors and windows apply to its host-cutting outline |

`TypeKind` gains one kind for family types, and the existing type commands (`set`, `duplicate`,
`delete`, `assign`, `match`), create similar, match type, select all of type and the Types panel
work on family types. Family instances do not copy type values onto the instance (the rule that
exists so that built-in geometry reads from the instance alone): evaluation reads definition, type
and instance together, so `ab.model.type_mismatch` does not apply to them. Built-in doors and
windows stay as opening recipes (AB-OPEN-009) with their current records.

Commands: `archbuilder.family.load` (from a package or another document; the user picks types;
type name conflicts are kept, replaced or renamed by a parameter; one undo step);
`archbuilder.family.update` (a newer version, with a preview of affected instances, parameter
mapping by identifier, values that change and instances that would fail; a major update requires a
mapping for removed or retyped parameters; one undo step); `archbuilder.family.extract` (saves
loaded definitions into a package, AB-FAM-033). Loading the same identifier and version with a
different checksum is refused; the user may load it as a new family, which gets a new identifier
and records the original as its source.

**Identifiers.** Family, parameter, type and part identifiers are assigned once and never change;
parameter mapping on update is by identifier. Instances are ordinary records. Non-shared nested
children and array copies are derived geometry and have no identifiers. Shared nested children are
derived elements, not stored records: each has an identifier derived from the instance identifier
and the nested-use key (and the array index), by a derivation that is a pure function of those
inputs and never collides with a generated record identifier. Authored data of a shared child
(a mark, a comment) is stored on the parent instance under the nested-use key. IFC GlobalIds derive
from these identifiers like any other.

### 4. IFC defaults for family instances

The mapping table in `archbuilder-io` gains a default per category; a family's or type's own IFC
assignment (AB-FAM-024) overrides it.

| Category | IFC4 entity, predefined type |
|---|---|
| Furniture | `IfcFurniture`, `NOTDEFINED` |
| Casework | `IfcFurniture`, `USERDEFINED` (`CASEWORK`) |
| Plumbing fixtures | `IfcSanitaryTerminal`, `NOTDEFINED` |
| Lighting fixtures | `IfcLightFixture`, `NOTDEFINED` |
| Electrical equipment | `IfcElectricAppliance`, `NOTDEFINED` |
| Mechanical equipment | `IfcUnitaryEquipment`, `NOTDEFINED` |
| Doors, windows (family inserts) | `IfcDoor`, `IfcWindow`, with an `IfcOpeningElement` voiding the host and a fills relation, as for built-in inserts |
| Columns (architectural) | `IfcColumn`, `NOTDEFINED` |
| Generic models, specialty equipment | `IfcBuildingElementProxy`, `NOTDEFINED`, listed in the loss report as a proxy by category |

- **Types.** Each family type maps to the type entity of its occurrence entity (`IfcFurnitureType`
  and so on) with `IfcRelDefinesByType`; its GlobalId derives from the family type's identifier.
- **Properties.** Parameters bound to an IFC property set by name (AB-PARAM-018) are written there.
  Every other non-empty parameter is written to `ArchBuilder_Type` on the type or
  `ArchBuilder_Instance` on the occurrence, so nothing is dropped silently. The family identifier,
  version and source are written to `ArchBuilder_Family` on the type.
- **Body.** The evaluated solids as faceted B-reps, one closed shell per solid. When a type's
  geometry depends on no instance parameter, it is one `IfcRepresentationMap` on the type and each
  occurrence uses a mapped item. Host-cutting voids become opening elements in the host.
- **Nesting.** A shared nested child is its own element, aggregated by its parent
  (`IfcRelAggregates`); a non-shared child is part of its parent's body.
- **Loss report.** Proxy use by category; parameters of types IFC cannot hold (family type,
  image) written as text; forms tessellated with the tolerance used; a hosted void that could not
  be cut; plan symbols and visibility conditions, which are not exported.

### 5. Groups

Model groups (AB-PHASE-010 to AB-PHASE-013) are separate machinery: their members are ordinary
records copied with new identifiers and an identity map, not evaluated definitions. They share only
the package manifest, checksum and immutable-version rules of section 3, and the update-with-preview
pattern. A group may contain family instances; saving or loading a group carries the family
versions it pins. Groups get their own ADR.

## Consequences

- `.archb` gains the records `archbuilder.parameter`, `archbuilder.variable`, `archbuilder.family`,
  `archbuilder.family_type`, `archbuilder.component` and `archbuilder.family_insert`, and every
  physical element type gains an optional `parameters` field as a minor schema version; files from
  before load unchanged. Each new field defines its canonical serialization.
- `.archbfam` exists as a package kind with its own record types and a canonical text whose hash
  is the checksum.
- `arc-solve` becomes a dependency of `archbuilder-model`; the layer table gains one entry.
- [families.md](../architecture/families.md), the parameter rows of
  [09-parameters-and-constraints.md](../product/capabilities/09-parameters-and-constraints.md),
  [domain-model.md](../architecture/domain-model.md), the glossary and OQ-01 are updated on
  acceptance; nothing in them changes before.
- M1 forms are limited to extrusions and prism voids until `arc-brep` exists, which leaves the
  curved part of AB-FAM-007 open in M1 unless the maintainer accepts faceted forms.
- `arc-core` is asked for: CORE-SOLVE-020 (scopes and dependency order), deterministic
  trigonometric functions, and a shared function for derived identifiers (today `derived_id` lives
  in `arc-io-ifc`).
- Every new element kind ships with its IFC mapping, fixtures and undo tests, per the repository
  invariants.

## Alternatives considered

- **Unicode names in formulas.** More natural for Turkish names; needs a grammar change in
  `arc-core` that also affects unit names, and stored formulas would change meaning if a parameter
  were renamed to an existing word. Formula identifiers keep the grammar and make renames explicit.
- **Project parameter values in `arc-doc` property sets.** Reuses existing storage, but property
  values cannot hold formulas or references and bind by type key rather than category.
- **Formulas with bare numbers in the current display unit.** Shorter to store; a change of project
  units would silently change values. The stored unit fixes the meaning.
- **A text file as the native `.archbfam`.** Easy to diff, but no transactions, undo, recovery or
  assets for the family editor; the canonical text gives the diff without those losses.
- **A reference to the library file from documents.** Saves space; a document would not open
  alone and a library change could alter it, which ADR 0002 forbids.
- **Family types as the existing door and window type records.** Would force every family into a
  fixed field layout; a separate family type kind with keyed values keeps the existing commands and
  layouts intact.
- **Shared nested children as stored records.** Simple to query, but they would be generated data
  stored as authority and would need reconciliation on every parameter change.
- **A new crate `archbuilder-family`.** Clearer boundary; not needed while the evaluator is a module
  of `archbuilder-model`, which [families.md](../architecture/families.md) already names. Split when
  build times or ownership justify it.
- **Solver-based family sketches first.** Matches the long-term model, but the solver does not
  exist; anchored profiles deliver M1 families now and remain valid later.

## Reopen when

A reference door, window or casework family cannot be authored with anchored profiles,
extrusions and the version 1 function set, recorded in an issue with the family; or `arc-solve`
changes its grammar or built-in set; or `arc-brep` is available and the form list is extended.

## Resolutions

The questions raised with this proposal were settled when it was accepted:

1. The version 1 function set is accepted without `exp`, `ln` and `log`.
2. The limits table closes OQ-01.
3. M1 offers extrusions and prism voids only; blend, revolve and sweep wait for `arc-brep`.
4. The IFC defaults per category are accepted, including casework as user-defined furniture and
   the property sets `ArchBuilder_Type`, `ArchBuilder_Instance` and `ArchBuilder_Family`.
5. Formulas use ASCII formula identifiers.
6. `.archbfam` is the extension, and one package may hold several definitions.
7. Shared nested children use derived identifiers (version 8) as element identifiers.
8. Until deterministic trigonometric functions exist in `arc-core`, tests compare results within
   tolerance.

## Implementation order

Each unit is one pull request with its tests, ordered by dependency. The `arc-core` items are
requests to that repository and block only the units that name them.

| Unit | Scope | Capabilities | Crates | Tests and acceptance |
|---|---|---|---|---|
| C1 (`arc-core`) | Parameter scopes with dependency order and cycle listing; deterministic trigonometric functions; derived identifier function in `arc-base` | CORE-SOLVE-020 | `arc-solve`, `arc-base` | Specified in `arc-core` |
| 1 | `arc-solve` dependency; `archbuilder-model::expr`: formula value (text and unit), parsing with the limits, reference extraction, identifier rewrite, evaluation with dimension check | AB-PARAM-010, AB-PARAM-011 | `archbuilder-model`, `xtask` | Every function and limit; dimension errors with spans; rewrite keeps spans and meaning; `cargo xtask layers` passes |
| 2 | Document variables: record, commands to create, set, rename and delete, ordered evaluation, cycle refusal, regeneration of dependents, `inspect` list | AB-PARAM-005, AB-PARAM-012 (reads) | `archbuilder-model`, `archbuilder-commands`, CLI | Cycle refused with the cycle named; one undo per command; rename rewrites formulas; canonical text stable |
| 3 | Project parameters: record with bindings, `parameters` field on physical elements, scope and rules, property grid groups | AB-PARAM-002, AB-PARAM-003, AB-PARAM-007 to AB-PARAM-009 | model, commands, app | Old files load unchanged; scope change preserves values; reject rules refuse; undo tests |
| 4 | Family definition records, validation and limits, canonical text writer and reader, checksum | AB-FAM-002, AB-FAM-029 | `archbuilder-model` | Round trip byte-identical for fixture families; hostile input refused within limits |
| 5 | Family evaluator: parameters, reference planes, anchored profiles, extrusions, prism voids, visibility, symbolic geometry, face roles, test cases | AB-FAM-006, AB-FAM-007 (extrusion and voids), AB-FAM-009 to AB-FAM-014 | `archbuilder-model` | A fixture table, door and cabinet evaluate for each type and edge case; failures name the form; volumes match hand values |
| 6 | `.archbfam` packages: create, read, manifest, publish with test cases, checksum verification, refusal report; CLI `family check` and `family publish` | AB-FAM-031, AB-FAM-033 | model, commands, CLI | Tampered checksum refused; missing dependency refused; published text equals unit 4 output |
| 7 | Loading into projects: family, family type and component records, the new type kind in the type commands and panel, load and update with preview and conflicts, plan symbols in plan views | AB-FAM-019, AB-FAM-030, AB-OPEN-015 | model, commands, app | Load, update and conflict paths each one undo step; major update without mapping refused; instance survives save and reopen |
| 8 | IFC export of family instances and types with the defaults, property sets and loss report | AB-FAM-024 | `archbuilder-io` | Independent validation passes; proxies only for generic categories and listed; GlobalIds stable across exports |
| 9 | Hosted family inserts: placement rules, host-cutting voids, door and window families | AB-FAM-021, AB-OPEN-001 | model, commands, io | Placement refusals as for built-in inserts; host volume net of the void; IFC opening and fills |
| 10 | Nested families, shared children with derived identifiers, family-type parameters | AB-FAM-015, AB-FAM-016 | model, io | Identifiers stable across regeneration; nesting cycle and depth refused; shared child scheduled and aggregated in IFC |
| 11 | Arrays, lookup tables, profile families | AB-FAM-017, AB-FAM-018, AB-FAM-027 | model | Counts at the limits; `lookup` fallback and unit checks |
| 12 | Family editing context in the application, templates per category | AB-FAM-004, AB-FAM-005 | app | Edit from an instance, commit loads a new version and reports affected instances |
| 13 | Curved forms: blend, revolve, sweep, swept blend | AB-FAM-007 (rest) | model | Depends on open question 3 or on `arc-brep` |

Library search (AB-FAM-032) and groups follow separately.
