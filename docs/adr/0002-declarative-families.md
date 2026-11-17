# 0002 Declarative families with a bounded expression language

Status: Accepted

## Context

Families are shared widely between projects, firms and manufacturers. A family format that runs
code would turn every downloaded family into a security risk, make evaluation nondeterministic,
block headless validation and make two versions impossible to compare. At the same time families
need real parametric power: dimensions driven by formulas, conditional visibility, nested components
and lookup tables.

## Decision

A family definition is declarative data: typed parameters, reference planes, constrained sketches,
geometry recipes, visibility conditions, nested uses, connectors, lookup tables and test cases, as
specified in [families.md](../architecture/families.md).

Computation inside families is limited to the bounded expression language: typed and unit-checked,
no loops, recursion, user functions, assignments, file, network, clock or random access, with
evaluation depth and step limits and acyclic parameter dependencies. The same engine (`arc-solve`)
evaluates schedule fields, document variables and filter values. Geometry that sketches cannot
express may use a node-graph recipe restricted to pure geometry nodes under the same limits.

Families are versioned and pinned by documents; updates are explicit commands with previews.

## Consequences

- Any family can be loaded, evaluated, validated and compared without trusting its author.
- Evaluation is deterministic across platforms, which test cases, comparison and multi-user editing
  require.
- Some forms authored elsewhere as scripted objects cannot be reproduced; they are exchanged as
  geometry (STEP, meshes) or rebuilt with recipes and node-graph recipes.
- The expression function set is a product contract and is changed only with a format version.

## Alternatives considered

- **Embedded scripting in families.** Maximum flexibility; rejected for security, determinism and
  the suite's rule against embedded interpreters in the first releases.
- **Sandboxed WebAssembly family logic.** Safe but opaque to comparison and validation and heavy
  per instance; plug-ins remain available at document level instead.
- **No expressions (pure tables).** Too weak for common door, window and casework families.

## Reopen when

A significant class of families from the reference libraries cannot be authored with recipes,
expressions and node-graph recipes, as recorded in issues with concrete examples.
