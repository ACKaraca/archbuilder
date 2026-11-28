//! The slope text of ramps in a view from above: each run of a ramp on the active storey gets its
//! slope in percent at the middle of the run, written with the decimal separator of the interface
//! language and marked when it passes the largest slope of the ramp's type. The text is derived
//! each frame from the records; the lines of the ramp are the drawing's own.

use arc_base::ObjectId;
use arc_base::math::Point3;
use arc_doc::Snapshot;
use arc_ui::ProductOverlay;
use archbuilder_model::ramp::Placed;
use archbuilder_model::spatial;
use archbuilder_model::types::RAMP;

/// The most ramps whose slope is written.
const MAX_RAMPS: usize = 500;

/// The slope texts of the ramps on `storey`, as the ramp, the point in plan, the elevation above
/// the project's base and the text; ramps that `hidden` names have none.
#[must_use]
pub fn texts(
    snapshot: &Snapshot,
    storey: ObjectId,
    language: &str,
    hidden: &dyn Fn(ObjectId) -> bool,
) -> Vec<(ObjectId, [f64; 2], f64, String)> {
    let separator = crate::labels::decimal_separator(language);
    let mut out = Vec::new();
    for object in spatial::children(snapshot, RAMP, storey)
        .iter()
        .take(MAX_RAMPS)
        .filter(|o| !hidden(o.id))
    {
        let Some(placed) = Placed::of(snapshot, object) else {
            continue;
        };
        let Ok(layout) = &placed.layout else {
            continue;
        };
        for (at, slope, z) in layout.run_midpoints() {
            let mark = if slope > layout.max_slope + 1.0e-9 {
                " \u{26a0}"
            } else {
                ""
            };
            let number = format!("{:.1}", slope * 100.0).replace('.', &separator.to_string());
            out.push((object.id, at, placed.base + z, format!("{number} %{mark}")));
        }
    }
    out
}

/// The slope texts of the ramps of the active storey, in a view from above that shows ramps.
#[must_use]
pub fn labels(snapshot: &Snapshot, language: &str, shown: bool) -> Vec<ProductOverlay> {
    let view = crate::room_view::shared();
    let Some(storey) = view.active().filter(|_| shown) else {
        return Vec::new();
    };
    texts(snapshot, storey, language, &|id| view.is_hidden(id))
        .into_iter()
        .map(|(_, at, z, text)| ProductOverlay::Label {
            at: Point3::new(at[0], at[1], z + 0.01),
            text,
        })
        .collect()
}
