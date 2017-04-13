use std::fmt;
use config::MAX_PORTS;
use nalcell::PortNumber;
use name::{TreeID, PortID, NameError};
use port::Port;
use routing_table_entry::RoutingTableEntry;

#[derive(Debug, Clone)]
pub struct Traph {
	tree_id: TreeID,
	table_entry: RoutingTableEntry,
	elements: Box<[TraphElement]>,
}
impl Traph {
	pub fn new(tree_id: TreeID, table_entry: RoutingTableEntry) -> Result<Traph, TraphError> {
		let default = TraphElement::new(0, table_entry.get_index(), 0);
		let mut elements = Vec::new();
		for _ in 0..MAX_PORTS { elements.push(default.clone()); }
		Ok(Traph { tree_id: tree_id, table_entry: table_entry, elements: elements.into_boxed_slice() })
	}
	pub fn get_tree_id(&self) -> TreeID { self.tree_id.clone() }
	pub fn get_table_index(&self) -> u32 { self.table_entry.get_index() }
	pub fn add_element(&mut self, port_number: PortNumber, my_index: u32, other_index: u32) {
		let port_no = port_number.get_port_no();
		let element = TraphElement::new(port_no, my_index, other_index);
		self.elements[port_no as usize] = element;
	}
	pub  fn get_other_indices(&self) -> [u32; MAX_PORTS as usize] {
		let mut indices = [0; MAX_PORTS as usize];
		for element in self.elements.iter() {
			indices[element.get_port_no() as usize] = element.get_other_index();
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
			s = s + &format!("\nPort Index Other Connected Broken Status Hops Path"); 
			for element in self.elements.iter() { s = s + &element.stringify(); }
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
#[derive(Debug)]
pub enum TraphError {
	Name(NameError),
	Lookup(LookupError),
}
impl Error for TraphError {
	fn description(&self) -> &str {
		match *self {
			TraphError::Name(ref err) => err.description(),
			TraphError::Lookup(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			TraphError::Name(ref err) => Some(err),
			TraphError::Lookup(ref err) => Some(err),
		}
	}
}
impl fmt::Display for TraphError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			TraphError::Name(ref err) => write!(f, "Traph Name Error caused by {}", err),
			TraphError::Lookup(ref err) => write!(f, "Traph Lookup Error caused by {}", err),
		}
	}
}
#[derive(Debug)]
pub struct LookupError { msg: String }
impl LookupError { 
	pub fn new(port_id: PortID) -> LookupError {
		LookupError { msg: format!("No traph entry for port {}", port_id) }
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

#[derive(Debug, Clone)]
struct TraphElement {
	port_no: u8,
	my_index: u32,
	other_index: u32,
	is_connected: bool,
	is_broken: bool,
	status: PortStatus,
	hops: usize,
	path: Option<PortNumber> 
}
impl TraphElement {
	fn new(port_no: u8, my_index: u32, other_index: u32) -> TraphElement {
		TraphElement { port_no: port_no,  my_index: my_index, other_index: other_index, is_connected: true,
					is_broken: false, status: PortStatus::Pruned, hops: 0, path: None } 
	}
	fn stringify(&self) -> String {
		format!("\n{:4} {:6} {:5} {:9} {:6} {:6} {:4} {:?}", self.port_no, self.my_index, self.other_index,
			self.is_connected, self.is_broken, self.status, self.hops, self.path)
	}
	fn get_port_no(&self) -> u8 { self.port_no }
	fn get_other_index(&self) -> u32 { self.other_index }
	fn is_connected(&self) -> bool { self.is_connected }
	fn set_connected(&mut self) { self.is_connected = true; }
	fn set_disconnected(&mut self) { self.is_connected = false; }
	
}