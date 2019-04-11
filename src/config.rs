use std::{fmt,
          ops::{Deref}};

use failure::Error;

use crate::blueprint::{CellNo, Edge};
use crate::utility::{PortNumber};
use crate::uuid_ec::Uuid;

pub const SCHEMA_VERSION: &str = "0.1";
pub const REPO: &str = "CellAgent";
pub const CENTRAL_TREE: & str = "Tree:C:2";
// Sizes
pub const MAX_NUM_PHYS_PORTS_PER_CELL: PortQty = PortQty(9);          // Limit on number of ports per cell
pub const MIN_NUM_BORDER_CELLS: CellQty = CellQty(1);   // Minimum acceptable number of border cells
pub const PACKET_MIN: usize = 64;
pub const PACKET_MAX: usize = 9000;
// Control
pub const CONTINUE_ON_ERROR: bool = false; // Don't close channel following an error if true
pub const RACE_SLEEP: u64 = 4; // Set to 2 (better is 4) to avoid race condition, 0 if you want to see it
pub const AUTO_BREAK: Option<Edge> = None; //Some(Edge(CellNo(1), CellNo(2)));//  Some(Edge(CellNo(0), CellNo(1))); // Set to edge to break when debugging broken link with VSCode, else 0
#[derive(Debug, Copy, Clone, Hash, Serialize, Deserialize)]
pub enum CellConfig { Small, Medium, Large }
impl fmt::Display for CellConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match *self {
            CellConfig::Small  => "Small",
            CellConfig::Medium => "Medium",
            CellConfig::Large  => "Large"
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Quench { Simple, RootPort }
pub const QUENCH: Quench = Quench::RootPort;
pub const PAYLOAD_DEFAULT_ELEMENT: u8 = 0;
// Place to write output data
pub const OUTPUT_DIR_NAME: &str = "/tmp/multicell/";
pub const OUTPUT_FILE_NAME: &str = "/tmp/multicell/trace";
pub const KAFKA_SERVER: & str = "172.16.1.2";
pub const KAFKA_TOPIC: & str = "CellAgent";
//pub const MAX_ENTRIES: TableIndex    = TableIndex(64);  // Max number of active trees
pub const MAX_CHARS: usize = 32; // Longest valid name
//pub const MAX_PACKETS: PacketNo    = 255;  // Maximum number of packets collected before processing
// Things used in constructing names
pub const SEPARATOR: & str = "+"; // Separator for compound names
// Default names for up trees
pub const CONTROL_TREE_NAME: & str = "Control";
pub const CONNECTED_PORTS_TREE_NAME: & str = "Connected";
//pub const BASE_TREE_NAME: & str = "Base";

// TODO: Use log crate for this
pub struct TraceOptions {
    pub all:      bool,
    pub dc:       bool,
    pub nal:      bool,
    pub noc:      bool,
    pub svc:      bool,
    pub vm:       bool,
    pub ca:       bool,
    pub cm:       bool,
    pub pe:       bool,
    pub pe_cm:    bool,
    pub pe_port:  bool,
    pub port:     bool,
    pub port_noc: bool,
    pub link:     bool
}
pub const TRACE_OPTIONS: TraceOptions = TraceOptions {
    all:      false,
    dc:       true,
    nal:      true,
    noc:      true,
    svc:      true,
    vm:       true,
    ca:       true,
    cm:       true,
    pe:       true,
    pe_cm:    true,
    pe_port:  false,
    port:     false,
    port_noc: true,
    link:     false
};

pub struct DebugOptions {
    pub all:            bool,
    pub flow_control:   bool,
    pub ca_msg_recv:    bool,
    pub ca_msg_send:    bool,
    pub cm_from_ca:     bool,
    pub cm_to_ca:       bool,
    pub cm_from_pe:     bool,
    pub cm_to_pe:       bool,
    pub application:    bool,
    pub deploy:         bool,
    pub discover:       bool,
    pub discoverd:      bool,
    pub manifest:       bool,
    pub pe_pkt_recv:    bool,
    pub pe_pkt_send:    bool,
    pub process_msg:    bool,
    pub pe_process_pkt: bool,
    pub saved_discover: bool,
    pub saved_msgs:     bool,
    pub stack_tree:     bool,
    pub traph_entry:    bool,
}
pub const DEBUG_OPTIONS: DebugOptions = DebugOptions {
    all:            false,
    flow_control:   false,
    ca_msg_recv:    false,
    ca_msg_send:    false,
    cm_from_ca:     false,
    cm_to_ca:       false,
    cm_from_pe:     false,
    cm_to_pe:       false,
    application:    false,
    deploy:         false,
    discover:       false,
    discoverd:      false,
    manifest:       false,
    pe_pkt_recv:    false,
    pe_pkt_send:    false,
    process_msg:    false,
    pe_process_pkt: false,
    saved_discover: false,
    saved_msgs:     false,
    stack_tree:     false,
    traph_entry:    false,
};
// Size of various fields
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ByteArray(pub Vec<u8>);
impl Deref for ByteArray { type Target = Vec<u8>; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct CellQty(pub usize);
impl Deref for CellQty { type Target = usize; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone)]
pub struct ContainerNo(pub usize);
impl Deref for ContainerNo { type Target = usize; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone)]
pub struct DatacenterNo(pub u16);
impl Deref for DatacenterNo { type Target = u16; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone)]
pub struct LinkQty(pub usize);
impl Deref for LinkQty { type Target = usize; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct MaskValue(pub u16);
impl Deref for MaskValue { type Target = u16; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone, Serialize)]
pub struct PacketNo(pub u16);
impl Deref for PacketNo { type Target = u16; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct PathLength(pub CellQty);
impl Deref for PathLength { type Target = CellQty; fn deref(&self) -> &Self::Target { &self.0 } }
impl fmt::Display for PathLength { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", *self.0)} }
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct PortQty(pub u8);
impl Deref for PortQty { type Target = u8; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct PortNo(pub u8);
impl PortNo {
    pub fn make_port_number(self, no_ports: PortQty) -> Result<PortNumber, Error> {
        Ok(PortNumber::new(self, no_ports)?)
    }
    pub fn as_usize(self) -> usize { self.0 as usize }
}
impl Deref for PortNo { type Target = u8; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone)]
pub enum CellType {
    Border,
    Interior
}
impl fmt::Display for PortNo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "P:{}", *self)
    }
}
impl fmt::Display for CellType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match *self {
            CellType::Border   => "Border",
            CellType::Interior => "Interior",
        };
        write!(f, "{}", s)
    }
}
// Connections
pub fn get_geometry(num_cells: CellQty) -> (usize, usize, Vec<(usize, usize)>) {
    let geometry = match num_cells {
        CellQty(3)  => vec![(0,0), (0,2), (1,1)],
        CellQty(4)  => vec![(0,0), (0,1), (1,0), (1,1)],
        CellQty(10) => vec![(0,0), (0,1), (0,2), (0,3), (0,4),
                           (1,0), (1,1), (1,2), (1,3), (1,4)],
        CellQty(47) => vec![(0,2), (0,4), (0,5), (0,7), (0,9), (0,10),
                           (1,0), (1,1), (1,2), (1,3), (1,4), (1,5), (1,7), (1,8), (1,9), (1,10),
                           (2,0), (2,1), (2,2), (2,3), (2,4), (2,6), (2,7), (2,8), (2,9),
                           (3,1), (3,2), (3,4), (3,5), (3,8), (3,9), (3,10),
                           (4,0), (4,1), (4,2), (4,3), (4,4), (4,6), (4,7), (4,8), (4,9),
                           (5,0), (5,1), (5,2), (5,3), (5,4), (5,6)],
        _ => panic!("Invalid number of cells")
    };
    let max_x = geometry
        .iter()
        .max_by_key(|xy| xy.0)
        .map(|xy| xy.0 +1)
        .unwrap_or(0);
    let max_y = geometry
        .iter()
        .max_by_key(|xy| xy.1)
        .map(|xy| xy.1 + 1)
        .unwrap_or(0);
    if geometry.len() != *num_cells { panic!(format!("Topology has {} entries for {} cells", geometry.len(), *num_cells)) };
    (max_x, max_y, geometry)
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct CellInfo {   // Any data the cell agent wants to expose to applications
    external_id: Uuid   // An externally visible identifier so applications can talk about individual cells
}
impl CellInfo {
    pub fn new() -> CellInfo {
        CellInfo { external_id: Uuid::new() }
    }
    pub fn get_external_id(&self) -> Uuid {
        return self.external_id;
    }
}
impl fmt::Display for CellInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "External ID {}", self.external_id)
    }
}
