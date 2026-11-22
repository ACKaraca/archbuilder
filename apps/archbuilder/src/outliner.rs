//! The outliner: the project, its site, the buildings of the site, the storeys of each
//! building and the walls of each storey, with the doors and windows of each wall under it, as a tree labelled with the names the user gave (a wall, floor, door or window also with its
//! type's name in brackets), the highest storey at the top as in
//! an elevation drawing. Selecting a row runs `archbuilder.object.select`, which selects a storey
//! with its level; renaming a row runs `archbuilder.object.rename`.

use std::collections::{BTreeMap, BTreeSet};

use arc_app::commands;
use arc_app::shell::{Action, Panel, PanelContext};
use arc_base::ObjectId;
use arc_doc::Snapshot;
use arc_ui::Localize;
use arc_ui::egui::Ui;
use arc_ui::{Intent, TreeCommands, TreeItem, TreeSource, TreeView, matches_filter};
use archbuilder_commands::structure::{OBJECT_RENAME, OBJECT_SELECT, PROJECT_NEW};
use archbuilder_model::spatial;
use archbuilder_model::types::{self, BUILDING, SITE};

/// A message of the application.
struct Message {
    message_id: &'static str,
}

const NO_PROJECT: Message = Message {
    message_id: "ab-outliner-no-project",
};
const CREATE_PROJECT: Message = Message {
    message_id: "ab-outliner-create-project",
};

fn say(localize: &dyn Localize, message: &Message) -> String {
    localize
        .message(message.message_id, &[])
        .unwrap_or_else(|| message.message_id.to_owned())
}

/// The tree of one snapshot: the children of each node, in display order.
struct Structure<'a> {
    children: BTreeMap<Option<ObjectId>, Vec<(ObjectId, String)>>,
    selection: &'a [ObjectId],
    language: &'a str,
}

fn name(object: &arc_doc::Object) -> String {
    types::text(&object.values, 0)
        .unwrap_or_default()
        .to_owned()
}

/// The label of a wall, floor, door or window: its name and, in brackets, its type's name.
fn labelled(snapshot: &Snapshot, object: &arc_doc::Object) -> String {
    match archbuilder_model::element_type::type_of_object(snapshot, object) {
        Some((_, t)) => format!(
            "{} [{}]",
            name(object),
            archbuilder_model::element_type::name(&t)
        ),
        None => name(object),
    }
}

/// A rename starts from the row's label, which shows the type's name in brackets: the name the
/// rename sets leaves that part out.
fn without_type(snapshot: &Snapshot, mut intent: Intent) -> Intent {
    if intent.command != OBJECT_RENAME {
        return intent;
    }
    let object = intent
        .params
        .get("object")
        .and_then(serde_json::Value::as_str)
        .and_then(|id| id.parse::<ObjectId>().ok())
        .and_then(|id| spatial::get(snapshot, id));
    let suffix = object
        .and_then(|o| archbuilder_model::element_type::type_of_object(snapshot, &o))
        .map(|(_, t)| format!(" [{}]", archbuilder_model::element_type::name(&t)));
    let name = intent
        .params
        .get("name")
        .and_then(serde_json::Value::as_str);
    if let (Some(suffix), Some(name)) = (suffix, name)
        && let Some(stripped) = name.trim_end().strip_suffix(&suffix)
    {
        let stripped = stripped.to_owned();
        intent
            .params
            .insert("name".to_owned(), serde_json::Value::String(stripped));
    }
    intent
}

impl<'a> Structure<'a> {
    fn new(snapshot: &Snapshot, selection: &'a [ObjectId], language: &'a str) -> Self {
        let mut children: BTreeMap<Option<ObjectId>, Vec<(ObjectId, String)>> = BTreeMap::new();
        if let Some(project) = spatial::project(snapshot) {
            children.insert(None, vec![(project.id, name(&project))]);
            let sites = spatial::children(snapshot, SITE, project.id);
            children.insert(
                Some(project.id),
                sites.iter().map(|s| (s.id, name(s))).collect(),
            );
            for site in sites {
                let buildings = spatial::children(snapshot, BUILDING, site.id);
                children.insert(
                    Some(site.id),
                    buildings.iter().map(|b| (b.id, name(b))).collect(),
                );
                for building in buildings {
                    // `storeys` lists from the lowest level up; the outliner lists the highest first.
                    let storeys = spatial::storeys(snapshot, building.id);
                    for s in &storeys {
                        // "Wall 2" before "Wall 10": by length of the name, then by name.
                        let mut walls: Vec<_> = spatial::walls(snapshot, s.storey.id)
                            .iter()
                            .map(|w| (w.id, labelled(snapshot, w)))
                            .collect();
                        walls.sort_by(|a, b| (a.1.len(), &a.1).cmp(&(b.1.len(), &b.1)));
                        for (wall, _) in &walls {
                            // The doors and windows of a wall, under it, by distance along it.
                            let inserts: Vec<_> =
                                archbuilder_model::insert::hosted(snapshot, *wall)
                                    .iter()
                                    .map(|(o, _)| (o.id, labelled(snapshot, o)))
                                    .collect();
                            children.insert(Some(*wall), inserts);
                        }
                        // The floors of the storey come after its walls.
                        let mut floors: Vec<_> = spatial::children(
                            snapshot,
                            archbuilder_model::types::FLOOR,
                            s.storey.id,
                        )
                        .iter()
                        .map(|f| (f.id, labelled(snapshot, f)))
                        .collect();
                        floors.sort_by(|a, b| (a.1.len(), &a.1).cmp(&(b.1.len(), &b.1)));
                        walls.extend(floors);
                        // Then its stairs, by name.
                        let mut stairs: Vec<_> = spatial::children(
                            snapshot,
                            archbuilder_model::types::STAIR,
                            s.storey.id,
                        )
                        .iter()
                        .map(|f| (f.id, name(f)))
                        .collect();
                        stairs.sort_by(|a, b| (a.1.len(), &a.1).cmp(&(b.1.len(), &b.1)));
                        walls.extend(stairs);
                        // The rooms come last, by number.
                        let mut rooms = spatial::children(
                            snapshot,
                            archbuilder_model::types::ROOM,
                            s.storey.id,
                        );
                        rooms.sort_by_key(|r| {
                            types::text(&r.values, archbuilder_model::types::room::NUMBER)
                                .map(|n| (n.len(), n.to_owned()))
                        });
                        walls.extend(rooms.iter().map(|r| {
                            let number =
                                types::text(&r.values, archbuilder_model::types::room::NUMBER)
                                    .unwrap_or_default();
                            (r.id, format!("{number} {}", name(r)).trim().to_owned())
                        }));
                        children.insert(Some(s.storey.id), walls);
                    }
                    children.insert(
                        Some(building.id),
                        storeys
                            .into_iter()
                            .rev()
                            .map(|s| (s.storey.id, name(&s.storey)))
                            .collect(),
                    );
                }
            }
        }
        Self {
            children,
            selection,
            language,
        }
    }

    fn matching<'s>(
        &'s self,
        parent: Option<ObjectId>,
        filter: &'s str,
    ) -> impl Iterator<Item = &'s (ObjectId, String)> + 's {
        self.children
            .get(&parent)
            .into_iter()
            .flatten()
            .filter(move |(_, label)| matches_filter(label, filter, self.language))
    }
}

impl TreeSource for Structure<'_> {
    fn child_count(&self, parent: Option<ObjectId>, filter: &str) -> usize {
        self.matching(parent, filter).count()
    }

    fn child(&self, parent: Option<ObjectId>, index: usize, filter: &str) -> Option<TreeItem> {
        let (id, label) = self.matching(parent, filter).nth(index)?;
        Some(TreeItem {
            id: *id,
            label: label.clone(),
            has_children: self.children.get(&Some(*id)).is_some_and(|c| !c.is_empty()),
            selected: self.selection.contains(id),
            visible: None,
            locked: None,
        })
    }

    fn index_of(&self, parent: Option<ObjectId>, id: ObjectId, filter: &str) -> Option<usize> {
        self.matching(parent, filter).position(|(i, _)| *i == id)
    }
}

/// The outliner panel.
pub struct OutlinerPanel {
    tree: TreeView,
    commands: TreeCommands,
    /// Nodes already opened once: the tree starts open, and a node the user closed stays closed.
    opened: BTreeSet<ObjectId>,
    /// The tab the tree was drawn for: the tree forgets the nodes of another document.
    tab: Option<arc_app::application::TabId>,
}

impl Default for OutlinerPanel {
    fn default() -> Self {
        // Visibility, locking and moving have no meaning for the structure; the tree draws no such
        // control, and a drag onto a row names the selection command, which refuses it.
        Self {
            tree: TreeView::default(),
            commands: TreeCommands {
                select: OBJECT_SELECT.into(),
                rename: OBJECT_RENAME.into(),
                set_visible: commands::SELECT_SET.into(),
                set_locked: commands::SELECT_SET.into(),
                move_to: commands::SELECT_SET.into(),
            },
            opened: BTreeSet::new(),
            tab: None,
        }
    }
}

impl OutlinerPanel {
    /// Opens every node that has children the first time it is seen.
    fn open_new(&mut self, source: &Structure<'_>) {
        let mut parents: Vec<Option<ObjectId>> = vec![None];
        while let Some(parent) = parents.pop() {
            for (index, (id, _)) in source
                .children
                .get(&parent)
                .into_iter()
                .flatten()
                .enumerate()
            {
                if source
                    .children
                    .get(&Some(*id))
                    .is_some_and(|c| !c.is_empty())
                {
                    if self.opened.insert(*id) {
                        self.tree.expand(*id, parent, index);
                    }
                    parents.push(Some(*id));
                }
            }
        }
    }
}

impl Panel for OutlinerPanel {
    fn show(&mut self, ui: &mut Ui, cx: &mut PanelContext<'_>) {
        let (Some(view), Some(tab)) = (cx.view.as_deref(), cx.tab) else {
            return;
        };
        let Ok(tab) = cx.app.tab(tab) else {
            return;
        };
        let snapshot = tab.document().snapshot();
        if spatial::project(&snapshot).is_none() {
            ui.label(say(cx.localize, &NO_PROJECT));
            if ui.button(say(cx.localize, &CREATE_PROJECT)).clicked() {
                cx.actions.push(Action::Intent(Intent::new(PROJECT_NEW)));
            }
            return;
        }
        let source = Structure::new(&snapshot, view.selection(), cx.language);
        if self.tab != cx.tab {
            // The tree dropped the expanded nodes of the other document: open this one again.
            self.tab = cx.tab;
            self.opened.clear();
        }
        self.open_new(&source);
        if let Some(intent) = self.tree.show(ui, &source, &self.commands, cx.localize) {
            cx.actions
                .push(Action::Intent(without_type(&snapshot, intent)));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arc_command::{Invocation, Invoker, Permission, Session, Surface};
    use arc_doc::graph::Evaluators;
    use arc_doc::{Document, DocumentConfig};

    use super::*;

    #[test]
    fn a_rename_from_the_outliner_leaves_the_type_out_of_the_name() {
        let document = Document::open(
            Snapshot::empty(Arc::new(archbuilder_commands::types().unwrap())),
            Arc::new(Evaluators::new()),
            DocumentConfig {
                workers: 0,
                ..DocumentConfig::default()
            },
        )
        .unwrap();
        let registry = archbuilder_commands::registry().unwrap();
        let session = Session::new(Surface::Ui, [Permission::Read, Permission::Edit]);
        let run = |command: &str, params: serde_json::Value| {
            Invoker::new(&registry)
                .invoke(&document, &session, Invocation::new(command, params))
                .unwrap()
        };
        run("archbuilder.project.new", serde_json::json!({}));
        let wall = run(
            "archbuilder.wall.create",
            serde_json::json!({"start": [0.0, 0.0], "end": [3.0, 0.0]}),
        )
        .value["wall"]
            .as_str()
            .unwrap()
            .to_owned();
        let snapshot = document.snapshot();
        let rename = |name: &str| {
            let intent = Intent::new(OBJECT_RENAME)
                .with("object", wall.as_str())
                .with("name", name);
            without_type(&snapshot, intent).params["name"].clone()
        };
        assert_eq!(rename("Facade [Generic 200]"), "Facade");
        assert_eq!(rename("Facade [Other]"), "Facade [Other]");
    }
}
