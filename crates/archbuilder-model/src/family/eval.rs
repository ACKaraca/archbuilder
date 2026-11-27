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

use arc_base::math::{Point3, Vector3};
use arc_base::{Error, Limits, Result, Tolerance};
use arc_geom::{FaceTag, PlaneFrame, Polyhedron, PrismTool, extrude};
use arc_solve::Value as SolveValue;

use super::def::{
    Axis, Definition, Detail, FormKind, Role, Scope, SymbolKind, Vertex, ViewKind, Visibility,
    origin_key,
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
        s.evaluate(&lookup, data)
    }

    fn length(&mut self, s: Option<&Stored>, what: &str, form: &str) -> Result<f64> {
        let Some(s) = s else { return Ok(0.0) };
        match self.eval(s, DataType::Length) {
            Ok(Evaluated::Number(v)) => Ok(v),
            Ok(_) => Err(form_failed(form, what)),
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
        spent: 0,
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
        let results = expr::evaluate_scope(&nodes, &outer)?;
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
        match f.kind {
            FormKind::Solid => solids.push((f.key.clone(), solid, height > 0.0, f)),
            FormKind::Void => {
                voids.push(Void {
                    key: f.key.clone(),
                    region,
                    frame,
                    start: height.min(0.0),
                    end: height.max(0.0),
                    bounds: solid.bounding_box(),
                });
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
        let results = expr::evaluate_scope(&nodes, &outer)?;
        for (p, v) in reporting.iter().zip(results) {
            values.insert(p.key.clone(), v);
        }
    }
    Ok(Evaluation {
        values,
        planes,
        solids: out_solids,
        symbols,
        volume,
        size,
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
        let known = type_name.is_none_or(|n| self.type_named(n).is_some());
        if !known {
            return Err(
                Error::new(codes::FAMILY_TYPE_UNKNOWN).with_arg("type", type_name.unwrap_or(""))
            );
        }
        d.evaluate(Some(name), &instance)
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
