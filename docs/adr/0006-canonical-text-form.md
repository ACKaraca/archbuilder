# 0006 Canonical text form for comparison and merge

Status: Accepted

## Context

The document package is a SQLite database (suite decision), which is efficient and inspectable but
not readable by general-purpose diff and review tools. Teams want to see what changed between
versions, review changes in pull-request style workflows, and merge branches of a model. Element-level
comparison already exists through stable identifiers and the operation journal, but it is visible
only inside the product.

## Decision

- The CLI writes a canonical text form of a document's semantic content: one record per object,
  ordered by table and identifier, with fields in schema order, numbers in a fixed shortest
  round-trip format, and derived data excluded. The same document always produces byte-identical
  text.
- Model comparison (AB-COLLAB-004) and the text form use the same record normalization, so the
  product's comparison and an external text diff report the same elements.
- Branch merge (AB-COLLAB-007) works on element records with a three-way comparison against the
  common base; conflicting edits to the same record keep both states for the user to resolve.
- The text form is a review and comparison format. It is readable back for tests but is not a second
  native format.

## Consequences

- Schema changes must define the canonical serialization of new fields.
- Large documents produce large text; the CLI can limit output to tables, buildings or storeys.
- Merge correctness depends on generated children having deterministic identifiers (see the
  identity rules in [domain-model.md](../architecture/domain-model.md)).

## Alternatives considered

- **Text as the native format.** Easy diffing, but loses atomic saves, partial loading and the suite
  package format; rejected.
- **Binary comparison only inside the product.** Rejected because external review and CI tooling
  cannot use it.

## Reopen when

Hub-side comparison and merge cover the review workflows that the text form serves, and the text
form is no longer used in practice.
