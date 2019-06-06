use std::{fmt,
          ops::{Deref}};

use lazy_static::lazy_static;

use crate::utility::{CellNo, Edge, PortNo, Quench, is2e};

// System parameters
pub const SCHEMA_VERSION: &str = "0.1";
pub const REPO: &str = "CellAgent";
pub const MAX_CHARS: usize = 32; // Longest valid name
pub const SEPARATOR: & str = "+"; // Separator for compound names
pub const CONTROL_TREE_NAME: & str = "Control";
pub const CONNECTED_PORTS_TREE_NAME: & str = "Connected";
pub const BASE_TREE_NAME: & str = "Base";
// Run parameters
pub const MAX_NUM_PHYS_PORTS_PER_CELL: PortQty = PortQty(9);          // Limit on number of ports per cell
pub const MIN_NUM_BORDER_CELLS: CellQty = CellQty(1);   // Minimum acceptable number of border cells
pub const PACKET_MIN: usize = 64;
pub const PACKET_MAX: usize = 9000;
pub const QUENCH: Quench = Quench::RootPort;
pub const CONTINUE_ON_ERROR: bool = false; // Don't close channel following an error if true
pub const AUTO_BREAK: Option<Edge> = (None, Some(Edge(CellNo(1), CellNo(2)))).0;// Use .1 to auto break link
pub const DISCOVER_QUIESCE_FACTOR: usize = 4; // Bigger means wait longer to quiesce
pub const PAYLOAD_DEFAULT_ELEMENT: u8 = 0;
pub const OUTPUT_DIR_NAME: &str = "/tmp/multicell/";
pub const OUTPUT_FILE_NAME: &str = "/tmp/multicell/trace";
pub const KAFKA_SERVER: & str = "172.16.1.2";
pub const KAFKA_TOPIC: & str = "CellAgent";
pub const NUM_CELLS: CellQty = CellQty(10);
pub const NUM_PORTS_PER_CELL:PortQty = PortQty(8);
pub const CELL_PORT_EXCEPTIONS: [(CellNo, PortQty); 2] = [(CellNo(5), PortQty(7)), (CellNo(5), PortQty(7))];
lazy_static! {
    pub static ref BORDER_CELL_PORTS: Vec<(CellNo, Vec<PortNo>)> = vec![(CellNo(2), vec![PortNo(2)]),
                                                                       (CellNo(7), vec![PortNo(2)])];
    pub static ref EDGE_LIST: Vec<Edge> = vec![is2e(0,1), is2e(1,2), is2e(3,4), is2e(2,3),
                                               is2e(1,6), is2e(5,6), is2e(6,7), is2e(7,8), is2e(8,9),
                                               is2e(0,5), is2e(2,7), is2e(3,8), is2e(4,9)];
}

pub const TRACE_OPTIONS: TraceOptions = TraceOptions {
    all:      false,
    dc:       true,
    nal:      true,
    noc:      true,
    svc:      true,
    vm:       true,
    ca:       true,
    cm:       false,
    pe:       false,
    pe_cm:    false,
    pe_port:  false,
    port:     false,
    link:     false
};
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
    port:           false,
    saved_discover: false,
    saved_stack:    false,
    stack_tree:     false,
    traph_entry:    false,
};

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
    pub link:     bool
}

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
    pub port:           bool,
    pub saved_discover: bool,
    pub saved_stack:    bool,
    pub stack_tree:     bool,
    pub traph_entry:    bool,
}
// Size of various fields
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


