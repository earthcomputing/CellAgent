use std::fmt;
use std::ops::{Deref};

use failure::Error;
use utility::PortNumber;

pub struct DebugOptions {
    pub trace_all:      bool,
    pub ca_msg_recv:    bool,
    pub ca_msg_send:    bool,
    pub cm_from_ca:     bool,
    pub cm_to_ca:       bool,
    pub cm_from_pe:     bool,
    pub cm_to_pe:       bool,
    pub deploy:         bool,
    pub pe_pkt_recv:    bool,
    pub pe_pkt_send:    bool,
    pub process_msg:    bool,
    pub pe_process_pkt: bool,
    pub saved_msgs:     bool,
    pub stack_tree:     bool,
    pub traph_state:    bool,
}
pub const DEBUG_OPTIONS: DebugOptions = DebugOptions {
    trace_all:      true,
    ca_msg_recv:    false,
    ca_msg_send:    false,
    cm_from_ca:     false,
    cm_to_ca:       false,
    cm_from_pe:     false,
    cm_to_pe:       false,
    deploy:         false,
    pe_pkt_recv:    false,
    pe_pkt_send:    false,
    process_msg:    false,
    pe_process_pkt: false,
    saved_msgs:     false,
    stack_tree:     false,
    traph_state:    false,
};
pub const CONTINUE_ON_ERROR: bool = false; // Don't close channel following an error
pub const AUTO_BREAK: bool = false; // True when debugging broken link with VSCode
#[derive(Debug, Copy, Clone)]
pub enum Quench { Simple, RootPort }
pub const QUENCH: Quench = Quench::RootPort;
// Size of various fields
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ByteArray(pub Vec<u8>);
impl Deref for ByteArray { type Target = Vec<u8>; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct CellNo(pub usize);
impl Deref for CellNo { type Target = usize; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone)]
pub struct ContainerNo(pub usize);
impl Deref for ContainerNo { type Target = usize; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone)]
pub struct DatacenterNo(pub u16);
impl Deref for DatacenterNo { type Target = u16; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Edge(pub CellNo, pub CellNo);
#[derive(Debug, Copy, Clone)]
pub struct LinkNo(pub CellNo);
impl Deref for LinkNo { type Target = CellNo; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct MaskValue(pub u16);
impl Deref for MaskValue { type Target = u16; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct MsgID(pub u64);
impl Deref for MsgID { type Target = u64; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone, Serialize)]
pub struct PacketNo(pub u16);
impl Deref for PacketNo { type Target = u16; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct PathLength(pub CellNo);
impl Deref for PathLength { type Target = CellNo; fn deref(&self) -> &Self::Target { &self.0 } }
impl fmt::Display for PathLength { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", *self.0)} }
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct PortNo(pub u8);
impl PortNo {
    pub fn make_port_number(self, no_ports: PortNo) -> Result<PortNumber, Error> {
        Ok(PortNumber::new(self, no_ports)?)
    }
}
impl Deref for PortNo { type Target = u8; fn deref(&self) -> &Self::Target { &self.0 } }
//#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
//pub struct TableIndex(pub u32);
//impl Deref for TableIndex { type Target = u32; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone)]
pub struct VmNo(pub usize);
// Cell types
#[derive(Debug, Copy, Clone)]
pub enum CellType {
    Border,
    Interior
}
impl fmt::Display for CellType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match *self {
            CellType::Border   => "Border",
            CellType::Interior => "Interior",
        };
        write!(f, "{}", s)
    }
}
pub const SCHEMA_VERSION: &'static str = "0.1";
pub const REPO: &'static str = "CellAgent";
// Default inputs
pub const MAX_PORTS: PortNo = PortNo(9); 	// Limit on number of ports per cell
pub const NCELLS: CellNo    = CellNo(10);
pub const NPORTS: PortNo    =  PortNo(8);
pub const NLINKS: LinkNo    = LinkNo(CellNo(40));
// Connections
pub fn get_edges() -> Vec<Edge> {
    match NCELLS {
        CellNo(3)  => vec![is2e(0,1), is2e(0,2), is2e(1,2)],
        CellNo(4)  => vec![is2e(0,1), is2e(0,2), is2e(0,3), is2e(1,2),
                           is2e(1,3), is2e(2,3)],
        CellNo(10) => vec![is2e(0,1),is2e(1,2),is2e(1,6),is2e(3,4),
                           is2e(5,6),is2e(6,7),is2e(7,8),is2e(8,9),
                           is2e(0,5),is2e(2,3),is2e(2,7),is2e(3,8),is2e(4,9)],
        CellNo(47) => vec![
            is2e(0, 5), is2e(1, 5), is2e(2, 8), is2e(3, 8), is2e(4, 5), is2e(5, 8), is2e(5, 22), is2e(6, 5), is2e(7, 5), is2e(8, 22), is2e(9, 5),
            is2e(10, 5), is2e(11, 5), is2e(12, 5), is2e(13, 8), is2e(14, 8), is2e(15, 22), is2e(16, 22), is2e(17, 22), is2e(18, 22), is2e(19, 24),
            is2e(20, 24), is2e(21, 22), is2e(23, 24), is2e(24, 22), is2e(25, 24), is2e(26, 22), is2e(27, 32), is2e(28, 22), is2e(29, 24),
            is2e(30, 24), is2e(31, 32), is2e(32, 22), is2e(33, 32), is2e(34, 24), is2e(35, 40), is2e(36, 32), is2e(37, 43), is2e(38, 43), is2e(39, 40),
            is2e(40, 22), is2e(41, 39), is2e(42, 43), is2e(43, 22), is2e(44, 40), is2e(45, 40), is2e(46, 43)
        ],
        _ => panic!("Invalid number of cells")
    }
}
pub fn get_geometry() -> Vec<(usize, usize)> {
    let geometry = match NCELLS {
        CellNo(3)  => vec![(0,0), (0,2), (1,1)],
        CellNo(4)  => vec![(0,0), (0,1), (1,0), (1,1)],
        CellNo(10) => vec![(0,0), (0,1), (0,2), (0,3), (0,4),
                           (1,0), (1,1), (1,2), (1,3), (1,4)],
        CellNo(47) => vec![(0,2), (0,4), (0,5), (0,7), (0,9), (0,10),
                           (1,0), (1,1), (1,2), (1,3), (1,4), (1,5), (1,7), (1,8), (1,9), (1,10),
                           (2,0), (2,1), (2,2), (2,3), (2,4), (2,6), (2,7), (2,8), (2,9),
                           (3,1), (3,2), (3,4), (3,5), (3,8), (3,9), (3,10),
                           (4,0), (4,1), (4,2), (4,3), (4,4), (4,6), (4,7), (4,8), (4,9),
                           (5,0), (5,1), (5,2), (5,3), (5,4), (5,6)],
        _ => panic!("Invalid number of cells")
    };
    if geometry.len() != *NCELLS { panic!(format!("Topology has {} entries for {} cells", geometry.len(), *NCELLS)) };
    geometry
}
fn is2e(i: usize, j: usize) -> Edge { Edge(CellNo(i),CellNo(j)) }
// Size limits
//pub const MAX_ENTRIES: TableIndex    = TableIndex(64);  // Max number of active trees
//pub const MAX_CHARS: usize         = 128; // Longest valid name
pub const MIN_BOUNDARY_CELLS: CellNo = CellNo(1);   // Minimum acceptable number of border cells
//pub const MAX_PACKETS: PacketNo    = 255;  // Maximum number of packets collected before processing
// Things used in constructing names
pub const SEPARATOR: &'static str = "+"; // Separator for compound names
// Packet sizes in bytes including header
pub const PAYLOAD_DEFAULT_ELEMENT: u8 = 0;
pub const PACKET_MIN: usize = 64;
pub const PACKET_MAX: usize = 9000;
// Size of chunk identifier 
//pub const CHUNK_ID_SIZE: u64 = 48;
// Default names for up trees
pub const CONTROL_TREE_NAME: &'static str = "Control";
pub const CONNECTED_PORTS_TREE_NAME: &'static str = "Connected";
//pub const BASE_TREE_NAME: &'static str = "Base";
// Place to write output data
pub const OUTPUT_FILE_NAME: &'static str = "/Users/alan/Documents/multicell-trace.json";
pub const KAFKA_SERVER: &'static str = "172.16.1.77:9092";