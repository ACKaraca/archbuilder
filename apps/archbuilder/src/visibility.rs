//! Temporary hide and isolate: which objects the view does not show. The state belongs to one
//! tab's view; it is not part of the document, is not saved and not undone, and ends with
//! [`Visibility::reset`] or when the tab closes.
//!
//! Hiding or isolating a wall takes its doors and windows with it. A kind (walls, doors, windows,
//! floors, rooms) can be hidden or isolated as a whole; it then includes objects made later.
//! Picking, hovering and selecting skip what is not shown ([`Visibility::shows`]).

use std::collections::BTreeSet;

use arc_base::ObjectId;
use arc_doc::Snapshot;
use archbuilder_model::insert;
use archbuilder_model::spatial;
use archbuilder_model::types::WALL;

/// The kinds that can be hidden or isolated as a whole.
pub const KINDS: [&str; 13] = archbuilder_model::types::ELEMENTS;

/// The most objects one list holds.
const MAX_LISTED: usize = 100_000;

/// What an isolation keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Keep {
    /// These objects (and what they host).
    Objects(BTreeSet<ObjectId>),
    /// Every object of these kinds.
    Kinds(BTreeSet<&'static str>),
}

/// The lists the shell hands the renderer, with the snapshot they were made from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lists {
    /// The objects not shown.
    pub hidden: Vec<ObjectId>,
    /// When set, the only objects shown.
    pub isolated: Option<Vec<ObjectId>>,
}

/// The temporary display state of a view.
#[derive(Debug, Default)]
pub struct Visibility {
    hidden: BTreeSet<ObjectId>,
    hidden_kinds: BTreeSet<&'static str>,
    keep: Option<Keep>,
    generation: u64,
    cache: Option<(Snapshot, u64, Lists)>,
}

/// The kind key of `id` when it is one that can be hidden.
fn kind_of(snapshot: &Snapshot, id: ObjectId) -> Option<&'static str> {
    let key = spatial::type_of(snapshot, id)?;
    KINDS.into_iter().find(|k| *k == key)
}

/// The kinds of the hideable objects among `ids`, in the order of [`KINDS`].
#[must_use]
pub fn kinds_of(snapshot: &Snapshot, ids: &[ObjectId]) -> Vec<&'static str> {
    let present: BTreeSet<&'static str> =
        ids.iter().filter_map(|id| kind_of(snapshot, *id)).collect();
    KINDS.into_iter().filter(|k| present.contains(k)).collect()
}

/// `ids` and the doors and windows the walls among them host.
fn with_hosted(snapshot: &Snapshot, ids: &[ObjectId]) -> BTreeSet<ObjectId> {
    let mut out: BTreeSet<ObjectId> = ids.iter().copied().take(MAX_LISTED).collect();
    for id in ids.iter().take(MAX_LISTED) {
        if spatial::type_of(snapshot, *id).as_deref() == Some(WALL) {
            out.extend(insert::hosted(snapshot, *id).into_iter().map(|(o, _)| o.id));
        }
    }
    out
}

impl Visibility {
    /// Whether anything is hidden or isolated.
    #[must_use]
    pub fn active(&self) -> bool {
        !self.hidden.is_empty() || !self.hidden_kinds.is_empty() || self.keep.is_some()
    }

    /// The number of objects hidden by identifier, and whether an isolation is on.
    #[must_use]
    pub fn counts(&self) -> (usize, bool) {
        (
            self.hidden.len() + self.hidden_kinds.len(),
            self.keep.is_some(),
        )
    }

    fn changed(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.cache = None;
    }

    /// Hides `ids` and what the walls among them host.
    pub fn hide(&mut self, snapshot: &Snapshot, ids: &[ObjectId]) {
        self.hidden.extend(with_hosted(snapshot, ids));
        self.changed();
    }

    /// Shows only `ids` and what the walls among them host.
    pub fn isolate(&mut self, snapshot: &Snapshot, ids: &[ObjectId]) {
        self.keep = Some(Keep::Objects(with_hosted(snapshot, ids)));
        self.changed();
    }

    /// Hides every object of `kinds`.
    pub fn hide_kinds(&mut self, kinds: &[&'static str]) {
        self.hidden_kinds.extend(kinds.iter().copied());
        self.changed();
    }

    /// Shows only the objects of `kinds`.
    pub fn isolate_kinds(&mut self, kinds: &[&'static str]) {
        self.keep = Some(Keep::Kinds(kinds.iter().copied().collect()));
        self.changed();
    }

    /// Shows everything again.
    pub fn reset(&mut self) {
        *self = Self::default();
        self.changed();
    }

    /// Whether `id` is shown.
    #[must_use]
    pub fn shows(&self, snapshot: &Snapshot, id: ObjectId) -> bool {
        if !self.active() {
            return true;
        }
        if self.hidden.contains(&id) {
            return false;
        }
        let kind = kind_of(snapshot, id);
        if kind.is_some_and(|k| self.hidden_kinds.contains(k)) {
            return false;
        }
        match &self.keep {
            None => true,
            Some(Keep::Objects(set)) => set.contains(&id),
            Some(Keep::Kinds(kinds)) => kind.is_some_and(|k| kinds.contains(k)),
        }
    }

    /// The lists for the renderer, made once per snapshot and state.
    pub fn lists(&mut self, snapshot: &Snapshot) -> &Lists {
        let fresh = self
            .cache
            .as_ref()
            .is_some_and(|(s, g, _)| *g == self.generation && s.ptr_eq(snapshot));
        if !fresh {
            let lists = self.build(snapshot);
            self.cache = Some((snapshot.clone(), self.generation, lists));
        }
        self.cache.as_ref().map_or_else(
            || {
                static EMPTY: Lists = Lists {
                    hidden: Vec::new(),
                    isolated: None,
                };
                &EMPTY
            },
            |(_, _, lists)| lists,
        )
    }

    fn build(&self, snapshot: &Snapshot) -> Lists {
        if !self.active() {
            return Lists::default();
        }
        let mut hidden: BTreeSet<ObjectId> = self.hidden.clone();
        for kind in &self.hidden_kinds {
            hidden.extend(spatial::objects(snapshot, kind).iter().map(|o| o.id));
        }
        let isolated = self.keep.as_ref().map(|keep| match keep {
            Keep::Objects(set) => set.iter().copied().collect(),
            Keep::Kinds(kinds) => kinds
                .iter()
                .flat_map(|k| spatial::objects(snapshot, k))
                .map(|o| o.id)
                .collect(),
        });
        Lists {
            hidden: hidden.into_iter().take(MAX_LISTED).collect(),
            isolated,
        }
    }
}
