// Size of various fields
pub type CellNo     = usize;
pub type ContainerNo = usize;
pub type DatacenterNo = u16;
pub type Edge = (usize, usize);
pub type Json = String;
pub type LinkNo     = usize;
pub type MaskValue  = u16;
pub type PacketNo   = u16;
pub type PacketElement = u8; // Packets are made up of bytes
pub type PathLength = u32;
pub type PortNo     = u8;
pub type TableIndex = u32;
pub type MsgID = u64;
pub type VmNo       = usize;
// Default inputs
pub const NCELLS: CellNo = 10;
pub const NPORTS: PortNo =  6;
pub const NLINKS: LinkNo = 40;
// Size limits
pub const MAX_ENTRIES: TableIndex    = 64;  // Max number of active trees
pub const MAX_PORTS: PortNo          = 8; 	// Limit on number of ports per cell
pub const MAX_CHARS: usize           = 128; // Longest valid name
pub const MIN_BOUNDARY_CELLS: CellNo = 1;   // Minimum acceptable number of border cells
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