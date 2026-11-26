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
use arc_ui::egui::{self, Ui};
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
const VIEWS: Message = Message {
    message_id: "ab-outliner-views",
};
const VIEW_3D: Message = Message {
    message_id: "ab-outliner-view-3d",
};
const VIEW_OPEN: Message = Message {
    message_id: "ab-view-open",
};
const VIEW_DUPLICATE: Message = Message {
    message_id: "ab-view-duplicate",
};
const VIEW_RENAME: Message = Message {
    message_id: "ab-view-rename",
};
const VIEW_DELETE: Message = Message {
    message_id: "ab-view-delete",
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
    #[allow(
        clippy::too_many_lines,
        reason = "one list for each kind a storey holds"
    )]
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
                        // Then its roofs, by name.
                        let mut roofs: Vec<_> = spatial::children(
                            snapshot,
                            archbuilder_model::types::ROOF,
                            s.storey.id,
                        )
                        .iter()
                        .map(|f| (f.id, labelled(snapshot, f)))
                        .collect();
                        roofs.sort_by(|a, b| (a.1.len(), &a.1).cmp(&(b.1.len(), &b.1)));
                        walls.extend(roofs);
                        // Then its ceilings and shafts, by name.
                        for key in [
                            archbuilder_model::types::ROOF_EXTRUSION,
                            archbuilder_model::types::CEILING,
                            archbuilder_model::types::SHAFT,
                        ] {
                            let mut found: Vec<_> = spatial::children(snapshot, key, s.storey.id)
                                .iter()
                                .map(|f| (f.id, labelled(snapshot, f)))
                                .collect();
                            found.sort_by(|a, b| (a.1.len(), &a.1).cmp(&(b.1.len(), &b.1)));
                            walls.extend(found);
                        }
                        // The vertical openings of the floors, roofs and ceilings, under their
                        // hosts, by name.
                        for host in walls.iter().map(|(id, _)| *id).collect::<Vec<_>>() {
                            let cuts: Vec<_> = spatial::children(
                                snapshot,
                                archbuilder_model::types::VERTICAL_OPENING,
                                host,
                            )
                            .iter()
                            .map(|c| (c.id, labelled(snapshot, c)))
                            .collect();
                            let edges: Vec<_> = spatial::children(
                                snapshot,
                                archbuilder_model::types::FLOOR_EDGE,
                                host,
                            )
                            .iter()
                            .map(|c| (c.id, labelled(snapshot, c)))
                            .collect();
                            let cuts: Vec<_> = cuts.into_iter().chain(edges).collect();
                            if !cuts.is_empty() {
                                children.insert(Some(host), cuts);
                            }
                        }
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
    /// The view being renamed in the Views branch, with the text typed so far.
    renaming: Option<(ObjectId, String)>,
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
            renaming: None,
        }
    }
}

impl OutlinerPanel {
    /// The Views branch: the 3D view and the plan views. A click, a double click or Open makes a
    /// view the view of the viewport; the property grid shows the settings of the open plan view
    /// while nothing is selected.
    #[allow(clippy::too_many_lines, reason = "one row per view with its menu")]
    fn views(
        &mut self,
        ui: &mut Ui,
        cx: &mut PanelContext<'_>,
        tab: arc_app::application::TabId,
        snapshot: &Snapshot,
    ) {
        use crate::shortcuts::ui as cmd;
        use archbuilder_commands::view as vc;
        let current = crate::plan_state::current_in(tab, snapshot);
        // The controller carries the switch out; it is no command of the document, so the history
        // does not list it.
        let open = |id: Option<ObjectId>| {
            Action::Line(arc_ui::LineAction::Invoke(
                Intent::new(cmd::OPEN_VIEW).with(
                    "view",
                    id.map_or_else(|| "3d".to_owned(), |i| i.to_string()),
                ),
            ))
        };
        let open_storey = |storey: ObjectId| {
            Action::Line(arc_ui::LineAction::Invoke(
                Intent::new(cmd::OPEN_VIEW).with("storey", storey.to_string()),
            ))
        };
        let views = archbuilder_model::view::all(snapshot);
        egui::CollapsingHeader::new(say(cx.localize, &VIEWS))
            .default_open(true)
            .show(ui, |ui| {
                let r = ui.selectable_label(current.is_none(), say(cx.localize, &VIEW_3D));
                if r.clicked() || r.double_clicked() {
                    cx.actions.push(open(None));
                }
                // A storey without a view (a document from before plan views) is listed by its
                // name: its default view, which the document does not hold. Duplicate and rename
                // work on it (they make its view in the same step); delete does not.
                let defaults = spatial::objects(snapshot, archbuilder_model::types::STOREY)
                    .into_iter()
                    .filter(|s| archbuilder_model::view::of_storey(snapshot, s.id).is_empty())
                    .take(archbuilder_model::view::MAX_VIEWS)
                    .map(|s| (s, false));
                let rows: Vec<(std::sync::Arc<arc_doc::Object>, bool)> = defaults
                    .chain(views.iter().map(|v| (v.clone(), true)))
                    .collect();
                for (v, real) in &rows {
                    let name = name(v);
                    let (v_id, real_row) = (v.id, *real);
                    let opening = || {
                        if real_row {
                            open(Some(v_id))
                        } else {
                            open_storey(v_id)
                        }
                    };
                    if let Some((id, text)) = self.renaming.as_mut()
                        && *id == v.id
                    {
                        let edit = ui.text_edit_singleline(text);
                        if edit.lost_focus() {
                            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                cx.actions.push(Action::Intent(
                                    Intent::new(vc::VIEW_RENAME)
                                        .with("view", v.id.to_string())
                                        .with("name", text.trim()),
                                ));
                            }
                            self.renaming = None;
                        } else {
                            edit.request_focus();
                        }
                        continue;
                    }
                    let r = ui.selectable_label(current == Some(v.id), &name);
                    if r.clicked() || r.double_clicked() {
                        cx.actions.push(opening());
                    }
                    r.context_menu(|ui| {
                        let id = v.id.to_string();
                        if ui.button(say(cx.localize, &VIEW_OPEN)).clicked() {
                            cx.actions.push(opening());
                            ui.close();
                        }
                        if ui.button(say(cx.localize, &VIEW_DUPLICATE)).clicked() {
                            cx.actions.push(Action::Intent(
                                Intent::new(vc::VIEW_DUPLICATE).with("view", id.clone()),
                            ));
                            ui.close();
                        }
                        if ui.button(say(cx.localize, &VIEW_RENAME)).clicked() {
                            self.renaming = Some((v.id, name.clone()));
                            ui.close();
                        }
                        // The last view of a storey stays; a default view is none to delete.
                        if *real && ui.button(say(cx.localize, &VIEW_DELETE)).clicked() {
                            cx.actions.push(Action::Intent(
                                Intent::new(vc::VIEW_DELETE).with("view", id),
                            ));
                            ui.close();
                        }
                    });
                }
            });
    }

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
        let (Some(_), Some(tab)) = (cx.view.as_deref(), cx.tab) else {
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
        // The Views branch and the tree scroll together; the tree keeps a box one panel tall, in
        // which it scrolls its own rows (so a large model draws only the rows in sight).
        let height = ui.available_height();
        egui::ScrollArea::vertical()
            .id_salt("ab-outliner")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if let Some(t) = cx.tab {
                    self.views(ui, cx, t, &snapshot);
                }
                let Some(view) = cx.view.as_deref() else {
                    return;
                };
                let source = Structure::new(&snapshot, view.selection(), cx.language);
                if self.tab != cx.tab {
                    // The tree dropped the expanded nodes of the other document: open this one
                    // again.
                    self.tab = cx.tab;
                    self.opened.clear();
                }
                self.open_new(&source);
                let size = egui::vec2(ui.available_width(), height.max(120.0));
                let intent = ui
                    .allocate_ui(size, |ui| {
                        self.tree.show(ui, &source, &self.commands, cx.localize)
                    })
                    .inner;
                if let Some(intent) = intent {
                    cx.actions
                        .push(Action::Intent(without_type(&snapshot, intent)));
                }
            });
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
