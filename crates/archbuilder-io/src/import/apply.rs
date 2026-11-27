//! Writing the plan of an import into the document: objects, the identity map and the data kept
//! beside each object, all in the transaction of the calling command.

use std::collections::{BTreeMap, BTreeSet};

use arc_base::id::Identifier;
use arc_base::{Error, ObjectId, Result};
use arc_command::Context;
use arc_doc::source_map::SourceMap;
use arc_doc::{GeometryBytes, Header, Snapshot, StoredForeign, Value};
use archbuilder_model::element_type::{
    self as et, Exposure, Function, Layer, Layers, WallType, Wrap,
};
use archbuilder_model::floor::{Floor, Vertex};
use archbuilder_model::insert::{Facing, Hand, Operation};
use archbuilder_model::types::{
    BUILDING, CEILING, DOOR, ELEVATION_BASE_PROJECT, FLOOR, FOREIGN_ELEMENT, GRID, LEVEL,
    PLAN_VIEW, PROJECT, ROOM, SELECTION_SET, SITE, STOREY, WALL, WINDOW, foreign_element, project,
};
use archbuilder_model::view::PlanView;
use archbuilder_model::wall::{JoinStyle, LocationLine};
use archbuilder_model::{spatial, types};

use super::data::{FORMAT, KIND_ELEMENT, KIND_REPORT, KIND_SOURCE, Kept};
use super::plan::{
    BuildingPlan, ForeignPlan, GridPlan, InsertPlan, Item, Key, LayeredSpec, Plan, RoomPlan,
    SlabPlan, StoreyPlan, WallPlan,
};

/// What an import made.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Applied {
    /// The objects made, by plan key.
    pub ids: BTreeMap<Key, ObjectId>,
    /// The buildings, storeys and types made.
    pub buildings: usize,
    /// The storeys made.
    pub storeys: usize,
    /// The types made.
    pub types: usize,
}

struct Writer<'c, 'a> {
    ctx: &'c mut Context<'a>,
    file_name: String,
    schema: String,
    applied: Applied,
    wall_types: BTreeMap<String, ObjectId>,
    floor_types: BTreeMap<String, ObjectId>,
    ceiling_types: BTreeMap<String, ObjectId>,
    insert_types: BTreeMap<String, ObjectId>,
    taken: BTreeSet<(&'static str, String)>,
    used: BTreeSet<(String, ObjectId, String)>,
    claimed: BTreeSet<ObjectId>,
    map: SourceMap,
    project: ObjectId,
}

const ALPHABET: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$";

/// The 128 bits a `GlobalId` encodes; `None` for text that is not one.
#[must_use]
pub(crate) fn decode_global_id(text: &str) -> Option<u128> {
    if text.len() != 22 {
        return None;
    }
    let mut value: u128 = 0;
    for (at, c) in text.chars().enumerate() {
        let digit = u128::try_from(ALPHABET.find(c)?).ok()?;
        if at == 0 && digit > 3 {
            return None;
        }
        value = (value << 6) | digit;
    }
    Some(value)
}

fn blank(snapshot: &Snapshot, key: &str) -> Vec<Option<Value>> {
    snapshot
        .registry()
        .get(key)
        .map(|d| d.schema.fields.iter().map(|f| f.default.clone()).collect())
        .unwrap_or_default()
}

fn set(values: &mut [Option<Value>], at: usize, value: Value) {
    if let Some(slot) = values.get_mut(at) {
        *slot = Some(value);
    }
}

fn text(s: &str) -> Value {
    Value::String(s.trim().chars().take(250).collect())
}

fn function_of(i: usize, core: usize) -> Function {
    if i == core {
        Function::Structure
    } else {
        Function::Finish
    }
}

impl Writer<'_, '_> {
    fn create(
        &mut self,
        key: &str,
        owner: Option<ObjectId>,
        values: Vec<Option<Value>>,
    ) -> Result<ObjectId> {
        let id = self.fresh()?;
        self.create_as(key, id, owner, values)?;
        Ok(id)
    }

    /// A new identifier that no object of the document and no object of this import has.
    fn fresh(&mut self) -> Result<ObjectId> {
        for _ in 0..64 {
            let id: ObjectId = self.ctx.new_id();
            if !self.claimed.contains(&id) && self.ctx.txn()?.snapshot().resolve(id).is_err() {
                self.claimed.insert(id);
                return Ok(id);
            }
        }
        Err(Error::new(arc_command::error::codes::PARAMS_INVALID))
    }

    /// Creates the object of a source product. Its identifier is the one its `GlobalId` encodes,
    /// so that the export writes the same `GlobalId` again (the document's identifiers and the
    /// file's `GlobalId`s are one encoding); a `GlobalId` that is malformed, repeated in the
    /// file or taken by another object gets a new identifier.
    fn create_for(
        &mut self,
        kept: &Kept,
        key: &str,
        owner: Option<ObjectId>,
        values: Vec<Option<Value>>,
    ) -> Result<ObjectId> {
        let wanted = decode_global_id(&kept.global_id).map(ObjectId::from_u128);
        let id = match wanted {
            Some(id)
                if !self.claimed.contains(&id)
                    && self.ctx.txn()?.snapshot().resolve(id).is_err() =>
            {
                id
            }
            _ => self.fresh()?,
        };
        self.claimed.insert(id);
        self.create_as(key, id, owner, values)?;
        Ok(id)
    }

    fn create_as(
        &mut self,
        key: &str,
        id: ObjectId,
        owner: Option<ObjectId>,
        values: Vec<Option<Value>>,
    ) -> Result<()> {
        let header = Header {
            owner,
            ..Header::default()
        };
        // Values made for an older layout get the fields appended since then at their defaults.
        let mut values = values;
        let defaults = blank(self.ctx.txn()?.snapshot(), key);
        if values.len() < defaults.len() {
            values.extend(defaults.into_iter().skip(values.len()));
        }
        self.ctx.txn()?.create(key, id, header, values)?;
        Ok(())
    }

    fn blank(&mut self, key: &str) -> Result<Vec<Option<Value>>> {
        Ok(blank(self.ctx.txn()?.snapshot(), key))
    }

    /// A name not used yet by a type of `kind`.
    fn free_name(&mut self, kind: &'static str, wanted: &str) -> Result<String> {
        let snapshot = self.ctx.txn()?.snapshot().clone();
        let used = |name: &str| {
            self.taken.contains(&(kind, name.to_lowercase()))
                || spatial::objects(&snapshot, kind)
                    .iter()
                    .any(|t| et::name(t).eq_ignore_ascii_case(name))
        };
        let base: String = wanted.trim().chars().take(240).collect();
        let base = if base.is_empty() {
            "Imported".to_owned()
        } else {
            base
        };
        let mut name = base.clone();
        let mut n = 2;
        while used(&name) {
            name = format!("{base} ({n})");
            n += 1;
        }
        self.taken.insert((kind, name.to_lowercase()));
        Ok(name)
    }

    /// `name` made unique among the objects of `key` that `owner` holds, as the model requires.
    fn unique(&mut self, key: &str, owner: ObjectId, name: &str) -> Result<String> {
        let snapshot = self.ctx.txn()?.snapshot().clone();
        let seed = |me: &mut Self| {
            if !me
                .used
                .iter()
                .any(|(k, o, n)| k == key && *o == owner && n.is_empty())
            {
                for o in spatial::objects(&snapshot, key) {
                    if o.header.owner == Some(owner) {
                        me.used.insert((
                            key.to_owned(),
                            owner,
                            archbuilder_model::names::fold(
                                types::text(&o.values, 0).unwrap_or_default(),
                            ),
                        ));
                    }
                }
                me.used.insert((key.to_owned(), owner, String::new()));
            }
        };
        seed(self);
        let base: String = name.trim().chars().take(240).collect();
        let base = if base.is_empty() {
            "Imported".to_owned()
        } else {
            base
        };
        let mut candidate = base.clone();
        let mut n = 2;
        while self.used.contains(&(
            key.to_owned(),
            owner,
            archbuilder_model::names::fold(&candidate),
        )) {
            candidate = format!("{base} ({n})");
            n += 1;
        }
        self.used.insert((
            key.to_owned(),
            owner,
            archbuilder_model::names::fold(&candidate),
        ));
        Ok(candidate)
    }

    fn layers(spec: &LayeredSpec, wall: bool) -> Layers {
        Layers {
            layers: spec
                .layers
                .iter()
                .enumerate()
                .map(|(i, (material, t))| {
                    Layer::new(
                        if wall {
                            function_of(i, spec.core)
                        } else {
                            Function::Structure
                        },
                        material,
                        *t,
                    )
                })
                .collect(),
            core: (spec.core, spec.core + 1),
        }
    }

    fn wall_type(&mut self, spec: &LayeredSpec) -> Result<ObjectId> {
        let signature = format!("{spec:?}");
        if let Some(id) = self.wall_types.get(&signature) {
            return Ok(*id);
        }
        let name = self.free_name(types::WALL_TYPE, &spec.name)?;
        let values = et::wall_type_values(
            &name,
            &WallType {
                layers: Self::layers(spec, true),
                exposure: if spec.external {
                    Exposure::Exterior
                } else {
                    Exposure::Interior
                },
                load_bearing: spec.load_bearing,
                room_bounding: true,
                wrap_ends: Wrap::None,
                wrap_inserts: Wrap::None,
            },
        );
        let id = self.create(types::WALL_TYPE, Some(self.project), values)?;
        self.applied.types += 1;
        self.wall_types.insert(signature, id);
        Ok(id)
    }

    fn floor_type(&mut self, spec: &LayeredSpec, ceiling: bool) -> Result<ObjectId> {
        let signature = format!("{spec:?}");
        let cache = if ceiling {
            &self.ceiling_types
        } else {
            &self.floor_types
        };
        if let Some(id) = cache.get(&signature) {
            return Ok(*id);
        }
        let layers = Self::layers(spec, false);
        let (key, values) = if ceiling {
            let name = self.free_name(types::CEILING_TYPE, &spec.name)?;
            (
                types::CEILING_TYPE,
                et::ceiling_type_values(
                    &name,
                    &et::CeilingType {
                        layers,
                        height_offset: types::DEFAULT_CEILING_HEIGHT,
                        grid: None,
                    },
                ),
            )
        } else {
            let name = self.free_name(types::FLOOR_TYPE, &spec.name)?;
            (types::FLOOR_TYPE, et::floor_type_values(&name, &layers))
        };
        let id = self.create(key, Some(self.project), values)?;
        self.applied.types += 1;
        if ceiling {
            self.ceiling_types.insert(signature, id);
        } else {
            self.floor_types.insert(signature, id);
        }
        Ok(id)
    }

    fn insert_type(&mut self, plan: &InsertPlan) -> Result<ObjectId> {
        let signature = format!("{}|{:?}", plan.window, plan.spec);
        if let Some(id) = self.insert_types.get(&signature) {
            return Ok(*id);
        }
        let key = if plan.window {
            types::WINDOW_TYPE
        } else {
            types::DOOR_TYPE
        };
        let wanted = format!(
            "{} {}x{}",
            plan.spec.name,
            (plan.spec.width * 1000.0).round(),
            (plan.spec.height * 1000.0).round()
        );
        let first = self.free_name(key, &plan.spec.name);
        // The plain name when it is free, else the name with the size.
        let name = match first {
            Ok(n) if n == plan.spec.name.trim() => n,
            Ok(_) | Err(_) => self.free_name(key, &wanted)?,
        };
        let t = et::InsertType {
            width: plan.spec.width,
            height: plan.spec.height,
            frame_width: 0.05,
            frame_depth: 0.1,
            operation: Operation::from_code(plan.spec.operation),
            sill: plan.sill,
        };
        let id = self.create(key, Some(self.project), t.values(&name))?;
        self.applied.types += 1;
        self.insert_types.insert(signature, id);
        Ok(id)
    }

    /// Records the identity and the kept data of an object made from a source product.
    fn identify(&mut self, id: ObjectId, kept: &Kept) -> Result<()> {
        if kept.global_id.is_empty() {
            return Ok(());
        }
        let file_name = self.file_name.clone();
        let txn = self.ctx.txn()?;
        self.map
            .bind(txn, FORMAT, &file_name, &kept.global_id, id)?;
        txn.add_foreign(StoredForeign {
            source_format: FORMAT.to_owned(),
            source_version: Some(self.schema.clone()),
            source_id: Some(kept.global_id.clone()),
            object: Some(id.as_u128().to_be_bytes().to_vec()),
            record_kind: KIND_ELEMENT.to_owned(),
            data: kept.to_bytes(),
            ..StoredForeign::default()
        })?;
        Ok(())
    }

    fn storey(&mut self, building: ObjectId, plan: &StoreyPlan) -> Result<ObjectId> {
        let name = {
            let used: BTreeSet<String> = spatial::storeys(self.ctx.txn()?.snapshot(), building)
                .iter()
                .map(|e| {
                    types::text(&e.storey.values, types::storey::NAME)
                        .unwrap_or("")
                        .to_lowercase()
                })
                .collect();
            let mut name = plan.name.trim().to_owned();
            let mut n = 2;
            while used.contains(&name.to_lowercase()) {
                name = format!("{} ({n})", plan.name.trim());
                n += 1;
            }
            name
        };
        let mut level = self.blank(LEVEL)?;
        set(&mut level, types::level::NAME, text(&name));
        set(
            &mut level,
            types::level::ELEVATION,
            types::length(plan.elevation),
        );
        set(
            &mut level,
            types::level::ELEVATION_BASE,
            Value::Enum(ELEVATION_BASE_PROJECT),
        );
        let level = self.create(LEVEL, Some(building), level)?;
        let mut values = self.blank(STOREY)?;
        set(&mut values, types::storey::NAME, text(&name));
        set(&mut values, types::storey::LEVEL, Value::Reference(level));
        let storey = self.create_for(&plan.kept, STOREY, Some(building), values)?;
        self.create(PLAN_VIEW, Some(storey), PlanView::new(&name).values())?;
        self.applied.storeys += 1;
        self.identify(storey, &plan.kept)?;
        Ok(storey)
    }

    fn wall(&mut self, storey: ObjectId, w: &WallPlan) -> Result<ObjectId> {
        let wall_type = self.wall_type(&w.spec)?;
        let mut v = self.blank(WALL)?;
        let w_ = types::wall::NAME;
        let wall_name = self.unique(WALL, storey, &w.name)?;
        set(&mut v, w_, text(&wall_name));
        set(&mut v, types::wall::START_X, types::length(w.start[0]));
        set(&mut v, types::wall::START_Y, types::length(w.start[1]));
        set(&mut v, types::wall::END_X, types::length(w.end[0]));
        set(&mut v, types::wall::END_Y, types::length(w.end[1]));
        set(&mut v, types::wall::HEIGHT, types::length(w.height));
        set(
            &mut v,
            types::wall::BASE_OFFSET,
            types::length(w.base_offset),
        );
        set(
            &mut v,
            types::wall::LOCATION_LINE,
            Value::Enum(LocationLine::Center.code()),
        );
        // The body is the file's: no join may change it.
        // A square end stays square; a slanted one is a miter the model's joins make again.
        let join = |auto: bool| {
            Value::Enum(
                if auto {
                    JoinStyle::Auto
                } else {
                    JoinStyle::Disallowed
                }
                .code(),
            )
        };
        set(&mut v, types::wall::START_JOIN, join(w.start_auto));
        set(&mut v, types::wall::END_JOIN, join(w.end_auto));
        set(&mut v, types::wall::WALL_TYPE, Value::Reference(wall_type));
        set(&mut v, types::wall::THICKNESS, types::length(w.thickness));
        let layers = Self::layers(&w.spec, true);
        let [exterior, interior] = layers.core_offsets();
        set(&mut v, types::wall::CORE_EXTERIOR, types::length(exterior));
        set(&mut v, types::wall::CORE_INTERIOR, types::length(interior));
        let id = self.create_for(&w.kept, WALL, Some(storey), v)?;
        self.identify(id, &w.kept)?;
        Ok(id)
    }

    fn insert(&mut self, host: ObjectId, i: &InsertPlan) -> Result<ObjectId> {
        let type_id = self.insert_type(i)?;
        let key = if i.window { WINDOW } else { DOOR };
        let mut v = self.blank(key)?;
        let insert_name = self.unique(key, host, &i.name)?;
        set(&mut v, types::insert::NAME, text(&insert_name));
        set(&mut v, types::insert::DISTANCE, types::length(i.distance));
        set(&mut v, types::insert::WIDTH, types::length(i.width));
        set(&mut v, types::insert::HEIGHT, types::length(i.height));
        set(&mut v, types::insert::SILL, types::length(i.sill));
        set(
            &mut v,
            types::insert::OPERATION,
            Value::Enum(i.spec.operation),
        );
        set(
            &mut v,
            types::insert::HAND,
            Value::Enum(
                if i.right_hand {
                    Hand::Right
                } else {
                    Hand::Left
                }
                .code(),
            ),
        );
        set(
            &mut v,
            types::insert::FACING,
            Value::Enum(Facing::Exterior.code()),
        );
        set(
            &mut v,
            types::insert::INSERT_TYPE,
            Value::Reference(type_id),
        );
        let id = self.create_for(&i.kept, key, Some(host), v)?;
        self.identify(id, &i.kept)?;
        Ok(id)
    }

    fn vertices(plan: &SlabPlan) -> Vec<Vertex> {
        let mut out = Vec::new();
        for (loop_index, ring) in std::iter::once(&plan.outer).chain(&plan.holes).enumerate() {
            for p in ring {
                out.push(Vertex {
                    loop_index: u32::try_from(loop_index).unwrap_or(0),
                    point: *p,
                    edge: None,
                    bulge: 0.0,
                });
            }
        }
        out
    }

    fn slab(&mut self, storey: ObjectId, s: &SlabPlan, ceiling: bool) -> Result<ObjectId> {
        let type_id = self.floor_type(&s.spec, ceiling)?;
        let vertices = Floor::vertex_values(&Self::vertices(s));
        let id = if ceiling {
            let mut v = self.blank(CEILING)?;
            let n = self.unique(CEILING, storey, &s.name)?;
            set(&mut v, types::ceiling::NAME, text(&n));
            set(
                &mut v,
                types::ceiling::THICKNESS,
                types::length(s.thickness),
            );
            set(
                &mut v,
                types::ceiling::HEIGHT_OFFSET,
                types::length(s.offset),
            );
            set(&mut v, types::ceiling::VERTICES, vertices);
            set(
                &mut v,
                types::ceiling::CEILING_TYPE,
                Value::Reference(type_id),
            );
            self.create_for(&s.kept, CEILING, Some(storey), v)?
        } else {
            let mut v = self.blank(FLOOR)?;
            let n = self.unique(FLOOR, storey, &s.name)?;
            set(&mut v, types::floor::NAME, text(&n));
            set(&mut v, types::floor::THICKNESS, types::length(s.thickness));
            set(&mut v, types::floor::TOP_OFFSET, types::length(s.offset));
            set(&mut v, types::floor::VERTICES, vertices);
            set(&mut v, types::floor::FLOOR_TYPE, Value::Reference(type_id));
            self.create_for(&s.kept, FLOOR, Some(storey), v)?
        };
        self.identify(id, &s.kept)?;
        Ok(id)
    }

    fn room(&mut self, storey: ObjectId, r: &RoomPlan) -> Result<ObjectId> {
        let mut v = self.blank(ROOM)?;
        set(&mut v, types::room::NAME, text(&r.name));
        if let Some(n) = &r.number {
            set(&mut v, types::room::NUMBER, text(n));
        }
        set(&mut v, types::room::X, types::length(r.point[0]));
        set(&mut v, types::room::Y, types::length(r.point[1]));
        set(
            &mut v,
            types::room::BASE_OFFSET,
            types::length(r.base_offset),
        );
        let id = self.create_for(&r.kept, ROOM, Some(storey), v)?;
        self.identify(id, &r.kept)?;
        Ok(id)
    }

    fn foreign(&mut self, owner: ObjectId, f: &ForeignPlan) -> Result<ObjectId> {
        let mut v = self.blank(FOREIGN_ELEMENT)?;
        set(&mut v, foreign_element::NAME, text(&f.name));
        set(&mut v, foreign_element::CLASS, text(&f.class));
        if let Some(p) = &f.kept.predefined {
            set(&mut v, foreign_element::PREDEFINED_TYPE, text(p));
        }
        if let Some(p) = &f.kept.object_type {
            set(&mut v, foreign_element::OBJECT_TYPE, text(p));
        }
        if let Some(p) = &f.kept.tag {
            set(&mut v, foreign_element::TAG, text(p));
        }
        if let Some(mesh) = &f.mesh {
            set(
                &mut v,
                foreign_element::MESH,
                Value::Geometry(GeometryBytes(mesh.clone())),
            );
        }
        set(
            &mut v,
            foreign_element::SOURCE_SCHEMA,
            text(&self.schema.clone()),
        );
        let id = self.create_for(&f.kept, FOREIGN_ELEMENT, Some(owner), v)?;
        self.identify(id, &f.kept)?;
        Ok(id)
    }

    fn grid(&mut self, building: ObjectId, g: &GridPlan) -> Result<ObjectId> {
        let mut v = self.blank(GRID)?;
        set(&mut v, types::grid::NAME, text(&g.name));
        let (first, last) = (g.points[0], g.points[g.points.len() - 1]);
        set(&mut v, types::grid::START_X, types::length(first[0]));
        set(&mut v, types::grid::START_Y, types::length(first[1]));
        set(&mut v, types::grid::END_X, types::length(last[0]));
        set(&mut v, types::grid::END_Y, types::length(last[1]));
        if g.points.len() > 2 {
            let path: Vec<Value> = g.points[1..g.points.len() - 1]
                .iter()
                .map(|p| {
                    Value::Record(vec![
                        Some(types::length(p[0])),
                        Some(types::length(p[1])),
                        Some(Value::F64(0.0)),
                    ])
                })
                .collect();
            set(&mut v, types::grid::PATH, Value::List(path));
        }
        self.create(GRID, Some(building), v)
    }

    fn building(&mut self, site: ObjectId, b: &BuildingPlan) -> Result<ObjectId> {
        let mut v = self.blank(BUILDING)?;
        let n = self.unique(BUILDING, site, &b.name)?;
        set(&mut v, types::building::NAME, text(&n));
        set(
            &mut v,
            types::building::ELEVATION,
            types::length(b.elevation),
        );
        let id = self.create_for(&b.kept, BUILDING, Some(site), v)?;
        self.applied.buildings += 1;
        self.identify(id, &b.kept)?;
        Ok(id)
    }
}

/// Whether the document already holds the product with this `GlobalId`: an object whose
/// identifier it encodes (an element this document exported, or one a former import made from
/// it), or one the source map of the document binds to it.
#[must_use]
pub(crate) fn is_known(snapshot: &Snapshot, bound: &BTreeSet<String>, global_id: &str) -> bool {
    bound.contains(global_id)
        || decode_global_id(global_id)
            .is_some_and(|raw| snapshot.resolve(ObjectId::from_u128(raw)).is_ok())
}

/// The `GlobalId`s the source map of `snapshot` binds to live objects.
#[must_use]
pub(crate) fn bound(snapshot: &Snapshot) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for row in snapshot.foreign_rows() {
        if row.source_format == FORMAT
            && row.record_kind == arc_doc::source_map::RECORD_KIND
            && let Some(id) = &row.source_id
            && row
                .object
                .as_deref()
                .and_then(|b| <[u8; 16]>::try_from(b).ok())
                .is_some_and(|b| {
                    snapshot
                        .resolve(ObjectId::from_u128(u128::from_be_bytes(b)))
                        .is_ok()
                })
        {
            out.insert(id.clone());
        }
    }
    out
}

/// Writes `plan` into the document of `ctx`.
///
/// # Errors
/// The transaction's errors.
#[allow(
    clippy::too_many_lines,
    reason = "one pass over the plan, level by level"
)]
pub fn apply(
    ctx: &mut Context<'_>,
    plan: &Plan,
    file_name: &str,
    original: Option<&[u8]>,
    report_json: &[u8],
) -> Result<Applied> {
    let snapshot = ctx.snapshot().clone();
    let map = SourceMap::of(&snapshot);
    let existing_project = spatial::project(&snapshot);
    let existing_project_id = existing_project.as_ref().map(|p| p.id);
    let mut w = Writer {
        ctx,
        file_name: file_name.to_owned(),
        schema: plan.schema.clone(),
        applied: Applied::default(),
        wall_types: BTreeMap::new(),
        floor_types: BTreeMap::new(),
        ceiling_types: BTreeMap::new(),
        insert_types: BTreeMap::new(),
        taken: BTreeSet::new(),
        used: BTreeSet::new(),
        claimed: BTreeSet::new(),
        map,
        project: ObjectId::from_u128(0),
    };
    // The project, the site: the document's own when it has them, else new ones.
    let (project_id, site_id) = match (existing_project, spatial::site(&snapshot)) {
        (Some(p), Some(s)) => (p.id, s.id),
        (existing, site) => {
            let project_id = match existing {
                Some(p) => p.id,
                None => {
                    let mut v = w.blank(PROJECT)?;
                    set(
                        &mut v,
                        project::NAME,
                        text(plan.project.name.as_deref().unwrap_or("Project")),
                    );
                    w.create(PROJECT, None, v)?
                }
            };
            let site_id = match site {
                Some(s) => s.id,
                None => {
                    let mut v = w.blank(SITE)?;
                    set(
                        &mut v,
                        types::site::NAME,
                        text(plan.site_name.as_deref().unwrap_or("Site")),
                    );
                    w.create(SITE, Some(project_id), v)?
                }
            };
            (project_id, site_id)
        }
    };
    w.project = project_id;
    if let Some(project_object) = spatial::get(w.ctx.txn()?.snapshot(), project_id) {
        let txn = w.ctx.txn()?;
        let mut fill = |field: &str, at: usize, value: Option<&String>| -> Result<()> {
            if let Some(value) = value
                && types::text(&project_object.values, at).is_none_or(|t| t.trim().is_empty())
            {
                txn.modify(
                    project_id,
                    arc_doc::FieldPath::new(field),
                    Some(text(value)),
                )?;
            }
            Ok(())
        };
        fill(
            "long_name",
            project::LONG_NAME,
            plan.project.long_name.as_ref(),
        )?;
        fill(
            "description",
            project::DESCRIPTION,
            plan.project.description.as_ref(),
        )?;
        fill("status", project::STATUS, plan.project.phase.as_ref())?;
        if let Some(geo) = &plan.project.geo
            && types::text(&project_object.values, project::CRS).is_none()
        {
            for (field, value) in [
                ("crs", Value::String(geo.crs.chars().take(16_000).collect())),
                ("eastings", types::length(geo.eastings)),
                ("northings", types::length(geo.northings)),
                ("survey_height", types::length(geo.height)),
                ("true_north", types::angle(geo.true_north)),
                ("map_scale", Value::F64(geo.scale)),
            ] {
                txn.modify(project_id, arc_doc::FieldPath::new(field), Some(value))?;
            }
        }
    }
    if existing_project_id.is_none() {
        w.identify(project_id, &plan.project.kept)?;
    }
    for building in &plan.buildings {
        if building.storeys.is_empty() && building.grids.is_empty() && building.foreign.is_empty() {
            continue;
        }
        let building_id = w.building(site_id, building)?;
        let mut storey_ids: Vec<ObjectId> = Vec::new();
        for storey in &building.storeys {
            let storey_id = w.storey(building_id, storey)?;
            storey_ids.push(storey_id);
            for item in &storey.items {
                let made = match item {
                    Item::Wall(x) => w.wall(storey_id, x)?,
                    Item::Floor(x) => w.slab(storey_id, x, false)?,
                    Item::Ceiling(x) => w.slab(storey_id, x, true)?,
                    Item::Room(x) => w.room(storey_id, x)?,
                    Item::Foreign(x) => w.foreign(storey_id, x)?,
                    Item::Insert(_) => continue,
                };
                w.applied.ids.insert(item.key(), made);
            }
            for item in &storey.items {
                if let Item::Insert(i) = item
                    && let Some(host) = w.applied.ids.get(&i.host).copied()
                {
                    let made = w.insert(host, i)?;
                    w.applied.ids.insert(i.key, made);
                }
            }
        }
        for f in &building.foreign {
            let made = w.foreign(building_id, f)?;
            w.applied.ids.insert(f.key, made);
        }
        for g in &building.grids {
            let made = w.grid(building_id, g)?;
            w.applied.ids.insert(g.key, made);
        }
    }
    // Groups as selection sets of the members that are elements of the model.
    let current = w.ctx.txn()?.snapshot().clone();
    for group in &plan.groups {
        let members: Vec<Value> = group
            .members
            .iter()
            .filter_map(|k| w.applied.ids.get(k).copied())
            .filter(|id| spatial::type_of(&current, *id).is_some_and(|t| types::is_element(&t)))
            .map(|id| Value::Record(vec![Some(Value::Reference(id))]))
            .collect();
        if members.is_empty() {
            continue;
        }
        let mut v = w.blank(SELECTION_SET)?;
        let n = w.unique(SELECTION_SET, project_id, &group.name)?;
        set(&mut v, types::selection_set::NAME, text(&n));
        set(&mut v, types::selection_set::MEMBERS, Value::List(members));
        w.create(SELECTION_SET, Some(project_id), v)?;
    }
    // The original file and the loss report, at document level.
    let txn = w.ctx.txn()?;
    if let Some(bytes) = original {
        txn.add_foreign(StoredForeign {
            source_format: FORMAT.to_owned(),
            source_version: Some(plan.schema.clone()),
            source_id: Some(file_name.to_owned()),
            record_kind: KIND_SOURCE.to_owned(),
            data: bytes.to_vec(),
            ..StoredForeign::default()
        })?;
    }
    txn.add_foreign(StoredForeign {
        source_format: FORMAT.to_owned(),
        source_version: Some(plan.schema.clone()),
        source_id: Some(file_name.to_owned()),
        record_kind: KIND_REPORT.to_owned(),
        data: report_json.to_vec(),
        ..StoredForeign::default()
    })?;
    let _ = Error::new;
    Ok(w.applied)
}
