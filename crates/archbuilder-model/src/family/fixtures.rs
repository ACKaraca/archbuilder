//! Reference families built in code: a table, a door and a cabinet. They are the fixtures of the
//! text round trip, the evaluator, the packages and the loading of families into projects.
//! Volumes in the tests that use them are computed by hand from the dimensions.

use std::collections::BTreeSet;

use arc_base::id::Identifier;
use arc_base::{ObjectId, Unit};

use super::def::{
    Axis, Category, Definition, Detail, FormDef, FormKind, Header, Hosting, ParamDef, PlaneDef,
    PlaneRole, ProfileDef, Role, Scope, SymbolDef, SymbolKind, TestCase, TypeDef, Version, Vertex,
    ViewKind, Visibility,
};
use crate::expr::{DataType, Evaluated, Formula, Stored};

fn id(n: u128) -> ObjectId {
    ObjectId::from_u128(n)
}

/// A literal length in meters.
#[must_use]
pub fn len(v: f64) -> Stored {
    Stored::Literal(Evaluated::Number(v))
}

/// A formula of lengths (bare numbers are meters). A text that does not parse becomes the
/// literal 0, which the fixtures' own tests would notice.
#[must_use]
pub fn fm(text: &str) -> Stored {
    Formula::new(text, Unit::by_symbol("m").ok())
        .map_or(Stored::Literal(Evaluated::Number(0.0)), Stored::Formula)
}

fn header(n: u128, name: &str, category: Category, hosting: Hosting) -> Header {
    Header {
        id: id(n),
        version: Version {
            major: 1,
            minor: 0,
            patch: 0,
        },
        name: name.into(),
        category,
        hosting,
        flags: BTreeSet::new(),
        ifc_entity: String::new(),
        ifc_type: String::new(),
        functions: (1, 0),
        source: "ArchBuilder fixtures".into(),
        license: "Apache-2.0".into(),
    }
}

fn param(
    n: u128,
    key: &str,
    name: &str,
    data: DataType,
    scope: Scope,
    default: Option<Stored>,
) -> ParamDef {
    ParamDef {
        id: id(n),
        key: key.into(),
        name: name.into(),
        data,
        scope,
        role: Role::Driving,
        group: "Dimensions".into(),
        order: i64::try_from(n % 1000).unwrap_or(0),
        default,
        minimum: None,
        maximum: None,
        allowed: Vec::new(),
    }
}

fn plane(key: &str, axis: Axis, offset: Stored, role: PlaneRole) -> PlaneDef {
    PlaneDef {
        key: key.into(),
        name: key.into(),
        axis,
        offset,
        role,
        strong: true,
    }
}

/// A vertex at plane `u` plus `du`, plane `v` plus `dv`.
#[must_use]
pub fn vx(u: &str, du: Option<Stored>, v: &str, dv: Option<Stored>) -> Vertex {
    Vertex {
        u: Some(u.into()),
        du,
        v: Some(v.into()),
        dv,
        bulge: None,
    }
}

fn form(key: &str, kind: FormKind, profile: &str, from: &str, to: &str) -> FormDef {
    FormDef {
        key: key.into(),
        kind,
        profile: profile.into(),
        from: from.into(),
        to: Some(to.into()),
        depth: None,
        material: "body".into(),
        subcategory: String::new(),
        visibility: Visibility::default(),
        cut_host: false,
    }
}

fn ty(n: u128, name: &str, values: &[(&str, f64)]) -> TypeDef {
    TypeDef {
        id: id(n),
        name: name.into(),
        values: values
            .iter()
            .map(|(k, v)| ((*k).to_owned(), len(*v)))
            .collect(),
    }
}

fn empty(header: Header) -> Definition {
    Definition {
        header,
        parameters: Vec::new(),
        types: Vec::new(),
        planes: Vec::new(),
        profiles: Vec::new(),
        forms: Vec::new(),
        symbols: Vec::new(),
        tables: Vec::new(),
        arrays: Vec::new(),
        nested: Vec::new(),
        tests: Vec::new(),
    }
}

fn case(name: &str, type_name: Option<&str>, volume: Option<f64>) -> TestCase {
    TestCase {
        name: name.into(),
        type_name: type_name.map(str::to_owned),
        volume,
        tolerance: Some(1e-9),
        ..TestCase::default()
    }
}

/// A table: a top on four legs. Types `1400x800` and `1800x900`; instance parameter `LegSize`.
#[must_use]
#[allow(
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    reason = "one pass over the parts of a definition, in order"
)]
pub fn table() -> Definition {
    let mut d = empty(header(0x10, "Table", Category::Furniture, Hosting::None));
    d.parameters = vec![
        param(
            0x11,
            "Width",
            "Width",
            DataType::Length,
            Scope::Type,
            Some(len(1.4)),
        ),
        param(
            0x12,
            "Depth",
            "Depth",
            DataType::Length,
            Scope::Type,
            Some(len(0.8)),
        ),
        param(
            0x13,
            "Height",
            "Height",
            DataType::Length,
            Scope::Type,
            Some(len(0.75)),
        ),
        param(
            0x14,
            "TopThickness",
            "Top thickness",
            DataType::Length,
            Scope::Type,
            Some(len(0.04)),
        ),
        param(
            0x15,
            "LegSize",
            "Leg size",
            DataType::Length,
            Scope::Instance,
            Some(len(0.06)),
        ),
    ];
    let mut volume = param(
        0x16,
        "Volume",
        "Volume",
        DataType::Volume,
        Scope::Instance,
        Some(fm("solid_volume")),
    );
    volume.role = Role::Reporting;
    d.parameters.push(volume);
    d.types = vec![
        ty(
            0x21,
            "1400x800",
            &[("Width", 1.4), ("Depth", 0.8), ("Height", 0.75)],
        ),
        ty(
            0x22,
            "1800x900",
            &[("Width", 1.8), ("Depth", 0.9), ("Height", 0.75)],
        ),
    ];
    d.planes = vec![
        plane("left", Axis::X, len(0.0), PlaneRole::Left),
        plane("right", Axis::X, fm("Width"), PlaneRole::Right),
        plane("front", Axis::Y, len(0.0), PlaneRole::Front),
        plane("back", Axis::Y, fm("Depth"), PlaneRole::Back),
        plane("top", Axis::Z, fm("Height"), PlaneRole::Top),
        plane(
            "under",
            Axis::Z,
            fm("Height - TopThickness"),
            PlaneRole::None,
        ),
    ];
    let rect = |a: (&str, Option<Stored>),
                b: (&str, Option<Stored>),
                c: (&str, Option<Stored>),
                e: (&str, Option<Stored>)| {
        vec![
            vx(a.0, a.1.clone(), c.0, c.1.clone()),
            vx(b.0, b.1.clone(), c.0, c.1),
            vx(b.0, b.1, e.0, e.1.clone()),
            vx(a.0, a.1, e.0, e.1),
        ]
    };
    d.profiles.push(ProfileDef {
        open: false,
        key: "top_outline".into(),
        axis: Axis::Z,
        vertices: rect(
            ("left", None),
            ("right", None),
            ("front", None),
            ("back", None),
        ),
    });
    let leg = fm("LegSize");
    let legs = [
        (
            "leg_lf",
            ("left", None),
            ("left", Some(leg.clone())),
            ("front", None),
            ("front", Some(leg.clone())),
        ),
        (
            "leg_rf",
            ("right", Some(fm("-LegSize"))),
            ("right", None),
            ("front", None),
            ("front", Some(leg.clone())),
        ),
        (
            "leg_lb",
            ("left", None),
            ("left", Some(leg.clone())),
            ("back", Some(fm("-LegSize"))),
            ("back", None),
        ),
        (
            "leg_rb",
            ("right", Some(fm("-LegSize"))),
            ("right", None),
            ("back", Some(fm("-LegSize"))),
            ("back", None),
        ),
    ];
    for (key, a, b, c, e) in legs {
        d.profiles.push(ProfileDef {
            open: false,
            key: format!("{key}_outline"),
            axis: Axis::Z,
            vertices: rect(a, b, c, e),
        });
        d.forms.push(form(
            key,
            FormKind::Solid,
            &format!("{key}_outline"),
            "origin_z",
            "under",
        ));
    }
    d.forms
        .push(form("top", FormKind::Solid, "top_outline", "under", "top"));
    let top = 1.4 * 0.8 * 0.04 + 4.0 * 0.06 * 0.06 * (0.75 - 0.04);
    let big = 1.8 * 0.9 * 0.04 + 4.0 * 0.06 * 0.06 * (0.75 - 0.04);
    let mut zero = case("zero width", Some("1400x800"), None);
    zero.set.insert("Width".into(), len(0.0));
    zero.fails = Some("top".into());
    d.tests = vec![
        case("1400x800", Some("1400x800"), Some(top)),
        case("1800x900", Some("1800x900"), Some(big)),
        zero,
    ];
    d
}

/// A door: a U-shaped frame, a leaf and a swing symbol in plan. Types `0900x2100` and
/// `1000x2100`; instance parameter `ShowSwing`.
#[must_use]
#[allow(
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    reason = "one pass over the parts of a definition, in order"
)]
pub fn door() -> Definition {
    let mut d = empty(header(0x30, "Door", Category::Door, Hosting::None));
    d.parameters = vec![
        param(
            0x31,
            "Width",
            "Width",
            DataType::Length,
            Scope::Type,
            Some(len(0.9)),
        ),
        param(
            0x32,
            "Height",
            "Height",
            DataType::Length,
            Scope::Type,
            Some(len(2.1)),
        ),
        param(
            0x33,
            "FrameWidth",
            "Frame width",
            DataType::Length,
            Scope::Type,
            Some(len(0.05)),
        ),
        param(
            0x34,
            "FrameDepth",
            "Frame depth",
            DataType::Length,
            Scope::Type,
            Some(len(0.1)),
        ),
        param(
            0x35,
            "LeafThickness",
            "Leaf thickness",
            DataType::Length,
            Scope::Type,
            Some(len(0.04)),
        ),
        param(
            0x36,
            "ShowSwing",
            "Show swing",
            DataType::Boolean,
            Scope::Instance,
            Some(Stored::Literal(Evaluated::Bool(true))),
        ),
    ];
    d.types = vec![
        ty(0x41, "0900x2100", &[("Width", 0.9), ("Height", 2.1)]),
        ty(0x42, "1000x2100", &[("Width", 1.0), ("Height", 2.1)]),
    ];
    d.planes = vec![
        plane("left", Axis::X, len(0.0), PlaneRole::Left),
        plane("right", Axis::X, fm("Width"), PlaneRole::Right),
        plane("inner_left", Axis::X, fm("FrameWidth"), PlaneRole::None),
        plane(
            "inner_right",
            Axis::X,
            fm("Width - FrameWidth"),
            PlaneRole::None,
        ),
        plane("front", Axis::Y, len(0.0), PlaneRole::Front),
        plane("back", Axis::Y, fm("FrameDepth"), PlaneRole::Back),
        plane(
            "leaf_front",
            Axis::Y,
            fm("(FrameDepth - LeafThickness) / 2"),
            PlaneRole::None,
        ),
        plane(
            "leaf_back",
            Axis::Y,
            fm("(FrameDepth + LeafThickness) / 2"),
            PlaneRole::None,
        ),
        plane("head", Axis::Z, fm("Height"), PlaneRole::Top),
        plane(
            "head_inner",
            Axis::Z,
            fm("Height - FrameWidth"),
            PlaneRole::None,
        ),
    ];
    // A plane perpendicular to y: the profile axes are (z, x).
    let zx = |z: (&str, Option<Stored>), x: (&str, Option<Stored>)| vx(z.0, z.1, x.0, x.1);
    d.profiles.push(ProfileDef {
        open: false,
        key: "frame_outline".into(),
        axis: Axis::Y,
        vertices: vec![
            zx(("origin_z", None), ("left", None)),
            zx(("origin_z", None), ("inner_left", None)),
            zx(("head_inner", None), ("inner_left", None)),
            zx(("head_inner", None), ("inner_right", None)),
            zx(("origin_z", None), ("inner_right", None)),
            zx(("origin_z", None), ("right", None)),
            zx(("head", None), ("right", None)),
            zx(("head", None), ("left", None)),
        ],
    });
    d.profiles.push(ProfileDef {
        open: false,
        key: "leaf_outline".into(),
        axis: Axis::Y,
        vertices: vec![
            zx(("origin_z", None), ("inner_left", None)),
            zx(("origin_z", None), ("inner_right", None)),
            zx(("head_inner", None), ("inner_right", None)),
            zx(("head_inner", None), ("inner_left", None)),
        ],
    });
    let mut frame = form("frame", FormKind::Solid, "frame_outline", "front", "back");
    frame.material = "frame".into();
    let mut leaf = form(
        "leaf",
        FormKind::Solid,
        "leaf_outline",
        "leaf_front",
        "leaf_back",
    );
    leaf.material = "leaf".into();
    d.forms = vec![frame, leaf];
    // Plan symbols: the leaf opened ninety degrees and its swing arc, at medium and high detail.
    let swing_visibility = Visibility {
        details: vec![Detail::Medium, Detail::High],
        views: vec![ViewKind::Plan],
        when: Formula::new("ShowSwing", None).ok(),
    };
    let xy = |x: (&str, Option<Stored>), y: (&str, Option<Stored>)| vx(x.0, x.1, y.0, y.1);
    d.symbols.push(SymbolDef {
        key: "swing".into(),
        view: ViewKind::Plan,
        kind: SymbolKind::Line,
        subcategory: "swing".into(),
        visibility: swing_visibility.clone(),
        vertices: vec![
            {
                let mut v = xy(
                    ("inner_left", None),
                    ("front", Some(fm("-(Width - 2 * FrameWidth)"))),
                );
                v.bulge = Formula::new("tan(22.5 deg)", None)
                    .ok()
                    .map(Stored::Formula);
                v
            },
            xy(("inner_right", None), ("front", None)),
        ],
    });
    d.symbols.push(SymbolDef {
        key: "swing_leaf".into(),
        view: ViewKind::Plan,
        kind: SymbolKind::Line,
        subcategory: "swing".into(),
        visibility: swing_visibility,
        vertices: vec![
            xy(("inner_left", None), ("front", None)),
            xy(
                ("inner_left", None),
                ("front", Some(fm("-(Width - 2 * FrameWidth)"))),
            ),
        ],
    });
    let frame_v = 0.1 * (0.9 * 2.1 - (0.9 - 0.1) * (2.1 - 0.05));
    let leaf_v = (0.9 - 0.1) * (2.1 - 0.05) * 0.04;
    let wide = (0.1 * (1.0 * 2.1 - (1.0 - 0.1) * (2.1 - 0.05))) + (1.0 - 0.1) * (2.1 - 0.05) * 0.04;
    let mut thick = case("frame wider than the door", Some("0900x2100"), None);
    thick.set.insert("FrameWidth".into(), len(0.5));
    thick.fails = Some("frame".into());
    d.tests = vec![
        case("0900x2100", Some("0900x2100"), Some(frame_v + leaf_v)),
        case("1000x2100", Some("1000x2100"), Some(wide)),
        thick,
    ];
    d
}

/// A cabinet: a body with a plinth notch cut by a prism void through its depth. Types `600`,
/// `800` and `1200`; `Volume` is a reporting parameter.
#[must_use]
#[allow(
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    reason = "one pass over the parts of a definition, in order"
)]
pub fn cabinet() -> Definition {
    let mut d = empty(header(0x50, "Cabinet", Category::Casework, Hosting::None));
    d.parameters = vec![
        param(
            0x51,
            "Width",
            "Width",
            DataType::Length,
            Scope::Type,
            Some(len(0.6)),
        ),
        param(
            0x52,
            "Depth",
            "Depth",
            DataType::Length,
            Scope::Type,
            Some(len(0.6)),
        ),
        param(
            0x53,
            "Height",
            "Height",
            DataType::Length,
            Scope::Type,
            Some(len(0.9)),
        ),
        param(
            0x54,
            "Plinth",
            "Plinth",
            DataType::Length,
            Scope::Type,
            Some(len(0.1)),
        ),
    ];
    let mut volume = param(
        0x55,
        "Volume",
        "Volume",
        DataType::Volume,
        Scope::Instance,
        Some(fm("solid_volume")),
    );
    volume.role = Role::Reporting;
    d.parameters.push(volume);
    d.types = vec![
        ty(0x61, "600", &[("Width", 0.6)]),
        ty(0x62, "800", &[("Width", 0.8)]),
        ty(0x63, "1200", &[("Width", 1.2)]),
    ];
    d.planes = vec![
        plane("left", Axis::X, len(0.0), PlaneRole::Left),
        plane("right", Axis::X, fm("Width"), PlaneRole::Right),
        plane("front", Axis::Y, len(0.0), PlaneRole::Front),
        plane("back", Axis::Y, fm("Depth"), PlaneRole::Back),
        plane("top", Axis::Z, fm("Height"), PlaneRole::Top),
        plane("notch_left", Axis::X, fm("Width / 10"), PlaneRole::None),
        plane(
            "notch_right",
            Axis::X,
            fm("Width - Width / 10"),
            PlaneRole::None,
        ),
        plane("notch_top", Axis::Z, fm("Plinth"), PlaneRole::None),
        plane("through_front", Axis::Y, fm("-0.1"), PlaneRole::None),
        plane("through_back", Axis::Y, fm("Depth + 0.1"), PlaneRole::None),
    ];
    d.profiles.push(ProfileDef {
        open: false,
        key: "body_outline".into(),
        axis: Axis::Z,
        vertices: vec![
            vx("left", None, "front", None),
            vx("right", None, "front", None),
            vx("right", None, "back", None),
            vx("left", None, "back", None),
        ],
    });
    // The notch profile lies in a plane perpendicular to y: axes (z, x).
    d.profiles.push(ProfileDef {
        open: false,
        key: "notch_outline".into(),
        axis: Axis::Y,
        vertices: vec![
            vx("origin_z", Some(fm("-0.1")), "notch_left", None),
            vx("notch_top", None, "notch_left", None),
            vx("notch_top", None, "notch_right", None),
            vx("origin_z", Some(fm("-0.1")), "notch_right", None),
        ],
    });
    d.forms = vec![
        form(
            "carcass",
            FormKind::Solid,
            "body_outline",
            "origin_z",
            "top",
        ),
        form(
            "notch",
            FormKind::Void,
            "notch_outline",
            "through_front",
            "through_back",
        ),
    ];
    let full = |w: f64| w * 0.6 * 0.9 - 0.8 * w * 0.6 * 0.1;
    let mut zero = case("zero width", Some("600"), None);
    zero.set.insert("Width".into(), len(0.0));
    zero.fails = Some("carcass".into());
    d.tests = vec![
        case("600", Some("600"), Some(full(0.6))),
        case("800", Some("800"), Some(full(0.8))),
        case("1200", Some("1200"), Some(full(1.2))),
        zero,
    ];
    d
}

/// A door hosted by walls: a leaf panel and a void through the host, centered on the wall's
/// axis. Types `0900x2100` and `1000x2100`.
#[must_use]
pub fn hosted_door() -> Definition {
    let mut d = empty(header(0x70, "Wall door", Category::Door, Hosting::Wall));
    d.parameters = vec![
        param(
            0x71,
            "Width",
            "Width",
            DataType::Length,
            Scope::Type,
            Some(len(0.9)),
        ),
        param(
            0x72,
            "Height",
            "Height",
            DataType::Length,
            Scope::Type,
            Some(len(2.1)),
        ),
    ];
    d.types = vec![
        ty(0x73, "0900x2100", &[("Width", 0.9), ("Height", 2.1)]),
        ty(0x74, "1000x2100", &[("Width", 1.0), ("Height", 2.1)]),
    ];
    d.planes = vec![
        plane("left", Axis::X, len(0.0), PlaneRole::Left),
        plane("right", Axis::X, fm("Width"), PlaneRole::Right),
        plane("leaf_front", Axis::Y, len(-0.02), PlaneRole::None),
        plane("leaf_back", Axis::Y, len(0.02), PlaneRole::None),
        plane("through_front", Axis::Y, len(-0.5), PlaneRole::None),
        plane("through_back", Axis::Y, len(0.5), PlaneRole::None),
        plane("head", Axis::Z, fm("Height"), PlaneRole::Top),
    ];
    for (key, front, back) in [
        ("leaf_outline", "leaf_front", "leaf_back"),
        ("opening_outline", "through_front", "through_back"),
    ] {
        d.profiles.push(ProfileDef {
            open: false,
            key: key.into(),
            axis: Axis::Z,
            vertices: vec![
                vx("left", None, front, None),
                vx("right", None, front, None),
                vx("right", None, back, None),
                vx("left", None, back, None),
            ],
        });
    }
    d.forms = vec![
        form("leaf", FormKind::Solid, "leaf_outline", "origin_z", "head"),
        form(
            "opening",
            FormKind::Void,
            "opening_outline",
            "origin_z",
            "head",
        ),
    ];
    d.forms[1].cut_host = true;
    d
}
