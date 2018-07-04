use std::fmt;
use std::collections::HashMap;

use failure::{Error, ResultExt};

use config::{MAX_ENTRIES, TableIndex};
use name::{Name, CellID};
use routing_table_entry::{RoutingTableEntry};
use utility::{S};
use uuid_fake::Uuid;

#[derive(Debug)]
pub struct RoutingTable {
	id: CellID,
	entries: Vec<RoutingTableEntry>,
	entries_hash: HashMap<Uuid, RoutingTableEntry>,
	connected_ports: Vec<u8>
}
impl RoutingTable {
	pub fn new(id: CellID) -> Result<RoutingTable, Error> {
		let mut entries = Vec::new();
		for i in 0..*MAX_ENTRIES {
			let entry = RoutingTableEntry::default(TableIndex(i)).context(RoutingTableError::Chain { func_name: "new", comment: S(id.get_name())})?;
			entries.push(entry);
		}
		Ok(RoutingTable { id, entries, entries_hash: HashMap::new(), connected_ports: Vec::new() })
	}
	pub fn get_entry(&self, TableIndex(index): TableIndex, uuid: Uuid) -> Result<RoutingTableEntry, RoutingTableError> {
		let f = "get_entry";
		let entry = match self.entries.get(index as usize) {
			Some(e) => *e,
			None => return Err(RoutingTableError::Index { cell_id: self.id.clone(), index: TableIndex(index), func_name: f })
		};
        //println!("Routing Table {}: cell {} uuid {}", f, self.id, uuid);
        let entry = match self.entries_hash.get(&uuid) {
            Some(e) => e.clone(),
            None => return Err(RoutingTableError::Uuid { func_name: f, cell_id: self.id.clone(), uuid })
        };
        Ok(entry)
	}
	pub fn set_entry(&mut self, entry: RoutingTableEntry) {
        let f = "set_entry";
		self.entries[*entry.get_index() as usize] = entry;
        self.entries_hash.insert(entry.get_uuid(), entry);
		//println!("Routing Table {}: cell {} uuid {}, mask {}", f, self.id, entry.get_uuid(), entry.get_mask());
	}
}
impl fmt::Display for RoutingTable {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("\nRouting Table with {} Entries", *MAX_ENTRIES);
		s = s + &format!("\n Index Tree UUID  In Use Send? Parent Mask             Indices");
		for entry in self.entries.iter() {
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
    #[fail(display = "RoutingTableError::Index {}: {:?} is not a valid routing table index on cell {}", func_name, index, cell_id)]
    Index { func_name: &'static str, index: TableIndex, cell_id: CellID},
    #[fail(display = "RoutingTableError::Uuid {}: {:?} is not a valid routing table uuid on cell {}", func_name, uuid, cell_id)]
    Uuid { func_name: &'static str, uuid: Uuid, cell_id: CellID}
}
