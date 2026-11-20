# Messages of the error codes of archbuilder-model (crates/archbuilder-model/src/error.rs).

ab-model-name-empty = The name cannot be empty.
ab-model-name-taken = The name "{ $name }" is already used here. Names are unique within their building, site or project, whatever the case of the letters.
ab-model-parent-invalid = "{ $name }" is not in a container of the right kind. A site belongs to the project, a building to a site, a storey or level to a building, a wall to a storey, and a door or window to a wall.
ab-model-project-not-unique = A document holds exactly one project.
ab-model-level-invalid = The storey "{ $name }" needs a level of its own that belongs to the same building and is marked as a storey level.
ab-model-wall-too-short = The wall "{ $name }" has no length. Its baseline needs two different points, at least a millimeter apart.
ab-model-level-below-walls = The level cannot go there: the top of { $count } wall(s), such as "{ $name }", would not be above its base. Move or reconnect those walls first.
ab-model-wall-top-invalid = The top of the wall "{ $name }" is not above its base. Constrain it to a higher level or give it a height.
ab-model-insert-outside = "{ $name }" does not fit in the wall "{ $wall }": the opening must lie between { $from } m and { $to } m along the wall, clear of the corners.
ab-model-insert-too-tall = "{ $name }" reaches too high in the wall "{ $wall }": the top of the opening must stay below { $top } m.
ab-model-insert-overlap = "{ $name }" overlaps "{ $other }" in the same wall. Move it or make it narrower.
ab-model-insert-operation = "{ $name }" cannot operate as { $operation }: a door swings, and a window is fixed or casement.
ab-model-insert-frame = The frame of "{ $name }" leaves no clear opening. Make the opening larger or the frame bars narrower.
ab-model-insert-meets-wall = "{ $name }" would open into the wall "{ $other }", which meets or crosses its wall there. Move it clear of that wall.
ab-model-floor-malformed = The floor "{ $name }" has no usable boundary: each loop needs at least three points inside the allowed range, and its edges must be readable.
ab-model-floor-edge-short = The floor "{ $name }" has an edge shorter than a millimeter. Move the points apart or remove one.
ab-model-floor-self-intersecting = The boundary of the floor "{ $name }" crosses or touches itself. Redraw it so that no edges cross.
ab-model-floor-area = The floor "{ $name }" has no usable area. Its outer boundary must enclose at least 1 cm² and at most 1 000 km².
ab-model-floor-hole-outside = A hole of the floor "{ $name }" is not inside its outer boundary, or touches it. Keep the hole clear of the edges.
ab-model-floor-holes-overlap = Two holes of the floor "{ $name }" overlap or lie inside each other. Keep the holes apart.
