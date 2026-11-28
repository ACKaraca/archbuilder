//! Formulas of parameters and document variables (ADR 0009, section 2): the stored value (a
//! literal or a formula with the unit its bare numbers take), parsing through `arc-solve` with the
//! product's limits, the version 1 function set, reference extraction, identifier rewriting on
//! rename, evaluation with a dimension check, and the ordered evaluation of one scope.
//!
//! A formula is stored as its source text with `.` as the decimal separator and the unit of bare
//! numbers; the parsed tree, the names it reads and its result are derived and never stored.
//! Names in formulas are formula identifiers: ASCII, unique among the names a formula can see.

use std::collections::{BTreeMap, BTreeSet};

use arc_base::{Dimension, Error, Quantity, Result, Unit};
use arc_doc::{Bound, FieldDecl, Value, ValueType};
use arc_solve::{Context, Expr, ExprKind, ExprLimits, Functions, Names, Span, Value as SolveValue};

use crate::error::codes;

/// The version of the function set formulas may call; recorded with every definition.
pub const FUNCTION_SET_VERSION: (u32, u32) = (1, 1);

/// The first function set version that has `lookup`; a definition that calls it declares at least
/// this one.
pub const LOOKUP_SINCE: (u32, u32) = (1, 1);

/// The functions of version 1.1 (`lookup` reads the embedded tables of a family; outside a
/// family it has no table and fails).
pub const FUNCTIONS: [&str; 23] = [
    "abs", "min", "max", "sqrt", "pow", "sin", "cos", "tan", "asin", "acos", "atan", "atan2",
    "round", "floor", "ceil", "if", "len", "sum", "first", "last", "concat", "format", "lookup",
];

/// The limits of one formula (the value part of OQ-01).
pub const LIMITS: ExprLimits = ExprLimits::DEFAULT
    .with_text_bytes(2048)
    .with_depth(32)
    .with_list_length(1000)
    .with_steps(10_000)
    .with_value_bytes(65_536);

/// The longest formula identifier, in bytes.
pub const MAX_IDENTIFIER: usize = 64;
/// The most document variables, and the most project parameters, of one document.
pub const MAX_DEFINITIONS: usize = 1000;
/// The longest unit symbol a formula stores, in bytes.
const MAX_UNIT: u64 = 32;

coded! {
    /// The data type of a parameter or variable. Quantity types carry a dimension; material and
    /// family type values are literals only in version 1.
    DataType {
        /// A length.
        #[default]
        Length = 0, "length";
        /// An area.
        Area = 1, "area";
        /// A volume.
        Volume = 2, "volume";
        /// An angle.
        Angle = 3, "angle";
        /// A slope: rise over run, dimensionless.
        Slope = 4, "slope";
        /// A dimensionless number.
        Number = 5, "number";
        /// A whole dimensionless number.
        Integer = 6, "integer";
        /// A truth value.
        Boolean = 7, "boolean";
        /// A text.
        Text = 8, "text";
        /// A text naming one of the allowed values.
        Enumeration = 9, "enumeration";
        /// The name of a material.
        Material = 10, "material";
        /// The name of a family type.
        FamilyType = 11, "family_type";
    }
}

impl DataType {
    /// The dimension of a numeric type; `None` for truth values and texts.
    #[must_use]
    pub const fn dimension(self) -> Option<Dimension> {
        match self {
            Self::Length => Some(Dimension::LENGTH),
            Self::Area => Some(Dimension::AREA),
            Self::Volume => Some(Dimension::VOLUME),
            Self::Angle => Some(Dimension::ANGLE),
            Self::Slope | Self::Number | Self::Integer => Some(Dimension::NONE),
            Self::Boolean | Self::Text | Self::Enumeration | Self::Material | Self::FamilyType => {
                None
            }
        }
    }

    /// Whether a value of this type is a text.
    #[must_use]
    pub const fn textual(self) -> bool {
        matches!(
            self,
            Self::Text | Self::Enumeration | Self::Material | Self::FamilyType
        )
    }

    /// Whether a value of this type may be a formula.
    #[must_use]
    pub const fn takes_formula(self) -> bool {
        !matches!(self, Self::Material | Self::FamilyType)
    }

    /// The value a definition of this type has without one: zero, false or empty.
    #[must_use]
    pub fn zero(self) -> Evaluated {
        if self.dimension().is_some() {
            Evaluated::Number(0.0)
        } else if self == Self::Boolean {
            Evaluated::Bool(false)
        } else {
            Evaluated::Text(String::new())
        }
    }
}

/// A value after evaluation: a number in SI base units of the type's dimension, a truth value or
/// a text. Derived, never stored for a formula.
#[derive(Debug, Clone, PartialEq)]
pub enum Evaluated {
    /// A number in SI base units.
    Number(f64),
    /// A truth value.
    Bool(bool),
    /// A text.
    Text(String),
}

impl Evaluated {
    /// The value as a formula reads it, with the dimension of `data`.
    #[must_use]
    pub fn to_solve(&self, data: DataType) -> SolveValue {
        match self {
            Self::Number(v) => SolveValue::Number(Quantity::new(
                *v,
                data.dimension().unwrap_or(Dimension::NONE),
            )),
            Self::Bool(b) => SolveValue::Bool(*b),
            Self::Text(t) => SolveValue::text(t.as_str()),
        }
    }

    /// Whether this value has the kind `data` takes (an integer is whole).
    #[must_use]
    pub fn fits(&self, data: DataType) -> bool {
        match self {
            Self::Number(v) => {
                data.dimension().is_some()
                    && v.is_finite()
                    && (data != DataType::Integer || v.fract() == 0.0)
            }
            Self::Bool(_) => data == DataType::Boolean,
            Self::Text(t) => data.textual() && t.len() <= LIMITS.text_bytes,
        }
    }
}

/// Whether `text` is a formula identifier: an ASCII letter or `_`, then letters, digits and `_`,
/// at most [`MAX_IDENTIFIER`] bytes.
#[must_use]
pub fn is_identifier(text: &str) -> bool {
    text.len() <= MAX_IDENTIFIER
        && text.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Checks that `text` can name a parameter or variable: an identifier that is not `true`,
/// `false`, a function or a unit, so that a formula never reads it as something else.
///
/// # Errors
/// `ab.model.param_identifier_invalid`.
pub fn check_identifier(text: &str) -> Result<()> {
    let reserved = matches!(text, "true" | "false")
        || FUNCTIONS.contains(&text)
        || text == "lookup"
        || Unit::by_symbol(text).is_ok()
        || Unit::by_name(text).is_ok();
    if is_identifier(text) && !reserved {
        Ok(())
    } else {
        Err(Error::new(codes::PARAM_IDENTIFIER_INVALID).with_arg("identifier", text))
    }
}

/// The identifier proposed for the display name `name`: Turkish letters transliterated, other
/// characters `_`, a leading digit prefixed with `_`, and a number appended until `taken` is
/// false and the identifier is valid.
#[must_use]
pub fn propose_identifier(name: &str, taken: &dyn Fn(&str) -> bool) -> String {
    let mut base = String::new();
    for c in name.trim().chars() {
        let c = match c {
            'ç' => 'c',
            'Ç' => 'C',
            'ğ' => 'g',
            'Ğ' => 'G',
            'ı' => 'i',
            'İ' => 'I',
            'ö' => 'o',
            'Ö' => 'O',
            'ş' => 's',
            'Ş' => 'S',
            'ü' => 'u',
            'Ü' => 'U',
            c if c.is_ascii_alphanumeric() => c,
            _ => '_',
        };
        if !(c == '_' && base.ends_with('_')) {
            base.push(c);
        }
    }
    let mut base = base.trim_matches('_').to_owned();
    if base.is_empty() || base.starts_with(|c: char| c.is_ascii_digit()) {
        base.insert(0, '_');
    }
    base.truncate(MAX_IDENTIFIER - 6);
    let mut candidate = base.clone();
    let mut n = 2_u32;
    while check_identifier(&candidate).is_err() || taken(&candidate) {
        candidate = format!("{base}_{n}");
        n = n.saturating_add(1);
        if n == u32::MAX {
            break;
        }
    }
    candidate
}

/// A formula: its canonical source text and the unit its bare numbers take.
#[derive(Debug, Clone, PartialEq)]
pub struct Formula {
    text: String,
    unit: Option<&'static Unit>,
}

impl Formula {
    /// A formula from canonical text (`.` as the decimal separator), checked by parsing.
    ///
    /// # Errors
    /// The `solve.expr.*` parse errors with their spans, and `solve.expr.unknown_function`
    /// (with the span of the name) for a call outside the version 1 function set.
    pub fn new(text: &str, unit: Option<&'static Unit>) -> Result<Self> {
        let formula = Self {
            text: text.trim().to_owned(),
            unit,
        };
        formula.parse()?;
        Ok(formula)
    }

    /// A formula from text typed with the decimal separator `separator`, stored canonically:
    /// the separator inside number literals becomes `.`; the rest stays as typed.
    ///
    /// # Errors
    /// Those of [`Formula::new`] and of [`Expr::parse_localized`].
    pub fn localized(text: &str, unit: Option<&'static Unit>, separator: char) -> Result<Self> {
        let text = text.trim();
        if separator == '.' {
            return Self::new(text, unit);
        }
        let expr = Expr::parse_localized(text, &LIMITS, separator)?;
        let mut numbers = Vec::new();
        walk(&expr, &mut |node| {
            if matches!(
                node.kind(),
                ExprKind::Bare(_) | ExprKind::Quantity(_) | ExprKind::Absolute(_)
            ) {
                numbers.push(node.span());
            }
        });
        let mut out = text.to_owned();
        numbers.sort_by_key(|s| std::cmp::Reverse(s.start));
        for span in numbers {
            if let Some(part) = text.get(span.start..span.end) {
                let mut buffer = [0_u8; 4];
                let replaced = part.replace(&*separator.encode_utf8(&mut buffer), ".");
                out.replace_range(span.start..span.end, &replaced);
            }
        }
        Self::new(&out, unit)
    }

    /// The canonical source text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The unit bare numbers take; `None` keeps them dimensionless.
    #[must_use]
    pub fn unit(&self) -> Option<&'static Unit> {
        self.unit
    }

    /// The parsed tree, with every call checked against the function set.
    ///
    /// # Errors
    /// As [`Formula::new`].
    pub fn parse(&self) -> Result<Expr> {
        let expr = Expr::parse(&self.text, &LIMITS)?;
        let mut unknown = None;
        walk(&expr, &mut |node| {
            if let ExprKind::Call(name, _) = node.kind()
                && !FUNCTIONS.contains(&name.as_str())
                && unknown.is_none()
            {
                let start = node.span().start;
                unknown = Some(Span {
                    start,
                    end: start + name.len(),
                });
            }
        });
        match unknown {
            Some(span) => Err(Error::new(arc_solve::error::codes::EXPR_UNKNOWN_FUNCTION)
                .with_detail("start", i64::try_from(span.start).unwrap_or(i64::MAX))
                .with_detail("end", i64::try_from(span.end).unwrap_or(i64::MAX))),
            None => Ok(expr),
        }
    }

    /// The names of the functions the formula calls.
    #[must_use]
    pub fn calls(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        if let Ok(expr) = self.parse() {
            walk(&expr, &mut |node| {
                if let ExprKind::Call(name, _) = node.kind() {
                    out.insert(name.clone());
                }
            });
        }
        out
    }

    /// Every name the formula reads, with the span of each use, in text order.
    ///
    /// # Errors
    /// As [`Formula::new`].
    pub fn references(&self) -> Result<Vec<(String, Span)>> {
        let expr = self.parse()?;
        let mut out = Vec::new();
        walk(&expr, &mut |node| {
            if let ExprKind::Name(name) = node.kind() {
                out.push((name.clone(), node.span()));
            }
        });
        out.sort_by_key(|(_, s)| s.start);
        Ok(out)
    }

    /// The distinct names the formula reads; empty when it does not parse.
    #[must_use]
    pub fn names(&self) -> BTreeSet<String> {
        self.references()
            .map(|r| r.into_iter().map(|(n, _)| n).collect())
            .unwrap_or_default()
    }

    /// The formula with every use of the name `from` replaced by `to`, token by token from the
    /// parsed spans; `None` when it does not read `from`. A formula that does not parse is left
    /// as it is.
    ///
    /// # Errors
    /// `ab.model.param_identifier_invalid` when `to` cannot name a parameter, and the parse
    /// errors of the result (for example a text grown past the limit).
    pub fn renamed(&self, from: &str, to: &str) -> Result<Option<Self>> {
        check_identifier(to)?;
        let Ok(references) = self.references() else {
            return Ok(None);
        };
        let spans: Vec<Span> = references
            .into_iter()
            .filter(|(n, _)| n == from)
            .map(|(_, s)| s)
            .collect();
        if spans.is_empty() {
            return Ok(None);
        }
        let mut text = self.text.clone();
        for span in spans.iter().rev() {
            text.replace_range(span.start..span.end, to);
        }
        Self::new(&text, self.unit).map(Some)
    }

    /// Evaluates the formula as a value of `data`, reading names through `names`.
    ///
    /// # Errors
    /// The `solve.expr.*` errors with their spans; a result of another dimension or kind is
    /// `solve.expr.dimension_mismatch` or `solve.expr.type_mismatch` with the span of the whole
    /// formula; `ab.model.param_value_invalid` for a type that takes no formula, an integer that
    /// is not whole or a text past the limit.
    pub fn evaluate(&self, names: &dyn Names, data: DataType) -> Result<Evaluated> {
        self.evaluate_with(names, data, None)
    }

    /// [`Self::evaluate`] with host functions (the `lookup` of a family's tables).
    ///
    /// # Errors
    /// As [`Self::evaluate`].
    pub fn evaluate_with(
        &self,
        names: &dyn Names,
        data: DataType,
        functions: Option<&Functions>,
    ) -> Result<Evaluated> {
        if !data.takes_formula() {
            return Err(Error::new(codes::PARAM_VALUE_INVALID));
        }
        let expr = self.parse()?;
        let mut context = Context::new().with_limits(LIMITS).with_names(names);
        if let Some(f) = functions {
            context = context.with_functions(f);
        }
        if let Some(unit) = self.unit {
            context = context.with_bare_unit(*unit);
        }
        let whole = expr.span();
        let mismatch = || {
            Error::new(arc_solve::error::codes::EXPR_TYPE_MISMATCH)
                .with_detail("start", i64::try_from(whole.start).unwrap_or(i64::MAX))
                .with_detail("end", i64::try_from(whole.end).unwrap_or(i64::MAX))
        };
        let value = match data.dimension() {
            Some(dimension) => Evaluated::Number(expr.evaluate_as(&context, dimension)?.value()),
            None => match expr.evaluate(&context)? {
                SolveValue::Bool(b) if data == DataType::Boolean => Evaluated::Bool(b),
                SolveValue::Text(t) if data.textual() => Evaluated::Text(t.to_string()),
                _ => return Err(mismatch()),
            },
        };
        if value.fits(data) {
            Ok(value)
        } else {
            Err(Error::new(codes::PARAM_VALUE_INVALID))
        }
    }
}

/// Calls `visit` on every node of `expr`, without recursion.
fn walk(expr: &Expr, visit: &mut dyn FnMut(&Expr)) {
    let mut pending = vec![expr];
    while let Some(node) = pending.pop() {
        visit(node);
        match node.kind() {
            ExprKind::List(items) | ExprKind::Call(_, items) => pending.extend(items.iter()),
            ExprKind::Unary(_, operand) => pending.push(operand),
            ExprKind::Binary(_, a, b) => {
                pending.push(a);
                pending.push(b);
            }
            ExprKind::Chain(first, rest) => {
                pending.push(first);
                pending.extend(rest.iter().map(|(_, e)| e));
            }
            _ => {}
        }
    }
}

/// The value of a parameter or variable as stored: a literal (a number in SI base units, a truth
/// value or a text) or a formula.
#[derive(Debug, Clone, PartialEq)]
pub enum Stored {
    /// A literal.
    Literal(Evaluated),
    /// A formula.
    Formula(Formula),
}

/// Field positions of a stored value record.
pub mod stored {
    /// A literal number, in SI base units.
    pub const NUMBER: usize = 0;
    /// A literal truth value.
    pub const FLAG: usize = 1;
    /// A literal text.
    pub const TEXT: usize = 2;
    /// A formula's canonical text.
    pub const FORMULA: usize = 3;
    /// The symbol of the unit a formula's bare numbers take.
    pub const UNIT: usize = 4;
}

/// The fields of a stored value record: one of a literal number, truth value, text or formula
/// (with the unit of its bare numbers).
#[must_use]
pub fn stored_fields() -> Vec<FieldDecl> {
    let text = |name: &str, max: u64| {
        FieldDecl::new(name, ValueType::String)
            .optional()
            .with_bound(Bound::Length { max })
            .in_aspect("data")
    };
    vec![
        FieldDecl::new("number", ValueType::F64)
            .optional()
            .in_aspect("data"),
        FieldDecl::new("flag", ValueType::Bool)
            .optional()
            .in_aspect("data"),
        text("text", LIMITS.text_bytes as u64),
        text("formula", LIMITS.text_bytes as u64),
        text("unit", MAX_UNIT),
    ]
}

impl Stored {
    /// The record value.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut fields: Vec<Option<Value>> = vec![None; 5];
        match self {
            Self::Literal(Evaluated::Number(v)) => fields[stored::NUMBER] = Some(Value::F64(*v)),
            Self::Literal(Evaluated::Bool(b)) => fields[stored::FLAG] = Some(Value::Bool(*b)),
            Self::Literal(Evaluated::Text(t)) => {
                fields[stored::TEXT] = Some(Value::String(t.clone()));
            }
            Self::Formula(f) => {
                fields[stored::FORMULA] = Some(Value::String(f.text.clone()));
                fields[stored::UNIT] = f.unit.map(|u| Value::String(u.symbol.to_owned()));
            }
        }
        Value::Record(fields)
    }

    /// The stored value of a record value; `None` when it holds none. A formula is read without
    /// parsing (a formula that no longer parses is reported when evaluated); an unknown unit
    /// leaves bare numbers dimensionless.
    #[must_use]
    pub fn from_value(value: Option<&Value>) -> Option<Self> {
        let Some(Value::Record(fields)) = value else {
            return None;
        };
        let get = |at: usize| fields.get(at).and_then(Option::as_ref);
        if let Some(Value::String(text)) = get(stored::FORMULA) {
            let unit = match get(stored::UNIT) {
                Some(Value::String(symbol)) => Unit::by_symbol(symbol).ok(),
                _ => None,
            };
            return Some(Self::Formula(Formula {
                text: text.clone(),
                unit,
            }));
        }
        match (get(stored::NUMBER), get(stored::FLAG), get(stored::TEXT)) {
            (Some(Value::F64(v)), _, _) => Some(Self::Literal(Evaluated::Number(*v))),
            (_, Some(Value::Bool(b)), _) => Some(Self::Literal(Evaluated::Bool(*b))),
            (_, _, Some(Value::String(t))) => Some(Self::Literal(Evaluated::Text(t.clone()))),
            _ => None,
        }
    }

    /// The formula, when this is one.
    #[must_use]
    pub fn formula(&self) -> Option<&Formula> {
        match self {
            Self::Formula(f) => Some(f),
            Self::Literal(_) => None,
        }
    }

    /// The names a formula reads; empty for a literal.
    #[must_use]
    pub fn names(&self) -> BTreeSet<String> {
        self.formula().map(Formula::names).unwrap_or_default()
    }

    /// Evaluates the value as `data`.
    ///
    /// # Errors
    /// Those of [`Formula::evaluate`]; `ab.model.param_value_invalid` for a literal of another
    /// kind.
    pub fn evaluate(&self, names: &dyn Names, data: DataType) -> Result<Evaluated> {
        self.evaluate_with(names, data, None)
    }

    /// [`Self::evaluate`] with host functions.
    ///
    /// # Errors
    /// As [`Self::evaluate`].
    pub fn evaluate_with(
        &self,
        names: &dyn Names,
        data: DataType,
        functions: Option<&Functions>,
    ) -> Result<Evaluated> {
        match self {
            Self::Formula(f) => f.evaluate_with(names, data, functions),
            Self::Literal(v) if v.fits(data) => Ok(v.clone()),
            Self::Literal(_) => Err(Error::new(codes::PARAM_VALUE_INVALID)),
        }
    }
}

/// One definition of a scope: a parameter or variable with its value there.
#[derive(Debug, Clone, Copy)]
pub struct Node<'a> {
    /// The stable identifier; ties of the evaluation order are broken by it.
    pub key: arc_base::ObjectId,
    /// The formula identifier.
    pub identifier: &'a str,
    /// The display name, for messages.
    pub name: &'a str,
    /// The data type.
    pub data: DataType,
    /// The value; without one the type's zero.
    pub value: Option<&'a Stored>,
}

/// The evaluation order of `nodes`: topological over the names each formula reads (only names of
/// the scope count), ties broken by key.
///
/// # Errors
/// `ab.model.param_cycle` naming the cycle by display name (`a → b → a`).
pub fn order(nodes: &[Node<'_>]) -> Result<Vec<usize>> {
    let index: BTreeMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.identifier, i))
        .collect();
    // reads[i]: the nodes formula i reads.
    let reads: Vec<Vec<usize>> = nodes
        .iter()
        .map(|n| {
            n.value
                .map(Stored::names)
                .unwrap_or_default()
                .iter()
                .filter_map(|name| index.get(name.as_str()).copied())
                .collect()
        })
        .collect();
    let mut readers: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    let mut waiting: Vec<usize> = reads.iter().map(Vec::len).collect();
    for (i, r) in reads.iter().enumerate() {
        for &j in r {
            readers[j].push(i);
        }
    }
    let mut ready: BTreeSet<(arc_base::ObjectId, usize)> = waiting
        .iter()
        .enumerate()
        .filter(|(_, w)| **w == 0)
        .map(|(i, _)| (nodes[i].key, i))
        .collect();
    let mut out = Vec::with_capacity(nodes.len());
    while let Some(first) = ready.pop_first() {
        let i = first.1;
        out.push(i);
        for &k in &readers[i] {
            waiting[k] = waiting[k].saturating_sub(1);
            if waiting[k] == 0 {
                ready.insert((nodes[k].key, k));
            }
        }
    }
    if out.len() == nodes.len() {
        return Ok(out);
    }
    // Every node left reads one that is left: walk back along those until one repeats.
    let left: Vec<bool> = waiting.iter().map(|w| *w > 0).collect();
    let start = left.iter().position(|l| *l).unwrap_or_default();
    let mut path = vec![start];
    let mut at = start;
    while let Some(&next) = reads[at].iter().find(|j| left[**j]) {
        if let Some(p) = path.iter().position(|v| *v == next) {
            path.drain(..p);
            path.push(next);
            break;
        }
        path.push(next);
        at = next;
    }
    // The path runs from a reader to what it reads; the cycle is told in reading order reversed,
    // from a value to the formulas that read it.
    path.reverse();
    let cycle = path
        .iter()
        .map(|i| nodes[*i].name)
        .collect::<Vec<_>>()
        .join(" → ");
    Err(Error::new(codes::PARAM_CYCLE).with_arg("cycle", cycle))
}

/// Evaluates every node of a scope in [`order`], reading a name of the scope from the results
/// so far and any other name through `outer`.
///
/// # Errors
/// Those of [`order`], and the first evaluation error with the argument `parameter` (the
/// display name of the node whose formula failed).
pub fn evaluate_scope(
    nodes: &[Node<'_>],
    outer: &dyn Fn(&str) -> Option<SolveValue>,
) -> Result<Vec<Evaluated>> {
    evaluate_scope_with(nodes, outer, None)
}

/// [`evaluate_scope`] with host functions.
///
/// # Errors
/// As [`evaluate_scope`].
pub fn evaluate_scope_with(
    nodes: &[Node<'_>],
    outer: &dyn Fn(&str) -> Option<SolveValue>,
    functions: Option<&Functions>,
) -> Result<Vec<Evaluated>> {
    let order = order(nodes)?;
    let mut results: Vec<Option<Evaluated>> = vec![None; nodes.len()];
    let index: BTreeMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.identifier, i))
        .collect();
    for i in order {
        let node = nodes[i];
        let value = match node.value {
            None => node.data.zero(),
            Some(stored) => {
                let lookup = |name: &str| -> Result<Option<SolveValue>> {
                    Ok(match index.get(name) {
                        Some(&j) => results[j].as_ref().map(|v| v.to_solve(nodes[j].data)),
                        None => outer(name),
                    })
                };
                stored
                    .evaluate_with(&lookup, node.data, functions)
                    .map_err(|e| e.with_arg("parameter", node.name))?
            }
        };
        results[i] = Some(value);
    }
    Ok(results
        .into_iter()
        .zip(nodes)
        .map(|(r, n)| r.unwrap_or_else(|| n.data.zero()))
        .collect())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

    use std::f64::consts::PI;

    use arc_base::ObjectId;
    use arc_base::id::Identifier;
    use arc_solve::error::span_of;

    use super::*;

    fn mm() -> Option<&'static Unit> {
        Unit::by_symbol("mm").ok()
    }

    fn none(_: &str) -> Result<Option<SolveValue>> {
        Ok(None)
    }

    fn eval(text: &str, data: DataType) -> Result<Evaluated> {
        Formula::new(text, None)?.evaluate(&none, data)
    }

    fn number(text: &str) -> f64 {
        match eval(text, DataType::Number) {
            Ok(Evaluated::Number(v)) => v,
            other => panic!("{text}: {other:?}"),
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-9
    }

    fn code(text: &str, data: DataType) -> &'static str {
        match eval(text, data) {
            Err(e) => e.code(),
            Ok(v) => panic!("{text}: {v:?}"),
        }
    }

    #[test]
    #[allow(clippy::cognitive_complexity, reason = "one assertion per function")]
    fn every_function_of_version_1_evaluates() {
        assert!(close(number("abs(-2)"), 2.0));
        assert!(close(number("min(3, 1, 2)"), 1.0));
        assert!(close(number("max(3; 1; 2)"), 3.0));
        assert!(close(number("sqrt(16)"), 4.0));
        assert!(close(number("pow(2, 10)"), 1024.0));
        // Trigonometry compares within tolerance until it is deterministic.
        assert!(close(number("sin(30 deg)"), 0.5));
        assert!(close(number("cos(60 deg)"), 0.5));
        assert!(close(number("tan(45 deg)"), 1.0));
        let angle = |text: &str| match eval(text, DataType::Angle) {
            Ok(Evaluated::Number(v)) => v,
            other => panic!("{text}: {other:?}"),
        };
        assert!(close(angle("asin(1)"), PI / 2.0));
        assert!(close(angle("acos(1)"), 0.0));
        assert!(close(angle("atan(1)"), PI / 4.0));
        assert!(close(angle("atan2(1 m, 1 m)"), PI / 4.0));
        assert!(close(number("round(2.5)"), 3.0));
        assert!(close(number("floor(2.7)"), 2.0));
        assert!(close(number("ceil(2.1)"), 3.0));
        assert!(close(number("ceil(7, 5)"), 10.0));
        assert!(close(number("if(1 < 2, 7, 8)"), 7.0));
        assert!(close(number("len([1, 2, 3])"), 3.0));
        assert!(close(number("sum([1, 2, 3])"), 6.0));
        assert!(close(number("first([4, 5])"), 4.0));
        assert!(close(number("last([4, 5])"), 5.0));
        assert_eq!(
            eval("concat(\"a\", \"b\")", DataType::Text).unwrap(),
            Evaluated::Text("ab".into())
        );
        assert!(matches!(
            eval("format(1200 mm, \"m\")", DataType::Text),
            Ok(Evaluated::Text(_))
        ));
        assert_eq!(
            eval("1 m > 2 m || !false && true", DataType::Boolean).unwrap(),
            Evaluated::Bool(true)
        );
        // Every listed function is one arc-solve knows.
        for name in FUNCTIONS.into_iter().filter(|n| *n != "lookup") {
            let text = format!("{name}()");
            let error = Formula::new(&text, None)
                .and_then(|f| f.evaluate(&none, DataType::Number))
                .unwrap_err();
            assert_ne!(error.code(), "solve.expr.unknown_function", "{name}");
        }
    }

    #[test]
    fn functions_outside_version_1_are_refused_at_the_name() {
        for text in ["exp(1)", "ln(2)", "log(10)", "1 + roundup(2)"] {
            let error = Formula::new(text, None).unwrap_err();
            assert_eq!(error.code(), "solve.expr.unknown_function", "{text}");
            let span = span_of(&error).unwrap();
            assert!(text[span.start..span.end].starts_with(|c: char| c.is_alphabetic()));
            assert!(!text[span.start..span.end].contains('('), "{text}");
        }
    }

    #[test]
    fn every_limit_holds() {
        let limit = |text: &str| {
            let error = Formula::new(text, None)
                .and_then(|f| f.evaluate(&none, DataType::Number))
                .unwrap_err();
            assert_eq!(error.code(), "solve.expr.limit_exceeded");
            match error.details().get("limit") {
                Some(arc_base::error::Value::Text(t)) => t.clone(),
                other => panic!("{other:?}"),
            }
        };
        assert_eq!(limit(&"1+".repeat(1100)), "text_bytes");
        assert_eq!(
            limit(&format!("{}1{}", "(".repeat(40), ")".repeat(40))),
            "depth"
        );
        let long = format!("len([{}])", vec!["1"; 1001].join(","));
        assert_eq!(limit(&long), "list_length");
        // Each read of a long list admits every item: three reads pass 65,536 bytes of values.
        let items: Vec<SolveValue> = (0..1000).map(|i| SolveValue::Bare(f64::from(i))).collect();
        let list = SolveValue::list(items);
        let names = move |name: &str| -> Result<Option<SolveValue>> {
            Ok((name == "xs").then(|| list.clone()))
        };
        let text = ["len(xs)"; 3].join(" + ");
        let error = Formula::new(&text, None)
            .unwrap()
            .evaluate(&names, DataType::Number)
            .unwrap_err();
        assert_eq!(
            error.details().get("limit"),
            Some(&arc_base::error::Value::Text("value_bytes".into()))
        );
        // Two reads stay inside every limit, steps included: 10,000 steps is a backstop that a
        // formula within the text and value limits does not reach.
        let text = ["len(xs)"; 2].join(" + ");
        assert!(
            Formula::new(&text, None)
                .unwrap()
                .evaluate(&names, DataType::Number)
                .is_ok()
        );
        // A text value past 2,048 bytes.
        let grow = format!("concat({})", vec!["\"aaaaaaaaaaaaaaaaaaaa\""; 99].join(","));
        let text = format!("concat({grow}, {grow})");
        assert!(matches!(
            limit(&text).as_str(),
            "text_bytes" | "value_bytes"
        ));
        let limits = (LIMITS.steps, LIMITS.value_bytes, MAX_DEFINITIONS);
        assert_eq!(limits, (10_000, 65_536, 1000));
    }

    #[test]
    fn dimension_errors_carry_spans() {
        let text = "2 * (1 m + 3 m^2)";
        let error = eval(text, DataType::Length).unwrap_err();
        assert_eq!(error.code(), "solve.expr.dimension_mismatch");
        let span = span_of(&error).unwrap();
        assert_eq!(&text[span.start..span.end], "(1 m + 3 m^2)");
        // A length assigned to an angle: the whole formula's span.
        let text = "1 m + 2 m";
        let error = eval(text, DataType::Angle).unwrap_err();
        assert_eq!(error.code(), "solve.expr.dimension_mismatch");
        let span = span_of(&error).unwrap();
        assert_eq!((span.start, span.end), (0, text.len()));
        assert_eq!(code("1 m", DataType::Boolean), "solve.expr.type_mismatch");
        assert_eq!(
            code("2.5", DataType::Integer),
            "ab.model.param_value_invalid"
        );
        assert_eq!(
            code("\"x\"", DataType::Material),
            "ab.model.param_value_invalid"
        );
    }

    #[test]
    fn bare_numbers_take_the_stored_unit() {
        let f = Formula::new("900 + 100", mm()).unwrap();
        assert_eq!(
            f.evaluate(&none, DataType::Length).unwrap(),
            Evaluated::Number(1.0)
        );
        let stored = Stored::Formula(f);
        assert_eq!(Stored::from_value(Some(&stored.to_value())), Some(stored));
        for literal in [
            Evaluated::Number(0.9),
            Evaluated::Bool(true),
            Evaluated::Text("Oak".into()),
        ] {
            let s = Stored::Literal(literal);
            assert_eq!(Stored::from_value(Some(&s.to_value())), Some(s));
        }
    }

    #[test]
    fn localized_input_is_stored_canonically() {
        let f = Formula::localized("max(1,5 m; width) + 0,25 m", None, ',').unwrap();
        assert_eq!(f.text(), "max(1.5 m; width) + 0.25 m");
        let f = Formula::localized("concat(\"1,5\"; \"x\")", None, ',').unwrap();
        assert_eq!(f.text(), "concat(\"1,5\"; \"x\")");
    }

    #[test]
    fn references_are_names_with_spans() {
        let f = Formula::new("width * 2 + if(tall, height, width) + sin(a)", None).unwrap();
        let refs = f.references().unwrap();
        let names: Vec<&str> = refs.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["width", "tall", "height", "width", "a"]);
        for (name, span) in &refs {
            assert_eq!(&f.text()[span.start..span.end], name);
        }
    }

    #[test]
    fn rename_keeps_spans_and_meaning() {
        let text = "width*2 + if(width > 1 m, widthx, \"width\") - w";
        let f = Formula::new("width*2 + max(width, 3 m) - widthx", None).unwrap();
        let g = f.renamed("width", "breadth").unwrap().unwrap();
        assert_eq!(g.text(), "breadth*2 + max(breadth, 3 m) - widthx");
        // The names that were not renamed keep their place relative to their neighbors.
        let before = f.references().unwrap();
        let after = g.references().unwrap();
        assert_eq!(before.len(), after.len());
        for ((a, sa), (b, sb)) in before.iter().zip(&after) {
            assert_eq!(a == "width", b == "breadth");
            assert_eq!(&g.text()[sb.start..sb.end], b);
            assert_eq!(&f.text()[sa.start..sa.end], a);
        }
        let read = |w: &'static str| {
            move |name: &str| -> Result<Option<SolveValue>> {
                Ok(match name {
                    n if n == w => Some(SolveValue::Number(Quantity::new(2.0, Dimension::LENGTH))),
                    "widthx" => Some(SolveValue::Number(Quantity::new(0.5, Dimension::LENGTH))),
                    _ => None,
                })
            }
        };
        assert_eq!(
            f.evaluate(&read("width"), DataType::Length).unwrap(),
            g.evaluate(&read("breadth"), DataType::Length).unwrap()
        );
        // A text literal that spells the name is not a use of it.
        let t = Formula::new(text, None).unwrap();
        let r = t.renamed("width", "b").unwrap().unwrap();
        assert_eq!(r.text(), "b*2 + if(b > 1 m, widthx, \"width\") - w");
        assert_eq!(t.renamed("height", "b").unwrap(), None);
        assert!(t.renamed("width", "sin").is_err());
        assert!(t.renamed("width", "2x").is_err());
    }

    #[test]
    fn identifiers_are_ascii_and_proposed_from_names() {
        assert!(check_identifier("Genislik_2").is_ok());
        for bad in ["Genişlik", "2a", "", "true", "if", "mm", "lookup", "a b"] {
            assert!(check_identifier(bad).is_err(), "{bad}");
        }
        assert_eq!(propose_identifier("Genişlik", &|_| false), "Genislik");
        assert_eq!(
            propose_identifier("Kapı Yüksekliği", &|_| false),
            "Kapi_Yuksekligi"
        );
        assert_eq!(propose_identifier("2. kat", &|_| false), "_2_kat");
        assert_eq!(propose_identifier("width", &|n| n == "width"), "width_2");
        assert_eq!(propose_identifier("m", &|_| false), "m_2");
    }

    fn id(n: u128) -> ObjectId {
        ObjectId::from_u128(n)
    }

    #[test]
    fn a_scope_evaluates_in_dependency_order_and_refuses_cycles() {
        let a = Stored::Formula(Formula::new("b + 1 m", None).unwrap());
        let b = Stored::Literal(Evaluated::Number(2.0));
        let c = Stored::Formula(Formula::new("a * 2 + outer", None).unwrap());
        let nodes = [
            Node {
                key: id(3),
                identifier: "c",
                name: "C",
                data: DataType::Length,
                value: Some(&c),
            },
            Node {
                key: id(1),
                identifier: "a",
                name: "A",
                data: DataType::Length,
                value: Some(&a),
            },
            Node {
                key: id(2),
                identifier: "b",
                name: "B",
                data: DataType::Length,
                value: Some(&b),
            },
        ];
        assert_eq!(order(&nodes).unwrap(), vec![2, 1, 0]);
        let outer = |name: &str| {
            (name == "outer").then(|| SolveValue::Number(Quantity::new(1.0, Dimension::LENGTH)))
        };
        let values = evaluate_scope(&nodes, &outer).unwrap();
        assert_eq!(values[0], Evaluated::Number(7.0));
        // Close the cycle: b reads c.
        let b2 = Stored::Formula(Formula::new("c / 2", None).unwrap());
        let mut cyclic = nodes;
        cyclic[2].value = Some(&b2);
        let error = evaluate_scope(&cyclic, &outer).unwrap_err();
        assert_eq!(error.code(), "ab.model.param_cycle");
        let Some(arc_base::error::Value::Text(cycle)) = error.arguments().get("cycle") else {
            panic!("{error:?}");
        };
        let names: Vec<&str> = cycle.split(" → ").collect();
        assert_eq!(names.len(), 4, "{cycle}");
        assert_eq!(names.first(), names.last());
        for n in ["A", "B", "C"] {
            assert!(names.contains(&n), "{cycle}");
        }
        // A failing formula names its parameter.
        let bad = Stored::Formula(Formula::new("b + 1 m^2", None).unwrap());
        let mut failing = nodes;
        failing[1].value = Some(&bad);
        let error = evaluate_scope(&failing, &outer).unwrap_err();
        assert_eq!(error.code(), "solve.expr.dimension_mismatch");
        assert_eq!(
            error.arguments().get("parameter"),
            Some(&arc_base::error::Value::Text("A".into()))
        );
    }
}
