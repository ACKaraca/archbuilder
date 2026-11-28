//! The drawing of a plan view: the `arc-view` source of the model, its classes, symbols, joins,
//! back-link picking and its caches.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::{Cancel, ObjectId};
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use arc_view::{PrimitiveKind, ViewResult, VisibilityClass};
use archbuilder_model::plan::{self, AREA_KEY, Built};
use archbuilder_model::spatial;
use archbuilder_model::types::STOREY;
use serde_json::{Value as Json, json};

static SEEDS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(40_000);

struct Env {
    seed: u64,
    document: Document,
    session: Session,
    registry: arc_command::Registry,
}

impl Env {
    fn new() -> Self {
        let config = DocumentConfig {
            workers: 0,
            ..DocumentConfig::default()
        };
        let document = Document::open(
            Snapshot::empty(Arc::new(archbuilder_commands::types().unwrap())),
            Arc::new(Evaluators::new()),
            config,
        )
        .unwrap();
        archbuilder_commands::install(&document);
        let env = Self {
            seed: SEEDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            document,
            session: Session::new(Surface::Ui, [Permission::Read, Permission::Edit]),
            registry: archbuilder_commands::registry().unwrap(),
        };
        env.run("archbuilder.project.new", json!({})).unwrap();
        env
    }

    fn run(&self, command: &str, params: Json) -> arc_base::Result<Outcome> {
        let invocation = Invocation {
            seed: self.seed,
            ..Invocation::new(command, params)
        };
        Invoker::new(&self.registry).invoke(&self.document, &self.session, invocation)
    }

    fn id(outcome: &Outcome, key: &str) -> ObjectId {
        outcome.value[key].as_str().unwrap().parse().unwrap()
    }

    fn ground(&self) -> ObjectId {
        let s = self.document.snapshot();
        spatial::storeys(&s, spatial::buildings(&s)[0].id)[0]
            .storey
            .id
    }

    /// A 10 x 6 house of four walls, 3 m high, on `storey`; the walls.
    fn house(&self, storey: ObjectId) -> Vec<ObjectId> {
        let c = [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]];
        (0..4)
            .map(|k| {
                let made = self
                    .run(
                        "archbuilder.wall.create",
                        json!({"start": c[k], "end": c[(k + 1) % 4], "height": 3.0,
                               "storey": storey.to_string()}),
                    )
                    .unwrap();
                Self::id(&made, "wall")
            })
            .collect()
    }

    fn view(&self, storey: ObjectId) -> Arc<arc_doc::Object> {
        archbuilder_model::view::of_storey(&self.document.snapshot(), storey)
            .into_iter()
            .next()
            .unwrap()
    }

    fn built(&self, storey: ObjectId) -> Arc<Built> {
        self.filtered(storey, &plan::Filter::default())
    }

    fn filtered(&self, storey: ObjectId, filter: &plan::Filter) -> Arc<Built> {
        plan::build(
            &self.document.snapshot(),
            &self.view(storey),
            filter,
            &Cancel::new(),
        )
        .unwrap()
    }

    fn upper(&self) -> ObjectId {
        let s = self.document.snapshot();
        spatial::storeys(&s, spatial::buildings(&s)[0].id)[1]
            .storey
            .id
    }

    fn set(&self, object: ObjectId, field: &str, value: Json) {
        self.run(
            "archbuilder.property.set",
            json!({"objects": [object.to_string()], "field": field, "value": value}),
        )
        .unwrap();
    }
}

impl Env {
    /// Gives `walls` the default cavity wall type: several layers of different materials.
    fn cavity(&self, walls: &[ObjectId]) {
        let snapshot = self.document.snapshot();
        let layered = spatial::objects(&snapshot, archbuilder_model::types::WALL_TYPE)
            .into_iter()
            .find(|t| {
                archbuilder_model::element_type::Layers::from_values(&t.values)
                    .is_some_and(|l| l.layers.len() > 3)
            })
            .unwrap();
        for w in walls {
            self.set(*w, "type", json!(layered.id.to_string()));
        }
    }
}

fn links_of(built: &Built, source: ObjectId) -> Vec<(usize, usize)> {
    built.primitives_of(source)
}

fn classes(result: &ViewResult, source: ObjectId) -> Vec<VisibilityClass> {
    result
        .links
        .iter()
        .filter(|l| l.source == source)
        .map(|l| l.class)
        .collect()
}

#[test]
fn walls_give_one_part_per_layer_and_same_material_layers_join_across_an_l() {
    let env = Env::new();
    let ground = env.ground();
    let walls = env.house(ground);
    env.cavity(&walls);
    let built = env.built(ground);
    let result = &built.geometry;
    // Layers of the walls are separate parts, each with its own key: the fills of the walls
    // carry the keys of more than three layers.
    let keys: std::collections::BTreeSet<u32> = result
        .links
        .iter()
        .filter(|l| l.kind == PrimitiveKind::Fill && walls.contains(&l.source))
        .map(|l| l.sub_key)
        .collect();
    assert!(keys.len() >= 4, "{keys:?}");
    // At the corners the regions of one material of two walls were united into one fill: the
    // joined fill names the other wall, so no seam is drawn inside the material.
    assert!(result.links.iter().any(
        |l| l.kind == PrimitiveKind::Fill && l.joined.iter().any(|(id, _)| walls.contains(id))
    ));
    // Materials that differ make seams: more cut lines than the two outlines of one material.
    assert!(result.count(VisibilityClass::Cut) > 8);
}

#[test]
fn a_door_gives_its_swing_and_a_stair_its_arrow() {
    let env = Env::new();
    let ground = env.ground();
    let walls = env.house(ground);
    let door = Env::id(
        &env.run(
            "archbuilder.door.place",
            json!({"wall": walls[0].to_string(), "distance": 2.0}),
        )
        .unwrap(),
        "insert",
    );
    let stair = Env::id(
        &env.run(
            "archbuilder.stair.create",
            json!({"start": [1.0, 3.0], "to": [5.0, 3.0]}),
        )
        .unwrap(),
        "stair",
    );
    let built = env.built(ground);
    let symbols = |id: ObjectId| -> Vec<u32> {
        built
            .geometry
            .links
            .iter()
            .filter(|l| l.source == id && l.kind == PrimitiveKind::Symbol)
            .map(|l| l.sub_key)
            .collect()
    };
    let door_keys = symbols(door);
    assert!(
        door_keys
            .iter()
            .filter(|k| (100..AREA_KEY).contains(*k))
            .count()
            >= 2,
        "{door_keys:?}"
    );
    assert!(symbols(stair).contains(&1002), "the arrow head");
    assert!(symbols(stair).contains(&1001), "the walking line");
}

#[test]
fn a_slab_below_draws_beyond_and_a_roof_above_overhead() {
    let env = Env::new();
    let ground = env.ground();
    env.house(ground);
    let floor = Env::id(
        &env.run(
            "archbuilder.floor.create",
            json!({"points": [[-1.0, -1.0], [11.0, -1.0], [11.0, 7.0], [-1.0, 7.0]]}),
        )
        .unwrap(),
        "floor",
    );
    let roof = Env::id(
        &env.run(
            "archbuilder.roof.create",
            json!({"picks": [[5.0, 3.0]], "base_offset": 2.5}),
        )
        .unwrap(),
        "roof",
    );
    let built = env.built(ground);
    let floor_classes = classes(&built.geometry, floor);
    assert!(
        floor_classes.contains(&VisibilityClass::Beyond),
        "{floor_classes:?}"
    );
    assert!(!floor_classes.contains(&VisibilityClass::Overhead));
    let roof_classes = classes(&built.geometry, roof);
    assert!(
        roof_classes.contains(&VisibilityClass::Overhead),
        "{roof_classes:?}"
    );
    assert!(!roof_classes.contains(&VisibilityClass::Cut));
}

#[test]
fn the_cut_height_changes_what_is_cut() {
    let env = Env::new();
    let ground = env.ground();
    let walls = env.house(ground);
    let window = Env::id(
        &env.run(
            "archbuilder.window.place",
            json!({"wall": walls[0].to_string(), "distance": 5.0}),
        )
        .unwrap(),
        "insert",
    );
    let view = env.view(ground).id;
    env.set(view, "detail", json!("fine"));
    // A window's frame spans from its sill (0.9 m) up: the default cut at 1.2 m cuts it.
    let cut = classes(&env.built(ground).geometry, window);
    assert!(cut.contains(&VisibilityClass::Cut), "{cut:?}");
    // A cut below the sill leaves it above the cut: overhead.
    env.set(view, "cut", json!(0.5));
    let low = classes(&env.built(ground).geometry, window);
    assert!(
        !low.contains(&VisibilityClass::Cut) && low.contains(&VisibilityClass::Overhead),
        "{low:?}"
    );
}

#[test]
fn picking_through_back_links_finds_the_element_under_the_point() {
    let env = Env::new();
    let ground = env.ground();
    let walls = env.house(ground);
    let built = env.built(ground);
    // The south wall's body: 0.05 m north of its baseline. The drawing joins the four walls into
    // one region; with the picking order of the model the wall under the point comes first.
    let snapshot = env.document.snapshot();
    let at = [5.0, 0.05];
    let order = |built: &Built| {
        let stack = archbuilder_model::pick::stack(&snapshot, &[], at, 0.02, None);
        plan::hits(built, at, 0.02).filter(stack)
    };
    assert_eq!(order(&built).first(), Some(&walls[0]));
    // Open ground inside the house is no wall.
    let inside = plan::hits(&built, [5.0, 3.0], 0.02);
    assert!(
        inside
            .direct
            .iter()
            .chain(&inside.joined)
            .all(|id| !walls.contains(id))
    );
    // A wall hidden in the view has no drawing to pick.
    env.run(
        "archbuilder.view.hide_elements",
        json!({"view": env.view(ground).id.to_string(), "objects": [walls[0].to_string()]}),
    )
    .unwrap();
    let again = env.built(ground);
    assert!(links_of(&again, walls[0]).is_empty());
    let snapshot = env.document.snapshot();
    let stack = archbuilder_model::pick::stack(&snapshot, &[], [5.0, 0.05], 0.02, None);
    assert!(
        !plan::hits(&again, [5.0, 0.05], 0.02)
            .filter(stack)
            .contains(&walls[0])
    );
}

#[test]
fn the_view_rebuilds_on_an_element_edit_and_not_on_an_unrelated_change() {
    let env = Env::new();
    let ground = env.ground();
    let walls = env.house(ground);
    let made = env
        .run("archbuilder.room.place", json!({"points": [[5.0, 3.0]]}))
        .unwrap();
    let room: ObjectId = made.value["rooms"][0].as_str().unwrap().parse().unwrap();
    let first = env.built(ground);
    // A change that no drawing reads (the project's name) keeps both passes.
    let project = spatial::project(&env.document.snapshot()).unwrap().id;
    env.set(project, "name", json!("Renamed"));
    let same = env.built(ground);
    assert!(Arc::ptr_eq(&first.geometry, &same.geometry));
    assert!(Arc::ptr_eq(&first.notes, &same.notes));
    // A room renamed builds the notes again and leaves the geometry of the storey alone.
    env.set(room, "name", json!("Kitchen"));
    let renamed = env.built(ground);
    assert!(Arc::ptr_eq(&first.geometry, &renamed.geometry));
    assert!(!Arc::ptr_eq(&first.notes, &renamed.notes));
    // A wall moved builds the geometry again.
    env.set(walls[0], "start_y", json!(0.5));
    let moved = env.built(ground);
    assert!(!Arc::ptr_eq(&first.geometry, &moved.geometry));
    // The storeys above are untouched by a change on this one: their views keep their result.
    let storeys: Vec<_> = spatial::objects(&env.document.snapshot(), STOREY)
        .iter()
        .map(|s| s.id)
        .collect();
    assert_eq!(storeys.len(), 3);
}

#[test]
fn a_temporary_filter_hides_and_isolates_through_overrides_and_is_part_of_the_cache_key() {
    let env = Env::new();
    let ground = env.ground();
    let walls = env.house(ground);
    let all = env.built(ground);
    assert!(!links_of(&all, walls[0]).is_empty());
    let hide = plan::Filter {
        hidden: [walls[0]].into(),
        only: None,
    };
    let hidden = env.filtered(ground, &hide);
    assert!(links_of(&hidden, walls[0]).is_empty());
    assert!(!links_of(&hidden, walls[1]).is_empty());
    let only = plan::Filter {
        hidden: std::collections::BTreeSet::new(),
        only: Some([walls[2]].into()),
    };
    let isolated = env.filtered(ground, &only);
    assert!(!links_of(&isolated, walls[2]).is_empty());
    assert!(
        isolated
            .geometry
            .links
            .iter()
            .chain(&isolated.notes.links)
            .all(|l| l.source == walls[2])
    );
    // The view record itself is untouched, and without a filter every wall is back.
    assert!(
        archbuilder_model::view::PlanView::from_values(&env.view(ground).values)
            .unwrap()
            .hidden
            .is_empty()
    );
    assert!(!links_of(&env.built(ground), walls[0]).is_empty());
}

#[test]
fn the_room_outlines_follow_a_change_of_which_walls_bound_rooms() {
    let env = Env::new();
    let ground = env.ground();
    let walls = env.house(ground);
    let made = env
        .run("archbuilder.room.place", json!({"points": [[5.0, 3.0]]}))
        .unwrap();
    let room: ObjectId = made.value["rooms"][0].as_str().unwrap().parse().unwrap();
    let first = env.built(ground);
    assert!(!links_of(&first, room).is_empty());
    // The walls' type stops bounding rooms: the room has no enclosure, so its outline goes.
    let snapshot = env.document.snapshot();
    let wall = spatial::get(&snapshot, walls[0]).unwrap();
    let ty = archbuilder_model::element_type::type_of(
        &snapshot,
        archbuilder_model::element_type::TypeKind::Wall,
        &wall.values,
    )
    .unwrap();
    env.set(ty.id, "room_bounding", json!(false));
    let after = env.built(ground);
    assert!(!Arc::ptr_eq(&first.notes, &after.notes));
    assert_ne!(
        links_of(&first, room).len(),
        links_of(&after, room).len(),
        "the outline did not follow"
    );
}

#[test]
fn a_cut_wall_and_its_door_draw_no_overhead_copies_of_themselves() {
    let env = Env::new();
    let ground = env.ground();
    let walls = env.house(ground);
    env.cavity(&walls);
    let door = Env::id(
        &env.run(
            "archbuilder.door.place",
            json!({"wall": walls[0].to_string(), "distance": 2.0}),
        )
        .unwrap(),
        "insert",
    );
    env.set(env.view(ground).id, "detail", json!("fine"));
    let built = env.built(ground);
    let wall = classes(&built.geometry, walls[0]);
    assert!(wall.contains(&VisibilityClass::Cut), "{wall:?}");
    assert!(!wall.contains(&VisibilityClass::Overhead), "{wall:?}");
    let door = classes(&built.geometry, door);
    assert!(!door.contains(&VisibilityClass::Overhead), "{door:?}");
}

#[test]
fn a_tall_wall_and_a_roof_of_the_storey_below_show_in_the_upper_plan() {
    let env = Env::new();
    let ground = env.ground();
    let tall = Env::id(
        &env.run(
            "archbuilder.wall.create",
            json!({"start": [0.0, -3.0], "end": [10.0, -3.0], "height": 6.0,
                   "storey": ground.to_string()}),
        )
        .unwrap(),
        "wall",
    );
    let walls = env.house(ground);
    let roof = Env::id(
        &env.run(
            "archbuilder.roof.create",
            json!({"picks": [[5.0, 3.0]], "base_offset": 3.5, "storey": ground.to_string()}),
        )
        .unwrap(),
        "roof",
    );
    let upper = env.built(env.upper());
    let tall_classes = classes(&upper.geometry, tall);
    assert!(
        tall_classes.contains(&VisibilityClass::Cut),
        "{tall_classes:?}"
    );
    assert!(!links_of(&upper, roof).is_empty());
    // A wall of the storey below that ends at this level is not drawn.
    assert!(links_of(&upper, walls[0]).is_empty());
}

#[test]
fn a_stair_arriving_from_below_draws_its_treads_beyond_with_its_arrow() {
    let env = Env::new();
    let stair = Env::id(
        &env.run(
            "archbuilder.stair.create",
            json!({"start": [1.0, 3.0], "to": [5.0, 3.0]}),
        )
        .unwrap(),
        "stair",
    );
    let upper = env.built(env.upper());
    let treads: Vec<_> = upper
        .geometry
        .links
        .iter()
        .filter(|l| l.source == stair && l.sub_key >= plan::TREAD_KEY && l.sub_key < AREA_KEY)
        .collect();
    assert!(treads.len() > 5, "{}", treads.len());
    assert!(treads.iter().all(|l| l.class == VisibilityClass::Beyond));
    let keys: Vec<u32> = upper
        .geometry
        .links
        .iter()
        .filter(|l| l.source == stair)
        .map(|l| l.sub_key)
        .collect();
    assert!(keys.contains(&1002), "the arrow head");
}

#[test]
fn the_view_depth_reaches_a_slab_below_the_bottom() {
    let env = Env::new();
    let ground = env.ground();
    env.house(ground);
    let floor = Env::id(
        &env.run(
            "archbuilder.floor.create",
            json!({"points": [[-1.0, -1.0], [11.0, -1.0], [11.0, 7.0], [-1.0, 7.0]]}),
        )
        .unwrap(),
        "floor",
    );
    env.set(floor, "top_offset", json!(-0.3));
    assert!(classes(&env.built(ground).geometry, floor).is_empty());
    let view = env.view(ground).id;
    env.set(view, "depth", json!(-0.6));
    let deep = classes(&env.built(ground).geometry, floor);
    assert!(deep.contains(&VisibilityClass::Beyond), "{deep:?}");
    // The depth is never above the bottom.
    assert!(
        env.run(
            "archbuilder.property.set",
            json!({"objects": [view.to_string()], "field": "depth", "value": 0.5}),
        )
        .is_err()
    );
}

#[test]
fn ceilings_are_off_by_default_and_a_shown_ceiling_draws_a_part_per_layer_of_its_type() {
    let env = Env::new();
    let ground = env.ground();
    env.house(ground);
    let made = env
        .run("archbuilder.ceiling.create", json!({"picks": [[5.0, 3.0]]}))
        .unwrap();
    let ceiling = Env::id(&made, "ceiling");
    assert!(links_of(&env.built(ground), ceiling).is_empty());
    let view = env.view(ground).id;
    env.set(view, "show_ceilings", json!(true));
    let built = env.built(ground);
    let keys: std::collections::BTreeSet<u32> = built
        .geometry
        .links
        .iter()
        .filter(|l| l.source == ceiling)
        .map(|l| l.sub_key)
        .collect();
    let snapshot = env.document.snapshot();
    let layers = archbuilder_model::element_type::type_of(
        &snapshot,
        archbuilder_model::element_type::TypeKind::Ceiling,
        &spatial::get(&snapshot, ceiling).unwrap().values,
    )
    .and_then(|t| archbuilder_model::element_type::Layers::from_values(&t.values))
    .map_or(1, |l| l.layers.len());
    assert!(!keys.is_empty());
    assert_eq!(keys.len(), layers, "{keys:?}");
}

#[test]
fn ramps_and_railings_are_drawn_under_their_own_categories_and_not_under_stairs() {
    let env = Env::new();
    let ground = env.ground();
    env.house(ground);
    let stair = Env::id(
        &env.run(
            "archbuilder.stair.create",
            json!({"start": [1.0, 1.0], "to": [4.0, 1.0]}),
        )
        .unwrap(),
        "stair",
    );
    let made = env
        .run(
            "archbuilder.ramp.create",
            json!({"start": [1.0, 4.0], "to": [8.0, 4.0], "rise": 0.5, "railings": ["left"]}),
        )
        .unwrap();
    let ramp = Env::id(&made, "ramp");
    let railing: ObjectId = made.value["railings"][0].as_str().unwrap().parse().unwrap();
    let view = env.view(ground).id;
    let drawn = |env: &Env, id: ObjectId| !links_of(&env.built(ground), id).is_empty();
    assert!(drawn(&env, stair) && drawn(&env, ramp) && drawn(&env, railing));
    // Each flag takes away its own kind and leaves the others.
    for (field, gone) in [
        ("show_stairs", stair),
        ("show_ramps", ramp),
        ("show_railings", railing),
    ] {
        env.set(view, field, json!(false));
        for id in [stair, ramp, railing] {
            assert_eq!(drawn(&env, id), id != gone, "{field}");
        }
        env.set(view, field, json!(true));
    }
    // A view from a file before the two flags shows both.
    let snapshot = env.document.snapshot();
    let object = spatial::get(&snapshot, view).unwrap();
    let mut values = object.values.clone();
    values.truncate(archbuilder_model::types::plan_view::SHOW_RAILINGS);
    let settings = archbuilder_model::view::PlanView::from_values(&values).unwrap();
    assert!(
        settings.shows(archbuilder_model::view::Category::Ramp)
            && settings.shows(archbuilder_model::view::Category::Railing)
    );
    // The record round-trips through the settings, flags and all.
    assert_eq!(
        archbuilder_model::view::PlanView::from_values(&settings.values()),
        Some(settings)
    );
}
