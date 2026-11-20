//! The level labels of the viewport: the name and the elevation of each level as text that faces
//! the screen, anchored at the end of the level's outline. The elevation is written in the
//! project's length unit with the interface language's decimal separator. The labels come from the
//! document at each frame, so they follow a rename, a new elevation and a change of unit or
//! language at once. The viewport keeps labels that would cover each other apart (it steps the
//! later one down), so the labels are given from the highest level to the lowest: seen from above,
//! where every level projects to the same place, they stack in the order of the storeys.

use arc_app::application::{Application, TabId};
use arc_base::math::Point3;
use arc_doc::{Object, Snapshot};
use arc_ui::ProductOverlay;
use archbuilder_model::spatial;
use archbuilder_model::types::{self, level};
use archbuilder_model::units::LengthUnit;

use crate::extract_level::HALF;

pub use archbuilder_model::units::decimal_separator;

/// `meters` in `unit` with the decimal separator `separator`.
#[must_use]
pub fn elevation_text(meters: f64, unit: LengthUnit, separator: char) -> String {
    archbuilder_model::units::length_text(meters, unit, separator)
}

/// The text of a level's label: its name, then its elevation.
#[must_use]
pub fn label(level_object: &Object, unit: LengthUnit, language: &str) -> String {
    let name = types::text(&level_object.values, level::NAME).unwrap_or_default();
    let meters = types::meters(&level_object.values, level::ELEVATION).unwrap_or(0.0);
    format!(
        "{name}  {}",
        elevation_text(meters, unit, decimal_separator(language))
    )
}

/// The labels of every level of `snapshot`, highest level first.
#[must_use]
pub fn of_snapshot(snapshot: &Snapshot, language: &str) -> Vec<ProductOverlay> {
    let unit = spatial::length_unit(snapshot);
    let mut levels = spatial::objects(snapshot, types::LEVEL);
    let elevation = |o: &Object| types::meters(&o.values, level::ELEVATION).unwrap_or(0.0);
    levels.sort_by(|a, b| elevation(b).total_cmp(&elevation(a)));
    levels
        .iter()
        .map(|l| ProductOverlay::Label {
            at: Point3::new(HALF[0], HALF[1], elevation(l)),
            text: label(l, unit, language),
        })
        .collect()
}

/// The overlay probe of the shell: the labels of the active tab's levels.
#[must_use]
pub fn overlays(app: &Application, tab: TabId, language: &str) -> Vec<ProductOverlay> {
    app.tab(tab).map_or_else(
        |_| Vec::new(),
        |t| of_snapshot(&t.document().snapshot(), language),
    )
}
