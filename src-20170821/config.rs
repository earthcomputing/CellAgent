// Size of various fields
#[derive(Debug, Copy, Clone, Hash, Serialize, Deserialize)]
pub struct CellNo(pub usize);
#[derive(Debug, Copy, Clone)]
pub struct ContainerNo(pub usize);
#[derive(Debug, Copy, Clone)]
pub struct DatacenterNo(pub u16);
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Edge { pub v: (CellNo, CellNo) }
#[derive(Debug, Copy, Clone)]
pub struct LinkNo(pub CellNo);
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct MaskValue(pub u16);
#[derive(Debug, Copy, Clone)]
pub struct PacketNo(pub u16);
#[derive(Debug, Copy, Clone, Hash, Serialize, Deserialize)]
pub struct PathLength(pub CellNo);
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct PortNo { pub v: u8 }
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct TableIndex(pub u32);
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct MsgID(pub u64);
#[derive(Debug, Copy, Clone)]
pub struct VmNo(pub usize);
// Default inputs
pub const NCELLS: CellNo = CellNo(10);
pub const NPORTS: PortNo =  PortNo { v: 6 };
pub const NLINKS: LinkNo = LinkNo(CellNo(40));
// Size limits
pub const MAX_ENTRIES: TableIndex    = TableIndex(64);  // Max number of active trees
pub const MAX_PORTS: PortNo          = PortNo { v: 8 }; 	// Limit on number of ports per cell
pub const MAX_CHARS: usize           = 128; // Longest valid name
pub const MIN_BOUNDARY_CELLS: CellNo = CellNo(1);   // Minimum acceptable number of border cells
//pub const MAX_PACKETS: PacketNo   = 255;  // Maximum number of packets collected before processing
// Things used in constructing names
pub const SEPARATOR: &'static str = "+"; // Separator for compound names
// Packet sizes in bytes including header
pub const PAYLOAD_DEFAULT_ELEMENT: u8 = 0;
pub const PACKET_MIN: usize = 64;
pub const PACKET_MAX: usize = 9000;
// Size of chunk identifier 
//pub const CHUNK_ID_SIZE: u64 = 48;
pub const PHYSICAL_UP_TREE_NAME: &str = "Physical";
