# Circulation

This area specifies vertical and horizontal circulation elements: stairs, ramps, railings, elevator
assemblies, moving walks, and the checks that keep them usable. Fabrication-level steel stairs and
ladders are specified in [22-steel-detailing.md](22-steel-detailing.md).

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Stair | Base level and offset, top level and offset, rise, runs, landings, supports, repeat members | Instance of a stair type; owns runs, landings and supports; may own a shaft opening and railings |
| Stair type | Calculation rules, construction (monolithic or assembled), tread, riser, nosing and support types | System family type |
| Run | Shape (straight, spiral, winder), width, tread and riser counts, path | Owned by a stair |
| Landing | Shape (automatic or sketched), thickness | Connects runs |
| Ramp | Path, slope, width, thickness, landings, form | Instance of a ramp type; may own railings |
| Railing | Path (3D polyline, open or closed), host (stair, ramp, floor edge or none) | Instance of a railing type |
| Railing type | Top rail, handrails, infill rails, baluster pattern, posts, panel infill, extensions | System family type |
| Elevator assembly | Cabin type, served levels with offsets, shaft wall type, landing door type, pit and overrun | Owns shaft walls, landing doors and cabin |
| Clearance result | Walking line, minimum clearance, location, obstructing element | Derived |

## Stairs

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-CIRC-001 | Stair by components | Assembles a stair from runs (straight, full-step spiral, center-ended spiral, L and U winders), automatic or sketched landings and supports (stringers, carriages) between a base and a top constraint. Riser count and actual riser height are derived from the total rise. | T1 | 2 |
| AB-CIRC-002 | Stair calculation rules | Holds per-type rules: maximum riser, minimum tread, minimum run width and a stride rule (a formula over riser and tread with allowed range). Violations appear as warnings on the stair and in the warnings register. | T1 | 2 |
| AB-CIRC-003 | Stair by sketch | Creates irregular stairs from sketched boundary, riser and path lines, with the same rules and outputs as component stairs. | T2 | 2 |
| AB-CIRC-004 | Stair construction | Configures monolithic or assembled construction, tread and riser thickness, nosing profile and length, slanted risers, and support connection at base and top. | T2 | 2 |
| AB-CIRC-005 | Multistorey stairs | Repeats a stair across selected levels; each member adapts its riser count to its own floor-to-floor height while following the source's plan position and type. Members accept explicit exceptions, and editing the source updates every member without an exception. | T1 | 2 |
| AB-CIRC-006 | Stair plan representation | Draws the cut line, the portion below the cut solid and the portion above dashed, a walking line with arrow and up or down text, and optional tread or riser numbers, on both connected levels. | T1 | 2 |
| AB-CIRC-007 | Headroom check | Computes the clearance between the walking surface and overhead elements along each run and landing, and reports the minimum clearance, its location and the obstructing element. Structure below the walking surface is excluded. | T3 | 2 |
| AB-CIRC-008 | Stair shaft coordination | Lets a stair own a shaft opening that cuts every floor it passes between its base and top levels; moving or deleting the stair updates or removes the opening. | T2 | 2 |

## Ramps and railings

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-CIRC-009 | Ramps | Creates ramps by run sketch or path with landings, width, thickness and solid or thick-slab form. A per-type maximum slope produces a warning on any run that exceeds it; changing one run's slope shifts downstream elevations only. | T1 | 2 |
| AB-CIRC-010 | Railings by path | Creates railings along a sketched 3D path or hosted on a stair, ramp or floor edge. Placing a stair or ramp creates railings on the chosen sides automatically, and hosted railings follow their host. | T1 | 2 |
| AB-CIRC-011 | Railing types | Defines top rail, handrails at several heights and offsets, infill rails, a baluster pattern (spacing, justification, count per tread), posts at start, end and corners, and panel infill. Actual spacing is computed in 3D along the path. | T1 | 2 |
| AB-CIRC-012 | Rail extensions and transitions | Adds handrail extensions at ends (horizontal, return to wall, return to floor) and transitions at landings and turns, with rails mitered or filleted at joins. | T2 | 2 |
| AB-CIRC-013 | Continuous rails | Keeps a top rail or handrail continuous across railing segments and hosts, joining segments with miters or fillets instead of separate pieces. | T2 | 2 |

## Lifts and moving walks

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-CIRC-014 | Elevator assemblies | Creates an elevator from a cabin type, a list of served levels with offsets, shaft wall and landing door types, pit depth and overrun. Shaft walls and a landing door per served level are generated; unserved levels keep shaft walls only; exports use the IFC transport element. | T2 | 2 |
| AB-CIRC-015 | Escalators and moving walks | Places escalators and moving walks as family-based assemblies between two levels with inclination, step width and landing lengths, and reserves their structural opening. | T3 | 3 |
| AB-CIRC-016 | Travel path measurement | Measures the shortest walking path between two points around walls, furniture and other obstacles with a clearance width, and keeps the path as an annotation that updates when the model changes. | T3 | 3 |
