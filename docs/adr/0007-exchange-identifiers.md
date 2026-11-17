# 0007 Exchange identifiers independent of the other application's identifiers

Status: Accepted

## Context

Exchange with the native project files of other applications through connectors
([suite ADR-0017](https://github.com/ACKaraca/arc-suite/blob/main/docs/adr/0017-exchange-connectors-and-licensed-bridges.md))
must survive repeated round trips. The other application's own identifiers change when a file is
copied, detached from a shared model or split by worksharing, so matching elements by them breaks
change detection and duplicates elements. IFC GlobalIds are derived from element identifiers and
serve a different exchange.

## Decision

- Each element in an exchange receives an exchange identifier assigned once by the exchange and
  stored on the element in this document and, by the connector, in the other application.
- The identity map records the exchange identifier beside the element identifier, the GlobalId and
  the other application's identifiers. Matching on re-import uses the exchange identifier first; an
  element without one is matched only by an explicit user decision, never by proximity.
- Exchange identifiers are never reused after deletion and never derived from the other
  application's identifiers.
- Element identifiers and GlobalIds keep their rules from [domain-model.md](../architecture/domain-model.md).

## Consequences

- Copying or detaching a file on either side does not break matching.
- Every converted element carries one more identifier in the identity map; the package schema must
  carry it in both directions.
- Elements created independently on both sides are reported as unmatched, not merged.

## Alternatives considered

- **Use the other application's identifiers.** Simple; rejected because they change with file
  operations.
- **Match by geometry and name.** Needs no stored identifier; rejected because it rebinds by
  proximity, which the persistent-reference rules forbid.
- **Reuse the IFC GlobalId.** Rejected because it couples two exchanges with different lifetimes.

## Reopen when

A host application offers an identifier that is documented as stable across file copies and
worksharing.
