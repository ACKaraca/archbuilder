//! Every element kind is handled wherever the older kinds are: a document with one element of
//! each kind of `types::ELEMENTS`, and for each one its plan points (zoom to selection, the
//! clipboard's base point), its name in the property grid, the plan's picking, the statistics,
//! Delete (one undo step, never the storey) and the clipboard.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "tests"
)]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_base::id::Identifier;
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
        let roof = self.first(types::ROOF);
        self.run(
            "archbuilder.dormer.create",
            json!({"roof": roof.to_string(), "center": [3.0, 0.8], "width": 1.0, "depth": 1.0}),
            &[],
        );
        self.run(
            "archbuilder.roof_edge.create",
            json!({"roof": roof.to_string()}),
            &[],
        );
        self.run(
            "archbuilder.elevator.create",
            json!({"center": [20.0, 8.0]}),
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
        // A group of the walls of the first room, placed beside it: its instance is an element.
        let group = self.run(
            "archbuilder.group.create",
            json!({"name": "Unit", "ids": [walls[0].clone(), walls[1].clone()]}),
            &[],
        )["group"]
            .as_str()
            .unwrap()
            .to_owned();
        let storey = self.first(STOREY).to_string();
        self.run(
            "archbuilder.group.place",
            json!({"group": group, "storey": storey, "origin": [30.0, 0.0]}),
            &[],
        );
        self.component();
        self.hosted(&walls[3]);
        self.foreign();
    }

    /// An imported element: a box in the storey, as the import writes one.
    fn foreign(&self) {
        let storey = self.first(STOREY);
        let mut values = self.blank(types::FOREIGN_ELEMENT);
        if let Some(slot) = values.get_mut(types::foreign_element::NAME) {
            *slot = Some(arc_doc::Value::String("Column".to_owned()));
        }
        if let Some(slot) = values.get_mut(types::foreign_element::MESH) {
            *slot = archbuilder_model::foreign::box_value([25.0, 0.0, 0.0], [26.0, 1.0, 2.0]);
        }
        self.document
            .submit(move |txn| {
                txn.create(
                    types::FOREIGN_ELEMENT,
                    ObjectId::from_u128(0xD0),
                    arc_doc::Header {
                        owner: Some(storey),
                        ..Default::default()
                    },
                    values,
                )
            })
            .wait()
            .unwrap();
    }

    /// The default values of a record of `key`.
    fn blank(&self, key: &str) -> Vec<Option<arc_doc::Value>> {
        self.snapshot()
            .registry()
            .get(key)
            .map(|d| d.schema.fields.iter().map(|f| f.default.clone()).collect())
            .unwrap_or_default()
    }

    /// A loaded family with one type and a component of it.
    fn component(&self) {
        use archbuilder_model::family::fixtures::table;
        use archbuilder_model::family::loaded::Component;
        let storey = self.first(STOREY);
        let ty = self.load_family(&table(), 0xB0);
        let comp = Component {
            id: ObjectId::from_u128(0xB3),
            storey: Some(storey),
            name: String::new(),
            family_type: ty,
            offset: 0.0,
            origin: [20.0, 20.0],
            rotation: 0.0,
            mirror: false,
            values: Default::default(),
        };
        self.document
            .submit(move |txn| {
                txn.create(
                    types::COMPONENT,
                    ObjectId::from_u128(0xB3),
                    arc_doc::Header {
                        owner: Some(storey),
                        ..Default::default()
                    },
                    comp.record(),
                )
            })
            .wait()
            .unwrap();
    }

    /// A hosted instance of the hosted door family, in the wall `wall` (a wall without openings).
    fn hosted(&self, wall: &str) {
        use archbuilder_model::family::fixtures::hosted_door;
        let ty = self.load_family(&hosted_door(), 0xC0);
        self.run(
            "archbuilder.family_insert.place",
            json!({"host": wall, "type": ty.to_string(), "distance": 1.0}),
            &[],
        );
    }

    /// Loads `definition` with its family, one type, all identifiers from `base`; returns the
    /// family type.
    fn load_family(
        &self,
        definition: &archbuilder_model::family::def::Definition,
        base: u128,
    ) -> ObjectId {
        use archbuilder_model::family::loaded::{FamilyType, Loaded};
        use archbuilder_model::family::package::publish;
        let project = spatial::project(&self.snapshot()).unwrap().id;
        let published = publish(
            definition,
            None,
            archbuilder_model::family::Version {
                major: 1,
                minor: 0,
                patch: 0,
            },
        )
        .unwrap();
        let ty = definition.types.first().unwrap();
        let values = ty.values.clone();
        let name = ty.name.clone();
        let (f, t) = (ObjectId::from_u128(base + 1), ObjectId::from_u128(base + 2));
        let definition = definition.clone();
        self.document
            .submit(move |txn| {
                let owned = |owner| arc_doc::Header {
                    owner: Some(owner),
                    ..Default::default()
                };
                txn.create(
                    types::FAMILY,
                    f,
                    owned(project),
                    Loaded::values(&published, &definition),
                )?;
                txn.create(
                    types::FAMILY_TYPE,
                    t,
                    owned(project),
                    FamilyType::record(&name, f, &values),
                )
            })
            .wait()
            .unwrap();
        t
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

#[test]
fn a_component_moves_mirrors_and_copies_like_any_element() {
    use archbuilder_model::family::loaded::Component;
    let rig = Rig::new();
    rig.component();
    let id = rig.first(types::COMPONENT);
    let read = |rig: &Rig, id| Component::of(&spatial::get(&rig.snapshot(), id).unwrap()).unwrap();
    rig.run(
        "archbuilder.modify.move",
        json!({"displacement": [1.0, 2.0]}),
        &[id],
    );
    assert_eq!(read(&rig, id).origin, [21.0, 22.0]);
    // A mirror across the x axis keeps the point's x, flips y and the handedness.
    rig.run(
        "archbuilder.modify.mirror",
        json!({"axis": [[0.0, 0.0], [1.0, 0.0]], "keep": false}),
        &[id],
    );
    let m = read(&rig, id);
    assert!(m.mirror);
    assert!((m.origin[0] - 21.0).abs() < 1e-9 && (m.origin[1] + 22.0).abs() < 1e-9);
    let made = rig.run(
        "archbuilder.modify.copy",
        json!({"displacement": [5.0, 0.0]}),
        &[id],
    );
    let copy = made["created"][0].as_str().unwrap().parse().unwrap();
    assert_eq!(read(&rig, copy).origin[0], 26.0);
    assert_eq!(spatial::objects(&rig.snapshot(), types::COMPONENT).len(), 2);
}

#[test]
fn a_foreign_element_is_hidden_and_isolated_by_kind() {
    let rig = Rig::new();
    rig.every_kind();
    let s = rig.snapshot();
    let id = rig.first(types::FOREIGN_ELEMENT);
    let wall = rig.first(types::WALL);
    let mut vis = archbuilder::visibility::Visibility::default();
    vis.hide_kinds(&[types::FOREIGN_ELEMENT]);
    assert!(!vis.shows(&s, id), "hidden by kind");
    assert!(vis.shows(&s, wall));
    vis.reset();
    vis.isolate_kinds(&[types::FOREIGN_ELEMENT]);
    assert!(vis.shows(&s, id), "isolated by kind");
    assert!(!vis.shows(&s, wall));
}
