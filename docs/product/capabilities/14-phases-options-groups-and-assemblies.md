# Phases, design options, groups and assemblies

This area specifies the states and reuse structures of a model: construction phases and how views
display them, design options inside one document, model groups for repetition across storeys and
buildings, assemblies, and parts. Branches of a whole document through Arc Hub are specified in
[16-collaboration.md](16-collaboration.md).

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Phase | Name, order | Elements reference a created and a demolished phase |
| Phase display rule | Status (new, existing, demolished, temporary), display (shown, overridden, hidden), override graphics | Applied per view with the view's phase |
| Option set | Name, options, primary option | Elements belong to the main model or one option |
| Design option | Name, primary flag | Owns elements |
| Group definition | Name, member records, source frame | Instances share its content |
| Group instance | Definition, placement, level mapping, exclusions, permitted exceptions, identity map | Places a definition |
| Unit library entry | Identity, version, group content with types, materials and pinned families, source, license, placement rules, derived descriptors | Placed into documents as group instances |
| Assembly | Name, members, main member, assembly type | Owns assembly views |
| Part | Source element and layer, division geometry, independent flag | Derived from a layered element |

## Phases

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PHASE-001 | Phases | Defines an ordered list of phases; each element has a created phase and an optional demolished phase that must not precede it. | T1 | 3 |
| AB-PHASE-002 | Phase display rules | Displays elements in each view according to the view's phase and a display rule that shows, overrides or hides new, existing, demolished and temporary elements. Users define their own rules. | T1 | 3 |
| AB-PHASE-003 | Demolition | Marks selected elements as demolished in the view's phase in one command; hosted inserts are demolished with their host, and demolished elements stop bounding rooms in later phases. | T1 | 3 |
| AB-PHASE-004 | Phase-aware rooms and relations | Evaluates rooms, door room relations and schedules per phase, so a room exists only in the phases where its boundaries exist. | T2 | 3 |
| AB-PHASE-005 | Phase mapping for links | Maps each linked document's phases to this document's phases so linked elements display by the host view's phase. | T2 | 3 |

## Design options

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PHASE-006 | Option sets | Defines option sets each with options and exactly one primary option; every element belongs to the main model or to one option. | T1 | 3 |
| AB-PHASE-007 | Option editing and display | Edits one option at a time, moving elements into or out of it; each view shows the primary or a chosen option per set. Elements of the main model cannot join to elements of a non-displayed option. | T1 | 3 |
| AB-PHASE-008 | Option acceptance | Accepts the primary option of a set by merging its elements into the main model and deleting the other options, after listing what will be deleted. The command is undoable. | T1 | 3 |
| AB-PHASE-009 | Option comparison | Compares options side by side by area, counts, quantities, carbon and cost, and shows the differences in a table and in paired views. | T3 | 3 |

## Groups

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PHASE-010 | Model groups | Creates group definitions from a selection that is closed under dependencies (hosted inserts, owned openings and children join automatically). Instances share content; editing one instance updates all, per-instance member exclusions are allowed, and groups can nest. | T1 | 2 |
| AB-PHASE-011 | Cross-storey group placement | Places a group instance by mapping each source level to a target level of the same building, so a repeated apartment or core adapts to each storey's elevation. Removing a mapped level is rejected until the instance is separated. | T1 | 2 |
| AB-PHASE-012 | Permitted instance exceptions | Allows defined per-instance exceptions (room numbers and names, furniture offsets, marks) without breaking the instance's link to its definition. Exceptions are listed per instance and can be cleared. | T2 | 2 |
| AB-PHASE-013 | Group separation and reuse | Separates an instance into ordinary elements, saves groups to packages and documents, and loads them into other documents. | T2 | 2 |
| AB-PHASE-016 | Unit and core library | Saves a group (an apartment, a core) with its dependency closure, types, materials and pinned families as a versioned library entry with source, license and placement rules (allowed rotations, permitted exceptions), and places entries into other documents by level mapping as one undoable command. An entry is immutable per version: different content under the same identity and version is refused. Each entry carries derived descriptors (footprint per storey, net room areas and counts, named shaft axes) for placement checks. | T2 | 3 |

## Assemblies and parts

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-PHASE-014 | Assemblies | Creates named assemblies from elements; identical assemblies, compared by geometry and parameters, share an assembly type. Assembly views (plans, elevations, sections, 3D, schedules) are generated on request. | T2 | 3 |
| AB-PHASE-015 | Parts | Splits layered walls, floors, roofs and ceilings into parts per layer, divides parts by sketches or datums, merges parts, and shows parts, originals or both per view. Parts follow their source until made independent and appear in schedules and takeoffs. | T2 | 3 |
