//! The family evaluator (ADR 0009, section 3): a definition, a type and instance values become
//! parameter values, reference plane positions, closed solids (extrusions with prism voids
//! subtracted), symbolic geometry and reporting values, all in the family's own frame and
//! derived: nothing here is stored.
//!
//! Evaluation order: type-scope parameters, then instance-scope parameters, reference planes,
//! forms (solids, then voids), visibility, symbols, and last the reporting parameters, which read
//! `solid_volume`, `size_x`, `size_y` and `size_z` of the evaluated solids. A failure is an
//! error naming the form, parameter or plane that failed.
//!
//! shortcut: the budget of one evaluation counts the bytes of the formulas evaluated, a bound on
//! their nodes, instead of the steps `arc-solve` takes; replace when `arc-solve` reports steps.

use std::collections::BTreeMap;
use std::sync::Arc;

use arc_base::math::{Matrix4, Point3, Transform3, Vector3};
use arc_base::{Error, Limits, ObjectId, Result, Tolerance};
use arc_geom::{FaceTag, PlaneFrame, Polyhedron, PrismTool, extrude};
use arc_solve::{Functions, Value as SolveValue};

use super::def::{
    ArrayDef, ArrayKind, Axis, Definition, Detail, FormKind, NestedUse, Role, Scope, SymbolKind,
    Version, Vertex, ViewKind, Visibility, limits as def_limits, origin_key,
};
use crate::error::codes;
use crate::expr::{self, DataType, Evaluated, Node, Stored};
use crate::floor::{Boundary, flatten_edge};

/// The most formula bytes one evaluation may run.
pub const BUDGET: usize = 1_000_000;
/// The chord tolerance arcs are faceted at, in meters.
pub const CHORD: f64 = 0.002;

/// What a form or symbol shows with: its levels, views and the result of its condition.
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    /// The detail levels; empty means all.
    pub details: Vec<Detail>,
    /// The view orientations; empty means all.
    pub views: Vec<ViewKind>,
    /// The condition evaluated; true without one.
    pub when: bool,
}

impl Shown {
    /// Whether it shows at `detail` in `view`.
    #[must_use]
    pub fn at(&self, detail: Detail, view: ViewKind) -> bool {
        self.when
            && (self.details.is_empty() || self.details.contains(&detail))
            && (self.views.is_empty() || self.views.contains(&view))
    }
}

/// One evaluated solid form.
#[derive(Debug, Clone)]
pub struct FormSolid {
    /// The form's key.
    pub form: String,
    /// The solid, in the family's frame, voids subtracted.
    pub solid: Polyhedron,
    /// The stable role of each face (`family.form[<key>].<face>`), in face order.
    pub roles: Vec<String>,
    /// The material slot.
    pub material: String,
    /// The subcategory.
    pub subcategory: String,
    /// Visibility.
    pub shown: Shown,
}

/// One evaluated symbolic element, in the coordinates of its view.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolOut {
    /// The key.
    pub key: String,
    /// The view orientation.
    pub view: ViewKind,
    /// Line, region or mask.
    pub kind: SymbolKind,
    /// The subcategory.
    pub subcategory: String,
    /// The points, arcs faceted; closed for regions and masks (the first point is not repeated).
    pub points: Vec<[f64; 2]>,
    /// Visibility.
    pub shown: Shown,
}

/// The result of evaluating a definition.
#[derive(Debug, Clone)]
pub struct Evaluation {
    /// Every parameter's value, by key.
    pub values: BTreeMap<String, Evaluated>,
    /// The position of each reference plane along its axis, by key (origin planes included).
    pub planes: BTreeMap<String, f64>,
    /// The solids.
    pub solids: Vec<FormSolid>,
    /// The symbolic elements.
    pub symbols: Vec<SymbolOut>,
    /// The total volume of the solids, in cubic meters.
    pub volume: f64,
    /// The size of the bounding box of the solids, `[x, y, z]`.
    pub size: [f64; 3],
    /// The shared nested children: derived elements of their own, in this family's frame.
    pub shared: Vec<SharedOut>,
    /// The evaluated profiles of a profile family (empty for other categories).
    pub profiles: Vec<ProfileOut>,
    /// The host-cutting voids of a hosted family: the box of each, in the family's frame.
    pub host_cuts: Vec<HostCut>,
}

/// A host-cutting void: its form key and its bounding box in the family's frame.
#[derive(Debug, Clone, PartialEq)]
pub struct HostCut {
    /// The form's key.
    pub form: String,
    /// The least corner.
    pub min: [f64; 3],
    /// The greatest corner.
    pub max: [f64; 3],
}

/// A shared nested child: its key (with `#index` for an array copy), the child family, the
/// evaluation of the child in its own frame and where the child lies in the parent's frame.
#[derive(Debug, Clone)]
pub struct SharedOut {
    /// The nested-use key, `key#i` for copy `i` of an array.
    pub key: String,
    /// The child family's identifier.
    pub family: ObjectId,
    /// The child's type name, when it has one.
    pub type_name: Option<String>,
    /// The child's evaluation (shared by the copies of an array).
    pub evaluation: Arc<Evaluation>,
    /// The child's frame in the parent's frame, row-major.
    pub matrix: [[f64; 4]; 4],
}

/// An evaluated profile of a profile family, in the plane's `(u, v)` coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileOut {
    /// The key.
    pub key: String,
    /// The axis the plane is perpendicular to.
    pub axis: Axis,
    /// Whether the polyline is open; a closed one does not repeat its first point.
    pub open: bool,
    /// The points, arcs faceted.
    pub points: Vec<[f64; 2]>,
}

/// What an evaluation reads besides the definition: the type, the instance's own values and the
/// document variables.
pub struct Inputs<'a> {
    /// The type by name; none uses the defaults.
    pub type_name: Option<&'a str>,
    /// The instance's own values, by parameter key.
    pub instance: &'a BTreeMap<String, Stored>,
    /// The document variables by formula identifier.
    pub variables: &'a dyn Fn(&str) -> Option<SolveValue>,
    /// The definition of a pinned child family and version, for nested uses.
    pub children: &'a dyn Fn(ObjectId, Version) -> Option<Arc<Definition>>,
}

/// What nested evaluations share: the budget, the placed children and the families being
/// evaluated (to refuse a cycle).
#[derive(Default)]
struct Shared {
    spent: usize,
    placed: usize,
    stack: Vec<ObjectId>,
}

fn form_failed(form: &str, reason: &str) -> Error {
    Error::new(codes::FAMILY_FORM)
        .with_arg("form", form)
        .with_arg("reason", reason)
}

fn failed_in(form: &str, e: &Error) -> Error {
    form_failed(form, e.code())
}

struct Run<'a> {
    names: BTreeMap<String, SolveValue>,
    variables: &'a dyn Fn(&str) -> Option<SolveValue>,
    functions: Functions,
    spent: usize,
}

impl Run<'_> {
    fn lookup(&self, name: &str) -> Option<SolveValue> {
        self.names
            .get(name)
            .cloned()
            .or_else(|| (self.variables)(name))
    }

    fn charge(&mut self, s: &Stored) -> Result<()> {
        if let Stored::Formula(f) = s {
            self.spent += f.text().len();
            if self.spent > BUDGET {
                return Err(Error::new(codes::FAMILY_LIMIT).with_arg("limit", "steps"));
            }
        }
        Ok(())
    }

    fn eval(&mut self, s: &Stored, data: DataType) -> Result<Evaluated> {
        self.charge(s)?;
        let lookup = |n: &str| -> Result<Option<SolveValue>> { Ok(self.lookup(n)) };
        s.evaluate_with(&lookup, data, Some(&self.functions))
    }

    fn length(&mut self, s: Option<&Stored>, what: &str, form: &str) -> Result<f64> {
        let Some(s) = s else { return Ok(0.0) };
        match self.eval(s, DataType::Length) {
            Ok(Evaluated::Number(v)) => Ok(v),
            Ok(_) => Err(form_failed(form, what)),
            Err(e) => Err(e.with_arg("form", form)),
        }
    }

    fn length_of(&mut self, s: Option<&Stored>, data: DataType, form: &str) -> Result<f64> {
        let Some(s) = s else { return Ok(0.0) };
        match self.eval(s, data) {
            Ok(Evaluated::Number(v)) if v.is_finite() => Ok(v),
            Ok(_) => Err(form_failed(form, "value")),
            Err(e) => Err(e.with_arg("form", form)),
        }
    }

    fn plane(&self, key: &str, planes: &BTreeMap<String, f64>) -> f64 {
        planes.get(key).copied().unwrap_or(0.0)
    }

    /// The points and bulges of a loop.
    fn loop_of(
        &mut self,
        vs: &[Vertex],
        planes: &BTreeMap<String, f64>,
        owner: &str,
    ) -> Result<(Vec<[f64; 2]>, Vec<f64>)> {
        let (mut points, mut bulges) = (Vec::new(), Vec::new());
        for v in vs {
            let u = v.u.as_deref().map_or(0.0, |k| self.plane(k, planes))
                + self.length(v.du.as_ref(), "vertex", owner)?;
            let w = v.v.as_deref().map_or(0.0, |k| self.plane(k, planes))
                + self.length(v.dv.as_ref(), "vertex", owner)?;
            let bulge = match &v.bulge {
                Some(b) => match self.eval(b, DataType::Number) {
                    Ok(Evaluated::Number(n)) => n,
                    Ok(_) => return Err(form_failed(owner, "bulge")),
                    Err(e) => return Err(e.with_arg("form", owner)),
                },
                None => 0.0,
            };
            if !(u.is_finite() && w.is_finite()) {
                return Err(form_failed(owner, "not_finite"));
            }
            points.push([u, w]);
            bulges.push(bulge);
        }
        Ok((points, bulges))
    }

    fn shown(&mut self, v: &Visibility, owner: &str) -> Result<Shown> {
        let when = match &v.when {
            None => true,
            Some(f) => {
                let s = Stored::Formula(f.clone());
                match self.eval(&s, DataType::Boolean) {
                    Ok(Evaluated::Bool(b)) => b,
                    Ok(_) => return Err(form_failed(owner, "when")),
                    Err(e) => return Err(e.with_arg("form", owner)),
                }
            }
        };
        Ok(Shown {
            details: v.details.clone(),
            views: v.views.clone(),
            when,
        })
    }
}

type Matrix = [[f64; 4]; 4];

const IDENTITY: Matrix = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

pub(super) fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
    let mut out = [[0.0; 4]; 4];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = (0..4).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    out
}

pub(super) fn to_transform(m: &Matrix) -> Transform3 {
    Transform3::from_matrix_unchecked(Matrix4::new(
        m[0][0], m[0][1], m[0][2], m[0][3], m[1][0], m[1][1], m[1][2], m[1][3], m[2][0], m[2][1],
        m[2][2], m[2][3], m[3][0], m[3][1], m[3][2], m[3][3],
    ))
}

/// The turn by `angle` about the vertical axis, then the move by `at`.
fn placement(angle: f64, at: [f64; 3]) -> Matrix {
    let (s, c) = angle.sin_cos();
    [
        [c, -s, 0.0, at[0]],
        [s, c, 0.0, at[1]],
        [0.0, 0.0, 1.0, at[2]],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// An array evaluated: how many copies and how copy `i` is placed.
struct Layout {
    kind: ArrayKind,
    axis: Axis,
    count: usize,
    spacing: f64,
    step: f64,
    center: [f64; 2],
}

impl Layout {
    fn matrix(&self, i: usize) -> Matrix {
        #[allow(clippy::cast_precision_loss, reason = "a count is at most 10,000")]
        let n = i as f64;
        match self.kind {
            ArrayKind::Linear => {
                let mut at = [0.0; 3];
                at[match self.axis {
                    Axis::X => 0,
                    Axis::Y => 1,
                    Axis::Z => 2,
                }] = n * self.spacing;
                placement(0.0, at)
            }
            ArrayKind::Radial => {
                // Turn about the center: move the center to the origin, turn, move back.
                let turn = placement(n * self.step, [0.0; 3]);
                let [cx, cy] = self.center;
                let (s, c) = (n * self.step).sin_cos();
                let at = [cx - (c * cx - s * cy), cy - (s * cx + c * cy), 0.0];
                let mut m = turn;
                m[0][3] = at[0];
                m[1][3] = at[1];
                m
            }
        }
    }
}

impl Run<'_> {
    /// The evaluated array `a` that repeats `owner`.
    fn layout(&mut self, a: &ArrayDef, owner: &str) -> Result<Layout> {
        let number = |run: &mut Self, s: &Stored, data| match run.eval(s, data) {
            Ok(Evaluated::Number(v)) if v.is_finite() => Ok(v),
            Ok(_) => Err(form_failed(owner, "array")),
            Err(e) => Err(e.with_arg("form", owner)),
        };
        let count = number(self, &a.count, DataType::Integer)?;
        if !(1.0..=to_f64(def_limits::PLACED_CHILDREN)).contains(&count) {
            return Err(Error::new(codes::FAMILY_LIMIT).with_arg("limit", "array_count"));
        }
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "checked above"
        )]
        let count = count as usize;
        let spacing = match &a.spacing {
            Some(s) => number(self, s, DataType::Length)?,
            None => 0.0,
        };
        let full = std::f64::consts::TAU;
        let angle = match &a.angle {
            Some(s) => number(self, s, DataType::Angle)?,
            None => full,
        };
        let mut center = [0.0; 2];
        for (c, s) in center.iter_mut().zip(&a.center) {
            if let Some(s) = s {
                *c = number(self, s, DataType::Length)?;
            }
        }
        Ok(Layout {
            kind: a.kind,
            axis: a.axis,
            count,
            spacing,
            step: angle / to_f64(count),
            center,
        })
    }
}

#[allow(clippy::cast_precision_loss, reason = "counts are small")]
fn to_f64(n: usize) -> f64 {
    n as f64
}

fn axis_vector(a: Axis) -> Vector3 {
    match a {
        Axis::X => Vector3::new(1.0, 0.0, 0.0),
        Axis::Y => Vector3::new(0.0, 1.0, 0.0),
        Axis::Z => Vector3::new(0.0, 0.0, 1.0),
    }
}

fn suffix_of(tag: FaceTag, forward: bool, voids: &[String], index: usize) -> String {
    match tag {
        FaceTag::Top => (if forward { "end" } else { "start" }).to_owned(),
        FaceTag::Bottom => (if forward { "start" } else { "end" }).to_owned(),
        FaceTag::Side {
            loop_index,
            curve,
            facet,
        } => format!("side.{loop_index}.{curve}.{facet}"),
        FaceTag::OpeningWall {
            opening,
            curve,
            facet,
        } => {
            let key = usize::try_from(opening)
                .ok()
                .and_then(|o| voids.get(o))
                .map_or("?", String::as_str);
            format!("void[{key}].{curve}.{facet}")
        }
        _ => format!("face.{index}"),
    }
}

/// Evaluates `def` (validated by the caller or by [`Definition::validate`]) for `inputs`.
///
/// # Errors
/// `ab.family.type_unknown`; the formula errors with the `parameter` or `form` argument;
/// `ab.family.form_failed` naming the form for a profile or extrusion the solid kernel refuses,
/// a void that does not pass through the solids it meets, or a result without material;
/// `ab.family.limit_exceeded`.
#[allow(
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    reason = "one pass over the parts of a definition, in order"
)]
pub fn evaluate(def: &Definition, inputs: &Inputs<'_>) -> Result<Evaluation> {
    let mut shared = Shared::default();
    shared.stack.push(def.header.id);
    evaluate_in(def, inputs, &mut shared)
}

#[allow(
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    reason = "one pass over the parts of a definition, in order"
)]
fn evaluate_in(def: &Definition, inputs: &Inputs<'_>, ctx: &mut Shared) -> Result<Evaluation> {
    let tolerance = Tolerance::DEFAULT;
    let limits = Limits::DEFAULT;
    let ty = match inputs.type_name {
        None => None,
        Some(n) => Some(
            def.type_named(n)
                .ok_or_else(|| Error::new(codes::FAMILY_TYPE_UNKNOWN).with_arg("type", n))?,
        ),
    };
    let type_value = |key: &str| ty.and_then(|t| t.values.get(key));
    let mut run = Run {
        names: BTreeMap::new(),
        variables: inputs.variables,
        functions: super::table::functions(&def.tables)?,
        spent: ctx.spent,
    };
    let mut values: BTreeMap<String, Evaluated> = BTreeMap::new();
    // Parameters: type scope reads variables only; instance scope also reads the type's.
    for scope in [Scope::Type, Scope::Instance] {
        let picked: Vec<(&super::def::ParamDef, Option<&Stored>)> = def
            .parameters
            .iter()
            .filter(|p| p.scope == scope && p.role == Role::Driving)
            .map(|p| {
                let own = (scope == Scope::Instance)
                    .then(|| inputs.instance.get(&p.key))
                    .flatten();
                (p, own.or_else(|| type_value(&p.key)).or(p.default.as_ref()))
            })
            .collect();
        for (_, s) in &picked {
            if let Some(s) = s {
                run.charge(s)?;
            }
        }
        let nodes: Vec<Node<'_>> = picked
            .iter()
            .map(|(p, s)| Node {
                key: p.id,
                identifier: &p.key,
                name: &p.name,
                data: p.data,
                value: *s,
            })
            .collect();
        let outer = |n: &str| run.names.get(n).cloned().or_else(|| (inputs.variables)(n));
        let results = expr::evaluate_scope_with(&nodes, &outer, Some(&run.functions))?;
        for ((p, _), v) in picked.iter().zip(results) {
            run.names.insert(p.key.clone(), v.to_solve(p.data));
            values.insert(p.key.clone(), v);
        }
    }
    // Rules of the values: bounds and allowed texts.
    for p in &def.parameters {
        if let Some(v) = values.get(&p.key) {
            let rule = crate::parameter::Rule {
                minimum: p.minimum,
                maximum: p.maximum,
                allowed: p.allowed.clone(),
                required: false,
                reject: true,
            };
            if let Some(rule) = rule.broken(Some(v)) {
                return Err(Error::new(codes::PARAM_RULE)
                    .with_arg("parameter", p.name.as_str())
                    .with_arg("object", def.header.name.as_str())
                    .with_arg("rule", rule));
            }
        }
    }
    // Reference planes.
    let mut planes: BTreeMap<String, f64> = Axis::ALL
        .into_iter()
        .map(|a| (origin_key(a).to_owned(), 0.0))
        .collect();
    for p in &def.planes {
        let v = match run.eval(&p.offset, DataType::Length) {
            Ok(Evaluated::Number(v)) if v.is_finite() => v,
            Ok(_) => return Err(Error::new(codes::FAMILY_PLANE).with_arg("plane", p.key.as_str())),
            Err(e) => return Err(e.with_arg("plane", p.key.as_str())),
        };
        planes.insert(p.key.clone(), v);
    }
    // Forms.
    struct Void {
        key: String,
        region: arc_geom::Region,
        frame: PlaneFrame,
        start: f64,
        end: f64,
        bounds: arc_geom::Aabb3,
    }
    let mut solids: Vec<(String, Polyhedron, bool, &super::def::FormDef)> = Vec::new();
    let mut voids: Vec<Void> = Vec::new();
    let mut host_cuts: Vec<HostCut> = Vec::new();
    let mut ordered: Vec<&super::def::FormDef> = def.forms.iter().collect();
    ordered.sort_by(|a, b| a.key.cmp(&b.key));
    for f in ordered {
        let Some(profile) = def.profiles.iter().find(|p| p.key == f.profile) else {
            return Err(form_failed(&f.key, "profile"));
        };
        let (points, bulges) = run.loop_of(&profile.vertices, &planes, &f.key)?;
        let region = Boundary {
            loops: vec![points],
            bulges: vec![bulges],
        }
        .region()
        .map_err(|e| failed_in(&f.key, &e))?;
        let from = run.plane(&f.from, &planes);
        let height = match (&f.to, &f.depth) {
            (Some(to), _) => run.plane(to, &planes) - from,
            (None, d) => run.length(d.as_ref(), "depth", &f.key)?,
        };
        let (u, v) = profile.axis.plane_axes();
        let origin = Point3::origin() + axis_vector(profile.axis) * from;
        let frame = PlaneFrame::new(origin, axis_vector(u), axis_vector(v), &tolerance)
            .map_err(|e| failed_in(&f.key, &e))?;
        let solid = extrude(&region, &frame, height, CHORD, &tolerance, &limits)
            .map_err(|e| failed_in(&f.key, &e))?;
        let layout = match def.arrays.iter().find(|a| a.target == f.key) {
            Some(a) => Some(run.layout(a, &f.key)?),
            None => None,
        };
        let count = layout.as_ref().map_or(1, |l| l.count);
        ctx.placed += count;
        if ctx.placed > def_limits::PLACED_CHILDREN {
            return Err(Error::new(codes::FAMILY_LIMIT).with_arg("limit", "placed_children"));
        }
        for i in 0..count {
            let m = layout.as_ref().map_or(IDENTITY, |l| l.matrix(i));
            let (key, copy) = if i == 0 {
                (f.key.clone(), solid.clone())
            } else {
                (
                    format!("{}#{i}", f.key),
                    solid
                        .transformed(&to_transform(&m), &tolerance, &limits)
                        .map_err(|e| failed_in(&f.key, &e))?,
                )
            };
            match f.kind {
                FormKind::Solid => solids.push((key, copy, height > 0.0, f)),
                FormKind::Void if f.cut_host => {
                    let b = copy.bounding_box();
                    host_cuts.push(HostCut {
                        form: key,
                        min: [b.min()[0], b.min()[1], b.min()[2]],
                        max: [b.max()[0], b.max()[1], b.max()[2]],
                    });
                }
                FormKind::Void => {
                    let at = frame.origin();
                    let moved = PlaneFrame::new(
                        Point3::new(at.x + m[0][3], at.y + m[1][3], at.z + m[2][3]),
                        frame.u(),
                        frame.v(),
                        &tolerance,
                    )
                    .map_err(|e| failed_in(&f.key, &e))?;
                    voids.push(Void {
                        key,
                        region: region.clone(),
                        frame: moved,
                        start: height.min(0.0),
                        end: height.max(0.0),
                        bounds: copy.bounding_box(),
                    });
                }
            }
        }
    }
    let void_keys: Vec<String> = voids.iter().map(|v| v.key.clone()).collect();
    let mut out_solids = Vec::new();
    for (key, mut solid, forward, form) in solids {
        for (n, v) in voids.iter().enumerate() {
            let (a, b) = (solid.bounding_box(), v.bounds);
            let overlaps = (0..3).all(|i| {
                a.min()[i] < b.max()[i] - tolerance.linear
                    && b.min()[i] < a.max()[i] - tolerance.linear
            });
            if !overlaps {
                continue;
            }
            let tool = PrismTool {
                profile: &v.region,
                frame: v.frame,
                start: v.start,
                end: v.end,
                chord: CHORD,
                opening: u32::try_from(n).unwrap_or(u32::MAX),
                allow_split: true,
            };
            solid = solid
                .subtract_prism(&tool, &tolerance, &limits)
                .map_err(|e| form_failed(&v.key, e.code()))?;
        }
        if solid.volume() <= tolerance.linear.powi(3) {
            return Err(form_failed(&key, "no_material"));
        }
        let roles = (0..solid.faces().len())
            .map(|i| {
                let tag = solid.face_tag(i).unwrap_or_default();
                format!(
                    "family.form[{key}].{}",
                    suffix_of(tag, forward, &void_keys, i)
                )
            })
            .collect();
        let shown = run.shown(&form.visibility, &key)?;
        out_solids.push(FormSolid {
            form: key,
            solid,
            roles,
            material: form.material.clone(),
            subcategory: form.subcategory.clone(),
            shown,
        });
    }
    // Nested uses: a non-shared child is part of this family's body, a shared child is kept
    // apart as an element of its own.
    let mut shared_out = Vec::new();
    let mut uses: Vec<&NestedUse> = def.nested.iter().collect();
    uses.sort_by(|a, b| a.key.cmp(&b.key));
    for n in uses {
        let layout = match def.arrays.iter().find(|a| a.target == n.key) {
            Some(a) => Some(run.layout(a, &n.key)?),
            None => None,
        };
        let count = layout.as_ref().map_or(1, |l| l.count);
        ctx.placed += count;
        if ctx.placed > def_limits::PLACED_CHILDREN {
            return Err(Error::new(codes::FAMILY_LIMIT).with_arg("limit", "placed_children"));
        }
        let child = (inputs.children)(n.family, n.version)
            .ok_or_else(|| form_failed(&n.key, "child_missing"))?;
        if ctx.stack.contains(&n.family) {
            return Err(Error::new(codes::FAMILY_INVALID)
                .with_arg("part", "nested")
                .with_arg("reason", "cycle"));
        }
        if ctx.stack.len() >= def_limits::NESTING_DEPTH {
            return Err(Error::new(codes::FAMILY_LIMIT).with_arg("limit", "nesting_depth"));
        }
        let type_name = match (&n.type_name, &n.type_param) {
            (Some(t), _) => Some(t.clone()),
            (None, Some(k)) => match values.get(k) {
                Some(Evaluated::Text(t)) if !t.is_empty() => Some(t.clone()),
                _ => return Err(form_failed(&n.key, "type")),
            },
            (None, None) => None,
        };
        let mut set = BTreeMap::new();
        for (key, stored) in &n.params {
            let p = child
                .parameter(key)
                .ok_or_else(|| form_failed(&n.key, "parameter"))?;
            let v = run
                .eval(stored, p.data)
                .map_err(|e| e.with_arg("form", n.key.as_str()))?;
            set.insert(key.clone(), Stored::Literal(v));
        }
        let (overlaid, instance, name) = child.overlaid(type_name.as_deref(), &set)?;
        ctx.spent = run.spent;
        ctx.stack.push(n.family);
        let evaluated = evaluate_in(
            &overlaid,
            &Inputs {
                type_name: name.as_deref(),
                instance: &instance,
                variables: inputs.variables,
                children: inputs.children,
            },
            ctx,
        );
        ctx.stack.pop();
        run.spent = ctx.spent;
        let evaluated = Arc::new(evaluated.map_err(|e| e.with_arg("form", n.key.as_str()))?);
        let at = |i: usize, run: &mut Run<'_>| -> Result<f64> {
            let a = &n.at[i];
            let base = a.plane.as_deref().map_or(0.0, |k| run.plane(k, &planes));
            Ok(base + run.length(a.offset.as_ref(), "nested", &n.key)?)
        };
        let rotation = run.length_of(n.rotation.as_ref(), DataType::Angle, &n.key)?;
        let base = placement(
            rotation,
            [at(0, &mut run)?, at(1, &mut run)?, at(2, &mut run)?],
        );
        for i in 0..count {
            let m = multiply(&layout.as_ref().map_or(IDENTITY, |l| l.matrix(i)), &base);
            let key = if layout.is_some() {
                format!("{}#{i}", n.key)
            } else {
                n.key.clone()
            };
            if n.shared {
                shared_out.push(SharedOut {
                    key,
                    family: n.family,
                    type_name: type_name.clone(),
                    evaluation: Arc::clone(&evaluated),
                    matrix: m,
                });
                continue;
            }
            let transform = to_transform(&m);
            for part in &evaluated.solids {
                out_solids.push(FormSolid {
                    form: format!("{key}/{}", part.form),
                    solid: part
                        .solid
                        .transformed(&transform, &tolerance, &limits)
                        .map_err(|e| failed_in(&n.key, &e))?,
                    roles: part
                        .roles
                        .iter()
                        .map(|r| format!("family.nested[{key}].{r}"))
                        .collect(),
                    material: part.material.clone(),
                    subcategory: part.subcategory.clone(),
                    shown: part.shown.clone(),
                });
            }
        }
    }
    // Symbols.
    let mut symbols = Vec::new();
    let mut ordered: Vec<&super::def::SymbolDef> = def.symbols.iter().collect();
    ordered.sort_by(|a, b| a.key.cmp(&b.key));
    for s in ordered {
        let (points, bulges) = run.loop_of(&s.vertices, &planes, &s.key)?;
        let closed = s.kind != SymbolKind::Line;
        let mut flat = Vec::new();
        let n = points.len();
        for i in 0..n {
            let last = i + 1 == n && !closed;
            let next = points[(i + 1) % n];
            if last {
                flat.push(points[i]);
            } else {
                flat.extend(flatten_edge(points[i], next, bulges[i], CHORD));
            }
        }
        if !closed {
            // The last vertex ends the line; flatten_edge leaves out the end of each edge.
            if let Some(end) = points.last()
                && flat.last() != Some(end)
            {
                flat.push(*end);
            }
        }
        symbols.push(SymbolOut {
            key: s.key.clone(),
            view: s.view,
            kind: s.kind,
            subcategory: s.subcategory.clone(),
            points: flat,
            shown: run.shown(&s.visibility, &s.key)?,
        });
    }
    // Measures and reporting parameters.
    let volume: f64 = out_solids.iter().map(|s| s.solid.volume()).sum();
    let mut size = [0.0; 3];
    if !out_solids.is_empty() {
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for s in &out_solids {
            let b = s.solid.bounding_box();
            for i in 0..3 {
                lo[i] = lo[i].min(b.min()[i]);
                hi[i] = hi[i].max(b.max()[i]);
            }
        }
        for i in 0..3 {
            size[i] = hi[i] - lo[i];
        }
    }
    let measure = |name: &str| -> Option<SolveValue> {
        let q = |v: f64, d| SolveValue::Number(arc_base::Quantity::new(v, d));
        match name {
            "solid_volume" => Some(q(volume, arc_base::Dimension::VOLUME)),
            "size_x" => Some(q(size[0], arc_base::Dimension::LENGTH)),
            "size_y" => Some(q(size[1], arc_base::Dimension::LENGTH)),
            "size_z" => Some(q(size[2], arc_base::Dimension::LENGTH)),
            _ => None,
        }
    };
    let reporting: Vec<&super::def::ParamDef> = def
        .parameters
        .iter()
        .filter(|p| p.role == Role::Reporting)
        .collect();
    if !reporting.is_empty() {
        let nodes: Vec<Node<'_>> = reporting
            .iter()
            .map(|p| Node {
                key: p.id,
                identifier: &p.key,
                name: &p.name,
                data: p.data,
                value: p.default.as_ref(),
            })
            .collect();
        let outer = |n: &str| {
            measure(n)
                .or_else(|| run.names.get(n).cloned())
                .or_else(|| (inputs.variables)(n))
        };
        let results = expr::evaluate_scope_with(&nodes, &outer, Some(&run.functions))?;
        for (p, v) in reporting.iter().zip(results) {
            values.insert(p.key.clone(), v);
        }
    }
    let mut profiles = Vec::new();
    if def.header.category == super::def::Category::Profile {
        for p in &def.profiles {
            let (points, bulges) = run.loop_of(&p.vertices, &planes, &p.key)?;
            let n = points.len();
            let mut flat = Vec::new();
            for i in 0..n {
                if p.open && i + 1 == n {
                    flat.push(points[i]);
                } else {
                    flat.extend(flatten_edge(
                        points[i],
                        points[(i + 1) % n],
                        bulges[i],
                        CHORD,
                    ));
                }
            }
            profiles.push(ProfileOut {
                key: p.key.clone(),
                axis: p.axis,
                open: p.open,
                points: flat,
            });
        }
    }
    ctx.spent = run.spent;
    Ok(Evaluation {
        values,
        planes,
        solids: out_solids,
        symbols,
        volume,
        size,
        shared: shared_out,
        profiles,
        host_cuts,
    })
}

impl Definition {
    /// Evaluates the definition for a type and an instance's values, without document variables.
    ///
    /// # Errors
    /// As [`evaluate`].
    pub fn evaluate(
        &self,
        type_name: Option<&str>,
        instance: &BTreeMap<String, Stored>,
    ) -> Result<Evaluation> {
        evaluate(
            self,
            &Inputs {
                type_name,
                instance,
                variables: &|_| None,
                children: &|_, _| None,
            },
        )
    }

    /// Evaluates for a type with `set` laid over it: values of type-scope parameters replace the
    /// type's (as a test case does), values of instance-scope parameters are the instance's.
    ///
    /// # Errors
    /// As [`evaluate`].
    pub fn evaluate_with(
        &self,
        type_name: Option<&str>,
        set: &BTreeMap<String, Stored>,
    ) -> Result<Evaluation> {
        let (d, instance, name) = self.overlaid(type_name, set)?;
        d.evaluate(name.as_deref(), &instance)
    }

    /// A copy of the definition with `set` laid over the type's values (type-scope parameters),
    /// the instance values (instance-scope parameters) and the name of the added type.
    ///
    /// # Errors
    /// `ab.family.type_unknown` for a type the definition lacks.
    pub fn overlaid(
        &self,
        type_name: Option<&str>,
        set: &BTreeMap<String, Stored>,
    ) -> Result<(Self, BTreeMap<String, Stored>, Option<String>)> {
        if let Some(n) = type_name
            && self.type_named(n).is_none()
        {
            return Err(Error::new(codes::FAMILY_TYPE_UNKNOWN).with_arg("type", n));
        }
        let mut d = self.clone();
        let mut instance = BTreeMap::new();
        let mut over = BTreeMap::new();
        for (k, v) in set {
            match self.parameter(k).map(|p| p.scope) {
                Some(Scope::Instance) => {
                    instance.insert(k.clone(), v.clone());
                }
                _ => {
                    over.insert(k.clone(), v.clone());
                }
            }
        }
        let name = "\u{1}override";
        let mut values = type_name
            .and_then(|n| self.type_named(n))
            .map(|t| t.values.clone())
            .unwrap_or_default();
        values.extend(over);
        d.types.push(super::def::TypeDef {
            id: self.header.id,
            name: name.to_owned(),
            values,
        });
        Ok((d, instance, Some(name.to_owned())))
    }

    /// Runs every test case: the type, the values set, and what the evaluation must give.
    ///
    /// # Errors
    /// `ab.family.test_failed` naming the first failing test and the reason.
    pub fn run_tests(&self) -> Result<()> {
        for t in &self.tests {
            let fail = |reason: &str| {
                Error::new(codes::FAMILY_TEST)
                    .with_arg("test", t.name.as_str())
                    .with_arg("reason", reason)
            };
            let result = self.evaluate_with(t.type_name.as_deref(), &t.set);
            match (&t.fails, result) {
                (Some(key), Err(e)) => {
                    let display = self.parameter(key).map(|p| p.name.as_str());
                    let named = ["form", "parameter", "plane"].into_iter().any(|a| {
                        matches!(e.arguments().get(a),
                            Some(arc_base::error::Value::Text(v)) if v == key || Some(v.as_str()) == display)
                    });
                    if !named {
                        return Err(fail("wrong_failure"));
                    }
                }
                (Some(_), Ok(_)) => return Err(fail("did_not_fail")),
                (None, Err(e)) => return Err(fail(e.code())),
                (None, Ok(ev)) => {
                    if let Some(v) = t.volume
                        && (ev.volume - v).abs() > t.tolerance.unwrap_or(1e-6)
                    {
                        return Err(fail("volume"));
                    }
                    if let Some(n) = t.solids
                        && usize::try_from(n).ok() != Some(ev.solids.len())
                    {
                        return Err(fail("solids"));
                    }
                }
            }
        }
        Ok(())
    }
}
