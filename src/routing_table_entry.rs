use std::fmt;
use config::{MAX_PORTS, MAX_ENTRIES, PortNo, TableIndex};
use utility::{Mask, PortNumber, PortNumberError};

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
	pub fn default(index: TableIndex) -> Result<RoutingTableEntry, RoutingTableEntryError> {
		let port_number = PortNumber::new(0, MAX_PORTS)?;
		Ok(RoutingTableEntry::new(index, false, port_number, Mask::empty(), [0; MAX_PORTS as usize]))
	}
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
	pub fn add_children(&mut self, children: &Vec<PortNumber>) {
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
use std::error::Error;
use utility::UtilityError;
#[derive(Debug)]
pub enum RoutingTableEntryError {
	Port(PortError),
	PortNumber(PortNumberError),
	Index(IndexError),
	Utility(UtilityError)
}
impl Error for RoutingTableEntryError {
	fn description(&self) -> &str {
		match *self {
			RoutingTableEntryError::Port(ref err) => err.description(),
			RoutingTableEntryError::PortNumber(ref err) => err.description(),
			RoutingTableEntryError::Index(ref err) => err.description(),
			RoutingTableEntryError::Utility(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			RoutingTableEntryError::Port(ref err) => Some(err),
			RoutingTableEntryError::PortNumber(ref err) => Some(err),
			RoutingTableEntryError::Index(ref err) => Some(err),
			RoutingTableEntryError::Utility(ref err) => Some(err),
		}
	}
}
impl fmt::Display for RoutingTableEntryError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			RoutingTableEntryError::Port(ref err) => write!(f, "Routing Table Entry Port Error caused by {}", err),
			RoutingTableEntryError::PortNumber(ref err) => write!(f, "Routing Table Entry Port Number Error caused by {}", err),
			RoutingTableEntryError::Index(ref err) => write!(f, "Routing Table Entry Port Error caused by {}", err),
			RoutingTableEntryError::Utility(ref err) => write!(f, "Routing Table Utility Error caused by {}", err),
		}
	}
}
#[derive(Debug)]
pub struct PortError { msg: String }
impl PortError { 
	pub fn new(port_number: PortNumber) -> PortError {
		PortError { msg: format!("Port number {} is greater than the maximum of {}", port_number, MAX_PORTS) }
	}
}
impl Error for PortError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for PortError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<PortError> for RoutingTableEntryError {
	fn from(err: PortError) -> RoutingTableEntryError { RoutingTableEntryError::Port(err) }
}
#[derive(Debug)]
pub struct IndexError { msg: String }
impl IndexError { 
	pub fn new(index: u32) -> IndexError {
		IndexError { msg: format!("Index number {} is greater than the maximum of {}", index, MAX_ENTRIES) }
	}
}
impl Error for IndexError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for IndexError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<IndexError> for RoutingTableEntryError {
	fn from(err: IndexError) -> RoutingTableEntryError { RoutingTableEntryError::Index(err) }
}
impl From<PortNumberError> for RoutingTableEntryError {
	fn from(err: PortNumberError) -> RoutingTableEntryError { RoutingTableEntryError::PortNumber(err) }
}
impl From<UtilityError> for RoutingTableEntryError {
	fn from(err: UtilityError) -> RoutingTableEntryError { RoutingTableEntryError::Utility(err) }
}
