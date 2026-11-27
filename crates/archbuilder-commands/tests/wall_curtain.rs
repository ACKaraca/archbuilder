//! Curved curtain walls: flat panels between vertical lines with their chord deviation.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use arc_base::ObjectId;
use archbuilder_model::spatial;
use common::Env;
use serde_json::json;

#[test]
fn a_curved_curtain_wall_has_flat_panels_between_vertical_lines_with_their_chord_deviation() {
    let env = Env::new();
    let made = env
        .run(
            "archbuilder.curtain.type",
            json!({"name": "CW", "vertical": {"kind": "fixed_distance", "value": 1.0},
                   "horizontal": {"kind": "fixed_number", "value": 1}}),
        )
        .unwrap()
        .value;
    let ty = made["type"].as_str().unwrap().to_owned();
    // A quarter circle of radius 5 (7.85 m along the arc): 8 cells of about 0.98 m.
    let id = env
        .run(
            "archbuilder.wall.create",
            json!({"start": [5.0, 0.0], "end": [0.0, 5.0], "bulge": (std::f64::consts::FRAC_PI_2 / 4.0).tan(),
                   "type": ty, "height": 3.0}),
        )
        .unwrap()
        .value["wall"]
        .as_str()
        .unwrap()
        .parse::<ObjectId>()
        .unwrap();
    let snapshot = env.snapshot();
    let d =
        archbuilder_model::curtain::Derived::of(&snapshot, &spatial::get(&snapshot, id).unwrap())
            .unwrap();
    assert!(d.panels.len() >= 12);
    let solids = d.solids().unwrap();
    assert!(!solids.is_empty());
    // Every panel reports the sagitta of the arc over its width: r - sqrt(r^2 - (w/2)^2) at the
    // middle of the wall (radius 5 for a wall of thickness 0.15 on its center... the glass sits
    // on the middle of the thickness).
    for p in &d.panels {
        let r = d
            .wall
            .radius_at((d.wall.extent().0 + d.wall.extent().1) / 2.0)
            .unwrap();
        let h = p.width() / 2.0;
        assert!((p.deviation - (r - (r * r - h * h).sqrt())).abs() < 1e-12);
        assert!(p.deviation > 0.0 && p.deviation < 0.05);
    }
    // The schedule carries the deviation.
    let rows = archbuilder_commands::curtain::schedule(&snapshot, Some(id));
    assert!(rows[0].panels.iter().all(|p| p.chord_deviation > 0.0));
    // Panels lie on the arc: every glass vertex is within the deviation of the circle.
    for s in &solids {
        if s.class == archbuilder_model::curtain::Class::Glass {
            for v in s.solid.vertices() {
                let r = v.x.hypot(v.y);
                assert!(r < 5.0 + 0.1 && r > 5.0 - 0.1);
            }
        }
    }
}
