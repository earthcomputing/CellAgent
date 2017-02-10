pub const NCELLS: usize = 10;
pub const NPORTS: u8    =  6;
pub const NLINKS: usize = 40;

pub const MAX_ENTRIES: usize = 64; // Max number of active trees
pub const MAX_PORTS: usize = 15; // Must be less than 32 for RoutingEntry to compile
pub const MAX_NAME_SIZE: usize = 50; // Longest name allowed