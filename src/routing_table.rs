use std::fmt;

use failure::{Error, ResultExt};

use config::{MAX_ENTRIES, TableIndex};
use name::{Name, CellID};
use routing_table_entry::{RoutingTableEntry};
use utility::{S};

#[derive(Debug)]
pub struct RoutingTable {
	id: CellID,
	entries: Vec<RoutingTableEntry>,
	connected_ports: Vec<u8>
}
impl RoutingTable {
	pub fn new(id: CellID) -> Result<RoutingTable, Error> {
		let mut entries = Vec::new();
		for i in 0..*MAX_ENTRIES {
			let entry = RoutingTableEntry::default(TableIndex(i)).context(RoutingTableError::Chain { func_name: "new", comment: S(id.get_name())})?;
			entries.push(entry);
		}
		Ok(RoutingTable { id, entries, connected_ports: Vec::new() })
	}
	pub fn get_entry(&self, TableIndex(index): TableIndex) -> Result<RoutingTableEntry, RoutingTableError> {
		let f = "get_entry";
		match self.entries.get(index as usize) {
			Some(e) => Ok(*e),
			None => Err(RoutingTableError::Index { cell_id: self.id.clone(), index: TableIndex(index), func_name: f })
		}
	}
	pub fn set_entry(&mut self, entry: RoutingTableEntry) { 
		self.entries[*entry.get_index() as usize] = entry; 
		//println!("Routing Table {}: index {}, mask {}", self.id, entry.get_index(), entry.get_mask());
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
    Index { func_name: &'static str, index: TableIndex, cell_id: CellID}
}
