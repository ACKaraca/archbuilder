//! The family definition (ADR 0009, section 3): the authored data of one family, with its
//! validation and limits. Only authored data is here: the parsed trees, the evaluated planes,
//! solids and symbols are derived by [`super::eval`] and never stored.
//!
//! M1 forms are extrusions and prism voids between reference planes or by a depth; profiles are
//! closed loops whose vertices are anchored to reference planes.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use arc_base::{Error, ObjectId, Result};

use crate::error::codes;
use crate::expr::{self, DataType, Evaluated, Formula, Node, Stored};

/// Limits of one definition and of its text.
pub mod limits {
    /// Parameters (also the limit of variables per document).
    pub const PARAMETERS: usize = 1000;
    /// Types.
    pub const TYPES: usize = 5000;
    /// Reference planes.
    pub const PLANES: usize = 256;
    /// Profiles.
    pub const PROFILES: usize = 256;
    /// Vertices of one loop.
    pub const VERTICES: usize = 512;
    /// Forms.
    pub const FORMS: usize = 512;
    /// Symbolic elements.
    pub const SYMBOLS: usize = 512;
    /// Test cases.
    pub const TESTS: usize = 256;
    /// Tables.
    pub const TABLES: usize = 64;
    /// Arrays and nested uses.
    pub const ARRAYS: usize = 256;
    /// Nested uses.
    pub const NESTED: usize = 256;
    /// Nesting depth of nested uses.
    pub const NESTING_DEPTH: usize = 8;
    /// Placed children (nested uses and array copies) in one instance.
    pub const PLACED_CHILDREN: usize = 10_000;
    /// Values one test case or type sets.
    pub const VALUES: usize = 1000;
    /// The bytes of a canonical text.
    pub const TEXT_BYTES: usize = 4 * 1024 * 1024;
    /// The lines of a canonical text.
    pub const LINES: usize = 100_000;
    /// The bytes of one line.
    pub const LINE_BYTES: usize = 4096;
    /// The bytes of one string value.
    pub const STRING_BYTES: usize = 2048;
}

coded! {
    /// The category of a family.
    Category {
        /// Furniture.
        #[default]
        Furniture = 0, "furniture";
        /// Casework.
        Casework = 1, "casework";
        /// Plumbing fixtures.
        PlumbingFixture = 2, "plumbing_fixture";
        /// Lighting fixtures.
        LightingFixture = 3, "lighting_fixture";
        /// Electrical equipment.
        ElectricalEquipment = 4, "electrical_equipment";
        /// Mechanical equipment.
        MechanicalEquipment = 5, "mechanical_equipment";
        /// Doors.
        Door = 6, "door";
        /// Windows.
        Window = 7, "window";
        /// Architectural columns.
        Column = 8, "column";
        /// Generic models.
        GenericModel = 9, "generic_model";
        /// Specialty equipment.
        SpecialtyEquipment = 10, "specialty_equipment";
        /// A profile family: closed or open 2D profiles used by edges, bands and mullions; it
        /// has no body of its own and is never placed.
        Profile = 11, "profile";
    }
}

coded! {
    /// How an array lays out its copies.
    ArrayKind {
        /// Along an axis at a spacing.
        #[default]
        Linear = 0, "linear";
        /// About the vertical axis through a point, over an angle.
        Radial = 1, "radial";
    }
}

coded! {
    /// What hosts a family.
    Hosting {
        /// Freestanding.
        #[default]
        None = 0, "none";
        /// A wall.
        Wall = 1, "wall";
        /// A floor.
        Floor = 2, "floor";
        /// A ceiling.
        Ceiling = 3, "ceiling";
        /// A roof.
        Roof = 4, "roof";
        /// A face.
        Face = 5, "face";
        /// A work plane.
        WorkPlane = 6, "work_plane";
    }
}

coded! {
    /// Where a parameter's value is set.
    Scope {
        /// On the type.
        #[default]
        Type = 0, "type";
        /// On each instance.
        Instance = 1, "instance";
    }
}

coded! {
    /// Whether a parameter drives geometry or reports a value measured from it.
    Role {
        /// Drives geometry; evaluated before it.
        #[default]
        Driving = 0, "driving";
        /// Reports a measure of the evaluated geometry (`solid_volume`, `size_x`, `size_y`,
        /// `size_z`); read-only.
        Reporting = 1, "reporting";
    }
}

coded! {
    /// An axis of the family's frame; a reference plane is perpendicular to its axis.
    Axis {
        /// Left to right.
        #[default]
        X = 0, "x";
        /// Front to back.
        Y = 1, "y";
        /// Bottom to top.
        Z = 2, "z";
    }
}

coded! {
    /// The orientation role of a reference plane.
    PlaneRole {
        /// No role.
        #[default]
        None = 0, "none";
        /// Left.
        Left = 1, "left";
        /// Right.
        Right = 2, "right";
        /// Front.
        Front = 3, "front";
        /// Back.
        Back = 4, "back";
        /// Top.
        Top = 5, "top";
        /// Bottom.
        Bottom = 6, "bottom";
        /// Center.
        Center = 7, "center";
    }
}

coded! {
    /// The kind of a form.
    FormKind {
        /// Adds material.
        #[default]
        Solid = 0, "solid";
        /// Subtracts from the family's solids.
        Void = 1, "void";
    }
}

coded! {
    /// The view orientation of visibility and symbols.
    ViewKind {
        /// Plan.
        #[default]
        Plan = 0, "plan";
        /// Front.
        Front = 1, "front";
        /// Back.
        Back = 2, "back";
        /// Left.
        Left = 3, "left";
        /// Right.
        Right = 4, "right";
        /// Three-dimensional views.
        ThreeD = 5, "3d";
    }
}

coded! {
    /// A detail level.
    Detail {
        /// Low.
        #[default]
        Low = 0, "low";
        /// Medium.
        Medium = 1, "medium";
        /// High.
        High = 2, "high";
    }
}

coded! {
    /// The kind of a symbolic element.
    SymbolKind {
        /// A line (open polyline with arcs).
        #[default]
        Line = 0, "line";
        /// A filled region.
        Region = 1, "region";
        /// A masking region.
        Mask = 2, "mask";
    }
}

macro_rules! parse_name {
    ($($t:ident),*) => { $(impl $t {
        /// The variant named `name`.
        #[must_use]
        pub fn parse(name: &str) -> Option<Self> {
            Self::ALL.into_iter().find(|v| v.name() == name)
        }
    })* };
}
parse_name!(
    Category, Hosting, Scope, Role, Axis, PlaneRole, FormKind, ViewKind, Detail, SymbolKind,
    ArrayKind
);

impl Axis {
    /// The in-plane axes of a plane perpendicular to this one: the profile frame `(u, v)` with
    /// `u x v` along this axis.
    #[must_use]
    pub const fn plane_axes(self) -> (Self, Self) {
        match self {
            Self::X => (Self::Y, Self::Z),
            Self::Y => (Self::Z, Self::X),
            Self::Z => (Self::X, Self::Y),
        }
    }
}

impl ViewKind {
    /// The model axes a symbol of this view is drawn on, `(u, v)`; `None` for 3D.
    #[must_use]
    pub const fn axes(self) -> Option<(Axis, Axis)> {
        match self {
            Self::Plan => Some((Axis::X, Axis::Y)),
            Self::Front | Self::Back => Some((Axis::X, Axis::Z)),
            Self::Left | Self::Right => Some((Axis::Y, Axis::Z)),
            Self::ThreeD => None,
        }
    }
}

/// The behavior flags a header may carry (AB-FAM-021).
pub const FLAGS: [&str; 4] = [
    "always_vertical",
    "room_calculation_point",
    "shared",
    "work_plane_based",
];

/// A version `major.minor.patch`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Version {
    /// Major: parameters removed, retyped, or hosting changed.
    pub major: u32,
    /// Minor: parameters added.
    pub minor: u32,
    /// Patch: geometry or metadata only.
    pub patch: u32,
}

impl Version {
    /// A version from `major.minor.patch`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.split('.');
        let (a, b, c) = (parts.next()?, parts.next()?, parts.next()?);
        if parts.next().is_some() {
            return None;
        }
        let n = |s: &str| {
            (!s.is_empty() && s.len() <= 9 && s.bytes().all(|b| b.is_ascii_digit()))
                .then(|| s.parse().ok())
                .flatten()
        };
        Some(Self {
            major: n(a)?,
            minor: n(b)?,
            patch: n(c)?,
        })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// The header of a definition.
#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    /// The stable identifier.
    pub id: ObjectId,
    /// The version.
    pub version: Version,
    /// The name.
    pub name: String,
    /// The category.
    pub category: Category,
    /// The hosting kind.
    pub hosting: Hosting,
    /// Behavior flags, from [`FLAGS`].
    pub flags: BTreeSet<String>,
    /// The IFC entity (empty: the default of the category).
    pub ifc_entity: String,
    /// The IFC predefined type.
    pub ifc_type: String,
    /// The version of the function set the definition needs.
    pub functions: (u32, u32),
    /// Where it came from.
    pub source: String,
    /// The SPDX license.
    pub license: String,
}

/// A parameter of the definition.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamDef {
    /// The stable identifier; mapping on update is by it.
    pub id: ObjectId,
    /// The formula identifier, unique among the parameters.
    pub key: String,
    /// The display name.
    pub name: String,
    /// The data type.
    pub data: DataType,
    /// Type or instance.
    pub scope: Scope,
    /// Driving or reporting.
    pub role: Role,
    /// The group shown in.
    pub group: String,
    /// The order within the group.
    pub order: i64,
    /// The default value (a reporting parameter holds its formula here).
    pub default: Option<Stored>,
    /// The least number allowed, in SI units.
    pub minimum: Option<f64>,
    /// The greatest number allowed.
    pub maximum: Option<f64>,
    /// The texts an enumeration allows.
    pub allowed: Vec<String>,
}

/// A type: a named set of values.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeDef {
    /// The stable identifier.
    pub id: ObjectId,
    /// The name, unique among the types.
    pub name: String,
    /// The values, by parameter key.
    pub values: BTreeMap<String, Stored>,
}

/// A named reference plane perpendicular to `axis` at an offset from the origin plane.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaneDef {
    /// The key (an identifier); `origin_x`, `origin_y` and `origin_z` are the origin planes.
    pub key: String,
    /// The display name.
    pub name: String,
    /// The axis the plane is perpendicular to.
    pub axis: Axis,
    /// The offset, a length: literal or formula.
    pub offset: Stored,
    /// The orientation role.
    pub role: PlaneRole,
    /// Whether the plane is a strong reference.
    pub strong: bool,
}

/// A vertex anchored to reference planes: coordinate `u` is the offset of plane `u` (the origin
/// without one) plus `du`, likewise `v`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Vertex {
    /// The plane of the first coordinate.
    pub u: Option<String>,
    /// The offset from it.
    pub du: Option<Stored>,
    /// The plane of the second coordinate.
    pub v: Option<String>,
    /// The offset from it.
    pub dv: Option<Stored>,
    /// The bulge of the edge starting here (a number formula or literal; 0 is straight).
    pub bulge: Option<Stored>,
}

/// A loop of vertices in a plane perpendicular to `axis`; closed unless `open`.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileDef {
    /// The key.
    pub key: String,
    /// The axis the profile's plane is perpendicular to.
    pub axis: Axis,
    /// An open polyline (for edges, bands and railings) that no form extrudes.
    pub open: bool,
    /// The vertices, at least three.
    pub vertices: Vec<Vertex>,
}

/// Where and when a form or symbol shows.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Visibility {
    /// The detail levels; empty means all.
    pub details: Vec<Detail>,
    /// The view orientations; empty means all.
    pub views: Vec<ViewKind>,
    /// A boolean formula; absent means always.
    pub when: Option<Formula>,
}

/// A form: an extrusion of a profile.
#[derive(Debug, Clone, PartialEq)]
pub struct FormDef {
    /// The key.
    pub key: String,
    /// Solid or void.
    pub kind: FormKind,
    /// The profile.
    pub profile: String,
    /// The reference plane the profile lies on.
    pub from: String,
    /// The reference plane the extrusion ends on.
    pub to: Option<String>,
    /// Or the signed depth along the axis.
    pub depth: Option<Stored>,
    /// The material slot.
    pub material: String,
    /// The subcategory.
    pub subcategory: String,
    /// Visibility.
    pub visibility: Visibility,
    /// A void that cuts the host element (a hosted family) instead of the family's own solids.
    pub cut_host: bool,
}

/// A position along one axis: a reference plane (the origin without one) plus an offset.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Anchor {
    /// The reference plane.
    pub plane: Option<String>,
    /// The offset from it, a length.
    pub offset: Option<Stored>,
}

/// A copy pattern of a form or a nested use (AB-FAM-017). Copy 0 is the original; copy `i` of a
/// linear array is moved by `i` times the spacing along the axis, copy `i` of a radial array is
/// turned by `i` times `angle / count` about the vertical axis through the center.
#[derive(Debug, Clone, PartialEq)]
pub struct ArrayDef {
    /// The key.
    pub key: String,
    /// The key of the form or nested use that is repeated.
    pub target: String,
    /// Linear or radial.
    pub kind: ArrayKind,
    /// The number of copies including the original: an integer, literal or formula.
    pub count: Stored,
    /// Linear: the distance between copies, a length.
    pub spacing: Option<Stored>,
    /// Linear: the direction.
    pub axis: Axis,
    /// Radial: the angle the copies share (a full turn without one).
    pub angle: Option<Stored>,
    /// Radial: the center, `x` and `y` lengths.
    pub center: [Option<Stored>; 2],
}

/// A use of another family inside this one (AB-FAM-015, AB-FAM-016): a pinned child definition,
/// its type (fixed, or chosen by a family-type parameter), a placement on reference planes, the
/// associations of child parameters with formulas of this family, and whether the child is shared.
#[derive(Debug, Clone, PartialEq)]
pub struct NestedUse {
    /// The key.
    pub key: String,
    /// The identifier of the child family.
    pub family: ObjectId,
    /// The pinned version of the child.
    pub version: Version,
    /// The child's type by name; without one and without `type_param` the child's defaults.
    pub type_name: Option<String>,
    /// The key of a family-type parameter of this family whose text names the child's type.
    pub type_param: Option<String>,
    /// A shared child is its own element (selectable, scheduled); otherwise it is part of the
    /// parent's body.
    pub shared: bool,
    /// The position along x, y and z.
    pub at: [Anchor; 3],
    /// The turn about the vertical axis, an angle.
    pub rotation: Option<Stored>,
    /// Child parameters set from formulas of this family, by child parameter key.
    pub params: BTreeMap<String, Stored>,
}

/// A symbolic element of one view orientation.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolDef {
    /// The key.
    pub key: String,
    /// The view orientation (not 3D).
    pub view: ViewKind,
    /// Line, region or mask.
    pub kind: SymbolKind,
    /// The subcategory.
    pub subcategory: String,
    /// Visibility.
    pub visibility: Visibility,
    /// The vertices (two or more for a line, three or more for a region).
    pub vertices: Vec<Vertex>,
}

/// A named value set evaluated before publishing, with what it must give.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TestCase {
    /// The name.
    pub name: String,
    /// The type whose values it starts from.
    pub type_name: Option<String>,
    /// Values set over the type's, by parameter key.
    pub set: BTreeMap<String, Stored>,
    /// The volume the solids must have, in cubic meters.
    pub volume: Option<f64>,
    /// The tolerance of the volume.
    pub tolerance: Option<f64>,
    /// The number of solids.
    pub solids: Option<u32>,
    /// The key of the form or parameter the evaluation must fail on.
    pub fails: Option<String>,
}

/// A family definition.
#[derive(Debug, Clone, PartialEq)]
pub struct Definition {
    /// The header.
    pub header: Header,
    /// The parameters, by key.
    pub parameters: Vec<ParamDef>,
    /// The types.
    pub types: Vec<TypeDef>,
    /// The reference planes.
    pub planes: Vec<PlaneDef>,
    /// The profiles.
    pub profiles: Vec<ProfileDef>,
    /// The forms.
    pub forms: Vec<FormDef>,
    /// The symbolic elements.
    pub symbols: Vec<SymbolDef>,
    /// The embedded lookup tables.
    pub tables: Vec<super::table::TableDef>,
    /// The arrays.
    pub arrays: Vec<ArrayDef>,
    /// The nested uses.
    pub nested: Vec<NestedUse>,
    /// The test cases.
    pub tests: Vec<TestCase>,
}

fn invalid(part: &str, reason: &str) -> Error {
    Error::new(codes::FAMILY_INVALID)
        .with_arg("part", part)
        .with_arg("reason", reason)
}

fn limit(name: &str) -> Error {
    Error::new(codes::FAMILY_LIMIT).with_arg("limit", name)
}

/// The origin plane key of `axis`.
#[must_use]
pub const fn origin_key(axis: Axis) -> &'static str {
    match axis {
        Axis::X => "origin_x",
        Axis::Y => "origin_y",
        Axis::Z => "origin_z",
    }
}

impl Definition {
    /// The parameter with the key `key`.
    #[must_use]
    pub fn parameter(&self, key: &str) -> Option<&ParamDef> {
        self.parameters.iter().find(|p| p.key == key)
    }

    /// The type named `name`.
    #[must_use]
    pub fn type_named(&self, name: &str) -> Option<&TypeDef> {
        self.types.iter().find(|t| t.name == name)
    }

    /// The axis of the plane `key` (origin planes included).
    #[must_use]
    pub fn plane_axis(&self, key: &str) -> Option<Axis> {
        Axis::ALL
            .into_iter()
            .find(|a| origin_key(*a) == key)
            .or_else(|| self.planes.iter().find(|p| p.key == key).map(|p| p.axis))
    }

    /// Checks the definition: counts within the limits, keys unique identifiers, every
    /// reference resolved, planes of the right axes, forms complete, values of their types,
    /// no cycle among parameters, no driving value reading a reporting parameter and no type
    /// value reading an instance parameter.
    ///
    /// # Errors
    /// `ab.family.limit_exceeded` naming the limit; `ab.family.invalid` naming the part and
    /// the reason; `ab.model.param_cycle`; `ab.family.functions_newer` for a function set newer
    /// than this build's.
    pub fn validate(&self) -> Result<()> {
        self.check_counts()?;
        if self.header.functions > expr::FUNCTION_SET_VERSION {
            return Err(Error::new(codes::FAMILY_FUNCTIONS).with_arg(
                "version",
                format!("{}.{}", self.header.functions.0, self.header.functions.1),
            ));
        }
        if self.header.name.trim().is_empty() {
            return Err(invalid("definition", "name"));
        }
        if self
            .header
            .flags
            .iter()
            .any(|f| !FLAGS.contains(&f.as_str()))
        {
            return Err(invalid("definition", "flag"));
        }
        self.check_parameters()?;
        self.check_types()?;
        self.check_geometry()?;
        self.check_tables()?;
        self.check_nested()?;
        self.check_arrays()?;
        self.check_tests()?;
        self.check_reads()?;
        self.check_functions()
    }

    fn check_counts(&self) -> Result<()> {
        let over = |n: usize, max: usize, name: &str| (n > max).then(|| limit(name));
        let sizes = [
            over(self.parameters.len(), limits::PARAMETERS, "parameters"),
            over(self.types.len(), limits::TYPES, "types"),
            over(self.planes.len(), limits::PLANES, "planes"),
            over(self.profiles.len(), limits::PROFILES, "profiles"),
            over(self.forms.len(), limits::FORMS, "forms"),
            over(self.symbols.len(), limits::SYMBOLS, "symbols"),
            over(self.tests.len(), limits::TESTS, "tests"),
            over(self.tables.len(), limits::TABLES, "tables"),
            over(self.arrays.len(), limits::ARRAYS, "arrays"),
            over(self.nested.len(), limits::NESTED, "nested"),
        ];
        if let Some(e) = sizes.into_iter().flatten().next() {
            return Err(e);
        }
        for v in self
            .profiles
            .iter()
            .map(|p| p.vertices.len())
            .chain(self.symbols.iter().map(|s| s.vertices.len()))
        {
            if v > limits::VERTICES {
                return Err(limit("vertices"));
            }
        }
        for n in self
            .types
            .iter()
            .map(|t| t.values.len())
            .chain(self.tests.iter().map(|t| t.set.len()))
        {
            if n > limits::VALUES {
                return Err(limit("values"));
            }
        }
        Ok(())
    }

    fn unique<'a>(part: &str, keys: impl Iterator<Item = &'a str>, ident: bool) -> Result<()> {
        let mut seen = BTreeSet::new();
        for key in keys {
            if ident {
                expr::check_identifier(key).map_err(|_| invalid(part, "key"))?;
            } else if key.trim().is_empty() {
                return Err(invalid(part, "name"));
            }
            if !seen.insert(key) {
                return Err(invalid(part, "duplicate"));
            }
        }
        Ok(())
    }

    fn check_stored(&self, part: &str, value: &Stored, data: DataType) -> Result<()> {
        match value {
            Stored::Literal(v) if !v.fits(data) => Err(invalid(part, "value")),
            Stored::Formula(_) if !data.takes_formula() => Err(invalid(part, "value")),
            _ => Ok(()),
        }
    }

    fn check_parameters(&self) -> Result<()> {
        Self::unique(
            "parameter",
            self.parameters.iter().map(|p| p.key.as_str()),
            true,
        )?;
        let mut ids = BTreeSet::new();
        for p in &self.parameters {
            if !ids.insert(p.id) {
                return Err(invalid("parameter", "duplicate_id"));
            }
            if p.name.trim().is_empty() {
                return Err(invalid("parameter", "name"));
            }
            if let Some(d) = &p.default {
                self.check_stored("parameter", d, p.data)?;
            }
            if p.allowed
                .iter()
                .any(|a| a.contains('|') || a.len() > limits::STRING_BYTES)
            {
                return Err(invalid("parameter", "allowed"));
            }
            if p.role == Role::Reporting
                && (p.scope != Scope::Instance || p.data.dimension().is_none())
            {
                return Err(invalid("parameter", "reporting"));
            }
        }
        // Cycles, over every parameter's default.
        let nodes: Vec<Node<'_>> = self
            .parameters
            .iter()
            .map(|p| Node {
                key: p.id,
                identifier: &p.key,
                name: &p.name,
                data: p.data,
                value: p.default.as_ref(),
            })
            .collect();
        expr::order(&nodes).map(|_| ())
    }

    fn check_types(&self) -> Result<()> {
        Self::unique("type", self.types.iter().map(|t| t.name.as_str()), false)?;
        for t in &self.types {
            for (key, value) in &t.values {
                let p = self
                    .parameter(key)
                    .ok_or_else(|| invalid("type", "parameter"))?;
                if p.role == Role::Reporting {
                    return Err(invalid("type", "reporting"));
                }
                self.check_stored("type", value, p.data)?;
            }
        }
        Ok(())
    }

    fn plane_ok(&self, part: &str, key: Option<&String>, axis: Axis) -> Result<()> {
        match key {
            None => Ok(()),
            Some(k) if self.plane_axis(k) == Some(axis) => Ok(()),
            Some(_) => Err(invalid(part, "plane")),
        }
    }

    fn check_vertices(&self, part: &str, vs: &[Vertex], axes: (Axis, Axis)) -> Result<()> {
        for v in vs {
            self.plane_ok(part, v.u.as_ref(), axes.0)?;
            self.plane_ok(part, v.v.as_ref(), axes.1)?;
            for s in [&v.du, &v.dv].into_iter().flatten() {
                self.check_stored(part, s, DataType::Length)?;
            }
            if let Some(b) = &v.bulge {
                self.check_stored(part, b, DataType::Number)?;
            }
        }
        Ok(())
    }

    fn check_geometry(&self) -> Result<()> {
        Self::unique("plane", self.planes.iter().map(|p| p.key.as_str()), true)?;
        if self
            .planes
            .iter()
            .any(|p| Axis::ALL.into_iter().any(|a| origin_key(a) == p.key))
        {
            return Err(invalid("plane", "reserved"));
        }
        for p in &self.planes {
            self.check_stored("plane", &p.offset, DataType::Length)?;
        }
        Self::unique(
            "profile",
            self.profiles.iter().map(|p| p.key.as_str()),
            true,
        )?;
        for p in &self.profiles {
            if p.vertices.len() < if p.open { 2 } else { 3 } {
                return Err(invalid("profile", "vertices"));
            }
            self.check_vertices("profile", &p.vertices, p.axis.plane_axes())?;
        }
        Self::unique("form", self.forms.iter().map(|f| f.key.as_str()), true)?;
        for f in &self.forms {
            let profile = self
                .profiles
                .iter()
                .find(|p| p.key == f.profile)
                .ok_or_else(|| invalid("form", "profile"))?;
            if profile.open {
                return Err(invalid("form", "open_profile"));
            }
            self.plane_ok("form", Some(&f.from), profile.axis)?;
            match (&f.to, &f.depth) {
                (Some(to), None) => {
                    self.plane_ok("form", Some(to), profile.axis)?;
                    if *to == f.from {
                        return Err(invalid("form", "extent"));
                    }
                }
                (None, Some(d)) => self.check_stored("form", d, DataType::Length)?,
                _ => return Err(invalid("form", "extent")),
            }
            self.check_visibility("form", &f.visibility)?;
            if f.cut_host && (f.kind != FormKind::Void || self.header.hosting == Hosting::None) {
                return Err(invalid("form", "cut_host"));
            }
        }
        Self::unique("symbol", self.symbols.iter().map(|s| s.key.as_str()), true)?;
        for s in &self.symbols {
            let axes = s.view.axes().ok_or_else(|| invalid("symbol", "view"))?;
            let least = if s.kind == SymbolKind::Line { 2 } else { 3 };
            if s.vertices.len() < least {
                return Err(invalid("symbol", "vertices"));
            }
            self.check_vertices("symbol", &s.vertices, axes)?;
            self.check_visibility("symbol", &s.visibility)?;
        }
        Ok(())
    }

    fn check_tables(&self) -> Result<()> {
        Self::unique("table", self.tables.iter().map(|t| t.name.as_str()), true)?;
        self.tables
            .iter()
            .try_for_each(super::table::TableDef::validate)
    }

    fn check_nested(&self) -> Result<()> {
        Self::unique("nested", self.nested.iter().map(|n| n.key.as_str()), true)?;
        for n in &self.nested {
            if n.family == self.header.id {
                return Err(invalid("nested", "cycle"));
            }
            if n.type_name.is_some() && n.type_param.is_some() {
                return Err(invalid("nested", "type"));
            }
            if let Some(k) = &n.type_param
                && self.parameter(k).map(|p| p.data) != Some(DataType::FamilyType)
            {
                return Err(invalid("nested", "type_param"));
            }
            for (axis, a) in Axis::ALL.into_iter().zip(&n.at) {
                self.plane_ok("nested", a.plane.as_ref(), axis)?;
                if let Some(o) = &a.offset {
                    self.check_stored("nested", o, DataType::Length)?;
                }
            }
            if let Some(r) = &n.rotation {
                self.check_stored("nested", r, DataType::Angle)?;
            }
            for k in n.params.keys() {
                expr::check_identifier(k).map_err(|_| invalid("nested", "param"))?;
            }
        }
        Ok(())
    }

    fn check_arrays(&self) -> Result<()> {
        Self::unique("array", self.arrays.iter().map(|a| a.key.as_str()), true)?;
        let mut targets = BTreeSet::new();
        for a in &self.arrays {
            let form = self.forms.iter().find(|f| f.key == a.target);
            if form.is_none() && !self.nested.iter().any(|n| n.key == a.target) {
                return Err(invalid("array", "target"));
            }
            if !targets.insert(a.target.as_str()) {
                return Err(invalid("array", "duplicate_target"));
            }
            self.check_stored("array", &a.count, DataType::Integer)?;
            if let Stored::Literal(Evaluated::Number(n)) = &a.count
                && !(1.0..=(limits::PLACED_CHILDREN as f64)).contains(n)
            {
                return Err(limit("array_count"));
            }
            match a.kind {
                ArrayKind::Linear => {
                    let spacing = a
                        .spacing
                        .as_ref()
                        .ok_or_else(|| invalid("array", "spacing"))?;
                    self.check_stored("array", spacing, DataType::Length)?;
                }
                ArrayKind::Radial => {
                    if form.is_some_and(|f| f.kind == FormKind::Void) {
                        return Err(invalid("array", "void_radial"));
                    }
                    if let Some(an) = &a.angle {
                        self.check_stored("array", an, DataType::Angle)?;
                    }
                    for c in a.center.iter().flatten() {
                        self.check_stored("array", c, DataType::Length)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// A definition that calls `lookup` declares function set 1.1 or later.
    fn check_functions(&self) -> Result<()> {
        let uses = self
            .driving_formulas()
            .iter()
            .filter_map(|(_, s)| s.formula())
            .any(|f| f.calls().contains("lookup"));
        if uses && self.header.functions < expr::LOOKUP_SINCE {
            return Err(invalid("definition", "functions"));
        }
        Ok(())
    }

    fn check_visibility(&self, part: &str, v: &Visibility) -> Result<()> {
        let unique = |n: usize, total: usize| n == total;
        let d: BTreeSet<_> = v.details.iter().map(|d| d.code()).collect();
        let w: BTreeSet<_> = v.views.iter().map(|d| d.code()).collect();
        if !unique(d.len(), v.details.len()) || !unique(w.len(), v.views.len()) {
            return Err(invalid(part, "visibility"));
        }
        Ok(())
    }

    fn check_tests(&self) -> Result<()> {
        Self::unique("test", self.tests.iter().map(|t| t.name.as_str()), false)?;
        for t in &self.tests {
            if let Some(n) = &t.type_name
                && self.type_named(n).is_none()
            {
                return Err(invalid("test", "type"));
            }
            for (key, value) in &t.set {
                let p = self
                    .parameter(key)
                    .ok_or_else(|| invalid("test", "parameter"))?;
                self.check_stored("test", value, p.data)?;
            }
            for f in [t.volume, t.tolerance].into_iter().flatten() {
                if !f.is_finite() {
                    return Err(invalid("test", "expect"));
                }
            }
        }
        Ok(())
    }

    /// The formulas that drive geometry, with where they are (for messages).
    fn driving_formulas(&self) -> Vec<(&'static str, &Stored)> {
        let mut out: Vec<(&'static str, &Stored)> = Vec::new();
        let vertex = |part: &'static str, v: &'static Vertex| {
            let _ = (part, v);
        };
        let _ = vertex;
        for p in self.parameters.iter().filter(|p| p.role == Role::Driving) {
            out.extend(p.default.as_ref().map(|s| ("parameter", s)));
        }
        out.extend(self.planes.iter().map(|p| ("plane", &p.offset)));
        for v in self
            .profiles
            .iter()
            .flat_map(|p| p.vertices.iter())
            .chain(self.symbols.iter().flat_map(|s| s.vertices.iter()))
        {
            out.extend(
                [&v.du, &v.dv, &v.bulge]
                    .into_iter()
                    .flatten()
                    .map(|s| ("vertex", s)),
            );
        }
        out.extend(
            self.forms
                .iter()
                .filter_map(|f| f.depth.as_ref().map(|d| ("form", d))),
        );
        out.extend(
            self.types
                .iter()
                .flat_map(|t| t.values.values().map(|s| ("type", s))),
        );
        for a in &self.arrays {
            out.extend(
                [Some(&a.count), a.spacing.as_ref(), a.angle.as_ref()]
                    .into_iter()
                    .flatten()
                    .chain(a.center.iter().flatten())
                    .map(|s| ("array", s)),
            );
        }
        for n in &self.nested {
            out.extend(
                n.at.iter()
                    .filter_map(|a| a.offset.as_ref())
                    .chain(n.rotation.as_ref())
                    .chain(n.params.values())
                    .map(|s| ("nested", s)),
            );
        }
        out
    }

    fn check_reads(&self) -> Result<()> {
        let reporting: BTreeSet<&str> = self
            .parameters
            .iter()
            .filter(|p| p.role == Role::Reporting)
            .map(|p| p.key.as_str())
            .collect();
        let instance: BTreeSet<&str> = self
            .parameters
            .iter()
            .filter(|p| p.scope == Scope::Instance)
            .map(|p| p.key.as_str())
            .collect();
        for (part, s) in self.driving_formulas() {
            if s.names().iter().any(|n| reporting.contains(n.as_str())) {
                return Err(invalid(part, "reads_reporting"));
            }
        }
        for f in self
            .forms
            .iter()
            .filter_map(|f| f.visibility.when.as_ref())
            .chain(
                self.symbols
                    .iter()
                    .filter_map(|s| s.visibility.when.as_ref()),
            )
        {
            if f.names().iter().any(|n| reporting.contains(n.as_str())) {
                return Err(invalid("visibility", "reads_reporting"));
            }
        }
        // A type formula reads type parameters, never instance ones.
        for p in self.parameters.iter().filter(|p| p.scope == Scope::Type) {
            if p.default
                .iter()
                .any(|s| s.names().iter().any(|n| instance.contains(n.as_str())))
            {
                return Err(invalid("parameter", "type_reads_instance"));
            }
        }
        for t in &self.types {
            for (key, s) in &t.values {
                let scope = self.parameter(key).map(|p| p.scope);
                if scope == Some(Scope::Type)
                    && s.names().iter().any(|n| instance.contains(n.as_str()))
                {
                    return Err(invalid("type", "type_reads_instance"));
                }
            }
        }
        Ok(())
    }
}
