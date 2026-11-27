//! Export self-validation: the written IFC bytes are read back with the reader of `arc-io-ifc`
//! and checked against the model that was written. A problem is a loss item with the outcome
//! `failed`, so the export reports it instead of succeeding silently. The checks:
//!
//! - the file reads back;
//! - every object of the model (project, site, building, storeys, elements, element types) is in
//!   the file under its `GlobalId`;
//! - no two rooted entities of the file share a `GlobalId`;
//! - the products of the file (walls, slabs, roofs, coverings, doors, windows, spaces, openings,
//!   stairs and their flights, ramps and their flights, railings, curtain walls, plates, members
//!   and grids) number as many as the elements and grids written;
//! - when the build has the schema tables of the file's schema, every rule of the conformance
//!   check of the reader (levels 1 to 3) holds: each failed rule is a failed item. Without the
//!   tables nothing is added and the caller is told that conformance was not checked.

use std::collections::{BTreeMap, BTreeSet};

use arc_base::id::Identifier;
use arc_base::{Cancel, Limits};
use arc_io_common::loss::{Argument, LossItem, LossReport, Reason};
use arc_io_ifc::write::{ElementKind, global_id};
use arc_io_ifc::{Finding, Parameter, ReadOptions, WriteModel, read};

use crate::error::reasons;

const ALPHABET: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$";

/// Whether `entity` is an entity of a product the export counts.
fn is_product(entity: &str) -> bool {
    [
        "IFCWALL",
        "IFCSLAB",
        "IFCROOF",
        "IFCCOVERING",
        "IFCSPACE",
        "IFCOPENINGELEMENT",
        "IFCSTAIR",
        "IFCRAMP",
        "IFCRAILING",
        "IFCCURTAINWALL",
        "IFCPLATE",
        "IFCMEMBER",
        "IFCTRANSPORTELEMENT",
        "IFCPIPESEGMENT",
    ]
    .iter()
    .any(|p| entity.starts_with(p) && !entity.ends_with("TYPE"))
        || matches!(entity, "IFCDOOR" | "IFCWINDOW" | "IFCGRID")
}

/// Whether `entity` has a `GlobalId` as its first attribute and is one the writer makes.
fn is_rooted(entity: &str) -> bool {
    entity.starts_with("IFCREL")
        || entity.ends_with("TYPE")
        || is_product(entity)
        || matches!(
            entity,
            "IFCPROJECT"
                | "IFCSITE"
                | "IFCBUILDING"
                | "IFCBUILDINGSTOREY"
                | "IFCOPENINGELEMENT"
                | "IFCGROUP"
                | "IFCPROPERTYSET"
                | "IFCELEMENTQUANTITY"
        )
}

fn item(reason: Reason, kind: &str, id: Option<u128>, count: u64) -> LossItem {
    let mut arguments = BTreeMap::new();
    arguments.insert(
        "count".to_owned(),
        Argument::Int(i64::try_from(count).unwrap_or(i64::MAX)),
    );
    LossItem {
        outcome: reason.outcome,
        severity: reason.severity,
        source_id: None,
        record_kind: Some(kind.to_owned()),
        object_id: id.map(|id| arc_base::ObjectId::from_u128(id).to_string()),
        reason: reason.code.to_owned(),
        message_id: reason.message_id.to_owned(),
        arguments,
    }
}

/// What the self-validation established.
pub(crate) struct Validation {
    /// Whether every check that ran passed.
    pub valid: bool,
    /// The schema the conformance check ran against; `None` when it did not run (no tables in
    /// the build, or a limit stopped it), which is not a pass.
    pub conformance: Option<&'static str>,
}

/// The failed item of one finding of the conformance check.
fn conformance_item(finding: &Finding) -> LossItem {
    let mut out = item(reasons::VALIDATION_CONFORMANCE, &finding.entity, None, 1);
    let text = |s: &str| Argument::Text(s.to_owned());
    out.arguments
        .insert("rule".to_owned(), text(finding.rule.code));
    out.arguments
        .insert("entity".to_owned(), text(&finding.entity));
    out.arguments.insert(
        "instance".to_owned(),
        Argument::Int(i64::try_from(finding.instance).unwrap_or(i64::MAX)),
    );
    out.arguments.insert(
        "attribute".to_owned(),
        text(finding.attribute.as_deref().unwrap_or_default()),
    );
    out
}

/// Reads `bytes` back and adds a failed item to `report` for each problem.
pub(crate) fn validate(
    bytes: &[u8],
    model: &WriteModel,
    (limits, cancel): (Limits, &Cancel),
    report: &mut LossReport,
    foreign: &BTreeSet<String>,
) -> Validation {
    let mut conformance = None;
    let mut found: Vec<LossItem> = Vec::new();
    match read(bytes, &mut ReadOptions::with_limits(limits)) {
        Err(_) => found.push(item(reasons::VALIDATION_UNREADABLE, "IFC", None, 1)),
        Ok(output) => {
            let file = output.file;
            let mut seen: BTreeSet<String> = BTreeSet::new();
            let mut repeated: BTreeSet<String> = BTreeSet::new();
            let mut products = 0usize;
            for instance in file.instances() {
                for record in &instance.records {
                    // A foreign element of an import is written under its source class.
                    let is_foreign = matches!(record.parameters.first(),
                        Some(Parameter::Text(id)) if foreign.contains(id));
                    if is_product(&record.type_name) && !is_foreign {
                        products += 1;
                    }
                    if !is_rooted(&record.type_name) && !is_foreign {
                        continue;
                    }
                    if let Some(Parameter::Text(id)) = record.parameters.first()
                        && id.len() == 22
                        && id.chars().all(|c| ALPHABET.contains(c))
                        && !seen.insert(id.clone())
                    {
                        repeated.insert(id.clone());
                    }
                }
            }
            let expected = std::iter::once((&model.project.id, "IFCPROJECT"))
                .chain([
                    (&model.site.id, "IFCSITE"),
                    (&model.building.id, "IFCBUILDING"),
                ])
                .chain(model.storeys.iter().map(|s| (&s.id, "IFCBUILDINGSTOREY")))
                .chain(model.elements.iter().map(|e| (&e.id, kind_of(e.kind))))
                .chain(model.grids.iter().map(|g| (&g.id, "IFCGRID")))
                .chain(model.groups.iter().map(|g| (&g.id, "IFCGROUP")))
                .chain(model.types.iter().map(|t| (&t.id, "IFCTYPEOBJECT")));
            for (id, kind) in expected {
                if !seen.contains(&global_id(*id)) {
                    found.push(item(reasons::VALIDATION_MISSING, kind, Some(*id), 1));
                }
            }
            for id in &repeated {
                let mut duplicate = item(reasons::VALIDATION_DUPLICATE_ID, "IFC", None, 1);
                duplicate
                    .arguments
                    .insert("name".to_owned(), Argument::Text(id.clone()));
                found.push(duplicate);
            }
            // Conformance needs the schema tables, which a build may not have: then it is not
            // checked, and nothing is reported for it.
            if let Ok(checked) = file.conformance(&limits, cancel)
                && checked.schema.is_some()
            {
                conformance = checked.schema;
                found.extend(checked.findings.iter().map(conformance_item));
            }
            let exported =
                model.elements.len() - foreign.len().min(model.elements.len()) + model.grids.len();
            if products != exported {
                let mut count = item(reasons::VALIDATION_COUNT, "IFC", None, 1);
                count.arguments.insert(
                    "found".to_owned(),
                    Argument::Int(i64::try_from(products).unwrap_or(i64::MAX)),
                );
                count.arguments.insert(
                    "expected".to_owned(),
                    Argument::Int(i64::try_from(exported).unwrap_or(i64::MAX)),
                );
                found.push(count);
            }
        }
    }
    let valid = found.is_empty();
    report.summary.failed += found.len() as u64;
    report.items.extend(found);
    Validation { valid, conformance }
}

fn kind_of(kind: ElementKind) -> &'static str {
    match kind {
        ElementKind::Wall => "IFCWALL",
        ElementKind::Slab => "IFCSLAB",
        ElementKind::Door => "IFCDOOR",
        ElementKind::Window => "IFCWINDOW",
        ElementKind::Opening => "IFCOPENINGELEMENT",
        ElementKind::Roof => "IFCROOF",
        ElementKind::Space => "IFCSPACE",
        ElementKind::Covering => "IFCCOVERING",
        ElementKind::Stair => "IFCSTAIR",
        ElementKind::StairFlight => "IFCSTAIRFLIGHT",
        ElementKind::Railing => "IFCRAILING",
        ElementKind::Ramp => "IFCRAMP",
        ElementKind::RampFlight => "IFCRAMPFLIGHT",
        ElementKind::CurtainWall => "IFCCURTAINWALL",
        ElementKind::Plate => "IFCPLATE",
        ElementKind::Member => "IFCMEMBER",
        ElementKind::TransportElement => "IFCTRANSPORTELEMENT",
        ElementKind::PipeSegment => "IFCPIPESEGMENT",
    }
}

#[cfg(test)]
mod tests {
    use arc_io_ifc::write::{Element, Spatial, Storey};
    use arc_io_ifc::{WriteOptions, write};

    use super::*;

    fn model() -> WriteModel {
        let mut model = WriteModel::new(
            Spatial::new(1, "P"),
            Spatial::new(2, "S"),
            Spatial::new(3, "B"),
        );
        model.storeys.push(Storey::new(4, "L0", 0.0));
        model
            .elements
            .push(Element::new(5, ElementKind::Wall, "W", 4));
        model
    }

    fn check(bytes: &[u8], model: &WriteModel) -> (bool, LossReport) {
        let written = write(model, &WriteOptions::default()).unwrap();
        let mut report = written.report;
        let _ = bytes;
        let ok = validate(
            bytes,
            model,
            (Limits::DEFAULT, &Cancel::new()),
            &mut report,
            &BTreeSet::new(),
        )
        .valid;
        (ok, report)
    }

    #[test]
    fn a_written_model_is_valid_and_each_failure_is_a_failed_item() {
        let model = model();
        let bytes = write(&model, &WriteOptions::default()).unwrap().bytes;
        assert!(check(&bytes, &model).0);
        // An object the file does not hold: it is missing and the product count differs.
        let mut more = model.clone();
        more.elements
            .push(Element::new(6, ElementKind::Wall, "X", 4));
        let (ok, report) = check(&bytes, &more);
        assert!(!ok && report.summary.failed == 2);
        assert!(
            report
                .items
                .iter()
                .any(|i| i.reason == "ab.io.ifc.validation_missing")
        );
        // Bytes that do not read back.
        let (ok, report) = check(b"not an ifc file", &model);
        assert!(
            !ok && report
                .items
                .iter()
                .any(|i| i.reason == "ab.io.ifc.validation_unreadable")
        );
    }
}
