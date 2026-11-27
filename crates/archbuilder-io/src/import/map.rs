//! Mapping the objects of an IFC file to the plan of an import.
//!
//! Native elements are made only where the file's geometry is the shape the model has: a wall is a
//! vertical prism with a rectangular footprint, a floor or ceiling a horizontal prism, a door or
//! window sits in a wall that was itself mapped. Every other product becomes a foreign element
//! with its source class, its data and its mesh. Nothing is bound by proximity: a door's wall is
//! the one the file's void and fill relationships give.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use arc_base::{Cancel, Limits, Result};
use arc_io_common::loss::{Direction, LossHeader, LossLog, Source as LossSource};
use arc_io_ifc::{
    ClassificationRef, MaterialAssociation, Matrix, Nominal, ObjectView, Origin, PropertyValue,
    ResolvedPropertySet, ResolvedQuantitySet, Value,
};
use arc_mesh::Quantization;
use archbuilder_model::insert::{Hand, Operation};
use archbuilder_model::types::COORDINATE_RANGE;

use super::data::{Class, Kept, Material, MaterialPart, Nom, Prop, PropKind, Pset, Qto, Qty};
use super::geom::{self, Soup};
use super::plan::{
    BuildingPlan, ForeignPlan, GeoPlan, GridPlan, GroupPlan, InsertPlan, InsertSpec, Item, Key,
    LayeredSpec, Plan, ProjectPlan, RoomPlan, SlabPlan, StoreyPlan, Tally, WallPlan,
};
use super::read::{self, Source, number, reference, references, text};
use crate::error::reasons;

/// What a class of the file is to the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Wall,
    Slab,
    Covering,
    Door,
    Window,
    Space,
    Grid,
    Opening,
    /// Project, site, building, storey and the other spatial structure elements.
    Structure,
    Other,
}

fn kind_of(class: &str) -> Kind {
    match class {
        "IFCWALL" | "IFCWALLSTANDARDCASE" => Kind::Wall,
        "IFCSLAB" | "IFCSLABSTANDARDCASE" => Kind::Slab,
        "IFCCOVERING" => Kind::Covering,
        "IFCDOOR" | "IFCDOORSTANDARDCASE" => Kind::Door,
        "IFCWINDOW" | "IFCWINDOWSTANDARDCASE" => Kind::Window,
        "IFCSPACE" => Kind::Space,
        "IFCGRID" => Kind::Grid,
        "IFCOPENINGELEMENT" | "IFCOPENINGSTANDARDCASE" => Kind::Opening,
        "IFCSITE"
        | "IFCBUILDING"
        | "IFCBUILDINGSTOREY"
        | "IFCSPATIALZONE"
        | "IFCFACILITY"
        | "IFCFACILITYPART"
        | "IFCROAD"
        | "IFCBRIDGE"
        | "IFCRAILWAY"
        | "IFCMARINEFACILITY"
        | "IFCROADPART"
        | "IFCBRIDGEPART"
        | "IFCRAILWAYPART"
        | "IFCMARINEPART"
        | "IFCEXTERNALSPATIALELEMENT"
        | "IFCSPATIALELEMENT" => Kind::Structure,
        _ => Kind::Other,
    }
}

/// The options of the mapping.
pub struct MapOptions<'a> {
    /// The limits of the import.
    pub limits: Limits,
    /// The cancellation token.
    pub cancel: Cancel,
    /// The GlobalIds the document already holds: those products are matched, not made again.
    pub known: &'a dyn Fn(&str) -> bool,
}

/// A wall that was mapped, for the doors and windows it hosts.
struct Host {
    key: Key,
    start: [f64; 2],
    end: [f64; 2],
    base: f64,
    storey: (usize, usize),
}

struct Mapper<'a> {
    src: &'a Source,
    options: &'a MapOptions<'a>,
    log: LossLog,
    tally: BTreeMap<String, Tally>,
    next: Key,
    identities: Vec<(String, Key)>,
    hosts: HashMap<u64, Host>,
    /// The key of the element an instance became.
    made: HashMap<u64, Key>,
    buildings: Vec<BuildingPlan>,
    building_of: HashMap<u64, usize>,
    storey_of: HashMap<u64, (usize, usize)>,
}

fn class_name(src: &Source, upper: &str) -> String {
    src.file
        .tables()
        .and_then(|t| t.entity(upper))
        .map_or_else(|| title(upper), |e| e.name.to_owned())
}

fn title(upper: &str) -> String {
    let lower = upper.to_ascii_lowercase();
    let mut out = String::new();
    if let Some(rest) = lower.strip_prefix("ifc") {
        out.push_str("Ifc");
        let mut chars = rest.chars();
        if let Some(c) = chars.next() {
            out.extend(c.to_uppercase());
        }
        out.extend(chars);
    } else {
        out = lower;
    }
    out
}

fn apply(m: &Matrix, p: [f64; 3]) -> [f64; 3] {
    [
        m[0] * p[0] + m[1] * p[1] + m[2] * p[2] + m[3],
        m[4] * p[0] + m[5] * p[1] + m[6] * p[2] + m[7],
        m[8] * p[0] + m[9] * p[1] + m[10] * p[2] + m[11],
    ]
}

fn nom(n: &Nominal, scale: Option<f64>) -> Nom {
    let mut out = Nom {
        ty: n.type_name.clone(),
        ..Nom::default()
    };
    match &n.value {
        Value::Text(t) => out.text = Some(t.clone()),
        Value::Real(r) => out.real = Some(scale.map_or(*r, |s| r * s)),
        Value::Integer(i) => out.int = Some(*i),
        Value::Enumeration(e) => out.enumeration = Some(e.clone()),
    }
    out
}

fn psets(sets: &[ResolvedPropertySet]) -> Vec<Pset> {
    sets.iter()
        .map(|set| Pset {
            name: set.name.clone().unwrap_or_default(),
            props: set
                .properties
                .iter()
                .map(|p| {
                    let scale = p.scale_to_si.filter(|s| (*s - 1.0).abs() > f64::EPSILON);
                    let (kind, values) = match &p.property.value {
                        PropertyValue::Single { nominal, .. } => (
                            PropKind::Single,
                            nominal.iter().map(|n| nom(n, scale)).collect(),
                        ),
                        PropertyValue::Enumerated { values, .. } => (
                            PropKind::Enumerated,
                            values.iter().map(|n| nom(n, scale)).collect(),
                        ),
                        PropertyValue::List { values, .. } => (
                            PropKind::List,
                            values.iter().map(|n| nom(n, scale)).collect(),
                        ),
                        PropertyValue::Bounded {
                            upper,
                            lower,
                            set_point,
                            ..
                        } => (
                            PropKind::Bounded,
                            [upper, lower, set_point]
                                .into_iter()
                                .flatten()
                                .map(|n| nom(n, scale))
                                .collect(),
                        ),
                    };
                    Prop {
                        name: p.property.name.clone().unwrap_or_default(),
                        description: p.property.description.clone(),
                        kind,
                        values,
                        inherited: p.origin == Origin::Type,
                    }
                })
                .collect(),
        })
        .collect()
}

fn qtos(sets: &[ResolvedQuantitySet]) -> Vec<Qto> {
    use arc_io_ifc::{Number, QuantityKind};
    sets.iter()
        .map(|set| Qto {
            name: set.name.clone().unwrap_or_default(),
            quantities: set
                .quantities
                .iter()
                .filter_map(|q| {
                    let kind = match q.quantity.kind {
                        QuantityKind::Length => "length",
                        QuantityKind::Area => "area",
                        QuantityKind::Volume => "volume",
                        QuantityKind::Count => "count",
                        QuantityKind::Weight => "weight",
                        QuantityKind::Time => "time",
                    };
                    #[allow(clippy::cast_precision_loss, reason = "a count")]
                    let raw = match q.quantity.value? {
                        Number::Integer(i) => i as f64,
                        Number::Real(r) => r,
                    };
                    Some(Qty {
                        name: q.quantity.name.clone().unwrap_or_default(),
                        kind: kind.to_owned(),
                        value: q.si_value.unwrap_or(raw),
                    })
                })
                .collect(),
        })
        .collect()
}

fn classifications(list: &[ClassificationRef]) -> Vec<Class> {
    list.iter()
        .map(|c| Class {
            identification: c.identification.clone(),
            name: c.name.clone(),
            system: c.system.as_ref().and_then(|s| s.name.clone()),
            edition: c.system.as_ref().and_then(|s| s.edition.clone()),
        })
        .collect()
}

fn material(assoc: &MaterialAssociation) -> Material {
    let part = |m: &Option<arc_io_ifc::MaterialRef>, thickness: Option<f64>| MaterialPart {
        material: m.as_ref().and_then(|m| m.name.clone()),
        thickness,
    };
    match assoc {
        MaterialAssociation::Single(m) => Material {
            kind: "material".to_owned(),
            name: m.name.clone(),
            parts: vec![part(&Some(m.clone()), None)],
        },
        MaterialAssociation::List(list) => Material {
            kind: "list".to_owned(),
            name: None,
            parts: list.iter().map(|m| part(&Some(m.clone()), None)).collect(),
        },
        MaterialAssociation::LayerSet { name, layers, .. } => Material {
            kind: "layers".to_owned(),
            name: name.clone(),
            parts: layers
                .iter()
                .map(|l| part(&l.material, l.thickness))
                .collect(),
        },
        MaterialAssociation::ProfileSet { name, profiles, .. } => Material {
            kind: "profiles".to_owned(),
            name: name.clone(),
            parts: profiles.iter().map(|p| part(&p.material, None)).collect(),
        },
        MaterialAssociation::ConstituentSet {
            name, constituents, ..
        } => Material {
            kind: "constituents".to_owned(),
            name: name.clone(),
            parts: constituents
                .iter()
                .map(|c| part(&c.material, None))
                .collect(),
        },
    }
}

fn clean(text: &Option<String>) -> Option<String> {
    text.as_ref()
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
}

impl<'a> Mapper<'a> {
    fn kept(&self, o: &ObjectView) -> Kept {
        let src = self.src;
        let type_object = o.type_object.and_then(|t| src.views.import.object(t));
        let long_name = o
            .instance
            .checked_add(0)
            .and_then(|i| src.attribute(i, "LongName"))
            .and_then(|p| text(Some(p)));
        Kept {
            class: class_name(src, &o.type_name),
            global_id: o.global_id.clone().unwrap_or_default(),
            predefined: o.effective_predefined_type.clone(),
            object_type: o
                .user_defined_type
                .clone()
                .or_else(|| o.object_type.clone()),
            tag: clean(&o.tag),
            description: clean(&o.description),
            long_name,
            type_class: type_object.map(|t| class_name(src, &t.type_name)),
            type_name: type_object.and_then(|t| clean(&t.name)),
            psets: psets(&o.property_sets),
            qtos: qtos(&o.quantity_sets),
            classifications: classifications(&o.classifications),
            material: o.material.as_ref().map(|(m, _)| material(m)),
            groups: o
                .groups
                .iter()
                .filter_map(|g| src.views.import.group(*g).and_then(|g| clean(&g.name)))
                .collect(),
        }
    }

    fn lose(&mut self, reason: crate::error::Reason, class: &str, o: &ObjectView) {
        let gid = o.global_id.clone().unwrap_or_default();
        let source = if gid.is_empty() {
            LossSource::Decimal(o.instance)
        } else {
            LossSource::Text(&gid)
        };
        self.log.add(reason, class, Some(source), 1);
    }

    fn key(&mut self, o: &ObjectView) -> Key {
        let key = self.next;
        self.next += 1;
        self.made.insert(o.instance, key);
        if let Some(gid) = &o.global_id {
            self.identities.push((gid.clone(), key));
        }
        key
    }

    fn name(&self, o: &ObjectView, fallback: &str) -> String {
        clean(&o.name).unwrap_or_else(|| fallback.to_owned())
    }

    fn count(&mut self, class: &str, f: impl FnOnce(&mut Tally)) {
        f(self.tally.entry(class.to_owned()).or_default());
    }

    fn level_of(&self, o: &ObjectView) -> f64 {
        o.elevation
            .or_else(|| o.placement.as_ref().map(|p| p.world[11]))
            .unwrap_or(0.0)
    }

    /// The building and storey of a product: from its container, aggregate or nest parent up. A
    /// product that no storey holds goes to the lowest storey of its building (`true` then).
    fn place(&self, o: &ObjectView) -> (usize, usize, bool) {
        let mut cur = o;
        let mut building = 0;
        for _ in 0..64 {
            let Some(next) = cur
                .container
                .or(cur.aggregate_parent)
                .or(cur.nest_parent)
                .and_then(|n| self.src.views.import.object(n))
            else {
                break;
            };
            if let Some((b, s)) = self.storey_of.get(&next.instance) {
                return (*b, *s, false);
            }
            if let Some(b) = self.building_of.get(&next.instance) {
                building = *b;
                break;
            }
            cur = next;
        }
        (building, 0, true)
    }

    fn soup(&self, o: &ObjectView, cut: bool) -> Option<Soup> {
        let items = if cut {
            self.src.cut_items(o.instance)
        } else {
            self.src.solid_items(o.instance)
        };
        if items.is_empty() {
            return None;
        }
        Soup::of(&items, &self.options.limits)
            .ok()
            .filter(|s| !s.is_empty())
    }

    fn in_range(points: impl IntoIterator<Item = [f64; 2]>) -> bool {
        points.into_iter().all(|p| {
            p.iter()
                .all(|c| (COORDINATE_RANGE.0..=COORDINATE_RANGE.1).contains(c))
        })
    }

    /// A foreign element for `o`, with its mesh.
    fn foreign(&mut self, o: &ObjectView, why: Option<crate::error::Reason>) -> Item {
        let class = class_name(self.src, &o.type_name);
        let mesh = self.soup(o, true).and_then(|soup| {
            soup.to_mesh(&self.options.limits)
                .and_then(|m| m.encode(&Quantization::default()))
                .ok()
        });
        if mesh.is_none() {
            self.lose(reasons::IMPORT_NO_GEOMETRY, &class, o);
        }
        if let Some(reason) = why {
            self.lose(reason, &class, o);
        }
        self.lose(reasons::IMPORT_PRESERVED, &class, o);
        self.count(&o.type_name.clone(), |t| t.foreign += 1);
        let key = self.key(o);
        Item::Foreign(ForeignPlan {
            key,
            name: self.name(o, &class),
            class,
            mesh,
            kept: self.kept(o),
        })
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the layer set, its usage and the type's flags"
    )]
    fn layered(&self, o: &ObjectView, thickness: f64, wall: bool) -> (LayeredSpec, bool) {
        let src = self.src;
        let type_name = o
            .type_object
            .and_then(|t| src.views.import.object(t))
            .and_then(|t| clean(&t.name));
        let (set_name, layers, usage) = match &o.material {
            Some((
                MaterialAssociation::LayerSet {
                    name,
                    layers,
                    usage,
                    ..
                },
                _,
            )) => (name.clone(), layers.clone(), usage.clone()),
            _ => (None, Vec::new(), None),
        };
        let mut entries: Vec<(String, f64)> = layers
            .iter()
            .filter_map(|l| {
                Some((
                    l.material
                        .as_ref()
                        .and_then(|m| m.name.clone())
                        .unwrap_or_default(),
                    l.thickness?,
                ))
            })
            .filter(|(_, t)| *t > 0.0)
            .collect();
        let total: f64 = entries.iter().map(|(_, t)| t).sum();
        let mut mismatch = false;
        let single_material = || match &o.material {
            Some((MaterialAssociation::Single(m), _)) => m.name.clone(),
            Some((MaterialAssociation::LayerSet { layers, .. }, _)) => layers
                .iter()
                .find_map(|l| l.material.as_ref().and_then(|m| m.name.clone())),
            _ => None,
        };
        if entries.is_empty() || (total - thickness).abs() > 1.0e-3_f64.max(thickness * 0.01) {
            mismatch = !entries.is_empty();
            entries = vec![(single_material().unwrap_or_default(), thickness)];
        } else if !wall {
            // Layers of a slab run from the bottom in the file unless the usage says otherwise; the
            // model lists them from the top.
            let negative = usage
                .as_ref()
                .and_then(|u| u.sense.as_deref())
                .is_some_and(|s| s == "NEGATIVE");
            if !negative {
                entries.reverse();
            }
        }
        for (material, _) in &mut entries {
            if material.trim().is_empty() {
                "Generic".clone_into(material);
            }
        }
        let core = entries
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.1.total_cmp(&b.1.1))
            .map_or(0, |(at, _)| at);
        let pset_bool = |name: &str| {
            o.property_sets
                .iter()
                .find(|s| s.name.as_deref().is_some_and(|n| n.ends_with("Common")))
                .and_then(|s| {
                    s.properties
                        .iter()
                        .find(|p| p.property.name.as_deref() == Some(name))
                })
                .and_then(|p| match &p.property.value {
                    PropertyValue::Single {
                        nominal: Some(n), ..
                    } => match &n.value {
                        Value::Enumeration(e) => Some(e == "T"),
                        _ => None,
                    },
                    _ => None,
                })
        };
        let name = type_name
            .or_else(|| set_name.and_then(|n| clean(&Some(n))))
            .unwrap_or_else(|| {
                format!(
                    "{} {} mm",
                    if wall {
                        "Imported wall"
                    } else {
                        "Imported slab"
                    },
                    (thickness * 1000.0).round()
                )
            });
        (
            LayeredSpec {
                name,
                layers: entries,
                core,
                external: pset_bool("IsExternal").unwrap_or(false),
                load_bearing: pset_bool("LoadBearing").unwrap_or(false),
            },
            mismatch,
        )
    }

    fn wall(&mut self, o: &ObjectView, storey: (usize, usize), level: f64) -> Option<Item> {
        let soup = self.soup(o, false)?;
        let prism = geom::prism(&soup)?;
        if !prism.holes.is_empty() {
            return None;
        }
        let rect = geom::strip(&prism.outer)?;
        if !Self::in_range([rect.start, rect.end]) {
            return None;
        }
        let (mut start, mut end) = (rect.start, rect.end);
        let (spec, mismatch) = self.layered(o, rect.thickness, true);
        // The first layer is the exterior face, on the left of the model's baseline.
        let usage = match &o.material {
            Some((MaterialAssociation::LayerSet { usage, .. }, _)) => usage.clone(),
            _ => None,
        };
        if let (Some(u), Some(p)) = (usage, o.placement.as_ref())
            && u.direction.as_deref() == Some("AXIS2")
        {
            let sign = if u.sense.as_deref() == Some("NEGATIVE") {
                -1.0
            } else {
                1.0
            };
            let d = [sign * p.world[1], sign * p.world[5]];
            // left(baseline) = -d: the baseline direction is d turned a quarter counterclockwise.
            let wanted = [-d[1], d[0]];
            let dir = [end[0] - start[0], end[1] - start[1]];
            if wanted[0] * dir[0] + wanted[1] * dir[1] < 0.0 {
                std::mem::swap(&mut start, &mut end);
            }
        }
        if mismatch {
            self.lose(
                reasons::IMPORT_LAYERS_MISMATCH,
                &class_name(self.src, &o.type_name),
                o,
            );
        }
        let key = self.key(o);
        let name = self.name(o, "Wall");
        self.hosts.insert(
            o.instance,
            Host {
                key,
                start,
                end,
                base: prism.z0,
                storey,
            },
        );
        self.count(&o.type_name.clone(), |t| t.native += 1);
        Some(Item::Wall(WallPlan {
            key,
            name,
            start,
            end,
            thickness: rect.thickness,
            height: prism.z1 - prism.z0,
            base_offset: prism.z0 - level,
            spec,
            start_auto: !rect.start_square,
            end_auto: !rect.end_square,
            kept: self.kept(o),
        }))
    }

    fn slab(&mut self, o: &ObjectView, level: f64, ceiling: bool) -> Option<Item> {
        let soup = self.soup(o, true)?;
        let prism = geom::prism(&soup)?;
        if !Self::in_range(prism.outer.iter().copied()) || prism.outer.len() > 4096 {
            return None;
        }
        let thickness = prism.z1 - prism.z0;
        let (spec, mismatch) = self.layered(o, thickness, false);
        if mismatch {
            self.lose(
                reasons::IMPORT_LAYERS_MISMATCH,
                &class_name(self.src, &o.type_name),
                o,
            );
        }
        let key = self.key(o);
        let class = if ceiling { "Ceiling" } else { "Floor" };
        let plan = SlabPlan {
            key,
            name: self.name(o, class),
            thickness,
            offset: if ceiling { prism.z0 } else { prism.z1 } - level,
            outer: prism.outer,
            holes: prism.holes,
            spec,
            kept: self.kept(o),
        };
        self.count(&o.type_name.clone(), |t| t.native += 1);
        Some(if ceiling {
            Item::Ceiling(plan)
        } else {
            Item::Floor(plan)
        })
    }

    fn room(&mut self, o: &ObjectView, level: f64) -> Item {
        let kept = self.kept(o);
        let (point, base) = match self.soup(o, true) {
            Some(soup) => {
                let prism = geom::prism(&soup);
                let bounds = soup.bounds();
                match (prism, bounds) {
                    (Some(p), _) => (centre(&p.outer), p.z0 - level),
                    (None, Some((lo, hi))) => (
                        [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0],
                        lo[2] - level,
                    ),
                    _ => (self.origin_of(o), 0.0),
                }
            }
            None => (self.origin_of(o), 0.0),
        };
        let long = kept.long_name.clone();
        let (name, number) = match (long, clean(&o.name)) {
            (Some(long), number) => (long, number),
            (None, Some(n)) => (n, None),
            (None, None) => ("Room".to_owned(), None),
        };
        let key = self.key(o);
        self.count(&o.type_name.clone(), |t| t.native += 1);
        Item::Room(RoomPlan {
            key,
            name,
            number,
            point,
            base_offset: if base.abs() > 1.0e-3 { base } else { 0.0 },
            kept,
        })
    }

    fn origin_of(&self, o: &ObjectView) -> [f64; 2] {
        o.placement
            .as_ref()
            .map_or([0.0; 2], |p| [p.world[3], p.world[7]])
    }

    fn insert(&mut self, o: &ObjectView, window: bool) -> Option<(usize, usize, Item)> {
        // The wall the file's relationships name: the filled opening's host.
        let opening = self.src.views.import.object(o.fills?)?;
        let host_instance = opening.voids?;
        let host = self.hosts.get(&host_instance)?;
        let (building, storey) = host.storey;
        let (start, end, wall_base) = (host.start, host.end, host.base);
        let host_key = host.key;
        let dir = [end[0] - start[0], end[1] - start[1]];
        let length = dir[0].hypot(dir[1]);
        if length <= 0.0 {
            return None;
        }
        let u = [dir[0] / length, dir[1] / length];
        let m = self.src.meters();
        let attr = |name: &str| number(self.src.attribute(o.instance, name)).map(|v| v * m);
        let (centre, width, z0, z1) = match self.soup(o, false) {
            Some(soup) => {
                let (mut a0, mut a1, mut z0, mut z1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
                for p in &soup.p {
                    let along = (p[0] - start[0]) * u[0] + (p[1] - start[1]) * u[1];
                    a0 = a0.min(along);
                    a1 = a1.max(along);
                    z0 = z0.min(p[2]);
                    z1 = z1.max(p[2]);
                }
                ((a0 + a1) / 2.0, a1 - a0, z0, z1)
            }
            None => {
                let origin = self.origin_of(o);
                let along = (origin[0] - start[0]) * u[0] + (origin[1] - start[1]) * u[1];
                let z = o.placement.as_ref().map_or(wall_base, |p| p.world[11]);
                let (w, h) = (attr("OverallWidth")?, attr("OverallHeight")?);
                (along + w / 2.0, w, z, z + h)
            }
        };
        let height = z1 - z0;
        if width < 0.1 || height < 0.1 || centre < 0.0 || centre > length {
            return None;
        }
        let type_object = o.type_object.and_then(|t| self.src.views.import.object(t));
        let operation_text = type_object
            .and_then(|t| text(self.src.attribute(t.instance, "OperationType")))
            .or_else(|| {
                type_object.and_then(|t| text(self.src.attribute(t.instance, "PartitioningType")))
            })
            .unwrap_or_default();
        let operation = if window {
            if operation_text.contains("SLIDING") {
                Operation::Sliding
            } else if operation_text.contains("FOLDING") {
                Operation::Folding
            } else {
                Operation::Fixed
            }
        } else if operation_text.contains("DOUBLE_DOOR") || operation_text.contains("DOUBLE_SWING")
        {
            Operation::DoubleSwing
        } else if operation_text.contains("SLIDING") {
            Operation::Sliding
        } else if operation_text.contains("FOLDING") {
            Operation::Folding
        } else {
            Operation::SingleSwing
        };
        let hand = if operation_text.ends_with("RIGHT") {
            Hand::Right
        } else {
            Hand::Left
        };
        let class = class_name(self.src, &o.type_name);
        self.lose(reasons::IMPORT_SWING_ASSUMED, &class, o);
        self.lose(reasons::IMPORT_INSERT_REGENERATED, &class, o);
        let key = self.key(o);
        let type_name = type_object.and_then(|t| clean(&t.name));
        let name = self.name(o, if window { "Window" } else { "Door" });
        self.count(&o.type_name.clone(), |t| t.native += 1);
        Some((
            building,
            storey,
            Item::Insert(InsertPlan {
                key,
                window,
                name: name.clone(),
                host: host_key,
                distance: centre,
                width,
                height,
                sill: (z0 - wall_base).max(0.0),
                right_hand: hand == Hand::Right,
                spec: InsertSpec {
                    name: type_name.unwrap_or(name),
                    width,
                    height,
                    operation: operation.code(),
                },
                kept: self.kept(o),
            }),
        ))
    }

    fn grid(&mut self, o: &ObjectView) -> Vec<GridPlan> {
        let m = self.src.meters();
        let world = o.placement.as_ref().map(|p| p.world);
        let to_world = |p: [f64; 3]| world.map_or(p, |w| apply(&w, p));
        let mut out = Vec::new();
        for list in ["UAxes", "VAxes", "WAxes"] {
            for axis in references(self.src.attribute(o.instance, list)) {
                let tag = text(self.src.attribute(axis, "AxisTag")).unwrap_or_default();
                let Some(curve) = reference(self.src.attribute(axis, "AxisCurve")) else {
                    continue;
                };
                let points = self.curve_points(curve, m);
                let points: Vec<[f64; 2]> = points
                    .into_iter()
                    .map(|p| {
                        let w = to_world([p[0], p[1], 0.0]);
                        [w[0], w[1]]
                    })
                    .collect();
                if points.len() < 2 || !Self::in_range(points.iter().copied()) {
                    continue;
                }
                let key = self.next;
                self.next += 1;
                out.push(GridPlan {
                    key,
                    name: if tag.trim().is_empty() {
                        format!("{}", out.len() + 1)
                    } else {
                        tag.trim().to_owned()
                    },
                    points,
                    kept: Kept {
                        class: "IfcGridAxis".to_owned(),
                        global_id: o.global_id.clone().unwrap_or_default(),
                        ..Kept::default()
                    },
                });
            }
        }
        out
    }

    /// The points of a polyline or line curve, in meters (2D).
    fn curve_points(&self, curve: u64, m: f64) -> Vec<[f64; 2]> {
        let Some(record) = self.src.record(curve) else {
            return Vec::new();
        };
        let point = |n: u64| -> Option<[f64; 2]> {
            let r = self.src.record(n)?;
            match self.src.file.attribute(r, "Coordinates")? {
                arc_io_ifc::Parameter::List(c) => {
                    let get = |i: usize| number(c.get(i)).map(|v| v * m);
                    Some([get(0)?, get(1)?])
                }
                _ => None,
            }
        };
        match record.type_name.as_str() {
            "IFCPOLYLINE" => references(self.src.file.attribute(record, "Points"))
                .into_iter()
                .filter_map(point)
                .collect(),
            "IFCLINE" => {
                let Some(p) = reference(self.src.file.attribute(record, "Pnt")).and_then(point)
                else {
                    return Vec::new();
                };
                let vector = reference(self.src.file.attribute(record, "Dir"))
                    .and_then(|v| self.src.record(v))
                    .and_then(|v| reference(self.src.file.attribute(v, "Orientation")))
                    .and_then(|d| self.src.record(d))
                    .and_then(|d| match self.src.file.attribute(d, "DirectionRatios")? {
                        arc_io_ifc::Parameter::List(c) => {
                            Some([number(c.first())?, number(c.get(1))?])
                        }
                        _ => None,
                    });
                let magnitude = reference(self.src.file.attribute(record, "Dir"))
                    .and_then(|v| self.src.record(v))
                    .and_then(|v| number(self.src.file.attribute(v, "Magnitude")))
                    .unwrap_or(1.0)
                    * m;
                match vector {
                    Some(d) => {
                        let len = d[0].hypot(d[1]).max(1.0e-12);
                        let reach = 1.0e3_f64.max(magnitude);
                        vec![p, [p[0] + d[0] / len * reach, p[1] + d[1] / len * reach]]
                    }
                    None => Vec::new(),
                }
            }
            _ => Vec::new(),
        }
    }
}

fn centre(outer: &[[f64; 2]]) -> [f64; 2] {
    let n = outer.len();
    let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let (p, q) = (outer[i], outer[(i + 1) % n]);
        let w = p[0] * q[1] - q[0] * p[1];
        a += w;
        cx += (p[0] + q[0]) * w;
        cy += (p[1] + q[1]) * w;
    }
    if a.abs() < 1.0e-12 {
        return outer.first().copied().unwrap_or([0.0; 2]);
    }
    [cx / (3.0 * a), cy / (3.0 * a)]
}

/// Maps the objects of `src`.
///
/// # Errors
/// `base.cancelled`.
#[allow(
    clippy::too_many_lines,
    reason = "one pass over the products, kind by kind"
)]
#[allow(
    clippy::cognitive_complexity,
    reason = "one pass over the products, kind by kind"
)]
pub fn map(src: &Source, options: &MapOptions<'_>) -> Result<Plan> {
    let views = &src.views.import;
    let mut mapper = Mapper {
        src,
        options,
        log: LossLog::bounded(),
        tally: BTreeMap::new(),
        next: 0,
        identities: Vec::new(),
        hosts: HashMap::new(),
        made: HashMap::new(),
        buildings: Vec::new(),
        building_of: HashMap::new(),
        storey_of: HashMap::new(),
    };
    // The spatial structure. The export writes one building, so the buildings of a file are one
    // building of the document: the first one's, with the storeys of the others (their names
    // prefixed by their building's); each further building is reported.
    let mut building_names: Vec<(u64, String)> = Vec::new();
    for o in views
        .objects
        .iter()
        .filter(|o| o.type_name == "IFCBUILDING")
    {
        if mapper.buildings.is_empty() {
            let kept = mapper.kept(o);
            mapper.buildings.push(BuildingPlan {
                name: mapper.name(o, "Building"),
                address: None,
                elevation: o.elevation.unwrap_or(0.0),
                kept,
                storeys: Vec::new(),
                grids: Vec::new(),
                foreign: Vec::new(),
            });
        } else {
            mapper.lose(reasons::IMPORT_BUILDING_MERGED, "IfcBuilding", o);
        }
        building_names.push((o.instance, mapper.name(o, "Building")));
        mapper.building_of.insert(o.instance, 0);
    }
    if mapper.buildings.is_empty() {
        mapper.buildings.push(BuildingPlan {
            name: "Building".to_owned(),
            address: None,
            elevation: 0.0,
            kept: Kept::default(),
            storeys: Vec::new(),
            grids: Vec::new(),
            foreign: Vec::new(),
        });
    }
    let mut storeys: Vec<&ObjectView> = views
        .objects
        .iter()
        .filter(|o| o.type_name == "IFCBUILDINGSTOREY")
        .collect();
    storeys.sort_by(|a, b| mapper.level_of(a).total_cmp(&mapper.level_of(b)));
    for o in &storeys {
        let building = {
            let mut cur = *o;
            let mut found = 0;
            for _ in 0..16 {
                let Some(parent) = cur.aggregate_parent.and_then(|p| views.object(p)) else {
                    break;
                };
                if let Some(b) = mapper.building_of.get(&parent.instance) {
                    found = *b;
                    break;
                }
                cur = parent;
            }
            found
        };
        let kept = mapper.kept(o);
        let own_building = o
            .aggregate_parent
            .and_then(|p| building_names.iter().find(|(i, _)| *i == p))
            .filter(|(i, _)| {
                views
                    .objects
                    .iter()
                    .find(|b| b.type_name == "IFCBUILDING")
                    .is_some_and(|b| b.instance != *i)
            })
            .map(|(_, n)| n.clone());
        let list = &mut mapper.buildings[building].storeys;
        let storey_name = clean(&o.name).unwrap_or_else(|| format!("Storey {}", list.len() + 1));
        list.push(StoreyPlan {
            name: match own_building {
                Some(b) => format!("{b} - {storey_name}"),
                None => storey_name,
            },
            elevation: 0.0,
            kept,
            items: Vec::new(),
        });
        let at = list.len() - 1;
        let level = o
            .elevation
            .or_else(|| o.placement.as_ref().map(|p| p.world[11]))
            .unwrap_or(0.0);
        mapper.buildings[building].storeys[at].elevation = level;
        mapper.storey_of.insert(o.instance, (building, at));
    }
    // A building without a storey gets one, so that every product has a storey to be in.
    for (at, b) in mapper.buildings.iter_mut().enumerate() {
        if b.storeys.is_empty() {
            b.storeys.push(StoreyPlan {
                name: "Level 0".to_owned(),
                elevation: 0.0,
                kept: Kept::default(),
                items: Vec::new(),
            });
            mapper
                .log
                .add(reasons::IMPORT_STOREY_ASSUMED, "IfcBuildingStorey", None, 1);
            let _ = at;
        }
    }
    // Products, kind by kind, so that walls exist before their doors and windows.
    let products: Vec<&ObjectView> = views
        .objects
        .iter()
        .filter(|o| o.kind == arc_io_ifc::ObjectKind::Product)
        .collect();
    let mut put: Vec<(usize, usize, Item)> = Vec::new();
    let mut grids: Vec<(usize, Vec<GridPlan>)> = Vec::new();
    let order = |k: Kind| match k {
        Kind::Wall => 0,
        Kind::Door | Kind::Window => 2,
        _ => 1,
    };
    let mut sorted = products.clone();
    sorted.sort_by_key(|o| (order(kind_of(&o.type_name)), o.instance));
    for o in sorted {
        options.cancel.check()?;
        let kind = kind_of(&o.type_name);
        if kind == Kind::Structure {
            // Sites, buildings and storeys are the structure; the others carry no model data.
            if !matches!(
                o.type_name.as_str(),
                "IFCSITE" | "IFCBUILDING" | "IFCBUILDINGSTOREY"
            ) {
                mapper.lose(
                    reasons::IMPORT_NOT_CARRIED,
                    &class_name(src, &o.type_name),
                    o,
                );
                mapper.count(&o.type_name.clone(), |t| {
                    t.seen += 1;
                    t.lost += 1;
                });
            }
            continue;
        }
        mapper.count(&o.type_name.clone(), |t| t.seen += 1);
        if o.global_id.as_ref().is_some_and(|g| (options.known)(g)) {
            mapper.count(&o.type_name.clone(), |t| t.matched += 1);
            continue;
        }
        let (building, storey, assumed) = mapper.place(o);
        let level = mapper.buildings[building].storeys[storey].elevation;
        if assumed {
            mapper.lose(
                reasons::IMPORT_STOREY_ASSUMED,
                &class_name(src, &o.type_name),
                o,
            );
        }
        let class = class_name(src, &o.type_name);
        let item: Option<Item> = match kind {
            Kind::Wall => mapper
                .wall(o, (building, storey), level)
                .or_else(|| Some(mapper.foreign(o, Some(reasons::IMPORT_SHAPE_NOT_RECOGNIZED)))),
            Kind::Slab => {
                let predefined = o.effective_predefined_type.as_deref().unwrap_or("");
                let roof = predefined == "ROOF";
                let flat = mapper.slab(o, level, false);
                match flat {
                    Some(item) if roof => {
                        mapper.lose(reasons::IMPORT_ROOF_AS_FLOOR, &class, o);
                        Some(item)
                    }
                    Some(item) => Some(item),
                    None => Some(mapper.foreign(o, Some(reasons::IMPORT_SHAPE_NOT_RECOGNIZED))),
                }
            }
            Kind::Covering => {
                let ceiling = o.effective_predefined_type.as_deref() == Some("CEILING");
                match Some(())
                    .filter(|()| ceiling)
                    .and_then(|()| mapper.slab(o, level, true))
                {
                    Some(item) => Some(item),
                    None => Some(
                        mapper.foreign(o, ceiling.then_some(reasons::IMPORT_SHAPE_NOT_RECOGNIZED)),
                    ),
                }
            }
            Kind::Door | Kind::Window => match mapper.insert(o, kind == Kind::Window) {
                Some((b, s, item)) => {
                    put.push((b, s, item));
                    continue;
                }
                None => Some(mapper.foreign(o, Some(reasons::IMPORT_INSERT_UNHOSTED))),
            },
            Kind::Space => Some(mapper.room(o, level)),
            Kind::Grid => {
                let list = mapper.grid(o);
                if list.is_empty() {
                    mapper.lose(reasons::IMPORT_NOT_CARRIED, &class, o);
                    mapper.count(&o.type_name.clone(), |t| t.lost += 1);
                } else {
                    mapper.count(&o.type_name.clone(), |t| t.native += 1);
                    mapper.lose(reasons::IMPORT_GRID_IDENTITY, &class, o);
                    mapper.key(o);
                    grids.push((building, list));
                }
                None
            }
            Kind::Opening => {
                // An opening is carried by its host: cut into the wall's body when the wall is a
                // foreign element, and by the door or window that fills it otherwise.
                if o.filled_by.is_empty() {
                    mapper.lose(reasons::IMPORT_OPENING_UNFILLED, &class, o);
                    mapper.count(&o.type_name.clone(), |t| t.lost += 1);
                } else {
                    mapper.count(&o.type_name.clone(), |t| t.native += 1);
                }
                None
            }
            Kind::Structure => None,
            Kind::Other => {
                if is_element(src, &o.type_name) {
                    Some(mapper.foreign(o, None))
                } else {
                    mapper.lose(reasons::IMPORT_NOT_CARRIED, &class, o);
                    mapper.count(&o.type_name.clone(), |t| t.lost += 1);
                    None
                }
            }
        };
        if let Some(item) = item {
            if o.aggregate_parent.is_some() && o.container.is_none() {
                mapper.lose(reasons::IMPORT_AGGREGATION_FLATTENED, &class, o);
            }
            put.push((building, storey, item));
        }
    }
    for (building, storey, item) in put {
        mapper.buildings[building].storeys[storey].items.push(item);
    }
    for (building, list) in grids {
        mapper.buildings[building].grids.extend(list);
    }
    // Types without an occurrence and sites beyond the one the document has are listed.
    let used: BTreeSet<u64> = views.objects.iter().filter_map(|o| o.type_object).collect();
    for o in views
        .objects
        .iter()
        .filter(|o| o.kind == arc_io_ifc::ObjectKind::Type && !used.contains(&o.instance))
    {
        mapper.lose(
            reasons::IMPORT_TYPE_UNUSED,
            &class_name(src, &o.type_name),
            o,
        );
    }
    for o in views.objects.iter().filter(|o| o.type_name == "IFCSITE") {
        if o.latitude_longitude.is_some() {
            mapper.lose(reasons::IMPORT_SITE_DATA, "IfcSite", o);
        }
    }
    let project_object = views.objects.iter().find(|o| o.type_name == "IFCPROJECT");
    let project = project_object.map_or_else(ProjectPlan::default, |o| ProjectPlan {
        name: clean(&o.name),
        long_name: src
            .attribute(o.instance, "LongName")
            .and_then(|p| text(Some(p)))
            .filter(|t| !t.trim().is_empty()),
        description: clean(&o.description),
        phase: src
            .attribute(o.instance, "Phase")
            .and_then(|p| text(Some(p)))
            .filter(|t| !t.trim().is_empty()),
        geo: views.georeference.as_ref().and_then(|g| {
            let crs = g.crs.as_ref().and_then(|c| c.name.clone())?;
            let u = g.crs.as_ref().map_or(1.0, |c| c.map_unit_to_meters);
            Some(GeoPlan {
                crs,
                eastings: g.eastings * u,
                northings: g.northings * u,
                height: g.orthogonal_height * u,
                true_north: (-g.x_axis_ordinate).atan2(g.x_axis_abscissa),
                scale: g.scale,
            })
        }),
        kept: mapper.kept(o),
    });
    let site_name = views
        .objects
        .iter()
        .find(|o| o.type_name == "IFCSITE")
        .and_then(|o| clean(&o.name));
    // Groups, zones and systems with the members that became elements.
    let groups: Vec<GroupPlan> = views
        .groups
        .iter()
        .filter_map(|g| {
            let members: Vec<Key> = g
                .members
                .iter()
                .filter_map(|m| mapper.made.get(m).copied())
                .collect();
            (!members.is_empty()).then(|| GroupPlan {
                name: clean(&g.name).unwrap_or_else(|| "Group".to_owned()),
                members,
            })
        })
        .collect();
    let mut report = src.report.clone();
    let log = std::mem::take(&mut mapper.log);
    let mine = log.finish(LossHeader::new(
        "ifc",
        src.file.schema.identifier(),
        Direction::Import,
        Some(options.limits),
    ));
    // The reader lists a product class as preserved; the import's own items say what became of
    // each product, so the classes it handled are left out of the reader's list.
    report.items.retain(|i| {
        let handled = i.reason == "io.ifc.entity_preserved"
            && i.record_kind
                .as_deref()
                .is_some_and(|k| mapper.tally.contains_key(k));
        if handled {
            let count = match i.arguments.get("count") {
                Some(arc_io_common::loss::Argument::Int(n)) => u64::try_from(*n).unwrap_or(0),
                _ => 0,
            };
            report.summary.preserved = report.summary.preserved.saturating_sub(count);
        }
        !handled
    });
    read::merge(&mut report, mine);
    report.summary.mapped += mapper.tally.values().map(|t| t.native).sum::<u64>();
    report
        .items
        .sort_by(|a, b| (&a.reason, &a.record_kind).cmp(&(&b.reason, &b.record_kind)));
    Ok(Plan {
        schema: src.file.schema.identifier().to_owned(),
        project,
        site_name,
        buildings: mapper.buildings,
        groups,
        report,
        tally: mapper.tally,
        identities: mapper.identities,
    })
}

/// Whether the class is an `IfcElement` of the file's schema.
fn is_element(src: &Source, upper: &str) -> bool {
    let Some(tables) = src.file.tables() else {
        return true;
    };
    match (
        tables.entity_index(upper),
        tables.entity_index("IFCELEMENT"),
    ) {
        (Some(entity), Some(element)) => tables.is_subtype_of(entity, element),
        _ => false,
    }
}
