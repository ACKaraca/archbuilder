//! A foreign element (an imported product without a native kind) is an element of its own kind:
//! picked in plan, moved, rotated, copied and pasted with its mesh transformed, refused a mirror
//! with the catalog code, deleted, and each step is undone in one step.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_base::id::Identifier;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Header, Snapshot, Value};
use archbuilder_model::foreign;
use archbuilder_model::spatial;
use archbuilder_model::types::{self, FOREIGN_ELEMENT, STOREY};
use serde_json::{Value as Json, json};

struct Env {
    document: Document,
    session: Session,
    registry: arc_command::Registry,
}

impl Env {
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
        let env = Self {
            document,
            session: Session::new(Surface::Ui, [Permission::Read, Permission::Edit]),
            registry: archbuilder_commands::registry().unwrap(),
        };
        env.run("archbuilder.project.new", json!({}), &[]).unwrap();
        env
    }

    fn run(
        &self,
        command: &str,
        params: Json,
        selection: &[ObjectId],
    ) -> arc_base::Result<Outcome> {
        let invocation = Invocation {
            seed: 31,
            selection: selection.to_vec(),
            ..Invocation::new(command, params)
        };
        Invoker::new(&self.registry).invoke(&self.document, &self.session, invocation)
    }

    fn ok(&self, command: &str, params: Json, selection: &[ObjectId]) -> Json {
        self.run(command, params, selection)
            .unwrap_or_else(|e| panic!("{command}: {e:?}"))
            .value
    }

    fn snapshot(&self) -> Snapshot {
        self.document.snapshot()
    }

    fn undo(&self) {
        assert!(self.document.undo().wait().unwrap().value);
    }

    fn storey(&self) -> ObjectId {
        spatial::objects(&self.snapshot(), STOREY)[0].id
    }

    /// The foreign elements of the document.
    fn foreign(&self) -> Vec<ObjectId> {
        spatial::objects(&self.snapshot(), FOREIGN_ELEMENT)
            .iter()
            .map(|o| o.id)
            .collect()
    }

    /// The plan box of element `id`: the lowest and the highest corner.
    fn bounds(&self, id: ObjectId) -> ([f64; 3], [f64; 3]) {
        let s = self.snapshot();
        foreign::bounds(&spatial::get(&s, id).unwrap().values).unwrap()
    }

    /// Creates an imported element of the box `lo`..`hi` in the storey, as the import writes one.
    fn create_foreign(&self, id: u128, lo: [f64; 3], hi: [f64; 3]) -> ObjectId {
        let storey = self.storey();
        let snapshot = self.snapshot();
        let mut values: Vec<Option<Value>> = snapshot
            .registry()
            .get(FOREIGN_ELEMENT)
            .map(|d| d.schema.fields.iter().map(|f| f.default.clone()).collect())
            .unwrap_or_default();
        if let Some(slot) = values.get_mut(types::foreign_element::NAME) {
            *slot = Some(Value::String("Column".to_owned()));
        }
        if let Some(slot) = values.get_mut(types::foreign_element::CLASS) {
            *slot = Some(Value::String("IfcColumn".to_owned()));
        }
        if let Some(slot) = values.get_mut(types::foreign_element::MESH) {
            *slot = foreign::box_value(lo, hi);
        }
        let object = ObjectId::from_u128(id);
        self.document
            .submit(move |txn| {
                txn.create(
                    FOREIGN_ELEMENT,
                    object,
                    Header {
                        owner: Some(storey),
                        ..Default::default()
                    },
                    values,
                )
            })
            .wait()
            .unwrap();
        object
    }
}

#[test]
fn a_foreign_element_is_picked_moved_copied_pasted_hidden_by_kind_and_deleted_with_undo() {
    let env = Env::new();
    let id = env.create_foreign(0xE0, [5.0, 5.0, 0.0], [6.0, 7.0, 2.0]);
    assert_eq!(env.foreign(), vec![id]);
    assert!(types::ELEMENTS.contains(&FOREIGN_ELEMENT));

    // Picked in plan over its box, and not beside it.
    let s = env.snapshot();
    let rooms = archbuilder_model::pick::all_rooms(&s);
    let at = [5.5, 6.0];
    assert!(
        archbuilder_model::pick::stack(&s, &rooms, at, 0.3, None).contains(&id),
        "not picked"
    );
    assert!(!archbuilder_model::pick::stack(&s, &rooms, [20.0, 20.0], 0.3, None).contains(&id));

    // Move: the stored mesh moves, the identifier stays.
    env.ok(
        "archbuilder.modify.move",
        json!({"objects": [id.to_string()], "displacement": [2.0, 0.0]}),
        &[],
    );
    assert_eq!(env.bounds(id).0[0], 7.0);
    env.undo();
    assert_eq!(env.bounds(id).0[0], 5.0);

    // Rotate a quarter turn about the origin: the box's x and y swap their extents.
    env.ok(
        "archbuilder.modify.rotate",
        json!({"objects": [id.to_string()], "base": [0.0, 0.0], "angle": "90"}),
        &[],
    );
    let (lo, hi) = env.bounds(id);
    assert!(
        (lo[0] + 7.0).abs() < 1e-6 && (hi[0] + 5.0).abs() < 1e-6,
        "{lo:?} {hi:?}"
    );
    assert!(
        (lo[1] - 5.0).abs() < 1e-6 && (hi[1] - 6.0).abs() < 1e-6,
        "{lo:?} {hi:?}"
    );
    env.undo();
    assert_eq!(env.bounds(id).0[0], 5.0);

    // A mirror is refused with its catalog code, and nothing changes.
    let err = env
        .run(
            "archbuilder.modify.mirror",
            json!({"objects": [id.to_string()], "axis": [[0.0, 0.0], [1.0, 0.0]], "keep": false}),
            &[],
        )
        .unwrap_err();
    assert_eq!(err.code(), "ab.cmd.foreign_mirror");
    assert_eq!(env.bounds(id).0[0], 5.0);

    // Copy keeps the source and adds a second element.
    let made = env.ok(
        "archbuilder.modify.copy",
        json!({"objects": [id.to_string()], "displacement": [0.0, 10.0]}),
        &[],
    );
    let copy: ObjectId = made["created"][0].as_str().unwrap().parse().unwrap();
    assert_ne!(copy, id);
    assert_eq!(env.bounds(copy).0[1], 15.0);
    assert_eq!(env.foreign().len(), 2);
    env.undo();
    assert_eq!(env.foreign(), vec![id]);

    // Clipboard: copy, then paste after a displacement; the paste has its own identifier.
    let content = archbuilder_commands::clip::capture(&env.snapshot(), &[id]).unwrap();
    assert!(
        content["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["key"] == FOREIGN_ELEMENT)
    );
    let pasted = env.ok(
        "archbuilder.clipboard.paste",
        json!({"objects": [id.to_string()], "displacement": [10.0, 0.0]}),
        &[],
    );
    let paste: ObjectId = pasted["created"][0].as_str().unwrap().parse().unwrap();
    assert_ne!(paste, id);
    assert_eq!(env.bounds(paste).0[0], 15.0);
    env.undo();
    assert_eq!(env.foreign(), vec![id]);
    // The clipboard content (another document's paste) carries the mesh too.
    let pasted = env.ok(
        "archbuilder.clipboard.paste",
        json!({"content": content, "displacement": [10.0, 0.0], "storey": env.storey()}),
        &[],
    );
    let paste: ObjectId = pasted["created"][0].as_str().unwrap().parse().unwrap();
    assert_eq!(env.bounds(paste).0[0], 15.0);
    env.undo();

    // Delete takes the element alone, in one step, and the storey stays.
    let storeys = spatial::objects(&env.snapshot(), STOREY).len();
    env.ok("archbuilder.object.delete", json!({}), &[id]);
    assert!(env.foreign().is_empty());
    assert_eq!(spatial::objects(&env.snapshot(), STOREY).len(), storeys);
    env.undo();
    assert_eq!(env.foreign(), vec![id]);
    assert_eq!(env.bounds(id).0[0], 5.0);
}
