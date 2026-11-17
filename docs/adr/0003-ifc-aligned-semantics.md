# 0003 IFC-aligned semantics with explicit mapping and no silent proxies

Status: Accepted

## Context

Open exchange is the main reason teams need a new authoring tool, and IFC is the open schema for
building data. Two extremes exist: use IFC itself as the in-memory model, or treat IFC as a late
export with best-effort mapping. The first constrains editing behavior (joins, hosts, constraints,
regeneration) to a schema designed for exchange; the second produces exports where difficult
elements become generic proxies and properties are lost without notice.

## Decision

- The domain model is native and optimized for authoring, but every category, system family and
  element kind has a defined IFC 4.3 entity, predefined type, relationships and property and
  quantity sets. The mapping is data in `archbuilder-io`, editable and versioned with documents.
- Quantities exported to IFC come from the same quantity rules as schedules.
- Exports never substitute a generic proxy silently: an element without a specific mapping is
  reported, and any proxy use is listed in the loss report.
- GlobalIds derive from stable element identifiers, so exports are stable and re-import updates in
  place.
- Imports convert what has a native counterpart, preserve the rest as foreign data, keep the
  original file, and report every decision.
- IDS validation and BCF issue exchange are product features, not add-ons.

## Consequences

- Each new element kind must ship with its IFC mapping and fixtures before it is complete.
- Mapping tables become part of project standards and can be shared like templates.
- Authoring behavior is free to use structures IFC does not have (join priorities, constraints),
  which are carried as preserved data or rebuilt on import.

## Alternatives considered

- **IFC as the native in-memory model.** Simplifies export but forces authoring structures to the
  exchange schema and makes regeneration rules awkward; rejected.
- **Best-effort export only.** Simple; rejected because silent losses are the problem being solved.

## Reopen when

The IFC schema gains authoring-level constructs (for example parametric constraints) adopted by
receiving tools, making a closer internal alignment cheaper than mapping.
