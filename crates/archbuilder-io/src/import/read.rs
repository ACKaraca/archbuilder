//! Reading an IFC file for an import: the file, its typed and import views and the geometry of
//! its products, with the reports of the three merged into one loss report.

use std::collections::HashMap;

use arc_base::{Cancel, Limits, Result, Tolerance};
use arc_io_common::loss::{Direction, LossHeader, LossReport};
use arc_io_ifc::{
    Geometry, GeometryItem, GeometryOptions, IfcFile, Parameter, ReadOptions, Record, TypedViews,
    read,
};

/// What an import reads from the file.
pub struct Source {
    /// The parsed file.
    pub file: IfcFile,
    /// The typed views and the import views.
    pub views: TypedViews,
    /// The geometry with the bodies as the file gives them (openings not subtracted).
    pub solid: Geometry,
    /// The geometry with the openings subtracted, when the file has any.
    pub cut: Option<Geometry>,
    solid_index: HashMap<u64, Vec<usize>>,
    cut_index: HashMap<u64, Vec<usize>>,
    /// The merged loss report of the reading.
    pub report: LossReport,
}

fn index(geometry: &Geometry) -> HashMap<u64, Vec<usize>> {
    let mut by: HashMap<u64, Vec<usize>> = HashMap::new();
    for (at, item) in geometry.items.iter().enumerate() {
        by.entry(item.product).or_default().push(at);
    }
    by
}

/// Adds the items and counts of `other` to `into`.
pub fn merge(into: &mut LossReport, other: LossReport) {
    into.summary.mapped += other.summary.mapped;
    into.summary.approximated += other.summary.approximated;
    into.summary.preserved += other.summary.preserved;
    into.summary.dropped += other.summary.dropped;
    into.summary.failed += other.summary.failed;
    into.truncated |= other.truncated;
    into.items.extend(other.items);
}

/// Reads `bytes` (an IFC file or an IFC-ZIP archive).
///
/// # Errors
/// The reader's errors (`io.*`), `base.cancelled`, and the limits.
pub fn open(bytes: &[u8], limits: &Limits, cancel: &Cancel) -> Result<Source> {
    let mut options = ReadOptions::with_limits(*limits);
    options.cancel = cancel.clone();
    let output = read(bytes, &mut options)?;
    let file = output.file;
    let mut views = file.typed_views(limits, cancel)?;
    let mut report = LossReport {
        header: LossHeader::new(
            "ifc",
            file.schema.identifier(),
            Direction::Import,
            Some(*limits),
        ),
        summary: output.report.summary,
        items: output.report.items,
        truncated: output.report.truncated,
    };
    merge(
        &mut report,
        std::mem::replace(&mut views.report, empty_report(limits)),
    );
    let tolerance = Tolerance::DEFAULT;
    let solid = file.geometry_with_options(
        limits,
        cancel,
        &tolerance,
        &GeometryOptions {
            subtract_openings: false,
        },
    )?;
    let voids = views.import.objects.iter().any(|o| !o.openings.is_empty());
    let cut = if voids {
        Some(file.geometry_with_options(
            limits,
            cancel,
            &tolerance,
            &GeometryOptions {
                subtract_openings: true,
            },
        )?)
    } else {
        None
    };
    // The reports of the evaluation count the products once: the cut evaluation's is the one kept.
    let evaluation = cut.as_ref().unwrap_or(&solid).report.clone();
    merge(&mut report, evaluation);
    Ok(Source {
        solid_index: index(&solid),
        cut_index: cut.as_ref().map(index).unwrap_or_default(),
        file,
        views,
        solid,
        cut,
        report,
    })
}

fn empty_report(limits: &Limits) -> LossReport {
    LossReport {
        header: LossHeader::new("ifc", "", Direction::Import, Some(*limits)),
        summary: arc_io_common::loss::LossSummary::default(),
        items: Vec::new(),
        truncated: false,
    }
}

impl Source {
    /// The evaluated items of the body of `product`, bodies as the file gives them.
    #[must_use]
    pub fn solid_items(&self, product: u64) -> Vec<&GeometryItem> {
        pick(&self.solid, self.solid_index.get(&product), false)
    }

    /// The evaluated items of the body of `product` with the openings subtracted.
    #[must_use]
    pub fn cut_items(&self, product: u64) -> Vec<&GeometryItem> {
        match &self.cut {
            Some(cut) => pick(cut, self.cut_index.get(&product), false),
            None => self.solid_items(product),
        }
    }

    /// The record of an instance.
    #[must_use]
    pub fn record(&self, name: u64) -> Option<&Record> {
        self.file.instance(name)?.records.first()
    }

    /// The attribute `name` of an instance.
    #[must_use]
    pub fn attribute(&self, instance: u64, name: &str) -> Option<&Parameter> {
        self.file.attribute(self.record(instance)?, name)
    }

    /// The meters per length unit of the file.
    #[must_use]
    pub fn meters(&self) -> f64 {
        self.views.import.units.length_to_meters
    }
}

/// The items of `ids`: the body items, or the axis items when `axis`. An item is of the body when
/// its representation says so or says nothing; polylines are never body.
fn pick<'a>(geometry: &'a Geometry, ids: Option<&Vec<usize>>, axis: bool) -> Vec<&'a GeometryItem> {
    let Some(ids) = ids else {
        return Vec::new();
    };
    let is_axis = |i: &GeometryItem| i.representation_identifier.as_deref() == Some("Axis");
    ids.iter()
        .filter_map(|&at| geometry.items.get(at))
        .filter(|i| {
            if axis {
                is_axis(i)
            } else {
                !is_axis(i)
                    && matches!(i.shape.as_ref(), arc_io_ifc::Shape::Mesh(_))
                    && !matches!(
                        i.representation_identifier.as_deref(),
                        Some("Box" | "FootPrint" | "Annotation" | "Profile")
                    )
            }
        })
        .collect()
}

/// A reference parameter.
#[must_use]
pub fn reference(p: Option<&Parameter>) -> Option<u64> {
    match p {
        Some(Parameter::Reference(n)) => Some(*n),
        _ => None,
    }
}

/// A number parameter, read through a type wrapper.
#[must_use]
pub fn number(p: Option<&Parameter>) -> Option<f64> {
    match p? {
        #[allow(clippy::cast_precision_loss, reason = "a measure, not an identifier")]
        Parameter::Integer(i) => Some(*i as f64),
        Parameter::Real(r) => Some(*r),
        Parameter::Typed { value, .. } => number(Some(value)),
        _ => None,
    }
}

/// A text parameter.
#[must_use]
pub fn text(p: Option<&Parameter>) -> Option<String> {
    match p? {
        Parameter::Text(t) => Some(t.clone()),
        Parameter::Enumeration(t) => Some(t.clone()),
        Parameter::Typed { value, .. } => text(Some(value)),
        _ => None,
    }
}

/// The references of a list parameter.
#[must_use]
pub fn references(p: Option<&Parameter>) -> Vec<u64> {
    match p {
        Some(Parameter::List(items)) => items
            .iter()
            .filter_map(|i| match i {
                Parameter::Reference(n) => Some(*n),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}
