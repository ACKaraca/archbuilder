# Parameters and constraints

This area specifies typed parameters, the bounded expression language, property editing, rule-based
numbering, classification and property sets, and constraints between element references in
projects. Families use the same parameter model and solver; see
[08-families-and-family-editor.md](08-families-and-family-editor.md).

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Parameter definition | Stable ID, name, data type, quantity kind, group, instance or type scope, validation rule | Built in, project-scoped, library-defined or family-defined |
| Parameter binding | Definition, categories | Attaches a project or library parameter to categories |
| Parameter value | Element or type, definition, value or expression | Stored on elements and types |
| Document variable | Name, data type, value or expression | Project-wide; drives parameters and dimensions |
| Expression | Source text, parsed tree, referenced parameters | Evaluated by the parameter engine |
| Numbering template | Element filter, grouping, ordering, format, target parameter | Applied by a numbering command |
| Classification reference | System, code, title, source | Assigned to elements and types |
| Property set | Name, member parameters, IFC mapping | Groups parameters for exchange |
| Constraint | Kind (alignment lock, dimension lock, equality), references | Solved during regeneration |

## Parameter definitions

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PARAM-001 | Built-in parameters | Gives each category a fixed set of parameters, including read-only computed values (length, area, volume, perimeter, counts) that update with geometry. | T1 | 2 |
| AB-PARAM-002 | Parameter data types | Supports length, area, volume, angle, slope, number, integer, boolean, text, multiline text, date, URL, image, material, family type, enumeration, and the discipline quantity kinds (force, moment, stress, flow, pressure, velocity, power, current, voltage, temperature, illuminance). Values are stored as typed quantities in SI units. | T1 | 2 |
| AB-PARAM-003 | Project parameters | Defines parameters in a document and binds them to categories as instance or type parameters; they appear in properties, schedules, filters and exports. | T1 | 2 |
| AB-PARAM-004 | Library parameter definitions | Reads parameter definitions with stable identifiers from parameter library files in an open text format. The same definition keeps its identity in families, tags, schedules and IFC mappings across documents. | T1 | 2 |
| AB-PARAM-005 | Document variables | Defines project-wide named values with optional expressions that drive dimensions and element parameters; changing a variable regenerates every dependent element in one transaction. | T2 | 2 |
| AB-PARAM-006 | Reporting parameters | Exposes measured values in families (a dimension reported rather than driving) for use in expressions and schedules. | T2 | 2 |
| AB-PARAM-007 | Parameter groups and order | Places parameters in named groups and a user-defined order in the properties panel, per family and per document. | T1 | 2 |
| AB-PARAM-008 | Validation rules | Attaches minimum, maximum, allowed values or a required flag to a definition. Each rule either warns or rejects the edit, as configured. | T2 | 2 |
| AB-PARAM-009 | Type and instance scope | Applies type values to all instances of a type and keeps instance values per element. Changing a definition's scope between type and instance preserves existing values and reports elements whose values diverge. | T1 | 2 |

## Expressions

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PARAM-010 | Bounded expression language | Evaluates arithmetic, comparison, logical and conditional operators, rounding, trigonometric, logarithmic, min and max, text concatenation and formatting, and table lookup. There are no loops, user functions, file or network access; evaluation is limited in depth and steps, and cyclic references are rejected. | T1 | 2 |
| AB-PARAM-011 | Unit checking | Rejects expressions whose quantity kinds do not combine (adding a length to an area, a length assigned to an angle), pointing to the offending sub-expression. | T1 | 2 |
| AB-PARAM-012 | Dependency view | Shows which parameters and variables an expression reads and which parameters, dimensions and elements depend on a given value. | T2 | 2 |

## Property editing and data

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PARAM-013 | Properties panel | Shows a type selector, instance parameters and type parameters for the selection, with type duplication and renaming. With several elements selected, common parameters are editable and differing values are shown as mixed. | T1 | 2 |
| AB-PARAM-014 | Type editing impact | States, before a type parameter edit is applied, how many instances it changes, and reports instances whose hosts or constraints reject the new value. | T1 | 2 |
| AB-PARAM-015 | Rule-based numbering | Applies numbering templates that select elements by filter, group them by parameter values, order them by position along a path, by coordinates or by creation order, and format numbers with prefix, suffix and padding into a target parameter. Re-running updates numbers; individually locked values are kept. | T2 | 2 |
| AB-PARAM-016 | Classification | Assigns classification references from loaded classification tables (any hierarchical system) to elements and types, and exports them as IFC classification references. | T1 | 2 |
| AB-PARAM-017 | Data dictionary lookup | Looks up classes and property definitions in the buildingSMART Data Dictionary, caches them for offline use, and binds selected properties as library parameters with their dictionary URIs. | T3 | 3 |
| AB-PARAM-018 | Property sets | Groups parameters into named property sets mapped to IFC property set names, with standard IFC property set templates available to bind. | T1 | 2 |

## Constraints

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PARAM-019 | Alignment and dimension locks | Locks an alignment between two references or the value of a dimension, and adds equality constraints across a dimension chain. Locked relationships are maintained when either side moves. | T1 | 2 |
| AB-PARAM-020 | Project constraint solving | Solves project constraints inside the transaction that changes the model. An edit that would over-constrain is rejected with the conflicting constraints listed and an offer to remove one. | T1 | 2 |
| AB-PARAM-021 | Constraint display | Shows locks and equality marks in the active view on request and lets the user select and delete constraints there. | T1 | 2 |
| AB-PARAM-022 | Datum constraints | Binds element faces, ends and centerlines to grids, levels and reference planes so they move with the datum. | T1 | 2 |
| AB-PARAM-023 | Persistent references | Keeps constraint and dimension references attached to faces and edges through edits using persistent references. A reference that no longer resolves produces a warning; it is never rebound to a nearby face. | T1 | 2 |
