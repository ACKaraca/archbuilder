# 0001 Three editing contexts for views and drawings

Status: Accepted

## Context

Construction documents need two kinds of content: drawings that must stay true to the model
(plans, sections, elevations, schedules) and drawing detail that the model does not and should not
contain (insulation patterns, flashing, manufacturer details, hand-refined enlarged details). If
drawing detail is forced into the model, models become heavy and modeling effort is spent on 2D
results. If model views can be edited as lines, drawings silently stop matching the model. Teams
also need finished details that stay fixed while the design continues to change.

## Decision

ArchBuilder has exactly three editing contexts, described in
[views-and-documentation.md](../architecture/views-and-documentation.md):

1. **Linked model views.** Projections of the model. Selecting a projected element and editing it
   runs a model command. Projected geometry is never editable as drafting lines.
2. **View detail layers.** Each model view owns a layer of drafting entities and annotations,
   anchored to model references or view coordinates, stored separately and never changing the
   model.
3. **Detached drawings.** Independent drafting documents created explicitly from a view's
   projection and selected detail items, with new identities and a recorded source view and
   revision. They never write back to the model, and model changes never alter them.

Commands are routed by context; a command batch never mixes model and drafting targets. Conversion
from drafting entities to model elements is a separate explicit command.

## Consequences

- Drawings in linked views are always derived and can be rebuilt; detail layers survive model
  changes through anchors, and anchors that fail become orphaned rather than wrong.
- Detached drawings give teams frozen details with a visible source-changed indicator, at the cost
  of manual refresh by detaching again.
- The interface must always show the active context.
- The drafting engine (`arc-draft`) is the single implementation for detail layers, drafting views,
  detached drawings and sheets, shared with DraftBuilder.

## Alternatives considered

- **Editable projections with override records.** Lets users trim projected lines; rejected because
  overrides diverge silently from the model and cannot be validated.
- **Two contexts only (model views and drafting views).** Rejected because every detail would either
  lose its link to the model or have to be redrawn from scratch.
- **Automatic synchronization of detached drawings.** Rejected because it would overwrite deliberate
  hand edits; detached means independent.

## Reopen when

Users repeatedly need a detail that follows some model changes but not others, and anchored detail
layers plus explicit re-detachment cannot express it.
