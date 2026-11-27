//! The project information in the IFC file (AB-DATUM-002): the long name, description and phase
//! of the `IfcProject`, the postal address of the site and the building, the author and the
//! issuing organization and the client as actors assigned to the project, and the properties of
//! `Pset_BuildingCommon` that the IFC4 property set templates define (`Reference`,
//! `OccupancyType`, `YearOfConstruction`, `ConstructionMethod`).
//!
//! The writer of the foundation writes the project, the site and the building with their names
//! only, so the values are added to its output: the three attribute lists are completed and the
//! new instances are appended with the next free names. The file is read back and checked after
//! this, like every export. IFC4 has no `Pset_ProjectCommon`, and the issue date has no
//! attribute, so the issue date is reported as not written.

use std::fmt::Write as _;

use arc_doc::Object;
use archbuilder_model::types::{self, building, project};

/// The values to write; every one is optional.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Info {
    pub number: Option<String>,
    pub client: Option<String>,
    /// The address of the site (the project's).
    pub site_address: Option<String>,
    /// The address of the building: its own, else the project's.
    pub building_address: Option<String>,
    pub status: Option<String>,
    pub author: Option<String>,
    pub organization: Option<String>,
    pub description: Option<String>,
    pub long_name: Option<String>,
    pub occupancy_type: Option<String>,
    pub year_of_construction: Option<String>,
    pub construction_method: Option<String>,
    pub issue_date: Option<String>,
}

impl Info {
    pub(crate) fn of(project_object: &Object, building_object: &Object) -> Self {
        let get = |o: &Object, at: usize| {
            types::text(&o.values, at)
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_owned)
        };
        let address = get(project_object, project::ADDRESS);
        Self {
            number: get(project_object, project::NUMBER),
            client: get(project_object, project::CLIENT),
            site_address: address.clone(),
            building_address: get(building_object, building::ADDRESS).or(address),
            status: get(project_object, project::STATUS),
            author: get(project_object, project::AUTHOR),
            organization: get(project_object, project::ORGANIZATION),
            description: get(project_object, project::DESCRIPTION),
            long_name: get(project_object, project::LONG_NAME),
            occupancy_type: get(project_object, project::OCCUPANCY_TYPE),
            year_of_construction: get(project_object, project::YEAR_OF_CONSTRUCTION),
            construction_method: get(project_object, project::CONSTRUCTION_METHOD),
            issue_date: get(project_object, project::ISSUE_DATE),
        }
    }

    /// Whether any value reaches the file.
    fn writes_anything(&self) -> bool {
        let Self {
            issue_date: _,
            number,
            client,
            site_address,
            building_address,
            status,
            author,
            organization,
            description,
            long_name,
            occupancy_type,
            year_of_construction,
            construction_method,
        } = self;
        [
            number,
            client,
            site_address,
            building_address,
            status,
            author,
            organization,
            description,
            long_name,
            occupancy_type,
            year_of_construction,
            construction_method,
        ]
        .iter()
        .any(|v| v.is_some())
    }
}

/// A STEP string: printable ASCII as it is, the apostrophe and the backslash doubled, every other
/// character in `\X2\` or `\X4\` runs (ISO 10303-21).
fn text(content: &str) -> String {
    let mut out = String::from("'");
    let mut run: Option<bool> = None;
    for c in content.chars().filter(|c| *c != '\0') {
        let wanted = if (' '..='~').contains(&c) {
            None
        } else {
            Some(u32::from(c) > 0xFFFF)
        };
        if wanted != run {
            if run.is_some() {
                out.push_str("\\X0\\");
            }
            match wanted {
                Some(true) => out.push_str("\\X4\\"),
                Some(false) => out.push_str("\\X2\\"),
                None => {}
            }
            run = wanted;
        }
        match (wanted, c) {
            (None, '\'') => out.push_str("''"),
            (None, '\\') => out.push_str("\\\\"),
            (None, _) => out.push(c),
            (Some(true), _) => {
                let _ = write!(out, "{:08X}", u32::from(c));
            }
            (Some(false), _) => {
                let _ = write!(out, "{:04X}", u32::from(c));
            }
        }
    }
    if run.is_some() {
        out.push_str("\\X0\\");
    }
    out.push('\'');
    out
}

fn opt(value: Option<&String>) -> String {
    value.map_or_else(|| "$".to_owned(), |v| text(v))
}

/// The top-level parameters of a record's parameter list (the text between the outer brackets).
fn split_parameters(list: &str) -> Vec<&str> {
    let (mut parts, mut depth, mut in_text, mut start) = (Vec::new(), 0_i32, false, 0);
    for (at, c) in list.char_indices() {
        match c {
            '\'' => in_text = !in_text,
            '(' if !in_text => depth += 1,
            ')' if !in_text => depth -= 1,
            ',' if !in_text && depth == 0 => {
                parts.push(&list[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    parts.push(&list[start..]);
    parts
}

/// The name of an instance line `#12=IFCX(...);`: its number and its parameters.
fn instance(line: &str) -> Option<(u64, &str, &str)> {
    let rest = line.strip_prefix('#')?;
    let (number, rest) = rest.split_once('=')?;
    let (type_name, rest) = rest.split_once('(')?;
    let list = rest.strip_suffix(");")?;
    Some((number.parse().ok()?, type_name, list))
}

/// The postal address entity of an address text: one address line per line of the text.
fn address_lines(address: &str) -> String {
    let lines: Vec<String> = address
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(text)
        .collect();
    format!("({})", lines.join(","))
}

/// The Global ID of an instance this module adds: the project's identifier with a tag in its top
/// bits, so it is the same on every export and apart from every object identifier.
fn global_id(project_id: u128, tag: u128) -> String {
    arc_io_ifc::global_id(project_id ^ ((0xA5_u128 << 120) | tag))
}

/// Writes `info` into the IFC4 file `bytes`. `None` when the file is not the shape the writer
/// produces (a missing project, site or building), so the caller reports it.
#[allow(
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    reason = "one list of entities after another, in the order they refer to each other"
)]
pub(crate) fn apply(bytes: &[u8], info: &Info, project_id: u128) -> Option<Vec<u8>> {
    if !info.writes_anything() {
        return Some(bytes.to_vec());
    }
    let source = std::str::from_utf8(bytes).ok()?;
    let mut lines: Vec<String> = source.lines().map(str::to_owned).collect();
    let end = lines.iter().rposition(|l| l.trim() == "ENDSEC;")?;
    let mut next = lines
        .iter()
        .filter_map(|l| instance(l).map(|(n, _, _)| n))
        .max()?
        + 1;
    let mut added: Vec<String> = Vec::new();
    let mut add = |added: &mut Vec<String>, body: String| -> u64 {
        let id = next;
        next += 1;
        added.push(format!("#{id}={body};"));
        id
    };
    let mut ids = (None, None, None);
    for line in &lines {
        match instance(line) {
            Some((n, "IFCPROJECT", _)) => ids.0 = Some(n),
            Some((n, "IFCSITE", _)) => ids.1 = Some(n),
            Some((n, "IFCBUILDING", _)) => ids.2 = Some(n),
            _ => {}
        }
    }
    let (project_n, site_n, building_n) = (ids.0?, ids.1?, ids.2?);

    // Addresses of the site and the building.
    let mut postal = |added: &mut Vec<String>, address: &Option<String>| {
        address.as_ref().map(|a| {
            add(
                added,
                format!("IFCPOSTALADDRESS($,$,$,$,{},$,$,$,$,$)", address_lines(a)),
            )
        })
    };
    let site_address = postal(&mut added, &info.site_address);
    let building_address = postal(&mut added, &info.building_address);

    // The actors: the client, and the author with the issuing organization.
    let mut actors: Vec<u64> = Vec::new();
    let mut tag = 1_u128;
    if let Some(client) = &info.client {
        let role = add(&mut added, "IFCACTORROLE(.CLIENT.,$,$)".to_owned());
        let org = add(
            &mut added,
            format!(
                "IFCORGANIZATION($,{},$,({}),$)",
                text(client),
                format_args!("#{role}")
            ),
        );
        actors.push(add(
            &mut added,
            format!(
                "IFCACTOR('{}',$,{},$,$,#{org})",
                global_id(project_id, tag),
                text("Client")
            ),
        ));
        tag += 1;
    }
    if info.author.is_some() || info.organization.is_some() {
        let person = info.author.as_ref().map(|author| {
            let (given, family) = match author.trim().rsplit_once(' ') {
                Some((given, family)) => (Some(given.trim().to_owned()), family.to_owned()),
                None => (None, author.trim().to_owned()),
            };
            add(
                &mut added,
                format!(
                    "IFCPERSON($,{},{},$,$,$,$,$)",
                    text(&family),
                    opt(given.as_ref())
                ),
            )
        });
        let org = info
            .organization
            .as_ref()
            .map(|o| add(&mut added, format!("IFCORGANIZATION($,{},$,$,$)", text(o))));
        let the_actor = match (person, org) {
            (Some(p), Some(o)) => add(&mut added, format!("IFCPERSONANDORGANIZATION(#{p},#{o},$)")),
            (Some(p), None) => p,
            (None, Some(o)) => o,
            (None, None) => return None,
        };
        actors.push(add(
            &mut added,
            format!(
                "IFCACTOR('{}',$,{},$,$,#{the_actor})",
                global_id(project_id, tag),
                text("Author")
            ),
        ));
        tag += 1;
    }
    for actor in actors {
        add(
            &mut added,
            format!(
                "IFCRELASSIGNSTOACTOR('{}',$,$,$,(#{project_n}),$,#{actor},$)",
                global_id(project_id, tag)
            ),
        );
        tag += 1;
    }

    // Pset_BuildingCommon: only properties the IFC4 template defines, with its data types.
    let mut properties: Vec<u64> = Vec::new();
    for (name, kind, value) in [
        ("Reference", "IFCIDENTIFIER", &info.number),
        ("OccupancyType", "IFCLABEL", &info.occupancy_type),
        ("YearOfConstruction", "IFCLABEL", &info.year_of_construction),
        ("ConstructionMethod", "IFCLABEL", &info.construction_method),
    ] {
        if let Some(value) = value {
            properties.push(add(
                &mut added,
                format!(
                    "IFCPROPERTYSINGLEVALUE({},$,{kind}({}),$)",
                    text(name),
                    text(value)
                ),
            ));
        }
    }
    if !properties.is_empty() {
        let list: Vec<String> = properties.iter().map(|p| format!("#{p}")).collect();
        let set = add(
            &mut added,
            format!(
                "IFCPROPERTYSET('{}',$,{},$,({}))",
                global_id(project_id, tag),
                text("Pset_BuildingCommon"),
                list.join(",")
            ),
        );
        tag += 1;
        add(
            &mut added,
            format!(
                "IFCRELDEFINESBYPROPERTIES('{}',$,$,$,(#{building_n}),#{set})",
                global_id(project_id, tag)
            ),
        );
    }

    // Complete the three attribute lists.
    for line in &mut lines {
        let Some((n, type_name, list)) = instance(line) else {
            continue;
        };
        let mut parts: Vec<String> = split_parameters(list)
            .iter()
            .map(|p| (*p).to_owned())
            .collect();
        let set = |parts: &mut Vec<String>, at: usize, value: String| {
            if let Some(slot) = parts.get_mut(at) {
                *slot = value;
            }
        };
        match (type_name, n) {
            // GlobalId, OwnerHistory, Name, Description, ObjectType, LongName, Phase, ...
            ("IFCPROJECT", _) if n == project_n => {
                if parts.len() != 9 {
                    return None;
                }
                if let Some(d) = &info.description {
                    set(&mut parts, 3, text(d));
                }
                if let Some(l) = &info.long_name {
                    set(&mut parts, 5, text(l));
                }
                if let Some(s) = &info.status {
                    set(&mut parts, 6, text(s));
                }
            }
            // ..., LongName, CompositionType, RefLatitude, RefLongitude, RefElevation,
            // LandTitleNumber, SiteAddress
            ("IFCSITE", _) if n == site_n => {
                if parts.len() != 14 {
                    return None;
                }
                if let Some(a) = site_address {
                    set(&mut parts, 13, format!("#{a}"));
                }
            }
            // ..., LongName, CompositionType, ElevationOfRefHeight, ElevationOfTerrain,
            // BuildingAddress
            ("IFCBUILDING", _) if n == building_n => {
                if parts.len() != 12 {
                    return None;
                }
                if let Some(a) = building_address {
                    set(&mut parts, 11, format!("#{a}"));
                }
            }
            _ => continue,
        }
        *line = format!("#{n}={type_name}({});", parts.join(","));
    }
    for (offset, line) in added.into_iter().enumerate() {
        lines.insert(end + offset, line);
    }
    let mut out = lines.join("\n");
    out.push('\n');
    Some(out.into_bytes())
}
