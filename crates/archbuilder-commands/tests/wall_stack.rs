//! Stacked wall types.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use std::sync::Arc;

use arc_base::ObjectId;
use archbuilder_model::spatial;
use common::Env;
use serde_json::json;

#[test]
fn a_stacked_wall_shows_each_segments_layers_and_is_one_wall_for_hosting() {
    let env = Env::new();
    // Two basic types of the same thickness (0.3 m): a 200 mm block with 50 mm finish each side
    // (the cavity default is thicker; use single layers from the commands instead).
    let snapshot = env.snapshot();
    let types = archbuilder_model::element_type::of_kind(
        &snapshot,
        archbuilder_model::element_type::TypeKind::Wall,
    );
    let name = |t: &Arc<arc_doc::Object>| archbuilder_model::element_type::name(t).to_owned();
    let generic = types
        .iter()
        .find(|t| name(t).starts_with("Generic"))
        .unwrap();
    let partition = types
        .iter()
        .find(|t| name(t).starts_with("Interior"))
        .unwrap();
    let total = |t: &Arc<arc_doc::Object>| {
        archbuilder_model::element_type::Layers::from_values(&t.values)
            .unwrap()
            .total()
    };
    // The default types have different thicknesses: the stack is refused with the reason.
    assert!((total(generic) - total(partition)).abs() > 1e-6);
    let error = env
        .run(
            "archbuilder.wall.stack_type",
            json!({"segments": [{"type": generic.id.to_string(), "height": 1.0},
                                {"type": partition.id.to_string()}]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "ab.model.wall_stack_invalid");
    // Two types of one thickness stack: a segment of 1 m and the variable rest.
    let other = env
        .run(
            "archbuilder.type.duplicate",
            json!({"type": generic.id.to_string(), "name": "Generic copy"}),
        )
        .unwrap()
        .value;
    let copy = other["type"].as_str().unwrap().to_owned();
    // The copy has two layers of the same total thickness.
    let t = archbuilder_model::element_type::Layers::from_values(&generic.values)
        .unwrap()
        .total();
    env.run(
        "archbuilder.type.set",
        json!({"type": copy, "layers": [
            {"function": "finish", "material": "Plaster", "thickness": 0.05},
            {"function": "structure", "material": "Block", "thickness": t - 0.05}], "core": [1, 2]}),
    )
    .unwrap();
    let made = env
        .run(
            "archbuilder.wall.stack_type",
            json!({"name": "Plinth and field", "segments": [
                {"type": generic.id.to_string(), "height": 1.0},
                {"type": copy}]}),
        )
        .unwrap()
        .value;
    let stacked = made["type"].as_str().unwrap().to_owned();
    let before = env.dump();
    let wall = env
        .run(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [5.0, 0.0], "type": stacked, "height": 3.0}),
        )
        .unwrap()
        .value["wall"]
        .as_str()
        .unwrap()
        .parse::<ObjectId>()
        .unwrap();
    let snapshot = env.snapshot();
    let object = spatial::get(&snapshot, wall).unwrap();
    assert_eq!(
        archbuilder_model::cleanup::stack_ranges_of(&snapshot, &object),
        vec![(0.0, 1.0), (1.0, 3.0)]
    );
    // One wall: the full volume, in pieces that stop at 1 m.
    let pieces = archbuilder_model::cleanup::wall_pieces(&snapshot, &object)
        .unwrap()
        .unwrap();
    let volume: f64 = pieces.iter().map(|p| p.solid.volume()).sum();
    assert!((volume - 5.0 * t * 3.0).abs() < 1e-9, "{volume}");
    // The plinth (segment 0) is one layer, the field above it (segment 1) has two.
    let layers_of = |segment: usize| {
        let mut l: Vec<usize> = pieces
            .iter()
            .filter(|p| p.segment == segment)
            .map(|p| p.layer)
            .collect();
        l.sort_unstable();
        l.dedup();
        l
    };
    assert_eq!(layers_of(0), vec![0]);
    assert_eq!(layers_of(1), vec![0, 1]);
    // It hosts a door like any wall, and the type cannot be deleted while a stack uses it.
    env.run(
        "archbuilder.door.place",
        json!({"wall": wall, "distance": 2.5}),
    )
    .unwrap();
    let refused = env.run("archbuilder.type.delete", json!({"type": copy}));
    assert!(refused.is_err());
    assert!(env.document.undo().wait().unwrap().value);
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(env.dump(), before);
}
