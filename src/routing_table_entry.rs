use std::fmt;
use config;
use routing_table::RoutingTableError;
use utility::int_to_mask;

const MAX_PORTS: usize = config::MAX_PORTS as usize;
#[derive(Debug, Copy, Clone)]
pub struct RoutingTableEntry {
	index: usize,
	inuse: bool,
	parent: u8,
	mask: u16,
	other_indices: [usize; MAX_PORTS as usize]
}
impl RoutingTableEntry {
	pub fn new(table_index: usize, inuse: bool, parent: u8, mask: u16, 
			other_indices: [usize; MAX_PORTS]) -> RoutingTableEntry {
		let mut indices = [0; MAX_PORTS as usize];
		RoutingTableEntry { index: table_index, parent: 0,
			inuse: inuse, mask: mask, other_indices: indices }
	}
	pub fn get_index(&self) -> usize { self.index }
	pub fn set_index(&mut self, index: usize) -> Result<(), RoutingTableEntryError> { 
		if index > config::MAX_ENTRIES { Err(RoutingTableEntryError::Index(IndexError::new(index as u32))) }
		else {
			self.index = index; 
			Ok(())
		}
	}
	pub fn get_inuse(&self) -> bool { self.inuse }
	pub fn set_inuse(&mut self) { self.inuse = true; }
	pub fn set_not_inuse(&mut self) { self.inuse = false; }
	pub fn get_parent(&self) -> u8 { self.parent }
	pub fn set_parent(&mut self, parent: u8) { self.parent = parent; }
	pub fn get_mask(&self) -> u16 { self.mask }
	pub fn set_mask(&mut self, mask: u16) { self.mask = mask; }
	pub fn get_other_indices(&self) -> [usize; MAX_PORTS as usize] { self.other_indices }
	pub fn set_other_index(&mut self, port_index: u8, other_index: usize) -> Result<(),RoutingTableEntryError> {
		{
			match self.other_indices.get(port_index as usize) {
				Some(other) => (),
				None => return Err(RoutingTableEntryError::Port(PortError::new(port_index)))
			};
		}
		self.other_indices[port_index as usize] = other_index;
		Ok(())
	}
	pub fn update_parent(&self, parent: u8, other_index: usize) -> Result<RoutingTableEntry,RoutingTableEntryError> {
		if parent > MAX_PORTS as u8 { return Err(RoutingTableEntryError::Port(PortError::new(parent))); }
		let mut indices = self.other_indices.clone();
		indices[parent as usize] = other_index;
		Ok(RoutingTableEntry { index: self.index, parent: parent, inuse: self.inuse, mask: self.mask,
							other_indices: indices })
	}
	pub fn update_children(&self, child: u8, other_index: usize) -> Result<RoutingTableEntry,RoutingTableEntryError> {
		let mut indices = self.other_indices.clone();
		let child_mask = match int_to_mask(child) {
			Ok(m) => m,
			Err(_) => return Err(RoutingTableEntryError::Port(PortError::new(child)))
		};
		let mask = self.mask | child_mask;
		indices[child as usize] = other_index;
		Ok(RoutingTableEntry { index: self.index, parent: self.parent, inuse: self.inuse, mask: 0,
							other_indices: indices })
	}
	pub fn stringify(&self) -> String {
		let mut s = format!("\n{:6}", self.index);
		if self.inuse { s = s + &format!("  Yes  ") }
		else          { s = s + &format!("  No   ") }
		s = s + &format!("{:7}", self.parent);
		s = s + &format!(" {:016.b}", self.mask);
		s = s + &format!(" {:?}", self.other_indices.to_vec());
		s
	}
}
impl fmt::Display for RoutingTableEntry {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.stringify()) }
}
// Errors
use std::error::Error;
#[derive(Debug)]
pub enum RoutingTableEntryError {
	Port(PortError),
	Index(IndexError)
}
impl Error for RoutingTableEntryError {
	fn description(&self) -> &str {
		match *self {
			RoutingTableEntryError::Port(ref err) => err.description(),
			RoutingTableEntryError::Index(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			RoutingTableEntryError::Port(ref err) => Some(err),
			RoutingTableEntryError::Index(ref err) => Some(err),
		}
	}
}
impl fmt::Display for RoutingTableEntryError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			RoutingTableEntryError::Port(ref err) => write!(f, "Routing Table Entry Port Error caused by {}", err),
			RoutingTableEntryError::Index(ref err) => write!(f, "Routing Table Entry Port Error caused by {}", err),
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
#[derive(Debug)]
pub struct IndexError { msg: String }
impl IndexError { 
	pub fn new(index: u32) -> IndexError {
		IndexError { msg: format!("Index number {} is greater than the maximum of {}", index, config::MAX_ENTRIES) }
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
