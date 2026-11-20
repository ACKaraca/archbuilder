# Messages of the error codes of archbuilder-model (crates/archbuilder-model/src/error.rs).

ab-model-name-empty = The name cannot be empty.
ab-model-name-taken = The name "{ $name }" is already used here. Names are unique within their building, site or project, whatever the case of the letters.
ab-model-parent-invalid = "{ $name }" is not in a container of the right kind. A site belongs to the project, a building to a site, and a storey or level to a building.
ab-model-project-not-unique = A document holds exactly one project.
ab-model-level-invalid = The storey "{ $name }" needs a level of its own that belongs to the same building and is marked as a storey level.
