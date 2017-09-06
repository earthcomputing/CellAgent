use std::fmt;
use std::collections::HashSet;
use uuid::Uuid;

use config::{MAX_PORTS, MAX_ENTRIES, PortNo, TableIndex};
use name::{Name, TreeID};
use utility::{Mask, PortNumber};

#[derive(Debug, Copy, Clone)]
pub struct RoutingTableEntry {
	index: TableIndex,
	uuid: Uuid,
	may_send: bool, 
	inuse: bool,
	parent: PortNo,
	mask: Mask,
	other_indices: [TableIndex; MAX_PORTS.v as usize]
}
impl RoutingTableEntry {
	pub fn new(index: TableIndex, tree_id: &TreeID, inuse: bool, parent: PortNumber, mask: Mask, 
			may_send: bool, other_indices: [TableIndex; MAX_PORTS.v as usize]) -> RoutingTableEntry {
		RoutingTableEntry { index: index, uuid: tree_id.get_uuid(), parent: parent.get_port_no(),
			may_send: may_send, inuse: inuse, mask: mask, other_indices: other_indices }
	}
	pub fn default(index: TableIndex) -> Result<RoutingTableEntry> {
		let port_number = PortNumber::new(PortNo{v:0}, MAX_PORTS).chain_err(|| ErrorKind::RoutingTableEntryError)?;
		Ok(RoutingTableEntry::new(index, &TreeID::new("Default")?, false, port_number, Mask::empty(), false, [TableIndex(0); MAX_PORTS.v as usize]))
	}
	pub fn is_in_use(&self) -> bool { self.inuse }
	pub fn may_send(&self) -> bool { self.may_send }
	pub fn enable_send(&mut self) { self.may_send = true; }
	pub fn disable_send(&mut self) { self.may_send = false; }
	pub fn get_index(&self) -> TableIndex { self.index }
	pub fn get_uuid(&self) -> Uuid { self.uuid }
	pub fn set_uuid(&mut self, uuid: &Uuid) { self.uuid = *uuid; }
	pub fn or_with_mask(&mut self, mask: Mask) { self.mask = self.mask.or(mask); }
	pub fn and_with_mask(&mut self, mask: Mask) { self.mask = self.mask.and(mask); }
	pub fn set_inuse(&mut self) { self.inuse = true; }
	pub fn set_not_inuse(&mut self) { self.inuse = false; }
	pub fn get_parent(&self) -> PortNo { self.parent }
	pub fn get_mask(&self) -> Mask { self.mask }
	pub fn set_mask(&mut self, mask: Mask) { self.mask = mask; }
	pub fn get_other_indices(&self) -> [TableIndex; MAX_PORTS.v as usize] { self.other_indices }
	pub fn set_other_indices(&mut self, other_indices: [TableIndex;8]) { self.other_indices = other_indices }
	pub fn set_tree_id(&mut self, tree_id: &TreeID) {
		self.uuid = tree_id.get_uuid();
	}
	pub fn get_other_index(&self, port_number: PortNumber) -> TableIndex {
		let port_no = port_number.get_port_no().v as usize;
		self.other_indices[port_no]
	}
	pub fn add_other_index(&mut self, port_index: PortNumber, other_index: TableIndex) {
		let port_no = port_index.get_port_no();
		self.other_indices[port_no.v as usize] = other_index;
	}
	pub fn add_children(&mut self, children: &HashSet<PortNumber>) {
		let mask = Mask::make(children);
		self.or_with_mask(mask);
	}
	pub fn clear_children(&mut self) {
		self.and_with_mask(Mask::new0())
	}
	pub fn set_parent(&mut self, port_number: PortNumber) {
		self.parent = port_number.get_port_no();
	}
	pub fn set_table_index(&mut self, index: TableIndex) {
		self.index = index;
	}
}
impl fmt::Display for RoutingTableEntry {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("{:6}", self.index.0);
		let mut uuid = self.uuid.to_string();
		uuid.truncate(8);
		s = s + &format!(" {:8?}", uuid);
		if self.inuse { s = s + &format!("  Yes  ") }
		else          { s = s + &format!("  No   ") }
		if self.may_send { s = s + &format!("  Yes ") }
		else             { s = s + &format!("  No  ") }
		s = s + &format!("{:7}", self.parent.v);
		s = s + &format!("{}", self.mask);
		let mut other_indices = Vec::new();
		for other_index in self.other_indices.iter() {
			other_indices.push(other_index.0);
		}
		s = s + &format!(" {:?}", other_indices);
		write!(f, "{}", s) 
	}
}
// Errors
error_chain! {
	links {
		Name(::name::Error, ::name::ErrorKind);
		Utility(::utility::Error, ::utility::ErrorKind);
	}
	errors { RoutingTableEntryError
		Index(index: TableIndex, func_name: String) {
			display("{}: RoutingTableEntry: Index number {} is greater than the maximum of {}", func_name, index.0, MAX_ENTRIES.0)
		}
	}
}
