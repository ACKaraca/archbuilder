//! Embedded lookup tables of a family (ADR 0009, sections 2 and 3; AB-FAM-018) and the
//! `lookup(table, column, key, ..., fallback)` host function that reads them.
//!
//! A table has typed columns; its first `keys` columns are the keys. `lookup` compares the keys
//! it is given with those columns exactly (numbers to within a relative `1e-12`, which only
//! absorbs the rounding of unit conversion), returns the named column of the first row that
//! matches and the fallback when none does. Every argument is dimension-checked against the
//! column: a key or the fallback of another dimension is `solve.expr.dimension_mismatch`.

use std::sync::Arc;

use arc_base::{Dimension, Error, Quantity, Result, Unit};
use arc_solve::error::codes as solve;
use arc_solve::{Functions, Value as SolveValue};

use crate::error::codes;
use crate::expr::{self, DataType, Evaluated};

/// Limits of one table.
pub mod limits {
    /// Rows.
    pub const ROWS: usize = 10_000;
    /// Columns.
    pub const COLUMNS: usize = 64;
    /// Bytes of the cells.
    pub const BYTES: usize = 1024 * 1024;
}

/// A column: a name and its data type (a quantity, a truth value or a text).
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnDef {
    /// The name, an identifier.
    pub name: String,
    /// The data type.
    pub data: DataType,
}

/// An embedded table.
#[derive(Debug, Clone, PartialEq)]
pub struct TableDef {
    /// The name, an identifier.
    pub name: String,
    /// How many leading columns are keys.
    pub keys: usize,
    /// The columns.
    pub columns: Vec<ColumnDef>,
    /// The rows; a number is in SI units of its column's dimension.
    pub rows: Vec<Vec<Evaluated>>,
}

fn invalid(reason: &str) -> Error {
    Error::new(codes::FAMILY_INVALID)
        .with_arg("part", "table")
        .with_arg("reason", reason)
}

impl TableDef {
    /// Checks the table: counts within the limits, identifiers, keys, cells of their columns'
    /// types and no two rows with the same keys.
    ///
    /// # Errors
    /// `ab.family.limit_exceeded` for rows, columns or bytes; `ab.family.invalid` otherwise.
    pub fn validate(&self) -> Result<()> {
        let over = |name: &str| Error::new(codes::FAMILY_LIMIT).with_arg("limit", name);
        if self.rows.len() > limits::ROWS {
            return Err(over("table_rows"));
        }
        if self.columns.len() > limits::COLUMNS {
            return Err(over("table_columns"));
        }
        expr::check_identifier(&self.name).map_err(|_| invalid("name"))?;
        if self.keys == 0 || self.keys > self.columns.len() {
            return Err(invalid("keys"));
        }
        let mut names = std::collections::BTreeSet::new();
        for c in &self.columns {
            expr::check_identifier(&c.name).map_err(|_| invalid("column"))?;
            let kind_ok = c.data.dimension().is_some()
                || matches!(c.data, DataType::Boolean | DataType::Text);
            if !kind_ok || !names.insert(c.name.as_str()) {
                return Err(invalid("column"));
            }
        }
        let mut bytes = 0_usize;
        let mut seen = std::collections::BTreeSet::new();
        for row in &self.rows {
            if row.len() != self.columns.len() {
                return Err(invalid("row"));
            }
            for (cell, c) in row.iter().zip(&self.columns) {
                if !cell.fits(c.data) {
                    return Err(invalid("cell"));
                }
                bytes += match cell {
                    Evaluated::Text(t) => t.len() + 1,
                    _ => 8,
                };
            }
            let mut key = String::new();
            for cell in &row[..self.keys] {
                match cell {
                    Evaluated::Number(v) => key.push_str(&format!("n{v}|")),
                    Evaluated::Bool(b) => key.push_str(&format!("b{b}|")),
                    Evaluated::Text(t) => key.push_str(&format!("t{}:{t}|", t.len())),
                }
            }
            if !seen.insert(key) {
                return Err(invalid("duplicate_key"));
            }
        }
        if bytes > limits::BYTES {
            return Err(over("table_bytes"));
        }
        Ok(())
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1.0)
}

fn fail(code: arc_base::ErrorCode) -> Error {
    Error::new(code)
}

/// The SI number of `value` read as the dimension `dimension`; a bare number takes `unit`.
fn number_of(value: &SolveValue, dimension: Dimension, unit: Option<Unit>) -> Result<f64> {
    match value {
        SolveValue::Number(q) if q.dimension() == dimension => Ok(q.value()),
        SolveValue::Number(_) => Err(fail(solve::EXPR_DIMENSION_MISMATCH)),
        SolveValue::Bare(v) if dimension == Dimension::NONE => Ok(*v),
        SolveValue::Bare(v) => match unit {
            Some(u) if u.dimension == dimension => Ok(v * u.scale),
            _ => Err(fail(solve::EXPR_DIMENSION_MISMATCH)),
        },
        _ => Err(fail(solve::EXPR_TYPE_MISMATCH)),
    }
}

/// Whether `cell` of a column of `data` equals the argument `value`.
fn same(cell: &Evaluated, data: DataType, value: &SolveValue, unit: Option<Unit>) -> Result<bool> {
    Ok(match (cell, data) {
        (Evaluated::Number(c), d) => close(
            *c,
            number_of(value, d.dimension().unwrap_or(Dimension::NONE), unit)?,
        ),
        (Evaluated::Bool(c), _) => match value {
            SolveValue::Bool(v) => c == v,
            _ => return Err(fail(solve::EXPR_TYPE_MISMATCH)),
        },
        (Evaluated::Text(c), _) => match value {
            SolveValue::Text(v) => c.as_str() == &**v,
            _ => return Err(fail(solve::EXPR_TYPE_MISMATCH)),
        },
    })
}

fn result(cell: &Evaluated, data: DataType) -> SolveValue {
    cell.to_solve(data)
}

fn text_of(value: &SolveValue) -> Result<&str> {
    match value {
        SolveValue::Text(t) => Ok(t),
        _ => Err(fail(solve::EXPR_TYPE_MISMATCH)),
    }
}

/// The host functions of a family with `tables`: `lookup`.
///
/// # Errors
/// `solve.function.invalid` when `lookup` cannot be registered (it cannot fail for a fresh
/// registry).
pub fn functions(tables: &[TableDef]) -> Result<Functions> {
    let tables: Arc<Vec<TableDef>> = Arc::new(tables.to_vec());
    let mut f = Functions::new();
    f.register(
        "lookup",
        4,
        3 + limits::COLUMNS,
        move |args: &[SolveValue], unit: Option<Unit>| {
            let (Some(table), Some(column)) = (args.first(), args.get(1)) else {
                return Err(fail(solve::EXPR_ARGUMENT_COUNT));
            };
            let (table, column) = (text_of(table)?, text_of(column)?);
            let t = tables
                .iter()
                .find(|t| t.name == table)
                .ok_or_else(|| fail(solve::EXPR_DOMAIN))?;
            let at = t
                .columns
                .iter()
                .position(|c| c.name == column)
                .ok_or_else(|| fail(solve::EXPR_DOMAIN))?;
            // table, column, one value per key, fallback.
            if args.len() != 3 + t.keys {
                return Err(fail(solve::EXPR_ARGUMENT_COUNT));
            }
            let keys = &args[2..2 + t.keys];
            let data = t.columns[at].data;
            let dimension = data.dimension().unwrap_or(Dimension::NONE);
            let fallback = &args[2 + t.keys];
            // The fallback has the column's type and dimension whether or not a row matches.
            let checked = match data {
                DataType::Boolean => matches!(fallback, SolveValue::Bool(_)),
                DataType::Text => matches!(fallback, SolveValue::Text(_)),
                _ => {
                    number_of(fallback, dimension, unit)?;
                    true
                }
            };
            if !checked {
                return Err(fail(solve::EXPR_TYPE_MISMATCH));
            }
            for row in &t.rows {
                let mut hit = true;
                for (cell, (c, key)) in row.iter().zip(t.columns.iter().zip(keys)) {
                    if !same(cell, c.data, key, unit)? {
                        hit = false;
                        break;
                    }
                }
                if hit {
                    return Ok(result(&row[at], data));
                }
            }
            Ok(match fallback {
                SolveValue::Bare(v) => SolveValue::Number(Quantity::new(
                    number_of(fallback, dimension, unit).unwrap_or(*v),
                    dimension,
                )),
                other => other.clone(),
            })
        },
    )?;
    Ok(f)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use arc_base::Unit;

    use super::*;
    use crate::expr::Formula;

    fn sizes() -> TableDef {
        let n = Evaluated::Number;
        let row = |k: &str, w: f64, fire: bool| {
            vec![Evaluated::Text(k.into()), n(w), Evaluated::Bool(fire)]
        };
        TableDef {
            name: "sizes".into(),
            keys: 1,
            columns: vec![
                ColumnDef {
                    name: "code".into(),
                    data: DataType::Text,
                },
                ColumnDef {
                    name: "width".into(),
                    data: DataType::Length,
                },
                ColumnDef {
                    name: "fire".into(),
                    data: DataType::Boolean,
                },
            ],
            rows: vec![row("S", 0.9, false), row("L", 1.2, true)],
        }
    }

    fn run(text: &str, data: DataType) -> Result<Evaluated> {
        let f = functions(&[sizes()])?;
        let unit = Unit::by_symbol("m").ok();
        Formula::new(text, unit)?.evaluate_with(&|_: &str| Ok(None), data, Some(&f))
    }

    #[test]
    fn lookup_finds_the_row_and_falls_back() {
        let width = |t: &str| match run(t, DataType::Length).unwrap() {
            Evaluated::Number(v) => v,
            other => panic!("{other:?}"),
        };
        assert!((width("lookup(\"sizes\", \"width\", \"L\", 2 m)") - 1.2).abs() < 1e-12);
        // No row for the key: the fallback, in the column's dimension.
        assert!((width("lookup(\"sizes\", \"width\", \"XL\", 700 mm)") - 0.7).abs() < 1e-12);
        assert_eq!(
            run(
                "lookup(\"sizes\", \"fire\", \"L\", false)",
                DataType::Boolean
            )
            .unwrap(),
            Evaluated::Bool(true)
        );
    }

    #[test]
    fn lookup_checks_units_names_and_counts() {
        for text in [
            "lookup(\"sizes\", \"width\", \"L\", 30 deg)",
            "lookup(\"sizes\", \"width\", 3 m, 1 m)",
            "lookup(\"nope\", \"width\", \"L\", 1 m)",
            "lookup(\"sizes\", \"nope\", \"L\", 1 m)",
            "lookup(\"sizes\", \"width\", \"L\", \"a\", 1 m)",
        ] {
            assert!(run(text, DataType::Length).is_err(), "{text}");
        }
        // Without the family's tables the function is unknown.
        let none = Formula::new("lookup(\"a\", \"b\", 1, 1)", None).unwrap();
        assert!(
            none.evaluate(&|_: &str| Ok(None), DataType::Number)
                .is_err()
        );
    }

    #[test]
    fn a_table_is_checked_against_its_limits_and_types() {
        assert!(sizes().validate().is_ok());
        let mut t = sizes();
        t.rows.push(t.rows[0].clone());
        assert!(t.validate().is_err(), "duplicate keys");
        let mut t = sizes();
        t.rows[0][1] = Evaluated::Text("x".into());
        assert!(t.validate().is_err(), "cell of another type");
        let mut t = sizes();
        t.rows = (0..=limits::ROWS)
            .map(|i| {
                vec![
                    Evaluated::Text(i.to_string()),
                    Evaluated::Number(1.0),
                    Evaluated::Bool(false),
                ]
            })
            .collect();
        assert_eq!(t.validate().unwrap_err().code(), "ab.family.limit_exceeded");
        let mut t = sizes();
        t.rows = (0..limits::ROWS)
            .map(|i| {
                vec![
                    Evaluated::Text(i.to_string()),
                    Evaluated::Number(1.0),
                    Evaluated::Bool(false),
                ]
            })
            .collect();
        assert!(t.validate().is_ok(), "exactly the row limit");
    }
}
