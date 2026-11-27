//! The drafts the type editor keeps for railing types and ramp types: the values as typed (lengths
//! in millimeters, the largest slope of a ramp in percent), checked against the ranges of the
//! model, and the parameters of the define command they become on Apply.

use arc_base::ObjectId;
use arc_doc::Snapshot;
use archbuilder_model::railing::{
    Corner, HEIGHT_LIMITS, Profile, RailingType, SECTION_RANGE, SPACING_RANGE,
};
use archbuilder_model::ramp::{self, SLOPE_RANGE, THICKNESS_RANGE};
use archbuilder_model::spatial;
use archbuilder_model::types::{RAILING_TYPE, RAMP_TYPE};

/// The two kinds of circulation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircKind {
    /// Railing types.
    Railing,
    /// Ramp types.
    Ramp,
}

impl CircKind {
    /// The key of the type records.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Railing => RAILING_TYPE,
            Self::Ramp => RAMP_TYPE,
        }
    }

    /// The kind of circulation type the object `id` (an instance, or a type) belongs to.
    #[must_use]
    pub fn of_object(snapshot: &Snapshot, id: ObjectId) -> Option<(Self, Option<ObjectId>)> {
        use archbuilder_model::types::{RAILING, RAMP, railing as r, ramp as m};
        let key = spatial::type_of(snapshot, id)?;
        let object = spatial::get(snapshot, id)?;
        let reference = |at: usize| match object.values.get(at) {
            Some(Some(arc_doc::Value::Reference(t))) => Some(*t),
            _ => None,
        };
        match key.as_str() {
            RAILING => Some((Self::Railing, reference(r::RAILING_TYPE))),
            RAMP => Some((Self::Ramp, reference(m::RAMP_TYPE))),
            RAILING_TYPE => Some((Self::Railing, Some(id))),
            RAMP_TYPE => Some((Self::Ramp, Some(id))),
            _ => None,
        }
    }
}

/// The types of `kind` as `(identifier, name)`, by name.
#[must_use]
pub fn types_of(snapshot: &Snapshot, kind: CircKind) -> Vec<(ObjectId, String)> {
    let mut out: Vec<(ObjectId, String)> = spatial::objects(snapshot, kind.key())
        .iter()
        .map(|o| {
            (
                o.id,
                archbuilder_model::types::text(&o.values, 0)
                    .unwrap_or_default()
                    .to_owned(),
            )
        })
        .collect();
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

/// How many railings or ramps use the type `id`.
#[must_use]
pub fn users(snapshot: &Snapshot, id: ObjectId) -> usize {
    archbuilder_commands::circulation::type_users(snapshot, id).len()
}

/// The values of a railing type as typed.
#[derive(Debug, Clone, PartialEq)]
pub struct RailingDraft {
    /// The name.
    pub name: String,
    /// The lengths in millimeters: top rail height, rail width, rail depth, handrail height,
    /// baluster size, baluster spacing, post size, fillet radius.
    pub lengths: [String; 8],
    /// The profile of the top rail.
    pub rail_profile: Profile,
    /// The profile of the balusters.
    pub baluster_profile: Profile,
    /// How the rail turns at a corner.
    pub corner: Corner,
}

/// The values of a ramp type as typed.
#[derive(Debug, Clone, PartialEq)]
pub struct RampDraft {
    /// The name.
    pub name: String,
    /// The largest slope in percent.
    pub max_slope: String,
    /// The thickness in millimeters.
    pub thickness: String,
}

/// A draft of either kind.
#[derive(Debug, Clone, PartialEq)]
pub enum Draft {
    /// A railing type.
    Railing(RailingDraft),
    /// A ramp type.
    Ramp(RampDraft),
}

/// A message of the catalog, named by identifier.
#[derive(Debug, Clone, Copy)]
pub struct Label {
    /// The identifier in the catalogs.
    pub message_id: &'static str,
}

/// The message identifiers of the railing lengths, in the order of [`RailingDraft::lengths`].
pub const RAILING_LABELS: [Label; 8] = [
    Label {
        message_id: "ab-types-rail-height-mm",
    },
    Label {
        message_id: "ab-types-rail-width-mm",
    },
    Label {
        message_id: "ab-types-rail-depth-mm",
    },
    Label {
        message_id: "ab-types-handrail-height-mm",
    },
    Label {
        message_id: "ab-types-baluster-size-mm",
    },
    Label {
        message_id: "ab-types-baluster-spacing-mm",
    },
    Label {
        message_id: "ab-types-post-size-mm",
    },
    Label {
        message_id: "ab-types-fillet-radius-mm",
    },
];

/// The message identifiers of the ramp fields.
pub const RAMP_LABELS: [Label; 2] = [
    Label {
        message_id: "ab-types-max-slope-percent",
    },
    Label {
        message_id: "ab-types-ramp-thickness-mm",
    },
];

fn mm(meters: f64) -> String {
    let v = (meters * 1000.0 * 100.0).round() / 100.0;
    format!("{v}")
}

fn number(text: &str) -> Option<f64> {
    text.trim()
        .replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

/// A field that cannot be applied: the message identifier of its label.
pub type Invalid = &'static str;

impl Draft {
    /// The draft of the type `id` of `kind`; `None` when the record is unreadable.
    #[must_use]
    pub fn of(snapshot: &Snapshot, kind: CircKind, id: ObjectId) -> Option<Self> {
        let object = spatial::get(snapshot, id)?;
        let name = archbuilder_model::types::text(&object.values, 0)
            .unwrap_or_default()
            .to_owned();
        Some(match kind {
            CircKind::Railing => {
                let t = RailingType::from_values(&object.values)?;
                Self::Railing(RailingDraft {
                    name,
                    lengths: [
                        t.height,
                        t.rail_width,
                        t.rail_depth,
                        t.handrail_height,
                        t.baluster_size,
                        t.baluster_spacing,
                        t.post_size,
                        t.fillet_radius,
                    ]
                    .map(mm),
                    rail_profile: t.rail_profile,
                    baluster_profile: t.baluster_profile,
                    corner: t.corner,
                })
            }
            CircKind::Ramp => {
                let max = match object
                    .values
                    .get(archbuilder_model::types::ramp_type::MAX_SLOPE)
                {
                    Some(Some(arc_doc::Value::F64(v))) => *v,
                    _ => ramp::DEFAULT_MAX_SLOPE,
                };
                let thickness = archbuilder_model::types::meters(
                    &object.values,
                    archbuilder_model::types::ramp_type::THICKNESS,
                )
                .unwrap_or(ramp::DEFAULT_THICKNESS);
                Self::Ramp(RampDraft {
                    name,
                    max_slope: format!("{}", (max * 10_000.0).round() / 100.0),
                    thickness: mm(thickness),
                })
            }
        })
    }

    /// The name as typed.
    pub fn name_mut(&mut self) -> &mut String {
        match self {
            Self::Railing(d) => &mut d.name,
            Self::Ramp(d) => &mut d.name,
        }
    }

    /// The parameters of the define command (`archbuilder.railing_type.define` or
    /// `archbuilder.ramp_type.define`) that apply the draft to the type `id`.
    ///
    /// # Errors
    /// The label of the first field that is not a number within its range.
    pub fn params(&self, id: ObjectId) -> Result<(&'static str, serde_json::Value), Invalid> {
        match self {
            Self::Railing(d) => {
                let limits: [(f64, f64); 8] = [
                    HEIGHT_LIMITS,
                    SECTION_RANGE,
                    SECTION_RANGE,
                    (0.0, HEIGHT_LIMITS.1),
                    SECTION_RANGE,
                    SPACING_RANGE,
                    SECTION_RANGE,
                    (0.0, 2.0),
                ];
                let mut v = [0.0_f64; 8];
                for k in 0..8 {
                    let m = number(&d.lengths[k]).map(|x| x / 1000.0);
                    // A handrail height of 0 means none: the range starts at the lowest height
                    // otherwise.
                    v[k] = m
                        .filter(|x| {
                            (*x >= limits[k].0 - 1.0e-9 && *x <= limits[k].1 + 1.0e-9)
                                && (k != 3 || *x < 1.0e-9 || *x >= HEIGHT_LIMITS.0 - 1.0e-9)
                        })
                        .ok_or(RAILING_LABELS[k].message_id)?;
                }
                let names = [
                    "height",
                    "rail_width",
                    "rail_depth",
                    "handrail_height",
                    "baluster_size",
                    "baluster_spacing",
                    "post_size",
                    "fillet_radius",
                ];
                let mut map = serde_json::Map::new();
                map.insert("type".into(), id.to_string().into());
                map.insert("name".into(), d.name.trim().into());
                for (n, x) in names.iter().zip(v) {
                    map.insert((*n).into(), x.into());
                }
                map.insert("rail_profile".into(), d.rail_profile.name().into());
                map.insert("baluster_profile".into(), d.baluster_profile.name().into());
                map.insert("corner".into(), d.corner.name().into());
                Ok((
                    archbuilder_commands::circulation::RAILING_TYPE_DEFINE,
                    serde_json::Value::Object(map),
                ))
            }
            Self::Ramp(d) => {
                let slope = number(&d.max_slope)
                    .map(|p| p / 100.0)
                    .filter(|s| *s >= SLOPE_RANGE.0 - 1.0e-9 && *s <= SLOPE_RANGE.1 + 1.0e-9)
                    .ok_or(RAMP_LABELS[0].message_id)?;
                let thickness = number(&d.thickness)
                    .map(|x| x / 1000.0)
                    .filter(|t| {
                        *t >= THICKNESS_RANGE.0 - 1.0e-9 && *t <= THICKNESS_RANGE.1 + 1.0e-9
                    })
                    .ok_or(RAMP_LABELS[1].message_id)?;
                Ok((
                    archbuilder_commands::circulation::RAMP_TYPE_DEFINE,
                    serde_json::json!({
                        "type": id.to_string(), "name": d.name.trim(),
                        "max_slope": slope, "thickness": thickness,
                    }),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use arc_base::id::Identifier as _;

    use super::*;

    #[test]
    fn typed_values_are_checked_against_the_ranges_and_named_by_label() {
        let id = ObjectId::from_u128(9);
        let mut d = Draft::Railing(RailingDraft {
            name: " Rail ".into(),
            lengths: ["900", "50", "40", "0", "20", "120", "50", "150"].map(String::from),
            rail_profile: Profile::Square,
            baluster_profile: Profile::Round,
            corner: Corner::Fillet,
        });
        let (command, params) = d.params(id).unwrap();
        assert_eq!(command, "archbuilder.railing_type.define");
        assert_eq!(params["name"], "Rail");
        assert_eq!(params["handrail_height"], 0.0);
        assert_eq!(params["baluster_profile"], "round");
        // A comma is a decimal separator; a handrail between 0 and the lowest height, or a
        // spacing outside its range, names its field.
        if let Draft::Railing(r) = &mut d {
            r.lengths[3] = "100".into();
        }
        assert_eq!(d.params(id).unwrap_err(), RAILING_LABELS[3].message_id);
        if let Draft::Railing(r) = &mut d {
            r.lengths[3] = "0".into();
            r.lengths[5] = "5000".into();
        }
        assert_eq!(d.params(id).unwrap_err(), RAILING_LABELS[5].message_id);
        let ramp = Draft::Ramp(RampDraft {
            name: "R".into(),
            max_slope: "8,33".into(),
            thickness: "200".into(),
        });
        let (command, params) = ramp.params(id).unwrap();
        assert_eq!(command, "archbuilder.ramp_type.define");
        assert!((params["max_slope"].as_f64().unwrap() - 0.0833).abs() < 1.0e-9);
        let bad = Draft::Ramp(RampDraft {
            name: "R".into(),
            max_slope: "250".into(),
            thickness: "200".into(),
        });
        assert_eq!(bad.params(id).unwrap_err(), RAMP_LABELS[0].message_id);
    }
}
