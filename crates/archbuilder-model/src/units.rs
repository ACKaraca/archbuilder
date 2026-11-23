//! The length unit of a project. Stored lengths are meters; the unit decides how lengths are
//! shown and typed (the foundation's display formats do the formatting and parsing).

use arc_base::{DisplayFormat, Unit};

/// A length unit a project can show lengths in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LengthUnit {
    /// Meters.
    #[default]
    Meter,
    /// Millimeters.
    Millimeter,
    /// Centimeters.
    Centimeter,
    /// Feet.
    Foot,
    /// Inches.
    Inch,
}

impl LengthUnit {
    /// Every unit, in code order.
    pub const ALL: [Self; 5] = [
        Self::Meter,
        Self::Millimeter,
        Self::Centimeter,
        Self::Foot,
        Self::Inch,
    ];

    /// The name of the unit in the foundation's unit registry, which is also the name of the
    /// enum variant in the project record.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Meter => "meter",
            Self::Millimeter => "millimeter",
            Self::Centimeter => "centimeter",
            Self::Foot => "foot",
            Self::Inch => "inch",
        }
    }

    /// The code stored in the project record.
    #[must_use]
    pub const fn code(self) -> u32 {
        match self {
            Self::Meter => 0,
            Self::Millimeter => 1,
            Self::Centimeter => 2,
            Self::Foot => 3,
            Self::Inch => 4,
        }
    }

    /// The unit of a stored code; the default for a code that names none.
    #[must_use]
    pub fn from_code(code: u32) -> Self {
        Self::ALL
            .into_iter()
            .find(|u| u.code() == code)
            .unwrap_or_default()
    }

    /// The registry unit. `None` only if the registry lacks the name, which its tests rule out.
    #[must_use]
    pub fn unit(self) -> Option<&'static Unit> {
        Unit::by_name(self.name()).ok()
    }

    /// The display format of lengths in this unit: enough decimals for a building.
    #[must_use]
    pub fn format(self) -> Option<DisplayFormat> {
        let mut format = DisplayFormat::new(*self.unit()?);
        format.precision = match self {
            Self::Meter | Self::Foot => 3,
            Self::Millimeter => 0,
            Self::Centimeter => 1,
            Self::Inch => 2,
        };
        Some(format)
    }
}

/// The decimal separator of the interface language `language` (a tag such as `tr` or `en-US`).
#[must_use]
pub fn decimal_separator(language: &str) -> char {
    if language.starts_with("tr") { ',' } else { '.' }
}

/// `meters` in `unit` with the decimal separator `separator`; empty when the unit has no format.
#[must_use]
pub fn length_text(meters: f64, unit: LengthUnit, separator: char) -> String {
    unit.format()
        .and_then(|f| {
            f.format(
                arc_base::Quantity::new(meters, arc_base::Dimension::LENGTH),
                separator,
            )
            .ok()
        })
        .unwrap_or_default()
}

coded! {
    /// How a project shows and reads slopes.
    SlopeUnit {
        /// An angle in degrees: `30°`.
        #[default]
        Degrees = 0, "degrees";
        /// Rise per run as a percentage: `58%`.
        Percent = 1, "percent";
        /// Rise to run, the rise being one: `1:1.73`.
        RiseRun = 2, "rise_run";
    }
}

/// The slope unit of a project record's values.
#[must_use]
pub fn slope_unit(values: &[Option<arc_doc::Value>]) -> SlopeUnit {
    SlopeUnit::of_field(values, crate::types::project::SLOPE_UNIT)
}

/// A slope of `radians` in `unit` with the decimal separator `separator`: `30°`, `57.7%`.
#[must_use]
pub fn slope_text(radians: f64, unit: SlopeUnit, separator: char) -> String {
    let text = match unit {
        SlopeUnit::Degrees => format!("{:.2}°", radians.to_degrees()),
        SlopeUnit::Percent => format!("{:.1}%", radians.tan() * 100.0),
        SlopeUnit::RiseRun => format!("1:{:.2}", 1.0 / radians.tan()),
    };
    text.replace('.', &separator.to_string())
}

/// The slope a person typed, in radians: an angle (`30`, `30°`, `30 deg`), a percentage (`58%`)
/// or a rise over a run (`1/2`, `1:2`, two lengths in any one unit); a bare number is read in
/// `unit`. Commas are decimal separators too. `None` for anything else.
#[must_use]
pub fn parse_slope(text: &str, unit: SlopeUnit) -> Option<f64> {
    let t = text.trim().replace(',', ".");
    let number = |s: &str| s.trim().parse::<f64>().ok().filter(|v| v.is_finite());
    let degrees = |v: f64| v.to_radians();
    let percent = |v: f64| (v / 100.0).atan();
    let value = if let Some(v) = t.strip_suffix('°').or_else(|| t.strip_suffix("deg")) {
        degrees(number(v)?)
    } else if let Some(v) = t.strip_suffix('%') {
        percent(number(v)?)
    } else if let Some((rise, run)) = t.split_once('/').or_else(|| t.split_once(':')) {
        let (rise, run) = (number(rise)?, number(run)?);
        if run <= 0.0 {
            return None;
        }
        (rise / run).atan()
    } else {
        let v = number(&t)?;
        match unit {
            SlopeUnit::Degrees => degrees(v),
            SlopeUnit::Percent => percent(v),
            // A bare number is the rise over the run.
            SlopeUnit::RiseRun => v.atan(),
        }
    };
    value.is_finite().then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slopes_are_read_as_angles_percentages_and_rise_over_run() {
        let close = |a: Option<f64>, b: f64| (a.unwrap() - b).abs() < 1e-12;
        assert!(close(
            parse_slope("45", SlopeUnit::Degrees),
            std::f64::consts::FRAC_PI_4
        ));
        assert!(close(
            parse_slope("100", SlopeUnit::Percent),
            std::f64::consts::FRAC_PI_4
        ));
        assert!(close(
            parse_slope("1:1", SlopeUnit::Degrees),
            std::f64::consts::FRAC_PI_4
        ));
        assert!(close(
            parse_slope("30°", SlopeUnit::Percent),
            30_f64.to_radians()
        ));
        assert!(close(
            parse_slope("50%", SlopeUnit::Degrees),
            0.5_f64.atan()
        ));
        assert!(parse_slope("1/0", SlopeUnit::Degrees).is_none());
        assert_eq!(
            slope_text(30_f64.to_radians(), SlopeUnit::Degrees, ','),
            "30,00°"
        );
        assert_eq!(
            slope_text(std::f64::consts::FRAC_PI_4, SlopeUnit::RiseRun, '.'),
            "1:1.00"
        );
        assert!(close(parse_slope("50", SlopeUnit::Percent), 0.5_f64.atan()));
        assert!(close(
            parse_slope("45%", SlopeUnit::Percent),
            0.45_f64.atan()
        ));
    }

    #[test]
    fn every_unit_is_in_the_registry_and_codes_round_trip() {
        for unit in LengthUnit::ALL {
            assert!(unit.format().is_some(), "{}", unit.name());
            assert_eq!(LengthUnit::from_code(unit.code()), unit);
        }
    }
}
