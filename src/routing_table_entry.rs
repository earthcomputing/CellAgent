use std::fmt;
use config::MAX_PORTS;
use utility::int_to_mask;
#[derive(Copy, Clone)]
pub struct RoutingTableEntry {
	index: usize,
	inuse: bool,
	parent: u8,
	mask: u16,
	other_indices: [usize; MAX_PORTS as usize]
}
impl RoutingTableEntry {
	pub fn new(table_index: usize, inuse: bool) -> RoutingTableEntry {
		let mut indices = [0; MAX_PORTS as usize];
		RoutingTableEntry { index: table_index, parent: 0,
			inuse: inuse, mask: 0, other_indices: indices }
	}
	pub fn get_index(&self) -> usize { self.index }
	pub fn set_index(&mut self, index: usize) { self.index = index; }
	pub fn update_parent(&self, parent: u8, other_index: usize) -> Result<RoutingTableEntry,RoutingTableEntryError> {
		if parent > MAX_PORTS { return Err(RoutingTableEntryError::Port(PortError::new(parent))); }
		let mut indices = self.other_indices.clone();
		indices[parent as usize] = other_index;
		Ok(RoutingTableEntry { index: self.index, parent: parent, inuse: self.inuse, mask: self.mask,
							other_indices: indices })
	}
	pub fn update_children(&self, child: u8, other_index: usize) -> Result<RoutingTableEntry,RoutingTableEntryError> {
		let mut indices = self.other_indices.clone();
		let child_mask = match int_to_mask(child) {
			Some(m) => m,
			None => return Err(RoutingTableEntryError::Port(PortError::new(child)))
		};
		let mask = self.mask | child_mask;
		indices[child as usize] = other_index;
		Ok(RoutingTableEntry { index: self.index, parent: self.parent, inuse: self.inuse, mask: mask,
							other_indices: indices })
	}
	pub fn to_string(&self) -> String {
		let mut s = format!("{:4}", self.index);
		if self.inuse { s = s + &format!(" Yes  ") }
		else          { s = s + &format!(" No   ") }
		s = s + &format!(" {:016.b}", self.mask);
		s = s + &format!(" {:?}", self.other_indices.to_vec());
		s
	}
}
impl fmt::Display for RoutingTableEntry {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.to_string()) }
}
impl fmt::Debug for RoutingTableEntry {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.to_string()) }
}
// Errors
use std::error::Error;
#[derive(Debug)]
pub enum RoutingTableEntryError {
	Port(PortError)
}
impl Error for RoutingTableEntryError {
	fn description(&self) -> &str {
		match *self {
			RoutingTableEntryError::Port(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			RoutingTableEntryError::Port(ref err) => Some(err),
		}
	}
}
impl fmt::Display for RoutingTableEntryError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			RoutingTableEntryError::Port(ref err) => write!(f, "Routing Table Entry Port Error caused by {}", err),
		}
	}
}
#[derive(Debug)]
pub struct PortError { msg: String }
impl PortError { 
	pub fn new(port_no: u8) -> PortError {
		PortError { msg: format!("Port number {} is greater than the maximum of {}", port_no, MAX_PORTS) }
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
