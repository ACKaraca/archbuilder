//! What the viewport draws for rooms: a tag centered on the placement point (name, number, area in
//! m² with the language's decimal separator, or the state when there is no area), a light diagonal
//! hatch of the region (one polyline per room, distinct from the tinted floor), and, for a
//! selected room, its outline at the base and the top with the corner edges: the wireframe volume.
//! Rooms have no mesh, so nothing of a room is drawn as a solid in 3D.
//!
//! Only the rooms of the active storey are drawn, and in a view from above only those whose point
//! is in the visible area. What does not fit the overlay budget is never dropped silently: a
//! notice at the top left of the view says how many tags are not shown, or that the hatch is off.
//! A tag that would overlap another tag or a level label moves to the nearest place that does not,
//! inside its room when it fits there, with a short leader line to the point.
//!
//! The shell hands the selection to the work-plane probe only, and the camera to nobody but the
//! pointer events, so [`shared`] is fed from there ([`RoomView::set_selection`],
//! [`RoomView::set_view`]); the derived data is cached per snapshot, view and language.

use std::sync::{Mutex, OnceLock};

use arc_base::ObjectId;
use arc_base::math::Point3;
use arc_doc::Snapshot;
use arc_ui::ProductOverlay;
use archbuilder_model::floor::Boundary;
use archbuilder_model::room::{Entry, State};

/// A message of the application, named by identifier.
struct Msg {
    message_id: &'static str,
}

const TAG_UNENCLOSED: Msg = Msg {
    message_id: "ab-room-tag-unenclosed",
};
const TAG_REDUNDANT: Msg = Msg {
    message_id: "ab-room-tag-redundant",
};
const TAGS_HIDDEN: Msg = Msg {
    message_id: "ab-room-tags-hidden",
};
const HATCH_HIDDEN: Msg = Msg {
    message_id: "ab-room-hatch-hidden",
};

const HATCH_STEP: f64 = 0.4;
const MAX_ROWS: u16 = 400;
/// The overlays rooms may use of the viewport's 256 (the rest is for levels, symbols and tools).
const BUDGET: usize = 190;
/// The fewest hatch strokes per room worth drawing; below it the hatch is off.
const MIN_STROKES: usize = 2;
/// The view cube's corner of the image, which tags keep clear of, in pixels.
const CUBE_PX: f64 = 112.0;
/// A tag's size is estimated from its text, in pixels.
const LINE_PX: f64 = 15.0;
/// How much smaller than its room a tag may be and still count as inside, in pixels.
const INSET_PX: f64 = 3.0;
/// The gap the viewport leaves between an anchor and its label, in pixels.
const GAP_PX: f64 = 8.0;

/// A rectangle in view pixels: `[left, top, right, bottom]`.
pub type PixelRect = [f64; 4];

/// The visible area of a view from above: where the camera looks, which way the screen's axes run
/// in the plane, the scale and the size of the image in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewRect {
    /// The model point at the center of the image.
    pub center: [f64; 2],
    /// The model direction of the screen's right, a unit vector.
    pub right: [f64; 2],
    /// The model direction of the screen's up, a unit vector.
    pub up: [f64; 2],
    /// Pixels per meter.
    pub ppu: f64,
    /// The image's width and height in pixels.
    pub size: [f64; 2],
}

impl ViewRect {
    /// The pixel of a model point.
    #[must_use]
    pub fn to_pixel(&self, m: [f64; 2]) -> [f64; 2] {
        let d = [m[0] - self.center[0], m[1] - self.center[1]];
        [
            self.size[0] / 2.0 + (d[0] * self.right[0] + d[1] * self.right[1]) * self.ppu,
            self.size[1] / 2.0 - (d[0] * self.up[0] + d[1] * self.up[1]) * self.ppu,
        ]
    }

    /// The model point of a pixel.
    #[must_use]
    pub fn to_model(&self, p: [f64; 2]) -> [f64; 2] {
        let (x, y) = (
            (p[0] - self.size[0] / 2.0) / self.ppu,
            (self.size[1] / 2.0 - p[1]) / self.ppu,
        );
        [
            self.center[0] + x * self.right[0] + y * self.up[0],
            self.center[1] + x * self.right[1] + y * self.up[1],
        ]
    }

    /// Whether the model point is inside the image, or within `margin` pixels of it.
    #[must_use]
    pub fn contains(&self, m: [f64; 2], margin: f64) -> bool {
        let p = self.to_pixel(m);
        p[0] >= -margin
            && p[1] >= -margin
            && p[0] <= self.size[0] + margin
            && p[1] <= self.size[1] + margin
    }

    /// A key that changes when the view changes by a pixel or so, for the cache.
    fn key(&self) -> [i64; 6] {
        #[allow(clippy::cast_possible_truncation, reason = "a quantized cache key")]
        let q = |v: f64, k: f64| (v * k).round() as i64;
        [
            q(self.center[0], 20.0),
            q(self.center[1], 20.0),
            q(self.ppu, 20.0),
            q(self.size[0], 1.0),
            q(self.size[1], 1.0),
            q(self.right[0].atan2(self.right[1]), 100.0),
        ]
    }

    /// The view assumed when none is known: axes of the plan, 20 px per meter.
    fn assumed() -> Self {
        Self {
            center: [0.0, 0.0],
            right: [1.0, 0.0],
            up: [0.0, 1.0],
            ppu: 20.0,
            size: [800.0, 600.0],
        }
    }
}

/// The width of one character of the viewport's label font (12 pt), estimated by class.
fn char_px(c: char) -> f64 {
    match c {
        'i' | 'l' | 'j' | 't' | 'f' | 'r' | 'I' | '.' | ',' | ':' | '\'' | '|' => 4.5,
        ' ' => 4.0,
        'm' | 'w' | 'M' | 'W' => 11.5,
        '²' | '³' => 6.0,
        '0'..='9' => 8.0,
        c if c.is_uppercase() => 8.5,
        _ => 7.0,
    }
}

/// The estimated size in pixels of a label with `text`: the text, as the viewport lays it out
/// (its plate adds a pixel or two around).
#[must_use]
pub fn label_size(text: &str) -> [f64; 2] {
    let lines = text.lines().count().max(1);
    let widest = text
        .lines()
        .map(|l| l.chars().map(char_px).sum::<f64>())
        .fold(0.0_f64, f64::max);
    [widest, lines as f64 * LINE_PX]
}

/// The model anchor that puts a label of `size` pixels with its center at pixel `center`
/// (the viewport draws a label up and to the right of its anchor).
fn anchor_for_center(view: &ViewRect, center: [f64; 2], size: [f64; 2]) -> [f64; 2] {
    view.to_model([
        center[0] - GAP_PX - size[0] / 2.0,
        center[1] + GAP_PX + size[1] / 2.0,
    ])
}

fn overlaps(a: PixelRect, b: PixelRect) -> bool {
    a[0] < b[2] + 1.0 && b[0] < a[2] + 1.0 && a[1] < b[3] + 1.0 && b[1] < a[3] + 1.0
}

fn rect_at(center: [f64; 2], size: [f64; 2]) -> PixelRect {
    [
        center[0] - size[0] / 2.0,
        center[1] - size[1] / 2.0,
        center[0] + size[0] / 2.0,
        center[1] + size[1] / 2.0,
    ]
}

/// Where the notice sits: the top left of the image.
fn notice_rect(text: &str) -> PixelRect {
    let size = label_size(text);
    [6.0, 24.0, 6.0 + size[0], 24.0 + size[1]]
}

/// The text of a room's tag.
fn tag(entry: &Entry, language: &str, say: &dyn Fn(&str) -> String) -> String {
    let separator = archbuilder_model::units::decimal_separator(language);
    let second = match entry.state() {
        State::Enclosed => entry
            .area()
            .map(|a| format!("{a:.2} m²").replace('.', &separator.to_string())),
        State::Unenclosed => Some(say(TAG_UNENCLOSED.message_id)),
        State::Redundant => Some(say(TAG_REDUNDANT.message_id)),
    };
    let mut lines = vec![entry.room.name.clone()];
    lines.extend(entry.room.number.clone());
    lines.extend(second);
    lines.join("\n")
}

/// Diagonal hatch strokes of `polygon`, each clipped to the region (no connectors along the
/// boundary), at most `cap` of them: the spacing widens until they fit. `None` when even the
/// widest spacing gives more.
fn hatch(polygon: &[[f64; 2]], z: f64, cap: usize) -> Option<Vec<ProductOverlay>> {
    let s = std::f64::consts::FRAC_1_SQRT_2;
    let to = |p: [f64; 2]| [(p[0] + p[1]) * s, (p[1] - p[0]) * s];
    let back = |p: [f64; 2]| [(p[0] - p[1]) * s, (p[0] + p[1]) * s];
    let rotated: Vec<[f64; 2]> = polygon.iter().map(|p| to(*p)).collect();
    let (low, high) = rotated
        .iter()
        .fold((f64::MAX, f64::MIN), |(l, h), p| (l.min(p[1]), h.max(p[1])));
    if !(high - low).is_finite() || high - low < HATCH_STEP || cap == 0 {
        return None;
    }
    let mut step = HATCH_STEP;
    for _ in 0..12 {
        let rows = ((high - low) / step).floor();
        let mut strokes: Vec<ProductOverlay> = Vec::new();
        for k in 0..MAX_ROWS {
            if f64::from(k) >= rows {
                break;
            }
            let y = low + step * (f64::from(k) + 0.5);
            let mut xs: Vec<f64> = Vec::new();
            for i in 0..rotated.len() {
                let (a, b) = (rotated[i], rotated[(i + 1) % rotated.len()]);
                if (a[1] > y) != (b[1] > y) {
                    xs.push(a[0] + (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]));
                }
            }
            xs.sort_by(f64::total_cmp);
            for pair in xs.chunks_exact(2) {
                let (p, q) = (back([pair[0], y]), back([pair[1], y]));
                strokes.push(ProductOverlay::Path {
                    points: vec![Point3::new(p[0], p[1], z), Point3::new(q[0], q[1], z)],
                    dashed: false,
                });
            }
        }
        if strokes.len() <= cap {
            return Some(strokes).filter(|s| !s.is_empty());
        }
        step *= 1.4;
    }
    None
}

/// A room's tag to lay out, in view pixels.
pub struct TagIn {
    /// Where the tag is centered: the room's point.
    pub center: [f64; 2],
    /// The tag's size.
    pub size: [f64; 2],
    /// The room's region; empty for a room without one.
    pub polygon: Vec<[f64; 2]>,
    /// The room's area, which decides who keeps the place when two tags would overlap.
    pub area: f64,
    /// A well-inside place of the room (display only), tried when the point does not fit.
    pub alt: Option<[f64; 2]>,
}

/// Where each of `tags` is shown, `None` for a tag that is not. A tag is centered on its point,
/// else on the room's well-inside place when the point does not suit; it is shown when it lies
/// inside the image and inside its room's region (a few pixels of slack), overlaps none of the
/// `blocked` rectangles (the notice, the view cube, the level labels) and no tag of a larger room
/// (the room with the larger area keeps the place; ties by order).
#[must_use]
pub fn layout_tags(
    tags: &[TagIn],
    image: [f64; 2],
    blocked: &[PixelRect],
) -> Vec<Option<[f64; 2]>> {
    let mut order: Vec<usize> = (0..tags.len()).collect();
    order.sort_by(|a, b| tags[*b].area.total_cmp(&tags[*a].area));
    let mut shown: Vec<Option<[f64; 2]>> = vec![None; tags.len()];
    let mut taken: Vec<PixelRect> = blocked.to_vec();
    for k in order {
        let t = &tags[k];
        let boundary = Boundary {
            loops: vec![t.polygon.clone()],
        };
        let mid = |a: f64, b: f64| (a + b) / 2.0;
        let suits = |c: [f64; 2], taken: &[PixelRect]| {
            let r = rect_at(c, t.size);
            let on_image = r[0] >= 0.0 && r[1] >= 0.0 && r[2] <= image[0] && r[3] <= image[1];
            // The room is tested with the tag drawn a little smaller than it is.
            let q = [
                r[0] + INSET_PX,
                r[1] + INSET_PX,
                r[2] - INSET_PX,
                r[3] - INSET_PX,
            ];
            let in_room = t.polygon.len() < 3
                || [
                    [q[0], q[1]],
                    [q[2], q[1]],
                    [q[2], q[3]],
                    [q[0], q[3]],
                    [mid(q[0], q[2]), q[1]],
                    [mid(q[0], q[2]), q[3]],
                    [q[0], mid(q[1], q[3])],
                    [q[2], mid(q[1], q[3])],
                ]
                .iter()
                .all(|p| boundary.contains(*p));
            on_image && in_room && !taken.iter().any(|b| overlaps(r, *b))
        };
        let at = [Some(t.center), t.alt]
            .into_iter()
            .flatten()
            .find(|c| suits(*c, &taken));
        if let Some(c) = at {
            taken.push(rect_at(c, t.size));
            shown[k] = Some(c);
        }
    }
    shown
}

struct Cache {
    snapshot: Snapshot,
    language: String,
    active: Option<ObjectId>,
    view: Option<[i64; 6]>,
    rooms: Vec<Entry>,
    drawn: Vec<ProductOverlay>,
}

/// The room overlays of one application.
#[derive(Default)]
pub struct RoomView {
    cache: Mutex<Option<Cache>>,
    selection: Mutex<Vec<ObjectId>>,
    active: Mutex<Option<ObjectId>>,
    view: Mutex<Option<ViewRect>>,
}

/// The room overlays the shell's probes feed and read.
#[must_use]
pub fn shared() -> &'static RoomView {
    static SHARED: OnceLock<RoomView> = OnceLock::new();
    SHARED.get_or_init(RoomView::default)
}

fn set<T: PartialEq>(slot: &Mutex<T>, value: T) {
    if let Ok(mut s) = slot.lock()
        && *s != value
    {
        *s = value;
    }
}

impl RoomView {
    /// Remembers the tab's selection for the highlight of selected rooms.
    pub fn set_selection(&self, selection: &[ObjectId]) {
        set(&self.selection, selection.to_vec());
    }

    /// Remembers the storey the viewport works on: the only one whose rooms are drawn.
    pub fn set_active(&self, storey: Option<ObjectId>) {
        set(&self.active, storey);
    }

    /// Remembers the visible area of a view from above, `None` for any other view.
    pub fn set_view(&self, view: Option<ViewRect>) {
        set(&self.view, view);
    }

    /// The visible area last remembered.
    #[must_use]
    pub fn view(&self) -> Option<ViewRect> {
        self.view.lock().ok().and_then(|v| *v)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one pass: filter, lay out, budget, notice"
    )]
    fn build(
        snapshot: &Snapshot,
        language: &str,
        active: Option<ObjectId>,
        view: Option<ViewRect>,
    ) -> Cache {
        let rooms = active.map_or_else(Vec::new, |s| {
            archbuilder_model::room::of_storey(snapshot, s)
        });
        let localizer = crate::localizer(&[language.to_owned()]).ok();
        let say = |id: &str, count: usize| {
            localizer.as_ref().map_or_else(
                || id.to_owned(),
                |l| {
                    l.format(
                        id,
                        &[(
                            "count",
                            arc_base::error::Value::Int(i64::try_from(count).unwrap_or(i64::MAX)),
                        )],
                    )
                    .text
                },
            )
        };
        let frame = view.unwrap_or_else(ViewRect::assumed);
        // Only the rooms whose point is in view count; the others are not drawn or counted.
        let seen: Vec<&Entry> = rooms
            .iter()
            .filter(|e| view.is_none_or(|v| v.contains(e.room.point, 0.0)))
            .collect();
        let texts: Vec<String> = seen
            .iter()
            .map(|e| tag(e, language, &|id| say(id, 0)))
            .collect();
        let shown: Vec<Option<[f64; 2]>> = match view {
            None => vec![Some([0.0, 0.0]); seen.len()],
            Some(v) => {
                let tags: Vec<TagIn> = seen
                    .iter()
                    .zip(&texts)
                    .map(|(e, t)| TagIn {
                        center: v.to_pixel(e.room.point),
                        size: label_size(t),
                        polygon: e.footprint.polygon.iter().map(|p| v.to_pixel(*p)).collect(),
                        area: e.area().unwrap_or(0.0),
                        alt: archbuilder_model::room::pole(&e.footprint.polygon)
                            .map(|p| v.to_pixel(p)),
                    })
                    .collect();
                let mut blocked = crate::labels::level_rects(snapshot, language, view);
                blocked.push([v.size[0] - CUBE_PX, 0.0, v.size[0], CUBE_PX]);
                let first = layout_tags(&tags, v.size, &blocked);
                if first.iter().all(Option::is_some) {
                    first
                } else {
                    // The notice takes its place too; lay out again around it.
                    blocked.push(notice_rect(&say(TAGS_HIDDEN.message_id, seen.len())));
                    layout_tags(&tags, v.size, &blocked)
                }
            }
        };
        let hidden = shown.iter().filter(|s| s.is_none()).count();
        let tagged = seen.len() - hidden;
        // What the budget leaves goes to the hatch of every enclosed room in view, tagged or not,
        // shared evenly; when it cannot give each room a few strokes the hatch is off.
        let enclosed = seen.iter().filter(|e| e.state() == State::Enclosed).count();
        let per_room = BUDGET.saturating_sub(tagged + 1) / enclosed.max(1);
        let hatched = per_room >= MIN_STROKES;
        let mut drawn: Vec<ProductOverlay> = Vec::new();
        for ((entry, text), at) in seen.iter().zip(texts).zip(&shown) {
            if hatched
                && entry.state() == State::Enclosed
                && let Some(strokes) = hatch(&entry.footprint.polygon, entry.base, per_room)
            {
                drawn.extend(strokes);
            }
            let Some(center) = at else {
                continue;
            };
            let at = if view.is_some() {
                anchor_for_center(&frame, *center, label_size(&text))
            } else {
                entry.room.point
            };
            drawn.push(ProductOverlay::Label {
                at: Point3::new(at[0], at[1], entry.base),
                text,
            });
        }
        // Whatever was left out is said, at the top left of the view.
        let notice = if hidden > 0 {
            Some(say(TAGS_HIDDEN.message_id, hidden))
        } else if !hatched && enclosed > 0 {
            Some(say(HATCH_HIDDEN.message_id, enclosed))
        } else {
            None
        };
        if let Some(text) = notice {
            let r = notice_rect(&text);
            let size = label_size(&text);
            let at = if view.is_some() {
                anchor_for_center(&frame, [r[0] + size[0] / 2.0, r[1] + size[1] / 2.0], size)
            } else {
                rooms.first().map_or([0.0, 0.0], |e| e.room.point)
            };
            drawn.push(ProductOverlay::Label {
                at: Point3::new(at[0], at[1], rooms.first().map_or(0.0, |e| e.base)),
                text,
            });
        }
        Cache {
            snapshot: snapshot.clone(),
            language: language.to_owned(),
            active,
            view: view.map(|v| v.key()),
            rooms,
            drawn,
        }
    }

    /// The overlays of the rooms of `snapshot`.
    #[must_use]
    pub fn overlays(&self, snapshot: &Snapshot, language: &str) -> Vec<ProductOverlay> {
        let active = self.active.lock().ok().and_then(|a| *a);
        let view = self.view();
        let Ok(mut guard) = self.cache.lock() else {
            return Self::build(snapshot, language, active, view).drawn;
        };
        let fresh = guard.as_ref().is_some_and(|c| {
            c.snapshot.ptr_eq(snapshot)
                && c.language == language
                && c.active == active
                && c.view == view.map(|v| v.key())
        });
        if !fresh {
            *guard = Some(Self::build(snapshot, language, active, view));
        }
        let Some(cache) = guard.as_ref() else {
            return Vec::new();
        };
        let mut out = cache.drawn.clone();
        let selected = self.selection.lock().map(|s| s.clone()).unwrap_or_default();
        for entry in cache.rooms.iter().filter(|e| selected.contains(&e.id)) {
            out.extend(volume(entry));
        }
        out
    }
}

/// The wireframe of a selected room: base loop, top loop and the corner edges.
fn volume(entry: &Entry) -> Vec<ProductOverlay> {
    let polygon = &entry.footprint.polygon;
    if polygon.is_empty() {
        return Vec::new();
    }
    let ring = |z: f64| ProductOverlay::Path {
        points: polygon
            .iter()
            .chain(polygon.first())
            .map(|p| Point3::new(p[0], p[1], z))
            .collect(),
        dashed: false,
    };
    let mut out = vec![ring(entry.base), ring(entry.top)];
    out.extend(polygon.iter().take(64).map(|p| ProductOverlay::Path {
        points: vec![
            Point3::new(p[0], p[1], entry.base),
            Point3::new(p[0], p[1], entry.top),
        ],
        dashed: true,
    }));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag_at(center: [f64; 2], room: [f64; 4], area: f64) -> TagIn {
        TagIn {
            center,
            size: [50.0, 45.0],
            polygon: vec![
                [room[0], room[1]],
                [room[2], room[1]],
                [room[2], room[3]],
                [room[0], room[3]],
            ],
            area,
            alt: None,
        }
    }

    fn flags(tags: &[TagIn], image: [f64; 2], blocked: &[PixelRect]) -> Vec<bool> {
        layout_tags(tags, image, blocked)
            .iter()
            .map(Option::is_some)
            .collect()
    }

    const IMAGE: [f64; 2] = [800.0, 600.0];

    #[test]
    fn a_tag_that_fits_its_room_is_shown_and_one_that_does_not_is_counted() {
        let big = tag_at([150.0, 150.0], [100.0, 100.0, 200.0, 200.0], 9.0);
        let small = tag_at([400.0, 150.0], [390.0, 140.0, 420.0, 160.0], 1.0);
        assert_eq!(flags(&[big, small], IMAGE, &[]), [true, false]);
    }

    #[test]
    fn two_tags_that_would_overlap_keep_the_larger_room() {
        // Both rooms are large enough, but the points are close.
        let a = tag_at([300.0, 300.0], [200.0, 200.0, 400.0, 400.0], 4.0);
        let b = tag_at([320.0, 310.0], [250.0, 250.0, 450.0, 450.0], 6.0);
        assert_eq!(flags(&[a, b], IMAGE, &[]), [false, true]);
    }

    #[test]
    fn the_notice_the_cube_and_the_level_labels_are_respected() {
        let a = tag_at([150.0, 150.0], [100.0, 100.0, 200.0, 200.0], 9.0);
        let b = tag_at([450.0, 150.0], [400.0, 100.0, 500.0, 200.0], 9.0);
        let c = tag_at([750.0, 60.0], [700.0, 10.0, 799.0, 110.0], 9.0);
        let blocked = [
            [120.0, 120.0, 180.0, 140.0],
            [800.0 - CUBE_PX, 0.0, 800.0, CUBE_PX],
        ];
        assert_eq!(flags(&[a, b, c], IMAGE, &blocked), [false, true, false]);
    }

    #[test]
    fn a_tag_leaving_the_image_is_not_shown_and_a_centered_one_is_not_moved() {
        let edge = tag_at([10.0, 150.0], [0.0, 100.0, 100.0, 200.0], 9.0);
        let free = tag_at([300.0, 300.0], [200.0, 200.0, 400.0, 400.0], 9.0);
        assert_eq!(flags(&[edge, free], IMAGE, &[]), [false, true]);
    }

    #[test]
    fn hatch_strokes_stay_in_the_region_and_in_their_budget() {
        let square = [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]];
        let strokes = hatch(&square, 0.0, 12).unwrap();
        assert!(!strokes.is_empty() && strokes.len() <= 12);
        for s in strokes {
            let ProductOverlay::Path { points, .. } = s else {
                panic!("a path");
            };
            assert_eq!(points.len(), 2);
            assert!(points.iter().all(
                |p| (-1e-9..=4.0 + 1e-9).contains(&p.x) && (-1e-9..=4.0 + 1e-9).contains(&p.y)
            ));
        }
    }

    #[test]
    fn a_tag_a_little_larger_than_its_room_still_fits_and_the_inside_place_is_tried() {
        // The room is 2 px narrower than the tag: within the slack.
        let snug = tag_at([100.0, 100.0], [75.0, 70.0, 123.0, 130.0], 1.0);
        assert_eq!(flags(&[snug], IMAGE, &[]), [true]);
        // An L-shaped room whose point is in the notch of the corner: the centered tag does not
        // fit, the well-inside place does and the tag is centered there.
        let mut l = tag_at([95.0, 95.0], [0.0, 0.0, 10.0, 10.0], 5.0);
        l.polygon = vec![
            [0.0, 0.0],
            [300.0, 0.0],
            [300.0, 80.0],
            [100.0, 80.0],
            [100.0, 300.0],
            [0.0, 300.0],
        ];
        l.center = [90.0, 75.0];
        l.alt = Some([150.0, 40.0]);
        let placed = layout_tags(&[l], IMAGE, &[]);
        assert_eq!(placed, [Some([150.0, 40.0])]);
    }
}
