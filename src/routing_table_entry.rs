use std::fmt;
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
impl RoutingTableEntry {
	pub fn new(table_index: TableIndex, inuse: bool, parent: PortNumber, mask: Mask, 
			other_indices: [TableIndex; MAX_PORTS as usize]) -> RoutingTableEntry {
		RoutingTableEntry { index: table_index, parent: parent.get_port_no(),
			inuse: inuse, mask: mask, other_indices: other_indices }
	}
	pub fn get_index(&self) -> TableIndex { self.index }
	pub fn set_index(&mut self, index: TableIndex) -> Result<(), RoutingTableEntryError> { 
		if index > MAX_ENTRIES { Err(RoutingTableEntryError::Index(IndexError::new(index as TableIndex))) }
		else {
			self.index = index; 
			Ok(())
		}
	}
	pub fn or_with_mask(&mut self, mask: Mask) { self.mask = self.mask.or(mask); }
	pub fn and_with_mask(&mut self, mask: Mask) { self.mask = self.mask.and(mask); }
	pub fn get_inuse(&self) -> bool { self.inuse }
	pub fn set_inuse(&mut self) { self.inuse = true; }
	pub fn set_not_inuse(&mut self) { self.inuse = false; }
	pub fn get_parent(&self) -> PortNo { self.parent }
	pub fn set_parent(&mut self, parent: PortNo) { self.parent = parent; }
	pub fn get_mask(&self) -> Mask { self.mask }
	pub fn set_mask(&mut self, mask: Mask) { self.mask = mask; }
	pub fn get_other_indices(&self) -> [TableIndex; MAX_PORTS as usize] { self.other_indices }
	pub fn set_other_index(&mut self, port_index: PortNo, other_index: TableIndex) -> Result<(),RoutingTableEntryError> {
		{
			match self.other_indices.get(port_index as usize) {
				Some(other) => (),
				None => return Err(RoutingTableEntryError::Port(PortError::new(port_index)))
			};
		}
		self.other_indices[port_index as usize] = other_index;
		Ok(())
	}
	pub fn update_parent(&self, parent: u8, other_index: u32) -> Result<RoutingTableEntry,RoutingTableEntryError> {
		if parent > MAX_PORTS as u8 { return Err(RoutingTableEntryError::Port(PortError::new(parent))); }
		let mut indices = self.other_indices.clone();
		indices[parent as usize] = other_index;
		Ok(RoutingTableEntry { index: self.index, parent: parent, inuse: self.inuse, mask: self.mask,
							other_indices: indices })
	}
	pub fn update_children(&mut self, child: u8, other_index: u32) -> Result<RoutingTableEntry,RoutingTableEntryError> {
		let mut indices = self.other_indices.clone();
		let child_mask = match Mask::new(child) {
			Ok(m) => m,
			Err(_) => return Err(RoutingTableEntryError::Port(PortError::new(child)))
		};
		self.mask = self.mask.or(child_mask);
		indices[child as usize] = other_index;
		Ok(RoutingTableEntry { index: self.index, parent: self.parent, inuse: self.inuse, 
				mask: self.mask, other_indices: indices })
	}
}
impl fmt::Display for RoutingTableEntry {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("\n{:6}", self.index);
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
