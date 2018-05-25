use std::fmt;
use std::ops::{Deref};

// Size of various fields
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
pub struct Edge { pub v: (CellNo, CellNo) }
#[derive(Debug, Copy, Clone)]
pub struct LinkNo(pub CellNo);
impl Deref for LinkNo { type Target = CellNo; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct MaskValue(pub u16);
impl Deref for MaskValue { type Target = u16; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct MsgID(pub u64);
impl Deref for MsgID { type Target = u64; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone)]
pub struct PacketNo(pub u16);
impl Deref for PacketNo { type Target = u16; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone, Hash, Serialize, Deserialize)]
pub struct PathLength(pub CellNo);
impl Deref for PathLength { type Target = CellNo; fn deref(&self) -> &Self::Target { &self.0 } }
impl fmt::Display for PathLength { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", *self.0)} }
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct PortNo { pub v: u8 }
impl Deref for PortNo { type Target = u8; fn deref(&self) -> &Self::Target { &self.v } }
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct TableIndex(pub u32);
impl Deref for TableIndex { type Target = u32; fn deref(&self) -> &Self::Target { &self.0 } }
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
// Default inputs
pub const NCELLS: CellNo = CellNo(10);
pub const NPORTS: PortNo =  PortNo { v: 6 };
pub const NLINKS: LinkNo = LinkNo(CellNo(40));
// Size limits
pub const MAX_ENTRIES: TableIndex    = TableIndex(64);  // Max number of active trees
pub const MAX_PORTS: PortNo          = PortNo { v: 8 }; 	// Limit on number of ports per cell
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