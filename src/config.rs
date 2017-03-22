// Default inputs
pub const NCELLS: usize = 10;
pub const NPORTS: u8    =  6;
pub const NLINKS: usize = 40;
// Size limits
pub const MAX_ENTRIES: usize = 64; // Max number of active trees
pub const MAX_PORTS: u8 = 8; // Must be less than 32 for RoutingEntry to compile
// Things used in constructing names
pub const SEPARATOR: &'static str = "+"; // Separator for compound names
// Packet sizes in bytes including header
pub const PACKET_SMALL: usize = 64;
pub const PACKET_MEDIUM: usize = 1500;
pub const PACKET_LARGE: usize = 4096;
// Size of chunk identifier 
pub const CHUNK_ID_SIZE: u64 = 48;