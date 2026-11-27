# Architecture decision records

This folder records decisions specific to ArchBuilder. Suite-wide decisions (repository topology,
UI toolkit, geometry tiers, document model, package format, commands, regeneration, units,
interoperability policy, rendering, collaboration, extensibility, licensing, assistance, robustness,
localization) are in the [suite ADRs](https://github.com/ACKaraca/arc-suite/tree/main/docs/adr) and
are not restated here.

| ADR | Title | Status |
|---|---|---|
| [0001](0001-three-editing-contexts.md) | Three editing contexts for views and drawings | Accepted |
| [0002](0002-declarative-families.md) | Declarative families with a bounded expression language | Accepted |
| [0003](0003-ifc-aligned-semantics.md) | IFC-aligned semantics with explicit mapping and no silent proxies | Accepted |
| [0004](0004-element-geometry-tiers.md) | Element geometry from planar generators, exact solids only where needed | Accepted |
| [0005](0005-housing-acceptance-and-independent-validation.md) | Housing fixtures and independent validation as release gates | Accepted |
| [0006](0006-canonical-text-form.md) | Canonical text form for comparison and merge | Accepted |
| [0007](0007-exchange-identifiers.md) | Exchange identifiers independent of the other application's identifiers | Accepted |
| [0008](0008-dwg-writing-in-the-first-production-release.md) | DWG writing in the first production release | Accepted |
| [0009](0009-parameters-expressions-and-family-documents.md) | Parameters, expressions and family documents | Accepted |
| [0010](0010-model-groups.md) | Model groups: definitions, instances and generated elements | Proposed |

## Template

```markdown
# NNNN Title

Status: Proposed | Accepted | Superseded by NNNN

## Context

What forces the decision: requirements, constraints, observed problems.

## Decision

What is decided, stated so it can be checked in code and documents.

## Consequences

What becomes easier and harder; obligations created for implementation and tests.

## Alternatives considered

Each alternative with the reason it was not chosen.

## Reopen when

The concrete trigger that would justify revisiting the decision.
```
