//! The IFC export of arc, slanted, tapered, profiled and banded walls: the file passes the
//! exporter's own read-back, loses nothing, and holds a body whose volume is the derived one.
//! With `ARCB_IFC_OUT` set to a directory, the files are also written there for an independent
//! validator.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use common::{Env, export};
use serde_json::json;

#[test]
fn every_wall_shape_exports_valid_and_lossless() {
    for (name, door, window) in [
        ("arc_plain", false, false),
        ("arc_door", true, false),
        ("arc_window", false, true),
    ] {
        let env = Env::new();
        let arc = env.wall(json!({"start": [5.0, 0.0], "end": [0.0, 5.0],
            "through": [3.5355339059327378, 3.5355339059327378], "thickness": 0.3, "height": 3.0}));
        if door {
            env.run(
                "archbuilder.door.place",
                json!({"wall": arc, "distance": 2.0}),
            );
        }
        if window {
            env.run(
                "archbuilder.window.place",
                json!({"wall": arc, "distance": 5.5}),
            );
        }
        export(&env, name);
    }
    // Arc wall with a door and a window, typed.
    let env = Env::new();
    let arc = env.wall(json!({"start": [5.0, 0.0], "end": [0.0, 5.0],
        "through": [3.5355339059327378, 3.5355339059327378], "thickness": 0.3, "height": 3.0}));
    env.run(
        "archbuilder.door.place",
        json!({"wall": arc, "distance": 2.0}),
    );
    env.run(
        "archbuilder.window.place",
        json!({"wall": arc, "distance": 5.5}),
    );
    let v = export(&env, "arc_wall");
    let exact = std::f64::consts::FRAC_PI_2 / 2.0 * (5.15_f64.powi(2) - 4.85_f64.powi(2)) * 3.0;
    assert!(v < exact && v > exact - 3.0, "{v} {exact}");

    // An arc joined to a straight wall, with a layered type.
    let env = Env::new();
    let arc = env.wall(
        json!({"start": [5.0, 0.0], "end": [0.0, 5.0], "bulge": 0.4142135623730951,
        "type": "Exterior masonry cavity", "height": 3.0}),
    );
    let _ = arc;
    env.wall(json!({"start": [0.0, 5.0], "end": [-4.0, 5.0], "type": "Exterior masonry cavity", "height": 3.0}));
    export(&env, "arc_joined_layered");

    // A slanted and a tapered wall.
    let env = Env::new();
    let slanted =
        env.wall(json!({"start": [0.0, 0.0], "end": [4.0, 0.0], "thickness": 0.3, "height": 3.0}));
    env.run(
        "archbuilder.wall.set_lean",
        json!({"walls": [slanted], "degrees": 10.0}),
    );
    let tapered =
        env.wall(json!({"start": [0.0, 4.0], "end": [4.0, 4.0], "thickness": 0.3, "height": 3.0}));
    env.run(
        "archbuilder.wall.set_lean",
        json!({"walls": [tapered], "exterior_degrees": 8.0, "interior_degrees": 0.0}),
    );
    export(&env, "slanted_tapered");

    // A profile with a hole, and a door.
    let env = Env::new();
    let w =
        env.wall(json!({"start": [0.0, 0.0], "end": [6.0, 0.0], "thickness": 0.2, "height": 3.0}));
    env.run(
        "archbuilder.wall.set_profile",
        json!({"wall": w, "loops": [
            [[0.0, 0.0], [6.0, 0.0], [6.0, 2.0], [3.0, 3.0], [0.0, 2.0]],
            [[1.0, 0.5], [2.0, 0.5], [2.0, 1.5], [1.0, 1.5]]]}),
    );
    env.run(
        "archbuilder.door.place",
        json!({"wall": w, "distance": 4.5}),
    );
    export(&env, "profile_wall");

    // Reference: a layered wall with a door, no shape at all.
    let env = Env::new();
    let w = env.wall(json!({"start": [0.0, 0.0], "end": [5.0, 0.0], "type": "Exterior masonry cavity", "height": 3.0}));
    env.run(
        "archbuilder.door.place",
        json!({"wall": w, "distance": 2.5}),
    );
    export(&env, "layered_door");

    // Bands: projecting and recessed.
    let env = Env::new();
    let w =
        env.wall(json!({"start": [0.0, 0.0], "end": [6.0, 0.0], "thickness": 0.2, "height": 3.0}));
    env.run(
        "archbuilder.door.place",
        json!({"wall": w, "distance": 3.0}),
    );
    env.run(
        "archbuilder.wall.set_bands",
        json!({"walls": [w], "bands": [
            {"face": "exterior", "position": 1.0, "width": 0.3, "depth": 0.05},
            {"face": "interior", "position": 2.5, "width": 0.2, "depth": -0.05}]}),
    );
    export(&env, "banded_wall");
}
