//! The IFC export of a curved curtain wall: an `IfcCurtainWall` with flat plates and member pieces.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use common::{Env, export_curtain};
use serde_json::json;

#[test]
fn a_curved_curtain_wall_exports_valid_and_lossless() {
    // A curved curtain wall: IfcCurtainWall with plates and members.
    let env = Env::new();
    let ty = env.run(
        "archbuilder.curtain.type",
        json!({"name": "CW", "vertical": {"kind": "fixed_distance", "value": 1.0},
               "horizontal": {"kind": "fixed_number", "value": 1}}),
    )["type"]
        .as_str()
        .unwrap()
        .to_owned();
    env.wall(
        json!({"start": [5.0, 0.0], "end": [0.0, 5.0], "bulge": 0.4142135623730951,
        "type": ty, "height": 3.0}),
    );
    export_curtain(&env, "curved_curtain");
}
