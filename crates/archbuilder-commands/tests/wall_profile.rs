//! Elevation profiles of walls.

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
fn a_profile_replaces_the_rectangle_and_inserts_outside_it_refuse_the_edit() {
    let env = Env::new();
    let made = env
        .run(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [6.0, 0.0], "thickness": 0.2, "height": 3.0}),
        )
        .unwrap()
        .value;
    let id: ObjectId = made["wall"].as_str().unwrap().parse().unwrap();
    // A gable: 2 m high at the left end, 3 m at 3 m, 2 m at the right end, with a 1 x 1 hole.
    env.run(
        "archbuilder.wall.set_profile",
        json!({"wall": id, "loops": [
            [[0.0, 0.0], [6.0, 0.0], [6.0, 2.0], [3.0, 3.0], [0.0, 2.0]],
            [[1.0, 0.5], [2.0, 0.5], [2.0, 1.5], [1.0, 1.5]]]}),
    )
    .unwrap();
    let area = 6.0 * 2.0 + 0.5 * 6.0 * 1.0 - 1.0;
    assert!((volume(&env, id) - area * 0.2).abs() < 1e-6);
    // A door where the profile has its opening is refused, one in the material is allowed.
    let refused = env
        .run(
            "archbuilder.door.place",
            json!({"wall": id, "distance": 1.5}),
        )
        .unwrap_err();
    assert_eq!(refused.code(), "ab.model.insert_outside");
    env.run(
        "archbuilder.door.place",
        json!({"wall": id, "distance": 4.5}),
    )
    .unwrap();
    // A profile that would leave the door outside is refused and changes nothing.
    let before = env.dump();
    let error = env
        .run(
            "archbuilder.wall.set_profile",
            json!({"wall": id, "loops": [[[0.0, 0.0], [3.0, 0.0], [3.0, 2.0], [0.0, 2.0]]]}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "ab.model.wall_profile_inserts");
    assert_eq!(env.dump(), before);
    // Resetting restores the rectangle.
    env.run("archbuilder.wall.set_profile", json!({"wall": id}))
        .unwrap();
    assert!((volume(&env, id) - (6.0 * 3.0 * 0.2 - 0.9 * 2.1 * 0.2)).abs() < 1e-6);
    assert!(env.document.undo().wait().unwrap().value);
    assert!(env.document.undo().wait().unwrap().value);
}
