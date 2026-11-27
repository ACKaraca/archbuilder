//! Families in a project (ADR 0009, section 3): the records a document keeps of a loaded
//! family (`archbuilder.family`: the published canonical text, version and checksum;
//! `archbuilder.family_type`: a named value set; `archbuilder.component`: a freestanding
//! instance), their readers, and the placed geometry of a component, derived on demand from the
//! definition, the type's and instance's values and the document variables.
//!
//! A document never refers to a library file: loading copies the definition's text.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, PoisonError};

use arc_base::id::Identifier;
use arc_base::math::Point3;
use arc_base::{Error, Limits, ObjectId, Result, Tolerance};
use arc_doc::{Object, Snapshot, Value};
use arc_geom::Polyhedron;

use super::def::{Definition, SymbolKind, Version};
use super::eval::{Evaluation, FormSolid, Inputs, Shown, evaluate};
use super::package::Published;
use crate::error::codes;
use crate::expr::Stored;
use crate::spatial;
use crate::types::{self, COMPONENT, FAMILY, FAMILY_TYPE, component, family, family_type};
use crate::variable;

/// The parsed definitions of the texts most recently read, by checksum, so that a viewport
/// drawing many components does not parse a text for each.
fn cached(text: &str, checksum: &str) -> Result<Arc<Definition>> {
    static CACHE: Mutex<Option<HashMap<String, Arc<Definition>>>> = Mutex::new(None);
    let key = format!("{checksum}:{}", super::text::checksum(text));
    let mut guard = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
    let map = guard.get_or_insert_with(HashMap::new);
    if let Some(found) = map.get(&key) {
        return Ok(Arc::clone(found));
    }
    let parsed = Arc::new(Definition::from_text(text)?);
    if map.len() >= 64 {
        map.clear();
    }
    map.insert(key, Arc::clone(&parsed));
    Ok(parsed)
}

/// A loaded family as the project stores it.
#[derive(Debug, Clone)]
pub struct Loaded {
    /// The record.
    pub record: ObjectId,
    /// The family identifier.
    pub family: ObjectId,
    /// The pinned version.
    pub version: Version,
    /// The name.
    pub name: String,
    /// The checksum as stored.
    pub checksum: String,
    /// The canonical text.
    pub text: String,
}

impl Loaded {
    /// The family stored in `object`.
    #[must_use]
    pub fn of(object: &Object) -> Option<Self> {
        let v = &object.values;
        let t = |at| types::text(v, at).unwrap_or_default().to_owned();
        Some(Self {
            record: object.id,
            family: t(family::ID).parse().ok()?,
            version: Version::parse(&t(family::VERSION))?,
            name: t(family::NAME),
            checksum: t(family::CHECKSUM),
            text: t(family::TEXT),
        })
    }

    /// The definition of the stored text.
    ///
    /// # Errors
    /// The errors of [`Definition::from_text`].
    pub fn definition(&self) -> Result<Arc<Definition>> {
        cached(&self.text, &self.checksum)
    }

    /// The record values of a published definition loaded as a family; the definition is the
    /// parsed text of `published`.
    #[must_use]
    pub fn values(published: &Published, def: &Definition) -> Vec<Option<Value>> {
        let s = |v: &str| Some(Value::String(v.to_owned()));
        let opt = |v: &str| (!v.is_empty()).then(|| Value::String(v.to_owned()));
        vec![
            s(&published.name),
            s(&published.family.to_string()),
            s(&published.version.to_string()),
            opt(def.header.category.name()),
            opt(def.header.hosting.name()),
            s(&published.checksum),
            opt(&def.header.ifc_entity),
            opt(&def.header.ifc_type),
            opt(&def.header.source),
            opt(&def.header.license),
            s(&published.text),
        ]
    }
}

/// Every loaded family of the document, by name and then identifier.
#[must_use]
pub fn families(snapshot: &Snapshot) -> Vec<Loaded> {
    let mut found: Vec<Loaded> = spatial::objects(snapshot, FAMILY)
        .iter()
        .filter_map(|o| Loaded::of(o))
        .collect();
    found.sort_by(|a, b| {
        (crate::names::fold(&a.name), a.record).cmp(&(crate::names::fold(&b.name), b.record))
    });
    found
}

/// A keyed list of values as stored on a family type or a component.
#[must_use]
pub fn values_of(object: &Object, at: usize) -> BTreeMap<String, Stored> {
    let Some(Some(Value::List(items))) = object.values.get(at) else {
        return BTreeMap::new();
    };
    items
        .iter()
        .take(crate::expr::MAX_DEFINITIONS)
        .filter_map(|item| match item {
            Value::Record(f) => match (f.first(), f.get(1)) {
                (Some(Some(Value::String(k))), Some(v)) => {
                    Some((k.clone(), Stored::from_value(v.as_ref())?))
                }
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// The field value of a keyed list; absent when empty.
#[must_use]
pub fn values_value(values: &BTreeMap<String, Stored>) -> Option<Value> {
    (!values.is_empty()).then(|| {
        Value::List(
            values
                .iter()
                .map(|(k, s)| {
                    Value::Record(vec![Some(Value::String(k.clone())), Some(s.to_value())])
                })
                .collect(),
        )
    })
}

/// A family type as the project stores it.
#[derive(Debug, Clone, PartialEq)]
pub struct FamilyType {
    /// The record.
    pub id: ObjectId,
    /// The name.
    pub name: String,
    /// The family record.
    pub family: ObjectId,
    /// The values, by parameter key.
    pub values: BTreeMap<String, Stored>,
}

impl FamilyType {
    /// The family type stored in `object`.
    #[must_use]
    pub fn of(object: &Object) -> Option<Self> {
        let Some(Some(Value::Reference(fam))) = object.values.get(family_type::FAMILY) else {
            return None;
        };
        Some(Self {
            id: object.id,
            name: types::text(&object.values, family_type::NAME)
                .unwrap_or_default()
                .to_owned(),
            family: *fam,
            values: values_of(object, family_type::VALUES),
        })
    }

    /// The record values.
    #[must_use]
    pub fn record(
        name: &str,
        family: ObjectId,
        values: &BTreeMap<String, Stored>,
    ) -> Vec<Option<Value>> {
        vec![
            Some(Value::String(name.trim().to_owned())),
            Some(Value::Reference(family)),
            values_value(values),
        ]
    }
}

/// The family types of the document, by family, then name.
#[must_use]
pub fn family_types(snapshot: &Snapshot) -> Vec<FamilyType> {
    let mut found: Vec<FamilyType> = spatial::objects(snapshot, FAMILY_TYPE)
        .iter()
        .filter_map(|o| FamilyType::of(o))
        .collect();
    found.sort_by(|a, b| {
        (a.family, crate::names::fold(&a.name), a.id).cmp(&(
            b.family,
            crate::names::fold(&b.name),
            b.id,
        ))
    });
    found
}

/// A freestanding instance of a family type.
#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    /// The record.
    pub id: ObjectId,
    /// The storey that owns it.
    pub storey: Option<ObjectId>,
    /// The mark.
    pub name: String,
    /// The family type.
    pub family_type: ObjectId,
    /// The distance above the storey's level.
    pub offset: f64,
    /// The insertion point.
    pub origin: [f64; 2],
    /// The rotation about the vertical axis, in radians.
    pub rotation: f64,
    /// Whether it is mirrored across its own left-right axis.
    pub mirror: bool,
    /// The instance's own values, by parameter key.
    pub values: BTreeMap<String, Stored>,
}

impl Component {
    /// The component stored in `object`.
    #[must_use]
    pub fn of(object: &Object) -> Option<Self> {
        let v = &object.values;
        let Some(Some(Value::Reference(ty))) = v.get(component::FAMILY_TYPE) else {
            return None;
        };
        let m = |at| types::meters(v, at).unwrap_or(0.0);
        Some(Self {
            id: object.id,
            storey: object.header.owner,
            name: types::text(v, component::NAME)
                .unwrap_or_default()
                .to_owned(),
            family_type: *ty,
            offset: m(component::OFFSET),
            origin: [m(component::X), m(component::Y)],
            rotation: match v.get(component::ROTATION) {
                Some(Some(Value::Quantity(q))) => q.value(),
                _ => 0.0,
            },
            mirror: matches!(v.get(component::MIRROR), Some(Some(Value::Bool(true)))),
            values: values_of(object, component::VALUES),
        })
    }

    /// The component under the plane motion `xf`: the insertion point moved, the heading turned
    /// and the mirror flag toggled by a reflection. The placement's second column (the local
    /// y axis, which a mirror leaves alone) gives the heading.
    #[must_use]
    pub fn moved(&self, xf: &crate::transform::Xform) -> Self {
        let (s, c) = crate::transform::sin_cos(self.rotation);
        let origin = xf.point(self.origin);
        let side = xf.point([self.origin[0] - s, self.origin[1] + c]);
        Self {
            origin,
            rotation: (side[1] - origin[1]).atan2(side[0] - origin[0])
                - std::f64::consts::FRAC_PI_2,
            mirror: self.mirror != xf.reflects(),
            ..self.clone()
        }
    }

    /// The record values.
    #[must_use]
    pub fn record(&self) -> Vec<Option<Value>> {
        let q = |m: f64| Some(types::length(m));
        vec![
            (!self.name.is_empty()).then(|| Value::String(self.name.clone())),
            Some(Value::Reference(self.family_type)),
            q(self.offset),
            q(self.origin[0]),
            q(self.origin[1]),
            Some(Value::Quantity(arc_base::Quantity::new(
                self.rotation,
                arc_base::Dimension::ANGLE,
            ))),
            Some(Value::Bool(self.mirror)),
            values_value(&self.values),
            // The project parameter values of the element (`parameters`), none yet.
            None,
        ]
    }
}

/// Every component of the document.
#[must_use]
pub fn components(snapshot: &Snapshot) -> Vec<Component> {
    spatial::objects(snapshot, COMPONENT)
        .iter()
        .filter_map(|o| Component::of(o))
        .collect()
}

/// A placed solid.
#[derive(Debug, Clone)]
pub struct PlacedSolid {
    /// The form.
    pub form: String,
    /// The solid in project coordinates.
    pub solid: Polyhedron,
    /// The role of each face.
    pub roles: Vec<String>,
    /// The material slot.
    pub material: String,
    /// The subcategory.
    pub subcategory: String,
    /// Visibility.
    pub shown: Shown,
}

/// A placed symbolic element, in plan coordinates (project meters).
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedSymbol {
    /// The key.
    pub key: String,
    /// Line, region or mask.
    pub kind: SymbolKind,
    /// The subcategory.
    pub subcategory: String,
    /// The points; closed for regions and masks.
    pub points: Vec<[f64; 2]>,
    /// Visibility.
    pub shown: Shown,
}

/// A shared nested child of a component: an element derived from the component, with the
/// identifier derived from the component's and the nested-use key ([`super::nested_id`]).
#[derive(Debug, Clone)]
pub struct PlacedShared {
    /// The derived identifier.
    pub id: ObjectId,
    /// The nested-use key, `key#i` for an array copy.
    pub key: String,
    /// The child family's identifier.
    pub family: ObjectId,
    /// The child's type name.
    pub type_name: Option<String>,
    /// The child's solids in project coordinates.
    pub solids: Vec<PlacedSolid>,
}

/// A component's geometry: its solids and its plan symbols, placed, with the evaluation.
#[derive(Debug, Clone)]
pub struct Placed {
    /// The solids.
    pub solids: Vec<PlacedSolid>,
    /// The shared nested children, each its own element.
    pub shared: Vec<PlacedShared>,
    /// The symbols of the plan view.
    pub plan: Vec<PlacedSymbol>,
    /// The evaluation in the family's frame.
    pub evaluation: Evaluation,
}

/// The family record, definition and type of a component, with the type's values.
#[derive(Debug, Clone)]
pub struct Resolved {
    /// The loaded family.
    pub family: Loaded,
    /// Its definition.
    pub definition: Arc<Definition>,
    /// The family type.
    pub ty: FamilyType,
}

/// What `component` refers to.
///
/// # Errors
/// `ab.family.evaluation_failed` (reason `type`, `family` or the text's error) when the type or
/// family is missing or the text no longer reads.
pub fn resolve(snapshot: &Snapshot, component: &Component) -> Result<Resolved> {
    let fail = |reason: &str| {
        Error::new(codes::FAMILY_EVALUATION)
            .with_arg("instance", component.name.as_str())
            .with_arg("reason", reason)
    };
    let ty = spatial::get(snapshot, component.family_type)
        .and_then(|o| FamilyType::of(&o))
        .ok_or_else(|| fail("type"))?;
    let family = spatial::get(snapshot, ty.family)
        .and_then(|o| Loaded::of(&o))
        .ok_or_else(|| fail("family"))?;
    let definition = family.definition().map_err(|e| fail(e.code()))?;
    Ok(Resolved {
        family,
        definition,
        ty,
    })
}

/// Evaluates the definition for `ty` and `instance`, reading the document's variables.
///
/// # Errors
/// The errors of the variables' evaluation and of [`evaluate`].
pub fn evaluate_in(
    snapshot: &Snapshot,
    definition: &Definition,
    ty: &FamilyType,
    instance: &BTreeMap<String, Stored>,
) -> Result<Evaluation> {
    let variables = variable::evaluate(snapshot)?;
    let lookup = |name: &str| variable::lookup(&variables, name);
    // A nested use pins a family identifier and a version; the project holds the copy it loaded.
    let loaded = families(snapshot);
    let children = |id: ObjectId, version: Version| -> Option<Arc<Definition>> {
        loaded
            .iter()
            .find(|f| f.family == id && f.version == version)
            .and_then(|f| f.definition().ok())
    };
    // The type's values lie over the definition's, as a test case's do over a type's.
    let mut d = definition.clone();
    let name = "\u{1}type";
    d.types.push(super::def::TypeDef {
        id: ty.family,
        name: name.to_owned(),
        values: ty.values.clone(),
    });
    evaluate(
        &d,
        &Inputs {
            type_name: Some(name),
            instance,
            variables: &lookup,
            children: &children,
        },
    )
}

/// The geometry of `component` in project coordinates.
///
/// # Errors
/// `ab.family.evaluation_failed` naming the component and the reason, the evaluation's own
/// error (naming the form) when it fails, and the solid kernel's errors for a placement it
/// cannot make.
pub fn place(snapshot: &Snapshot, component: &Component) -> Result<Placed> {
    let r = resolve(snapshot, component)?;
    let evaluation = evaluate_in(snapshot, &r.definition, &r.ty, &component.values)?;
    let elevation = component
        .storey
        .and_then(|s| spatial::get(snapshot, s))
        .map_or(0.0, |s| spatial::storey_elevation(snapshot, &s));
    let (s, c) = component.rotation.sin_cos();
    let m = if component.mirror { -1.0 } else { 1.0 };
    let rows = [
        [m * c, -s, 0.0, component.origin[0]],
        [m * s, c, 0.0, component.origin[1]],
        [0.0, 0.0, 1.0, elevation + component.offset],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let transform = super::eval::to_transform(&rows);
    let (tolerance, limits) = (Tolerance::DEFAULT, Limits::DEFAULT);
    let mut solids = Vec::with_capacity(evaluation.solids.len());
    for FormSolid {
        form,
        solid,
        roles,
        material,
        subcategory,
        shown,
    } in &evaluation.solids
    {
        solids.push(PlacedSolid {
            form: form.clone(),
            solid: solid.transformed(&transform, &tolerance, &limits)?,
            roles: roles.clone(),
            material: material.clone(),
            subcategory: subcategory.clone(),
            shown: shown.clone(),
        });
    }
    let plan = evaluation
        .symbols
        .iter()
        .filter(|s| s.view == super::def::ViewKind::Plan)
        .map(|s| PlacedSymbol {
            key: s.key.clone(),
            kind: s.kind,
            subcategory: s.subcategory.clone(),
            points: s
                .points
                .iter()
                .map(|p| {
                    let q = transform.transform_point(&Point3::new(p[0], p[1], 0.0));
                    [q.x, q.y]
                })
                .collect(),
            shown: s.shown.clone(),
        })
        .collect();
    let mut shared = Vec::with_capacity(evaluation.shared.len());
    for child in &evaluation.shared {
        let t = super::eval::to_transform(&super::eval::multiply(&rows, &child.matrix));
        let mut parts = Vec::with_capacity(child.evaluation.solids.len());
        for p in &child.evaluation.solids {
            parts.push(PlacedSolid {
                form: p.form.clone(),
                solid: p.solid.transformed(&t, &tolerance, &limits)?,
                roles: p.roles.clone(),
                material: p.material.clone(),
                subcategory: p.subcategory.clone(),
                shown: p.shown.clone(),
            });
        }
        shared.push(PlacedShared {
            id: ObjectId::from_u128(super::nested_id(component.id, &child.key)),
            key: child.key.clone(),
            family: child.family,
            type_name: child.type_name.clone(),
            solids: parts,
        });
    }
    Ok(Placed {
        solids,
        shared,
        plan,
        evaluation,
    })
}

/// The components that use a type of `family`.
#[must_use]
pub fn components_of(snapshot: &Snapshot, family: ObjectId) -> Vec<Component> {
    let tys: Vec<ObjectId> = family_types(snapshot)
        .iter()
        .filter(|t| t.family == family)
        .map(|t| t.id)
        .collect();
    components(snapshot)
        .into_iter()
        .filter(|c| tys.contains(&c.family_type))
        .collect()
}

/// Checks that every component evaluates and is placed; returns the first failure, with the
/// arguments `instance` (the mark or identifier) and `reason`.
///
/// # Errors
/// `ab.family.evaluation_failed` for the first component that does not evaluate, with the
/// arguments `instance`, `reason` and `count` (how many fail).
pub fn check_components(snapshot: &Snapshot, only: Option<&[ObjectId]>) -> Result<()> {
    let mut first: Option<Error> = None;
    let mut count = 0_usize;
    for c in components(snapshot)
        .iter()
        .filter(|c| only.is_none_or(|o| o.contains(&c.id)))
    {
        if let Err(e) = place(snapshot, c) {
            count += 1;
            first.get_or_insert_with(|| {
                let id = if c.name.is_empty() {
                    c.id.to_string()
                } else {
                    c.name.clone()
                };
                let form = match e.arguments().get("form") {
                    Some(arc_base::error::Value::Text(t)) => t.clone(),
                    _ => String::new(),
                };
                Error::new(codes::FAMILY_EVALUATION)
                    .with_arg("instance", id)
                    .with_arg("reason", e.code())
                    .with_arg("form", form)
            });
        }
    }
    match first {
        Some(e) => Err(e.with_arg("count", i64::try_from(count).unwrap_or(i64::MAX))),
        None => Ok(()),
    }
}

/// The plan footprint of a component: the rectangle around its placed solids, else its insertion
/// point alone. Empty when the component does not evaluate.
#[must_use]
pub fn outline(snapshot: &Snapshot, component: &Component) -> Vec<[f64; 2]> {
    let Ok(placed) = place(snapshot, component) else {
        return Vec::new();
    };
    let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for s in &placed.solids {
        let b = s.solid.bounding_box();
        for i in 0..2 {
            lo[i] = lo[i].min(b.min()[i]);
            hi[i] = hi[i].max(b.max()[i]);
        }
    }
    if lo[0].is_finite() {
        vec![
            [lo[0], lo[1]],
            [hi[0], lo[1]],
            [hi[0], hi[1]],
            [lo[0], hi[1]],
        ]
    } else {
        vec![component.origin]
    }
}

// ----- hosted family instances ------------------------------------------------------------------

/// The element types that can host a family of the hosting kind of `def`, by type key.
#[must_use]
pub fn host_keys(def: &Definition) -> &'static [&'static str] {
    use super::def::Hosting;
    use crate::types::{CEILING, FLOOR, ROOF, ROOF_EXTRUSION, WALL};
    match def.header.hosting {
        Hosting::Wall => &[WALL],
        Hosting::Floor => &[FLOOR],
        Hosting::Ceiling => &[CEILING],
        Hosting::Roof => &[ROOF, ROOF_EXTRUSION],
        Hosting::Face => &[WALL, FLOOR, CEILING, ROOF, ROOF_EXTRUSION],
        Hosting::None | Hosting::WorkPlane => &[],
    }
}

/// Whether a family may be placed in a host of type `host_key`.
///
/// # Errors
/// `ab.family.invalid` with the reason `hosting` (a door or window family that is not hosted by a
/// wall, or a family that is not hosted), `host_kind` (the host is not of a kind the family's
/// hosting allows) or `host_unsupported` (the host kind is allowed, but only walls carry
/// geometry of hosted instances so far).
pub fn check_host(def: &Definition, host_key: &str) -> Result<()> {
    use super::def::{Category, Hosting};
    let refuse = |reason: &str| {
        Error::new(codes::FAMILY_INVALID)
            .with_arg("part", "host")
            .with_arg("reason", reason)
    };
    let door_or_window = matches!(def.header.category, Category::Door | Category::Window);
    if def.header.hosting == Hosting::None
        || (door_or_window && def.header.hosting != Hosting::Wall)
    {
        return Err(refuse("hosting"));
    }
    if !host_keys(def).contains(&host_key) {
        return Err(refuse("host_kind"));
    }
    if host_key != crate::types::WALL {
        return Err(refuse("host_unsupported"));
    }
    Ok(())
}

/// A hosted instance of a family type: its host (the header owner), the insertion point along the
/// host's baseline and above its base, the hand and the facing.
#[derive(Debug, Clone, PartialEq)]
pub struct FamilyInsert {
    /// The record.
    pub id: ObjectId,
    /// The host.
    pub host: Option<ObjectId>,
    /// The mark.
    pub name: String,
    /// The family type.
    pub family_type: ObjectId,
    /// The distance from the baseline's start to the insertion point.
    pub distance: f64,
    /// The height of the insertion point above the host's base.
    pub offset: f64,
    /// The hand: right mirrors the family across its own left-right axis.
    pub hand: crate::insert::Hand,
    /// The side the family faces: interior turns it half a turn.
    pub facing: crate::insert::Facing,
    /// The instance's own values, by parameter key.
    pub values: BTreeMap<String, Stored>,
}

impl FamilyInsert {
    /// The hosted instance stored in `object`.
    #[must_use]
    pub fn of(object: &Object) -> Option<Self> {
        use crate::types::family_insert as f;
        let v = &object.values;
        let Some(Some(Value::Reference(ty))) = v.get(f::FAMILY_TYPE) else {
            return None;
        };
        Some(Self {
            id: object.id,
            host: object.header.owner,
            name: types::text(v, f::NAME).unwrap_or_default().to_owned(),
            family_type: *ty,
            distance: types::meters(v, f::DISTANCE)?,
            offset: types::meters(v, f::OFFSET).unwrap_or(0.0),
            hand: crate::insert::Hand::of_field(v, f::HAND),
            facing: crate::insert::Facing::of_field(v, f::FACING),
            values: values_of(object, f::VALUES),
        })
    }

    /// The record values.
    #[must_use]
    pub fn record(&self) -> Vec<Option<Value>> {
        vec![
            (!self.name.is_empty()).then(|| Value::String(self.name.clone())),
            Some(Value::Reference(self.family_type)),
            Some(types::length(self.distance)),
            Some(types::length(self.offset)),
            Some(Value::Enum(self.hand.code())),
            Some(Value::Enum(self.facing.code())),
            values_value(&self.values),
            None,
        ]
    }

    /// The direction of the family's x axis along the host's baseline: `-1` for a family that
    /// faces the interior (turned half a turn) or is mirrored for the right hand, both reversing
    /// the x axis seen along the baseline.
    #[must_use]
    pub fn along(&self) -> f64 {
        let turned = if self.facing == crate::insert::Facing::Interior {
            -1.0
        } else {
            1.0
        };
        let mirrored = if self.hand == crate::insert::Hand::Right {
            -1.0
        } else {
            1.0
        };
        turned * mirrored
    }
}

/// The family, definition and type of a hosted instance.
///
/// # Errors
/// `ab.family.evaluation_failed` when the type or family is missing or the text no longer reads.
pub fn resolve_insert(snapshot: &Snapshot, insert: &FamilyInsert) -> Result<Resolved> {
    let component = Component {
        id: insert.id,
        storey: None,
        name: insert.name.clone(),
        family_type: insert.family_type,
        offset: 0.0,
        origin: [0.0; 2],
        rotation: 0.0,
        mirror: false,
        values: BTreeMap::new(),
    };
    resolve(snapshot, &component)
}

/// The opening a hosted instance cuts in its wall: a rectangle, the box of its host-cutting
/// voids, with the insertion point at the instance's distance. `None` when the family cuts
/// nothing or does not evaluate.
#[must_use]
pub fn opening_of(snapshot: &Snapshot, insert: &FamilyInsert) -> Option<crate::insert::Insert> {
    use crate::insert::{Insert, Kind, Operation};
    let r = resolve_insert(snapshot, insert).ok()?;
    let ev = evaluate_in(snapshot, &r.definition, &r.ty, &insert.values).ok()?;
    let first = ev.host_cuts.first()?;
    let (mut lo, mut hi) = (first.min, first.max);
    for c in &ev.host_cuts {
        for i in 0..3 {
            lo[i] = lo[i].min(c.min[i]);
            hi[i] = hi[i].max(c.max[i]);
        }
    }
    let centre = (lo[0] + hi[0]) / 2.0;
    Some(Insert {
        kind: Kind::Opening,
        distance: insert.distance + insert.along() * centre,
        width: hi[0] - lo[0],
        height: hi[2] - lo[2],
        sill: insert.offset + lo[2],
        frame_width: 0.0,
        frame_depth: 0.0,
        operation: Operation::default(),
        hand: insert.hand,
        facing: insert.facing,
        rise: 0.0,
        profile: None,
    })
}

/// The opening a hosted instance object cuts, read from its record; `None` when the family cuts
/// nothing or does not evaluate.
#[must_use]
pub fn insert_of(snapshot: &Snapshot, object: &Object) -> Option<crate::insert::Insert> {
    FamilyInsert::of(object).and_then(|i| opening_of(snapshot, &i))
}

/// The rules a hosted instance breaks: those of the opening it cuts, and a family that no longer
/// evaluates.
#[must_use]
pub fn insert_problems(snapshot: &Snapshot, object: &Object) -> Vec<Error> {
    let Some(insert) = FamilyInsert::of(object) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    match resolve_insert(snapshot, &insert) {
        Err(e) => return vec![e],
        Ok(r) => {
            if let Err(e) = evaluate_in(snapshot, &r.definition, &r.ty, &insert.values) {
                out.push(
                    Error::new(codes::FAMILY_EVALUATION)
                        .with_arg("instance", insert.name.as_str())
                        .with_arg("reason", e.code()),
                );
            }
        }
    }
    if let Some(item) = opening_of(snapshot, &insert) {
        out.extend(crate::insert::problems_of(snapshot, object, item));
    }
    out
}
