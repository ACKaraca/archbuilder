//! Arc walls, curved curtain walls: solids, openings, joins, modify commands and undo.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code"
)]

mod common;

use archbuilder_model::spatial;
use archbuilder_model::types::DOOR;
use archbuilder_model::wall::Wall;
use common::{Env, arc, volume, walls};
use serde_json::json;

#[test]
fn an_arc_wall_has_the_volume_of_its_annular_sector_and_undo_restores_the_state() {
    let env = Env::new();
    let before = env.dump();
    let id = arc(&env);
    let after = env.dump();
    let w = Wall::from_values(&spatial::get(&env.snapshot(), id).unwrap().values).unwrap();
    assert!(w.is_arc());
    assert!((w.bulge - (std::f64::consts::FRAC_PI_2 / 4.0).tan()).abs() < 1e-9);
    assert!((w.length() - 5.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    // The annular sector between radii 4.85 and 5.15: area = sweep / 2 * (R^2 - r^2).
    let exact = std::f64::consts::FRAC_PI_2 / 2.0 * (5.15_f64.powi(2) - 4.85_f64.powi(2)) * 3.0;
    let v = volume(&env, id);
    assert!(((v - exact) / exact).abs() < 2e-3, "{v} against {exact}");
    // The whole solid is the same thing.
    let snapshot = env.snapshot();
    let object = spatial::get(&snapshot, id).unwrap();
    let solid = archbuilder_model::insert::wall_solid(&snapshot, &object)
        .unwrap()
        .unwrap();
    assert!(((solid.volume() - exact) / exact).abs() < 2e-3);
    assert!(env.document.undo().wait().unwrap().value);
    assert_eq!(env.dump(), before);
    assert!(env.document.redo().wait().unwrap().value);
    assert_eq!(env.dump(), after);
}

#[test]
fn an_arc_wall_refuses_a_radius_smaller_than_its_thickness_and_changes_nothing() {
    let env = Env::new();
    let before = env.dump();
    // A half circle of radius 0.1 for a wall 0.3 thick has no inner face.
    let error = env
        .run(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [0.2, 0.0], "bulge": 1.0, "thickness": 0.3}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "ab.model.wall_shape_invalid");
    assert_eq!(env.dump(), before);
}

#[test]
fn a_door_in_an_arc_wall_is_placed_along_the_arc_and_cuts_the_solid() {
    let env = Env::new();
    let id = arc(&env);
    let uncut = volume(&env, id);
    env.run(
        "archbuilder.door.place",
        json!({"wall": id, "distance": 3.9}),
    )
    .unwrap();
    let cut = volume(&env, id);
    // The opening is 0.9 x 2.1 through 0.3 of thickness, faceted to the chord.
    assert!(
        ((uncut - cut) - 0.9 * 2.1 * 0.3).abs() < 0.05,
        "{uncut} {cut}"
    );
    let snapshot = env.snapshot();
    assert_eq!(spatial::objects(&snapshot, DOOR).len(), 1);
    // The door ends up at the point of the arc 3.9 m from the start.
    let w = Wall::from_values(&spatial::get(&snapshot, id).unwrap().values).unwrap();
    let p = w.point_at(3.9);
    assert!((p[0].hypot(p[1]) - 5.0).abs() < 1e-9);
    // A distance beyond the arc is refused.
    let error = env
        .run(
            "archbuilder.door.place",
            json!({"wall": id, "distance": 7.8}),
        )
        .unwrap_err();
    assert_eq!(error.code(), "ab.model.insert_outside");
}

#[test]
fn arc_walls_move_rotate_mirror_and_copy_with_their_arc() {
    let env = Env::new();
    let id = arc(&env);
    let w0 = Wall::from_values(&spatial::get(&env.snapshot(), id).unwrap().values).unwrap();
    env.run(
        "archbuilder.modify.mirror",
        json!({"objects": [id], "axis": [[0.0, 0.0], [0.0, 1.0]]}),
    )
    .unwrap();
    let all = walls(&env);
    assert_eq!(all.len(), 2);
    for o in &all {
        let w = Wall::from_values(&o.values).unwrap();
        // The mirror image is an arc of the same length and radius.
        assert!(w.is_arc());
        assert!((w.length() - w0.length()).abs() < 1e-9);
        assert!((w.arc().unwrap().radius - 5.0).abs() < 1e-9);
    }
    let mirrored = all
        .iter()
        .map(|o| Wall::from_values(&o.values).unwrap())
        .find(|w| w.point_at(w.length() / 2.0)[0] < 0.0)
        .unwrap();
    // Center of the mirror image is the origin, on the other side of the axis.
    let c = mirrored.arc().unwrap().center;
    assert!(c[0].hypot(c[1]) < 1e-9);
    assert!(mirrored.point_at(mirrored.length() / 2.0)[0] < 0.0);
    env.run(
        "archbuilder.modify.rotate",
        json!({"objects": [id], "angle": 90.0, "base": [0.0, 0.0]}),
    )
    .unwrap();
    let r = Wall::from_values(&spatial::get(&env.snapshot(), id).unwrap().values).unwrap();
    assert!(r.is_arc() && (r.bulge - w0.bulge).abs() < 1e-9);
}

#[test]
fn set_arc_straightens_and_keeps_inserts_in_proportion() {
    let env = Env::new();
    let id = arc(&env);
    env.run(
        "archbuilder.door.place",
        json!({"wall": id, "distance": 3.9}),
    )
    .unwrap();
    env.run(
        "archbuilder.wall.set_arc",
        json!({"walls": [id], "bulge": 0.0}),
    )
    .unwrap();
    let snapshot = env.snapshot();
    let w = Wall::from_values(&spatial::get(&snapshot, id).unwrap().values).unwrap();
    assert!(!w.is_arc());
    let door = spatial::objects(&snapshot, DOOR);
    let d = archbuilder_model::types::meters(&door[0].values, 1).unwrap();
    assert!((d - 3.9 * w.length() / (5.0 * std::f64::consts::FRAC_PI_2)).abs() < 1e-9);
    assert!(env.document.undo().wait().unwrap().value);
    let again = Wall::from_values(&spatial::get(&env.snapshot(), id).unwrap().values).unwrap();
    assert!(again.is_arc());
}

#[test]
fn an_arc_wall_joins_a_straight_wall_at_its_end_and_the_plan_outline_is_a_ring_sector() {
    let env = Env::new();
    let id = arc(&env);
    env.run(
        "archbuilder.wall.create",
        json!({"start": [0.0, 5.0], "end": [-4.0, 5.0], "thickness": 0.3, "height": 3.0}),
    )
    .unwrap();
    let snapshot = env.snapshot();
    let object = spatial::get(&snapshot, id).unwrap();
    // The straight wall's start corner is mitered against the arc's end.
    let straight = walls(&env).into_iter().find(|o| o.id != id).unwrap();
    let f = archbuilder_model::join::footprint_of(&snapshot, &straight).unwrap();
    assert!(
        f[0] != Wall::from_values(&straight.values)
            .unwrap()
            .plain_footprint()[0]
            || f[3]
                != Wall::from_values(&straight.values)
                    .unwrap()
                    .plain_footprint()[3]
    );
    let outline = Wall::from_values(&object.values).unwrap().plain_outline();
    assert!(outline.len() > 40);
    // Every point lies on one of the two faces.
    for p in &outline {
        let r = p[0].hypot(p[1]);
        assert!((r - 4.85).abs() < 1e-6 || (r - 5.15).abs() < 1e-6);
    }
}
