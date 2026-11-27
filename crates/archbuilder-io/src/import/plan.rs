//! The plan of an import: what the file maps to, as plain data. The mapper builds it from the
//! file; the writer turns it into objects of the document inside one transaction.

use arc_io_common::loss::LossReport;

use super::data::Kept;

/// The index of a planned element, for references between planned elements (a door's wall).
pub type Key = usize;

/// The type a wall, floor or ceiling gets: a name and its layers from the exterior face (walls) or
/// the top (floors, ceilings).
#[derive(Debug, Clone, PartialEq)]
pub struct LayeredSpec {
    /// The type name.
    pub name: String,
    /// The material and thickness of each layer, in meters.
    pub layers: Vec<(String, f64)>,
    /// Index of the core layer (walls).
    pub core: usize,
    /// Whether the wall is external (the function).
    pub external: bool,
    /// Whether the wall carries load.
    pub load_bearing: bool,
}

/// The type of a door or window.
#[derive(Debug, Clone, PartialEq)]
pub struct InsertSpec {
    /// The type name.
    pub name: String,
    /// The width of the opening, meters.
    pub width: f64,
    /// The height of the opening, meters.
    pub height: f64,
    /// The operation code of the model (`Operation`).
    pub operation: u32,
}

/// A wall of the model.
#[derive(Debug, Clone, PartialEq)]
pub struct WallPlan {
    pub key: Key,
    pub name: String,
    pub start: [f64; 2],
    pub end: [f64; 2],
    pub thickness: f64,
    pub height: f64,
    /// The base above the storey's level.
    pub base_offset: f64,
    pub spec: LayeredSpec,
    /// Whether the start end is slanted (mitered): it joins with its neighbors.
    pub start_auto: bool,
    /// Whether the end end is slanted.
    pub end_auto: bool,
    pub kept: Kept,
}

/// A door or window of the model, hosted by a planned wall.
#[derive(Debug, Clone, PartialEq)]
pub struct InsertPlan {
    pub key: Key,
    pub window: bool,
    pub name: String,
    pub host: Key,
    pub distance: f64,
    pub width: f64,
    pub height: f64,
    pub sill: f64,
    pub right_hand: bool,
    pub spec: InsertSpec,
    pub kept: Kept,
}

/// A floor, or a ceiling, of the model: a boundary between two horizontal planes.
#[derive(Debug, Clone, PartialEq)]
pub struct SlabPlan {
    pub key: Key,
    pub name: String,
    pub thickness: f64,
    /// The top (floor) or the underside (ceiling) above the storey's level.
    pub offset: f64,
    pub outer: Vec<[f64; 2]>,
    pub holes: Vec<Vec<[f64; 2]>>,
    pub spec: LayeredSpec,
    pub kept: Kept,
}

/// A room of the model.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomPlan {
    pub key: Key,
    pub name: String,
    pub number: Option<String>,
    pub point: [f64; 2],
    pub base_offset: f64,
    pub kept: Kept,
}

/// A grid line of the model.
#[derive(Debug, Clone, PartialEq)]
pub struct GridPlan {
    pub key: Key,
    pub name: String,
    pub points: Vec<[f64; 2]>,
    pub kept: Kept,
}

/// A product that has no native counterpart: its source class, its data and its mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct ForeignPlan {
    pub key: Key,
    pub name: String,
    pub class: String,
    pub mesh: Option<Vec<u8>>,
    pub kept: Kept,
}

/// A native element or a foreign one, in a storey.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Wall(WallPlan),
    Insert(InsertPlan),
    Floor(SlabPlan),
    Ceiling(SlabPlan),
    Room(RoomPlan),
    Foreign(ForeignPlan),
}

impl Item {
    /// The key of the element.
    #[must_use]
    pub fn key(&self) -> Key {
        match self {
            Self::Wall(w) => w.key,
            Self::Insert(i) => i.key,
            Self::Floor(s) | Self::Ceiling(s) => s.key,
            Self::Room(r) => r.key,
            Self::Foreign(f) => f.key,
        }
    }
}

/// A storey with its elements.
#[derive(Debug, Clone, PartialEq)]
pub struct StoreyPlan {
    pub name: String,
    pub elevation: f64,
    pub kept: Kept,
    pub items: Vec<Item>,
}

/// A building with its storeys.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildingPlan {
    pub name: String,
    pub address: Option<String>,
    pub elevation: f64,
    pub kept: Kept,
    pub storeys: Vec<StoreyPlan>,
    pub grids: Vec<GridPlan>,
    /// Foreign elements that belong to the building, not to a storey.
    pub foreign: Vec<ForeignPlan>,
}

/// The georeference of the project.
#[derive(Debug, Clone, PartialEq)]
pub struct GeoPlan {
    pub crs: String,
    pub eastings: f64,
    pub northings: f64,
    pub height: f64,
    /// Counterclockwise from project north to true north, radians.
    pub true_north: f64,
    pub scale: f64,
}

/// A group of the source and its members.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupPlan {
    pub name: String,
    pub members: Vec<Key>,
}

/// The project.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProjectPlan {
    pub name: Option<String>,
    pub long_name: Option<String>,
    pub description: Option<String>,
    pub phase: Option<String>,
    pub geo: Option<GeoPlan>,
    pub kept: Kept,
}

/// The counts of one entity class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tally {
    /// Products of the class in the file.
    pub seen: u64,
    /// Mapped to native elements.
    pub native: u64,
    /// Kept as foreign elements.
    pub foreign: u64,
    /// Not carried (listed in the loss report as dropped).
    pub lost: u64,
    /// Matched to objects the document already has, by identifier.
    pub matched: u64,
}

/// The plan of an import.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub schema: String,
    pub project: ProjectPlan,
    pub site_name: Option<String>,
    pub buildings: Vec<BuildingPlan>,
    pub groups: Vec<GroupPlan>,
    pub report: LossReport,
    pub tally: std::collections::BTreeMap<String, Tally>,
    /// The source identities and keys of everything planned: `(GlobalId, key)`.
    pub identities: Vec<(String, Key)>,
}
