# Messages of the error codes of archbuilder-model (crates/archbuilder-model/src/error.rs).

ab-model-name-empty = The name cannot be empty.
ab-model-name-taken = The name "{ $name }" is already used here. Names are unique within their building, site or project, whatever the case of the letters.
ab-model-parent-invalid = "{ $name }" is not in a container of the right kind. A site belongs to the project, a building to a site, a storey or level to a building, and a wall to a storey.
ab-model-project-not-unique = A document holds exactly one project.
ab-model-level-invalid = The storey "{ $name }" needs a level of its own that belongs to the same building and is marked as a storey level.
ab-model-wall-too-short = The wall "{ $name }" has no length. Its baseline needs two different points, at least a millimeter apart.
ab-model-level-below-walls = The level cannot go there: the top of { $count } wall(s), such as "{ $name }", would not be above its base. Move or reconnect those walls first.
ab-model-wall-top-invalid = The top of the wall "{ $name }" is not above its base. Constrain it to a higher level or give it a height.
