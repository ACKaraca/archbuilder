# 0005 Housing fixtures and independent validation as release gates

Status: Accepted

## Context

BIM authoring tools fail in practice on scale and integration rather than on single features: many
storeys of different heights, repeated units, several buildings on one site, cores through every
floor, and exports that open in other tools. A feature tested in isolation, or an export read back
only by the code that wrote it, gives false confidence.

## Decision

- The first acceptance scenario is housing. Two synthetic fixtures, defined in
  [workflows.md](../product/workflows.md), are built by scripts through the automation protocol: a
  multi-building, multi-storey project with repeated units and cores (fixture A) and a small duplex
  (fixture B). Every milestone's exit criteria run on them.
- A capability is complete only when its workflow steps pass on the fixtures.
- Every export format is validated in CI by an independent reader that is not part of the writing
  code path (an independent IFC toolkit and IDS checker, an independent DWG and DXF reader, PDF text
  and scale extraction, a glTF validator). Self-reading is not evidence of compatibility.
- The data model has no fixed limit on storeys, buildings or element counts; resource limits are
  explicit per operation and reported when reached.

## Consequences

- Fixtures are maintained as code and grow with each milestone.
- A format profile can block a milestone when its independent validation fails, even if the writer
  works.
- Other building types (offices, hospitals, industrial) are added as fixtures after M2 and inherit
  the same gates.

## Alternatives considered

- **Feature-level tests only.** Faster to start; rejected because integration failures surface late.
- **Real project files as fixtures.** Representative, but licensing and confidentiality make them
  unsuitable for public corpus repositories.

## Reopen when

The housing fixtures no longer exercise a capability class that users report problems with, such
as long-span structures or large services systems.
