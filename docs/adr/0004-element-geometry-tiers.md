# 0004 Element geometry from planar generators, exact solids only where needed

Status: Accepted

## Context

Most building elements (walls, floors, roofs, ceilings, stairs, openings) are bounded by planes and
by surfaces swept from lines and arcs. They are numerous and regenerate often. Family forms
(blends, revolves, sweeps) and imported solids need exact curved geometry and general booleans. The
suite provides an owned polyhedral and profile tier (`arc-geom`) and an exact boundary-representation
tier (`arc-brep`) whose first implementation delegates robust operations to an isolated kernel.

## Decision

- System element generators produce closed solids from profiles, layer planes and plane clipping in
  `arc-geom`, with line and arc carriers kept analytic and per-face provenance (element, layer, face
  role, source edge) attached. Joins, roof cells and host cuts operate in this tier.
- Family forms that are curved, and imported STEP solids, use `arc-brep`. Only `archbuilder-model`
  links it, and element generators call it only for those forms.
- Curved surfaces are tessellated for display and views with a stated chord tolerance recorded in
  outputs; cut regions of analytic profiles are computed analytically.
- Every generator has explicit resource limits and rejects invalid input instead of producing
  non-manifold output.

## Consequences

- The common path is pure Rust, fast and deterministic, and regenerates in parallel.
- Persistent face roles are defined by generators, which gives stable anchors for dimensions and
  constraints.
- Elements that would need general curved booleans (for example a curved wall cut by an arbitrary
  curved void) are produced through the exact tier, at higher cost, or rejected with a reason until
  supported.

## Alternatives considered

- **Exact B-rep for every element.** Uniform but slower, couples every element to the kernel tier,
  and makes stable face naming harder; rejected.
- **Meshes for every element.** Fast but loses exact cut regions and analytic edges needed for
  drawings and exchange; rejected.

## Reopen when

Profiling shows the exact tier is fast enough for system elements on the housing fixtures, or a
required element kind cannot be generated reliably in the planar tier.
