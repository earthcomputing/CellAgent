use std::fmt;
use std::collections::HashSet;
use config::{MAX_PORTS, MAX_ENTRIES, PortNo, TableIndex};
use utility::{Mask, PortNumber};

#[derive(Debug, Copy, Clone)]
pub struct RoutingTableEntry {
	index: TableIndex,
	inuse: bool,
	parent: PortNo,
	mask: Mask,
	other_indices: [TableIndex; MAX_PORTS as usize]
}
#[deny(unused_must_use)]
impl RoutingTableEntry {
	pub fn new(index: TableIndex, inuse: bool, parent: PortNumber, mask: Mask, 
			other_indices: [TableIndex; MAX_PORTS as usize]) -> RoutingTableEntry {
		RoutingTableEntry { index: index, parent: parent.get_port_no(),
			inuse: inuse, mask: mask, other_indices: other_indices }
	}
	pub fn default(index: TableIndex) -> Result<RoutingTableEntry> {
		let port_number = PortNumber::new(0, MAX_PORTS).chain_err(|| ErrorKind::RoutingTableEntryError)?;
		Ok(RoutingTableEntry::new(index, false, port_number, Mask::empty(), [0; MAX_PORTS as usize]))
	}
	pub fn is_inuse(&self) -> bool { self.inuse }
	pub fn get_index(&self) -> TableIndex { self.index }
	pub fn or_with_mask(&mut self, mask: Mask) { self.mask = self.mask.or(mask); }
	pub fn and_with_mask(&mut self, mask: Mask) { self.mask = self.mask.and(mask); }
//	pub fn get_inuse(&self) -> bool { self.inuse }
	pub fn set_inuse(&mut self) { self.inuse = true; }
//	pub fn set_not_inuse(&mut self) { self.inuse = false; }
	pub fn get_parent(&self) -> PortNo { self.parent }
//	pub fn set_parent(&mut self, parent: PortNo) { self.parent = parent; }
	pub fn get_mask(&self) -> Mask { self.mask }
//	pub fn set_mask(&mut self, mask: Mask) { self.mask = mask; }
	pub fn get_other_indices(&self) -> [TableIndex; MAX_PORTS as usize] { self.other_indices }
	pub fn get_other_index(&self, port_number: PortNumber) -> TableIndex {
		let port_no = port_number.get_port_no() as usize;
		self.other_indices[port_no]
	}
	pub fn add_other_index(&mut self, port_index: PortNumber, other_index: TableIndex) {
		let port_no = port_index.get_port_no();
		self.other_indices[port_no as usize] = other_index;
	}
	pub fn add_children(&mut self, children: &HashSet<PortNumber>) {
		let mask = Mask::make(children);
		self.or_with_mask(mask);
	}
	pub fn set_parent(&mut self, port_number: PortNumber) {
		self.parent = port_number.get_port_no();
	}
}
impl fmt::Display for RoutingTableEntry {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("{:6}", self.index);
		if self.inuse { s = s + &format!("  Yes  ") }
		else          { s = s + &format!("  No   ") }
		s = s + &format!("{:7}", self.parent);
		s = s + &format!(" {}", self.mask);
		s = s + &format!(" {:?}", self.other_indices.to_vec());
		write!(f, "{}", s) 
	}
}
// Errors
error_chain! {
	links {
		Utility(::utility::Error, ::utility::ErrorKind);
	}
	errors { RoutingTableEntryError
		Index(index: TableIndex) {
			description("Specified table index is too large")
			display("Index number {} is greater than the maximum of {}", index, MAX_ENTRIES)
		}
	}
}
