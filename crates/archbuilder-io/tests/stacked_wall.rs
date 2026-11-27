//! The IFC export of a stacked wall.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use archbuilder_model::spatial;
use common::{Env, export};
use serde_json::json;

#[test]
fn a_stacked_wall_exports_valid_and_lossless() {
    // A stacked wall: a plinth of one type and the rest of another of the same thickness.
    let env = Env::new();
    let types = archbuilder_model::element_type::of_kind(
        &env.document.snapshot(),
        archbuilder_model::element_type::TypeKind::Wall,
    );
    let generic = types
        .iter()
        .find(|t| archbuilder_model::element_type::name(t).starts_with("Generic"))
        .unwrap()
        .id;
    let total = archbuilder_model::element_type::Layers::from_values(
        &spatial::get(&env.document.snapshot(), generic)
            .unwrap()
            .values,
    )
    .unwrap()
    .total();
    let copy = env.run(
        "archbuilder.type.duplicate",
        json!({"type": generic.to_string(), "name": "Plastered"}),
    )["type"]
        .as_str()
        .unwrap()
        .to_owned();
    env.run(
        "archbuilder.type.set",
        json!({"type": copy, "layers": [
            {"function": "finish", "material": "Plaster", "thickness": 0.05},
            {"function": "structure", "material": "Block", "thickness": total - 0.05}], "core": [1, 2]}),
    );
    let stacked = env.run(
        "archbuilder.wall.stack_type",
        json!({"segments": [{"type": generic.to_string(), "height": 1.0}, {"type": copy}]}),
    )["type"]
        .as_str()
        .unwrap()
        .to_owned();
    let w =
        env.wall(json!({"start": [0.0, 0.0], "end": [5.0, 0.0], "type": stacked, "height": 3.0}));
    env.run(
        "archbuilder.door.place",
        json!({"wall": w, "distance": 2.5}),
    );
    export(&env, "stacked_wall");
}
