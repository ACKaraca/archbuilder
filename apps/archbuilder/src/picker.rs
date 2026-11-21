//! The picking of a plan view: one ordered candidate list ([`archbuilder_model::pick::stack`]:
//! doors and windows, walls, rooms, floors, levels) serves both the hover highlight and the
//! click, so the highlighted object is the one a click selects. A click within a few pixels of
//! the last one, with the document unchanged, steps down the list; the hover shows what that
//! next click would select.

use std::time::{Duration, Instant};

use arc_base::ObjectId;
use arc_doc::Snapshot;
use archbuilder_model::room::Entry;

/// How far a repeated click may be from the last one to step down the list, in pixels.
const CYCLE_PIXELS: f64 = 4.0;
/// How long after a click a repeated click still steps down the list.
const CYCLE_TIME: Duration = Duration::from_millis(1500);

struct Last {
    pixel: [f64; 2],
    snapshot: Snapshot,
    stack: Vec<ObjectId>,
    index: usize,
    at: Instant,
}

/// The picker of one tab.
#[derive(Default)]
pub struct Picker {
    last: Option<Last>,
    rooms: Option<(Snapshot, Vec<Entry>)>,
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

    /// What a click at `pixel` (model point `at`) would pick now: the hover and the click both
    /// ask this.
    pub fn target(
        &mut self,
        snapshot: &Snapshot,
        pixel: [f64; 2],
        at: [f64; 2],
        aperture: f64,
        ray: Option<([f64; 3], [f64; 3])>,
        now: Instant,
    ) -> Option<Target> {
        if let Some(last) = &self.last
            && last.snapshot.ptr_eq(snapshot)
            && (last.pixel[0] - pixel[0]).hypot(last.pixel[1] - pixel[1]) <= CYCLE_PIXELS
            && !last.stack.is_empty()
            && now.saturating_duration_since(last.at) <= CYCLE_TIME
        {
            let index = (last.index + 1) % last.stack.len();
            return Some(Target {
                id: *last.stack.get(index)?,
                stack: last.stack.clone(),
                index,
            });
        }
        let rooms = self.rooms(snapshot).to_vec();
        let stack = archbuilder_model::pick::stack(snapshot, &rooms, at, aperture, ray);
        Some(Target {
            id: *stack.first()?,
            stack,
            index: 0,
        })
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
    }

    /// Forgets the last click (Escape): the next click starts at the top of the list.
    pub fn reset(&mut self) {
        self.last = None;
    }

    /// The outlines that mark `id` for the hover.
    pub fn outline(&mut self, snapshot: &Snapshot, id: ObjectId) -> Vec<Vec<[f64; 3]>> {
        let rooms = self.rooms(snapshot).to_vec();
        archbuilder_model::pick::outline(snapshot, &rooms, id)
    }
}
