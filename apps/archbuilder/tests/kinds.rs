//! Every element kind is handled wherever the older kinds are: a document with one element of
//! each kind of `types::ELEMENTS`, and for each one its plan points (zoom to selection, the
//! clipboard's base point), its name in the property grid, the plan's picking, the statistics,
//! Delete (one undo step, never the storey) and the clipboard.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::spatial;
use archbuilder_model::types::{self, ELEMENTS, STOREY};
use serde_json::{Value as Json, json};

struct Rig {
    document: Document,
    registry: arc_command::Registry,
    session: Session,
}

impl Rig {
    fn new() -> Self {
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
        let rig = Self {
            document,
            registry: archbuilder_commands::registry().unwrap(),
            session: Session::new(Surface::Ui, [Permission::Read, Permission::Edit]),
        };
        rig.run("archbuilder.project.new", json!({}), &[]);
        rig
    }

    fn run(&self, command: &str, params: Json, selection: &[ObjectId]) -> Json {
        let invocation = Invocation {
            seed: 11,
            selection: selection.to_vec(),
            ..Invocation::new(command, params)
        };
        Invoker::new(&self.registry)
            .invoke(&self.document, &self.session, invocation)
            .unwrap_or_else(|e| panic!("{command}: {e:?}"))
            .value
    }

    fn snapshot(&self) -> Snapshot {
        self.document.snapshot()
    }

    fn first(&self, key: &str) -> ObjectId {
        spatial::objects(&self.snapshot(), key)[0].id
    }

    /// One element of every kind.
    fn every_kind(&self) {
        let c = [[0.0, 0.0], [6.0, 0.0], [6.0, 4.0], [0.0, 4.0]];
        let walls: Vec<String> = (0..4)
            .map(|k| {
                let made = self.run(
                    "archbuilder.wall.create",
                    json!({"start": c[k], "end": c[(k + 1) % 4]}),
                    &[],
                );
                made["wall"].as_str().unwrap().to_owned()
            })
            .collect();
        self.run(
            "archbuilder.door.place",
            json!({"wall": walls[0], "distance": 1.5}),
            &[],
        );
        self.run(
            "archbuilder.window.place",
            json!({"wall": walls[2], "distance": 2.0}),
            &[],
        );
        self.run(
            "archbuilder.opening.place",
            json!({"wall": walls[1], "distance": 2.0, "width": 1.0, "height": 2.1}),
            &[],
        );
        self.run(
            "archbuilder.floor.create",
            json!({"picks": [[3.0, 2.0]]}),
            &[],
        );
        let floor = self.first(types::FLOOR);
        self.run(
            "archbuilder.room.place",
            json!({"points": [[3.0, 2.0]]}),
            &[],
        );
        self.run(
            "archbuilder.stair.create",
            json!({"start": [1.0, 1.0], "to": [4.0, 1.0]}),
            &[],
        );
        self.run(
            "archbuilder.railing.create",
            json!({"points": [[9.0, 0.0], [11.0, 0.0]]}),
            &[],
        );
        self.run(
            "archbuilder.ramp.create",
            json!({"start": [9.0, 3.0], "to": [15.0, 3.0]}),
            &[],
        );
        self.run(
            "archbuilder.roof.create",
            json!({"picks": [[3.0, 2.0]], "base_offset": 3.0}),
            &[],
        );
        self.run(
            "archbuilder.ceiling.create",
            json!({"picks": [[3.0, 2.0]]}),
            &[],
        );
        self.run(
            "archbuilder.shaft.create",
            json!({"points": [[20.0, 0.0], [21.0, 0.0], [21.0, 1.0], [20.0, 1.0]], "top_offset": 1.0}),
            &[],
        );
        self.run(
            "archbuilder.vertical_opening.create",
            json!({"host": floor.to_string(),
                   "points": [[4.5, 2.5], [5.5, 2.5], [5.5, 3.5], [4.5, 3.5]]}),
            &[],
        );
        self.run(
            "archbuilder.floor_edge.create",
            json!({"floor": floor.to_string()}),
            &[],
        );
        self.run(
            "archbuilder.roof_extrusion.create",
            json!({"plane": [[0.0, 10.0], [4.0, 10.0]], "profile": [[0.0, 3.0], [2.0, 4.0], [4.0, 3.0]],
                   "end": 3.0}),
            &[],
        );
        self.run(
            "archbuilder.grid.create",
            json!({"start": [-3.0, -3.0], "end": [-3.0, 12.0]}),
            &[],
        );
        self.run(
            "archbuilder.reference_plane.create",
            json!({"start": [-5.0, -3.0], "end": [16.0, -3.0]}),
            &[],
        );
    }
}

/// The element of `key`, and the elements it needs beside it in a selection to be copied (its
/// host).
fn with_host(s: &Snapshot, key: &str) -> (ObjectId, Vec<ObjectId>) {
    let object = spatial::objects(s, key)
        .into_iter()
        .find(|o| {
            // A hosted railing is the ramp's; the railing by path stands alone.
            key != types::RAILING
                || archbuilder_model::railing::Railing::from_values(&o.values)
                    .is_some_and(|r| r.host.is_none())
        })
        .unwrap();
    let host = object
        .header
        .owner
        .filter(|h| spatial::type_of(s, *h).as_deref() != Some(STOREY))
        .filter(|h| spatial::type_of(s, *h).as_deref() != Some(types::BUILDING));
    let mut ids = vec![object.id];
    ids.extend(host);
    (object.id, ids)
}

#[test]
fn every_element_kind_has_plan_points_a_name_a_pick_a_count_a_delete_and_a_copy() {
    let rig = Rig::new();
    rig.every_kind();
    let s = rig.snapshot();
    let rooms = archbuilder_model::pick::all_rooms(&s);
    let stats = archbuilder_commands::stats::statistics(&s);
    let storeys = spatial::objects(&s, STOREY).len();
    for key in ELEMENTS {
        assert!(!spatial::objects(&s, key).is_empty(), "no {key} made");
        let (id, copied) = with_host(&s, key);
        let points = archbuilder::selection::plan_points(&s, id);
        assert!(!points.is_empty(), "{key}: no plan points");
        assert!(
            archbuilder::properties::kind_message(key).is_some(),
            "{key}: no name"
        );
        let n = points.len() as f64;
        let center = points
            .iter()
            .fold([0.0, 0.0], |a, p| [a[0] + p[0] / n, a[1] + p[1] / n]);
        let picked = points
            .iter()
            .chain([&center])
            .any(|at| archbuilder_model::pick::stack(&s, &rooms, *at, 0.3, None).contains(&id));
        assert!(picked, "{key}: not picked in plan");
        assert!(
            stats.by_kind.iter().any(|k| k.kind == key && k.count > 0),
            "{key}: not counted"
        );
        let content = archbuilder_commands::clip::capture(&s, &copied).unwrap();
        let items = content["items"].as_array().unwrap();
        assert!(items.iter().any(|i| i["key"] == key), "{key}: not copied");
        // Delete takes the element, never the storey, in one undo step.
        rig.run("archbuilder.object.delete", json!({}), &[id]);
        let after = rig.snapshot();
        assert!(spatial::get(&after, id).is_none(), "{key}: not deleted");
        assert_eq!(spatial::objects(&after, STOREY).len(), storeys);
        rig.document.undo().wait().unwrap();
        assert!(spatial::get(&rig.snapshot(), id).is_some());
    }
}
