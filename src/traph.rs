use std::fmt;
use std::collections::BTreeSet;
use config::{MAX_PORTS, PathLength, PortNo, TableIndex};
use name::{TreeID, PortID};
use port::Port;
use routing_table_entry::RoutingTableEntry;
use utility::{Mask, Path, PortNumber, UtilityError};

#[derive(Debug, Clone)]
pub struct Traph {
	tree_id: TreeID,
	my_index: TableIndex,
	table_entry: RoutingTableEntry,
	elements: Box<[TraphElement]>,
}
//#[deny(unused_must_use)] Need to figure out get_all_hops and get_other_indices with this enabled
impl Traph {
	pub fn new(tree_id: TreeID, table_entry: RoutingTableEntry) -> Result<Traph, TraphError> {
		let mut elements = Vec::new();
		for i in 0..MAX_PORTS { 
			elements.push(TraphElement::new(false, i as u8, 0, PortStatus::Pruned, 0, None)); 
		}
		Ok(Traph { tree_id: tree_id, table_entry: table_entry, my_index: table_entry.get_index(),
				elements: elements.into_boxed_slice() })
	}
	pub fn get_tree_id(&self) -> TreeID { self.tree_id.clone() }
	pub fn get_table_index(&self) -> TableIndex { self.table_entry.get_index() }
	pub fn add_child(&mut self, port_number: PortNumber) -> Result<RoutingTableEntry, TraphError> {
		let port_no = port_number.get_port_no();
		if let Some(element) = self.elements.get_mut(port_no as usize) {
			element.set_status(PortStatus::Child);
			self.table_entry.or_with_mask(Mask::new(port_no)?);
			Ok(self.table_entry)
		} else {
			return Err(TraphError::Lookup(LookupError::new(port_number)))
		}
	}
	fn get_all_hops(&self) -> BTreeSet<PathLength> {
		let mut set = BTreeSet::new();
		//self.elements.iter().map(|e| set.insert(e.get_hops()));
		for e in self.elements.iter() {
			set.insert(e.get_hops());
		}
		set
	}
	pub fn add_element(&mut self, port_number: PortNumber, my_index: TableIndex, other_index: TableIndex,
			port_status: PortStatus, hops: PathLength, path: Option<Path>) {
		let port_no = port_number.get_port_no();
		let element = TraphElement::new(true, port_no, other_index, port_status, hops, path);
		self.elements[port_no as usize] = element;
	}
	pub  fn get_other_indices(&self) -> [TableIndex; MAX_PORTS as usize] {
		let mut indices = [0; MAX_PORTS as usize];
		// Not sure why map gives warning about unused result
		//self.elements.iter().map(|e| indices[e.get_port_no() as usize] = e.get_other_index());
		for e in self.elements.iter() {
			indices[e.get_port_no() as usize] = e.get_other_index();
		}
		indices
	}
	pub fn set_connected(&mut self, port_no: PortNumber) -> Result<(), TraphError> { 
		self.set_connected_state(port_no, true); 
		Ok(())
	}
	pub fn set_disconnected(&mut self, port_no: PortNumber) -> Result<(), TraphError> { 
		self.set_connected_state(port_no, false); 
		Ok(())
	}
	fn set_connected_state(&mut self, port_no: PortNumber, state: bool) -> Result<(),TraphError> {
		if state { self.elements[port_no.get_port_no() as usize].set_connected(); }
		else     { self.elements[port_no.get_port_no() as usize].set_disconnected(); }
		Ok(())
	}
}
impl fmt::Display for Traph {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("\nTraph for TreeID {}", self.tree_id);
		let mut connected = false;
		for element in self.elements.iter() { if element.is_connected() { connected = true; } }
		if connected {
			s = s + &format!("\nPort Other Connected Broken Status Hops Path");
			// Can't replace with map() because s gets moved into closure 
			for element in self.elements.iter() { 
				if element.is_connected() { s = s + &format!("{}",element);} 
			}
		} else {
			s = s + &format!("\nNo entries yet for this tree"); 
		}
		write!(f, "{}", s) 
	}
}
#[derive(Debug, Copy, Clone)]
pub enum PortStatus {
	Parent,
	Child,
	Pruned
}
impl fmt::Display for PortStatus {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			PortStatus::Parent => write!(f, "Parent"),
			PortStatus::Child  => write!(f, "Child "),
			PortStatus::Pruned => write!(f, "Pruned")
		}
	}
}
// Errors
use std::error::Error;
use name::NameError;
#[derive(Debug)]
pub enum TraphError {
	Name(NameError),
	Lookup(LookupError),
	Utility(UtilityError)
}
impl Error for TraphError {
	fn description(&self) -> &str {
		match *self {
			TraphError::Name(ref err) => err.description(),
			TraphError::Lookup(ref err) => err.description(),
			TraphError::Utility(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			TraphError::Name(ref err) => Some(err),
			TraphError::Lookup(ref err) => Some(err),
			TraphError::Utility(ref err) => Some(err),
		}
	}
}
impl fmt::Display for TraphError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			TraphError::Name(ref err) => write!(f, "Traph Name Error caused by {}", err),
			TraphError::Lookup(ref err) => write!(f, "Traph Lookup Error caused by {}", err),
			TraphError::Utility(ref err) => write!(f, "Traph Utility Error caused by {}", err),
		}
	}
}
#[derive(Debug)]
pub struct LookupError { msg: String }
impl LookupError { 
	pub fn new(port_number: PortNumber) -> LookupError {
		LookupError { msg: format!("No traph entry for port {}", port_number) }
	}
}
impl Error for LookupError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for LookupError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<LookupError> for TraphError {
	fn from(err: LookupError) -> TraphError { TraphError::Lookup(err) }
}
impl From<NameError> for TraphError {
	fn from(err: NameError) -> TraphError { TraphError::Name(err) }
}
impl From<UtilityError> for TraphError {
	fn from(err: UtilityError) -> TraphError { TraphError::Utility(err) }
}

#[derive(Debug, Clone)]
struct TraphElement {
	port_no: PortNo,
	other_index: TableIndex,
	is_connected: bool,
	is_broken: bool,
	status: PortStatus,
	hops: PathLength,
	path: Option<Path> 
}
#[deny(unused_must_use)]
impl TraphElement {
	fn new(is_connected: bool, port_no: PortNo, other_index: TableIndex, 
			status: PortStatus, hops: PathLength, path: Option<Path>) -> TraphElement {
		TraphElement { port_no: port_no,  other_index: other_index, 
			is_connected: is_connected, is_broken: false, status: status, 
			hops: hops, path: path } 
	}
	fn get_port_no(&self) -> PortNo { self.port_no }
	fn get_hops(&self) -> PathLength { self.hops }
	fn get_other_index(&self) -> TableIndex { self.other_index }
	fn is_connected(&self) -> bool { self.is_connected }
	fn set_connected(&mut self) { self.is_connected = true; }
	fn set_disconnected(&mut self) { self.is_connected = false; }
	fn set_status(&mut self, status: PortStatus) { self.status = status; }	
}
impl fmt::Display for TraphElement {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let mut s = format!("\n{:4} {:5} {:9} {:6} {:6} {:4}", 
			self.port_no, self.other_index, self.is_connected, 
			self.is_broken, self.status, self.hops);
		match self.path {
			Some(p) => s = s + &format!(" {:4}", p.get_port_no()),
			None    => s = s + &format!(" None")
		}
		write!(f, "{}", s)
	}
}