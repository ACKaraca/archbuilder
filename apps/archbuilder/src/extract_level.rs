//! What the viewport draws for a level: the outline of a horizontal reference plane at the level's
//! elevation. Seen from the front or the side the plane is a horizontal line at its elevation,
//! as an architect expects of a level. The level's name and elevation are a label that faces the
//! screen (see [`crate::labels`]). Level objects are the only objects of the structure that have a
//! place in space; changing an elevation re-extracts the level, so the plane moves with it.

use arc_base::math::Point3;
use arc_base::{Cancel, Color, Result};
use arc_doc::Object;
use arc_gpu::{Extraction, LineItem, LinePath, LineStyle, LineWidth, RenderItem};
use archbuilder_model::types::{self, level};

/// Half the width and depth of the level outline, in meters.
pub const HALF: [f64; 2] = [12.0, 8.0];

fn point(x: f64, y: f64) -> [f32; 3] {
    // Display coordinates are single precision relative to the item origin.
    #[expect(clippy::cast_possible_truncation, reason = "display precision")]
    [x as f32, y as f32, 0.0]
}

/// The extraction of a level: the outline.
///
/// # Errors
/// None for this extraction; the extractor interface returns a result.
pub fn extract_level(object: &Object, _cancel: &Cancel) -> Result<Extraction> {
    let elevation = types::meters(&object.values, level::ELEVATION).unwrap_or(0.0);
    let [hx, hy] = HALF;
    let outline = LinePath {
        points: vec![
            point(-hx, -hy),
            point(hx, -hy),
            point(hx, hy),
            point(-hx, hy),
        ],
        closed: true,
    };
    Ok(Extraction {
        items: vec![RenderItem::Lines(LineItem {
            origin: Point3::new(0.0, 0.0, elevation),
            paths: vec![outline],
            style: LineStyle::round(LineWidth::Screen(2.0)),
            color: Color::new(0.25, 0.62, 0.95, 1.0),
        })],
        ..Extraction::default()
    })
}
