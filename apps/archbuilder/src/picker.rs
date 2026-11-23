//! The picking of a plan view: one ordered candidate list ([`archbuilder_model::pick::stack`]:
//! doors and windows, walls, rooms, floors, levels) serves both the hover highlight and the
//! click, so the highlighted object is the one a click selects. A click within a few pixels of
//! the last one, with the document unchanged, steps down the list; the hover shows what that
//! next click would select.
//!
//! Tab steps the hover down the same list ([`Picker::step`]) while the pointer stays within a few
//! pixels; a click then selects the candidate the hover shows. Objects the view does not show
//! ([`crate::visibility`]) are not in the list.

use std::time::{Duration, Instant};

use arc_base::ObjectId;
use arc_doc::Snapshot;
use archbuilder_model::room::Entry;

/// How far a repeated click may be from the last one to step down the list, in pixels.
const CYCLE_PIXELS: f64 = 4.0;
/// How long after a click a repeated click still steps down the list.
const CYCLE_TIME: Duration = Duration::from_millis(1500);
/// A second click sooner than this is the second click of a double click: it picks the same
/// object again instead of stepping down.
const DOUBLE_CLICK: Duration = Duration::from_millis(400);

struct Last {
    pixel: [f64; 2],
    snapshot: Snapshot,
    stack: Vec<ObjectId>,
    index: usize,
    at: Instant,
}

/// The candidates under the pointer when it last moved, for Tab to step through.
struct Under {
    pixel: [f64; 2],
    stack: Vec<ObjectId>,
}

/// Where and when the pointer asks what it would pick.
#[derive(Clone, Copy)]
pub struct Probe<'a> {
    /// The pointer's pixel.
    pub pixel: [f64; 2],
    /// The model point under it, in meters.
    pub at: [f64; 2],
    /// How far from an outline a point still counts, in meters.
    pub aperture: f64,
    /// The view ray through the pointer, when the view has one.
    pub ray: Option<([f64; 3], [f64; 3])>,
    /// The time of the event.
    pub now: Instant,
    /// Whether an object is shown (hidden objects are not candidates).
    pub shown: &'a dyn Fn(ObjectId) -> bool,
}

/// The picker of one tab.
#[derive(Default)]
pub struct Picker {
    last: Option<Last>,
    rooms: Option<(Snapshot, Vec<Entry>)>,
    under: Option<Under>,
    /// The candidate Tab chose: where the pointer was and the index into the list.
    tab: Option<([f64; 2], usize)>,
    /// The objects a plan view's drawing has under the pointer, when a plan view is open: only
    /// these are candidates, in the order of the list.
    view_hits: Option<archbuilder_model::plan::Hits>,
}

fn near(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).hypot(a[1] - b[1]) <= CYCLE_PIXELS
}

/// What a click would pick: the object and where it stands in its candidate list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// The object.
    pub id: ObjectId,
    stack: Vec<ObjectId>,
    index: usize,
}

impl Picker {
    fn rooms(&mut self, snapshot: &Snapshot) -> &[Entry] {
        if !self.rooms.as_ref().is_some_and(|(s, _)| s.ptr_eq(snapshot)) {
            self.rooms = Some((
                snapshot.clone(),
                archbuilder_model::pick::all_rooms(snapshot),
            ));
        }
        self.rooms.as_ref().map_or(&[], |(_, r)| r.as_slice())
    }

    /// What a click at `probe.pixel` (model point `probe.at`) would pick now: the hover and the
    /// click both ask this.
    pub fn target(&mut self, snapshot: &Snapshot, probe: &Probe<'_>) -> Option<Target> {
        let Probe {
            pixel,
            at,
            aperture,
            ray,
            now,
            shown,
        } = *probe;
        if self.tab.as_ref().is_some_and(|(p, _)| !near(*p, pixel)) {
            self.tab = None;
        }
        if self.tab.is_none()
            && let Some(last) = &self.last
            && last.snapshot.ptr_eq(snapshot)
            && (last.pixel[0] - pixel[0]).hypot(last.pixel[1] - pixel[1]) <= CYCLE_PIXELS
            && !last.stack.is_empty()
            && now.saturating_duration_since(last.at) <= CYCLE_TIME
        {
            let quick = now.saturating_duration_since(last.at) < DOUBLE_CLICK;
            let index = if quick {
                last.index
            } else {
                (last.index + 1) % last.stack.len()
            };
            return Some(Target {
                id: *last.stack.get(index)?,
                stack: last.stack.clone(),
                index,
            });
        }
        // A plan view's drawing has the rooms already (its back-links): the model's room
        // derivation of every storey, slow after an edit, is not needed for its candidates.
        let rooms = if self.view_hits.is_some() {
            Vec::new()
        } else {
            self.rooms(snapshot).to_vec()
        };
        let mut stack = archbuilder_model::pick::stack(snapshot, &rooms, at, aperture, ray);
        stack.retain(|id| shown(*id));
        if let Some(hits) = &self.view_hits {
            // What the drawing shows under the pointer, in the order of the list.
            stack = hits.filter(stack);
            stack.retain(|id| shown(*id));
        }
        self.under = Some(Under {
            pixel,
            stack: stack.clone(),
        });
        let index = self.tab.map_or(0, |(_, i)| i % stack.len().max(1));
        Some(Target {
            id: *stack.get(index)?,
            stack,
            index,
        })
    }

    /// Sets the objects the open plan view's drawing has under the pointer (`None` without a
    /// plan view): the next [`Picker::target`] looks only at them.
    pub fn set_view_hits(&mut self, hits: Option<archbuilder_model::plan::Hits>) {
        self.view_hits = hits;
    }

    /// Tab: the hover moves to the next candidate under the pointer (and back to the first after
    /// the last). Returns whether there is more than one candidate to step through.
    pub fn step(&mut self) -> bool {
        let Some(under) = self.under.as_ref().filter(|u| u.stack.len() > 1) else {
            return false;
        };
        let at = self
            .tab
            .filter(|(p, _)| near(*p, under.pixel))
            .map_or(0, |(_, i)| i);
        self.tab = Some((under.pixel, (at + 1) % under.stack.len()));
        true
    }

    /// The object the last click picked, when `pixel` is where it was.
    #[must_use]
    pub fn last_picked(&self, pixel: [f64; 2]) -> Option<ObjectId> {
        let last = self.last.as_ref().filter(|l| near(l.pixel, pixel))?;
        last.stack.get(last.index).copied()
    }

    /// The centers of the rooms of `storey` (the area-weighted center of each footprint).
    pub fn centers(&mut self, snapshot: &Snapshot, storey: ObjectId) -> Vec<[f64; 2]> {
        self.rooms(snapshot)
            .iter()
            .filter(|e| {
                archbuilder_model::spatial::get(snapshot, e.id)
                    .is_some_and(|o| o.header.owner == Some(storey))
            })
            .filter_map(|e| crate::snap::centroid(&e.footprint.polygon))
            .collect()
    }

    /// Records a click that picked `target`, so the next one nearby steps down the list.
    pub fn commit(&mut self, snapshot: &Snapshot, pixel: [f64; 2], target: &Target, now: Instant) {
        self.last = Some(Last {
            pixel,
            snapshot: snapshot.clone(),
            stack: target.stack.clone(),
            index: target.index,
            at: now,
        });
        self.tab = None;
    }

    /// Forgets the last click (Escape): the next click starts at the top of the list.
    pub fn reset(&mut self) {
        self.last = None;
        self.tab = None;
    }

    /// The outlines that mark `id` for the hover.
    pub fn outline(&mut self, snapshot: &Snapshot, id: ObjectId) -> Vec<Vec<[f64; 3]>> {
        let rooms = self.rooms(snapshot).to_vec();
        archbuilder_model::pick::outline(snapshot, &rooms, id)
    }
}
