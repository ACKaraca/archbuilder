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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_unit_is_in_the_registry_and_codes_round_trip() {
        for unit in LengthUnit::ALL {
            assert!(unit.format().is_some(), "{}", unit.name());
            assert_eq!(LengthUnit::from_code(unit.code()), unit);
        }
    }
}
