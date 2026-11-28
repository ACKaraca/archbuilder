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
pub fn length_text(meters: f64, unit: impl Into<UnitSettings>, separator: char) -> String {
    unit.into().length_text(meters, separator)
}

impl From<LengthUnit> for UnitSettings {
    fn from(length: LengthUnit) -> Self {
        Self {
            length,
            ..Self::default()
        }
    }
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
pub fn slope_text(radians: f64, unit: impl Into<UnitSettings>, separator: char) -> String {
    unit.into().slope_text(radians, separator)
}

impl From<SlopeUnit> for UnitSettings {
    fn from(slope: SlopeUnit) -> Self {
        Self {
            slope,
            ..Self::default()
        }
    }
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

coded! {
    /// The unit areas are shown in.
    AreaUnit {
        /// Square meters.
        #[default]
        SquareMeter = 0, "square_meter";
        /// Square centimeters.
        SquareCentimeter = 1, "square_centimeter";
        /// Square millimeters.
        SquareMillimeter = 2, "square_millimeter";
        /// Square feet.
        SquareFoot = 3, "square_foot";
    }
}

coded! {
    /// The unit volumes are shown in.
    VolumeUnit {
        /// Cubic meters.
        #[default]
        CubicMeter = 0, "cubic_meter";
        /// Liters.
        Liter = 1, "liter";
        /// Cubic feet.
        CubicFoot = 2, "cubic_foot";
        /// Cubic yards.
        CubicYard = 3, "cubic_yard";
    }
}

coded! {
    /// The unit angles are shown in.
    AngleUnit {
        /// Degrees.
        #[default]
        Degree = 0, "degree";
        /// Radians.
        Radian = 1, "radian";
        /// Gradians.
        Gradian = 2, "gradian";
    }
}

coded! {
    /// A kind of quantity whose display unit, precision and rounding the project sets.
    UnitKind {
        /// Lengths.
        #[default]
        Length = 0, "length";
        /// Areas.
        Area = 1, "area";
        /// Volumes.
        Volume = 2, "volume";
        /// Angles.
        Angle = 3, "angle";
        /// Slopes.
        Slope = 4, "slope";
    }
}

/// The most decimals a display precision can ask for.
pub const MAX_PRECISION: i64 = 8;
/// The range of a rounding increment, in display units.
pub const ROUNDING_RANGE: (f64, f64) = (1e-9, 1e9);

/// How a project shows and reads lengths, areas, volumes, angles and slopes (AB-DATUM-004): the
/// unit of each kind, its decimals and an optional rounding increment in display units. Stored
/// values stay SI; this only decides text.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct UnitSettings {
    /// The length unit.
    pub length: LengthUnit,
    /// The area unit.
    pub area: AreaUnit,
    /// The volume unit.
    pub volume: VolumeUnit,
    /// The angle unit.
    pub angle: AngleUnit,
    /// The slope unit.
    pub slope: SlopeUnit,
    /// The decimals asked for per [`UnitKind`] (in code order); the unit's default when absent.
    pub precision: [Option<u8>; 5],
    /// The rounding increment per [`UnitKind`], in display units; none when absent.
    pub rounding: [Option<f64>; 5],
}

impl UnitSettings {
    /// The settings of a project record's values; absent fields give the defaults.
    #[must_use]
    pub fn of(values: &[Option<arc_doc::Value>]) -> Self {
        use crate::types::project as p;
        let int = |at| match values.get(at) {
            Some(Some(arc_doc::Value::I64(v))) => u8::try_from(*v)
                .ok()
                .filter(|v| i64::from(*v) <= MAX_PRECISION),
            _ => None,
        };
        let real = |at| match values.get(at) {
            Some(Some(arc_doc::Value::F64(v))) if v.is_finite() && *v > 0.0 => Some(*v),
            _ => None,
        };
        let length = match values.get(p::LENGTH_UNIT) {
            Some(Some(arc_doc::Value::Enum(code))) => LengthUnit::from_code(*code),
            _ => LengthUnit::default(),
        };
        Self {
            length,
            area: AreaUnit::of_field(values, p::AREA_UNIT),
            volume: VolumeUnit::of_field(values, p::VOLUME_UNIT),
            angle: AngleUnit::of_field(values, p::ANGLE_UNIT),
            slope: SlopeUnit::of_field(values, p::SLOPE_UNIT),
            precision: [
                int(p::LENGTH_PRECISION),
                int(p::AREA_PRECISION),
                int(p::VOLUME_PRECISION),
                int(p::ANGLE_PRECISION),
                int(p::SLOPE_PRECISION),
            ],
            rounding: [
                real(p::LENGTH_ROUNDING),
                real(p::AREA_ROUNDING),
                real(p::VOLUME_ROUNDING),
                real(p::ANGLE_ROUNDING),
                real(p::SLOPE_ROUNDING),
            ],
        }
    }

    fn at(kind: UnitKind) -> usize {
        // `UnitKind` codes are the positions of the arrays.
        usize::try_from(kind.code()).unwrap_or(0)
    }

    /// The registry name of the unit of a kind (slopes have none).
    #[must_use]
    pub fn unit_name(&self, kind: UnitKind) -> Option<&'static str> {
        match kind {
            UnitKind::Length => Some(self.length.name()),
            UnitKind::Area => Some(self.area.name()),
            UnitKind::Volume => Some(self.volume.name()),
            UnitKind::Angle => Some(self.angle.name()),
            UnitKind::Slope => None,
        }
    }

    /// The display format of a kind with the project's precision and rounding; `None` for
    /// slopes, which [`Self::slope_text`] writes.
    #[must_use]
    pub fn format(&self, kind: UnitKind) -> Option<DisplayFormat> {
        let name = self.unit_name(kind)?;
        let mut format = DisplayFormat::new(*Unit::by_name(name).ok()?);
        format.precision = match kind {
            UnitKind::Length => self.length.format()?.precision,
            UnitKind::Area => match self.area {
                AreaUnit::SquareMeter | AreaUnit::SquareFoot => 2,
                AreaUnit::SquareCentimeter | AreaUnit::SquareMillimeter => 0,
            },
            UnitKind::Volume => match self.volume {
                VolumeUnit::CubicMeter | VolumeUnit::CubicFoot | VolumeUnit::CubicYard => 3,
                VolumeUnit::Liter => 0,
            },
            UnitKind::Angle => match self.angle {
                AngleUnit::Degree => 1,
                AngleUnit::Radian => 3,
                AngleUnit::Gradian => 2,
            },
            UnitKind::Slope => return None,
        };
        if let Some(p) = self.precision[Self::at(kind)] {
            format.precision = p;
        }
        format.rounding_increment = self.rounding[Self::at(kind)];
        Some(format)
    }

    /// `si` (m, m², m³ or radians) written in the unit of `kind`; empty when it has no format.
    #[must_use]
    pub fn text(&self, kind: UnitKind, si: f64, separator: char) -> String {
        let dimension = match kind {
            UnitKind::Length => arc_base::Dimension::LENGTH,
            UnitKind::Area => arc_base::Dimension::AREA,
            UnitKind::Volume => arc_base::Dimension::VOLUME,
            UnitKind::Angle => arc_base::Dimension::ANGLE,
            UnitKind::Slope => return self.slope_text(si, separator),
        };
        self.format(kind)
            .and_then(|f| {
                f.format(arc_base::Quantity::new(si, dimension), separator)
                    .ok()
            })
            .unwrap_or_default()
    }

    /// A length in meters.
    #[must_use]
    pub fn length_text(&self, meters: f64, separator: char) -> String {
        self.text(UnitKind::Length, meters, separator)
    }

    /// An area in square meters.
    #[must_use]
    pub fn area_text(&self, square_meters: f64, separator: char) -> String {
        self.text(UnitKind::Area, square_meters, separator)
    }

    /// A volume in cubic meters.
    #[must_use]
    pub fn volume_text(&self, cubic_meters: f64, separator: char) -> String {
        self.text(UnitKind::Volume, cubic_meters, separator)
    }

    /// An angle in radians.
    #[must_use]
    pub fn angle_text(&self, radians: f64, separator: char) -> String {
        self.text(UnitKind::Angle, radians, separator)
    }

    /// A slope given as an angle in radians, as the project's slope unit with its precision and
    /// rounding.
    #[must_use]
    pub fn slope_text(&self, radians: f64, separator: char) -> String {
        let at = Self::at(UnitKind::Slope);
        let (value, default, suffix) = match self.slope {
            SlopeUnit::Degrees => (radians.to_degrees(), 2, "°"),
            SlopeUnit::Percent => (radians.tan() * 100.0, 1, "%"),
            SlopeUnit::RiseRun => (1.0 / radians.tan(), 2, ""),
        };
        let value = match self.rounding[at] {
            Some(step) => (value / step).round() * step,
            None => value,
        };
        let precision = usize::from(self.precision[at].unwrap_or(default));
        let text = match self.slope {
            SlopeUnit::RiseRun => format!("1:{value:.precision$}"),
            _ => format!("{value:.precision$}{suffix}"),
        };
        text.replace('.', &separator.to_string())
    }

    /// The display units for a property grid: one format per kind that has a registry unit.
    #[must_use]
    pub fn display_units(&self) -> arc_base::DisplayUnits {
        let mut units = arc_base::DisplayUnits::new();
        for kind in [
            UnitKind::Length,
            UnitKind::Area,
            UnitKind::Volume,
            UnitKind::Angle,
        ] {
            if let Some(format) = self.format(kind) {
                units.set(format);
            }
        }
        units
    }

    /// A value a person typed, in SI (meters, square meters, cubic meters, radians). A unit
    /// symbol or one of the ascii spellings `m2`, `m3`, `deg` says its own unit. A bare number
    /// is meters, square meters or cubic meters for lengths, areas and volumes (the rule of every
    /// command), and the project's angle or slope unit for angles and slopes. `None` for text
    /// that is no value of the kind.
    #[must_use]
    pub fn parse(&self, kind: UnitKind, text: &str, separator: char) -> Option<f64> {
        let dimension = match kind {
            UnitKind::Length => arc_base::Dimension::LENGTH,
            UnitKind::Area => arc_base::Dimension::AREA,
            UnitKind::Volume => arc_base::Dimension::VOLUME,
            UnitKind::Angle => arc_base::Dimension::ANGLE,
            UnitKind::Slope => return parse_slope(text, self.slope),
        };
        let text = text.trim();
        let aliases = [("m2", "m²"), ("m3", "m³"), ("deg", "°"), ("sqm", "m²")];
        // The longest symbol the text ends with.
        let mut best: Option<(&str, &'static Unit)> = None;
        for unit in Unit::all().iter().filter(|u| u.dimension == dimension) {
            let spellings = std::iter::once(unit.symbol).chain(
                aliases
                    .iter()
                    .filter(|(_, s)| *s == unit.symbol)
                    .map(|(a, _)| *a),
            );
            for spelling in spellings {
                if text.ends_with(spelling) && best.is_none_or(|(b, _)| spelling.len() > b.len()) {
                    best = Some((spelling, unit));
                }
            }
        }
        let (number, unit) = match best {
            Some((spelling, unit)) => (text.get(..text.len() - spelling.len())?.trim(), Some(unit)),
            None => (text, None),
        };
        let number = if separator == ',' {
            number.replace(',', ".")
        } else {
            number.to_owned()
        };
        let value = number.parse::<f64>().ok().filter(|v| v.is_finite())?;
        let si = match unit {
            Some(unit) => unit.to_si(value),
            None => match kind {
                UnitKind::Angle => Unit::by_name(self.angle.name()).ok()?.to_si(value),
                _ => value,
            },
        };
        si.is_finite().then_some(si)
    }
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

    #[test]
    fn display_units_set_the_text_precision_and_rounding_of_each_kind() {
        let mut u = UnitSettings::default();
        assert_eq!(u.area_text(12.3456, '.'), "12.35 m²");
        assert_eq!(u.volume_text(2.0, ','), "2 m³");
        assert_eq!(u.angle_text(std::f64::consts::FRAC_PI_2, '.'), "90°");
        u.area = AreaUnit::SquareFoot;
        u.precision[1] = Some(0);
        assert_eq!(u.area_text(1.0, '.'), "11 ft²");
        u.rounding[0] = Some(0.05);
        assert_eq!(u.length_text(1.234, '.'), "1.25 m");
        u.slope = SlopeUnit::Percent;
        u.precision[4] = Some(0);
        assert_eq!(u.slope_text(std::f64::consts::FRAC_PI_4, '.'), "100%");
    }

    #[test]
    fn typed_values_take_a_unit_and_a_bare_number_stays_si() {
        let mut u = UnitSettings::default();
        let close = |a: Option<f64>, b: f64| (a.unwrap() - b).abs() < 1e-9;
        assert!(close(u.parse(UnitKind::Length, "0.35", '.'), 0.35));
        assert!(close(u.parse(UnitKind::Length, "350 mm", '.'), 0.35));
        assert!(close(u.parse(UnitKind::Length, "0,35", ','), 0.35));
        assert!(close(u.parse(UnitKind::Area, "12", '.'), 12.0));
        assert!(close(u.parse(UnitKind::Area, "100 ft²", '.'), 9.290_304));
        assert!(close(u.parse(UnitKind::Volume, "500 L", '.'), 0.5));
        assert!(close(
            u.parse(UnitKind::Angle, "90", '.'),
            std::f64::consts::FRAC_PI_2
        ));
        assert!(close(
            u.parse(UnitKind::Angle, "100 gon", '.'),
            std::f64::consts::FRAC_PI_2
        ));
        u.angle = AngleUnit::Radian;
        assert!(close(u.parse(UnitKind::Angle, "1", '.'), 1.0));
        assert!(u.parse(UnitKind::Length, "abc", '.').is_none());
        assert!(u.parse(UnitKind::Length, "1 m²", '.').is_none());
    }
}
