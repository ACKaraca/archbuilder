//! Slanted and tapered walls.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use arc_base::ObjectId;
use archbuilder_model::spatial;
use common::{Env, volume};
use serde_json::json;

#[test]
fn a_slanted_wall_keeps_its_volume_and_a_tapered_one_loses_the_wedge() {
    let env = Env::new();
    let made = env
        .run(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [4.0, 0.0], "thickness": 0.3, "height": 3.0}),
        )
        .unwrap()
        .value;
    let id: ObjectId = made["wall"].as_str().unwrap().parse().unwrap();
    env.run(
        "archbuilder.wall.set_lean",
        json!({"walls": [id], "degrees": 10.0}),
    )
    .unwrap();
    let v = volume(&env, id);
    assert!((v - 4.0 * 0.3 * 3.0).abs() < 1e-6, "{v}");
    // The top is displaced sideways by 3 tan 10 degrees.
    let snapshot = env.snapshot();
    let object = spatial::get(&snapshot, id).unwrap();
    let solid = archbuilder_model::insert::wall_solid(&snapshot, &object)
        .unwrap()
        .unwrap();
    let bounds = solid.bounding_box();
    let shift = 3.0 * 10.0_f64.to_radians().tan();
    assert!((bounds.max().y - (0.15 + shift)).abs() < 1e-9);
    // A taper: the exterior (left, +y) face leans 10 degrees, the interior one stays.
    env.run(
        "archbuilder.wall.set_lean",
        json!({"walls": [id], "exterior_degrees": 10.0, "interior_degrees": 0.0}),
    )
    .unwrap();
    let t = volume(&env, id);
    let wedge = 4.0 * 0.5 * 3.0 * shift;
    assert!((t - (4.0 * 0.3 * 3.0 + wedge)).abs() < 1e-6, "{t}");
    // Too much taper leaves no thickness at the top: refused.
    let error = env
        .run(
            "archbuilder.wall.set_lean",
            json!({"walls": [id], "exterior_degrees": -50.0, "interior_degrees": 50.0}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "ab.model.wall_shape_invalid");
    assert!(env.document.undo().wait().unwrap().value);
    env.run(
        "archbuilder.wall.set_lean",
        json!({"walls": [id], "degrees": 0.0}),
    )
    .unwrap();
    assert!((volume(&env, id) - 3.6).abs() < 1e-9);
}
