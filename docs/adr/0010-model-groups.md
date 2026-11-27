# 0010 Model groups

Status: Proposed

## Context

Repeated units (an apartment, a core) are placed on several storeys of a building. ADR 0009
(section 5) decides that groups are separate machinery: their members are ordinary records, not
evaluated definitions, and they share with families only the package manifest, the checksum and
the immutable-version rules. Capability rows AB-PHASE-010 to AB-PHASE-013 require a definition
closed under dependencies, instances that share it, cross-storey placement by level mapping,
per-instance exceptions, separation and reuse.

## Decision

- **Definition.** `archbuilder.group` is a project record: a name, a reference point, the
  elevations of its levels relative to the first, and the level index of each member. Its members
  are ordinary element records (walls, doors, windows, wall openings, family components, rooms)
  owned by the definition, or by its wall for hosted kinds, in definition-local coordinates (the
  reference point is the origin). Members are not part of the building: `spatial::objects` leaves
  them out, so every reader, view, schedule and export sees the instances only.
- **Closure.** A selection is closed under hosting: a wall takes its doors, windows and wall
  openings; a hosted element takes its wall and the wall's openings. Other kinds (floors, stairs,
  shafts, family inserts and the rest) are refused until their placement is specified for groups.
- **Instance.** `archbuilder.group_instance` is a storey element: definition, reference point,
  rotation, mirror, the storey of each definition level, the excluded members, text exceptions
  (`name`, `number`) and offset exceptions, both keyed by member, and the fingerprint of each
  generated element as it was last written.
- **Generated elements.** Each instance generates ordinary elements on the mapped storeys (owner:
  the storey, or the generated wall for hosted kinds). Identifier: a version 8 identifier of the
  instance and the definition member ([`curtain::child_id`](../../crates/archbuilder-model/src/curtain.rs)
  with its own role), so the same after every regeneration. Element identifiers never change.
  The generated elements are derived data; the definition and the instance are the authority.
  A regeneration removes and recreates them in one transaction.
- **Direct edits.** A generated element changed outside the group commands no longer matches its
  fingerprint. A regeneration that would overwrite it is refused, and `archbuilder.group.edit`
  writes the element back into the definition first, then regenerates every instance.
- **Level mapping.** A placement maps each definition level to a storey of the same building, by
  elevation unless `levels` names the storeys. A mapped storey cannot be deleted
  (`archbuilder.storey.delete` refuses with `ab.cmd.group_level_mapped`) until the instance is
  separated. The document also blocks the deletion through the reference rule.
- **Separation.** `archbuilder.group.separate` creates ordinary copies with new identifiers (hosted
  elements follow their new host), deletes the generated elements and the instance, and returns
  the pairs.
- **IFC.** Each instance is an `IfcGroup` (type `Group instance`) that relates its generated
  elements, the same mechanism selection sets use. The definition is not exported.
- **Not in this decision.** Nested groups (a group inside a definition), copying a definition into
  another document or a package (a new file format needs the maintainer's approval first), and
  moving or copying an instance as a unit. Each is a follow-up.

## Consequences

- `.archb` gains two record kinds and the `ELEMENTS` list gains the instance kind; files from
  before load unchanged. Each field defines its canonical value.
- Readers that enumerate by type go through `spatial::objects`, which now hides definition
  members; code that must read them uses `spatial::objects_raw`.
- Generated elements are stored (regenerated, never authoritative). Their fingerprint check costs
  one hash per element per regeneration.
- Editing a generated element directly is refused by a later regeneration, so the user is told
  to use the group commands.

## Alternatives considered

- **Materialize nothing; derive instance members at every read.** Rejected: every reader, pick,
  solid and IFC writer iterates stored records, so the instance would be invisible to them.
- **Store definition members as nested data.** Rejected: the record schema has no typed value for
  a heterogeneous list of element records; owned records reuse the element schemas.
- **Instance owns generated elements.** Rejected: storey readers select elements by storey, so the
  generated walls must be owned by the storey.
