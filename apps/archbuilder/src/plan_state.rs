//! Which plan view each tab shows, and the temporary display state laid over it. The 3D view is
//! the default (no plan view); opening a plan view makes it current. The state is the
//! application's, not the document's: it is read by the outliner, the viewport header, the
//! controller, the overlays and the extractor (on the extraction workers), which run in different
//! places, so it lives here, keyed by tab and by view. Only an open view is drawn
//! ([`is_open`]); a view's identifier belongs to one document, so its filter belongs to the one
//! tab that shows it.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use arc_app::application::TabId;
use arc_base::ObjectId;
use arc_doc::Snapshot;
use archbuilder_model::plan::Filter;
use archbuilder_model::{spatial, view};

static CURRENT: Mutex<BTreeMap<TabId, ObjectId>> = Mutex::new(BTreeMap::new());
static FILTERS: Mutex<BTreeMap<ObjectId, Arc<Filter>>> = Mutex::new(BTreeMap::new());

/// The most tabs whose view is remembered.
const MAX_TABS: usize = 256;

/// The plan view `tab` shows; `None` for the 3D view.
#[must_use]
pub fn current(tab: TabId) -> Option<ObjectId> {
    CURRENT.lock().ok().and_then(|m| m.get(&tab).copied())
}

/// What the view `id` a tab holds shows in `snapshot`: a plan view record itself; for a storey
/// (opened as its default view), its first view once it has one (an edit of the default view made
/// it) and the storey itself while it has none (so an undo of that edit shows the default again);
/// `None` when it is gone.
#[must_use]
pub fn resolve(snapshot: &Snapshot, id: ObjectId) -> Option<ObjectId> {
    if !view::is_shown_view(snapshot, id) {
        return None;
    }
    if spatial::type_of(snapshot, id).as_deref() == Some(archbuilder_model::types::STOREY) {
        return Some(view::of_storey(snapshot, id).first().map_or(id, |v| v.id));
    }
    Some(id)
}

/// The plan view `tab` shows in `snapshot` (a view record, or a storey for its default view), else
/// the 3D view (see [`resolve`]).
#[must_use]
pub fn current_in(tab: TabId, snapshot: &Snapshot) -> Option<ObjectId> {
    current(tab).and_then(|id| resolve(snapshot, id))
}

/// Whether a tab shows `view` in `snapshot`: only then is it drawn.
#[must_use]
pub fn is_open_in(snapshot: &Snapshot, view: ObjectId) -> bool {
    open()
        .into_iter()
        .any(|id| resolve(snapshot, id) == Some(view))
}

/// The plan views some tab holds, as opened (resolve them with [`resolve`]).
#[must_use]
pub fn open() -> Vec<ObjectId> {
    CURRENT
        .lock()
        .map(|m| m.values().copied().collect())
        .unwrap_or_default()
}

/// Shows `view` in `tab`; `None` shows the 3D view. The filter of the view it showed before ends.
pub fn set(tab: TabId, view: Option<ObjectId>) {
    let Ok(mut m) = CURRENT.lock() else {
        return;
    };
    let before = match view {
        Some(id) => {
            if m.len() >= MAX_TABS && !m.contains_key(&tab) {
                m.clear();
            }
            m.insert(tab, id)
        }
        None => m.remove(&tab),
    };
    drop(m);
    if let Some(old) = before.filter(|old| Some(*old) != view && !open().contains(old))
        && let Ok(mut f) = FILTERS.lock()
    {
        f.remove(&old);
    }
}

/// The temporary display state laid over `view` (empty when none).
#[must_use]
pub fn filter(view: ObjectId) -> Arc<Filter> {
    FILTERS
        .lock()
        .ok()
        .and_then(|f| f.get(&view).cloned())
        .unwrap_or_default()
}

/// Lays `filter` over `view`; whether it differs from the one before (the drawing must then be
/// extracted again).
pub fn set_filter(view: ObjectId, filter: Filter) -> bool {
    let Ok(mut f) = FILTERS.lock() else {
        return false;
    };
    let same = f.get(&view).is_some_and(|old| **old == filter);
    if same || (filter.is_empty() && !f.contains_key(&view)) {
        return false;
    }
    if filter.is_empty() {
        f.remove(&view);
    } else {
        if f.len() >= MAX_TABS && !f.contains_key(&view) {
            f.clear();
        }
        f.insert(view, Arc::new(filter));
    }
    true
}

/// The storey and level elevation of the plan view `id`.
#[must_use]
pub fn storey_of(snapshot: &Snapshot, id: ObjectId) -> Option<(ObjectId, f64)> {
    let object = spatial::get(snapshot, id)?;
    view::levels_of(snapshot, &object).map(|(s, level, _)| (s, level))
}

/// The drawing of the plan view `id` with its filter, when it is built already (nothing is
/// built here).
#[must_use]
pub fn peek(snapshot: &Snapshot, id: ObjectId) -> Option<Arc<archbuilder_model::plan::Built>> {
    let object = spatial::get(snapshot, id)?;
    archbuilder_model::plan::peek(snapshot, &object, &filter(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arc_base::id::Identifier;

    #[test]
    fn a_filter_change_is_reported_once_and_an_empty_filter_ends_it() {
        let view = ObjectId::from_u128(0xfeed_0001);
        let wall = ObjectId::from_u128(0xfeed_0002);
        let hide = Filter {
            hidden: [wall].into(),
            only: None,
        };
        assert!(!set_filter(view, Filter::default()));
        assert!(set_filter(view, hide.clone()));
        assert!(!set_filter(view, hide.clone()));
        assert_eq!(*filter(view), hide);
        assert!(set_filter(view, Filter::default()));
        assert!(filter(view).is_empty());
    }
}
