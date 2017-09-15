use std::fmt;

use config::{MAX_ENTRIES, MAX_PORTS, TableIndex};
use name::CellID;
use routing_table_entry::{RoutingTableEntry};
use utility::{Mask, PortNumber, S};

#[derive(Debug)]
pub struct RoutingTable {
	id: CellID,
	entries: Vec<RoutingTableEntry>,
	connected_ports: Vec<u8>
}
impl RoutingTable {
	pub fn new(id: CellID) -> Result<RoutingTable> {
		let mut entries = Vec::new();
		for i in 0..*MAX_ENTRIES {
			let entry = RoutingTableEntry::default(TableIndex(i)).chain_err(|| ErrorKind::RoutingTableError)?;
			entries.push(entry);
		}
		Ok(RoutingTable { id: id, entries: entries, connected_ports: Vec::new() })
	}
	pub fn get_entry(&self, TableIndex(index): TableIndex) -> Result<RoutingTableEntry> { 
		let f = "get_entry";
		match self.entries.get(index as usize) {
			Some(e) => Ok(*e),
			None => Err(ErrorKind::Index(self.id.clone(), TableIndex(index), S(f)).into())
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
error_chain! {
	errors { RoutingTableError
		Index(cell_id: CellID, index: TableIndex, func_name: String) {
			display("RoutingTable {}: {} is not a valid routing table index on cell {}", func_name, **index, cell_id)
		}
		Name(cell_id: CellID, func_name: String, name: String) {
			display("RoutingTable {}: Error making name from {} on cell {}", func_name, name, cell_id)
		}
	}
}
