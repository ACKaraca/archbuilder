//! The canonical text of a definition (ADR 0009, section 3; the rules of ADR 0006): a first line
//! naming the text format version and the function set version, then one line per part ordered
//! by kind and then by key, fields in schema order, numbers in the shortest form that reads back
//! the same, formulas as stored, derived data excluded. [`Definition::from_text`] reads it back;
//! text read and written again is byte-identical. The checksum of a definition is the BLAKE3
//! hash of its canonical text.
//!
//! A line is `kind name=value ...`. A value is a bare word or a double-quoted string with the
//! escapes `\\`, `\"`, `\n`, `\r` and `\t`. A stored value `x` is `x=<number>`, `x=true`,
//! `x="text"` or `x_f="formula"` with an optional `x_u=<unit symbol>`.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Write as _;

use arc_base::{ObjectId, Result, Unit};

use super::def::{
    Axis, Category, Definition, Detail, FormDef, FormKind, Header, Hosting, ParamDef, PlaneDef,
    PlaneRole, ProfileDef, Role, Scope, SymbolDef, SymbolKind, TestCase, TypeDef, Version, Vertex,
    ViewKind, Visibility, limits,
};
use crate::error::codes;
use crate::expr::{DataType, Evaluated, Formula, Stored};

/// The version of the text format.
pub const TEXT_VERSION: u32 = 1;

fn bad(line: usize, reason: &str) -> arc_base::Error {
    arc_base::Error::new(codes::FAMILY_TEXT)
        .with_arg("line", i64::try_from(line).unwrap_or(i64::MAX))
        .with_arg("reason", reason)
}

/// The checksum of a canonical text: the lowercase hex of its BLAKE3 hash.
#[must_use]
pub fn checksum(text: &str) -> String {
    blake3::hash(text.as_bytes()).to_hex().to_string()
}

fn number(v: f64) -> String {
    if v == 0.0 {
        "0".to_owned()
    } else {
        format!("{v}")
    }
}

/// The writer of one text.
struct Out {
    text: String,
    error: Option<arc_base::Error>,
}

impl Out {
    fn begin(&mut self, kind: &str) {
        if !self.text.is_empty() {
            self.text.push('\n');
        }
        self.text.push_str(kind);
    }
    fn word(&mut self, name: &str, v: &str) {
        let _ = write!(self.text, " {name}={v}");
    }
    fn string(&mut self, name: &str, v: &str) {
        let _ = write!(self.text, " {name}=\"");
        for c in v.chars() {
            match c {
                '\\' => self.text.push_str("\\\\"),
                '"' => self.text.push_str("\\\""),
                '\n' => self.text.push_str("\\n"),
                '\r' => self.text.push_str("\\r"),
                '\t' => self.text.push_str("\\t"),
                c if c.is_control() => {
                    self.error.get_or_insert_with(|| bad(0, "control"));
                }
                c => self.text.push(c),
            }
        }
        self.text.push('"');
    }
    fn num(&mut self, name: &str, v: f64) {
        if !v.is_finite() {
            self.error.get_or_insert_with(|| bad(0, "not_finite"));
        }
        self.word(name, &number(v));
    }
    fn stored(&mut self, name: &str, v: Option<&Stored>) {
        match v {
            None => {}
            Some(Stored::Literal(Evaluated::Number(n))) => self.num(name, *n),
            Some(Stored::Literal(Evaluated::Bool(b))) => self.word(name, &b.to_string()),
            Some(Stored::Literal(Evaluated::Text(t))) => self.string(name, t),
            Some(Stored::Formula(f)) => {
                self.string(&format!("{name}_f"), f.text());
                if let Some(u) = f.unit() {
                    self.word(&format!("{name}_u"), u.symbol);
                }
            }
        }
    }
    fn visibility(&mut self, v: &Visibility) {
        if !v.details.is_empty() {
            let mut d: Vec<_> = v.details.clone();
            d.sort_by_key(|x| x.code());
            let list: Vec<&str> = d.iter().map(|x| x.name()).collect();
            self.word("detail", &list.join(","));
        }
        if !v.views.is_empty() {
            let mut d: Vec<_> = v.views.clone();
            d.sort_by_key(|x| x.code());
            let list: Vec<&str> = d.iter().map(|x| x.name()).collect();
            self.word("views", &list.join(","));
        }
        if let Some(w) = &v.when {
            self.string("when", w.text());
        }
    }
    fn vertices(&mut self, vs: &[Vertex]) {
        for v in vs {
            self.begin("vertex");
            if let Some(u) = &v.u {
                self.word("u", u);
            }
            self.stored("du", v.du.as_ref());
            if let Some(w) = &v.v {
                self.word("v", w);
            }
            self.stored("dv", v.dv.as_ref());
            self.stored("bulge", v.bulge.as_ref());
        }
    }
}

impl Definition {
    /// The canonical text.
    ///
    /// # Errors
    /// `ab.family.text_invalid` for a number that is not finite or a control character in a
    /// string; the errors of [`Definition::validate`].
    #[allow(
        clippy::too_many_lines,
        clippy::cognitive_complexity,
        reason = "one pass over the parts of a definition, in order"
    )]
    pub fn to_text(&self) -> Result<String> {
        self.validate()?;
        let mut o = Out {
            text: format!(
                "archbfam-text {TEXT_VERSION} functions {}.{}",
                self.header.functions.0, self.header.functions.1
            ),
            error: None,
        };
        let h = &self.header;
        o.begin("definition");
        o.word("id", &h.id.to_string());
        o.word("version", &h.version.to_string());
        o.string("name", &h.name);
        o.word("category", h.category.name());
        o.word("hosting", h.hosting.name());
        let flags: Vec<&str> = h.flags.iter().map(String::as_str).collect();
        o.string("flags", &flags.join(","));
        o.string("ifc_entity", &h.ifc_entity);
        o.string("ifc_type", &h.ifc_type);
        o.string("source", &h.source);
        o.string("license", &h.license);
        let mut params: Vec<&ParamDef> = self.parameters.iter().collect();
        params.sort_by(|a, b| a.key.cmp(&b.key));
        for p in params {
            o.begin("parameter");
            o.word("key", &p.key);
            o.word("id", &p.id.to_string());
            o.string("name", &p.name);
            o.word("data", p.data.name());
            o.word("scope", p.scope.name());
            o.word("role", p.role.name());
            o.string("group", &p.group);
            o.word("order", &p.order.to_string());
            if let Some(m) = p.minimum {
                o.num("min", m);
            }
            if let Some(m) = p.maximum {
                o.num("max", m);
            }
            if !p.allowed.is_empty() {
                o.string("allowed", &p.allowed.join("|"));
            }
            o.stored("default", p.default.as_ref());
        }
        let mut types: Vec<&TypeDef> = self.types.iter().collect();
        types.sort_by(|a, b| a.name.cmp(&b.name));
        for t in types {
            o.begin("type");
            o.word("id", &t.id.to_string());
            o.string("name", &t.name);
            for (key, v) in &t.values {
                o.begin("value");
                o.word("param", key);
                o.stored("v", Some(v));
            }
        }
        let mut planes: Vec<&PlaneDef> = self.planes.iter().collect();
        planes.sort_by(|a, b| a.key.cmp(&b.key));
        for p in planes {
            o.begin("plane");
            o.word("key", &p.key);
            o.string("name", &p.name);
            o.word("axis", p.axis.name());
            o.word("role", p.role.name());
            o.word("strong", &p.strong.to_string());
            o.stored("offset", Some(&p.offset));
        }
        let mut profiles: Vec<&ProfileDef> = self.profiles.iter().collect();
        profiles.sort_by(|a, b| a.key.cmp(&b.key));
        for p in profiles {
            o.begin("profile");
            o.word("key", &p.key);
            o.word("axis", p.axis.name());
            o.vertices(&p.vertices);
        }
        let mut forms: Vec<&FormDef> = self.forms.iter().collect();
        forms.sort_by(|a, b| a.key.cmp(&b.key));
        for f in forms {
            o.begin("form");
            o.word("key", &f.key);
            o.word("kind", f.kind.name());
            o.word("profile", &f.profile);
            o.word("from", &f.from);
            if let Some(t) = &f.to {
                o.word("to", t);
            }
            o.stored("depth", f.depth.as_ref());
            o.string("material", &f.material);
            o.string("subcategory", &f.subcategory);
            o.visibility(&f.visibility);
        }
        let mut symbols: Vec<&SymbolDef> = self.symbols.iter().collect();
        symbols.sort_by(|a, b| a.key.cmp(&b.key));
        for s in symbols {
            o.begin("symbol");
            o.word("key", &s.key);
            o.word("view", s.view.name());
            o.word("kind", s.kind.name());
            o.string("subcategory", &s.subcategory);
            o.visibility(&s.visibility);
            o.vertices(&s.vertices);
        }
        let mut tests: Vec<&TestCase> = self.tests.iter().collect();
        tests.sort_by(|a, b| a.name.cmp(&b.name));
        for t in tests {
            o.begin("test");
            o.string("name", &t.name);
            if let Some(n) = &t.type_name {
                o.string("type", n);
            }
            if let Some(v) = t.volume {
                o.num("volume", v);
            }
            if let Some(v) = t.tolerance {
                o.num("tolerance", v);
            }
            if let Some(v) = t.solids {
                o.word("solids", &v.to_string());
            }
            if let Some(v) = &t.fails {
                o.word("fails", v);
            }
            for (key, v) in &t.set {
                o.begin("set");
                o.word("param", key);
                o.stored("v", Some(v));
            }
        }
        o.text.push('\n');
        match o.error {
            Some(e) => Err(e),
            None => Ok(o.text),
        }
    }

    /// The checksum of the canonical text.
    ///
    /// # Errors
    /// As [`Definition::to_text`].
    pub fn checksum(&self) -> Result<String> {
        self.to_text().map(|t| checksum(&t))
    }

    /// Reads a canonical text, bounded by [`limits`], and validates the definition.
    ///
    /// # Errors
    /// `ab.family.text_invalid` with the line and the reason for anything malformed;
    /// `ab.family.limit_exceeded` for a text, line or count past the limits; the errors of
    /// [`Definition::validate`].
    pub fn from_text(text: &str) -> Result<Self> {
        if text.len() > limits::TEXT_BYTES {
            return Err(arc_base::Error::new(codes::FAMILY_LIMIT).with_arg("limit", "text_bytes"));
        }
        let mut lines: VecDeque<Fields> = VecDeque::new();
        let mut functions = None;
        for (i, raw) in text.lines().enumerate() {
            let n = i + 1;
            if n > limits::LINES {
                return Err(arc_base::Error::new(codes::FAMILY_LIMIT).with_arg("limit", "lines"));
            }
            if raw.len() > limits::LINE_BYTES {
                return Err(
                    arc_base::Error::new(codes::FAMILY_LIMIT).with_arg("limit", "line_bytes")
                );
            }
            if i == 0 {
                functions = Some(first_line(raw).ok_or_else(|| bad(1, "first_line"))?);
                continue;
            }
            if raw.is_empty() {
                return Err(bad(n, "empty_line"));
            }
            lines.push_back(Fields::parse(raw, n)?);
        }
        let functions = functions.ok_or_else(|| bad(1, "first_line"))?;
        let mut r = Reader { lines };
        let def = r.definition(functions)?;
        def.validate()?;
        Ok(def)
    }
}

fn first_line(raw: &str) -> Option<(u32, u32)> {
    let rest = raw.strip_prefix("archbfam-text ")?;
    let (v, rest) = rest.split_once(' ')?;
    if v.parse::<u32>().ok()? != TEXT_VERSION {
        return None;
    }
    let f = rest.strip_prefix("functions ")?;
    let (a, b) = f.split_once('.')?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

#[derive(Debug, Clone)]
enum Tok {
    Word(String),
    Str(String),
}

struct Fields {
    kind: String,
    line: usize,
    items: VecDeque<(String, Tok)>,
}

impl Fields {
    fn parse(raw: &str, line: usize) -> Result<Self> {
        let (kind, mut rest) = raw.split_once(' ').unwrap_or((raw, ""));
        if kind.is_empty() || !kind.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') {
            return Err(bad(line, "kind"));
        }
        let mut items = VecDeque::new();
        loop {
            rest = rest.trim_start_matches(' ');
            if rest.is_empty() {
                break;
            }
            let (name, after) = rest.split_once('=').ok_or_else(|| bad(line, "field"))?;
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            {
                return Err(bad(line, "name"));
            }
            if let Some(quoted) = after.strip_prefix('"') {
                let mut value = String::new();
                let mut chars = quoted.char_indices();
                let end = loop {
                    let Some((at, c)) = chars.next() else {
                        return Err(bad(line, "unterminated"));
                    };
                    match c {
                        '"' => break at,
                        '\\' => match chars.next().map(|(_, c)| c) {
                            Some('\\') => value.push('\\'),
                            Some('"') => value.push('"'),
                            Some('n') => value.push('\n'),
                            Some('r') => value.push('\r'),
                            Some('t') => value.push('\t'),
                            _ => return Err(bad(line, "escape")),
                        },
                        c if c.is_control() => return Err(bad(line, "control")),
                        c => value.push(c),
                    }
                };
                if value.len() > limits::STRING_BYTES {
                    return Err(
                        arc_base::Error::new(codes::FAMILY_LIMIT).with_arg("limit", "string_bytes")
                    );
                }
                items.push_back((name.to_owned(), Tok::Str(value)));
                rest = &quoted[end + 1..];
                if !(rest.is_empty() || rest.starts_with(' ')) {
                    return Err(bad(line, "after_string"));
                }
            } else {
                let (value, tail) = after.split_once(' ').unwrap_or((after, ""));
                if value.is_empty() || value.contains('"') {
                    return Err(bad(line, "value"));
                }
                items.push_back((name.to_owned(), Tok::Word(value.to_owned())));
                rest = tail;
            }
        }
        Ok(Self {
            kind: kind.to_owned(),
            line,
            items,
        })
    }

    fn peek(&self, name: &str) -> bool {
        self.items.front().is_some_and(|(n, _)| n == name)
    }

    fn take_tok(&mut self, name: &str) -> Result<Option<Tok>> {
        if self.peek(name) {
            Ok(self.items.pop_front().map(|(_, t)| t))
        } else {
            Ok(None)
        }
    }

    fn string(&mut self, name: &str) -> Result<String> {
        self.opt_string(name)?
            .ok_or_else(|| bad(self.line, &format!("missing:{name}")))
    }

    fn opt_string(&mut self, name: &str) -> Result<Option<String>> {
        Ok(self.take_tok(name)?.map(|t| match t {
            Tok::Word(w) | Tok::Str(w) => w,
        }))
    }

    fn word(&mut self, name: &str) -> Result<String> {
        self.string(name)
    }

    fn num(&mut self, name: &str) -> Result<Option<f64>> {
        match self.opt_string(name)? {
            None => Ok(None),
            Some(t) => t
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .map(Some)
                .ok_or_else(|| bad(self.line, &format!("number:{name}"))),
        }
    }

    fn coded<T>(&mut self, name: &str, parse: fn(&str) -> Option<T>) -> Result<T> {
        let w = self.word(name)?;
        parse(&w).ok_or_else(|| bad(self.line, &format!("value:{name}")))
    }

    fn id(&mut self, name: &str) -> Result<ObjectId> {
        let w = self.word(name)?;
        w.parse().map_err(|_| bad(self.line, "id"))
    }

    fn stored(&mut self, name: &str) -> Result<Option<Stored>> {
        let formula = format!("{name}_f");
        if self.peek(name) {
            return Ok(match self.items.pop_front() {
                Some((_, Tok::Str(t))) => Some(Stored::Literal(Evaluated::Text(t))),
                Some((_, Tok::Word(w))) if w == "true" || w == "false" => {
                    Some(Stored::Literal(Evaluated::Bool(w == "true")))
                }
                Some((_, Tok::Word(w))) => {
                    let v = w
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite())
                        .ok_or_else(|| bad(self.line, "number"))?;
                    Some(Stored::Literal(Evaluated::Number(v)))
                }
                None => None,
            });
        }
        if self.peek(&formula) {
            let text = self.string(&formula)?;
            let unit = match self.opt_string(&format!("{name}_u"))? {
                Some(sym) => Some(Unit::by_symbol(&sym).map_err(|_| bad(self.line, "unit"))?),
                None => None,
            };
            return Ok(Some(Stored::Formula(Formula::new(&text, unit)?)));
        }
        Ok(None)
    }

    fn visibility(&mut self) -> Result<Visibility> {
        let list = |s: Option<String>| -> Vec<String> {
            s.map(|s| s.split(',').map(str::to_owned).collect())
                .unwrap_or_default()
        };
        let details = list(self.opt_string("detail")?)
            .iter()
            .map(|d| Detail::parse(d).ok_or_else(|| bad(self.line, "detail")))
            .collect::<Result<Vec<_>>>()?;
        let views = list(self.opt_string("views")?)
            .iter()
            .map(|d| ViewKind::parse(d).ok_or_else(|| bad(self.line, "views")))
            .collect::<Result<Vec<_>>>()?;
        let when = match self.opt_string("when")? {
            Some(t) => Some(Formula::new(&t, None)?),
            None => None,
        };
        Ok(Visibility {
            details,
            views,
            when,
        })
    }

    fn finish(&self) -> Result<()> {
        if self.items.is_empty() {
            Ok(())
        } else {
            Err(bad(self.line, "unexpected_field"))
        }
    }
}

struct Reader {
    lines: VecDeque<Fields>,
}

impl Reader {
    fn next_if(&mut self, kind: &str) -> Option<Fields> {
        if self.lines.front().is_some_and(|l| l.kind == kind) {
            self.lines.pop_front()
        } else {
            None
        }
    }

    fn vertices(&mut self) -> Result<Vec<Vertex>> {
        let mut out = Vec::new();
        while let Some(mut l) = self.next_if("vertex") {
            if out.len() >= limits::VERTICES {
                return Err(arc_base::Error::new(codes::FAMILY_LIMIT).with_arg("limit", "vertices"));
            }
            let u = l.opt_string("u")?;
            let du = l.stored("du")?;
            let v = l.opt_string("v")?;
            let dv = l.stored("dv")?;
            let bulge = l.stored("bulge")?;
            l.finish()?;
            out.push(Vertex {
                u,
                du,
                v,
                dv,
                bulge,
            });
        }
        Ok(out)
    }

    #[allow(
        clippy::too_many_lines,
        clippy::cognitive_complexity,
        reason = "one pass over the parts of a definition, in order"
    )]
    fn definition(&mut self, functions: (u32, u32)) -> Result<Definition> {
        let mut l = self
            .next_if("definition")
            .ok_or_else(|| bad(2, "definition"))?;
        let header = Header {
            id: l.id("id")?,
            version: {
                let w = l.word("version")?;
                Version::parse(&w).ok_or_else(|| bad(l.line, "version"))?
            },
            name: l.string("name")?,
            category: l.coded("category", Category::parse)?,
            hosting: l.coded("hosting", Hosting::parse)?,
            flags: l
                .string("flags")?
                .split(',')
                .filter(|f| !f.is_empty())
                .map(str::to_owned)
                .collect::<BTreeSet<_>>(),
            ifc_entity: l.string("ifc_entity")?,
            ifc_type: l.string("ifc_type")?,
            functions,
            source: l.string("source")?,
            license: l.string("license")?,
        };
        l.finish()?;
        let mut def = Definition {
            header,
            parameters: Vec::new(),
            types: Vec::new(),
            planes: Vec::new(),
            profiles: Vec::new(),
            forms: Vec::new(),
            symbols: Vec::new(),
            tests: Vec::new(),
        };
        while let Some(mut l) = self.next_if("parameter") {
            let key = l.word("key")?;
            let id = l.id("id")?;
            let name = l.string("name")?;
            let data = l.coded("data", data_type_parse)?;
            let scope = l.coded("scope", Scope::parse)?;
            let role = l.coded("role", Role::parse)?;
            let group = l.string("group")?;
            let order = l
                .word("order")?
                .parse::<i64>()
                .map_err(|_| bad(l.line, "order"))?;
            let minimum = l.num("min")?;
            let maximum = l.num("max")?;
            let allowed = l
                .opt_string("allowed")?
                .map(|a| a.split('|').map(str::to_owned).collect())
                .unwrap_or_default();
            let default = l.stored("default")?;
            l.finish()?;
            def.parameters.push(ParamDef {
                id,
                key,
                name,
                data,
                scope,
                role,
                group,
                order,
                default,
                minimum,
                maximum,
                allowed,
            });
        }
        while let Some(mut l) = self.next_if("type") {
            let id = l.id("id")?;
            let name = l.string("name")?;
            l.finish()?;
            let mut values = BTreeMap::new();
            while let Some(mut v) = self.next_if("value") {
                let key = v.word("param")?;
                let s = v.stored("v")?.ok_or_else(|| bad(v.line, "value"))?;
                v.finish()?;
                if values.insert(key, s).is_some() {
                    return Err(bad(v.line, "duplicate_value"));
                }
            }
            def.types.push(TypeDef { id, name, values });
        }
        while let Some(mut l) = self.next_if("plane") {
            let key = l.word("key")?;
            let name = l.string("name")?;
            let axis = l.coded("axis", Axis::parse)?;
            let role = l.coded("role", PlaneRole::parse)?;
            let strong = l.word("strong")? == "true";
            let offset = l.stored("offset")?.ok_or_else(|| bad(l.line, "offset"))?;
            l.finish()?;
            def.planes.push(PlaneDef {
                key,
                name,
                axis,
                offset,
                role,
                strong,
            });
        }
        while let Some(mut l) = self.next_if("profile") {
            let key = l.word("key")?;
            let axis = l.coded("axis", Axis::parse)?;
            l.finish()?;
            let vertices = self.vertices()?;
            def.profiles.push(ProfileDef {
                key,
                axis,
                vertices,
            });
        }
        while let Some(mut l) = self.next_if("form") {
            let form = FormDef {
                key: l.word("key")?,
                kind: l.coded("kind", FormKind::parse)?,
                profile: l.word("profile")?,
                from: l.word("from")?,
                to: l.opt_string("to")?,
                depth: l.stored("depth")?,
                material: l.string("material")?,
                subcategory: l.string("subcategory")?,
                visibility: l.visibility()?,
            };
            l.finish()?;
            def.forms.push(form);
        }
        while let Some(mut l) = self.next_if("symbol") {
            let key = l.word("key")?;
            let view = l.coded("view", ViewKind::parse)?;
            let kind = l.coded("kind", SymbolKind::parse)?;
            let subcategory = l.string("subcategory")?;
            let visibility = l.visibility()?;
            l.finish()?;
            let vertices = self.vertices()?;
            def.symbols.push(SymbolDef {
                key,
                view,
                kind,
                subcategory,
                visibility,
                vertices,
            });
        }
        while let Some(mut l) = self.next_if("test") {
            let mut t = TestCase {
                name: l.string("name")?,
                type_name: l.opt_string("type")?,
                volume: l.num("volume")?,
                tolerance: l.num("tolerance")?,
                solids: match l.opt_string("solids")? {
                    Some(s) => Some(s.parse().map_err(|_| bad(l.line, "solids"))?),
                    None => None,
                },
                fails: l.opt_string("fails")?,
                set: BTreeMap::new(),
            };
            l.finish()?;
            while let Some(mut v) = self.next_if("set") {
                let key = v.word("param")?;
                let s = v.stored("v")?.ok_or_else(|| bad(v.line, "value"))?;
                v.finish()?;
                t.set.insert(key, s);
            }
            def.tests.push(t);
        }
        if let Some(rest) = self.lines.front() {
            return Err(bad(rest.line, "unexpected_line"));
        }
        Ok(def)
    }
}

fn data_type_parse(name: &str) -> Option<DataType> {
    DataType::ALL.into_iter().find(|d| d.name() == name)
}
