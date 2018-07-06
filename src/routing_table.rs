use std::fmt;
use std::collections::HashMap;

use failure::{Error};

use name::{CellID};
use routing_table_entry::{RoutingTableEntry};
use uuid_fake::Uuid;

#[derive(Debug)]
pub struct RoutingTable {
	id: CellID,
	entries: HashMap<Uuid, RoutingTableEntry>,
	connected_ports: Vec<u8>
}
impl RoutingTable {
	pub fn new(id: CellID) -> Result<RoutingTable, Error> {
		Ok(RoutingTable { id, entries: HashMap::new(), connected_ports: Vec::new() })
	}
	pub fn get_entry(&self, uuid: Uuid) -> Result<RoutingTableEntry, RoutingTableError> {
		let f = "get_entry";
        //println!("Routing Table {}: cell {} uuid {}", f, self.id, uuid);
        let entry = match self.entries.get(&uuid) {
            Some(e) => e.clone(),
            None => return Err(RoutingTableError::Uuid { func_name: f, cell_id: self.id.clone(), uuid })
        };
        Ok(entry)
	}
	pub fn set_entry(&mut self, entry: RoutingTableEntry) {
        let f = "set_entry";
        self.entries.insert(entry.get_uuid(), entry);
		if false { println!("Routing Table {}: cell {} uuid {}, mask {}", f, self.id, entry.get_uuid(), entry.get_mask()); }
	}
}
impl fmt::Display for RoutingTable {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("\nRouting Table");
		s = s + &format!("\n Tree UUID  In Use Send? Parent Mask ");
		for entry in self.entries.values() {
			if entry.is_in_use() { s = s + &format!("\n{}", entry); }
		}
		write!(f, "{}", s) 
	}	
}
// Errors
#[derive(Debug, Fail)]
pub enum RoutingTableError {
	#[fail(display = "RoutingTableError::Chain {} {}", func_name, comment)]
	Chain { func_name: &'static str, comment: String },
    #[fail(display = "RoutingTableError::Uuid {}: {:?} is not a valid routing table uuid on cell {}", func_name, uuid, cell_id)]
    Uuid { func_name: &'static str, uuid: Uuid, cell_id: CellID}
}
