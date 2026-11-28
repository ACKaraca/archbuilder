//! Bands on wall faces.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use arc_base::ObjectId;
use common::{Env, volume};
use serde_json::json;

#[test]
fn bands_project_and_recess_and_stop_at_a_door() {
    let env = Env::new();
    let made = env
        .run(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [6.0, 0.0], "thickness": 0.2, "height": 3.0}),
        )
        .unwrap()
        .value;
    let id: ObjectId = made["wall"].as_str().unwrap().parse().unwrap();
    env.run(
        "archbuilder.door.place",
        json!({"wall": id, "distance": 3.0}),
    )
    .unwrap();
    let plain = volume(&env, id);
    // A projecting band 0.3 high and 0.05 deep along the whole exterior face at 1 m (the door's
    // 2.1 m head is above it: the band stops at the door's width).
    env.run(
        "archbuilder.wall.set_bands",
        json!({"walls": [id], "bands": [
            {"face": "exterior", "position": 1.0, "width": 0.3, "depth": 0.05}]}),
    )
    .unwrap();
    let with = volume(&env, id);
    assert!(
        (with - plain - (6.0 - 0.9) * 0.3 * 0.05).abs() < 1e-6,
        "{with} {plain}"
    );
    // A recess above the door's head (2.1 m) runs the whole length and takes its volume out.
    env.run(
        "archbuilder.wall.set_bands",
        json!({"walls": [id], "bands": [
            {"face": "exterior", "position": 2.5, "width": 0.2, "depth": -0.05}]}),
    )
    .unwrap();
    let recessed = volume(&env, id);
    assert!(
        (plain - recessed - 6.0 * 0.2 * 0.05).abs() < 1e-6,
        "{recessed} {plain}"
    );
    // A band outside the wall is refused.
    let error = env
        .run(
            "archbuilder.wall.set_bands",
            json!({"walls": [id], "bands": [{"face": "exterior", "position": 2.9, "width": 0.5, "depth": 0.05}]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "ab.model.wall_band_invalid");
}
