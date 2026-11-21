//! The room tool and the pick priority of a plan view, headless: clicks inside the regions of a
//! house place numbered rooms and a click outside waits; what the hover would outline is what a
//! click picks, in the order door and window, wall, room, floor, level.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::sync::Arc;
use std::time::{Duration, Instant};

use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use arc_ui::ProductOverlay;
use archbuilder::picker::Picker;
use archbuilder::room_tool::RoomTool;
use archbuilder::room_view::{RoomView, ViewRect};
use archbuilder::wall_tool::Env;
use archbuilder_model::spatial;
use archbuilder_model::types::{DOOR, FLOOR, LEVEL, ROOM, WALL};
use archbuilder_model::units::LengthUnit;
use serde_json::json;

struct Rig {
    document: Document,
    registry: arc_command::Registry,
    session: Session,
    tool: RoomTool,
    seed: u64,
}

impl Rig {
    fn new() -> Self {
        let rig = Self::empty();
        let c = [[0.0, 0.0], [0.0, 6.0], [10.0, 6.0], [10.0, 0.0]];
        for k in 0..4 {
            rig.invoke(
                "archbuilder.wall.create",
                json!({"start": c[k], "end": c[(k + 1) % 4]}),
            );
        }
        rig.invoke(
            "archbuilder.wall.create",
            json!({"start": [5.0, 0.0], "end": [5.0, 6.0]}),
        );
        rig
    }

    fn empty() -> Self {
        let document = Document::open(
            Snapshot::empty(Arc::new(archbuilder_commands::types().unwrap())),
            Arc::new(Evaluators::new()),
            DocumentConfig {
                workers: 0,
                ..DocumentConfig::default()
            },
        )
        .unwrap();
        archbuilder_commands::install(&document);
        let mut session = Session::new(Surface::CommandLine, [Permission::Read, Permission::Edit]);
        session.units = vec![*LengthUnit::Meter.unit().unwrap()];
        let rig = Self {
            document,
            registry: archbuilder_commands::registry().unwrap(),
            session,
            tool: RoomTool::new(),
            seed: 1,
        };
        rig.invoke("archbuilder.project.new", json!({}));
        rig
    }

    fn invoke(&self, command: &str, params: serde_json::Value) {
        Invoker::new(&self.registry)
            .invoke(
                &self.document,
                &self.session,
                Invocation {
                    seed: 99,
                    ..Invocation::new(command, params)
                },
            )
            .unwrap();
    }

    fn step(
        &mut self,
        f: impl FnOnce(
            &mut RoomTool,
            &Env<'_>,
            &mut dyn FnMut(Invocation) -> arc_base::Result<arc_command::Outcome>,
        ) -> arc_base::Result<()>,
    ) {
        self.seed += 1;
        let invoker = Invoker::new(&self.registry);
        let env = Env {
            registry: &self.registry,
            document: &self.document,
            session: &self.session,
            selection: &[],
            seed: self.seed,
        };
        let (document, session) = (&self.document, &self.session);
        f(&mut self.tool, &env, &mut |inv| {
            invoker.invoke(document, session, inv)
        })
        .unwrap();
    }

    fn count(&self, key: &str) -> usize {
        spatial::objects(&self.document.snapshot(), key).len()
    }
}

#[test]
fn clicks_place_numbered_rooms_until_the_tool_stops_and_a_click_outside_waits() {
    let mut rig = Rig::new();
    rig.step(|t, env, run| t.begin(env, run));
    assert!(rig.tool.running());
    rig.step(|t, env, run| t.click(env, run, [50.0, 50.0]));
    assert_eq!(rig.count(ROOM), 0);
    assert!(rig.tool.take_error().is_some() && rig.tool.running());
    rig.step(|t, env, run| t.click(env, run, [2.5, 3.0]));
    rig.step(|t, env, run| t.click(env, run, [7.5, 3.0]));
    assert_eq!(rig.count(ROOM), 2);
    // A third click in an occupied region is refused and the tool keeps asking.
    rig.step(|t, env, run| t.click(env, run, [2.0, 2.0]));
    assert!(rig.tool.take_error().is_some() && rig.tool.running());
    assert_eq!(rig.count(ROOM), 2);
    rig.step(|t, _, _| {
        t.stop();
        Ok(())
    });
    assert!(!rig.tool.running());
}

fn pick(
    picker: &mut Picker,
    snapshot: &Snapshot,
    at: [f64; 2],
) -> Option<archbuilder::picker::Target> {
    picker.target(
        snapshot,
        [at[0] * 20.0, at[1] * 20.0],
        at,
        0.1,
        None,
        Instant::now(),
    )
}

#[test]
fn the_hover_and_the_click_see_one_list_walls_before_rooms_before_floors() {
    let rig = Rig::new();
    rig.invoke("archbuilder.floor.create", json!({"picks": [[2.5, 3.0]]}));
    rig.invoke("archbuilder.room.place_all", json!({}));
    rig.invoke(
        "archbuilder.door.place",
        json!({"wall": spatial::objects(&rig.document.snapshot(), WALL)[0].id.to_string(), "distance": 2.0}),
    );
    let snapshot = rig.document.snapshot();
    let kind = |id| spatial::type_of(&snapshot, id).unwrap();
    let mut picker = Picker::default();
    // On the drawn center line of the outer wall, over the floor's edge region: the wall.
    let on_wall = pick(&mut picker, &snapshot, [0.0, 4.5]).unwrap();
    assert_eq!(kind(on_wall.id), WALL);
    // Asking again (the hover does) gives the same answer as the click did.
    assert_eq!(pick(&mut picker, &snapshot, [0.0, 4.5]).unwrap(), on_wall);
    // Inside the house: the room, then the floor on the next click, then back to the room.
    let inside = pick(&mut picker, &snapshot, [2.5, 3.0]).unwrap();
    assert_eq!(kind(inside.id), ROOM);
    picker.commit(&snapshot, [50.0, 60.0], &inside, Instant::now());
    let next = pick(&mut picker, &snapshot, [2.5, 3.0]).unwrap();
    assert_eq!(kind(next.id), FLOOR);
    // Nothing but the level outline lies beyond the house.
    let far = pick(&mut picker, &snapshot, [-12.0, 0.0]).unwrap();
    assert_eq!(kind(far.id), LEVEL);
    assert!(pick(&mut picker, &snapshot, [30.0, 30.0]).is_none());
    // The door's opening comes before its wall (a vertical ray through it).
    let door = &spatial::objects(&snapshot, DOOR)[0];
    let wall = spatial::get(&snapshot, door.header.owner.unwrap()).unwrap();
    let w = archbuilder_model::wall::Wall::of(&snapshot, &wall).unwrap();
    let t = 2.0 / w.length();
    let at = [
        w.start[0] + (w.end[0] - w.start[0]) * t,
        w.start[1] + (w.end[1] - w.start[1]) * t,
    ];
    let over_door = pick(&mut picker, &snapshot, at).unwrap();
    assert_eq!(over_door.id, door.id);
}

#[test]
fn a_repeated_click_steps_down_the_list_until_escape_the_pointer_or_time_resets_it() {
    let rig = Rig::new();
    rig.invoke("archbuilder.floor.create", json!({"picks": [[2.5, 3.0]]}));
    rig.invoke("archbuilder.room.place_all", json!({}));
    let snapshot = rig.document.snapshot();
    let kind = |id| spatial::type_of(&snapshot, id).unwrap();
    let (pixel, at) = ([50.0, 60.0], [2.5, 3.0]);
    let now = Instant::now();
    let mut picker = Picker::default();
    let ask = |p: &mut Picker, pixel: [f64; 2], when: Instant| {
        p.target(&snapshot, pixel, at, 0.1, None, when).unwrap()
    };
    let first = ask(&mut picker, pixel, now);
    assert_eq!(kind(first.id), ROOM);
    picker.commit(&snapshot, pixel, &first, now);
    let soon = now + Duration::from_millis(500);
    assert_eq!(kind(ask(&mut picker, [52.0, 61.0], soon).id), FLOOR);
    // Moved more than a few pixels, or too long after the click, or after Escape: the top again.
    assert_eq!(kind(ask(&mut picker, [60.0, 60.0], soon).id), ROOM);
    assert_eq!(
        kind(ask(&mut picker, pixel, now + Duration::from_secs(2)).id),
        ROOM
    );
    picker.reset();
    assert_eq!(kind(ask(&mut picker, pixel, soon).id), ROOM);
}

#[test]
fn rooms_beyond_the_budget_are_counted_in_a_notice_and_other_storeys_do_not_use_it() {
    let rig = Rig::empty();
    // 15 x 10 cells of 1 m: sixteen vertical and eleven horizontal long walls.
    for k in 0..=15 {
        let x = f64::from(k);
        rig.invoke(
            "archbuilder.wall.create",
            json!({"start": [x, -1.0], "end": [x, 11.0]}),
        );
    }
    for k in 0..=10 {
        let y = f64::from(k);
        rig.invoke(
            "archbuilder.wall.create",
            json!({"start": [-1.0, y], "end": [16.0, y]}),
        );
    }
    rig.invoke("archbuilder.room.place_all", json!({}));
    // A box of four walls on the next storey: one room there.
    let snapshot = rig.document.snapshot();
    let storeys = spatial::storeys(&snapshot, spatial::buildings(&snapshot)[0].id);
    let upper = storeys[1].storey.id.to_string();
    for (a, b) in [
        ([0.0, 0.0], [4.0, 0.0]),
        ([4.0, 0.0], [4.0, 4.0]),
        ([4.0, 4.0], [0.0, 4.0]),
        ([0.0, 4.0], [0.0, 0.0]),
    ] {
        rig.invoke(
            "archbuilder.wall.create",
            json!({"storey": upper, "start": a, "end": b}),
        );
    }
    rig.invoke("archbuilder.room.place_all", json!({"storey": upper}));
    let snapshot = rig.document.snapshot();
    assert_eq!(spatial::objects(&snapshot, ROOM).len(), 151);
    let view = RoomView::default();
    view.set_active(Some(storeys[0].storey.id));
    view.set_view(Some(ViewRect {
        center: [7.5, 5.0],
        right: [1.0, 0.0],
        up: [0.0, 1.0],
        ppu: 80.0,
        size: [1300.0, 900.0],
    }));
    let overlays = view.overlays(&snapshot, "en-US");
    let labels: Vec<&str> = overlays
        .iter()
        .filter_map(|o| match o {
            ProductOverlay::Label { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    let tags = labels.iter().filter(|t| t.contains(" m²")).count();
    // The message isolates its number with invisible marks: read the digits.
    let hidden: usize = labels
        .iter()
        .find(|t| t.contains("room tags not shown"))
        .map_or(0, |t| {
            t.chars()
                .filter(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .unwrap()
        });
    // Every room of the active storey is tagged or counted; the other storey's are not.
    assert_eq!(tags + hidden, 150, "{tags} tags, {hidden} hidden");
    // Zoomed to a corner, only the rooms in view are drawn, all of them tagged.
    view.set_view(Some(ViewRect {
        center: [1.5, 1.5],
        right: [1.0, 0.0],
        up: [0.0, 1.0],
        ppu: 80.0,
        size: [400.0, 300.0],
    }));
    let near = view.overlays(&snapshot, "en-US");
    let near_tags = near
        .iter()
        .filter(|o| matches!(o, ProductOverlay::Label { text, .. } if text.contains(" m²")))
        .count();
    assert!((9..=40).contains(&near_tags), "{near_tags}");
}
